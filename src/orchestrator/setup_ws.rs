//! WebSocket surface: `GET /v1/ws`, subprotocol `rivet.v1`, refs multiplexed
//! over one socket. Each ref is a connection-owned session; a pump per ref
//! forwards its events as frames.
//!
//! ```text
//!  upgrade (auth once) ─▶ reader: text frame ─▶ serve.multiplex_ws ─▶ SessionDriver
//!                                                  │ opened(ref, session) ─▶ pump: read ─▶ data* ─▶ result|error
//!  socket close ─▶ cancel + join every in-flight ref
//! ```

// vhco:surface ws kind websocket calls serve/multiplex_ws, serve/authenticate_principal, serve/authorize_operation
// vhco:trigger ws serve/multiplex_ws = GET /v1/ws (Sec-WebSocket-Protocol: rivet.v1) text frames request|input|finish_input|cancel
// vhco:trigger ws serve/authenticate_principal = Authorization: Bearer TOKEN on the upgrade request
// vhco:trigger ws serve/authorize_operation = serve.principals check per request frame
// vhco:api ws serve/multiplex_ws GET /v1/ws -- JSON text frames multiplexed by client-chosen ref (max 8 in flight, one terminal frame per ref)
// vhco:request { "type": "request|input|finish_input|cancel", "ref": "string", "id": "string (request)", "params": "object (request)", "seq": "int (input)", "data": "Value (input)" }
// vhco:response { "type": "data|result|error", "ref": "string", "seq": "int (data)", "data": "Value (data)", "completion": "Completion (result)", "error": "RivetError (error)" }

use super::setup_serve::{ServeState, error_response};
use crate::domain::RivetError;
use crate::domain::contracts::{Envelope, Principal};
use crate::domain::ports::{SessionDriver, WsConnection};
use crate::domain::serve::{WS_SUBPROTOCOL, WsFrame, WsInbound};
use crate::domain::sessions::{SessionReadInput, SessionRef};
use crate::features::serve::multiplex_ws::multiplex_ws;
use crate::infra::serve_listener::WsOutbox;
use crate::io::ws::parse_client_frame;
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub fn routes(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/v1/ws", get(upgrade))
        .with_state(state)
}

async fn upgrade(
    State(st): State<Arc<ServeState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let principal = match st.authenticate("ws", &headers, peer) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let offered = headers
        .get_all("sec-websocket-protocol")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|p| p.trim() == WS_SUBPROTOCOL);
    if !offered {
        return error_response(&RivetError::validation(
            "validation.subprotocol",
            "the WebSocket client must offer subprotocol rivet.v1",
        ));
    }
    ws.protocols([WS_SUBPROTOCOL])
        .on_upgrade(move |socket| connection(st, principal, socket))
}

type Refs = Arc<Mutex<HashMap<String, String>>>;

async fn connection(st: Arc<ServeState>, principal: Principal, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<String>(64);
    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let outbox: Arc<dyn WsConnection> = Arc::new(WsOutbox::new(tx));
    let driver = st.runtime.sessions();
    let refs: Refs = Arc::new(Mutex::new(HashMap::new()));
    let mut pumps = tokio::task::JoinSet::new();
    while let Some(msg) = stream.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(_)) => {
                let _ = outbox
                    .send(WsFrame::error(
                        "",
                        RivetError::validation("validation.frame", "frames must be JSON text"),
                    ))
                    .await;
                continue;
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        let open_refs: Vec<(String, String)> = refs
            .lock()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let outcome = multiplex_ws(
            WsInbound {
                frame: parse_client_frame(&text),
                principal: principal.clone(),
                serve: st.serve.clone(),
                open_refs,
            },
            outbox.as_ref(),
            driver.as_ref(),
        )
        .await;
        match outcome {
            Ok(o) => {
                if let Some((r, session_id)) = o.opened {
                    if let Ok(mut m) = refs.lock() {
                        m.insert(r.clone(), session_id.clone());
                    }
                    pumps.spawn(pump(
                        r,
                        session_id,
                        principal.clone(),
                        Arc::clone(&driver),
                        Arc::clone(&outbox),
                        Arc::clone(&refs),
                    ));
                }
            }
            Err(_) => break, // the outbound side closed
        }
    }
    // Socket closed: cancel and join every in-flight ref (connection-owned).
    let open: Vec<String> = refs
        .lock()
        .map(|m| m.values().cloned().collect())
        .unwrap_or_default();
    for session_id in open {
        let _ = driver
            .cancel(SessionRef {
                session_id,
                principal: principal.clone(),
            })
            .await;
    }
    pumps.abort_all();
    while pumps.join_next().await.is_some() {}
    drop(outbox);
    let _ = writer.await;
}

/// Forward one ref's session events as frames until its terminal frame.
async fn pump(
    r: String,
    session_id: String,
    principal: Principal,
    driver: Arc<dyn SessionDriver>,
    out: Arc<dyn WsConnection>,
    refs: Refs,
) {
    let mut after = 0u64;
    loop {
        let batch = driver
            .read(SessionReadInput {
                session_id: session_id.clone(),
                after_seq: after,
                max_events: None,
                wait_ms: Some(5000),
                principal: principal.clone(),
            })
            .await;
        let batch = match batch {
            Ok(b) => b,
            Err(e) => {
                let _ = out.send(WsFrame::error(&r, e)).await;
                break;
            }
        };
        let mut done = false;
        for ev in &batch.events {
            let frame = match &ev.envelope {
                Envelope::Data(d) => WsFrame::data(&r, ev.seq, d.data.clone()),
                Envelope::Result(c) => WsFrame::result(&r, c.clone()),
                Envelope::Error { error, .. } => WsFrame::error(&r, (**error).clone()),
            };
            done |= frame.is_terminal();
            if out.send(frame).await.is_err() {
                done = true;
                break;
            }
        }
        after = batch.last_seq;
        if done || batch.terminal {
            break;
        }
    }
    if let Ok(mut m) = refs.lock() {
        m.remove(&r);
    }
}
