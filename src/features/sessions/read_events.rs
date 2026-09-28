use super::ports::SessionDriver;
use crate::domain::sessions::{SessionBatch, SessionReadInput};
use crate::domain::{RivetError, RivetResult};

// vhco:usecase sessions.read_events(input: SessionReadInput) -> SessionBatch needs SessionDriver
// vhco:label Read events
// vhco:about Long-polls the session's ordered event log after a cursor; `after_seq` acknowledges delivered events so they can be evicted, and exactly one terminal result/error ends the sequence.
// vhco:example input={session_id:"ses_1", after_seq:0, wait_ms:1000} => { "events": [{"seq": 1, "type": "data", "data": 3}], "last_seq": 1, "terminal": false }
pub async fn read_events(
    input: SessionReadInput,
    driver: &dyn SessionDriver,
) -> RivetResult<SessionBatch> {
    // vhco:todo validate_cursor -- clamp wait_ms (default 1000, max 5000) and max_events (default 16, host cap 16, min 1) from the driver limits; the driver checks ownership (not_found.session), serializes concurrent readers, acknowledges events ≤ after_seq, rejects a cursor beyond the last delivered event (conflict.cursor, 409) and a cursor behind evicted events (stream.cursor_expired, 409)
    // vhco:error blank_session -- session_id is empty => not_found.session returns
    if input.session_id.trim().is_empty() {
        return Err(RivetError::not_found("not_found.session", "no session ``"));
    }
    let limits = driver.limits();
    // vhco:step clamp limits.clamp_wait -- wait_ms defaults to 1000 and is capped at 5000; max_events defaults to 16 and is capped at 16
    let input = SessionReadInput {
        wait_ms: Some(limits.clamp_wait(input.wait_ms)),
        max_events: Some(limits.clamp_max_events(input.max_events)),
        ..input
    };
    // vhco:todo read_bounded -- the driver returns the retained events after the cursor in order (up to max_events), waiting up to wait_ms when none are ready and returning early when one arrives; producers block while 16 events are retained (backpressure); the batch reports last_seq and terminal:true once the single terminal event is included; reading refreshes the idle lease
    // vhco:step read driver.read -- bounded long-poll of the event log
    driver.read(input).await
}
