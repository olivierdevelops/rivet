//! HTTP surface (REST + SSE) on the shared serve listener.
//!
//! ```text
//!  POST /v1/request {operation, data, deadline_ms?, restrict?}   (0.1.0 {id, params}: Deprecation: true)
//!     ├─ serve.parse_input ─▶ InputEnvelope │ bad shape ─▶ 422 validation.input_envelope envelope
//!     ├─ Accept: text/event-stream (sse) ─▶ event: data records … one event: result record
//!     │                                     (?pretty=true on SSE ─▶ 400 validation.pretty_stream)
//!     └─ otherwise (http)                ─▶ 200 envelope status ok │ 422 stream.required for streaming ops │ error envelope
//!  GET /v1/operations[/{id}[/outputs]], GET /v1/io, POST /v1/policy/generate ─▶ envelopes whose data is the payload
//!  every JSON answer: ?pretty=true ─▶ same envelope, 2-space indented
//! ```

// vhco:surface http kind http calls execution/request_operation, registry/describe_operations, registry/inspect_outputs, serve/authenticate_principal, serve/authorize_operation, serve/parse_input, sessions/open_session, sessions/send_input, sessions/finish_input, sessions/read_events, sessions/cancel_session, audit/inspect_effects, policy/generate_policy, auth/begin_authorization, auth/complete_authorization, auth/credential_status, auth/disconnect_account, auth/cancel_authorization, audit/read_trace, connectors/invoke_mcp
// vhco:trigger http auth/begin_authorization = POST /v1/request {"operation":"rivet.auth.begin"}
// vhco:trigger http auth/complete_authorization = POST /v1/request {"operation":"rivet.auth.complete"}
// vhco:trigger http auth/credential_status = POST /v1/request {"operation":"rivet.auth.status"}
// vhco:trigger http auth/disconnect_account = POST /v1/request {"operation":"rivet.auth.disconnect"}
// vhco:trigger http auth/cancel_authorization = POST /v1/request {"operation":"rivet.auth.cancel"}
// vhco:trigger http execution/request_operation = POST /v1/request
// vhco:trigger http serve/parse_input = POST /v1/request {operation, data, deadline_ms?, restrict?} (id/params: deprecated aliases, Deprecation: true)
// vhco:trigger http registry/describe_operations = GET /v1/operations | GET /v1/operations/{id}
// vhco:trigger http registry/inspect_outputs = GET /v1/operations/{id}/outputs
// vhco:trigger http audit/inspect_effects = GET /v1/io?by=target&kind=file&check_policy=true
// vhco:trigger http policy/generate_policy = POST /v1/policy/generate
// vhco:trigger http audit/read_trace = POST /v1/request {"operation":"rivet.trace.show","data":{"request_id":"…"}} (local principal or explicit listing)
// vhco:trigger http connectors/invoke_mcp = POST /v1/request {"operation":"rivet.connectors.sync","data":{"name":"…","output":"./…"}} | POST /v1/request {"operation":"CONNECTOR.tools.NAME"}
// vhco:trigger http serve/authenticate_principal = Authorization: Bearer TOKEN on every route
// vhco:trigger http serve/authorize_operation = serve.principals check on every route
// vhco:trigger http sessions/open_session = POST /v1/request {"operation":"rivet.sessions.open"}
// vhco:trigger http sessions/send_input = POST /v1/request {"operation":"rivet.sessions.send"}
// vhco:trigger http sessions/finish_input = POST /v1/request {"operation":"rivet.sessions.finish_input"}
// vhco:trigger http sessions/read_events = POST /v1/request {"operation":"rivet.sessions.read"}
// vhco:trigger http sessions/cancel_session = POST /v1/request {"operation":"rivet.sessions.cancel"}
// vhco:api http execution/request_operation POST /v1/request -- invoke one operation; a JSON ResponseEnvelope (status ok, or error/cancelled with the registry HTTP status), or SSE records with Accept: text/event-stream; ?pretty=true indents JSON answers (refused on SSE: 400 validation.pretty_stream); a valid W3C traceparent header supplies the trace id, and every answer carries a traceparent header; the deprecated {id, params} body still works and adds Deprecation: true
// vhco:request { "operation": "string — operation ID (deprecated alias: id)", "data": "object — operation input, default {} (deprecated alias: params)", "deadline_ms": "int? — requested deadline, capped at 600000", "restrict": "{grants:[{capability, targets, access?}]}? — narrows this request's authority, never widens", "query": "pretty=true? — 2-space indented JSON", "headers": "traceparent? — W3C 00-<trace-id>-<parent-id>-<flags>" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "string", "type": "result", "status": "ok|error|cancelled", "data": "Value|null", "error": "{kind, code, message, retryable, details?, source?, hint?}|null", "effects": "none|committed|partial|unknown", "data_count": "int", "headers": "traceparent; Deprecation: true when id/params were used" }
// vhco:api http audit/inspect_effects GET /v1/io -- envelope (operation rivet.io) whose data is the bare I/O manifest (IoManifest JSON); format=table|markdown|csv (or report=true) returns the rendered IoReport {format, by, rendered, diagnostics, exit_code, manifest} as data instead; needs an explicit rivet.io listing for non-local principals; check_files is refused remotely
// vhco:request { "query": "by=operation|target|capability, kind=K, access=V,V, check_policy=bool, needs=bool, strict=bool, include_bootstrap=bool, ids=ID,ID, all=bool, trace=REQ, format=json|table|markdown|csv, report=bool, pretty=bool" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.io", "type": "result", "status": "ok", "data": "{bundle: FileDigest, policy: FileDigest|null, complete: bool, sites: EffectSite[], targets: TargetSummary[]}", "error": "null", "effects": "none", "data_count": "0" }
// vhco:api http policy/generate_policy POST /v1/policy/generate -- envelope (operation rivet.policy.generate) whose data is the least-privilege policy draft; never writes files
// vhco:request { "ids": "string[]", "all": "bool" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.policy.generate", "type": "result", "status": "ok", "data": "{policy: policy.json v1, review: EffectSite[], complete: bool}", "error": "null", "effects": "none", "data_count": "0" }
// vhco:api http registry/describe_operations GET /v1/operations -- envelope (operation rivet.list) whose data lists the authorized operation summaries
// vhco:request { "headers": "Authorization: Bearer TOKEN (when serve.auth is bearer)", "query": "pretty=true?" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.list", "type": "result", "status": "ok", "data": "{operations: [{id, name, description, streaming}], next_cursor: null}", "error": "null", "effects": "none", "data_count": "0" }
// vhco:api http registry/inspect_outputs GET /v1/operations/{id}/outputs -- envelope (operation rivet.outputs) whose data is the declared output, emits, receives and errors JSON Schema
// vhco:request { "path": "id — operation ID", "query": "pretty=true?" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.outputs", "type": "result", "status": "ok", "data": "{id, output: JSON Schema, emits: JSON Schema|null, receives: JSON Schema|null, errors: [{code, description}]}", "error": "null", "effects": "none", "data_count": "0" }

use super::builtins::visible;
use super::setup_serve::{
    ServeState, error_response, json_response, note_access, noted_pretty, payload_response,
    read_input, trace_context, with_traceparent,
};
use crate::domain::contracts::{Completion, DataEvent};
use crate::domain::envelope::{PRETTY_STREAM, ResponseEnvelope};
use crate::domain::ports::DataSink;
use crate::domain::{RivetError, RivetResult};
use crate::io::http::{catalog_json, sse_data, sse_error, sse_result, wants_sse};
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
    note_access(|n| n.operation = Some("rivet.list".into()));
    let who = match st.authenticate("http", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    match st.runtime.list() {
        Ok(c) => {
            let data = catalog_json(
                c.entries
                    .iter()
                    .filter(|e| visible(&st.runtime, &who, &e.id)),
            );
            payload_response(&st, &headers, who, "rivet.list", data)
        }
        Err(e) => error_response(&e),
    }
}

async fn describe(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    note_access(|n| n.operation = Some("rivet.describe".into()));
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
        Ok(c) => {
            let data = c.entries[0].describe_json();
            payload_response(&st, &headers, who, "rivet.describe", data)
        }
        Err(e) => error_response(&e),
    }
}

async fn outputs(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    note_access(|n| n.operation = Some("rivet.outputs".into()));
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
        Ok(r) if !r.is_empty() => {
            let data = r[0].to_json();
            payload_response(&st, &headers, who, "rivet.outputs", data)
        }
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
    // serve.parse_input: {operation, data, …} (or the deprecated {id, params}).
    let input = match read_input(&body) {
        Ok(i) => i,
        Err(e) => return error_response(&e),
    };
    if sse && noted_pretty() {
        // SSE records are one line each: pretty would break the framing.
        return error_response(&RivetError::validation(
            PRETTY_STREAM,
            "pretty JSON cannot be used with an event stream (Accept: text/event-stream); drop ?pretty=true",
        ));
    }
    let trace = trace_context(&headers);
    if !sse
        && !input.operation.starts_with("rivet.")
        && visible(&st.runtime, &who, &input.operation)
        && let Ok(c) = st.runtime.describe(std::slice::from_ref(&input.operation))
        && c.entries[0].streaming()
    {
        return error_response(&RivetError::validation(
            "stream.required",
            format!(
                "`{}` streams; use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open",
                input.operation
            ),
        ));
    }
    let mut req = st
        .runtime
        .new_request_traced(&input.operation, input.data, who, trace.as_ref());
    if let Some(ms) = input.deadline_ms {
        req.deadline_ms = ms;
    }
    // G31: `restrict` narrows this request only (validated in dispatch_request).
    req.restrict = input.restrict;
    let (rid, tid) = (req.request_id.clone(), req.trace_id.clone());
    st.runtime
        .note_deprecated_input(&rid, &tid, &input.operation, &input.aliases);
    if !sse {
        let r = match st.runtime.dispatch_request(req, None).await {
            Ok(c) => json_response(200, ResponseEnvelope::from_completion(&c).to_json()),
            Err(e) => error_response(&e),
        };
        return with_traceparent(r, &tid, &rid);
    }
    with_traceparent(sse_response(st, req).await, &tid, &rid)
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

/// When the SSE body is dropped (client disconnect) the request is cancelled
/// through its token (it closes its resources within the grace); the task is
/// aborted only if it is still running after the grace.
struct AbortOnDrop(
    tokio::task::JoinHandle<()>,
    crate::domain::cancel::CancelToken,
);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if self.0.is_finished() {
            return;
        }
        self.1
            .cancel(crate::domain::cancel::CancelReason::Cancelled);
        let task = self.0.abort_handle();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(6)).await;
            task.abort();
        });
    }
}

async fn sse_response(st: Arc<ServeState>, req: crate::domain::contracts::Request) -> Response {
    let (tx, mut rx) = mpsc::channel::<Msg>(16);
    let (rid, tid) = (req.request_id.clone(), req.trace_id.clone());
    let op = req.operation_id.clone();
    let sink: Arc<dyn DataSink> = Arc::new(ChannelSink(tx.clone()));
    let rt = st.runtime.clone();
    let token = req.cancel.clone();
    let task = tokio::spawn(async move {
        let out = rt.dispatch_request(req, Some(sink)).await;
        let _ = tx.send(Msg::Done(out)).await;
    });
    let guard = AbortOnDrop(task, token);
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
            Msg::Done(Err(e)) => sse_error(&rid, &tid, &op, &e, last_seq + 1),
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
    note_access(|n| n.operation = Some("rivet.io".into()));
    let mut params = crate::domain::Value::Object(Vec::new());
    for (k, v) in q {
        let value = match v.as_str() {
            "true" => crate::domain::Value::Bool(true),
            "false" => crate::domain::Value::Bool(false),
            _ => crate::domain::Value::Text(v),
        };
        params.set(&k, value);
    }
    // The result is the bare IoManifest unless format=table|markdown|csv
    // (or report=true) asked for the rendered report.
    let req =
        st.runtime
            .new_request_traced("rivet.io", params, who, trace_context(&headers).as_ref());
    let (rid, tid) = (req.request_id.clone(), req.trace_id.clone());
    let r = match st.runtime.dispatch_request(req, None).await {
        Ok(c) => json_response(200, ResponseEnvelope::from_completion(&c).to_json()),
        Err(e) => error_response(&e),
    };
    with_traceparent(r, &tid, &rid)
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
    note_access(|n| n.operation = Some("rivet.policy.generate".into()));
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
    let req = st.runtime.new_request_traced(
        "rivet.policy.generate",
        params,
        who,
        trace_context(&headers).as_ref(),
    );
    let (rid, tid) = (req.request_id.clone(), req.trace_id.clone());
    let r = match st.runtime.dispatch_request(req, None).await {
        Ok(c) => json_response(200, ResponseEnvelope::from_completion(&c).to_json()),
        Err(e) => error_response(&e),
    };
    with_traceparent(r, &tid, &rid)
}
