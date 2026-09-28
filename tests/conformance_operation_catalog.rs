//! T-16 — one file, many described operations, one catalog (PROP Increment
//! 12; REF S103–S106, S109, S119, S121–S127). Metadata survives unchanged on
//! every surface; duplicate IDs fail the whole candidate load with both
//! spans; private helpers are callable only in-bundle; `rivet.*` is reserved;
//! `check --strict-docs` reports missing documentation.
//!
//! ```text
//!  catalog.rivet ─compile─▶ RegistryEntry[] ─┬─ CLI list / describe / outputs
//!                                             ├─ HTTP /v1/operations[/{id}[/outputs]]
//!                                             ├─ Rust rt.list / rt.describe / rt.outputs
//!                                             └─ MCP tools/list (title, description, schemas)
//! ```
#![allow(clippy::result_large_err)]

#[path = "support/mod.rs"]
mod serve_support;
#[path = "p3_support/mod.rs"]
mod support;

use rivet::domain::ErrorKind;
use rivet::domain::Value;
use rivet::domain::source::{SourceBundle, SourceFile};
use rivet::features::language::compile_program::compile_program;
use rivet::infra::capy_parser::CapyParser;
use rivet::orchestrator::setup_serve::{ServeOptions, start};
use serde_json::{Value as Json, json};
use support::*;

/// S103 + S121 + S109 in one file.
const CATALOG: &str = r#"operation demo.greet
    name "Greet a person"
    description "Return a greeting for the supplied person."
    param person text required description "The person's display name."
    output text description "Greeting sentence."
    return "Hello, ${person}!"
end

operation demo.add
    name "Add two integers"
    description "Add two signed integers and return their sum."
    param a integer required description "First operand."
    param b integer default 0 description "Second operand; defaults to zero."
    output integer description "Sum of a and b."
    return a + b
end

operation demo.health
    name "Check availability"
    description "Return a constant readiness response without I/O."
    output object description "Readiness report."
        field ready boolean required description "Always true when the runtime answers."
    end
    return {ready: true}
end

operation demo.average
    name "Average two numbers"
    description "Return the arithmetic mean of two numbers."
    param a number required description "First value."
    param b number required description "Second value."
    output number description "Mean of a and b."
    total = a + b
    return total / 2
end

operation helper.normalize
    private true
    param value text required
    output text
    return value
end

operation demo.echo
    description "Return a supplied string through an internal helper."
    param value text required description "Text to return."
    output text description "The same text."
    return (request "helper.normalize" {value: value})
end
"#;

const PUBLIC: [&str; 5] = [
    "demo.add",
    "demo.average",
    "demo.echo",
    "demo.greet",
    "demo.health",
];

fn catalog_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("catalog.rivet"), CATALOG).unwrap();
    dir
}

fn cli_json(dir: &std::path::Path, args: &[&str]) -> Json {
    let mut all = vec!["--json", "--file", "catalog.rivet"];
    all.extend_from_slice(args);
    let r = rivet(dir, &all);
    assert_eq!(r.code, 0, "{args:?}: {}", r.stderr);
    r.json()
}

// vhco:test registry.describe_operations -- S103/S104/S105: one multi-operation file publishes identical metadata through the library, CLI list/describe/outputs, HTTP /v1/operations and MCP tools/list
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metadata_is_identical_on_every_surface() {
    let dir = catalog_dir();
    let d = dir.path().to_path_buf();
    let rt = rivet::Runtime::builder()
        .source("catalog.rivet", CATALOG, ".")
        .build()
        .unwrap();

    // Library: the public catalog in declaration order, the helper hidden.
    let lib = rt.list().unwrap();
    let ids: Vec<&str> = lib.entries.iter().map(|e| e.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(sorted, PUBLIC);

    // CLI list/describe/outputs (subprocess; the fixture server is not needed).
    let list = tokio::task::spawn_blocking({
        let d = d.clone();
        move || {
            let list = cli_json(&d, &["list"]);
            let describe: Vec<Json> = PUBLIC
                .iter()
                .map(|id| cli_json(&d, &["describe", id]))
                .collect();
            let outputs: Vec<Json> = PUBLIC
                .iter()
                .map(|id| cli_json(&d, &["outputs", id]))
                .collect();
            (list, describe, outputs)
        }
    })
    .await
    .unwrap();
    let (cli_list, cli_describe, cli_outputs) = list;
    let listed: Vec<&str> = cli_list["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ids, "CLI list == library list");

    let add = &cli_describe[0];
    assert_eq!(add["id"], "demo.add");
    assert_eq!(add["name"], "Add two integers");
    assert_eq!(
        add["description"],
        "Add two signed integers and return their sum."
    );
    assert_eq!(add["input"]["properties"]["b"]["default"], 0);
    assert_eq!(
        add["input"]["properties"]["a"]["description"],
        "First operand."
    );
    assert_eq!(add["input"]["required"], json!(["a"]));
    assert_eq!(add["output"]["description"], "Sum of a and b.");
    assert_eq!(cli_outputs[1]["output"]["description"], "Mean of a and b.");

    // HTTP and MCP from one serve over the same bundle.
    let h = start(
        rt.clone(),
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = h.addr.unwrap();
    let sid = serve_support::mcp_init(addr, &[]).await;
    let tools = serve_support::mcp_raw(
        addr,
        &sid,
        &[],
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await
    .json()["result"]["tools"]
        .as_array()
        .unwrap()
        .clone();
    for (i, id) in PUBLIC.iter().enumerate() {
        let lib_entry = rt.describe(&[id.to_string()]).unwrap().entries[0].describe_json();
        assert_eq!(
            cli_describe[i], lib_entry,
            "{id}: CLI describe == library describe"
        );
        let http = serve_support::http(addr, "GET", &format!("/v1/operations/{id}"), &[], "").await;
        assert_eq!(http.status, 200);
        assert_eq!(
            http.json(),
            cli_describe[i],
            "{id}: HTTP describe == CLI describe"
        );
        let http = serve_support::http(
            addr,
            "GET",
            &format!("/v1/operations/{id}/outputs"),
            &[],
            "",
        )
        .await;
        assert_eq!(http.status, 200);
        assert_eq!(
            http.json(),
            cli_outputs[i],
            "{id}: HTTP outputs == CLI outputs --json"
        );
        let tool = tools
            .iter()
            .find(|t| t["name"] == *id)
            .unwrap_or_else(|| panic!("{id} is an MCP tool"));
        assert_eq!(tool["title"], cli_describe[i]["name"], "{id}");
        assert_eq!(tool["description"], cli_describe[i]["description"], "{id}");
        assert_eq!(tool["inputSchema"], cli_describe[i]["input"], "{id}");
        assert_eq!(
            tool["outputSchema"]["properties"]["result"], cli_describe[i]["output"],
            "{id}"
        );
    }
    assert!(tools.iter().all(|t| t["name"] != "helper.normalize"));

    // Same results from the library, HTTP and MCP.
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id":"demo.average","params":{"a":2,"b":5}}),
        &[],
    )
    .await;
    assert_eq!(r.json()["result"], 3.5);
    let r = serve_support::mcp_call(addr, &sid, &[], "demo.add", json!({"a":2,"b":3})).await;
    assert_eq!(r["structuredContent"]["result"], 5);
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id":"demo.add","params":{"b":3}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"]["code"], "validation.required");
    h.shutdown().await;
}

// vhco:test language.compile_program -- S109: a second declaration of an existing ID (same file or another bundled file) rejects the whole candidate catalog with registry.duplicate_id and both spans; nothing is half-registered
#[tokio::test]
async fn duplicate_id_rejects_the_whole_candidate() {
    let second =
        format!("{CATALOG}\noperation demo.echo\n    output text\n    return \"shadow\"\nend\n");
    let e = compile(&second).unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Syntax, "registry.duplicate_id")
    );
    let first_line = CATALOG
        .lines()
        .position(|l| l == "operation demo.echo")
        .unwrap() as i64
        + 1;
    let second_line = CATALOG.lines().count() as i64 + 2;
    assert_eq!(
        e.details.get("first").unwrap().get("line"),
        Some(&Value::Int(first_line))
    );
    assert_eq!(
        e.details.get("second").unwrap().get("line"),
        Some(&Value::Int(second_line))
    );
    assert_eq!(
        span(&e).0 as i64,
        second_line,
        "the error points at the second declaration"
    );
    assert_eq!((e.exit_code(), e.http_status()), (2, 422));

    // Across files of one bundle: both file names are reported.
    let bundle = SourceBundle {
        entry: "catalog.rivet".into(),
        root: ".".into(),
        files: vec![
            SourceFile {
                path: "catalog.rivet".into(),
                text: CATALOG.into(),
            },
            SourceFile {
                path: "more.rivet".into(),
                text: "operation demo.add\n    output integer\n    return 0\nend\n".into(),
            },
        ],
    };
    let e = compile_program(&bundle, &CapyParser::new().unwrap()).unwrap_err();
    assert_eq!(e.code, "registry.duplicate_id");
    assert_eq!(
        e.details.get("first").unwrap().get("file"),
        Some(&Value::text("catalog.rivet"))
    );
    assert_eq!(
        e.details.get("second").unwrap().get("file"),
        Some(&Value::text("more.rivet"))
    );

    // The CLI shows the error at the second span and names the first.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("catalog.rivet"), &second).unwrap();
    let r = rivet(dir.path(), &["--file", "catalog.rivet", "list"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(
        r.stderr.contains("error[registry.duplicate_id]"),
        "{}",
        r.stderr
    );
    assert!(
        r.stderr
            .contains(&format!("(first at catalog.rivet:{first_line})")),
        "{}",
        r.stderr
    );
    assert!(
        r.stderr
            .contains(&format!("--> catalog.rivet:{second_line}:1")),
        "{}",
        r.stderr
    );

    // A running runtime keeps its catalog when a replacement fails to load.
    let running = runtime(CATALOG, ".", r#"{"version":1}"#);
    assert!(
        rivet::Runtime::builder()
            .source("catalog.rivet", &second, ".")
            .build()
            .is_err()
    );
    let c = running
        .request(
            "demo.echo",
            Value::object([("value", Value::text("x"))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(c.result, Value::text("x"));
}

// vhco:test execution.request_operation -- S109: a private helper is callable in-bundle but hidden from list/describe/outputs and refused by direct request, rivet.request, sessions.open, HTTP, MCP and the CLI
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_helper_is_hidden_but_callable_in_bundle() {
    let rt = runtime(CATALOG, ".", r#"{"version":1}"#);
    let c = rt
        .request(
            "demo.echo",
            Value::object([("value", Value::text("hi"))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(c.result, Value::text("hi"), "in-bundle call succeeds");

    let helper = "helper.normalize";
    let params = Value::object([("value", Value::text("x"))]);
    assert!(rt.list().unwrap().entries.iter().all(|e| e.id != helper));
    let describe = rt.describe(&[helper.to_string()]);
    assert!(
        describe.is_err() || describe.unwrap().entries.is_empty(),
        "describe does not disclose the helper"
    );
    assert_eq!(
        rt.outputs(Some(helper), false).unwrap_err().kind,
        ErrorKind::NotFound
    );
    let e = rt.request(helper, params.clone(), None).await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    let e = rt
        .request(
            "rivet.request",
            Value::object([("id", Value::text(helper)), ("params", params.clone())]),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound, "no generic-request bypass");
    let e = rt
        .open_session(rivet::domain::sessions::SessionOpenInput {
            id: helper.into(),
            params: params.clone(),
            principal: rivet::domain::contracts::Principal::local(),
            connection_owned: false,
            deadline_ms: None,
            trace: None,
            restrict: None,
        })
        .await
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound, "no sessions.open bypass");

    let h = start(
        rt.clone(),
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = h.addr.unwrap();
    for path in [
        format!("/v1/operations/{helper}"),
        format!("/v1/operations/{helper}/outputs"),
    ] {
        let r = serve_support::http(addr, "GET", &path, &[], "").await;
        assert_eq!(r.status, 404, "{path}");
    }
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id": helper, "params": {"value": "x"}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 404);
    let listed = serve_support::http(addr, "GET", "/v1/operations", &[], "").await;
    assert!(!listed.text.contains(helper));
    let sid = serve_support::mcp_init(addr, &[]).await;
    let r = serve_support::mcp_call(addr, &sid, &[], helper, json!({"value": "x"})).await;
    assert!(
        r.get("error").is_some() || r["isError"] == true,
        "MCP tools/call refuses the private helper: {r}"
    );
    h.shutdown().await;

    let dir = catalog_dir();
    let r = rivet_async(
        dir.path(),
        &[
            "--file",
            "catalog.rivet",
            "request",
            helper,
            "--params",
            r#"{"value":"x"}"#,
        ],
    )
    .await;
    assert_eq!(r.code, 4, "{}", r.stderr);
    let r = rivet_async(dir.path(), &["--file", "catalog.rivet", "outputs", helper]).await;
    assert_eq!(r.code, 4, "{}", r.stderr);
}

// vhco:test language.compile_program -- IDs under `rivet.` are reserved for built-ins: syntax.reserved_id at the ID
#[test]
fn reserved_rivet_ids_are_rejected() {
    for id in ["rivet.list", "rivet.mine"] {
        let e = compile(&format!(
            "operation {id}\n    output json\n    return 1\nend\n"
        ))
        .unwrap_err();
        assert_eq!(
            (e.kind, e.code.as_str()),
            (ErrorKind::Syntax, "syntax.reserved_id"),
            "{id}"
        );
        assert_eq!(
            span(&e),
            (1, 11, 1, 11 + id.len() as u32),
            "{id}: the span covers the ID"
        );
    }
}

// vhco:test language.compile_program -- S119/S121/S123: `check --strict-docs` passes a fully documented catalog and reports each missing public description and undeclared fail code with its span (exit 2); private helpers are exempt
#[test]
fn strict_docs_findings() {
    let dir = catalog_dir();
    let r = rivet(
        dir.path(),
        &["--file", "catalog.rivet", "check", "--strict-docs"],
    );
    assert_eq!(r.code, 0, "the documented catalog passes: {}", r.stderr);

    let undocumented = "operation t.bare\n    param a integer required\n    output object\n        field id integer required\n    end\n    if a == 0\n        fail \"t.zero\" {}\n    end\n    return {id: a}\nend\n\noperation t.hidden\n    private true\n    param x integer required\n    output integer\n    return x\nend\n";
    std::fs::write(dir.path().join("bare.rivet"), undocumented).unwrap();
    let r = rivet(dir.path(), &["--file", "bare.rivet", "check"]);
    assert_eq!(r.code, 0, "plain check only warns: {}", r.stderr);
    let r = rivet(
        dir.path(),
        &["--file", "bare.rivet", "check", "--strict-docs"],
    );
    assert_eq!(r.code, 2, "{}", r.stderr);
    for code in [
        "docs.description",
        "docs.param_description",
        "docs.output_description",
        "docs.field_description",
        "docs.undeclared_error",
    ] {
        assert!(
            r.stderr.contains(&format!("error[{code}]")),
            "{code}: {}",
            r.stderr
        );
    }
    assert!(
        r.stderr.contains("--> bare.rivet:7:9"),
        "the fail line: {}",
        r.stderr
    );
    assert!(
        !r.stderr.contains("t.hidden"),
        "private helpers are exempt: {}",
        r.stderr
    );
}

// vhco:test registry.describe_capabilities -- G36/S102: `request rivet.capabilities` needs no policy.json and reports protocols, the sandbox backend for this OS, Stage C refusals and the version
#[test]
fn capabilities_builtin_reports_this_build() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        "operation demo.one\n    output integer\n    return 1\nend\n",
    )
    .unwrap();
    let r = rivet(
        dir.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "rivet.capabilities",
            "--params",
            "{}",
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let v = &r.json()["result"];
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    let feature = |n: &str| {
        v["features"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == n)
            .cloned()
            .unwrap_or_else(|| panic!("feature {n} missing: {v}"))
    };
    assert_eq!(feature("http")["versions"], json!(["1.1", "2", "3"]));
    assert_eq!(feature("udp_multicast")["support"], "supported");
    assert_eq!(
        feature("grpc")["modes"],
        json!(["unary", "server_stream", "client_stream", "bidi"])
    );
    assert_eq!(feature("file_watch")["support"], "unsupported");
    assert_eq!(feature("oauth2_password")["support"], "unsupported");
    let status = v["sandbox"]["status"].as_str().unwrap();
    assert!(["active", "gated", "unsupported"].contains(&status), "{v}");
    if cfg!(target_os = "linux") {
        assert_eq!(v["sandbox"]["backend"], "linux-landlock-seccomp");
    }
    if cfg!(target_os = "macos") {
        assert_eq!(v["sandbox"]["backend"], "macos-seatbelt");
    }
}
