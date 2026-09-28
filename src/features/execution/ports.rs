//! Ports needed by the execution feature.

// vhco:port ExecutionDriver { drive(ExecutionPlan) -> Completion }
pub use crate::domain::ports::ExecutionDriver;
pub use crate::domain::ports::{DataSink, Registry};
