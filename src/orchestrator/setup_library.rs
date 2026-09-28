//! Library surface: the Rust `Runtime` API is itself a surface. It calls the
//! same use cases as the CLI, HTTP, WebSocket, polling and MCP surfaces, and
//! wires the session driver those surfaces share.
//!
//! ```text
//!  Runtime::builder().file(..).build()?
//!     ├─ request / request_as / dispatch_request ─▶ execution.request_operation
//!     ├─ list / describe / outputs               ─▶ registry use cases
//!     └─ open_session / send_input / finish_input / read_events / cancel_session ─▶ sessions use cases
//! ```

// vhco:surface library kind library calls language/compile_program, policy/load_policy, execution/request_operation, registry/describe_operations, registry/inspect_outputs, sessions/open_session, sessions/send_input, sessions/finish_input, sessions/read_events, sessions/cancel_session, serve/authorize_operation, serve/start_serve
// vhco:trigger library language/compile_program = Runtime::builder().file(PATH).build()
// vhco:trigger library policy/load_policy = Runtime::builder().policy_file(PATH).build()
// vhco:trigger library execution/request_operation = Runtime::request(ID, PARAMS, sink)
// vhco:trigger library registry/describe_operations = Runtime::list() | Runtime::describe(IDS)
// vhco:trigger library registry/inspect_outputs = Runtime::outputs(ID, all)
// vhco:trigger library sessions/open_session = Runtime::open_session(SessionOpenInput)
// vhco:trigger library sessions/send_input = Runtime::send_input(SessionSendInput)
// vhco:trigger library sessions/finish_input = Runtime::finish_input(SessionRef)
// vhco:trigger library sessions/read_events = Runtime::read_events(SessionReadInput)
// vhco:trigger library sessions/cancel_session = Runtime::cancel_session(SessionRef)
// vhco:trigger library serve/authorize_operation = Runtime::dispatch_request(Request) (principal check at depth 0)
// vhco:trigger library serve/start_serve = orchestrator::setup_serve::serve(runtime, options)
// vhco:api library execution/request_operation Runtime::request(id, params, on_data) -- invoke one operation in-process; returns Completion or RivetError
// vhco:request { "id": "string — operation ID", "params": "Value object", "on_data": "Option<Arc<dyn DataSink>>" }
// vhco:response { "request_id": "string", "trace_id": "string", "result": "Value", "data_count": "int", "effects": "none|committed|partial|unknown" }
// vhco:api library sessions/open_session Runtime::open_session(SessionOpenInput) -- open a live session (duplex or server-streaming) owned by the principal
// vhco:request { "id": "string", "params": "Value", "principal": "Principal", "connection_owned": "bool" }
// vhco:response { "session_id": "string", "request_id": "string", "catalog_version": "string", "input_schema": "json|null", "emits_schema": "json|null", "next_send_seq": "int", "expires_at": "RFC 3339" }

use super::runtime::Runtime;
use crate::domain::RivetResult;
use crate::domain::sessions::{
    CancelReceipt, SessionAck, SessionBatch, SessionLimits, SessionOpenInput, SessionReadInput,
    SessionReceipt, SessionRef, SessionSendInput,
};
use crate::features::execution::request_operation::validate_params;
use crate::features::sessions::{
    cancel_session, finish_input, open_session, read_events, send_input,
};
use crate::infra::registry::ProgramRegistry;
use crate::infra::session_driver::{LaunchFn, MintFn, SessionHost, ValidateFn};
use std::sync::Arc;

/// Re-enters the runtime from the session driver without a strong cycle.
pub type UpgradeFn = dyn Fn() -> Option<Runtime> + Send + Sync;

/// Build the shared session driver: runs go through `Runtime::dispatch_session`
/// (the one dispatcher), IDs are minted by the runtime, params are validated
/// with the dispatcher's own rules before a session is created.
pub fn session_host(
    upgrade: Arc<UpgradeFn>,
    registry: Arc<ProgramRegistry>,
    catalog_version: String,
) -> SessionHost {
    let up = Arc::clone(&upgrade);
    let launch: Arc<LaunchFn> = Arc::new(move |req, sink, input| {
        let rt = up();
        Box::pin(async move {
            match rt {
                Some(rt) => rt.dispatch_session(req, sink, input).await,
                None => Err(crate::domain::RivetError::new(
                    crate::domain::ErrorKind::Cancelled,
                    "cancelled.runtime",
                    "runtime shut down",
                )),
            }
        })
    });
    let up = Arc::clone(&upgrade);
    let mint: Arc<MintFn> = Arc::new(move |id, params, principal| match up() {
        Some(rt) => rt.new_request(id, params, principal),
        None => crate::domain::contracts::Request {
            request_id: "req_shutdown".into(),
            trace_id: "tr_shutdown".into(),
            operation_id: id.to_string(),
            params,
            principal,
            parent_request_id: None,
            depth: 0,
            deadline_ms: 1,
            include_private: false,
        },
    });
    let validate: Arc<ValidateFn> = Arc::new(validate_params);
    SessionHost::new(
        registry,
        launch,
        mint,
        validate,
        catalog_version,
        SessionLimits::default(),
    )
}

impl Runtime {
    /// `rivet.sessions.open` for library hosts.
    pub async fn open_session(&self, input: SessionOpenInput) -> RivetResult<SessionReceipt> {
        open_session::open_session(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.send` for library hosts.
    pub async fn send_input(&self, input: SessionSendInput) -> RivetResult<SessionAck> {
        send_input::send_input(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.finish_input` for library hosts.
    pub async fn finish_input(&self, input: SessionRef) -> RivetResult<SessionAck> {
        finish_input::finish_input(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.read` for library hosts.
    pub async fn read_events(&self, input: SessionReadInput) -> RivetResult<SessionBatch> {
        read_events::read_events(input, self.sessions().as_ref()).await
    }

    /// `rivet.sessions.cancel` for library hosts.
    pub async fn cancel_session(&self, input: SessionRef) -> RivetResult<CancelReceipt> {
        cancel_session::cancel_session(input, self.sessions().as_ref()).await
    }
}
