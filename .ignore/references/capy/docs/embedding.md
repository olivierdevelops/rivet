---
title: Embed Capy in Rust
---

# Embed Capy in a Rust program

Capy is a Rust library. You don't have to ship the `capy` binary or maintain
separate `lib.capy` files — your program can carry its own grammar inline
and transpile user input at runtime.

```rust
use capy_core::capy::Library;

let lib = Library::new(r#"
extension html

function button
    arg literal "button"
    arg capture label string
    write `<button>${label}</button>
`
end
"#)?;

let out = lib.run(r#"button "Click me""#)?;
// → <button>"Click me"</button>
```

That's the entire API surface for the common case.

## When to embed Capy

- **Your CLI takes a config file** in a friendlier-than-YAML DSL. Write
  the parser in 50 lines of Capy instead of 500 lines of `serde` plumbing
  and string interpolation.
- **Your tool generates code** (think Prisma's `model User { ... }` →
  SQL migrations). Embedding Capy lets users write that DSL natively
  while your Rust code consumes the generated output.
- **You want hot-swappable grammars** — read a library file at startup,
  let users contribute new ones without recompiling.
- **You want a sandboxed scripting surface** for an AI agent — let it
  emit Capy DSL, your binary transpiles to whatever target you trust.

## Install

Not on crates.io yet, so depend on it from git — cargo finds the crate inside the
repo's `rust/` subdirectory on its own:

```toml
[dependencies]
capy-core = { git = "https://github.com/olivierdevelops/capy" }
```

One dependency (`regex`), no C symbols, no build script. Minimum supported Rust
version is 1.74.

## The full API

`capy_core::capy` exposes a tiny, intentionally-stable surface:

```rust
// Compile a library from an in-memory string, a file, or raw bytes.
impl Library {
    pub fn new(library_src: &str) -> Result<Library, CapyError>;
    pub fn from_file(path: &str) -> Result<Library, CapyError>;
    pub fn from_bytes(format: &str, src: &[u8]) -> Result<Library, CapyError>;

    // Run a source script through the library.
    pub fn run(&self, script_src: &str) -> Result<String, CapyError>;
    // …and the multi-file form, for libraries declaring `file "path"` blocks.
    pub fn run_multi(&self, script_src: &str)
        -> Result<(String, BTreeMap<String, String>), CapyError>;

    // Diagnostic helpers.
    pub fn extension(&self) -> &str;            // declared `extension:` field
    pub fn output_file(&self) -> &str;          // declared `output_file:` field
    pub fn function_names(&self) -> Vec<String>;// declared function keys, sorted

    // Introspection — the library describes itself (see below).
    pub fn introspect(&self) -> Vec<FunctionInfo>;
    pub fn comment_markers(&self) -> Vec<String>;

    // Opt in to real env / args / filesystem access.
    pub fn set_host(&mut self, h: Option<Arc<dyn Host + Send + Sync>>);
}

// Markdown reference docs — the same text `capy docs <lib>` prints.
pub fn render_library_docs(lib: &Library) -> String;
```

That's the whole module. Everything else is convention.

## Reuse and threads

`Library` is `Send + Sync`. Compile the grammar once and share it: `run` takes
`&self` and builds a fresh accumulating context per call, so concurrent
transpiles need no lock.

```rust
use std::sync::{Arc, OnceLock};

static LIB: OnceLock<Arc<Library>> = OnceLock::new();

fn grammar() -> &'static Arc<Library> {
    LIB.get_or_init(|| Arc::new(Library::new(MY_GRAMMAR).expect("valid grammar")))
}

// From an axum handler, a rayon map, a spawned thread — all fine:
let lib = Arc::clone(grammar());
std::thread::spawn(move || lib.run(script));
```

A custom `Host` must therefore be `Send + Sync` too: use `Mutex`/`RwLock` for
interior mutability, not `RefCell`.

## Sandboxing

A fresh `Library` runs on `NoOpHost`: the `env`, `arg` and `read_file` inner-DSL
primitives return empty values and `read_file` errors. That is the safe default
for untrusted library sources. Opt in explicitly:

```rust
use capy_core::infra::os_host::OsHost;
use std::sync::Arc;

let mut lib = Library::new(src)?;
lib.set_host(Some(Arc::new(OsHost {
    user_args: vec![],
    base_dir: "/path/to/scripts".into(),
})));
```

See [Host capabilities](host-capabilities.md) for the full surface.

## Introspection — the library describes itself

`introspect()` returns the full declared shape of every function —
name, doc string, argument list (literal vs. capture, capture type,
per-arg description, and whether the arg is **optional** with a
**default**), block kind, and priority. The data comes straight from
the compiled library, so a tool can derive its metadata instead of
hand-maintaining a parallel catalogue that silently drifts.

This is what powers a live editor's autocomplete, hover-docs, syntax
highlighting, and reference panel — all from one source of truth (the
`.capy` library itself).

```rust
pub struct FunctionInfo {
    pub name: String,
    pub description: String,
    pub args: Vec<ArgInfo>,
    pub block: String,   // e.g. "verbatim:end", "dedent", "closer:end"
    pub priority: i64,
}

pub struct ArgInfo {
    pub kind: String,        // "literal" or "capture"
    pub value: String,       // literal token text
    pub name: String,        // capture's bound name
    pub type_: String,       // capture's declared type (`type` is a Rust keyword)
    pub description: String, // trailing doc string
    pub optional: bool,      // trailing arg with a default
    pub default: String,     // value bound when omitted
}
```

Example — introspecting a one-function library:

```rust
let lib = Library::new(r##"
extension html

comments
    line "#"
end

function button
    description "A clickable button."
    arg literal "button"
    arg capture label   string "Visible text."
    arg capture variant string default "primary"   "Style variant."
    write `<button class="btn-${variant}">${label}</button>`
end
"##)?;

for fn_ in lib.introspect() {
    println!("{} - {}", fn_.name, fn_.description);
    for a in &fn_.args {
        if a.kind == "capture" {
            let opt = if a.optional {
                format!(" (optional, default {:?})", a.default)
            } else {
                String::new()
            };
            println!("  {}: {}{} — {}", a.name, a.type_, opt, a.description);
        }
    }
}
println!("comment markers: {:?}", lib.comment_markers());
```

Output:

```
button - A clickable button.
  label: string — Visible text.
  variant: string (optional, default "primary") — Style variant.
comment markers: ["#"]
```

The same data is available from the browser via the wasm build —
`capyIntrospect(librarySrc)` returns the identical JSON shape. An
editor can `JSON.parse` it and build autocomplete with zero
hand-maintenance.

## A real example

The repo ships [`examples/embed-html-dsl/`](https://github.com/olivierdevelops/capy/tree/main/examples/embed-html-dsl)
— a small Rust program that defines its own HTML DSL inline and
transpiles a hardcoded source. Run it:

```sh
cargo run --manifest-path examples/embed-html-dsl/Cargo.toml
```

Output (real, no escaping omitted):

```html
<!DOCTYPE html>
<html>
  <head><title>"Hello from embedded Capy"</title></head>
  <body>
    <h1>"Welcome!"</h1>
    <p>"This entire page was generated by a Capy library compiled INTO this Rust binary."</p>
    <p>"No external lib.capy, no separate capy CLI."</p>
    <a href="https://github.com/olivierdevelops/capy">"Source"</a>
  </body>
</html>
```

Everything between `<!DOCTYPE html>` and `</html>` came from a Capy
library compiled into the Rust binary. No filesystem, no subprocess.

## Patterns

### Pattern 1 — your config is your CLI's input

```rust
const CONFIG_LIB: &str = r#"
extension json

function server
    arg literal "server"
    arg capture name string
    block_closer end
    write `{ "name": ${name}, "routes": [
${indent 4 body}
]}
`
end

function route
    arg literal "route"
    arg capture method ident
    arg capture path string
    write `  { "method": "${method}", "path": ${path} },
`
end

function end
end
"#;

fn load_config(user_input: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let lib = Library::new(CONFIG_LIB)?;
    let json_str = lib.run(user_input)?;
    Ok(serde_json::from_str(&json_str)?)
}
```

Now your users write:

```
server "api"
    route GET  "/healthz"
    route POST "/v1/users"
end
```

…and your Rust binary parses real JSON without you writing any DSL parser
code.

### Pattern 2 — let users extend your tool

Ship a default library compiled into the binary, but allow a user-supplied
override:

```rust
fn load_lib(path: Option<&str>) -> Result<Library, CapyError> {
    match path {
        Some(p) => Library::from_file(p),   // user-provided
        None => Library::new(BUILTIN_LIB),  // baked-in default
    }
}
```

The same `Library` works in both cases.

### Pattern 3 — multiple grammars in one process

Each `Library` is independent. Run the same source through different
libraries to compare outputs, or pick a grammar at runtime based on
some flag:

```rust
let html_lib = Library::new(HTML_GRAMMAR)?;
let md_lib = Library::new(MARKDOWN_GRAMMAR)?;

match output_format {
    "html" => html_lib.run(src),
    "md" => md_lib.run(src),
    other => Err(CapyError::msg(format!("unknown format {other:?}"))),
}
```

## Performance notes

- `Library::new` compiles the grammar once. Reuse the returned `Library`
  — don't recompile per request.
- `run` is allocation-heavy (template rendering, AST walking). For
  hot paths consider caching outputs keyed by source hash.
- No threads are spawned. `run` is synchronous and CPU-bound.

## Caveats and edge cases

- **String captures preserve their quotes.** When your library declares
  `arg capture text string`, the captured value renders as `"foo"`
  (with quotes) in templates — Capy is a *transpiler*, so input syntax
  is preserved into the output. Use `arg capture text any` if you want
  the bare value, or strip the quotes inline with the `unquote` helper:
  `${text | unquote}`.
- **Errors include line numbers** from the source — wire them through
  to your user-facing error path. `domain::errors::format_with_source`
  renders an error with a source caret, the way the CLI does.
- **Raw-string fences need care.** A `.capy` library routinely contains
  `"#` (a `line "#"` comment marker, a shell comment in a template),
  which closes an `r#"…"#` literal early. Widen the fence to `r##"…"##`
  when that happens — several examples above do.
- **There is no eval of user code.** A Capy library defines patterns
  and templates; it cannot execute arbitrary Rust from the source
  script. Embedding Capy in a server is safe from that angle.

## Source positions

Every node the parser produces carries a `Span`:

```rust
pub struct Span {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,   // EXCLUSIVE — one past the last byte
}
```

`FuncCall.span` covers the whole statement, including a block body and its
closer. `CaptureValue.span` covers exactly the tokens that produced that value,
so a diagnostic can point at the argument a rule is about rather than the
statement it sits in. `Span::is_unset()` is true only where nothing was consumed
— an optional capture that bound its default, for example — and `Span::join`
ignores an unset operand rather than dragging a range back to line 0.

The existing `line` / `col` fields are unchanged and still drive the `${line}` /
`${col}` render locals; they are the same position as `span.start`.

> **Byte offsets are not present yet.** `Span` is `#[non_exhaustive]`, so they can
> be added without breaking your code — but construct spans through `Span::new`
> rather than a struct literal.

### Comments

A script's comments are retained and the ones immediately preceding a statement
are attached to it:

```rust
for c in &stmt.leading_comments {
    println!("comment at {}:{}", c.start_line, c.start_col);
}
```

**A node's own `span` excludes its attached comments.** A formatter needs the
node's range without them and the comments' ranges separately, and folding the
two together loses one irrecoverably. Trailing and interior comments are retained
as trivia but are not attached to anything.

Comments never reach the matcher, so retaining them changes nothing about what
parses or what is rendered.

### Getting the tree

```rust
let result = lib.parse(source);

// ⚠ `tree.stmts` alone does NOT mean the parse succeeded. A partial parse looks
//   exactly like a complete one if you only read `stmts` — check diagnostics.
if !result.diagnostics.is_empty() {
    for d in &result.diagnostics { eprintln!("{}: {}", d.code, d.full_message()); }
}

for stmt in &result.tree.stmts {
    // the statements that DID parse
}
for region in &result.tree.errors {
    // the regions that did not, each pointing at its diagnostic by index
}
```

`ParseResult { tree, diagnostics }` — the tree is always returned, because
parsing recovers. `result.is_clean()` is the short form of "no diagnostics and
no error regions".

`Library::run` is unchanged: it still returns `Result<String, CapyError>` with
the first error and no output, so nothing that embeds Capy today has to move.

## What it's not

- Not a Rust-side imperative API for building libraries. Libraries are
  always declarative `.capy` text. If you need to construct one
  programmatically, write a function that builds the string.
- Not a hot-reload watcher — that's your responsibility. `Library::new`
  is cheap to call; just re-call it when the source changes.
- Not mutable from several threads at once. `set_host` takes `&mut self`,
  so install the host before sharing; after that, `run(&self)` is free to
  be called concurrently.
