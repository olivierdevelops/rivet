//! Lowering: Rivet `SyntaxTree` → typed IR (`CompiledProgram`).
//!
//! Enforces the header order and the leading-options rule, pairs `try` with
//! its `catch`, recognizes effect forms (`http`, `file`, `grpc`, `command`, …),
//! and reports every problem with its exact source span.

use super::expr::{SpanMap, parse_args, parse_expr};
use crate::domain::ir::{
    Arg, CatchFilter, DagNode, Declaration, EffectForm, EffectKind, Expr, FailurePolicy,
    GroupOptions, MemberCall, Operation, OperationKind, OptionLine, Rhs, Stmt, parse_duration_ms,
};
use crate::domain::outputs::{DeclaredError, FieldSpec, OutputSpec, ParamSpec, ValueSpec};
use crate::domain::source::SourceSpan;
use crate::domain::syntax_tree::{SyntaxNode, SyntaxTree};
use crate::domain::{RivetError, Value};

/// Collects every diagnostic instead of stopping at the first.
#[derive(Default)]
pub struct Lowerer {
    pub errors: Vec<RivetError>,
    pub warnings: Vec<RivetError>,
}

/// Effect keywords that may start an assignment value or a `with` resource.
const EFFECT_WORDS: &[&str] = &[
    "http",
    "file",
    "grpc",
    "command",
    "tcp",
    "unix",
    "pipe",
    "udp",
    "quic",
    "websocket",
];

/// Header statements in their required order.
fn header_rank(func: &str) -> Option<u8> {
    Some(match func {
        "name" => 0,
        "description" => 1,
        "private" => 2,
        "param" => 3,
        "output" | "output_block" => 4,
        "emits" | "emits_block" => 5,
        "receives" | "receives_block" => 6,
        "declared_error" => 7,
        _ => return None,
    })
}

fn is_option(func: &str) -> bool {
    func.starts_with("opt_") || func == "auth_use"
}

fn option_key(func: &str) -> String {
    if func == "auth_use" {
        "auth".into()
    } else {
        func.trim_start_matches("opt_")
            .trim_end_matches("_block")
            .to_string()
    }
}

fn map_for(node: &SyntaxNode, capture: &str) -> SpanMap {
    SpanMap::new(node.capture_span(capture), node.text(capture))
}

impl Lowerer {
    fn fail(&mut self, e: RivetError) {
        self.errors.push(e);
    }

    fn syntax(&mut self, code: &str, msg: impl Into<String>, span: &SourceSpan) {
        self.errors
            .push(RivetError::syntax(code, msg, Some(span.clone())));
    }

    fn expr(&mut self, node: &SyntaxNode, capture: &str) -> Option<Expr> {
        let text = node.text(capture);
        match parse_expr(text, &map_for(node, capture)) {
            Ok(e) => Some(e),
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    fn args(&mut self, node: &SyntaxNode, capture: &str) -> Vec<Arg> {
        let text = node.text(capture);
        if text.trim().is_empty() {
            return Vec::new();
        }
        match parse_args(text, &map_for(node, capture)) {
            Ok(a) => a,
            Err(e) => {
                self.fail(e);
                Vec::new()
            }
        }
    }

    /// Lower every top-level declaration of one parsed file.
    pub fn lower_tree(
        &mut self,
        tree: &SyntaxTree,
        ops: &mut Vec<Operation>,
        connectors: &mut Vec<Declaration>,
        auth: &mut Vec<Declaration>,
    ) {
        for d in &tree.diagnostics {
            let mut e = RivetError::syntax(&d.code, d.message.clone(), Some(d.span.clone()));
            if let Some(h) = &d.help {
                e = e.with_hint(h.clone());
            }
            self.fail(e);
        }
        if !tree.diagnostics.is_empty() {
            return;
        }
        for node in &tree.nodes {
            match node.func.as_str() {
                "operation" | "pipeline" => {
                    if let Some(op) = self.lower_operation(node, &tree.file) {
                        ops.push(op);
                    }
                }
                "connector" => connectors.push(self.lower_declaration(node, "connector")),
                "auth_profile" => auth.push(self.lower_declaration(node, "auth")),
                other => self.syntax(
                    "syntax.top_level",
                    format!(
                        "`{}` must appear inside an operation, pipeline, connector or auth profile",
                        display_func(other)
                    ),
                    &node.span,
                ),
            }
        }
    }

    fn lower_declaration(&mut self, node: &SyntaxNode, what: &str) -> Declaration {
        let mut options = Vec::new();
        for child in node.children() {
            if is_option(&child.func) {
                options.push(self.option_line(child));
            } else {
                self.syntax(
                    "syntax.option_expected",
                    format!(
                        "{what} blocks contain option lines only; found `{}`",
                        display_func(&child.func)
                    ),
                    &child.span,
                );
            }
        }
        Declaration {
            name: node.text("name").to_string(),
            kind: node.text("kind").to_string(),
            options,
            span: node.span.clone(),
        }
    }

    fn option_line(&mut self, node: &SyntaxNode) -> OptionLine {
        let args = self.args(node, "rest");
        let mut children = Vec::new();
        for c in node.children() {
            if is_option(&c.func) {
                children.push(self.option_line(c));
            } else if matches!(
                c.func.as_str(),
                "field" | "field_block" | "file" | "file_block"
            ) {
                // multipart parts: `field NAME VALUE`, `file NAME PATH [TYPE]`
                let mut args = Vec::new();
                if c.func.starts_with("field") {
                    args.push(Arg::Word(
                        c.text("name").to_string(),
                        c.capture_span("name"),
                    ));
                }
                args.extend(self.args(c, "rest"));
                let key = if c.func.starts_with("field") {
                    "field"
                } else {
                    "file"
                };
                children.push(OptionLine {
                    key: key.into(),
                    args,
                    span: c.span.clone(),
                    children: Vec::new(),
                });
            } else {
                self.syntax(
                    "syntax.option_expected",
                    format!(
                        "`{}` is not valid inside an option block",
                        display_func(&c.func)
                    ),
                    &c.span,
                );
            }
        }
        OptionLine {
            key: option_key(&node.func),
            args,
            span: node.span.clone(),
            children,
        }
    }

    fn lower_operation(&mut self, node: &SyntaxNode, file: &str) -> Option<Operation> {
        let id = node.text("id").to_string();
        if !valid_id(&id) {
            self.syntax(
                "syntax.operation_id",
                format!("`{id}` is not a valid operation ID (dotted snake_case segments, 1–128 ASCII characters)"),
                &node.capture_span("id"),
            );
        }
        if id.starts_with("rivet.") {
            self.syntax(
                "syntax.reserved_id",
                "`rivet.*` IDs are reserved for built-in operations",
                &node.capture_span("id"),
            );
        }
        let mut op = Operation {
            id: id.clone(),
            kind: if node.func == "pipeline" {
                OperationKind::Pipeline
            } else {
                OperationKind::Operation
            },
            name: id.clone(),
            description: None,
            private: false,
            params: Vec::new(),
            output: OutputSpec::default(),
            emits: None,
            receives: None,
            errors: Vec::new(),
            body: Vec::new(),
            span: node.span.clone(),
            file: file.to_string(),
            calls: Vec::new(),
        };
        let mut last_rank = 0u8;
        let mut body_started = false;
        let mut body_nodes: Vec<&SyntaxNode> = Vec::new();
        let mut output_seen = false;
        for child in node.children() {
            if let Some(rank) = header_rank(&child.func) {
                if body_started {
                    self.syntax(
                        "syntax.option_after_body",
                        format!(
                            "`{}` is a header line and must come before the first body statement",
                            display_func(&child.func)
                        ),
                        &child.span,
                    );
                    continue;
                }
                if rank < last_rank {
                    self.syntax(
                        "syntax.header_order",
                        format!(
                            "`{}` is out of order; header order is name, description, private, param, output, emits, receives, error",
                            display_func(&child.func)
                        ),
                        &child.span,
                    );
                }
                last_rank = rank;
                self.header_line(child, &mut op, &mut output_seen);
            } else {
                body_started = true;
                body_nodes.push(child);
            }
        }
        if !output_seen {
            self.syntax(
                "syntax.output_required",
                format!("operation `{id}` must declare its `output`"),
                &node.span,
            );
        }
        op.body = self.lower_block(&body_nodes, BlockCtx::Operation);
        let mut calls = Vec::new();
        collect_calls(&op.body, &mut calls);
        calls.sort();
        calls.dedup();
        op.calls = calls;
        Some(op)
    }

    fn header_line(&mut self, child: &SyntaxNode, op: &mut Operation, output_seen: &mut bool) {
        match child.func.as_str() {
            "name" => op.name = unquote(child.text("value")),
            "description" => op.description = Some(unquote(child.text("value"))),
            "private" => op.private = child.text("value") == "true",
            "param" => {
                if let Some(p) = self.param(child) {
                    if op.params.iter().any(|q| q.name == p.name) {
                        self.syntax(
                            "syntax.duplicate_param",
                            format!("parameter `{}` is declared twice", p.name),
                            &child.span,
                        );
                    }
                    op.params.push(p);
                }
            }
            "output" | "output_block" => {
                if *output_seen {
                    self.syntax(
                        "syntax.duplicate_output",
                        "an operation declares exactly one `output`",
                        &child.span,
                    );
                }
                *output_seen = true;
                if let Some((spec, desc)) = self.typed_decl(child) {
                    op.output = OutputSpec {
                        spec,
                        description: desc,
                    };
                }
            }
            "emits" | "emits_block" => op.emits = self.typed_decl(child).map(|(s, _)| s),
            "receives" | "receives_block" => op.receives = self.typed_decl(child).map(|(s, _)| s),
            "declared_error" => {
                let code = unquote(child.text("code"));
                let args = self.args(child, "rest");
                let description = keyword_text(&args, "description");
                if op.errors.iter().any(|e| e.code == code) {
                    self.syntax(
                        "syntax.duplicate_error",
                        format!("error `{code}` is declared twice"),
                        &child.span,
                    );
                }
                op.errors.push(DeclaredError { code, description });
            }
            _ => {}
        }
    }

    fn param(&mut self, node: &SyntaxNode) -> Option<ParamSpec> {
        let name = node.text("name").to_string();
        let args = self.args(node, "rest");
        let mut it = args.iter().peekable();
        let spec = self.type_from_args(&mut it, &node.span)?;
        let mut p = ParamSpec {
            name,
            spec,
            required: false,
            default: None,
            min: None,
            max: None,
            enum_values: None,
            description: None,
        };
        let mut explicit_required = false;
        while let Some(a) = it.next() {
            match a.word() {
                Some("required") => {
                    p.required = true;
                    explicit_required = true;
                }
                Some("optional") => p.required = false,
                Some("default") => p.default = it.next().and_then(|v| const_value(&v.to_expr())),
                Some("min") => p.min = it.next().and_then(|v| const_number(&v.to_expr())),
                Some("max") => p.max = it.next().and_then(|v| const_number(&v.to_expr())),
                Some("enum") => {
                    p.enum_values = it.next().and_then(|v| match const_value(&v.to_expr()) {
                        Some(Value::List(items)) => Some(items),
                        _ => None,
                    })
                }
                Some("description") => {
                    p.description = it.next().and_then(|v| v.to_expr().const_text())
                }
                _ => self.syntax(
                    "syntax.param_modifier",
                    "unknown parameter modifier",
                    a.span(),
                ),
            }
        }
        if explicit_required && p.default.is_some() {
            self.syntax(
                "syntax.param_modifier",
                "a parameter cannot be both `required` and have a `default`",
                &node.span,
            );
        }
        if !explicit_required && p.default.is_none() {
            // Neither required nor default: optional, value null when absent.
            p.required = false;
        }
        Some(p)
    }

    fn type_from_args<'a, I: Iterator<Item = &'a Arg>>(
        &mut self,
        it: &mut std::iter::Peekable<I>,
        span: &SourceSpan,
    ) -> Option<ValueSpec> {
        let Some(first) = it.next() else {
            self.syntax(
                "syntax.type",
                "expected a type (text, integer, number, boolean, bytes, json, object, list T)",
                span,
            );
            return None;
        };
        match first.word() {
            Some("list") => {
                let inner = self.type_from_args(it, span)?;
                Some(ValueSpec::List(Box::new(inner)))
            }
            Some("object") => Some(ValueSpec::Object {
                fields: Vec::new(),
                open: false,
            }),
            Some(w) => match ValueSpec::parse_scalar(w) {
                Some(s) => Some(s),
                None => {
                    self.syntax("syntax.type", format!("unknown type `{w}`"), first.span());
                    None
                }
            },
            None => {
                self.syntax("syntax.type", "expected a type name", first.span());
                None
            }
        }
    }

    /// `output|emits|receives TYPE [description "…"]` (+ field block for objects).
    pub fn typed_decl(&mut self, node: &SyntaxNode) -> Option<(ValueSpec, Option<String>)> {
        let args = self.args(node, "rest");
        let mut it = args.iter().peekable();
        let mut spec = self.type_from_args(&mut it, &node.span)?;
        let rest: Vec<Arg> = it.cloned().collect();
        let desc = keyword_text(&rest, "description");
        if let ValueSpec::Object { fields, open } = &mut spec {
            let (f, o) = self.fields(node.children());
            *fields = f;
            *open = o;
        } else if !node.children().is_empty() {
            self.syntax(
                "syntax.fields",
                "only `object` outputs take a field block",
                &node.span,
            );
        }
        Some((spec, desc))
    }

    fn fields(&mut self, children: &[SyntaxNode]) -> (Vec<FieldSpec>, bool) {
        let mut out = Vec::new();
        let mut open = false;
        for c in children {
            match c.func.as_str() {
                "field" | "field_block" => {
                    let name = c.text("name").to_string();
                    let args = self.args(c, "rest");
                    let mut it = args.iter().peekable();
                    let Some(mut spec) = self.type_from_args(&mut it, &c.span) else {
                        continue;
                    };
                    let mut required = true;
                    let mut description = None;
                    while let Some(a) = it.next() {
                        match a.word() {
                            Some("required") => required = true,
                            Some("optional") => required = false,
                            Some("description") => {
                                description = it.next().and_then(|v| v.to_expr().const_text())
                            }
                            _ => self.syntax(
                                "syntax.field_modifier",
                                "unknown field modifier",
                                a.span(),
                            ),
                        }
                    }
                    if let ValueSpec::Object { fields, open } = &mut spec {
                        let (f, o) = self.fields(c.children());
                        *fields = f;
                        *open = o;
                    }
                    if out.iter().any(|f: &FieldSpec| f.name == name) {
                        self.syntax(
                            "syntax.duplicate_field",
                            format!("field `{name}` is declared twice"),
                            &c.span,
                        );
                    }
                    out.push(FieldSpec {
                        name,
                        spec,
                        required,
                        description,
                    });
                }
                "opt_open" => open = c.text("rest").trim() == "true",
                other => self.syntax(
                    "syntax.fields",
                    format!("`{}` is not allowed in a field block", display_func(other)),
                    &c.span,
                ),
            }
        }
        (out, open)
    }

    fn lower_block(&mut self, nodes: &[&SyntaxNode], ctx: BlockCtx) -> Vec<Stmt> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < nodes.len() {
            let n = nodes[i];
            if n.func == "try" {
                let next = nodes.get(i + 1).filter(|c| c.func == "catch");
                match next {
                    Some(c) => {
                        let body = self.lower_children(n, BlockCtx::Body);
                        let handler = self.lower_children(c, BlockCtx::Body);
                        let filter = self.catch_filter(c);
                        out.push(Stmt::Try {
                            body,
                            filter,
                            handler,
                            span: n.span.clone(),
                        });
                        i += 2;
                        continue;
                    }
                    None => {
                        self.syntax("syntax.try_without_catch", "`try` must be followed by `catch error kind K` or `catch error code \"C\"`", &n.span);
                        i += 1;
                        continue;
                    }
                }
            }
            if n.func == "catch" {
                self.syntax(
                    "syntax.catch_without_try",
                    "`catch` must directly follow a `try` block",
                    &n.span,
                );
                i += 1;
                continue;
            }
            if let Some(s) = self.lower_stmt(n, ctx) {
                out.push(s);
            }
            i += 1;
        }
        out
    }

    fn lower_children(&mut self, node: &SyntaxNode, ctx: BlockCtx) -> Vec<Stmt> {
        let kids: Vec<&SyntaxNode> = node.children().iter().collect();
        self.lower_block(&kids, ctx)
    }

    fn catch_filter(&mut self, node: &SyntaxNode) -> CatchFilter {
        let args = self.args(node, "filter");
        let words: Vec<Option<&str>> = args.iter().map(Arg::word).collect();
        match (
            words.first().copied().flatten(),
            words.get(1).copied().flatten(),
        ) {
            (Some("error"), Some("kind")) => CatchFilter {
                kind: args.get(2).and_then(|a| a.word()).map(str::to_string),
                code: None,
            },
            (Some("error"), Some("code")) => CatchFilter {
                kind: None,
                code: args.get(2).and_then(|a| a.to_expr().const_text()),
            },
            (Some("error"), None) => CatchFilter::default(),
            _ => {
                self.syntax(
                    "syntax.catch",
                    "expected `catch error`, `catch error kind K` or `catch error code \"C\"`",
                    &node.span,
                );
                CatchFilter::default()
            }
        }
    }

    /// Split a block's children into leading option lines and body statements.
    fn options_then_body<'a>(
        &mut self,
        node: &'a SyntaxNode,
    ) -> (Vec<OptionLine>, Vec<&'a SyntaxNode>) {
        let mut options = Vec::new();
        let mut body = Vec::new();
        for c in node.children() {
            if is_option(&c.func) && c.func != "opt_until" {
                if !body.is_empty() {
                    self.syntax(
                        "syntax.option_after_body",
                        format!(
                            "option `{}` must come before the first body statement of this block",
                            option_key(&c.func)
                        ),
                        &c.span,
                    );
                    continue;
                }
                options.push(self.option_line(c));
            } else {
                body.push(c);
            }
        }
        (options, body)
    }

    fn group_options(&mut self, node: &SyntaxNode, capture: &str) -> GroupOptions {
        let args = self.args(node, capture);
        let mut o = GroupOptions::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.word() {
                Some("limit") => {
                    o.limit = it
                        .next()
                        .and_then(|v| const_number(&v.to_expr()))
                        .map(|n| n as i64)
                }
                Some("timeout") => o.timeout_ms = it.next().and_then(|v| self.duration(v)),
                Some("fail") => match it.next().and_then(Arg::word) {
                    Some("fast") => o.failure = FailurePolicy::Fast,
                    Some("independent") => o.failure = FailurePolicy::Independent,
                    _ => self.syntax(
                        "syntax.group_option",
                        "expected `fail fast` or `fail independent`",
                        a.span(),
                    ),
                },
                _ => self.syntax(
                    "syntax.group_option",
                    "expected `limit N`, `timeout \"D\"` or `fail fast|independent`",
                    a.span(),
                ),
            }
        }
        o
    }

    fn duration(&mut self, a: &Arg) -> Option<u64> {
        match a.to_expr().const_text() {
            Some(t) => match parse_duration_ms(&t) {
                Some(ms) => Some(ms),
                None => {
                    self.syntax(
                        "syntax.duration",
                        format!("`\"{t}\"` is not a duration; use digits plus ms, s, m or h"),
                        a.span(),
                    );
                    None
                }
            },
            None => {
                self.syntax(
                    "syntax.duration",
                    "durations are quoted strings such as \"5s\"",
                    a.span(),
                );
                None
            }
        }
    }

    /// Value side of `x = …`, `return …`, `emit …`, `yield …`.
    fn rhs(
        &mut self,
        node: &SyntaxNode,
        options: Vec<OptionLine>,
    ) -> Option<(Rhs, Vec<OptionLine>)> {
        let value_text = node.text("value").trim().to_string();
        let rest_text = node.text("rest").trim().to_string();
        if EFFECT_WORDS.contains(&value_text.as_str()) {
            let head = self.args(node, "rest");
            let head = if value_text == "grpc" {
                grpc_head(head)
            } else {
                head
            };
            let form = EffectForm {
                kind: EffectKind::parse(&value_text),
                head,
                options,
                span: node.span.clone(),
                effect_ids: Vec::new(),
            };
            self.check_effect_head(&form);
            return Some((Rhs::Effect(form), Vec::new()));
        }
        if !rest_text.is_empty() {
            if rest_text.starts_with('(') {
                self.syntax(
                    "syntax.fcall_style",
                    format!("`{value_text}(…)` is function-call style; Rivet uses prefix calls"),
                    &node.capture_span("rest"),
                );
                if let Some(e) = self.errors.last_mut() {
                    e.hint = Some(format!(
                        "write ({value_text} arg …) — for example (request \"id\" {{k: v}})"
                    ));
                }
                return None;
            }
            let expr = self.expr(node, "value")?;
            if let Expr::Path(segs, _) = &expr
                && segs.len() >= 2
            {
                let args = self.args(node, "rest");
                let (object, method) = (
                    segs[..segs.len() - 1].to_vec(),
                    segs[segs.len() - 1].clone(),
                );
                return Some((
                    Rhs::Member(MemberCall {
                        object,
                        method,
                        args,
                        span: node.span.clone(),
                    }),
                    options,
                ));
            }
            self.syntax(
                "syntax.trailing",
                format!("unexpected `{rest_text}` after the value"),
                &node.capture_span("rest"),
            );
            return None;
        }
        let expr = self.expr(node, "value")?;
        Some((Rhs::Expr(expr), options))
    }

    fn check_effect_head(&mut self, form: &EffectForm) {
        match form.kind {
            EffectKind::Http => {
                let ok = form.head.len() >= 2
                    && form.head[0].word().is_some_and(|m| {
                        matches!(
                            m,
                            "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
                        )
                    });
                if !ok {
                    self.syntax(
                        "syntax.http",
                        "expected `http METHOD URL` (get, post, put, patch, delete, head, options)",
                        &form.span,
                    );
                }
            }
            EffectKind::File => {
                let ok = form.head.first().and_then(Arg::word).is_some_and(|v| {
                    crate::domain::files::FileVerb::parse(v).is_some()
                        || v == "open"
                        || v == "watch"
                });
                if !ok {
                    self.syntax("syntax.file", "expected `file VERB PATH` (read, list, stat, create, update, write, append, delete, copy, move)", &form.span);
                }
            }
            _ => {}
        }
    }

    fn lower_stmt(&mut self, n: &SyntaxNode, ctx: BlockCtx) -> Option<Stmt> {
        let span = n.span.clone();
        match n.func.as_str() {
            "assign" => {
                let (rhs, options) = self.rhs(n, Vec::new())?;
                Some(Stmt::Assign {
                    var: n.text("var").to_string(),
                    rhs,
                    options,
                    span,
                })
            }
            "assign_block" => {
                let (opts, body) = self.options_then_body(n);
                for b in body {
                    self.syntax(
                        "syntax.option_expected",
                        format!(
                            "only option lines may follow `{} = …`; found `{}`",
                            n.text("var"),
                            display_func(&b.func)
                        ),
                        &b.span,
                    );
                }
                let (rhs, options) = self.rhs(n, opts)?;
                Some(Stmt::Assign {
                    var: n.text("var").to_string(),
                    rhs,
                    options,
                    span,
                })
            }
            "assign_map" => {
                let iter = self.expr(n, "iter")?;
                let args = self.args(n, "rest");
                let mut limit = None;
                let mut it = args.iter();
                while let Some(a) = it.next() {
                    match a.word() {
                        Some("limit") => {
                            limit = it
                                .next()
                                .and_then(|v| const_number(&v.to_expr()))
                                .map(|x| x as i64)
                        }
                        _ => self.syntax("syntax.map", "expected `limit N`", a.span()),
                    }
                }
                let body = self.lower_children(n, BlockCtx::MapOrPoll);
                if !contains_yield(&body) {
                    self.syntax(
                        "syntax.map_yield",
                        "a `map` body must `yield` each item's value",
                        &span,
                    );
                }
                Some(Stmt::Assign {
                    var: n.text("var").to_string(),
                    rhs: Rhs::Map {
                        item: n.text("item").to_string(),
                        iter,
                        limit,
                        body,
                    },
                    options: Vec::new(),
                    span,
                })
            }
            "assign_poll" => {
                let args = self.args(n, "rest");
                let (mut every, mut timeout) = (None, None);
                let mut it = args.iter();
                while let Some(a) = it.next() {
                    match a.word() {
                        Some("every") => every = it.next().and_then(|v| self.duration(v)),
                        Some("timeout") => timeout = it.next().and_then(|v| self.duration(v)),
                        _ => self.syntax(
                            "syntax.poll",
                            "expected `every \"D\"` and `timeout \"D\"`",
                            a.span(),
                        ),
                    }
                }
                if timeout.is_none() {
                    self.syntax(
                        "syntax.poll",
                        "`poll` needs a finite `timeout \"D\"`",
                        &span,
                    );
                }
                let body = self.lower_children(n, BlockCtx::MapOrPoll);
                Some(Stmt::Assign {
                    var: n.text("var").to_string(),
                    rhs: Rhs::Poll {
                        every,
                        timeout,
                        body,
                    },
                    options: Vec::new(),
                    span,
                })
            }
            "append_assign" => Some(Stmt::Append {
                var: n.text("var").to_string(),
                value: self.expr(n, "value")?,
                span,
            }),
            "return" => {
                let (value, _) = self.rhs(n, Vec::new())?;
                Some(Stmt::Return { value, span })
            }
            "yield" => {
                if ctx != BlockCtx::MapOrPoll {
                    self.syntax("syntax.yield", "`yield` only produces a value inside `map` or `poll`; use `return` to exit the operation", &span);
                }
                let (value, _) = self.rhs(n, Vec::new())?;
                Some(Stmt::Yield { value, span })
            }
            "emit" => {
                let (value, _) = self.rhs(n, Vec::new())?;
                Some(Stmt::Emit { value, span })
            }
            "fail" => {
                let details = if n.capture("details").and_then(|c| c.span.clone()).is_some() {
                    self.expr(n, "details")?
                } else {
                    Expr::Lit(Value::Null)
                };
                Some(Stmt::Fail {
                    code: unquote(n.text("code")),
                    details,
                    span,
                })
            }
            "break" => Some(Stmt::Break { span }),
            "if" => Some(Stmt::If {
                cond: self.expr(n, "cond")?,
                then: self.lower_children(n, ctx.inner()),
                // `if COND … else … end`: the grammar's `else` block section
                otherwise: match n.section("else") {
                    Some(kids) => self.lower_block(&kids.iter().collect::<Vec<_>>(), ctx.inner()),
                    None => Vec::new(),
                },
                span,
            }),
            "while" => Some(Stmt::While {
                cond: self.expr(n, "cond")?,
                body: self.lower_children(n, ctx.inner()),
                span,
            }),
            "for" => Some(Stmt::For {
                var: n.text("var").to_string(),
                iter: self.expr(n, "iter")?,
                body: self.lower_children(n, ctx.inner()),
                span,
            }),
            "dag" => {
                let options = self.group_options(n, "rest");
                let mut nodes = Vec::new();
                for c in n.children() {
                    match c.func.as_str() {
                        "node" | "node_after" => {
                            let Some(expr) = self.expr(c, "value") else {
                                continue;
                            };
                            let after = if c.func == "node_after" {
                                match self.expr(c, "deps") {
                                    Some(Expr::List(items)) => items
                                        .iter()
                                        .filter_map(|i| match i {
                                            Expr::Path(p, _) if p.len() == 1 => Some(p[0].clone()),
                                            _ => None,
                                        })
                                        .collect(),
                                    _ => {
                                        self.syntax(
                                            "syntax.dag",
                                            "`after` takes a list of node names, e.g. after [a, b]",
                                            &c.span,
                                        );
                                        Vec::new()
                                    }
                                }
                            } else {
                                Vec::new()
                            };
                            nodes.push(DagNode {
                                name: c.text("name").to_string(),
                                after,
                                expr,
                                span: c.span.clone(),
                            });
                        }
                        other => self.syntax(
                            "syntax.dag",
                            format!(
                                "a `dag` block contains only `node` lines; found `{}`",
                                display_func(other)
                            ),
                            &c.span,
                        ),
                    }
                }
                self.check_dag(&nodes, &span);
                Some(Stmt::Dag {
                    options,
                    nodes,
                    span,
                })
            }
            "concurrent" => {
                let options = self.group_options(n, "rest");
                let mut tasks = Vec::new();
                for c in n.children() {
                    if c.func == "task" {
                        tasks.push((
                            c.text("name").to_string(),
                            self.lower_children(c, BlockCtx::Body),
                        ));
                    } else {
                        self.syntax(
                            "syntax.concurrent",
                            "a `concurrent` block contains only `task NAME … end` blocks",
                            &c.span,
                        );
                    }
                }
                Some(Stmt::Concurrent {
                    options,
                    tasks,
                    span,
                })
            }
            "iterate" => {
                let args = self.args(n, "rest");
                let max = match (
                    args.first().and_then(Arg::word),
                    args.get(1).map(|a| const_number(&a.to_expr())),
                ) {
                    (Some("max"), Some(Some(v))) => v as i64,
                    _ => {
                        self.syntax("syntax.iterate", "expected `iterate max N`", &span);
                        1
                    }
                };
                Some(Stmt::Iterate {
                    max,
                    body: self.lower_children(n, ctx.inner()),
                    span,
                })
            }
            "scope" => {
                let args = self.args(n, "rest");
                let timeout_ms = match (args.first().and_then(Arg::word), args.get(1)) {
                    (Some("timeout"), Some(v)) => self.duration(v),
                    (None, None) => None,
                    _ => {
                        self.syntax("syntax.scope", "expected `scope timeout \"D\"`", &span);
                        None
                    }
                };
                Some(Stmt::Scope {
                    timeout_ms,
                    body: self.lower_children(n, ctx.inner()),
                    span,
                })
            }
            "with" => self.lower_with(n),
            "file" | "file_block" => {
                let head = self.args(n, "rest");
                let (options, body) = self.options_then_body(n);
                for b in body {
                    self.syntax(
                        "syntax.option_expected",
                        format!(
                            "a `file` option block takes option lines only; found `{}`",
                            display_func(&b.func)
                        ),
                        &b.span,
                    );
                }
                let form = EffectForm {
                    kind: EffectKind::File,
                    head,
                    options,
                    span: span.clone(),
                    effect_ids: Vec::new(),
                };
                self.check_effect_head(&form);
                Some(Stmt::Effect { form, span })
            }
            "member_call" => {
                let mut object = vec![n.text("obj").to_string()];
                let method_path: Vec<String> =
                    n.text("method").split('.').map(str::to_string).collect();
                let method = method_path.last().cloned().unwrap_or_default();
                object.extend(
                    method_path[..method_path.len().saturating_sub(1)]
                        .iter()
                        .cloned(),
                );
                let args = self.args(n, "rest");
                Some(Stmt::Member {
                    call: MemberCall {
                        object,
                        method,
                        args,
                        span: span.clone(),
                    },
                    span,
                })
            }
            "call_stmt" => Some(Stmt::Call {
                expr: self.expr(n, "call")?,
                span,
            }),
            "secret" => {
                let args = self.args(n, "rest");
                let words: Vec<Option<&str>> = args.iter().map(Arg::word).collect();
                if words.first().copied().flatten() != Some("from")
                    || words.get(1).copied().flatten() != Some("env")
                {
                    self.syntax(
                        "syntax.secret",
                        "expected `secret NAME from env \"VAR\" for \"ORIGIN\"`",
                        &span,
                    );
                    return None;
                }
                let env = args
                    .get(2)
                    .and_then(|a| a.to_expr().const_text())
                    .unwrap_or_default();
                let mut origins = Vec::new();
                if words.get(3).copied().flatten() == Some("for") {
                    for a in &args[4..] {
                        match a.to_expr() {
                            Expr::List(items) => {
                                origins.extend(items.iter().filter_map(Expr::const_text))
                            }
                            e => origins.extend(e.const_text()),
                        }
                    }
                }
                if origins.is_empty() {
                    self.syntax(
                        "syntax.secret",
                        "a secret must be bound to destination origins with `for \"ORIGIN\"`",
                        &span,
                    );
                }
                Some(Stmt::Secret {
                    name: n.text("name").to_string(),
                    env,
                    origins,
                    span,
                })
            }
            "opt_until" => {
                if ctx != BlockCtx::MapOrPoll {
                    self.syntax(
                        "syntax.until",
                        "`until` is only valid inside a `poll` body",
                        &span,
                    );
                }
                let text = n.text("rest");
                let cond = match parse_expr(text, &map_for(n, "rest")) {
                    Ok(e) => e,
                    Err(e) => {
                        self.fail(e);
                        return None;
                    }
                };
                Some(Stmt::Until { cond, span })
            }
            f if header_rank(f).is_some() => {
                self.syntax(
                    "syntax.option_after_body",
                    format!(
                        "`{}` is a header line and must come before the first body statement",
                        display_func(f)
                    ),
                    &span,
                );
                None
            }
            f if is_option(f) => {
                self.syntax(
                    "syntax.option_misplaced",
                    format!("option `{}` is only valid at the start of a resource block (http, with, file, …)", option_key(f)),
                    &span,
                );
                None
            }
            "node" | "node_after" => {
                self.syntax(
                    "syntax.dag",
                    "`node` lines belong inside a `dag` block",
                    &span,
                );
                None
            }
            "task" => {
                self.syntax(
                    "syntax.concurrent",
                    "`task` blocks belong inside a `concurrent` block",
                    &span,
                );
                None
            }
            other => {
                self.syntax(
                    "syntax.statement",
                    format!("`{}` is not valid here", display_func(other)),
                    &span,
                );
                None
            }
        }
    }

    fn lower_with(&mut self, n: &SyntaxNode) -> Option<Stmt> {
        let span = n.span.clone();
        let args = self.args(n, "rest");
        let as_pos = args.iter().rposition(|a| a.word() == Some("as"));
        let (head_args, bind) = match as_pos {
            Some(p) if p + 1 < args.len() => {
                (args[..p].to_vec(), args[p + 1].word().map(str::to_string))
            }
            _ => (args.clone(), None),
        };
        if bind.is_none() && !matches!(head_args.first().and_then(Arg::word), Some("udp")) {
            self.syntax("syntax.with", "expected `with RESOURCE … as NAME`", &span);
        }
        let (options, body_nodes) = self.options_then_body(n);
        let body = self.lower_block(&body_nodes, BlockCtx::Body);
        let first = head_args.first().cloned();
        let (kind, head, source) = match &first {
            Some(Arg::Word(w, _)) if w == "grpc" => {
                (EffectKind::Grpc, grpc_head(head_args[1..].to_vec()), None)
            }
            Some(Arg::Word(w, _)) if EFFECT_WORDS.contains(&w.as_str()) => {
                (EffectKind::parse(w), head_args[1..].to_vec(), None)
            }
            Some(Arg::Expr(Expr::Call { func, .. }, _)) if func == "request.stream" => (
                EffectKind::RequestStream,
                Vec::new(),
                Some(first.as_ref().unwrap().to_expr()),
            ),
            Some(Arg::Expr(Expr::Path(p, _), _)) if p.len() >= 2 => {
                // `with connection.open bidi as stream` — a resource derived from an enclosing handle.
                (
                    EffectKind::Connection,
                    head_args[1..].to_vec(),
                    Some(first.as_ref().unwrap().to_expr()),
                )
            }
            _ => {
                self.syntax("syntax.with", "unknown resource; expected http, tcp, unix, pipe, udp, quic, websocket, grpc, command, file, (request.stream …) or HANDLE.open", &span);
                return None;
            }
        };
        let form = EffectForm {
            kind,
            head,
            options,
            span: span.clone(),
            effect_ids: Vec::new(),
        };
        Some(Stmt::With {
            form,
            source,
            bind,
            body,
            span,
        })
    }

    fn check_dag(&mut self, nodes: &[DagNode], span: &SourceSpan) {
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        for (i, n) in nodes.iter().enumerate() {
            if names[..i].contains(&n.name.as_str()) {
                self.syntax(
                    "syntax.dag",
                    format!("node `{}` is declared twice", n.name),
                    &n.span,
                );
            }
            for dep in &n.after {
                if !names.contains(&dep.as_str()) {
                    self.syntax(
                        "syntax.dag",
                        format!("node `{}` depends on unknown node `{dep}`", n.name),
                        &n.span,
                    );
                }
            }
            let mut read = Vec::new();
            n.expr.paths(&mut read);
            for p in read {
                if let Some(first) = p.first()
                    && names.contains(&first.as_str())
                    && first != &n.name
                    && !n.after.contains(first)
                {
                    self.syntax(
                        "syntax.dag",
                        format!(
                            "node `{}` reads `{first}` without declaring `after [{first}]`",
                            n.name
                        ),
                        &n.span,
                    );
                }
            }
        }
        // Cycle check (Kahn). In-degree counts distinct, declared dependencies:
        // `after [a, a]` or an unknown name must not look like a cycle.
        let mut indeg: Vec<usize> = nodes
            .iter()
            .map(|n| {
                let mut deps: Vec<&String> = n
                    .after
                    .iter()
                    .filter(|d| names.contains(&d.as_str()))
                    .collect();
                deps.sort();
                deps.dedup();
                deps.len()
            })
            .collect();
        let mut ready: Vec<usize> = (0..nodes.len()).filter(|&i| indeg[i] == 0).collect();
        let mut seen = 0;
        while let Some(i) = ready.pop() {
            seen += 1;
            for (j, m) in nodes.iter().enumerate() {
                // Guarded: a duplicated node name (already a syntax error) may be
                // popped twice and must not underflow the counter.
                if m.after.contains(&nodes[i].name) && indeg[j] > 0 {
                    indeg[j] -= 1;
                    if indeg[j] == 0 {
                        ready.push(j);
                    }
                }
            }
        }
        if seen < nodes.len() {
            self.syntax(
                "syntax.dag_cycle",
                "the dag's `after` edges form a cycle",
                span,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCtx {
    Operation,
    Body,
    MapOrPoll,
}

impl BlockCtx {
    fn inner(self) -> BlockCtx {
        match self {
            BlockCtx::MapOrPoll => BlockCtx::MapOrPoll,
            _ => BlockCtx::Body,
        }
    }
}

/// `grpc users.GetUser`: the head names a descriptor method, not a variable
/// path, so it stays a bare word (`users.GetUser`) and is never evaluated.
fn grpc_head(mut head: Vec<Arg>) -> Vec<Arg> {
    if let Some(Arg::Expr(Expr::Path(p, _), span)) = head.first() {
        head[0] = Arg::Word(p.join("."), span.clone());
    }
    head
}

fn display_func(f: &str) -> String {
    match f {
        "declared_error" => "error".into(),
        "auth_use" | "auth_profile" => "auth".into(),
        "output_block" | "emits_block" | "receives_block" | "field_block" | "file_block" => {
            f.trim_end_matches("_block").into()
        }
        "node_after" => "node".into(),
        "assign" | "assign_block" | "assign_map" | "assign_poll" => "assignment".into(),
        "append_assign" => "+=".into(),
        "member_call" => "method call".into(),
        "call_stmt" => "call".into(),
        other => other.trim_start_matches("opt_").into(),
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.is_ascii()
        && id.split('.').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Strip one pair of surrounding quotes and decode escapes of a string capture.
pub fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && (s.starts_with('"') && s.ends_with('"') || s.starts_with('\'') && s.ends_with('\''))
    {
        let map = SpanMap::new(SourceSpan::default(), s);
        if let Ok(e) = parse_expr(s, &map)
            && let Some(t) = e.const_text()
        {
            return t;
        }
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

fn keyword_text(args: &[Arg], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a.word() == Some(key))
        .and_then(|p| args.get(p + 1))
        .and_then(|a| a.to_expr().const_text())
}

pub fn const_value(e: &Expr) -> Option<Value> {
    match e {
        Expr::Lit(v) => Some(v.clone()),
        Expr::List(items) => items
            .iter()
            .map(const_value)
            .collect::<Option<Vec<_>>>()
            .map(Value::List),
        Expr::Object(pairs) => pairs
            .iter()
            .map(|(k, v)| const_value(v).map(|v| (k.clone(), v)))
            .collect::<Option<Vec<_>>>()
            .map(Value::Object),
        Expr::Template(_) => e.const_text().map(Value::Text),
        Expr::Neg(inner) => match const_value(inner)? {
            Value::Int(i) => Some(Value::Int(-i)),
            Value::Float(f) => Some(Value::Float(-f)),
            _ => None,
        },
        _ => None,
    }
}

fn const_number(e: &Expr) -> Option<f64> {
    match const_value(e)? {
        Value::Int(i) => Some(i as f64),
        Value::Float(f) => Some(f),
        _ => None,
    }
}

fn contains_yield(body: &[Stmt]) -> bool {
    body.iter().any(|s| match s {
        Stmt::Yield { .. } => true,
        Stmt::If {
            then, otherwise, ..
        } => contains_yield(then) || contains_yield(otherwise),
        Stmt::Try { body, handler, .. } => contains_yield(body) || contains_yield(handler),
        Stmt::Scope { body, .. }
        | Stmt::With { body, .. }
        | Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Iterate { body, .. } => contains_yield(body),
        _ => false,
    })
}

/// Literal `(request "id" …)` / `(request.stream "id" …)` targets.
pub fn collect_calls(body: &[Stmt], out: &mut Vec<String>) {
    fn expr(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Call { func, args, .. } => {
                if (func == "request" || func == "request.stream")
                    && let Some(id) = args.first().and_then(Expr::const_text)
                {
                    out.push(id);
                }
                args.iter().for_each(|a| expr(a, out));
            }
            Expr::List(items) => items.iter().for_each(|i| expr(i, out)),
            Expr::Object(pairs) => pairs.iter().for_each(|(_, v)| expr(v, out)),
            Expr::Binary { lhs, rhs, .. } => {
                expr(lhs, out);
                expr(rhs, out);
            }
            Expr::Not(i) | Expr::Neg(i) => expr(i, out),
            _ => {}
        }
    }
    fn rhs(r: &Rhs, out: &mut Vec<String>) {
        match r {
            Rhs::Expr(e) => expr(e, out),
            Rhs::Map { iter, body, .. } => {
                expr(iter, out);
                collect_calls(body, out);
            }
            Rhs::Poll { body, .. } => collect_calls(body, out),
            _ => {}
        }
    }
    for s in body {
        match s {
            Stmt::Assign { rhs: r, .. }
            | Stmt::Return { value: r, .. }
            | Stmt::Yield { value: r, .. }
            | Stmt::Emit { value: r, .. } => rhs(r, out),
            Stmt::Append { value, .. } | Stmt::Until { cond: value, .. } => expr(value, out),
            Stmt::Fail { details, .. } => expr(details, out),
            Stmt::Call { expr: e, .. } => expr(e, out),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                expr(cond, out);
                collect_calls(then, out);
                collect_calls(otherwise, out);
            }
            Stmt::While { cond, body, .. } => {
                expr(cond, out);
                collect_calls(body, out);
            }
            Stmt::For { iter, body, .. } => {
                expr(iter, out);
                collect_calls(body, out);
            }
            Stmt::Try { body, handler, .. } => {
                collect_calls(body, out);
                collect_calls(handler, out);
            }
            Stmt::Dag { nodes, .. } => nodes.iter().for_each(|n| expr(&n.expr, out)),
            Stmt::Concurrent { tasks, .. } => tasks.iter().for_each(|(_, b)| collect_calls(b, out)),
            Stmt::Iterate { body, .. } | Stmt::Scope { body, .. } => collect_calls(body, out),
            Stmt::With { source, body, .. } => {
                if let Some(e) = source {
                    expr(e, out);
                }
                collect_calls(body, out);
            }
            _ => {}
        }
    }
}

/// `check --strict-docs` findings: every public operation, parameter, output
/// and field needs a description, and every `fail` code must be declared.
pub fn strict_doc_findings(program: &crate::domain::ir::CompiledProgram) -> Vec<RivetError> {
    fn fields(spec: &ValueSpec, path: &str, op: &Operation, out: &mut Vec<RivetError>) {
        if let ValueSpec::Object { fields, .. } = spec {
            for f in fields {
                let p = format!("{path}.{}", f.name);
                if f.description.as_deref().unwrap_or("").trim().is_empty() {
                    out.push(RivetError::syntax(
                        "docs.field_description",
                        format!("`{}` field `{p}` has no description", op.id),
                        Some(op.span.clone()),
                    ));
                }
                fields_rec(&f.spec, &p, op, out);
            }
        }
    }
    fn fields_rec(spec: &ValueSpec, path: &str, op: &Operation, out: &mut Vec<RivetError>) {
        match spec {
            ValueSpec::List(inner) => fields(inner, path, op, out),
            other => fields(other, path, op, out),
        }
    }
    fn fail_codes(body: &[Stmt], out: &mut Vec<(String, SourceSpan)>) {
        for s in body {
            match s {
                Stmt::Fail { code, span, .. } => out.push((code.clone(), span.clone())),
                Stmt::If {
                    then, otherwise, ..
                } => {
                    fail_codes(then, out);
                    fail_codes(otherwise, out);
                }
                Stmt::Try { body, handler, .. } => {
                    fail_codes(body, out);
                    fail_codes(handler, out);
                }
                Stmt::For { body, .. }
                | Stmt::While { body, .. }
                | Stmt::Iterate { body, .. }
                | Stmt::Scope { body, .. }
                | Stmt::With { body, .. } => fail_codes(body, out),
                Stmt::Concurrent { tasks, .. } => {
                    tasks.iter().for_each(|(_, b)| fail_codes(b, out))
                }
                Stmt::Assign {
                    rhs: Rhs::Map { body, .. } | Rhs::Poll { body, .. },
                    ..
                } => fail_codes(body, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for op in program.operations.iter().filter(|o| !o.private) {
        if op.description.as_deref().unwrap_or("").trim().is_empty() {
            out.push(RivetError::syntax(
                "docs.description",
                format!("operation `{}` has no description", op.id),
                Some(op.span.clone()),
            ));
        }
        for p in &op.params {
            if p.description.as_deref().unwrap_or("").trim().is_empty() {
                out.push(RivetError::syntax(
                    "docs.param_description",
                    format!("`{}` parameter `{}` has no description", op.id, p.name),
                    Some(op.span.clone()),
                ));
            }
        }
        if op
            .output
            .description
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            out.push(RivetError::syntax(
                "docs.output_description",
                format!("`{}` output has no description", op.id),
                Some(op.span.clone()),
            ));
        }
        fields_rec(&op.output.spec, "output", op, &mut out);
        for (label, spec) in [("emits", &op.emits), ("receives", &op.receives)] {
            if let Some(s) = spec {
                fields_rec(s, label, op, &mut out);
            }
        }
        let mut codes = Vec::new();
        fail_codes(&op.body, &mut codes);
        for (code, span) in codes {
            if !op.errors.iter().any(|e| e.code == code) {
                out.push(RivetError::syntax(
                    "docs.undeclared_error",
                    format!(
                        "`{}` can fail with `{code}` but declares no `error \"{code}\"` line",
                        op.id
                    ),
                    Some(span),
                ));
            }
        }
    }
    out
}
