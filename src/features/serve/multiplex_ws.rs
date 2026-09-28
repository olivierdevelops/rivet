use super::authorize_operation::require_operation;
use super::ports::{SessionDriver, WsConnection};
use crate::domain::serve::{OperationAccess, WsFrame, WsFrameType, WsInbound, WsOutcome};
use crate::domain::sessions::{MAX_WS_REFS, SessionOpenInput, SessionRef, SessionSendInput};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};

// vhco:usecase serve.multiplex_ws(input: WsInbound) -> WsOutcome needs WsConnection, SessionDriver
// vhco:label Multiplex ws
// vhco:about Handles one client frame on /v1/ws (subprotocol rivet.v1): opens a connection-owned ref, routes input/finish_input/cancel to that ref's session, and answers malformed or refused frames with an error frame without closing the socket.
// vhco:example input={frame:{type:"request", ref:"c1", id:"demo.add", params:{a:2,b:3}}} => { "opened": ["c1", "ses_…"] }
pub async fn multiplex_ws(
    input: WsInbound,
    conn: &dyn WsConnection,
    driver: &dyn SessionDriver,
) -> RivetResult<WsOutcome> {
    let reply = |r: &str, e: RivetError| WsFrame::error(r, e);
    // vhco:todo parse_frame -- the surface parses JSON text frames; a malformed frame (bad JSON, unknown type, missing ref) arrives here as an error and is answered with {type:error, ref, error} (validation, 422 semantics) without closing the socket
    // vhco:step malformed conn.send -- parse failure → error frame for the ref if it could be read
    let frame = match input.frame {
        Ok(f) => f,
        Err((r, e)) => {
            conn.send(reply(&r, e)).await?;
            return Ok(WsOutcome {
                opened: None,
                replied: true,
            });
        }
    };
    let session_of = |r: &str| {
        input
            .open_refs
            .iter()
            .find(|(x, _)| x == r)
            .map(|(_, s)| s.clone())
    };
    let outcome = match frame.kind {
        // vhco:todo open_ref -- type request: a ref already in flight is conflict.ref (409); a 9th concurrent ref is limit.ws_refs (429, max 8 per connection); authorize the operation for the connection's principal (403); then SessionDriver.open with connection_owned=true (bounded 16-frame event log) and report the new (ref, session_id) so the host starts its event pump
        // vhco:error duplicate_ref -- the ref is already in flight => conflict.ref error frame
        // vhco:error ninth_ref -- 8 refs already in flight => limit.ws_refs error frame
        WsFrameType::Request => {
            let r = frame.r#ref.clone();
            let opened: RivetResult<String> = async {
                if session_of(&r).is_some() {
                    return Err(RivetError::new(
                        ErrorKind::Conflict,
                        "conflict.ref",
                        format!("ref `{r}` is already in flight"),
                    ));
                }
                if input.open_refs.len() >= MAX_WS_REFS {
                    return Err(RivetError::new(
                        ErrorKind::Limit,
                        "limit.ws_refs",
                        format!("at most {MAX_WS_REFS} refs may be in flight per connection"),
                    ));
                }
                let id = frame.id.clone().unwrap_or_default();
                // vhco:step authorize require_operation -- same principal rules as every surface
                require_operation(&OperationAccess {
                    principal: input.principal.clone(),
                    operation_id: id.clone(),
                    serve: input.serve.clone(),
                })?;
                // vhco:step open driver.open -- connection-owned session drives the operation
                let receipt = driver
                    .open(SessionOpenInput {
                        id,
                        params: frame.params.clone().unwrap_or(Value::Null),
                        principal: input.principal.clone(),
                        connection_owned: true,
                        deadline_ms: None,
                    })
                    .await?;
                Ok(receipt.session_id)
            }
            .await;
            match opened {
                Ok(session_id) => WsOutcome {
                    opened: Some((r, session_id)),
                    replied: false,
                },
                Err(e) => {
                    conn.send(reply(&r, e)).await?;
                    WsOutcome {
                        opened: None,
                        replied: true,
                    }
                }
            }
        }
        // vhco:todo route_input -- input {seq, data} -> SessionDriver.send with the session sequence rules; finish_input -> SessionDriver.finish_input; cancel -> SessionDriver.cancel (the ref's pump then delivers exactly one terminal `cancelled` error frame); a frame for an unknown ref gets an error frame; a refused input or finish cancels its ref so it still ends with exactly one terminal frame
        // vhco:error unknown_ref -- input/finish_input/cancel for a ref not in flight => not_found.ref error frame
        kind => {
            let r = frame.r#ref.clone();
            let Some(session_id) = session_of(&r) else {
                conn.send(reply(
                    &r,
                    RivetError::not_found("not_found.ref", format!("ref `{r}` is not in flight")),
                ))
                .await?;
                return Ok(WsOutcome {
                    opened: None,
                    replied: true,
                });
            };
            let sref = SessionRef {
                session_id: session_id.clone(),
                principal: input.principal.clone(),
            };
            // vhco:step route driver.send -- input/finish_input/cancel on the ref's session
            let routed = match kind {
                WsFrameType::Input => driver
                    .send(SessionSendInput {
                        session_id,
                        send_seq: frame.seq.unwrap_or(0),
                        data: frame.data.clone().unwrap_or(Value::Null),
                        principal: input.principal.clone(),
                    })
                    .await
                    .map(|_| ()),
                WsFrameType::FinishInput => driver.finish_input(sref.clone()).await.map(|_| ()),
                _ => driver.cancel(sref.clone()).await.map(|_| ()),
            };
            // vhco:step refuse driver.cancel -- a refused input/finish cancels the ref (one terminal frame follows)
            if routed.is_err() && kind != WsFrameType::Cancel {
                let _ = driver.cancel(sref).await;
            }
            WsOutcome::default()
        }
    };
    // vhco:todo stream_out -- the host's per-ref pump reads the session with a bounded wait and forwards {type:data, ref, seq, data}, then exactly one {type:result, ref, completion} or {type:error, ref, error}; a full 16-event log blocks the producer (backpressure)
    // vhco:todo close_cancels -- on socket close the host cancels and joins every in-flight ref through SessionDriver.cancel (connection-owned, unlike polling sessions); WS is a projection only, no WS-only operations
    Ok(outcome)
}
