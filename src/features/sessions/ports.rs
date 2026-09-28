//! Ports needed by the sessions feature.

// vhco:port SessionDriver { open(SessionOpenInput) -> SessionReceipt; send(SessionSendInput) -> SessionAck; finish_input(SessionRef) -> SessionAck; read(SessionReadInput) -> SessionBatch; cancel(SessionRef) -> CancelReceipt }
pub use crate::domain::ports::SessionDriver;
