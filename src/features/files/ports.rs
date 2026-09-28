//! Ports needed by the files feature.

// vhco:port FileAccess { apply(FileOperation) -> FileResult }
pub use crate::domain::ports::FileAccess;
pub use crate::domain::ports::PolicyEvaluator;
