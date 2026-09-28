//! Composition root for one loaded bundle: compiles the program, loads the
//! policy, wires every use case to its adapters and exposes the single
//! dispatcher used by the CLI, HTTP, MCP, WebSocket, polling and the library.

use crate::domain::contracts::{
    Catalog, CatalogQuery, Completion, DEFAULT_DEADLINE_MS, OutputReport, Principal, Request,
};
use crate::domain::errors::ErrorKind;
use crate::domain::files::FileOperation;
use crate::domain::io_manifest::{
    IoQuery, IoReport, PolicyDraft, TraceEvent, TraceQuery, TraceResult,
};
use crate::domain::ir::CompiledProgram;
use crate::domain::policy::{AccessVerb, Capability, Decision};
use crate::domain::policy::{EffectIntent, Permit, Policy};
use crate::domain::ports::GrpcDriver;
use crate::domain::ports::TraceStore;
use crate::domain::ports::{
    DataSink, Dispatcher, FileAccess, PolicyEvaluator, PolicyLocator, Registry, SessionDriver,
    SourceLoader,
};
use crate::domain::serve::OperationAccess;
use crate::domain::source::SourceBundle;
use crate::domain::{RivetError, RivetResult, Value};
use crate::features::audit::effect_sites::analyze_program;
use crate::features::audit::inspect_effects::{AuditPorts, inspect_effects};
use crate::features::audit::read_trace::read_trace;
use crate::features::datagrams::exchange_datagrams::exchange_datagrams;
use crate::features::execution::request_operation::request_operation;
use crate::features::files::apply_file_operation::{FileRequest, apply_file_operation};
use crate::features::grpc::invoke_rpc::{check_program as check_grpc_program, invoke_rpc};
use crate::features::language::compile_program::compile_program;
use crate::features::policy::authorize_effect::authorize_effect;
use crate::features::policy::generate_policy::{PolicyGenerateInput, generate_policy};
use crate::features::policy::load_policy::{load_policy, parse_policy};
use crate::features::quic::exchange_quic::exchange_quic;
use crate::features::registry::describe_operations::describe_operations;
use crate::features::registry::inspect_outputs::{OutputQuery, inspect_outputs};
use crate::features::serve::authorize_operation::require_operation;
use crate::infra::capy_parser::CapyParser;
use crate::infra::execution_driver::Interpreter;
use crate::infra::file_access::ConfinedFiles;
use crate::infra::grpc_adapter::{GrpcEffects, GrpcTransport, InvokeFn};
use crate::infra::policy_broker::PolicyBroker;
use crate::infra::policy_draft_writer::ExclusiveDraftWriter;
use crate::infra::policy_file_reader::DiskPolicyReader;
use crate::infra::quic_adapter::QuicAdapter;
use crate::infra::registry::ProgramRegistry;
use crate::infra::session_driver::SessionHost;
use crate::infra::source_loader::DiskSourceLoader;
use crate::infra::trace_store::{MemoryTraceStore, current_effect_scope};
use crate::infra::udp_adapter::UdpAdapter;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Semaphore;

/// How the host supplies policy.
pub enum PolicySource {
    /// Discover policy.json beside the entry file (or none → deny-by-default).
    Discover,
    /// `--policy PATH`.
    File(String),
    /// Library host: an already-parsed policy.
    Given(Policy),
}

/// Builder for a runtime over one bundle.
pub struct RuntimeBuilder {
    entry: Option<String>,
    source: Option<SourceBundle>,
    policy: PolicySource,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        RuntimeBuilder {
            entry: None,
            source: None,
            policy: PolicySource::Discover,
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

    pub fn policy_file(mut self, path: &str) -> Self {
        self.policy = PolicySource::File(path.to_string());
        self
    }

    pub fn policy(mut self, policy: Policy) -> Self {
        self.policy = PolicySource::Given(policy);
        self
    }

    pub fn build(self) -> RivetResult<Runtime> {
        let bundle = match (self.source, &self.entry) {
            (Some(b), _) => b,
            (None, Some(entry)) => DiskSourceLoader.load(entry)?,
            (None, None) => {
                return Err(RivetError::validation(
                    "validation.usage",
                    "no source: use .file(PATH) or .source(…)",
                ));
            }
        };
        let parser = CapyParser::new()?;
        let program = Arc::new(compile_program(&bundle, &parser)?);
        let policy = match self.policy {
            PolicySource::Given(p) => p,
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
        Runtime::assemble(bundle, program, policy)
    }
}

/// Parse policy JSON with the same strict schema as policy.json (library hosts).
pub fn policy_from_json(bytes: &[u8], base_dir: &str) -> RivetResult<Policy> {
    parse_policy(bytes, "<memory>", base_dir)
}

struct Inner {
    bundle: SourceBundle,
    program: Arc<CompiledProgram>,
    registry: Arc<ProgramRegistry>,
    driver: Arc<Interpreter>,
    broker: Arc<PolicyBroker>,
    trace: Arc<MemoryTraceStore>,
    concurrency: Arc<Semaphore>,
    counter: AtomicU64,
    /// Live sessions (polling, WebSocket refs, `rivet.sessions.*`, library).
    sessions: Arc<SessionHost>,
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

/// The evaluator every adapter sees: the broker's decision plus one trace event
/// per attempt, attributed to request + manifest effect_id through the
/// interpreter's effect scope.
struct TracedEvaluator {
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
        let permit = self.broker.evaluate(intent);
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
                node_id: None,
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
        apply_file_operation(
            FileRequest {
                op,
                operation_id: String::new(),
                span: None,
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

/// The dispatcher handed to the interpreter for nested `(request …)` calls.
struct NestedDispatcher {
    runtime: std::sync::Weak<Inner>,
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
        Runtime { inner }.dispatch_request(request, sink).await
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
    ) -> RivetResult<Runtime> {
        let decide: Arc<crate::infra::policy_broker::DecideFn> =
            Arc::new(|i: &EffectIntent, p: &Policy, root: &str| -> Permit {
                authorize_effect(i, p, root)
            });
        let broker = Arc::new(PolicyBroker::new(policy.clone(), &bundle.root, decide));
        let effects = analyze_program(&program);
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
        let trace = Arc::new(MemoryTraceStore::default());
        let evaluator: Arc<dyn PolicyEvaluator> = Arc::new(TracedEvaluator {
            broker: Arc::clone(&broker),
            trace: Arc::clone(&trace),
            index,
            attempts: Mutex::new(HashMap::new()),
        });
        let files: Arc<dyn FileAccess> = Arc::new(PolicedFiles {
            evaluator: Arc::clone(&evaluator),
            raw: ConfinedFiles::new(&bundle.root),
        });
        let mut interp = Interpreter::new(Arc::clone(&program), files, evaluator);
        register_transports(&mut interp, &bundle.root);
        // gRPC: descriptor sets are bootstrap reads; unknown methods or wrong
        // call modes fail the load before anything dials.
        let grpc = Arc::new(GrpcTransport::load(&program, &bundle.root)?);
        check_grpc_program(&program, grpc.catalog())?;
        let invoke: Arc<InvokeFn> = Arc::new(move |plan, policy| {
            let driver = Arc::clone(&grpc);
            Box::pin(async move { invoke_rpc(plan, policy.as_ref(), driver.as_ref()).await })
        });
        interp.register_adapter("grpc", Arc::new(GrpcEffects::new(invoke)));
        let driver = Arc::new(interp);
        let registry = Arc::new(ProgramRegistry::new(Arc::clone(&program)).with_effects(effects));
        let catalog_version = program.source_hash.clone();
        let inner = Arc::new_cyclic(|weak: &std::sync::Weak<Inner>| {
            let w = weak.clone();
            let upgrade: Arc<super::setup_library::UpgradeFn> =
                Arc::new(move || w.upgrade().map(|inner| Runtime { inner }));
            let sessions = Arc::new(super::setup_library::session_host(
                upgrade,
                registry.clone(),
                catalog_version,
            ));
            Inner {
                bundle,
                program,
                registry,
                driver,
                broker,
                trace,
                concurrency: Arc::new(Semaphore::new(
                    policy.limits.max_concurrent_requests as usize,
                )),
                counter: AtomicU64::new(0),
                sessions,
            }
        });
        inner.driver.set_dispatcher(Arc::new(NestedDispatcher {
            runtime: Arc::downgrade(&inner),
        }));
        Ok(Runtime { inner })
    }

    pub fn program(&self) -> Arc<CompiledProgram> {
        Arc::clone(&self.inner.program)
    }

    pub fn bundle(&self) -> &SourceBundle {
        &self.inner.bundle
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

    /// Dispatch a fully formed request (surfaces set principal, deadline, IDs).
    pub async fn dispatch_request(
        &self,
        req: Request,
        sink: Option<Arc<dyn DataSink>>,
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
        // Host-wide budget shared by nested calls and DAG nodes; nested calls run
        // inside their parent's permit, so only top-level requests acquire one.
        let _permit = if req.depth == 0 {
            Some(
                Arc::clone(&self.inner.concurrency)
                    .try_acquire_owned()
                    .map_err(|_| {
                        RivetError::new(
                            ErrorKind::Limit,
                            "limit.concurrency",
                            format!(
                                "limits.max_concurrent_requests ({}) reached",
                                limits.max_concurrent_requests
                            ),
                        )
                    })?,
            )
        } else {
            None
        };
        request_operation(
            req,
            self.inner.registry.as_ref(),
            self.inner.driver.as_ref(),
            &limits,
            sink,
        )
        .await
    }

    pub fn list(&self) -> RivetResult<Catalog> {
        describe_operations(&CatalogQuery::default(), self.inner.registry.as_ref())
    }

    pub fn describe(&self, ids: &[String]) -> RivetResult<Catalog> {
        describe_operations(
            &CatalogQuery {
                ids: ids.to_vec(),
                include_private: false,
            },
            self.inner.registry.as_ref(),
        )
    }

    pub fn outputs(&self, id: Option<&str>, all: bool) -> RivetResult<Vec<OutputReport>> {
        inspect_outputs(
            &OutputQuery {
                id: id.map(str::to_string),
                all,
            },
            self.inner.registry.as_ref(),
        )
    }

    pub fn registry(&self) -> Arc<dyn Registry> {
        self.inner.registry.clone()
    }

    /// Catalog version pinned by sessions (`sha256:` of the bundle sources).
    pub fn catalog_version(&self) -> String {
        self.inner.program.source_hash.clone()
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
        self.inner.driver.attach_input(&request_id, input);
        let out = self.dispatch_request(req, Some(sink)).await;
        self.inner.driver.detach_input(&request_id);
        out
    }

    fn static_evaluator(&self) -> StaticEvaluator {
        StaticEvaluator {
            policy: self.policy().clone(),
            root: self.inner.bundle.root.clone(),
        }
    }

    /// The I/O manifest (`rivet io`, `rivet.io`, `rt.io`): synchronous, reads only
    /// the compiled catalog and loaded policy; `check_files` probes needed paths
    /// through the broker, `trace_request_id` joins this host's trace store.
    pub fn io(&self, query: &IoQuery) -> RivetResult<IoReport> {
        let evaluator = self.static_evaluator();
        let probe = ConfinedFiles::new(&self.inner.bundle.root);
        inspect_effects(
            query,
            &AuditPorts {
                registry: self.inner.registry.as_ref(),
                policy: &evaluator,
                trace: Some(self.inner.trace.as_ref()),
                probe: Some(&probe),
            },
        )
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
            .map(|o| rebase_prefix(o, &self.inner.bundle.root))
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

    /// The raw trace store (for hosts that export or inspect it).
    pub fn trace_store(&self) -> Arc<dyn TraceStore> {
        self.inner.trace.clone()
    }
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
}
