//! Port traits. Each feature's `ports.rs` re-exports the traits it needs and
//! carries the `vhco:port` annotation; infra adapters implement these traits.
//! Keeping the definitions here lets `features`, `infra` and `io` depend only
//! on `domain` (AGENTS.md five-folder rule).

use super::contracts::{
    Catalog, CatalogQuery, Completion, DataEvent, ExecutionPlan, Request, RunOutcome,
};
use super::errors::RivetResult;
use super::files::FileOperation;
use super::ir::CompiledProgram;
use super::policy::{EffectIntent, Permit, Policy};
use super::source::{SourceBundle, SourceFile};
use super::syntax_tree::SyntaxTree;
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
