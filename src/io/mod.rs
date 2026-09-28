//! IO: surface adapters (argument parsing, wire encoding, rendering).

#[cfg(feature = "cli")]
pub mod cli;
pub mod http;
pub mod mcp;
pub mod ws;
