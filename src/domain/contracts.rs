//! Request, completion, stream and catalog contracts shared by every surface.

use super::errors::{EffectsStatus, RivetError};
use super::ir::OperationKind;
use super::outputs::{DeclaredError, OutputSpec, ParamSpec, ValueSpec, params_schema};
use super::source::SourceSpan;
use super::value::Value;
use serde_json::{Value as Json, json};

// vhco:domain Principal { name: string; authenticated_by: string }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    pub name: String,
    pub authenticated_by: String,
}

impl Principal {
    pub fn local() -> Principal {
        Principal {
            name: "local".into(),
            authenticated_by: "none".into(),
        }
    }
}

// vhco:domain Request { request_id: string; trace_id: string; operation_id: string; params: Value; principal: Principal; parent_request_id?: string; depth: int; deadline_ms: int; include_private: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub request_id: String,
    pub trace_id: String,
    pub operation_id: String,
    pub params: Value,
    pub principal: Principal,
    pub parent_request_id: Option<String>,
    pub depth: u32,
    pub deadline_ms: u64,
    /// In-bundle calls may reach private helpers; external surfaces never set this.
    pub include_private: bool,
}

/// Default request deadline (PROP-2026-0001 defaults).
pub const DEFAULT_DEADLINE_MS: u64 = 30_000;

// vhco:domain Completion { request_id: string; trace_id: string; result: Value; data_count: int; effects: EffectsStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    pub request_id: String,
    pub trace_id: String,
    pub result: Value,
    pub data_count: u64,
    pub effects: EffectsStatus,
}

impl Completion {
    /// Decode a Completion (or a `type: result` envelope) received from a
    /// remote `rivet serve`; `None` when the object is not a Completion.
    pub fn from_json(j: &Json) -> Option<Completion> {
        let s = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
        Some(Completion {
            request_id: s("request_id")?,
            trace_id: s("trace_id").unwrap_or_default(),
            result: j.get("result").map(Value::from_json).unwrap_or(Value::Null),
            data_count: j.get("data_count").and_then(Json::as_u64).unwrap_or(0),
            effects: EffectsStatus::parse(&s("effects").unwrap_or_default()),
        })
    }

    pub fn to_json(&self) -> Json {
        json!({
            "request_id": self.request_id,
            "trace_id": self.trace_id,
            "result": self.result.to_json(),
            "data_count": self.data_count,
            "effects": self.effects.as_str(),
        })
    }
}

// vhco:domain DataEvent { request_id: string; trace_id: string; seq: int; data: Value }
#[derive(Clone, Debug, PartialEq)]
pub struct DataEvent {
    pub request_id: String,
    pub trace_id: String,
    pub seq: u64,
    pub data: Value,
}

impl DataEvent {
    /// Decode a `type: data` envelope (SSE event or NDJSON line).
    pub fn from_json(j: &Json) -> Option<DataEvent> {
        let s = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
        Some(DataEvent {
            request_id: s("request_id").unwrap_or_default(),
            trace_id: s("trace_id").unwrap_or_default(),
            seq: j.get("seq").and_then(Json::as_u64)?,
            data: j.get("data").map(Value::from_json).unwrap_or(Value::Null),
        })
    }
}

// vhco:domain Envelope { data: DataEvent | result: Completion | error: RivetError }
#[derive(Clone, Debug, PartialEq)]
pub enum Envelope {
    Data(DataEvent),
    Result(Completion),
    Error {
        request_id: String,
        trace_id: String,
        seq: u64,
        error: Box<RivetError>,
    },
}

impl Envelope {
    pub fn to_json(&self) -> Json {
        match self {
            Envelope::Data(d) => json!({
                "request_id": d.request_id, "trace_id": d.trace_id, "seq": d.seq,
                "type": "data", "data": d.data.to_json()
            }),
            Envelope::Result(c) => {
                let mut j = c.to_json();
                j["type"] = json!("result");
                j
            }
            Envelope::Error {
                request_id,
                trace_id,
                seq,
                error,
            } => json!({
                "request_id": request_id, "trace_id": trace_id, "seq": seq,
                "type": "error", "error": error.to_value().to_json()
            }),
        }
    }
}

/// ErrorEnvelope body for unary surfaces.
pub fn error_envelope(request_id: &str, trace_id: &str, error: &RivetError) -> Json {
    json!({"request_id": request_id, "trace_id": trace_id, "error": error.to_value().to_json()})
}

// vhco:domain RegistryEntry { id: string; name: string; description?: string; kind: OperationKind; private: bool; params: ParameterSpec[]; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; errors: DeclaredError[]; source: SourceSpan; raw_input_schema?: Json; raw_output_schema?: Json }
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: OperationKind,
    pub private: bool,
    pub params: Vec<ParamSpec>,
    pub output: OutputSpec,
    pub emits: Option<ValueSpec>,
    pub receives: Option<ValueSpec>,
    pub errors: Vec<DeclaredError>,
    pub source: SourceSpan,
    /// Imported MCP operations keep their reviewed snapshot `inputSchema`
    /// verbatim (`params` is only its best-effort projection); `None` for
    /// operations declared in `.rivet` source.
    pub raw_input_schema: Option<Json>,
    /// The snapshot `outputSchema` of an imported MCP tool, verbatim.
    pub raw_output_schema: Option<Json>,
}

impl RegistryEntry {
    /// The input JSON Schema every surface shows (`describe`, MCP tools/list).
    pub fn input_schema(&self) -> Json {
        self.raw_input_schema
            .clone()
            .unwrap_or_else(|| params_schema(&self.params))
    }

    /// Decode a `describe` (or `list` summary) JSON object received from a
    /// remote `rivet serve` so the CLI renders the same tables as a local run.
    pub fn from_json(j: &Json) -> Option<RegistryEntry> {
        let s = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
        let spec = |k: &str| {
            j.get(k)
                .filter(|v| !v.is_null())
                .map(ValueSpec::from_json_schema)
        };
        let mut emits = spec("emits");
        let receives = spec("receives");
        if emits.is_none()
            && receives.is_none()
            && j.get("streaming").and_then(Json::as_bool) == Some(true)
        {
            emits = Some(ValueSpec::Json);
        }
        let id = s("id")?;
        Some(RegistryEntry {
            name: s("name").unwrap_or_else(|| id.clone()),
            id,
            description: s("description"),
            kind: if s("kind").as_deref() == Some("pipeline") {
                OperationKind::Pipeline
            } else {
                OperationKind::Operation
            },
            private: false,
            params: j
                .get("input")
                .map(ParamSpec::list_from_schema)
                .unwrap_or_default(),
            output: OutputSpec {
                spec: spec("output").unwrap_or(ValueSpec::Json),
                description: j
                    .get("output")
                    .and_then(|o| o.get("description"))
                    .and_then(Json::as_str)
                    .map(str::to_string),
            },
            emits,
            receives,
            errors: j
                .get("errors")
                .and_then(Json::as_array)
                .map(|e| {
                    e.iter()
                        .map(|x| DeclaredError {
                            code: x
                                .get("code")
                                .and_then(Json::as_str)
                                .unwrap_or("")
                                .to_string(),
                            description: x
                                .get("description")
                                .and_then(Json::as_str)
                                .map(str::to_string),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            source: SourceSpan {
                file: j
                    .get("source")
                    .and_then(|x| x.get("file"))
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string(),
                start_line: j
                    .get("source")
                    .and_then(|x| x.get("line"))
                    .and_then(Json::as_u64)
                    .unwrap_or(0) as u32,
                ..SourceSpan::default()
            },
            raw_input_schema: None,
            raw_output_schema: None,
        })
    }

    /// The OutputReport projection of this entry.
    pub fn output_report(&self) -> OutputReport {
        OutputReport {
            id: self.id.clone(),
            name: self.name.clone(),
            output: self.output.clone(),
            emits: self.emits.clone(),
            receives: self.receives.clone(),
            errors: self.errors.clone(),
            raw_output_schema: self.raw_output_schema.clone(),
        }
    }

    pub fn streaming(&self) -> bool {
        self.emits.is_some() || self.receives.is_some()
    }

    pub fn delivery(&self) -> &'static str {
        if self.streaming() { "session" } else { "unary" }
    }

    pub fn summary_json(&self) -> Json {
        json!({
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "streaming": self.streaming(),
        })
    }

    pub fn describe_json(&self) -> Json {
        json!({
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "kind": match self.kind { OperationKind::Operation => "operation", OperationKind::Pipeline => "pipeline" },
            "input": self.input_schema(),
            "output": self.output_schema(),
            "emits": self.emits.as_ref().map(ValueSpec::to_json_schema),
            "receives": self.receives.as_ref().map(ValueSpec::to_json_schema),
            "errors": self.errors.iter().map(|e| json!({"code": e.code, "description": e.description})).collect::<Vec<_>>(),
            "delivery": self.delivery(),
            "source": {"file": self.source.file, "line": self.source.start_line},
        })
    }

    pub fn output_schema(&self) -> Json {
        if let Some(raw) = &self.raw_output_schema {
            return raw.clone();
        }
        let mut s = self.output.spec.to_json_schema();
        if let (Some(d), Json::Object(m)) = (&self.output.description, &mut s) {
            m.insert("description".into(), Json::String(d.clone()));
        }
        s
    }
}

// vhco:domain CatalogQuery { ids: string[]; include_private: bool; principal?: Principal }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CatalogQuery {
    pub ids: Vec<String>,
    pub include_private: bool,
}

// vhco:domain Catalog { entries: RegistryEntry[]; next_cursor?: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Catalog {
    pub entries: Vec<RegistryEntry>,
}

// vhco:domain ExecutionPlan { request: Request; operation_id: string; params: Value }
/// Validated request handed to the execution driver.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionPlan {
    pub request: Request,
    pub params: Value,
}

// vhco:domain RunOutcome { result: Value; data_count: int; effects: EffectsStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct RunOutcome {
    pub result: Value,
    pub data_count: u64,
    pub effects: EffectsStatus,
}

// vhco:domain OutputReport { id: string; name: string; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; errors: DeclaredError[]; raw_output_schema?: Json }
#[derive(Clone, Debug, PartialEq)]
pub struct OutputReport {
    pub id: String,
    pub name: String,
    pub output: OutputSpec,
    pub emits: Option<ValueSpec>,
    pub receives: Option<ValueSpec>,
    pub errors: Vec<DeclaredError>,
    /// Imported MCP tools: the snapshot `outputSchema`, verbatim.
    pub raw_output_schema: Option<Json>,
}

impl OutputReport {
    pub fn to_json(&self) -> Json {
        let mut output = self.output.spec.to_json_schema();
        if let (Some(d), Json::Object(m)) = (&self.output.description, &mut output) {
            m.insert("description".into(), Json::String(d.clone()));
        }
        if let Some(raw) = &self.raw_output_schema {
            output = raw.clone();
        }
        json!({
            "id": self.id,
            "output": output,
            "emits": self.emits.as_ref().map(ValueSpec::to_json_schema),
            "receives": self.receives.as_ref().map(ValueSpec::to_json_schema),
            "errors": self.errors.iter().map(|e| json!({"code": e.code, "description": e.description})).collect::<Vec<_>>(),
        })
    }
}
