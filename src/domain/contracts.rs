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

// vhco:domain Envelope { data: DataEvent | result: Completion | error: RivetError }
#[derive(Clone, Debug, PartialEq)]
pub enum Envelope {
    Data(DataEvent),
    Result(Completion),
    Error {
        request_id: String,
        trace_id: String,
        seq: u64,
        error: RivetError,
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

// vhco:domain RegistryEntry { id: string; name: string; description?: string; kind: OperationKind; private: bool; params: ParameterSpec[]; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; errors: DeclaredError[]; source: SourceSpan }
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
}

impl RegistryEntry {
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
            "input": params_schema(&self.params),
            "output": self.output_schema(),
            "emits": self.emits.as_ref().map(ValueSpec::to_json_schema),
            "receives": self.receives.as_ref().map(ValueSpec::to_json_schema),
            "errors": self.errors.iter().map(|e| json!({"code": e.code, "description": e.description})).collect::<Vec<_>>(),
            "delivery": self.delivery(),
            "source": {"file": self.source.file, "line": self.source.start_line},
        })
    }

    pub fn output_schema(&self) -> Json {
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

// vhco:domain OutputReport { id: string; name: string; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; errors: DeclaredError[] }
#[derive(Clone, Debug, PartialEq)]
pub struct OutputReport {
    pub id: String,
    pub name: String,
    pub output: OutputSpec,
    pub emits: Option<ValueSpec>,
    pub receives: Option<ValueSpec>,
    pub errors: Vec<DeclaredError>,
}

impl OutputReport {
    pub fn to_json(&self) -> Json {
        let mut output = self.output.spec.to_json_schema();
        if let (Some(d), Json::Object(m)) = (&self.output.description, &mut output) {
            m.insert("description".into(), Json::String(d.clone()));
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
