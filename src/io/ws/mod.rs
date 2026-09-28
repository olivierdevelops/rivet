//! WebSocket surface decoding (`/v1/ws`, subprotocol `rivet.v1`): client text
//! frames → [`WsFrame`]. Server frames encode with `WsFrame::to_json`.
//!
//! ```text
//!  {"type":"request","ref":"c1","id":"demo.add","params":{…}}   {"type":"input","ref":"c1","seq":1,"data":…}
//!  {"type":"finish_input","ref":"c1"}                           {"type":"cancel","ref":"c1"}
//! ```

use crate::domain::serve::{WsFrame, WsFrameType};
use crate::domain::{RivetError, Value};
use serde_json::Value as Json;

/// Parse one client frame. Failures carry the ref when it could be read so the
/// error frame can be correlated.
pub fn parse_client_frame(text: &str) -> Result<WsFrame, (String, RivetError)> {
    let bad = |r: &str, msg: String| {
        (
            r.to_string(),
            RivetError::validation("validation.frame", msg),
        )
    };
    let j: Json =
        serde_json::from_str(text).map_err(|e| bad("", format!("frame is not JSON: {e}")))?;
    let r = j
        .get("ref")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_string();
    if !j.is_object() {
        return Err(bad("", "frame must be a JSON object".into()));
    }
    if r.is_empty() {
        return Err(bad("", "frame needs a client-chosen `ref`".into()));
    }
    let kind = j
        .get("type")
        .and_then(Json::as_str)
        .and_then(WsFrameType::parse)
        .ok_or_else(|| {
            bad(
                &r,
                "type must be request, input, finish_input or cancel".into(),
            )
        })?;
    let mut f = WsFrame::empty(kind, &r);
    match kind {
        WsFrameType::Request => {
            f.id = Some(
                j.get("id")
                    .and_then(Json::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| bad(&r, "request frame needs `id`".into()))?
                    .to_string(),
            );
            f.params = j.get("params").map(Value::from_json);
            f.restrict = j.get("restrict").map(Value::from_json);
        }
        WsFrameType::Input => {
            f.seq = Some(
                j.get("seq")
                    .and_then(Json::as_u64)
                    .ok_or_else(|| bad(&r, "input frame needs integer `seq`".into()))?,
            );
            f.data = Some(j.get("data").map(Value::from_json).unwrap_or(Value::Null));
        }
        _ => {}
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_demo_frames() {
        for line in include_str!("../../../docs/demos/01-catalog/requests/ws-frames.jsonl").lines()
        {
            let f = parse_client_frame(line).unwrap();
            assert_eq!(f.kind, WsFrameType::Request);
        }
        let (r, e) = parse_client_frame(r#"{"type":"bogus","ref":"x"}"#).unwrap_err();
        assert_eq!((r.as_str(), e.code.as_str()), ("x", "validation.frame"));
        assert!(parse_client_frame("nope").is_err());
        let f = parse_client_frame(r#"{"type":"input","ref":"c3","seq":1,"data":"hi"}"#).unwrap();
        assert_eq!(f.seq, Some(1));
    }
}
