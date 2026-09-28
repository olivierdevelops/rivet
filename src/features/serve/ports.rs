//! Ports needed by the serve feature.

// vhco:port ServeListener { bind(ServeConfig) -> ListenerHandle; mount(SurfaceMount) -> MountReceipt }
pub use crate::domain::ports::ServeListener;
// vhco:port Authenticator { authenticate(AuthnInput) -> Principal }
pub use crate::domain::ports::Authenticator;
// vhco:port WsConnection { send(WsFrame) -> WsAck }
pub use crate::domain::ports::WsConnection;
// vhco:port SessionDriver { open(SessionOpenInput) -> SessionReceipt; send(SessionSendInput) -> SessionAck; finish_input(SessionRef) -> SessionAck; read(SessionReadInput) -> SessionBatch; cancel(SessionRef) -> CancelReceipt }
pub use crate::domain::ports::SessionDriver;
