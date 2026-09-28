//! Composition root for one loaded bundle: compiles the program, loads the
//! policy, wires every use case to its adapters and exposes the single
//! dispatcher used by the CLI, HTTP, MCP, WebSocket, polling and the library.

use crate::domain::contracts::{
    Catalog, CatalogQuery, Completion, DEFAULT_DEADLINE_MS, OutputReport, Principal, Request,
};
use crate::domain::errors::ErrorKind;
use crate::domain::files::FileOperation;
use crate::domain::ir::CompiledProgram;
use crate::domain::policy::{EffectIntent, Permit, Policy};
use crate::domain::ports::{
    DataSink, Dispatcher, FileAccess, PolicyEvaluator, PolicyLocator, Registry, SourceLoader,
};
use crate::domain::source::SourceBundle;
use crate::domain::{RivetError, RivetResult, Value};
use crate::features::datagrams::exchange_datagrams::exchange_datagrams;
use crate::features::execution::request_operation::request_operation;
use crate::features::files::apply_file_operation::{FileRequest, apply_file_operation};
use crate::features::language::compile_program::compile_program;
use crate::features::policy::authorize_effect::authorize_effect;
use crate::features::policy::load_policy::{load_policy, parse_policy};
use crate::features::quic::exchange_quic::exchange_quic;
use crate::features::registry::describe_operations::describe_operations;
use crate::features::registry::inspect_outputs::{OutputQuery, inspect_outputs};
use crate::infra::capy_parser::CapyParser;
use crate::infra::execution_driver::Interpreter;
use crate::infra::file_access::ConfinedFiles;
use crate::infra::policy_broker::PolicyBroker;
use crate::infra::policy_file_reader::DiskPolicyReader;
use crate::infra::quic_adapter::QuicAdapter;
use crate::infra::registry::ProgramRegistry;
use crate::infra::source_loader::DiskSourceLoader;
use crate::infra::udp_adapter::UdpAdapter;
use async_trait::async_trait;
use std::sync::Arc;
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
    concurrency: Arc<Semaphore>,
    counter: AtomicU64,
}

/// A loaded, compiled bundle ready to serve requests from every surface.
#[derive(Clone)]
pub struct Runtime {
    inner: Arc<Inner>,
}

/// FileAccess seen by the interpreter: every call runs the `files.apply_file_operation`
/// use case (authorization per intent) before the confined adapter touches disk.
struct PolicedFiles {
    evaluator: Arc<PolicyBroker>,
    raw: ConfinedFiles,
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
        let files: Arc<dyn FileAccess> = Arc::new(PolicedFiles {
            evaluator: Arc::clone(&broker),
            raw: ConfinedFiles::new(&bundle.root),
        });
        let evaluator: Arc<dyn PolicyEvaluator> = broker.clone();
        let mut interp = Interpreter::new(Arc::clone(&program), files, evaluator);
        register_transports(&mut interp, &bundle.root);
        let driver = Arc::new(interp);
        let registry = Arc::new(ProgramRegistry::new(Arc::clone(&program)));
        let inner = Arc::new(Inner {
            bundle,
            program,
            registry,
            driver,
            broker,
            concurrency: Arc::new(Semaphore::new(
                policy.limits.max_concurrent_requests as usize,
            )),
            counter: AtomicU64::new(0),
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
