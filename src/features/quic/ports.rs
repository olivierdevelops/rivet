//! Ports needed by the quic feature.

// vhco:port QuicDriver { resolve(string, u16) -> SocketAddr[]; exchange(QuicPlan) -> QuicResult }
pub use crate::domain::ports::PolicyEvaluator;
pub use crate::domain::ports::QuicDriver;
