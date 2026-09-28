//! WebSocket surface decoding (`/v1/ws`, subprotocol `rivet.v1`): client text
//! frames → [`WsFrame`]. Server frames encode with `WsFrame::to_json` (one
//! ResponseEnvelope record with `ref` first).
//!
//! ```text
//!  {"type":"request","ref":"c1","operation":"demo.add","data":{…}}   {"type":"input","ref":"c1","seq":1,"data":…}
//!  {"type":"finish_input","ref":"c1"}                                {"type":"cancel","ref":"c1"}
//!  (0.1.0 request frames {"id","params"} are deprecated aliases through 0.2.x)
//! ```

use crate::domain::envelope::InputEnvelope;
use crate::domain::serve::{WsFrame, WsFrameType};
use crate::domain::{RivetError, Value};
use serde_json::Value as Json;

/// Parse one client frame. A request frame's envelope keys are decoded by
/// `parse_input` (the `serve.parse_input` use case, injected by the
/// orchestrator). Failures carry the ref when it could be read so the error
/// frame can be correlated.
pub fn parse_client_frame(
    text: &str,
    parse_input: impl Fn(&Json) -> Result<InputEnvelope, RivetError>,
) -> Result<WsFrame, (String, RivetError)> {
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
            // `type` and `ref` sit beside the envelope keys and are ignored by the parser.
            f.input = Some(parse_input(&j).map_err(|e| (r.clone(), e))?);
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

    /// A stand-in for serve.parse_input: `operation` only.
    fn op_only(j: &Json) -> Result<InputEnvelope, RivetError> {
        j.get("operation")
            .and_then(Json::as_str)
            .map(InputEnvelope::new)
            .ok_or_else(|| RivetError::validation("validation.required", "needs operation"))
    }

    #[test]
    fn parses_demo_frames() {
        for line in include_str!("../../../docs/demos/01-catalog/requests/ws-frames.jsonl").lines()
        {
            let f = parse_client_frame(line, op_only).unwrap();
            assert_eq!(f.kind, WsFrameType::Request);
            assert!(f.input.is_some());
        }
        let (r, e) = parse_client_frame(r#"{"type":"bogus","ref":"x"}"#, op_only).unwrap_err();
        assert_eq!((r.as_str(), e.code.as_str()), ("x", "validation.frame"));
        assert!(parse_client_frame("nope", op_only).is_err());
        let (r, e) = parse_client_frame(r#"{"type":"request","ref":"c9"}"#, op_only).unwrap_err();
        assert_eq!((r.as_str(), e.code.as_str()), ("c9", "validation.required"));
        let f = parse_client_frame(
            r#"{"type":"input","ref":"c3","seq":1,"data":"hi"}"#,
            op_only,
        )
        .unwrap();
        assert_eq!(f.seq, Some(1));
    }
}
