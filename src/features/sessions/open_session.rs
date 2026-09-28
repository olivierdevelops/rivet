use super::ports::SessionDriver;
use crate::domain::sessions::{SessionOpenInput, SessionReceipt};
use crate::domain::{RivetError, RivetResult, Value};

// vhco:usecase sessions.open_session(input: SessionOpenInput) -> SessionReceipt needs SessionDriver
// vhco:label Open session
// vhco:about Opens a bounded, principal-owned request lifetime for any public operation (streaming or unary) and returns the SessionReceipt with the pinned catalog version, input/emits schemas and the first send sequence.
// vhco:example input={id:"demo.countdown", params:{}} => { "session_id": "ses_…", "next_send_seq": 1, "emits_schema": {"type": "integer"} }
pub async fn open_session(
    input: SessionOpenInput,
    driver: &dyn SessionDriver,
) -> RivetResult<SessionReceipt> {
    // vhco:todo validate_open -- require a non-empty operation ID and object/null params before anything is scheduled; the driver then resolves the public catalog entry (private/unknown → not_found), validates params with the shared dispatcher rules and enforces 8 live sessions per principal (limit.sessions, 429) before starting the run
    // vhco:step guard_id return -- blank ID → validation.required (422) with no session created
    // vhco:error blank_id -- id is empty => validation.required returns
    if input.id.trim().is_empty() {
        return Err(RivetError::validation(
            "validation.required",
            "sessions.open needs the operation `id`",
        ));
    }
    // vhco:step guard_params return -- params must be an object (or absent) → validation.params otherwise
    // vhco:error bad_params -- params is not an object => validation.params returns
    if !matches!(input.params, Value::Object(_) | Value::Null) {
        return Err(RivetError::validation(
            "validation.params",
            format!("params must be an object, got {}", input.params.type_name()),
        ));
    }
    // vhco:todo own_session -- hand the validated open to SessionDriver.open, which mints an opaque principal-bound session ID, starts the run in a host-owned task with a 16-frame bounded event log and an input channel for `incoming`, pins the catalog version and returns the receipt (expires_at = now + idle lease); no handle is exposed
    // vhco:step open driver.open -- start the owned run and return the receipt unchanged
    driver.open(input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contracts::Principal;
    use crate::domain::sessions::*;
    use async_trait::async_trait;

    struct Refuse;
    #[async_trait]
    impl SessionDriver for Refuse {
        async fn open(&self, _: SessionOpenInput) -> RivetResult<SessionReceipt> {
            Err(RivetError::internal("must not be reached"))
        }
        async fn send(&self, _: SessionSendInput) -> RivetResult<SessionAck> {
            Err(RivetError::internal("must not be reached"))
        }
        async fn finish_input(&self, _: SessionRef) -> RivetResult<SessionAck> {
            Err(RivetError::internal("must not be reached"))
        }
        async fn read(&self, _: SessionReadInput) -> RivetResult<SessionBatch> {
            Err(RivetError::internal("must not be reached"))
        }
        async fn cancel(&self, _: SessionRef) -> RivetResult<CancelReceipt> {
            Err(RivetError::internal("must not be reached"))
        }
        fn limits(&self) -> SessionLimits {
            SessionLimits::default()
        }
    }

    // vhco:test sessions.open_session -- blank IDs and non-object params are rejected before the driver runs
    #[tokio::test]
    async fn rejects_before_driver() {
        let mk = |id: &str, params: Value| SessionOpenInput {
            id: id.into(),
            params,
            principal: Principal::local(),
            connection_owned: false,
            deadline_ms: None,
            trace: None,
            restrict: None,
        };
        assert_eq!(
            open_session(mk(" ", Value::Null), &Refuse)
                .await
                .unwrap_err()
                .code,
            "validation.required"
        );
        assert_eq!(
            open_session(mk("demo.add", Value::Int(1)), &Refuse)
                .await
                .unwrap_err()
                .code,
            "validation.params"
        );
        assert_eq!(
            open_session(mk("demo.add", Value::Null), &Refuse)
                .await
                .unwrap_err()
                .code,
            "internal",
            "valid input reaches the driver"
        );
    }
}
