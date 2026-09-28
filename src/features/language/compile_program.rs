use super::lower::Lowerer;
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
        // vhco:todo compile_catalog -- lower every top-level `operation`/`pipeline` into one Operation (ID, display name, description, described params, output, emits, receives, declared errors, body) and every `connector`/`auth NAME oauth2` into a Declaration; file basenames never prefix IDs
        // vhco:step lower lowerer.lower_tree -- header order, leading-options rule, try/catch pairing, effect forms, quoted durations and prefix calls are enforced here with exact spans
        lowerer.lower_tree(
            &tree,
            &mut program.operations,
            &mut program.connectors,
            &mut program.auth_profiles,
        );
    }

    // vhco:todo check_program -- reject duplicate operation IDs across the whole bundle (registry.duplicate_id, both spans), literal `(request "id" …)` calls to operations that do not exist and are not connector-imported, and static call cycles over literal targets (check.call_cycle); nothing executes
    // vhco:step duplicates check_duplicates -- first declaration wins the span; the second is the error location
    check_duplicates(&program, &mut lowerer.errors);
    // vhco:step calls check_calls -- unknown literal targets and cycles in the literal call graph
    check_calls(&program, &mut lowerer.errors);

    // vhco:todo check_auth_transport -- every `auth PROFILE account …` and `grpc CONNECTOR.Method` reference names a declared auth profile or connector; unknown names fail compilation before any request
    // vhco:step refs check_references -- walk option lines and effect heads for profile/connector names
    check_references(&program, &mut lowerer.errors);

    // vhco:error syntax -- any parse, lowering or check error => kind syntax (exit 2) returns with every other error in `suppressed`
    if !lowerer.errors.is_empty() {
        let mut errors = lowerer.errors;
        let mut first = errors.remove(0);
        first.suppressed = errors;
        return Err(first);
    }
    program.warnings = lowerer.warnings;
    // vhco:step hash sha256 -- the source hash pins requests, sessions and manifests to this exact program
    program.source_hash = source_hash(input);
    Ok(program)
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
        if let Some(first) = program.operations[..i].iter().find(|o| o.id == op.id) {
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
    for op in &program.operations {
        for call in &op.calls {
            if !known(call) {
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
                    if let Some(name) = o.first_word() {
                        if program.auth_profile(name).is_none() {
                            errors.push(RivetError::syntax(
                                "check.unknown_auth_profile",
                                format!("unknown auth profile `{name}`"),
                                Some(o.span.clone()),
                            ));
                        }
                    }
                }
                if f.kind == EffectKind::Grpc {
                    if let Some(crate::domain::ir::Arg::Expr(
                        crate::domain::ir::Expr::Path(p, span),
                        _,
                    )) = f.head.first()
                    {
                        if program.connector(&p[0]).is_none() {
                            errors.push(RivetError::syntax(
                                "check.unknown_connector",
                                format!("unknown connector `{}`", p[0]),
                                Some(span.clone()),
                            ));
                        }
                    }
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
            if let Some(name) = o.first_word() {
                if program.auth_profile(name).is_none() {
                    errors.push(RivetError::syntax(
                        "check.unknown_auth_profile",
                        format!("unknown auth profile `{name}`"),
                        Some(o.span.clone()),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ir::{Rhs, Stmt};
    use crate::infra::capy_parser::CapyParser;

    fn compile(text: &str) -> RivetResult<CompiledProgram> {
        compile_program(
            &SourceBundle::single("app.rivet", text),
            &CapyParser::new().unwrap(),
        )
    }

    // vhco:test language.compile_program -- the 01-catalog bundle compiles into three described operations
    #[test]
    fn compiles_the_catalog_demo() {
        let text = std::fs::read_to_string("docs/demos/01-catalog/app.rivet").unwrap();
        let p = compile(&text).unwrap();
        let ids: Vec<_> = p.operations.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"demo.add"));
        let add = p.operation("demo.add").unwrap();
        assert_eq!(add.params.len(), 2);
        assert!(add.params[0].required);
        assert_eq!(add.params[1].default, Some(crate::domain::Value::Int(0)));
        assert!(matches!(
            add.body[0],
            Stmt::Return {
                value: Rhs::Expr(_),
                ..
            }
        ));
    }

    #[test]
    fn duplicate_ids_fail_with_both_spans() {
        let err = compile("operation a.b\n    output json\n    return 1\nend\n\noperation a.b\n    output json\n    return 2\nend\n").unwrap_err();
        assert_eq!(err.code, "registry.duplicate_id");
        assert_eq!(err.source.unwrap().start_line, 6);
    }

    #[test]
    fn fcall_style_is_rejected_with_a_prefix_hint() {
        let err = compile(
            "operation a.b\n    output json\n    x = request(\"a.b\", {})\n    return x\nend\n",
        )
        .unwrap_err();
        assert_eq!(err.code, "syntax.fcall_style");
        assert!(err.hint.unwrap().contains("(request"));
    }

    #[test]
    fn header_after_body_is_option_after_body() {
        let err = compile("operation a.b\n    output json\n    x = 1\n    description \"late\"\n    return x\nend\n").unwrap_err();
        assert_eq!(err.code, "syntax.option_after_body");
    }

    #[test]
    fn literal_call_cycles_are_rejected() {
        let err = compile(
            "operation a.one\n    output json\n    return (request \"a.two\" {})\nend\n\noperation a.two\n    output json\n    return (request \"a.one\" {})\nend\n",
        )
        .unwrap_err();
        assert!(
            std::iter::once(&err)
                .chain(err.suppressed.iter())
                .any(|e| e.code == "check.call_cycle")
        );
    }
}
