//! Value ⇄ bytes codecs and argument parsing shared by the socket-like
//! resource handles (`socket.send json V`, `stream.receive json timeout "5s"`).

use crate::domain::files::Codec;
use crate::domain::ir::parse_duration_ms;
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use crate::infra::execution_driver::EvalArg;

/// Encode one application message (JSON is compact: one message, no trailing newline).
pub fn encode(codec: Codec, value: &Value) -> RivetResult<Vec<u8>> {
    Ok(match (codec, value) {
        (Codec::Bytes, Value::Bytes(b)) => b.clone(),
        (Codec::Bytes, other) => {
            return Err(RivetError::validation(
                "validation.codec",
                format!(
                    "the bytes codec needs a bytes value, got {}",
                    other.type_name()
                ),
            ));
        }
        (Codec::Text, Value::Text(s)) => s.clone().into_bytes(),
        (Codec::Text, other) => other.to_display().into_bytes(),
        (Codec::Json, v) => serde_json::to_vec(&v.to_json()).unwrap_or_default(),
    })
}

/// Decode one received message; malformed payloads are `parse` errors.
pub fn decode(codec: Codec, bytes: Vec<u8>) -> RivetResult<Value> {
    match codec {
        Codec::Bytes => Ok(Value::Bytes(bytes)),
        Codec::Text => String::from_utf8(bytes).map(Value::Text).map_err(|_| {
            RivetError::new(
                ErrorKind::Parse,
                "parse.utf8",
                "received payload is not UTF-8 text",
            )
        }),
        Codec::Json => serde_json::from_slice::<serde_json::Value>(&bytes)
            .map(|j| Value::from_json(&j))
            .map_err(|e| {
                RivetError::new(
                    ErrorKind::Parse,
                    "parse.json",
                    format!("received payload is not JSON: {e}"),
                )
            }),
    }
}

/// `json|text|bytes` at `args[i]`.
pub fn codec_at(args: &[EvalArg], i: usize, method: &str) -> RivetResult<Codec> {
    args.get(i)
        .and_then(EvalArg::word)
        .and_then(Codec::parse)
        .ok_or_else(|| {
            RivetError::validation(
                "validation.codec",
                format!("`{method}` needs a codec: json, text or bytes"),
            )
        })
}

/// A value argument (words that are not variables are taken literally as text).
pub fn value_at(args: &[EvalArg], i: usize, method: &str) -> RivetResult<Value> {
    match args.get(i) {
        Some(EvalArg::Value(v)) => Ok(v.clone()),
        Some(EvalArg::Word(w)) => Ok(Value::text(w)),
        None => Err(RivetError::validation(
            "validation.argument",
            format!("`{method}` is missing its value"),
        )),
    }
}

/// Trailing `timeout "D"` in a method call's arguments.
pub fn timeout_arg(args: &[EvalArg]) -> RivetResult<Option<u64>> {
    match args.iter().position(|a| a.word() == Some("timeout")) {
        None => Ok(None),
        Some(p) => duration_value(args.get(p + 1)).map(Some),
    }
}

/// A quoted duration (`"5s"`) as milliseconds.
pub fn duration_value(arg: Option<&EvalArg>) -> RivetResult<u64> {
    arg.and_then(EvalArg::value)
        .and_then(Value::as_str)
        .and_then(parse_duration_ms)
        .ok_or_else(|| {
            RivetError::validation(
                "validation.duration",
                "timeout needs a quoted duration such as \"5s\"",
            )
        })
}

/// A non-negative integer option value.
pub fn uint_value(arg: Option<&EvalArg>, what: &str) -> RivetResult<u64> {
    arg.and_then(EvalArg::value)
        .and_then(Value::as_i64)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| {
            RivetError::validation(
                "validation.option",
                format!("`{what}` needs a non-negative integer"),
            )
        })
}

/// A text option value (`bind "0.0.0.0:5000"`, `alpn "rivet-rpc/1"`).
pub fn text_value(arg: Option<&EvalArg>, what: &str) -> RivetResult<String> {
    match arg {
        Some(EvalArg::Value(Value::Text(s))) => Ok(s.clone()),
        Some(EvalArg::Word(w)) => Ok(w.clone()),
        _ => Err(RivetError::validation(
            "validation.option",
            format!("`{what}` needs a text value"),
        )),
    }
}

/// `true` / `false` option value.
pub fn bool_value(arg: Option<&EvalArg>, what: &str) -> RivetResult<bool> {
    match arg {
        Some(EvalArg::Value(Value::Bool(b))) => Ok(*b),
        Some(EvalArg::Word(w)) if w == "true" => Ok(true),
        Some(EvalArg::Word(w)) if w == "false" => Ok(false),
        _ => Err(RivetError::validation(
            "validation.option",
            format!("`{what}` needs true or false"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codecs_round_trip_and_args_parse() {
        let v = Value::object([("a", Value::Int(1))]);
        let b = encode(Codec::Json, &v).unwrap();
        assert_eq!(b, br#"{"a":1}"#);
        assert_eq!(decode(Codec::Json, b).unwrap(), v);
        assert!(encode(Codec::Bytes, &Value::Int(1)).is_err());
        assert_eq!(
            decode(Codec::Json, b"{".to_vec()).unwrap_err().code,
            "parse.json"
        );
        let args = vec![
            EvalArg::Word("json".into()),
            EvalArg::Word("timeout".into()),
            EvalArg::Value(Value::text("2s")),
        ];
        assert_eq!(codec_at(&args, 0, "m").unwrap(), Codec::Json);
        assert_eq!(timeout_arg(&args).unwrap(), Some(2000));
    }
}
