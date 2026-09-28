//! PROP-2026-0002 R1–R6 (PLAN-2026-0002 T-01…T-05, T-15): one output envelope
//! and one input envelope on every surface, pretty JSON, and the deprecated
//! 0.1.0 `id`/`params` aliases.
//!
//! ```text
//!  input.json {"operation","data"} ─┬─ rivet request --input FILE ─┐
//!                                   ├─ POST /v1/request ───────────┤
//!                                   ├─ POST /v1/requests (poll) ───┤
//!                                   ├─ /v1/ws request frame ───────┼─▶ ResponseEnvelope (validated against
//!                                   ├─ MCP rivet.request ──────────┤    docs/api/schemas/*.schema.json,
//!                                   └─ Runtime::call_json ─────────┘    key order fixed)
//! ```
//!
//! The JSON Schemas are checked with a small in-test validator covering the
//! keywords they use (type, const, enum, required, properties,
//! additionalProperties, oneOf, anyOf, allOf, not, if/then, $ref, minimum,
//! minLength), so no schema crate is added to the dependency set (ADR-0002).

mod support;

use rivet::domain::envelope::{InputEnvelope, ResponseEnvelope};
use rivet::domain::errors::ALL_KINDS;
use rivet::domain::{RivetError, Value};
use serde_json::{Value as Json, json};
use std::path::Path;
use support::*;

const RIVET: &str = env!("CARGO_BIN_EXE_rivet");

/// R1 key order of a result record (seq only on stream terminals, ref first on WS).
const RESULT_KEYS: [&str; 9] = [
    "request_id",
    "trace_id",
    "operation",
    "type",
    "status",
    "data",
    "error",
    "effects",
    "data_count",
];

// ---------------------------------------------------------------- schema validator

fn schema(name: &str) -> Json {
    let text = std::fs::read_to_string(format!("docs/api/schemas/{name}")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn type_ok(t: &str, v: &Json) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        other => panic!("validator: unknown type {other}"),
    }
}

/// Errors of `v` against `s` (empty = valid).
fn check(s: &Json, v: &Json, at: &str, out: &mut Vec<String>) {
    let Some(s) = s.as_object() else { return };
    if let Some(r) = s.get("$ref").and_then(Json::as_str) {
        check(&schema(r), v, at, out);
    }
    if let Some(t) = s.get("type") {
        let ok = match t {
            Json::String(t) => type_ok(t, v),
            Json::Array(ts) => ts.iter().any(|t| type_ok(t.as_str().unwrap(), v)),
            _ => true,
        };
        if !ok {
            out.push(format!("{at}: {v} is not {t}"));
        }
    }
    if let Some(c) = s.get("const")
        && c != v
    {
        out.push(format!("{at}: {v} != const {c}"));
    }
    if let Some(Json::Array(e)) = s.get("enum")
        && !e.contains(v)
    {
        out.push(format!("{at}: {v} not in {e:?}"));
    }
    if let (Some(m), Some(n)) = (s.get("minimum").and_then(Json::as_f64), v.as_f64())
        && n < m
    {
        out.push(format!("{at}: {n} < {m}"));
    }
    if let (Some(m), Some(t)) = (s.get("minLength").and_then(Json::as_u64), v.as_str())
        && (t.chars().count() as u64) < m
    {
        out.push(format!("{at}: `{t}` shorter than {m}"));
    }
    if let Some(obj) = v.as_object() {
        if let Some(Json::Array(req)) = s.get("required") {
            for k in req {
                if !obj.contains_key(k.as_str().unwrap()) {
                    out.push(format!("{at}: missing `{}`", k.as_str().unwrap()));
                }
            }
        }
        let props = s.get("properties").and_then(Json::as_object);
        if let Some(p) = props {
            for (k, sub) in p {
                if let Some(x) = obj.get(k) {
                    check(sub, x, &format!("{at}/{k}"), out);
                }
            }
        }
        if s.get("additionalProperties") == Some(&Json::Bool(false)) {
            for k in obj.keys() {
                if !props.is_some_and(|p| p.contains_key(k)) {
                    out.push(format!("{at}: unexpected key `{k}`"));
                }
            }
        }
    }
    let valid = |sub: &Json| {
        let mut e = Vec::new();
        check(sub, v, at, &mut e);
        e.is_empty()
    };
    if let Some(Json::Array(all)) = s.get("allOf") {
        for sub in all {
            check(sub, v, at, out);
        }
    }
    if let Some(Json::Array(any)) = s.get("anyOf")
        && !any.iter().any(valid)
    {
        out.push(format!("{at}: matches no anyOf branch"));
    }
    if let Some(Json::Array(one)) = s.get("oneOf") {
        let n = one.iter().filter(|sub| valid(sub)).count();
        if n != 1 {
            out.push(format!("{at}: matches {n} oneOf branches"));
        }
    }
    if let Some(not) = s.get("not")
        && valid(not)
    {
        out.push(format!("{at}: matches `not`"));
    }
    if let Some(cond) = s.get("if")
        && valid(cond)
        && let Some(then) = s.get("then")
    {
        check(then, v, at, out);
    }
}

fn assert_schema(name: &str, v: &Json) {
    let mut errs = Vec::new();
    check(&schema(name), v, "$", &mut errs);
    assert!(errs.is_empty(), "{name}: {errs:?}\n{v}");
}

/// A unary/terminal record: valid, keys in the R1 order.
fn assert_result(v: &Json) {
    assert_schema("response.schema.json", v);
    let keys: Vec<&str> = v
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "ref" && *k != "seq")
        .collect();
    assert_eq!(keys, RESULT_KEYS, "{v}");
}

fn assert_record(v: &Json) {
    assert_schema("stream-record.schema.json", v);
}

// ---------------------------------------------------------------- CLI helpers

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn bundle() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("app.rivet"), catalog()).unwrap();
    d
}

fn rivet(dir: &Path, args: &[&str], stdin: Option<&str>) -> Out {
    use std::io::Write;
    let mut child = std::process::Command::new(RIVET)
        .current_dir(dir)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(s) = stdin {
        child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    } else {
        drop(child.stdin.take());
    }
    let out = child.wait_with_output().unwrap();
    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

async fn rivet_async(dir: &Path, args: &[&str], stdin: Option<&str>) -> Out {
    let dir = dir.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let stdin = stdin.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        rivet(&dir, &a, stdin.as_deref())
    })
    .await
    .unwrap()
}

fn json_line(s: &str) -> Json {
    serde_json::from_str(s.trim()).unwrap_or_else(|e| panic!("{e}: {s}"))
}

/// The error envelope on stderr: the last JSON line, or the last pretty
/// (multi-line) object when --pretty was given.
fn stderr_envelope(o: &Out) -> Json {
    let lines: Vec<&str> = o.stderr.lines().collect();
    let start = lines
        .iter()
        .rposition(|l| l.starts_with('{'))
        .unwrap_or_else(|| panic!("no envelope on stderr: {}", o.stderr));
    if lines[start] == "{" {
        return json_line(&lines[start..].join("\n"));
    }
    json_line(lines[start])
}

// ---------------------------------------------------------------- T-01

// vhco:test execution.request_operation -- T-01: every surface (CLI, HTTP, SSE, NDJSON, polling, WS, MCP structuredContent, library) answers ok, error, cancelled and accepted outcomes with a ResponseEnvelope that validates against docs/api/schemas and keeps the R1 key order; every error kind renders status error (cancelled for kind cancelled) with `effects` at the top level
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t01_envelope_schema_on_every_surface() {
    let s = serve(None).await;
    let a = s.addr;
    let d = bundle();

    // CLI ok / error
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.add",
            "--data",
            r#"{"a":2,"b":3}"#,
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v = json_line(&o.stdout);
    assert_result(&v);
    assert_eq!(
        (
            v["status"].clone(),
            v["data"].clone(),
            v["operation"].clone()
        ),
        (json!("ok"), json!(5), json!("demo.add"))
    );
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.add",
            "--data",
            r#"{"b":3}"#,
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 2);
    assert!(o.stdout.is_empty(), "errors stay on stderr: {}", o.stdout);
    let v = stderr_envelope(&o);
    assert_result(&v);
    assert_eq!(
        (v["status"].clone(), v["error"]["code"].clone()),
        (json!("error"), json!("validation.required"))
    );

    // HTTP ok / 422 / 404 / 400 / unmounted route
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"demo.add","data":{"a":2,"b":3}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 200);
    assert_result(&r.json());
    for (body, status) in [
        (json!({"operation":"demo.add","data":{"b":3}}), 422),
        (json!({"operation":"demo.secret"}), 404),
        (json!({"operation":"demo.count"}), 422),
    ] {
        let r = post(a, "/v1/request", body.clone(), &[]).await;
        assert_eq!(r.status, status, "{body}");
        let v = r.json();
        assert_result(&v);
        assert_eq!(v["status"], "error");
        assert!(v["data"].is_null());
    }
    let r = http(a, "POST", "/v1/request", &[], "{nope").await;
    assert_eq!(r.status, 400);
    assert_result(&r.json());
    let r = http(a, "GET", "/v1/nowhere", &[], "").await;
    assert_eq!(r.status, 404);
    assert_result(&r.json());
    assert!(r.json()["operation"].is_null());

    // SSE records
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"operation":"demo.count","data":{"n":2}}).to_string(),
    )
    .await;
    let ev = sse_events(&r.text);
    assert_eq!(
        ev.iter().map(|e| e.1.as_str()).collect::<Vec<_>>(),
        ["data", "data", "result"]
    );
    for e in &ev {
        assert_record(&e.2);
    }
    assert_eq!(
        (ev[2].2["seq"].clone(), ev[2].2["data_count"].clone()),
        (json!(3), json!(2))
    );

    // polling: accepted receipt, then records
    let r = post(
        a,
        "/v1/requests",
        json!({"operation":"demo.count","data":{"n":1}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 202);
    let v = r.json();
    assert_result(&v);
    assert_eq!(v["status"], "accepted");
    let url = v["data"]["events_url"].as_str().unwrap().to_string();
    let b = http(
        a,
        "GET",
        &format!("{url}?after_seq=0&wait_ms=5000"),
        &[],
        "",
    )
    .await
    .json();
    let mut events: Vec<Json> = b["events"].as_array().unwrap().clone();
    let mut last = b["last_seq"].as_u64().unwrap();
    while !events.last().is_some_and(|e| e["type"] == "result") {
        let b = http(
            a,
            "GET",
            &format!("{url}?after_seq={last}&wait_ms=5000"),
            &[],
            "",
        )
        .await
        .json();
        events.extend(b["events"].as_array().unwrap().clone());
        last = b["last_seq"].as_u64().unwrap();
    }
    for e in &events {
        assert_record(e);
    }
    let r = http(a, "GET", "/v1/requests/ses_nope/events", &[], "").await;
    assert_eq!(r.status, 404);
    assert_result(&r.json());

    // WS: records with ref first; ok, error and cancelled terminals
    let mut ws = ws_connect(a, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"ok","operation":"demo.add","data":{"a":1,"b":1}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"bad","operation":"demo.add","data":{}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"relay","operation":"demo.relay","data":{}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"relay","seq":1,"data":"x"}),
    )
    .await;
    let first = ws_until_terminal(&mut ws, &["ok", "bad"]).await;
    ws_send(&mut ws, json!({"type":"cancel","ref":"relay"})).await;
    let rest = ws_until_terminal(&mut ws, &["relay"]).await;
    let frames: Vec<Json> = first.into_iter().chain(rest).collect();
    for f in &frames {
        // A ref that never opened gets one error result (no seq); streams carry records.
        if f.get("seq").is_some() {
            assert_record(f);
        } else {
            assert_result(f);
        }
        assert_eq!(
            f.as_object().unwrap().keys().next().map(String::as_str),
            Some("ref"),
            "{f}"
        );
    }
    let terminal = |r: &str| {
        frames
            .iter()
            .find(|f| f["ref"] == r && f["type"] == "result")
            .cloned()
            .unwrap()
    };
    assert_eq!(terminal("ok")["status"], "ok");
    assert_eq!(terminal("bad")["status"], "error");
    let c = terminal("relay");
    assert_eq!(
        (c["status"].clone(), c["error"]["kind"].clone()),
        (json!("cancelled"), json!("cancelled")),
        "{c}"
    );
    assert_eq!(c["operation"], "demo.relay");

    // MCP: structuredContent is the envelope; text content is its JSON
    let sid = mcp_init(a, &[]).await;
    let res = mcp_call(a, &sid, &[], "demo.add", json!({"a":2,"b":3})).await;
    assert_result(&res["structuredContent"]);
    assert_eq!(
        json_line(res["content"][0]["text"].as_str().unwrap()),
        res["structuredContent"]
    );
    let res = mcp_call(a, &sid, &[], "demo.add", json!({"b":3})).await;
    assert_eq!(res["isError"], true);
    assert_result(&res["structuredContent"]);
    let res = mcp_call(a, &sid, &[], "demo.count", json!({"n":1})).await;
    assert_result(&res["structuredContent"]);
    assert_eq!(res["structuredContent"]["status"], "accepted");

    // library
    let v =
        s.rt.call(InputEnvelope::new("demo.add").data(json!({"a":2,"b":3})))
            .await;
    assert!(v.status().is_ok());
    assert_result(&v.to_json());
    let v = s.rt.call(InputEnvelope::new("demo.add")).await;
    assert_result(&v.to_json());
    assert_eq!(v.to_json()["status"], "error");

    // every error kind: status error (cancelled for kind cancelled), effects top-level
    for k in ALL_KINDS {
        let mut e = RivetError::new(k, format!("{}.probe", k.as_str()), "probe");
        e.request_id = Some("req_x".into());
        e.trace_id = Some("tr_x".into());
        let v = ResponseEnvelope::from_error(Some("demo.add"), &e).to_json();
        assert_result(&v);
        let want = if k.as_str() == "cancelled" {
            "cancelled"
        } else {
            "error"
        };
        assert_eq!(v["status"], want);
        assert_eq!(v["error"]["kind"], k.as_str());
    }
    s.handle.shutdown().await;
}

// ---------------------------------------------------------------- T-02

// vhco:test serve.parse_input -- T-02: one input envelope file {operation, data} gives identical `data` through `rivet request --input FILE`, `--input -`, `--endpoint … --input`, POST /v1/request, POST /v1/requests, a WS request frame, MCP rivet.request and Runtime::call_json
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_one_input_file_on_every_surface() {
    let s = serve(None).await;
    let a = s.addr;
    let d = bundle();
    let input = json!({"operation":"demo.add","data":{"a":2,"b":3}});
    std::fs::write(d.path().join("input.json"), input.to_string()).unwrap();
    let url = format!("http://{a}");
    let mut got: Vec<(String, Json)> = Vec::new();

    for args in [
        vec!["--file", "app.rivet", "request", "--input", "input.json"],
        vec![
            "--file",
            "app.rivet",
            "request",
            "demo.add",
            "--input",
            "input.json",
        ],
        vec![
            "--endpoint",
            url.as_str(),
            "request",
            "--input",
            "input.json",
        ],
    ] {
        let o = rivet_async(d.path(), &args, None).await;
        assert_eq!(o.code, 0, "{args:?}: {}", o.stderr);
        got.push((format!("{args:?}"), json_line(&o.stdout)["data"].clone()));
    }
    let o = rivet_async(
        d.path(),
        &["--file", "app.rivet", "request", "--input", "-"],
        Some(&input.to_string()),
    )
    .await;
    assert_eq!(o.code, 0, "{}", o.stderr);
    got.push(("--input -".into(), json_line(&o.stdout)["data"].clone()));
    // naming a different operation on the command line is a usage error
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.greet",
            "--input",
            "input.json",
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 2);
    assert_eq!(stderr_envelope(&o)["error"]["code"], "validation.usage");

    let r = post(a, "/v1/request", input.clone(), &[]).await;
    got.push(("http".into(), r.json()["data"].clone()));
    let r = post(a, "/v1/requests", input.clone(), &[]).await;
    let events_url = r.json()["data"]["events_url"].as_str().unwrap().to_string();
    let b = http(
        a,
        "GET",
        &format!("{events_url}?after_seq=0&wait_ms=5000"),
        &[],
        "",
    )
    .await
    .json();
    got.push(("poll".into(), b["events"][0]["data"].clone()));
    let mut ws = ws_connect(a, &[]).await.unwrap();
    let mut frame = input.clone();
    frame["type"] = json!("request");
    frame["ref"] = json!("t2");
    ws_send(&mut ws, frame).await;
    got.push((
        "ws".into(),
        ws_until_terminal(&mut ws, &["t2"]).await.last().unwrap()["data"].clone(),
    ));
    let sid = mcp_init(a, &[]).await;
    let res = mcp_call(a, &sid, &[], "rivet.request", input.clone()).await;
    got.push(("mcp".into(), res["structuredContent"]["data"].clone()));
    got.push((
        "library".into(),
        s.rt.call_json(&input.to_string()).await.to_json()["data"].clone(),
    ));

    for (surface, data) in &got {
        assert_eq!(data, &json!(5), "{surface}");
    }
    assert_eq!(got.len(), 9);
    s.handle.shutdown().await;
}

// ---------------------------------------------------------------- T-03

// vhco:test registry.describe_operations -- T-03: built-ins, GET routes and every CLI --json output (list, describe, outputs, io, graph, check, policy explain/generate, trace show error) are envelopes whose operation is the built-in ID and whose data is the payload
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_builtins_and_json_outputs_are_envelopes() {
    let s = serve(None).await;
    let a = s.addr;
    for (path, op) in [
        ("/v1/operations", "rivet.list"),
        ("/v1/operations/demo.add", "rivet.describe"),
        ("/v1/operations/demo.add/outputs", "rivet.outputs"),
        ("/v1/io", "rivet.io"),
        ("/v1/health", "rivet.health"),
    ] {
        let r = http(a, "GET", path, &[], "").await;
        assert_eq!(r.status, 200, "{path}");
        let v = r.json();
        assert_result(&v);
        assert_eq!(
            (v["operation"].clone(), v["status"].clone()),
            (json!(op), json!("ok")),
            "{path}"
        );
        assert!(r.headers.contains_key("traceparent"), "{path}");
    }
    assert_eq!(
        http(a, "GET", "/v1/operations", &[], "").await.json()["data"]["operations"][0]["id"],
        "demo.greet"
    );
    let r = post(a, "/v1/policy/generate", json!({"all": true}), &[]).await;
    assert_result(&r.json());
    assert_eq!(r.json()["operation"], "rivet.policy.generate");
    for (op, data) in [
        ("rivet.list", json!({})),
        ("rivet.capabilities", json!({})),
        ("rivet.describe", json!({"id":"demo.add"})),
    ] {
        let r = post(
            a,
            "/v1/request",
            json!({"operation": op, "data": data}),
            &[],
        )
        .await;
        assert_result(&r.json());
        assert_eq!(r.json()["operation"], op);
    }

    let d = bundle();
    for (args, op) in [
        (vec!["list"], "rivet.list"),
        (vec!["list", "--outputs"], "rivet.list"),
        (vec!["describe", "demo.add"], "rivet.describe"),
        (vec!["outputs", "demo.add"], "rivet.outputs"),
        (vec!["io", "demo.add"], "rivet.io"),
        (vec!["graph", "demo.add"], "rivet.graph"),
        (vec!["check"], "rivet.check"),
        (vec!["policy", "explain"], "rivet.policy.explain"),
        (vec!["policy", "generate", "--all"], "rivet.policy.generate"),
    ] {
        let mut all = vec!["--json", "--file", "app.rivet"];
        all.extend(args.iter());
        let o = rivet_async(d.path(), &all, None).await;
        assert!(o.code == 0 || o.code == 7, "{args:?}: {}", o.stderr);
        let v = json_line(&o.stdout);
        assert_result(&v);
        assert_eq!(v["operation"], op, "{args:?}");
        assert!(
            v["request_id"].as_str().unwrap().starts_with("req_"),
            "{args:?}"
        );
    }
    let o = rivet_async(
        d.path(),
        &["--file", "app.rivet", "io", "demo.add", "--format", "json"],
        None,
    )
    .await;
    assert!(json_line(&o.stdout)["data"]["sites"].is_array());
    // trace show of an unknown request: error envelope on stderr, exit 4
    let o = rivet_async(
        d.path(),
        &["--file", "app.rivet", "trace", "show", "req_nope"],
        None,
    )
    .await;
    assert_eq!(o.code, 4);
    let v = stderr_envelope(&o);
    assert_result(&v);
    assert_eq!(v["operation"], "rivet.trace.show");
    // without --json the tables are unchanged
    let o = rivet_async(d.path(), &["--file", "app.rivet", "list"], None).await;
    assert!(o.stdout.starts_with("ID "), "{}", o.stdout);
    s.handle.shutdown().await;
}

// ---------------------------------------------------------------- T-04

/// A sink that stops after the first item (a consumer that has seen enough).
struct StopAfterOne(std::sync::atomic::AtomicU64);

#[async_trait::async_trait]
impl rivet::domain::ports::DataSink for StopAfterOne {
    async fn send(
        &self,
        _e: rivet::domain::contracts::DataEvent,
    ) -> rivet::domain::RivetResult<()> {
        if self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 1 {
            return Err(RivetError::consumer_stop());
        }
        Ok(())
    }
}

// vhco:test execution.request_operation -- T-04: streams are type data records (seq 1..n) followed by exactly one type result record carrying seq n+1 and data_count n on NDJSON, SSE, WS and the library; a consumer stop ends status cancelled
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_stream_records_then_one_result() {
    let d = bundle();
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.count",
            "--data",
            r#"{"n":3}"#,
            "--stream",
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 0, "{}", o.stderr);
    let lines: Vec<Json> = o.stdout.lines().map(json_line).collect();
    assert_eq!(lines.len(), 4);
    for (i, l) in lines.iter().enumerate() {
        assert_record(l);
        assert_eq!(l["seq"], (i + 1) as u64);
    }
    assert_eq!(lines.iter().filter(|l| l["type"] == "result").count(), 1);
    assert_eq!(
        (
            lines[3]["type"].clone(),
            lines[3]["data"].clone(),
            lines[3]["data_count"].clone()
        ),
        (json!("result"), json!(3), json!(3))
    );
    assert_eq!(lines[0]["operation"], "demo.count");

    // a failing stream: the terminal error record (on stderr) counts the items before it
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.relay",
            "--input-jsonl",
            "-",
            "--stream",
        ],
        Some("\"a\"\n7\n"),
    )
    .await;
    assert_eq!(o.code, 2, "{}", o.stderr);
    let v = stderr_envelope(&o);
    assert_record(&v);
    assert_eq!(
        (v["type"].clone(), v["status"].clone()),
        (json!("result"), json!("error"))
    );

    // library: the stream handle's records, then a consumer stop → cancelled
    let rt = runtime(None);
    let recs = rt
        .scope(|scope| async move {
            let mut h = scope
                .stream("demo.count", Value::object([("n", Value::Int(2))]))
                .await?;
            let mut out = Vec::new();
            while let Some(env) = h.next().await? {
                out.push(env.record().to_json());
            }
            Ok(out)
        })
        .await
        .unwrap();
    assert_eq!(
        recs.iter()
            .map(|r| r["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["data", "data", "result"]
    );
    for r in &recs[..2] {
        assert_record(r);
    }
    assert_result(&recs[2]);
    let stop: std::sync::Arc<dyn rivet::domain::ports::DataSink> =
        std::sync::Arc::new(StopAfterOne(Default::default()));
    let out = rt
        .request(
            "demo.count",
            Value::object([("n", Value::Int(5))]),
            Some(stop),
        )
        .await;
    let v = ResponseEnvelope::from_outcome("demo.count", &out).to_json();
    assert_result(&v);
    assert_eq!(
        (v["status"].clone(), v["error"]["code"].clone()),
        (json!("cancelled"), json!("consumer.stop"))
    );
}

// ---------------------------------------------------------------- T-05

/// Replace the varying IDs so a golden can be compared.
fn golden(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let l = if line.contains("\"request_id\"") {
            "  \"request_id\": \"<id>\",".to_string()
        } else if line.contains("\"trace_id\"") {
            "  \"trace_id\": \"<id>\",".to_string()
        } else {
            line.to_string()
        };
        out.push_str(&l);
        out.push('\n');
    }
    out
}

const PRETTY_GOLDEN: &str = r#"{
  "request_id": "<id>",
  "trace_id": "<id>",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 5,
  "error": null,
  "effects": "none",
  "data_count": 0
}
"#;

// vhco:test execution.request_operation -- T-05: --pretty, ?pretty=true and to_json_pretty give the golden 2-space indented envelope with unchanged key order; --pretty with --stream is validation.usage (exit 2) and ?pretty=true with SSE is 400 validation.pretty_stream
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t05_pretty_goldens_and_stream_refusal() {
    let d = bundle();
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "--pretty",
            "request",
            "demo.add",
            "--data",
            r#"{"a":2,"b":3}"#,
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(golden(&o.stdout), PRETTY_GOLDEN);
    let compact = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.add",
            "--data",
            r#"{"a":2,"b":3}"#,
        ],
        None,
    )
    .await;
    assert_eq!(
        compact.stdout.lines().count(),
        1,
        "compact stays the default"
    );
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.count",
            "--stream",
            "--pretty",
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 2);
    assert!(o.stdout.is_empty());
    assert_eq!(stderr_envelope(&o)["error"]["code"], "validation.usage");
    // --json outputs are pretty too
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "--json",
            "--pretty",
            "describe",
            "demo.add",
        ],
        None,
    )
    .await;
    assert!(
        o.stdout.starts_with("{\n  \"request_id\": "),
        "{}",
        o.stdout
    );

    let s = serve(None).await;
    let a = s.addr;
    let r = http(
        a,
        "POST",
        "/v1/request?pretty=true",
        &[],
        &json!({"operation":"demo.add","data":{"a":2,"b":3}}).to_string(),
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(golden(&format!("{}\n", r.text)), PRETTY_GOLDEN);
    let r = http(a, "GET", "/v1/operations?pretty=true", &[], "").await;
    assert!(
        r.text.contains("\n  \"operation\": \"rivet.list\""),
        "{}",
        r.text
    );
    let r = http(
        a,
        "POST",
        "/v1/request?pretty=true",
        &[("accept", "text/event-stream")],
        &json!({"operation":"demo.count"}).to_string(),
    )
    .await;
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["error"]["code"], "validation.pretty_stream");
    assert!(r.text.contains("\n  "), "the refusal itself is pretty");
    let v =
        s.rt.call(InputEnvelope::new("demo.add").data(json!({"a":2,"b":3})))
            .await;
    assert_eq!(golden(&format!("{}\n", v.to_json_pretty())), PRETTY_GOLDEN);
    s.handle.shutdown().await;
}

// ---------------------------------------------------------------- T-15

// vhco:test serve.parse_input -- T-15: 0.1.0 {id, params} still works in 0.2.x with deprecation signals (HTTP Deprecation: true + access log deprecated=1, CLI warning[deprecated.params]/[deprecated.input], a trace note) on HTTP, polling, WS, MCP, CLI and the library; mixing a key with its alias or sending `error` is validation.input_envelope (422, exit 2)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t15_legacy_aliases_and_refusals() {
    let rt = runtime(None);
    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Json>::new()));
    let sink = std::sync::Arc::clone(&lines);
    let handle = rivet::orchestrator::setup_serve::start(
        rt.clone(),
        rivet::orchestrator::setup_serve::ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            access_log: Some(std::sync::Arc::new(move |l: &str| {
                sink.lock().unwrap().push(serde_json::from_str(l).unwrap());
            })),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let a = handle.addr.unwrap();

    // HTTP legacy body: same answer, Deprecation header, access-log flag, trace note
    let r = post(
        a,
        "/v1/request",
        json!({"id":"demo.add","params":{"a":2,"b":3}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_result(&r.json());
    assert_eq!(r.json()["data"], 5);
    assert_eq!(r.headers["deprecation"], "true");
    let rid = r.json()["request_id"].as_str().unwrap().to_string();
    let trace = rt.trace(&rid).unwrap().to_json();
    assert!(trace.to_string().contains("\"deprecated\""), "{trace}");
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"demo.add","data":{"a":1}}),
        &[],
    )
    .await;
    assert!(r.headers.get("deprecation").is_none());
    let log = lines.lock().unwrap().clone();
    assert_eq!(log[0]["deprecated"], 1, "{log:?}");
    assert!(log[1].get("deprecated").is_none(), "{log:?}");

    // mixing keys, or an `error` key: 422 validation.input_envelope, itself an envelope
    for body in [
        json!({"operation":"demo.add","id":"demo.add"}),
        json!({"operation":"demo.add","data":{},"params":{}}),
        json!({"operation":"demo.add","error":{"code":"x"}}),
        json!(["demo.add"]),
    ] {
        let r = post(a, "/v1/request", body.clone(), &[]).await;
        assert_eq!(r.status, 422, "{body}");
        let v = r.json();
        assert_result(&v);
        assert_eq!(v["error"]["code"], "validation.input_envelope", "{body}");
    }
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"demo.add","id":"demo.add"}),
        &[],
    )
    .await;
    assert_eq!(
        r.json()["error"]["message"],
        "use `operation` or the deprecated `id`, not both"
    );

    // polling, WS and MCP accept the aliases
    let r = post(
        a,
        "/v1/requests",
        json!({"id":"demo.add","params":{"a":2,"b":3}}),
        &[],
    )
    .await;
    assert_eq!(
        (r.status, r.headers["deprecation"].to_str().unwrap()),
        (202, "true")
    );
    let mut ws = ws_connect(a, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"old","id":"demo.add","params":{"a":2,"b":3}}),
    )
    .await;
    let f = ws_until_terminal(&mut ws, &["old"]).await.pop().unwrap();
    assert_eq!(
        (f["status"].clone(), f["data"].clone()),
        (json!("ok"), json!(5))
    );
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"mix","operation":"demo.add","id":"demo.add"}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["error"]["code"], "validation.input_envelope", "{f}");
    let sid = mcp_init(a, &[]).await;
    let r = mcp_raw(
        a,
        &sid,
        &[],
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"rivet.request","arguments":{"id":"demo.add","params":{"a":2,"b":3}}}}),
    )
    .await;
    assert_eq!(r.json()["result"]["structuredContent"]["data"], 5);
    assert_eq!(r.headers["deprecation"], "true");

    // library: call_json with the legacy body works; mixed keys and bad JSON are error envelopes
    let v = rt
        .call_json(r#"{"id":"demo.add","params":{"a":2,"b":3}}"#)
        .await
        .to_json();
    assert_eq!(v["data"], 5);
    assert!(
        rt.trace(v["request_id"].as_str().unwrap()).is_ok(),
        "trace note recorded"
    );
    let v = rt
        .call_json(r#"{"operation":"demo.add","id":"x"}"#)
        .await
        .to_json();
    assert_result(&v);
    assert_eq!(
        (v["error"]["code"].clone(), v["operation"].clone()),
        (json!("validation.input_envelope"), json!("demo.add"))
    );
    let v = rt.call_json("{nope").await.to_json();
    assert_result(&v);
    assert_eq!(v["error"]["code"], "validation.input_envelope");

    // CLI: --params warns and still works; --input with legacy keys warns; mixing refuses
    let d = bundle();
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.add",
            "--params",
            r#"{"a":2,"b":3}"#,
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.contains("warning[deprecated.params]"),
        "{}",
        o.stderr
    );
    assert_eq!(json_line(&o.stdout)["data"], 5);
    std::fs::write(
        d.path().join("old.json"),
        r#"{"id":"demo.add","params":{"a":2,"b":3}}"#,
    )
    .unwrap();
    let o = rivet_async(
        d.path(),
        &["--file", "app.rivet", "request", "--input", "old.json"],
        None,
    )
    .await;
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.contains("warning[deprecated.input]"),
        "{}",
        o.stderr
    );
    let o = rivet_async(
        d.path(),
        &[
            "--file",
            "app.rivet",
            "request",
            "demo.add",
            "--data",
            "{}",
            "--params",
            "{}",
        ],
        None,
    )
    .await;
    assert_eq!(o.code, 2);
    assert_eq!(stderr_envelope(&o)["error"]["code"], "validation.usage");
    std::fs::write(
        d.path().join("mixed.json"),
        r#"{"operation":"demo.add","id":"demo.add"}"#,
    )
    .unwrap();
    let o = rivet_async(
        d.path(),
        &["--file", "app.rivet", "request", "--input", "mixed.json"],
        None,
    )
    .await;
    assert_eq!(o.code, 2);
    assert_eq!(
        stderr_envelope(&o)["error"]["code"],
        "validation.input_envelope"
    );
    handle.shutdown().await;
}
