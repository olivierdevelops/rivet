use super::compile_globals::compile_globals;
use super::lowering::expr::CallScope;
use super::lowering::lower::{Lowerer, strict_doc_findings};
use super::lowering::modules::namespace_modules;
use super::ports::Parser;
use crate::domain::ir::CompiledProgram;
use crate::domain::source::SourceBundle;
use crate::domain::{RivetError, RivetResult};
use sha2::{Digest, Sha256};

// vhco:usecase language.compile_program(input: SourceBundle) -> CompiledProgram needs Parser
// vhco:label Compile program
// vhco:about Parses every bundled .rivet file with the Capy grammar, lowers the syntax tree to typed IR, checks IDs, headers, DAG edges and call cycles, and freezes an immutable program. Performs no I/O and runs nothing.
// vhco:example input={entry:"app.rivet", files:[{path:"app.rivet", text:"operation demo.add …"}]} => { "operations": ["demo.add"], "source_hash": "sha256:…" }
pub fn compile_program(input: &SourceBundle, parser: &dyn Parser) -> RivetResult<CompiledProgram> {
    let mut lowerer = Lowerer::default();
    let mut program = CompiledProgram {
        entry: input.entry.clone(),
        root: input.root.clone(),
        ..CompiledProgram::default()
    };

    // vhco:todo parse_clean -- hand every bundled file to the Parser port (capy_parser: Library::parse → AST JSON schema 1 → SyntaxTree); any Capy diagnostic, recovered error region or indentation error becomes a kind-syntax error and nothing is lowered from that file
    // vhco:step parse parser.parse -- one SyntaxTree per SourceFile, spans are file/line/column (no byte offsets)
    for file in &input.files {
        let tree = parser.parse(file)?;
        // Calls by operation ID: the file's own IDs and `ALIAS.…` of its imports.
        lowerer.calls = std::sync::Arc::new(CallScope {
            ids: tree
                .nodes
                .iter()
                .filter(|n| n.func == "operation" || n.func == "pipeline")
                .map(|n| n.text("id").to_string())
                .collect(),
            aliases: input
                .module_of(&file.path)
                .map(|m| m.imports.iter().map(|i| i.alias.clone()).collect())
                .unwrap_or_default(),
            any: false,
        });
        // vhco:todo compile_catalog -- lower every top-level `operation`/`pipeline` into one Operation (ID, display name, description, described params, output, emits, receives, declared errors, body) and every `connector`/`auth NAME oauth2` into a Declaration; file basenames never prefix IDs
        // vhco:step lower lowerer.lower_tree -- header order, leading-options rule, try/catch pairing, effect forms, quoted durations and prefix calls are enforced here with exact spans
        lowerer.lower_tree(
            &tree,
            &mut program.operations,
            &mut program.connectors,
            &mut program.auth_profiles,
        );
    }

    // vhco:todo namespace_modules -- with resolved modules, namespace each imported file's operations as ALIAS.ID (transitively), resolve every literal call target in its own file's scope (own IDs, import aliases, own connectors, rivet.*), refuse unresolvable module calls and calls to another module's private operations (check.unknown_operation), report namespaced IDs that collide with another operation and cross-file connector/auth name clashes (check.import_collision), make operations of non-public modules private, and warn check.module_policy_ignored for a module's own policy.json
    // vhco:step modules namespace_modules -- ALIAS.ID namespacing, per-file call resolution, visibility and import collisions (only when the bundle has modules)
    namespace_modules(
        &mut program,
        input,
        &mut lowerer.errors,
        &mut lowerer.warnings,
    );
    program.modules = input.modules.clone();

    // vhco:todo check_program -- reject duplicate operation IDs across the whole bundle (registry.duplicate_id, both spans), literal `(request "id" …)` calls to operations that do not exist and are not connector-imported, and static call cycles over literal targets (check.call_cycle); nothing executes
    // vhco:step duplicates check_duplicates -- first declaration wins the span; the second is the error location
    check_duplicates(&program, &mut lowerer.errors);
    // vhco:step calls check_calls -- unknown literal targets and cycles in the literal call graph
    check_calls(&program, &mut lowerer.errors);

    // vhco:todo check_auth_transport -- every `auth PROFILE account …` and `grpc CONNECTOR.Method` reference names a declared auth profile or connector; unknown names fail compilation before any request
    // vhco:step refs check_references -- walk option lines and effect heads for profile/connector names
    check_references(&program, &mut lowerer.errors);

    // vhco:todo compile_globals -- evaluate every file's `global NAME = EXPR` once through language.compile_globals (declaration order, constants only) and keep the frozen scopes on the program; shadowing or assigning a global in an operation is a check error collected with the rest
    // vhco:step globals compile_globals -- one frozen GlobalScope per file; its check.global_* errors join the other diagnostics
    program.globals = std::mem::take(&mut lowerer.globals);
    match compile_globals(&program) {
        Ok(scopes) => program.global_scopes = scopes,
        Err(mut e) => {
            let rest = std::mem::take(&mut e.suppressed);
            lowerer.errors.push(e);
            lowerer.errors.extend(rest);
        }
    }

    // vhco:error syntax -- any parse, lowering or check error => kind syntax (exit 2) returns with every other error in `suppressed`
    if !lowerer.errors.is_empty() {
        let mut errors = lowerer.errors;
        let mut first = errors.remove(0);
        first.suppressed = errors;
        return Err(first);
    }
    program.warnings = lowerer.warnings;
    // `check` warnings (never fatal here; `--strict-docs` promotes the undeclared
    // codes): undeclared `fail` codes and unguarded `.result` reads of
    // `fail independent` DAG nodes (S46, S123).
    program.warnings.extend(
        strict_doc_findings(&program)
            .into_iter()
            .filter(|f| f.code == "docs.undeclared_error"),
    );
    for op in &program.operations {
        unguarded_results(&op.body, &mut Vec::new(), &[], &mut program.warnings);
    }
    // vhco:step hash sha256 -- the source hash pins requests, sessions and manifests to this exact program
    program.source_hash = source_hash(input);
    Ok(program)
}

/// Warn when a `return` reads `NODE.result` of a `fail independent` DAG node
/// without an enclosing `if NODE.status …` guard: that result is null unless
/// the node succeeded. `nodes` are the independent nodes declared so far in
/// this block chain; `guarded` are names whose `.status` an enclosing `if` tests.
fn unguarded_results(
    body: &[crate::domain::ir::Stmt],
    nodes: &mut Vec<String>,
    guarded: &[String],
    out: &mut Vec<RivetError>,
) {
    use crate::domain::ir::{FailurePolicy, Rhs, Stmt};
    for s in body {
        match s {
            Stmt::Dag {
                options, nodes: ns, ..
            } if options.failure == FailurePolicy::Independent => {
                nodes.extend(ns.iter().map(|n| n.name.clone()));
            }
            Stmt::Return {
                value: Rhs::Expr(e),
                span,
            } => {
                let mut paths = Vec::new();
                e.paths(&mut paths);
                for p in paths {
                    if p.len() >= 2
                        && p[1] == "result"
                        && nodes.contains(&p[0])
                        && !guarded.contains(&p[0])
                    {
                        out.push(RivetError::syntax(
                            "check.unguarded_result",
                            format!(
                                "`{0}.result` is null unless `{0}` succeeded; guard it with `if {0}.status == \"succeeded\"`",
                                p[0]
                            ),
                            Some(span.clone()),
                        ));
                    }
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                let mut paths = Vec::new();
                cond.paths(&mut paths);
                let mut inner: Vec<String> = guarded.to_vec();
                inner.extend(
                    paths
                        .into_iter()
                        .filter(|p| p.len() >= 2 && p[1] == "status")
                        .map(|p| p[0].clone()),
                );
                unguarded_results(then, &mut nodes.clone(), &inner, out);
                unguarded_results(otherwise, &mut nodes.clone(), &inner, out);
            }
            Stmt::Try { body, handler, .. } => {
                unguarded_results(body, &mut nodes.clone(), guarded, out);
                unguarded_results(handler, &mut nodes.clone(), guarded, out);
            }
            Stmt::For { body, .. }
            | Stmt::While { body, .. }
            | Stmt::Iterate { body, .. }
            | Stmt::Scope { body, .. }
            | Stmt::With { body, .. } => unguarded_results(body, &mut nodes.clone(), guarded, out),
            _ => {}
        }
    }
}

fn source_hash(input: &SourceBundle) -> String {
    let mut h = Sha256::new();
    for f in &input.files {
        h.update(f.path.as_bytes());
        h.update([0]);
        h.update(f.text.as_bytes());
        h.update([0]);
    }
    let digest = h.finalize();
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn check_duplicates(program: &CompiledProgram, errors: &mut Vec<RivetError>) {
    for (i, op) in program.operations.iter().enumerate() {
        // Clashes across modules are check.import_collision (namespace_modules).
        if let Some(first) = program.operations[..i]
            .iter()
            .find(|o| o.id == op.id && o.module == op.module)
        {
            errors.push(
                RivetError::syntax(
                    "registry.duplicate_id",
                    format!(
                        "operation ID `{}` is declared twice (first at {}:{})",
                        op.id, first.span.file, first.span.start_line
                    ),
                    Some(op.span.clone()),
                )
                .with_details(crate::domain::Value::object([
                    ("first", first.span.to_value()),
                    ("second", op.span.to_value()),
                ])),
            );
        }
    }
}

fn check_calls(program: &CompiledProgram, errors: &mut Vec<RivetError>) {
    let known = |id: &str| {
        program.operation(id).is_some()
            || id.starts_with("rivet.")
            || program
                .connectors
                .iter()
                .any(|c| id.starts_with(&format!("{}.", c.name)))
    };
    // A module call that namespace_modules already refused is not repeated.
    let refused: Vec<String> = errors
        .iter()
        .filter(|e| e.code == "check.unknown_operation")
        .map(|e| e.message.clone())
        .collect();
    for op in &program.operations {
        for call in &op.calls {
            if !known(call) && !refused.iter().any(|m| m.starts_with(&format!("`{call}` "))) {
                errors.push(RivetError::syntax(
                    "check.unknown_operation",
                    format!("`{}` calls unknown operation `{call}`", op.id),
                    Some(op.span.clone()),
                ));
            }
        }
    }
    // Cycle detection over literal call edges (DFS with colours).
    let ids: Vec<&str> = program.operations.iter().map(|o| o.id.as_str()).collect();
    let mut colour = vec![0u8; ids.len()];
    fn visit(
        i: usize,
        program: &CompiledProgram,
        ids: &[&str],
        colour: &mut [u8],
        stack: &mut Vec<String>,
        errors: &mut Vec<RivetError>,
    ) {
        colour[i] = 1;
        stack.push(ids[i].to_string());
        for call in &program.operations[i].calls {
            if let Some(j) = ids.iter().position(|id| id == call) {
                if colour[j] == 1 {
                    let mut cycle = stack.clone();
                    cycle.push(call.clone());
                    errors.push(RivetError::syntax(
                        "check.call_cycle",
                        format!(
                            "operations call each other in a cycle: {}",
                            cycle.join(" → ")
                        ),
                        Some(program.operations[i].span.clone()),
                    ));
                } else if colour[j] == 0 {
                    visit(j, program, ids, colour, stack, errors);
                }
            }
        }
        stack.pop();
        colour[i] = 2;
    }
    for i in 0..ids.len() {
        if colour[i] == 0 {
            visit(i, program, &ids, &mut colour, &mut Vec::new(), errors);
        }
    }
}

fn check_references(program: &CompiledProgram, errors: &mut Vec<RivetError>) {
    use crate::domain::ir::{EffectKind, Stmt};
    // With modules, a file may reference only its own connectors and auth
    // profiles (they stay inside the module, PROP-2026-0002 R20).
    fn visible(program: &CompiledProgram, decl_file: &str, from: &str) -> bool {
        program.modules.is_empty() || decl_file == from
    }
    fn walk(body: &[Stmt], program: &CompiledProgram, errors: &mut Vec<RivetError>) {
        for s in body {
            let (forms, children): (Vec<&crate::domain::ir::EffectForm>, Vec<&[Stmt]>) = match s {
                Stmt::Assign {
                    rhs: crate::domain::ir::Rhs::Effect(f),
                    ..
                } => (vec![f], vec![]),
                Stmt::Effect { form, .. } => (vec![form], vec![]),
                Stmt::With { form, body, .. } => (vec![form], vec![body.as_slice()]),
                Stmt::If {
                    then, otherwise, ..
                } => (vec![], vec![then.as_slice(), otherwise.as_slice()]),
                Stmt::Try { body, handler, .. } => {
                    (vec![], vec![body.as_slice(), handler.as_slice()])
                }
                Stmt::For { body, .. }
                | Stmt::While { body, .. }
                | Stmt::Iterate { body, .. }
                | Stmt::Scope { body, .. } => (vec![], vec![body.as_slice()]),
                Stmt::Concurrent { tasks, .. } => {
                    (vec![], tasks.iter().map(|(_, b)| b.as_slice()).collect())
                }
                _ => (vec![], vec![]),
            };
            for f in forms {
                for o in f.options_named("auth") {
                    if let Some(name) = o.first_word()
                        && !program
                            .auth_profile(name)
                            .is_some_and(|p| visible(program, &p.span.file, &o.span.file))
                    {
                        errors.push(RivetError::syntax(
                            "check.unknown_auth_profile",
                            format!("unknown auth profile `{name}`"),
                            Some(o.span.clone()),
                        ));
                    }
                }
                let grpc_connector = (f.kind == EffectKind::Grpc)
                    .then(|| crate::domain::grpc::method_ref(&f.head))
                    .flatten()
                    .map(|(connector, _)| connector);
                if let Some(connector) = grpc_connector.filter(|c| {
                    !program
                        .connector(c)
                        .is_some_and(|d| visible(program, &d.span.file, &f.span.file))
                }) {
                    errors.push(RivetError::syntax(
                        "check.unknown_connector",
                        format!("unknown connector `{connector}`"),
                        f.head.first().map(|a| a.span().clone()),
                    ));
                }
            }
            for c in children {
                walk(c, program, errors);
            }
        }
    }
    for op in &program.operations {
        walk(&op.body, program, errors);
    }
    for c in &program.connectors {
        for o in c.options.iter().filter(|o| o.key == "auth") {
            if let Some(name) = o.first_word()
                && !program
                    .auth_profile(name)
                    .is_some_and(|p| visible(program, &p.span.file, &c.span.file))
            {
                errors.push(RivetError::syntax(
                    "check.unknown_auth_profile",
                    format!("unknown auth profile `{name}`"),
                    Some(o.span.clone()),
                ));
            }
        }
    }
}
