//! Infra: adapters that satisfy ports.

pub mod capy_parser;
pub mod codec;
pub mod effect_args;
pub mod execution_driver;
pub mod file_access;
pub mod grpc_adapter;
pub mod http_adapter;
pub mod mcp_client;
pub mod net_tls;
pub mod policy_broker;
pub mod policy_draft_writer;
pub mod policy_file_reader;
pub mod process_adapter;
pub mod quic_adapter;
pub mod registry;
#[cfg(target_os = "linux")]
pub mod sandbox_linux;
#[cfg(target_os = "macos")]
pub mod sandbox_macos;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub mod sandbox_unsupported;
pub mod serve_listener;
pub mod session_driver;
pub mod socket_adapter;
pub mod source_loader;
pub mod trace_store;
pub mod udp_adapter;
pub mod wire_codec;
