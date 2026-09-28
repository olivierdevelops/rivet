//! Parameter, output, emits and receives schemas (R20, R23).

use super::value::Value;
use serde_json::{Value as Json, json};

// vhco:domain ValueSpec { text | integer | number | boolean | bytes | json | object | list }
#[derive(Clone, Debug, PartialEq)]
pub enum ValueSpec {
    Text,
    Integer,
    Number,
    Boolean,
    Bytes,
    Json,
    Object { fields: Vec<FieldSpec>, open: bool },
    List(Box<ValueSpec>),
}

impl ValueSpec {
    pub fn parse_scalar(word: &str) -> Option<ValueSpec> {
        Some(match word {
            "text" => ValueSpec::Text,
            "integer" => ValueSpec::Integer,
            "number" => ValueSpec::Number,
            "boolean" => ValueSpec::Boolean,
            "bytes" => ValueSpec::Bytes,
            "json" => ValueSpec::Json,
            _ => return None,
        })
    }

    pub fn name(&self) -> String {
        match self {
            ValueSpec::Text => "text".into(),
            ValueSpec::Integer => "integer".into(),
            ValueSpec::Number => "number".into(),
            ValueSpec::Boolean => "boolean".into(),
            ValueSpec::Bytes => "bytes".into(),
            ValueSpec::Json => "json".into(),
            ValueSpec::Object { .. } => "object".into(),
            ValueSpec::List(inner) => format!("list {}", inner.name()),
        }
    }

    /// JSON Schema projection used by describe/outputs/MCP.
    pub fn to_json_schema(&self) -> Json {
        match self {
            ValueSpec::Text => json!({"type": "string"}),
            ValueSpec::Integer => json!({"type": "integer"}),
            ValueSpec::Number => json!({"type": "number"}),
            ValueSpec::Boolean => json!({"type": "boolean"}),
            ValueSpec::Bytes => json!({
                "type": "object",
                "properties": {"$type": {"const": "bytes"}, "base64": {"type": "string"}},
                "required": ["$type", "base64"]
            }),
            ValueSpec::Json => json!({}),
            ValueSpec::List(inner) => json!({"type": "array", "items": inner.to_json_schema()}),
            ValueSpec::Object { fields, open } => {
                let mut props = serde_json::Map::new();
                let mut required = Vec::new();
                for f in fields {
                    let mut s = f.spec.to_json_schema();
                    if let (Some(d), Json::Object(m)) = (&f.description, &mut s) {
                        m.insert("description".into(), Json::String(d.clone()));
                    }
                    props.insert(f.name.clone(), s);
                    if f.required {
                        required.push(Json::String(f.name.clone()));
                    }
                }
                json!({
                    "type": "object",
                    "properties": props,
                    "required": required,
                    "additionalProperties": open
                })
            }
        }
    }

    /// Best-effort inverse of [`ValueSpec::to_json_schema`] (remote `describe`
    /// tables, imported MCP tool schemas). Constructs Rivet cannot express
    /// (`oneOf`, `format`, tuples, …) become `json` — looser, never stricter.
    pub fn from_json_schema(schema: &Json) -> ValueSpec {
        let Some(m) = schema.as_object() else {
            return ValueSpec::Json;
        };
        let ty = m.get("type").and_then(Json::as_str).unwrap_or("");
        match ty {
            "string" => ValueSpec::Text,
            "integer" => ValueSpec::Integer,
            "number" => ValueSpec::Number,
            "boolean" => ValueSpec::Boolean,
            "array" => ValueSpec::List(Box::new(
                m.get("items")
                    .map(ValueSpec::from_json_schema)
                    .unwrap_or(ValueSpec::Json),
            )),
            "object" => {
                let props = m.get("properties").and_then(Json::as_object);
                if props
                    .and_then(|p| p.get("$type"))
                    .and_then(|t| t.get("const"))
                    .and_then(Json::as_str)
                    == Some("bytes")
                {
                    return ValueSpec::Bytes;
                }
                let required: Vec<&str> = m
                    .get("required")
                    .and_then(Json::as_array)
                    .map(|r| r.iter().filter_map(Json::as_str).collect())
                    .unwrap_or_default();
                let fields = props
                    .map(|p| {
                        p.iter()
                            .map(|(name, s)| FieldSpec {
                                name: name.clone(),
                                spec: ValueSpec::from_json_schema(s),
                                required: required.contains(&name.as_str()),
                                description: s
                                    .get("description")
                                    .and_then(Json::as_str)
                                    .map(str::to_string),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ValueSpec::Object {
                    fields,
                    open: m
                        .get("additionalProperties")
                        .map(|a| a.as_bool().unwrap_or(true))
                        .unwrap_or(true),
                }
            }
            _ => ValueSpec::Json,
        }
    }

    /// Structural check of a value against this spec. Returns every violation.
    pub fn check(&self, value: &Value, path: &str, out: &mut Vec<SchemaViolation>) {
        let ok = match (self, value) {
            (ValueSpec::Json, _) => true,
            (ValueSpec::Text, Value::Text(_)) => true,
            (ValueSpec::Integer, Value::Int(_)) => true,
            (ValueSpec::Number, Value::Int(_) | Value::Float(_)) => true,
            (ValueSpec::Boolean, Value::Bool(_)) => true,
            (ValueSpec::Bytes, Value::Bytes(_)) => true,
            (ValueSpec::List(inner), Value::List(items)) => {
                for (i, item) in items.iter().enumerate() {
                    inner.check(item, &format!("{path}[{i}]"), out);
                }
                true
            }
            (ValueSpec::Object { fields, open }, Value::Object(pairs)) => {
                for f in fields {
                    match value.get(&f.name) {
                        Some(Value::Null) | None if f.required => out.push(SchemaViolation {
                            path: join_path(path, &f.name),
                            expected: f.spec.name(),
                            found: "missing".into(),
                        }),
                        Some(Value::Null) | None => {}
                        Some(v) => f.spec.check(v, &join_path(path, &f.name), out),
                    }
                }
                if !open {
                    for (k, _) in pairs {
                        if !fields.iter().any(|f| &f.name == k) {
                            out.push(SchemaViolation {
                                path: join_path(path, k),
                                expected: "no such field".into(),
                                found: "unexpected field".into(),
                            });
                        }
                    }
                }
                true
            }
            _ => false,
        };
        if !ok {
            out.push(SchemaViolation {
                path: if path.is_empty() {
                    "$".into()
                } else {
                    path.to_string()
                },
                expected: self.name(),
                found: value.type_name().into(),
            });
        }
    }
}

fn join_path(base: &str, key: &str) -> String {
    if base.is_empty() {
        key.to_string()
    } else {
        format!("{base}.{key}")
    }
}

// vhco:domain FieldSpec { name: string; spec: ValueSpec; required: bool; description?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSpec {
    pub name: String,
    pub spec: ValueSpec,
    pub required: bool,
    pub description: Option<String>,
}

// vhco:domain OutputSpec { spec: ValueSpec; description?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct OutputSpec {
    pub spec: ValueSpec,
    pub description: Option<String>,
}

impl Default for OutputSpec {
    fn default() -> Self {
        OutputSpec {
            spec: ValueSpec::Json,
            description: None,
        }
    }
}

// vhco:domain DeclaredError { code: string; description?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredError {
    pub code: String,
    pub description: Option<String>,
}

// vhco:domain ParameterSpec { name: string; spec: ValueSpec; required: bool; default?: Value; min?: number; max?: number; enum_values?: Value[]; description?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct ParamSpec {
    pub name: String,
    pub spec: ValueSpec,
    pub required: bool,
    pub default: Option<Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub enum_values: Option<Vec<Value>>,
    pub description: Option<String>,
}

impl ParamSpec {
    /// Every parameter of an object input schema (inverse of [`params_schema`]).
    pub fn list_from_schema(schema: &Json) -> Vec<ParamSpec> {
        let required: Vec<&str> = schema
            .get("required")
            .and_then(Json::as_array)
            .map(|r| r.iter().filter_map(Json::as_str).collect())
            .unwrap_or_default();
        let Some(props) = schema.get("properties").and_then(Json::as_object) else {
            return Vec::new();
        };
        props
            .iter()
            .map(|(name, s)| ParamSpec {
                name: name.clone(),
                spec: ValueSpec::from_json_schema(s),
                required: required.contains(&name.as_str()),
                default: s.get("default").map(Value::from_json),
                min: s.get("minimum").and_then(Json::as_f64),
                max: s.get("maximum").and_then(Json::as_f64),
                enum_values: s
                    .get("enum")
                    .and_then(Json::as_array)
                    .map(|e| e.iter().map(Value::from_json).collect()),
                description: s
                    .get("description")
                    .and_then(Json::as_str)
                    .map(str::to_string),
            })
            .collect()
    }

    pub fn to_json_schema(&self) -> Json {
        let mut s = self.spec.to_json_schema();
        if let Json::Object(m) = &mut s {
            if let Some(d) = &self.description {
                m.insert("description".into(), Json::String(d.clone()));
            }
            if let Some(v) = &self.default {
                m.insert("default".into(), v.to_json());
            }
            if let Some(min) = self.min {
                m.insert("minimum".into(), json!(min));
            }
            if let Some(max) = self.max {
                m.insert("maximum".into(), json!(max));
            }
            if let Some(e) = &self.enum_values {
                m.insert(
                    "enum".into(),
                    Json::Array(e.iter().map(Value::to_json).collect()),
                );
            }
        }
        s
    }
}

/// Closed input schema for a parameter list (no-param = `{type:object, additionalProperties:false}`).
pub fn params_schema(params: &[ParamSpec]) -> Json {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for p in params {
        props.insert(p.name.clone(), p.to_json_schema());
        if p.required {
            required.push(Json::String(p.name.clone()));
        }
    }
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

// vhco:domain SchemaViolation { path: string; expected: string; found: string }
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaViolation {
    pub path: String,
    pub expected: String,
    pub found: String,
}

impl SchemaViolation {
    pub fn to_value(&self) -> Value {
        Value::object([
            ("path", Value::text(&self.path)),
            ("expected", Value::text(&self.expected)),
            ("found", Value::text(&self.found)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_spec() -> ValueSpec {
        ValueSpec::Object {
            fields: vec![
                FieldSpec {
                    name: "id".into(),
                    spec: ValueSpec::Integer,
                    required: true,
                    description: None,
                },
                FieldSpec {
                    name: "email".into(),
                    spec: ValueSpec::Text,
                    required: false,
                    description: None,
                },
            ],
            open: false,
        }
    }

    #[test]
    fn closed_object_rejects_missing_wrong_and_extra_fields() {
        let mut out = Vec::new();
        let v = Value::object([("id", Value::text("x")), ("extra", Value::Int(1))]);
        user_spec().check(&v, "", &mut out);
        let paths: Vec<_> = out.iter().map(|v| v.path.as_str()).collect();
        assert_eq!(paths, vec!["id", "extra"]);
    }

    #[test]
    fn valid_object_passes() {
        let mut out = Vec::new();
        user_spec().check(&Value::object([("id", Value::Int(1))]), "", &mut out);
        assert!(out.is_empty());
    }
}
