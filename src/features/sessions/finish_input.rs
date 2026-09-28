use super::ports::SessionDriver;
use crate::domain::sessions::{SessionAck, SessionRef};
use crate::domain::{RivetError, RivetResult};

// vhco:usecase sessions.finish_input(input: SessionRef) -> SessionAck needs SessionDriver
// vhco:label Finish input
// vhco:about Half-closes the session's input: already accepted messages drain, then `for message in incoming` ends; output keeps flowing until the terminal event.
// vhco:example input={session_id:"ses_1"} => { "session_id": "ses_1", "accepted_seq": null, "input_closed": true }
pub async fn finish_input(
    input: SessionRef,
    driver: &dyn SessionDriver,
) -> RivetResult<SessionAck> {
    // vhco:todo validate_finish -- a blank session_id is not_found here; the driver rejects unknown, expired or other-principal sessions with not_found.session (404) and answers an already-finished input idempotently with the same acknowledgement
    // vhco:error blank_session -- session_id is empty => not_found.session returns
    if input.session_id.trim().is_empty() {
        return Err(RivetError::not_found("not_found.session", "no session ``"));
    }
    // vhco:todo half_close -- the driver marks input closed (later sends are conflict.input_closed) and drops the input sender so queued messages drain before `incoming` ends; the run and its resources are not disposed
    // vhco:step close driver.finish_input -- mark closed, drop the sender, acknowledge {accepted_seq:null, input_closed:true}
    driver.finish_input(input).await
}
