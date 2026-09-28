//! HTTP surface encoding: request bodies, SSE framing, catalog JSON and error
//! statuses. Handlers live in `orchestrator::setup_http`; this module only
//! decodes and encodes.
//!
//! ```text
//!  POST /v1/request {id, params}
//!     Accept: application/json  ─▶ 200 Completion | ErrorEnvelope (registry status)
//!     Accept: text/event-stream ─▶ id: N / event: data|result|error / data: {envelope with seq}
//! ```

pub mod poll;

use crate::domain::contracts::{Completion, DataEvent, Envelope, RegistryEntry, error_envelope};
use crate::domain::{RivetError, Value};
use serde_json::{Value as Json, json};

/// Code for an unparseable JSON body (HTTP 400, not 422).
pub const MALFORMED_JSON: &str = "validation.malformed_json";

/// Host cap on a caller-requested `deadline_ms` (10 minutes).
pub const MAX_REQUEST_DEADLINE_MS: u64 = 600_000;

/// `{id, params}` of `POST /v1/request` and `POST /v1/requests`.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestBody {
    pub id: String,
    pub params: Value,
    /// Optional `deadline_ms` (the CLI `--timeout` in `--endpoint` mode), capped
    /// by [`MAX_REQUEST_DEADLINE_MS`].
    pub deadline_ms: Option<u64>,
}

/// Parse a JSON object body; malformed JSON is `validation.malformed_json` (400).
pub fn parse_json_object(bytes: &[u8]) -> Result<Json, RivetError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(json!({}));
    }
    let j: Json = serde_json::from_slice(bytes).map_err(|e| {
        RivetError::validation(
            MALFORMED_JSON,
            format!("request body is not valid JSON: {e}"),
        )
    })?;
    if !j.is_object() {
        return Err(RivetError::validation(
            "validation.params",
            "request body must be a JSON object",
        ));
    }
    Ok(j)
}

pub fn parse_request_body(bytes: &[u8]) -> Result<RequestBody, RivetError> {
    let j = parse_json_object(bytes)?;
    let id = j
        .get("id")
        .and_then(Json::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            RivetError::validation("validation.required", "request body needs `id`")
                .with_details(Value::object([("field", Value::text("id"))]))
        })?
        .to_string();
    let params = j.get("params").map(Value::from_json).unwrap_or(Value::Null);
    let deadline_ms = j
        .get("deadline_ms")
        .and_then(Json::as_u64)
        .map(|d| d.clamp(1, MAX_REQUEST_DEADLINE_MS));
    Ok(RequestBody {
        id,
        params,
        deadline_ms,
    })
}

/// Registry status, except malformed JSON which is 400.
pub fn error_status(e: &RivetError) -> u16 {
    if e.code == MALFORMED_JSON {
        400
    } else {
        e.http_status()
    }
}

/// ErrorEnvelope body for an error.
pub fn error_body(e: &RivetError) -> Json {
    error_envelope(
        e.request_id.as_deref().unwrap_or(""),
        e.trace_id.as_deref().unwrap_or(""),
        e,
    )
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

pub fn sse_data(ev: &DataEvent) -> String {
    sse_event(ev.seq, "data", &Envelope::Data(ev.clone()).to_json())
}

pub fn sse_result(c: &Completion, seq: u64) -> String {
    let mut j = Envelope::Result(c.clone()).to_json();
    j["seq"] = json!(seq);
    sse_event(seq, "result", &j)
}

pub fn sse_error(request_id: &str, trace_id: &str, e: &RivetError, seq: u64) -> String {
    let j = Envelope::Error {
        request_id: request_id.to_string(),
        trace_id: trace_id.to_string(),
        seq,
        error: Box::new(e.clone()),
    }
    .to_json();
    sse_event(seq, "error", &j)
}

/// `GET /v1/operations` body.
pub fn catalog_json<'a>(entries: impl Iterator<Item = &'a RegistryEntry>) -> Json {
    let ops: Vec<Json> = entries.map(RegistryEntry::summary_json).collect();
    json!({"operations": ops, "next_cursor": null})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_and_accept() {
        let b = parse_request_body(br#"{"id":"demo.add","params":{"a":2}}"#).unwrap();
        assert_eq!(b.id, "demo.add");
        assert_eq!(
            error_status(&parse_request_body(b"{nope").unwrap_err()),
            400
        );
        assert_eq!(
            parse_request_body(b"{}").unwrap_err().code,
            "validation.required"
        );
        assert!(wants_sse(Some("application/json, text/event-stream")));
        assert!(!wants_sse(Some("application/json")));
        assert_eq!(
            sse_event(1, "data", &json!(3)),
            "id: 1\nevent: data\ndata: 3\n\n"
        );
    }
}
