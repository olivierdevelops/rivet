//! Ports needed by the execution feature.

// vhco:port ExecutionDriver { drive(ExecutionPlan) -> Completion }
pub use crate::domain::ports::ExecutionDriver;
pub use crate::domain::ports::{DataSink, Registry};
// vhco:port RequestControl { lookup(string) -> RequestState; signal(string) -> bool }
pub use crate::domain::ports::RequestControl;
// vhco:port DagNodeRunner { run_node(usize, List<(String, Value)>) -> Value; now() -> SystemTime; cancel_nodes() -> () }
pub use crate::domain::dag::DagNodeRunner;
