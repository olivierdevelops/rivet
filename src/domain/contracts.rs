//! Request, completion, stream and catalog contracts shared by every surface.

use super::cancel::CancelToken;
use super::envelope::{RecordType, ResponseEnvelope};
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

// vhco:domain Request { request_id: string; trace_id: string; operation_id: string; params: Value; principal: Principal; parent_request_id?: string; parent_span_id?: string; depth: int; deadline_ms: int; include_private: bool; cancel: CancelToken; restrict?: Value }
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
    /// W3C parent span: the caller's `traceparent` parent-id on a top-level
    /// request, the parent request's span ([`span_id`]) on a nested one.
    pub parent_span_id: Option<String>,
    /// Structured cancellation of this request's scope (a child of the
    /// parent's token for nested requests).
    pub cancel: CancelToken,
    /// Per-request restriction `{grants:[…]}` (policy.json grant format) sent by an
    /// HTTP/MCP/WebSocket caller or a library host. It is intersected with the
    /// effective policy for this request (and its nested calls) only: it can
    /// narrow, never widen (proposal Increments 5 and 16).
    pub restrict: Option<Value>,
}

/// Default request deadline (PROP-2026-0001 defaults).
pub const DEFAULT_DEADLINE_MS: u64 = 30_000;

/// Host cap on any caller-requested deadline (`deadline_ms`, `--timeout`, sessions.open): 10 minutes.
pub const MAX_DEADLINE_MS: u64 = 600_000;

// vhco:domain TraceContext { trace_id: string; parent_id: string; flags: int }
/// A valid W3C `traceparent` (version 00): 32-hex trace-id, 16-hex parent-id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,
    pub parent_id: String,
    pub flags: u8,
}

impl TraceContext {
    /// Parse `00-<trace-id>-<parent-id>-<flags>`; invalid or all-zero IDs are `None`
    /// (the caller then mints its own trace id, as W3C requires).
    pub fn parse(header: &str) -> Option<TraceContext> {
        let parts: Vec<&str> = header.trim().split('-').collect();
        let hex = |s: &str, n: usize| {
            s.len() == n
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if parts.len() < 4 || !hex(parts[0], 2) || parts[0] == "ff" {
            return None;
        }
        if parts[0] == "00" && parts.len() != 4 {
            return None;
        }
        let (trace, parent, flags) = (parts[1], parts[2], parts[3]);
        if !hex(trace, 32) || !hex(parent, 16) || !hex(flags, 2) {
            return None;
        }
        if trace.bytes().all(|b| b == b'0') || parent.bytes().all(|b| b == b'0') {
            return None;
        }
        Some(TraceContext {
            trace_id: trace.to_string(),
            parent_id: parent.to_string(),
            flags: u8::from_str_radix(flags, 16).ok()?,
        })
    }
}

fn fnv64(seed: u64, s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Avoid the all-zero ID W3C forbids.
    if h == 0 { 1 } else { h }
}

/// The 32-hex W3C trace-id of a Rivet trace id: itself when it already is one
/// (it came from a caller's `traceparent`), otherwise a stable hash of it.
pub fn w3c_trace_id(trace_id: &str) -> String {
    let is_hex = trace_id.len() == 32
        && trace_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if is_hex {
        return trace_id.to_string();
    }
    format!("{:016x}{:016x}", fnv64(1, trace_id), fnv64(2, trace_id))
}

/// The 16-hex W3C span (parent-id) of one request, stable for its request id.
pub fn span_id(request_id: &str) -> String {
    format!("{:016x}", fnv64(3, request_id))
}

/// The `traceparent` a surface emits for one request (sampled flag set).
pub fn traceparent_header(trace_id: &str, request_id: &str) -> String {
    format!("00-{}-{}-01", w3c_trace_id(trace_id), span_id(request_id))
}

/// UTC RFC 3339 timestamp with millisecond precision.
pub fn rfc3339_millis(t: std::time::SystemTime) -> String {
    let d = t
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil_from_days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        d.subsec_millis()
    )
}

// vhco:domain Completion { request_id: string; trace_id: string; operation: string; result: Value; data_count: int; effects: EffectsStatus }
/// A finished request. Every surface writes it as a [`ResponseEnvelope`]
/// (`result` becomes the envelope's `data`).
#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    pub request_id: String,
    pub trace_id: String,
    /// The operation this completion answers (the envelope's `operation`).
    pub operation: String,
    pub result: Value,
    pub data_count: u64,
    pub effects: EffectsStatus,
}

impl Completion {
    /// Decode a `type: result, status: ok` envelope received from a remote
    /// `rivet serve`; `None` when the object is not such an envelope.
    pub fn from_json(j: &Json) -> Option<Completion> {
        ResponseEnvelope::from_json(j)
            .filter(|e| e.record_type == RecordType::Result)
            .and_then(|e| e.into_outcome().ok())
    }

    /// This completion as an envelope.
    pub fn envelope(&self) -> ResponseEnvelope {
        ResponseEnvelope::from_completion(self)
    }
}

// vhco:domain DataEvent { request_id: string; trace_id: string; operation: string; seq: int; data: Value }
#[derive(Clone, Debug, PartialEq)]
pub struct DataEvent {
    pub request_id: String,
    pub trace_id: String,
    /// The operation that emitted the item.
    pub operation: String,
    pub seq: u64,
    pub data: Value,
}

impl DataEvent {
    /// Decode a `type: data` record (SSE event, NDJSON line or WS frame).
    pub fn from_json(j: &Json) -> Option<DataEvent> {
        let e = ResponseEnvelope::from_json(j).filter(|e| e.record_type == RecordType::Data)?;
        Some(DataEvent {
            request_id: e.request_id,
            trace_id: e.trace_id,
            operation: e.operation.unwrap_or_default(),
            seq: e.seq?,
            data: e.data.as_ref().map(Value::from_json).unwrap_or(Value::Null),
        })
    }

    /// This item as a `type: data` record.
    pub fn envelope(&self) -> ResponseEnvelope {
        ResponseEnvelope::from_data(self)
    }
}

// vhco:domain Envelope { data: DataEvent | result: Completion | error: RivetError }
/// One stream event in Rust form; [`Envelope::record`] is its wire record.
#[derive(Clone, Debug, PartialEq)]
pub enum Envelope {
    Data(DataEvent),
    Result(Completion),
    Error {
        request_id: String,
        trace_id: String,
        /// The operation the failed request ran.
        operation: String,
        seq: u64,
        error: Box<RivetError>,
    },
}

impl Envelope {
    /// The wire record: `type: data` for items, `type: result` (status ok,
    /// error or cancelled) for the terminal event.
    pub fn record(&self) -> ResponseEnvelope {
        match self {
            Envelope::Data(d) => ResponseEnvelope::from_data(d),
            Envelope::Result(c) => ResponseEnvelope::from_completion(c),
            Envelope::Error {
                request_id,
                trace_id,
                operation,
                error,
                ..
            } => {
                let mut e = (**error).clone();
                e.request_id = Some(request_id.clone());
                e.trace_id = Some(trace_id.clone());
                ResponseEnvelope::from_error(Some(operation), &e)
            }
        }
    }

    /// Compact JSON of [`Envelope::record`].
    pub fn to_json(&self) -> Json {
        self.record().to_json()
    }
}

/// The JSON Schema of an `emits`/`receives` item, with its declared description.
fn item_schema(spec: Option<&ValueSpec>, description: Option<&str>) -> Option<Json> {
    let mut schema = spec?.to_json_schema();
    if let (Some(d), Json::Object(m)) = (description, &mut schema) {
        m.insert("description".into(), Json::String(d.to_string()));
    }
    Some(schema)
}

// vhco:domain RegistryEntry { id: string; name: string; description?: string; kind: OperationKind; private: bool; params: ParameterSpec[]; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; emits_description?: string; receives_description?: string; errors: DeclaredError[]; source: SourceSpan; raw_input_schema?: Json; raw_output_schema?: Json }
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
    pub emits_description: Option<String>,
    pub receives_description: Option<String>,
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
        let item_desc = |k: &str| {
            j.get(k)
                .and_then(|o| o.get("description"))
                .and_then(Json::as_str)
                .map(str::to_string)
        };
        let id = s("id")?;
        Some(RegistryEntry {
            emits_description: item_desc("emits"),
            receives_description: item_desc("receives"),
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
            emits_description: self.emits_description.clone(),
            receives_description: self.receives_description.clone(),
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
            "emits": item_schema(self.emits.as_ref(), self.emits_description.as_deref()),
            "receives": item_schema(self.receives.as_ref(), self.receives_description.as_deref()),
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

// vhco:domain OutputReport { id: string; name: string; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; emits_description?: string; receives_description?: string; errors: DeclaredError[]; raw_output_schema?: Json }
#[derive(Clone, Debug, PartialEq)]
pub struct OutputReport {
    pub id: String,
    pub name: String,
    pub output: OutputSpec,
    pub emits: Option<ValueSpec>,
    pub receives: Option<ValueSpec>,
    pub emits_description: Option<String>,
    pub receives_description: Option<String>,
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
            "emits": item_schema(self.emits.as_ref(), self.emits_description.as_deref()),
            "receives": item_schema(self.receives.as_ref(), self.receives_description.as_deref()),
            "errors": self.errors.iter().map(|e| json!({"code": e.code, "description": e.description})).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod trace_tests {
    use super::*;

    #[test]
    fn traceparent_parse_and_emit() {
        let h = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let c = TraceContext::parse(h).unwrap();
        assert_eq!(c.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(c.parent_id, "00f067aa0ba902b7");
        for bad in [
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-x",
            "garbage",
        ] {
            assert!(TraceContext::parse(bad).is_none(), "{bad}");
        }
        assert_eq!(w3c_trace_id(&c.trace_id), c.trace_id);
        let own = w3c_trace_id("tr_01abcdef12");
        assert_eq!(own.len(), 32);
        assert_eq!(own, w3c_trace_id("tr_01abcdef12"));
        let emitted = traceparent_header("tr_01abcdef12", "req_01");
        assert!(TraceContext::parse(&emitted).is_some());
        assert_ne!(span_id("req_01"), span_id("req_02"));
        let t =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_709_164_800_123);
        assert_eq!(rfc3339_millis(t), "2024-02-29T00:00:00.123Z");
    }
}
