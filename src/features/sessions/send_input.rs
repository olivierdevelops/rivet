use super::ports::SessionDriver;
use crate::domain::sessions::{SessionAck, SessionSendInput};
use crate::domain::{ErrorKind, RivetError, RivetResult};

// vhco:usecase sessions.send_input(input: SessionSendInput) -> SessionAck needs SessionDriver
// vhco:label Send input
// vhco:about Enqueues one input message for an operation that declares `receives`; the acknowledgement proves local queue acceptance only.
// vhco:example input={session_id:"ses_1", send_seq:1, data:"hi"} => { "session_id": "ses_1", "accepted_seq": 1, "input_closed": false }
pub async fn send_input(
    input: SessionSendInput,
    driver: &dyn SessionDriver,
) -> RivetResult<SessionAck> {
    // vhco:todo validate_input -- send_seq must be ≥ 1 (checked here); the driver then checks session ownership (another principal's or an expired session is not_found), that the operation declares `receives` (else validation.no_input), the item against the receives schema (422), and the sequence rules: next seq enqueues, an identical retry of the most recent seq is acknowledged without a second enqueue, any other seq or a changed payload is conflict.input_sequence (409), sends after finish_input are conflict.input_closed
    // vhco:step guard_seq return -- send_seq 0 → conflict.input_sequence before touching the session
    // vhco:error zero_seq -- send_seq is 0 => conflict.input_sequence returns
    if input.send_seq == 0 {
        return Err(RivetError::new(
            ErrorKind::Conflict,
            "conflict.input_sequence",
            "send_seq starts at 1",
        ));
    }
    // vhco:todo enqueue_input -- the driver awaits capacity in the 16-message input queue within a 5 s action deadline (limit.input_queue on timeout), enqueues, records the seq and payload hash, and acknowledges {session_id, accepted_seq, input_closed:false}; it never claims remote processing
    // vhco:step send driver.send -- sequence check, bounded enqueue and acknowledgement
    driver.send(input).await
}
