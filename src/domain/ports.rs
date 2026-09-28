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
