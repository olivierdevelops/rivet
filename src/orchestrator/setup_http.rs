//! HTTP surface (REST + SSE) on the shared serve listener.
//!
//! ```text
//!  POST /v1/request {id, params}
//!     ├─ Accept: text/event-stream (sse) ─▶ data events … one result|error event
//!     └─ otherwise (http)                ─▶ 200 Completion │ 422 stream.required for streaming ops │ ErrorEnvelope
//!  GET /v1/operations[/{id}[/outputs]]   ─▶ catalog filtered by the principal's authorization
//! ```

// vhco:surface http kind http calls execution/request_operation, registry/describe_operations, registry/inspect_outputs, serve/authenticate_principal, serve/authorize_operation, sessions/open_session, sessions/send_input, sessions/finish_input, sessions/read_events, sessions/cancel_session, audit/inspect_effects, policy/generate_policy, auth/begin_authorization, auth/complete_authorization, auth/credential_status, auth/disconnect_account, auth/cancel_authorization, audit/read_trace, connectors/invoke_mcp
// vhco:trigger http auth/begin_authorization = POST /v1/request {"id":"rivet.auth.begin"}
// vhco:trigger http auth/complete_authorization = POST /v1/request {"id":"rivet.auth.complete"}
// vhco:trigger http auth/credential_status = POST /v1/request {"id":"rivet.auth.status"}
// vhco:trigger http auth/disconnect_account = POST /v1/request {"id":"rivet.auth.disconnect"}
// vhco:trigger http auth/cancel_authorization = POST /v1/request {"id":"rivet.auth.cancel"}
// vhco:trigger http execution/request_operation = POST /v1/request
// vhco:trigger http registry/describe_operations = GET /v1/operations | GET /v1/operations/{id}
// vhco:trigger http registry/inspect_outputs = GET /v1/operations/{id}/outputs
// vhco:trigger http audit/inspect_effects = GET /v1/io?by=target&kind=file&check_policy=true
// vhco:trigger http policy/generate_policy = POST /v1/policy/generate
// vhco:trigger http audit/read_trace = POST /v1/request {"id":"rivet.trace.show","params":{"request_id":"…"}} (local principal or explicit listing)
// vhco:trigger http connectors/invoke_mcp = POST /v1/request {"id":"rivet.connectors.sync","params":{"name":"…","output":"./…"}} | POST /v1/request {"id":"CONNECTOR.tools.NAME"}
// vhco:trigger http serve/authenticate_principal = Authorization: Bearer TOKEN on every route
// vhco:trigger http serve/authorize_operation = serve.principals check on every route
// vhco:trigger http sessions/open_session = POST /v1/request {"id":"rivet.sessions.open"}
// vhco:trigger http sessions/send_input = POST /v1/request {"id":"rivet.sessions.send"}
// vhco:trigger http sessions/finish_input = POST /v1/request {"id":"rivet.sessions.finish_input"}
// vhco:trigger http sessions/read_events = POST /v1/request {"id":"rivet.sessions.read"}
// vhco:trigger http sessions/cancel_session = POST /v1/request {"id":"rivet.sessions.cancel"}
// vhco:api http execution/request_operation POST /v1/request -- invoke one operation; JSON Completion, or SSE envelopes with Accept: text/event-stream
// vhco:request { "id": "string — operation ID", "params": "object", "deadline_ms": "int? — requested deadline, capped at 600000" }
// vhco:response { "request_id": "string", "trace_id": "string", "result": "Value", "data_count": "int", "effects": "none|committed|partial|unknown" }
// vhco:api http audit/inspect_effects GET /v1/io -- the I/O manifest (IoManifest JSON); needs an explicit rivet.io listing for non-local principals; check_files is refused remotely
// vhco:request { "query": "by=operation|target|capability, kind=K, access=V,V, check_policy=bool, needs=bool, strict=bool, include_bootstrap=bool, ids=ID,ID, all=bool, trace=REQ, format=json|table|markdown|csv" }
// vhco:response { "bundle": "FileDigest", "policy": "FileDigest|null", "complete": "bool", "sites": "EffectSite[]", "targets": "TargetSummary[]" }
// vhco:api http policy/generate_policy POST /v1/policy/generate -- least-privilege policy draft; never writes files
// vhco:request { "ids": "string[]", "all": "bool" }
// vhco:response { "policy": "policy.json v1", "review": "EffectSite[]", "complete": "bool" }
// vhco:api http registry/describe_operations GET /v1/operations -- authorized operation summaries
// vhco:request { "headers": "Authorization: Bearer TOKEN (when serve.auth is bearer)" }
// vhco:response { "operations": "[{id, name, description, streaming}]", "next_cursor": "null" }
// vhco:api http registry/inspect_outputs GET /v1/operations/{id}/outputs -- declared output, emits, receives and errors JSON Schema
// vhco:request { "path": "id — operation ID" }
// vhco:response { "id": "string", "output": "JSON Schema", "emits": "JSON Schema|null", "receives": "JSON Schema|null", "errors": "[{code, description}]" }

use super::builtins::visible;
use super::setup_serve::{ServeState, error_response, json_response};
use crate::domain::contracts::{Completion, DataEvent};
use crate::domain::ports::DataSink;
use crate::domain::{RivetError, RivetResult};
use crate::io::http::{
    catalog_json, parse_request_body, sse_data, sse_error, sse_result, wants_sse,
};
use async_trait::async_trait;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;

type St = State<Arc<ServeState>>;

/// REST routes; `/v1/request` is included when `with_request` (http enabled).
pub fn routes(state: Arc<ServeState>, with_request: bool) -> Router {
    let mut r = Router::new()
        .route("/v1/operations", get(list))
        .route("/v1/operations/{id}", get(describe))
        .route("/v1/operations/{id}/outputs", get(outputs))
        .route("/v1/io", get(io_manifest))
        .route("/v1/policy/generate", post(policy_generate));
    if with_request {
        r = r.route("/v1/request", post(request));
    }
    r.with_state(state)
}

/// `/v1/request` alone (sse enabled while http is disabled).
pub fn request_route(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/v1/request", post(request))
        .with_state(state)
}

fn not_mounted() -> Response {
    error_response(&RivetError::not_found(
        "not_found.route",
        "this surface is disabled in serve.surfaces",
    ))
}

async fn list(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let who = match st.authenticate("http", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    match st.runtime.list() {
        Ok(c) => json_response(
            200,
            catalog_json(
                c.entries
                    .iter()
                    .filter(|e| visible(&st.runtime, &who, &e.id)),
            ),
        ),
        Err(e) => error_response(&e),
    }
}

async fn describe(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let who = match st.authenticate("http", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    if !visible(&st.runtime, &who, &id) {
        return error_response(&RivetError::not_found(
            "not_found.operation",
            format!("no operation `{id}`"),
        ));
    }
    match st.runtime.describe(std::slice::from_ref(&id)) {
        Ok(c) => json_response(200, c.entries[0].describe_json()),
        Err(e) => error_response(&e),
    }
}

async fn outputs(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let who = match st.authenticate("http", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    if !visible(&st.runtime, &who, &id) {
        return error_response(&RivetError::not_found(
            "not_found.operation",
            format!("no operation `{id}`"),
        ));
    }
    match st.runtime.outputs(Some(&id), false) {
        Ok(r) if !r.is_empty() => json_response(200, r[0].to_json()),
        Ok(_) => error_response(&RivetError::not_found(
            "not_found.operation",
            format!("no operation `{id}`"),
        )),
        Err(e) => error_response(&e),
    }
}

async fn request(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let sse = wants_sse(headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()));
    let surface = if sse { "sse" } else { "http" };
    if !st.enabled(surface) {
        return not_mounted();
    }
    let who = match st.authenticate(surface, &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let body = match parse_request_body(&body) {
        Ok(b) => b,
        Err(e) => return error_response(&e),
    };
    if !sse
        && !body.id.starts_with("rivet.")
        && visible(&st.runtime, &who, &body.id)
        && let Ok(c) = st.runtime.describe(std::slice::from_ref(&body.id))
        && c.entries[0].streaming()
    {
        return error_response(&RivetError::validation(
            "stream.required",
            format!(
                "`{}` streams; use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open",
                body.id
            ),
        ));
    }
    let mut req = st.runtime.new_request(&body.id, body.params, who);
    if let Some(ms) = body.deadline_ms {
        req.deadline_ms = ms;
    }
    if !sse {
        return match st.runtime.dispatch_request(req, None).await {
            Ok(c) => json_response(200, c.to_json()),
            Err(e) => error_response(&e),
        };
    }
    sse_response(st, req).await
}

enum Msg {
    Data(DataEvent),
    Done(RivetResult<Completion>),
}

struct ChannelSink(mpsc::Sender<Msg>);

#[async_trait]
impl DataSink for ChannelSink {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        self.0.send(Msg::Data(event)).await.map_err(|_| {
            RivetError::new(
                crate::domain::ErrorKind::Cancelled,
                "cancelled.disconnect",
                "the SSE client disconnected",
            )
        })
    }
}

/// Aborts the request when the SSE body is dropped (client disconnect).
struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn sse_response(st: Arc<ServeState>, req: crate::domain::contracts::Request) -> Response {
    let (tx, mut rx) = mpsc::channel::<Msg>(16);
    let (rid, tid) = (req.request_id.clone(), req.trace_id.clone());
    let sink: Arc<dyn DataSink> = Arc::new(ChannelSink(tx.clone()));
    let rt = st.runtime.clone();
    let task = tokio::spawn(async move {
        let out = rt.dispatch_request(req, Some(sink)).await;
        let _ = tx.send(Msg::Done(out)).await;
    });
    let guard = AbortOnDrop(task);
    // Errors before the first data item keep their HTTP status (headers not sent yet).
    let first = rx.recv().await;
    if let Some(Msg::Done(Err(e))) = &first {
        return error_response(e);
    }
    let mut last_seq = 0u64;
    let mut encode = move |m: Msg| -> String {
        match m {
            Msg::Data(ev) => {
                last_seq = ev.seq;
                sse_data(&ev)
            }
            Msg::Done(Ok(c)) => sse_result(&c, last_seq + 1),
            Msg::Done(Err(e)) => sse_error(&rid, &tid, &e, last_seq + 1),
        }
    };
    let head = first.map(&mut encode);
    let stream = futures_util::stream::unfold(
        (head, Some(rx), guard, encode),
        |(head, rx, guard, mut encode)| async move {
            if let Some(chunk) = head {
                return Some((
                    Ok::<Bytes, std::convert::Infallible>(Bytes::from(chunk)),
                    (None, rx, guard, encode),
                ));
            }
            let mut rx = rx?;
            let m = rx.recv().await?;
            let terminal = matches!(m, Msg::Done(_));
            let chunk = encode(m);
            Some((
                Ok(Bytes::from(chunk)),
                (None, if terminal { None } else { Some(rx) }, guard, encode),
            ))
        },
    );
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// `GET /v1/io?...` → the `rivet.io` built-in for the authenticated principal.
async fn io_manifest(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let who = match st.authenticate("http", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let mut params = crate::domain::Value::Object(Vec::new());
    for (k, v) in q {
        let value = match v.as_str() {
            "true" => crate::domain::Value::Bool(true),
            "false" => crate::domain::Value::Bool(false),
            _ => crate::domain::Value::Text(v),
        };
        params.set(&k, value);
    }
    match st.runtime.request_as(who, "rivet.io", params, None).await {
        Ok(c) => json_response(200, c.result.to_json()),
        Err(e) => error_response(&e),
    }
}

/// `POST /v1/policy/generate {ids?, all?}` → the `rivet.policy.generate` built-in.
async fn policy_generate(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let who = match st.authenticate("http", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let params = if body.is_empty() {
        crate::domain::Value::Object(Vec::new())
    } else {
        match serde_json::from_slice::<serde_json::Value>(&body) {
            Ok(j) => crate::domain::Value::from_json(&j),
            Err(e) => {
                return error_response(&RivetError::validation(
                    "validation.body",
                    format!("request body is not JSON: {e}"),
                ));
            }
        }
    };
    match st
        .runtime
        .request_as(who, "rivet.policy.generate", params, None)
        .await
    {
        Ok(c) => json_response(200, c.result.to_json()),
        Err(e) => error_response(&e),
    }
}
