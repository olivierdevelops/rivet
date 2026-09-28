//! One serve, every surface (PROP-2026-0001 Increment 17): listener, principal,
//! operation-access, WebSocket frame and polling-route contracts.
//!
//! ```text
//!   rivet serve --listen ADDR ─▶ bind once ─▶ mount http | sse | poll | ws | mcp
//!   request ─▶ authenticate (none | bearer | mtls) ─▶ Principal ─▶ authorize operation ─▶ dispatcher
//! ```

use super::contracts::{Completion, Principal};
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

// vhco:domain WsFrame { type: WsFrameType; ref: string; id?: string; params?: Value; seq?: int; data?: Value; completion?: Completion; error?: RivetError; request_id?: string; trace_id?: string }
/// One JSON text frame on `/v1/ws` (client or server direction).
#[derive(Clone, Debug, PartialEq)]
pub struct WsFrame {
    pub kind: WsFrameType,
    pub r#ref: String,
    pub id: Option<String>,
    pub params: Option<Value>,
    pub seq: Option<u64>,
    pub data: Option<Value>,
    pub completion: Option<Completion>,
    pub error: Option<RivetError>,
    /// Server data/error frames name the request they belong to (the result
    /// frame carries them inside its Completion).
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

impl WsFrame {
    pub fn empty(kind: WsFrameType, r#ref: &str) -> WsFrame {
        WsFrame {
            kind,
            r#ref: r#ref.to_string(),
            id: None,
            params: None,
            seq: None,
            data: None,
            completion: None,
            error: None,
            request_id: None,
            trace_id: None,
        }
    }

    /// Stamp the request this data/error frame belongs to.
    pub fn for_request(mut self, request_id: &str, trace_id: &str) -> WsFrame {
        self.request_id = Some(request_id.to_string());
        self.trace_id = Some(trace_id.to_string());
        self
    }

    pub fn error(r#ref: &str, error: RivetError) -> WsFrame {
        WsFrame {
            error: Some(error),
            ..WsFrame::empty(WsFrameType::Error, r#ref)
        }
    }

    pub fn data(r#ref: &str, seq: u64, data: Value) -> WsFrame {
        WsFrame {
            seq: Some(seq),
            data: Some(data),
            ..WsFrame::empty(WsFrameType::Data, r#ref)
        }
    }

    pub fn result(r#ref: &str, completion: Completion) -> WsFrame {
        WsFrame {
            completion: Some(completion),
            ..WsFrame::empty(WsFrameType::Result, r#ref)
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.kind, WsFrameType::Result | WsFrameType::Error)
    }

    /// Server frame JSON: `{type, ref, request_id?, trace_id?, seq?, data?, completion?, error?}`.
    pub fn to_json(&self) -> Json {
        let mut m = serde_json::Map::new();
        m.insert("type".into(), json!(self.kind.as_str()));
        m.insert("ref".into(), json!(self.r#ref));
        if let Some(r) = &self.request_id {
            m.insert("request_id".into(), json!(r));
        }
        if let Some(t) = &self.trace_id {
            m.insert("trace_id".into(), json!(t));
        }
        if let Some(id) = &self.id {
            m.insert("id".into(), json!(id));
        }
        if let Some(p) = &self.params {
            m.insert("params".into(), p.to_json());
        }
        if let Some(s) = self.seq {
            m.insert("seq".into(), json!(s));
        }
        if let Some(d) = &self.data {
            m.insert("data".into(), d.to_json());
        }
        if let Some(c) = &self.completion {
            m.insert("completion".into(), c.to_json());
        }
        if let Some(e) = &self.error {
            m.insert("error".into(), e.to_value().to_json());
        }
        Json::Object(m)
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
