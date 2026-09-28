//! Ports needed by the policy feature.

// vhco:port PolicyFileReader { discover(PolicyLocator) -> PolicyDiscovery; read(PolicyDiscovery) -> PolicyBytes }
pub use crate::domain::ports::PolicyFileReader;
// vhco:port PolicyEvaluator { evaluate(EffectIntent) -> Permit }
pub use crate::domain::ports::PolicyEvaluator;
// vhco:port PolicyDraftWriter { write_new(input: PolicyDraftFile) -> PolicyDraftReceipt }
pub use crate::domain::ports::PolicyDraftWriter;
