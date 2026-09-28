//! Immutable in-memory catalog built from a compiled program.

use crate::domain::RivetResult;
use crate::domain::contracts::{Catalog, CatalogQuery, RegistryEntry};
use crate::domain::ir::{CompiledProgram, Operation};
use crate::domain::ports::Registry;
use std::sync::Arc;

// vhco:infra registry satisfies Registry
pub struct ProgramRegistry {
    program: Arc<CompiledProgram>,
    entries: Vec<RegistryEntry>,
}

impl ProgramRegistry {
    pub fn new(program: Arc<CompiledProgram>) -> ProgramRegistry {
        let entries = program.operations.iter().map(entry_of).collect();
        ProgramRegistry { program, entries }
    }
}

pub fn entry_of(op: &Operation) -> RegistryEntry {
    RegistryEntry {
        id: op.id.clone(),
        name: op.name.clone(),
        description: op.description.clone(),
        kind: op.kind,
        private: op.private,
        params: op.params.clone(),
        output: op.output.clone(),
        emits: op.emits.clone(),
        receives: op.receives.clone(),
        errors: op.errors.clone(),
        source: op.span.clone(),
    }
}

impl Registry for ProgramRegistry {
    fn describe(&self, query: &CatalogQuery) -> RivetResult<Catalog> {
        let entries = self
            .entries
            .iter()
            .filter(|e| query.include_private || !e.private)
            .filter(|e| query.ids.is_empty() || query.ids.contains(&e.id))
            .cloned()
            .collect();
        Ok(Catalog { entries })
    }

    fn program(&self) -> Arc<CompiledProgram> {
        Arc::clone(&self.program)
    }
}
