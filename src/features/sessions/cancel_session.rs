use super::ports::SessionDriver;
use crate::domain::sessions::{CancelReceipt, SessionRef};
use crate::domain::{RivetError, RivetResult};

// vhco:usecase sessions.cancel_session(input: SessionRef) -> CancelReceipt needs SessionDriver
// vhco:label Cancel session
// vhco:about Cancels the session's owned request tree, awaits bounded cleanup and returns a CancelReceipt; the cancelled terminal event stays readable during retention.
// vhco:example input={session_id:"ses_1"} => { "session_id": "ses_1", "request_id": "req_1", "state": "cancelled" }
pub async fn cancel_session(
    input: SessionRef,
    driver: &dyn SessionDriver,
) -> RivetResult<CancelReceipt> {
    // vhco:todo authorize_cancel -- a blank ID is not_found here; the driver resolves the session only for its owning principal (unknown, expired and foreign sessions are all not_found.session, 404, without disclosure); cancelling an already-terminal session is idempotent and returns its terminal state (succeeded/failed/cancelled)
    // vhco:error blank_session -- session_id is empty => not_found.session returns
    if input.session_id.trim().is_empty() {
        return Err(RivetError::not_found("not_found.session", "no session ``"));
    }
    // vhco:todo cancel_and_join -- the driver records the cancel first (it wins over a completion not yet recorded), closes input, fires the run's cancellation token and joins the run within the 5 s cleanup grace while it closes its scoped resources (abort only after the grace), appends one terminal `cancelled` error event (committed effects are preserved, never rolled back) and returns {session_id, request_id, state:"cancelled"}
    // vhco:step cancel driver.cancel -- abort, join, record the terminal event, return the receipt
    driver.cancel(input).await
}
