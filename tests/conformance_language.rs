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

fn runtime(text: &str) -> rivet::Runtime {
    rivet::Runtime::builder()
        .source("app.rivet", text, ".")
        .build()
        .unwrap()
}

// vhco:test language.compile_program -- INC-2026-0009: numeric path segments (`xs.0`, `m.rows.1.0`, a global's `retry_on.0`) parse in return, object, list, call, condition and assignment positions and evaluate as list indexes
#[tokio::test]
async fn inc_2026_0009_numeric_index_paths_parse_and_evaluate() {
    let text = "global retry_on = [429, 503]\nglobal first = retry_on.0\n\noperation a.b\n    output json\n    xs = [10, 20, 30]\n    m = {rows: [[1, 2], [3, 4]]}\n    y = xs.2\n    total = 0\n    if xs.0 > 5\n        total = xs.0 + xs.1\n    end\n    return {a: xs.0, b: [xs.1, y], c: m.rows.1.0, d: (length m.rows.0), e: retry_on.1, f: first, t: total, s: \"${xs.1}\"}\nend\n";
    let c = runtime(text)
        .request(
            "a.b",
            rivet::domain::Value::object::<[(&str, rivet::domain::Value); 0], &str>([]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        c.result.to_json(),
        serde_json::json!({"a": 10, "b": [20, 30], "c": 3, "d": 2, "e": 503, "f": 429, "t": 30, "s": "20"})
    );
}

// vhco:test language.compile_program -- INC-2026-0009: an out-of-range index is value.missing_key at the path's span; floats (`1.5`) still lex as numbers
#[tokio::test]
async fn inc_2026_0009_out_of_range_index_is_a_typed_error() {
    let text = "operation a.b\n    output json\n    xs = [1.5, 2]\n    return xs.5\nend\n";
    let c = runtime(text)
        .request(
            "a.b",
            rivet::domain::Value::object::<[(&str, rivet::domain::Value); 0], &str>([]),
            None,
        )
        .await;
    let e = c.expect_err("index 5 of a two-item list");
    assert_eq!(e.code, "value.missing_key");
    assert!(
        e.message.contains("index 5 is out of range for `xs`"),
        "{}",
        e.message
    );
    let s = e.source.expect("span");
    assert_eq!((s.start_line, s.start_col, s.end_col), (4, 12, 16));
    let ok = runtime("operation a.b\n    output json\n    xs = [1.5, 2]\n    return xs.0\nend\n")
        .request(
            "a.b",
            rivet::domain::Value::object::<[(&str, rivet::domain::Value); 0], &str>([]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(ok.result.to_json(), serde_json::json!(1.5));
}

// vhco:test language.compile_program -- INC-2026-0009: a keyword statement whose value does not parse is syntax.expression naming the keyword, never the internal `assign_map`
#[test]
fn inc_2026_0009_bad_value_names_the_statement_not_assign_map() {
    for (line, kw) in [("return {x: }", "return"), ("emit [1,", "emit")] {
        let text = format!("operation a.b\n    output json\n    {line}\n    return 1\nend\n");
        let e = compile(&text).unwrap_err();
        assert_eq!(e.code, "syntax.expression", "{line}: {e:?}");
        assert_eq!(
            e.message,
            format!("the expression after `{kw}` does not parse")
        );
        assert!(!e.message.contains("assign_map"));
        let s = e.source.expect("span");
        assert_eq!((s.start_line, s.start_col), (3, 5));
    }
}
