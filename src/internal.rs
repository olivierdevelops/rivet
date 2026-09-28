//! Rivet's implementation: the five VHCO buckets (AGENTS.md). Public only for
//! Rivet's own tests, examples and the `rivet-ffi` crate; not a stable API.
//! Embedders use the facade at the crate root (PROP-2026-0002 R10).
//!
//! ```text
//!  rivet::internal::{domain, features, io, infra, orchestrator}   ← #[doc(hidden)]
//!  rivet::{Runtime, Module, InputEnvelope, ResponseEnvelope, …}   ← the facade
//! ```

#[path = "domain/mod.rs"]
pub mod domain;
#[path = "features/mod.rs"]
pub mod features;
#[path = "infra/mod.rs"]
pub mod infra;
#[path = "io/mod.rs"]
pub mod io;
#[path = "orchestrator/mod.rs"]
pub mod orchestrator;
