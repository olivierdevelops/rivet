//! Rivet: protocol-visible operations, scoped resources and composable DAGs.
//!
//! Five VHCO buckets (AGENTS.md): `domain` (shared data), `features` (pure use
//! cases and their ports), `io` (surface adapters), `infra` (port adapters) and
//! `orchestrator` (composition root).

// RivetError deliberately carries the full error contract (kind, code, span,
// details, cause, suppressed errors, effects); errors are the cold path, so the
// size of `Result<_, RivetError>` is accepted instead of boxing every error.
#![allow(clippy::result_large_err)]

pub mod domain;
pub mod features;
pub mod infra;
pub mod io;
pub mod orchestrator;

pub use orchestrator::runtime::{Runtime, RuntimeBuilder};
pub use orchestrator::setup_library::Module;
