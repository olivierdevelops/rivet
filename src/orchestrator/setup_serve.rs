//! `rivet serve`: one listener for REST, SSE, polling, WebSocket and MCP, or MCP
//! over stdio. Builds each surface's routes, then runs `serve.start_serve` with
//! the axum listener adapter.
//!
//! ```text
//!  rivet --file app.rivet serve [--listen 127.0.0.1:8080] | --stdio
//!        │
//!        ├─ start_serve: refuse non-loopback + auth none (serve.auth_required, exit 2)
//!        ├─ bind once ─▶ mount http, sse, poll, ws, mcp (policy serve.surfaces; others 404)
//!        └─ every request: authenticate_principal ─▶ authorize_operation ─▶ shared dispatcher
//! ```

use super::runtime::Runtime;
use crate::domain::contracts::{Principal, error_envelope};
use crate::domain::policy::ServePolicy;
use crate::domain::ports::Authenticator;
use crate::domain::serve::{AuthnInput, ServeReceipt, ServeStartInput};
use crate::domain::{RivetError, RivetResult};
use crate::features::serve::authenticate_principal::authenticate_principal;
use crate::features::serve::start_serve::start_serve;
use crate::infra::serve_listener::AxumListener;
use crate::io::http::{error_body, error_status};
use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// How to serve.
#[derive(Clone, Default)]
pub struct ServeOptions {
    pub listen: Option<String>,
    pub stdio: bool,
    /// Library host authentication callback (replaces serve.auth).
    pub authenticator: Option<Arc<dyn Authenticator>>,
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
        authenticate_principal(
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
        )
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

/// ErrorEnvelope with the registry status (401 adds `WWW-Authenticate`).
pub fn error_response(e: &RivetError) -> Response {
    let status = error_status(e);
    let mut r = json_response(status, error_body(e));
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
        error_envelope(
            "",
            "",
            &RivetError::not_found("not_found.route", "no such route on this listener"),
        ),
    )
}

/// A running listener.
pub struct ServeHandle {
    pub receipt: ServeReceipt,
    pub addr: Option<SocketAddr>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<RivetResult<()>>>,
}

impl ServeHandle {
    /// Stop accepting and let the listener task end (open WebSockets close with the process).
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.task.take() {
            t.abort();
            let _ = t.await;
        }
    }
}

/// Start serving (network mode) or validate stdio mode. For `--stdio` the
/// caller then runs [`super::setup_mcp::run_stdio`].
pub async fn start(runtime: Runtime, opts: ServeOptions) -> RivetResult<ServeHandle> {
    let state = Arc::new(ServeState::new(runtime.clone(), opts.authenticator.clone()));
    let http_on = state.enabled("http");
    let surfaces: Vec<(String, Router)> = vec![
        (
            "http".into(),
            super::setup_http::routes(Arc::clone(&state), http_on),
        ),
        (
            "sse".into(),
            if http_on {
                Router::new()
            } else {
                super::setup_http::request_route(Arc::clone(&state))
            },
        ),
        ("poll".into(), super::setup_poll::routes(Arc::clone(&state))),
        ("ws".into(), super::setup_ws::routes(Arc::clone(&state))),
        ("mcp".into(), super::setup_mcp::routes(Arc::clone(&state))),
    ];
    let mut listener = AxumListener::new(surfaces, Router::new().fallback(not_mounted));
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
        shutdown: Some(tx),
        task: Some(task),
    })
}

/// `rivet serve` from the CLI; returns the process exit code.
pub async fn run_cli(runtime: Runtime, listen: &str, stdio: bool) -> Result<i32, RivetError> {
    let handle = start(
        runtime.clone(),
        ServeOptions {
            listen: Some(listen.to_string()),
            stdio,
            authenticator: None,
        },
    )
    .await?;
    eprintln!("{}", handle.receipt.to_json());
    if stdio {
        super::setup_mcp::run_stdio(runtime).await;
        return Ok(0);
    }
    let _ = tokio::signal::ctrl_c().await;
    handle.shutdown().await;
    Ok(0)
}
