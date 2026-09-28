//! Infra: adapters that satisfy ports.

pub mod capy_parser;
pub mod codec;
pub mod effect_args;
pub mod execution_driver;
pub mod file_access;
pub mod file_stream;
#[cfg(feature = "grpc")]
pub mod grpc_adapter;
#[cfg(feature = "quic")]
pub mod h3_client;
pub mod http_adapter;
pub mod mcp_client;
pub mod net_tls;
#[cfg(feature = "oauth")]
pub mod oauth_adapter;
pub mod policy_broker;
pub mod policy_draft_writer;
pub mod policy_file_reader;
pub mod process_adapter;
#[cfg(feature = "quic")]
pub mod quic_adapter;
pub mod registry;
pub mod remote_client;
pub mod request_control;
#[cfg(target_os = "linux")]
pub mod sandbox_linux;
#[cfg(target_os = "macos")]
pub mod sandbox_macos;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub mod sandbox_unsupported;
#[cfg(feature = "serve")]
pub mod serve_listener;
pub mod session_driver;
pub mod socket_adapter;
pub mod source_loader;
pub mod trace_store;
pub mod udp_adapter;
/// Stand-ins for adapters whose Cargo feature is compiled out: each refuses
/// with `unsupported.feature` (PROP-2026-0002 R11).
pub mod unsupported_features;
pub mod wire_codec;
