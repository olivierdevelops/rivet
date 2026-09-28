//! Port traits. Each feature's `ports.rs` re-exports the traits it needs and
//! carries the `vhco:port` annotation; infra adapters implement these traits.
//! Keeping the definitions here lets `features`, `infra` and `io` depend only
//! on `domain` (AGENTS.md five-folder rule).

use super::contracts::{
    Catalog, CatalogQuery, Completion, DataEvent, ExecutionPlan, Request, RunOutcome,
};
use super::errors::RivetResult;
use super::files::FileOperation;
use super::io_manifest::{
    EffectCatalog, FileProbeInput, FileProbeResult, PolicyDraftFile, PolicyDraftReceipt,
    TraceEvent, TraceQuery, TraceResult,
};
use super::ir::CompiledProgram;
use super::policy::{EffectIntent, Permit, Policy};
use super::source::{SourceBundle, SourceFile};
use super::syntax_tree::SyntaxTree;
use super::transport::{DatagramPlan, DatagramResult, QuicPlan, QuicResult};
use super::value::Value;
use async_trait::async_trait;
use std::sync::Arc;

/// Parses one source file of a bundle into Rivet's SyntaxTree.
pub trait Parser: Send + Sync {
    fn parse(&self, file: &SourceFile) -> RivetResult<SyntaxTree>;
}

/// Immutable operation catalog built from a compiled program.
pub trait Registry: Send + Sync {
    fn describe(&self, query: &CatalogQuery) -> RivetResult<Catalog>;
    fn program(&self) -> Arc<CompiledProgram>;
    /// Lowered effect sites of every operation (computed once at assembly;
    /// empty when the host did not attach an effect analysis).
    fn effect_sites(&self) -> Arc<EffectCatalog> {
        Arc::new(EffectCatalog::default())
    }
}

/// Bounded store of broker decisions/attempts per request (audit trace).
pub trait TraceStore: Send + Sync {
    fn record(&self, event: TraceEvent);
    /// A request's events in order; not_found when the store has none.
    fn read(&self, query: &TraceQuery) -> RivetResult<TraceResult>;
}

/// Metadata probe of one needed file (`io --check-files`); never reads content.
pub trait FileProbe: Send + Sync {
    fn stat(&self, input: &FileProbeInput) -> FileProbeResult;
}

/// Writes a generated policy draft, refusing to overwrite any existing path.
pub trait PolicyDraftWriter: Send + Sync {
    fn write_new(&self, file: &PolicyDraftFile) -> RivetResult<PolicyDraftReceipt>;
}

/// Receives streamed data items. Returning an error stops the producer and
/// cancels the request (consumer_failed).
#[async_trait]
pub trait DataSink: Send + Sync {
    async fn send(&self, event: DataEvent) -> RivetResult<()>;
}

/// Nested `(request …)` calls re-enter the shared dispatcher through this port
/// so validation, visibility, limits and output checks apply at every depth.
#[async_trait]
pub trait Dispatcher: Send + Sync {
    async fn dispatch(
        &self,
        request: Request,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<Completion>;
}

/// Interprets a validated plan inside a supervised scope.
#[async_trait]
pub trait ExecutionDriver: Send + Sync {
    async fn drive(
        &self,
        plan: ExecutionPlan,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<RunOutcome>;
}

/// Runtime target gate: decides every actual effect attempt.
pub trait PolicyEvaluator: Send + Sync {
    fn evaluate(&self, intent: &EffectIntent) -> Permit;
    fn policy(&self) -> &Policy;
}

// vhco:domain PolicyLocator { entry: string; explicit_path?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyLocator {
    pub entry: String,
    pub explicit_path: Option<String>,
}

// vhco:domain PolicyDiscovery { path?: string; explicit: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyDiscovery {
    /// `None` = no file found (only possible for auto-discovery).
    pub path: Option<String>,
    pub explicit: bool,
}

// vhco:domain PolicyBytes { path: string; bytes: bytes; sha256: string }
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyBytes {
    pub path: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

/// Finds and reads policy.json (host bootstrap I/O, outside script authority).
pub trait PolicyFileReader: Send + Sync {
    /// `--policy PATH` when given (missing → error), else `policy.json` beside the entry file.
    fn discover(&self, locator: &PolicyLocator) -> RivetResult<PolicyDiscovery>;
    fn read(&self, discovery: &PolicyDiscovery) -> RivetResult<PolicyBytes>;
}

/// Confined file CRUD through broker permits.
#[async_trait]
pub trait FileAccess: Send + Sync {
    async fn apply(&self, op: FileOperation) -> RivetResult<Value>;
}

/// Loads a whole bundle from disk (host bootstrap, before compilation).
pub trait SourceLoader: Send + Sync {
    fn load(&self, entry: &str) -> RivetResult<SourceBundle>;
}

/// One scoped UDP socket (`with udp …`): resolves peers for the broker check,
/// then runs already-authorized steps on its own socket.
#[async_trait]
pub trait DatagramDriver: Send + Sync {
    /// Resolve `host` (DNS or literal) to candidate addresses; no packet is sent.
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<std::net::SocketAddr>>;
    async fn exchange(&self, plan: DatagramPlan) -> RivetResult<DatagramResult>;
}

/// One scoped native QUIC connection (`with quic …`) and its child streams.
#[async_trait]
pub trait QuicDriver: Send + Sync {
    /// Resolve `host` (DNS or literal) to candidate addresses; no packet is sent.
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<std::net::SocketAddr>>;
    async fn exchange(&self, plan: QuicPlan) -> RivetResult<QuicResult>;
}

/// Native gRPC transport over HTTP/2 driven by pinned descriptors. It never
/// authorizes anything itself: callers pass an already authorized `GrpcDial`.
#[async_trait]
pub trait GrpcDriver: Send + Sync {
    /// Connectors and methods from the pinned descriptor sets (read at load).
    fn catalog(&self) -> &super::grpc::GrpcCatalog;
    /// Check a ProtoJSON value against the method's input type (no I/O).
    fn validate_input(
        &self,
        method: &super::grpc::GrpcMethodInfo,
        message: &Value,
    ) -> RivetResult<()>;
    /// Resolve a host name to candidate addresses (the caller checks each one).
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<std::net::IpAddr>>;
    /// Dial the checked address and start the call; returns its two halves.
    async fn invoke(
        &self,
        dial: super::grpc::GrpcDial,
    ) -> RivetResult<(Box<dyn GrpcSender>, Box<dyn GrpcReceiver>)>;
}

/// Send half of a started call (request messages, then half-close).
#[async_trait]
pub trait GrpcSender: Send {
    /// Encode (ProtoJSON → protobuf, validated against the input type) and queue.
    async fn send(&mut self, message: Value) -> RivetResult<()>;
    /// Half-close the request stream; idempotent.
    async fn finish(&mut self);
}

/// Receive half of a started call: messages in order, then exactly one End.
#[async_trait]
pub trait GrpcReceiver: Send {
    async fn next_event(&mut self) -> RivetResult<super::grpc::GrpcEvent>;
    /// Cancel the call (RST_STREAM) if it has not finished.
    fn cancel(&mut self);
}

/// A scope-owned gRPC call with cardinality and status rules applied. Both
/// halves may be used concurrently (bidi: one task sends, another receives).
#[async_trait]
pub trait GrpcCall: Send + Sync {
    /// `rpc.send V`.
    async fn send(&self, message: Value) -> RivetResult<()>;
    /// `rpc.finish_send` (idempotent half-close).
    async fn finish_send(&self) -> RivetResult<()>;
    /// `for message in rpc`; `Ok(None)` only after a final OK status.
    async fn next(&self) -> RivetResult<Option<Value>>;
    /// `rpc.result` / unary response: the single message plus final OK.
    async fn result(&self) -> RivetResult<super::grpc::GrpcResult>;
    /// `rpc.completion`: final OK status, metadata and trailers.
    async fn completion(&self) -> RivetResult<super::grpc::GrpcResult>;
    /// Scope exit: cancel the call unless it already ended.
    async fn close(&self);
}

/// Live sessions: principal-owned request lifetimes with sequenced input and a
/// bounded replayable event log (sessions feature, polling, WebSocket, MCP).
#[async_trait]
pub trait SessionDriver: Send + Sync {
    async fn open(
        &self,
        input: super::sessions::SessionOpenInput,
    ) -> RivetResult<super::sessions::SessionReceipt>;
    async fn send(
        &self,
        input: super::sessions::SessionSendInput,
    ) -> RivetResult<super::sessions::SessionAck>;
    async fn finish_input(
        &self,
        input: super::sessions::SessionRef,
    ) -> RivetResult<super::sessions::SessionAck>;
    async fn read(
        &self,
        input: super::sessions::SessionReadInput,
    ) -> RivetResult<super::sessions::SessionBatch>;
    async fn cancel(
        &self,
        input: super::sessions::SessionRef,
    ) -> RivetResult<super::sessions::CancelReceipt>;
    /// Host limits the use cases clamp against.
    fn limits(&self) -> super::sessions::SessionLimits;
}

/// Library-host authentication callback (replaces the policy-driven serve.auth).
pub trait Authenticator: Send + Sync {
    fn authenticate(
        &self,
        input: &super::serve::AuthnInput,
    ) -> RivetResult<super::contracts::Principal>;
}

/// The single serve listener: bind once, then mount each enabled surface.
#[async_trait]
pub trait ServeListener: Send {
    async fn bind(
        &mut self,
        config: super::serve::ServeConfig,
    ) -> RivetResult<super::serve::ListenerHandle>;
    async fn mount(
        &mut self,
        mount: super::serve::SurfaceMount,
    ) -> RivetResult<super::serve::MountReceipt>;
}

/// Outbound side of one WebSocket connection.
#[async_trait]
pub trait WsConnection: Send + Sync {
    async fn send(&self, frame: super::serve::WsFrame) -> RivetResult<()>;
}

// vhco:domain CancelRequest { request_id: string; principal: Principal }
#[derive(Clone, Debug, PartialEq)]
pub struct CancelRequest {
    pub request_id: String,
    pub principal: super::contracts::Principal,
}

/// Owner of the running top-level requests' cancellation signals.
pub trait RequestControl: Send + Sync {
    /// The principal that owns a running request, or its terminal state if it
    /// finished recently; `None` when the ID is unknown.
    fn lookup(&self, request_id: &str) -> Option<RequestState>;
    /// Signal cancellation; idempotent. Returns false when nothing was running.
    fn signal(&self, request_id: &str) -> bool;
}

// vhco:domain RequestState { owner: string; state: string }
#[derive(Clone, Debug, PartialEq)]
pub struct RequestState {
    pub owner: String,
    /// `running`, `cancelling`, `succeeded`, `failed` or `cancelled`.
    pub state: String,
}
