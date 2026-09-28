//! Immutable in-memory catalog built from a compiled program.

use crate::domain::RivetResult;
use crate::domain::contracts::{Catalog, CatalogQuery, RegistryEntry};
use crate::domain::io_manifest::EffectCatalog;
use crate::domain::ir::{CompiledProgram, Operation};
use crate::domain::ports::Registry;
use std::sync::Arc;

// vhco:infra registry satisfies Registry
pub struct ProgramRegistry {
    program: Arc<CompiledProgram>,
    entries: Vec<RegistryEntry>,
    effects: Arc<EffectCatalog>,
}

impl ProgramRegistry {
    pub fn new(program: Arc<CompiledProgram>) -> ProgramRegistry {
        let entries = program.operations.iter().map(entry_of).collect();
        ProgramRegistry {
            program,
            entries,
            effects: Arc::new(EffectCatalog::default()),
        }
    }

    /// Add imported MCP operations (reviewed snapshot schemas) after the
    /// program's own operations; the loader already rejected ID collisions.
    pub fn with_imports(mut self, imports: Vec<RegistryEntry>) -> ProgramRegistry {
        self.entries.extend(imports);
        self
    }

    /// Attach the effect sites computed once from the same program.
    pub fn with_effects(mut self, effects: EffectCatalog) -> ProgramRegistry {
        self.effects = Arc::new(effects);
        self
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
        raw_input_schema: None,
        raw_output_schema: None,
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

    fn effect_sites(&self) -> Arc<EffectCatalog> {
        Arc::clone(&self.effects)
    }
}
