//! Polling routes decoding: `/v1/requests/{id}/events` query and the input body.

use super::parse_json_object;
use crate::domain::{RivetError, Value};
use serde_json::Value as Json;

/// `after_seq`, `wait_ms` and `max_events` from the events query string.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EventsQuery {
    pub after_seq: u64,
    pub wait_ms: Option<u32>,
    pub max_events: Option<u32>,
}

pub fn parse_events_query(query: Option<&str>) -> Result<EventsQuery, RivetError> {
    let mut q = EventsQuery::default();
    for pair in query.unwrap_or("").split('&').filter(|s| !s.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let num = |v: &str| {
            v.parse::<u64>().map_err(|_| {
                RivetError::validation(
                    "validation.type",
                    format!("query `{k}` must be a non-negative integer"),
                )
            })
        };
        match k {
            "after_seq" => q.after_seq = num(v)?,
            "wait_ms" => q.wait_ms = Some(num(v)?.min(u32::MAX as u64) as u32),
            "max_events" => q.max_events = Some(num(v)?.min(u32::MAX as u64) as u32),
            _ => {}
        }
    }
    Ok(q)
}

/// `{send_seq, data}` of `POST /v1/requests/{id}/input`.
pub fn parse_input_body(bytes: &[u8]) -> Result<(u64, Value), RivetError> {
    let j = parse_json_object(bytes)?;
    let seq = j.get("send_seq").and_then(Json::as_u64).ok_or_else(|| {
        RivetError::validation("validation.required", "input body needs integer `send_seq`")
    })?;
    let data = j.get("data").map(Value::from_json).unwrap_or(Value::Null);
    Ok((seq, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_and_input() {
        let q = parse_events_query(Some("after_seq=2&wait_ms=5000")).unwrap();
        assert_eq!(
            (q.after_seq, q.wait_ms, q.max_events),
            (2, Some(5000), None)
        );
        assert!(parse_events_query(Some("after_seq=x")).is_err());
        assert_eq!(
            parse_input_body(br#"{"send_seq":1,"data":"hi"}"#).unwrap(),
            (1, Value::text("hi"))
        );
    }
}
