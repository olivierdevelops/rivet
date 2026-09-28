//! Rivet: protocol-visible operations, scoped resources and composable DAGs.
//!
//! Five VHCO buckets (AGENTS.md): `domain` (shared data), `features` (pure use
//! cases and their ports), `io` (surface adapters), `infra` (port adapters) and
//! `orchestrator` (composition root).

pub mod domain;
pub mod features;
pub mod infra;
pub mod io;
pub mod orchestrator;

pub use orchestrator::runtime::{Runtime, RuntimeBuilder};
