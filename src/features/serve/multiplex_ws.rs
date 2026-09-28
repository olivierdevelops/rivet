use super::authorize_operation::require_operation;
use super::ports::{SessionDriver, WsConnection};
use crate::domain::serve::{OperationAccess, WsFrame, WsFrameType, WsInbound, WsOutcome};
use crate::domain::sessions::{MAX_WS_REFS, SessionOpenInput, SessionRef, SessionSendInput};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};

// vhco:usecase serve.multiplex_ws(input: WsInbound) -> WsOutcome needs WsConnection, SessionDriver
// vhco:label Multiplex ws
// vhco:about Handles one client frame on /v1/ws (subprotocol rivet.v1): opens a connection-owned ref, routes input/finish_input/cancel to that ref's session, and answers malformed or refused frames with an error frame without closing the socket; a refused input ends its ref with that specific error.
// vhco:example input={frame:{type:"request", ref:"c1", input:{operation:"demo.add", data:{a:2,b:3}}}} => { "opened": ["c1", "ses_…"] }
pub async fn multiplex_ws(
    input: WsInbound,
    conn: &dyn WsConnection,
    driver: &dyn SessionDriver,
) -> RivetResult<WsOutcome> {
    let reply = |r: &str, e: RivetError| WsFrame::error(r, e);
    // vhco:todo parse_frame -- the surface parses JSON text frames (request frames through serve.parse_input: {operation, data}, deprecated {id, params}); a malformed frame (bad JSON, unknown type, missing ref, bad envelope) arrives here as an error and is answered with an error record {ref, …, type:"result", status:"error", error} (validation, 422 semantics) without closing the socket
    // vhco:step malformed conn.reply -- parse failure → error frame for the ref if it could be read
    let frame = match input.frame {
        Ok(f) => f,
        Err((r, e)) => {
            conn.reply(reply(&r, e)).await?;
            return Ok(WsOutcome {
                replied: true,
                ..WsOutcome::default()
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
                let request = frame
                    .input
                    .clone()
                    .unwrap_or_else(|| crate::domain::envelope::InputEnvelope::new(""));
                let id = request.operation.clone();
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
                        params: request.data,
                        principal: input.principal.clone(),
                        connection_owned: true,
                        deadline_ms: request.deadline_ms,
                        trace: input.trace.clone(),
                        restrict: request.restrict,
                    })
                    .await?;
                Ok(receipt.session_id)
            }
            .await;
            match opened {
                Ok(session_id) => WsOutcome {
                    opened: Some((r, session_id)),
                    ..WsOutcome::default()
                },
                Err(e) => {
                    // The ref never opened: this error is its only frame.
                    let op = frame.input.as_ref().map(|i| i.operation.as_str());
                    conn.reply(WsFrame::error_for(&r, op, e)).await?;
                    WsOutcome {
                        replied: true,
                        ..WsOutcome::default()
                    }
                }
            }
        }
        // vhco:todo route_input -- input {seq, data} -> SessionDriver.send with the session sequence rules; finish_input -> SessionDriver.finish_input; cancel -> SessionDriver.cancel (the ref's pump then delivers exactly one terminal `cancelled` error frame); a frame for an unknown ref gets an error frame; a refused input or finish first sends that ref's terminal error frame with the specific code (conflict.input_sequence, validation.input, conflict.input_closed …) and only then cancels its session, so the ref still ends with exactly one terminal frame
        // vhco:error unknown_ref -- input/finish_input/cancel for a ref not in flight => not_found.ref error frame
        kind => {
            let r = frame.r#ref.clone();
            let Some(session_id) = session_of(&r) else {
                conn.reply(reply(
                    &r,
                    RivetError::not_found("not_found.ref", format!("ref `{r}` is not in flight")),
                ))
                .await?;
                return Ok(WsOutcome {
                    replied: true,
                    ..WsOutcome::default()
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
            // vhco:step refuse conn.send -- a refused input/finish ends the ref: its terminal error frame carries the specific code, sent before any cancellation
            // vhco:error refused_input -- the session refused an input/finish_input frame => that error (e.g. conflict.input_sequence, validation.input) as the ref's terminal frame, then the session is cancelled
            match routed {
                Err(e) if kind != WsFrameType::Cancel => {
                    conn.send(reply(&r, e)).await?;
                    // vhco:step cancel driver.cancel -- then cancel the ref's session (its pump's own terminal frame is discarded: the lane already closed)
                    let _ = driver.cancel(sref).await;
                    WsOutcome {
                        replied: true,
                        ended: Some(r),
                        ..WsOutcome::default()
                    }
                }
                _ => WsOutcome::default(),
            }
        }
    };
    // vhco:todo stream_out -- the host's per-ref pump reads the session with a bounded wait and forwards each data record {ref, request_id, trace_id, operation, type:"data", seq, data, error:null}, then exactly one terminal record {ref, …, type:"result", status:ok|error|cancelled, data, error, effects, data_count}; a full 16-event log blocks the producer (backpressure)
    // vhco:todo close_cancels -- on socket close the host cancels and joins every in-flight ref through SessionDriver.cancel (connection-owned, unlike polling sessions); WS is a projection only, no WS-only operations
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contracts::Principal;
    use crate::domain::policy::ServePolicy;
    use crate::domain::sessions::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Records every call in order: `send:<ref>:<code>`, `reply:…`, `cancel`.
    #[derive(Default)]
    struct Log(Mutex<Vec<String>>);

    impl Log {
        fn push(&self, s: String) {
            self.0.lock().unwrap().push(s);
        }
    }

    #[async_trait]
    impl WsConnection for Log {
        async fn send(&self, f: WsFrame) -> RivetResult<()> {
            let code = f.error_ref().map(|e| e.code.clone()).unwrap_or_default();
            self.push(format!("send:{}:{code}", f.r#ref));
            Ok(())
        }
        async fn reply(&self, f: WsFrame) -> RivetResult<()> {
            let code = f.error_ref().map(|e| e.code.clone()).unwrap_or_default();
            self.push(format!("reply:{}:{code}", f.r#ref));
            Ok(())
        }
    }

    struct Driver<'a>(&'a Log);

    #[async_trait]
    impl SessionDriver for Driver<'_> {
        async fn open(&self, _: SessionOpenInput) -> RivetResult<SessionReceipt> {
            Err(RivetError::internal("unused"))
        }
        async fn send(&self, input: SessionSendInput) -> RivetResult<SessionAck> {
            if input.send_seq == 1 {
                return Ok(SessionAck {
                    session_id: input.session_id,
                    accepted_seq: Some(1),
                    input_closed: false,
                });
            }
            Err(RivetError::new(
                ErrorKind::Conflict,
                "conflict.input_sequence",
                "expected send_seq 2",
            ))
        }
        async fn finish_input(&self, _: SessionRef) -> RivetResult<SessionAck> {
            Err(RivetError::internal("unused"))
        }
        async fn read(&self, _: SessionReadInput) -> RivetResult<SessionBatch> {
            Err(RivetError::internal("unused"))
        }
        async fn cancel(&self, input: SessionRef) -> RivetResult<CancelReceipt> {
            self.0.push("cancel".into());
            Ok(CancelReceipt {
                request_id: "req".into(),
                session_id: Some(input.session_id),
                state: "cancelled".into(),
            })
        }
        fn limits(&self) -> SessionLimits {
            SessionLimits::default()
        }
    }

    fn inbound(frame: WsFrame) -> WsInbound {
        WsInbound {
            frame: Ok(frame),
            principal: Principal::local(),
            serve: ServePolicy::default(),
            open_refs: vec![("c1".into(), "ses_1".into())],
            trace: None,
        }
    }

    fn input(seq: u64) -> WsFrame {
        let mut f = WsFrame::empty(WsFrameType::Input, "c1");
        f.seq = Some(seq);
        f.data = Some(Value::text("hi"));
        f
    }

    // vhco:test serve.multiplex_ws -- G17/G27: a refused input frame sends that ref's terminal error frame with the specific code (conflict.input_sequence) before the session is cancelled; unknown refs get a reply, not a ref frame
    #[tokio::test]
    async fn refused_input_sends_the_specific_error_before_cancelling() {
        let log = Log::default();
        let ok = multiplex_ws(inbound(input(1)), &log, &Driver(&log))
            .await
            .unwrap();
        assert_eq!(ok, WsOutcome::default());
        let refused = multiplex_ws(inbound(input(5)), &log, &Driver(&log))
            .await
            .unwrap();
        assert_eq!(refused.ended.as_deref(), Some("c1"));
        let mut unknown = input(1);
        unknown.r#ref = "zz".into();
        multiplex_ws(inbound(unknown), &log, &Driver(&log))
            .await
            .unwrap();
        assert_eq!(
            *log.0.lock().unwrap(),
            vec![
                "send:c1:conflict.input_sequence".to_string(),
                "cancel".to_string(),
                "reply:zz:not_found.ref".to_string(),
            ]
        );
    }
}
