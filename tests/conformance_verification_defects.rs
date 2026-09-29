//! INC-2026-0012 — regression tests for the defects found by the v0.2.0
//! documentation and demo verification (PLAN-2026-0002, before P3/P5).
//!
//! ```text
//!  item  surface            defect (fixed)                                   test
//!  ────  ─────────────────  ───────────────────────────────────────────────  ─────────────────────────────────────────
//!   1    tests (T-29)       demos compiled without imports                   conformance_samples::every_demo_bundle_compiles
//!   2    WS                 refused-input terminal: no seq, data_count 0     ws_refused_input_terminal_is_numbered
//!   3    library            terminal record() has no seq                     library_terminal_records_carry_seq
//!   4    every input        rejected envelope answers operation null         rejected_envelope_echoes_the_operation
//!   5    MCP                rivet.request error names rivet.request          mcp_errors_name_the_target_or_null
//!   6    MCP                session_required names tools/list                mcp_errors_name_the_target_or_null
//!   7    envelope           nested suppressed[]/cause carry effects          nested_errors_have_no_effects
//!   8    WS / polling       validation errors have empty IDs                 ws_open_refusals_carry_ids
//!   9    highlight          unclosed block drops its header tokens           highlight_keeps_tokens_of_an_unclosed_block
//!  10    language           inline `open true` ignored                       inline_open_object_output_is_honoured
//!  11    remote CLI         --timeout not sent over WebSocket                remote_ws_sends_the_timeout
//!  12    policy explain     --data, --json denial, params through modules    policy_explain_data_denial_and_modules
//!  13    library / FFI      rt.load refusal has empty IDs                    load_refusal_has_ids_and_registry_kind
//!  14    CLI                module policy warning only from check            request_prints_the_module_policy_warning
//!  15    facade             sessions / trace / serve only under internal     facade_exposes_sessions_trace_and_serve
//!  16    WS                 conflict.ref looks like the ref's terminal       ws_duplicate_ref_refusal_is_detached
//!  17    features           compiled-out gRPC call points at endpoint line   compiled_out_grpc_points_at_the_call
//!  18    language           URL import reads a path                          url_imports_are_refused_at_check
//!  19    tests (T-29)       global/import blocks treated as fragments        conformance_samples::every_reference_fragment_lowers
//!   +    WS                 legacy id/params frames                          ws_legacy_frame_leaves_a_deprecation_note
//! ```
#![cfg(all(feature = "serve", feature = "cli"))]
#![allow(clippy::result_large_err)]

mod support;

use rivet::internal::domain::capabilities::require_build_features;
use rivet::internal::domain::envelope::error_object;
use rivet::internal::domain::source::SourceBundle;
use rivet::internal::features::audit::effect_sites::analyze_program;
use rivet::internal::features::language::compile_program::compile_program;
use rivet::internal::infra::capy_parser::CapyParser;
use rivet::{Envelope, Error, ErrorKind, Runtime, Value};
use serde_json::{Value as Json, json};
use std::process::Command;
use support::*;

fn rivet_cli(dir: &str, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run rivet");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn repo() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

// vhco:test serve.multiplex_ws -- INC-2026-0012 item 2: a ref ended by a refused input (seq 3 after seq 1) gets a terminal record with the next seq (2) and the real data_count (1), as stream-record.schema.json requires
#[tokio::test]
async fn ws_refused_input_terminal_is_numbered() {
    let s = serve(None).await;
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"c3","operation":"demo.relay","data":{}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"c3","seq":1,"data":"hi"}),
    )
    .await;
    let d = ws_recv(&mut ws).await;
    assert_eq!(
        (d["type"].clone(), d["seq"].clone()),
        (json!("data"), json!(1))
    );
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"c3","seq":3,"data":"x"}),
    )
    .await;
    let t = ws_recv(&mut ws).await;
    assert_eq!(t["error"]["code"], "conflict.input_sequence", "{t}");
    assert_eq!(t["seq"], 2, "{t}");
    assert_eq!(t["data_count"], 1, "{t}");
    assert!(t["request_id"].as_str().unwrap().starts_with("req_"), "{t}");
    // A refusal of the very first input: seq 1, data_count 0.
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"c4","operation":"demo.relay","data":{}}),
    )
    .await;
    ws_send(&mut ws, json!({"type":"input","ref":"c4","seq":1,"data":5})).await;
    let t = ws_recv(&mut ws).await;
    assert_eq!(t["error"]["code"], "validation.input", "{t}");
    assert_eq!(
        (t["seq"].clone(), t["data_count"].clone()),
        (json!(1), json!(0))
    );
    s.handle.shutdown().await;
}

// vhco:test execution.request_operation -- INC-2026-0012 item 3: a library stream's terminal Envelope::Result record carries seq = data_count + 1, like the CLI, SSE, WS, polling and C records
#[tokio::test]
async fn library_terminal_records_carry_seq() {
    let rt = runtime(None);
    let records = rt
        .scope(|scope| async move {
            let mut st = scope
                .stream("demo.count", Value::object([("n", Value::Int(3))]))
                .await?;
            let mut out = Vec::new();
            while let Some(env) = st.next().await? {
                out.push(env.record().to_json());
            }
            Ok(out)
        })
        .await
        .unwrap();
    assert_eq!(records.len(), 4, "{records:?}");
    let last = &records[3];
    assert_eq!(last["type"], "result");
    assert_eq!(last["seq"], 4, "{last}");
    assert_eq!(last["data_count"], 3, "{last}");
    // An error terminal numbered by its seq reports data_count = seq - 1.
    let env = Envelope::Error {
        request_id: "req_1".into(),
        trace_id: "tr_1".into(),
        operation: "demo.count".into(),
        seq: 3,
        error: Box::new(Error::new(ErrorKind::Internal, "internal.x", "boom")),
    };
    let j = env.record().to_json();
    assert_eq!(
        (j["seq"].clone(), j["data_count"].clone()),
        (json!(3), json!(2))
    );
}

// vhco:test serve.parse_input -- INC-2026-0012 item 4: an input envelope refused after naming an operation string (wrong-typed deadline_ms) answers with that operation on REST, WebSocket and Runtime::call_json, not null
#[tokio::test]
async fn rejected_envelope_echoes_the_operation() {
    let s = serve(None).await;
    let r = post(
        s.addr,
        "/v1/request",
        json!({"operation":"demo.add","data":{"a":1},"deadline_ms":"soon"}),
        &[],
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["operation"], "demo.add", "{}", r.text);
    assert_eq!(r.json()["error"]["code"], "validation.input_envelope");
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"e1","operation":"demo.add","data":{},"deadline_ms":-1}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["operation"], "demo.add", "{f}");
    let env =
        s.rt.call_json(r#"{"operation":"demo.add","stream":"yes"}"#)
            .await;
    assert_eq!(env.operation.as_deref(), Some("demo.add"));
    // No operation named: still null.
    let r = post(s.addr, "/v1/request", json!({"data":{}}), &[]).await;
    assert_eq!(r.json()["operation"], Json::Null, "{}", r.text);
    s.handle.shutdown().await;
}

// vhco:test execution.request_operation -- INC-2026-0012 items 5 and 6: an MCP rivet.request error envelope names the target operation (like its success envelope); mcp.session_required has operation null, not the JSON-RPC method
#[tokio::test]
async fn mcp_errors_name_the_target_or_null() {
    let s = serve(None).await;
    let sid = mcp_init(s.addr, &[]).await;
    let bad = mcp_call(
        s.addr,
        &sid,
        &[],
        "rivet.request",
        json!({"operation":"demo.add","data":{"b":2}}),
    )
    .await;
    assert_eq!(bad["isError"], true);
    assert_eq!(bad["structuredContent"]["operation"], "demo.add", "{bad}");
    assert_eq!(
        bad["structuredContent"]["error"]["code"],
        "validation.required"
    );
    let ok = mcp_call(
        s.addr,
        &sid,
        &[],
        "rivet.request",
        json!({"operation":"demo.add","data":{"a":1}}),
    )
    .await;
    assert_eq!(ok["structuredContent"]["operation"], "demo.add");
    let r = http(
        s.addr,
        "POST",
        "/mcp",
        &[("accept", "application/json, text/event-stream")],
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}).to_string(),
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"]["code"], "mcp.session_required");
    assert_eq!(r.json()["operation"], Json::Null, "{}", r.text);
    s.handle.shutdown().await;
}

// vhco:test execution.request_operation -- INC-2026-0012 item 7: `effects` appears once, at the envelope's top level: nested suppressed[] and cause errors carry none
#[test]
fn nested_errors_have_no_effects() {
    let mut e = Error::new(ErrorKind::Validation, "validation.type", "outer");
    let mut inner = Error::new(ErrorKind::Validation, "validation.required", "inner");
    inner
        .suppressed
        .push(Error::new(ErrorKind::Validation, "validation.min", "deep"));
    e.suppressed.push(inner);
    e.cause = Some(Box::new(Error::new(
        ErrorKind::Internal,
        "internal.cause",
        "cause",
    )));
    let j = error_object(&e);
    let text = j.to_string();
    assert!(!text.contains("\"effects\""), "{text}");
    assert_eq!(
        j["suppressed"][0]["suppressed"][0]["code"],
        "validation.min"
    );
    assert_eq!(j["cause"]["code"], "internal.cause");
}

// vhco:test sessions.open_session -- INC-2026-0012 item 8: a WS request (and a polling open) refused at open (validation.required, not_found.operation) carries request and trace IDs like REST, the CLI and MCP; a unary WS result keeps seq 1 (every WS ref is a session, API-2026-0002)
#[tokio::test]
async fn ws_open_refusals_carry_ids() {
    let s = serve(None).await;
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"v1","operation":"demo.add","data":{"b":2}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["error"]["code"], "validation.required", "{f}");
    assert!(f["request_id"].as_str().unwrap().starts_with("req_"), "{f}");
    assert!(f["trace_id"].as_str().unwrap().starts_with("tr_"), "{f}");
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"v2","operation":"demo.nope","data":{}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["error"]["code"], "not_found.operation", "{f}");
    assert!(f["request_id"].as_str().unwrap().starts_with("req_"), "{f}");
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"u1","operation":"demo.add","data":{"a":1,"b":2}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["status"].clone(), f["seq"].clone()),
        (json!("ok"), json!(1))
    );
    let r = post(
        s.addr,
        "/v1/requests",
        json!({"operation":"demo.add","data":{"b":2}}),
        &[],
    )
    .await;
    assert!(
        r.json()["request_id"].as_str().unwrap().starts_with("req_"),
        "{}",
        r.text
    );
    s.handle.shutdown().await;
}

// vhco:test language.highlight_source -- INC-2026-0012 item 9: an unclosed block keeps every token before the error (`operation`, `x.y` on line 1, `name "X"` on line 2)
#[test]
fn highlight_keeps_tokens_of_an_unclosed_block() {
    let (tokens, e) = rivet::highlight::tokens("operation x.y\n    name \"X\"\n    return (\n")
        .expect_err("syntax error");
    assert_eq!(e.kind, ErrorKind::Syntax);
    let got: Vec<(u32, &str, &str)> = tokens
        .iter()
        .map(|t| (t.line, t.class.as_str(), t.text.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            (1, "keyword", "operation"),
            (1, "operation_id", "x.y"),
            (2, "keyword", "name"),
            (2, "string", "\"X\""),
        ]
    );
}

// vhco:test language.compile_program -- INC-2026-0012 item 10: `output object description "x" open true` on the header line opens the object (extra keys pass), like the block's `open true` line; `open` on a non-object type is syntax.type
#[tokio::test]
async fn inline_open_object_output_is_honoured() {
    let src = "operation t.o\n    output object description \"x\" open true\n    return {a: 1}\nend\n\noperation t.c\n    output object description \"x\"\n    return {a: 1}\nend\n";
    let rt = Runtime::builder()
        .source("app.rivet", src, ".")
        .build()
        .unwrap();
    let ok = rt.call(rivet::InputEnvelope::new("t.o")).await;
    assert_eq!(ok.data, Some(json!({"a": 1})), "{}", ok.to_json_string());
    let closed = rt.call(rivet::InputEnvelope::new("t.c")).await;
    assert_eq!(closed.error.unwrap().code, "output.invalid");
    let bad = Runtime::builder()
        .source(
            "app.rivet",
            "operation t.i\n    output integer open true\n    return 1\nend\n",
            ".",
        )
        .build()
        .err()
        .expect("refused");
    assert_eq!(bad.code, "syntax.type", "{bad:?}");
}

// vhco:test execution.request_operation -- INC-2026-0012 item 11: `rivet --endpoint URL request ID --stream --input-jsonl - --timeout 300ms` sends deadline_ms on the WebSocket request frame, so the duplex ref times out (timeout.request) instead of running under the 30 s default
#[tokio::test]
async fn remote_ws_sends_the_timeout() {
    let s = serve(None).await;
    let url = format!("http://{}", s.addr);
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args([
            "--endpoint",
            &url,
            "request",
            "demo.relay",
            "--stream",
            "--input-jsonl",
            "-",
            "--timeout",
            "300ms",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // stdin stays open: only the deadline can end the ref. The terminal
    // record must arrive well before the 30 s default deadline.
    use tokio::io::AsyncBufReadExt;
    let stdin = child.stdin.take();
    // A status error record is written to stderr (CLI convention).
    let mut lines = tokio::io::BufReader::new(child.stderr.take().unwrap()).lines();
    let line = tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
        .await
        .expect("the 300 ms deadline ends the ref long before the 30 s default")
        .unwrap()
        .unwrap_or_default();
    assert!(line.contains("timeout.request"), "{line}");
    assert!(line.contains("300 ms deadline"), "{line}");
    // The CLI exits once its stdin reader sees EOF.
    drop(stdin);
    let status = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.code(), Some(6));
    s.handle.shutdown().await;
}

// vhco:test policy.load_policy -- INC-2026-0012 item 12: policy explain takes --data (--params alias); a denied call with --json prints a status error / kind permission envelope and exits 3; params flow through a call into a module (report.remote → users.fetch is exact)
#[test]
fn policy_explain_data_denial_and_modules() {
    let demo = format!("{}/docs/demos/17-modules", repo());
    for flag in ["--data", "--params"] {
        let (code, out, err) = rivet_cli(
            &demo,
            &[
                "--file",
                "app.rivet",
                "--json",
                "policy",
                "explain",
                "report.remote",
                flag,
                r#"{"id":3}"#,
            ],
        );
        assert_eq!(code, 0, "{flag}: {err}");
        assert!(!err.contains("deprecated"), "{flag}: {err}");
        let env: Json = serde_json::from_str(out.trim()).unwrap();
        let site = env["data"]["sites"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["operation_id"] == "users.fetch")
            .cloned()
            .expect("users.fetch site");
        assert_eq!(site["knowledge"], "exact", "{site}");
        assert_eq!(
            site["target"]["template"],
            "https://api.example.com/users/3.json"
        );
        assert_eq!(site["decision"], "allowed");
    }
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("app.rivet"),
        "operation f.read\n    param name text required\n    output text\n    t = file read \"./data/${name}\"\n    return t\nend\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("policy.json"),
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/a.txt"]}]}"#,
    )
    .unwrap();
    let dir = tmp.path().to_str().unwrap();
    let args = |name: &str| {
        vec![
            "--file".to_string(),
            "app.rivet".into(),
            "--json".into(),
            "policy".into(),
            "explain".into(),
            "f.read".into(),
            "--data".into(),
            format!(r#"{{"name":"{name}"}}"#),
        ]
    };
    let run = |a: Vec<String>| {
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        rivet_cli(dir, &refs)
    };
    let (code, out, err) = run(args("b.txt"));
    assert_eq!(code, 3, "{err}");
    assert!(out.trim().is_empty(), "{out}");
    let env: Json = serde_json::from_str(err.trim()).unwrap();
    assert_eq!(env["status"], "error");
    assert_eq!(env["operation"], "rivet.policy.explain");
    assert_eq!(env["error"]["kind"], "permission");
    assert_eq!(env["error"]["code"], "permission.denied");
    assert_eq!(
        env["error"]["details"]["denied"][0]["target"],
        "./data/b.txt"
    );
    assert!(env["request_id"].as_str().unwrap().starts_with("req_"));
    let (code, out, _) = run(args("a.txt"));
    assert_eq!(code, 0);
    let env: Json = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(env["status"], "ok");
}

// vhco:test registry.load_module -- INC-2026-0012 item 13: a refused Runtime::load (check.import_duplicate) keeps the registry kind (syntax, as `rivet check` reports it) and carries minted request/trace IDs
#[tokio::test]
async fn load_refusal_has_ids_and_registry_kind() {
    let demo = format!("{}/docs/demos/17-modules", repo());
    let rt = Runtime::builder().root(&demo).build().unwrap();
    rt.load("./users.rivet").unwrap();
    let e = rt.load("./users.rivet").err().expect("duplicate");
    assert_eq!(e.code, "check.import_duplicate");
    assert_eq!(e.kind, ErrorKind::Syntax);
    assert_eq!(e.exit_code(), 2);
    assert!(
        e.request_id.as_deref().unwrap().starts_with("req_"),
        "{e:?}"
    );
    assert!(e.trace_id.as_deref().unwrap().starts_with("tr_"), "{e:?}");
}

// vhco:test language.resolve_imports -- INC-2026-0012 item 14: `rivet request` prints warning[check.module_policy_ignored] on stderr at load (once), like `rivet check`
#[test]
fn request_prints_the_module_policy_warning() {
    let demo = format!("{}/docs/demos/17-modules", repo());
    let (code, out, err) = rivet_cli(
        &demo,
        &["--file", "errors/module_policy.rivet", "request", "top"],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("\"status\":\"ok\""), "{out}");
    assert_eq!(
        err.matches("warning[check.module_policy_ignored]").count(),
        1,
        "{err}"
    );
}

// vhco:test serve.start_serve -- INC-2026-0012 item 15: the facade exposes the session methods' types (rivet::types::Session*), rivet::TraceResult and rivet::serve::{start, ServeOptions} without rivet::internal
#[tokio::test]
async fn facade_exposes_sessions_trace_and_serve() {
    use rivet::types::{Principal, SessionOpenInput, SessionReadInput};
    let rt = runtime(None);
    let receipt = rt
        .open_session(SessionOpenInput {
            id: "demo.count".into(),
            params: Value::object([("n", Value::Int(2))]),
            principal: Principal::local(),
            connection_owned: false,
            deadline_ms: None,
            trace: None,
            restrict: None,
        })
        .await
        .unwrap();
    let batch = rt
        .read_events(SessionReadInput {
            session_id: receipt.session_id.clone(),
            after_seq: 0,
            max_events: None,
            wait_ms: Some(2000),
            principal: Principal::local(),
        })
        .await
        .unwrap();
    assert!(!batch.events.is_empty());
    let trace: rivet::Result<rivet::TraceResult> = rt.trace(&receipt.request_id);
    let _ = trace;
    let handle = rivet::serve::start(
        rt.clone(),
        rivet::serve::ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..rivet::serve::ServeOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(handle.addr.is_some());
    handle.shutdown().await;
}

// vhco:test serve.multiplex_ws -- INC-2026-0012 item 16: a request frame reusing an in-flight ref is refused with ref "" and details.ref (never a type:"result" record for that ref); the in-flight ref still ends with its own single terminal record
#[tokio::test]
async fn ws_duplicate_ref_refusal_is_detached() {
    let s = serve(None).await;
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"c1","operation":"demo.relay","data":{}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"c1","operation":"demo.add","data":{"a":1}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["ref"], "", "{f}");
    assert_eq!(f["error"]["code"], "conflict.ref");
    assert_eq!(f["error"]["details"]["ref"], "c1");
    assert_eq!(f["operation"], "demo.add");
    // A malformed frame naming the in-flight ref is detached too.
    ws_send(&mut ws, json!({"type":"input","ref":"c1","seq":"one"})).await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["ref"].clone(), f["error"]["code"].clone()),
        (json!(""), json!("validation.frame")),
        "{f}"
    );
    ws_send(&mut ws, json!({"type":"finish_input","ref":"c1"})).await;
    let frames = ws_until_terminal(&mut ws, &["c1"]).await;
    let c1: Vec<&Json> = frames.iter().filter(|f| f["ref"] == "c1").collect();
    assert_eq!(c1.len(), 1, "{frames:?}");
    assert_eq!(c1[0]["status"], "ok");
    s.handle.shutdown().await;
}

// vhco:test registry.describe_capabilities -- INC-2026-0012 item 17: without the grpc feature a gRPC call is reported at its call site (the `response = grpc …` line), not at the connector's endpoint line
#[test]
fn compiled_out_grpc_points_at_the_call() {
    let text = "connector users grpc\n    endpoint \"http://127.0.0.1:50051\"\n    descriptor \"./schemas/users.pb\"\n    service \"example.Users\"\nend\n\noperation users.get\n    param id text required\n    output json\n    response = grpc users.GetUser\n        message {id: id}\n    end\n    return response\nend\n";
    let program = compile_program(
        &SourceBundle::single("app.rivet", text),
        &CapyParser::new().unwrap(),
    )
    .unwrap();
    let e = require_build_features(&program, &analyze_program(&program), &[]).unwrap_err();
    let lines: Vec<u32> = std::iter::once(&e)
        .chain(e.suppressed.iter())
        .map(|d| d.source.as_ref().unwrap().start_line)
        .collect();
    assert_eq!(lines, vec![1, 10], "connector line, then the call line");
    assert!(e.suppressed[0].message.contains("a gRPC call"));
}

// vhco:test language.resolve_imports -- INC-2026-0012 item 18: `import "https://x.com/a.rivet" as x` (any scheme://) is syntax.import at check, never read as a path
#[test]
fn url_imports_are_refused_at_check() {
    for url in [
        "https://x.com/a.rivet",
        "file:///etc/a.rivet",
        "git+ssh://h/a.rivet",
    ] {
        let src = format!(
            "import \"{url}\" as x\noperation a.b\n    output integer\n    return 1\nend\n"
        );
        let e = Runtime::builder()
            .source("app.rivet", &src, ".")
            .build()
            .err()
            .expect("refused");
        assert_eq!(e.code, "syntax.import", "{url}: {e:?}");
        assert!(
            e.message.contains("URL imports are not supported"),
            "{}",
            e.message
        );
    }
}

// vhco:test serve.multiplex_ws -- INC-2026-0012 (coordinator): a legacy WebSocket request frame ({id, params}) is accepted and leaves the deprecation trace note (phase input, decision deprecated) on its request, like HTTP and MCP
#[tokio::test]
async fn ws_legacy_frame_leaves_a_deprecation_note() {
    let s = serve(None).await;
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"l1","id":"demo.add","params":{"a":1}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["status"], "ok", "{f}");
    let rid = f["request_id"].as_str().unwrap().to_string();
    let trace = s.rt.trace(&rid).unwrap();
    let note = trace
        .events
        .iter()
        .find(|e| e.decision == "deprecated")
        .expect("deprecation note");
    assert_eq!(note.target, "id,params");
    s.handle.shutdown().await;
}
