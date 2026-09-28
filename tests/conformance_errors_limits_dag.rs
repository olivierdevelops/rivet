//! T-24 — one error registry on every surface, limits and static checks.
//! Real failures are driven through the `rivet` binary (exit code +
//! ErrorEnvelope on stderr) and through `rivet serve` (HTTP status + the same
//! envelope); limits cover `max_concurrent_requests` (65th concurrent call),
//! `max_call_depth` (depth 17), literal call cycles, `output.invalid` with
//! preserved effects, and cancellation (exit 130).
//!
//! ```text
//!  kind ─▶ HTTP ─▶ exit          syntax/validation 422/2  auth 401/3  permission 403/3
//!                                not_found/conflict 404|409/4  limit 429/5  timeout 504/6
//!                                dependency 502/5  unsupported 501/5  output_invalid 500/5
//!                                cancelled 409/130
//! ```
#![allow(clippy::result_large_err)]

#[path = "support/mod.rs"]
mod serve_support;
#[path = "p3_support/mod.rs"]
mod support;

use rivet::Runtime;
use rivet::internal::domain::contracts::Principal;
use rivet::internal::domain::errors::ALL_KINDS;
use rivet::internal::domain::{ErrorKind, Value};
use rivet::internal::orchestrator::runtime::policy_from_json;
use rivet::internal::orchestrator::setup_serve::{ServeOptions, start};
use serde_json::json;
use std::time::Duration;
use support::*;

// vhco:test execution.request_operation -- every registry kind maps to the documented HTTP status, CLI exit and retryability (REF "Error registry")
#[test]
fn registry_table_matches_the_reference() {
    use ErrorKind::*;
    let table = [
        (Syntax, 422, 2),
        (Validation, 422, 2),
        (Auth, 401, 3),
        (Permission, 403, 3),
        (NotFound, 404, 4),
        (Conflict, 409, 4),
        (Limit, 429, 5),
        (Timeout, 504, 6),
        (Connection, 502, 5),
        (Dns, 502, 5),
        (Tls, 502, 5),
        (Protocol, 502, 5),
        (Http, 502, 5),
        (Application, 502, 5),
        (Unsupported, 501, 5),
        (OutputInvalid, 500, 5),
        (Internal, 500, 5),
        (Cleanup, 500, 5),
        (ConsumerFailed, 500, 5),
        (Parse, 502, 5),
        (Process, 502, 5),
        (Cancelled, 409, 130),
    ];
    assert_eq!(table.len(), ALL_KINDS.len(), "every kind is covered");
    for (kind, http, exit) in table {
        assert_eq!(kind.http_status(), http, "{kind:?} HTTP");
        assert_eq!(kind.exit_code(), exit, "{kind:?} exit");
        assert_eq!(kind.retryable(), kind == Limit, "{kind:?} retryable");
    }
}

/// The failure bundle: one operation per registry kind we can drive for real.
fn bundle(base: &str, closed: u16, port: u16) -> String {
    format!(
        r#"operation t.ok
    output integer
    return 1
end

operation t.need
    param a integer required
    output integer
    return a
end

operation t.denied
    output json
    r = http get "http://127.0.0.2:9/"
    return r.status
end

operation t.conflict
    output json
    file create "./out/once.json" json {{n: 1}}
    return 1
end

operation t.deep
    param n integer required
    param target text default "t.deep"
    output integer
    if n == 0
        return 0
    end
    next = (request target {{n: (n - 1)}})
        allow ["t.deep"]
    end
    return next + 1
end

operation t.slow
    param ms integer required
    output json
    r = http get "{base}/sleep/${{ms}}/slow"
        decode json
    end
    return r.body.tag
end

operation t.refused
    output json
    r = http get "http://127.0.0.1:{closed}/"
        timeout "2s"
    end
    return r.status
end

operation t.tls
    output json
    r = http get "https://127.0.0.1:{port}/"
        timeout "2s"
    end
    return r.status
end

operation t.status
    output json
    r = http get "{base}/status/500"
    return r.status
end

operation t.parse
    output json
    r = http get "{base}/text"
        decode json
    end
    return r.body
end

operation t.process
    output json
    r = command "/usr/bin/false"
    return r.exit
end

operation t.shell
    output json
    r = command "/bin/sh"
        args ["-c", "echo hi"]
    end
    return r
end

operation t.app
    output json
    fail "t.app_failure" {{why: "fixture"}}
end

operation t.brief
    output object
        field id integer required
        field name text required
    end
    file append "./out/audit.log" text "brief requested\n"
    return {{id: 1, display: "Ada"}}
end
"#
    )
}

fn policy_json(port: u16, closed: u16) -> String {
    format!(
        r#"{{"version":1,"grants":[
  {{"capability":"allow_network","targets":["http://127.0.0.1:{port}","https://127.0.0.1:{port}","http://127.0.0.1:{closed}"]}},
  {{"capability":"allow_write","targets":["./out/**"]}},
  {{"capability":"allow_exec","targets":["/usr/bin/false","/bin/sh"]}}]}}"#
    )
}

struct Bundle {
    dir: tempfile::TempDir,
    stats: std::sync::Arc<SlowStats>,
}

async fn write_bundle() -> Bundle {
    let (port, stats) = slow_server().await;
    let closed = closed_port().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("out")).unwrap();
    // `file append` never creates a missing file (S37); the log exists up front.
    std::fs::write(dir.path().join("out/audit.log"), "").unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        bundle(&format!("http://127.0.0.1:{port}"), closed, port),
    )
    .unwrap();
    std::fs::write(dir.path().join("policy.json"), policy_json(port, closed)).unwrap();
    Bundle { dir, stats }
}

/// `t.process` under policy.json: a sandboxed spawn really runs only on macOS. Linux (backend
/// gated until kernel >= 6.12) and Windows (no backend) refuse before spawning (ADR-0003).
const PROCESS_FAILURE: (&str, &str, i32, u16) = if cfg!(target_os = "macos") {
    ("process", "process.exit", 5, 502)
} else {
    ("unsupported", "unsupported.sandbox_backend", 5, 501)
};

/// (operation, params, kind, code, exit, HTTP)
const CASES: &[(&str, &str, &str, &str, i32, u16)] = &[
    ("t.need", "{}", "validation", "validation.required", 2, 422),
    (
        "t.need",
        r#"{"a":1,"c":2}"#,
        "validation",
        "validation.unknown_field",
        2,
        422,
    ),
    ("t.denied", "{}", "permission", "permission.denied", 3, 403),
    (
        "t.missing",
        "{}",
        "not_found",
        "not_found.operation",
        4,
        404,
    ),
    ("t.deep", r#"{"n":17}"#, "limit", "limit.call_depth", 5, 429),
    ("t.refused", "{}", "connection", "", 5, 502),
    ("t.tls", "{}", "tls", "", 5, 502),
    ("t.status", "{}", "http", "http.status", 5, 502),
    ("t.parse", "{}", "parse", "", 5, 502),
    (
        "t.process",
        "{}",
        PROCESS_FAILURE.0,
        PROCESS_FAILURE.1,
        PROCESS_FAILURE.2,
        PROCESS_FAILURE.3,
    ),
    ("t.shell", "{}", "unsupported", "unsupported.shell", 5, 501),
    ("t.app", "{}", "application", "t.app_failure", 5, 502),
    ("t.brief", "{}", "output_invalid", "output.invalid", 5, 500),
];

fn check_envelope(label: &str, e: &serde_json::Value, kind: &str, code: &str) {
    assert_eq!(e["kind"], kind, "{label}: {e}");
    if !code.is_empty() {
        assert_eq!(e["code"], code, "{label}: {e}");
    } else {
        assert!(
            e["code"].as_str().unwrap().starts_with(kind),
            "{label}: {e}"
        );
    }
}

// vhco:test execution.request_operation -- CLI: real failures of every drivable kind exit with the registry code and print one ErrorEnvelope on stderr
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_exit_codes_for_real_failures() {
    let b = write_bundle().await;
    let d = b.dir.path();
    let ok = rivet_async(
        d,
        &["--file", "app.rivet", "request", "t.ok", "--data", "{}"],
    )
    .await;
    assert_eq!(ok.code, 0, "{}", ok.stderr);
    assert_eq!(ok.json()["data"], 1);
    for (id, params, kind, code, exit, _) in CASES {
        let r = rivet_async(d, &["--file", "app.rivet", "request", id, "--data", params]).await;
        assert_eq!(r.code, *exit, "{id} {params}: {}", r.stderr);
        check_envelope(id, &r.error(), kind, code);
    }
    // conflict: the second exclusive create of the same file.
    let first = rivet_async(
        d,
        &[
            "--file",
            "app.rivet",
            "request",
            "t.conflict",
            "--data",
            "{}",
        ],
    )
    .await;
    assert_eq!(first.code, 0, "{}", first.stderr);
    let r = rivet_async(
        d,
        &[
            "--file",
            "app.rivet",
            "request",
            "t.conflict",
            "--data",
            "{}",
        ],
    )
    .await;
    assert_eq!(r.code, 4, "{}", r.stderr);
    check_envelope(
        "conflict",
        &r.error(),
        "conflict",
        "conflict.already_exists",
    );
    // timeout: the request deadline (--timeout) expires during the slow call.
    let r = rivet_async(
        d,
        &[
            "--file",
            "app.rivet",
            "request",
            "t.slow",
            "--data",
            r#"{"ms":5000}"#,
            "--timeout",
            "300ms",
        ],
    )
    .await;
    assert_eq!(r.code, 6, "{}", r.stderr);
    assert_eq!(r.error()["kind"], "timeout");
    assert!(
        b.stats.wait_aborted(1).await,
        "the timed-out call is aborted"
    );
    // validation at load time: an invalid policy.json is policy.invalid, exit 2.
    std::fs::write(d.join("bad.json"), r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./x"],"access":["delete"]}]}"#).unwrap();
    let r = rivet_async(
        d,
        &[
            "--json",
            "--file",
            "app.rivet",
            "--policy",
            "bad.json",
            "request",
            "t.ok",
            "--data",
            "{}",
        ],
    )
    .await;
    assert_eq!(r.code, 2, "{}", r.stderr);
    check_envelope("policy", &r.error(), "validation", "policy.invalid");
}

// vhco:test execution.request_operation -- output.invalid: the result is checked before Completion, exit 5 / HTTP 500, and the committed effect (appended audit line) is preserved and reported
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_invalid_preserves_effects() {
    let b = write_bundle().await;
    let d = b.dir.path();
    let r = rivet_async(
        d,
        &["--file", "app.rivet", "request", "t.brief", "--data", "{}"],
    )
    .await;
    assert_eq!(r.code, 5, "{}", r.stderr);
    let e = r.error();
    assert_eq!(e["kind"], "output_invalid");
    assert_eq!(e["code"], "output.invalid");
    assert_eq!(e["operation_id"], "t.brief");
    assert_eq!(r.envelope()["effects"], "committed");
    assert_eq!(r.envelope()["status"], "error");
    assert_eq!(e["details"]["missing"], json!(["name"]));
    assert_eq!(e["details"]["unexpected"], json!(["display"]));
    assert_eq!(
        std::fs::read_to_string(d.join("out/audit.log")).unwrap(),
        "brief requested\n",
        "the effect is not rolled back"
    );
}

// vhco:test execution.request_operation -- HTTP surface: the same failures answer POST /v1/request with the registry status and the same ErrorEnvelope kind/code
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_status_for_real_failures() {
    let b = write_bundle().await;
    let entry = b.dir.path().join("app.rivet");
    let rt = Runtime::builder()
        .file(entry.to_str().unwrap())
        .build()
        .unwrap();
    let h = start(
        rt,
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = h.addr.unwrap();
    for (id, params, kind, code, _, status) in CASES {
        let body = json!({"operation": id, "data": serde_json::from_str::<serde_json::Value>(params).unwrap()});
        let r = serve_support::post(addr, "/v1/request", body, &[]).await;
        assert_eq!(r.status, *status, "{id}: {}", r.text);
        let env = r.json();
        check_envelope(id, &env["error"], kind, code);
    }
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"operation":"t.ok","data":{}}),
        &[],
    )
    .await;
    assert_eq!((r.status, r.json()["data"].clone()), (200, json!(1)));
    h.shutdown().await;
}

// vhco:test serve.authenticate_principal -- auth kind: a missing bearer token is 401 over HTTP and exit 3 through `rivet --endpoint`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_failure_on_http_and_cli() {
    let hash = serve_support::sha256_hex("s3cret-token");
    let policy = json!({"version":1,"serve":{"auth":{"type":"bearer","tokens":[{"principal":"ada","sha256":hash}]}}}).to_string();
    let rt = Runtime::builder()
        .source(
            "app.rivet",
            "operation t.ok\n    output integer\n    return 1\nend\n",
            ".",
        )
        .policy(policy_from_json(policy.as_bytes(), ".").unwrap())
        .build()
        .unwrap();
    let h = start(
        rt,
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = h.addr.unwrap();
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"operation":"t.ok","data":{}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 401, "{}", r.text);
    assert_eq!(r.json()["error"]["kind"], "auth");
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{addr}");
    let cli = rivet_async(
        dir.path(),
        &["--endpoint", &url, "request", "t.ok", "--data", "{}"],
    )
    .await;
    assert_eq!(cli.code, 3, "{}", cli.stderr);
    std::fs::write(dir.path().join("token"), "s3cret-token\n").unwrap();
    let cli = rivet_async(
        dir.path(),
        &[
            "--endpoint",
            &url,
            "--token-file",
            "token",
            "request",
            "t.ok",
            "--data",
            "{}",
        ],
    )
    .await;
    assert_eq!(cli.code, 0, "{}", cli.stderr);
    h.shutdown().await;
}

// vhco:test execution.request_operation -- limits.max_call_depth: depth 16 runs, depth 17 is limit.call_depth (429 / exit 5, retryable)
#[tokio::test]
async fn call_depth_limit() {
    let src = "operation t.deep\n    param n integer required\n    param target text default \"t.deep\"\n    output integer\n    if n == 0\n        return 0\n    end\n    next = (request target {n: (n - 1)})\n        allow [\"t.deep\"]\n    end\n    return next + 1\nend\n";
    let rt = runtime(src, ".", r#"{"version":1}"#);
    let c = rt
        .request("t.deep", Value::object([("n", Value::Int(16))]), None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::Int(16));
    let e = rt
        .request("t.deep", Value::object([("n", Value::Int(17))]), None)
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Limit, "limit.call_depth")
    );
    assert!(e.retryable);
    assert_eq!((e.http_status(), e.exit_code()), (429, 5));
    // A lower policy limit applies the same rule.
    let rt = runtime(src, ".", r#"{"version":1,"limits":{"max_call_depth":3}}"#);
    assert!(
        rt.request("t.deep", Value::object([("n", Value::Int(3))]), None)
            .await
            .is_ok()
    );
    let e = rt
        .request("t.deep", Value::object([("n", Value::Int(4))]), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "limit.call_depth");
}

fn slow_src(base: &str) -> String {
    format!(
        "operation t.slow\n    param ms integer required\n    output json\n    r = http get \"{base}/sleep/${{ms}}/s\"\n        decode json\n    end\n    return r.body.tag\nend\n\n\
operation t.fan\n    param items json required\n    output json\n    r = map i in items limit 100\n        yield (request \"t.slow\" {{ms: 1500}})\n    end\n    return (length r)\nend\n"
    )
}

// vhco:test execution.request_operation -- limits.max_concurrent_requests (64): with 64 requests in flight the 65th top-level request is limit.concurrency; capacity returns afterwards
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sixty_fifth_concurrent_request_is_refused() {
    let (port, stats) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let rt = runtime(&slow_src(&base), ".", &net_policy(&base));
    let mut running = Vec::new();
    for _ in 0..64 {
        let rt = rt.clone();
        running.push(tokio::spawn(async move {
            rt.request("t.slow", Value::object([("ms", Value::Int(1500))]), None)
                .await
        }));
    }
    assert!(stats.wait_started(64).await, "64 requests in flight");
    let e = rt
        .request("t.slow", Value::object([("ms", Value::Int(1))]), None)
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Limit, "limit.concurrency")
    );
    assert_eq!((e.http_status(), e.exit_code()), (429, 5));
    for r in running {
        assert!(r.await.unwrap().is_ok());
    }
    assert!(
        rt.request("t.slow", Value::object([("ms", Value::Int(1))]), None)
            .await
            .is_ok(),
        "permits are released"
    );
}

// vhco:test execution.request_operation -- nested calls share the host-wide budget: a parent plus 63 concurrent nested calls fit in 64; one more nested call makes the 65th concurrent request and is limit.concurrency
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nested_calls_share_the_concurrency_budget() {
    let (port, _) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let rt = runtime(&slow_src(&base), ".", &net_policy(&base));
    let items = |n: usize| Value::object([("items", Value::List(vec![Value::Int(0); n]))]);
    let c = rt.request("t.fan", items(63), None).await.unwrap();
    assert_eq!(c.result, Value::Int(63));
    let e = rt.request("t.fan", items(64), None).await.unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Limit, "limit.concurrency")
    );
}

// vhco:test language.compile_program -- a literal `(request "ID" …)` cycle is check.call_cycle (syntax kind, exit 2) from `rivet check`; nothing runs
#[test]
fn literal_call_cycle_is_rejected_by_check() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        "operation a.one\n    output json\n    return (request \"a.two\" {})\nend\n\noperation a.two\n    output json\n    return (request \"a.one\" {})\nend\n",
    )
    .unwrap();
    let r = rivet(dir.path(), &["--file", "app.rivet", "check"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(r.stderr.contains("error[check.call_cycle]"), "{}", r.stderr);
    assert!(r.stderr.contains("a.one → a.two → a.one"), "{}", r.stderr);
    assert!(r.stderr.contains("--> app.rivet:6:1"), "{}", r.stderr);
}

// vhco:test language.compile_program -- `check` warns (exit 0) on an unguarded `.result` of a fail-independent node and on an undeclared `fail` code; `--strict-docs` makes the undeclared code an error (exit 2)
#[test]
fn check_warnings_for_unguarded_result_and_undeclared_codes() {
    let dir = tempfile::tempdir().unwrap();
    let src = "operation t.run\n    description \"Run.\"\n    output json description \"Result.\"\n    dag fail independent\n        node a = 1\n    end\n    if a.status == \"succeeded\"\n        return a.result\n    end\n    fail \"t.undeclared\" {}\n    return a.result\nend\n";
    std::fs::write(dir.path().join("app.rivet"), src).unwrap();
    let r = rivet(dir.path(), &["--file", "app.rivet", "check"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let warnings: Vec<&str> = r
        .stderr
        .lines()
        .filter(|l| l.starts_with("warning["))
        .collect();
    assert_eq!(warnings.len(), 2, "{}", r.stderr);
    assert!(
        r.stderr.contains("warning[docs.undeclared_error]"),
        "{}",
        r.stderr
    );
    assert!(r.stderr.contains("--> app.rivet:10:5"), "{}", r.stderr);
    assert!(
        r.stderr.contains("warning[check.unguarded_result]"),
        "{}",
        r.stderr
    );
    assert!(
        r.stderr.contains("--> app.rivet:11:5"),
        "the guarded return on line 8 is fine: {}",
        r.stderr
    );
    assert!(!r.stderr.contains("app.rivet:8:"), "{}", r.stderr);

    let r = rivet(
        dir.path(),
        &["--file", "app.rivet", "check", "--strict-docs"],
    );
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(
        r.stderr.contains("error[docs.undeclared_error]"),
        "{}",
        r.stderr
    );
    assert!(
        !r.stderr.contains("warning[docs.undeclared_error]"),
        "{}",
        r.stderr
    );
}

// vhco:test execution.cancel_request -- SIGINT during `rivet request` cancels the running request: one `cancelled` ErrorEnvelope, exit 130, the in-flight call is aborted
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigint_cancels_with_exit_130() {
    let b = write_bundle().await;
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args([
            "--file",
            "app.rivet",
            "request",
            "t.slow",
            "--data",
            r#"{"ms":10000}"#,
        ])
        .current_dir(b.dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    assert!(b.stats.wait_started(1).await, "the request is in flight");
    // SAFETY: plain kill(2) on our own child's PID.
    let rc = unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    assert_eq!(rc, 0);
    let out = tokio::task::spawn_blocking(move || child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(130), "{stderr}");
    let run = Run {
        code: 130,
        stdout: String::new(),
        stderr,
    };
    assert_eq!(run.error()["kind"], "cancelled");
    assert!(
        b.stats.wait_aborted(1).await,
        "cleanup closed the upstream call"
    );
    assert_eq!(b.stats.get(|s| &s.finished), 0);
}

// vhco:test execution.cancel_request -- a library host cancels its own running request by ID: kind cancelled (409 before headers, exit 130) and the upstream call is aborted
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_cancel_by_request_id() {
    let (port, stats) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let rt = runtime(&slow_src(&base), ".", &net_policy(&base));
    let req = rt.new_request(
        "t.slow",
        Value::object([("ms", Value::Int(10_000))]),
        Principal::local(),
    );
    let id = req.request_id.clone();
    let rt2 = rt.clone();
    let task = tokio::spawn(async move { rt2.dispatch_request(req, None).await });
    assert!(stats.wait_started(1).await);
    rt.cancel(&id, Principal::local()).unwrap();
    let e = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("cancel is prompt")
        .unwrap()
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Cancelled);
    assert_eq!((e.http_status(), e.exit_code()), (409, 130));
    assert!(stats.wait_aborted(1).await);
}

/// One-route HTTP fixture answering every request with `len` bytes of JSON text.
async fn big_body_server(len: usize) -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let mut seen = Vec::new();
                while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => seen.extend_from_slice(&buf[..n]),
                    }
                }
                let body = format!("\"{}\"", "x".repeat(len - 2));
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes()).await;
                let _ = s.write_all(body.as_bytes()).await;
                let _ = s.shutdown().await;
            });
        }
    });
    port
}

// vhco:test execution.request_operation -- G11 default size limits follow the proposal (8 MiB frame/body, 16-frame / 32 MiB session queues, 256 MiB host budget): a 9 MiB HTTP body is limit.http_body by default and passes with an explicit `max_body`
#[tokio::test]
async fn default_body_limit_is_8_mib_and_max_body_overrides() {
    let defaults = rivet::internal::domain::sessions::SessionLimits::default();
    assert_eq!(
        (defaults.queue_frames, defaults.queue_bytes),
        (16, 32 << 20)
    );
    assert_eq!(
        rivet::internal::domain::transport::DEFAULT_MAX_FRAME,
        8 << 20
    );
    assert_eq!(
        rivet::internal::domain::policy::PolicyLimits::default().max_buffered_bytes,
        256 << 20
    );
    let port = big_body_server(9 << 20).await;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    let policy = format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["http://127.0.0.1:{port}"]}}]}}"#
    );
    let src = format!(
        "operation t.default\n    output json\n    r = http get \"http://127.0.0.1:{port}/big\"\n        decode json\n    end\n    return (length r.body)\nend\n\noperation t.raised\n    output json\n    r = http get \"http://127.0.0.1:{port}/big\"\n        max_body 16777216\n        decode json\n    end\n    return (length r.body)\nend\n"
    );
    let rt = Runtime::builder()
        .source("app.rivet", &src, root)
        .policy(policy_from_json(policy.as_bytes(), root).unwrap())
        .build()
        .unwrap();
    let e = rt
        .request("t.default", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "limit.http_body", "{e:?}");
    let c = rt.request("t.raised", Value::Null, None).await.unwrap();
    assert_eq!(c.result, Value::Int((9 << 20) - 2));
}
