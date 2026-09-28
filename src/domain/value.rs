//! Runtime values shared by every layer.
//!
//! Rivet values are JSON-compatible plus first-class bytes and signed 64-bit
//! integers. JSON surfaces encode bytes as `{"$type":"bytes","base64":…}` and
//! integers outside the exact interoperable range as
//! `{"$type":"integer","decimal":…}` (REF-2026-0002 "Core syntax and semantics").

use base64::Engine;
use serde_json::{Map, Number, Value as Json};
use std::fmt;

/// Largest integer JSON consumers can hold exactly (2^53 − 1).
pub const JSON_SAFE_INT: i64 = 9_007_199_254_740_991;

// vhco:domain Value { null | bool | int | float | text | bytes | list | object }
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn object<I: IntoIterator<Item = (S, Value)>, S: Into<String>>(pairs: I) -> Value {
        Value::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn text(s: impl Into<String>) -> Value {
        Value::Text(s.into())
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Float(_) => "number",
            Value::Text(_) => "text",
            Value::Bytes(_) => "bytes",
            Value::List(_) => "list",
            Value::Object(_) => "object",
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Value::Object(pairs) => pairs.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Insert or replace a key, keeping first-insertion order.
    pub fn set(&mut self, key: &str, value: Value) {
        if let Value::Object(pairs) = self {
            if let Some(slot) = pairs.iter_mut().find(|(k, _)| k == key) {
                slot.1 = value;
            } else {
                pairs.push((key.to_string(), value));
            }
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Truthiness used by `if`/`while`/`until`: only `false` and `null` are false.
    pub fn truthy(&self) -> bool {
        !matches!(self, Value::Null | Value::Bool(false))
    }

    /// Encode for a JSON surface (tagged bytes, tagged wide integers).
    pub fn to_json(&self) -> Json {
        match self {
            Value::Null => Json::Null,
            Value::Bool(b) => Json::Bool(*b),
            Value::Int(i) if i.abs() <= JSON_SAFE_INT => Json::Number((*i).into()),
            Value::Int(i) => serde_json::json!({"$type": "integer", "decimal": i.to_string()}),
            Value::Float(f) => Number::from_f64(*f).map(Json::Number).unwrap_or(Json::Null),
            Value::Text(s) => Json::String(s.clone()),
            Value::Bytes(b) => serde_json::json!({
                "$type": "bytes",
                "base64": base64::engine::general_purpose::STANDARD.encode(b)
            }),
            Value::List(items) => Json::Array(items.iter().map(Value::to_json).collect()),
            Value::Object(pairs) => {
                let mut map = Map::new();
                for (k, v) in pairs {
                    map.insert(k.clone(), v.to_json());
                }
                Json::Object(map)
            }
        }
    }

    /// Decode from a JSON surface, recognising the tagged bytes/integer forms.
    pub fn from_json(json: &Json) -> Value {
        match json {
            Json::Null => Value::Null,
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else {
                    Value::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            Json::String(s) => Value::Text(s.clone()),
            Json::Array(items) => Value::List(items.iter().map(Value::from_json).collect()),
            Json::Object(map) => {
                if let (Some(Json::String(t)), 2) = (map.get("$type"), map.len()) {
                    if t == "bytes" {
                        if let Some(Json::String(b64)) = map.get("base64") {
                            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64)
                            {
                                return Value::Bytes(bytes);
                            }
                        }
                    }
                    if t == "integer" {
                        if let Some(Json::String(d)) = map.get("decimal") {
                            if let Ok(i) = d.parse::<i64>() {
                                return Value::Int(i);
                            }
                        }
                    }
                }
                Value::Object(
                    map.iter()
                        .map(|(k, v)| (k.clone(), Value::from_json(v)))
                        .collect(),
                )
            }
        }
    }

    /// Text form used by `${…}` interpolation and `text` codecs.
    pub fn to_display(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Text(s) => s.clone(),
            Value::Bytes(b) => base64::engine::general_purpose::STANDARD.encode(b),
            other => other.to_json().to_string(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_and_wide_integers_round_trip_through_json() {
        let v = Value::object([
            ("b", Value::Bytes(vec![0, 1, 2])),
            ("big", Value::Int(i64::MAX)),
            ("small", Value::Int(5)),
        ]);
        let json = v.to_json();
        assert_eq!(json["b"]["base64"], "AAEC");
        assert_eq!(json["big"]["decimal"], "9223372036854775807");
        assert_eq!(json["small"], 5);
        assert_eq!(Value::from_json(&json), v);
    }

    #[test]
    fn object_keeps_insertion_order() {
        let mut v = Value::object([("z", Value::Int(1))]);
        v.set("a", Value::Int(2));
        assert_eq!(v.to_json().to_string(), r#"{"z":1,"a":2}"#);
    }
}
