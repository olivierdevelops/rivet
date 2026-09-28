//! Load a `.rivet` file into a running runtime as a module object
//! (PROP-2026-0002 R22, R24).
//!
//! ```text
//!  ModuleLoad{path "./users.rivet", alias?}
//!    ─▶ alias = alias ∨ file stem ("users")      bad / reserved ─▶ syntax.import
//!    ─▶ already a namespace of the catalog?        yes ─▶ check.import_duplicate
//!    ─▶ next bundle = current roots + {users, public, depth 0}
//!    ─▶ CatalogStore.compile (resolve_imports + compile_program)   error ─▶ returned, nothing swapped
//!    ─▶ CatalogStore.publish (Arc swap)   running requests keep the old snapshot
//!    ─▶ ModuleSummary{alias, file, public, operations [get, list], warnings}
//! ```

use super::ports::CatalogStore;
use crate::domain::errors::codes;
use crate::domain::modules::{ModuleLoad, ModuleSummary};
use crate::domain::source::{ModuleRef, SourceBundle, join_root, rel_to_root, resolve_rel};
use crate::domain::{ErrorKind, RivetError, RivetResult};

// vhco:usecase registry.load_module(input: ModuleLoad) -> ModuleSummary needs CatalogStore
// vhco:label Load a module
// vhco:about Loads a .rivet file into a running runtime as a module object (`Runtime::load` / `load_as`): picks and checks the alias, compiles the current bundle plus the new root through the catalog store, and publishes the next immutable catalog snapshot; requests already running keep theirs (PROP-2026-0002 R22, R24).
// vhco:example input={path:"./users.rivet"} => { "alias": "users", "file": "users.rivet", "public": true, "operations": ["get", "list"], "warnings": [] }
pub fn load_module(input: &ModuleLoad, store: &dyn CatalogStore) -> RivetResult<ModuleSummary> {
    // vhco:step current store.current -- the snapshot new requests use today
    let current = store.current();
    let root = current.bundle.root.clone();
    let rel = module_rel(&input.path, &root)?;

    // vhco:todo pick_alias -- the given alias or the file stem (`./lib/billing.rivet` → billing); an identifier other than `rivet` (syntax.import), not already a namespace of the current snapshot (check.import_duplicate)
    // vhco:step alias module_alias -- file stem or the given alias; identifier, not reserved, not loaded yet
    let alias = module_alias(input, &rel)?;
    // vhco:error alias -- the alias is not an identifier, is `rivet`, or is already loaded => syntax.import / check.import_duplicate (exit 2) returns
    if current.bundle.modules.iter().any(|m| m.alias == alias) {
        return Err(RivetError::syntax(
            codes::IMPORT_DUPLICATE,
            format!("a module named `{alias}` is already loaded; use load_as(path, alias)"),
            None,
        ));
    }

    // vhco:todo compile_next -- the current bundle's roots plus the file as a public root ModuleRef (depth 0), compiled through CatalogStore.compile; any error is returned unchanged and the current snapshot stays
    let mut next = SourceBundle {
        entry: current.bundle.entry.clone(),
        root: root.clone(),
        files: current.bundle.files.clone(),
        modules: current
            .bundle
            .modules
            .iter()
            .filter(|m| m.depth == 0)
            .cloned()
            .collect(),
    };
    let file = join_root(&root, &rel);
    next.modules.push(ModuleRef {
        alias: alias.clone(),
        file: file.clone(),
        public: true,
        depth: 0,
        imports: Vec::new(),
        policy_ignored: false,
    });
    // vhco:step compile store.compile -- resolve_imports + compile_program over the current roots plus the new one
    // vhco:error compile -- the file or one of its imports fails to resolve or compile => the resolve/compile error returns
    let snapshot = store.compile(&next)?;

    // vhco:todo summarize -- ModuleSummary{alias, file, public, operations = the module's own IDs without the alias, warnings = its check.module_policy_ignored warnings}
    let prefix = format!("{alias}.");
    let module = snapshot
        .bundle
        .modules
        .iter()
        .find(|m| m.alias == alias)
        .cloned()
        .unwrap_or_default();
    let summary = ModuleSummary {
        alias: alias.clone(),
        file: module.file,
        public: module.public,
        operations: snapshot
            .program
            .operations
            .iter()
            .filter(|o| o.module == alias && !o.private)
            .filter_map(|o| o.id.strip_prefix(&prefix).map(str::to_string))
            .collect(),
        warnings: snapshot
            .program
            .warnings
            .iter()
            .filter(|w| {
                w.code == codes::MODULE_POLICY_IGNORED
                    && snapshot
                        .bundle
                        .modules
                        .iter()
                        .filter(|m| m.alias == alias || m.alias.starts_with(&prefix))
                        .any(|m| w.message.contains(&format!("module `{}`", m.alias)))
            })
            .cloned()
            .collect(),
    };

    // vhco:todo publish -- CatalogStore.publish swaps the snapshot in atomically; requests already running keep theirs, new requests see ALIAS.ID
    // vhco:step publish store.publish -- atomic Arc swap; in-flight requests keep their snapshot
    store.publish(snapshot)?;
    // vhco:step summary ModuleSummary -- short operation IDs and load warnings
    Ok(summary)
}

/// The module path relative to the runtime root (`./users.rivet` →
/// `users.rivet`); outside the root is permission.import_outside_root.
fn module_rel(path: &str, root: &str) -> RivetResult<String> {
    let outside = || {
        RivetError::new(
            ErrorKind::Permission,
            codes::IMPORT_OUTSIDE_ROOT,
            format!("{path} is outside the runtime root {root}"),
        )
    };
    let relative = if path.starts_with('/') {
        let r = root.trim_end_matches('/');
        match path.strip_prefix(&format!("{r}/")) {
            Some(rest) if r.starts_with('/') => rest.to_string(),
            _ => return Err(outside()),
        }
    } else {
        rel_to_root(path, ".")
    };
    resolve_rel("", &relative).ok_or_else(outside)
}

fn module_alias(input: &ModuleLoad, rel: &str) -> RivetResult<String> {
    let alias = match &input.alias {
        Some(a) => a.clone(),
        None => {
            let base = rel.rsplit('/').next().unwrap_or(rel);
            base.strip_suffix(".rivet").unwrap_or(base).to_string()
        }
    };
    let ident = alias
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !ident || alias == "rivet" {
        return Err(RivetError::syntax(
            codes::SYNTAX_IMPORT,
            format!("`{alias}` is not a valid module alias (an identifier; `rivet` is reserved)"),
            None,
        )
        .with_hint("pass an alias: load_as(path, \"name\")"));
    }
    Ok(alias)
}
