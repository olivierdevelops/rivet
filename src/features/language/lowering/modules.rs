//! Module namespacing after lowering (PROP-2026-0002 R20, R21, R24).
//!
//! ```text
//!  users.rivet (alias "users")          app.rivet (alias "")
//!    operation get        ─▶ users.get     (users.get {id: 1})       ─▶ (request "users.get" {id: 1})
//!    (get {id: 1})        ─▶ (request "users.get" …)   (request "users.list") ─▶ unchanged
//!    (request "b.x")      ─▶ (request "users.b.x" …)   users imported without `public` ─▶ users.* private
//!
//!  a namespaced ID equal to another ID ─▶ check.import_collision (at the import)
//!  a module call that resolves nowhere  ─▶ check.unknown_operation
//!  another module's `private true` op   ─▶ check.unknown_operation (private to its module)
//! ```

use super::lower::collect_calls;
use crate::domain::errors::codes;
use crate::domain::ir::{BUILTIN_FUNCTIONS, CompiledProgram, Expr, walk_body_exprs_mut};
use crate::domain::source::{ModuleRef, SourceBundle, SourceSpan};
use crate::domain::{RivetError, Value};
use std::collections::HashMap;

/// What one file may call without qualification.
struct FileScope {
    alias: String,
    /// Operation IDs declared in the file (before namespacing).
    local: Vec<String>,
    /// Import alias → canonical module alias.
    imports: HashMap<String, String>,
    /// Connector names declared in the file (their MCP/gRPC IDs stay as declared).
    connectors: Vec<String>,
}

impl FileScope {
    fn qualify(&self, id: &str) -> String {
        if self.alias.is_empty() {
            id.to_string()
        } else {
            format!("{}.{id}", self.alias)
        }
    }

    /// The catalog ID a call target names from this file, or `None` when a
    /// module cannot resolve it.
    fn resolve(&self, target: &str) -> Option<String> {
        if target.starts_with("rivet.") {
            return Some(target.to_string());
        }
        if let Some((first, rest)) = target.split_once('.')
            && let Some(canonical) = self.imports.get(first)
        {
            return Some(format!("{canonical}.{rest}"));
        }
        if self.local.iter().any(|l| l == target) {
            return Some(self.qualify(target));
        }
        if self.connectors.iter().any(|c| {
            target.len() > c.len()
                && target.starts_with(c.as_str())
                && target.as_bytes()[c.len()] == b'.'
        }) {
            return Some(target.to_string());
        }
        // The entry bundle keeps 0.1.0 behaviour: check_calls reports unknown IDs.
        self.alias.is_empty().then(|| target.to_string())
    }
}

/// Span of the import that brought `alias` into the bundle (its first
/// importer), if it was imported rather than loaded by the host.
pub fn import_span(modules: &[ModuleRef], alias: &str) -> Option<SourceSpan> {
    modules
        .iter()
        .flat_map(|m| m.imports.iter())
        .find(|i| i.target == alias)
        .map(|i| i.span.clone())
}

/// Namespace every operation of an imported file, rewrite each literal call
/// target in its file's scope, hide internal modules and report collisions.
/// Also rewrites `(ID {…})` calls to `(request "ID" {…})` in single-file
/// bundles, where every file shares the entry namespace.
pub fn namespace_modules(
    program: &mut CompiledProgram,
    bundle: &SourceBundle,
    errors: &mut Vec<RivetError>,
    warnings: &mut Vec<RivetError>,
) {
    let modules = &bundle.modules;
    let scope_of = |file: &str, program: &CompiledProgram| -> FileScope {
        let m = modules.iter().find(|m| m.file == file);
        FileScope {
            alias: m.map(|m| m.alias.clone()).unwrap_or_default(),
            local: program
                .operations
                .iter()
                .filter(|o| o.file == file)
                .map(|o| o.id.clone())
                .collect(),
            imports: m
                .map(|m| {
                    m.imports
                        .iter()
                        .filter(|i| !i.target.is_empty())
                        .map(|i| (i.alias.clone(), i.target.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            connectors: program
                .connectors
                .iter()
                .filter(|c| c.span.file == file)
                .map(|c| c.name.clone())
                .collect(),
        }
    };
    let mut files: Vec<String> = program.operations.iter().map(|o| o.file.clone()).collect();
    files.dedup();
    let scopes: HashMap<String, FileScope> = files
        .iter()
        .map(|f| (f.clone(), scope_of(f, program)))
        .collect();
    // Declared-private operations by their final ID, before visibility folds in.
    let mut declared_private: HashMap<String, String> = HashMap::new();

    // 1. Namespace IDs (and default display names) of imported files.
    for op in &mut program.operations {
        let Some(scope) = scopes.get(&op.file) else {
            continue;
        };
        op.module = scope.alias.clone();
        if !scope.alias.is_empty() {
            let full = scope.qualify(&op.id);
            if op.name == op.id {
                op.name = full.clone();
            }
            op.id = full;
        }
        if op.private {
            declared_private.insert(op.id.clone(), op.module.clone());
        }
    }

    // 2. Resolve every literal call target in its own file's scope.
    for op in &mut program.operations {
        let Some(scope) = scopes.get(&op.file) else {
            continue;
        };
        let caller_module = op.module.clone();
        let mut problems: Vec<RivetError> = Vec::new();
        let mut check = |target: &str, span: &SourceSpan| -> String {
            match scope.resolve(target) {
                Some(id) => {
                    if let Some(owner) = declared_private.get(&id)
                        && *owner != caller_module
                    {
                        problems.push(RivetError::syntax(
                            "check.unknown_operation",
                            format!("`{id}` is private to module `{owner}`"),
                            Some(span.clone()),
                        ));
                    }
                    id
                }
                None => {
                    problems.push(RivetError::syntax(
                        "check.unknown_operation",
                        format!(
                            "`{target}` is not an operation of module `{}` or of its imports",
                            scope.alias
                        ),
                        Some(span.clone()),
                    ));
                    target.to_string()
                }
            }
        };
        walk_body_exprs_mut(&mut op.body, &mut |e: &mut Expr| {
            let Expr::Call { func, args, span } = e else {
                return;
            };
            if func == "request" || func == "request.stream" {
                if let Some(target) = args.first().and_then(Expr::const_text) {
                    let id = check(&target, span);
                    args[0] = Expr::Lit(Value::Text(id));
                }
            } else if !BUILTIN_FUNCTIONS.contains(&func.as_str()) {
                // `(get {id: 1})` / `(users.get {…})`: a call by operation ID.
                let id = check(func, span);
                let mut new_args = vec![Expr::Lit(Value::Text(id))];
                new_args.append(args);
                *e = Expr::Call {
                    func: "request".into(),
                    args: new_args,
                    span: span.clone(),
                };
            }
        });
        errors.append(&mut problems);
        let mut calls = Vec::new();
        collect_calls(&op.body, &mut calls);
        calls.sort();
        calls.dedup();
        op.calls = calls;
    }
    if modules.is_empty() {
        return;
    }

    // 3. Collisions: a namespaced ID equal to any other operation's ID.
    for (i, op) in program.operations.iter().enumerate() {
        if op.module.is_empty() {
            continue;
        }
        if let Some(other) = program
            .operations
            .iter()
            .enumerate()
            .find(|(j, o)| *j != i && o.id == op.id)
            .map(|(_, o)| o)
        {
            if !other.module.is_empty() && other.module == op.module {
                continue; // same file twice: registry.duplicate_id reports it
            }
            if !other.module.is_empty() && other.file < op.file {
                continue; // reported once, from the other side
            }
            errors.push(RivetError::syntax(
                codes::IMPORT_COLLISION,
                format!(
                    "`{}` from {} collides with the operation declared at {}:{}",
                    op.id, op.file, other.span.file, other.span.start_line
                ),
                import_span(modules, &op.module).or_else(|| Some(op.span.clone())),
            ));
        }
    }
    for (what, decls) in [
        ("connector", &program.connectors),
        ("auth profile", &program.auth_profiles),
    ] {
        for (i, d) in decls.iter().enumerate() {
            if let Some(first) = decls[..i]
                .iter()
                .find(|o| o.name == d.name && o.span.file != d.span.file)
            {
                errors.push(RivetError::syntax(
                    codes::IMPORT_COLLISION,
                    format!(
                        "{what} `{}` in {} collides with the one declared at {}:{}; connector and auth profile names are unique in a bundle (the loader's policy grants them by name)",
                        d.name, d.span.file, first.span.file, first.span.start_line
                    ),
                    Some(d.span.clone()),
                ));
            }
        }
    }

    // 4. Internal modules: callable inside the bundle, never listed.
    for op in &mut program.operations {
        if let Some(m) = modules.iter().find(|m| m.file == op.file)
            && !m.public
        {
            op.private = true;
        }
    }

    // 5. A module's own policy.json is ignored: the loader's policy governs.
    for m in modules.iter().filter(|m| m.policy_ignored) {
        warnings.push(
            RivetError::syntax(
                codes::MODULE_POLICY_IGNORED,
                format!(
                    "the policy.json beside {} is ignored: module `{}` runs under the loader's policy",
                    m.file, m.alias
                ),
                import_span(modules, &m.alias),
            )
            .with_hint("grant what the module needs in the entry bundle's policy.json (or the host policy)"),
        );
    }
}
