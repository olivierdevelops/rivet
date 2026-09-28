use super::ports::RequestControl;
use crate::domain::ports::CancelRequest;
use crate::domain::sessions::CancelReceipt;
use crate::domain::{RivetError, RivetResult};

// vhco:usecase execution.cancel_request(input: CancelRequest) -> CancelReceipt needs RequestControl
// vhco:label Cancel a running request
// vhco:about Signals cancellation of a caller's own running top-level request; the request's scope then drops its tasks and handles and its caller receives one `cancelled` error (exit 130). Idempotent; never reveals other principals' requests.
// vhco:example input={request_id:"req_01"} => { "request_id": "req_01", "state": "cancelling" }
pub fn cancel_request(
    input: &CancelRequest,
    control: &dyn RequestControl,
) -> RivetResult<CancelReceipt> {
    let receipt = |state: &str| CancelReceipt {
        request_id: input.request_id.clone(),
        session_id: None,
        state: state.to_string(),
    };
    // vhco:todo authorize_cancel -- look the request up; unknown IDs and requests owned by another principal are both not_found (404, exit 4) so ownership is never disclosed; the local CLI/library principal owns its own requests
    // vhco:step lookup control.lookup -- running or recently finished request and its owner
    // vhco:error unknown_request -- unknown or foreign request id => not_found.request returns
    let state = control
        .lookup(&input.request_id)
        .filter(|s| s.owner == input.principal.name)
        .ok_or_else(|| {
            RivetError::not_found(
                "not_found.request",
                format!("no running request `{}`", input.request_id),
            )
        })?;
    if state.state != "running" && state.state != "cancelling" {
        // Already terminal: report it instead of pretending to cancel.
        return Ok(receipt(&state.state));
    }
    // vhco:todo signal_cancel -- signal the shared cancellation for that request (idempotent) and acknowledge `cancelling`; the final `cancelled` terminal is delivered to the request's own caller after its scope is dropped
    // vhco:step signal control.signal -- wakes the request's select; a second signal is a no-op
    control.signal(&input.request_id);
    Ok(receipt("cancelling"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contracts::Principal;
    use crate::domain::ports::RequestState;
    use std::sync::Mutex;

    struct Fake(Mutex<Option<RequestState>>, Mutex<u32>);
    impl RequestControl for Fake {
        fn lookup(&self, _: &str) -> Option<RequestState> {
            self.0.lock().unwrap().clone()
        }
        fn signal(&self, _: &str) -> bool {
            *self.1.lock().unwrap() += 1;
            true
        }
    }

    fn req(who: &str) -> CancelRequest {
        CancelRequest {
            request_id: "req_1".into(),
            principal: Principal {
                name: who.into(),
                authenticated_by: "none".into(),
            },
        }
    }

    // vhco:test execution.cancel_request -- owners cancel, others see not_found, terminal requests report their state
    #[test]
    fn ownership_and_terminal_states() {
        let f = Fake(
            Mutex::new(Some(RequestState {
                owner: "ada".into(),
                state: "running".into(),
            })),
            Mutex::new(0),
        );
        assert_eq!(cancel_request(&req("ada"), &f).unwrap().state, "cancelling");
        assert_eq!(*f.1.lock().unwrap(), 1);
        assert_eq!(
            cancel_request(&req("eve"), &f).unwrap_err().code,
            "not_found.request"
        );
        *f.0.lock().unwrap() = Some(RequestState {
            owner: "ada".into(),
            state: "succeeded".into(),
        });
        assert_eq!(cancel_request(&req("ada"), &f).unwrap().state, "succeeded");
    }
}
