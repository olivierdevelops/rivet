//! Domain: the shared vocabulary. Imports nothing internal.

pub mod auth;
pub mod contracts;
pub mod dag;
pub mod effect_checks;
pub mod errors;
pub mod files;
pub mod grpc;
pub mod io_manifest;
pub mod ir;
pub mod mcp;
pub mod outputs;
pub mod policy;
pub mod ports;
pub mod serve;
pub mod sessions;
pub mod source;
pub mod syntax_tree;
pub mod transport;
pub mod transports;
pub mod value;

pub use errors::{EffectsStatus, ErrorKind, RivetError, RivetResult};
pub use value::Value;
