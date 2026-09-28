//! Domain: the shared vocabulary. Imports nothing internal.

pub mod auth;
pub mod call_graph;
pub mod cancel;
pub mod capabilities;
pub mod const_eval;
pub mod contracts;
pub mod dag;
pub mod effect_checks;
pub mod envelope;
pub mod errors;
pub mod files;
pub mod grpc;
pub mod highlight;
pub mod io_manifest;
pub mod ir;
pub mod mcp;
pub mod modules;
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
