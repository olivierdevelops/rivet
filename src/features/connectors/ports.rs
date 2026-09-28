//! Ports needed by the connectors feature.

// vhco:port McpClient { catalog() -> McpCatalog; open(McpConnectorInfo, McpContext) -> McpSession }
pub use crate::domain::ports::{McpClient, McpSession};
// vhco:port PolicyEvaluator { evaluate(EffectIntent) -> Permit }
pub use crate::domain::ports::PolicyEvaluator;
