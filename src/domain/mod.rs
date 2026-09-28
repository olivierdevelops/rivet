//! Domain: the shared vocabulary. Imports nothing internal.

pub mod contracts;
pub mod errors;
pub mod files;
pub mod io_manifest;
pub mod ir;
pub mod outputs;
pub mod policy;
pub mod ports;
pub mod source;
pub mod syntax_tree;
pub mod value;

pub use errors::{EffectsStatus, ErrorKind, RivetError, RivetResult};
pub use value::Value;
