//! Library surface: the Rust `Runtime` API is itself a surface. It calls the
//! same use cases as the CLI, HTTP, WebSocket, polling and MCP surfaces, and
//! wires the session driver those surfaces share.
//!
//! ```text
//!  Runtime::builder().file(..).build()?
//!     ├─ call(InputEnvelope) / call_json(JSON) ─▶ serve.parse_input ─▶ execution.request_operation ─▶ ResponseEnvelope
//!     ├─ request / request_as / dispatch_request ─▶ execution.request_operation (Completion or RivetError)
//!     ├─ scope(|scope| …): scope.stream / scope.duplex ─▶ execution.request_operation (owned, joined)
//!     ├─ list / describe / outputs               ─▶ registry use cases
//!     └─ open_session / send_input / finish_input / read_events / cancel_session ─▶ sessions use cases
//! ```

// vhco:surface library kind library calls language/compile_program, language/compile_globals, policy/load_policy, execution/request_operation, registry/describe_operations, registry/inspect_outputs, sessions/open_session, sessions/send_input, sessions/finish_input, sessions/read_events, sessions/cancel_session, serve/authorize_operation, serve/start_serve, serve/parse_input, auth/begin_authorization, auth/complete_authorization, auth/credential_status, auth/disconnect_account, auth/cancel_authorization
// vhco:trigger library auth/begin_authorization = Runtime::request("rivet.auth.begin", DATA, None)
// vhco:trigger library auth/complete_authorization = Runtime::request("rivet.auth.complete", DATA, None)
// vhco:trigger library auth/credential_status = Runtime::request("rivet.auth.status", DATA, None)
// vhco:trigger library auth/disconnect_account = Runtime::request("rivet.auth.disconnect", DATA, None)
// vhco:trigger library auth/cancel_authorization = Runtime::request("rivet.auth.cancel", DATA, None)
// vhco:trigger library serve/parse_input = Runtime::call_json(INPUT_JSON)
// vhco:trigger library language/compile_program = Runtime::builder().file(PATH).build()
// vhco:trigger library language/compile_globals = Runtime::builder().file(PATH).build()
// vhco:trigger library policy/load_policy = Runtime::builder().policy_file(PATH).build() | Policy::from_file(PATH) | Policy::from_json(BYTES) | Runtime::builder().ceiling(Policy)
// vhco:trigger library execution/request_operation = Runtime::call(InputEnvelope) | Runtime::request(ID, DATA, sink) | Runtime::scope(|scope| …) scope.stream(ID, DATA) | scope.duplex(ID, DATA)
// vhco:trigger library registry/describe_operations = Runtime::list() | Runtime::describe(IDS)
// vhco:trigger library registry/inspect_outputs = Runtime::outputs(ID, all)
// vhco:trigger library sessions/open_session = Runtime::open_session(SessionOpenInput)
// vhco:trigger library sessions/send_input = Runtime::send_input(SessionSendInput)
// vhco:trigger library sessions/finish_input = Runtime::finish_input(SessionRef)
// vhco:trigger library sessions/read_events = Runtime::read_events(SessionReadInput)
// vhco:trigger library sessions/cancel_session = Runtime::cancel_session(SessionRef)
// vhco:trigger library serve/authorize_operation = Runtime::dispatch_request(Request) (principal check at depth 0)
// vhco:trigger library serve/start_serve = orchestrator::setup_serve::serve(runtime, options)
// vhco:api library execution/request_operation Runtime::call(InputEnvelope) -- invoke one operation in-process and always get a ResponseEnvelope (errors included; to_json / to_json_pretty); Runtime::request(id, data, on_data) stays for `?`-style use and returns Completion or RivetError
// vhco:request { "operation": "string — operation ID", "data": "Value object (default {})", "deadline_ms": "int?", "restrict": "{grants:[…]}?" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "string", "type": "result", "status": "ok|error|cancelled", "data": "Value|null", "error": "RivetError object|null", "effects": "none|committed|partial|unknown", "data_count": "int" }
// vhco:api library sessions/open_session Runtime::open_session(SessionOpenInput) -- open a live session (duplex or server-streaming) owned by the principal
// vhco:request { "id": "string", "params": "Value", "principal": "Principal", "connection_owned": "bool" }
// vhco:response { "session_id": "string", "request_id": "string", "catalog_version": "string", "input_schema": "json|null", "emits_schema": "json|null", "next_send_seq": "int", "expires_at": "RFC 3339" }

use super::runtime::Runtime;
use crate::domain::contracts::{Completion, DataEvent, Envelope, Principal};
use crate::domain::envelope::{InputEnvelope, RawInput, ResponseEnvelope};
use crate::domain::ports::DataSink;
use crate::domain::sessions::{
    CancelReceipt, SessionAck, SessionBatch, SessionLimits, SessionOpenInput, SessionReadInput,
    SessionReceipt, SessionRef, SessionSendInput,
};
use crate::domain::{RivetError, RivetResult, Value};
use crate::features::execution::request_operation::validate_params;
use crate::features::serve::parse_input::parse_input;
use crate::features::sessions::{
    cancel_session, finish_input, open_session, read_events, send_input,
};
use crate::infra::registry::ProgramRegistry;
use crate::infra::session_driver::{LaunchFn, MintFn, SessionHost, ValidateFn};
use std::sync::Arc;

/// Re-enters the runtime from the session driver without a strong cycle.
pub type UpgradeFn = dyn Fn() -> Option<Runtime> + Send + Sync;

/// Build the shared session driver: runs go through `Runtime::dispatch_session`
/// (the one dispatcher), IDs are minted by the runtime, params are validated
/// with the dispatcher's own rules before a session is created.
pub fn session_host(
    upgrade: Arc<UpgradeFn>,
    registry: Arc<ProgramRegistry>,
    catalog_version: String,
    limits: SessionLimits,
) -> SessionHost {
    let up = Arc::clone(&upgrade);
    let launch: Arc<LaunchFn> = Arc::new(move |req, sink, input| {
        let rt = up();
        Box::pin(async move {
            match rt {
                Some(rt) => rt.dispatch_session(req, sink, input).await,
                None => Err(crate::domain::RivetError::new(
                    crate::domain::ErrorKind::Cancelled,
                    "cancelled.runtime",
                    "runtime shut down",
                )),
            }
        })
    });
    let up = Arc::clone(&upgrade);
    let mint: Arc<MintFn> = Arc::new(move |id, params, principal| match up() {
        Some(rt) => rt.new_request(id, params, principal),
        None => crate::domain::contracts::Request {
            request_id: "req_shutdown".into(),
            trace_id: "tr_shutdown".into(),
            operation_id: id.to_string(),
            params,
            principal,
            parent_request_id: None,
            depth: 0,
            deadline_ms: 1,
            include_private: false,
            parent_span_id: None,
            cancel: crate::domain::cancel::CancelToken::new(),
            restrict: None,
        },
    });
    let validate: Arc<ValidateFn> = Arc::new(validate_params);
    SessionHost::new(registry, launch, mint, validate, catalog_version, limits)
}

impl Runtime {
    /// Invoke one operation from an [`InputEnvelope`] as the local principal and
    /// always get a [`ResponseEnvelope`] (errors included) — the same record every
    /// other surface writes. `Runtime::request` stays for `?`-style Rust use.
    pub async fn call(&self, input: InputEnvelope) -> ResponseEnvelope {
        let mut req = self.new_request(&input.operation, input.data, Principal::local());
        if let Some(ms) = input.deadline_ms {
            req.deadline_ms = ms;
        }
        req.restrict = input.restrict;
        self.note_deprecated_input(
            &req.request_id,
            &req.trace_id,
            &input.operation,
            &input.aliases,
        );
        let operation = req.operation_id.clone();
        let outcome = self.dispatch_request(req, None).await;
        ResponseEnvelope::from_outcome(&operation, &outcome)
    }

    /// [`Runtime::call`] from input-envelope JSON text (`serve.parse_input`):
    /// the same file works with `rivet request --input`, `POST /v1/request` and
    /// the FFI. Malformed JSON or a bad envelope is an error envelope.
    pub async fn call_json(&self, input_json: &str) -> ResponseEnvelope {
        let body = match serde_json::from_str::<serde_json::Value>(input_json) {
            Ok(j) => j,
            Err(e) => {
                return ResponseEnvelope::from_error(
                    None,
                    &RivetError::validation(
                        crate::domain::envelope::INPUT_ENVELOPE,
                        format!("the input envelope is not valid JSON: {e}"),
                    ),
                );
            }
        };
        let hint = body
            .get("operation")
            .or_else(|| body.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        match parse_input(RawInput::new(body)) {
            Ok(input) => self.call(input).await,
            Err(e) => ResponseEnvelope::from_error(hint.as_deref(), &e),
        }
    }

    /// `rivet.sessions.open` for library hosts.
    pub async fn open_session(&self, input: SessionOpenInput) -> RivetResult<SessionReceipt> {
        open_session::open_session(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.send` for library hosts.
    pub async fn send_input(&self, input: SessionSendInput) -> RivetResult<SessionAck> {
        send_input::send_input(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.finish_input` for library hosts.
    pub async fn finish_input(&self, input: SessionRef) -> RivetResult<SessionAck> {
        finish_input::finish_input(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.read` for library hosts.
    pub async fn read_events(&self, input: SessionReadInput) -> RivetResult<SessionBatch> {
        read_events::read_events(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.cancel` for library hosts.
    pub async fn cancel_session(&self, input: SessionRef) -> RivetResult<CancelReceipt> {
        cancel_session::cancel_session(input, self.sessions().as_ref()).await
    }
}

// ---------------------------------------------------------------- Runtime::scope

/// A request scope (`rt.scope(|scope| async move { … })`, PROP E11): every
/// stream and duplex it starts is owned by it and is cancelled and joined when
/// the scope body returns (or the scope future is dropped).
///
/// ```text
///  rt.scope(|scope| async move {
///      let mut s = scope.stream("events.count", params).await?;   ─┐ task + request
///      while let Some(env) = s.next().await? { … }                  │ Data … Result
///      let mut d = scope.duplex("chat.relay", params).await?;     ─┤ task + input feed
///      d.send(v).await?; d.finish_send(); d.next().await?;          │
///      Ok(())                                                       │
///  }).await   ── body done ─▶ cancel unfinished requests ─▶ join ──┘
/// ```
#[derive(Clone)]
pub struct Scope {
    rt: Runtime,
    inner: Arc<ScopeInner>,
}

#[derive(Default)]
struct ScopeInner {
    tasks: std::sync::Mutex<Vec<(String, tokio::task::JoinHandle<()>)>>,
}

impl Drop for ScopeInner {
    fn drop(&mut self) {
        // The scope future was dropped before `scope` could join: abort
        // (dropping a request future runs its cleanup).
        let tasks = self.tasks.get_mut().unwrap_or_else(|e| e.into_inner());
        for (_, t) in tasks.drain(..) {
            t.abort();
        }
    }
}

/// Grace period for cancelled requests to finish their cleanup before abort.
const SCOPE_JOIN_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// Envelopes a scope handle may queue ahead of the consumer (backpressure).
const STREAM_QUEUE: usize = 16;

impl Runtime {
    /// Run `body` with a [`Scope`]; everything the scope started is cancelled
    /// (if still running) and joined before this returns.
    pub async fn scope<F, Fut, T>(&self, body: F) -> RivetResult<T>
    where
        F: FnOnce(Scope) -> Fut,
        Fut: std::future::Future<Output = RivetResult<T>>,
    {
        let inner = Arc::new(ScopeInner::default());
        let scope = Scope {
            rt: self.clone(),
            inner: Arc::clone(&inner),
        };
        let out = body(scope).await;
        let tasks: Vec<(String, tokio::task::JoinHandle<()>)> = inner
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        for (request_id, task) in &tasks {
            if !task.is_finished() {
                let _ = self.cancel(request_id, Principal::local());
            }
        }
        for (_, mut task) in tasks {
            if tokio::time::timeout(SCOPE_JOIN_GRACE, &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
        out
    }
}

impl Scope {
    /// Start `id` as a server stream owned by this scope.
    pub async fn stream(&self, id: &str, params: Value) -> RivetResult<StreamHandle> {
        let req = self.rt.new_request(id, params, Principal::local());
        let request_id = req.request_id.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<Envelope>(STREAM_QUEUE);
        let rt = self.rt.clone();
        let task = tokio::spawn(async move {
            let sink: Arc<dyn DataSink> = Arc::new(EnvelopeSink { tx: tx.clone() });
            let (rid, tid, op) = (
                req.request_id.clone(),
                req.trace_id.clone(),
                req.operation_id.clone(),
            );
            let out = rt.dispatch_request(req, Some(sink)).await;
            let _ = tx.send(terminal(out, rid, tid, op)).await;
        });
        self.own(request_id.clone(), task);
        Ok(StreamHandle {
            request_id,
            rx,
            done: false,
        })
    }

    /// Start `id` (an operation that `receives`) as a duplex owned by this
    /// scope: `send` items (checked against `receives`), `finish_send`, `next`.
    pub async fn duplex(&self, id: &str, params: Value) -> RivetResult<DuplexHandle> {
        let entry = self
            .rt
            .describe(&[id.to_string()])?
            .entries
            .into_iter()
            .next()
            .ok_or_else(|| {
                RivetError::not_found("not_found.operation", format!("no operation `{id}`"))
            })?;
        let receives = super::remote_cli::receives_of(&entry)?;
        let req = self.rt.new_request(id, params, Principal::local());
        let request_id = req.request_id.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<Envelope>(STREAM_QUEUE);
        let (in_tx, in_rx) = tokio::sync::mpsc::channel::<Value>(STREAM_QUEUE);
        let rt = self.rt.clone();
        let task = tokio::spawn(async move {
            let sink: Arc<dyn DataSink> = Arc::new(EnvelopeSink { tx: tx.clone() });
            let (rid, tid, op) = (
                req.request_id.clone(),
                req.trace_id.clone(),
                req.operation_id.clone(),
            );
            let out = rt.dispatch_session(req, sink, in_rx).await;
            let _ = tx.send(terminal(out, rid, tid, op)).await;
        });
        self.own(request_id.clone(), task);
        Ok(DuplexHandle {
            sender: DuplexSender {
                input: Some(in_tx),
                receives,
            },
            stream: StreamHandle {
                request_id,
                rx,
                done: false,
            },
        })
    }

    fn own(&self, request_id: String, task: tokio::task::JoinHandle<()>) {
        self.inner
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((request_id, task));
    }
}

fn terminal(
    out: RivetResult<Completion>,
    request_id: String,
    trace_id: String,
    operation: String,
) -> Envelope {
    match out {
        Ok(c) => Envelope::Result(c),
        Err(e) => Envelope::Error {
            request_id,
            trace_id,
            operation,
            seq: 0,
            error: Box::new(e),
        },
    }
}

/// Forwards data items to the handle; a dropped handle is a typed stop, so
/// the request ends `cancelled`, never `consumer_failed`.
struct EnvelopeSink {
    tx: tokio::sync::mpsc::Sender<Envelope>,
}

#[async_trait::async_trait]
impl DataSink for EnvelopeSink {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        self.tx
            .send(Envelope::Data(event))
            .await
            .map_err(|_| RivetError::consumer_stop())
    }
}

/// The receive side of a scope stream or duplex.
pub struct StreamHandle {
    request_id: String,
    rx: tokio::sync::mpsc::Receiver<Envelope>,
    done: bool,
}

impl StreamHandle {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// The next envelope: `Data` items in order, then exactly one terminal
    /// `Result`, then `None`. A terminal error is returned as `Err` (so `?`
    /// ends the scope body), after which `next` yields `None`.
    pub async fn next(&mut self) -> RivetResult<Option<Envelope>> {
        if self.done {
            return Ok(None);
        }
        match self.rx.recv().await {
            Some(Envelope::Error { error, .. }) => {
                self.done = true;
                Err(*error)
            }
            Some(env @ Envelope::Result(_)) => {
                self.done = true;
                Ok(Some(env))
            }
            Some(env) => Ok(Some(env)),
            None => {
                self.done = true;
                Ok(None)
            }
        }
    }
}

/// The send side of a scope duplex.
pub struct DuplexSender {
    input: Option<tokio::sync::mpsc::Sender<Value>>,
    receives: crate::domain::outputs::ValueSpec,
}

impl DuplexSender {
    /// Send one input item (checked against `receives`; waits for capacity).
    pub async fn send(&mut self, item: Value) -> RivetResult<()> {
        let mut violations = Vec::new();
        self.receives.check(&item, "", &mut violations);
        if let Some(x) = violations.first() {
            return Err(RivetError::validation(
                "validation.input",
                format!(
                    "input item at {} must be {}, got {}",
                    if x.path.is_empty() { "$" } else { &x.path },
                    x.expected,
                    x.found
                ),
            ));
        }
        let closed = || {
            RivetError::new(
                crate::domain::ErrorKind::Conflict,
                "conflict.input_finished",
                "input is finished or the request already ended",
            )
        };
        match &self.input {
            Some(tx) => tx.send(item).await.map_err(|_| closed()),
            None => Err(closed()),
        }
    }

    /// End the input (`incoming` ends for the operation). Idempotent.
    pub fn finish_send(&mut self) {
        self.input = None;
    }
}

/// A scope duplex: [`DuplexSender`] + [`StreamHandle`]; `into_split` hands
/// the two halves to different tasks.
pub struct DuplexHandle {
    sender: DuplexSender,
    stream: StreamHandle,
}

impl DuplexHandle {
    pub fn request_id(&self) -> &str {
        self.stream.request_id()
    }

    pub async fn send(&mut self, item: Value) -> RivetResult<()> {
        self.sender.send(item).await
    }

    pub fn finish_send(&mut self) {
        self.sender.finish_send();
    }

    pub async fn next(&mut self) -> RivetResult<Option<Envelope>> {
        self.stream.next().await
    }

    pub fn into_split(self) -> (DuplexSender, StreamHandle) {
        (self.sender, self.stream)
    }
}
