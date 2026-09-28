//! WebSocket surface: `GET /v1/ws`, subprotocol `rivet.v1`, refs multiplexed
//! over one socket. Each ref is a connection-owned session; a pump per ref
//! forwards its events as frames.
//!
//! ```text
//!  upgrade (auth once) ─▶ reader: text frame ─▶ serve.parse_input (request frames) ─▶ serve.multiplex_ws ─▶ SessionDriver
//!                                                  │ opened(ref, session) ─▶ pump: read ─▶ data records* ─▶ one result record
//!  socket close ─▶ cancel + join every in-flight ref
//! ```

// vhco:surface ws kind websocket calls serve/multiplex_ws, serve/authenticate_principal, serve/authorize_operation, serve/parse_input
// vhco:trigger ws serve/multiplex_ws = GET /v1/ws (Sec-WebSocket-Protocol: rivet.v1) text frames request {operation, data}|input|finish_input|cancel
// vhco:trigger ws serve/parse_input = request frame {type:"request", ref, operation, data}
// vhco:trigger ws serve/authenticate_principal = Authorization: Bearer TOKEN on the upgrade request
// vhco:trigger ws serve/authorize_operation = serve.principals check per request frame
// vhco:api ws serve/multiplex_ws GET /v1/ws -- JSON text frames multiplexed by client-chosen ref (max 8 in flight, one terminal record per ref); server frames are ResponseEnvelope records with `ref` first
// vhco:request { "type": "request|input|finish_input|cancel", "ref": "string", "operation": "string (request; deprecated alias id)", "data": "object (request: operation input; deprecated alias params) | Value (input: the item)", "deadline_ms": "int? (request)", "restrict": "{grants:[…]}? (request)", "seq": "int (input)" }
// vhco:response { "ref": "string", "request_id": "string", "trace_id": "string", "operation": "string", "type": "data|result", "seq": "int (data)", "status": "ok|error|cancelled (result)", "data": "Value|null", "error": "{kind, code, message, retryable, …}|null", "effects": "none|committed|partial|unknown (result)", "data_count": "int (result)" }

use super::setup_serve::{ServeState, error_response, trace_context};
use crate::domain::RivetError;
use crate::domain::contracts::Principal;
use crate::domain::contracts::TraceContext;
use crate::domain::envelope::RawInput;
use crate::domain::ports::WsConnection;
use crate::domain::serve::{WS_SUBPROTOCOL, WsFrame, WsInbound};
use crate::domain::sessions::{SessionReadInput, SessionRef};
use crate::features::serve::multiplex_ws::multiplex_ws;
use crate::features::serve::parse_input::parse_input;
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
    let trace = trace_context(&headers);
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
        .on_upgrade(move |socket| connection(st, principal, trace, socket))
}

type Refs = Arc<Mutex<HashMap<String, String>>>;

/// Frames answering a client frame name the ref's operation: the multiplexer
/// knows refs only by session, so records without one are stamped here.
struct NamedReplies<'a> {
    inner: &'a dyn WsConnection,
    operation: Option<String>,
}

impl NamedReplies<'_> {
    fn stamp(&self, mut f: WsFrame) -> WsFrame {
        if let (Some(op), Some(rec)) = (&self.operation, f.record.as_mut())
            && rec.operation.is_none()
        {
            rec.operation = Some(op.clone());
        }
        f
    }
}

#[async_trait::async_trait]
impl WsConnection for NamedReplies<'_> {
    async fn send(&self, frame: WsFrame) -> crate::domain::RivetResult<()> {
        self.inner.send(self.stamp(frame)).await
    }
    async fn reply(&self, frame: WsFrame) -> crate::domain::RivetResult<()> {
        self.inner.reply(self.stamp(frame)).await
    }
}

async fn connection(
    st: Arc<ServeState>,
    principal: Principal,
    trace: Option<TraceContext>,
    socket: WebSocket,
) {
    let (mut sink, mut stream) = socket.split();
    // Per-ref lanes of 16 frames merged into one writer (backpressure per ref).
    let (lanes, mut frames) = WsOutbox::new();
    let writer = tokio::spawn(async move {
        while let Some(text) = frames.next().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let lanes = Arc::new(lanes);
    let outbox: Arc<dyn WsConnection> = lanes.clone();
    let driver = st.runtime.sessions();
    let refs: Refs = Arc::new(Mutex::new(HashMap::new()));
    // ref → operation of every ref this connection opened (names its later records).
    let mut ops: HashMap<String, String> = HashMap::new();
    let mut pumps = tokio::task::JoinSet::new();
    while let Some(msg) = stream.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(_)) => {
                let _ = outbox
                    .reply(WsFrame::error(
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
        // Request frames: {operation, data} through serve.parse_input.
        let frame = parse_client_frame(&text, |j| parse_input(RawInput::new(j.clone())));
        let (op, aliases) = match &frame {
            Ok(f) => f
                .input
                .as_ref()
                .map(|i| (i.operation.clone(), i.aliases.clone()))
                .unwrap_or_default(),
            Err(_) => Default::default(),
        };
        let frame_ref = frame.as_ref().ok().map(|f| f.r#ref.clone());
        let named = NamedReplies {
            inner: outbox.as_ref(),
            operation: if op.is_empty() {
                frame_ref.and_then(|r| ops.get(&r).cloned())
            } else {
                Some(op.clone())
            },
        };
        let outcome = multiplex_ws(
            WsInbound {
                frame,
                principal: principal.clone(),
                serve: st.serve.clone(),
                open_refs,
                trace: trace.clone(),
            },
            &named,
            driver.as_ref(),
        )
        .await;
        match outcome {
            Ok(o) => {
                if let Some((r, session_id)) = o.opened {
                    lanes.open_ref(&r);
                    if let Ok(mut m) = refs.lock() {
                        m.insert(r.clone(), session_id.clone());
                    }
                    ops.insert(r.clone(), op.clone());
                    pumps.spawn(pump(
                        Pump {
                            r,
                            session_id,
                            operation: op,
                            deprecated: aliases,
                        },
                        principal.clone(),
                        st.runtime.clone(),
                        Arc::clone(&lanes),
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
    drop(lanes);
    let _ = writer.await;
}

/// One opened ref: its session, operation and any deprecated input aliases.
struct Pump {
    r: String,
    session_id: String,
    operation: String,
    deprecated: Vec<String>,
}

/// Forward one ref's session events as records until its terminal record. A
/// ref opened with deprecated aliases gets one trace note (its request id is
/// known from the first record).
async fn pump(
    p: Pump,
    principal: Principal,
    runtime: crate::orchestrator::runtime::Runtime,
    out: Arc<WsOutbox>,
    refs: Refs,
) {
    let Pump {
        r,
        session_id,
        operation,
        deprecated,
    } = p;
    let driver = runtime.sessions();
    let mut noted = deprecated.is_empty();
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
                let _ = out.send(WsFrame::error_for(&r, Some(&operation), e)).await;
                break;
            }
        };
        let mut done = false;
        for ev in &batch.events {
            let record = ev.record();
            if !noted {
                noted = true;
                runtime.note_deprecated_input(
                    &record.request_id,
                    &record.trace_id,
                    &operation,
                    &deprecated,
                );
            }
            let frame = WsFrame::from_record(&r, record);
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
    // Release the lane before the ref name becomes reusable.
    out.release_ref(&r);
    if let Ok(mut m) = refs.lock() {
        m.remove(&r);
    }
}
