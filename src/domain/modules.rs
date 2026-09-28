//! File modules loaded by a host (PROP-2026-0002 R22): the load request, the
//! module summary a host sees, and the immutable catalog snapshot a runtime
//! swaps in when a module is loaded.
//!
//! ```text
//!  rt.load("./users.rivet") ──▶ ModuleLoad{path, alias?} ──▶ registry.load_module
//!        ──▶ CatalogSnapshot v2 (Arc swap) ──▶ ModuleSummary{alias "users", operations [get, list]}
//!  requests already running keep CatalogSnapshot v1 until they finish
//! ```

use super::errors::RivetError;
use super::ir::CompiledProgram;
use super::source::SourceBundle;
use std::sync::Arc;

// vhco:domain ModuleLoad { path: string; alias?: string }
/// `Runtime::load(path)` / `load_as(path, alias)`: `path` is relative to the
/// runtime root (or absolute inside it); the alias defaults to the file stem.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ModuleLoad {
    pub path: String,
    pub alias: Option<String>,
}

// vhco:domain ModuleSummary { alias: string; file: string; public: bool; operations: string[]; warnings: RivetError[] }
/// What a host gets back from a load: the alias, the file, the module's own
/// operation IDs (short, without the alias) and any load warnings
/// (`check.module_policy_ignored`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ModuleSummary {
    pub alias: String,
    pub file: String,
    pub public: bool,
    pub operations: Vec<String>,
    pub warnings: Vec<RivetError>,
}

// vhco:domain CatalogSnapshot { version: string; bundle: SourceBundle; program: CompiledProgram }
/// One immutable catalog: every resolved file and the program compiled from
/// them. A runtime holds the current snapshot behind an `Arc`; a load builds
/// the next one and swaps it in, and in-flight requests keep theirs.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CatalogSnapshot {
    /// The program's source hash (`sha256:…` over every file).
    pub version: String,
    pub bundle: SourceBundle,
    pub program: Arc<CompiledProgram>,
}

impl CatalogSnapshot {
    pub fn new(bundle: SourceBundle, program: Arc<CompiledProgram>) -> CatalogSnapshot {
        CatalogSnapshot {
            version: program.source_hash.clone(),
            bundle,
            program,
        }
    }
}
