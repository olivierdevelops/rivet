//! Polling surface: the HTTP projection of `rivet.sessions.*`.
//!
//! ```text
//!  POST /v1/requests {id, params} ─▶ 202 SessionReceipt + events_url
//!  GET  /v1/requests/{id}/events?after_seq=N&wait_ms=M ─▶ 200 SessionBatch (long-poll ≤ 5 s)
//!  POST /v1/requests/{id}/input {send_seq, data} │ /finish_input │ /cancel
//! ```

// vhco:surface poll kind http calls serve/project_polling, serve/authenticate_principal, serve/authorize_operation
// vhco:trigger poll serve/project_polling = POST /v1/requests | GET /v1/requests/{id}/events | POST /v1/requests/{id}/input | POST /v1/requests/{id}/finish_input | POST /v1/requests/{id}/cancel
// vhco:trigger poll serve/authenticate_principal = Authorization: Bearer TOKEN on every polling route
// vhco:trigger poll serve/authorize_operation = serve.principals check when a session is opened
// vhco:api poll serve/project_polling POST /v1/requests -- open a principal-owned session (unary or streaming) and poll its events
// vhco:request { "id": "string — operation ID", "params": "object", "deadline_ms": "int? — total deadline, capped at 600000", "headers": "traceparent? — W3C trace context for the session" }
// vhco:response { "session_id": "string", "request_id": "string", "catalog_version": "string", "input_schema": "json|null", "emits_schema": "json|null", "next_send_seq": "int", "expires_at": "RFC 3339", "events_url": "/v1/requests/{session_id}/events" }

use super::setup_serve::{
    ServeState, error_response, json_response, note_access, trace_context, with_traceparent,
};
use crate::domain::serve::{PollAction, PollRoute};
use crate::features::serve::project_polling::project_polling;
use crate::io::http::parse_request_body;
use crate::io::http::poll::{parse_events_query, parse_input_body};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use std::net::SocketAddr;
use std::sync::Arc;

type St = State<Arc<ServeState>>;

pub fn routes(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/v1/requests", post(open))
        .route("/v1/requests/{id}/events", get(events))
        .route("/v1/requests/{id}/input", post(input))
        .route("/v1/requests/{id}/finish_input", post(finish))
        .route("/v1/requests/{id}/cancel", post(cancel))
        .with_state(state)
}

async fn run(
    st: &ServeState,
    headers: &HeaderMap,
    peer: SocketAddr,
    action: PollAction,
) -> Response {
    let principal = match st.authenticate("poll", headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let op = match &action {
        PollAction::Open { id, .. } => id.clone(),
        PollAction::Events { .. } => "rivet.sessions.read".into(),
        PollAction::Input { .. } => "rivet.sessions.send".into(),
        PollAction::FinishInput { .. } => "rivet.sessions.finish_input".into(),
        PollAction::Cancel { .. } => "rivet.sessions.cancel".into(),
    };
    note_access(|n| n.operation = Some(op));
    let r = project_polling(
        PollRoute {
            action,
            principal,
            serve: st.serve.clone(),
            trace: trace_context(headers),
        },
        st.runtime.sessions().as_ref(),
    )
    .await;
    let ids = (
        r.body
            .get("trace_id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        r.body
            .get("request_id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    );
    let resp = json_response(r.status, r.body);
    match ids {
        (Some(tid), Some(rid)) if r.status == 202 => with_traceparent(resp, &tid, &rid),
        _ => resp,
    }
}

async fn open(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(e) = st.authenticate("poll", &headers, peer) {
        return error_response(&e);
    }
    match parse_request_body(&body) {
        Ok(b) => {
            run(
                &st,
                &headers,
                peer,
                PollAction::Open {
                    id: b.id,
                    params: b.params,
                    deadline_ms: b.deadline_ms,
                },
            )
            .await
        }
        Err(e) => error_response(&e),
    }
}

async fn events(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    RawQuery(q): RawQuery,
) -> Response {
    if let Err(e) = st.authenticate("poll", &headers, peer) {
        return error_response(&e);
    }
    match parse_events_query(q.as_deref()) {
        Ok(q) => {
            run(
                &st,
                &headers,
                peer,
                PollAction::Events {
                    session_id: id,
                    after_seq: q.after_seq,
                    wait_ms: q.wait_ms,
                    max_events: q.max_events,
                },
            )
            .await
        }
        Err(e) => error_response(&e),
    }
}

async fn input(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    if let Err(e) = st.authenticate("poll", &headers, peer) {
        return error_response(&e);
    }
    match parse_input_body(&body) {
        Ok((send_seq, data)) => {
            run(
                &st,
                &headers,
                peer,
                PollAction::Input {
                    session_id: id,
                    send_seq,
                    data,
                },
            )
            .await
        }
        Err(e) => error_response(&e),
    }
}

async fn finish(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    run(
        &st,
        &headers,
        peer,
        PollAction::FinishInput { session_id: id },
    )
    .await
}

async fn cancel(
    State(st): St,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    run(&st, &headers, peer, PollAction::Cancel { session_id: id }).await
}
