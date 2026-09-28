//! Structured cancellation (PROP-2026-0001 Increments 2 and 3).
//!
//! ```text
//!  top-level request token ──child()──▶ nested request / concurrent group / DAG / request.stream
//!        │ cancel(Cancelled | Timeout)            (a parent cancel reaches every live child)
//!        ▼
//!  interpreter observes it at every await point and handle operation
//!        ▼
//!  `with` scopes close their handles in reverse order within the cleanup grace
//!        ▼
//!  one terminal `cancelled` / `timeout` error (cleanup failures suppressed on it)
//!        ▼
//!  only after the grace: the remaining future is dropped (last resort)
//! ```

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::Notify;

// vhco:domain CancelReason { cancelled | timeout }
/// Why a scope is being cancelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelReason {
    /// The caller (or a parent scope, `fail fast`, a session cancel, shutdown) cancelled.
    Cancelled,
    /// The request deadline expired.
    Timeout,
}

impl CancelReason {
    pub fn as_str(self) -> &'static str {
        match self {
            CancelReason::Cancelled => "cancelled",
            CancelReason::Timeout => "timeout",
        }
    }
}

struct Inner {
    /// 0 live, 1 cancelled, 2 timeout.
    state: AtomicU8,
    notify: Notify,
    children: Mutex<Vec<Weak<Inner>>>,
}

impl Inner {
    fn fire(&self, reason: CancelReason) {
        let code = match reason {
            CancelReason::Cancelled => 1,
            CancelReason::Timeout => 2,
        };
        if self
            .state
            .compare_exchange(0, code, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        self.notify.notify_waiters();
        let children: Vec<Weak<Inner>> = self
            .children
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default();
        for c in children {
            if let Some(c) = c.upgrade() {
                c.fire(reason);
            }
        }
    }

    fn reason(&self) -> Option<CancelReason> {
        match self.state.load(Ordering::SeqCst) {
            1 => Some(CancelReason::Cancelled),
            2 => Some(CancelReason::Timeout),
            _ => None,
        }
    }
}

// vhco:domain CancelToken { reason?: CancelReason }
/// A cancellation signal shared by one request scope and its children. Cheap
/// to clone; cancelling is idempotent (the first reason wins) and reaches every
/// child token created with [`CancelToken::child`].
#[derive(Clone)]
pub struct CancelToken {
    inner: Arc<Inner>,
}

impl Default for CancelToken {
    fn default() -> Self {
        CancelToken::new()
    }
}

impl std::fmt::Debug for CancelToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CancelToken({:?})", self.reason())
    }
}

/// Tokens carry no data a caller could compare; two requests are equal
/// regardless of their cancellation state.
impl PartialEq for CancelToken {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken {
            inner: Arc::new(Inner {
                state: AtomicU8::new(0),
                notify: Notify::new(),
                children: Mutex::new(Vec::new()),
            }),
        }
    }

    /// A token cancelled whenever this one is (with the same reason); cancelling
    /// the child does not cancel the parent.
    pub fn child(&self) -> CancelToken {
        let child = CancelToken::new();
        if let Ok(mut c) = self.inner.children.lock() {
            c.retain(|w| w.strong_count() > 0);
            c.push(Arc::downgrade(&child.inner));
        }
        // A parent cancelled before the registration is seen here.
        if let Some(r) = self.inner.reason() {
            child.inner.fire(r);
        }
        child
    }

    /// Signal cancellation; later calls keep the first reason.
    pub fn cancel(&self, reason: CancelReason) {
        self.inner.fire(reason);
    }

    pub fn reason(&self) -> Option<CancelReason> {
        self.inner.reason()
    }

    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }

    /// Resolves once the token is cancelled (immediately if it already is).
    pub async fn cancelled(&self) -> CancelReason {
        loop {
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(r) = self.inner.reason() {
                return r;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn first_reason_wins_and_reaches_children() {
        let root = CancelToken::new();
        let child = root.child();
        let grandchild = child.child();
        assert!(!grandchild.is_cancelled());
        let waiter = tokio::spawn({
            let g = grandchild.clone();
            async move { g.cancelled().await }
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        root.cancel(CancelReason::Timeout);
        root.cancel(CancelReason::Cancelled);
        assert_eq!(waiter.await.unwrap(), CancelReason::Timeout);
        assert_eq!(child.reason(), Some(CancelReason::Timeout));
        // A child created after the cancel starts cancelled.
        assert_eq!(root.child().reason(), Some(CancelReason::Timeout));
        // Cancelling a child leaves its parent live.
        let other = CancelToken::new();
        other.child().cancel(CancelReason::Cancelled);
        assert!(!other.is_cancelled());
    }
}
