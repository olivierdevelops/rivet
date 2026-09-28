//! T-01 — language: compilation, header rules, duplicates, prefix-call and cycle checks.
#![allow(clippy::result_large_err)]

use rivet::domain::RivetResult;
use rivet::domain::ir::CompiledProgram;
use rivet::domain::ir::{Rhs, Stmt};
use rivet::domain::source::SourceBundle;
use rivet::features::language::compile_program::compile_program;
use rivet::infra::capy_parser::CapyParser;

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
    assert_eq!(add.params[1].default, Some(rivet::domain::Value::Int(0)));
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
    let err = compile(
        "operation a.b\n    output json\n    x = 1\n    description \"late\"\n    return x\nend\n",
    )
    .unwrap_err();
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
