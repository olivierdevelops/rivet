//! T-06–T-08 — global constants (PROP-2026-0002 R7–R9, UC-04):
//! `global NAME = EXPR` evaluated once at load, read-only in every operation,
//! every check code with its exact span, and exact manifest targets and grants.
//!
//! ```text
//!  T-06  values visible everywhere; lookup locals → params → globals; concurrent requests equal
//!  T-07  syntax.global · check.global_{not_constant,forward_ref,duplicate,shadow,assign}  (line:col, exit 2)
//!  T-08  literals + globals ⇒ `exact` targets; `policy generate` grants them exactly
//! ```
#![allow(clippy::result_large_err)]

use rivet::Runtime;
use rivet::domain::io_manifest::{IoQuery, Knowledge};
use rivet::domain::ir::CompiledProgram;
use rivet::domain::policy::Capability;
use rivet::domain::source::SourceBundle;
use rivet::domain::{RivetError, RivetResult, Value};
use rivet::features::language::compile_program::compile_program;
use rivet::infra::capy_parser::CapyParser;
use std::process::Command;

fn compile(text: &str) -> RivetResult<CompiledProgram> {
    compile_program(
        &SourceBundle::single("app.rivet", text),
        &CapyParser::new().unwrap(),
    )
}

fn rt(text: &str) -> Runtime {
    Runtime::builder()
        .source("app.rivet", text, ".")
        .build()
        .unwrap()
}

/// Every diagnostic of a failed compile (first + suppressed).
fn all(e: &RivetError) -> Vec<&RivetError> {
    std::iter::once(e).chain(e.suppressed.iter()).collect()
}

/// The (line, column, end column) of the one diagnostic with `code`.
fn at(text: &str, code: &str) -> (u32, u32, u32) {
    let e = compile(text).unwrap_err();
    let found: Vec<_> = all(&e).into_iter().filter(|d| d.code == code).collect();
    assert_eq!(found.len(), 1, "{code}: {:?}", all(&e));
    let s = found[0].source.clone().expect("span");
    assert_eq!(s.file, "app.rivet");
    assert_eq!(found[0].exit_code(), 2);
    (s.start_line, s.start_col, s.end_col)
}

const APP: &str = r#"global api        = "https://api.example.com"
global users_url  = "${api}/users"
global page_size  = 50
global retry_on   = [429, 503]
global headers    = {accept: "application/json"}
global data_dir   = "./data"
global double     = page_size * 2

operation users.get
    param id integer required min 1
    output json
    r = http get "${users_url}/${id}"
        header "accept" headers.accept
        decode json
    end
    return r.body
end

operation users.all
    output json
    r = http get users_url
        header "accept" headers.accept
        decode json
    end
    return r.body
end

operation data.read
    output json
    value = file read "${data_dir}/public.json" as json
    return value
end

operation info.show
    param page integer default 1
    output json
    local = page_size + page
    items = map n in retry_on limit 2
        yield n + page_size
    end
    return {api: api, url: "${users_url}?limit=${page_size}", h: headers.accept, codes: retry_on, local: local, items: items, double: double}
end

operation info.nested
    output json
    return (request "info.show" {page: 2})
end
"#;

// vhco:test language.compile_globals -- globals evaluate once in declaration order (interpolation, arithmetic, lists, objects, pure built-ins) into one frozen scope per file
#[test]
fn t06_globals_evaluate_in_declaration_order() {
    let p = compile(
        "global api = \"https://api.example.com\"\nglobal users_url = \"${api}/users\"\nglobal size = 25 * 2\nglobal h = {accept: \"application/json\"}\nglobal n = (length [1, 2, 3])\nglobal b = (base64.encode \"hi\")\nglobal big = size > 10 and n == 3\n\noperation a.b\n    output json\n    return users_url\nend\n",
    )
    .unwrap();
    let scope = p.global_scope("app.rivet").unwrap();
    assert_eq!(
        scope.names,
        ["api", "users_url", "size", "h", "n", "b", "big"]
    );
    assert_eq!(
        scope.get("users_url"),
        Some(&Value::text("https://api.example.com/users"))
    );
    assert_eq!(scope.get("size"), Some(&Value::Int(50)));
    assert_eq!(scope.get("n"), Some(&Value::Int(3)));
    assert_eq!(scope.get("b"), Some(&Value::text("aGk=")));
    assert_eq!(scope.get("big"), Some(&Value::Bool(true)));
    assert_eq!(
        scope.get("h").and_then(|h| h.get("accept")),
        Some(&Value::text("application/json"))
    );
}

// vhco:test language.compile_globals -- every operation (templates, map bodies, nested requests) reads the same frozen values; params and locals are looked up before globals
#[tokio::test]
async fn t06_globals_are_visible_in_every_operation() {
    let r = rt(APP);
    let c = r
        .request("info.show", Value::object([("page", Value::Int(3))]), None)
        .await
        .unwrap();
    let j = c.result.to_json();
    assert_eq!(j["api"], "https://api.example.com");
    assert_eq!(j["url"], "https://api.example.com/users?limit=50");
    assert_eq!(j["h"], "application/json");
    assert_eq!(j["codes"], serde_json::json!([429, 503]));
    assert_eq!(j["local"], 53);
    assert_eq!(j["items"], serde_json::json!([479, 553]));
    assert_eq!(j["double"], 100);
    let nested = r.request("info.nested", Value::Null, None).await.unwrap();
    assert_eq!(nested.result.to_json()["local"], 52);
}

// vhco:test language.compile_globals -- 32 concurrent requests on one runtime read identical global values
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t06_concurrent_requests_see_the_same_values() {
    let r = rt(APP);
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let r = r.clone();
        tasks.push(tokio::spawn(async move {
            r.request("info.show", Value::object([("page", Value::Int(1))]), None)
                .await
                .unwrap()
                .result
        }));
    }
    let mut seen = Vec::new();
    for t in tasks {
        seen.push(t.await.unwrap());
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]));
    assert_eq!(seen[0].to_json()["double"], 100);
}

// vhco:test language.compile_globals -- syntax.global: a line without `=`, a bad name, or `global` inside an operation, each at its span (exit 2)
#[test]
fn t07_syntax_global() {
    assert_eq!(
        at(
            "global api\n\noperation a.b\n    output json\n    return 1\nend\n",
            "syntax.global"
        ),
        (1, 1, 11)
    );
    assert_eq!(at("global 1x = 3\n", "syntax.global"), (1, 8, 10));
    assert_eq!(
        at(
            "operation a.b\n    output json\n    global x = 1\n    return 1\nend\n",
            "syntax.global"
        ),
        (3, 5, 17)
    );
}

// vhco:test language.compile_globals -- check.global_not_constant: env, request, an effect, and a param/local name, each at its span with a hint
#[test]
fn t07_not_constant() {
    // UC-04 sample: the whole right-hand side is underlined.
    let text = "global token = (env \"API_TOKEN\")\n";
    assert_eq!(at(text, "check.global_not_constant"), (1, 16, 33));
    let e = compile(text).unwrap_err();
    assert!(e.message.contains("cannot read the environment"));
    assert!(e.hint.unwrap().contains("secret token from env"));
    assert_eq!(
        at(
            "global x = (request \"a.b\" {})\n",
            "check.global_not_constant"
        ),
        (1, 12, 30)
    );
    assert_eq!(
        at(
            "global page = http get \"https://x\"\n",
            "check.global_not_constant"
        ),
        (1, 15, 35)
    );
    // A name that is not an earlier global (a param or local at request time).
    assert_eq!(
        at("global x = id + 1\n", "check.global_not_constant"),
        (1, 12, 14)
    );
}

// vhco:test language.compile_globals -- check.global_forward_ref: a later global or the global itself, at the reference
#[test]
fn t07_forward_ref() {
    assert_eq!(
        at(
            "global a = b * 2\nglobal b = 1\n",
            "check.global_forward_ref"
        ),
        (1, 12, 13)
    );
    assert_eq!(
        at("global a = a + 1\n", "check.global_forward_ref"),
        (1, 12, 13)
    );
}

// vhco:test language.compile_globals -- check.global_duplicate at the second declaration's name
#[test]
fn t07_duplicate() {
    assert_eq!(
        at("global a = 1\nglobal a = 2\n", "check.global_duplicate"),
        (2, 8, 9)
    );
}

// vhco:test language.compile_globals -- check.global_shadow: a param, loop variable, `with … as`, map item, dag node and secret reusing a global name
#[test]
fn t07_shadow() {
    let g = "global api = \"https://api.example.com\"\n\n";
    let case = |body: &str| format!("{g}operation a.b\n{body}end\n");
    assert_eq!(
        at(
            &case("    param api text\n    output json\n    return 1\n"),
            "check.global_shadow"
        ),
        (4, 11, 14)
    );
    assert_eq!(
        at(
            &case("    output json\n    for api in [1]\n        x = 1\n    end\n    return 1\n"),
            "check.global_shadow"
        ),
        (5, 5, 8)
    );
    assert_eq!(
        at(
            &case(
                "    output json\n    xs = map api in [1]\n        yield 1\n    end\n    return xs\n"
            ),
            "check.global_shadow"
        ),
        (5, 5, 8)
    );
    assert_eq!(
        at(
            &case(
                "    output json\n    secret api from env \"K\" for \"https://api.example.com\"\n    return 1\n"
            ),
            "check.global_shadow"
        ),
        (5, 5, 58)
    );
    let e = compile(&case("    param api text\n    output json\n    return 1\n")).unwrap_err();
    assert!(
        e.message
            .contains("parameter `api` reuses the name of a global")
    );
}

// vhco:test language.compile_globals -- check.global_assign: `NAME = …` and `NAME += …` on a global, at the statement
#[test]
fn t07_assign() {
    let g = "global api = \"https://api.example.com\"\nglobal n = 1\n\n";
    assert_eq!(
        at(
            &format!("{g}operation a.b\n    output json\n    api = \"x\"\n    return api\nend\n"),
            "check.global_assign"
        ),
        (6, 5, 14)
    );
    assert_eq!(
        at(
            &format!("{g}operation a.b\n    output json\n    n += 1\n    return n\nend\n"),
            "check.global_assign"
        ),
        (6, 5, 11)
    );
}

// vhco:test language.compile_globals -- `rivet check` renders check.global_not_constant with the UC-04 span and hint and exits 2
#[test]
fn t07_cli_check_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.rivet");
    std::fs::write(&bad, "global token = (env \"API_TOKEN\")\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(["--file", bad.to_str().unwrap(), "check"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("error[check.global_not_constant]"), "{err}");
    assert!(err.contains("bad.rivet:1:16"), "{err}");
    assert!(err.contains("^^^^^^^^^^^^^^^^^"), "{err}");
}

// vhco:test audit.inspect_effects -- targets built from literals and globals are exact; params keep their placeholder with the global parts resolved
#[test]
fn t08_manifest_substitutes_globals() {
    let r = rt(APP);
    let m = r
        .io(&IoQuery {
            all: true,
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap()
        .manifest;
    let site = |op: &str| m.sites.iter().find(|s| s.operation_id == op).unwrap();
    let all = site("users.all");
    assert_eq!(all.target.template, "https://api.example.com/users");
    assert_eq!(all.knowledge, Knowledge::Exact);
    let get = site("users.get");
    assert_eq!(get.target.template, "https://api.example.com/users/{id}");
    assert_eq!(get.target.host.as_deref(), Some("api.example.com"));
    assert_eq!(get.knowledge, Knowledge::ParamDependent);
    let read = site("data.read");
    assert_eq!(read.target.template, "./data/public.json");
    assert_eq!(read.knowledge, Knowledge::Exact);
}

// vhco:test policy.generate_policy -- `policy generate` turns global-built targets into exact grants (library and CLI)
#[test]
fn t08_policy_generate_grants_exactly() {
    let r = rt(APP);
    let d = r.generate_policy(&["users.all", "data.read"]).unwrap();
    assert!(d.complete && d.review.is_empty());
    let net = d
        .grants
        .iter()
        .find(|g| g.capability == Capability::Network)
        .unwrap();
    assert_eq!(net.targets, vec!["https://api.example.com:443"]);
    let read = d
        .grants
        .iter()
        .find(|g| g.capability == Capability::Read)
        .unwrap();
    assert_eq!(read.targets, vec!["./data/public.json"]);

    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("app.rivet");
    std::fs::write(&app, APP).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(["--file", app.to_str().unwrap(), "io", "--by", "target"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("https://api.example.com:443"), "{text}");
    assert!(!text.contains("{users_url}"), "{text}");
}
