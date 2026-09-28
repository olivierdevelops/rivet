use super::authorize_operation::require_operation;
use super::ports::SessionDriver;
use crate::domain::RivetError;
use crate::domain::contracts::error_envelope;
use crate::domain::serve::{OperationAccess, PollAction, PollResponse, PollRoute};
use crate::domain::sessions::{SessionOpenInput, SessionReadInput, SessionRef, SessionSendInput};

// vhco:usecase serve.project_polling(input: PollRoute) -> PollResponse needs SessionDriver
// vhco:label Project polling
// vhco:about The HTTP polling routes are a projection of the session operations: open → 202 SessionReceipt with events_url, events → SessionBatch, input/finish_input/cancel → SessionAck / CancelReceipt, all bound to the authenticated principal.
// vhco:example input={action:{open:{id:"demo.countdown"}}} => { "status": 202, "body": {"session_id": "ses_…", "events_url": "/v1/requests/ses_…/events"} }
pub async fn project_polling(input: PollRoute, driver: &dyn SessionDriver) -> PollResponse {
    let principal = input.principal.clone();
    let limits = driver.limits();
    let result: Result<(u16, serde_json::Value), RivetError> = match input.action {
        // vhco:todo open -- POST /v1/requests {id, params, deadline_ms?}: authorize the operation for the principal (403 permission.denied), then SessionDriver.open (principal-owned, survives reconnect; deadline_ms capped by the host at 600000; the request's W3C traceparent becomes the session's trace) and answer 202 SessionReceipt with events_url=/v1/requests/{session_id}/events; unary operations work the same way and their batch holds one terminal result event
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
                        (202, r.to_json())
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
    // vhco:todo ownership -- every route passes the authenticated principal to the driver, which answers 404 not_found.session for another principal's, unknown or expired session; errors become the ErrorEnvelope with the registry status (404/409/422/429)
    // vhco:step encode error_envelope -- error → {request_id, trace_id, error} with its registry HTTP status
    match result {
        Ok((status, body)) => PollResponse { status, body },
        Err(e) => PollResponse {
            status: e.http_status(),
            body: error_envelope(
                e.request_id.as_deref().unwrap_or(""),
                e.trace_id.as_deref().unwrap_or(""),
                &e,
            ),
        },
    }
}
