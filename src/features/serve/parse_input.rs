use crate::domain::contracts::MAX_DEADLINE_MS;
use crate::domain::envelope::{INPUT_ENVELOPE, InputEnvelope, RawInput};
use crate::domain::{RivetError, RivetResult, Value};
use serde_json::Value as Json;

fn refuse(message: impl Into<String>) -> RivetError {
    RivetError::validation(INPUT_ENVELOPE, message)
}

/// `key` or its deprecated `alias`; both together is refused, the alias only when `legacy_ok`.
fn pick<'a>(
    body: &'a serde_json::Map<String, Json>,
    key: &str,
    alias: &str,
    legacy_ok: bool,
    aliases: &mut Vec<String>,
) -> RivetResult<Option<&'a Json>> {
    let main = body.get(key);
    let old = body.get(alias);
    match (main, old) {
        (Some(_), Some(_)) => {
            Err(
                refuse(format!("use `{key}` or the deprecated `{alias}`, not both")).with_details(
                    Value::object([("key", Value::text(key)), ("alias", Value::text(alias))]),
                ),
            )
        }
        (Some(v), None) => Ok(Some(v)),
        (None, Some(v)) if legacy_ok => {
            aliases.push(alias.to_string());
            Ok(Some(v))
        }
        (None, Some(_)) => Err(
            refuse(format!("`{alias}` was removed; send `{key}` instead")).with_hint(format!(
                "rename `{alias}` to `{key}` (0.2.0 input envelope)"
            )),
        ),
        (None, None) => Ok(None),
    }
}

// vhco:usecase serve.parse_input(input: RawInput) -> InputEnvelope
// vhco:label Parse input envelope
// vhco:about Turns one JSON request body (HTTP /v1/request and /v1/requests, a WebSocket request frame, the MCP rivet.request and rivet.sessions.open arguments, `rivet request --input FILE|-` / `--data`, the library's Runtime::call_json and the FFI) into the one InputEnvelope {operation, data, deadline_ms?, restrict?, stream?}; the 0.1.0 keys id/params are deprecated aliases through 0.2.x and are reported so each surface can emit its deprecation signal.
// vhco:example input={body:{operation:"users.get", data:{id:42}}, legacy_ok:true} => { "operation": "users.get", "data": {"id": 42}, "aliases": [] }
// vhco:example input={body:{id:"users.get", params:{id:42}}, legacy_ok:true} => { "operation": "users.get", "data": {"id": 42}, "aliases": ["id", "params"] }
pub fn parse_input(input: RawInput) -> RivetResult<InputEnvelope> {
    // vhco:todo object_only -- the body must be a JSON object (anything else is validation.input_envelope); an `error` key is refused (an input envelope never carries an error); unknown keys are ignored as in 0.1.0
    // vhco:step shape as_object -- a non-object body is refused before any key is read
    // vhco:error not_object -- the body is not a JSON object or carries an `error` key => validation.input_envelope (422, exit 2) returns
    let Some(body) = input.body.as_object() else {
        return Err(refuse("the input envelope must be a JSON object"));
    };
    // vhco:step no_error body.get -- `error` belongs to output envelopes only
    if body.contains_key("error") {
        return Err(refuse(
            "an input envelope has no `error` key (errors only appear in responses)",
        ));
    }
    let mut aliases = Vec::new();
    // vhco:todo operation_alias -- `operation` (non-empty string) names the operation; the alias `id` only when legacy_ok; both → validation.input_envelope; neither → validation.required with details.field = "operation"; a non-string value → validation.input_envelope
    // vhco:step operation pick -- `operation`, else the deprecated `id` (recorded in aliases)
    // vhco:error mixed_keys -- `operation` with `id`, or `data` with `params` => validation.input_envelope returns
    // vhco:error no_operation -- neither `operation` nor `id` => validation.required (details.field operation) returns
    let operation = match pick(body, "operation", "id", input.legacy_ok, &mut aliases)? {
        None => {
            return Err(RivetError::validation(
                "validation.required",
                "the input envelope needs `operation`",
            )
            .with_details(Value::object([("field", Value::text("operation"))])));
        }
        Some(Json::String(s)) if !s.is_empty() => s.clone(),
        Some(Json::String(_)) => {
            return Err(RivetError::validation(
                "validation.required",
                "`operation` must not be empty",
            )
            .with_details(Value::object([("field", Value::text("operation"))])));
        }
        Some(_) => return Err(refuse("`operation` must be a string")),
    };
    // vhco:todo data_alias -- `data` is the operation input and defaults to {} when absent or null; the alias `params` only when legacy_ok; `data` with `params` → validation.input_envelope
    // vhco:step data pick -- `data`, else the deprecated `params`; absent or null becomes {}
    let data = match pick(body, "data", "params", input.legacy_ok, &mut aliases)? {
        None | Some(Json::Null) => Value::Object(Vec::new()),
        Some(v) => Value::from_json(v),
    };
    // vhco:todo options -- deadline_ms (non-negative integer, clamped to 1..=600000), restrict (passed through; the runtime validates it), stream and pretty (bools); a wrong type → validation.input_envelope naming the key
    // vhco:step options body.get -- typed optional keys; the deadline is clamped like 0.1.0
    let deadline_ms = match body.get("deadline_ms") {
        None | Some(Json::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .ok_or_else(|| refuse("`deadline_ms` must be a non-negative integer"))?
                .clamp(1, MAX_DEADLINE_MS),
        ),
    };
    let flag = |k: &str| -> RivetResult<Option<bool>> {
        match body.get(k) {
            None | Some(Json::Null) => Ok(None),
            Some(Json::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(refuse(format!("`{k}` must be true or false"))),
        }
    };
    let stream = flag("stream")?;
    let pretty = flag("pretty")?;
    let restrict = body
        .get("restrict")
        .filter(|r| !r.is_null())
        .map(Value::from_json);
    // vhco:todo report_aliases -- return the InputEnvelope with `aliases` listing every deprecated key used ("id", "params") so the surface emits its deprecation signal (HTTP Deprecation: true + access log deprecated=1, CLI warning on stderr, trace note); pure, no ports
    // vhco:step build InputEnvelope -- the one input shape every surface dispatches
    Ok(InputEnvelope {
        operation,
        data,
        deadline_ms,
        restrict,
        stream,
        pretty,
        aliases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(j: Json) -> RivetResult<InputEnvelope> {
        parse_input(RawInput::new(j))
    }

    // vhco:test serve.parse_input -- {operation,data} parses; data defaults to {}; options typed and clamped; aliases reported; mixing, `error`, non-objects and missing operation refused
    #[test]
    fn envelope_aliases_and_refusals() {
        let e = parse(
            json!({"operation": "demo.add", "data": {"a": 2}, "deadline_ms": 0, "stream": true}),
        )
        .unwrap();
        assert_eq!(e.operation, "demo.add");
        assert_eq!(e.data.to_json(), json!({"a": 2}));
        assert_eq!(
            (e.deadline_ms, e.stream, e.is_legacy()),
            (Some(1), Some(true), false)
        );
        let d = parse(json!({"operation": "demo.list"})).unwrap();
        assert_eq!(d.data.to_json(), json!({}));
        let l = parse(json!({"id": "demo.add", "params": {"a": 1}})).unwrap();
        assert_eq!(
            (l.operation.as_str(), l.aliases.clone()),
            ("demo.add", vec!["id".to_string(), "params".to_string()])
        );
        for bad in [
            json!({"operation": "a", "id": "a"}),
            json!({"operation": "a", "data": {}, "params": {}}),
            json!({"operation": "a", "error": {}}),
            json!([1]),
            json!({"operation": 3}),
            json!({"operation": "a", "stream": "yes"}),
            json!({"operation": "a", "deadline_ms": -1}),
        ] {
            assert_eq!(
                parse(bad.clone()).unwrap_err().code,
                INPUT_ENVELOPE,
                "{bad}"
            );
        }
        let missing = parse(json!({"data": {}})).unwrap_err();
        assert_eq!(missing.code, "validation.required");
        assert_eq!(
            missing.details.get("field").and_then(Value::as_str),
            Some("operation")
        );
        let strict = parse_input(RawInput {
            body: json!({"id": "a"}),
            legacy_ok: false,
        })
        .unwrap_err();
        assert_eq!(strict.code, INPUT_ENVELOPE);
        assert!(strict.hint.is_some());
    }
}
