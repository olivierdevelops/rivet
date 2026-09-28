//! Composition root for one loaded bundle: compiles the program, loads the
//! policy, wires every use case to its adapters and exposes the single
//! dispatcher used by the CLI, HTTP, MCP, WebSocket, polling and the library.

use crate::domain::auth::{CredentialInput, CredentialLease, OAuthProfile};
use crate::domain::contracts::{
    Catalog, CatalogQuery, Completion, DEFAULT_DEADLINE_MS, OutputReport, Principal, Request,
    TraceContext, span_id,
};
use crate::domain::errors::ErrorKind;
use crate::domain::files::FileOperation;
use crate::domain::io_manifest::{
    IoQuery, IoReport, PolicyDraft, TraceEvent, TraceQuery, TraceResult,
};
use crate::domain::ir::CompiledProgram;
use crate::domain::mcp::{
    BridgeHops, McpContext, McpImportKind, McpRequest, McpSnapshot, snapshot_hash,
};
use crate::domain::modules::CatalogSnapshot;
use crate::domain::policy::{AccessVerb, Capability, Decision};
use crate::domain::policy::{EffectIntent, Permit, Policy};
#[cfg(feature = "grpc")]
use crate::domain::ports::GrpcDriver;
use crate::domain::ports::McpClient;
use crate::domain::ports::TraceStore;
use crate::domain::ports::{CredentialProvider, OAuthSessionDriver};
use crate::domain::ports::{
    DataSink, Dispatcher, FileAccess, PolicyEvaluator, PolicyLocator, Registry, SessionDriver,
    SourceLoader,
};
use crate::domain::serve::OperationAccess;
use crate::domain::source::SourceBundle;
use crate::domain::transports::ProcessRunner;
use crate::domain::{RivetError, RivetResult, Value};
use crate::features::audit::effect_sites::analyze_program;
use crate::features::audit::inspect_effects::{AuditPorts, inspect_effects};
use crate::features::audit::read_trace::read_trace;
use crate::features::auth::acquire_credential::acquire_credential;
use crate::features::connectors::invoke_mcp::invoke_mcp;
use crate::features::datagrams::exchange_datagrams::exchange_datagrams;
use crate::features::execution::request_operation::request_operation;
use crate::features::files::apply_file_operation::{FileRequest, apply_file_operation};
#[cfg(feature = "grpc")]
use crate::features::grpc::invoke_rpc::{check_program as check_grpc_program, invoke_rpc};
use crate::features::language::compile_program::compile_program;
use crate::features::language::resolve_imports::resolve_imports;
use crate::features::policy::authorize_effect::authorize_effect;
use crate::features::policy::generate_policy::{PolicyGenerateInput, generate_policy};
use crate::features::policy::load_policy::{load_policy, parse_policy};
#[cfg(feature = "quic")]
use crate::features::quic::exchange_quic::exchange_quic;
use crate::features::registry::describe_operations::describe_operations;
use crate::features::registry::inspect_outputs::{OutputQuery, inspect_outputs};
use crate::features::serve::authorize_operation::require_operation;
use crate::features::transports::exchange_http::exchange_http;
use crate::features::transports::run_process::confine_process;
use crate::infra::capy_parser::CapyParser;
use crate::infra::execution_driver::Interpreter;
use crate::infra::file_access::ConfinedFiles;
#[cfg(feature = "grpc")]
use crate::infra::grpc_adapter::{GrpcEffects, GrpcTransport, InvokeFn};
use crate::infra::mcp_client::{McpHttpFn, McpPeer, McpSpawnFn};
#[cfg(feature = "oauth")]
use crate::infra::oauth_adapter::OAuthAdapter;
use crate::infra::policy_broker::PolicyBroker;
use crate::infra::policy_draft_writer::ExclusiveDraftWriter;
use crate::infra::policy_file_reader::DiskPolicyReader;
#[cfg(feature = "quic")]
use crate::infra::quic_adapter::QuicAdapter;
use crate::infra::registry::ProgramRegistry;
use crate::infra::session_driver::SessionHost;
use crate::infra::source_loader::DiskSourceLoader;
use crate::infra::trace_store::{MemoryTraceStore, current_effect_scope};
use crate::infra::udp_adapter::UdpAdapter;
#[cfg(not(feature = "oauth"))]
use crate::infra::unsupported_features::oauth::NoOAuth as OAuthAdapter;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use tokio::sync::Semaphore;

/// How the host supplies policy.
pub enum PolicySource {
    /// Discover policy.json beside the entry file (or none → deny-by-default).
    Discover,
    /// `--policy PATH`.
    File(String),
    /// Library host: an already-parsed policy.
    Given(Box<Policy>),
}

/// Builder for a runtime over one bundle.
pub struct RuntimeBuilder {
    entry: Option<String>,
    source: Option<SourceBundle>,
    /// `.root(DIR)`: a runtime with no entry file; modules are loaded later.
    root: Option<String>,
    policy: PolicySource,
    /// `connectors sync`: MCP connectors may lack a reviewed snapshot.
    discovery: bool,
    /// Library host ceiling intersected with the loaded policy (G10).
    ceiling: Option<Policy>,
    /// Host session caps (idle lease, retention, queues); Increment 14 defaults.
    session_limits: crate::domain::sessions::SessionLimits,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        RuntimeBuilder {
            entry: None,
            source: None,
            root: None,
            policy: PolicySource::Discover,
            discovery: false,
            ceiling: None,
            session_limits: crate::domain::sessions::SessionLimits::default(),
        }
    }
}

impl RuntimeBuilder {
    /// Load the entry `.rivet` file from disk (bootstrap I/O).
    pub fn file(mut self, path: &str) -> Self {
        self.entry = Some(path.to_string());
        self
    }

    /// Compile from in-memory source (no file read); `root` anchors relative paths.
    pub fn source(mut self, path: &str, text: &str, root: &str) -> Self {
        let mut b = SourceBundle::single(path, text);
        b.root = root.to_string();
        self.source = Some(b);
        self
    }

    /// Start without an entry file (PROP-2026-0002 R22): `dir` is the runtime
    /// root; modules come from `Runtime::load` / `load_as` and must live below it.
    pub fn root(mut self, dir: &str) -> Self {
        self.root = Some(dir.to_string());
        self
    }

    pub fn policy_file(mut self, path: &str) -> Self {
        self.policy = PolicySource::File(path.to_string());
        self
    }

    pub fn policy(mut self, policy: Policy) -> Self {
        self.policy = PolicySource::Given(Box::new(policy));
        self
    }

    /// Host ceiling: every attempt must be allowed by BOTH the loaded policy
    /// (policy.json, `policy_file` or `policy`) and this ceiling; a deny in
    /// either wins, and limits narrow to the smaller value. Calling it twice
    /// intersects both ceilings.
    pub fn ceiling(mut self, ceiling: Policy) -> Self {
        self.ceiling = Some(match self.ceiling.take() {
            Some(prev) => prev.with_ceiling(ceiling),
            None => ceiling,
        });
        self
    }

    /// Load for `connectors sync`: an MCP connector whose `schema` file is
    /// missing or not yet approved loads without imports instead of failing.
    pub fn connector_discovery(mut self) -> Self {
        self.discovery = true;
        self
    }

    /// Host session caps (idle lease, retention, per-principal count, queues).
    pub fn session_limits(mut self, limits: crate::domain::sessions::SessionLimits) -> Self {
        self.session_limits = limits;
        self
    }

    pub fn build(self) -> RivetResult<Runtime> {
        let bundle = match (self.source, &self.entry, &self.root) {
            (Some(b), _, _) => b,
            (None, Some(entry), _) => DiskSourceLoader.load(entry)?,
            (None, None, Some(root)) => SourceBundle {
                entry: String::new(),
                root: root.clone(),
                files: Vec::new(),
                modules: Vec::new(),
            },
            (None, None, None) => {
                return Err(RivetError::validation(
                    "validation.usage",
                    "no source: use .file(PATH), .source(…) or .root(DIR)",
                ));
            }
        };
        let parser = CapyParser::new()?;
        // Every `import` is resolved (and read, below the root) before compiling.
        let bundle = resolve_imports(&bundle, &parser, &DiskSourceLoader)?;
        let program = Arc::new(compile_program(&bundle, &parser)?);
        let mut policy = match self.policy {
            PolicySource::Given(p) => *p,
            PolicySource::Discover if self.entry.is_none() => Policy::deny_all(&bundle.root),
            PolicySource::Discover => load_policy(
                &PolicyLocator {
                    entry: bundle.entry.clone(),
                    explicit_path: None,
                },
                &DiskPolicyReader,
            )?,
            PolicySource::File(p) => load_policy(
                &PolicyLocator {
                    entry: bundle.entry.clone(),
                    explicit_path: Some(p),
                },
                &DiskPolicyReader,
            )?,
        };
        // `Policy::from_json(bytes)` has no file: anchor it at the bundle root.
        fn anchor(p: &mut Policy, root: &str) {
            if p.base_dir.is_empty() {
                p.base_dir = root.to_string();
            }
            if let Some(c) = p.ceiling.as_mut() {
                anchor(c, root);
            }
        }
        if let Some(ceiling) = self.ceiling {
            policy = policy.with_ceiling(ceiling);
        }
        anchor(&mut policy, &bundle.root);
        Runtime::assemble(bundle, program, policy, self.discovery, self.session_limits)
    }
}

/// The Cargo features compiled into this build, in report order
/// (`rivet.capabilities.build_features`, PROP-2026-0002 R11).
pub fn build_features() -> Vec<&'static str> {
    let mut on = Vec::new();
    if cfg!(feature = "serve") {
        on.push("serve");
    }
    if cfg!(feature = "grpc") {
        on.push("grpc");
    }
    if cfg!(feature = "quic") {
        on.push("quic");
    }
    if cfg!(feature = "oauth") {
        on.push("oauth");
    }
    if cfg!(feature = "cli") {
        on.push("cli");
    }
    on
}

/// Parse policy JSON with the same strict schema as policy.json (library hosts).
/// Alias of [`Policy::from_json`] with an explicit base directory.
pub fn policy_from_json(bytes: &[u8], base_dir: &str) -> RivetResult<Policy> {
    parse_policy(bytes, "<memory>", base_dir)
}

/// Library constructors (PROP Increment 16): the same strict schema v1 as a
/// discovered policy.json.
impl Policy {
    /// Load and validate a policy file; relative targets resolve against the
    /// file's own directory and its sha256 becomes the policy hash.
    pub fn from_file(path: &str) -> RivetResult<Policy> {
        load_policy(
            &PolicyLocator {
                entry: String::new(),
                explicit_path: Some(path.to_string()),
            },
            &DiskPolicyReader,
        )
    }

    /// Validate in-memory policy JSON; relative targets resolve against the
    /// bundle root of the runtime it is given to.
    pub fn from_json(bytes: &[u8]) -> RivetResult<Policy> {
        parse_policy(bytes, "<memory>", "")
    }
}

struct Inner {
    /// Bundle root: every source, effect path and module lives below it.
    root: String,
    broker: Arc<PolicyBroker>,
    trace: Arc<MemoryTraceStore>,
    concurrency: Arc<Semaphore>,
    counter: AtomicU64,
    /// Live sessions (polling, WebSocket refs, `rivet.sessions.*`, library).
    sessions: Arc<SessionHost>,
    /// Cancellation signals for running top-level requests.
    requests: Arc<crate::infra::request_control::RunningRequests>,
    /// Bridge hop state of requests that arrived over MCP, keyed by trace ID.
    bridges: Mutex<HashMap<String, BridgeHops>>,
    /// `connectors sync`: MCP connectors may lack a reviewed snapshot.
    discovery: bool,
    /// The current catalog and its adapters (PROP-2026-0002 R22): replaced
    /// whole by a module load; a request holds the `Arc` it started with.
    snapshot: RwLock<Arc<Snapshot>>,
    /// Loads are serialized; requests never wait for them.
    load_lock: Mutex<()>,
}

/// Everything derived from one compiled catalog: the program, its registry,
/// interpreter and connector/auth adapters. Immutable once published.
///
/// ```text
///  Runtime ──RwLock<Arc<Snapshot>>──▶ Snapshot v1 ◀── requests started before the load
///            (swap on load)       └─▶ Snapshot v2 ◀── requests started after it
/// ```
struct Snapshot {
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<ProgramRegistry>,
    driver: Arc<Interpreter>,
    /// MCP client connectors (reviewed snapshots + sessions).
    mcp: Arc<McpPeer>,
    /// The traced evaluator adapters see (for use cases run by the host).
    evaluator: Arc<dyn PolicyEvaluator>,
    /// Policed files (connector snapshot writes by `connectors sync`).
    files: Arc<dyn FileAccess>,
    /// OAuth transactions, account state and the credential store (`rivet.auth.*`).
    oauth: Arc<OAuthAdapter>,
    /// The authorized credential provider (auth.acquire_credential) for MCP connectors.
    credentials: Arc<dyn CredentialProvider>,
}

/// CredentialProvider seen by transport adapters: every lease runs the
/// `auth.acquire_credential` use case (origin binding, allow_auth use,
/// allow_credentials) before the OAuth adapter touches a store or endpoint.
struct AuthorizedCredentials {
    raw: Arc<OAuthAdapter>,
}

#[async_trait]
impl CredentialProvider for AuthorizedCredentials {
    fn profile(&self, name: &str) -> Option<OAuthProfile> {
        CredentialProvider::profile(self.raw.as_ref(), name)
    }

    async fn acquire(
        &self,
        input: CredentialInput,
        evaluator: &dyn PolicyEvaluator,
    ) -> RivetResult<CredentialLease> {
        acquire_credential(input, evaluator, self.raw.as_ref()).await
    }

    fn invalidate(&self, lease: &CredentialLease) {
        self.raw.invalidate(lease);
    }
}

/// A loaded, compiled bundle ready to serve requests from every surface.
#[derive(Clone)]
pub struct Runtime {
    inner: Arc<Inner>,
}

/// FileAccess seen by the interpreter: every call runs the `files.apply_file_operation`
/// use case (authorization per intent) before the confined adapter touches disk.
struct PolicedFiles {
    evaluator: Arc<dyn PolicyEvaluator>,
    raw: ConfinedFiles,
}

/// Static evaluator for `io --check-policy` / `--check-files`: the same
/// decision function over the effective policy, without the decision log or trace.
struct StaticEvaluator {
    policy: Policy,
    root: String,
}

impl PolicyEvaluator for StaticEvaluator {
    fn evaluate(&self, intent: &EffectIntent) -> Permit {
        authorize_effect(intent, &self.policy, &self.root)
    }

    fn policy(&self) -> &Policy {
        &self.policy
    }
}

type SiteIndex = HashMap<(String, u32, Capability, AccessVerb), Vec<(String, String)>>;

tokio::task_local! {
    /// Per-request restrictions in force for the running request (G31): each
    /// entry only narrows; a nested restriction is appended, never substituted.
    static REQUEST_RESTRICTION: Arc<Vec<Arc<Policy>>>;
}

/// The evaluator every adapter sees: the broker's decision plus one trace event
/// per attempt, attributed to request + manifest effect_id through the
/// interpreter's effect scope.
struct TracedEvaluator {
    /// Bundle root: selector resolution of request restrictions.
    root: String,
    broker: Arc<PolicyBroker>,
    trace: Arc<MemoryTraceStore>,
    index: SiteIndex,
    attempts: Mutex<HashMap<(String, String), u32>>,
}

/// Drop query strings, fragments and URL userinfo from traced targets.
fn redact_target(t: &str) -> String {
    let t = t.split(['?', '#']).next().unwrap_or(t);
    match t.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            let host = authority.rsplit('@').next().unwrap_or(authority);
            format!("{scheme}://{host}{path}")
        }
        None => t.to_string(),
    }
}

impl PolicyEvaluator for TracedEvaluator {
    fn evaluate(&self, intent: &EffectIntent) -> Permit {
        let mut permit = self.broker.evaluate(intent);
        // Host ceiling ∩ policy.json (the broker) ∩ every request restriction.
        if permit.decision == Decision::Allowed
            && let Ok(stack) = REQUEST_RESTRICTION.try_with(Arc::clone)
            && let Some(narrowed) = stack.iter().find_map(|r| {
                let p = authorize_effect(intent, r, &self.root);
                (p.decision == Decision::Denied).then_some(p)
            })
        {
            permit = Permit {
                rule: format!("request restriction: {}", narrowed.rule),
                ..narrowed
            };
        }
        if let Some(scope) = current_effect_scope() {
            let target = intent.target.as_str();
            let effect_id = intent.effect_id.clone().or_else(|| {
                let cands = self.index.get(&(
                    scope.operation_id.clone(),
                    scope.line,
                    intent.capability,
                    intent.verb,
                ))?;
                cands
                    .iter()
                    .find(|(_, tpl)| tpl == target)
                    .or(cands.first())
                    .map(|(id, _)| id.clone())
            });
            let node_id = scope.node_id.clone();
            let attempt = {
                let key = (
                    scope.request_id.clone(),
                    effect_id.clone().unwrap_or_default(),
                );
                let mut g = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
                if g.len() > 50_000 {
                    g.clear();
                }
                let n = g.entry(key).or_default();
                *n += 1;
                *n
            };
            self.trace.record(TraceEvent {
                request_id: scope.request_id,
                trace_id: scope.trace_id,
                node_id,
                attempt,
                effect_id,
                operation_id: scope.operation_id,
                phase: "decision".into(),
                capability: intent.capability.as_str().into(),
                access: intent.verb.as_str().into(),
                target: redact_target(target),
                decision: match permit.decision {
                    Decision::Allowed => "allowed".into(),
                    Decision::Denied => "denied".into(),
                },
                policy_hash: self.broker.policy().sha256.clone().unwrap_or_default(),
                source: intent.span.clone(),
                outcome: serde_json::json!({"rule": permit.rule}),
            });
        }
        permit
    }

    fn policy(&self) -> &Policy {
        self.broker.policy()
    }
}

#[async_trait]
impl FileAccess for PolicedFiles {
    async fn apply(&self, op: FileOperation) -> RivetResult<Value> {
        // The interpreter runs every file effect inside its effect scope, so
        // the broker intent (trace + permission error) names the operation and
        // the statement span (B2).
        let scope = current_effect_scope().unwrap_or_default();
        apply_file_operation(
            FileRequest {
                op,
                operation_id: scope.operation_id,
                span: scope.span,
                effect_id: None,
            },
            self.evaluator.as_ref(),
            &self.raw,
        )
        .await
    }
}

/// Register the UDP and QUIC adapters (WS-D). Each adapter runs its feature
/// use case through an injected closure so every step is authorized first.
fn register_transports(interp: &mut Interpreter, root: &str) {
    interp.register_adapter(
        "udp",
        Arc::new(UdpAdapter::new(Arc::new(|plan, ev, drv| {
            Box::pin(async move { exchange_datagrams(plan, ev.as_ref(), drv.as_ref()).await })
        }))),
    );
    // Without the `quic` feature no adapter is registered: bundles using
    // `quic` were already refused at load (unsupported.feature).
    #[cfg(feature = "quic")]
    {
        let tls_files: Arc<dyn FileAccess> = Arc::new(ConfinedFiles::new(root));
        interp.register_adapter(
            "quic",
            Arc::new(QuicAdapter::new(
                Arc::new(|plan, ev, drv| {
                    Box::pin(async move { exchange_quic(plan, ev.as_ref(), drv.as_ref()).await })
                }),
                tls_files,
            )),
        );
    }
    #[cfg(not(feature = "quic"))]
    let _ = root;
}

/// The dispatcher handed to the interpreter for nested `(request …)` calls.
/// Nested calls stay on the snapshot their interpreter belongs to, so a
/// request started before a module load finishes on its own catalog.
struct NestedDispatcher {
    runtime: std::sync::Weak<Inner>,
    snapshot: std::sync::Weak<Snapshot>,
}

#[async_trait]
impl Dispatcher for NestedDispatcher {
    async fn dispatch(
        &self,
        request: Request,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<Completion> {
        let inner = self.runtime.upgrade().ok_or_else(|| {
            RivetError::new(
                ErrorKind::Cancelled,
                "cancelled.runtime",
                "runtime shut down",
            )
        })?;
        // Each nested request runs as its own task so poll recursion does not
        // grow the caller's stack with every level (depth 16 overflowed a 2 MiB
        // thread stack). Dropping the parent aborts the child task (cleanup
        // still runs in its drop path), keeping scoped cancellation.
        struct AbortOnDrop(tokio::task::AbortHandle);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let runtime = Runtime { inner };
        let pinned = self.snapshot.upgrade();
        let task =
            tokio::spawn(
                async move { runtime.dispatch_request_pinned(request, sink, pinned).await },
            );
        let _guard = AbortOnDrop(task.abort_handle());
        match task.await {
            Ok(out) => out,
            Err(e) if e.is_panic() => Err(RivetError::internal("a nested request panicked")),
            Err(_) => Err(RivetError::new(
                ErrorKind::Cancelled,
                "cancelled.request",
                "the nested request was cancelled",
            )),
        }
    }
}

impl Snapshot {
    /// Build the adapters of one catalog. `prev` is the snapshot being
    /// replaced by a module load: its OAuth adapter (pending authorizations,
    /// account state) is kept when the auth profiles did not change.
    fn build(
        catalog: &Arc<CatalogSnapshot>,
        broker: &Arc<PolicyBroker>,
        trace: &Arc<MemoryTraceStore>,
        discovery: bool,
        prev: Option<&Snapshot>,
    ) -> RivetResult<Arc<Snapshot>> {
        let program = Arc::clone(&catalog.program);
        let root = catalog.bundle.root.as_str();
        let policy = broker.policy().clone();
        let effects = analyze_program(&program);
        // A bundle that uses an adapter compiled out of this build is refused
        // here, before anything runs (unsupported.feature, R11).
        crate::domain::capabilities::require_build_features(&program, &effects, &build_features())?;
        let mut index: SiteIndex = HashMap::new();
        for op in &effects.operations {
            for s in &op.sites {
                for v in &s.access {
                    index
                        .entry((op.operation_id.clone(), s.statement_line, s.capability, *v))
                        .or_default()
                        .push((s.effect_id.clone(), s.target.template.clone()));
                }
            }
        }
        let evaluator: Arc<dyn PolicyEvaluator> = Arc::new(TracedEvaluator {
            root: root.to_string(),
            broker: Arc::clone(broker),
            trace: Arc::clone(trace),
            index,
            attempts: Mutex::new(HashMap::new()),
        });
        let files: Arc<dyn FileAccess> = Arc::new(PolicedFiles {
            evaluator: Arc::clone(&evaluator),
            raw: ConfinedFiles::new(root),
        });
        // MCP connectors: reviewed snapshots are bootstrap reads checked against
        // policy.json approved.snapshots before anything is served.
        let mcp_catalog = McpPeer::load(&program, root, &policy.approved.snapshots, discovery)?;
        // An internal module's connector imports are callable, never listed.
        let mut import_entries = mcp_catalog.import_entries();
        for e in &mut import_entries {
            let connector = mcp_catalog.import(&e.id).map(|i| i.connector.clone());
            let declared_in =
                connector.and_then(|c| program.connector(&c).map(|d| d.span.file.clone()));
            if let Some(m) = declared_in.and_then(|f| program.module_of(&f))
                && !m.public
            {
                e.private = true;
            }
        }
        let mcp = Arc::new(McpPeer::new(
            mcp_catalog,
            root,
            Arc::clone(&evaluator),
            mcp_http(),
            mcp_spawn(root),
            Arc::clone(&files),
        ));
        let host_evaluator = Arc::clone(&evaluator);
        let host_files = Arc::clone(&files);
        let mut interp = Interpreter::new(Arc::clone(&program), Arc::clone(&files), evaluator);
        // OAuth (WS-F): profiles are validated at load; the adapter reaches token
        // endpoints only through the brokered exchange_http use case.
        let oauth = match prev {
            Some(p) if p.catalog.program.auth_profiles == program.auth_profiles => {
                Arc::clone(&p.oauth)
            }
            _ => Arc::new(OAuthAdapter::load(
                &program,
                crate::orchestrator::transports::exchange_http_fn(),
            )?),
        };
        let credentials: Arc<dyn CredentialProvider> = Arc::new(AuthorizedCredentials {
            raw: Arc::clone(&oauth),
        });
        crate::orchestrator::transports::register(
            &mut interp,
            files,
            root,
            Some(Arc::clone(&credentials)),
        );
        register_transports(&mut interp, root);
        super::file_streams::register(&mut interp, root);
        // gRPC: descriptor sets are bootstrap reads; unknown methods or wrong
        // call modes fail the load before anything dials.
        #[cfg(feature = "grpc")]
        {
            let grpc = Arc::new(GrpcTransport::load(&program, root)?);
            check_grpc_program(&program, grpc.catalog())?;
            let grpc_credentials = Arc::clone(&credentials);
            let invoke: Arc<InvokeFn> = Arc::new(move |plan, policy| {
                let driver = Arc::clone(&grpc);
                let creds = Arc::clone(&grpc_credentials);
                Box::pin(async move {
                    invoke_rpc(plan, policy.as_ref(), driver.as_ref(), Some(creds.as_ref())).await
                })
            });
            interp.register_adapter("grpc", Arc::new(GrpcEffects::new(invoke)));
        }
        let registry = Arc::new(
            ProgramRegistry::new(Arc::clone(&program))
                .with_imports(import_entries)
                .with_effects(effects),
        );
        Ok(Arc::new(Snapshot {
            catalog: Arc::clone(catalog),
            registry,
            driver: Arc::new(interp),
            mcp,
            evaluator: host_evaluator,
            files: host_files,
            oauth,
            credentials,
        }))
    }

    /// Connect the interpreter to the runtime: DAGs run the execution.run_dag
    /// use case and nested requests dispatch on this same snapshot.
    fn wire(self: &Arc<Snapshot>, inner: &Arc<Inner>) {
        self.driver.set_dag_executor(Arc::new(DagUseCase));
        self.driver.set_dispatcher(Arc::new(NestedDispatcher {
            runtime: Arc::downgrade(inner),
            snapshot: Arc::downgrade(self),
        }));
    }
}

/// `registry.load_module`'s CatalogStore: compiles under the loader's policy
/// and swaps the runtime's snapshot (loads are serialized by the caller).
pub(super) struct RuntimeCatalog<'a> {
    rt: &'a Runtime,
}

impl crate::domain::ports::CatalogStore for RuntimeCatalog<'_> {
    fn current(&self) -> Arc<CatalogSnapshot> {
        self.rt.catalog()
    }

    fn compile(&self, bundle: &SourceBundle) -> RivetResult<CatalogSnapshot> {
        let parser = CapyParser::new()?;
        let resolved = resolve_imports(bundle, &parser, &DiskSourceLoader)?;
        let program = compile_program(&resolved, &parser)?;
        Ok(CatalogSnapshot::new(resolved, Arc::new(program)))
    }

    fn publish(&self, next: CatalogSnapshot) -> RivetResult<()> {
        let inner = &self.rt.inner;
        let current = self.rt.snap();
        let next = Snapshot::build(
            &Arc::new(next),
            &inner.broker,
            &inner.trace,
            inner.discovery,
            Some(&current),
        )?;
        next.wire(inner);
        let mut slot = inner.snapshot.write().unwrap_or_else(|e| e.into_inner());
        *slot = next;
        Ok(())
    }
}

/// The registry the long-lived session host sees: always the current snapshot's.
struct CurrentRegistry {
    runtime: std::sync::Weak<Inner>,
}

impl CurrentRegistry {
    fn current(&self) -> Option<Arc<ProgramRegistry>> {
        let inner = self.runtime.upgrade()?;
        let g = inner.snapshot.read().unwrap_or_else(|e| e.into_inner());
        Some(Arc::clone(&g.registry))
    }
}

impl Registry for CurrentRegistry {
    fn describe(&self, query: &CatalogQuery) -> RivetResult<Catalog> {
        match self.current() {
            Some(r) => r.describe(query),
            None => Ok(Catalog::default()),
        }
    }

    fn program(&self) -> Arc<CompiledProgram> {
        self.current()
            .map(|r| r.program())
            .unwrap_or_else(|| Arc::new(CompiledProgram::default()))
    }

    fn effect_sites(&self) -> Arc<crate::domain::io_manifest::EffectCatalog> {
        self.current().map(|r| r.effect_sites()).unwrap_or_default()
    }
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
    }

    fn assemble(
        bundle: SourceBundle,
        program: Arc<CompiledProgram>,
        policy: Policy,
        discovery: bool,
        session_limits: crate::domain::sessions::SessionLimits,
    ) -> RivetResult<Runtime> {
        let decide: Arc<crate::infra::policy_broker::DecideFn> =
            Arc::new(|i: &EffectIntent, p: &Policy, root: &str| -> Permit {
                authorize_effect(i, p, root)
            });
        let broker = Arc::new(PolicyBroker::new(policy.clone(), &bundle.root, decide));
        let trace = Arc::new(MemoryTraceStore::default());
        let root = bundle.root.clone();
        let catalog = Arc::new(CatalogSnapshot::new(bundle, program));
        let snapshot = Snapshot::build(&catalog, &broker, &trace, discovery, None)?;
        let catalog_version = catalog.version.clone();
        let inner = Arc::new_cyclic(|weak: &std::sync::Weak<Inner>| {
            let w = weak.clone();
            let upgrade: Arc<super::setup_library::UpgradeFn> =
                Arc::new(move || w.upgrade().map(|inner| Runtime { inner }));
            let registry: Arc<dyn Registry> = Arc::new(CurrentRegistry {
                runtime: weak.clone(),
            });
            let sessions = Arc::new(
                super::setup_library::session_host(
                    upgrade,
                    registry,
                    catalog_version,
                    session_limits,
                )
                .with_budget(crate::infra::session_driver::BufferBudget::new(
                    policy.limits.max_buffered_bytes,
                )),
            );
            Inner {
                root,
                broker,
                trace,
                concurrency: Arc::new(Semaphore::new(
                    policy.limits.max_concurrent_requests as usize,
                )),
                counter: AtomicU64::new(0),
                sessions,
                requests: Arc::new(crate::infra::request_control::RunningRequests::default()),
                bridges: Mutex::new(HashMap::new()),
                discovery,
                snapshot: RwLock::new(Arc::clone(&snapshot)),
                load_lock: Mutex::new(()),
            }
        });
        snapshot.wire(&inner);
        Ok(Runtime { inner })
    }

    /// Serialize module loads and hand `f` the runtime's CatalogStore.
    pub(super) fn with_catalog_store<T>(
        &self,
        f: impl FnOnce(&RuntimeCatalog<'_>) -> RivetResult<T>,
    ) -> RivetResult<T> {
        let _guard = self
            .inner
            .load_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        f(&RuntimeCatalog { rt: self })
    }

    /// The catalog snapshot new requests use now.
    fn snap(&self) -> Arc<Snapshot> {
        let g = self
            .inner
            .snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner());
        Arc::clone(&g)
    }

    /// The current immutable catalog (bundle files, modules and program).
    pub fn catalog(&self) -> Arc<CatalogSnapshot> {
        Arc::clone(&self.snap().catalog)
    }

    pub fn program(&self) -> Arc<CompiledProgram> {
        Arc::clone(&self.snap().catalog.program)
    }

    /// The current bundle: the entry file (if any) and every resolved module.
    pub fn bundle(&self) -> SourceBundle {
        self.snap().catalog.bundle.clone()
    }

    pub fn policy(&self) -> &Policy {
        self.inner.broker.policy()
    }

    pub fn decisions(&self) -> Vec<Permit> {
        self.inner.broker.decisions()
    }

    fn next_ids(&self) -> (String, String) {
        let n = self.inner.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0) as u64;
        let tag = (nanos ^ (n.wrapping_mul(0x9E37_79B9_7F4A_7C15))) & 0xffff_ffff;
        (
            format!("req_{n:02}{tag:08x}"),
            format!("tr_{n:02}{tag:08x}"),
        )
    }

    /// Build a top-level request for an external surface.
    pub fn new_request(&self, operation_id: &str, params: Value, principal: Principal) -> Request {
        let (request_id, trace_id) = self.next_ids();
        Request {
            request_id,
            trace_id,
            operation_id: operation_id.to_string(),
            params,
            principal,
            parent_request_id: None,
            depth: 0,
            deadline_ms: DEFAULT_DEADLINE_MS,
            include_private: false,
            parent_span_id: None,
            cancel: crate::domain::cancel::CancelToken::new(),
            restrict: None,
        }
    }

    /// A top-level request continuing the caller's W3C trace: a valid
    /// `traceparent` trace-id becomes the Rivet trace id and its parent-id the
    /// request's parent span.
    pub fn new_request_traced(
        &self,
        operation_id: &str,
        params: Value,
        principal: Principal,
        trace: Option<&TraceContext>,
    ) -> Request {
        let mut req = self.new_request(operation_id, params, principal);
        if let Some(t) = trace {
            req.trace_id = t.trace_id.clone();
            req.parent_span_id = Some(t.parent_id.clone());
        }
        req
    }

    /// A request a built-in starts on behalf of `parent` (generic dispatch):
    /// new request id, same trace id, parent request and span set, cancelled
    /// with its parent.
    pub fn follow_request(&self, parent: &Request, operation_id: &str, params: Value) -> Request {
        let mut req = self.new_request(operation_id, params, parent.principal.clone());
        req.trace_id = parent.trace_id.clone();
        req.parent_request_id = Some(parent.request_id.clone());
        req.parent_span_id = Some(span_id(&parent.request_id));
        req.deadline_ms = parent.deadline_ms;
        req.cancel = parent.cancel.child();
        req
    }

    /// Host shutdown (serve SIGINT/SIGTERM): cancel every running top-level
    /// request and live session — each unwinds and closes its resources within
    /// the cleanup grace — and wait up to `drain` for them to finish.
    pub async fn shutdown(&self, drain: std::time::Duration) {
        let until = std::time::Instant::now() + drain;
        self.inner.requests.cancel_all();
        let _ = tokio::time::timeout(drain, self.inner.sessions.cancel_all()).await;
        while self.inner.requests.running_count() > 0 && std::time::Instant::now() < until {
            self.inner.requests.cancel_all();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// `request(ID, PARAMS, on_data)` — the one entry point every surface uses.
    pub async fn request(
        &self,
        operation_id: &str,
        params: Value,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<Completion> {
        let req = self.new_request(operation_id, params, Principal::local());
        self.dispatch_request(req, sink).await
    }

    /// `request` with a per-request restriction (G31, library request option):
    /// `restrict` is `{grants:[…]}` in the policy.json grant format and is
    /// intersected with the effective policy for this request only.
    pub async fn request_restricted(
        &self,
        operation_id: &str,
        params: Value,
        restrict: Value,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<Completion> {
        let mut req = self.new_request(operation_id, params, Principal::local());
        req.restrict = Some(restrict);
        self.dispatch_request(req, sink).await
    }

    /// Validate a caller's `restrict: {grants:[…]}` into a restriction policy:
    /// same grant schema and selector resolution as policy.json (relative to the
    /// policy file's directory), the loaded network rules kept. Unknown keys or
    /// malformed grants are policy.invalid naming `/restrict/…`.
    fn parse_restriction(&self, r: &Value) -> RivetResult<Policy> {
        let j = r.to_json();
        let obj = j.as_object().ok_or_else(|| {
            RivetError::validation(
                "policy.invalid",
                "restrict must be an object {\"grants\": [...]}",
            )
            .with_details(Value::object([("pointer", Value::text("/restrict"))]))
        })?;
        if let Some(k) = obj.keys().find(|k| k.as_str() != "grants") {
            return Err(RivetError::validation(
                "policy.invalid",
                format!("restrict accepts only `grants` (got `{k}`); it can narrow, never grant"),
            )
            .with_details(Value::object([(
                "pointer",
                Value::text(format!("/restrict/{k}")),
            )])));
        }
        let doc = serde_json::json!({
            "version": 1,
            "grants": obj.get("grants").cloned().unwrap_or_else(|| serde_json::json!([])),
        });
        let base = self.policy().base_dir.clone();
        let mut p =
            parse_policy(doc.to_string().as_bytes(), "restrict", &base).map_err(|mut e| {
                if let Some(ptr) = e.details.get("pointer").and_then(Value::as_str) {
                    let ptr = format!("/restrict{ptr}");
                    e.details.set("pointer", Value::text(ptr));
                }
                e
            })?;
        p.network = self.policy().network.clone();
        Ok(p)
    }

    /// Dispatch a fully formed request (surfaces set principal, deadline, IDs).
    /// A request `restrict` is pushed onto the task's restriction stack for the
    /// whole request (nested calls copy it); every broker decision must then pass
    /// the effective policy AND every stacked restriction (narrow-only).
    pub async fn dispatch_request(
        &self,
        req: Request,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<Completion> {
        self.dispatch_request_pinned(req, sink, None).await
    }

    /// `dispatch_request` on a given catalog snapshot (nested calls stay on
    /// their parent's snapshot); `None` takes the current one.
    async fn dispatch_request_pinned(
        &self,
        req: Request,
        sink: Option<Arc<dyn DataSink>>,
        pinned: Option<Arc<Snapshot>>,
    ) -> RivetResult<Completion> {
        let snap = pinned.unwrap_or_else(|| self.snap());
        // A top-level request's error always names that request, even when it
        // was raised by a nested call (DAG node, `request`, generic dispatch).
        let top = (req.depth == 0).then(|| (req.request_id.clone(), req.trace_id.clone()));
        self.dispatch_scoped(req, sink, snap)
            .await
            .map_err(|mut e| {
                if let Some((r, t)) = top {
                    e.request_id = Some(r);
                    e.trace_id = Some(t);
                }
                e
            })
    }

    async fn dispatch_scoped(
        &self,
        req: Request,
        sink: Option<Arc<dyn DataSink>>,
        snap: Arc<Snapshot>,
    ) -> RivetResult<Completion> {
        let Some(r) = &req.restrict else {
            return self.dispatch_unrestricted(req, sink, snap).await;
        };
        let parsed = self.parse_restriction(r).map_err(|mut e| {
            e.request_id = Some(req.request_id.clone());
            e.trace_id = Some(req.trace_id.clone());
            e.operation_id = Some(req.operation_id.clone());
            e
        })?;
        let mut stack: Vec<Arc<Policy>> = REQUEST_RESTRICTION
            .try_with(|s| s.as_ref().clone())
            .unwrap_or_default();
        stack.push(Arc::new(parsed));
        REQUEST_RESTRICTION
            .scope(Arc::new(stack), self.dispatch_unrestricted(req, sink, snap))
            .await
    }

    async fn dispatch_unrestricted(
        &self,
        req: Request,
        sink: Option<Arc<dyn DataSink>>,
        snap: Arc<Snapshot>,
    ) -> RivetResult<Completion> {
        let limits = self.policy().limits.clone();
        if req.depth == 0 {
            // One principal model on every surface (serve.principals); the CLI and
            // library principal `local` is always allowed.
            require_operation(&OperationAccess {
                principal: req.principal.clone(),
                operation_id: req.operation_id.clone(),
                serve: self.policy().serve.clone(),
            })
            .map_err(|mut e| {
                e.request_id = Some(req.request_id.clone());
                e.trace_id = Some(req.trace_id.clone());
                e
            })?;
        }
        if req.operation_id.starts_with("rivet.") {
            return super::builtins::dispatch_builtin(self, req, sink).await;
        }
        let imported = snap.catalog.program.operation(&req.operation_id).is_none()
            && snap.mcp.catalog().owns(&req.operation_id);
        // Host-wide budget shared by top-level requests, nested `(request …)`
        // calls and DAG/map nodes (PROP Increment 7): every in-flight request
        // holds one permit, so the 65th concurrent call fails with limit.concurrency.
        let _permit = Arc::clone(&self.inner.concurrency)
            .try_acquire_owned()
            .map_err(|_| {
                let mut e = RivetError::new(
                    ErrorKind::Limit,
                    "limit.concurrency",
                    format!(
                        "limits.max_concurrent_requests ({}) reached",
                        limits.max_concurrent_requests
                    ),
                );
                e.operation_id = Some(req.operation_id.clone());
                e
            })?;
        if imported {
            return self.dispatch_mcp(req, &snap).await;
        }
        if req.depth > 0 {
            return request_operation(
                req,
                snap.registry.as_ref(),
                snap.driver.as_ref(),
                &limits,
                sink,
            )
            .await;
        }
        // Top-level requests can be cancelled by ID: the signal fires the
        // request's structured token, the interpreter unwinds and closes every
        // handle within the cleanup grace and returns one terminal `cancelled`.
        // Only if the run is still going after the grace is it dropped here.
        let request_id = req.request_id.clone();
        let trace_id = req.trace_id.clone();
        let token = req.cancel.clone();
        self.inner
            .requests
            .start(&request_id, &req.principal.name, token.clone());
        let run = request_operation(
            req,
            snap.registry.as_ref(),
            snap.driver.as_ref(),
            &limits,
            sink,
        );
        let last_resort = async {
            token.cancelled().await;
            tokio::time::sleep(
                crate::infra::execution_driver::CLEANUP_GRACE
                    + 2 * crate::infra::execution_driver::DEADLINE_BACKSTOP,
            )
            .await;
        };
        let out = tokio::select! {
            r = run => r,
            _ = last_resort => {
                let mut e = RivetError::new(ErrorKind::Cancelled, "cancelled.request", "the request was cancelled by its caller");
                e.request_id = Some(request_id.clone());
                e.trace_id = Some(trace_id);
                e.effects = crate::domain::errors::EffectsStatus::Unknown;
                Err(e)
            }
        };
        let state = match &out {
            Ok(_) => "succeeded",
            Err(e) if e.kind == ErrorKind::Cancelled => "cancelled",
            Err(_) => "failed",
        };
        self.inner.requests.finish(&request_id, state);
        out
    }

    /// Cancel one of the caller's own running top-level requests (idempotent).
    pub fn cancel(
        &self,
        request_id: &str,
        principal: Principal,
    ) -> RivetResult<crate::domain::sessions::CancelReceipt> {
        crate::features::execution::cancel_request::cancel_request(
            &crate::domain::ports::CancelRequest {
                request_id: request_id.to_string(),
                principal,
            },
            self.inner.requests.as_ref(),
        )
    }

    /// Identity of this host in MCP bridge chains.
    fn bridge_identity(&self) -> String {
        let hash = self.snap().catalog.version.clone();
        let h = hash.trim_start_matches("sha256:");
        format!("rivet:{}", &h[..h.len().min(16)])
    }

    fn mcp_context(&self, operation_id: &str, req: Option<&Request>) -> McpContext {
        McpContext {
            operation_id: operation_id.to_string(),
            request_id: req.map(|r| r.request_id.clone()).unwrap_or_default(),
            deadline_ms: req.map(|r| r.deadline_ms).unwrap_or(DEFAULT_DEADLINE_MS),
            bridge: req
                .and_then(|r| {
                    self.inner
                        .bridges
                        .lock()
                        .ok()
                        .and_then(|m| m.get(&r.trace_id).cloned())
                })
                .unwrap_or_default(),
            identity: self.bridge_identity(),
            span: None,
            principal: req.map(|r| r.principal.clone()),
            bearer: None,
        }
    }

    /// An imported connector operation (`crm.tools.search`): the
    /// `connectors.invoke_mcp` use case under the request deadline.
    async fn dispatch_mcp(&self, req: Request, snap: &Snapshot) -> RivetResult<Completion> {
        let id = req.operation_id.clone();
        let tag = |mut e: RivetError| {
            e.operation_id.get_or_insert_with(|| id.clone());
            e.request_id.get_or_insert_with(|| req.request_id.clone());
            e.trace_id.get_or_insert_with(|| req.trace_id.clone());
            e
        };
        let catalog = snap.mcp.catalog();
        let (connector, method) = id.split_once('.').unwrap_or((id.as_str(), ""));
        let kind = catalog.import(&id).map(|i| i.kind);
        let input = McpRequest {
            connector: connector.to_string(),
            method: method.to_string(),
            params: req.params.clone(),
            schema_hash: catalog
                .connector(connector)
                .and_then(|c| c.schema_hash.clone())
                .unwrap_or_default(),
            context: self.mcp_context(&id, Some(&req)),
        };
        let run = invoke_mcp(
            input,
            snap.evaluator.as_ref(),
            snap.mcp.as_ref(),
            Some(snap.credentials.as_ref()),
        );
        let result = match tokio::time::timeout(
            std::time::Duration::from_millis(req.deadline_ms.max(1)),
            run,
        )
        .await
        {
            Ok(r) => r.map_err(tag)?,
            Err(_) => {
                return Err(tag(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.mcp",
                    format!(
                        "`{id}` exceeded its {} ms deadline (the MCP call was cancelled)",
                        req.deadline_ms
                    ),
                )
                .with_effects(crate::domain::EffectsStatus::Unknown)));
            }
        };
        let kind = kind.unwrap_or(McpImportKind::Tool);
        Ok(Completion {
            request_id: req.request_id,
            trace_id: req.trace_id,
            operation: id,
            result: result.to_value(kind),
            data_count: 0,
            effects: if kind == McpImportKind::Tool {
                crate::domain::EffectsStatus::Unknown
            } else {
                crate::domain::EffectsStatus::None
            },
        })
    }

    /// A request that arrived over MCP with bridge `_meta` (hop count and
    /// identity chain); nested connector calls continue that chain.
    pub async fn request_bridged(
        &self,
        principal: Principal,
        operation_id: &str,
        params: Value,
        bridge: BridgeHops,
        trace: Option<&TraceContext>,
        restrict: Option<Value>,
    ) -> RivetResult<Completion> {
        let mut req = self.new_request_traced(operation_id, params, principal, trace);
        req.restrict = restrict;
        let trace = req.trace_id.clone();
        let tracked = bridge != BridgeHops::default();
        if tracked && let Ok(mut m) = self.inner.bridges.lock() {
            m.insert(trace.clone(), bridge);
        }
        let out = self.dispatch_request(req, None).await;
        if tracked && let Ok(mut m) = self.inner.bridges.lock() {
            m.remove(&trace);
        }
        out
    }

    /// `rivet connectors sync NAME --output PATH`: authorized discovery
    /// (allow_mcp NAME/discover + transport grants), then an exclusive create of
    /// the candidate snapshot (allow_write). Never overwrites an existing file,
    /// never changes this runtime's catalog. `output` is bundle-root relative.
    pub async fn sync_connector(&self, name: &str, output: &str) -> RivetResult<ConnectorSync> {
        // G21: the snapshot is created exclusively, so an existing --output is refused
        // BEFORE any discovery traffic reaches the server (the exclusive create below
        // still guards the race where the path appears meanwhile).
        let probe = FileOperation::new(crate::domain::files::FileVerb::Stat, output);
        match ConfinedFiles::new(&self.bundle().root).apply(probe).await {
            Ok(_) => {
                return Err(RivetError::new(
                    ErrorKind::Conflict,
                    "conflict.already_exists",
                    format!("{output} already exists; connectors sync never overwrites a snapshot"),
                )
                .with_details(Value::object([("path", Value::text(output))])));
            }
            Err(e) if e.code.starts_with("not_found") => {}
            Err(e) => return Err(e),
        }
        let input = McpRequest {
            connector: name.to_string(),
            method: "discover".into(),
            params: Value::Null,
            schema_hash: String::new(),
            context: self.mcp_context("connectors.sync", None),
        };
        let snap = self.snap();
        let found = invoke_mcp(
            input,
            snap.evaluator.as_ref(),
            snap.mcp.as_ref(),
            Some(snap.credentials.as_ref()),
        )
        .await?;
        let snapshot_json = found
            .structured_content
            .map(|v| v.to_json())
            .unwrap_or_default();
        let snapshot = McpSnapshot::parse(snapshot_json.to_string().as_bytes())?;
        let bytes = snapshot.serialize();
        let sha256 = snapshot_hash(&bytes);
        let mut op = FileOperation::new(crate::domain::files::FileVerb::Create, output);
        op.codec = Some(crate::domain::files::Codec::Bytes);
        op.content = Some(Value::Bytes(bytes));
        op.overwrite = false;
        snap.files.apply(op).await?;
        Ok(ConnectorSync {
            connector: name.to_string(),
            path: output.to_string(),
            sha256,
            protocol_version: snapshot.protocol_version.clone(),
            tools: snapshot.tools.iter().map(|t| t.name.clone()).collect(),
            resources: snapshot.resources.iter().map(|r| r.uri.clone()).collect(),
            prompts: snapshot.prompts.iter().map(|p| p.name.clone()).collect(),
        })
    }

    /// Imported MCP operation IDs of this bundle (`crm.tools.search`, …).
    pub fn connector_imports(&self) -> Vec<String> {
        self.snap()
            .mcp
            .catalog()
            .imports
            .iter()
            .map(|i| i.id.clone())
            .collect()
    }

    pub fn list(&self) -> RivetResult<Catalog> {
        describe_operations(&CatalogQuery::default(), self.snap().registry.as_ref())
    }

    pub fn describe(&self, ids: &[String]) -> RivetResult<Catalog> {
        describe_operations(
            &CatalogQuery {
                ids: ids.to_vec(),
                include_private: false,
            },
            self.snap().registry.as_ref(),
        )
    }

    pub fn outputs(&self, id: Option<&str>, all: bool) -> RivetResult<Vec<OutputReport>> {
        inspect_outputs(
            &OutputQuery {
                id: id.map(str::to_string),
                all,
            },
            self.snap().registry.as_ref(),
        )
    }

    pub fn registry(&self) -> Arc<dyn Registry> {
        self.snap().registry.clone()
    }

    /// Catalog version pinned by sessions (`sha256:` of the bundle sources).
    pub fn catalog_version(&self) -> String {
        self.snap().catalog.version.clone()
    }

    /// The OAuth session driver behind `rivet.auth.*`.
    pub fn oauth(&self) -> Arc<dyn OAuthSessionDriver> {
        self.snap().oauth.clone()
    }

    /// The broker evaluator (traced) used by adapters and built-ins.
    pub fn evaluator(&self) -> Arc<dyn PolicyEvaluator> {
        Arc::clone(&self.snap().evaluator)
    }

    /// The live-session driver shared by polling, WebSocket, MCP and the library.
    pub fn sessions(&self) -> Arc<dyn SessionDriver> {
        self.inner.sessions.clone()
    }

    /// `request(ID, PARAMS)` as an authenticated principal (serve surfaces).
    pub async fn request_as(
        &self,
        principal: Principal,
        operation_id: &str,
        params: Value,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<Completion> {
        let req = self.new_request(operation_id, params, principal);
        self.dispatch_request(req, sink).await
    }

    /// Run one session request: attach its live input feed (for `receives`
    /// operations, iterated as `incoming`), then use the shared dispatcher.
    pub async fn dispatch_session(
        &self,
        req: Request,
        sink: Arc<dyn DataSink>,
        input: tokio::sync::mpsc::Receiver<Value>,
    ) -> RivetResult<Completion> {
        let request_id = req.request_id.clone();
        let snap = self.snap();
        snap.driver.attach_input(&request_id, input);
        let out = self
            .dispatch_request_pinned(req, Some(sink), Some(Arc::clone(&snap)))
            .await;
        snap.driver.detach_input(&request_id);
        out
    }

    fn static_evaluator(&self) -> StaticEvaluator {
        StaticEvaluator {
            policy: self.policy().clone(),
            root: self.inner.root.clone(),
        }
    }

    /// The I/O manifest (`rivet io`, `rivet.io`, `rt.io`): synchronous, reads only
    /// the compiled catalog and loaded policy; `check_files` probes needed paths
    /// through the broker, `trace_request_id` joins this host's trace store.
    pub fn io(&self, query: &IoQuery) -> RivetResult<IoReport> {
        let evaluator = self.static_evaluator();
        let probe = ConfinedFiles::new(&self.inner.root);
        let snap = self.snap();
        inspect_effects(
            query,
            &AuditPorts {
                registry: snap.registry.as_ref(),
                policy: &evaluator,
                trace: Some(self.inner.trace.as_ref()),
                probe: Some(&probe),
            },
        )
    }

    /// The static call graph of one operation (`rivet graph ID [--json]`).
    pub fn graph(
        &self,
        query: &crate::domain::call_graph::GraphQuery,
    ) -> RivetResult<crate::domain::call_graph::CallGraph> {
        crate::features::audit::build_graph::build_graph(query, self.snap().registry.as_ref())
    }

    /// Least-privilege draft for `ids` (empty = every public operation). Never writes.
    pub fn generate_policy(&self, ids: &[&str]) -> RivetResult<PolicyDraft> {
        let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        self.generate_policy_draft(&ids, false, None)
    }

    /// `rivet policy generate [ID ...|--all] [--output PATH]`.
    pub fn generate_policy_draft(
        &self,
        ids: &[String],
        all: bool,
        output: Option<&str>,
    ) -> RivetResult<PolicyDraft> {
        let report = self.io(&IoQuery {
            ids: ids.to_vec(),
            all,
            format: "json".into(),
            ..IoQuery::default()
        })?;
        let rebase = output
            .map(|o| rebase_prefix(o, &self.inner.root))
            .unwrap_or_default();
        generate_policy(
            &PolicyGenerateInput {
                manifest: report.manifest,
                output: output.map(str::to_string),
                rebase,
            },
            &ExclusiveDraftWriter,
        )
    }

    /// `rivet trace show REQ`: this host's recorded broker decisions for a request.
    pub fn trace(&self, request_id: &str) -> RivetResult<TraceResult> {
        read_trace(&TraceQuery::new(request_id), self.inner.trace.as_ref())
    }

    /// `rivet trace export REQ --output PATH` / `rivet.trace.export`: write this host's
    /// (already sanitized) trace of one request as JSON to a NEW file. Trace export is
    /// application I/O (proposal Increment 5): the write goes through the broker as
    /// `allow_write create PATH` and an existing path is `conflict.already_exists`
    /// (never overwritten). An unknown request is `not_found` before anything is written.
    pub async fn export_trace(&self, request_id: &str, path: &str) -> RivetResult<TraceExport> {
        let trace = self.trace(request_id)?;
        let body = serde_json::to_vec_pretty(&trace.to_json())
            .map_err(|e| RivetError::internal(format!("trace export: {e}")))?;
        let bytes = body.len();
        let mut op = FileOperation::new(crate::domain::files::FileVerb::Create, path);
        op.codec = Some(crate::domain::files::Codec::Bytes);
        op.content = Some(Value::Bytes(body));
        op.overwrite = false;
        self.snap().files.apply(op).await?;
        Ok(TraceExport {
            request_id: request_id.to_string(),
            path: path.to_string(),
            events: trace.events.len(),
            bytes,
        })
    }

    /// The raw trace store (for hosts that export or inspect it).
    pub fn trace_store(&self) -> Arc<dyn TraceStore> {
        self.inner.trace.clone()
    }

    /// Record a trace note that a request arrived with deprecated input aliases
    /// (`id`/`params`, PROP-2026-0002 R5); `rivet trace show` lists it as phase
    /// `input`, decision `deprecated`. No effect id, so manifests ignore it.
    pub fn note_deprecated_input(
        &self,
        request_id: &str,
        trace_id: &str,
        operation: &str,
        aliases: &[String],
    ) {
        if aliases.is_empty() {
            return;
        }
        self.inner
            .trace
            .record(crate::domain::io_manifest::TraceEvent {
                request_id: request_id.to_string(),
                trace_id: trace_id.to_string(),
                node_id: None,
                attempt: 0,
                effect_id: None,
                operation_id: operation.to_string(),
                phase: "input".into(),
                capability: "input".into(),
                access: "deprecated".into(),
                target: aliases.join(","),
                decision: "deprecated".into(),
                policy_hash: self.policy().sha256.clone().unwrap_or_default(),
                source: None,
                outcome: serde_json::json!({
                    "deprecated": aliases,
                    "hint": "send `operation` and `data`; `id` and `params` are removed in 0.3.0",
                }),
            });
    }
}

/// Receipt of `rivet trace export`: where the trace went and how much was written.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceExport {
    pub request_id: String,
    pub path: String,
    pub events: usize,
    pub bytes: usize,
}

impl TraceExport {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "request_id": self.request_id,
            "path": self.path,
            "events": self.events,
            "bytes": self.bytes,
        })
    }
}

/// Receipt of `rivet connectors sync`: where the candidate snapshot went and
/// the hash a maintainer must approve in policy.json.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectorSync {
    pub connector: String,
    pub path: String,
    pub sha256: String,
    pub protocol_version: String,
    pub tools: Vec<String>,
    pub resources: Vec<String>,
    pub prompts: Vec<String>,
}

impl ConnectorSync {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "connector": self.connector,
            "path": self.path,
            "sha256": self.sha256,
            "protocolVersion": self.protocol_version,
            "tools": self.tools,
            "resources": self.resources,
            "prompts": self.prompts,
        })
    }
}

/// The brokered HTTP client for MCP Streamable HTTP (transports.exchange_http).
fn mcp_http() -> Arc<McpHttpFn> {
    Arc::new(|x, ev| {
        Box::pin(async move {
            exchange_http(
                x,
                ev.as_ref(),
                &crate::infra::http_adapter::HyperClient,
                &crate::infra::codec::StdCodec,
            )
            .await
        })
    })
}

/// The process path for MCP stdio: confine (allow_exec + sandbox), then a duplex spawn.
fn mcp_spawn(root: &str) -> Arc<McpSpawnFn> {
    let runner = Arc::new(crate::infra::process_adapter::TokioRunner::new(root));
    Arc::new(move |mut plan, ev| {
        let runner = Arc::clone(&runner);
        Box::pin(async move {
            confine_process(&mut plan, ev.as_ref())?;
            runner.spawn_duplex(&plan).await
        })
    })
}

/// Relative path from the draft file's directory back to the bundle root.
fn rebase_prefix(output: &str, root: &str) -> String {
    let abs = |p: &std::path::Path| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    };
    let norm = |p: std::path::PathBuf| {
        let mut out: Vec<String> = Vec::new();
        for c in p.components() {
            match c {
                std::path::Component::ParentDir => {
                    out.pop();
                }
                std::path::Component::Normal(s) => out.push(s.to_string_lossy().to_string()),
                _ => {}
            }
        }
        out
    };
    let out_dir = std::path::Path::new(output)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let a = norm(abs(out_dir));
    let b = norm(abs(std::path::Path::new(root)));
    let common = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".to_string(); a.len() - common];
    parts.extend(b[common..].iter().cloned());
    parts.join("/")
}

/// The interpreter's DagExecutor: runs the `execution.run_dag` use case.
struct DagUseCase;

#[async_trait]
impl crate::domain::dag::DagExecutor for DagUseCase {
    async fn run(
        &self,
        input: crate::domain::dag::DagInput,
        runner: &dyn crate::domain::dag::DagNodeRunner,
    ) -> crate::domain::dag::DagCompletion {
        crate::features::execution::run_dag::run_dag(input, runner).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(text: &str) -> Runtime {
        Runtime::builder()
            .source("app.rivet", text, ".")
            .build()
            .unwrap()
    }

    // vhco:test execution.request_operation -- demo.add returns 5 through the shared dispatcher, defaults apply and output is checked
    #[tokio::test]
    async fn catalog_demo_add() {
        let r = Runtime::builder()
            .file("docs/demos/01-catalog/app.rivet")
            .build()
            .unwrap();
        let c = r
            .request(
                "demo.add",
                Value::object([("a", Value::Int(2)), ("b", Value::Int(3))]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(c.result, Value::Int(5));
        let c = r
            .request("demo.add", Value::object([("a", Value::Int(2))]), None)
            .await
            .unwrap();
        assert_eq!(c.result, Value::Int(2));
        let greet = r
            .request(
                "demo.greet",
                Value::object([("person", Value::text("Ada"))]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(greet.result, Value::text("Hello, Ada!"));
        let e = r
            .request("demo.add", Value::object([("a", Value::text("x"))]), None)
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation.type");
    }

    #[tokio::test]
    async fn output_mismatch_is_output_invalid() {
        let r = rt("operation a.b\n    output integer\n    return \"five\"\nend\n");
        assert_eq!(
            r.request("a.b", Value::Null, None).await.unwrap_err().code,
            "output.invalid"
        );
    }

    #[tokio::test]
    async fn nested_requests_reach_private_helpers_but_surfaces_do_not() {
        let r = rt(
            "operation m.double\n    private true\n    param value integer required\n    output integer\n    return value * 2\nend\n\noperation m.quad\n    param v integer required\n    output integer\n    x = (request \"m.double\" {value: v})\n    return (request \"m.double\" {value: x})\nend\n",
        );
        assert_eq!(
            r.request("m.quad", Value::object([("v", Value::Int(3))]), None)
                .await
                .unwrap()
                .result,
            Value::Int(12)
        );
        assert_eq!(
            r.request("m.double", Value::object([("value", Value::Int(3))]), None)
                .await
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn dag_fail_independent_reports_blocked_descendants() {
        let r = Runtime::builder()
            .file("docs/demos/05-dag/app.rivet")
            .build()
            .unwrap();
        let total = r
            .request(
                "report.total",
                Value::object([("a", Value::Int(2)), ("b", Value::Int(5))]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(total.result, Value::object([("total", Value::Int(14))]));
        let partial = r
            .request("report.partial", Value::Null, None)
            .await
            .unwrap();
        assert_eq!(
            partial.result.get("good").unwrap(),
            &Value::text("succeeded")
        );
        assert_eq!(partial.result.get("bad").unwrap(), &Value::text("failed"));
        assert_eq!(
            partial.result.get("blocked").unwrap(),
            &Value::text("blocked")
        );
    }

    #[tokio::test]
    async fn file_effects_without_policy_are_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let r = Runtime::builder()
            .source("app.rivet", "operation f.w\n    output json\n    file create \"./out/a.json\" json {n: 1}\n    return {ok: true}\nend\n", tmp.path().to_str().unwrap())
            .build()
            .unwrap();
        let e = r.request("f.w", Value::Null, None).await.unwrap_err();
        assert_eq!(e.code, "permission.denied");
        assert!(!tmp.path().join("out/a.json").exists());
    }

    #[tokio::test]
    async fn request_stream_iterates_child_items_and_reads_the_result() {
        let r = rt(
            "operation s.count\n    private true\n    output json\n    emits integer\n    emit 1\n    emit 2\n    emit 3\n    return {done: true}\nend\n\noperation s.sum\n    output json\n    total = 0\n    with (request.stream \"s.count\" {}) as items\n        for n in items\n            total += n\n        end\n        final = items.result\n    end\n    return {total: total, final: final}\nend\n",
        );
        let c = r.request("s.sum", Value::Null, None).await.unwrap();
        assert_eq!(
            c.result,
            Value::object([
                ("total", Value::Int(6)),
                ("final", Value::object([("done", Value::Bool(true))]))
            ])
        );
    }

    // vhco:test execution.cancel_request -- a running top-level request is cancelled by id; its caller gets cancelled (exit 130) and the state becomes cancelled
    #[tokio::test]
    async fn cancel_running_request_by_id() {
        let r = rt(
            "operation slow.op\n    output json\n    status = poll every \"50ms\" timeout \"10s\"\n        until false\n        yield 1\n    end\n    return status\nend\n",
        );
        let req = r.new_request("slow.op", Value::Null, Principal::local());
        let id = req.request_id.clone();
        let r2 = r.clone();
        let task = tokio::spawn(async move { r2.dispatch_request(req, None).await });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(
            r.cancel(&id, Principal::local()).unwrap().state,
            "cancelling"
        );
        let e = task.await.unwrap().unwrap_err();
        assert_eq!(e.exit_code(), 130);
        assert_eq!(
            r.cancel(&id, Principal::local()).unwrap().state,
            "cancelled"
        );
        let other = Principal {
            name: "eve".into(),
            authenticated_by: "bearer".into(),
        };
        assert_eq!(r.cancel(&id, other).unwrap_err().code, "not_found.request");
    }
}
