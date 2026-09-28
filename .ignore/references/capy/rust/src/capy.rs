//! Port of `capy.go` — the embedding API.
//!
//! Embeds Capy as a library so programs can define their own syntax inline
//! without invoking a separate binary or shipping library files.
//!
//! Quick start:
//!
//! ```no_run
//! use capy_core::capy::Library;
//!
//! let lib = Library::new(r#"
//! extension html
//! function button
//!     arg literal "button"
//!     arg capture label string
//!     write `<button>${label}</button>
//! `
//! end
//! "#).unwrap();
//!
//! let out = lib.run(r#"button "Click me""#).unwrap();
//! ```
//!
//! The library passed to [`Library::new`] is written in Capy's native (`.capy`)
//! syntax — the same grammar that drives user scripts. There is no separate
//! template / config language; the renderer walks the parsed AST directly.

use crate::domain::docs::render_library_docs as domain_render_docs;
use crate::domain::ast::{Block as DomainBlock, Span};
use crate::domain::errors::{codes, CapyError, Diagnostic};
use crate::domain::host::{Host, NoOpHost};
use crate::domain::library::Library as DomainLibrary;
use crate::infra::{define_extractor, preprocessor};
use crate::orchestrator::features::{
    make_evaluator, make_lexer, make_library_loader, make_parser,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// PLAN-2026-0002 R6 — what [`Library::parse`] returns.
///
/// Separate from `Result` on purpose: a recovering parse produces a tree AND
/// errors, and `Result` can carry only one of the two.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// Always present. May contain [`ErrorNode`](crate::domain::ast::ErrorNode)
    /// entries in `errors` when recovery kicked in.
    pub tree: DomainBlock,
    /// Empty ⇒ clean parse.
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseResult {
    /// True when nothing failed to parse.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty() && self.tree.errors.is_empty()
    }
}

/// A compiled, ready-to-run Capy library. Safe to reuse across many `run`
/// calls.
///
/// `Library` is `Send + Sync`: compile once, wrap in an `Arc`, and share it
/// across threads. `run` takes `&self` and builds a fresh accumulating context
/// per call, so concurrent transpiles need no lock. This is why the engine
/// holds `Arc` rather than `Rc` internally — see the `library_is_send_and_sync`
/// test, which pins the property at compile time.
///
/// ```
/// use capy_core::capy::Library;
/// use std::sync::Arc;
///
/// let lib = Arc::new(Library::new("extension txt\n")?);
/// let worker = { let lib = Arc::clone(&lib); std::thread::spawn(move || lib.run("")) };
/// worker.join().unwrap()?;
/// # Ok::<(), capy_core::domain::errors::CapyError>(())
/// ```
pub struct Library {
    lib: DomainLibrary,
    host: Arc<dyn Host + Send + Sync>,
}

impl std::fmt::Debug for Library {
    /// Hand-written because the held `Arc<dyn Host + Send + Sync>` can't derive `Debug`.
    /// Reports the library's identity, not its full compiled contents.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library")
            .field("extension", &self.lib.extension)
            .field("name", &self.lib.lib_name)
            .field("functions", &self.lib.functions.len())
            .field("types", &self.lib.types.len())
            .finish()
    }
}

impl Library {
    /// Port of `NewLibrary`.
    ///
    /// Compiles a library written in Capy's native (`.capy`) syntax from an
    /// in-memory string.
    pub fn new(library_src: &str) -> Result<Library, CapyError> {
        Library::from_bytes("capy", library_src.as_bytes())
    }

    /// Port of `NewLibraryFromFile`.
    pub fn from_file(path: &str) -> Result<Library, CapyError> {
        let dl = make_library_loader::load_library(path, make_lexer::tokenize)?;
        Ok(Library { lib: dl, host: Arc::new(NoOpHost) })
    }

    /// Port of `newFromBytes`.
    pub fn from_bytes(format: &str, src: &[u8]) -> Result<Library, CapyError> {
        let dl =
            make_library_loader::load_library_from_bytes(format, src, make_lexer::tokenize)?;
        Ok(Library { lib: dl, host: Arc::new(NoOpHost) })
    }

    /// Port of `SetHost`.
    ///
    /// Installs a [`Host`] that the library's `env` / `arg` / `read_file`
    /// inner-DSL primitives read from. The default is [`NoOpHost`] — every
    /// primitive returns the empty zero value and `read_file` errors out. Pass an
    /// `OsHost` to opt in to real env / args / filesystem (only when the library
    /// source is trusted).
    ///
    /// The `Send + Sync` bound is what keeps `Library` shareable across
    /// threads; a custom `Host` holding interior-mutable state must therefore
    /// use a thread-safe cell (`Mutex`/`RwLock`, not `RefCell`).
    pub fn set_host(&mut self, h: Option<Arc<dyn Host + Send + Sync>>) {
        self.host = h.unwrap_or_else(|| Arc::new(NoOpHost));
    }

    /// PLAN-2026-0002 R6 — parse a script into a tree plus diagnostics.
    ///
    /// The tree is ALWAYS returned. An empty `diagnostics` means a clean parse;
    /// otherwise `tree.errors` holds the regions that could not be parsed while
    /// the statements around them are intact.
    ///
    /// ```text
    /// let r = lib.parse(src);
    /// if !r.diagnostics.is_empty() { /* report */ }
    /// for st in &r.tree.stmts { /* the statements that DID parse */ }
    /// ```
    ///
    /// **`tree.stmts` alone does not mean the parse succeeded.** Check
    /// `diagnostics` (or `tree.errors`) before trusting the tree — a partial
    /// parse looks exactly like a complete one if you only read `stmts`.
    pub fn parse(&self, script_src: &str) -> ParseResult {
        let toks = match make_lexer::tokenize_with_trivia(script_src, &self.lib.comments) {
            Ok(t) => t,
            Err(e) => {
                return ParseResult {
                    tree: DomainBlock::default(),
                    diagnostics: vec![Diagnostic::error(
                        codes::NO_MATCH,
                        Span::new(e.line, e.col, e.line, e.col + 1),
                        e.msg,
                    )],
                }
            }
        };
        let (tree, diagnostics) =
            make_parser::parse_recovering(toks, script_src, &self.lib);
        ParseResult { tree, diagnostics }
    }

    /// Port of `Run`.
    ///
    /// Transpiles a single source script through this library. The library's
    /// `file_template` (if any) wraps the output.
    pub fn run(&self, script_src: &str) -> Result<String, CapyError> {
        let (out, _) = self.run_multi(script_src)?;
        Ok(out)
    }

    /// Port of `RunMulti`.
    ///
    /// Like [`Library::run`] but also returns the rendered multi-file map for
    /// libraries that declared `file "path"` blocks.
    ///
    /// Source-level metaprogramming is supported: any `define NAME … end` blocks
    /// at the top of the script are extracted and merged into the library before
    /// evaluation, exactly as the CLI does.
    pub fn run_multi(
        &self,
        script_src: &str,
    ) -> Result<(String, BTreeMap<String, String>), CapyError> {
        // Honour any inclusion directives the library declared via its
        // `preprocess` block. The wasm/embedded sandbox has no filesystem, so
        // most directives will fail to read — but the engine still defers
        // entirely to the library on what shapes count.
        let expanded = preprocessor::preprocess(script_src, ".", &self.lib.preprocess, self.host.as_ref())
            .map_err(CapyError::msg)?;
        // Extract `define … end` blocks from the script and merge them into a
        // copy of the library.
        let (cleaned, define_lib_src) =
            define_extractor::extract_defines(&expanded).map_err(CapyError::msg)?;
        let mut lib_to_use = self.lib.clone();
        if !define_lib_src.is_empty() {
            let define_lib = make_library_loader::load_library_from_bytes(
                "capy",
                define_lib_src.as_bytes(),
                make_lexer::tokenize,
            )?;
            // Source defines WIN on conflict (matches CLI behavior).
            for (k, v) in define_lib.functions {
                lib_to_use.functions.insert(k, v);
            }
        }
        // R27 — the user-script path retains comments; the parser strips them.
        let toks = make_lexer::tokenize_with_trivia(&cleaned, &lib_to_use.comments)?;
        let prog = make_parser::parse(toks, &cleaned, &lib_to_use)?;
        make_evaluator::run_multi(&prog, &lib_to_use, self.host.clone())
    }

    /// Port of `Extension` — the library's declared `extension` field.
    pub fn extension(&self) -> &str {
        &self.lib.extension
    }

    /// Port of `OutputFile` — the library's optional `output_file` field.
    pub fn output_file(&self) -> &str {
        &self.lib.output_file
    }

    /// Port of `FunctionNames`.
    ///
    /// DIVERGENCE (bug fix): Go's doc comment promises "the sorted list" but the
    /// function has no `sort` call, so it returns Go's randomized map order — I
    /// measured 4 distinct orderings across 10 runs on a 4-function library.
    /// `BTreeMap` here delivers what the contract advertises.
    pub fn function_names(&self) -> Vec<String> {
        self.lib.functions.keys().cloned().collect()
    }

    /// Port of `CommentMarkers`.
    ///
    /// The library's declared line-comment markers (from its `comments` block).
    /// Empty when the library declares none — in which case user scripts have no
    /// comment syntax.
    pub fn comment_markers(&self) -> Vec<String> {
        self.lib.comments.clone()
    }

    /// Port of `Introspect`.
    ///
    /// Returns the declared functions of the library — name, description,
    /// argument shapes, block kind, and priority. The data comes straight from
    /// the compiled library, so an editor can derive its autocomplete / hover /
    /// highlight metadata instead of hand-maintaining a parallel catalogue.
    /// Results are sorted by function name for stable output.
    pub fn introspect(&self) -> Vec<FunctionInfo> {
        let mut out: Vec<FunctionInfo> = Vec::with_capacity(self.lib.functions.len());
        // `BTreeMap` iterates sorted, matching Go's explicit sort.Strings.
        for fnv in self.lib.functions.values() {
            let mut fi = FunctionInfo {
                name: fnv.name.clone(),
                description: fnv.description.clone(),
                args: Vec::new(),
                block: String::new(),
                priority: fnv.priority,
            };
            for a in &fnv.args {
                fi.args.push(ArgInfo {
                    kind: a.kind.clone(),
                    value: a.value.clone(),
                    name: a.name.clone(),
                    type_: a.type_.clone(),
                    description: a.description.clone(),
                    optional: a.optional,
                    default: a.default.clone(),
                });
            }
            if let Some(b) = &fnv.block {
                fi.block = if b.is_verbatim {
                    format!("verbatim:{}", b.closer)
                } else if !b.sections.is_empty() {
                    format!("sections:{} closer:{}", b.sections.join(","), b.closer)
                } else if b.is_dedent {
                    "dedent".to_string()
                } else if !b.open.is_empty() {
                    format!("open:{} close:{}", b.open, b.close)
                } else if !b.closer.is_empty() {
                    format!("closer:{}", b.closer)
                } else {
                    String::new()
                };
            }
            out.push(fi);
        }
        out
    }

    /// The compiled library, for callers that need the domain model directly.
    pub fn domain(&self) -> &DomainLibrary {
        &self.lib
    }
}

/// Port of `RenderLibraryDocs`.
///
/// Markdown reference documentation for the given library — the same format
/// `capy docs <lib>` writes on the CLI.
pub fn render_library_docs(lib: &Library) -> String {
    domain_render_docs(&lib.lib)
}

/// Port of `ArgInfo` — one argument in a function's match shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArgInfo {
    /// `"literal"` or `"capture"`.
    pub kind: String,
    /// The literal token text (`kind == "literal"`).
    pub value: String,
    /// The capture's bound name (`kind == "capture"`).
    pub name: String,
    /// The capture's declared type (`kind == "capture"`).
    pub type_: String,
    /// The optional trailing doc string on the arg line.
    pub description: String,
    /// True for a trailing capture declared with `default`, i.e. one the call
    /// site may omit.
    pub optional: bool,
    /// The value bound when an optional capture is omitted.
    pub default: String,
}

/// Port of `FunctionInfo` — the introspected shape of one library function.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FunctionInfo {
    pub name: String,
    pub description: String,
    pub args: Vec<ArgInfo>,
    /// `""` for a non-block function, otherwise one of: `closer:NAME`,
    /// `open:X close:Y`, `dedent`, `verbatim:NAME`, `sections:… closer:NAME`.
    pub block: String,
    pub priority: i64,
}
