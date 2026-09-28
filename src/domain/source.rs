//! Source bundles and source positions.

use super::value::Value;

// vhco:domain SourceSpan { file: string; start_line: int; start_col: int; end_line: int; end_col: int }
/// 1-indexed, source-absolute; `end_col` is exclusive (Capy AST JSON convention).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl SourceSpan {
    pub fn new(
        file: &str,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> SourceSpan {
        SourceSpan {
            file: file.to_string(),
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Inverse of [`SourceSpan::to_value`] (remote error envelopes).
    pub fn from_value(v: &Value) -> Option<SourceSpan> {
        let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0).max(0) as u32;
        Some(SourceSpan {
            file: v.get("file")?.as_str()?.to_string(),
            start_line: n("line"),
            start_col: n("column"),
            end_line: n("end_line"),
            end_col: n("end_column"),
        })
    }

    pub fn to_value(&self) -> Value {
        Value::object([
            ("file", Value::text(&self.file)),
            ("line", Value::Int(self.start_line as i64)),
            ("column", Value::Int(self.start_col as i64)),
            ("end_line", Value::Int(self.end_line as i64)),
            ("end_column", Value::Int(self.end_col as i64)),
        ])
    }

    /// Short `file:line` form used in manifests.
    pub fn short(&self) -> String {
        format!("{}:{}", self.file, self.start_line)
    }

    /// Slice the exact source text covered by this span.
    pub fn slice<'a>(&self, text: &'a str) -> &'a str {
        let start = offset_of(text, self.start_line, self.start_col);
        let end = offset_of(text, self.end_line, self.end_col);
        match (start, end) {
            (Some(s), Some(e)) if e >= s => &text[s..e],
            _ => "",
        }
    }
}

/// Byte offset of a 1-indexed (line, column) position; column counts bytes.
pub fn offset_of(text: &str, line: u32, col: u32) -> Option<usize> {
    let mut offset = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i as u32 + 1 == line {
            let c = (col.saturating_sub(1)) as usize;
            return if c <= l.len() {
                Some(offset + c)
            } else {
                Some(offset + l.len())
            };
        }
        offset += l.len();
    }
    if line as usize == text.split_inclusive('\n').count() + 1 && col <= 1 {
        return Some(text.len());
    }
    None
}

/// A source path relative to the bundle root (`app.rivet`, `lib/users.rivet`):
/// `./` and a leading `root/` are dropped.
pub fn rel_to_root(file: &str, root: &str) -> String {
    let f = file.trim_start_matches("./");
    let r = root.trim_start_matches("./").trim_end_matches('/');
    if !r.is_empty()
        && r != "."
        && let Some(rest) = f.strip_prefix(&format!("{r}/"))
    {
        return rest.to_string();
    }
    f.to_string()
}

/// `root/rel` as the loader reads it (`rel` alone when the root is `.`).
pub fn join_root(root: &str, rel: &str) -> String {
    if root.is_empty() || root == "." {
        rel.to_string()
    } else {
        format!("{}/{rel}", root.trim_end_matches('/'))
    }
}

/// Resolve an import `path` against the directory of the importing file
/// `from_rel` (both relative to the root) and normalize it lexically. `None`
/// when a `..` would leave the root.
pub fn resolve_rel(from_rel: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = from_rel.split('/').collect();
    parts.pop(); // the importing file itself
    parts.retain(|p| !p.is_empty() && *p != ".");
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

// vhco:domain SourceFile { path: string; text: string }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

// vhco:domain ImportDecl { path: string; alias: string; public: bool; span: SourceSpan; target: string }
/// `import "PATH" as ALIAS [public]` (PROP-2026-0002 R19). `span` is the import
/// line in the importing file; `target` is the canonical module alias the path
/// resolved to (filled by `language.resolve_imports`, empty before).
///
/// ```text
///  app.rivet:1  import "./users.rivet" as users public
///               └─ path ─────────┘    └alias┘ └public┘   target = "users"
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ImportDecl {
    pub path: String,
    pub alias: String,
    pub public: bool,
    pub span: SourceSpan,
    pub target: String,
}

// vhco:domain ModuleRef { alias: string; file: string; public: bool; depth: int; imports: ImportDecl[]; policy_ignored: bool }
/// One resolved file of a bundle. `alias` is the canonical namespace of its
/// operation IDs (`""` for the entry bundle, `users`, `users.b` for nested
/// imports); `depth` is 0 for a root (the entry or a host-loaded module);
/// `public` means every import on some chain from a root is `public`, so its
/// operations are listed on the surfaces. `policy_ignored` records a
/// `policy.json` beside the module that the loader's policy overrides.
///
/// ```text
///  "" app.rivet (depth 0) ──▶ users (depth 1, internal) ──▶ users.b (depth 2)
///                         └─▶ billing (depth 1, public)
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ModuleRef {
    pub alias: String,
    pub file: String,
    pub public: bool,
    pub depth: u32,
    pub imports: Vec<ImportDecl>,
    pub policy_ignored: bool,
}

// vhco:domain SourceBundle { entry: string; root: string; files: SourceFile[]; modules: ModuleRef[] }
/// The in-memory source handed to the compiler. Loading the bytes is host
/// bootstrap I/O; compilation itself never reads files. `modules` is empty
/// for a single-file bundle (every file compiles into the entry namespace);
/// `language.resolve_imports` fills it with one `ModuleRef` per file.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SourceBundle {
    pub entry: String,
    pub root: String,
    pub files: Vec<SourceFile>,
    pub modules: Vec<ModuleRef>,
}

impl SourceBundle {
    pub fn single(path: &str, text: &str) -> SourceBundle {
        SourceBundle {
            entry: path.to_string(),
            root: ".".to_string(),
            files: vec![SourceFile {
                path: path.to_string(),
                text: text.to_string(),
            }],
            modules: Vec::new(),
        }
    }

    /// The resolved module a file belongs to (none for a single-file bundle).
    pub fn module_of(&self, file: &str) -> Option<&ModuleRef> {
        self.modules.iter().find(|m| m.file == file)
    }

    pub fn text_of(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_paths_stay_inside_the_root() {
        assert_eq!(
            resolve_rel("app.rivet", "./users.rivet").as_deref(),
            Some("users.rivet")
        );
        assert_eq!(
            resolve_rel("lib/a.rivet", "../b/c.rivet").as_deref(),
            Some("b/c.rivet")
        );
        assert_eq!(resolve_rel("app.rivet", "../x.rivet"), None);
        assert_eq!(resolve_rel("lib/a.rivet", "../../x.rivet"), None);
        assert_eq!(rel_to_root("./demo/app.rivet", "demo"), "app.rivet");
        assert_eq!(join_root(".", "a.rivet"), "a.rivet");
        assert_eq!(join_root("/tmp/x/", "a.rivet"), "/tmp/x/a.rivet");
    }

    #[test]
    fn slices_single_and_multi_line_spans() {
        let text = "abc\n  def ghi\nxyz";
        assert_eq!(SourceSpan::new("f", 2, 3, 2, 6).slice(text), "def");
        assert_eq!(SourceSpan::new("f", 1, 2, 2, 6).slice(text), "bc\n  def");
    }
}
