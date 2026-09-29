//! `rivet serve`: one listener for REST, SSE, polling, WebSocket and MCP, or MCP
//! over stdio. Builds each surface's routes, then runs `serve.start_serve` with
//! the axum listener adapter.
//!
//! ```text
//!  rivet --file app.rivet serve [--listen 127.0.0.1:8080] | --stdio
//!        │
//!        ├─ start_serve: refuse non-loopback + auth none (serve.auth_required, exit 2)
//!        ├─ bind once ─▶ mount http, sse, poll, ws, mcp (policy serve.surfaces; others 404)
//!        │              + GET /v1/health (loopback: open; otherwise authenticated per serve.auth)
//!        ├─ every request: authenticate_principal ─▶ authorize_operation ─▶ shared dispatcher
//!        │    ├─ every JSON answer is a ResponseEnvelope; `?pretty=true` re-renders it 2-space indented
//!        │    ├─ a deprecated input alias (id/params) adds `Deprecation: true` to the response
//!        │    └─ one access-log line on stderr: time surface method route principal operation status duration_ms
//!        │       [deprecated=1] (never params, bodies, queries or tokens); W3C traceparent accepted and emitted
//!        └─ SIGINT / SIGTERM ─▶ stop accepting ─▶ cancel in-flight requests and sessions
//!                              (each closes its resources within the 5 s grace) ─▶ exit 0
//! ```

// vhco:api http serve/start_serve GET /v1/health -- liveness: unauthenticated on a loopback bind, otherwise authenticated per serve.auth; a ResponseEnvelope (operation rivet.health) whose data carries the server version so a client can tell 0.2 servers apart
// vhco:request { "headers": "Authorization: Bearer TOKEN (non-loopback binds with bearer auth)", "query": "pretty=true? — 2-space indented JSON" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.health", "type": "result", "status": "ok", "data": "{status: ok, catalog_version: sha256:…, version: string}", "error": "null", "effects": "none", "data_count": "0" }

use super::runtime::Runtime;
use crate::domain::contracts::{Principal, TraceContext, rfc3339_millis, traceparent_header};
use crate::domain::envelope::{InputEnvelope, OutputFormat, RawInput, ResponseEnvelope};
use crate::domain::policy::ServePolicy;
use crate::domain::ports::Authenticator;
use crate::domain::serve::{AuthnInput, ServeReceipt, ServeStartInput};
use crate::domain::{RivetError, RivetResult};
use crate::features::serve::authenticate_principal::authenticate_principal;
use crate::features::serve::parse_input::parse_input;
use crate::features::serve::start_serve::start_serve;
use crate::infra::serve_listener::AxumListener;
use crate::io::http::{error_body, error_status, parse_json_body, wants_pretty};
use axum::Router;
use axum::extract::{ConnectInfo, MatchedPath, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value as Json, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Where access-log lines go (stderr unless a library host supplies a sink).
pub type AccessLogSink = Arc<dyn Fn(&str) + Send + Sync>;

/// How to serve.
#[derive(Clone, Default)]
pub struct ServeOptions {
    pub listen: Option<String>,
    pub stdio: bool,
    /// Library host authentication callback (replaces serve.auth).
    pub authenticator: Option<Arc<dyn Authenticator>>,
    /// Receives one JSON access-log line per request (default: stderr).
    pub access_log: Option<AccessLogSink>,
}

/// State shared by every mounted surface.
pub struct ServeState {
    pub runtime: Runtime,
    pub serve: ServePolicy,
    pub loopback: AtomicBool,
    pub authenticator: Option<Arc<dyn Authenticator>>,
    /// MCP session ID → principal name that initialized it.
    pub mcp_sessions: Mutex<HashMap<String, String>>,
}

tokio::task_local! {
    static ACCESS: Arc<Mutex<AccessNote>>;
}

/// What a handler learned about the request it served, for the access log.
#[derive(Clone, Debug, Default)]
pub struct AccessNote {
    pub surface: String,
    pub principal: Option<String>,
    pub operation: Option<String>,
    /// `?pretty=true` was on the request (JSON answers are re-rendered indented).
    pub pretty: bool,
    /// The input used a deprecated alias (`id`/`params`): `Deprecation: true` and `deprecated=1`.
    pub deprecated: bool,
}

/// Record the served operation (or refine the surface) for the access log.
pub fn note_access(update: impl FnOnce(&mut AccessNote)) {
    let _ = ACCESS.try_with(|n| {
        if let Ok(mut g) = n.lock() {
            update(&mut g);
        }
    });
}

/// The operation recorded for the request being served (errors name it).
pub fn noted_operation() -> Option<String> {
    ACCESS
        .try_with(|n| n.lock().ok().and_then(|g| g.operation.clone()))
        .ok()
        .flatten()
}

/// Whether the request being served asked for `?pretty=true`.
pub fn noted_pretty() -> bool {
    ACCESS
        .try_with(|n| n.lock().map(|g| g.pretty).unwrap_or(false))
        .unwrap_or(false)
}

/// Decode a request body into the one InputEnvelope (`serve.parse_input`). A
/// deprecated alias marks the answer (`Deprecation: true`, `deprecated=1`).
pub fn read_input(bytes: &[u8]) -> RivetResult<InputEnvelope> {
    let body = parse_json_body(bytes)?;
    let input = parse_input(RawInput::new(body))?;
    note_access(|n| n.operation = Some(input.operation.clone()));
    if input.is_legacy() {
        note_access(|n| n.deprecated = true);
    }
    Ok(input)
}

/// The caller's valid W3C `traceparent`, if any.
pub fn trace_context(headers: &HeaderMap) -> Option<TraceContext> {
    headers
        .get("traceparent")
        .and_then(|v| v.to_str().ok())
        .and_then(TraceContext::parse)
}

/// Emit `traceparent` for the request a response answers.
pub fn with_traceparent(mut r: Response, trace_id: &str, request_id: &str) -> Response {
    if !trace_id.is_empty()
        && let Ok(v) = HeaderValue::from_str(&traceparent_header(trace_id, request_id))
    {
        r.headers_mut().insert("traceparent", v);
    }
    r
}

/// Largest JSON answer re-rendered for `?pretty=true` (larger ones stay compact).
const PRETTY_LIMIT: usize = 64 * 1024 * 1024;

/// `?pretty=true`: re-render a JSON answer 2-space indented, keys in the same
/// order. Streams (SSE) never get here: the request handler refuses them.
async fn prettify(resp: Response) -> Response {
    let is_json = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.starts_with("application/json"));
    if !is_json {
        return resp;
    }
    let (mut parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, PRETTY_LIMIT).await {
        Ok(b) => b,
        Err(_) => return (parts, axum::body::Body::empty()).into_response(),
    };
    let text = match serde_json::from_slice::<Json>(&bytes) {
        Ok(j) => OutputFormat::Pretty.render(&j),
        Err(_) => return (parts, axum::body::Body::from(bytes)).into_response(),
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    (parts, axum::body::Body::from(text)).into_response()
}

/// One access-log line per request: `{time, surface, method, route, principal,
/// operation, status, duration_ms[, deprecated: 1]}`. The route is the matched
/// pattern (never the query string); params, bodies and credentials are never
/// logged. The same layer applies `?pretty=true` and the `Deprecation` header.
async fn access_log(
    State((surface, sink)): State<(&'static str, AccessLogSink)>,
    req: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = req.method().to_string();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());
    let note = Arc::new(Mutex::new(AccessNote {
        surface: surface.to_string(),
        pretty: wants_pretty(req.uri().query()),
        ..AccessNote::default()
    }));
    let mut resp = ACCESS.scope(Arc::clone(&note), next.run(req)).await;
    let n = note.lock().map(|g| g.clone()).unwrap_or_default();
    if n.deprecated {
        resp.headers_mut()
            .insert("deprecation", HeaderValue::from_static("true"));
    }
    if n.pretty {
        resp = prettify(resp).await;
    }
    let mut line = json!({
        "time": rfc3339_millis(SystemTime::now()),
        "surface": n.surface,
        "method": method,
        "route": route,
        "principal": n.principal,
        "operation": n.operation,
        "status": resp.status().as_u16(),
        "duration_ms": started.elapsed().as_millis() as u64,
    });
    if n.deprecated {
        line["deprecated"] = json!(1);
    }
    sink(&line.to_string());
    resp
}

/// Wrap one surface's routes with the access log.
fn logged(router: Router, surface: &'static str, sink: &AccessLogSink) -> Router {
    router.layer(axum::middleware::from_fn_with_state(
        (surface, Arc::clone(sink)),
        access_log,
    ))
}

/// `GET /v1/health` → envelope (operation `rivet.health`) with data
/// `{status:"ok", catalog_version, version}`.
async fn health(
    State(st): State<Arc<ServeState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    note_access(|n| n.operation = Some("rivet.health".into()));
    // Unauthenticated on a loopback bind; otherwise the usual serve.auth applies.
    let who = if st.loopback.load(Ordering::SeqCst) {
        Principal::local()
    } else {
        match st.authenticate("http", &headers, peer) {
            Ok(p) => p,
            Err(e) => return error_response(&e),
        }
    };
    let data = json!({
        "status": "ok",
        "catalog_version": st.runtime.catalog_version(),
        "version": env!("CARGO_PKG_VERSION"),
    });
    payload_response(&st, &headers, who, "rivet.health", data)
}

/// A 200 envelope for a payload produced without a dispatched run (GET
/// routes): IDs are minted like a request's so `traceparent` is emitted.
pub fn payload_response(
    st: &ServeState,
    headers: &HeaderMap,
    who: Principal,
    operation: &str,
    data: Json,
) -> Response {
    let req = st.runtime.new_request_traced(
        operation,
        crate::domain::Value::Object(Vec::new()),
        who,
        trace_context(headers).as_ref(),
    );
    let env = ResponseEnvelope::payload(&req.request_id, &req.trace_id, operation, data);
    with_traceparent(
        json_response(200, env.to_json()),
        &req.trace_id,
        &req.request_id,
    )
}

impl ServeState {
    pub fn new(runtime: Runtime, authenticator: Option<Arc<dyn Authenticator>>) -> ServeState {
        let serve = runtime.policy().serve.clone();
        ServeState {
            runtime,
            serve,
            loopback: AtomicBool::new(true),
            authenticator,
            mcp_sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn enabled(&self, surface: &str) -> bool {
        self.serve.surfaces.iter().any(|s| s == surface)
    }

    /// One authentication path for every surface.
    pub fn authenticate(
        &self,
        surface: &str,
        headers: &HeaderMap,
        remote: SocketAddr,
    ) -> Result<Principal, RivetError> {
        let who = authenticate_principal(
            &AuthnInput {
                surface: surface.to_string(),
                remote_addr: remote.to_string(),
                bind_is_loopback: self.loopback.load(Ordering::SeqCst),
                authorization: headers
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string),
                client_cert_subject: None,
                auth: self.serve.auth.clone(),
            },
            self.authenticator.as_deref(),
        );
        if let Ok(p) = &who {
            let name = p.name.clone();
            note_access(|n| {
                n.surface = surface.to_string();
                n.principal = Some(name);
            });
        }
        who
    }
}

/// JSON response with a status.
pub fn json_response(status: u16, body: Json) -> Response {
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// Error envelope with the registry status (401 adds `WWW-Authenticate`); it
/// names the operation the handler noted, when one is known.
pub fn error_response(e: &RivetError) -> Response {
    error_response_for(noted_operation().as_deref(), e)
}

/// Error envelope naming `operation` explicitly (`None` → `operation: null`):
/// for protocol-level refusals where no operation was requested (the access
/// log still records the route's note).
pub fn error_response_for(operation: Option<&str>, e: &RivetError) -> Response {
    let status = error_status(e);
    let mut r = json_response(status, error_body(operation, e));
    if status == 401 {
        r.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
    }
    r
}

async fn not_mounted() -> Response {
    json_response(
        404,
        error_body(
            None,
            &RivetError::not_found("not_found.route", "no such route on this listener"),
        ),
    )
}

/// A running listener.
pub struct ServeHandle {
    pub receipt: ServeReceipt,
    pub addr: Option<SocketAddr>,
    runtime: Option<Runtime>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<RivetResult<()>>>,
}

/// How long shutdown waits for in-flight work after cancelling it (the 5 s
/// cleanup grace plus a margin for responses to flush).
const DRAIN: Duration = Duration::from_secs(6);

impl ServeHandle {
    /// Graceful drain (SIGINT / SIGTERM): stop accepting, cancel every in-flight
    /// request and session (each closes its resources within the grace), let
    /// in-flight responses finish, then end the listener.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(rt) = self.runtime.take() {
            rt.shutdown(DRAIN).await;
        }
        if let Some(mut t) = self.task.take()
            && tokio::time::timeout(DRAIN, &mut t).await.is_err()
        {
            t.abort();
            let _ = t.await;
        }
    }
}

/// Start serving (network mode) or validate stdio mode. For `--stdio` the
/// caller then runs [`super::setup_mcp::run_stdio`].
pub async fn start(runtime: Runtime, opts: ServeOptions) -> RivetResult<ServeHandle> {
    let state = Arc::new(ServeState::new(runtime.clone(), opts.authenticator.clone()));
    let sink: AccessLogSink = opts
        .access_log
        .clone()
        .unwrap_or_else(|| Arc::new(|line: &str| eprintln!("{line}")));
    let http_on = state.enabled("http");
    let surfaces: Vec<(String, Router)> = vec![
        (
            "http".into(),
            logged(
                super::setup_http::routes(Arc::clone(&state), http_on),
                "http",
                &sink,
            ),
        ),
        (
            "sse".into(),
            if http_on {
                Router::new()
            } else {
                logged(
                    super::setup_http::request_route(Arc::clone(&state)),
                    "sse",
                    &sink,
                )
            },
        ),
        (
            "poll".into(),
            logged(super::setup_poll::routes(Arc::clone(&state)), "poll", &sink),
        ),
        (
            "ws".into(),
            logged(super::setup_ws::routes(Arc::clone(&state)), "ws", &sink),
        ),
        (
            "mcp".into(),
            logged(super::setup_mcp::routes(Arc::clone(&state)), "mcp", &sink),
        ),
    ];
    // Health and the 404 fallback are mounted whatever serve.surfaces says.
    let base = logged(
        Router::new()
            .route("/v1/health", get(health))
            .with_state(Arc::clone(&state))
            .fallback(not_mounted),
        "http",
        &sink,
    );
    let mut listener = AxumListener::new(surfaces, base);
    let receipt = start_serve(
        ServeStartInput {
            listen: opts.listen.clone(),
            stdio: opts.stdio,
            serve: state.serve.clone(),
            catalog_version: runtime.catalog_version(),
            policy_hash: runtime.policy().sha256.clone(),
        },
        &mut listener,
    )
    .await?;
    if receipt.stdio {
        return Ok(ServeHandle {
            receipt,
            addr: None,
            runtime: None,
            shutdown: None,
            task: None,
        });
    }
    let addr = listener.local_addr();
    state
        .loopback
        .store(addr.is_none_or(|a| a.ip().is_loopback()), Ordering::SeqCst);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(listener.run(async move {
        let _ = rx.await;
    }));
    Ok(ServeHandle {
        receipt,
        addr,
        runtime: Some(runtime),
        shutdown: Some(tx),
        task: Some(task),
    })
}

/// Resolves on SIGINT (Ctrl-C) or, on Unix, SIGTERM.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// `rivet serve` from the CLI; returns the process exit code.
pub async fn run_cli(runtime: Runtime, listen: &str, stdio: bool) -> Result<i32, RivetError> {
    let handle = start(
        runtime.clone(),
        ServeOptions {
            listen: Some(listen.to_string()),
            stdio,
            ..ServeOptions::default()
        },
    )
    .await?;
    eprintln!("{}", handle.receipt.to_json());
    if stdio {
        super::setup_mcp::run_stdio(runtime).await;
        return Ok(0);
    }
    // SIGTERM drains exactly like SIGINT and exits 0.
    shutdown_signal().await;
    handle.shutdown().await;
    Ok(0)
}
