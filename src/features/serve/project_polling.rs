use super::authorize_operation::require_operation;
use super::ports::SessionDriver;
use crate::domain::RivetError;
use crate::domain::envelope::ResponseEnvelope;
use crate::domain::serve::{OperationAccess, PollAction, PollResponse, PollRoute};
use crate::domain::sessions::{SessionOpenInput, SessionReadInput, SessionRef, SessionSendInput};

// vhco:usecase serve.project_polling(input: PollRoute) -> PollResponse needs SessionDriver
// vhco:label Project polling
// vhco:about The HTTP polling routes are a projection of the session operations: open → 202 ResponseEnvelope (status accepted) whose data is the SessionReceipt with events_url, events → SessionBatch whose events are ResponseEnvelope records, input/finish_input/cancel → SessionAck / CancelReceipt, all bound to the authenticated principal; every failure is an error envelope.
// vhco:example input={action:{open:{id:"demo.countdown"}}} => { "status": 202, "body": {"request_id": "req_…", "trace_id": "tr_…", "operation": "demo.countdown", "type": "result", "status": "accepted", "data": {"session_id": "ses_…", "events_url": "/v1/requests/ses_…/events"}, "error": null, "effects": "none", "data_count": 0} }
pub async fn project_polling(input: PollRoute, driver: &dyn SessionDriver) -> PollResponse {
    let principal = input.principal.clone();
    let limits = driver.limits();
    // The operation each route answers for (named by its error envelope).
    let operation = match &input.action {
        PollAction::Open { id, .. } => id.clone(),
        PollAction::Events { .. } => "rivet.sessions.read".to_string(),
        PollAction::Input { .. } => "rivet.sessions.send".to_string(),
        PollAction::FinishInput { .. } => "rivet.sessions.finish_input".to_string(),
        PollAction::Cancel { .. } => "rivet.sessions.cancel".to_string(),
    };
    let result: Result<(u16, serde_json::Value), RivetError> = match input.action {
        // vhco:todo open -- POST /v1/requests {operation, data, deadline_ms?} (id/params: deprecated aliases, parsed by serve.parse_input): authorize the operation for the principal (403 permission.denied), then SessionDriver.open (principal-owned, survives reconnect; deadline_ms capped by the host at 600000; the request's W3C traceparent becomes the session's trace) and answer 202 with an accepted ResponseEnvelope whose data is the SessionReceipt with events_url=/v1/requests/{session_id}/events; unary operations work the same way and their batch holds one terminal result event
        // vhco:error denied -- the principal may not call the operation => permission.denied (403) body
        PollAction::Open {
            id,
            params,
            deadline_ms,
            restrict,
        } => {
            // vhco:step authorize require_operation -- serve.principals decision for the requested ID
            match require_operation(&OperationAccess {
                principal: principal.clone(),
                operation_id: id.clone(),
                serve: input.serve.clone(),
            }) {
                Err(e) => Err(e),
                // vhco:step open driver.open -- start the principal-owned session
                Ok(_) => driver
                    .open(SessionOpenInput {
                        id,
                        params,
                        principal: principal.clone(),
                        connection_owned: false,
                        deadline_ms,
                        trace: input.trace.clone(),
                        restrict,
                    })
                    .await
                    .map(|mut r| {
                        r.events_url = Some(format!("/v1/requests/{}/events", r.session_id));
                        // vhco:step accept ResponseEnvelope::accepted -- 202 envelope, status accepted, data = the receipt
                        let env = ResponseEnvelope::accepted(
                            &r.request_id,
                            &r.trace_id,
                            &operation,
                            r.to_json(),
                        );
                        (202, env.to_json())
                    }),
            }
        }
        // vhco:todo read -- GET /v1/requests/{id}/events?after_seq=N&wait_ms=M -> SessionDriver.read with wait_ms defaulting to 1000 and capped at 5000 (returns early when events arrive) and max_events capped at 16; 200 SessionBatch
        PollAction::Events {
            session_id,
            after_seq,
            wait_ms,
            max_events,
        } => {
            // vhco:step read driver.read -- bounded long-poll with the clamped wait
            driver
                .read(SessionReadInput {
                    session_id,
                    after_seq,
                    max_events: Some(limits.clamp_max_events(max_events)),
                    wait_ms: Some(limits.clamp_wait(wait_ms)),
                    principal: principal.clone(),
                })
                .await
                .map(|b| (200, b.to_json()))
        }
        // vhco:todo input_finish_cancel -- POST .../input {send_seq, data} -> SessionDriver.send (200 SessionAck); POST .../finish_input -> SessionDriver.finish_input (200 SessionAck input_closed:true); POST .../cancel -> SessionDriver.cancel (200 CancelReceipt with session_id, request_id, state)
        PollAction::Input {
            session_id,
            send_seq,
            data,
        } => driver
            .send(SessionSendInput {
                session_id,
                send_seq,
                data,
                principal: principal.clone(),
            })
            .await
            .map(|a| (200, a.to_json())),
        PollAction::FinishInput { session_id } => driver
            .finish_input(SessionRef {
                session_id,
                principal: principal.clone(),
            })
            .await
            .map(|a| (200, a.to_json())),
        PollAction::Cancel { session_id } => driver
            .cancel(SessionRef {
                session_id,
                principal: principal.clone(),
            })
            .await
            .map(|c| (200, c.to_json())),
    };
    // vhco:todo ownership -- every route passes the authenticated principal to the driver, which answers 404 not_found.session for another principal's, unknown or expired session; errors become the error ResponseEnvelope (status error, data null) with the registry status (404/409/422/429)
    // vhco:step encode ResponseEnvelope::from_error -- error → envelope (status error|cancelled, data null) with its registry HTTP status
    match result {
        Ok((status, body)) => PollResponse { status, body },
        Err(e) => PollResponse {
            status: e.http_status(),
            body: ResponseEnvelope::from_error(Some(&operation), &e).to_json(),
        },
    }
}
