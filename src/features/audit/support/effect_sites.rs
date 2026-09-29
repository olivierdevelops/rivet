//! Static effect analysis over the compiled IR (no re-parse, no I/O, nothing
//! evaluated). Produces the non-transitive sites of every operation; the
//! `audit.inspect_effects` use case selects, joins and renders them.
//!
//! ```text
//!  Operation.body ──walk──▶ statement ──▶ sites (statement first, then its option lines)
//!                              │            file update  → stat + update
//!                              │            http … tls   → connect + file reads (before_connect)
//!                              │            http … auth  → env + credential r/w + auth use + token connect
//!                              └─(request "x")──▶ CallSite (operation) | connector expansion (mcp/grpc)
//! ```

use crate::domain::io_manifest::{
    CallArg, CallSite, EffectCatalog, EffectSite, Knowledge, OperationEffects, Phase, SiteKind,
    SiteOrigin, SiteTarget,
};
use crate::domain::ir::{
    Arg, BinOp, CompiledProgram, Declaration, EffectForm, EffectKind, Expr, MemberCall, Operation,
    OptionLine, Rhs, Stmt, TemplatePart,
};
use crate::domain::policy::AccessVerb;
use crate::domain::source::SourceSpan;
use crate::domain::value::Value;
use std::collections::HashMap;

/// Analyze every operation of a compiled program (pure; deterministic IDs `op#N`).
pub fn analyze_program(program: &CompiledProgram) -> EffectCatalog {
    let root = program.root.as_str();
    let mut operations = Vec::new();
    for op in &program.operations {
        let mut w = Walker::new(program, op, root);
        w.block(&op.body);
        w.finish();
        operations.push(OperationEffects {
            operation_id: op.id.clone(),
            private: op.private,
            sites: w.sites,
            calls: w.calls,
        });
    }
    let mut load_sites = Vec::new();
    for c in &program.connectors {
        for key in ["descriptor", "schema"] {
            for o in c.options.iter().filter(|o| o.key == key) {
                if let Some(p) = o.args.first().and_then(|a| a.to_expr().const_text()) {
                    let mut s =
                        EffectSite::new("bundle load", SiteKind::File, vec![AccessVerb::Read]);
                    s.effect_id = format!("{}#{key}", c.name);
                    s.target = file_target_exact(&p);
                    s.source = span_rel(&o.span, root);
                    s.statement_line = o.span.start_line;
                    s.origin = SiteOrigin::Option(key.to_string());
                    s.phase = Phase::Load;
                    s.requires_existing = true;
                    s.call_chain = Vec::new();
                    load_sites.push(s);
                }
            }
        }
    }
    EffectCatalog {
        operations,
        load_sites,
        bundle_file: rel_file(&program.entry, root),
        bundle_sha256: program.source_hash.clone(),
    }
}

/// Source file relative to the bundle root (`app.rivet`).
pub fn rel_file(file: &str, root: &str) -> String {
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

fn span_rel(span: &SourceSpan, root: &str) -> SourceSpan {
    let mut s = span.clone();
    s.file = rel_file(&span.file, root);
    s
}

/// Render an expression back to compact source form (for conditions and dynamic targets).
pub fn expr_text(e: &Expr) -> String {
    match e {
        Expr::Lit(v) => match v {
            Value::Text(s) => format!("{s:?}"),
            other => other.to_display(),
        },
        Expr::Template(parts) => {
            let mut s = String::from("\"");
            for p in parts {
                match p {
                    TemplatePart::Lit(t) => s.push_str(t),
                    TemplatePart::Path(p) => s.push_str(&format!("${{{}}}", p.join("."))),
                }
            }
            s.push('"');
            s
        }
        Expr::Path(p, _) => p.join("."),
        Expr::List(items) => format!(
            "[{}]",
            items.iter().map(expr_text).collect::<Vec<_>>().join(", ")
        ),
        Expr::Object(pairs) => format!(
            "{{{}}}",
            pairs
                .iter()
                .map(|(k, v)| format!("{k}: {}", expr_text(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Call { func, args, .. } => format!(
            "({func}{})",
            args.iter()
                .map(|a| format!(" {}", expr_text(a)))
                .collect::<String>()
        ),
        Expr::Binary { op, lhs, rhs } => {
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Rem => "%",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
                BinOp::And => "and",
                BinOp::Or => "or",
            };
            format!("{} {o} {}", expr_text(lhs), expr_text(rhs))
        }
        Expr::Not(e) => format!("not {}", expr_text(e)),
        Expr::Neg(e) => format!("-{}", expr_text(e)),
    }
}

fn file_target_exact(p: &str) -> SiteTarget {
    SiteTarget {
        template: p.to_string(),
        path: Some(p.to_string()),
        ..SiteTarget::default()
    }
}

/// Lexical normal form of a bundle-relative path (for "created earlier" checks).
pub fn norm_rel(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|l| *l != "..") {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

/// How a template placeholder is classified.
enum Placeholder {
    Param(String),
    /// A `global` constant (or a path into one): substituted as its value (R9).
    Const(String),
    Other(String),
}

struct Walker<'a> {
    program: &'a CompiledProgram,
    op: &'a Operation,
    root: &'a str,
    sites: Vec<EffectSite>,
    calls: Vec<CallSite>,
    secrets: HashMap<String, Vec<String>>,
    handles: HashMap<String, EffectKind>,
    conditions: Vec<String>,
    /// `as NAME` of the `with` block being analyzed.
    bind: Option<String>,
    /// The operation file's frozen globals (PROP-2026-0002 R9).
    globals: Option<&'a crate::domain::ir::GlobalScope>,
}

impl<'a> Walker<'a> {
    fn new(program: &'a CompiledProgram, op: &'a Operation, root: &'a str) -> Walker<'a> {
        Walker {
            program,
            op,
            root,
            sites: Vec::new(),
            calls: Vec::new(),
            secrets: HashMap::new(),
            handles: HashMap::new(),
            conditions: Vec::new(),
            bind: None,
            globals: program.global_scope(&op.file),
        }
    }

    /// The value of a global (or a path into one) this operation can read.
    /// `check.global_shadow` guarantees params and locals never reuse the name.
    fn global(&self, path: &[String]) -> Option<Value> {
        let g = self.globals?;
        if self.param(&path[0]).is_some() {
            return None;
        }
        crate::domain::const_eval::read_path(g.get(&path[0])?, &path[1..])
    }

    /// Fold an expression that reads only literals and globals.
    fn constant(&self, e: &Expr) -> Option<Value> {
        let lookup = |p: &[String]| self.global(p);
        crate::domain::const_eval::fold(e, &lookup).ok().flatten()
    }

    fn param(&self, name: &str) -> Option<&crate::domain::outputs::ParamSpec> {
        self.op.params.iter().find(|p| p.name == name)
    }

    fn classify(&self, path: &[String]) -> Placeholder {
        let head = path.first().cloned().unwrap_or_default();
        if path.len() == 1 && self.param(&head).is_some() {
            Placeholder::Param(head)
        } else if let Some(v) = self.global(path) {
            Placeholder::Const(v.to_display())
        } else {
            Placeholder::Other(path.join("."))
        }
    }

    /// Turn a target expression into (template, params, knowledge, expression).
    fn template_of(&self, e: &Expr) -> (String, Vec<String>, Knowledge, Option<String>) {
        self.template_in(e, false)
    }

    /// `url`: a global interpolated after the URL's scheme and authority is
    /// percent-encoded for its component, exactly as the runtime's
    /// component-aware interpolation does (`domain::transports::assemble_url`).
    fn template_in(&self, e: &Expr, url: bool) -> (String, Vec<String>, Knowledge, Option<String>) {
        let mut params = Vec::new();
        let mut dynamic = false;
        let mut text = String::new();
        let mut push_path = |p: &[String], text: &mut String| match self.classify(p) {
            Placeholder::Param(n) => {
                text.push_str(&format!("{{{n}}}"));
                if !params.contains(&n) {
                    params.push(n);
                }
            }
            Placeholder::Const(v) => {
                if url {
                    use crate::domain::transports::{UrlPiece, assemble_url};
                    match assemble_url(&[UrlPiece::Literal(text.clone()), UrlPiece::Value(v)]) {
                        Ok(full) => *text = full,
                        Err(_) => dynamic = true,
                    }
                } else {
                    text.push_str(&v);
                }
            }
            Placeholder::Other(x) => {
                text.push_str(&format!("{{{x}}}"));
                dynamic = true;
            }
        };
        // Templates and paths are substituted part by part below (component-aware
        // for URLs); any other expression is folded whole.
        if !matches!(e, Expr::Lit(_) | Expr::Template(_) | Expr::Path(..))
            && let Some(v) = self.constant(e)
            && !matches!(v, Value::List(_) | Value::Object(_) | Value::Null)
        {
            // Built only from literals and globals: an exact target (R9).
            return (v.to_display(), Vec::new(), Knowledge::Exact, None);
        }
        match e {
            Expr::Lit(Value::Text(s)) => text.push_str(s),
            Expr::Template(parts) => {
                for p in parts {
                    match p {
                        TemplatePart::Lit(s) => text.push_str(s),
                        TemplatePart::Path(p) => push_path(p, &mut text),
                    }
                }
            }
            Expr::Path(p, _) => match self.classify(p) {
                Placeholder::Param(n) => {
                    text = format!("{{{n}}}");
                    params.push(n);
                }
                Placeholder::Const(v) => text = v,
                Placeholder::Other(x) => {
                    return (String::new(), Vec::new(), Knowledge::Dynamic, Some(x));
                }
            },
            other => {
                return (
                    String::new(),
                    Vec::new(),
                    Knowledge::Dynamic,
                    Some(expr_text(other)),
                );
            }
        }
        let knowledge = if dynamic {
            Knowledge::Dynamic
        } else if params.is_empty() {
            Knowledge::Exact
        } else if params
            .iter()
            .all(|p| self.param(p).is_some_and(|s| s.enum_values.is_some()))
        {
            Knowledge::Bounded
        } else {
            Knowledge::ParamDependent
        };
        (text, params, knowledge, None)
    }

    fn expand_bound(&self, template: &str, params: &[String]) -> Vec<String> {
        let mut out = vec![template.to_string()];
        for p in params {
            let values: Vec<String> = self
                .param(p)
                .and_then(|s| s.enum_values.clone())
                .unwrap_or_default()
                .iter()
                .map(|v| v.to_display())
                .collect();
            let mut next = Vec::new();
            for t in &out {
                for v in &values {
                    next.push(t.replace(&format!("{{{p}}}"), v));
                }
            }
            out = next;
            if out.len() > 64 {
                out.truncate(64);
            }
        }
        out
    }

    fn path_target(&self, e: &Expr) -> (SiteTarget, Knowledge, Option<String>) {
        let (template, params, knowledge, expression) = self.template_of(e);
        let glob = if knowledge == Knowledge::ParamDependent {
            Some(glob_of(&template))
        } else {
            None
        };
        let bound = if knowledge == Knowledge::Bounded {
            self.expand_bound(&template, &params)
        } else {
            Vec::new()
        };
        (
            SiteTarget {
                path: (!template.is_empty()).then(|| template.clone()),
                template,
                glob,
                params,
                bound,
                ..SiteTarget::default()
            },
            knowledge,
            expression,
        )
    }

    /// URL-like target (`https://h/p`, `tcp://h:p`, bare `h:p` with a scheme).
    fn url_target(
        &self,
        e: &Expr,
        default_scheme: &str,
        query: &[(String, String, Vec<String>)],
    ) -> (SiteTarget, Knowledge, Option<String>) {
        let (mut template, mut params, mut knowledge, expression) = self.template_in(e, true);
        if template.is_empty() {
            return (SiteTarget::default(), knowledge, expression);
        }
        if !template.contains("://") {
            template = format!("{default_scheme}://{template}");
        }
        for (i, (k, v, ps)) in query.iter().enumerate() {
            template.push(if i == 0 && !template.contains('?') {
                '?'
            } else {
                '&'
            });
            template.push_str(&format!("{k}={v}"));
            for p in ps {
                if !params.contains(p) {
                    params.push(p.clone());
                }
            }
        }
        if !params.is_empty() && knowledge == Knowledge::Exact {
            knowledge = Knowledge::ParamDependent;
        }
        let (scheme, rest) = template.split_once("://").unwrap_or(("", &template));
        let cut = rest.find(['/', '?']).unwrap_or(rest.len());
        let (authority, path) = rest.split_at(cut);
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if !h.ends_with(']') || authority.starts_with('[') => {
                (h.to_string(), p.parse::<u16>().ok())
            }
            _ => (authority.to_string(), None),
        };
        let port = port.or(match scheme {
            "https" | "wss" => Some(443),
            "http" | "ws" => Some(80),
            _ => None,
        });
        if host.contains('{') {
            // A caller-steered host cannot be granted statically.
            knowledge = Knowledge::Dynamic;
        }
        let target = SiteTarget {
            template: template.clone(),
            scheme: Some(scheme.to_string()),
            host: Some(host),
            port,
            path: Some(if path.is_empty() {
                "/".into()
            } else {
                path.to_string()
            }),
            glob: None,
            bound: if knowledge == Knowledge::Bounded {
                self.expand_bound(&template, &params)
            } else {
                Vec::new()
            },
            params,
        };
        (target, knowledge, expression)
    }

    fn secrets_in(&self, form_exprs: &[Expr]) -> Vec<String> {
        let mut paths = Vec::new();
        for e in form_exprs {
            e.paths(&mut paths);
        }
        let mut out: Vec<String> = paths
            .iter()
            .filter_map(|p| p.first())
            .filter(|n| self.secrets.contains_key(*n))
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    fn base(&self, kind: SiteKind, access: Vec<AccessVerb>, span: &SourceSpan) -> EffectSite {
        let mut s = EffectSite::new(&self.op.id, kind, access);
        s.source = span_rel(span, self.root);
        s.statement_line = span.start_line;
        s.condition = (!self.conditions.is_empty()).then(|| self.conditions.join(" and "));
        s
    }

    fn push(&mut self, s: EffectSite) {
        self.sites.push(s);
    }

    /// Assign effect IDs and settle `requires_existing` against earlier creates.
    fn finish(&mut self) {
        let mut created: Vec<String> = Vec::new();
        for (i, s) in self.sites.iter_mut().enumerate() {
            s.effect_id = format!("{}#{}", self.op.id, i + 1);
            if s.kind == SiteKind::File && !s.target.template.is_empty() {
                let key = norm_rel(&s.target.template);
                if s.requires_existing && created.contains(&key) {
                    s.requires_existing = false;
                }
                if s.access.contains(&AccessVerb::Create) {
                    created.push(key);
                }
            }
        }
    }

    fn block(&mut self, body: &[Stmt]) {
        for s in body {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Assign { rhs, span, .. }
            | Stmt::Return { value: rhs, span }
            | Stmt::Yield { value: rhs, span }
            | Stmt::Emit { value: rhs, span } => self.rhs(rhs, span),
            Stmt::Append { value, .. } => self.expr(value),
            Stmt::Fail { details, .. } => self.expr(details),
            Stmt::Break { .. } => {}
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                self.expr(cond);
                self.conditions.push(expr_text(cond));
                self.block(then);
                self.conditions.pop();
                if !otherwise.is_empty() {
                    self.conditions.push(format!("not ({})", expr_text(cond)));
                    self.block(otherwise);
                    self.conditions.pop();
                }
            }
            Stmt::While { cond, body, .. } => {
                self.expr(cond);
                self.block(body);
            }
            Stmt::For { iter, body, .. } => {
                self.expr(iter);
                self.block(body);
            }
            Stmt::Try { body, handler, .. } => {
                self.block(body);
                self.block(handler);
            }
            Stmt::Dag { nodes, .. } => {
                for n in nodes {
                    self.expr(&n.expr);
                }
            }
            Stmt::Concurrent { tasks, .. } => {
                for (_, b) in tasks {
                    self.block(b);
                }
            }
            Stmt::Iterate { body, .. } | Stmt::Scope { body, .. } => self.block(body),
            Stmt::With {
                form,
                source,
                bind,
                body,
                span,
            } => {
                if let Some(src) = source {
                    self.expr(src);
                }
                self.bind = bind.clone();
                self.form(form, span, true, body);
                self.bind = None;
                if let Some(b) = bind {
                    self.handles.insert(b.clone(), form.kind.clone());
                }
                self.block(body);
            }
            Stmt::Effect { form, span } => self.form(form, span, false, &[]),
            Stmt::Member { call, span } => self.member(call, span),
            Stmt::Call { expr, .. } => self.expr(expr),
            Stmt::Secret {
                name,
                env,
                origins,
                span,
            } => {
                self.secrets.insert(name.clone(), origins.clone());
                let mut site = self.base(SiteKind::Env, vec![AccessVerb::Read], span);
                site.target = SiteTarget {
                    template: env.clone(),
                    ..SiteTarget::default()
                };
                site.origin = SiteOrigin::Statement("secret".into());
                site.secret = true;
                site.secrets = vec![name.clone()];
                site.bound_to = origins.clone();
                self.push(site);
            }
            Stmt::Until { cond, .. } => self.expr(cond),
        }
    }

    fn rhs(&mut self, rhs: &Rhs, span: &SourceSpan) {
        match rhs {
            Rhs::Expr(e) => self.expr(e),
            Rhs::Effect(f) => self.form(f, span, false, &[]),
            Rhs::Member(m) => self.member(m, span),
            Rhs::Map { iter, body, .. } => {
                self.expr(iter);
                self.block(body);
            }
            Rhs::Poll { body, .. } => self.block(body),
        }
    }

    /// Literal `(request "id" …)` edges inside any expression.
    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Call { func, args, span } => {
                // A literal ID, or a global holding one (R9), is a static edge.
                if (func == "request" || func == "request.stream")
                    && let Some(id) = args.first().and_then(|a| {
                        a.const_text().or_else(|| match self.constant(a) {
                            Some(Value::Text(t)) => Some(t),
                            _ => None,
                        })
                    })
                {
                    let bound = self.call_args(args.get(1));
                    self.call(&id, span, bound);
                }
                for a in args {
                    self.expr(a);
                }
            }
            Expr::List(items) => items.iter().for_each(|i| self.expr(i)),
            Expr::Object(pairs) => pairs.iter().for_each(|(_, v)| self.expr(v)),
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Not(x) | Expr::Neg(x) => self.expr(x),
            Expr::Lit(_) | Expr::Template(_) | Expr::Path(..) => {}
        }
    }

    /// The statically known arguments of a call's `{key: …}` object: a bare
    /// caller param (`{id: id}`) or a constant (`{id: 3}`, a global).
    fn call_args(&self, data: Option<&Expr>) -> Vec<CallArg> {
        let Some(Expr::Object(pairs)) = data else {
            return Vec::new();
        };
        pairs
            .iter()
            .filter_map(|(k, v)| match v {
                Expr::Path(p, _) if p.len() == 1 && self.param(&p[0]).is_some() => Some(CallArg {
                    param: k.clone(),
                    from_param: Some(p[0].clone()),
                    value: None,
                }),
                _ => self.constant(v).map(|c| CallArg {
                    param: k.clone(),
                    from_param: None,
                    value: Some(c),
                }),
            })
            .collect()
    }

    fn call(&mut self, id: &str, span: &SourceSpan, args: Vec<CallArg>) {
        if self.program.operation(id).is_some() {
            self.calls.push(CallSite {
                operation_id: self.op.id.clone(),
                callee: id.to_string(),
                connector: None,
                source: span_rel(span, self.root),
                args,
            });
            return;
        }
        let Some((conn_name, method)) = id.split_once('.') else {
            return;
        };
        let Some(conn) = self.program.connector(conn_name) else {
            return;
        };
        self.calls.push(CallSite {
            operation_id: self.op.id.clone(),
            callee: id.to_string(),
            connector: Some(conn_name.to_string()),
            source: span_rel(span, self.root),
            args: Vec::new(),
        });
        let conn = conn.clone();
        let before = self.sites.len();
        match conn.kind.as_str() {
            "mcp" => {
                self.transport_sites(&conn, span);
                let (kind_seg, name) = method.split_once('.').unwrap_or(("tools", method));
                let mode = match kind_seg {
                    "resources" => "resource",
                    "prompts" => "prompt",
                    _ => "tool",
                };
                let mut s = self.base(SiteKind::Mcp, vec![AccessVerb::Call], span);
                s.method = Some(mode.into());
                s.target = SiteTarget {
                    template: format!("{conn_name}/{kind_seg}/{name}"),
                    ..SiteTarget::default()
                };
                s.knowledge = Knowledge::OpaqueRemote;
                s.origin = SiteOrigin::Statement("request".into());
                self.push(s);
            }
            "grpc" => {
                self.grpc_sites(&conn, method, "unary", "request", span);
            }
            _ => {}
        }
        for s in &mut self.sites[before..] {
            s.via = Some(id.to_string());
        }
    }

    /// Connector transport: network connect (http) or process exec (command/stdio),
    /// plus its TLS file options.
    fn transport_sites(&mut self, conn: &Declaration, call_span: &SourceSpan) {
        for o in conn.options.iter().filter(|o| o.key == "transport") {
            let mode = o.first_word().unwrap_or("");
            match mode {
                "http" => {
                    if let Some(e) = o.args.get(1).map(Arg::to_expr) {
                        let (t, k, x) = self.url_target(&e, "https", &[]);
                        let mut s =
                            self.option_site(SiteKind::Network, AccessVerb::Connect, o, call_span);
                        s.method = Some("POST".into());
                        s.protocol = Some("http1|http2".into());
                        s.target = t;
                        s.knowledge = k;
                        s.expression = x;
                        s.origin = SiteOrigin::Option("transport http".into());
                        s.phase = Phase::Connect;
                        self.push(s);
                    }
                }
                "command" | "stdio" => {
                    let argv0 = o.args.get(1).map(Arg::to_expr).and_then(|e| match e {
                        Expr::List(items) => items.first().cloned(),
                        other => Some(other),
                    });
                    if let Some(e) = argv0 {
                        let (t, k, x) = self.path_target(&e);
                        let mut s =
                            self.option_site(SiteKind::Process, AccessVerb::Exec, o, call_span);
                        s.target = t;
                        s.knowledge = k;
                        s.expression = x;
                        s.origin = SiteOrigin::Option(format!("transport {mode}"));
                        s.phase = Phase::Connect;
                        self.push(s);
                    }
                }
                _ => {}
            }
        }
        self.tls_sites(&conn.options, call_span);
    }

    fn grpc_sites(
        &mut self,
        conn: &Declaration,
        method: &str,
        mode: &str,
        origin: &str,
        span: &SourceSpan,
    ) {
        if let Some(o) = conn.option("endpoint")
            && let Some(e) = o.args.first().map(Arg::to_expr)
        {
            let (t, k, x) = self.url_target(&e, "https", &[]);
            let mut s = self.option_site(SiteKind::Network, AccessVerb::Connect, o, span);
            s.method = Some("POST".into());
            s.protocol = Some("grpc".into());
            s.target = t;
            s.knowledge = k;
            s.expression = x;
            s.origin = SiteOrigin::Option("endpoint".into());
            s.phase = Phase::Connect;
            self.push(s);
        }
        self.tls_sites(&conn.options, span);
        let service = conn
            .option("service")
            .and_then(|o| o.args.first())
            .and_then(|a| a.to_expr().const_text())
            .unwrap_or_default();
        let mut s = self.base(SiteKind::Grpc, vec![AccessVerb::Call], span);
        s.method = Some(mode.into());
        s.target = SiteTarget {
            template: if service.is_empty() {
                format!("{}/{method}", conn.name)
            } else {
                format!("{}/{service}/{method}", conn.name)
            },
            ..SiteTarget::default()
        };
        s.origin = SiteOrigin::Statement(origin.into());
        self.push(s);
    }

    /// A site produced by an option line: source = the option line, statement line = the block.
    fn option_site(
        &self,
        kind: SiteKind,
        verb: AccessVerb,
        o: &OptionLine,
        stmt: &SourceSpan,
    ) -> EffectSite {
        let mut s = self.base(kind, vec![verb], stmt);
        s.source = span_rel(&o.span, self.root);
        s
    }

    fn file_option_site(
        &mut self,
        o: &OptionLine,
        e: &Expr,
        origin: &str,
        phase: Phase,
        secret: bool,
        stmt: &SourceSpan,
    ) {
        let (t, k, x) = self.path_target(e);
        let mut s = self.option_site(SiteKind::File, AccessVerb::Read, o, stmt);
        s.target = t;
        s.knowledge = k;
        s.expression = x;
        s.origin = SiteOrigin::Option(origin.into());
        s.phase = phase;
        s.requires_existing = true;
        s.secret = secret;
        self.push(s);
    }

    fn tls_sites(&mut self, options: &[OptionLine], stmt: &SourceSpan) {
        for o in options.iter().filter(|o| o.key == "tls") {
            let Some(which) = o.first_word() else {
                continue;
            };
            if !matches!(which, "ca_file" | "cert_file" | "key_file") {
                continue;
            }
            if let Some(e) = o.args.get(1).map(Arg::to_expr) {
                let origin = format!("tls {which}");
                self.file_option_site(
                    o,
                    &e,
                    &origin,
                    Phase::BeforeConnect,
                    which == "key_file",
                    stmt,
                );
            }
        }
    }

    fn body_file_sites(&mut self, options: &[OptionLine], stmt: &SourceSpan) {
        for o in options.iter().filter(|o| o.key == "body") {
            match o.first_word() {
                Some("file") => {
                    if let Some(e) = o.args.get(1).map(Arg::to_expr) {
                        self.file_option_site(o, &e, "body file", Phase::Body, false, stmt);
                    }
                }
                Some("multipart") => {
                    for c in o.children.iter().filter(|c| c.key == "file") {
                        if let Some(e) = c.args.get(1).map(Arg::to_expr) {
                            self.file_option_site(
                                c,
                                &e,
                                "body multipart file",
                                Phase::Body,
                                false,
                                stmt,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn descriptor_sites(&mut self, options: &[OptionLine], stmt: &SourceSpan) {
        for o in options.iter().filter(|o| o.key == "descriptor") {
            if let Some(e) = o.args.first().map(Arg::to_expr) {
                self.file_option_site(o, &e, "descriptor", Phase::Load, false, stmt);
            }
        }
    }

    /// `auth PROFILE account "A"` on an HTTP block: every effect a token
    /// acquisition or refresh may need (listed even when a cached token is reused).
    fn auth_sites(&mut self, o: &OptionLine, stmt: &SourceSpan) {
        let Some(profile_name) = o.first_word().map(str::to_string) else {
            return;
        };
        let account = o
            .args
            .iter()
            .position(|a| a.word() == Some("account"))
            .and_then(|i| o.args.get(i + 1))
            .and_then(|a| a.to_expr().const_text().or(a.word().map(str::to_string)))
            .unwrap_or_else(|| "default".into());
        let Some(profile) = self.program.auth_profile(&profile_name).cloned() else {
            return;
        };
        let logical = format!("{profile_name}/{account}");
        let token_origin = profile
            .option("token_url")
            .and_then(|t| t.args.first())
            .and_then(|a| a.to_expr().const_text())
            .map(|u| {
                let (t, _, _) = self.url_target(&Expr::Lit(Value::Text(u)), "https", &[]);
                t.group_key(SiteKind::Network)
            });
        if let Some(cs) = profile.option("client_secret")
            && cs.first_word() == Some("env")
            && let Some(var) = cs.args.get(1).and_then(|a| a.to_expr().const_text())
        {
            let mut s = self.option_site(SiteKind::Env, AccessVerb::Read, cs, stmt);
            s.target = SiteTarget {
                template: var,
                ..SiteTarget::default()
            };
            s.origin = SiteOrigin::Option("client_secret env".into());
            s.secret = true;
            s.bound_to = token_origin.clone().into_iter().collect();
            self.push(s);
        }
        for (kind, verb) in [
            (SiteKind::Credential, AccessVerb::Read),
            (SiteKind::Credential, AccessVerb::Write),
            (SiteKind::Auth, AccessVerb::Use),
        ] {
            let mut s = self.option_site(kind, verb, o, stmt);
            s.target = SiteTarget {
                template: logical.clone(),
                ..SiteTarget::default()
            };
            s.origin = SiteOrigin::Option("auth".into());
            self.push(s);
        }
        if let Some(t) = profile.option("token_url")
            && let Some(e) = t.args.first().map(Arg::to_expr)
        {
            let (target, k, x) = self.url_target(&e, "https", &[]);
            let mut s = self.option_site(SiteKind::Network, AccessVerb::Connect, t, stmt);
            s.method = Some("POST".into());
            s.protocol = Some("http1|http2".into());
            s.target = target;
            s.knowledge = k;
            s.expression = x;
            s.origin = SiteOrigin::Option("token_url".into());
            s.phase = Phase::Connect;
            s.note = Some("token endpoint".into());
            self.push(s);
        }
    }

    fn form_exprs(form: &EffectForm) -> Vec<Expr> {
        let mut v: Vec<Expr> = form.head.iter().map(Arg::to_expr).collect();
        for o in &form.options {
            v.extend(o.args.iter().map(Arg::to_expr));
        }
        v
    }

    fn form(&mut self, form: &EffectForm, span: &SourceSpan, scoped: bool, body: &[Stmt]) {
        for e in Self::form_exprs(form) {
            self.expr(&e);
        }
        let secrets = self.secrets_in(&Self::form_exprs(form));
        let before = self.sites.len();
        let with = if scoped { "with " } else { "" };
        match &form.kind {
            EffectKind::File => self.file_form(form, span, scoped),
            EffectKind::Http => {
                let method = form
                    .head
                    .first()
                    .and_then(Arg::word)
                    .unwrap_or("get")
                    .to_string();
                if let Some(a) = form.option("auth") {
                    self.auth_sites(a, span);
                }
                let query: Vec<(String, String, Vec<String>)> = form
                    .options_named("query")
                    .filter_map(|o| {
                        let k = o.args.first().and_then(|a| {
                            a.word().map(str::to_string).or(a.to_expr().const_text())
                        })?;
                        let v = o.args.get(1)?.to_expr();
                        if let Expr::Lit(l) = &v {
                            return Some((k, l.to_display(), Vec::new()));
                        }
                        let (t, ps, _, x) = self.template_of(&v);
                        let text = match x {
                            Some(x) => format!("{{{x}}}"),
                            None => t,
                        };
                        Some((k, text, ps))
                    })
                    .collect();
                if let Some(u) = form.option("unix") {
                    if let Some(e) = u.args.first().map(Arg::to_expr) {
                        let (t, k, x) = self.path_target(&e);
                        let mut s = self.base(SiteKind::Unix, vec![AccessVerb::Connect], span);
                        s.target = t;
                        s.knowledge = k;
                        s.expression = x;
                        s.method = Some(method.to_uppercase());
                        s.origin = SiteOrigin::Statement(format!("{with}http {method}"));
                        s.phase = Phase::Connect;
                        self.push(s);
                    }
                } else if let Some(e) = form.head.get(1).map(Arg::to_expr) {
                    let (t, k, x) = self.url_target(&e, "https", &query);
                    let mut s = self.base(SiteKind::Network, vec![AccessVerb::Connect], span);
                    s.method = Some(method.to_uppercase());
                    s.protocol = Some(http_protocol(form));
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Statement(format!("{with}http {method}"));
                    s.phase = Phase::Connect;
                    self.push(s);
                }
                self.tls_sites(&form.options, span);
                self.body_file_sites(&form.options, span);
            }
            EffectKind::WebSocket => {
                self.net_site(form, span, "wss", "with websocket", Some("GET"), "ws");
                self.tls_sites(&form.options, span);
            }
            EffectKind::Tcp => {
                self.net_site(form, span, "tcp", &format!("{with}tcp"), None, "tcp");
                self.tls_sites(&form.options, span);
            }
            EffectKind::Quic => {
                self.net_site(form, span, "quic", &format!("{with}quic"), None, "quic");
                self.tls_sites(&form.options, span);
            }
            EffectKind::Udp => self.udp_form(form, span, with),
            EffectKind::Unix => {
                if let Some(e) = form.head.first().map(Arg::to_expr) {
                    let (t, k, x) = self.path_target(&e);
                    let mut s = self.base(SiteKind::Unix, vec![AccessVerb::Connect], span);
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Statement(format!("{with}unix"));
                    s.phase = Phase::Connect;
                    self.push(s);
                }
            }
            EffectKind::Pipe => {
                let write = form
                    .head
                    .iter()
                    .position(|a| a.word() == Some("mode"))
                    .and_then(|i| form.head.get(i + 1))
                    .and_then(Arg::word)
                    == Some("write");
                if let Some(e) = form.head.first().map(Arg::to_expr) {
                    let (t, k, x) = self.path_target(&e);
                    let verb = if write {
                        AccessVerb::Write
                    } else {
                        AccessVerb::Read
                    };
                    let mut s = self.base(SiteKind::Pipe, vec![verb], span);
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Statement(format!("{with}pipe"));
                    s.phase = Phase::Connect;
                    self.push(s);
                }
            }
            EffectKind::Command => {
                if let Some(e) = form.head.first().map(Arg::to_expr) {
                    let (t, k, x) = self.path_target(&e);
                    let mut s = self.base(SiteKind::Process, vec![AccessVerb::Exec], span);
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Statement(format!("{with}command"));
                    s.phase = Phase::Connect;
                    self.push(s);
                }
            }
            EffectKind::Grpc => {
                let callee = form.head.first().map(|a| match a {
                    Arg::Word(w, _) => w.clone(),
                    Arg::Expr(e, _) => expr_text(e),
                });
                if let Some((conn_name, method)) = callee.as_deref().and_then(|c| c.split_once('.'))
                    && let Some(conn) = self.program.connector(conn_name).cloned()
                {
                    let mode = if !scoped {
                        "unary"
                    } else {
                        grpc_mode(body, self.bind.as_deref().unwrap_or(""))
                    };
                    let origin = if scoped { "with grpc" } else { "grpc" };
                    self.grpc_sites(&conn, method, mode, origin, span);
                }
            }
            EffectKind::RequestStream | EffectKind::Connection | EffectKind::Other(_) => {}
        }
        self.descriptor_sites(&form.options, span);
        for s in &mut self.sites[before..] {
            for n in &secrets {
                if !s.secrets.contains(n) {
                    s.secrets.push(n.clone());
                }
            }
        }
    }

    fn net_site(
        &mut self,
        form: &EffectForm,
        span: &SourceSpan,
        scheme: &str,
        origin: &str,
        method: Option<&str>,
        protocol: &str,
    ) {
        if let Some(e) = form.head.first().map(Arg::to_expr) {
            let (t, k, x) = self.url_target(&e, scheme, &[]);
            let mut s = self.base(SiteKind::Network, vec![AccessVerb::Connect], span);
            s.method = method.map(str::to_string);
            s.protocol = Some(protocol.into());
            s.target = t;
            s.knowledge = k;
            s.expression = x;
            s.origin = SiteOrigin::Statement(origin.into());
            s.phase = Phase::Connect;
            self.push(s);
        }
    }

    fn udp_form(&mut self, form: &EffectForm, span: &SourceSpan, with: &str) {
        let first = form.head.first().and_then(Arg::word);
        match first {
            Some("bind") => {
                if let Some(e) = form.head.get(1).map(Arg::to_expr) {
                    let (t, k, x) = self.url_target(&e, "udp", &[]);
                    let mut s = self.base(SiteKind::Network, vec![AccessVerb::Bind], span);
                    s.protocol = Some("udp".into());
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Statement(format!("{with}udp bind"));
                    s.phase = Phase::Connect;
                    self.push(s);
                }
            }
            Some("multicast") => {
                // Mirror exactly what the runtime authorizes when the socket opens
                // (datagrams.exchange_datagrams authorize_open): allow_network connect on the
                // group, then allow_listen bind + multicast_join on the bind address — the
                // `bind` option, or the unspecified address on the group's port by default.
                let mut group_port = None;
                let mut group_v6 = false;
                let mut group_knowledge = Knowledge::Exact;
                if let Some(e) = form.head.get(1).map(Arg::to_expr) {
                    let (t, k, x) = self.url_target(&e, "udp", &[]);
                    group_port = t.port;
                    group_v6 = t.host.as_deref().is_some_and(|h| h.contains(':'));
                    group_knowledge = k;
                    let mut s = self.base(SiteKind::Network, vec![AccessVerb::Connect], span);
                    s.protocol = Some("udp".into());
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Statement(format!("{with}udp multicast"));
                    s.phase = Phase::Connect;
                    self.push(s);
                }
                let listen = vec![AccessVerb::Bind, AccessVerb::MulticastJoin];
                if let Some(b) = form.option("bind")
                    && let Some(e) = b.args.first().map(Arg::to_expr)
                {
                    let (t, k, x) = self.url_target(&e, "udp", &[]);
                    let mut s = self.option_site(SiteKind::Network, AccessVerb::Bind, b, span);
                    s.access = listen;
                    s.protocol = Some("udp".into());
                    s.target = t;
                    s.knowledge = k;
                    s.expression = x;
                    s.origin = SiteOrigin::Option("bind".into());
                    s.phase = Phase::Connect;
                    self.push(s);
                } else {
                    let mut s = self.base(SiteKind::Network, listen, span);
                    s.protocol = Some("udp".into());
                    s.origin = SiteOrigin::Statement(format!("{with}udp multicast (default bind)"));
                    s.phase = Phase::Connect;
                    match group_port {
                        Some(port) if group_knowledge == Knowledge::Exact => {
                            let host = if group_v6 { "[::]" } else { "0.0.0.0" };
                            s.target = SiteTarget {
                                template: format!("udp://{host}:{port}"),
                                scheme: Some("udp".into()),
                                host: Some(host.into()),
                                port: Some(port),
                                path: Some("/".into()),
                                ..SiteTarget::default()
                            };
                        }
                        _ => {
                            s.knowledge = Knowledge::Dynamic;
                            s.expression = Some("unspecified address on the group port".into());
                        }
                    }
                    self.push(s);
                }
            }
            _ => self.net_site(form, span, "udp", &format!("{with}udp"), None, "udp"),
        }
    }

    fn file_form(&mut self, form: &EffectForm, span: &SourceSpan, scoped: bool) {
        let verb = form
            .head
            .first()
            .and_then(Arg::word)
            .unwrap_or("")
            .to_string();
        let Some(path) = form.head.get(1).map(Arg::to_expr) else {
            return;
        };
        let missing_ok = form
            .option("missing")
            .and_then(OptionLine::first_word)
            .is_some_and(|w| w == "ok");
        let to = form
            .head
            .iter()
            .position(|a| a.word() == Some("to"))
            .and_then(|i| form.head.get(i + 1))
            .map(Arg::to_expr);
        let overwrite = form.option("overwrite").is_some();
        let origin = if scoped {
            format!("with file {verb}")
        } else {
            format!("file {verb}")
        };
        use AccessVerb as V;
        // (path expr, verbs, requires_existing)
        let mut plan: Vec<(Expr, Vec<V>, bool)> = Vec::new();
        match verb.as_str() {
            "read" => plan.push((path, vec![V::Read], true)),
            "list" => plan.push((path, vec![V::List], true)),
            "stat" => plan.push((path, vec![V::Stat], true)),
            "watch" => plan.push((path, vec![V::Watch], true)),
            "create" => plan.push((path, vec![V::Create], false)),
            "append" => plan.push((path, vec![V::Append], false)),
            "write" => plan.push((path, vec![V::Create, V::Update], false)),
            "update" => {
                plan.push((path.clone(), vec![V::Stat], true));
                plan.push((path, vec![V::Update], true));
            }
            "delete" => plan.push((path, vec![V::Delete], !missing_ok)),
            "open" => {
                let mode = form
                    .head
                    .iter()
                    .position(|a| a.word() == Some("mode"))
                    .and_then(|i| form.head.get(i + 1))
                    .and_then(Arg::word)
                    .unwrap_or("read");
                let (verbs, needs) = match mode {
                    "write" => (vec![V::Create, V::Update], false),
                    "append" => (vec![V::Append], false),
                    _ => (vec![V::Read], true),
                };
                plan.push((path, verbs, needs));
            }
            "copy" | "move" => {
                plan.push((path.clone(), vec![V::Read], true));
                if let Some(dst) = to {
                    let mut vs = vec![V::Create];
                    if overwrite {
                        vs.push(V::Update);
                    }
                    plan.push((dst, vs, false));
                }
                if verb == "move" {
                    plan.push((path, vec![V::Delete], true));
                }
            }
            _ => {}
        }
        for (e, verbs, needs) in plan {
            let (t, k, x) = self.path_target(&e);
            let mut s = self.base(SiteKind::File, verbs, span);
            s.target = t;
            s.knowledge = k;
            s.expression = x;
            s.origin = SiteOrigin::Statement(origin.clone());
            s.phase = Phase::Body;
            s.requires_existing = needs;
            self.push(s);
        }
    }

    fn member(&mut self, call: &MemberCall, span: &SourceSpan) {
        for a in &call.args {
            self.expr(&a.to_expr());
        }
        let handle = call.object.first().cloned().unwrap_or_default();
        if self.handles.get(&handle) == Some(&EffectKind::Udp) && call.method == "send_to" {
            let peer = call.args.first().map(Arg::to_expr);
            let text = peer.as_ref().map(expr_text).unwrap_or_default();
            let mut s = self.base(SiteKind::Network, vec![AccessVerb::Connect], span);
            s.protocol = Some("udp".into());
            s.target = SiteTarget {
                template: format!("udp://{{{text}}}"),
                scheme: Some("udp".into()),
                ..SiteTarget::default()
            };
            s.expression = Some(text);
            s.knowledge = Knowledge::Dynamic;
            s.origin = SiteOrigin::Statement(format!("{handle}.send_to"));
            s.note = Some("dynamic: sender of the received datagram".into());
            self.push(s);
        }
    }
}

fn http_protocol(form: &EffectForm) -> String {
    let arg_text = |a: &Arg| match a {
        Arg::Word(w, _) => w.clone(),
        Arg::Expr(e, _) => expr_text(e).trim_matches('"').to_string(),
    };
    let version = form.option("version");
    let first = version.and_then(|o| o.args.first()).map(arg_text);
    if first.as_deref() == Some("prefer") {
        // `version prefer [3, 2]`: every listed transport is inventoried.
        let list = version
            .and_then(|o| o.args.get(1))
            .map(arg_text)
            .unwrap_or_default();
        let names: Vec<&str> = list
            .trim_matches(['[', ']'])
            .split(',')
            .filter_map(|v| match v.trim() {
                "3" => Some("http3"),
                "2" => Some("http2"),
                "1" | "1.1" => Some("http1"),
                _ => None,
            })
            .collect();
        if !names.is_empty() {
            return names.join("|");
        }
    }
    match first.as_deref() {
        Some("3") => "http3".into(),
        Some("2") => "http2".into(),
        Some("1") | Some("1.1") => "http1".into(),
        _ => "http1|http2".into(),
    }
}

/// gRPC call mode of a scoped call: does the body send on the handle, iterate it, or both?
fn grpc_mode(body: &[Stmt], handle: &str) -> &'static str {
    fn scan(body: &[Stmt], h: &str, sends: &mut bool, iterates: &mut bool) {
        for s in body {
            match s {
                Stmt::Member { call, .. }
                    if call.method == "send"
                        && call.object.first().map(String::as_str) == Some(h) =>
                {
                    *sends = true
                }
                Stmt::For { iter, body, .. } => {
                    if matches!(iter, Expr::Path(p, _) if p.len() == 1 && p[0] == h) {
                        *iterates = true;
                    }
                    scan(body, h, sends, iterates)
                }
                Stmt::If {
                    then, otherwise, ..
                } => {
                    scan(then, h, sends, iterates);
                    scan(otherwise, h, sends, iterates)
                }
                Stmt::While { body, .. }
                | Stmt::Scope { body, .. }
                | Stmt::Iterate { body, .. }
                | Stmt::Try { body, .. }
                | Stmt::With { body, .. } => scan(body, h, sends, iterates),
                Stmt::Concurrent { tasks, .. } => {
                    for (_, b) in tasks {
                        scan(b, h, sends, iterates)
                    }
                }
                _ => {}
            }
        }
    }
    let (mut sends, mut iterates) = (false, false);
    scan(body, handle, &mut sends, &mut iterates);
    match (sends, iterates) {
        (true, true) => "bidi",
        (true, false) => "client_stream",
        (false, true) => "server_stream",
        (false, false) => "unary",
    }
}

/// `./out/notes/{name}.json` → `./out/notes/*.json`.
pub fn glob_of(template: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in template.chars() {
        match c {
            '{' => {
                if depth == 0 {
                    out.push('*');
                }
                depth += 1;
            }
            '}' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // vhco:test audit.inspect_effects -- normalization helpers: derived globs, lexical paths and bundle-relative source files
    #[test]
    fn globs_paths_and_files() {
        assert_eq!(glob_of("./out/notes/{name}.json"), "./out/notes/*.json");
        assert_eq!(glob_of("./a/{x}/{y}.bin"), "./a/*/*.bin");
        assert_eq!(norm_rel("./out/../out/./a.json"), "out/a.json");
        assert_eq!(
            rel_file("docs/demos/x/app.rivet", "docs/demos/x"),
            "app.rivet"
        );
        assert_eq!(rel_file("./app.rivet", "."), "app.rivet");
    }
}
