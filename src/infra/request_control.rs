//! Cancellation signals for running top-level requests.
//!
//! ```text
//!  start(id, owner, token) ─▶ running ─signal─▶ cancelling (token fired: the run unwinds,
//!        │                                         closes its handles, then finish())
//!        └─ finish(id, state) ─▶ recent (bounded) for late cancel / lookup calls
//!  cancel_all() (serve shutdown) fires every running token
//! ```

use crate::domain::cancel::{CancelReason, CancelToken};
use crate::domain::ports::{RequestControl, RequestState};
use std::collections::HashMap;
use std::sync::Mutex;

/// Terminal states kept for late cancel calls (bounded).
const RECENT: usize = 1024;

/// (owner principal, cancellation token) per running request.
type Running = HashMap<String, (String, CancelToken)>;

// vhco:infra request_control satisfies RequestControl
#[derive(Default)]
pub struct RunningRequests {
    running: Mutex<Running>,
    recent: Mutex<Vec<(String, RequestState)>>,
}

impl RunningRequests {
    /// Register a running request and its structured cancellation token.
    pub fn start(&self, request_id: &str, owner: &str, token: CancelToken) {
        if let Ok(mut m) = self.running.lock() {
            m.insert(request_id.to_string(), (owner.to_string(), token));
        }
    }

    pub fn finish(&self, request_id: &str, state: &str) {
        let owner = self
            .running
            .lock()
            .ok()
            .and_then(|mut m| m.remove(request_id))
            .map(|(o, _)| o);
        if let (Some(owner), Ok(mut r)) = (owner, self.recent.lock()) {
            if r.len() >= RECENT {
                r.remove(0);
            }
            r.push((
                request_id.to_string(),
                RequestState {
                    owner,
                    state: state.to_string(),
                },
            ));
        }
    }

    /// Host shutdown: cancel every running top-level request.
    pub fn cancel_all(&self) -> usize {
        let tokens: Vec<CancelToken> = self
            .running
            .lock()
            .map(|m| m.values().map(|(_, t)| t.clone()).collect())
            .unwrap_or_default();
        for t in &tokens {
            t.cancel(CancelReason::Cancelled);
        }
        tokens.len()
    }

    /// Requests still running (shutdown drains until this reaches zero).
    pub fn running_count(&self) -> usize {
        self.running.lock().map(|m| m.len()).unwrap_or(0)
    }
}

impl RequestControl for RunningRequests {
    fn lookup(&self, request_id: &str) -> Option<RequestState> {
        if let Some((owner, token)) = self.running.lock().ok()?.get(request_id) {
            return Some(RequestState {
                owner: owner.clone(),
                state: if token.is_cancelled() {
                    "cancelling"
                } else {
                    "running"
                }
                .into(),
            });
        }
        self.recent
            .lock()
            .ok()?
            .iter()
            .rev()
            .find(|(id, _)| id == request_id)
            .map(|(_, s)| s.clone())
    }

    fn signal(&self, request_id: &str) -> bool {
        let Ok(m) = self.running.lock() else {
            return false;
        };
        match m.get(request_id) {
            Some((_, token)) => {
                token.cancel(CancelReason::Cancelled);
                true
            }
            None => false,
        }
    }
}
