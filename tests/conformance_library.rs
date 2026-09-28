//! T-10 — the Rust library is a surface: a `Runtime` compiled from in-memory
//! source runs inside the host's Tokio runtime (no nested runtime), streams to
//! a host `DataSink`, owns cleanup when the host drops a request future
//! (sockets closed, child processes killed), and exposes outputs, the I/O
//! manifest, policy drafts, sessions and `policy_from_json`.
//!
//! ```text
//!  host tokio runtime
//!    └─ Runtime::builder().source(..).policy(policy_from_json(..)).build()
//!         ├─ request(id, params, Some(sink))   ── sink Err ─▶ consumer_failed
//!         ├─ drop(request future)             ── closes sockets, kills children
//!         ├─ open_session / read_events       ── Envelope stream (scope.stream stand-in)
//!         └─ outputs / io / generate_policy
//! ```
#![allow(clippy::result_large_err)]

#[path = "p3_support/mod.rs"]
mod support;

use async_trait::async_trait;
use rivet::Runtime;
use rivet::domain::contracts::{DataEvent, Envelope, Principal};
use rivet::domain::io_manifest::IoQuery;
use rivet::domain::outputs::ValueSpec;
use rivet::domain::ports::DataSink;
use rivet::domain::sessions::{SessionOpenInput, SessionReadInput};
use rivet::domain::{ErrorKind, RivetError, RivetResult, Value};
use rivet::orchestrator::runtime::policy_from_json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use support::*;

const CATALOG: &str = "operation demo.add
    name \"Add two integers\"
    description \"Add two signed integers and return their sum.\"
    param a integer required description \"First operand.\"
    param b integer default 0 description \"Second operand; defaults to zero.\"
    output integer description \"Sum of a and b.\"
    return a + b
end

operation demo.greet
    param person text required
    output text
    return \"Hello, ${person}!\"
end

operation demo.count
    param n integer default 3
    output integer
    emits integer
    i = 0
    iterate max 1000
        if i == n
            break
        end
        i = i + 1
        emit i
    end
    return n
end
";

// vhco:test execution.request_operation -- S106: one in-memory compilation serves many calls inside the host's current-thread Tokio runtime; building never starts a nested runtime
#[tokio::test(flavor = "current_thread")]
async fn runtime_lives_inside_the_host_runtime() {
    let rt = Runtime::builder()
        .source("catalog.rivet", CATALOG, ".")
        .policy(policy_from_json(br#"{"version": 1}"#, ".").unwrap())
        .build()
        .expect("build inside a running runtime (no block_on, no nested runtime)");
    let sum = rt
        .request(
            "demo.add",
            Value::object([("a", Value::Int(2)), ("b", Value::Int(3))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(sum.result, Value::Int(5));
    let hi = rt
        .request(
            "demo.greet",
            Value::object([("person", Value::text("Ada"))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(hi.result, Value::text("Hello, Ada!"));
    // Concurrent requests on one runtime isolate their locals.
    let (x, y) = tokio::join!(
        rt.request("demo.add", Value::object([("a", Value::Int(1))]), None),
        rt.request(
            "demo.add",
            Value::object([("a", Value::Int(40)), ("b", Value::Int(2))]),
            None
        ),
    );
    assert_eq!(
        (x.unwrap().result, y.unwrap().result),
        (Value::Int(1), Value::Int(42))
    );
    assert_ne!(sum.request_id, hi.request_id);
}

/// Accepts `limit` items, then fails (the host stops consuming).
struct StopAfter {
    limit: usize,
    seen: Mutex<Vec<Value>>,
}

#[async_trait]
impl DataSink for StopAfter {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        let mut seen = self.seen.lock().unwrap();
        if seen.len() >= self.limit {
            return Err(RivetError::new(
                ErrorKind::ConsumerFailed,
                "consumer_failed",
                "host has enough",
            ));
        }
        seen.push(event.data);
        Ok(())
    }
}

// vhco:test execution.request_operation -- S70/S71: a DataSink receives ordered items; a sink that stops early ends the request with consumer_failed (500 / exit 5) and no further items are produced
#[tokio::test]
async fn data_sink_that_stops_early() {
    let rt = runtime(CATALOG, ".", r#"{"version":1}"#);
    let all = Arc::new(StopAfter {
        limit: usize::MAX,
        seen: Mutex::new(Vec::new()),
    });
    let c = rt
        .request(
            "demo.count",
            Value::object([("n", Value::Int(3))]),
            Some(all.clone()),
        )
        .await
        .unwrap();
    assert_eq!(c.result, Value::Int(3));
    assert_eq!(c.data_count, 3);
    assert_eq!(
        *all.seen.lock().unwrap(),
        vec![Value::Int(1), Value::Int(2), Value::Int(3)]
    );

    let first = Arc::new(StopAfter {
        limit: 1,
        seen: Mutex::new(Vec::new()),
    });
    let e = rt
        .request(
            "demo.count",
            Value::object([("n", Value::Int(500))]),
            Some(first.clone()),
        )
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::ConsumerFailed, "consumer_failed")
    );
    assert_eq!((e.http_status(), e.exit_code()), (500, 5));
    assert_eq!(*first.seen.lock().unwrap(), vec![Value::Int(1)]);
}

fn effect_src(base: &str) -> String {
    format!(
        "operation t.slow\n    param ms integer required\n    output json\n    r = http get \"{base}/sleep/${{ms}}/drop\"\n    return r.status\nend\n\n\
operation t.sleep\n    param marker text required\n    output json\n    r = command \"/bin/sleep\"\n        args [marker]\n    end\n    return r.exit\nend\n\n\
operation t.nested\n    output json\n    return (request \"t.slow\" {{ms: 10000}})\nend\n"
    )
}

fn effect_policy(base: &str) -> String {
    format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["{base}"]}},{{"capability":"allow_exec","targets":["/bin/sleep"]}}]}}"#
    )
}

fn process_running(marker: &str) -> bool {
    let ps = std::process::Command::new("/bin/ps")
        .args(["-axo", "command"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&ps.stdout)
        .lines()
        .any(|l| l.contains(marker) && !l.contains("/bin/ps"))
}

// vhco:test execution.request_operation -- dropping a request future mid-flight closes its open socket: the local fixture observes the client close, also for a nested child request
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_future_closes_the_socket() {
    let (port, stats) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let rt = runtime(&effect_src(&base), ".", &effect_policy(&base));
    let slow = Value::object([("ms", Value::Int(10_000))]);
    for (n, id, params) in [(1, "t.slow", slow), (2, "t.nested", Value::Null)] {
        let rt2 = rt.clone();
        let fut = tokio::spawn(async move { rt2.request(id, params, None).await });
        assert!(stats.wait_started(n).await, "{id} in flight");
        fut.abort();
        assert!(fut.await.unwrap_err().is_cancelled());
        assert!(
            stats.wait_aborted(n).await,
            "{id}: the socket is closed when the host drops the future"
        );
    }
    assert_eq!(stats.get(|s| &s.finished), 0);
}

// vhco:test execution.request_operation -- dropping a request future mid-flight leaves no running child process (killed and reaped by the runtime)
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_future_kills_the_child_process() {
    let marker = format!("31.{}", std::process::id());
    let rt = runtime(
        &effect_src("http://127.0.0.1:1"),
        ".",
        &effect_policy("http://127.0.0.1:1"),
    );
    let rt2 = rt.clone();
    let m = marker.clone();
    let fut = tokio::spawn(async move {
        rt2.request("t.sleep", Value::object([("marker", Value::text(m))]), None)
            .await
    });
    let mut seen = false;
    for _ in 0..100 {
        if process_running(&marker) {
            seen = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert!(seen, "the child started");
    fut.abort();
    let _ = fut.await;
    let mut gone = false;
    for _ in 0..100 {
        if !process_running(&marker) {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert!(gone, "no child process survives the dropped future");
}

// vhco:test sessions.read_events -- the host pulls a request's stream as ordered Data envelopes followed by exactly one terminal Result (the library stream API)
#[tokio::test]
async fn stream_pull_in_the_host() {
    let rt = runtime(CATALOG, ".", r#"{"version":1}"#);
    let receipt = rt
        .open_session(SessionOpenInput {
            id: "demo.count".into(),
            params: Value::object([("n", Value::Int(3))]),
            principal: Principal::local(),
            connection_owned: false,
            deadline_ms: None,
        })
        .await
        .unwrap();
    let mut after = 0;
    let mut data = Vec::new();
    let mut result = None;
    for _ in 0..50 {
        let batch = rt
            .read_events(SessionReadInput {
                session_id: receipt.session_id.clone(),
                after_seq: after,
                max_events: None,
                wait_ms: Some(500),
                principal: Principal::local(),
            })
            .await
            .unwrap();
        for ev in &batch.events {
            match &ev.envelope {
                Envelope::Data(d) => data.push(d.data.clone()),
                Envelope::Result(c) => result = Some(c.result.clone()),
                Envelope::Error { error, .. } => panic!("unexpected error {error}"),
            }
        }
        after = batch.last_seq;
        if batch.terminal {
            break;
        }
    }
    assert_eq!(data, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    assert_eq!(result, Some(Value::Int(3)));
    // Another principal cannot read this session.
    let e = rt
        .read_events(SessionReadInput {
            session_id: receipt.session_id.clone(),
            after_seq: 0,
            max_events: None,
            wait_ms: None,
            principal: Principal {
                name: "mallory".into(),
                ..Principal::local()
            },
        })
        .await
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
}

// vhco:test registry.inspect_outputs -- S106: rt.outputs returns the declared output (integer, "Sum of a and b."); private and unknown IDs are not_found
#[tokio::test]
async fn outputs_from_the_library() {
    let src = format!(
        "{CATALOG}\noperation t.hidden\n    private true\n    output integer\n    return 1\nend\n"
    );
    let rt = runtime(&src, ".", r#"{"version":1}"#);
    let spec = rt.outputs(Some("demo.add"), false).unwrap();
    assert_eq!(spec.len(), 1);
    assert_eq!(spec[0].output.spec, ValueSpec::Integer);
    assert_eq!(
        spec[0].output.description.as_deref(),
        Some("Sum of a and b.")
    );
    assert_eq!(spec[0].name, "Add two integers");
    for id in ["t.hidden", "t.nope"] {
        let e = rt.outputs(Some(id), false).unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound, "{id}");
    }
    let all = rt.outputs(None, true).unwrap();
    let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["demo.add", "demo.count", "demo.greet"]);
}

// vhco:test audit.inspect_effects -- rt.io lists the HTTP and exec sites of the bundle and rt.generate_policy drafts exactly those grants without writing a file
#[tokio::test]
async fn io_manifest_and_policy_draft_from_the_library() {
    let base = "http://127.0.0.1:8";
    let rt = runtime(&effect_src(base), ".", r#"{"version":1}"#);
    let report = rt
        .io(&IoQuery {
            ids: vec!["t.slow".into(), "t.sleep".into()],
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap();
    let sites = report.manifest.to_json()["sites"].clone();
    let caps: Vec<String> = sites
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["capability"].as_str().unwrap().to_string())
        .collect();
    assert!(caps.contains(&"allow_network".to_string()), "{sites}");
    assert!(caps.contains(&"allow_exec".to_string()), "{sites}");

    let draft = rt.generate_policy(&["t.sleep"]).unwrap();
    assert!(draft.written_to.is_none());
    let json: serde_json::Value = serde_json::from_str(&draft.render()).unwrap();
    assert_eq!(json["version"], 1);
    let grants = json["grants"].as_array().unwrap();
    assert!(
        grants.iter().any(|g| g["capability"] == "allow_exec"
            && g["targets"] == serde_json::json!(["/bin/sleep"])),
        "{json}"
    );
    assert!(
        grants.iter().all(|g| g["capability"] != "allow_network"),
        "only the requested operation's grants: {json}"
    );
}

// vhco:test policy.load_policy -- policy_from_json (Policy::from_json) accepts the policy.json schema; an empty policy denies new I/O; invalid JSON or a foreign access verb is policy.invalid
#[tokio::test]
async fn policy_from_json_matches_policy_files() {
    let (port, stats) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = effect_src(&base);
    let deny = runtime(&src, ".", r#"{"version": 1}"#);
    let e = deny
        .request("t.slow", Value::object([("ms", Value::Int(1))]), None)
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Permission, "permission.denied")
    );
    assert_eq!(stats.get(|s| &s.started), 0, "denied before any connection");

    let allow = runtime(&src, ".", &net_policy(&base));
    let c = allow
        .request("t.slow", Value::object([("ms", Value::Int(1))]), None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::Int(200));

    for bad in [
        "{not json",
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./x"],"access":["delete"]}]}"#,
        r#"{"version":1,"surprise":true}"#,
    ] {
        let e = policy_from_json(bad.as_bytes(), ".").unwrap_err();
        assert_eq!(
            (e.kind, e.code.as_str()),
            (ErrorKind::Validation, "policy.invalid"),
            "{bad}"
        );
    }
}
