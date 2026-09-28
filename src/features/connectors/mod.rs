//! Connectors feature: outbound MCP client connectors with reviewed schema
//! snapshots (PROP-2026-0001 Increment 6; REF S52–S61).
//!
//! ```text
//!  (request "crm.tools.search" {…})            rivet connectors sync crm
//!            │                                            │
//!            ▼                                            ▼
//!  connectors.invoke_mcp  ── allow_mcp crm/tools/search | crm/discover
//!            │               (transport: allow_network / allow_exec + sandbox, in the adapter)
//!            ▼
//!  McpClient.open ─▶ initialize ─▶ tools/call | resources/read | prompts/get | */list
//! ```

pub mod invoke_mcp;
pub mod ports;
