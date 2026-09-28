//! Native gRPC client vocabulary (PROP-2026-0001 Increment 13; REF S110–S113, S117).
//!
//! The pinned FileDescriptorSet is read once at bundle load; what the rest of
//! the runtime needs from it is summarised here as plain data (connectors,
//! methods and their cardinality), so use cases never touch protobuf types.

use super::source::SourceSpan;
use super::value::Value;

// vhco:domain RpcMode { unary | server_stream | client_stream | bidi }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcMode {
    Unary,
    ServerStream,
    ClientStream,
    Bidi,
}

impl RpcMode {
    pub fn from_streaming(client: bool, server: bool) -> RpcMode {
        match (client, server) {
            (false, false) => RpcMode::Unary,
            (false, true) => RpcMode::ServerStream,
            (true, false) => RpcMode::ClientStream,
            (true, true) => RpcMode::Bidi,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RpcMode::Unary => "unary",
            RpcMode::ServerStream => "server_stream",
            RpcMode::ClientStream => "client_stream",
            RpcMode::Bidi => "bidi",
        }
    }

    /// The caller sends a stream (`rpc.send`, `rpc.finish_send`).
    pub fn client_streams(self) -> bool {
        matches!(self, RpcMode::ClientStream | RpcMode::Bidi)
    }

    /// The server answers with a stream (`for message in rpc`).
    pub fn server_streams(self) -> bool {
        matches!(self, RpcMode::ServerStream | RpcMode::Bidi)
    }
}

// vhco:domain GrpcMethodInfo { connector: string; service: string; method: string; mode: RpcMode; input_type: string; output_type: string }
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcMethodInfo {
    pub connector: String,
    /// Fully qualified service, e.g. `example.Users`.
    pub service: String,
    /// Short method name, e.g. `GetUser`.
    pub method: String,
    pub mode: RpcMode,
    pub input_type: String,
    pub output_type: String,
}

impl GrpcMethodInfo {
    /// `CONNECTOR/Fully.Qualified.Service/Method` — the allow_grpc target.
    pub fn target(&self) -> String {
        format!("{}/{}/{}", self.connector, self.service, self.method)
    }

    /// HTTP/2 `:path` of the call, `/example.Users/GetUser`.
    pub fn path(&self) -> String {
        format!("/{}/{}", self.service, self.method)
    }
}

// vhco:domain GrpcTls { server_name?: string; ca_file?: string; cert_file?: string; key_file?: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GrpcTls {
    pub server_name: Option<String>,
    pub ca_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
}

// vhco:domain GrpcConnectorInfo { name: string; endpoint: string; service: string; descriptor: string; descriptor_hash: string; tls: GrpcTls; methods: GrpcMethodInfo[] }
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcConnectorInfo {
    pub name: String,
    /// `https://host:port` (or development-only `http://host:port`).
    pub endpoint: String,
    pub service: String,
    pub descriptor: String,
    /// `sha256:<hex>` of the descriptor bytes.
    pub descriptor_hash: String,
    pub tls: GrpcTls,
    pub methods: Vec<GrpcMethodInfo>,
}

// vhco:domain GrpcCatalog { connectors: GrpcConnectorInfo[] }
/// Everything the pinned descriptor sets say about the bundle's gRPC connectors.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GrpcCatalog {
    pub connectors: Vec<GrpcConnectorInfo>,
}

impl GrpcCatalog {
    pub fn connector(&self, name: &str) -> Option<&GrpcConnectorInfo> {
        self.connectors.iter().find(|c| c.name == name)
    }

    pub fn method(&self, connector: &str, method: &str) -> Option<&GrpcMethodInfo> {
        self.connector(connector)
            .and_then(|c| c.methods.iter().find(|m| m.method == method))
    }
}

// vhco:domain GrpcUsage { one_shot | scoped }
/// How the source uses the call: `x = grpc C.M … end` or `with grpc C.M … as rpc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrpcUsage {
    OneShot,
    Scoped,
}

// vhco:domain GrpcMetadataValue { ascii: string | binary: bytes }
#[derive(Clone, Debug, PartialEq)]
pub enum GrpcMetadataValue {
    Ascii(String),
    Binary(Vec<u8>),
}

// vhco:domain GrpcPlan { connector: string; method: string; usage: GrpcUsage; message?: Value; metadata: (string, GrpcMetadataValue)[]; timeout_ms?: int; deadline_ms: int; auth?: (string, string); operation_id: string; span?: SourceSpan }
/// One evaluated `grpc …` / `with grpc …` form.
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcPlan {
    pub connector: String,
    pub method: String,
    pub usage: GrpcUsage,
    /// The `message {…}` option (unary and server-streaming only).
    pub message: Option<Value>,
    /// `metadata K V` lines in source order; duplicates are preserved.
    pub metadata: Vec<(String, GrpcMetadataValue)>,
    /// `timeout "D"`.
    pub timeout_ms: Option<u64>,
    /// What is left of the request deadline when the form starts.
    pub deadline_ms: u64,
    /// `auth PROFILE account ACCOUNT`.
    pub auth: Option<(String, String)>,
    pub operation_id: String,
    pub span: Option<SourceSpan>,
}

// vhco:domain GrpcStatus { code: int; name: string; message: string }
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcStatus {
    pub code: i32,
    pub name: String,
    pub message: String,
}

impl GrpcStatus {
    pub fn ok() -> GrpcStatus {
        GrpcStatus::from_code(0, "")
    }

    pub fn from_code(code: i32, message: impl Into<String>) -> GrpcStatus {
        GrpcStatus {
            code,
            name: status_name(code).to_string(),
            message: message.into(),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.code == 0
    }
}

/// Canonical gRPC status names (https://grpc.io/docs/guides/status-codes/).
pub fn status_name(code: i32) -> &'static str {
    match code {
        0 => "OK",
        1 => "CANCELLED",
        2 => "UNKNOWN",
        3 => "INVALID_ARGUMENT",
        4 => "DEADLINE_EXCEEDED",
        5 => "NOT_FOUND",
        6 => "ALREADY_EXISTS",
        7 => "PERMISSION_DENIED",
        8 => "RESOURCE_EXHAUSTED",
        9 => "FAILED_PRECONDITION",
        10 => "ABORTED",
        11 => "OUT_OF_RANGE",
        12 => "UNIMPLEMENTED",
        13 => "INTERNAL",
        14 => "UNAVAILABLE",
        15 => "DATA_LOSS",
        16 => "UNAUTHENTICATED",
        _ => "UNKNOWN",
    }
}

// vhco:domain GrpcTerminal { status: GrpcStatus; initial_metadata: Value; trailers: Value }
/// The end of one call: final status plus the (already redacted) metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcTerminal {
    pub status: GrpcStatus,
    pub initial_metadata: Value,
    pub trailers: Value,
}

// vhco:domain GrpcEvent { message: Value | end: GrpcTerminal }
/// What the receive side of a call yields, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum GrpcEvent {
    Message(Value),
    End(GrpcTerminal),
}

// vhco:domain GrpcResult { message?: Value; initial_metadata: Value; trailers: Value; status: GrpcStatus; data_count: int }
/// GrpcResponse `{message, initial_metadata, trailers, status}` after a final OK.
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcResult {
    pub message: Option<Value>,
    pub initial_metadata: Value,
    pub trailers: Value,
    pub status: GrpcStatus,
    pub data_count: u64,
}

impl GrpcResult {
    /// Script-visible value: `response.message`, `rpc.completion.status`, …
    pub fn to_value(&self) -> Value {
        let mut v = Value::Object(Vec::new());
        if let Some(m) = &self.message {
            v.set("message", m.clone());
        }
        v.set("initial_metadata", self.initial_metadata.clone());
        v.set("trailers", self.trailers.clone());
        v.set("status", Value::text(&self.status.name));
        v.set("status_code", Value::Int(self.status.code as i64));
        v.set("data_count", Value::Int(self.data_count as i64));
        v
    }
}

// vhco:domain GrpcDial { method: GrpcMethodInfo; endpoint: string; addresses: string[]; tls: GrpcTls; metadata: (string, GrpcMetadataValue)[]; timeout_ms: int }
/// A fully authorized call the transport may start: the broker-checked
/// addresses are the only ones it may dial (tried in order).
#[derive(Clone, Debug, PartialEq)]
pub struct GrpcDial {
    pub method: GrpcMethodInfo,
    pub endpoint: String,
    pub addresses: Vec<std::net::SocketAddr>,
    pub tls: GrpcTls,
    pub metadata: Vec<(String, GrpcMetadataValue)>,
    pub timeout_ms: u64,
}

/// `CONNECTOR.Method` named by a `grpc` form head. Lowering keeps the head as
/// a bare word (`users.GetUser`) because it names a descriptor method, not a
/// variable; a dotted path is accepted too.
pub fn method_ref(head: &[super::ir::Arg]) -> Option<(String, String)> {
    use super::ir::{Arg, Expr};
    let dotted = match head.first()? {
        Arg::Word(w, _) => w.clone(),
        Arg::Expr(Expr::Path(p, _), _) => p.join("."),
        _ => return None,
    };
    let (connector, method) = dotted.split_once('.')?;
    if connector.is_empty() || method.is_empty() || method.contains('.') {
        return None;
    }
    Some((connector.to_string(), method.to_string()))
}
