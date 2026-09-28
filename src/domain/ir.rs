//! Typed intermediate representation produced by `compile_program`.
//!
//! Capy decides statement shapes; lowering turns each shape into these typed
//! forms. Effect forms stay generic (`EffectForm` = kind + head arguments +
//! option lines) so one representation serves the interpreter, the effect
//! analysis and the I/O manifest.

use super::outputs::{DeclaredError, OutputSpec, ParamSpec, ValueSpec};
use super::source::SourceSpan;
use super::value::Value;

// vhco:domain BinOp { add | sub | mul | div | rem | eq | ne | lt | le | gt | ge | and | or }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

// vhco:domain TemplatePart { lit: string | path: string[] }
#[derive(Clone, Debug, PartialEq)]
pub enum TemplatePart {
    Lit(String),
    Path(Vec<String>),
}

// vhco:domain Expr { lit | template | path | list | object | call | binary | not | neg }
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Lit(Value),
    /// String with `${dotted.path}` parts.
    Template(Vec<TemplatePart>),
    Path(Vec<String>, SourceSpan),
    List(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    /// Capy prefix call `(func arg …)`.
    Call {
        func: String,
        args: Vec<Expr>,
        span: SourceSpan,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Not(Box<Expr>),
    Neg(Box<Expr>),
}

impl Expr {
    /// Literal text value, if this is a constant string (no interpolation).
    pub fn const_text(&self) -> Option<String> {
        match self {
            Expr::Lit(Value::Text(s)) => Some(s.clone()),
            Expr::Template(parts) if parts.iter().all(|p| matches!(p, TemplatePart::Lit(_))) => {
                Some(
                    parts
                        .iter()
                        .map(|p| match p {
                            TemplatePart::Lit(s) => s.as_str(),
                            TemplatePart::Path(_) => "",
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }

    /// Every dotted path this expression reads (for dependency and param analysis).
    pub fn paths(&self, out: &mut Vec<Vec<String>>) {
        match self {
            Expr::Lit(_) => {}
            Expr::Template(parts) => {
                for p in parts {
                    if let TemplatePart::Path(path) = p {
                        out.push(path.clone());
                    }
                }
            }
            Expr::Path(p, _) => out.push(p.clone()),
            Expr::List(items) => items.iter().for_each(|e| e.paths(out)),
            Expr::Object(pairs) => pairs.iter().for_each(|(_, e)| e.paths(out)),
            Expr::Call { args, .. } => args.iter().for_each(|e| e.paths(out)),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.paths(out);
                rhs.paths(out);
            }
            Expr::Not(e) | Expr::Neg(e) => e.paths(out),
        }
    }
}

// vhco:domain Arg { word: string | expr: Expr }
/// One argument of an option line or effect head. Bare identifiers stay
/// `Word` (keywords such as `json`, `status`, `on`); lowering decides whether a
/// word names a variable.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Word(String, SourceSpan),
    Expr(Expr, SourceSpan),
}

impl Arg {
    pub fn word(&self) -> Option<&str> {
        match self {
            Arg::Word(w, _) => Some(w),
            _ => None,
        }
    }

    pub fn span(&self) -> &SourceSpan {
        match self {
            Arg::Word(_, s) | Arg::Expr(_, s) => s,
        }
    }

    /// Treat a word as a variable reference when an expression is needed.
    pub fn to_expr(&self) -> Expr {
        match self {
            Arg::Word(w, s) => match w.as_str() {
                "true" => Expr::Lit(Value::Bool(true)),
                "false" => Expr::Lit(Value::Bool(false)),
                "null" => Expr::Lit(Value::Null),
                _ => Expr::Path(vec![w.clone()], s.clone()),
            },
            Arg::Expr(e, _) => e.clone(),
        }
    }
}

// vhco:domain OptionLine { key: string; args: Arg[]; span: SourceSpan; children: OptionLine[] }
#[derive(Clone, Debug, PartialEq)]
pub struct OptionLine {
    pub key: String,
    pub args: Vec<Arg>,
    pub span: SourceSpan,
    pub children: Vec<OptionLine>,
}

impl OptionLine {
    pub fn first_word(&self) -> Option<&str> {
        self.args.first().and_then(Arg::word)
    }
}

// vhco:domain EffectKind { http | file | grpc | command | tcp | unix | pipe | udp | quic | websocket | request_stream | connection | other }
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Http,
    File,
    Grpc,
    Command,
    Tcp,
    Unix,
    Pipe,
    Udp,
    Quic,
    WebSocket,
    RequestStream,
    /// `connection.open uni`, `connection.accept bidi` inside a QUIC scope.
    Connection,
    Other(String),
}

impl EffectKind {
    pub fn parse(word: &str) -> EffectKind {
        match word {
            "http" => EffectKind::Http,
            "file" => EffectKind::File,
            "grpc" => EffectKind::Grpc,
            "command" => EffectKind::Command,
            "tcp" => EffectKind::Tcp,
            "unix" => EffectKind::Unix,
            "pipe" => EffectKind::Pipe,
            "udp" => EffectKind::Udp,
            "quic" => EffectKind::Quic,
            "websocket" => EffectKind::WebSocket,
            other => EffectKind::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            EffectKind::Http => "http",
            EffectKind::File => "file",
            EffectKind::Grpc => "grpc",
            EffectKind::Command => "command",
            EffectKind::Tcp => "tcp",
            EffectKind::Unix => "unix",
            EffectKind::Pipe => "pipe",
            EffectKind::Udp => "udp",
            EffectKind::Quic => "quic",
            EffectKind::WebSocket => "websocket",
            EffectKind::RequestStream => "request_stream",
            EffectKind::Connection => "connection",
            EffectKind::Other(s) => s,
        }
    }
}

// vhco:domain EffectForm { kind: EffectKind; head: Arg[]; options: OptionLine[]; span: SourceSpan; effect_ids: string[] }
/// `http get "url"`, `file create PATH json V`, `tcp "host:port"`, … plus the
/// option lines of its block.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectForm {
    pub kind: EffectKind,
    pub head: Vec<Arg>,
    pub options: Vec<OptionLine>,
    pub span: SourceSpan,
    /// IDs of the manifest sites this form produces (filled by effect analysis).
    pub effect_ids: Vec<String>,
}

impl EffectForm {
    pub fn option(&self, key: &str) -> Option<&OptionLine> {
        self.options.iter().find(|o| o.key == key)
    }

    pub fn options_named<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a OptionLine> + 'a {
        self.options.iter().filter(move |o| o.key == key)
    }
}

// vhco:domain MemberCall { object: string[]; method: string; args: Arg[]; span: SourceSpan }
/// `socket.send json {…}`, `conn.receive json timeout "5s"`, `rpc.finish_send`.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberCall {
    pub object: Vec<String>,
    pub method: String,
    pub args: Vec<Arg>,
    pub span: SourceSpan,
}

// vhco:domain Rhs { expr | effect | member | map | poll }
#[derive(Clone, Debug, PartialEq)]
pub enum Rhs {
    Expr(Expr),
    Effect(EffectForm),
    Member(MemberCall),
    Map {
        item: String,
        iter: Expr,
        limit: Option<i64>,
        body: Vec<Stmt>,
    },
    Poll {
        every: Option<u64>,
        timeout: Option<u64>,
        body: Vec<Stmt>,
    },
}

// vhco:domain FailurePolicy { fast | independent }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FailurePolicy {
    #[default]
    Fast,
    Independent,
}

// vhco:domain GroupOptions { limit?: int; timeout_ms?: int; failure: FailurePolicy }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GroupOptions {
    pub limit: Option<i64>,
    pub timeout_ms: Option<u64>,
    pub failure: FailurePolicy,
}

// vhco:domain DagNode { name: string; after: string[]; expr: Expr; span: SourceSpan }
#[derive(Clone, Debug, PartialEq)]
pub struct DagNode {
    pub name: String,
    pub after: Vec<String>,
    pub expr: Expr,
    pub span: SourceSpan,
}

// vhco:domain CatchFilter { kind?: string; code?: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CatchFilter {
    pub kind: Option<String>,
    pub code: Option<String>,
}

// vhco:domain Stmt { assign | append | return | yield | emit | fail | break | if | while | for | try | dag | concurrent | iterate | scope | with | effect | member | call | secret | until }
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Assign {
        var: String,
        rhs: Rhs,
        options: Vec<OptionLine>,
        span: SourceSpan,
    },
    Append {
        var: String,
        value: Expr,
        span: SourceSpan,
    },
    Return {
        value: Rhs,
        span: SourceSpan,
    },
    Yield {
        value: Rhs,
        span: SourceSpan,
    },
    Emit {
        value: Rhs,
        span: SourceSpan,
    },
    Fail {
        code: String,
        details: Expr,
        span: SourceSpan,
    },
    Break {
        span: SourceSpan,
    },
    If {
        cond: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
        span: SourceSpan,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    Try {
        body: Vec<Stmt>,
        filter: CatchFilter,
        handler: Vec<Stmt>,
        span: SourceSpan,
    },
    Dag {
        options: GroupOptions,
        nodes: Vec<DagNode>,
        span: SourceSpan,
    },
    Concurrent {
        options: GroupOptions,
        tasks: Vec<(String, Vec<Stmt>)>,
        span: SourceSpan,
    },
    Iterate {
        max: i64,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    Scope {
        timeout_ms: Option<u64>,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    With {
        form: EffectForm,
        source: Option<Expr>,
        bind: Option<String>,
        body: Vec<Stmt>,
        span: SourceSpan,
    },
    Effect {
        form: EffectForm,
        span: SourceSpan,
    },
    Member {
        call: MemberCall,
        span: SourceSpan,
    },
    Call {
        expr: Expr,
        span: SourceSpan,
    },
    Secret {
        name: String,
        env: String,
        origins: Vec<String>,
        span: SourceSpan,
    },
    Until {
        cond: Expr,
        span: SourceSpan,
    },
}

impl Stmt {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Stmt::Assign { span, .. }
            | Stmt::Append { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Yield { span, .. }
            | Stmt::Emit { span, .. }
            | Stmt::Fail { span, .. }
            | Stmt::Break { span }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Try { span, .. }
            | Stmt::Dag { span, .. }
            | Stmt::Concurrent { span, .. }
            | Stmt::Iterate { span, .. }
            | Stmt::Scope { span, .. }
            | Stmt::With { span, .. }
            | Stmt::Effect { span, .. }
            | Stmt::Member { span, .. }
            | Stmt::Call { span, .. }
            | Stmt::Secret { span, .. }
            | Stmt::Until { span, .. } => span,
        }
    }
}

// vhco:domain OperationKind { operation | pipeline }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Operation,
    Pipeline,
}

// vhco:domain Operation { id: string; kind: OperationKind; name: string; description?: string; private: bool; params: ParamSpec[]; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; errors: DeclaredError[]; body: Stmt[]; span: SourceSpan; file: string; calls: string[] }
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    pub id: String,
    pub kind: OperationKind,
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub params: Vec<ParamSpec>,
    pub output: OutputSpec,
    pub emits: Option<ValueSpec>,
    pub receives: Option<ValueSpec>,
    pub errors: Vec<DeclaredError>,
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
    pub file: String,
    /// Literal `(request "id" …)` targets, for call-graph checks.
    pub calls: Vec<String>,
}

// vhco:domain Declaration { name: string; kind: string; head: Arg[]; options: OptionLine[]; span: SourceSpan }
/// `connector NAME mcp|grpc … end` and `auth NAME oauth2 … end`.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub kind: String,
    pub options: Vec<OptionLine>,
    pub span: SourceSpan,
}

impl Declaration {
    pub fn option(&self, key: &str) -> Option<&OptionLine> {
        self.options.iter().find(|o| o.key == key)
    }
}

// vhco:domain CompiledProgram { operations: Operation[]; connectors: Declaration[]; auth_profiles: Declaration[]; source_hash: string; entry: string; root: string; warnings: RivetError[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CompiledProgram {
    pub operations: Vec<Operation>,
    pub connectors: Vec<Declaration>,
    pub auth_profiles: Vec<Declaration>,
    pub source_hash: String,
    pub entry: String,
    /// Bundle root: relative paths in source resolve against it.
    pub root: String,
    pub warnings: Vec<crate::domain::errors::RivetError>,
}

impl CompiledProgram {
    pub fn operation(&self, id: &str) -> Option<&Operation> {
        self.operations.iter().find(|o| o.id == id)
    }

    pub fn connector(&self, name: &str) -> Option<&Declaration> {
        self.connectors.iter().find(|c| c.name == name)
    }

    pub fn auth_profile(&self, name: &str) -> Option<&Declaration> {
        self.auth_profiles.iter().find(|c| c.name == name)
    }
}

/// Parse a quoted duration literal body (`"100ms"`, `"5s"`, `"2m"`, `"1h"`) to milliseconds.
pub fn parse_duration_ms(text: &str) -> Option<u64> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let n: u64 = digits.parse().ok()?;
    match &text[digits.len()..] {
        "ms" => Some(n),
        "s" => n.checked_mul(1_000),
        "m" => n.checked_mul(60_000),
        "h" => n.checked_mul(3_600_000),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_accept_only_the_documented_units() {
        assert_eq!(parse_duration_ms("100ms"), Some(100));
        assert_eq!(parse_duration_ms("5s"), Some(5_000));
        assert_eq!(parse_duration_ms("2m"), Some(120_000));
        assert_eq!(parse_duration_ms("1h"), Some(3_600_000));
        assert_eq!(parse_duration_ms("5"), None);
        assert_eq!(parse_duration_ms("5sec"), None);
        assert_eq!(parse_duration_ms("s"), None);
    }
}
