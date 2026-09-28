//! T-23 — syntax: Capy prefix calls with trailing objects and `allow`
//! blocks, quoted durations, `${dotted.path}` interpolation, string escapes,
//! component-aware URL interpolation, the leading-options rule, `yield` vs
//! `return`, header order, did-you-mean, indentation and the f-call style
//! rejection. Every diagnostic is checked at its exact line and column
//! (1-based; columns count bytes, as Capy reports them).
//!
//! ```text
//!  source text ──Capy──▶ SyntaxTree ──lower──▶ IR ──check──▶ CompiledProgram
//!                   │               │                 │
//!                   └─ syntax.indent / unknown_statement (did-you-mean)
//!                                   └─ duration_unquoted, interpolation, escape,
//!                                      option_after_body, header_order, yield, fcall_style
//! ```
#![allow(clippy::result_large_err)]

#[path = "p3_support/mod.rs"]
mod support;

use rivet::domain::ir::{Expr, Rhs, Stmt};
use rivet::domain::{ErrorKind, Value};
use support::*;

/// Wrap body lines in `operation t.run` / `output json` (4-space indented):
/// body line 1 is source line 3.
fn op(body: &str) -> String {
    let indented: String = body.lines().map(|l| format!("    {l}\n")).collect();
    format!("operation t.run\n    output json\n{indented}end\n")
}

fn err(text: &str) -> rivet::domain::RivetError {
    match compile(text) {
        Ok(_) => panic!("expected a compile error for:\n{text}"),
        Err(e) => e,
    }
}

async fn run(src: &str, params: Value) -> Result<Value, rivet::domain::RivetError> {
    let rt = runtime(src, ".", r#"{"version":1}"#);
    rt.request("t.run", params, None).await.map(|c| c.result)
}

// vhco:test language.compile_program -- G30: infix inside objects, lists and call arguments parses AND evaluates (`{n: n - 1}`, `[a + 1]`, `(request "x" {v: a * 2})`)
#[tokio::test]
async fn infix_inside_objects_lists_and_call_arguments_evaluates() {
    let src = "operation t.run\n    param a integer required\n    output json\n    o = {n: a - 1, big: a > 2 and true}\n    l = [a + 1, a * a, (length [a, a + 1])]\n    c = (request \"t.two\" {v: a * 2})\n    return {o: o, l: l, c: c}\nend\n\noperation t.two\n    param v integer required\n    output json\n    return {twice: v, half: v / 2}\nend\n";
    let p = compile(src).unwrap();
    assert_eq!(
        p.operation("t.run").unwrap().calls,
        vec!["t.two".to_string()]
    );
    let v = run(src, Value::object([("a", Value::Int(3))]))
        .await
        .unwrap()
        .to_json();
    assert_eq!(
        v,
        serde_json::json!({
            "o": {"n": 2, "big": true},
            "l": [4, 9, 2],
            "c": {"twice": 6, "half": 3}
        })
    );
}

// vhco:test language.compile_program -- G3: `(len x)` fails at compile time with check.unknown_function at the name and a did-you-mean
#[test]
fn unknown_function_fails_compile_with_did_you_mean() {
    let e = err(&op("x = [1, 2]\nn = (len x)\nreturn n"));
    assert_eq!(e.code, "check.unknown_function");
    assert_eq!(e.kind, ErrorKind::Syntax);
    assert_eq!(e.hint.as_deref(), Some("did you mean `length`?"));
    assert_eq!(span(&e), (4, 10, 4, 13));
}

// vhco:test language.compile_program -- a prefix call keeps named arguments as one trailing object and a literal `(request "ID" {…})` joins the call graph
#[tokio::test]
async fn prefix_calls_with_trailing_objects() {
    let src = "operation t.run\n    output json\n    n = (length [1, 2, 3])\n    x = (request \"t.two\" {value: n, tags: [\"a\"]})\n    return x\nend\n\noperation t.two\n    param value integer required\n    param tags json required\n    output json\n    return {value: value, tags: tags}\nend\n";
    let p = compile(src).unwrap();
    let run_op = p.operation("t.run").unwrap();
    assert_eq!(run_op.calls, vec!["t.two".to_string()]);
    let Stmt::Assign {
        rhs: Rhs::Expr(Expr::Call { func, args, .. }),
        ..
    } = &run_op.body[1]
    else {
        panic!("expected a prefix call, got {:?}", run_op.body[1]);
    };
    assert_eq!(func, "request");
    assert_eq!(args.len(), 2, "ID plus ONE trailing object");
    assert!(matches!(&args[1], Expr::Object(pairs) if pairs.len() == 2));
    let v = run(src, Value::Null).await.unwrap();
    assert_eq!(
        v,
        Value::object([
            ("value", Value::Int(3)),
            ("tags", Value::List(vec![Value::text("a")]))
        ])
    );
}

// vhco:test language.compile_program -- a call followed by an `allow [...]` block keeps the block after the closing paren; the dynamic target must be listed
#[tokio::test]
async fn allow_block_after_a_dynamic_call() {
    let src = "operation t.run\n    param target text required\n    output json\n    v = (request target {n: 2})\n        allow [\"t.double\"]\n    end\n    return v\nend\n\noperation t.double\n    param n integer required\n    output integer\n    return n * 2\nend\n\noperation t.other\n    param n integer required\n    output integer\n    return n\nend\n";
    let p = compile(src).unwrap();
    let Stmt::Assign { options, .. } = &p.operation("t.run").unwrap().body[0] else {
        panic!("assignment expected");
    };
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].key, "allow");
    assert_eq!(
        (options[0].span.start_line, options[0].span.start_col),
        (5, 9)
    );
    let ok = run(src, Value::object([("target", Value::text("t.double"))]))
        .await
        .unwrap();
    assert_eq!(ok, Value::Int(4));
    let e = run(src, Value::object([("target", Value::text("t.other"))]))
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Permission, "permission.denied")
    );
    assert_eq!(span(&e).0, 4, "the refusal points at the call line");
}

// vhco:test language.compile_program -- durations are quoted: `timeout 10s` / `dag timeout 5s` are syntax.duration_unquoted at the bare token with a quoted hint; a malformed quoted duration is syntax.duration
#[test]
fn quoted_versus_unquoted_durations() {
    let e = err(&op(
        "r = http get \"http://127.0.0.1:1/\"\n    timeout 10s\nend\nreturn r",
    ));
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Syntax, "syntax.duration_unquoted")
    );
    assert_eq!(span(&e), (4, 17, 4, 20));
    assert_eq!(e.hint.as_deref(), Some("write \"10s\""));
    assert_eq!((e.exit_code(), e.http_status()), (2, 422));

    let e = err(&op("dag limit 2 timeout 5s\n    node a = 1\nend\nreturn a"));
    assert_eq!(e.code, "syntax.duration_unquoted");
    assert_eq!(span(&e), (3, 25, 3, 27));

    let e = err(&op("dag timeout \"5sec\"\n    node a = 1\nend\nreturn a"));
    assert_eq!(e.code, "syntax.duration");
    assert_eq!((span(&e).0, span(&e).1), (3, 17));

    for ok in ["100ms", "5s", "2m", "1h"] {
        compile(&op(&format!(
            "dag timeout \"{ok}\"\n    node a = 1\nend\nreturn a.result"
        )))
        .unwrap_or_else(|e| panic!("\"{ok}\" is a duration: {e}"));
    }
}

// vhco:test language.compile_program -- `${a.b}` interpolates a dotted path; `${a + b}` is syntax.interpolation at the placeholder
#[tokio::test]
async fn interpolation_takes_dotted_paths_only() {
    let v = run(
        &op("a = {b: {c: 7}}\nname = \"Ada\"\nreturn \"v=${a.b.c}, ${name}!\""),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("v=7, Ada!"));

    let e = err(&op("a = 1\nb = 2\nreturn \"v=${a + b}\""));
    assert_eq!(e.code, "syntax.interpolation");
    assert_eq!(span(&e), (5, 15, 5, 23));
    assert!(e.message.contains("${a + b}"), "{}", e.message);

    let e = err(&op("return \"open ${a.b\""));
    assert_eq!(e.code, "syntax.interpolation");
    assert_eq!((span(&e).0, span(&e).1), (3, 18));
}

// vhco:test language.compile_program -- escapes: \x00, \t, \", \\ and \u00e9 decode; \0 is syntax.escape with a `write \x00` hint
#[tokio::test]
async fn string_escapes() {
    let v = run(
        &op("return [\"\\x00\", \"a\\tb\", \"q\\\"q\", \"b\\\\s\", \"\\u00e9\", \"\\x41\"]"),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(
        v,
        Value::List(vec![
            Value::text("\u{0}"),
            Value::text("a\tb"),
            Value::text("q\"q"),
            Value::text("b\\s"),
            Value::text("é"),
            Value::text("A"),
        ])
    );

    let e = err(&op("return \"\\0\""));
    assert_eq!(e.code, "syntax.escape");
    assert_eq!(span(&e), (3, 13, 3, 15));
    assert_eq!(e.hint.as_deref(), Some("write \\x00"));

    let e = err(&op("return \"\\q\""));
    assert_eq!(e.code, "syntax.escape");
    assert_eq!(e.hint.as_deref(), Some("remove the backslash"));

    let e = err(&op("return \"\\xZ1\""));
    assert_eq!(e.code, "syntax.escape");
    assert_eq!((span(&e).0, span(&e).1), (3, 13));
}

// vhco:test language.compile_program -- regression: an escape or number followed by a multi-byte character reports a diagnostic instead of panicking on a char boundary
#[test]
fn multibyte_characters_in_diagnostics_do_not_panic() {
    let e = err(&op("return \"\\é\""));
    assert_eq!(e.code, "syntax.escape");
    assert_eq!(span(&e), (3, 13, 3, 16));

    let e = err(&op("return \"\\x€\""));
    assert_eq!(e.code, "syntax.escape");

    let e = err(&op("x = [10é]\nreturn x"));
    assert_eq!(e.code, "syntax.duration_unquoted");
    assert_eq!(span(&e), (3, 10, 3, 14));
}

// vhco:test transports.exchange_http -- `${x}` in a path segment stays ONE encoded segment (`/ ? # @ ..` never add structure); in a query value it is one encoded query component; a bare `..` is refused
#[tokio::test]
async fn url_interpolation_is_component_aware() {
    let (port, _) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!(
        "operation t.run\n    param seg text required\n    param q text required\n    output json\n    r = http get \"{base}/echo/${{seg}}/tail?q=${{q}}&fixed=1\"\n        decode json\n    end\n    return r.body.target\nend\n"
    );
    let rt = runtime(&src, ".", &net_policy(&base));
    let call = |seg: &str, q: &str| {
        rt.request(
            "t.run",
            Value::object([("seg", Value::text(seg)), ("q", Value::text(q))]),
            None,
        )
    };
    let v = call("a/b?c#d@e..f", "x&y=z#w").await.unwrap().result;
    assert_eq!(
        v,
        Value::text("/echo/a%2Fb%3Fc%23d%40e..f/tail?q=x%26y%3Dz%23w&fixed=1")
    );
    let v = call("../admin", "../..").await.unwrap().result;
    assert_eq!(v, Value::text("/echo/..%2Fadmin/tail?q=..%2F..&fixed=1"));
    for dots in ["..", "."] {
        let e = call(dots, "q").await.unwrap_err();
        assert_eq!(
            (e.kind, e.code.as_str()),
            (ErrorKind::Validation, "validation.url_segment")
        );
    }
}

// vhco:test language.compile_program -- the leading-options rule: an option line after a body statement is syntax.option_after_body at that option line
#[test]
fn option_after_body() {
    let e = err(&op(
        "r = http get \"http://127.0.0.1:1/\"\n    decode json\n    x = 1\n    timeout \"1s\"\nend\nreturn r",
    ));
    assert_eq!(e.code, "syntax.option_after_body");
    assert_eq!(span(&e), (6, 9, 6, 21));
    assert!(e.message.contains("timeout"), "{}", e.message);

    // A resource block: `with http … as NAME` options must also lead.
    let e = err(&op(
        "with http get \"http://127.0.0.1:1/\" as s\n    stream lines\n    for l in s\n        break\n    end\n    decode text\nend\nreturn 1",
    ));
    assert_eq!(e.code, "syntax.option_after_body");
    assert_eq!((span(&e).0, span(&e).1), (8, 9));
}

// vhco:test language.compile_program -- header lines keep the order name, description, private, param, output, emits, receives, error; a header line after the body is syntax.option_after_body
#[test]
fn header_order_and_late_header() {
    let e = err("operation t.run\n    output json\n    description \"early\"\n    return 1\nend\n");
    assert_eq!(e.code, "syntax.header_order");
    assert_eq!(span(&e), (3, 5, 3, 24));

    let e = err(
        "operation t.run\n    output json\n    x = 1\n    description \"late\"\n    return x\nend\n",
    );
    assert_eq!(e.code, "syntax.option_after_body");
    assert_eq!(span(&e), (4, 5, 4, 23));

    let e = err(
        "operation t.run\n    param a integer required\n    name \"Late name\"\n    output json\n    return a\nend\n",
    );
    assert_eq!(e.code, "syntax.header_order");
    assert_eq!((span(&e).0, span(&e).1), (3, 5));

    compile("operation t.run\n    name \"N\"\n    description \"D\"\n    private true\n    param a integer required\n    output json\n    emits json\n    error \"t.bad\" description \"Bad.\"\n    return a\nend\n")
        .expect("the canonical header order compiles");
}

// vhco:test language.compile_program -- `yield` is only valid inside map/poll (syntax.yield); a map body must yield (syntax.map_yield); `return` inside map exits the whole operation
#[tokio::test]
async fn yield_versus_return() {
    let e = err(&op("yield 1"));
    assert_eq!(e.code, "syntax.yield");
    assert_eq!(span(&e), (3, 5, 3, 12));

    let e = err(&op("r = map i in [1, 2] limit 1\n    x = i\nend\nreturn r"));
    assert_eq!(e.code, "syntax.map_yield");
    assert_eq!(span(&e), (3, 5, 5, 8));

    let v = run(
        &op("r = map i in [1, 2, 3] limit 2\n    yield i * 10\nend\nreturn r"),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(
        v,
        Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)])
    );

    let v = run(
        &op("r = map i in [1, 2, 3] limit 1\n    if i == 2\n        return \"left the operation\"\n    end\n    yield i\nend\nreturn r"),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("left the operation"));
    // Regression: the interpreter used to refuse `return` in map/poll bodies at run time.
    let v = run(
        &op("r = poll every \"10ms\" timeout \"2s\"\n    return \"left from poll\"\n    until true\nend\nreturn r"),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("left from poll"));

    let v = run(
        &op("n = 0\nr = poll every \"10ms\" timeout \"2s\"\n    n = n + 1\n    until n == 3\n    yield {rounds: n}\nend\nreturn r"),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(v.get("rounds"), Some(&Value::Int(3)));
}

// vhco:test language.compile_program -- an unknown statement keyword is syntax.unknown_statement at the keyword with a did-you-mean hint
#[test]
fn did_you_mean_for_keyword_typos() {
    let e = err("operaton t.run\n    output json\n    return 1\nend\n");
    assert_eq!(e.code, "syntax.unknown_statement");
    assert_eq!((span(&e).0, span(&e).1), (1, 1));
    assert_eq!(e.hint.as_deref(), Some("did you mean `operation`?"));

    for (typo, want) in [("retrun 1", "return"), ("iff true\n    x = 1\nend", "if")] {
        let e = err(&op(&format!("{typo}\nreturn 2")));
        assert_eq!(e.code, "syntax.unknown_statement", "{typo}");
        assert_eq!((span(&e).0, span(&e).1), (3, 5), "{typo}");
        assert_eq!(
            e.hint.as_deref(),
            Some(format!("did you mean `{want}`?").as_str()),
            "{typo}"
        );
    }
}

// vhco:test language.compile_program -- two-space indentation is syntax.indent on every offending line; continuation lines inside brackets are exempt
#[test]
fn two_space_indentation_is_rejected() {
    let e = err("operation t.run\n  output json\n  return 1\nend\n");
    let all = all_errors(&e);
    let indents: Vec<_> = all
        .iter()
        .filter(|x| x.code == "syntax.indent")
        .map(span)
        .collect();
    assert_eq!(indents, vec![(2, 1, 2, 3), (3, 1, 3, 3)]);
    compile(
        "operation t.run\n    output json\n    return {\n      a: 1,\n       b: 2\n    }\nend\n",
    )
    .expect("bracket continuations are free-form");
    compile("operation t.run\n\toutput json\n\treturn 1\nend\n").expect("one tab per level");
}

// vhco:test language.compile_program -- `request("id", {…})` is syntax.fcall_style at the argument list with a prefix-call hint
#[test]
fn fcall_style_is_rejected() {
    let e = err(&op("x = request(\"t.other\", {})\nreturn x"));
    assert_eq!(e.code, "syntax.fcall_style");
    assert_eq!(span(&e), (3, 16, 3, 31));
    assert!(
        e.hint
            .as_deref()
            .unwrap()
            .contains("(request \"id\" {k: v})")
    );
    let e = err(&op("n = length([1, 2])\nreturn n"));
    assert_eq!(e.code, "syntax.fcall_style");
    assert_eq!((span(&e).0, span(&e).1), (3, 15));
}

// vhco:test language.compile_program -- the CLI renders a syntax diagnostic as FILE:LINE:COL with a caret and exits 2; --json prints the ErrorEnvelope with the same span
#[test]
fn cli_renders_the_exact_location() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        op("r = http get \"http://127.0.0.1:1/\"\n    timeout 10s\nend\nreturn r"),
    )
    .unwrap();
    let r = rivet(dir.path(), &["--file", "app.rivet", "check"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(
        r.stderr.contains("error[syntax.duration_unquoted]"),
        "{}",
        r.stderr
    );
    assert!(r.stderr.contains("--> app.rivet:4:17"), "{}", r.stderr);
    assert!(r.stderr.contains("^^^"), "{}", r.stderr);
    let r = rivet(
        dir.path(),
        &[
            "--json",
            "--file",
            "app.rivet",
            "request",
            "t.run",
            "--data",
            "{}",
        ],
    );
    assert_eq!(r.code, 2);
    let e = r.error();
    assert_eq!(e["kind"], "syntax");
    assert_eq!(e["code"], "syntax.duration_unquoted");
    assert_eq!(e["source"]["line"], 4);
    assert_eq!(e["source"]["column"], 17);
}

// vhco:test language.compile_program -- G34: `if COND … else … end` pairs the else section with its `if` (nested pairs stay with their own `if`) and lowers into Stmt::If.otherwise
#[test]
fn if_else_pairs_into_otherwise() {
    let p = compile(&op(
        "x = 1\nif x > 0\n    if x > 5\n        y = \"big\"\n    else\n        y = \"small\"\n    end\nelse\n    y = \"neg\"\nend\nif x == 1\n    z = 1\nend\nreturn y",
    ))
    .unwrap();
    let body = &p.operation("t.run").unwrap().body;
    let Stmt::If {
        then, otherwise, ..
    } = &body[1]
    else {
        panic!("expected if, got {:?}", body[1]);
    };
    assert_eq!(then.len(), 1);
    assert_eq!(otherwise.len(), 1, "outer else holds one assignment");
    let Stmt::If {
        then: inner_then,
        otherwise: inner_else,
        ..
    } = &then[0]
    else {
        panic!("expected nested if");
    };
    assert_eq!((inner_then.len(), inner_else.len()), (1, 1));
    let Stmt::If { otherwise, .. } = &body[2] else {
        panic!("expected plain if");
    };
    assert!(
        otherwise.is_empty(),
        "an `if` without else keeps an empty otherwise"
    );
}

// vhco:test language.compile_program -- G34: an orphan `else`, a second `else` and `else if COND` are syntax errors at the `else` line
#[test]
fn orphan_and_duplicate_else_are_rejected() {
    let e = err(&op("x = 1\nelse\n    x = 2\nend\nreturn x"));
    assert_eq!(e.code, "syntax.else_without_if");
    assert_eq!((span(&e).0, span(&e).1), (4, 5));

    let e = err(&op(
        "x = 1\nwhile x > 1\n    x = 0\nelse\n    x = 2\nend\nreturn x",
    ));
    assert!(
        all_errors(&e)
            .iter()
            .any(|x| x.code == "syntax.else_without_if" && span(x).0 == 6),
        "{e:?}"
    );

    let e = err(&op(
        "x = 1\nif x\n    y = 1\nelse\n    y = 2\nelse\n    y = 3\nend\nreturn y",
    ));
    assert!(
        all_errors(&e)
            .iter()
            .any(|x| x.code == "syntax.else_without_if"),
        "{e:?}"
    );

    let e = err(&op(
        "x = 1\nif x > 2\n    y = 1\nelse if x > 1\n    y = 2\nend\nreturn y",
    ));
    assert!(
        all_errors(&e).iter().any(|x| x.code == "syntax.else_if"),
        "{e:?}"
    );
}

// vhco:test execution.request_operation -- G34: the interpreter runs exactly one branch of `if … else … end` (nested else included)
#[tokio::test]
async fn if_else_runs_one_branch() {
    let src = "operation t.run\n    param n integer required\n    output text\n    if n > 0\n        if n > 5\n            r = \"big\"\n        else\n            r = \"small\"\n        end\n    else\n        r = \"not positive\"\n    end\n    return r\nend\n";
    for (n, want) in [(9, "big"), (3, "small"), (-1, "not positive")] {
        let v = run(src, Value::object([("n", Value::Int(n))]))
            .await
            .unwrap();
        assert_eq!(v, Value::text(want), "n = {n}");
    }
}
