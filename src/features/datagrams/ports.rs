//! Ports needed by the datagrams feature.

// vhco:port DatagramDriver { resolve(string, u16) -> SocketAddr[]; exchange(DatagramPlan) -> DatagramResult }
pub use crate::domain::ports::DatagramDriver;
pub use crate::domain::ports::PolicyEvaluator;
