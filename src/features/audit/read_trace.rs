use super::ports::TraceStore;
use crate::domain::io_manifest::{TraceQuery, TraceResult};
use crate::domain::{RivetError, RivetResult};

/// Upper bound on events returned by one read.
pub const MAX_TRACE_LIMIT: u32 = 10_000;

// vhco:usecase audit.read_trace(input: TraceQuery) -> TraceResult needs TraceStore
// vhco:label Read trace
// vhco:about Returns one request's ordered broker decisions and attempts (each carrying its manifest effect_id) from the host's bounded trace store; reports eviction gaps instead of claiming complete, tamper-proof history.
// vhco:example input={request_id:"req_12"} => { "request_id": "req_12", "attempts": [{ "effect_id": "users.get#2", "decision": "allowed" }], "complete": true }
pub fn read_trace(input: &TraceQuery, store: &dyn TraceStore) -> RivetResult<TraceResult> {
    // vhco:todo authorize_trace -- reject an empty request id and a limit of 0 or above MAX_TRACE_LIMIT as validation (exit 2) before touching the store; traces are only served to the host that recorded them (the CLI is one-shot, so its store is always empty)
    // vhco:error bad_query -- empty request id or out-of-range limit => validation.usage returns
    if input.request_id.trim().is_empty() {
        return Err(RivetError::validation(
            "validation.usage",
            "trace show needs a request ID",
        ));
    }
    if input.limit == 0 || input.limit > MAX_TRACE_LIMIT {
        return Err(RivetError::validation(
            "validation.usage",
            format!("limit must be between 1 and {MAX_TRACE_LIMIT}"),
        ));
    }
    // vhco:todo return_trace -- read the sanitized, ordered events through TraceStore.read (request, trace, attempt, effect_id, policy hash and source preserved; secret values never stored); an unknown request is not_found (exit 4); truncate to `limit` with a next_cursor and keep the store's gap count so evicted history is reported, never hidden
    // vhco:step read store.read -- not_found when the store never saw the request
    // vhco:error unknown_request -- the store holds no such request => not_found.trace (exit 4) returns
    let mut result = store.read(input)?;
    let start: usize = input
        .cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let total = result.events.len();
    let end = (start + input.limit as usize).min(total);
    result.events = result.events.drain(start.min(total)..end).collect();
    result.next_cursor = (end < total).then(|| end.to_string());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::io_manifest::TraceEvent;
    use std::sync::Mutex;

    struct Mem(Mutex<Vec<TraceEvent>>);

    impl TraceStore for Mem {
        fn record(&self, e: TraceEvent) {
            self.0.lock().unwrap().push(e);
        }
        fn read(&self, q: &TraceQuery) -> RivetResult<TraceResult> {
            let ev: Vec<TraceEvent> = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.request_id == q.request_id)
                .cloned()
                .collect();
            if ev.is_empty() {
                return Err(RivetError::not_found("not_found.trace", "none"));
            }
            Ok(TraceResult {
                request_id: q.request_id.clone(),
                events: ev,
                complete: true,
                next_cursor: None,
                gaps: 0,
            })
        }
    }

    fn ev(i: u32) -> TraceEvent {
        TraceEvent {
            request_id: "req_1".into(),
            trace_id: "tr_1".into(),
            node_id: None,
            attempt: i,
            effect_id: Some(format!("a.b#{i}")),
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

    // vhco:test audit.read_trace -- unknown requests are not_found, limits paginate with a cursor, empty ids are validation
    #[test]
    fn reads_paginates_and_rejects() {
        let store = Mem(Mutex::new(vec![ev(1), ev(2), ev(3)]));
        assert_eq!(
            read_trace(&TraceQuery::new("nope"), &store)
                .unwrap_err()
                .code,
            "not_found.trace"
        );
        assert_eq!(
            read_trace(&TraceQuery::new(" "), &store).unwrap_err().code,
            "validation.usage"
        );
        let mut q = TraceQuery::new("req_1");
        q.limit = 2;
        let r = read_trace(&q, &store).unwrap();
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.next_cursor.as_deref(), Some("2"));
        q.cursor = r.next_cursor;
        let r = read_trace(&q, &store).unwrap();
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.events[0].effect_id.as_deref(), Some("a.b#3"));
    }
}
