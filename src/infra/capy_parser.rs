//! Capy parser adapter — the only file that imports Capy.
//!
//! Uses Capy's public `Library::parse` and its versioned AST JSON
//! (`schema_version` 1) and converts the result into Rivet's `SyntaxTree`
//! (RES-2026-0001 findings 1, 2, 5, 8).

use crate::domain::ports::Parser;
use crate::domain::source::{SourceFile, SourceSpan};
use crate::domain::syntax_tree::{Capture, SyntaxDiagnostic, SyntaxNode, SyntaxTree};
use crate::domain::{RivetError, RivetResult};
use capy_core::capy::Library;
use capy_core::domain::ast_json;
use serde_json::Value as Json;
use std::sync::OnceLock;

/// The Rivet grammar, compiled into the binary.
pub const GRAMMAR: &str = include_str!("rivet.capy");
const AST_SCHEMA_VERSION: i64 = 1;

// vhco:infra capy_parser satisfies Parser
pub struct CapyParser {
    library: &'static Library,
}

fn library() -> RivetResult<&'static Library> {
    static LIB: OnceLock<Result<Library, String>> = OnceLock::new();
    match LIB.get_or_init(|| Library::new(GRAMMAR).map_err(|e| format!("{e:?}"))) {
        Ok(lib) => Ok(lib),
        Err(e) => Err(RivetError::internal(format!(
            "embedded rivet.capy failed to load: {e}"
        ))),
    }
}

impl CapyParser {
    pub fn new() -> RivetResult<CapyParser> {
        Ok(CapyParser {
            library: library()?,
        })
    }

    /// Every statement keyword the grammar knows (for did-you-mean hints).
    pub fn keywords(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .library
            .function_names()
            .into_iter()
            .map(|n| {
                n.trim_start_matches("opt_")
                    .trim_end_matches("_block")
                    .to_string()
            })
            .filter(|n| {
                !matches!(
                    n.as_str(),
                    "end"
                        | "assign"
                        | "assign_map"
                        | "assign_poll"
                        | "append_assign"
                        | "member_call"
                        | "call_stmt"
                        | "node_after"
                        | "auth_use"
                        | "auth_profile"
                        | "declared_error"
                )
            })
            .collect();
        out.push("auth".into());
        out.push("error".into());
        out.sort();
        out.dedup();
        out
    }
}

impl Parser for CapyParser {
    fn parse(&self, file: &SourceFile) -> RivetResult<SyntaxTree> {
        let result = self.library.parse(&file.text);
        let json_text = ast_json::to_json(&result);
        let json: Json = serde_json::from_str(&json_text)
            .map_err(|e| RivetError::internal(format!("Capy AST JSON did not parse: {e}")))?;
        let version = json
            .get("schema_version")
            .and_then(Json::as_i64)
            .unwrap_or(-1);
        if version != AST_SCHEMA_VERSION {
            return Err(RivetError::internal(format!(
                "unsupported Capy AST schema_version {version}; this build expects {AST_SCHEMA_VERSION}"
            )));
        }
        let mut tree = SyntaxTree {
            file: file.path.clone(),
            ..SyntaxTree::default()
        };
        if let Some(stmts) = json.pointer("/tree/stmts").and_then(Json::as_array) {
            tree.nodes = stmts.iter().map(|n| convert_node(n, file)).collect();
        }
        if let Some(diags) = json.get("diagnostics").and_then(Json::as_array) {
            for d in diags {
                tree.diagnostics
                    .push(convert_diagnostic(d, file, &self.keywords()));
            }
        }
        if let Some(errors) = json.pointer("/tree/errors").and_then(Json::as_array)
            && tree.diagnostics.is_empty()
            && !errors.is_empty()
        {
            for e in errors {
                tree.diagnostics.push(SyntaxDiagnostic {
                    code: "syntax.unparsed".into(),
                    message: "this region did not parse".into(),
                    span: span_of(e.get("span"), &file.path).unwrap_or_default(),
                    help: None,
                });
            }
        }
        tree.diagnostics.extend(indentation_diagnostics(file));
        Ok(tree)
    }
}

fn span_of(v: Option<&Json>, file: &str) -> Option<SourceSpan> {
    let v = v?;
    if v.is_null() {
        return None;
    }
    let n = |k: &str| v.get(k).and_then(Json::as_u64).unwrap_or(0) as u32;
    Some(SourceSpan::new(
        file,
        n("start_line"),
        n("start_col"),
        n("end_line"),
        n("end_col"),
    ))
}

fn convert_node(n: &Json, file: &SourceFile) -> SyntaxNode {
    let span = span_of(n.get("span"), &file.path).unwrap_or_default();
    let mut captures = Vec::new();
    if let Some(map) = n.get("captures").and_then(Json::as_object) {
        for (name, c) in map {
            let cspan = span_of(c.get("span"), &file.path);
            let text = c
                .get("text")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            let source_text = match &cspan {
                Some(s) => {
                    let sliced = s.slice(&file.text);
                    if sliced.is_empty() {
                        text.clone()
                    } else {
                        sliced.to_string()
                    }
                }
                None => text.clone(),
            };
            let sub = c
                .get("sub")
                .and_then(Json::as_array)
                .map(|a| a.iter().map(|s| convert_node(s, file)).collect())
                .unwrap_or_default();
            captures.push((
                name.clone(),
                Capture {
                    text,
                    source_text,
                    is_expr: c.get("is_expr").and_then(Json::as_bool).unwrap_or(false),
                    span: cspan,
                    sub,
                },
            ));
        }
    }
    let body = n
        .get("body")
        .filter(|b| !b.is_null())
        .and_then(|b| b.get("stmts"))
        .and_then(Json::as_array)
        .map(|a| a.iter().map(|s| convert_node(s, file)).collect());
    let closer = n
        .get("closer")
        .filter(|c| !c.is_null())
        .map(|c| Box::new(convert_node(c, file)));
    SyntaxNode {
        func: n
            .get("func")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string(),
        span,
        captures,
        body,
        closer,
    }
}

/// Rewrite Capy's furthest-attempt message for an unknown statement keyword
/// into "unknown statement `X`" with a did-you-mean (RES-2026-0001 finding 8).
fn convert_diagnostic(d: &Json, file: &SourceFile, keywords: &[String]) -> SyntaxDiagnostic {
    let span = span_of(d.get("primary"), &file.path).unwrap_or_default();
    let message = d
        .get("message")
        .and_then(Json::as_str)
        .unwrap_or("syntax error")
        .to_string();
    let capy_code = d.get("code").and_then(Json::as_str).unwrap_or("E0001");
    let line = file
        .text
        .lines()
        .nth(span.start_line.saturating_sub(1) as usize)
        .unwrap_or("");
    let first_col = line.len() - line.trim_start().len() + 1;
    let first_word: String = line
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if capy_code == "E0001"
        && span.start_col as usize == first_col
        && !first_word.is_empty()
        && !keywords.contains(&first_word)
    {
        let suggestion = closest(&first_word, keywords);
        return SyntaxDiagnostic {
            code: "syntax.unknown_statement".into(),
            message: format!("unknown statement `{first_word}`"),
            span,
            help: suggestion
                .map(|s| format!("did you mean `{s}`?"))
                .or_else(|| Some(format!("capy {capy_code}: {message}"))),
        };
    }
    let help = d.get("help").and_then(Json::as_str).map(str::to_string);
    SyntaxDiagnostic {
        code: format!("syntax.{}", capy_code.to_lowercase()),
        message,
        span,
        help,
    }
}

fn closest(word: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|c| (levenshtein(word, c), c))
        .filter(|(d, c)| *d <= 2.max(c.len() / 3))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j] + usize::from(ca != *cb))
                    .min(prev[j + 1] + 1)
                    .min(cur[j] + 1),
            );
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Capy's lexer accepts 2-space indentation; Rivet requires 4 spaces or tabs
/// per level (RES-2026-0001: syntax.indent). Continuation lines inside open
/// brackets or backtick strings are exempt.
fn indentation_diagnostics(file: &SourceFile) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut in_backtick = false;
    for (i, line) in file.text.lines().enumerate() {
        let continuation = depth > 0 || in_backtick;
        let trimmed = line.trim_start();
        if !continuation && !trimmed.is_empty() && !trimmed.starts_with('#') {
            let lead = &line[..line.len() - trimmed.len()];
            if !lead.contains('\t') && lead.len() % 4 != 0 {
                out.push(SyntaxDiagnostic {
                    code: "syntax.indent".into(),
                    message: format!(
                        "indentation of {} spaces; use 4 spaces (or one tab) per level",
                        lead.len()
                    ),
                    span: SourceSpan::new(
                        &file.path,
                        i as u32 + 1,
                        1,
                        i as u32 + 1,
                        lead.len() as u32 + 1,
                    ),
                    help: None,
                });
            }
        }
        let mut in_str: Option<char> = None;
        let mut escape = false;
        for ch in line.chars() {
            if escape {
                escape = false;
                continue;
            }
            match (in_str, in_backtick, ch) {
                (Some(_), _, '\\') => escape = true,
                (Some(q), _, c) if c == q => in_str = None,
                (Some(_), _, _) => {}
                (None, true, '`') => in_backtick = false,
                (None, true, _) => {}
                (None, false, '"' | '\'') => in_str = Some(ch),
                (None, false, '`') => in_backtick = true,
                (None, false, '#') => break,
                (None, false, '(' | '[' | '{') => depth += 1,
                (None, false, ')' | ']' | '}') => depth -= 1,
                _ => {}
            }
        }
        if depth < 0 {
            depth = 0;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> SyntaxTree {
        CapyParser::new()
            .unwrap()
            .parse(&SourceFile {
                path: "t.rivet".into(),
                text: text.into(),
            })
            .unwrap()
    }

    // vhco:test language.compile_program -- the embedded grammar parses a catalog and slices exact source text
    #[test]
    fn parses_catalog_and_slices_source_text() {
        let tree = parse(
            "operation demo.add\n    param a integer required\n    output integer\n    return a + b\nend\n",
        );
        assert!(tree.is_clean(), "{:?}", tree.diagnostics);
        let op = &tree.nodes[0];
        assert_eq!(op.func, "operation");
        assert_eq!(op.text("id"), "demo.add");
        let ret = op.children().iter().find(|n| n.func == "return").unwrap();
        assert_eq!(ret.text("value"), "a + b");
    }

    #[test]
    fn object_literal_keeps_source_form() {
        let tree = parse("operation x\n    return {ready: true}\nend\n");
        let ret = &tree.nodes[0].children()[0];
        assert_eq!(ret.text("value"), "{ready: true}");
    }

    #[test]
    fn unknown_keyword_gets_did_you_mean() {
        let tree = parse("operaton demo.x\n    return 1\nend\n");
        let d = &tree.diagnostics[0];
        assert_eq!(d.code, "syntax.unknown_statement");
        assert!(
            d.help.as_deref().unwrap_or("").contains("operation"),
            "{d:?}"
        );
    }

    #[test]
    fn two_space_indentation_is_rejected() {
        let tree = parse("operation x\n  return 1\nend\n");
        assert!(tree.diagnostics.iter().any(|d| d.code == "syntax.indent"));
    }

    #[test]
    fn multiline_object_continuations_are_not_indentation_errors() {
        let tree = parse("operation x\n    return {\n      a: 1\n    }\nend\n");
        assert!(
            !tree.diagnostics.iter().any(|d| d.code == "syntax.indent"),
            "{:?}",
            tree.diagnostics
        );
    }
}
