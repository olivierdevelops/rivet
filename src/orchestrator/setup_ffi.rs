//! FFI surface: the logic behind `librivet`'s C ABI (PROP-2026-0002 R13–R15,
//! R23; ADR-0005 decision 3). The `rivet-ffi` crate (`ffi/src/lib.rs`) only
//! converts C arguments and forwards here; every entry point below is safe
//! Rust, JSON in / JSON out.
//!
//! ```text
//!  C host ──rivet_*(…)──▶ ffi/src/lib.rs shim ──(bytes, handle tokens)──▶ setup_ffi::* ──▶ Runtime
//!                               │                                           │
//!                               │                      guard: catch_unwind → internal.panic envelope
//!                               │                      arg:   NULL / non-UTF-8 → validation.ffi_argument
//!                               │                      handle: unknown / freed / wrong kind → validation.ffi_argument
//!                               ▼
//!                        malloc'd UTF-8 JSON ◀── ResponseEnvelope::render(compact | pretty)
//!
//!  handles are opaque tokens ((generation << 2) | kind), never dereferenced: a
//!  table maps each live token to its object, so a double free or a use after
//!  free is detected (the token is gone) instead of undefined behaviour.
//!
//!  RivetCall (a session of the shared session driver, owned by the handle)
//!    start ──next()──▶ data … ──next()──▶ result ──next()──▶ NULL (done) ──free
//!      │  send()/finish_input() while running   │ cancel() ─▶ result{status:"cancelled"}
//!      │  next(timeout) with nothing new ─▶ {"type":"timeout"} (poll again)
//!      └──────── free() while running ─▶ cancel + bounded cleanup (5 s grace) ─▶ freed
//! ```

// vhco:surface ffi kind ffi calls language/compile_program, policy/load_policy, serve/parse_input, execution/request_operation, execution/cancel_request, sessions/open_session, sessions/send_input, sessions/finish_input, sessions/read_events, sessions/cancel_session, language/highlight_source, registry/load_module
// vhco:trigger ffi language/compile_program = rivet_runtime_new(OPTIONS_JSON, &rt, &error_json)
// vhco:trigger ffi policy/load_policy = rivet_runtime_new(OPTIONS_JSON, &rt, &error_json) (policy_file | policy_json, ceiling_json)
// vhco:trigger ffi serve/parse_input = rivet_request(rt, INPUT_JSON) | rivet_call_start(rt, INPUT_JSON)
// vhco:trigger ffi execution/request_operation = rivet_request(rt, INPUT_JSON) | rivet_module_call(module, ID, DATA_JSON)
// vhco:trigger ffi execution/cancel_request = rivet_call_cancel(call) | rivet_call_free(call) while running
// vhco:trigger ffi sessions/open_session = rivet_call_start(rt, INPUT_JSON) | rivet_module_call_start(module, ID, DATA_JSON)
// vhco:trigger ffi sessions/send_input = rivet_call_send(call, DATA_JSON)
// vhco:trigger ffi sessions/finish_input = rivet_call_finish_input(call)
// vhco:trigger ffi sessions/read_events = rivet_call_next(call, TIMEOUT_MS)
// vhco:trigger ffi sessions/cancel_session = rivet_call_cancel(call) | rivet_call_free(call) while running
// vhco:trigger ffi language/highlight_source = rivet_highlight(SOURCE, FORMAT)
// vhco:trigger ffi registry/load_module = rivet_load(rt, PATH, ALIAS_OR_NULL, &module, &error_json)
// vhco:api ffi execution/request_operation rivet_request(rt, INPUT_JSON) -- blocking call; always returns a ResponseEnvelope (errors included) as a malloc'd UTF-8 string the caller frees with rivet_string_free; `"pretty": true` (or the runtime option) indents it
// vhco:request { "operation": "string — operation ID", "data": "object (default {})", "deadline_ms": "int?", "restrict": "{grants:[…]}?", "pretty": "bool?" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "string", "type": "result", "status": "ok|error|cancelled", "data": "Value|null", "error": "RivetError object|null", "effects": "none|committed|partial|unknown", "data_count": "int" }
// vhco:api ffi sessions/open_session rivet_call_start(rt, INPUT_JSON) -- start a call handle over the session driver (any operation; streams and live input included); records are pulled with rivet_call_next
// vhco:request { "operation": "string", "data": "object (default {})", "deadline_ms": "int?", "restrict": "object?", "pretty": "bool?" }
// vhco:response { "handle": "RivetCall* — never NULL; a failed start yields its error record on the first rivet_call_next" }
// vhco:api ffi sessions/read_events rivet_call_next(call, TIMEOUT_MS) -- the next record (a `type: data` item or the terminal `type: result`), `{"type":"timeout"}` when nothing arrived within TIMEOUT_MS (negative = wait), NULL after the terminal record
// vhco:request { "timeout_ms": "int64 — 0 polls, negative waits until a record arrives" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "string", "type": "data|result", "seq": "int", "status": "ok|error|cancelled (result only)", "data": "Value|null", "error": "object|null" }
// vhco:api ffi sessions/send_input rivet_call_send(call, DATA_JSON) -- send one live input item to an operation that `receives` (sequence numbers are kept by the handle)
// vhco:request { "data": "Value — one item matching the operation's receives schema" }
// vhco:response { "session_id": "string", "accepted_seq": "int|null", "input_closed": "bool" }
// vhco:api ffi registry/load_module rivet_load(rt, PATH, ALIAS_OR_NULL, &module, &error_json) -- load a .rivet file below the runtime root as a module object (alias = file stem when NULL); RIVET_OK and a RivetModule*, or RIVET_ERROR and an error envelope
// vhco:request { "path": "string — relative to the runtime root", "alias": "string|NULL" }
// vhco:response { "status": "RIVET_OK (0) | RIVET_ERROR (1)", "module": "RivetModule* on success", "error_json": "ResponseEnvelope with status error on failure" }
// vhco:api ffi language/highlight_source rivet_highlight(SOURCE, FORMAT) -- tokens of a .rivet source as json (one token per line), html or ansi; a syntax error keeps the tokens before it and, in json, ends with an error envelope line
// vhco:request { "source": "string", "format": "json|html|ansi|NULL (json)" }
// vhco:response { "text": "string — json lines {line, col, len, class, text}, <pre> html or ANSI" }

use super::runtime::Runtime;
use crate::domain::contracts::Principal;
use crate::domain::envelope::{INPUT_ENVELOPE, OutputFormat, RawInput, ResponseEnvelope};
use crate::domain::errors::codes::INTERNAL_PANIC;
use crate::domain::ffi::{FfiOptions, ffi_argument};
use crate::domain::highlight::HighlightFormat;
use crate::domain::policy::Policy;
use crate::domain::sessions::{SessionOpenInput, SessionReadInput, SessionRef, SessionSendInput};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use crate::features::serve::parse_input::parse_input;
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// `RivetStatus` values.
pub const RIVET_OK: i32 = 0;
pub const RIVET_ERROR: i32 = 1;

/// The crate version as a NUL-terminated static string (`rivet_version()`).
pub const VERSION_NUL: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// Grace for draining a call or a runtime on free (proposal: 5 s).
const GRACE: Duration = Duration::from_secs(5);

/// A handle token as C sees it (cast to an opaque pointer by the shim).
pub type Handle = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Runtime = 1,
    Call = 2,
    Module = 3,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Runtime => "RivetRuntime",
            Kind::Call => "RivetCall",
            Kind::Module => "RivetModule",
        }
    }
}

// ------------------------------------------------------------------ handle table

#[derive(Default)]
struct Table {
    generation: u64,
    runtimes: HashMap<Handle, Arc<FfiRuntime>>,
    calls: HashMap<Handle, Arc<FfiCall>>,
    modules: HashMap<Handle, Arc<FfiModule>>,
    /// Addresses of strings handed to C and not yet freed.
    strings: HashSet<usize>,
}

static TABLE: LazyLock<Mutex<Table>> = LazyLock::new(|| Mutex::new(Table::default()));

fn table() -> MutexGuard<'static, Table> {
    TABLE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Mint a never-reused token: `(generation << 2) | kind` (never 0 = NULL).
fn mint(t: &mut Table, kind: Kind) -> Handle {
    t.generation += 1;
    (t.generation << 2) | kind as u64
}

fn kind_of(h: Handle) -> Option<Kind> {
    match h & 3 {
        1 => Some(Kind::Runtime),
        2 => Some(Kind::Call),
        3 => Some(Kind::Module),
        _ => None,
    }
}

/// `validation.ffi_argument` for a NULL, unknown, freed or mistyped handle.
fn bad_handle(h: Handle, want: Kind) -> RivetError {
    let why = if h == 0 {
        format!("the {} is NULL", want.name())
    } else if kind_of(h) != Some(want) {
        format!("the pointer is not a {} handle", want.name())
    } else {
        format!("unknown or already freed {} handle", want.name())
    };
    ffi_argument(want.name(), why)
}

fn runtime_of(h: Handle) -> RivetResult<Arc<FfiRuntime>> {
    let rt = table()
        .runtimes
        .get(&h)
        .cloned()
        .ok_or_else(|| bad_handle(h, Kind::Runtime))?;
    Ok(rt)
}

fn call_of(h: Handle) -> RivetResult<Arc<FfiCall>> {
    table()
        .calls
        .get(&h)
        .cloned()
        .ok_or_else(|| bad_handle(h, Kind::Call))
}

fn module_of(h: Handle) -> RivetResult<Arc<FfiModule>> {
    table()
        .modules
        .get(&h)
        .cloned()
        .ok_or_else(|| bad_handle(h, Kind::Module))
}

/// Record a string handed to C (its address), so `rivet_string_free` can
/// refuse a pointer it never produced or already freed.
pub fn track_string(addr: usize) {
    table().strings.insert(addr);
}

/// Forget a string before the shim frees it; `false` when the address is not
/// a live Rivet string (double free, foreign or static pointer): do not free.
pub fn untrack_string(addr: usize) -> bool {
    table().strings.remove(&addr)
}

// ------------------------------------------------------------------ guards

/// The error envelope every failed entry point returns (compact).
fn error_json(operation: Option<&str>, e: &RivetError, pretty: bool) -> String {
    ResponseEnvelope::from_error(operation, e).render(OutputFormat::pretty(pretty))
}

fn panic_error(what: &str, payload: &(dyn std::any::Any + Send)) -> RivetError {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".into());
    RivetError::new(
        ErrorKind::Internal,
        INTERNAL_PANIC,
        format!("{what} panicked inside librivet: {detail}"),
    )
}

/// Run one entry point that returns text; a panic becomes an `internal.panic`
/// envelope instead of unwinding into C.
pub fn guard_text(what: &str, f: impl FnOnce() -> String) -> String {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(s) => s,
        Err(p) => error_json(None, &panic_error(what, p.as_ref()), false),
    }
}

/// Run one entry point that returns an optional text (NULL = `None`).
pub fn guard_opt_text(what: &str, f: impl FnOnce() -> Option<String>) -> Option<String> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(s) => s,
        Err(p) => Some(error_json(None, &panic_error(what, p.as_ref()), false)),
    }
}

/// Run one entry point that returns a status and maybe an error envelope.
pub fn guard_status<T>(what: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => Err(error_json(None, &panic_error(what, p.as_ref()), false)),
    }
}

/// The error envelope for a NULL out-parameter (`&rt`, `&module`).
pub fn out_param_error(name: &str) -> String {
    error_json(
        None,
        &ffi_argument(name, format!("the out-parameter `{name}` is NULL")),
        false,
    )
}

/// A required C string argument: NULL or invalid UTF-8 is `validation.ffi_argument`.
fn arg<'a>(bytes: Option<&'a [u8]>, name: &str) -> RivetResult<&'a str> {
    let b = bytes.ok_or_else(|| ffi_argument(name, format!("`{name}` is NULL")))?;
    std::str::from_utf8(b)
        .map_err(|e| ffi_argument(name, format!("`{name}` is not valid UTF-8: {e}")))
}

/// An optional C string argument (NULL = `None`).
fn opt_arg<'a>(bytes: Option<&'a [u8]>, name: &str) -> RivetResult<Option<&'a str>> {
    match bytes {
        None => Ok(None),
        some => arg(some, name).map(Some),
    }
}

/// `data_json` of a module call or a send: NULL means `{}`.
fn data_arg(bytes: Option<&[u8]>, name: &str) -> RivetResult<serde_json::Value> {
    match opt_arg(bytes, name)? {
        None => Ok(serde_json::json!({})),
        Some(t) => serde_json::from_str(t)
            .map_err(|e| ffi_argument(name, format!("`{name}` is not valid JSON: {e}"))),
    }
}

// ------------------------------------------------------------------ runtime handle

/// A runtime handle: the loaded bundle plus the tokio runtime that drives it
/// (C threads block on it; `RivetRuntime` is thread-safe).
pub struct FfiRuntime {
    rt: Runtime,
    pretty: bool,
    closed: AtomicBool,
    tokio: Option<tokio::runtime::Runtime>,
}

impl FfiRuntime {
    fn block<F: std::future::Future>(&self, f: F) -> F::Output {
        match &self.tokio {
            Some(t) => t.block_on(f),
            None => unreachable!("the tokio runtime lives as long as the handle"),
        }
    }

    fn format(&self, pretty: Option<bool>) -> OutputFormat {
        OutputFormat::pretty(pretty.unwrap_or(self.pretty))
    }

    fn check_open(&self) -> RivetResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ffi_argument(
                "RivetRuntime",
                "the runtime was freed (rivet_runtime_free)",
            ));
        }
        Ok(())
    }
}

impl Drop for FfiRuntime {
    fn drop(&mut self) {
        // Tasks were drained by rivet_runtime_free; never block in a drop.
        if let Some(t) = self.tokio.take() {
            t.shutdown_background();
        }
    }
}

fn build_runtime(options: &FfiOptions) -> RivetResult<FfiRuntime> {
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("rivet-ffi")
        .build()
        .map_err(|e| RivetError::internal(format!("cannot start the async runtime: {e}")))?;
    let rt = {
        let _enter = tokio.enter();
        let mut b = Runtime::builder();
        if let Some(f) = &options.file {
            b = b.file(f);
        } else if let Some(src) = &options.source {
            b = b.source(
                options.path.as_deref().unwrap_or("app.rivet"),
                src,
                options.root.as_deref().unwrap_or("."),
            );
        } else if let Some(root) = &options.root {
            b = b.root(root);
        }
        if let Some(p) = &options.policy_file {
            b = b.policy_file(p);
        }
        if let Some(j) = &options.policy_json {
            b = b.policy(Policy::from_json(j.to_string().as_bytes())?);
        }
        if let Some(j) = &options.ceiling_json {
            b = b.ceiling(Policy::from_json(j.to_string().as_bytes())?);
        }
        b.build()?
    };
    Ok(FfiRuntime {
        rt,
        pretty: options.pretty,
        closed: AtomicBool::new(false),
        tokio: Some(tokio),
    })
}

/// `rivet_runtime_new(options_json, &rt, &error_json)`: `Ok(handle)` or an
/// error envelope (`validation.ffi_argument`, a compile or policy error, …).
pub fn runtime_new(options_json: Option<&[u8]>) -> Result<Handle, String> {
    let options = arg(options_json, "options_json")
        .and_then(FfiOptions::parse)
        .map_err(|e| error_json(None, &e, false))?;
    let rt = build_runtime(&options).map_err(|e| error_json(None, &e, options.pretty))?;
    let mut t = table();
    let h = mint(&mut t, Kind::Runtime);
    t.runtimes.insert(h, Arc::new(rt));
    Ok(h)
}

/// `rivet_runtime_free(rt)`: cancel and drain in-flight calls within the
/// grace, then release the handle. Call and module handles still alive keep
/// answering with error envelopes until they are freed.
pub fn runtime_free(h: Handle) -> i32 {
    let Some(rt) = table().runtimes.remove(&h) else {
        return RIVET_ERROR;
    };
    rt.closed.store(true, Ordering::SeqCst);
    rt.block(rt.rt.shutdown(GRACE));
    RIVET_OK
}

/// `rivet_request(rt, input_json)`: blocking; always one envelope.
pub fn request(h: Handle, input_json: Option<&[u8]>) -> String {
    let rt = match runtime_of(h).and_then(|rt| rt.check_open().map(|_| rt)) {
        Ok(rt) => rt,
        Err(e) => return error_json(None, &e, false),
    };
    let text = match arg(input_json, "input_json") {
        Ok(t) => t,
        Err(e) => return error_json(None, &e, rt.pretty),
    };
    let input = match parse_input_text(text) {
        Ok(i) => i,
        Err((hint, e)) => return error_json(hint.as_deref(), &e, rt.pretty),
    };
    let format = rt.format(input.pretty);
    rt.block(rt.rt.call(input)).render(format)
}

/// Input-envelope JSON text → `serve.parse_input`; on failure the operation
/// named in the body (if any) for the error envelope.
fn parse_input_text(
    text: &str,
) -> Result<crate::domain::envelope::InputEnvelope, (Option<String>, RivetError)> {
    let body: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        (
            None,
            RivetError::validation(
                INPUT_ENVELOPE,
                format!("the input envelope is not valid JSON: {e}"),
            ),
        )
    })?;
    let hint = body
        .get("operation")
        .or_else(|| body.get("id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    parse_input(RawInput::new(body)).map_err(|e| (hint, e))
}

// ------------------------------------------------------------------ call handle

/// One call handle: a session of the shared session driver owned by the
/// handle (never counted against the per-principal session limit).
pub struct FfiCall {
    rt: Arc<FfiRuntime>,
    operation: String,
    format: OutputFormat,
    state: Mutex<CallState>,
}

#[derive(Default)]
struct CallState {
    session_id: Option<String>,
    request_id: Option<String>,
    next_send_seq: u64,
    /// Highest session seq read so far (acknowledged on the next read).
    last_seq: u64,
    /// Records read but not yet returned, with whether each is terminal.
    pending: VecDeque<(String, bool)>,
    /// The terminal record has been read from the session.
    terminal_read: bool,
    /// The terminal record has been returned: next() answers NULL.
    done: bool,
}

impl FfiCall {
    fn state(&self) -> MutexGuard<'_, CallState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A call whose start failed: its only record is the error envelope.
    fn failed(rt: Arc<FfiRuntime>, operation: Option<&str>, e: &RivetError) -> FfiCall {
        let format = rt.format(None);
        let mut st = CallState::default();
        st.pending.push_back((
            ResponseEnvelope::from_error(operation, e).render(format),
            true,
        ));
        st.terminal_read = true;
        FfiCall {
            rt,
            operation: operation.unwrap_or_default().to_string(),
            format,
            state: Mutex::new(st),
        }
    }
}

fn insert_call(call: FfiCall) -> Handle {
    let mut t = table();
    let h = mint(&mut t, Kind::Call);
    t.calls.insert(h, Arc::new(call));
    h
}

/// Open the session behind a call handle (`sessions.open_session`).
fn open_call(
    rt: Arc<FfiRuntime>,
    operation: &str,
    data: Value,
    deadline_ms: Option<u64>,
    restrict: Option<Value>,
    pretty: Option<bool>,
    aliases: &[String],
) -> FfiCall {
    let opened = rt.block(rt.rt.open_session(SessionOpenInput {
        id: operation.to_string(),
        params: data,
        principal: Principal::local(),
        connection_owned: true,
        deadline_ms,
        trace: None,
        restrict,
    }));
    match opened {
        Ok(receipt) => {
            rt.rt
                .note_deprecated_input(&receipt.request_id, &receipt.trace_id, operation, aliases);
            let format = rt.format(pretty);
            FfiCall {
                rt,
                operation: operation.to_string(),
                format,
                state: Mutex::new(CallState {
                    session_id: Some(receipt.session_id),
                    request_id: Some(receipt.request_id),
                    next_send_seq: receipt.next_send_seq,
                    ..CallState::default()
                }),
            }
        }
        Err(e) => FfiCall::failed(rt, Some(operation), &e),
    }
}

/// `rivet_call_start(rt, input_json)`: never NULL for a live runtime; a
/// failed start yields its error record on the first `rivet_call_next`.
pub fn call_start(h: Handle, input_json: Option<&[u8]>) -> Handle {
    let rt = match runtime_of(h).and_then(|rt| rt.check_open().map(|_| rt)) {
        Ok(rt) => rt,
        // No runtime to attach to: a detached failed call still reports it.
        Err(e) => return insert_call(detached_failure(&e)),
    };
    let text = match arg(input_json, "input_json") {
        Ok(t) => t,
        Err(e) => return insert_call(FfiCall::failed(rt, None, &e)),
    };
    match parse_input_text(text) {
        Ok(input) => insert_call(open_call(
            rt,
            &input.operation,
            input.data,
            input.deadline_ms,
            input.restrict,
            input.pretty,
            &input.aliases,
        )),
        Err((hint, e)) => insert_call(FfiCall::failed(rt, hint.as_deref(), &e)),
    }
}

/// A failed call that is not attached to a runtime (NULL or freed `rt`).
fn detached_failure(e: &RivetError) -> FfiCall {
    // A tiny current-thread runtime keeps FfiRuntime's invariant (it is never used).
    let tokio = tokio::runtime::Builder::new_current_thread().build().ok();
    let shell = Runtime::builder().root(".");
    let rt = tokio
        .as_ref()
        .and_then(|t| {
            let _g = t.enter();
            shell.build().ok()
        })
        .map(|rt| FfiRuntime {
            rt,
            pretty: false,
            closed: AtomicBool::new(true),
            tokio,
        });
    match rt {
        Some(rt) => FfiCall::failed(Arc::new(rt), None, e),
        None => unreachable!("a root-only runtime always builds"),
    }
}

/// `rivet_call_next(call, timeout_ms)`: the next record, a timeout marker, or
/// `None` (NULL) once the terminal record was returned.
pub fn call_next(h: Handle, timeout_ms: i64) -> Option<String> {
    let call = match call_of(h) {
        Ok(c) => c,
        Err(e) => return Some(error_json(None, &e, false)),
    };
    let mut st = call.state();
    if let Some((rec, terminal)) = st.pending.pop_front() {
        st.done |= terminal;
        return Some(rec);
    }
    if st.done || st.terminal_read {
        st.done = true;
        return None;
    }
    let Some(session_id) = st.session_id.clone() else {
        st.done = true;
        return None;
    };
    let started = Instant::now();
    let limit = (timeout_ms >= 0).then(|| Duration::from_millis(timeout_ms as u64));
    loop {
        let wait = match limit {
            Some(l) => l.saturating_sub(started.elapsed()),
            None => Duration::from_secs(5),
        };
        let wait_ms = wait.as_millis().min(5_000) as u32;
        let read = call.rt.block(call.rt.rt.read_events(SessionReadInput {
            session_id: session_id.clone(),
            after_seq: st.last_seq,
            max_events: None,
            wait_ms: Some(wait_ms),
            principal: Principal::local(),
        }));
        match read {
            Ok(batch) => {
                for ev in &batch.events {
                    st.last_seq = st.last_seq.max(ev.seq);
                    let terminal = ev.is_terminal();
                    st.terminal_read |= terminal;
                    st.pending
                        .push_back((ev.record().render(call.format), terminal));
                }
            }
            Err(e) => {
                st.terminal_read = true;
                st.done = true;
                return Some(error_json(Some(&call.operation), &e, false));
            }
        }
        if let Some((rec, terminal)) = st.pending.pop_front() {
            st.done |= terminal;
            return Some(rec);
        }
        if let Some(l) = limit
            && started.elapsed() >= l
        {
            return Some(r#"{"type":"timeout"}"#.to_string());
        }
    }
}

/// `rivet_call_send(call, data_json)`: the session ack, or an error envelope.
pub fn call_send(h: Handle, data_json: Option<&[u8]>) -> String {
    let call = match call_of(h) {
        Ok(c) => c,
        Err(e) => return error_json(None, &e, false),
    };
    let data = match arg(data_json, "data_json").and_then(|t| {
        serde_json::from_str::<serde_json::Value>(t)
            .map_err(|e| ffi_argument("data_json", format!("`data_json` is not valid JSON: {e}")))
    }) {
        Ok(d) => d,
        Err(e) => return error_json(Some(&call.operation), &e, false),
    };
    let mut st = call.state();
    let Some(session_id) = st.session_id.clone() else {
        return error_json(Some(&call.operation), &not_running(), false);
    };
    let sent = call.rt.block(call.rt.rt.send_input(SessionSendInput {
        session_id,
        send_seq: st.next_send_seq,
        data: Value::from_json(&data),
        principal: Principal::local(),
    }));
    match sent {
        Ok(ack) => {
            st.next_send_seq += 1;
            ack.to_json().to_string()
        }
        Err(e) => error_json(Some(&call.operation), &e, false),
    }
}

fn not_running() -> RivetError {
    ffi_argument(
        "RivetCall",
        "the call did not start (read its error with rivet_call_next)",
    )
}

/// `rivet_call_finish_input(call)`: close the live input; ack or error envelope.
pub fn call_finish_input(h: Handle) -> String {
    let call = match call_of(h) {
        Ok(c) => c,
        Err(e) => return error_json(None, &e, false),
    };
    let Some(session_id) = call.state().session_id.clone() else {
        return error_json(Some(&call.operation), &not_running(), false);
    };
    match call.rt.block(call.rt.rt.finish_input(SessionRef {
        session_id,
        principal: Principal::local(),
    })) {
        Ok(ack) => ack.to_json().to_string(),
        Err(e) => error_json(Some(&call.operation), &e, false),
    }
}

/// Cancel the session (`sessions.cancel_session`) and signal its request
/// (`execution.cancel_request`); both are idempotent.
fn cancel_call(call: &FfiCall) {
    let (session_id, request_id) = {
        let st = call.state();
        (st.session_id.clone(), st.request_id.clone())
    };
    if let Some(session_id) = session_id {
        let _ = call.rt.block(call.rt.rt.cancel_session(SessionRef {
            session_id,
            principal: Principal::local(),
        }));
    }
    if let Some(request_id) = request_id {
        let _ = call.rt.rt.cancel(&request_id, Principal::local());
    }
}

/// `rivet_call_cancel(call)`: the next records end with `status: cancelled`.
pub fn call_cancel(h: Handle) -> i32 {
    match call_of(h) {
        Ok(call) => {
            cancel_call(&call);
            RIVET_OK
        }
        Err(_) => RIVET_ERROR,
    }
}

/// `rivet_call_free(call)`: a running call is cancelled and drained within
/// the grace first; a second free is refused (RIVET_ERROR), never UB.
pub fn call_free(h: Handle) -> i32 {
    let Some(call) = table().calls.remove(&h) else {
        return RIVET_ERROR;
    };
    let running = {
        let st = call.state();
        !st.terminal_read && st.session_id.is_some()
    };
    if running && !call.rt.closed.load(Ordering::SeqCst) {
        cancel_call(&call);
        let until = Instant::now() + GRACE;
        while Instant::now() < until {
            let left = until.saturating_duration_since(Instant::now()).as_millis() as i64;
            if call_next_on(&call, left).is_none() {
                break;
            }
        }
    }
    RIVET_OK
}

/// `call_next` on a handle already removed from the table (free's drain).
fn call_next_on(call: &Arc<FfiCall>, timeout_ms: i64) -> Option<String> {
    let mut t = table();
    let h = mint(&mut t, Kind::Call);
    t.calls.insert(h, Arc::clone(call));
    drop(t);
    let out = call_next(h, timeout_ms);
    table().calls.remove(&h);
    match out {
        Some(r) if r == r#"{"type":"timeout"}"# => None,
        other => other,
    }
}

// ------------------------------------------------------------------ module handle

/// A loaded module (`Runtime::load`): the module stays loaded in its runtime
/// after the handle is freed.
pub struct FfiModule {
    rt: Arc<FfiRuntime>,
    module: super::setup_library::Module,
}

/// `rivet_load(rt, path, alias_or_null, &module, &error_json)`.
pub fn load(h: Handle, path: Option<&[u8]>, alias: Option<&[u8]>) -> Result<Handle, String> {
    let rt = runtime_of(h)
        .and_then(|rt| rt.check_open().map(|_| rt))
        .map_err(|e| error_json(None, &e, false))?;
    let path = arg(path, "path").map_err(|e| error_json(None, &e, rt.pretty))?;
    let alias = opt_arg(alias, "alias").map_err(|e| error_json(None, &e, rt.pretty))?;
    let loaded = {
        let _g = rt.tokio.as_ref().map(|t| t.enter());
        match alias {
            Some(a) => rt.rt.load_as(path, a),
            None => rt.rt.load(path),
        }
    };
    let module = loaded.map_err(|e| error_json(Some("rivet.load"), &e, rt.pretty))?;
    let mut t = table();
    let mh = mint(&mut t, Kind::Module);
    t.modules.insert(mh, Arc::new(FfiModule { rt, module }));
    Ok(mh)
}

/// `rivet_module_operations(module)`: a JSON array of the module's operations
/// (short IDs), or an error envelope.
pub fn module_operations(h: Handle) -> String {
    let m = match module_of(h) {
        Ok(m) => m,
        Err(e) => return error_json(None, &e, false),
    };
    let ops: Vec<serde_json::Value> = m
        .module
        .operations()
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "operation": m.module.qualified(&e.id),
                "name": e.name,
                "description": e.description,
                "emits": e.emits.is_some(),
                "receives": e.receives.is_some(),
            })
        })
        .collect();
    let j = serde_json::Value::Array(ops);
    if m.rt.pretty {
        serde_json::to_string_pretty(&j).unwrap_or_default()
    } else {
        j.to_string()
    }
}

/// `rivet_module_call(module, id, data_json)`: blocking; one envelope whose
/// `operation` is the namespaced ID (`users.get`).
pub fn module_call(h: Handle, id: Option<&[u8]>, data_json: Option<&[u8]>) -> String {
    let m = match module_of(h) {
        Ok(m) => m,
        Err(e) => return error_json(None, &e, false),
    };
    if let Err(e) = m.rt.check_open() {
        return error_json(None, &e, false);
    }
    let id = match arg(id, "id") {
        Ok(i) => i,
        Err(e) => return error_json(None, &e, m.rt.pretty),
    };
    let qualified = m.module.qualified(id);
    let data = match data_arg(data_json, "data_json") {
        Ok(d) => d,
        Err(e) => return error_json(Some(&qualified), &e, m.rt.pretty),
    };
    m.rt.block(m.module.call(id, data))
        .render(m.rt.format(None))
}

/// `rivet_module_call_start(module, id, data_json)`: a call handle on the
/// module's namespaced operation.
pub fn module_call_start(h: Handle, id: Option<&[u8]>, data_json: Option<&[u8]>) -> Handle {
    let m = match module_of(h) {
        Ok(m) => m,
        Err(e) => return insert_call(detached_failure(&e)),
    };
    if let Err(e) = m.rt.check_open() {
        return insert_call(FfiCall::failed(Arc::clone(&m.rt), None, &e));
    }
    let id = match arg(id, "id") {
        Ok(i) => i,
        Err(e) => return insert_call(FfiCall::failed(Arc::clone(&m.rt), None, &e)),
    };
    let qualified = m.module.qualified(id);
    match data_arg(data_json, "data_json") {
        Ok(d) => insert_call(open_call(
            Arc::clone(&m.rt),
            &qualified,
            Value::from_json(&d),
            None,
            None,
            None,
            &[],
        )),
        Err(e) => insert_call(FfiCall::failed(Arc::clone(&m.rt), Some(&qualified), &e)),
    }
}

/// `rivet_module_free(module)`: release the handle (the module stays loaded).
pub fn module_free(h: Handle) -> i32 {
    match table().modules.remove(&h) {
        Some(_) => RIVET_OK,
        None => RIVET_ERROR,
    }
}

// ------------------------------------------------------------------ highlight

/// `rivet_highlight(source, format)`: `json` (default), `html` or `ansi`. A
/// syntax error keeps the tokens before it; in json a final error-envelope
/// line reports it.
pub fn highlight(source: Option<&[u8]>, format: Option<&[u8]>) -> String {
    let operation = Some("rivet.highlight");
    let source = match arg(source, "source") {
        Ok(s) => s,
        Err(e) => return error_json(operation, &e, false),
    };
    let format = match opt_arg(format, "format") {
        Ok(None) | Ok(Some("json")) => HighlightFormat::Json,
        Ok(Some("html")) => HighlightFormat::Html,
        Ok(Some("ansi")) => HighlightFormat::Ansi,
        Ok(Some(other)) => {
            let e = ffi_argument(
                "format",
                format!("unknown format `{other}` (expected json, html or ansi)"),
            );
            return error_json(operation, &e, false);
        }
        Err(e) => return error_json(operation, &e, false),
    };
    match super::setup_library::highlight::highlight(source, format) {
        Ok(text) => text,
        Err((mut text, e)) => {
            if format == HighlightFormat::Json {
                text.push_str(&error_json(operation, &e, false));
                text.push('\n');
            }
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // vhco:test execution.request_operation -- FFI surface: a runtime handle answers rivet_request with an envelope; NULL, bad UTF-8, bad JSON and freed handles are error envelopes, never a crash
    #[test]
    fn request_guards_every_argument() {
        let h = runtime_new(Some(
            br#"{"source":"operation demo.add\n    param a integer required\n    param b integer required\n    output integer\n    return a + b\nend\n"}"#,
        ))
        .unwrap();
        let ok: serde_json::Value = serde_json::from_str(&request(
            h,
            Some(br#"{"operation":"demo.add","data":{"a":2,"b":3}}"#),
        ))
        .unwrap();
        assert_eq!(ok["status"], "ok");
        assert_eq!(ok["data"], 5);
        for (input, code) in [
            (None, "validation.ffi_argument"),
            (Some(&b"\xff\xfe"[..]), "validation.ffi_argument"),
            (Some(&b"{not json"[..]), "validation.input_envelope"),
        ] {
            let e: serde_json::Value = serde_json::from_str(&request(h, input)).unwrap();
            assert_eq!(e["status"], "error");
            assert_eq!(e["error"]["code"], code);
        }
        assert_eq!(runtime_free(h), RIVET_OK);
        assert_eq!(runtime_free(h), RIVET_ERROR);
        let e: serde_json::Value =
            serde_json::from_str(&request(h, Some(br#"{"operation":"demo.add"}"#))).unwrap();
        assert_eq!(e["error"]["code"], "validation.ffi_argument");
    }

    // vhco:test execution.request_operation -- FFI surface: a panic inside an entry point becomes an internal.panic envelope
    #[test]
    fn panics_become_envelopes() {
        let out = guard_text("probe", || panic!("boom"));
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["error"]["code"], "internal.panic");
        assert!(j["error"]["message"].as_str().unwrap().contains("boom"));
    }
}
