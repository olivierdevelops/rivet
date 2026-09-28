//! Global constants (PROP-2026-0002 R7–R8): evaluate every file's
//! `global NAME = EXPR` once, in declaration order, into a frozen read-only
//! scope, and refuse every way an operation could shadow or assign one.
//!
//! ```text
//!  GlobalDecl (source order, per file)
//!     │ duplicate name ─────────────────────────────▶ check.global_duplicate   (second name)
//!     │ reads a later/its own global ───────────────▶ check.global_forward_ref (the reference)
//!     │ effect, request, env, secret, param, local ─▶ check.global_not_constant (the expression)
//!     ▼
//!  fold (pure: literals, lists, objects, + - * / %, comparisons, ${…}, pure built-ins)
//!     ▼
//!  GlobalScope{file, names, values}  ── operations: param/for/with/node/secret NAME ─▶ check.global_shadow
//!                                                   NAME = … / NAME += …             ─▶ check.global_assign
//! ```

use crate::domain::const_eval::{PURE_FUNCTIONS, fold, read_path};
use crate::domain::ir::{CompiledProgram, Expr, GlobalDecl, GlobalScope, Rhs, Stmt, TemplatePart};
use crate::domain::source::SourceSpan;
use crate::domain::{RivetError, RivetResult, Value};

// vhco:usecase language.compile_globals(input: CompiledProgram) -> GlobalScope[]
// vhco:label Compile global constants
// vhco:about Evaluates the bundle's top-level `global NAME = EXPR` declarations once at load, in declaration order, into one frozen read-only GlobalScope per file shared by every operation of that file (PROP-2026-0002 R7-R9).
// vhco:example input={globals:[api = "https://api.example.com", users_url = "${api}/users"]} => [{ "file": "app.rivet", "names": ["api", "users_url"], "values": {"api": "https://api.example.com", "users_url": "https://api.example.com/users"} }]
pub fn compile_globals(input: &CompiledProgram) -> RivetResult<Vec<GlobalScope>> {
    let mut errors = Vec::new();
    let mut scopes: Vec<GlobalScope> = Vec::new();

    // vhco:todo declaration_order -- group GlobalDecl by file in source order; a name declared twice in one file is check.global_duplicate at the second name; a reference to a global declared later in the file (or to itself) is check.global_forward_ref at the reference
    // vhco:step order GlobalDecl -- one scope per file, filled strictly in declaration order so only earlier globals are visible
    for decl in &input.globals {
        let file = decl.span.file.clone();
        if !scopes.iter().any(|s| s.file == file) {
            scopes.push(GlobalScope {
                file: file.clone(),
                names: Vec::new(),
                values: Value::Object(Vec::new()),
            });
        }
        let scope = scopes.iter_mut().find(|s| s.file == file).expect("scope");
        // vhco:error duplicate -- a global name is declared twice in one file => check.global_duplicate (exit 2) returns
        if scope.contains(&decl.name) {
            errors.push(RivetError::syntax(
                "check.global_duplicate",
                format!("global `{}` is declared twice in {file}", decl.name),
                Some(decl.span.clone()),
            ));
            continue;
        }
        let in_file: Vec<&str> = input
            .globals
            .iter()
            .filter(|g| g.span.file == file)
            .map(|g| g.name.as_str())
            .collect();
        // vhco:todo constant_only -- evaluate EXPR with the pure constant subset (literals, lists, objects, arithmetic, comparisons, ${…} interpolation, pure built-ins over earlier globals); an effect, `request`, env, secret, param or local reference is check.global_not_constant (exit 2) at the expression with a hint to move it into the operation
        // vhco:step evaluate eval_constant -- refuse non-constant parts first, then fold the expression over the earlier globals of the file
        if let Some(e) = eval_constant_problem(decl, &decl.expr, &scope.names, &in_file) {
            errors.push(e);
            continue;
        }
        let values = scope.values.clone();
        let lookup = |p: &[String]| read_path(values.get(&p[0])?, &p[1..]);
        match fold(&decl.expr, &lookup) {
            Ok(Some(v)) => {
                scope.names.push(decl.name.clone());
                scope.values.set(&decl.name, v);
            }
            // A path into an earlier global that does not exist (`headers.nope`).
            Ok(None) => errors.push(RivetError::syntax(
                "check.global_not_constant",
                format!(
                    "global `{}` reads a key or index its earlier globals do not have",
                    decl.name
                ),
                Some(decl.expr_span.clone()),
            )),
            // A constant that fails to evaluate keeps its own code (value.type, …).
            Err(e) => errors.push(e.with_span(Some(decl.expr_span.clone()))),
        }
    }

    // vhco:todo freeze_scope -- return one GlobalScope{file, names in order, values} per file, frozen behind an Arc by the interpreter; frames look it up after locals and params; a param, loop variable, `with … as NAME`, map item, dag node, task or secret reusing a global name is check.global_shadow, and `NAME = …` / `NAME += …` on a global is check.global_assign (both at compile time)
    // vhco:step shadow check_shadowing -- walk every operation of each file against that file's global names
    for op in &input.operations {
        let Some(scope) = scopes.iter().find(|s| s.file == op.file) else {
            continue;
        };
        for (p, span) in op.params.iter().zip(&op.param_spans) {
            if scope.contains(&p.name) {
                errors.push(shadow(&p.name, "parameter", span));
            }
        }
        check_shadowing(&op.body, scope, &mut errors);
    }

    // vhco:error not_constant -- EXPR reads an effect, env, secret, param or local => check.global_not_constant (exit 2) returns
    // vhco:error forward_ref -- EXPR names a later or the same global => check.global_forward_ref (exit 2) returns
    if !errors.is_empty() {
        let mut first = errors.remove(0);
        first.suppressed = errors;
        return Err(first);
    }
    // vhco:step freeze GlobalScope -- the scopes are immutable from here on; the interpreter shares them read-only across concurrent requests
    Ok(scopes)
}

/// The first reason `e` is not a constant over the `earlier` globals of its
/// file, if any.
fn eval_constant_problem(
    decl: &GlobalDecl,
    e: &Expr,
    earlier: &[String],
    in_file: &[&str],
) -> Option<RivetError> {
    let name_ok = |head: &str, span: &SourceSpan| -> Option<RivetError> {
        if earlier.iter().any(|n| n == head) {
            return None;
        }
        if in_file.contains(&head) {
            return Some(RivetError::syntax(
                "check.global_forward_ref",
                format!(
                    "global `{}` reads `{head}`, which is not declared before it; globals see only earlier globals",
                    decl.name
                ),
                Some(span.clone()),
            ));
        }
        Some(
            RivetError::syntax(
                "check.global_not_constant",
                format!(
                    "global `{}` reads `{head}`, which is not an earlier global; params, locals, env and secrets do not exist at load time",
                    decl.name
                ),
                Some(span.clone()),
            )
            .with_hint("declare the value inside the operation that uses it"),
        )
    };
    match e {
        Expr::Lit(_) => None,
        Expr::Path(p, span) => name_ok(&p[0], span),
        Expr::Template(parts) => parts.iter().find_map(|p| match p {
            TemplatePart::Path(path) => name_ok(&path[0], &decl.expr_span),
            TemplatePart::Lit(_) => None,
        }),
        Expr::List(items) => items
            .iter()
            .find_map(|i| eval_constant_problem(decl, i, earlier, in_file)),
        Expr::Object(pairs) => pairs
            .iter()
            .find_map(|(_, v)| eval_constant_problem(decl, v, earlier, in_file)),
        Expr::Binary { lhs, rhs, .. } => eval_constant_problem(decl, lhs, earlier, in_file)
            .or_else(|| eval_constant_problem(decl, rhs, earlier, in_file)),
        Expr::Not(i) | Expr::Neg(i) => eval_constant_problem(decl, i, earlier, in_file),
        Expr::Call { func, args, .. } => {
            if func == "env" {
                return Some(
                    RivetError::syntax(
                        "check.global_not_constant",
                        format!(
                            "global `{}` cannot read the environment; globals are fixed at load time",
                            decl.name
                        ),
                        Some(decl.expr_span.clone()),
                    )
                    .with_hint(format!(
                        "declare `secret {} from env \"…\" for \"https://…\"` inside the operation that uses it",
                        decl.name
                    )),
                );
            }
            if !PURE_FUNCTIONS.contains(&func.as_str()) {
                return Some(
                    RivetError::syntax(
                        "check.global_not_constant",
                        format!(
                            "global `{}` cannot call `{func}`; globals may use only literals, operators and the pure built-ins ({})",
                            decl.name,
                            PURE_FUNCTIONS.join(", ")
                        ),
                        Some(decl.expr_span.clone()),
                    )
                    .with_hint("move the call into the operation that uses it"),
                );
            }
            args.iter()
                .find_map(|a| eval_constant_problem(decl, a, earlier, in_file))
        }
    }
}

fn shadow(name: &str, what: &str, span: &SourceSpan) -> RivetError {
    RivetError::syntax(
        "check.global_shadow",
        format!("{what} `{name}` reuses the name of a global; globals cannot be shadowed"),
        Some(span.clone()),
    )
    .with_hint(format!("rename the {what}"))
}

fn assign(name: &str, span: &SourceSpan) -> RivetError {
    RivetError::syntax(
        "check.global_assign",
        format!("`{name}` is a global and globals are read-only"),
        Some(span.clone()),
    )
    .with_hint("assign to a new local name instead")
}

/// Every binding form of an operation body against one file's global names.
fn check_shadowing(body: &[Stmt], scope: &GlobalScope, errors: &mut Vec<RivetError>) {
    let is_global = |n: &str| scope.contains(n);
    for s in body {
        match s {
            Stmt::Assign { var, rhs, span, .. } => {
                if is_global(var) {
                    errors.push(assign(var, span));
                }
                match rhs {
                    Rhs::Map { item, body, .. } => {
                        if is_global(item) {
                            errors.push(shadow(item, "map item", span));
                        }
                        check_shadowing(body, scope, errors);
                    }
                    Rhs::Poll { body, .. } => check_shadowing(body, scope, errors),
                    _ => {}
                }
            }
            Stmt::Append { var, span, .. } => {
                if is_global(var) {
                    errors.push(assign(var, span));
                }
            }
            Stmt::For {
                var, body, span, ..
            } => {
                if is_global(var) {
                    errors.push(shadow(var, "loop variable", span));
                }
                check_shadowing(body, scope, errors);
            }
            Stmt::With {
                bind, body, span, ..
            } => {
                if let Some(b) = bind.as_deref().filter(|b| is_global(b)) {
                    errors.push(shadow(b, "binding", span));
                }
                check_shadowing(body, scope, errors);
            }
            Stmt::Secret { name, span, .. } => {
                if is_global(name) {
                    errors.push(shadow(name, "secret", span));
                }
            }
            Stmt::Dag { nodes, .. } => {
                for n in nodes.iter().filter(|n| is_global(&n.name)) {
                    errors.push(shadow(&n.name, "dag node", &n.span));
                }
            }
            Stmt::Concurrent { tasks, span, .. } => {
                for (name, b) in tasks {
                    if is_global(name) {
                        errors.push(shadow(name, "task", span));
                    }
                    check_shadowing(b, scope, errors);
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                check_shadowing(then, scope, errors);
                check_shadowing(otherwise, scope, errors);
            }
            Stmt::Try { body, handler, .. } => {
                check_shadowing(body, scope, errors);
                check_shadowing(handler, scope, errors);
            }
            Stmt::While { body, .. } | Stmt::Iterate { body, .. } | Stmt::Scope { body, .. } => {
                check_shadowing(body, scope, errors)
            }
            _ => {}
        }
    }
}
