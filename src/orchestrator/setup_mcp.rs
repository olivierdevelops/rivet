//! MCP surface (protocol 2025-11-25): Streamable HTTP at `/mcp` on the serve
//! listener and `rivet serve --stdio`. Direct named tools are canonical; the
//! built-in generic tools are listed beside them.
//!
//! ```text
//!  POST /mcp initialize ─▶ 200 + MCP-Session-Id      notifications/* ─▶ 202
//!  POST /mcp tools/list  ─▶ direct tools (authorized, public) + every built-in the principal may call
//!                          (rivet.request/list/describe/outputs/sessions.*/io/policy.generate/
//!                           trace.show/connectors.sync/auth.*), each with its schemas
//!  POST /mcp tools/call  ─▶ unary: Completion │ streaming: SessionReceipt │ failure: isError + ErrorEnvelope
//!  GET /mcp ─▶ 405        DELETE /mcp ─▶ 204 (unknown session 404)
//! ```

// vhco:surface mcp kind mcp calls execution/request_operation, registry/describe_operations, registry/inspect_outputs, sessions/open_session, sessions/send_input, sessions/finish_input, sessions/read_events, sessions/cancel_session, serve/authenticate_principal, serve/authorize_operation, auth/begin_authorization, auth/complete_authorization, auth/credential_status, auth/disconnect_account, auth/cancel_authorization
// vhco:trigger mcp auth/begin_authorization = tools/call {name: "rivet.auth.begin"}
// vhco:trigger mcp auth/complete_authorization = tools/call {name: "rivet.auth.complete"}
// vhco:trigger mcp auth/credential_status = tools/call {name: "rivet.auth.status"}
// vhco:trigger mcp auth/disconnect_account = tools/call {name: "rivet.auth.disconnect"}
// vhco:trigger mcp auth/cancel_authorization = tools/call {name: "rivet.auth.cancel"}
// vhco:trigger mcp execution/request_operation = tools/call {name: ID} | tools/call {name: "rivet.request"}
// vhco:trigger mcp registry/describe_operations = tools/list | tools/call {name: "rivet.list"|"rivet.describe"}
// vhco:trigger mcp registry/inspect_outputs = tools/call {name: "rivet.outputs"}
// vhco:trigger mcp sessions/open_session = tools/call {name: STREAMING_ID} | tools/call {name: "rivet.sessions.open"}
// vhco:trigger mcp sessions/send_input = tools/call {name: "rivet.sessions.send"}
// vhco:trigger mcp sessions/finish_input = tools/call {name: "rivet.sessions.finish_input"}
// vhco:trigger mcp sessions/read_events = tools/call {name: "rivet.sessions.read"}
// vhco:trigger mcp sessions/cancel_session = tools/call {name: "rivet.sessions.cancel"}
// vhco:trigger mcp serve/authenticate_principal = Authorization: Bearer TOKEN on every POST /mcp
// vhco:trigger mcp serve/authorize_operation = tools/list filtering and every tools/call
// vhco:api mcp execution/request_operation POST /mcp tools/call -- call an operation as its direct named tool
// vhco:request { "jsonrpc": "2.0", "id": "int", "method": "tools/call", "params": "{name: ID, arguments: object, restrict?: {grants:[…]} (narrows this call only)}" }
// vhco:response { "content": "[{type:text, text: serialized Completion}]", "structuredContent": "Completion | SessionReceipt | ErrorEnvelope", "isError": "bool" }

use super::builtins::{BUILTIN_IDS, visible};
use super::runtime::Runtime;
use super::setup_serve::{
    ServeState, error_response, json_response, note_access, trace_context, with_traceparent,
};
use crate::domain::contracts::Principal;
use crate::domain::contracts::TraceContext;
use crate::domain::mcp::BridgeHops;
use crate::domain::serve::{MCP_PROTOCOL_VERSION, OperationAccess};
use crate::domain::sessions::SessionOpenInput;
use crate::domain::{ErrorKind, RivetError, Value};
use crate::features::serve::authorize_operation::require_operation;
use crate::features::sessions::open_session::open_session;
use crate::io::http::error_body;
use crate::io::mcp::{
    INVALID_PARAMS, METHOD_NOT_FOUND, builtin_tools, initialize_result, parse_message, rpc_error,
    rpc_result, tool_descriptor, tool_result,
};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value as Json, json};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";

pub fn routes(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/mcp", post(post_mcp).get(get_mcp).delete(delete_mcp))
        .with_state(state)
}

/// Handle one JSON-RPC request (shared by HTTP and stdio). `Err` is a protocol error.
pub async fn handle_request(
    rt: &Runtime,
    principal: &Principal,
    method: &str,
    params: &Json,
    trace: Option<&TraceContext>,
) -> Result<Json, (i64, String)> {
    match method {
        "initialize" => Ok(initialize_result(env!("CARGO_PKG_VERSION"))),
        "ping" => Ok(json!({})),
        "tools/list" => {
            let mut tools: Vec<Json> = rt
                .list()
                .map_err(|e| (INVALID_PARAMS, e.message))?
                .entries
                .iter()
                .filter(|e| visible(rt, principal, &e.id))
                .map(tool_descriptor)
                .collect();
            // Built-ins the principal may actually call: sensitive ones
            // (rivet.io, policy.generate, trace.show, connectors.sync) only for
            // the local principal or an exact listing; rivet.auth.* when authorized.
            tools.extend(builtin_tools().into_iter().filter(|t| {
                t["name"]
                    .as_str()
                    .is_some_and(|name| visible(rt, principal, name))
            }));
            Ok(json!({"tools": tools}))
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Json::as_str)
                .ok_or((INVALID_PARAMS, "tools/call needs `name`".to_string()))?
                .to_string();
            let args = params
                .get("arguments")
                .map(Value::from_json)
                .unwrap_or(Value::Null);
            // Bridge recursion state (`_meta` rivet/hops + rivet/chain) continues into nested connector calls.
            let bridge = BridgeHops::from_meta(params);
            note_access(|n| n.operation = Some(name.clone()));
            // G31: optional `restrict: {grants:[…]}` beside name/arguments narrows this call.
            let restrict = params.get("restrict").map(Value::from_json);
            Ok(call_tool(rt, principal, &name, args, bridge, trace, restrict).await?)
        }
        other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
    }
}

async fn call_tool(
    rt: &Runtime,
    principal: &Principal,
    name: &str,
    args: Value,
    bridge: BridgeHops,
    trace: Option<&TraceContext>,
    restrict: Option<Value>,
) -> Result<Json, (i64, String)> {
    let unknown = || (INVALID_PARAMS, format!("Unknown tool: {name}"));
    let outcome = if BUILTIN_IDS.contains(&name) {
        let mut req = rt.new_request_traced(name, args, principal.clone(), trace);
        req.restrict = restrict;
        rt.dispatch_request(req, None).await.map(|c| c.to_json())
    } else {
        if !visible(rt, principal, name) {
            return Err(unknown());
        }
        let entry = match rt.describe(&[name.to_string()]) {
            Ok(mut c) => c.entries.remove(0),
            Err(_) => return Err(unknown()),
        };
        if entry.streaming() {
            let opened = async {
                require_operation(&OperationAccess {
                    principal: principal.clone(),
                    operation_id: name.to_string(),
                    serve: rt.policy().serve.clone(),
                })?;
                open_session(
                    SessionOpenInput {
                        id: name.to_string(),
                        params: args,
                        principal: principal.clone(),
                        connection_owned: false,
                        deadline_ms: None,
                        trace: trace.cloned(),
                        restrict,
                    },
                    rt.sessions().as_ref(),
                )
                .await
            }
            .await;
            opened.map(|r| r.to_json())
        } else {
            rt.request_bridged(principal.clone(), name, args, bridge, trace, restrict)
                .await
                .map(|c| c.to_json())
        }
    };
    Ok(match outcome {
        Ok(structured) => tool_result(structured, false),
        Err(e) => tool_result(error_body(&e), true),
    })
}

fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let host_of = |s: &str| {
        let s = s.split("://").nth(1).unwrap_or(s);
        let s = s.split('/').next().unwrap_or(s);
        let h = if s.starts_with('[') {
            s.split(']')
                .next()
                .unwrap_or(s)
                .trim_start_matches('[')
                .to_string()
        } else {
            s.split(':').next().unwrap_or(s).to_string()
        };
        h.to_ascii_lowercase()
    };
    let o = host_of(origin);
    if matches!(o.as_str(), "localhost" | "127.0.0.1" | "::1") {
        return true;
    }
    headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| host_of(h) == o)
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

async fn post_mcp(
    State(st): State<Arc<ServeState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !origin_allowed(&headers) {
        return error_response(&RivetError::permission("Origin is not allowed for /mcp"));
    }
    let principal = match st.authenticate("mcp", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    if let Some(v) = header_str(&headers, VERSION_HEADER)
        && v != MCP_PROTOCOL_VERSION
    {
        return error_response(&RivetError::validation(
            "mcp.protocol_version",
            format!(
                "unsupported MCP-Protocol-Version {v}; this server speaks {MCP_PROTOCOL_VERSION}"
            ),
        ));
    }
    let msg = match parse_message(&body) {
        Ok(m) => m,
        Err(err) => return json_response(400, err),
    };
    let id = msg.id.clone().unwrap_or(Json::Null);
    let trace = trace_context(&headers);
    if let Some(m) = &msg.method
        && m != "tools/call"
    {
        let m = m.clone();
        note_access(|n| n.operation = Some(m));
    }
    if msg.method.as_deref() == Some("initialize") && msg.is_request() {
        let result = match handle_request(
            &st.runtime,
            &principal,
            "initialize",
            &msg.params,
            trace.as_ref(),
        )
        .await
        {
            Ok(r) => rpc_result(&id, r),
            Err((c, m)) => rpc_error(&id, c, &m),
        };
        let sid = new_session_id();
        if let Ok(mut m) = st.mcp_sessions.lock() {
            m.insert(sid.clone(), principal.name.clone());
        }
        let mut r = json_response(200, result);
        if let Ok(v) = HeaderValue::from_str(&sid) {
            r.headers_mut().insert(SESSION_HEADER, v);
        }
        return r;
    }
    let owner = header_str(&headers, SESSION_HEADER).map(str::to_string);
    let Some(sid) = owner else {
        return error_response(&RivetError::validation(
            "mcp.session_required",
            "MCP-Session-Id header is required after initialize",
        ));
    };
    let known = st
        .mcp_sessions
        .lock()
        .map(|m| m.get(&sid) == Some(&principal.name))
        .unwrap_or(false);
    if !known {
        return error_response(&RivetError::not_found(
            "not_found.mcp_session",
            "unknown or expired MCP session",
        ));
    }
    if !msg.is_request() {
        return StatusCode::ACCEPTED.into_response();
    }
    let method = msg.method.clone().unwrap_or_default();
    let out = match handle_request(
        &st.runtime,
        &principal,
        &method,
        &msg.params,
        trace.as_ref(),
    )
    .await
    {
        Ok(r) => rpc_result(&id, r),
        Err((c, m)) => rpc_error(&id, c, &m),
    };
    // traceparent of the request a tools/call ran (Completion, SessionReceipt
    // or ErrorEnvelope carry its ids); other methods echo the caller's trace.
    let sc = &out["result"]["structuredContent"];
    let ids = |j: &Json| {
        Some((
            j.get("trace_id")?.as_str()?.to_string(),
            j.get("request_id")?.as_str()?.to_string(),
        ))
    };
    let ran = ids(sc).or_else(|| ids(&sc["error"]));
    let resp = json_response(200, out.clone());
    match (ran, trace) {
        (Some((tid, rid)), _) if !tid.is_empty() => with_traceparent(resp, &tid, &rid),
        (_, Some(t)) => with_traceparent(resp, &t.trace_id, &format!("mcp:{sid}:{id}")),
        _ => resp,
    }
}

async fn get_mcp() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST, DELETE")],
    )
        .into_response()
}

async fn delete_mcp(
    State(st): State<Arc<ServeState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let principal = match st.authenticate("mcp", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let sid = header_str(&headers, SESSION_HEADER)
        .unwrap_or("")
        .to_string();
    let removed = st
        .mcp_sessions
        .lock()
        .map(|mut m| {
            if m.get(&sid) == Some(&principal.name) {
                m.remove(&sid).is_some()
            } else {
                false
            }
        })
        .unwrap_or(false);
    if removed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        error_response(&RivetError::new(
            ErrorKind::NotFound,
            "not_found.mcp_session",
            "unknown or expired MCP session",
        ))
    }
}

fn new_session_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst) + 1;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mix = (t ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    format!("mcp_{n:x}{mix:016x}")
}

/// `rivet serve --stdio`: newline-delimited JSON-RPC on stdin/stdout; stdout
/// carries protocol messages only. The principal is the local operator.
pub async fn run_stdio(rt: Runtime) {
    let principal = Principal::local();
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match parse_message(line.as_bytes()) {
            Err(err) => Some(err),
            Ok(m) if m.is_request() => {
                let id = m.id.clone().unwrap_or(Json::Null);
                let method = m.method.clone().unwrap_or_default();
                Some(
                    match handle_request(&rt, &principal, &method, &m.params, None).await {
                        Ok(r) => rpc_result(&id, r),
                        Err((c, msg)) => rpc_error(&id, c, &msg),
                    },
                )
            }
            Ok(_) => None,
        };
        if let Some(r) = reply {
            let mut text = r.to_string();
            text.push('\n');
            if out.write_all(text.as_bytes()).await.is_err() || out.flush().await.is_err() {
                break;
            }
        }
    }
}
