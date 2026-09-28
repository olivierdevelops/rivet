//! Bounded in-memory trace store (default audit sink, PROP-2026-0001 Increment 5)
//! plus the task-local "current effect" scope the interpreter sets around each
//! effect so broker decisions can be attributed to request + source line.
//!
//! ```text
//!  interpreter run_effect ──scope{request, op, line}──▶ adapter / files use case
//!                                                          └─ PolicyEvaluator.evaluate
//!                                                               └─ tracer reads scope ─▶ TraceStore.record
//! ```

use crate::domain::io_manifest::{TraceEvent, TraceQuery, TraceResult};
use crate::domain::ports::TraceStore;
use crate::domain::{RivetError, RivetResult};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::Mutex;

/// Which request/operation/statement an effect attempt belongs to.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EffectScope {
    pub request_id: String,
    pub trace_id: String,
    pub operation_id: String,
    pub line: u32,
}

tokio::task_local! {
    static EFFECT_SCOPE: EffectScope;
}

/// Run `fut` with `scope` as the current effect scope (nests for child requests).
pub async fn with_effect_scope<F: Future>(scope: EffectScope, fut: F) -> F::Output {
    EFFECT_SCOPE.scope(scope, fut).await
}

/// The innermost effect scope, if the caller runs inside one.
pub fn current_effect_scope() -> Option<EffectScope> {
    EFFECT_SCOPE.try_with(|s| s.clone()).ok()
}

/// Default bound on stored events (oldest evicted first).
pub const DEFAULT_TRACE_CAPACITY: usize = 10_000;

struct Inner {
    events: VecDeque<TraceEvent>,
    evicted: HashMap<String, u32>,
}

// vhco:infra trace_store satisfies TraceStore
pub struct MemoryTraceStore {
    capacity: usize,
    inner: Mutex<Inner>,
}

impl MemoryTraceStore {
    pub fn new(capacity: usize) -> MemoryTraceStore {
        MemoryTraceStore {
            capacity: capacity.max(1),
            inner: Mutex::new(Inner {
                events: VecDeque::new(),
                evicted: HashMap::new(),
            }),
        }
    }
}

impl Default for MemoryTraceStore {
    fn default() -> Self {
        MemoryTraceStore::new(DEFAULT_TRACE_CAPACITY)
    }
}

impl TraceStore for MemoryTraceStore {
    fn record(&self, event: TraceEvent) {
        let Ok(mut g) = self.inner.lock() else {
            return;
        };
        if g.events.len() >= self.capacity
            && let Some(old) = g.events.pop_front()
        {
            *g.evicted.entry(old.request_id).or_default() += 1;
        }
        g.events.push_back(event);
    }

    fn read(&self, query: &TraceQuery) -> RivetResult<TraceResult> {
        let g = self
            .inner
            .lock()
            .map_err(|_| RivetError::internal("trace store poisoned"))?;
        let events: Vec<TraceEvent> = g
            .events
            .iter()
            .filter(|e| e.request_id == query.request_id)
            .cloned()
            .collect();
        let gaps = g.evicted.get(&query.request_id).copied().unwrap_or(0);
        if events.is_empty() && gaps == 0 {
            return Err(RivetError::not_found(
                "not_found.trace",
                format!(
                    "no trace for request `{}` in this host's trace store",
                    query.request_id
                ),
            ));
        }
        Ok(TraceResult {
            request_id: query.request_id.clone(),
            events,
            complete: gaps == 0,
            next_cursor: None,
            gaps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(req: &str) -> TraceEvent {
        TraceEvent {
            request_id: req.into(),
            trace_id: "t".into(),
            node_id: None,
            attempt: 1,
            effect_id: None,
            operation_id: "a.b".into(),
            phase: "decision".into(),
            capability: "allow_read".into(),
            access: "read".into(),
            target: "./x".into(),
            decision: "allowed".into(),
            policy_hash: String::new(),
            source: None,
            outcome: serde_json::Value::Null,
        }
    }

    // vhco:test audit.read_trace -- the bounded store evicts oldest events and reports the gap instead of claiming completeness
    #[test]
    fn bounded_store_reports_gaps() {
        let s = MemoryTraceStore::new(2);
        s.record(ev("r1"));
        s.record(ev("r2"));
        s.record(ev("r2"));
        let r1 = s.read(&TraceQuery::new("r1")).unwrap();
        assert!(!r1.complete && r1.events.is_empty() && r1.gaps == 1);
        assert_eq!(s.read(&TraceQuery::new("r2")).unwrap().events.len(), 2);
        assert!(s.read(&TraceQuery::new("r3")).is_err());
    }

    #[tokio::test]
    async fn scope_is_visible_inside_and_absent_outside() {
        assert!(current_effect_scope().is_none());
        let seen = with_effect_scope(
            EffectScope {
                request_id: "req".into(),
                line: 3,
                ..Default::default()
            },
            async { current_effect_scope() },
        )
        .await;
        assert_eq!(seen.unwrap().line, 3);
    }
}
