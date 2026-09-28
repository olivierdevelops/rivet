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

/// Every prefix-call function the language knows: `check` rejects any other
/// name (check.unknown_function) and the interpreter dispatches exactly these.
///
/// ```text
///   (len x)  ──check──▶  check.unknown_function  "did you mean `length`?"
/// ```
pub const BUILTIN_FUNCTIONS: &[&str] = &[
    "request",
    "request.stream",
    "length",
    "base64.encode",
    "base64.decode",
    "text",
    "keys",
    "xml.element",
];

/// The built-in function name closest to `name` (edit distance ≤ 3, or a
/// prefix/containment match such as `len` → `length`).
pub fn closest_builtin(name: &str) -> Option<&'static str> {
    fn distance(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut cur = vec![i + 1];
            for (j, cb) in b.iter().enumerate() {
                let cost = usize::from(ca != *cb);
                cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
            }
            prev = cur;
        }
        prev[b.len()]
    }
    BUILTIN_FUNCTIONS
        .iter()
        .map(|f| {
            let d = distance(name, f);
            let related = name.len() >= 3 && (f.starts_with(name) || name.starts_with(f));
            (if related { d.min(1) } else { d }, *f)
        })
        .filter(|(d, _)| *d <= 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, f)| f)
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

// vhco:domain Operation { id: string; kind: OperationKind; name: string; description?: string; private: bool; params: ParamSpec[]; output: OutputSpec; emits?: ValueSpec; receives?: ValueSpec; emits_description?: string; receives_description?: string; errors: DeclaredError[]; body: Stmt[]; span: SourceSpan; file: string; calls: string[]; module: string; param_spans: SourceSpan[] }
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
    /// `emits … description "…"` / `receives … description "…"`.
    pub emits_description: Option<String>,
    pub receives_description: Option<String>,
    pub errors: Vec<DeclaredError>,
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
    pub file: String,
    /// Literal `(request "id" …)` targets, for call-graph checks.
    pub calls: Vec<String>,
    /// Canonical module alias the operation was namespaced by (`""` for the
    /// entry bundle, `users` for `import "./users.rivet" as users`).
    pub module: String,
    /// Span of each parameter's name, parallel to `params` (check diagnostics).
    pub param_spans: Vec<SourceSpan>,
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

// vhco:domain GlobalDecl { name: string; expr: Expr; span: SourceSpan; expr_span: SourceSpan }
/// `global NAME = EXPR` at the top level of one file (PROP-2026-0002 R7).
/// `span` covers NAME; the file is `span.file`.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalDecl {
    pub name: String,
    pub expr: Expr,
    pub span: SourceSpan,
    /// Span of EXPR (the right-hand side), for check diagnostics.
    pub expr_span: SourceSpan,
}

// vhco:domain GlobalScope { file: string; names: string[]; values: Value }
/// The frozen, read-only constants of one file, evaluated once at load in
/// declaration order. Frames look names up here after locals and params.
///
/// ```text
///  lookup(name): locals ──▶ params ──▶ GlobalScope(file of the operation) ──▶ not defined
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GlobalScope {
    pub file: String,
    pub names: Vec<String>,
    /// An object `{name: value}` in declaration order.
    pub values: Value,
}

impl GlobalScope {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }
}

// vhco:domain CompiledProgram { operations: Operation[]; connectors: Declaration[]; auth_profiles: Declaration[]; source_hash: string; entry: string; root: string; warnings: RivetError[]; globals: GlobalDecl[]; global_scopes: GlobalScope[]; modules: ModuleRef[] }
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
    /// Every `global` declaration of every file, in source order.
    pub globals: Vec<GlobalDecl>,
    /// One frozen scope per file that declares globals (`language.compile_globals`).
    pub global_scopes: Vec<GlobalScope>,
    /// Resolved modules (empty for a single-file bundle).
    pub modules: Vec<super::source::ModuleRef>,
}

impl CompiledProgram {
    /// The frozen globals of `file` (none when the file declares no globals).
    pub fn global_scope(&self, file: &str) -> Option<&GlobalScope> {
        self.global_scopes.iter().find(|g| g.file == file)
    }

    /// The resolved module a file belongs to.
    pub fn module_of(&self, file: &str) -> Option<&super::source::ModuleRef> {
        self.modules.iter().find(|m| m.file == file)
    }

    /// Imported module files (not the entry), in resolution order.
    pub fn module_files(&self) -> Vec<String> {
        self.modules
            .iter()
            .filter(|m| !m.alias.is_empty())
            .map(|m| m.file.clone())
            .collect()
    }

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

impl Expr {
    /// Visit this expression and every sub-expression, parents first; `f` may
    /// rewrite a node in place (the rewritten node's children are visited).
    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Expr)) {
        f(self);
        match self {
            Expr::List(items) => items.iter_mut().for_each(|i| i.walk_mut(f)),
            Expr::Object(pairs) => pairs.iter_mut().for_each(|(_, v)| v.walk_mut(f)),
            Expr::Call { args, .. } => args.iter_mut().for_each(|a| a.walk_mut(f)),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.walk_mut(f);
                rhs.walk_mut(f);
            }
            Expr::Not(i) | Expr::Neg(i) => i.walk_mut(f),
            Expr::Lit(_) | Expr::Template(_) | Expr::Path(..) => {}
        }
    }
}

fn walk_args_mut(args: &mut [Arg], f: &mut dyn FnMut(&mut Expr)) {
    for a in args {
        if let Arg::Expr(e, _) = a {
            e.walk_mut(f);
        }
    }
}

fn walk_options_mut(options: &mut [OptionLine], f: &mut dyn FnMut(&mut Expr)) {
    for o in options {
        walk_args_mut(&mut o.args, f);
        walk_options_mut(&mut o.children, f);
    }
}

fn walk_form_mut(form: &mut EffectForm, f: &mut dyn FnMut(&mut Expr)) {
    walk_args_mut(&mut form.head, f);
    walk_options_mut(&mut form.options, f);
}

fn walk_rhs_mut(r: &mut Rhs, f: &mut dyn FnMut(&mut Expr)) {
    match r {
        Rhs::Expr(e) => e.walk_mut(f),
        Rhs::Effect(form) => walk_form_mut(form, f),
        Rhs::Member(m) => walk_args_mut(&mut m.args, f),
        Rhs::Map { iter, body, .. } => {
            iter.walk_mut(f);
            walk_body_exprs_mut(body, f);
        }
        Rhs::Poll { body, .. } => walk_body_exprs_mut(body, f),
    }
}

/// Visit every expression of an operation body (statements, effect heads,
/// option lines, DAG nodes, nested blocks), parents first (module call
/// resolution rewrites calls through this).
pub fn walk_body_exprs_mut(body: &mut [Stmt], f: &mut dyn FnMut(&mut Expr)) {
    for s in body {
        match s {
            Stmt::Assign { rhs, options, .. } => {
                walk_rhs_mut(rhs, f);
                walk_options_mut(options, f);
            }
            Stmt::Return { value, .. } | Stmt::Yield { value, .. } | Stmt::Emit { value, .. } => {
                walk_rhs_mut(value, f)
            }
            Stmt::Append { value, .. } => value.walk_mut(f),
            Stmt::Fail { details, .. } => details.walk_mut(f),
            Stmt::Until { cond, .. } => cond.walk_mut(f),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                cond.walk_mut(f);
                walk_body_exprs_mut(then, f);
                walk_body_exprs_mut(otherwise, f);
            }
            Stmt::While { cond, body, .. } => {
                cond.walk_mut(f);
                walk_body_exprs_mut(body, f);
            }
            Stmt::For { iter, body, .. } => {
                iter.walk_mut(f);
                walk_body_exprs_mut(body, f);
            }
            Stmt::Try { body, handler, .. } => {
                walk_body_exprs_mut(body, f);
                walk_body_exprs_mut(handler, f);
            }
            Stmt::Dag { nodes, .. } => nodes.iter_mut().for_each(|n| n.expr.walk_mut(f)),
            Stmt::Concurrent { tasks, .. } => tasks
                .iter_mut()
                .for_each(|(_, b)| walk_body_exprs_mut(b, f)),
            Stmt::Iterate { body, .. } | Stmt::Scope { body, .. } => walk_body_exprs_mut(body, f),
            Stmt::With {
                form, source, body, ..
            } => {
                walk_form_mut(form, f);
                if let Some(e) = source {
                    e.walk_mut(f);
                }
                walk_body_exprs_mut(body, f);
            }
            Stmt::Effect { form, .. } => walk_form_mut(form, f),
            Stmt::Member { call, .. } => walk_args_mut(&mut call.args, f),
            Stmt::Call { expr, .. } => expr.walk_mut(f),
            Stmt::Break { .. } | Stmt::Secret { .. } => {}
        }
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
