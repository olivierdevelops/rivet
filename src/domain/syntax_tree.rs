//! Rivet's own syntax tree. The parser adapter builds it from Capy's versioned
//! AST JSON (schema_version 1) so no Capy-internal type crosses into Rivet
//! (RES-2026-0001 finding 1).

use super::source::SourceSpan;

// vhco:domain Capture { text: string; source_text: string; is_expr: bool; span?: SourceSpan; sub: SyntaxNode[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Capture {
    /// Capy's normalized rendering (object keys quoted, …).
    pub text: String,
    /// Exact source text sliced by span (RES-2026-0001 finding 2); equals `text`
    /// when the capture fell back to its default and has no span.
    pub source_text: String,
    pub is_expr: bool,
    pub span: Option<SourceSpan>,
    pub sub: Vec<SyntaxNode>,
}

// vhco:domain SyntaxNode { func: string; span: SourceSpan; captures: map<string, Capture>; body?: SyntaxNode[]; closer?: SyntaxNode }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SyntaxNode {
    pub func: String,
    pub span: SourceSpan,
    pub captures: Vec<(String, Capture)>,
    pub body: Option<Vec<SyntaxNode>>,
    pub closer: Option<Box<SyntaxNode>>,
}

impl SyntaxNode {
    pub fn capture(&self, name: &str) -> Option<&Capture> {
        self.captures
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, c)| c)
    }

    /// Source text of a capture, or "" when absent/defaulted-empty.
    pub fn text(&self, name: &str) -> &str {
        self.capture(name)
            .map(|c| c.source_text.as_str())
            .unwrap_or("")
    }

    pub fn capture_span(&self, name: &str) -> SourceSpan {
        self.capture(name)
            .and_then(|c| c.span.clone())
            .unwrap_or_else(|| self.span.clone())
    }

    pub fn children(&self) -> &[SyntaxNode] {
        self.body.as_deref().unwrap_or(&[])
    }
}

// vhco:domain SyntaxDiagnostic { code: string; message: string; span: SourceSpan; help?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxDiagnostic {
    pub code: String,
    pub message: String,
    pub span: SourceSpan,
    pub help: Option<String>,
}

// vhco:domain SyntaxTree { file: string; nodes: SyntaxNode[]; diagnostics: SyntaxDiagnostic[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SyntaxTree {
    pub file: String,
    pub nodes: Vec<SyntaxNode>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

impl SyntaxTree {
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}
