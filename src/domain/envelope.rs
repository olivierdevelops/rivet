//! The one output envelope and the one input envelope every surface speaks
//! (PROP-2026-0002 R1–R6). Pure data plus its JSON projection: surfaces build a
//! [`ResponseEnvelope`] from a [`Completion`], a [`DataEvent`] or a
//! [`RivetError`] and write it compact or pretty; they hand request bodies to
//! `serve.parse_input`, which returns an [`InputEnvelope`].
//!
//! ```text
//!  input   {"operation":"users.get","data":{"id":42},"deadline_ms"?,"restrict"?,"stream"?}
//!          (0.1.0 `id`/`params` are deprecated aliases through 0.2.x)
//!
//!  output  {"request_id","trace_id","operation","type":"result","status":"ok",
//!           "data":{…},"error":null,"effects":"none","data_count":0}
//!  error   {…,"type":"result","status":"error"|"cancelled","data":null,"error":{kind,code,message,retryable,…},…}
//!  record  {"request_id","trace_id","operation","type":"data","seq":1,"data":…,"error":null}
//!  ws      the same records with "ref" first
//! ```
//!
//! Key order is fixed (serde_json `preserve_order`); `data` and `error` are
//! always present and exactly one is non-null, except `status: accepted`.

use super::contracts::{Completion, DataEvent};
use super::errors::{EffectsStatus, ErrorKind, RivetError};
use super::value::Value;
use serde_json::{Map, Value as Json, json};

/// Code for an input envelope with the wrong shape or mixed alias keys (exit 2, HTTP 422).
pub const INPUT_ENVELOPE: &str = "validation.input_envelope";

/// Code for pretty JSON requested on a line-delimited stream (HTTP 400; the CLI uses validation.usage).
pub const PRETTY_STREAM: &str = "validation.pretty_stream";

// vhco:domain EnvelopeStatus { ok | error | cancelled | accepted }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeStatus {
    Ok,
    Error,
    Cancelled,
    Accepted,
}

impl EnvelopeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            EnvelopeStatus::Ok => "ok",
            EnvelopeStatus::Error => "error",
            EnvelopeStatus::Cancelled => "cancelled",
            EnvelopeStatus::Accepted => "accepted",
        }
    }

    pub fn parse(s: &str) -> Option<EnvelopeStatus> {
        Some(match s {
            "ok" => EnvelopeStatus::Ok,
            "error" => EnvelopeStatus::Error,
            "cancelled" => EnvelopeStatus::Cancelled,
            "accepted" => EnvelopeStatus::Accepted,
            _ => return None,
        })
    }

    pub fn is_ok(self) -> bool {
        self == EnvelopeStatus::Ok
    }

    /// The status of a failure: `cancelled` for kind cancelled, otherwise `error`.
    pub fn of_error(e: &RivetError) -> EnvelopeStatus {
        if e.kind == ErrorKind::Cancelled {
            EnvelopeStatus::Cancelled
        } else {
            EnvelopeStatus::Error
        }
    }
}

// vhco:domain RecordType { result | data }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordType {
    /// Unary answer or the terminal record of a stream.
    Result,
    /// One streamed item.
    Data,
}

impl RecordType {
    pub fn as_str(self) -> &'static str {
        match self {
            RecordType::Result => "result",
            RecordType::Data => "data",
        }
    }
}

// vhco:domain OutputFormat { compact | pretty }
/// How a surface writes JSON: one line (the default) or 2-space indented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Compact,
    Pretty,
}

impl OutputFormat {
    pub fn pretty(on: bool) -> OutputFormat {
        if on {
            OutputFormat::Pretty
        } else {
            OutputFormat::Compact
        }
    }

    /// Serialize any JSON in this format; key order is preserved.
    pub fn render(self, j: &Json) -> String {
        match self {
            OutputFormat::Compact => j.to_string(),
            OutputFormat::Pretty => {
                serde_json::to_string_pretty(j).unwrap_or_else(|_| j.to_string())
            }
        }
    }
}

// vhco:domain ResponseEnvelope { ref?: string; request_id: string; trace_id: string; operation?: string; type: RecordType; seq?: int; status?: EnvelopeStatus; data?: Json; error?: Json; effects?: EffectsStatus; data_count?: int }
/// One output record. `status`, `effects` and `data_count` are `None` on
/// `type: data` records (they describe a finished request); `error` holds the
/// [`RivetError`] and is written as the error object (without `effects`,
/// which moved to the top level).
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseEnvelope {
    /// WebSocket correlation ref (written first); `None` on every other surface.
    pub r#ref: Option<String>,
    pub request_id: String,
    pub trace_id: String,
    /// The operation the record answers; `None` only when the input could not name one.
    pub operation: Option<String>,
    pub record_type: RecordType,
    /// Stream records carry their 1-based sequence.
    pub seq: Option<u64>,
    pub status: Option<EnvelopeStatus>,
    pub data: Option<Json>,
    pub error: Option<RivetError>,
    pub effects: Option<EffectsStatus>,
    pub data_count: Option<u64>,
}

impl ResponseEnvelope {
    /// A successful unary result (or a stream's terminal result).
    pub fn from_completion(c: &Completion) -> ResponseEnvelope {
        ResponseEnvelope {
            r#ref: None,
            request_id: c.request_id.clone(),
            trace_id: c.trace_id.clone(),
            operation: Some(c.operation.clone()),
            record_type: RecordType::Result,
            seq: None,
            status: Some(EnvelopeStatus::Ok),
            data: Some(c.result.to_json()),
            error: None,
            effects: Some(c.effects),
            data_count: Some(c.data_count),
        }
    }

    /// A failed (or cancelled) result. IDs come from the error when it names its
    /// request; `operation` is the one the caller asked for, when known.
    pub fn from_error(operation: Option<&str>, e: &RivetError) -> ResponseEnvelope {
        ResponseEnvelope {
            r#ref: None,
            request_id: e.request_id.clone().unwrap_or_default(),
            trace_id: e.trace_id.clone().unwrap_or_default(),
            operation: operation
                .map(str::to_string)
                .or_else(|| e.operation_id.clone()),
            record_type: RecordType::Result,
            seq: None,
            status: Some(EnvelopeStatus::of_error(e)),
            data: None,
            error: Some(e.clone()),
            effects: Some(e.effects),
            data_count: Some(0),
        }
    }

    /// `Ok` → [`from_completion`](Self::from_completion), `Err` → [`from_error`](Self::from_error).
    pub fn from_outcome(
        operation: &str,
        outcome: &Result<Completion, RivetError>,
    ) -> ResponseEnvelope {
        match outcome {
            Ok(c) => ResponseEnvelope::from_completion(c),
            Err(e) => ResponseEnvelope::from_error(Some(operation), e),
        }
    }

    /// One streamed item (`type: data`).
    pub fn from_data(ev: &DataEvent) -> ResponseEnvelope {
        ResponseEnvelope {
            r#ref: None,
            request_id: ev.request_id.clone(),
            trace_id: ev.trace_id.clone(),
            operation: Some(ev.operation.clone()),
            record_type: RecordType::Data,
            seq: Some(ev.seq),
            status: None,
            data: Some(ev.data.to_json()),
            error: None,
            effects: None,
            data_count: None,
        }
    }

    /// A payload produced for `operation` outside a dispatched run (CLI `--json`
    /// built-ins, GET routes): status ok, effects none.
    pub fn payload(
        request_id: &str,
        trace_id: &str,
        operation: &str,
        data: Json,
    ) -> ResponseEnvelope {
        ResponseEnvelope {
            r#ref: None,
            request_id: request_id.to_string(),
            trace_id: trace_id.to_string(),
            operation: Some(operation.to_string()),
            record_type: RecordType::Result,
            seq: None,
            status: Some(EnvelopeStatus::Ok),
            data: Some(data),
            error: None,
            effects: Some(EffectsStatus::None),
            data_count: Some(0),
        }
    }

    /// A request accepted for later completion (polling receipt): `data` is the receipt.
    pub fn accepted(
        request_id: &str,
        trace_id: &str,
        operation: &str,
        data: Json,
    ) -> ResponseEnvelope {
        ResponseEnvelope {
            status: Some(EnvelopeStatus::Accepted),
            ..ResponseEnvelope::payload(request_id, trace_id, operation, data)
        }
    }

    /// Stamp a stream sequence.
    pub fn with_seq(mut self, seq: u64) -> ResponseEnvelope {
        self.seq = Some(seq);
        self
    }

    /// Stamp a WebSocket ref.
    pub fn with_ref(mut self, r#ref: &str) -> ResponseEnvelope {
        self.r#ref = Some(r#ref.to_string());
        self
    }

    /// Terminal records report how many data records preceded them.
    pub fn with_data_count(mut self, n: u64) -> ResponseEnvelope {
        if self.record_type == RecordType::Result {
            self.data_count = Some(n);
        }
        self
    }

    pub fn is_terminal(&self) -> bool {
        self.record_type == RecordType::Result
    }

    /// `status` (data records count as `ok`).
    pub fn status(&self) -> EnvelopeStatus {
        self.status.unwrap_or(EnvelopeStatus::Ok)
    }

    /// The JSON object, keys in the R1 order.
    pub fn to_json(&self) -> Json {
        let mut m = Map::new();
        if let Some(r) = &self.r#ref {
            m.insert("ref".into(), json!(r));
        }
        m.insert("request_id".into(), json!(self.request_id));
        m.insert("trace_id".into(), json!(self.trace_id));
        m.insert("operation".into(), json!(self.operation));
        m.insert("type".into(), json!(self.record_type.as_str()));
        if let Some(s) = self.seq {
            m.insert("seq".into(), json!(s));
        }
        if let Some(s) = self.status {
            m.insert("status".into(), json!(s.as_str()));
        }
        m.insert("data".into(), self.data.clone().unwrap_or(Json::Null));
        m.insert(
            "error".into(),
            self.error.as_ref().map(error_object).unwrap_or(Json::Null),
        );
        if let Some(e) = self.effects {
            m.insert("effects".into(), json!(e.as_str()));
        }
        if let Some(n) = self.data_count {
            m.insert("data_count".into(), json!(n));
        }
        Json::Object(m)
    }

    /// Compact (one line) or pretty (2-space indent), same keys and order.
    pub fn render(&self, format: OutputFormat) -> String {
        format.render(&self.to_json())
    }

    /// One-line JSON (the default everywhere).
    pub fn to_json_string(&self) -> String {
        self.render(OutputFormat::Compact)
    }

    /// 2-space indented JSON with the same key order.
    pub fn to_json_pretty(&self) -> String {
        self.render(OutputFormat::Pretty)
    }

    /// Decode a record received from a remote `rivet serve` (the inverse of
    /// [`to_json`](Self::to_json)); `None` when the object is not an envelope
    /// (a 0.1.0 server answers without `type`).
    pub fn from_json(j: &Json) -> Option<ResponseEnvelope> {
        let s = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
        let record_type = match s("type")?.as_str() {
            "result" => RecordType::Result,
            "data" => RecordType::Data,
            _ => return None,
        };
        let status = s("status").and_then(|x| EnvelopeStatus::parse(&x));
        if record_type == RecordType::Result && status.is_none() {
            return None;
        }
        let effects = s("effects").map(|x| EffectsStatus::parse(&x));
        let error = j.get("error").filter(|e| e.is_object()).map(|e| {
            let mut err = RivetError::from_value(&Value::from_json(e));
            err.effects = effects.unwrap_or_default();
            err.request_id = s("request_id").filter(|x| !x.is_empty());
            err.trace_id = s("trace_id").filter(|x| !x.is_empty());
            err
        });
        Some(ResponseEnvelope {
            r#ref: s("ref"),
            request_id: s("request_id").unwrap_or_default(),
            trace_id: s("trace_id").unwrap_or_default(),
            operation: s("operation"),
            record_type,
            seq: j.get("seq").and_then(Json::as_u64),
            status,
            data: j.get("data").filter(|d| !d.is_null()).cloned(),
            error,
            effects,
            data_count: j.get("data_count").and_then(Json::as_u64),
        })
    }

    /// Back to the Rust outcome: an ok result is a [`Completion`], an error or
    /// cancelled result is its [`RivetError`] (with the envelope's IDs).
    pub fn into_outcome(self) -> Result<Completion, RivetError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        Ok(Completion {
            request_id: self.request_id,
            trace_id: self.trace_id,
            operation: self.operation.unwrap_or_default(),
            result: self
                .data
                .as_ref()
                .map(Value::from_json)
                .unwrap_or(Value::Null),
            data_count: self.data_count.unwrap_or(0),
            effects: self.effects.unwrap_or_default(),
        })
    }
}

impl From<&Completion> for ResponseEnvelope {
    fn from(c: &Completion) -> ResponseEnvelope {
        ResponseEnvelope::from_completion(c)
    }
}

impl From<Completion> for ResponseEnvelope {
    fn from(c: Completion) -> ResponseEnvelope {
        ResponseEnvelope::from_completion(&c)
    }
}

/// The `error` object of an envelope: kind, code, message, retryable and the
/// optional source, hint, details, operation_id, node_id, cause and
/// suppressed; `effects` is reported once, at the envelope's top level, so
/// it is removed from the error and from every nested `cause` and
/// `suppressed[]` error too (INC-2026-0012 item 7).
pub fn error_object(e: &RivetError) -> Json {
    let mut j = e.to_value().to_json();
    strip_effects(&mut j);
    j
}

fn strip_effects(j: &mut Json) {
    if let Json::Object(m) = j {
        m.shift_remove("effects");
        if let Some(c) = m.get_mut("cause") {
            strip_effects(c);
        }
        if let Some(Json::Array(list)) = m.get_mut("suppressed") {
            list.iter_mut().for_each(strip_effects);
        }
    }
}

// vhco:domain InputEnvelope { operation: string; data: Value; deadline_ms?: int; restrict?: Value; stream?: bool; pretty?: bool; aliases: string[] }
/// One request as every surface accepts it. `aliases` lists the deprecated
/// 0.1.0 keys (`id`, `params`) the caller used, so the surface can emit its
/// deprecation signal.
#[derive(Clone, Debug, PartialEq)]
pub struct InputEnvelope {
    pub operation: String,
    pub data: Value,
    pub deadline_ms: Option<u64>,
    pub restrict: Option<Value>,
    pub stream: Option<bool>,
    /// Honoured by the FFI (`rivet_request`); HTTP uses `?pretty=true`, the CLI `--pretty`.
    pub pretty: Option<bool>,
    pub aliases: Vec<String>,
}

impl InputEnvelope {
    /// `{operation, data: {}}`.
    pub fn new(operation: &str) -> InputEnvelope {
        InputEnvelope {
            operation: operation.to_string(),
            data: Value::Object(Vec::new()),
            deadline_ms: None,
            restrict: None,
            stream: None,
            pretty: None,
            aliases: Vec::new(),
        }
    }

    /// Set `data` from JSON.
    pub fn data(mut self, data: Json) -> InputEnvelope {
        self.data = Value::from_json(&data);
        self
    }

    /// Set `data` from a [`Value`].
    pub fn data_value(mut self, data: Value) -> InputEnvelope {
        self.data = data;
        self
    }

    pub fn deadline_ms(mut self, ms: u64) -> InputEnvelope {
        self.deadline_ms = Some(ms);
        self
    }

    pub fn restrict(mut self, restrict: Value) -> InputEnvelope {
        self.restrict = Some(restrict);
        self
    }

    /// Whether a deprecated alias (`id`, `params`) was used.
    pub fn is_legacy(&self) -> bool {
        !self.aliases.is_empty()
    }

    /// The 0.2.0 wire form (what a client sends): `{operation, data, …}`.
    pub fn to_json(&self) -> Json {
        let mut j = json!({"operation": self.operation, "data": self.data.to_json()});
        if let Some(d) = self.deadline_ms {
            j["deadline_ms"] = json!(d);
        }
        if let Some(r) = &self.restrict {
            j["restrict"] = r.to_json();
        }
        if let Some(s) = self.stream {
            j["stream"] = json!(s);
        }
        if let Some(p) = self.pretty {
            j["pretty"] = json!(p);
        }
        j
    }
}

// vhco:domain RawInput { body: Json; legacy_ok: bool }
/// A request body handed to `serve.parse_input`. `legacy_ok` accepts the 0.1.0
/// `id`/`params` aliases (true for every 0.2.x surface).
#[derive(Clone, Debug, PartialEq)]
pub struct RawInput {
    pub body: Json,
    pub legacy_ok: bool,
}

impl RawInput {
    /// A 0.2.x body: aliases accepted.
    pub fn new(body: Json) -> RawInput {
        RawInput {
            body,
            legacy_ok: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion() -> Completion {
        Completion {
            request_id: "req_1".into(),
            trace_id: "tr_1".into(),
            operation: "demo.add".into(),
            result: Value::Int(5),
            data_count: 0,
            effects: EffectsStatus::None,
        }
    }

    fn keys(j: &Json) -> Vec<String> {
        j.as_object().unwrap().keys().cloned().collect()
    }

    // vhco:test execution.request_operation -- R1 key order on ok, error, data and ws records; pretty keeps the order
    #[test]
    fn key_order_and_formats() {
        let ok = ResponseEnvelope::from_completion(&completion());
        assert_eq!(
            keys(&ok.to_json()),
            [
                "request_id",
                "trace_id",
                "operation",
                "type",
                "status",
                "data",
                "error",
                "effects",
                "data_count"
            ]
        );
        assert_eq!(
            ok.to_json_string(),
            r#"{"request_id":"req_1","trace_id":"tr_1","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}"#
        );
        let pretty = ok.to_json_pretty();
        assert!(pretty.starts_with("{\n  \"request_id\": \"req_1\",\n  \"trace_id\""));
        let back: Json = serde_json::from_str(&pretty).unwrap();
        assert_eq!(keys(&back), keys(&ok.to_json()));

        let mut e = RivetError::validation("validation.min", "`a` must be at least 1")
            .with_effects(EffectsStatus::None);
        e.request_id = Some("req_2".into());
        let err = ResponseEnvelope::from_error(Some("demo.add"), &e).to_json();
        assert_eq!(err["status"], "error");
        assert!(err["data"].is_null());
        assert_eq!(err["error"]["code"], "validation.min");
        assert!(
            err["error"].get("effects").is_none(),
            "effects moved to the top level"
        );
        assert_eq!(err["effects"], "none");

        let c = ResponseEnvelope::from_error(Some("x"), &RivetError::consumer_stop());
        assert_eq!(c.status(), EnvelopeStatus::Cancelled);

        let rec = ResponseEnvelope::from_data(&DataEvent {
            request_id: "req_3".into(),
            trace_id: "tr_3".into(),
            operation: "demo.count".into(),
            seq: 1,
            data: Value::Int(1),
        })
        .with_ref("c1")
        .to_json();
        assert_eq!(
            keys(&rec),
            [
                "ref",
                "request_id",
                "trace_id",
                "operation",
                "type",
                "seq",
                "data",
                "error"
            ]
        );
    }

    #[test]
    fn decode_round_trip() {
        let ok = ResponseEnvelope::from_completion(&completion()).with_seq(3);
        let back = ResponseEnvelope::from_json(&ok.to_json()).unwrap();
        assert_eq!(back.clone().into_outcome().unwrap(), completion());
        assert_eq!(back.seq, Some(3));
        let mut e = RivetError::permission("no").with_effects(EffectsStatus::Partial);
        e.request_id = Some("r9".into());
        let j = ResponseEnvelope::from_error(Some("demo.add"), &e).to_json();
        let got = ResponseEnvelope::from_json(&j)
            .unwrap()
            .into_outcome()
            .unwrap_err();
        assert_eq!(
            (got.code.as_str(), got.effects),
            ("permission.denied", EffectsStatus::Partial)
        );
        assert_eq!(got.request_id.as_deref(), Some("r9"));
        // a 0.1.0 Completion is not an envelope
        assert!(ResponseEnvelope::from_json(&json!({"request_id":"r","result":1})).is_none());
    }
}
