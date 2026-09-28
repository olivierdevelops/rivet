//! T-07 — DAGs: diamond, fan-out, `limit`, fail fast (the default) and fail
//! independent, the dag timeout, node envelopes `{status, result, error}` and
//! compile-time cycle / undeclared-read rejection. Slow nodes call a local
//! HTTP fixture that counts concurrent requests and observes client aborts.
//!
//! ```text
//!  pending ──deps succeeded──▶ running ──▶ succeeded | failed | cancelled
//!     ├── dependency failed/blocked ──▶ blocked
//!     └── fail fast before ready ─────▶ skipped
//! ```
#![allow(clippy::result_large_err)]

#[path = "p3_support/mod.rs"]
mod support;

use rivet::Runtime;
use rivet::domain::{ErrorKind, Value};
use std::time::{Duration, Instant};
use support::*;

/// Fixture operations (REF "Named operation fixtures"): `t.slow` waits on
/// the HTTP fixture, `fixture.fail` raises, `fixture.echo` returns its value.
fn fixtures(base: &str) -> String {
    format!(
        "operation t.slow\n    param ms integer required\n    param tag text required\n    output json\n    r = http get \"{base}/sleep/${{ms}}/${{tag}}\"\n        decode json\n    end\n    return r.body.tag\nend\n\n\
operation fixture.fail\n    output json\n    fail \"application.fixture_failure\" {{}}\nend\n\n\
operation t.fail_later\n    output json\n    x = (request \"t.slow\" {{ms: 200, tag: \"x\"}})\n    fail \"application.fixture_failure\" {{after: x}}\nend\n\n\
operation fixture.echo\n    param value json required\n    output json\n    return value\nend\n\n\
operation summary.make\n    param user json required\n    param orders json required\n    output json\n    return {{user: user, orders: orders}}\nend\n"
    )
}

async fn setup(ops: &str) -> (Runtime, std::sync::Arc<SlowStats>) {
    let (port, stats) = slow_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!("{}\n{ops}", fixtures(&base));
    (runtime(&src, ".", &net_policy(&base)), stats)
}

fn statuses(details: &Value) -> Vec<(String, String)> {
    let Some(Value::List(nodes)) = details.get("nodes") else {
        panic!("details.nodes missing: {details}");
    };
    nodes
        .iter()
        .map(|n| {
            (
                n.get("id").and_then(Value::as_str).unwrap().to_string(),
                n.get("status").and_then(Value::as_str).unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

// vhco:test execution.run_dag -- S44 diamond: the two roots run concurrently and the join starts only after both succeeded, reading their `.result`
#[tokio::test]
async fn diamond_graph() {
    let (rt, stats) = setup(
        "operation t.diamond\n    output json\n    dag limit 4 timeout \"10s\" fail fast\n        node user = (request \"t.slow\" {ms: 300, tag: \"u\"})\n        node orders = (request \"t.slow\" {ms: 300, tag: \"o\"})\n        node summary after [user, orders] = (request \"summary.make\" {\n            user: user.result, orders: orders.result\n        })\n    end\n    return summary.result\nend\n",
    )
    .await;
    let c = rt.request("t.diamond", Value::Null, None).await.unwrap();
    assert_eq!(
        c.result,
        Value::object([("user", Value::text("u")), ("orders", Value::text("o"))])
    );
    assert_eq!(stats.get(|s| &s.max_in_flight), 2, "roots overlap");
    assert_eq!(stats.get(|s| &s.finished), 2);
}

// vhco:test execution.run_dag -- fan-out: independent nodes are capped by `dag limit N`, every node succeeds and results keep declaration order
#[tokio::test]
async fn fan_out_respects_the_limit() {
    let nodes: String = (0..6)
        .map(|i| format!("        node n{i} = (request \"t.slow\" {{ms: 150, tag: \"n{i}\"}})\n"))
        .collect();
    let (rt, stats) = setup(&format!(
        "operation t.fan\n    output json\n    dag limit 2\n{nodes}    end\n    return [n0.result, n1.result, n2.result, n3.result, n4.result, n5.result, n5.status]\nend\n"
    ))
    .await;
    let c = rt.request("t.fan", Value::Null, None).await.unwrap();
    let want: Vec<Value> = (0..6)
        .map(|i| Value::text(format!("n{i}")))
        .chain([Value::text("succeeded")])
        .collect();
    assert_eq!(c.result, Value::List(want));
    assert_eq!(
        stats.get(|s| &s.max_in_flight),
        2,
        "never more than `limit 2` nodes in flight"
    );
    assert_eq!(stats.get(|s| &s.started), 6);
}

// vhco:test execution.run_dag -- S47 bounded `map` fan-out: at most `limit` calls in flight, results in input order
#[tokio::test]
async fn map_fan_out_is_bounded_and_ordered() {
    let (rt, stats) = setup(
        "operation t.map\n    output json\n    results = map item in [\"a\", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\"] limit 3\n        yield (request \"t.slow\" {ms: 120, tag: item})\n    end\n    return results\nend\n",
    )
    .await;
    let c = rt.request("t.map", Value::Null, None).await.unwrap();
    let want: Vec<Value> = ["a", "b", "c", "d", "e", "f", "g"]
        .iter()
        .map(|s| Value::text(*s))
        .collect();
    assert_eq!(c.result, Value::List(want));
    assert_eq!(stats.get(|s| &s.max_in_flight), 3);
}

// vhco:test execution.run_dag -- S138 fail fast is the default: the first failure fails the dag; the running sibling is cancelled (its connection closed), never-started nodes are skipped, dependants of the failure are blocked
#[tokio::test]
async fn fail_fast_is_the_default() {
    let (rt, stats) = setup(
        "operation report.build\n    description \"Fetch a user and orders, then assemble a summary.\"\n    param id integer required min 1 description \"User ID.\"\n    output json description \"Summary object.\"\n    dag limit 2 timeout \"10s\"\n        node user = (request \"t.slow\" {ms: 5000, tag: \"user\"})\n        node orders = (request \"t.fail_later\" {})\n        node enrich after [user] = (request \"fixture.echo\" {value: user.result})\n        node summary after [user, orders] = (request \"summary.make\" {user: user.result, orders: orders.result})\n    end\n    return summary.result\nend\n",
    )
    .await;
    let started = Instant::now();
    let e = rt
        .request(
            "report.build",
            Value::object([("id", Value::Int(42))]),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "fail fast must not wait for the slow sibling ({:?})",
        started.elapsed()
    );
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Application, "application.fixture_failure")
    );
    assert_eq!(e.node_id.as_deref(), Some("orders"));
    assert_eq!((e.exit_code(), e.http_status()), (5, 502));
    assert_eq!(
        statuses(&e.details),
        pairs(&[
            ("user", "cancelled"),
            ("orders", "failed"),
            ("enrich", "skipped"),
            ("summary", "blocked"),
        ])
    );
    assert!(
        stats.wait_aborted(1).await,
        "the cancelled node's HTTP request is aborted, not left running"
    );
    assert_eq!(
        stats.get(|s| &s.finished),
        1,
        "only the failing node's own call completed"
    );
}

// vhco:test execution.run_dag -- S46 fail independent: the independent node succeeds, the failed node's descendant is blocked, and every node name is a {status, result, error} envelope with `.result` null unless succeeded
#[tokio::test]
async fn fail_independent_keeps_independent_results() {
    let (rt, _) = setup(
        "operation t.indep\n    output json\n    dag limit 2 fail independent\n        node good = (request \"fixture.echo\" {value: \"ok\"})\n        node bad = (request \"fixture.fail\" {})\n        node blocked after [bad] = (request \"fixture.echo\" {value: bad.result})\n    end\n    return {good: good, bad: bad, blocked: blocked}\nend\n",
    )
    .await;
    let c = rt.request("t.indep", Value::Null, None).await.unwrap();
    let r = &c.result;
    assert_eq!(
        r.get("good").unwrap(),
        &Value::object([
            ("status", Value::text("succeeded")),
            ("result", Value::text("ok")),
            ("error", Value::Null),
        ])
    );
    let bad = r.get("bad").unwrap();
    assert_eq!(bad.get("status"), Some(&Value::text("failed")));
    assert_eq!(bad.get("result"), Some(&Value::Null));
    assert_eq!(
        bad.get("error").unwrap().get("code"),
        Some(&Value::text("application.fixture_failure"))
    );
    assert_eq!(
        bad.get("error").unwrap().get("node_id"),
        Some(&Value::text("bad"))
    );
    assert_eq!(
        r.get("blocked").unwrap(),
        &Value::object([
            ("status", Value::text("blocked")),
            ("result", Value::Null),
            ("error", Value::Null),
        ])
    );
}

// vhco:test execution.run_dag -- the dag timeout fails with timeout.dag (504 / exit 6), cancels running nodes and skips the rest
#[tokio::test]
async fn dag_timeout_cancels_running_nodes() {
    let (rt, stats) = setup(
        "operation t.late\n    output json\n    dag limit 1 timeout \"300ms\"\n        node slow = (request \"t.slow\" {ms: 5000, tag: \"s\"})\n        node never = (request \"fixture.echo\" {value: 1})\n    end\n    return never.result\nend\n",
    )
    .await;
    let started = Instant::now();
    let e = rt.request("t.late", Value::Null, None).await.unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Timeout, "timeout.dag")
    );
    assert_eq!((e.exit_code(), e.http_status()), (6, 504));
    assert_eq!(
        statuses(&e.details),
        pairs(&[("slow", "cancelled"), ("never", "skipped")])
    );
    assert!(stats.wait_aborted(1).await);
}

// vhco:test execution.run_dag -- a pure dag computes node values in dependency order; nodes may call operations that run their own nested dag
#[tokio::test]
async fn nodes_call_nested_requests() {
    let (rt, _) = setup(
        "operation t.inner\n    param x integer required\n    output json\n    dag\n        node a = x + 1\n        node b after [a] = a.result * 10\n    end\n    return b.result\nend\n\n\
operation t.outer\n    output json\n    dag limit 2\n        node one = (request \"t.inner\" {x: 1})\n        node two = (request \"t.inner\" {x: 2})\n        node sum after [one, two] = one.result + two.result\n    end\n    return {sum: sum.result, status: sum.status}\nend\n",
    )
    .await;
    let c = rt.request("t.outer", Value::Null, None).await.unwrap();
    assert_eq!(
        c.result,
        Value::object([
            ("sum", Value::Int(50)),
            ("status", Value::text("succeeded"))
        ])
    );
}

// vhco:test language.compile_program -- `after` cycles are syntax.dag_cycle and reading another node without `after` is syntax.dag, both at compile time with spans
#[test]
fn cycles_and_undeclared_reads_are_compile_errors() {
    let body = |dag: &str| format!("operation t.run\n    output json\n{dag}    return 1\nend\n");
    let e = compile(&body(
        "    dag\n        node a after [b] = 1\n        node b after [a] = 2\n    end\n",
    ))
    .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Syntax, "syntax.dag_cycle")
    );
    assert_eq!(span(&e), (3, 5, 6, 8));

    let e = compile(&body(
        "    dag\n        node a = 1\n        node b = a.result\n    end\n",
    ))
    .unwrap_err();
    assert_eq!(e.code, "syntax.dag");
    assert!(
        e.message.contains("without declaring `after [a]`"),
        "{}",
        e.message
    );
    assert_eq!(span(&e), (5, 9, 5, 26));

    let e = compile(&body("    dag\n        node b after [zz] = 1\n    end\n")).unwrap_err();
    assert_eq!(e.code, "syntax.dag");
    assert!(
        all_errors(&e).iter().all(|x| x.code != "syntax.dag_cycle"),
        "an unknown dependency is not a cycle: {:?}",
        all_errors(&e)
    );

    // Regression: a repeated dependency is not a cycle.
    compile(&body(
        "    dag\n        node a = 1\n        node b after [a, a] = a.result\n    end\n",
    ))
    .expect("after [a, a] is a plain edge");
    // Regression: duplicated node names are reported, never an arithmetic panic.
    let e = compile(&body(
        "    dag\n        node a = 1\n        node a = 2\n        node b after [a] = 3\n    end\n",
    ))
    .unwrap_err();
    assert_eq!(e.code, "syntax.dag");
    assert!(e.message.contains("declared twice"));
}
