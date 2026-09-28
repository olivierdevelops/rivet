//! Tree-walking interpreter for compiled operations (satisfies ExecutionDriver).
//!
//! Every effect goes through a port (FileAccess, PolicyEvaluator, protocol
//! adapters); nested `(request …)` calls re-enter the shared dispatcher.
//!
//! Cancellation is structured (PROP-2026-0001 Increments 2–3):
//!
//! ```text
//!  request token ─┬─ deadline (+250 ms backstop) fires it with `timeout`
//!                 ├─ caller cancel / session cancel / shutdown fires it with `cancelled`
//!                 └─ child tokens: nested requests, request.stream, concurrent/map/DAG groups, scopes
//!  every statement, effect, handle operation, sleep and nested call observes the token
//!        ─▶ the error unwinds through `with` scopes, which close handles (reverse order)
//!           within the cleanup grace ─▶ one terminal `cancelled`/`timeout` error with
//!           cleanup failures suppressed on it
//!  only if the body is still running when the grace ends is its future dropped (last resort)
//! ```

use crate::domain::cancel::{CancelReason, CancelToken};
use crate::domain::contracts::{DataEvent, ExecutionPlan, Request, RunOutcome, span_id};
use crate::domain::errors::{EffectsStatus, ErrorKind};
use crate::domain::files::{Codec, FileOperation, FileVerb};
use crate::domain::ir::{
    Arg, BinOp, CatchFilter, CompiledProgram, DagNode, EffectForm, EffectKind, Expr, FailurePolicy,
    GroupOptions, MemberCall, Operation, Rhs, Stmt, TemplatePart,
};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::ports::{DataSink, Dispatcher, ExecutionDriver, FileAccess, PolicyEvaluator};
use crate::domain::source::SourceSpan;
use crate::domain::transports::{UrlPiece, assemble_url, xml_element};
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use base64::Engine;
use futures_util::future::BoxFuture;
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
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

    /// `for item in NAME.MEMBER` (`for line in process.stdout`); `Ok(None)` ends it.
    async fn next_of(&mut self, ctx: &EffectCtx, member: &str) -> RivetResult<Option<Value>> {
        let _ = ctx;
        Err(RivetError::unsupported(
            "unsupported.iterate",
            format!("`{member}` cannot be iterated on this resource"),
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

/// Margin after the request deadline before the interpreter's own timeout fires.
pub const DEADLINE_BACKSTOP: Duration = Duration::from_millis(250);

type SharedHandle = Arc<tokio::sync::Mutex<Option<Box<dyn ResourceHandle>>>>;

/// Head arguments and options with every expression evaluated.
#[derive(Clone, Debug, Default)]
pub struct EvaluatedForm {
    pub head: Vec<EvalArg>,
    pub options: Vec<(String, Vec<EvalArg>)>,
    /// Evaluated child lines of each option, index-aligned with `options`
    /// (`body multipart … end` parts: `field NAME VALUE`, `file NAME PATH [TYPE]`).
    pub children: Vec<Vec<(String, Vec<EvalArg>)>>,
}

/// Codec and format words that select how a form reads or writes data.
const FORM_KEYWORDS: [&str; 11] = [
    "json",
    "text",
    "bytes",
    "lines",
    "jsonl",
    "sse",
    "xml",
    "raw",
    "newline",
    "form",
    "multipart",
];

/// Whether `args[i]` is a form keyword in keyword position: the first word of
/// an option or call, the word after `as`, or a codec that a value follows
/// (`text VALUE`). The slot right after a codec is always a value.
fn keyword_slot(args: &[Arg], i: usize) -> bool {
    let word = |k: usize| match args.get(k) {
        Some(Arg::Word(w, _)) => Some(w.as_str()),
        _ => None,
    };
    let Some(w) = word(i) else { return false };
    if !FORM_KEYWORDS.contains(&w) {
        return false;
    }
    let prev = i.checked_sub(1).and_then(word);
    let is_codec = |x: &str| Codec::parse(x).is_some();
    if prev.is_some_and(is_codec) {
        return false;
    }
    i == 0 || prev == Some("as") || (is_codec(w) && i + 1 < args.len())
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
    /// The calling scope's cancellation; adapters may observe it to stop early
    /// (the interpreter already abandons a pending effect when it fires).
    pub cancel: CancelToken,
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
    /// The `execution.run_dag` use case, injected by the orchestrator.
    dag: OnceLock<Arc<dyn crate::domain::dag::DagExecutor>>,
    adapters: HashMap<String, Arc<dyn EffectAdapter>>,
    /// Live input feeds for runs of operations that declare `receives`, keyed
    /// by request ID; `drive` takes the feed and exposes it as `incoming`.
    inputs: std::sync::Mutex<HashMap<String, tokio::sync::mpsc::Receiver<Value>>>,
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
            dag: OnceLock::new(),
            adapters: HashMap::new(),
            inputs: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Attach the live input feed for one request (sessions, WebSocket refs).
    /// The run of an operation that declares `receives` iterates it as `incoming`;
    /// the feed ends when every sender is dropped (finish_input).
    pub fn attach_input(&self, request_id: &str, feed: tokio::sync::mpsc::Receiver<Value>) {
        if let Ok(mut m) = self.inputs.lock() {
            m.insert(request_id.to_string(), feed);
        }
    }

    /// Drop a feed that was never taken (the request failed before driving).
    pub fn detach_input(&self, request_id: &str) {
        if let Ok(mut m) = self.inputs.lock() {
            m.remove(request_id);
        }
    }

    /// Nested `(request …)` target; set once by the orchestrator after the dispatcher exists.
    pub fn set_dispatcher(&self, d: Arc<dyn Dispatcher>) {
        let _ = self.dispatcher.set(d);
    }

    /// DAG scheduling is the `execution.run_dag` use case; the orchestrator sets it once.
    pub fn set_dag_executor(&self, d: Arc<dyn crate::domain::dag::DagExecutor>) {
        let _ = self.dag.set(d);
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
        let token = plan.request.cancel.clone();
        let run = Arc::new(RunState {
            data_seq: AtomicU64::new(0),
            effects: AtomicU8::new(0),
            deadline,
            sink,
            grace_until: Mutex::new(None),
        });
        let mut frame = Frame::new(&plan.request, Arc::clone(&run));
        let feed = self
            .inputs
            .lock()
            .ok()
            .and_then(|mut m| m.remove(&plan.request.request_id));
        if op.receives.is_some() {
            let Some(rx) = feed else {
                return Err(RivetError::validation(
                    "stream.input_required",
                    format!(
                        "`{}` receives live input; open a session (sessions.open, polling, WebSocket) to feed `incoming`",
                        op.id
                    ),
                ));
            };
            let handle: Box<dyn ResourceHandle> = Box::new(IncomingHandle {
                rx,
                spec: op.receives.clone(),
                seq: 0,
            });
            frame.handles.push((
                "incoming".to_string(),
                Arc::new(tokio::sync::Mutex::new(Some(handle))),
            ));
        }
        if let Value::Object(pairs) = &plan.params {
            for (k, v) in pairs {
                frame.define(k, v.clone());
            }
        }
        let machine = Machine {
            interp: self,
            op: &op,
        };
        let outcome = {
            let body = machine.exec_block(&mut frame, &op.body);
            tokio::pin!(body);
            // Adapters honour `deadline` themselves and report typed errors (for example
            // auth.refresh_uncertain); the request deadline fires the token a short
            // margin later so it never pre-empts a more specific error.
            let expiry = tokio::time::sleep_until((deadline + DEADLINE_BACKSTOP).into());
            tokio::pin!(expiry);
            let mut grace: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;
            loop {
                tokio::select! {
                    r = &mut body => break r,
                    _ = &mut expiry, if !token.is_cancelled() => {
                        token.cancel(CancelReason::Timeout);
                    }
                    _ = token.cancelled(), if grace.is_none() => {
                        // Cleanup starts now: the body unwinds and closes its handles
                        // within the grace; after it the body is dropped (last resort).
                        let until = Instant::now() + CLEANUP_GRACE;
                        if let Ok(mut g) = run.grace_until.lock() {
                            g.get_or_insert(until);
                        }
                        grace = Some(Box::pin(tokio::time::sleep_until(
                            (until + DEADLINE_BACKSTOP).into(),
                        )));
                    }
                    _ = async { if let Some(g) = grace.as_mut() { g.await } }, if grace.is_some() => {
                        let reason = token.reason().unwrap_or(CancelReason::Cancelled);
                        let mut e = cancel_error(&op.id, deadline_ms, reason, None);
                        e.suppressed.push(RivetError::new(
                            ErrorKind::Cleanup,
                            "cleanup.timeout",
                            format!(
                                "`{}` did not finish its cleanup within {} s; its remaining work was dropped",
                                op.id,
                                CLEANUP_GRACE.as_secs()
                            ),
                        ));
                        break Err(e);
                    }
                }
            }
        };
        let flow = outcome.map_err(|e| e.with_effects(run.effects()))?;
        let result = match flow {
            Flow::Return(v) => {
                secret_output_guard(&frame, &v, "returned")?;
                v
            }
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
    /// 0 none, 1 committed, 2 partial, 3 unknown — only ever raised (fetch_max):
    /// an opaque remote call (MCP tool, nested request reporting unknown) makes the
    /// whole request `unknown`, never `committed` (G22).
    effects: AtomicU8,
    deadline: Instant,
    sink: Option<Arc<dyn DataSink>>,
    /// End of the cleanup grace once the request token fired.
    grace_until: Mutex<Option<Instant>>,
}

impl RunState {
    /// Bound for one handle close: the rest of the grace after cancellation,
    /// the full grace otherwise.
    fn close_budget(&self) -> Duration {
        match self.grace_until.lock().ok().and_then(|g| *g) {
            Some(until) => until
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(50)),
            None => CLEANUP_GRACE,
        }
    }

    fn effects(&self) -> EffectsStatus {
        match self.effects.load(Ordering::SeqCst) {
            0 => EffectsStatus::None,
            1 => EffectsStatus::Committed,
            2 => EffectsStatus::Partial,
            _ => EffectsStatus::Unknown,
        }
    }

    fn commit(&self) {
        self.mark(EffectsStatus::Committed);
    }

    /// Fold a nested status in: unknown > partial > committed > none.
    fn mark(&self, status: EffectsStatus) {
        let rank = match status {
            EffectsStatus::None => 0,
            EffectsStatus::Committed => 1,
            EffectsStatus::Partial => 2,
            EffectsStatus::Unknown => 3,
        };
        self.effects.fetch_max(rank, Ordering::SeqCst);
    }
}

/// `scheme://host:port` of a URL (default port filled in), for secret bindings.
fn origin_key(u: &str) -> Option<(String, String, u16)> {
    let u = url::Url::parse(u).ok()?;
    Some((
        u.scheme().to_ascii_lowercase(),
        u.host_str()?.to_ascii_lowercase(),
        u.port_or_known_default()?,
    ))
}

// ------------------------------------------------------------ secret taint (G23b)
//
//  secret NAME from env "E" for ORIGIN…  ─▶ taint forms = [value]
//        │ explicit flows keep the value recognisable:
//        │   assignment, interpolation, string/list/object construction → the value (or a
//        │   derived form) appears as a substring of the new value;
//        │   encoding/transform helpers (base64, hex, json, text, …) → their OUTPUT is
//        │   recorded as a new derived form when any input carries a form
//        ▼
//  sinks: every effect form (head, options, child parts), `with` opens, handle member calls
//  (socket/stream sends), nested `(request …)` params (MCP/gRPC/local calls), return, emit
//        allowed only when the sink is a NETWORK destination whose scheme://host:port is one
//        of the secret's bound origins; files, processes, unix sockets, pipes and nested
//        requests never; return/emit always an error.
//
// Implicit flows (`if secret == x`, branching or loop counts on a secret, timing, lengths)
// are OUT OF SCOPE (proposal Increment 5: best-effort explicit-flow tracking). Derived forms
// shorter than MIN_DERIVED bytes are not recorded (they would taint unrelated values).

/// Shortest derived (encoded/transformed) form that is tracked.
const MIN_DERIVED: usize = 4;
/// Upper bound of tracked forms per secret (the original plus derived ones).
const MAX_FORMS: usize = 64;

/// `scheme://host:port` destination of a sink.
type Origin = (String, String, u16);

/// One `secret` declaration: every recognisable form of its value and its bound origins.
#[derive(Clone, Debug)]
struct SecretTaint {
    forms: Vec<Vec<u8>>,
    origins: Vec<String>,
}

fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && hay.len() >= needle.len()
        && hay.windows(needle.len()).any(|w| w == needle)
}

fn value_carries(v: &Value, form: &[u8]) -> bool {
    match v {
        Value::Text(t) => contains_bytes(t.as_bytes(), form),
        Value::Bytes(b) => contains_bytes(b, form),
        Value::List(items) => items.iter().any(|i| value_carries(i, form)),
        Value::Object(pairs) => pairs
            .iter()
            .any(|(k, v)| contains_bytes(k.as_bytes(), form) || value_carries(v, form)),
        _ => false,
    }
}

fn args_carry(args: &[EvalArg], form: &[u8]) -> bool {
    args.iter().any(|a| match a {
        EvalArg::Value(v) => value_carries(v, form),
        EvalArg::Word(_) => false,
    })
}

impl Frame {
    /// Names (sorted) of the secrets `pred` finds in some form.
    fn tainting(&self, pred: impl Fn(&[u8]) -> bool) -> Option<String> {
        let mut names: Vec<&String> = self.secrets.keys().collect();
        names.sort();
        names
            .into_iter()
            .find(|n| self.secrets[*n].forms.iter().any(|f| pred(f)))
            .cloned()
    }

    fn taint_of_value(&self, v: &Value) -> Option<String> {
        if self.secrets.is_empty() {
            return None;
        }
        self.tainting(|f| value_carries(v, f))
    }

    fn taint_of_args(&self, args: &[EvalArg]) -> Option<String> {
        if self.secrets.is_empty() {
            return None;
        }
        self.tainting(|f| args_carry(args, f))
    }

    fn taint_of_form(&self, f: &EvaluatedForm) -> Option<String> {
        if self.secrets.is_empty() {
            return None;
        }
        self.tainting(|form| {
            args_carry(&f.head, form)
                || f.options.iter().any(|(_, a)| args_carry(a, form))
                || f.children
                    .iter()
                    .flatten()
                    .any(|(_, a)| args_carry(a, form))
        })
    }

    /// An encoding/transform helper consumed a tainted input: its output is a new form.
    fn derive_taint(&mut self, inputs: &[String], out: &Value) {
        let bytes = match out {
            Value::Text(t) => t.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => return,
        };
        if bytes.len() < MIN_DERIVED {
            return;
        }
        for name in inputs {
            if let Some(t) = self.secrets.get_mut(name)
                && t.forms.len() < MAX_FORMS
                && !t.forms.iter().any(|f| contains_bytes(&bytes, f))
            {
                t.forms.push(bytes.clone());
            }
        }
    }
}

/// `scheme://host:port` of a URL (default port filled in), for secret bindings.
fn origin_of(u: &str) -> Option<Origin> {
    origin_key(u)
}

/// The network destination of an effect form, if it has one: the first text in
/// its head as a URL, or `KIND://HOST:PORT` for bare `HOST:PORT` transports.
/// Files, processes, unix sockets and pipes have none (a secret never reaches them).
fn dest_of(kind: &EffectKind, head: &[EvalArg]) -> Option<Origin> {
    let scheme = match kind {
        EffectKind::File | EffectKind::Command | EffectKind::Unix | EffectKind::Pipe => {
            return None;
        }
        EffectKind::Tcp => "tcp",
        EffectKind::Udp => "udp",
        EffectKind::Quic => "quic",
        EffectKind::WebSocket => "ws",
        EffectKind::Http => "https",
        EffectKind::Grpc => "https",
        EffectKind::RequestStream | EffectKind::Connection => return None,
        EffectKind::Other(k) => k.as_str(),
    };
    let t = head.iter().find_map(|a| match a {
        EvalArg::Value(Value::Text(t)) => Some(t.as_str()),
        _ => None,
    })?;
    if t.contains("://") {
        origin_of(t)
    } else {
        origin_of(&format!("{scheme}://{t}"))
    }
}

/// G23b sink rule: a tainted value may reach only a network destination bound by
/// `secret … for ORIGIN`; anything else (no destination, other origin) is denied.
fn sink_guard(
    frame: &Frame,
    tainted: Option<String>,
    dest: Option<&Origin>,
    sink: &str,
    span: &SourceSpan,
) -> RivetResult<()> {
    let Some(name) = tainted else {
        return Ok(());
    };
    let origins = &frame.secrets[&name].origins;
    if let Some(d) = dest
        && origins.iter().any(|o| origin_of(o).as_ref() == Some(d))
    {
        return Ok(());
    }
    let (message, target) = match dest {
        Some((scheme, host, port)) => (
            format!(
                "secret `{name}` is bound to {}; it may not be sent to {scheme}://{host}:{port}",
                origins.join(", ")
            ),
            format!("{scheme}://{host}:{port}"),
        ),
        None => (
            format!(
                "secret `{name}` may not reach {sink}; secrets travel only to their bound network origins ({})",
                origins.join(", ")
            ),
            sink.to_string(),
        ),
    };
    Err(RivetError::permission(message)
        .with_span(Some(span.clone()))
        .with_details(Value::object([
            ("secret", Value::text(&name)),
            ("origin", Value::text(target)),
        ])))
}

/// Returning or emitting a secret value is an error (explicit flows only).
fn secret_output_guard(frame: &Frame, v: &Value, how: &str) -> RivetResult<()> {
    if let Some(name) = frame.taint_of_value(v) {
        return Err(RivetError::permission(format!(
            "secret `{name}` cannot be {how}; secrets may only reach their bound origins"
        ))
        .with_details(Value::object([("secret", Value::text(name))])));
    }
    Ok(())
}

#[derive(Clone)]
struct Frame {
    scopes: Vec<HashMap<String, Value>>,
    request: Request,
    run: Arc<RunState>,
    /// `secret NAME … for ORIGIN…`: name → recognisable forms + bound origins. The
    /// values are kept only to recognise explicit flows (S140, G23b); never rendered.
    secrets: HashMap<String, SecretTaint>,
    /// Open resource handles, innermost last.
    handles: Vec<(String, SharedHandle)>,
    /// Cancellation of the innermost enclosing scope (request, group or node).
    cancel: CancelToken,
    /// DAG node whose expression this frame evaluates (trace attribution).
    node_id: Option<String>,
    /// Network destination of each open handle (secret sink rule for member calls).
    dests: Vec<(SharedHandle, Option<Origin>)>,
}

impl Frame {
    fn new(request: &Request, run: Arc<RunState>) -> Frame {
        Frame {
            scopes: vec![HashMap::new()],
            request: request.clone(),
            run,
            secrets: HashMap::new(),
            handles: Vec::new(),
            cancel: request.cancel.clone(),
            node_id: None,
            dests: Vec::new(),
        }
    }

    fn dest_of_handle(&self, h: &SharedHandle) -> Option<Origin> {
        self.dests
            .iter()
            .rev()
            .find(|(x, _)| Arc::ptr_eq(x, h))
            .and_then(|(_, d)| d.clone())
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

/// The terminal error of a cancelled scope: `cancelled.request` or `timeout.request`.
fn cancel_error(
    op_id: &str,
    deadline_ms: u64,
    reason: CancelReason,
    span: Option<&SourceSpan>,
) -> RivetError {
    let e = match reason {
        CancelReason::Cancelled => RivetError::new(
            ErrorKind::Cancelled,
            "cancelled.request",
            format!("`{op_id}` was cancelled"),
        ),
        CancelReason::Timeout => RivetError::new(
            ErrorKind::Timeout,
            "timeout.request",
            format!("`{op_id}` exceeded its {deadline_ms} ms deadline"),
        ),
    };
    e.with_span(span.cloned())
}

/// Await `fut` unless `token` fires first; then the pending operation is
/// abandoned (its handle stays owned by the scope, which closes it).
async fn guarded<T>(
    token: &CancelToken,
    on_cancel: impl FnOnce(CancelReason) -> RivetError,
    fut: impl std::future::Future<Output = RivetResult<T>>,
) -> RivetResult<T> {
    tokio::select! {
        biased;
        r = token.cancelled() => Err(on_cancel(r)),
        v = fut => v,
    }
}

/// Await a child that observes `token` itself (nested request, node): when
/// the token fires, keep awaiting it for the cleanup grace so it can close its
/// resources; only then give up on it.
async fn joined<T>(
    token: &CancelToken,
    grace: Duration,
    on_cancel: impl FnOnce(CancelReason) -> RivetError,
    fut: impl std::future::Future<Output = RivetResult<T>>,
) -> RivetResult<T> {
    tokio::pin!(fut);
    tokio::select! {
        biased;
        v = &mut fut => v,
        r = token.cancelled() => match tokio::time::timeout(grace, &mut fut).await {
            Ok(v) => v,
            Err(_) => Err(on_cancel(r)),
        },
    }
}

impl<'a> Machine<'a> {
    fn exec_block<'b>(
        &'b self,
        frame: &'b mut Frame,
        body: &'b [Stmt],
    ) -> BoxFuture<'b, RivetResult<Flow>> {
        Box::pin(async move {
            for stmt in body {
                if let Some(r) = frame.cancel.reason() {
                    return Err(self.cancelled(frame, r, stmt.span()));
                }
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

    /// The terminal error for a token that fired at `span`.
    fn cancelled(&self, frame: &Frame, reason: CancelReason, span: &SourceSpan) -> RivetError {
        let mut e = cancel_error(&self.op.id, frame.request.deadline_ms, reason, Some(span));
        e.operation_id = Some(self.op.id.clone());
        e
    }

    /// Run `body` under a child token for at most `limit`; on expiry cancel the
    /// child, let the body unwind (closing its handles) within the grace, then
    /// report `on_timeout`.
    async fn bounded(
        &self,
        frame: &mut Frame,
        limit: Duration,
        on_timeout: RivetError,
        body: &[Stmt],
    ) -> RivetResult<Flow> {
        let child = frame.cancel.child();
        let parent = std::mem::replace(&mut frame.cancel, child.clone());
        let grace = frame.run.close_budget();
        let r = {
            let fut = self.scoped(frame, body);
            tokio::pin!(fut);
            tokio::select! {
                v = &mut fut => v,
                _ = tokio::time::sleep(limit) => {
                    child.cancel(CancelReason::Cancelled);
                    match tokio::time::timeout(grace, &mut fut).await {
                        // A parent cancel that raced the limit keeps its own error.
                        Ok(Err(e)) if parent.is_cancelled() => Err(e),
                        Ok(Err(mut e)) if e.kind == ErrorKind::Cancelled => {
                            let mut t = on_timeout;
                            t.suppressed.append(&mut e.suppressed);
                            Err(t)
                        }
                        Ok(other) => other,
                        Err(_) => Err(on_timeout),
                    }
                }
            }
        };
        frame.cancel = parent;
        r
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
                    rhs: rhs @ (Rhs::Map { .. } | Rhs::Poll { .. }),
                    span,
                    ..
                } => match self.block_value(frame, rhs, span).await? {
                    // `return` inside a map/poll body exits the whole operation.
                    Flow::Return(v) => Ok(Flow::Return(v)),
                    Flow::Yield(v) => {
                        frame.assign(var, v);
                        Ok(Flow::Normal)
                    }
                    Flow::Normal | Flow::Break => {
                        frame.assign(var, Value::Null);
                        Ok(Flow::Normal)
                    }
                },
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
                    let v = self.eval_rhs(frame, value, &[], span).await?;
                    // Checked here too so the refusal points at the `return`.
                    secret_output_guard(frame, &v, "returned")
                        .map_err(|e| e.with_span(Some(span.clone())))?;
                    Ok(Flow::Return(v))
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
                    if let Expr::Path(path, _) = iter
                        && !path.is_empty()
                        && frame.get(&path[0]).is_none()
                        && let Some(handle) = frame.handle(&path[0])
                    {
                        loop {
                            let ctx = self.ctx(frame, span);
                            let pull = async {
                                if let Some(s) = shared_of(&handle).await {
                                    return s.next(&ctx).await;
                                }
                                let mut guard = handle.lock().await;
                                let h = guard.as_mut().ok_or_else(|| {
                                    RivetError::new(
                                        ErrorKind::Cleanup,
                                        "cleanup.closed",
                                        format!("`{}` is already closed", path[0]),
                                    )
                                })?;
                                if path.len() == 1 {
                                    h.next(&ctx).await
                                } else {
                                    h.next_of(&ctx, &path[1..].join(".")).await
                                }
                            };
                            let item =
                                guarded(&frame.cancel, |r| self.cancelled(frame, r, span), pull)
                                    .await
                                    .map_err(|e| e.with_span(Some(span.clone())))?;
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
                    // Cancellation is control flow: once the scope's token fired
                    // no catch revives it (not even `catch error kind timeout`).
                    Err(e) if !frame.cancel.is_cancelled() && catches(filter, &e) => {
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
                    Some(ms) => {
                        let on_timeout = RivetError::new(
                            ErrorKind::Timeout,
                            "timeout.scope",
                            format!("scope exceeded {ms} ms"),
                        )
                        .with_span(Some(span.clone()));
                        self.bounded(frame, Duration::from_millis(*ms), on_timeout, body)
                            .await
                    }
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
                    frame.secrets.insert(
                        name.clone(),
                        SecretTaint {
                            forms: if value.is_empty() {
                                Vec::new()
                            } else {
                                vec![value.as_bytes().to_vec()]
                            },
                            origins: origins.clone(),
                        },
                    );
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

    /// Attribution of broker decisions made while this effect runs (audit trace).
    fn effect_scope(&self, frame: &Frame, span: &SourceSpan) -> super::trace_store::EffectScope {
        super::trace_store::EffectScope {
            request_id: frame.request.request_id.clone(),
            trace_id: frame.request.trace_id.clone(),
            operation_id: self.op.id.clone(),
            line: span.start_line,
            span: Some(span.clone()),
            node_id: frame.node_id.clone(),
        }
    }

    fn ctx(&self, frame: &Frame, span: &SourceSpan) -> EffectCtx {
        EffectCtx {
            operation_id: self.op.id.clone(),
            request: frame.request.clone(),
            policy: Arc::clone(&self.interp.policy),
            span: span.clone(),
            deadline: frame.run.deadline,
            cancel: frame.cancel.clone(),
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
        for (i, a) in call.args.iter().enumerate() {
            args.push(self.eval_arg_at(frame, &call.args, i, a).await?);
        }
        // Socket/stream sends carry data to the handle's destination (G23b).
        sink_guard(
            frame,
            frame.taint_of_args(&args),
            frame.dest_of_handle(&handle).as_ref(),
            &format!("`{name}.{method}`"),
            span,
        )?;
        let ctx = self.ctx(frame, span);
        let method_ref = &method;
        let op = async move {
            if let Some(s) = shared_of(&handle).await {
                return s.call(&ctx, method_ref, args).await;
            }
            let mut guard = handle.lock().await;
            let h = guard.as_mut().ok_or_else(|| {
                RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.closed",
                    format!("`{name}` is already closed"),
                )
            })?;
            h.call(&ctx, method_ref, args).await
        };
        let r = guarded(&frame.cancel, |r| self.cancelled(frame, r, span), op)
            .await
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
        // Destination of the new handle: its own head, or the parent's for a child stream.
        let dest = match (&form.kind, source) {
            (EffectKind::Connection, Some(Expr::Path(path, _))) => frame
                .handle(&path[0])
                .and_then(|h| frame.dest_of_handle(&h)),
            _ => dest_of(&form.kind, &evaluated.head),
        };
        if form.kind != EffectKind::RequestStream {
            sink_guard(
                frame,
                frame.taint_of_form(&evaluated),
                dest.as_ref(),
                &format!("`with {}`", form.kind.as_str()),
                span,
            )?;
        }
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
                Some(adapter) => {
                    guarded(
                        &frame.cancel,
                        |r| self.cancelled(frame, r, span),
                        super::trace_store::with_effect_scope(
                            self.effect_scope(frame, span),
                            adapter.open(&ctx, form, evaluated),
                        ),
                    )
                    .await
                }
                None => Err(self.unsupported(form, span)),
            },
        };
        let handle: SharedHandle = Arc::new(tokio::sync::Mutex::new(Some(
            opened.map_err(|e| e.with_span(Some(span.clone())))?,
        )));
        let name = bind.unwrap_or("_").to_string();
        frame.handles.push((name.clone(), Arc::clone(&handle)));
        frame.dests.push((Arc::clone(&handle), dest));
        let result = self.scoped(frame, body).await;
        if let Some(pos) = frame
            .handles
            .iter()
            .rposition(|(n, h)| n == &name && Arc::ptr_eq(h, &handle))
        {
            frame.handles.remove(pos);
        }
        if let Some(pos) = frame
            .dests
            .iter()
            .rposition(|(h, _)| Arc::ptr_eq(h, &handle))
        {
            frame.dests.remove(pos);
        }
        // Always close — after success, error, break, return, cancellation or
        // deadline expiry — never interrupted by the token (disposal is a
        // non-transferable cleanup capability), bounded by the rest of the
        // grace. Inner scopes close first, so handles close in reverse order.
        let taken = handle.lock().await.take();
        let budget = frame.run.close_budget();
        let closed = match taken {
            Some(h) => match tokio::time::timeout(budget, h.close()).await {
                Ok(r) => r,
                Err(_) => Err(RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.timeout",
                    format!("`{name}` did not close within {} ms", budget.as_millis()),
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
        sink_guard(
            frame,
            frame.taint_of_value(&params),
            None,
            "another operation's params (the callee cannot enforce the binding)",
            span,
        )?;
        let dispatcher = Arc::clone(
            self.interp
                .dispatcher
                .get()
                .ok_or_else(|| RivetError::internal("nested dispatcher not configured"))?,
        );
        let token = frame.cancel.child();
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
            parent_span_id: Some(span_id(&frame.request.request_id)),
            cancel: token.clone(),
            restrict: frame.request.restrict.clone(),
        };
        let (tx, rx) = tokio::sync::mpsc::channel::<Value>(16);
        let sink: Arc<dyn DataSink> = Arc::new(ChannelSink { tx });
        let task = tokio::spawn(async move { dispatcher.dispatch(child, Some(sink)).await });
        Ok(Box::new(RequestStreamHandle {
            rx,
            task: Some(task),
            completion: None,
            token,
        }))
    }

    async fn emit(&self, frame: &mut Frame, v: Value, span: &SourceSpan) -> RivetResult<()> {
        secret_output_guard(frame, &v, "emitted").map_err(|e| e.with_span(Some(span.clone())))?;
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
                operation: frame.request.operation_id.clone(),
                seq,
                data: v,
            })
            .await
            .map_err(|e| {
                // A typed stop is the consumer's choice, not a failure: the
                // request ends cancelled (G33).
                if e.is_consumer_stop() {
                    return e.with_span(Some(span.clone()));
                }
                // A host limit (limit.buffered_bytes from the session queue budget)
                // is reported as itself, not as a consumer failure.
                if e.kind == ErrorKind::Limit {
                    return e;
                }
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
                Rhs::Map { .. } | Rhs::Poll { .. } => {
                    match self.block_value(frame, rhs, span).await? {
                        Flow::Yield(v) => Ok(v),
                        _ => Err(runtime_err(
                            "syntax.yield",
                            "`return` inside `map`/`poll` exits the operation; this position needs a value",
                            span,
                        )),
                    }
                }
            }
        })
    }

    /// Run a `map`/`poll` block: `Flow::Yield(value)` is the block value,
    /// `Flow::Return(value)` a `return` that exits the whole operation.
    async fn block_value(
        &self,
        frame: &mut Frame,
        rhs: &Rhs,
        span: &SourceSpan,
    ) -> RivetResult<Flow> {
        match rhs {
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
            _ => Err(RivetError::internal("block_value needs map or poll")),
        }
    }

    async fn evaluate_form(
        &self,
        frame: &mut Frame,
        form: &EffectForm,
    ) -> RivetResult<EvaluatedForm> {
        let mut out = EvaluatedForm::default();
        // Component-aware URL interpolation for `http METHOD URL` / `websocket URL`.
        let url_at = match form.kind {
            EffectKind::Http => Some(1),
            EffectKind::WebSocket => Some(0),
            _ => None,
        };
        for (i, a) in form.head.iter().enumerate() {
            if let (Some(u), Arg::Expr(Expr::Template(parts), span)) = (url_at, a)
                && u == i
            {
                let mut pieces = Vec::with_capacity(parts.len());
                for p in parts {
                    pieces.push(match p {
                        TemplatePart::Lit(l) => UrlPiece::Literal(l.clone()),
                        TemplatePart::Path(path) => {
                            UrlPiece::Value(lookup(frame, path, Some(span))?.to_display())
                        }
                    });
                }
                let url = assemble_url(&pieces).map_err(|e| e.with_span(Some(span.clone())))?;
                out.head.push(EvalArg::Value(Value::Text(url)));
                continue;
            }
            out.head
                .push(self.eval_arg_at(frame, &form.head, i, a).await?);
        }
        for o in &form.options {
            let mut args = Vec::new();
            for (i, a) in o.args.iter().enumerate() {
                args.push(self.eval_arg_at(frame, &o.args, i, a).await?);
            }
            out.options.push((o.key.clone(), args));
            let mut kids = Vec::with_capacity(o.children.len());
            for c in &o.children {
                let mut args = Vec::with_capacity(c.args.len());
                for (i, a) in c.args.iter().enumerate() {
                    // A part's bare NAME is a literal name, never a variable lookup.
                    args.push(match a {
                        Arg::Word(w, _) if i == 0 => EvalArg::Word(w.clone()),
                        _ => self.eval_arg_at(frame, &c.args, i, a).await?,
                    });
                }
                kids.push((c.key.clone(), args));
            }
            out.children.push(kids);
        }
        Ok(out)
    }

    /// Evaluates `args[i]`, keeping a form keyword in keyword position as a
    /// word even when a variable of the same name is in scope (a `text`
    /// parameter must not turn `file append P text text` into two values).
    async fn eval_arg_at(
        &self,
        frame: &mut Frame,
        args: &[Arg],
        i: usize,
        a: &Arg,
    ) -> RivetResult<EvalArg> {
        if keyword_slot(args, i)
            && let Arg::Word(w, _) = a
        {
            return Ok(EvalArg::Word(w.clone()));
        }
        self.eval_arg(frame, a).await
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
        sink_guard(
            frame,
            frame.taint_of_form(&evaluated),
            dest_of(&form.kind, &evaluated.head).as_ref(),
            &format!("`{}`", form.kind.as_str()),
            span,
        )?;
        let scope = self.effect_scope(frame, span);
        let ctx = self.ctx(frame, span);
        let work = super::trace_store::with_effect_scope(scope, async {
            if form.kind == EffectKind::File {
                self.run_file(&evaluated, span).await
            } else {
                match self.interp.adapters.get(form.kind.as_str()) {
                    Some(adapter) => adapter.run(&ctx, form, evaluated).await,
                    None => Err(self.unsupported(form, span)),
                }
            }
        });
        let result = guarded(&frame.cancel, |r| self.cancelled(frame, r, span), work).await;
        // `file delete … missing ok` on an absent file changed nothing.
        let noop = matches!(&result, Ok(v) if form.kind == EffectKind::File
            && v.get("deleted") == Some(&Value::Bool(false)));
        if result.is_ok() && mutates(form) && !noop {
            frame.run.commit();
        }
        result.map_err(|e| e.with_span(Some(span.clone())))
    }

    async fn run_file(&self, f: &EvaluatedForm, span: &SourceSpan) -> RivetResult<Value> {
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
    ) -> RivetResult<Flow> {
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
        // Items share one group token: an early exit cancels the siblings and
        // joins them (their scopes close their handles) before returning.
        let group = frame.cancel.child();
        let early: Option<RivetResult<Flow>> = loop {
            while running.len() < limit {
                let Some((i, value)) = pending.next() else {
                    break;
                };
                let mut child = frame.clone();
                child.cancel = group.clone();
                child
                    .scopes
                    .push(HashMap::from([(item.to_string(), value)]));
                running.push(async move {
                    let r = self.exec_block(&mut child, body).await;
                    (i, r)
                });
            }
            let Some((i, r)) = running.next().await else {
                break None;
            };
            match r {
                Ok(Flow::Yield(v)) => results[i] = Some(v),
                // `return` exits the whole operation.
                Ok(Flow::Return(v)) => break Some(Ok(Flow::Return(v))),
                Ok(_) => {
                    break Some(Err(runtime_err(
                        "syntax.map_yield",
                        "a `map` body finished without `yield`",
                        span,
                    )));
                }
                Err(e) => break Some(Err(e)),
            }
        };
        if let Some(exit) = early {
            group.cancel(CancelReason::Cancelled);
            join_all(&mut running, frame.run.close_budget()).await;
            return exit;
        }
        Ok(Flow::Yield(Value::List(
            results
                .into_iter()
                .map(|v| v.unwrap_or(Value::Null))
                .collect(),
        )))
    }

    async fn run_poll(
        &self,
        frame: &mut Frame,
        every: Option<u64>,
        timeout: Option<u64>,
        body: &[Stmt],
        span: &SourceSpan,
    ) -> RivetResult<Flow> {
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
                    Ok(Flow::Return(v)) => {
                        frame.scopes.pop();
                        return Ok(Flow::Return(v));
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
                return Ok(Flow::Yield(yielded.unwrap_or(Value::Null)));
            }
            if start.elapsed() + every > limit {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.poll",
                    format!("poll did not complete within {} ms", limit.as_millis()),
                )
                .with_span(Some(span.clone())));
            }
            let token = frame.cancel.clone();
            guarded(&token, |r| self.cancelled(frame, r, span), async {
                tokio::time::sleep(every).await;
                Ok(())
            })
            .await?;
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
        // One token for the group: fail fast and the group timeout cancel the
        // running tasks, which unwind (closing their handles) and are joined.
        let token = frame.cancel.child();
        let mut pending = tasks.iter();
        let mut running = FuturesUnordered::new();
        let mut first_error: Option<RivetError> = None;
        let expiry = options
            .timeout_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms));
        loop {
            if first_error.is_none() || options.failure != FailurePolicy::Fast {
                while running.len() < limit {
                    let Some((name, body)) = pending.next() else {
                        break;
                    };
                    let mut child = frame.clone();
                    child.cancel = token.clone();
                    running
                        .push(async move { (name.clone(), self.scoped(&mut child, body).await) });
                }
            }
            let next = match expiry {
                Some(at) => {
                    match tokio::time::timeout(
                        at.saturating_duration_since(Instant::now()),
                        running.next(),
                    )
                    .await
                    {
                        Ok(n) => n,
                        Err(_) => {
                            let ms = options.timeout_ms.unwrap_or_default();
                            token.cancel(CancelReason::Cancelled);
                            join_all(&mut running, frame.run.close_budget()).await;
                            return Err(RivetError::new(
                                ErrorKind::Timeout,
                                "timeout.concurrent",
                                format!("concurrent group exceeded {ms} ms"),
                            )
                            .with_span(Some(span.clone())));
                        }
                    }
                }
                None => running.next().await,
            };
            let Some((_name, r)) = next else {
                break;
            };
            if let Err(e) = r {
                let fast = options.failure == FailurePolicy::Fast;
                if first_error.is_none() && fast {
                    token.cancel(CancelReason::Cancelled);
                }
                // Siblings cancelled by the first failure report `cancelled`;
                // the first failure stays the primary error.
                match &mut first_error {
                    None => first_error = Some(e),
                    Some(primary) if e.kind != ErrorKind::Cancelled && !fast => {
                        primary.suppressed.push(e)
                    }
                    Some(_) => {}
                }
                if fast {
                    join_all(&mut running, frame.run.close_budget()).await;
                    break;
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(Flow::Normal),
        }
    }

    async fn run_dag(
        &self,
        frame: &mut Frame,
        options: &GroupOptions,
        nodes: &[DagNode],
        span: &SourceSpan,
    ) -> RivetResult<()> {
        let executor = self
            .interp
            .dag
            .get()
            .cloned()
            .ok_or_else(|| RivetError::internal("dag executor not configured"))?;
        let input = crate::domain::dag::DagInput {
            nodes: nodes
                .iter()
                .map(|n| crate::domain::dag::DagNodeSpec {
                    id: n.name.clone(),
                    after: n.after.clone(),
                })
                .collect(),
            failure: options.failure,
            limit: options.limit.unwrap_or(4).max(1) as usize,
            timeout_ms: options.timeout_ms,
        };
        let runner = NodeRunner {
            machine: self,
            base: frame.clone(),
            nodes,
            group: frame.cancel.child(),
        };
        let completion = executor.run(input, &runner).await;
        // Every node name now evaluates to its {status, result, error} envelope.
        for status in &completion.nodes {
            frame.define(&status.id, status.envelope());
        }
        match completion.fatal {
            Some(e) => Err(e.with_span(Some(span.clone()))),
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
                    if frame.get(&path[0]).is_none()
                        && let Some(handle) = frame.handle(&path[0])
                    {
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
                        let read = async {
                            if let Some(s) = shared_of(&handle).await {
                                return s.property(&ctx, &path[1]).await;
                            }
                            let mut guard = handle.lock().await;
                            let h = guard.as_mut().ok_or_else(|| {
                                RivetError::new(
                                    ErrorKind::Cleanup,
                                    "cleanup.closed",
                                    format!("`{}` is already closed", path[0]),
                                )
                            })?;
                            h.property(&ctx, &path[1]).await
                        };
                        let mut v =
                            guarded(&frame.cancel, |r| self.cancelled(frame, r, span), read)
                                .await
                                .map_err(|e| e.with_span(Some(span.clone())))?;
                        for seg in &path[2..] {
                            v = v.get(seg).cloned().ok_or_else(|| {
                                runtime_err("value.missing_key", format!("no key `{seg}`"), span)
                            })?;
                        }
                        return Ok(v);
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
        // G23b: a helper fed a tainted input yields a derived form (base64, hex, …).
        let tainted: Vec<String> = if frame.secrets.is_empty() || func == "request" {
            Vec::new()
        } else {
            let mut names: Vec<String> = frame.secrets.keys().cloned().collect();
            names.retain(|n| {
                frame.secrets[n]
                    .forms
                    .iter()
                    .any(|f| vals.iter().any(|v| value_carries(v, f)))
            });
            names
        };
        let out = self.call_values(frame, func, vals, span).await?;
        if !tainted.is_empty() {
            frame.derive_taint(&tainted, &out);
        }
        Ok(out)
    }

    async fn call_values(
        &self,
        frame: &mut Frame,
        func: &str,
        vals: Vec<Value>,
        span: &SourceSpan,
    ) -> RivetResult<Value> {
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
                // MCP / gRPC / local operation params: the callee cannot enforce the
                // secret's origin binding, so a tainted value never crosses (G23b).
                sink_guard(
                    frame,
                    frame.taint_of_value(&params),
                    None,
                    "another operation's params (the callee cannot enforce the binding)",
                    span,
                )?;
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
                    parent_span_id: Some(span_id(&frame.request.request_id)),
                    cancel: frame.cancel.child(),
                    restrict: frame.request.restrict.clone(),
                };
                // The child observes its own (child) token and unwinds; the
                // parent waits for its cleanup instead of dropping it.
                let completion = match joined(
                    &frame.cancel,
                    frame.run.close_budget(),
                    |r| self.cancelled(frame, r, span),
                    dispatcher.dispatch(child, None),
                )
                .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        frame.run.mark(e.effects);
                        return Err(e.with_span(Some(span.clone())));
                    }
                };
                // The callee's status is folded in as is: an opaque remote MCP call
                // (`unknown`) keeps the wrapping request `unknown`, not `committed`.
                frame.run.mark(completion.effects);
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
            "xml.element" => {
                arity(3)?;
                xml_element(&vals[0], &vals[1], &vals[2])
                    .map_err(|m| runtime_err("call.xml", m, span))
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
            // `check` already rejects names outside BUILTIN_FUNCTIONS
            // (check.unknown_function); `request.stream` only opens in `with`.
            other => Err(runtime_err(
                "call.unknown",
                format!(
                    "unknown function `{other}` here; built-in functions: {}",
                    crate::domain::ir::BUILTIN_FUNCTIONS.join(", ")
                ),
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
    /// The child request's token (a child of the owning scope's).
    token: CancelToken,
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
                    // The script-level completion object (language API, not wire output).
                    Some(c) => Value::object([
                        ("request_id", Value::text(&c.request_id)),
                        ("trace_id", Value::text(&c.trace_id)),
                        ("result", c.result.clone()),
                        ("data_count", Value::Int(c.data_count as i64)),
                        ("effects", Value::text(c.effects.as_str())),
                    ]),
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
        if let Some(mut task) = self.task.take() {
            // Cancel the child request and join its cleanup; the caller bounds
            // this close by the grace, after which the task is aborted.
            self.token.cancel(CancelReason::Cancelled);
            self.rx.close();
            let abort = task.abort_handle();
            let _guard = AbortOnDrop(abort);
            let _ = (&mut task).await;
        }
        Ok(())
    }
}

/// Aborts a spawned child when the join is given up (grace exceeded).
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Join every future still in a group (after its token fired) within `grace`;
/// whatever has not finished by then is dropped.
async fn join_all<F: std::future::Future>(running: &mut FuturesUnordered<F>, grace: Duration) {
    let _ = tokio::time::timeout(grace, async { while running.next().await.is_some() {} }).await;
}

/// `for message in incoming`: the run's live input feed (read-only iterable).
/// Every item is checked against the declared `receives` spec (surfaces check
/// too; this is the last line for any feeder).
struct IncomingHandle {
    rx: tokio::sync::mpsc::Receiver<Value>,
    spec: Option<crate::domain::outputs::ValueSpec>,
    seq: u64,
}

#[async_trait]
impl ResourceHandle for IncomingHandle {
    async fn next(&mut self, _ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        let Some(item) = self.rx.recv().await else {
            return Ok(None);
        };
        self.seq += 1;
        if let Some(spec) = &self.spec {
            let mut violations = Vec::new();
            spec.check(&item, "", &mut violations);
            if let Some(v) = violations.first() {
                return Err(RivetError::validation(
                    "validation.input",
                    format!(
                        "input item {} at {} must be {}, got {}",
                        self.seq,
                        if v.path.is_empty() { "$" } else { &v.path },
                        v.expected,
                        v.found
                    ),
                )
                .with_details(Value::object([
                    ("seq", Value::Int(self.seq as i64)),
                    ("path", Value::text(&v.path)),
                    ("expected", Value::text(&v.expected)),
                    ("found", Value::text(&v.found)),
                ])));
            }
        }
        Ok(Some(item))
    }
}

/// Evaluates one DAG node's expression in a copy of the enclosing frame, with
/// the envelopes of its completed dependencies in scope.
struct NodeRunner<'m> {
    machine: &'m Machine<'m>,
    base: Frame,
    nodes: &'m [DagNode],
    /// Token shared by the DAG's nodes (fail fast / dag timeout cancel it).
    group: CancelToken,
}

#[async_trait]
impl crate::domain::dag::DagNodeRunner for NodeRunner<'_> {
    async fn run_node(
        &self,
        index: usize,
        dependencies: Vec<(String, Value)>,
    ) -> RivetResult<Value> {
        let mut child = self.base.clone();
        child.cancel = self.group.clone();
        child.node_id = Some(self.nodes[index].name.clone());
        for (name, envelope) in dependencies {
            child.define(&name, envelope);
        }
        // A cancelled node unwinds through its own scopes; the join is bounded
        // by the grace so the scheduler always gets the node back.
        let grace = self.base.run.close_budget();
        let span = self.nodes[index].span.clone();
        let group = self.group.clone();
        let fut = self.machine.eval(&mut child, &self.nodes[index].expr);
        joined(
            &group,
            grace,
            |r| {
                cancel_error(
                    &self.machine.op.id,
                    self.base.request.deadline_ms,
                    r,
                    Some(&span),
                )
            },
            fut,
        )
        .await
    }

    fn now(&self) -> std::time::SystemTime {
        std::time::SystemTime::now()
    }

    fn cancel_nodes(&self) {
        self.group.cancel(CancelReason::Cancelled);
    }
}
