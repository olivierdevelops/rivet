//! One serve, every surface (PROP-2026-0001 Increment 17): listener, principal,
//! operation-access, WebSocket frame and polling-route contracts.
//!
//! ```text
//!   rivet serve --listen ADDR ─▶ bind once ─▶ mount http | sse | poll | ws | mcp
//!   request ─▶ authenticate (none | bearer | mtls) ─▶ Principal ─▶ authorize operation ─▶ dispatcher
//! ```

use super::contracts::{Completion, Principal};
use super::envelope::{EnvelopeStatus, InputEnvelope, RecordType, ResponseEnvelope};
use super::errors::RivetError;
use super::policy::{ServeAuth, ServePolicy};
use super::value::Value;
use serde_json::{Value as Json, json};

/// Default `--listen` address.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:8080";

/// MCP protocol revision served at `/mcp` and over stdio.
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

/// WebSocket subprotocol.
pub const WS_SUBPROTOCOL: &str = "rivet.v1";

// vhco:domain ServeStartInput { listen?: string; stdio: bool; serve: ServePolicy; catalog_version: string; policy_hash?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct ServeStartInput {
    pub listen: Option<String>,
    pub stdio: bool,
    pub serve: ServePolicy,
    pub catalog_version: String,
    pub policy_hash: Option<String>,
}

// vhco:domain ServeConfig { listen: string; loopback: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct ServeConfig {
    pub listen: String,
    pub loopback: bool,
}

// vhco:domain ListenerHandle { listen_addr: string; loopback: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct ListenerHandle {
    pub listen_addr: String,
    pub loopback: bool,
}

// vhco:domain SurfaceMount { surface: string; routes: string[]; listener: ListenerHandle }
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceMount {
    pub surface: String,
    pub routes: Vec<String>,
    pub listener: ListenerHandle,
}

// vhco:domain MountReceipt { surface: string; mounted: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct MountReceipt {
    pub surface: String,
    pub mounted: bool,
}

// vhco:domain ServeReceipt { listen_addr?: string; stdio: bool; surfaces: string[]; auth_type: string; catalog_version: string; policy_hash?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct ServeReceipt {
    pub listen_addr: Option<String>,
    pub stdio: bool,
    pub surfaces: Vec<String>,
    pub auth_type: String,
    pub catalog_version: String,
    pub policy_hash: Option<String>,
}

impl ServeReceipt {
    pub fn to_json(&self) -> Json {
        json!({
            "listen_addr": self.listen_addr,
            "stdio": self.stdio,
            "surfaces": self.surfaces,
            "auth_type": self.auth_type,
            "catalog_version": self.catalog_version,
            "policy_hash": self.policy_hash,
        })
    }
}

/// Routes each surface mounts on the shared listener.
pub fn surface_routes(surface: &str) -> Vec<String> {
    let r: &[&str] = match surface {
        "http" => &[
            "POST /v1/request",
            "GET /v1/operations",
            "GET /v1/operations/{id}",
            "GET /v1/operations/{id}/outputs",
        ],
        "sse" => &["POST /v1/request (Accept: text/event-stream)"],
        "poll" => &[
            "POST /v1/requests",
            "GET /v1/requests/{id}/events",
            "POST /v1/requests/{id}/input",
            "POST /v1/requests/{id}/finish_input",
            "POST /v1/requests/{id}/cancel",
        ],
        "ws" => &["GET /v1/ws"],
        "mcp" => &["POST /mcp", "GET /mcp", "DELETE /mcp"],
        _ => &[],
    };
    r.iter().map(|s| s.to_string()).collect()
}

pub fn auth_type_name(auth: &ServeAuth) -> &'static str {
    match auth {
        ServeAuth::None => "none",
        ServeAuth::Bearer(_) => "bearer",
        ServeAuth::Mtls { .. } => "mtls",
    }
}

// vhco:domain AuthnInput { surface: string; remote_addr: string; bind_is_loopback: bool; authorization?: string; client_cert_subject?: string; auth: ServeAuth }
/// Everything an authenticator may look at. `authorization` is the raw header
/// value and is never logged or echoed.
#[derive(Clone, PartialEq)]
pub struct AuthnInput {
    pub surface: String,
    pub remote_addr: String,
    pub bind_is_loopback: bool,
    pub authorization: Option<String>,
    pub client_cert_subject: Option<String>,
    pub auth: ServeAuth,
}

impl std::fmt::Debug for AuthnInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthnInput")
            .field("surface", &self.surface)
            .field("remote_addr", &self.remote_addr)
            .field("bind_is_loopback", &self.bind_is_loopback)
            .field(
                "authorization",
                &self.authorization.as_ref().map(|_| "<redacted>"),
            )
            .field("client_cert_subject", &self.client_cert_subject)
            .finish()
    }
}

// vhco:domain OperationAccess { principal: Principal; operation_id: string; serve: ServePolicy }
#[derive(Clone, Debug, PartialEq)]
pub struct OperationAccess {
    pub principal: Principal,
    pub operation_id: String,
    pub serve: ServePolicy,
}

// vhco:domain AccessDecision { allowed: bool; principal: string; matched_pattern?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct AccessDecision {
    pub allowed: bool,
    pub principal: String,
    pub matched_pattern: Option<String>,
}

// vhco:domain WsFrameType { request | input | finish_input | cancel | data | result | error }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WsFrameType {
    Request,
    Input,
    FinishInput,
    Cancel,
    Data,
    Result,
    Error,
}

impl WsFrameType {
    pub fn as_str(self) -> &'static str {
        match self {
            WsFrameType::Request => "request",
            WsFrameType::Input => "input",
            WsFrameType::FinishInput => "finish_input",
            WsFrameType::Cancel => "cancel",
            WsFrameType::Data => "data",
            WsFrameType::Result => "result",
            WsFrameType::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<WsFrameType> {
        Some(match s {
            "request" => WsFrameType::Request,
            "input" => WsFrameType::Input,
            "finish_input" => WsFrameType::FinishInput,
            "cancel" => WsFrameType::Cancel,
            _ => return None,
        })
    }
}

// vhco:domain WsFrame { type: WsFrameType; ref: string; input?: InputEnvelope; seq?: int; data?: Value; record?: ResponseEnvelope }
/// One JSON text frame on `/v1/ws` (client or server direction). Client
/// request frames carry an [`InputEnvelope`] (`{type:"request", ref,
/// operation, data}`), input frames `seq` + `data`; server frames carry one
/// [`ResponseEnvelope`] record, written with `ref` first.
#[derive(Clone, Debug, PartialEq)]
pub struct WsFrame {
    pub kind: WsFrameType,
    pub r#ref: String,
    /// Request frames: the parsed input envelope.
    pub input: Option<InputEnvelope>,
    /// Input frames: the send sequence.
    pub seq: Option<u64>,
    /// Input frames: the item.
    pub data: Option<Value>,
    /// Server frames: the record (data item or terminal result).
    pub record: Option<ResponseEnvelope>,
}

impl WsFrame {
    pub fn empty(kind: WsFrameType, r#ref: &str) -> WsFrame {
        WsFrame {
            kind,
            r#ref: r#ref.to_string(),
            input: None,
            seq: None,
            data: None,
            record: None,
        }
    }

    /// A server frame for one record: `data` items stay `data`, a terminal
    /// `ok` result is `result`, an error or cancelled result is `error`.
    pub fn from_record(r#ref: &str, record: ResponseEnvelope) -> WsFrame {
        let kind = match (record.record_type, record.status()) {
            (RecordType::Data, _) => WsFrameType::Data,
            (_, EnvelopeStatus::Ok) | (_, EnvelopeStatus::Accepted) => WsFrameType::Result,
            _ => WsFrameType::Error,
        };
        WsFrame {
            record: Some(record.with_ref(r#ref)),
            ..WsFrame::empty(kind, r#ref)
        }
    }

    /// A terminal error frame for `operation` (when known); it carries the
    /// error's request/trace ids when the error names them.
    pub fn error_for(r#ref: &str, operation: Option<&str>, error: RivetError) -> WsFrame {
        WsFrame::from_record(r#ref, ResponseEnvelope::from_error(operation, &error))
    }

    /// A terminal error frame (operation unknown or not relevant).
    pub fn error(r#ref: &str, error: RivetError) -> WsFrame {
        WsFrame::error_for(r#ref, None, error)
    }

    /// A data record with no request context (tests and synthetic frames).
    pub fn data(r#ref: &str, seq: u64, data: Value) -> WsFrame {
        WsFrame::from_record(
            r#ref,
            ResponseEnvelope::from_data(&super::contracts::DataEvent {
                request_id: String::new(),
                trace_id: String::new(),
                operation: String::new(),
                seq,
                data,
            }),
        )
    }

    /// A terminal ok result.
    pub fn result(r#ref: &str, completion: Completion) -> WsFrame {
        WsFrame::from_record(r#ref, ResponseEnvelope::from_completion(&completion))
    }

    /// The error a terminal error frame carries.
    pub fn error_ref(&self) -> Option<&RivetError> {
        self.record.as_ref().and_then(|r| r.error.as_ref())
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.kind, WsFrameType::Result | WsFrameType::Error)
    }

    /// Server frame JSON: the record with `ref` first. A frame without a
    /// record (never sent by the server) renders `{type, ref}`.
    pub fn to_json(&self) -> Json {
        match &self.record {
            Some(r) => r.to_json(),
            None => json!({"type": self.kind.as_str(), "ref": self.r#ref}),
        }
    }
}

// vhco:domain WsInbound { frame: WsFrame; principal: Principal; serve: ServePolicy; open_refs: string[]; trace?: TraceContext }
/// One parsed client frame plus the connection facts the multiplexer needs.
#[derive(Clone, Debug, PartialEq)]
pub struct WsInbound {
    /// Parsed frame, or the parse failure (with the ref when it could be read).
    pub frame: Result<WsFrame, (String, RivetError)>,
    pub principal: Principal,
    pub serve: ServePolicy,
    /// `(ref, session_id)` of every ref still in flight on this connection.
    pub open_refs: Vec<(String, String)>,
    /// W3C `traceparent` of the upgrade request (refs share its trace id).
    pub trace: Option<crate::domain::contracts::TraceContext>,
}

// vhco:domain WsOutcome { opened?: string; session_id?: string; replied: bool; ended?: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct WsOutcome {
    /// `(ref, session_id)` of a newly opened ref; the host starts its event pump.
    pub opened: Option<(String, String)>,
    /// An immediate frame (error) was sent for this client frame.
    pub replied: bool,
    /// A ref ended by a refused input/finish_input frame: its terminal error
    /// frame (with the specific code) was sent and its session cancelled.
    pub ended: Option<String>,
}

// vhco:domain WsClose { code: int; reason: string }
#[derive(Clone, Debug, PartialEq)]
pub struct WsClose {
    pub code: u16,
    pub reason: String,
}

// vhco:domain PollAction { open | events | input | finish_input | cancel }
#[derive(Clone, Debug, PartialEq)]
pub enum PollAction {
    Open {
        id: String,
        params: Value,
        /// Requested total deadline (capped by the host).
        deadline_ms: Option<u64>,
        restrict: Option<Value>,
    },
    Events {
        session_id: String,
        after_seq: u64,
        wait_ms: Option<u32>,
        max_events: Option<u32>,
    },
    Input {
        session_id: String,
        send_seq: u64,
        data: Value,
    },
    FinishInput {
        session_id: String,
    },
    Cancel {
        session_id: String,
    },
}

// vhco:domain PollRoute { action: PollAction; principal: Principal; serve: ServePolicy; trace?: TraceContext }
#[derive(Clone, Debug, PartialEq)]
pub struct PollRoute {
    pub action: PollAction,
    pub principal: Principal,
    pub serve: ServePolicy,
    /// W3C `traceparent` of the HTTP request (used when a session is opened).
    pub trace: Option<crate::domain::contracts::TraceContext>,
}

// vhco:domain PollResponse { status: int; body: Json }
#[derive(Clone, Debug, PartialEq)]
pub struct PollResponse {
    pub status: u16,
    pub body: Json,
}
