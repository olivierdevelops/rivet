//! HTTP surface encoding: JSON bodies, SSE framing, catalog JSON and error
//! statuses. Handlers live in `orchestrator::setup_http`; this module only
//! decodes and encodes. Request bodies are parsed into an InputEnvelope by the
//! `serve.parse_input` use case (called from the orchestrator).
//!
//! ```text
//!  POST /v1/request {operation, data}   (?pretty=true indents JSON answers)
//!     Accept: application/json  ─▶ ResponseEnvelope (status ok|error|cancelled) with the registry HTTP status
//!     Accept: text/event-stream ─▶ id: N / event: data|result / data: {record with seq}
//! ```

pub mod poll;

use crate::domain::RivetError;
use crate::domain::contracts::{Completion, DataEvent, RegistryEntry};
use crate::domain::envelope::{PRETTY_STREAM, ResponseEnvelope};
use serde_json::{Value as Json, json};

/// Code for an unparseable JSON body (HTTP 400, not 422).
pub const MALFORMED_JSON: &str = "validation.malformed_json";

/// Host cap on a caller-requested `deadline_ms` (10 minutes).
pub const MAX_REQUEST_DEADLINE_MS: u64 = 600_000;

/// Parse a JSON body; malformed JSON is `validation.malformed_json` (400). An
/// empty body is `{}`. The shape is checked by the caller (`serve.parse_input`
/// for request envelopes).
pub fn parse_json_body(bytes: &[u8]) -> Result<Json, RivetError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(json!({}));
    }
    serde_json::from_slice(bytes).map_err(|e| {
        RivetError::validation(
            MALFORMED_JSON,
            format!("request body is not valid JSON: {e}"),
        )
    })
}

/// Parse a JSON object body; malformed JSON is `validation.malformed_json` (400).
pub fn parse_json_object(bytes: &[u8]) -> Result<Json, RivetError> {
    let j = parse_json_body(bytes)?;
    if !j.is_object() {
        return Err(RivetError::validation(
            "validation.params",
            "request body must be a JSON object",
        ));
    }
    Ok(j)
}

/// Registry status, except malformed JSON and pretty-on-a-stream, which are 400.
pub fn error_status(e: &RivetError) -> u16 {
    if e.code == MALFORMED_JSON || e.code == PRETTY_STREAM {
        400
    } else {
        e.http_status()
    }
}

/// The error answer: a ResponseEnvelope with status error (or cancelled).
pub fn error_body(operation: Option<&str>, e: &RivetError) -> Json {
    ResponseEnvelope::from_error(operation, e).to_json()
}

/// Whether `?pretty=true` (or `pretty=1`) is in a raw query string.
pub fn wants_pretty(query: Option<&str>) -> bool {
    query.is_some_and(|q| {
        q.split('&').any(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, "true"));
            k == "pretty" && matches!(v, "true" | "1" | "")
        })
    })
}

/// Whether the caller asked for SSE framing.
pub fn wants_sse(accept: Option<&str>) -> bool {
    accept.is_some_and(|a| {
        a.split(',').any(|p| {
            p.trim()
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("text/event-stream")
        })
    })
}

/// One SSE event: `id: N`, `event: NAME`, `data: JSON`, blank line.
pub fn sse_event(seq: u64, event: &str, data: &Json) -> String {
    format!("id: {seq}\nevent: {event}\ndata: {data}\n\n")
}

/// `event: data` with the item's `type: data` record.
pub fn sse_data(ev: &DataEvent) -> String {
    sse_event(ev.seq, "data", &ResponseEnvelope::from_data(ev).to_json())
}

/// `event: result` with the terminal record (status ok) and its sequence.
pub fn sse_result(c: &Completion, seq: u64) -> String {
    let r = ResponseEnvelope::from_completion(c).with_seq(seq);
    sse_event(seq, "result", &r.to_json())
}

/// `event: result` with the terminal record (status error or cancelled).
pub fn sse_error(
    request_id: &str,
    trace_id: &str,
    operation: &str,
    e: &RivetError,
    seq: u64,
) -> String {
    let mut e = e.clone();
    e.request_id = Some(request_id.to_string());
    e.trace_id = Some(trace_id.to_string());
    let r = ResponseEnvelope::from_error(Some(operation), &e)
        .with_seq(seq)
        .with_data_count(seq.saturating_sub(1));
    sse_event(seq, "result", &r.to_json())
}

/// `GET /v1/operations` payload (the envelope's `data`).
pub fn catalog_json<'a>(entries: impl Iterator<Item = &'a RegistryEntry>) -> Json {
    let ops: Vec<Json> = entries.map(RegistryEntry::summary_json).collect();
    json!({"operations": ops, "next_cursor": null})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_and_accept() {
        let b = parse_json_body(br#"{"operation":"demo.add","data":{"a":2}}"#).unwrap();
        assert_eq!(b["operation"], "demo.add");
        assert_eq!(error_status(&parse_json_body(b"{nope").unwrap_err()), 400);
        assert_eq!(parse_json_body(b"  ").unwrap(), json!({}));
        assert_eq!(
            error_status(&RivetError::validation(PRETTY_STREAM, "no")),
            400
        );
        assert!(wants_pretty(Some("a=1&pretty=true")));
        assert!(!wants_pretty(Some("pretty=false")));
        assert!(!wants_pretty(None));
        assert!(wants_sse(Some("application/json, text/event-stream")));
        assert!(!wants_sse(Some("application/json")));
        assert_eq!(
            sse_event(1, "data", &json!(3)),
            "id: 1\nevent: data\ndata: 3\n\n"
        );
    }
}
