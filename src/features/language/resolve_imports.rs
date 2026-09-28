//! File modules (PROP-2026-0002 R19–R21, R24): follow every
//! `import "PATH" as ALIAS [public]` from the bundle's roots, read each file
//! once below the runtime root, and give it its canonical namespace.
//!
//! ```text
//!  roots (depth 0): app.rivet ("")  ·  host-loaded users.rivet ("users")
//!     │ breadth-first, shallowest import names a file
//!     ▼
//!  app.rivet ──import "./users.rivet" as users──▶ users.rivet  alias "users"    depth 1
//!      │                                              └─ import "./b.rivet" as b ─▶ alias "users.b" depth 2
//!      └──import "./billing.rivet" as billing public──▶ billing.rivet alias "billing" (public)
//!
//!  PATH ../ outside root ─▶ permission.import_outside_root   missing ─▶ not_found.import
//!  alias twice in a file ─▶ check.import_duplicate           a → b → a ─▶ check.import_cycle
//!  > 256 files or depth > 16 ─▶ limit.imports                policy.json beside a module ─▶ warning later
//! ```

use super::lowering::lower::Lowerer;
use super::ports::{Parser, SourceLoader};
use crate::domain::errors::codes;
use crate::domain::source::{
    ImportDecl, ModuleRef, SourceBundle, SourceFile, join_root, rel_to_root, resolve_rel,
};
use crate::domain::{ErrorKind, RivetError, RivetResult};
use std::collections::{HashMap, VecDeque};

/// At most this many files in one bundle (entry and loaded roots included).
pub const MAX_FILES: usize = 256;
/// Imports nest at most this deep below a root.
pub const MAX_DEPTH: u32 = 16;

struct Edge {
    from: usize,
    to: usize,
    public: bool,
    span: crate::domain::source::SourceSpan,
}

// vhco:usecase language.resolve_imports(input: SourceBundle) -> SourceBundle needs Parser, SourceLoader
// vhco:label Resolve file imports
// vhco:about Follows every `import "PATH" as ALIAS [public]` from the bundle's roots (the entry file and host-loaded modules), reads each imported file once through the source loader confined to the runtime root, assigns each file its canonical namespace (ALIAS, transitively ALIAS.B), detects cycles and limits, and marks which modules are public (PROP-2026-0002 R19-R21, R24). Compiles nothing.
// vhco:example input={entry:"app.rivet", files:[app.rivet: import "./users.rivet" as users]} => { "modules": [{"alias": "", "file": "app.rivet"}, {"alias": "users", "file": "users.rivet", "public": false, "depth": 1}] }
pub fn resolve_imports(
    input: &SourceBundle,
    parser: &dyn Parser,
    loader: &dyn SourceLoader,
) -> RivetResult<SourceBundle> {
    let root = input.root.as_str();
    let mut files: Vec<SourceFile> = input.files.clone();
    let mut modules: Vec<ModuleRef> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut rels: Vec<String> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut errors: Vec<RivetError> = Vec::new();
    let mut queue: VecDeque<usize> = VecDeque::new();

    // vhco:todo read_roots -- the depth-0 ModuleRefs of the input (entry "" and host-loaded modules) are the roots; with none, the entry file alone; files already in SourceBundle.files are reused, only new ones are read through the SourceLoader
    // vhco:step roots SourceBundle.modules -- the entry file and host-loaded modules are the depth-0 roots
    let mut roots: Vec<ModuleRef> = input
        .modules
        .iter()
        .filter(|m| m.depth == 0)
        .cloned()
        .collect();
    if roots.is_empty() && !input.entry.is_empty() {
        roots.push(ModuleRef {
            alias: String::new(),
            file: input.entry.clone(),
            public: true,
            depth: 0,
            imports: Vec::new(),
            policy_ignored: false,
        });
    }
    if roots.is_empty() {
        return Ok(input.clone());
    }
    let loader_dir = dir_of(&rel_to_root(&input.entry, root));
    for r in roots {
        let rel = rel_to_root(&r.file, root);
        if let Some(&j) = index.get(&rel) {
            errors.push(RivetError::syntax(
                codes::IMPORT_DUPLICATE,
                format!(
                    "{rel} is already loaded as `{}`; a file compiles once",
                    modules[j].alias
                ),
                None,
            ));
            continue;
        }
        let full = if files.iter().any(|f| f.path == r.file) {
            r.file.clone()
        } else {
            // A host-loaded root not read yet.
            match loader.read_module(root, &rel) {
                Ok(f) => {
                    let path = f.path.clone();
                    files.push(f);
                    path
                }
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            }
        };
        let policy_ignored =
            !r.alias.is_empty() && dir_of(&rel) != loader_dir && loader.policy_beside(&full);
        index.insert(rel.clone(), modules.len());
        rels.push(rel);
        queue.push_back(modules.len());
        modules.push(ModuleRef {
            alias: r.alias,
            file: full,
            public: true,
            depth: 0,
            imports: Vec::new(),
            policy_ignored,
        });
    }

    while let Some(i) = queue.pop_front() {
        let Some(file) = files.iter().find(|f| f.path == modules[i].file).cloned() else {
            continue;
        };
        // vhco:todo parse_imports -- parse each file with the Parser and lower only its import lines; malformed or misplaced imports are syntax.import at the import line; a file with syntax errors contributes no imports (compile_program reports them)
        // vhco:step parse parser.parse -- one SyntaxTree per file; only import lines are lowered here
        let tree = match parser.parse(&file) {
            Ok(t) if t.diagnostics.is_empty() => t,
            _ => continue,
        };
        let mut lowerer = Lowerer::default();
        // vhco:step imports lowerer.imports -- shape and placement (syntax.import)
        let mut decls: Vec<ImportDecl> = lowerer.imports(&tree);
        errors.append(&mut lowerer.errors);
        let mut aliases: Vec<String> = Vec::new();
        for decl in &mut decls {
            // vhco:error duplicate -- one file uses the same alias twice => check.import_duplicate (exit 2) returns
            if aliases.contains(&decl.alias) {
                errors.push(RivetError::syntax(
                    codes::IMPORT_DUPLICATE,
                    format!("alias `{}` is imported twice in {}", decl.alias, rels[i]),
                    Some(decl.span.clone()),
                ));
                continue;
            }
            aliases.push(decl.alias.clone());
            // vhco:todo confine_paths -- resolve PATH against the importing file's directory and normalize it lexically; outside the root is permission.import_outside_root (exit 3); read through SourceLoader.read_module, which refuses symlinks (permission.import_outside_root) and missing files (not_found.import, exit 4)
            // vhco:step confine confine -- relative to the importing file, lexically inside the runtime root
            let Some(target) = confine(&rels[i], &decl.path) else {
                // vhco:error outside_root -- PATH escapes the runtime root or crosses a symlink => permission.import_outside_root (exit 3) returns
                errors.push(
                    RivetError::new(
                        ErrorKind::Permission,
                        codes::IMPORT_OUTSIDE_ROOT,
                        format!(
                            "import `{}` resolves outside the runtime root {root}",
                            decl.path
                        ),
                    )
                    .with_span(Some(decl.span.clone())),
                );
                continue;
            };
            // vhco:todo compile_once -- breadth-first: the shallowest import names a file (importer alias + "." + ALIAS); a file reached again keeps its first namespace and ImportDecl.target points at it; more than 256 files or depth over 16 is limit.imports (exit 5)
            if let Some(&j) = index.get(&target) {
                decl.target = modules[j].alias.clone();
                edges.push(Edge {
                    from: i,
                    to: j,
                    public: decl.public,
                    span: decl.span.clone(),
                });
                continue;
            }
            let depth = modules[i].depth + 1;
            // vhco:error limits -- more than 256 files or depth over 16 => limit.imports (exit 5) returns
            if modules.len() >= MAX_FILES || depth > MAX_DEPTH {
                errors.push(
                    RivetError::new(
                        ErrorKind::Limit,
                        codes::LIMIT_IMPORTS,
                        if depth > MAX_DEPTH {
                            format!("imports nest deeper than {MAX_DEPTH} levels at {target}")
                        } else {
                            format!("a bundle may hold at most {MAX_FILES} files")
                        },
                    )
                    .with_span(Some(decl.span.clone())),
                );
                continue;
            }
            let full = join_root(root, &target);
            if !files.iter().any(|f| f.path == full) {
                // vhco:step read loader.read_module -- no symlinks, missing file = not_found.import; each file read once
                match loader.read_module(root, &target) {
                    Ok(f) => files.push(f),
                    Err(e) => {
                        // vhco:error missing -- the imported file does not exist => not_found.import (exit 4) returns
                        errors.push(e.with_span(Some(decl.span.clone())));
                        continue;
                    }
                }
            }
            let alias = if modules[i].alias.is_empty() {
                decl.alias.clone()
            } else {
                format!("{}.{}", modules[i].alias, decl.alias)
            };
            decl.target = alias.clone();
            // vhco:step policy loader.policy_beside -- a module policy.json is ignored (warning)
            let policy_ignored = dir_of(&target) != loader_dir && loader.policy_beside(&full);
            let j = modules.len();
            index.insert(target.clone(), j);
            rels.push(target);
            edges.push(Edge {
                from: i,
                to: j,
                public: decl.public,
                span: decl.span.clone(),
            });
            modules.push(ModuleRef {
                alias,
                file: full,
                public: false,
                depth,
                imports: Vec::new(),
                policy_ignored,
            });
            queue.push_back(j);
        }
        modules[i].imports = decls;
    }

    // vhco:todo detect_cycles -- depth-first over the import edges; an import returning to a file on the current path is check.import_cycle at the first import of the cycle, with the path in the message
    // vhco:step cycles find_cycles -- check.import_cycle with the path
    find_cycles(&modules, &rels, &edges, &mut errors);

    // vhco:todo mark_public -- public iff some chain of imports from a root is `public` at every step (roots are public); policy.json beside a module outside the loader's directory sets policy_ignored for the check.module_policy_ignored warning
    // vhco:step public mark_public -- public iff a chain of public imports reaches the module
    mark_public(&mut modules, &edges);

    // vhco:error cycle -- imports form a cycle => check.import_cycle (exit 2) returns
    // vhco:error syntax -- an import is malformed or placed after a declaration => syntax.import (exit 2) returns
    if !errors.is_empty() {
        let mut first = errors.remove(0);
        first.suppressed = errors;
        return Err(first);
    }
    Ok(SourceBundle {
        entry: input.entry.clone(),
        root: input.root.clone(),
        files,
        modules,
    })
}

/// The import target relative to the root, or `None` outside it (an
/// absolute PATH is refused by lowering already).
fn confine(from_rel: &str, path: &str) -> Option<String> {
    resolve_rel(from_rel, path)
}

fn dir_of(rel: &str) -> String {
    rel.rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default()
}

/// Report every import cycle once, at the import that starts it.
fn find_cycles(
    modules: &[ModuleRef],
    rels: &[String],
    edges: &[Edge],
    errors: &mut Vec<RivetError>,
) {
    fn visit(
        i: usize,
        rels: &[String],
        edges: &[Edge],
        colour: &mut [u8],
        stack: &mut Vec<(usize, Option<usize>)>,
        errors: &mut Vec<RivetError>,
    ) {
        colour[i] = 1;
        for (k, e) in edges.iter().enumerate().filter(|(_, e)| e.from == i) {
            stack.push((i, Some(k)));
            if colour[e.to] == 1 {
                // The cycle starts where `e.to` sits on the stack.
                let start = stack.iter().position(|(n, _)| *n == e.to).unwrap_or(0);
                let mut path: Vec<&str> = stack[start..]
                    .iter()
                    .map(|(n, _)| rels[*n].as_str())
                    .collect();
                path.push(rels[e.to].as_str());
                let first_edge = stack[start].1.map(|k| &edges[k]).unwrap_or(e);
                errors.push(RivetError::syntax(
                    codes::IMPORT_CYCLE,
                    format!("import cycle: {}", path.join(" → ")),
                    Some(first_edge.span.clone()),
                ));
            } else if colour[e.to] == 0 {
                visit(e.to, rels, edges, colour, stack, errors);
            }
            stack.pop();
        }
        colour[i] = 2;
    }
    let mut colour = vec![0u8; modules.len()];
    for i in 0..modules.len() {
        if colour[i] == 0 {
            visit(i, rels, edges, &mut colour, &mut Vec::new(), errors);
        }
    }
}

/// Roots are public; a module is public when a `public` import from a public
/// module reaches it.
fn mark_public(modules: &mut [ModuleRef], edges: &[Edge]) {
    let mut changed = true;
    while changed {
        changed = false;
        for e in edges.iter().filter(|e| e.public) {
            if modules[e.from].public && !modules[e.to].public {
                modules[e.to].public = true;
                changed = true;
            }
        }
    }
}
