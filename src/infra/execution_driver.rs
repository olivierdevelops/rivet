//! Tree-walking interpreter for compiled operations (satisfies ExecutionDriver).
//!
//! Every effect goes through a port (FileAccess, PolicyEvaluator, protocol
//! adapters); nested `(request …)` calls re-enter the shared dispatcher. The
//! whole run is bounded by the request deadline; DAG, `concurrent` and `map`
//! branches run cooperatively inside the request's own task and are dropped
//! (cancelled) together, so nothing outlives the request scope.

use crate::domain::contracts::{DataEvent, ExecutionPlan, Request, RunOutcome};
use crate::domain::errors::{EffectsStatus, ErrorKind};
use crate::domain::files::{Codec, FileOperation, FileVerb};
use crate::domain::ir::{
    Arg, BinOp, CatchFilter, CompiledProgram, DagNode, EffectForm, EffectKind, Expr, FailurePolicy,
    GroupOptions, MemberCall, Operation, Rhs, Stmt, TemplatePart,
};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::ports::{DataSink, Dispatcher, ExecutionDriver, FileAccess, PolicyEvaluator};
use crate::domain::source::SourceSpan;
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use base64::Engine;
use futures_util::future::BoxFuture;
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Adapter for protocol effects not built into the interpreter (HTTP, sockets,
/// processes, gRPC, QUIC, UDP, …). Registered per `EffectKind` by the
/// orchestrator; an unregistered kind fails with `unsupported.adapter`.
#[async_trait]
pub trait EffectAdapter: Send + Sync {
    /// One-shot form (`x = http get …`, `command …`).
    async fn run(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Value> {
        let _ = (ctx, args);
        Err(RivetError::unsupported(
            "unsupported.form",
            format!(
                "`{}` has no one-shot form; use `with {} … as NAME`",
                form.kind.as_str(),
                form.kind.as_str()
            ),
        ))
    }

    /// Scoped form (`with KIND … as NAME … end`). The returned handle is owned
    /// by the enclosing scope and closed when the scope exits for any reason.
    async fn open(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let _ = (ctx, args);
        Err(RivetError::unsupported(
            "unsupported.form",
            format!("`{}` has no scoped form", form.kind.as_str()),
        ))
    }
}

/// An open, scope-owned resource (`with … as NAME`). Handles are not values:
/// they cannot be returned, stored or passed to DAG nodes.
#[async_trait]
pub trait ResourceHandle: Send {
    /// `NAME.method ARGS` (`socket.send json {…}`, `rpc.finish_send`,
    /// `process.stdin.send json {…}` → method `stdin.send`).
    async fn call(
        &mut self,
        ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
    ) -> RivetResult<Value> {
        let _ = (ctx, args);
        Err(RivetError::unsupported(
            "unsupported.method",
            format!("this resource has no `{method}` method"),
        ))
    }

    /// `for item in NAME`; `Ok(None)` ends the iteration.
    async fn next(&mut self, ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        let _ = ctx;
        Err(RivetError::unsupported(
            "unsupported.iterate",
            "this resource cannot be iterated",
        ))
    }

    /// `NAME.property` reads (`rpc.completion`, `events.result`).
    async fn property(&mut self, ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        let _ = ctx;
        Err(RivetError::unsupported(
            "unsupported.property",
            format!("this resource has no `{name}` property"),
        ))
    }

    /// `with NAME.open bidi as child` — a child resource of this one.
    async fn open_child(
        &mut self,
        ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
        options: Vec<(String, Vec<EvalArg>)>,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let _ = (ctx, args, options);
        Err(RivetError::unsupported(
            "unsupported.child",
            format!("this resource cannot `{method}` a child resource"),
        ))
    }

    /// Graceful close; the caller bounds it by the cleanup grace period.
    async fn close(self: Box<Self>) -> RivetResult<()> {
        Ok(())
    }

    /// Resources whose directions progress independently (gRPC bidi: one
    /// `concurrent` task sends while another iterates) return a shared view;
    /// the interpreter then calls it without holding the scope lock, so a
    /// blocked `next` never blocks a `send`.
    fn shared(&self) -> Option<Arc<dyn SharedResource>> {
        None
    }
}

/// Lock-free view of a scope-owned resource (see `ResourceHandle::shared`).
#[async_trait]
pub trait SharedResource: Send + Sync {
    async fn call(&self, ctx: &EffectCtx, method: &str, args: Vec<EvalArg>) -> RivetResult<Value>;
    async fn next(&self, ctx: &EffectCtx) -> RivetResult<Option<Value>>;
    async fn property(&self, ctx: &EffectCtx, name: &str) -> RivetResult<Value>;
}

async fn shared_of(handle: &SharedHandle) -> Option<Arc<dyn SharedResource>> {
    handle.lock().await.as_ref().and_then(|h| h.shared())
}

/// Cleanup grace period (PROP-2026-0001 defaults).
pub const CLEANUP_GRACE: Duration = Duration::from_secs(5);

type SharedHandle = Arc<tokio::sync::Mutex<Option<Box<dyn ResourceHandle>>>>;

/// Head arguments and options with every expression evaluated.
#[derive(Clone, Debug, Default)]
pub struct EvaluatedForm {
    pub head: Vec<EvalArg>,
    pub options: Vec<(String, Vec<EvalArg>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EvalArg {
    Word(String),
    Value(Value),
}

impl EvalArg {
    pub fn word(&self) -> Option<&str> {
        match self {
            EvalArg::Word(w) => Some(w),
            _ => None,
        }
    }

    pub fn value(&self) -> Option<&Value> {
        match self {
            EvalArg::Value(v) => Some(v),
            _ => None,
        }
    }
}

impl EvaluatedForm {
    pub fn option(&self, key: &str) -> Option<&[EvalArg]> {
        self.options
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, a)| a.as_slice())
    }
}

/// What an adapter needs to know about the calling request.
pub struct EffectCtx {
    pub operation_id: String,
    pub request: Request,
    pub policy: Arc<dyn PolicyEvaluator>,
    pub span: SourceSpan,
    pub deadline: Instant,
}

impl EffectCtx {
    /// Authorize one intent; a denial is `permission.denied` with the rule.
    pub fn authorize(
        &self,
        capability: Capability,
        verb: AccessVerb,
        target: EffectTarget,
    ) -> RivetResult<()> {
        let permit = self.policy.evaluate(&EffectIntent {
            capability,
            verb,
            target: target.clone(),
            operation_id: self.operation_id.clone(),
            effect_id: None,
            span: Some(self.span.clone()),
        });
        if permit.decision == Decision::Denied {
            return Err(RivetError::permission(format!(
                "{} {} {} denied: {}",
                capability.as_str(),
                verb.as_str(),
                target.as_str(),
                permit.rule
            ))
            .with_span(Some(self.span.clone()))
            .with_details(Value::object([
                ("capability", Value::text(capability.as_str())),
                ("access", Value::text(verb.as_str())),
                ("target", Value::text(target.as_str())),
            ])));
        }
        Ok(())
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

// vhco:infra execution_driver satisfies ExecutionDriver
pub struct Interpreter {
    program: Arc<CompiledProgram>,
    files: Arc<dyn FileAccess>,
    policy: Arc<dyn PolicyEvaluator>,
    dispatcher: OnceLock<Arc<dyn Dispatcher>>,
    adapters: HashMap<String, Arc<dyn EffectAdapter>>,
}

impl Interpreter {
    pub fn new(
        program: Arc<CompiledProgram>,
        files: Arc<dyn FileAccess>,
        policy: Arc<dyn PolicyEvaluator>,
    ) -> Interpreter {
        Interpreter {
            program,
            files,
            policy,
            dispatcher: OnceLock::new(),
            adapters: HashMap::new(),
        }
    }

    /// Nested `(request …)` target; set once by the orchestrator after the dispatcher exists.
    pub fn set_dispatcher(&self, d: Arc<dyn Dispatcher>) {
        let _ = self.dispatcher.set(d);
    }

    pub fn register_adapter(&mut self, kind: &str, adapter: Arc<dyn EffectAdapter>) {
        self.adapters.insert(kind.to_string(), adapter);
    }
}

#[async_trait]
impl ExecutionDriver for Interpreter {
    async fn drive(
        &self,
        plan: ExecutionPlan,
        sink: Option<Arc<dyn DataSink>>,
    ) -> RivetResult<RunOutcome> {
        let op = self
            .program
            .operation(&plan.request.operation_id)
            .ok_or_else(|| {
                RivetError::not_found(
                    "not_found.operation",
                    format!("no operation `{}`", plan.request.operation_id),
                )
            })?
            .clone();
        let deadline_ms = plan.request.deadline_ms.max(1);
        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        let run = Arc::new(RunState {
            data_seq: AtomicU64::new(0),
            effects: AtomicU8::new(0),
            deadline,
            sink,
        });
        let mut frame = Frame::new(&plan.request, Arc::clone(&run));
        if let Value::Object(pairs) = &plan.params {
            for (k, v) in pairs {
                frame.define(k, v.clone());
            }
        }
        let machine = Machine {
            interp: self,
            op: &op,
        };
        let body = machine.exec_block(&mut frame, &op.body);
        let flow = match tokio::time::timeout(Duration::from_millis(deadline_ms), body).await {
            Ok(r) => r.map_err(|e| e.with_effects(run.effects()))?,
            Err(_) => {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.request",
                    format!("`{}` exceeded its {deadline_ms} ms deadline", op.id),
                )
                .with_effects(run.effects()));
            }
        };
        let result = match flow {
            Flow::Return(v) => v,
            Flow::Normal => Value::Null,
            Flow::Break => {
                return Err(RivetError::syntax(
                    "syntax.break",
                    "`break` outside a loop",
                    None,
                ));
            }
            Flow::Yield(_) => {
                return Err(RivetError::syntax(
                    "syntax.yield",
                    "`yield` outside `map`/`poll`",
                    None,
                ));
            }
        };
        Ok(RunOutcome {
            result,
            data_count: run.data_seq.load(Ordering::SeqCst),
            effects: run.effects(),
        })
    }
}

struct RunState {
    data_seq: AtomicU64,
    /// 0 none, 1 committed.
    effects: AtomicU8,
    deadline: Instant,
    sink: Option<Arc<dyn DataSink>>,
}

impl RunState {
    fn effects(&self) -> EffectsStatus {
        if self.effects.load(Ordering::SeqCst) > 0 {
            EffectsStatus::Committed
        } else {
            EffectsStatus::None
        }
    }

    fn commit(&self) {
        self.effects.store(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct Frame {
    scopes: Vec<HashMap<String, Value>>,
    request: Request,
    run: Arc<RunState>,
    secrets: HashMap<String, Vec<String>>,
    /// Open resource handles, innermost last.
    handles: Vec<(String, SharedHandle)>,
}

impl Frame {
    fn new(request: &Request, run: Arc<RunState>) -> Frame {
        Frame {
            scopes: vec![HashMap::new()],
            request: request.clone(),
            run,
            secrets: HashMap::new(),
            handles: Vec::new(),
        }
    }

    fn handle(&self, name: &str) -> Option<SharedHandle> {
        self.handles
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, h)| Arc::clone(h))
    }

    fn define(&mut self, name: &str, v: Value) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), v);
        }
    }

    /// Assign to the nearest scope that has the name; a new name is an
    /// operation-level variable (visible after the block that assigned it).
    /// Loop variables, `error` and parameters are block bindings via `define`.
    fn assign(&mut self, name: &str, v: Value) {
        for s in self.scopes.iter_mut().rev() {
            if let Some(slot) = s.get_mut(name) {
                *slot = v;
                return;
            }
        }
        if let Some(s) = self.scopes.first_mut() {
            s.insert(name.to_string(), v);
        }
    }

    fn get(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
}

enum Flow {
    Normal,
    Return(Value),
    Break,
    Yield(Value),
}

struct Machine<'a> {
    interp: &'a Interpreter,
    op: &'a Operation,
}

fn runtime_err(code: &str, msg: impl Into<String>, span: &SourceSpan) -> RivetError {
    RivetError::new(ErrorKind::Validation, code, msg).with_span(Some(span.clone()))
}

impl<'a> Machine<'a> {
    fn exec_block<'b>(
        &'b self,
        frame: &'b mut Frame,
        body: &'b [Stmt],
    ) -> BoxFuture<'b, RivetResult<Flow>> {
        Box::pin(async move {
            for stmt in body {
                if Instant::now() >= frame.run.deadline {
                    return Err(RivetError::new(
                        ErrorKind::Timeout,
                        "timeout.request",
                        "request deadline exhausted",
                    )
                    .with_span(Some(stmt.span().clone())));
                }
                match self.exec(frame, stmt).await? {
                    Flow::Normal => {}
                    other => return Ok(other),
                }
            }
            Ok(Flow::Normal)
        })
    }

    /// Run a nested block in its own lexical scope.
    async fn scoped(&self, frame: &mut Frame, body: &[Stmt]) -> RivetResult<Flow> {
        frame.scopes.push(HashMap::new());
        let r = self.exec_block(frame, body).await;
        frame.scopes.pop();
        r
    }

    fn exec<'b>(
        &'b self,
        frame: &'b mut Frame,
        stmt: &'b Stmt,
    ) -> BoxFuture<'b, RivetResult<Flow>> {
        Box::pin(async move {
            match stmt {
                Stmt::Assign {
                    var,
                    rhs,
                    options,
                    span,
                } => {
                    let v = self.eval_rhs(frame, rhs, options, span).await?;
                    frame.assign(var, v);
                    Ok(Flow::Normal)
                }
                Stmt::Append { var, value, span } => {
                    let add = self.eval(frame, value).await?;
                    let cur = frame.get(var).cloned().unwrap_or(Value::Null);
                    let next = match (cur, add) {
                        (Value::Null, a) => a,
                        (Value::Text(mut a), Value::Text(b)) => {
                            a.push_str(&b);
                            Value::Text(a)
                        }
                        (Value::List(mut a), Value::List(b)) => {
                            a.extend(b);
                            Value::List(a)
                        }
                        (Value::List(mut a), b) => {
                            a.push(b);
                            Value::List(a)
                        }
                        (a, b) => arith(BinOp::Add, a, b, span)?,
                    };
                    frame.assign(var, next);
                    Ok(Flow::Normal)
                }
                Stmt::Return { value, span } => {
                    Ok(Flow::Return(self.eval_rhs(frame, value, &[], span).await?))
                }
                Stmt::Yield { value, span } => {
                    Ok(Flow::Yield(self.eval_rhs(frame, value, &[], span).await?))
                }
                Stmt::Emit { value, span } => {
                    let v = self.eval_rhs(frame, value, &[], span).await?;
                    self.emit(frame, v, span).await?;
                    Ok(Flow::Normal)
                }
                Stmt::Fail {
                    code,
                    details,
                    span,
                } => {
                    let d = self.eval(frame, details).await?;
                    let declared = self
                        .op
                        .errors
                        .iter()
                        .find(|e| &e.code == code)
                        .and_then(|e| e.description.clone());
                    let mut e = RivetError::new(
                        ErrorKind::Application,
                        code.clone(),
                        declared.unwrap_or_else(|| format!("operation failed: {code}")),
                    )
                    .with_details(d)
                    .with_span(Some(span.clone()));
                    e.operation_id = Some(self.op.id.clone());
                    Err(e)
                }
                Stmt::Break { .. } => Ok(Flow::Break),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    if self.eval(frame, cond).await?.truthy() {
                        self.scoped(frame, then).await
                    } else {
                        self.scoped(frame, otherwise).await
                    }
                }
                Stmt::While { cond, body, .. } => {
                    while self.eval(frame, cond).await?.truthy() {
                        match self.scoped(frame, body).await? {
                            Flow::Break => break,
                            Flow::Normal => {}
                            other => return Ok(other),
                        }
                        tokio::task::yield_now().await;
                    }
                    Ok(Flow::Normal)
                }
                Stmt::For {
                    var,
                    iter,
                    body,
                    span,
                } => {
                    if let Expr::Path(path, _) = iter {
                        if path.len() == 1 && frame.get(&path[0]).is_none() {
                            if let Some(handle) = frame.handle(&path[0]) {
                                loop {
                                    let ctx = self.ctx(frame, span);
                                    let item = if let Some(s) = shared_of(&handle).await {
                                        s.next(&ctx)
                                            .await
                                            .map_err(|e| e.with_span(Some(span.clone())))?
                                    } else {
                                        let mut guard = handle.lock().await;
                                        let h = guard.as_mut().ok_or_else(|| {
                                            RivetError::new(
                                                ErrorKind::Cleanup,
                                                "cleanup.closed",
                                                format!("`{}` is already closed", path[0]),
                                            )
                                        })?;
                                        h.next(&ctx)
                                            .await
                                            .map_err(|e| e.with_span(Some(span.clone())))?
                                    };
                                    let Some(item) = item else { break };
                                    frame.scopes.push(HashMap::from([(var.clone(), item)]));
                                    let r = self.exec_block(frame, body).await;
                                    frame.scopes.pop();
                                    match r? {
                                        Flow::Break => break,
                                        Flow::Normal => {}
                                        other => return Ok(other),
                                    }
                                }
                                return Ok(Flow::Normal);
                            }
                        }
                    }
                    let items = match self.eval(frame, iter).await? {
                        Value::List(items) => items,
                        other => {
                            return Err(runtime_err(
                                "value.not_iterable",
                                format!("`for` needs a list, got {}", other.type_name()),
                                span,
                            ));
                        }
                    };
                    for item in items {
                        frame.scopes.push(HashMap::from([(var.clone(), item)]));
                        let r = self.exec_block(frame, body).await;
                        frame.scopes.pop();
                        match r? {
                            Flow::Break => break,
                            Flow::Normal => {}
                            other => return Ok(other),
                        }
                    }
                    Ok(Flow::Normal)
                }
                Stmt::Try {
                    body,
                    filter,
                    handler,
                    ..
                } => match self.scoped(frame, body).await {
                    Err(e) if catches(filter, &e) => {
                        frame
                            .scopes
                            .push(HashMap::from([("error".to_string(), e.to_value())]));
                        let r = self.exec_block(frame, handler).await;
                        frame.scopes.pop();
                        r
                    }
                    other => other,
                },
                Stmt::Iterate { max, body, .. } => {
                    for _ in 0..*max {
                        match self.scoped(frame, body).await? {
                            Flow::Break => break,
                            Flow::Normal => {}
                            other => return Ok(other),
                        }
                    }
                    Ok(Flow::Normal)
                }
                Stmt::Scope {
                    timeout_ms,
                    body,
                    span,
                } => match timeout_ms {
                    Some(ms) => match tokio::time::timeout(
                        Duration::from_millis(*ms),
                        self.scoped(frame, body),
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(_) => Err(RivetError::new(
                            ErrorKind::Timeout,
                            "timeout.scope",
                            format!("scope exceeded {ms} ms"),
                        )
                        .with_span(Some(span.clone()))),
                    },
                    None => self.scoped(frame, body).await,
                },
                Stmt::Dag {
                    options,
                    nodes,
                    span,
                } => {
                    self.run_dag(frame, options, nodes, span).await?;
                    Ok(Flow::Normal)
                }
                Stmt::Concurrent {
                    options,
                    tasks,
                    span,
                } => self.run_concurrent(frame, options, tasks, span).await,
                Stmt::Effect { form, span } => {
                    self.run_effect(frame, form, span).await?;
                    Ok(Flow::Normal)
                }
                Stmt::Call { expr, .. } => {
                    self.eval(frame, expr).await?;
                    Ok(Flow::Normal)
                }
                Stmt::Secret {
                    name,
                    env,
                    origins,
                    span,
                } => {
                    let ctx = self.ctx(frame, span);
                    ctx.authorize(
                        Capability::Env,
                        AccessVerb::Read,
                        EffectTarget::Env(env.clone()),
                    )?;
                    let value = std::env::var(env).map_err(|_| {
                        RivetError::not_found(
                            "not_found.env",
                            format!("environment variable {env} is not set"),
                        )
                        .with_span(Some(span.clone()))
                    })?;
                    frame.secrets.insert(name.clone(), origins.clone());
                    frame.define(name, Value::Text(value));
                    Ok(Flow::Normal)
                }
                Stmt::Until { cond, .. } => {
                    // Evaluated by the enclosing `poll`; outside poll it is rejected at compile time.
                    let _ = self.eval(frame, cond).await?;
                    Ok(Flow::Normal)
                }
                Stmt::With {
                    form,
                    source,
                    bind,
                    body,
                    span,
                } => {
                    self.run_with(frame, form, source.as_ref(), bind.as_deref(), body, span)
                        .await
                }
                Stmt::Member { call, span } => {
                    self.member(frame, call, span).await?;
                    Ok(Flow::Normal)
                }
            }
        })
    }

    fn ctx(&self, frame: &Frame, span: &SourceSpan) -> EffectCtx {
        EffectCtx {
            operation_id: self.op.id.clone(),
            request: frame.request.clone(),
            policy: Arc::clone(&self.interp.policy),
            span: span.clone(),
            deadline: frame.run.deadline,
        }
    }

    fn unsupported(&self, form: &EffectForm, span: &SourceSpan) -> RivetError {
        RivetError::unsupported(
            "unsupported.adapter",
            format!(
                "`{}` resources are not available in this build",
                form.kind.as_str()
            ),
        )
        .with_span(Some(span.clone()))
    }

    async fn member(
        &self,
        frame: &mut Frame,
        call: &MemberCall,
        span: &SourceSpan,
    ) -> RivetResult<Value> {
        let name = &call.object[0];
        let Some(handle) = frame.handle(name) else {
            return Err(RivetError::unsupported(
                "unsupported.handle",
                format!(
                    "`{}.{}`: `{name}` is not an open resource in this scope",
                    call.object.join("."),
                    call.method
                ),
            )
            .with_span(Some(span.clone())));
        };
        let mut method: Vec<&str> = call.object[1..].iter().map(String::as_str).collect();
        method.push(&call.method);
        let method = method.join(".");
        let mut args = Vec::with_capacity(call.args.len());
        for a in &call.args {
            args.push(self.eval_arg(frame, a).await?);
        }
        let ctx = self.ctx(frame, span);
        let r = if let Some(s) = shared_of(&handle).await {
            s.call(&ctx, &method, args).await
        } else {
            let mut guard = handle.lock().await;
            let h = guard.as_mut().ok_or_else(|| {
                RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.closed",
                    format!("`{name}` is already closed"),
                )
            })?;
            h.call(&ctx, &method, args).await
        }
        .map_err(|e| e.with_span(Some(span.clone())));
        if r.is_ok() && (method.contains("send") || method.contains("write")) {
            frame.run.commit();
        }
        r
    }

    /// `with RESOURCE … as NAME … end`: open, run the body, always close
    /// (reverse acquisition order is structural: inner scopes close first).
    async fn run_with(
        &self,
        frame: &mut Frame,
        form: &EffectForm,
        source: Option<&Expr>,
        bind: Option<&str>,
        body: &[Stmt],
        span: &SourceSpan,
    ) -> RivetResult<Flow> {
        let evaluated = self.evaluate_form(frame, form).await?;
        let ctx = self.ctx(frame, span);
        let opened: RivetResult<Box<dyn ResourceHandle>> = match (&form.kind, source) {
            (
                EffectKind::RequestStream,
                Some(Expr::Call {
                    args, span: cspan, ..
                }),
            ) => {
                let mut vals = Vec::new();
                for a in args {
                    vals.push(self.eval(frame, a).await?);
                }
                self.open_request_stream(frame, vals, cspan).await
            }
            (EffectKind::Connection, Some(Expr::Path(path, pspan))) => match frame.handle(&path[0])
            {
                None => Err(RivetError::unsupported(
                    "unsupported.handle",
                    format!("`{}` is not an open resource in this scope", path[0]),
                )
                .with_span(Some(pspan.clone()))),
                Some(parent) => {
                    let method = path[1..].join(".");
                    let mut guard = parent.lock().await;
                    match guard.as_mut() {
                        Some(h) => {
                            h.open_child(
                                &ctx,
                                &method,
                                evaluated.head.clone(),
                                evaluated.options.clone(),
                            )
                            .await
                        }
                        None => Err(RivetError::new(
                            ErrorKind::Cleanup,
                            "cleanup.closed",
                            format!("`{}` is already closed", path[0]),
                        )),
                    }
                }
            },
            _ => match self.interp.adapters.get(form.kind.as_str()) {
                Some(adapter) => adapter.open(&ctx, form, evaluated).await,
                None => Err(self.unsupported(form, span)),
            },
        };
        let handle: SharedHandle = Arc::new(tokio::sync::Mutex::new(Some(
            opened.map_err(|e| e.with_span(Some(span.clone())))?,
        )));
        let name = bind.unwrap_or("_").to_string();
        frame.handles.push((name.clone(), Arc::clone(&handle)));
        let result = self.scoped(frame, body).await;
        if let Some(pos) = frame
            .handles
            .iter()
            .rposition(|(n, h)| n == &name && Arc::ptr_eq(h, &handle))
        {
            frame.handles.remove(pos);
        }
        // Always close, after success, error, break or return; the primary error wins.
        let taken = handle.lock().await.take();
        let closed = match taken {
            Some(h) => match tokio::time::timeout(CLEANUP_GRACE, h.close()).await {
                Ok(r) => r,
                Err(_) => Err(RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.timeout",
                    format!(
                        "`{name}` did not close within {} s",
                        CLEANUP_GRACE.as_secs()
                    ),
                )),
            },
            None => Ok(()),
        };
        match (result, closed) {
            (Ok(flow), Ok(())) => Ok(flow),
            (Ok(_), Err(e)) => Err(RivetError::new(
                ErrorKind::Cleanup,
                "cleanup.failed",
                format!("closing `{name}` failed: {}", e.message),
            )
            .with_span(Some(span.clone()))),
            (Err(e), Ok(())) => Err(e),
            (Err(mut e), Err(c)) => {
                e.suppressed.push(c);
                Err(e)
            }
        }
    }

    /// Built-in `with (request.stream ID PARAMS) as events`: the child request
    /// runs in a task owned by this scope; items arrive through a bounded
    /// channel (16 frames) so a slow body applies backpressure.
    async fn open_request_stream(
        &self,
        frame: &mut Frame,
        vals: Vec<Value>,
        span: &SourceSpan,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let id = vals
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| {
                runtime_err(
                    "call.request",
                    "(request.stream ID PARAMS) needs an operation ID string",
                    span,
                )
            })?
            .to_string();
        let params = vals.get(1).cloned().unwrap_or(Value::Object(vec![]));
        let dispatcher = Arc::clone(
            self.interp
                .dispatcher
                .get()
                .ok_or_else(|| RivetError::internal("nested dispatcher not configured"))?,
        );
        let child = Request {
            request_id: format!("{}.s{}", frame.request.request_id, rand_suffix()),
            trace_id: frame.request.trace_id.clone(),
            operation_id: id,
            params,
            principal: frame.request.principal.clone(),
            parent_request_id: Some(frame.request.request_id.clone()),
            depth: frame.request.depth + 1,
            deadline_ms: frame
                .run
                .deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .max(1) as u64,
            include_private: true,
        };
        let (tx, rx) = tokio::sync::mpsc::channel::<Value>(16);
        let sink: Arc<dyn DataSink> = Arc::new(ChannelSink { tx });
        let task = tokio::spawn(async move { dispatcher.dispatch(child, Some(sink)).await });
        Ok(Box::new(RequestStreamHandle {
            rx,
            task: Some(task),
            completion: None,
        }))
    }

    async fn emit(&self, frame: &mut Frame, v: Value, span: &SourceSpan) -> RivetResult<()> {
        if self.op.emits.is_none() {
            return Err(runtime_err(
                "stream.emits_undeclared",
                format!("`{}` emits items but declares no `emits` type", self.op.id),
                span,
            ));
        }
        let seq = frame.run.data_seq.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(sink) = &frame.run.sink {
            sink.send(DataEvent {
                request_id: frame.request.request_id.clone(),
                trace_id: frame.request.trace_id.clone(),
                seq,
                data: v,
            })
            .await
            .map_err(|e| {
                let mut err = RivetError::new(
                    ErrorKind::ConsumerFailed,
                    "consumer_failed",
                    format!("the data consumer stopped: {}", e.message),
                );
                err.cause = Some(Box::new(e));
                err
            })?;
        }
        Ok(())
    }

    fn eval_rhs<'b>(
        &'b self,
        frame: &'b mut Frame,
        rhs: &'b Rhs,
        options: &'b [crate::domain::ir::OptionLine],
        span: &'b SourceSpan,
    ) -> BoxFuture<'b, RivetResult<Value>> {
        Box::pin(async move {
            match rhs {
                Rhs::Expr(e) => {
                    let allow = options.iter().find(|o| o.key == "allow");
                    match (allow, e) {
                        (
                            Some(a),
                            Expr::Call {
                                func,
                                args,
                                span: cspan,
                            },
                        ) if func == "request" => {
                            let allowed = match a.args.first().map(|x| x.to_expr()) {
                                Some(list) => self.eval(frame, &list).await?,
                                None => Value::List(vec![]),
                            };
                            let id = match args.first() {
                                Some(x) => self.eval(frame, x).await?,
                                None => Value::Null,
                            };
                            let permitted =
                                matches!(&allowed, Value::List(ids) if ids.contains(&id));
                            if !permitted {
                                return Err(RivetError::permission(format!(
                                    "dynamic request to {id} is not in the `allow` list"
                                ))
                                .with_span(Some(cspan.clone())));
                            }
                            self.eval(frame, e).await
                        }
                        _ => self.eval(frame, e).await,
                    }
                }
                Rhs::Effect(form) => self.run_effect(frame, form, span).await,
                Rhs::Member(call) => self.member(frame, call, span).await,
                Rhs::Map {
                    item,
                    iter,
                    limit,
                    body,
                } => self.run_map(frame, item, iter, *limit, body, span).await,
                Rhs::Poll {
                    every,
                    timeout,
                    body,
                } => self.run_poll(frame, *every, *timeout, body, span).await,
            }
        })
    }

    async fn evaluate_form(
        &self,
        frame: &mut Frame,
        form: &EffectForm,
    ) -> RivetResult<EvaluatedForm> {
        let mut out = EvaluatedForm::default();
        for a in &form.head {
            out.head.push(self.eval_arg(frame, a).await?);
        }
        for o in &form.options {
            let mut args = Vec::new();
            for a in &o.args {
                args.push(self.eval_arg(frame, a).await?);
            }
            out.options.push((o.key.clone(), args));
        }
        Ok(out)
    }

    async fn eval_arg(&self, frame: &mut Frame, a: &Arg) -> RivetResult<EvalArg> {
        Ok(match a {
            Arg::Word(w, _) if frame.get(w).is_none() => EvalArg::Word(w.clone()),
            Arg::Word(w, _) => EvalArg::Value(frame.get(w).cloned().unwrap_or(Value::Null)),
            Arg::Expr(e, _) => EvalArg::Value(self.eval(frame, e).await?),
        })
    }

    async fn run_effect(
        &self,
        frame: &mut Frame,
        form: &EffectForm,
        span: &SourceSpan,
    ) -> RivetResult<Value> {
        let evaluated = self.evaluate_form(frame, form).await?;
        let result = if form.kind == EffectKind::File {
            self.run_file(frame, &evaluated, span).await
        } else {
            match self.interp.adapters.get(form.kind.as_str()) {
                Some(adapter) => adapter.run(&self.ctx(frame, span), form, evaluated).await,
                None => Err(self.unsupported(form, span)),
            }
        };
        if result.is_ok() && mutates(form) {
            frame.run.commit();
        }
        result.map_err(|e| e.with_span(Some(span.clone())))
    }

    async fn run_file(
        &self,
        frame: &mut Frame,
        f: &EvaluatedForm,
        span: &SourceSpan,
    ) -> RivetResult<Value> {
        let bad = |m: &str| runtime_err("file.form", m.to_string(), span);
        let verb = f
            .head
            .first()
            .and_then(EvalArg::word)
            .and_then(FileVerb::parse)
            .ok_or_else(|| bad("expected `file VERB PATH`"))?;
        let path = match f.head.get(1) {
            Some(EvalArg::Value(Value::Text(p))) => p.clone(),
            Some(EvalArg::Word(w)) => {
                return Err(bad(&format!(
                    "`{w}` is not defined; file paths are strings"
                )));
            }
            _ => return Err(bad("expected a path string after the verb")),
        };
        let mut op = FileOperation::new(verb, path);
        let mut rest = f.head[2..].iter().peekable();
        while let Some(a) = rest.next() {
            match a.word() {
                Some("as") => op.codec = rest.next().and_then(EvalArg::word).and_then(Codec::parse),
                Some("to") => {
                    op.to = rest
                        .next()
                        .and_then(EvalArg::value)
                        .and_then(|v| v.as_str().map(str::to_string))
                }
                Some(w) if Codec::parse(w).is_some() => {
                    op.codec = Codec::parse(w);
                    if matches!(
                        verb,
                        FileVerb::Create | FileVerb::Update | FileVerb::Write | FileVerb::Append
                    ) {
                        op.content = rest.next().map(|v| match v {
                            EvalArg::Value(v) => v.clone(),
                            EvalArg::Word(w) => Value::Text(w.clone()),
                        });
                    }
                }
                _ => return Err(bad("unexpected argument in file statement")),
            }
        }
        for (k, args) in &f.options {
            match k.as_str() {
                "if_version" => {
                    op.if_version = args
                        .first()
                        .and_then(EvalArg::value)
                        .map(|v| v.to_display())
                }
                "missing" => op.missing_ok = args.first().and_then(EvalArg::word) == Some("ok"),
                "overwrite" => {
                    op.overwrite = !matches!(args.first(), Some(EvalArg::Value(Value::Bool(false))))
                }
                other => {
                    return Err(bad(&format!(
                        "option `{other}` is not valid for file statements"
                    )));
                }
            }
        }
        // Authorization happens in the files use case wrapped behind FileAccess.
        let _ = frame;
        self.interp.files.apply(op).await
    }

    async fn run_map(
        &self,
        frame: &mut Frame,
        item: &str,
        iter: &Expr,
        limit: Option<i64>,
        body: &[Stmt],
        span: &SourceSpan,
    ) -> RivetResult<Value> {
        let items = match self.eval(frame, iter).await? {
            Value::List(items) => items,
            other => {
                return Err(runtime_err(
                    "value.not_iterable",
                    format!("`map` needs a list, got {}", other.type_name()),
                    span,
                ));
            }
        };
        let limit = limit.unwrap_or(4).max(1) as usize;
        let mut results: Vec<Option<Value>> = vec![None; items.len()];
        let mut pending = items.into_iter().enumerate();
        let mut running = FuturesUnordered::new();
        loop {
            while running.len() < limit {
                let Some((i, value)) = pending.next() else {
                    break;
                };
                let mut child = frame.clone();
                child
                    .scopes
                    .push(HashMap::from([(item.to_string(), value)]));
                running.push(async move {
                    let r = self.exec_block(&mut child, body).await;
                    (i, r)
                });
            }
            let Some((i, r)) = running.next().await else {
                break;
            };
            match r? {
                Flow::Yield(v) => results[i] = Some(v),
                Flow::Return(_) => {
                    return Err(runtime_err(
                        "syntax.yield",
                        "`return` inside `map` exits the operation; use `yield` for the item value",
                        span,
                    ));
                }
                _ => {
                    return Err(runtime_err(
                        "syntax.map_yield",
                        "a `map` body finished without `yield`",
                        span,
                    ));
                }
            }
        }
        Ok(Value::List(
            results
                .into_iter()
                .map(|v| v.unwrap_or(Value::Null))
                .collect(),
        ))
    }

    async fn run_poll(
        &self,
        frame: &mut Frame,
        every: Option<u64>,
        timeout: Option<u64>,
        body: &[Stmt],
        span: &SourceSpan,
    ) -> RivetResult<Value> {
        let start = Instant::now();
        let every = Duration::from_millis(every.unwrap_or(1_000));
        let limit = Duration::from_millis(timeout.unwrap_or(30_000));
        loop {
            frame.scopes.push(HashMap::new());
            let mut yielded = None;
            let mut done = false;
            for stmt in body {
                if let Stmt::Until { cond, .. } = stmt {
                    if self.eval(frame, cond).await?.truthy() {
                        done = true;
                    }
                    continue;
                }
                match self.exec(frame, stmt).await {
                    Ok(Flow::Normal) => {}
                    Ok(Flow::Yield(v)) => {
                        yielded = Some(v);
                        break;
                    }
                    Ok(Flow::Return(_)) => {
                        frame.scopes.pop();
                        return Err(runtime_err(
                            "syntax.yield",
                            "use `yield` for the poll value",
                            span,
                        ));
                    }
                    Ok(Flow::Break) => break,
                    Err(e) => {
                        frame.scopes.pop();
                        return Err(e);
                    }
                }
            }
            frame.scopes.pop();
            if done {
                return Ok(yielded.unwrap_or(Value::Null));
            }
            if start.elapsed() + every > limit {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.poll",
                    format!("poll did not complete within {} ms", limit.as_millis()),
                )
                .with_span(Some(span.clone())));
            }
            tokio::time::sleep(every).await;
        }
    }

    async fn run_concurrent(
        &self,
        frame: &mut Frame,
        options: &GroupOptions,
        tasks: &[(String, Vec<Stmt>)],
        span: &SourceSpan,
    ) -> RivetResult<Flow> {
        let limit = options.limit.unwrap_or(tasks.len() as i64).max(1) as usize;
        let group = async {
            let mut pending = tasks.iter();
            let mut running = FuturesUnordered::new();
            let mut first_error: Option<RivetError> = None;
            loop {
                while running.len() < limit {
                    let Some((name, body)) = pending.next() else {
                        break;
                    };
                    let mut child = frame.clone();
                    running
                        .push(async move { (name.clone(), self.scoped(&mut child, body).await) });
                }
                let Some((_name, r)) = running.next().await else {
                    break;
                };
                if let Err(e) = r {
                    if options.failure == FailurePolicy::Fast {
                        return Err(e);
                    }
                    first_error.get_or_insert(e);
                }
            }
            match first_error {
                Some(e) => Err(e),
                None => Ok(Flow::Normal),
            }
        };
        match options.timeout_ms {
            Some(ms) => tokio::time::timeout(Duration::from_millis(ms), group)
                .await
                .unwrap_or_else(|_| {
                    Err(RivetError::new(
                        ErrorKind::Timeout,
                        "timeout.concurrent",
                        format!("concurrent group exceeded {ms} ms"),
                    )
                    .with_span(Some(span.clone())))
                }),
            None => group.await,
        }
    }

    async fn run_dag(
        &self,
        frame: &mut Frame,
        options: &GroupOptions,
        nodes: &[DagNode],
        span: &SourceSpan,
    ) -> RivetResult<()> {
        #[derive(Clone, Copy, PartialEq, Debug)]
        enum St {
            Pending,
            Running,
            Succeeded,
            Failed,
            Cancelled,
            Blocked,
            Skipped,
        }
        let name = |s: St| match s {
            St::Pending => "pending",
            St::Running => "running",
            St::Succeeded => "succeeded",
            St::Failed => "failed",
            St::Cancelled => "cancelled",
            St::Blocked => "blocked",
            St::Skipped => "skipped",
        };
        let limit = options.limit.unwrap_or(4).max(1) as usize;
        let mut state = vec![St::Pending; nodes.len()];
        let mut values: Vec<Value> = vec![Value::Null; nodes.len()];
        let mut errors: Vec<Option<RivetError>> = vec![None; nodes.len()];
        let mut fatal: Option<RivetError> = None;
        let deadline = options
            .timeout_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms));
        {
            let mut running = FuturesUnordered::new();
            loop {
                // Block descendants of failed/blocked nodes.
                let mut changed = true;
                while changed {
                    changed = false;
                    for i in 0..nodes.len() {
                        if state[i] == St::Pending
                            && nodes[i].after.iter().any(|d| {
                                nodes.iter().position(|n| &n.name == d).is_some_and(|j| {
                                    matches!(
                                        state[j],
                                        St::Failed | St::Blocked | St::Cancelled | St::Skipped
                                    )
                                })
                            })
                        {
                            state[i] = St::Blocked;
                            changed = true;
                        }
                    }
                }
                if fatal.is_none() {
                    for i in 0..nodes.len() {
                        if running.len() >= limit {
                            break;
                        }
                        let ready = state[i] == St::Pending
                            && nodes[i].after.iter().all(|d| {
                                nodes
                                    .iter()
                                    .position(|n| &n.name == d)
                                    .is_some_and(|j| state[j] == St::Succeeded)
                            });
                        if ready {
                            state[i] = St::Running;
                            let mut child = frame.clone();
                            for (j, n) in nodes.iter().enumerate() {
                                if state[j] == St::Succeeded {
                                    child.define(
                                        &n.name,
                                        Value::object([
                                            ("status", Value::text("succeeded")),
                                            ("result", values[j].clone()),
                                            ("error", Value::Null),
                                        ]),
                                    );
                                }
                            }
                            let expr = &nodes[i].expr;
                            running.push(async move {
                                let r = self.eval(&mut child, expr).await;
                                (i, r)
                            });
                        }
                    }
                }
                if running.is_empty() {
                    break;
                }
                let next = match deadline {
                    Some(d) => match tokio::time::timeout(
                        d.saturating_duration_since(Instant::now()),
                        running.next(),
                    )
                    .await
                    {
                        Ok(n) => n,
                        Err(_) => {
                            fatal = Some(
                                RivetError::new(
                                    ErrorKind::Timeout,
                                    "timeout.dag",
                                    "dag exceeded its timeout",
                                )
                                .with_span(Some(span.clone())),
                            );
                            break;
                        }
                    },
                    None => running.next().await,
                };
                let Some((i, r)) = next else { break };
                match r {
                    Ok(v) => {
                        state[i] = St::Succeeded;
                        values[i] = v;
                    }
                    Err(mut e) => {
                        state[i] = St::Failed;
                        e.node_id = Some(nodes[i].name.clone());
                        if options.failure == FailurePolicy::Fast && fatal.is_none() {
                            fatal = Some(e.clone());
                        }
                        errors[i] = Some(e);
                    }
                }
            }
            // Dropping `running` cancels still-running nodes (fail fast / timeout).
        }
        for s in state.iter_mut() {
            match *s {
                St::Running => *s = St::Cancelled,
                St::Pending => {
                    *s = if fatal.is_some() {
                        St::Skipped
                    } else {
                        St::Blocked
                    }
                }
                _ => {}
            }
        }
        for (i, n) in nodes.iter().enumerate() {
            let result = if state[i] == St::Succeeded {
                values[i].clone()
            } else {
                Value::Null
            };
            let error = errors[i]
                .as_ref()
                .map(|e| e.to_value())
                .unwrap_or(Value::Null);
            frame.define(
                &n.name,
                Value::object([
                    ("status", Value::text(name(state[i]))),
                    ("result", result),
                    ("error", error),
                ]),
            );
        }
        match fatal {
            Some(mut e) => {
                e.details = match e.details {
                    Value::Null => Value::object([(
                        "nodes",
                        Value::List(
                            nodes
                                .iter()
                                .enumerate()
                                .map(|(i, n)| {
                                    Value::object([
                                        ("id", Value::text(&n.name)),
                                        ("status", Value::text(name(state[i]))),
                                    ])
                                })
                                .collect(),
                        ),
                    )]),
                    d => d,
                };
                Err(e)
            }
            None => Ok(()),
        }
    }

    fn eval<'b>(&'b self, frame: &'b mut Frame, e: &'b Expr) -> BoxFuture<'b, RivetResult<Value>> {
        Box::pin(async move {
            match e {
                Expr::Lit(v) => Ok(v.clone()),
                Expr::Template(parts) => {
                    let mut s = String::new();
                    for p in parts {
                        match p {
                            TemplatePart::Lit(l) => s.push_str(l),
                            TemplatePart::Path(path) => {
                                s.push_str(&lookup(frame, path, None)?.to_display())
                            }
                        }
                    }
                    Ok(Value::Text(s))
                }
                Expr::Path(path, span) => {
                    if frame.get(&path[0]).is_none() {
                        if let Some(handle) = frame.handle(&path[0]) {
                            if path.len() < 2 {
                                return Err(runtime_err(
                                    "value.handle",
                                    format!(
                                        "`{}` is an open resource, not a value; read a property such as `{}.result`",
                                        path[0], path[0]
                                    ),
                                    span,
                                ));
                            }
                            let ctx = self.ctx(frame, span);
                            let mut v = if let Some(s) = shared_of(&handle).await {
                                s.property(&ctx, &path[1]).await
                            } else {
                                let mut guard = handle.lock().await;
                                let h = guard.as_mut().ok_or_else(|| {
                                    RivetError::new(
                                        ErrorKind::Cleanup,
                                        "cleanup.closed",
                                        format!("`{}` is already closed", path[0]),
                                    )
                                })?;
                                h.property(&ctx, &path[1]).await
                            }
                            .map_err(|e| e.with_span(Some(span.clone())))?;
                            for seg in &path[2..] {
                                v = v.get(seg).cloned().ok_or_else(|| {
                                    runtime_err(
                                        "value.missing_key",
                                        format!("no key `{seg}`"),
                                        span,
                                    )
                                })?;
                            }
                            return Ok(v);
                        }
                    }
                    lookup(frame, path, Some(span))
                }
                Expr::List(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for i in items {
                        out.push(self.eval(frame, i).await?);
                    }
                    Ok(Value::List(out))
                }
                Expr::Object(pairs) => {
                    let mut out = Vec::with_capacity(pairs.len());
                    for (k, v) in pairs {
                        out.push((k.clone(), self.eval(frame, v).await?));
                    }
                    Ok(Value::Object(out))
                }
                Expr::Not(inner) => Ok(Value::Bool(!self.eval(frame, inner).await?.truthy())),
                Expr::Neg(inner) => match self.eval(frame, inner).await? {
                    Value::Int(i) => i.checked_neg().map(Value::Int).ok_or_else(|| {
                        RivetError::validation("value.overflow", "integer overflow")
                    }),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    other => Err(RivetError::validation(
                        "value.type",
                        format!("cannot negate {}", other.type_name()),
                    )),
                },
                Expr::Binary { op, lhs, rhs } => {
                    let l = self.eval(frame, lhs).await?;
                    match op {
                        BinOp::And if !l.truthy() => return Ok(Value::Bool(false)),
                        BinOp::Or if l.truthy() => return Ok(l),
                        _ => {}
                    }
                    let r = self.eval(frame, rhs).await?;
                    binary(*op, l, r)
                }
                Expr::Call { func, args, span } => self.call(frame, func, args, span).await,
            }
        })
    }

    async fn call(
        &self,
        frame: &mut Frame,
        func: &str,
        args: &[Expr],
        span: &SourceSpan,
    ) -> RivetResult<Value> {
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.eval(frame, a).await?);
        }
        let arity = |n: usize| -> RivetResult<()> {
            if vals.len() != n {
                return Err(runtime_err(
                    "call.arity",
                    format!("({func} …) takes {n} argument(s), got {}", vals.len()),
                    span,
                ));
            }
            Ok(())
        };
        match func {
            "request" => {
                if vals.is_empty() || vals.len() > 2 {
                    return Err(runtime_err(
                        "call.arity",
                        "(request ID PARAMS) takes an ID and optional params",
                        span,
                    ));
                }
                let id = vals[0]
                    .as_str()
                    .ok_or_else(|| {
                        runtime_err(
                            "call.request",
                            "(request …) needs an operation ID string",
                            span,
                        )
                    })?
                    .to_string();
                let params = vals.get(1).cloned().unwrap_or(Value::Object(vec![]));
                let dispatcher = self
                    .interp
                    .dispatcher
                    .get()
                    .ok_or_else(|| RivetError::internal("nested dispatcher not configured"))?;
                let child = Request {
                    request_id: format!(
                        "{}.{}",
                        frame.request.request_id,
                        frame.run.data_seq.load(Ordering::SeqCst) + 1 + rand_suffix()
                    ),
                    trace_id: frame.request.trace_id.clone(),
                    operation_id: id,
                    params,
                    principal: frame.request.principal.clone(),
                    parent_request_id: Some(frame.request.request_id.clone()),
                    depth: frame.request.depth + 1,
                    deadline_ms: frame
                        .run
                        .deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .max(1) as u64,
                    include_private: true,
                };
                let completion = dispatcher
                    .dispatch(child, None)
                    .await
                    .map_err(|e| e.with_span(Some(span.clone())))?;
                if completion.effects != EffectsStatus::None {
                    frame.run.commit();
                }
                Ok(completion.result)
            }
            "length" => {
                arity(1)?;
                Ok(Value::Int(match &vals[0] {
                    Value::List(l) => l.len(),
                    Value::Text(s) => s.chars().count(),
                    Value::Object(o) => o.len(),
                    Value::Bytes(b) => b.len(),
                    other => {
                        return Err(runtime_err(
                            "call.length",
                            format!("(length …) of {}", other.type_name()),
                            span,
                        ));
                    }
                } as i64))
            }
            "base64.encode" => {
                arity(1)?;
                let bytes = match &vals[0] {
                    Value::Bytes(b) => b.clone(),
                    Value::Text(s) => s.as_bytes().to_vec(),
                    other => {
                        return Err(runtime_err(
                            "call.base64",
                            format!("(base64.encode …) of {}", other.type_name()),
                            span,
                        ));
                    }
                };
                Ok(Value::Text(
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                ))
            }
            "base64.decode" => {
                arity(1)?;
                let text = vals[0].as_str().ok_or_else(|| {
                    runtime_err("call.base64", "(base64.decode …) needs text", span)
                })?;
                base64::engine::general_purpose::STANDARD
                    .decode(text)
                    .map(Value::Bytes)
                    .map_err(|e| runtime_err("call.base64", format!("invalid base64: {e}"), span))
            }
            "text" => {
                arity(1)?;
                Ok(Value::Text(vals[0].to_display()))
            }
            "keys" => {
                arity(1)?;
                match &vals[0] {
                    Value::Object(o) => {
                        Ok(Value::List(o.iter().map(|(k, _)| Value::text(k)).collect()))
                    }
                    other => Err(runtime_err(
                        "call.keys",
                        format!("(keys …) of {}", other.type_name()),
                        span,
                    )),
                }
            }
            other => Err(runtime_err(
                "call.unknown",
                format!("unknown function `{other}`"),
                span,
            )),
        }
    }
}

fn rand_suffix() -> u64 {
    use std::sync::atomic::AtomicU64 as A;
    static N: A = A::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn catches(filter: &CatchFilter, e: &RivetError) -> bool {
    if e.kind == ErrorKind::Cancelled {
        return false;
    }
    match (&filter.kind, &filter.code) {
        (Some(k), _) => e.kind.as_str() == k || e.code.starts_with(&format!("{k}.")),
        (None, Some(c)) => &e.code == c,
        (None, None) => true,
    }
}

fn mutates(form: &EffectForm) -> bool {
    match form.kind {
        EffectKind::File => form
            .head
            .first()
            .and_then(Arg::word)
            .is_some_and(|v| !matches!(v, "read" | "list" | "stat")),
        EffectKind::Http => form
            .head
            .first()
            .and_then(Arg::word)
            .is_some_and(|m| !matches!(m, "get" | "head" | "options")),
        _ => true,
    }
}

fn lookup(frame: &Frame, path: &[String], span: Option<&SourceSpan>) -> RivetResult<Value> {
    let err = |msg: String| {
        let mut e = RivetError::validation("value.missing_key", msg);
        e.source = span.cloned();
        e
    };
    let first = &path[0];
    let mut cur = frame
        .get(first)
        .cloned()
        .ok_or_else(|| err(format!("`{first}` is not defined")))?;
    for (i, seg) in path.iter().enumerate().skip(1) {
        cur = match &cur {
            Value::Object(_) => cur
                .get(seg)
                .cloned()
                .ok_or_else(|| err(format!("`{}` has no key `{seg}`", path[..i].join("."))))?,
            Value::List(items) => match seg.parse::<usize>() {
                Ok(n) => items.get(n).cloned().ok_or_else(|| {
                    err(format!(
                        "index {n} is out of range for `{}`",
                        path[..i].join(".")
                    ))
                })?,
                Err(_) if seg == "length" => Value::Int(items.len() as i64),
                Err(_) => {
                    return Err(err(format!(
                        "`{}` is a list; `.{seg}` is not a key",
                        path[..i].join(".")
                    )));
                }
            },
            Value::Null => {
                return Err(err(format!(
                    "`{}` is null; cannot read `.{seg}`",
                    path[..i].join(".")
                )));
            }
            other => {
                return Err(err(format!(
                    "`{}` is {}; cannot read `.{seg}`",
                    path[..i].join("."),
                    other.type_name()
                )));
            }
        };
    }
    Ok(cur)
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn arith(op: BinOp, l: Value, r: Value, span: &SourceSpan) -> RivetResult<Value> {
    binary(op, l, r).map_err(|e| e.with_span(Some(span.clone())))
}

fn binary(op: BinOp, l: Value, r: Value) -> RivetResult<Value> {
    use BinOp::*;
    let overflow = || RivetError::validation("value.overflow", "integer overflow");
    Ok(match (op, l, r) {
        (Eq, a, b) => Value::Bool(values_equal(&a, &b)),
        (Ne, a, b) => Value::Bool(!values_equal(&a, &b)),
        (And, a, b) => Value::Bool(a.truthy() && b.truthy()),
        (Or, a, b) => {
            if a.truthy() {
                a
            } else {
                b
            }
        }
        (Add, Value::Text(a), b) => Value::Text(a + &b.to_display()),
        (Add, Value::List(mut a), Value::List(b)) => {
            a.extend(b);
            Value::List(a)
        }
        (Add, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_add(b).ok_or_else(overflow)?),
        (Sub, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_sub(b).ok_or_else(overflow)?),
        (Mul, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_mul(b).ok_or_else(overflow)?),
        (Div, Value::Int(_), Value::Int(0)) | (Rem, Value::Int(_), Value::Int(0)) => {
            return Err(RivetError::validation(
                "value.division_by_zero",
                "division by zero",
            ));
        }
        (Div, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_div(b).ok_or_else(overflow)?),
        (Rem, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_rem(b).ok_or_else(overflow)?),
        (op @ (Lt | Le | Gt | Ge), Value::Text(a), Value::Text(b)) => Value::Bool(match op {
            Lt => a < b,
            Le => a <= b,
            Gt => a > b,
            _ => a >= b,
        }),
        (op, a, b) => match (num(&a), num(&b)) {
            (Some(x), Some(y)) => match op {
                Add => Value::Float(x + y),
                Sub => Value::Float(x - y),
                Mul => Value::Float(x * y),
                Div if y == 0.0 => {
                    return Err(RivetError::validation(
                        "value.division_by_zero",
                        "division by zero",
                    ));
                }
                Div => Value::Float(x / y),
                Rem => Value::Float(x % y),
                Lt => Value::Bool(x < y),
                Le => Value::Bool(x <= y),
                Gt => Value::Bool(x > y),
                Ge => Value::Bool(x >= y),
                _ => unreachable!(),
            },
            _ => {
                return Err(RivetError::validation(
                    "value.type",
                    format!(
                        "operator {op:?} does not apply to {} and {}",
                        a.type_name(),
                        b.type_name()
                    ),
                ));
            }
        },
    })
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// DataSink that forwards items into a bounded channel.
struct ChannelSink {
    tx: tokio::sync::mpsc::Sender<Value>,
}

#[async_trait]
impl DataSink for ChannelSink {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        self.tx.send(event.data).await.map_err(|_| {
            RivetError::new(
                ErrorKind::Cancelled,
                "cancelled.consumer",
                "the stream consumer closed",
            )
        })
    }
}

struct RequestStreamHandle {
    rx: tokio::sync::mpsc::Receiver<Value>,
    task: Option<tokio::task::JoinHandle<RivetResult<crate::domain::contracts::Completion>>>,
    completion: Option<crate::domain::contracts::Completion>,
}

impl RequestStreamHandle {
    async fn finish(&mut self) -> RivetResult<()> {
        if let Some(task) = self.task.take() {
            match task.await {
                Ok(Ok(c)) => self.completion = Some(c),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(RivetError::internal(format!("stream task failed: {e}"))),
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ResourceHandle for RequestStreamHandle {
    async fn next(&mut self, _ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        match self.rx.recv().await {
            Some(v) => Ok(Some(v)),
            None => {
                self.finish().await?;
                Ok(None)
            }
        }
    }

    async fn property(&mut self, _ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        match name {
            "result" | "completion" => {
                while self.rx.recv().await.is_some() {}
                self.finish().await?;
                Ok(match &self.completion {
                    Some(c) if name == "result" => c.result.clone(),
                    Some(c) => Value::from_json(&c.to_json()),
                    None => Value::Null,
                })
            }
            other => Err(RivetError::unsupported(
                "unsupported.property",
                format!("a request stream has no `{other}` property (use result or completion)"),
            )),
        }
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        Ok(())
    }
}
