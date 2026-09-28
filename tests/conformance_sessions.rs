//! T-19 — live sessions: open/send/read/finish/cancel through the library,
//! the dispatcher's `rivet.sessions.*` operations and the polling routes;
//! duplicate sends, sequence conflicts, stale cursors, backpressure,
//! cross-principal access and per-principal limits.

mod support;

use rivet::domain::contracts::Principal;
use rivet::domain::sessions::{SessionOpenInput, SessionReadInput, SessionRef, SessionSendInput};
use rivet::domain::{ErrorKind, Value};
use serde_json::json;
use support::*;

fn open(id: &str, who: &Principal) -> SessionOpenInput {
    SessionOpenInput {
        id: id.into(),
        params: Value::Null,
        principal: who.clone(),
        connection_owned: false,
        deadline_ms: None,
        trace: None,
    }
}

fn read(sid: &str, after: u64, who: &Principal) -> SessionReadInput {
    SessionReadInput {
        session_id: sid.into(),
        after_seq: after,
        max_events: None,
        wait_ms: Some(2000),
        principal: who.clone(),
    }
}

fn send(sid: &str, seq: u64, data: &str, who: &Principal) -> SessionSendInput {
    SessionSendInput {
        session_id: sid.into(),
        send_seq: seq,
        data: Value::text(data),
        principal: who.clone(),
    }
}

fn sref(sid: &str, who: &Principal) -> SessionRef {
    SessionRef {
        session_id: sid.into(),
        principal: who.clone(),
    }
}

// vhco:test sessions.send_input -- one enqueue per accepted retry; changed payload and gaps conflict; finish drains input and ends `incoming`
#[tokio::test]
async fn duplex_sequence_rules_through_the_library() {
    let rt = runtime(None);
    let me = Principal::local();
    let r = rt.open_session(open("demo.relay", &me)).await.unwrap();
    assert_eq!(r.next_send_seq, 1);
    assert_eq!(r.input_schema, Some(json!({"type":"string"})));
    let sid = r.session_id.clone();
    let ack = rt.send_input(send(&sid, 1, "hi", &me)).await.unwrap();
    assert_eq!((ack.accepted_seq, ack.input_closed), (Some(1), false));
    // identical retry of the most recent seq: same ack, no second enqueue
    assert_eq!(rt.send_input(send(&sid, 1, "hi", &me)).await.unwrap(), ack);
    let e = rt
        .send_input(send(&sid, 1, "changed", &me))
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Conflict, "conflict.input_sequence")
    );
    assert_eq!(
        rt.send_input(send(&sid, 3, "gap", &me))
            .await
            .unwrap_err()
            .code,
        "conflict.input_sequence"
    );
    assert_eq!(
        rt.send_input(send(&sid, 0, "zero", &me))
            .await
            .unwrap_err()
            .code,
        "conflict.input_sequence"
    );
    // wrong input schema (receives text)
    let mut bad = send(&sid, 2, "x", &me);
    bad.data = Value::Int(1);
    assert_eq!(rt.send_input(bad).await.unwrap_err().http_status(), 422);
    rt.send_input(send(&sid, 2, "there", &me)).await.unwrap();
    let fin = rt.finish_input(sref(&sid, &me)).await.unwrap();
    assert!(fin.input_closed && fin.accepted_seq.is_none());
    assert_eq!(
        rt.finish_input(sref(&sid, &me)).await.unwrap(),
        fin,
        "idempotent"
    );
    assert_eq!(
        rt.send_input(send(&sid, 3, "late", &me))
            .await
            .unwrap_err()
            .code,
        "conflict.input_closed"
    );
    let mut after = 0;
    let mut items = Vec::new();
    loop {
        let b = rt.read_events(read(&sid, after, &me)).await.unwrap();
        for ev in &b.events {
            items.push(ev.to_json());
        }
        after = b.last_seq;
        if b.terminal {
            break;
        }
    }
    let data: Vec<_> = items
        .iter()
        .filter(|e| e["type"] == "data")
        .map(|e| e["data"].clone())
        .collect();
    assert_eq!(data, vec![json!("hi"), json!("there")]);
    let terminal: Vec<_> = items.iter().filter(|e| e["type"] != "data").collect();
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["result"], 2, "exactly two items were relayed");
}

// vhco:test sessions.read_events -- cursors: future cursor conflicts, evicted cursor is stream.cursor_expired, re-reading a cursor replays; producers wait for acknowledgement
#[tokio::test]
async fn cursors_and_backpressure() {
    let rt = runtime(None);
    let me = Principal::local();
    let mut o = open("demo.count", &me);
    o.params = Value::object([("n", Value::Int(40))]);
    let sid = rt.open_session(o).await.unwrap().session_id;
    let e = rt.read_events(read(&sid, 5, &me)).await.unwrap_err();
    assert_eq!(e.code, "conflict.cursor");
    let b1 = rt.read_events(read(&sid, 0, &me)).await.unwrap();
    assert_eq!(b1.events.len(), 16, "16-frame retention caps the producer");
    let again = rt.read_events(read(&sid, 0, &me)).await.unwrap();
    assert_eq!(
        again.events, b1.events,
        "same cursor replays retained events"
    );
    let b2 = rt.read_events(read(&sid, 16, &me)).await.unwrap();
    assert_eq!(b2.events.first().unwrap().seq, 17);
    let e = rt.read_events(read(&sid, 3, &me)).await.unwrap_err();
    assert_eq!(e.code, "stream.cursor_expired");
    let mut after = b2.last_seq;
    let mut seqs: Vec<u64> = b1.events.iter().chain(&b2.events).map(|e| e.seq).collect();
    let mut terminal = b2.terminal;
    while !terminal {
        let b = rt.read_events(read(&sid, after, &me)).await.unwrap();
        seqs.extend(b.events.iter().map(|e| e.seq));
        after = b.last_seq;
        terminal = b.terminal;
    }
    assert_eq!(
        seqs,
        (1..=41).collect::<Vec<_>>(),
        "ordered, gap-free, one terminal"
    );
}

// vhco:test sessions.cancel_session -- cancel is idempotent, leaves a readable cancelled terminal event, and foreign principals see not_found
#[tokio::test]
async fn cancel_ownership_and_limits() {
    let rt = runtime(None);
    let ada = Principal {
        name: "ada".into(),
        authenticated_by: "bearer".into(),
    };
    let eve = Principal {
        name: "eve".into(),
        authenticated_by: "bearer".into(),
    };
    let sid = rt
        .open_session(open("demo.relay", &ada))
        .await
        .unwrap()
        .session_id;
    for e in [
        rt.read_events(read(&sid, 0, &eve)).await.unwrap_err(),
        rt.send_input(send(&sid, 1, "x", &eve)).await.unwrap_err(),
        rt.cancel_session(sref(&sid, &eve)).await.unwrap_err(),
    ] {
        assert_eq!(e.code, "not_found.session");
    }
    let c = rt.cancel_session(sref(&sid, &ada)).await.unwrap();
    assert_eq!(
        (c.state.as_str(), c.session_id.as_deref()),
        ("cancelled", Some(sid.as_str()))
    );
    let c2 = rt.cancel_session(sref(&sid, &ada)).await.unwrap();
    assert_eq!(c2.state, "cancelled", "idempotent");
    let b = rt.read_events(read(&sid, 0, &ada)).await.unwrap();
    assert!(b.terminal);
    assert_eq!(b.events[0].to_json()["error"]["kind"], "cancelled");
    // 8 live sessions per principal
    for _ in 0..8 {
        rt.open_session(open("demo.relay", &ada)).await.unwrap();
    }
    let e = rt.open_session(open("demo.relay", &ada)).await.unwrap_err();
    assert_eq!((e.code.as_str(), e.http_status()), ("limit.sessions", 429));
    rt.open_session(open("demo.relay", &eve)).await.unwrap();
    // private and unknown IDs cannot be opened
    assert_eq!(
        rt.open_session(open("demo.secret", &eve))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
    // a unary request of a `receives` operation needs a session
    let e = rt
        .request("demo.relay", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "stream.input_required");
}

// vhco:test sessions.open_session -- the rivet.sessions.* registry operations drive the same sessions through the dispatcher (CLI/HTTP/MCP path)
#[tokio::test]
async fn session_operations_through_the_dispatcher() {
    let rt = runtime(None);
    let call = |id: &'static str, p: serde_json::Value| {
        let rt = rt.clone();
        async move { rt.request(id, Value::from_json(&p), None).await }
    };
    let r = call(
        "rivet.sessions.open",
        json!({"id":"demo.relay","params":{}}),
    )
    .await
    .unwrap();
    let sid = r
        .result
        .get("session_id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let a = call(
        "rivet.sessions.send",
        json!({"session_id":sid,"send_seq":1,"data":"hello"}),
    )
    .await
    .unwrap();
    assert_eq!(
        a.result.to_json(),
        json!({"session_id":sid,"accepted_seq":1,"input_closed":false})
    );
    let f = call("rivet.sessions.finish_input", json!({"session_id":sid}))
        .await
        .unwrap();
    assert_eq!(f.result.to_json()["input_closed"], true);
    let mut after = 0;
    let mut last = json!(null);
    loop {
        let b = call(
            "rivet.sessions.read",
            json!({"session_id":sid,"after_seq":after,"wait_ms":2000}),
        )
        .await
        .unwrap()
        .result
        .to_json();
        if let Some(ev) = b["events"].as_array().unwrap().last() {
            last = ev.clone();
        }
        after = b["last_seq"].as_u64().unwrap();
        if b["terminal"] == true {
            break;
        }
    }
    assert_eq!(
        (last["type"].clone(), last["result"].clone()),
        (json!("result"), json!(1))
    );
    let c = call("rivet.sessions.cancel", json!({"session_id":sid}))
        .await
        .unwrap();
    assert_eq!(
        c.result.to_json()["state"],
        "succeeded",
        "cancel after terminal keeps its state"
    );
    let r = call("rivet.request", json!({"id":"demo.count","params":{"n":1}}))
        .await
        .unwrap();
    assert!(
        r.result.get("session_id").is_some(),
        "rivet.request opens a session for streaming ops"
    );
    let r = call(
        "rivet.request",
        json!({"id":"demo.add","params":{"a":2,"b":3}}),
    )
    .await
    .unwrap();
    assert_eq!(r.result, Value::Int(5));
}

// vhco:test serve.project_polling -- polling input/finish_input/cancel routes drive a duplex session; unknown sessions are 404, conflicts 409
#[tokio::test]
async fn polling_routes_drive_a_duplex_session() {
    let s = serve(None).await;
    let a = s.addr;
    let r = post(
        a,
        "/v1/requests",
        json!({"id":"demo.relay","params":{}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 202);
    let sid = r.json()["session_id"].as_str().unwrap().to_string();
    let base = format!("/v1/requests/{sid}");
    let r = post(
        a,
        &format!("{base}/input"),
        json!({"send_seq":1,"data":"hi"}),
        &[],
    )
    .await;
    assert_eq!(
        (r.status, r.json()["accepted_seq"].clone()),
        (200, json!(1))
    );
    let r = post(
        a,
        &format!("{base}/input"),
        json!({"send_seq":1,"data":"other"}),
        &[],
    )
    .await;
    assert_eq!(r.status, 409);
    let r = post(a, &format!("{base}/finish_input"), json!({}), &[]).await;
    assert_eq!(r.json()["input_closed"], true);
    let mut after = 0;
    let mut events = Vec::new();
    loop {
        let b = http(
            a,
            "GET",
            &format!("{base}/events?after_seq={after}&wait_ms=2000"),
            &[],
            "",
        )
        .await
        .json();
        events.extend(b["events"].as_array().unwrap().clone());
        after = b["last_seq"].as_u64().unwrap();
        if b["terminal"] == true {
            break;
        }
    }
    assert_eq!(events[0]["data"], "hi");
    assert_eq!(events.last().unwrap()["result"], 1);
    let r = post(a, &format!("{base}/cancel"), json!({}), &[]).await;
    assert_eq!(
        (r.status, r.json()["state"].clone()),
        (200, json!("succeeded"))
    );
    let r = post(a, "/v1/requests/ses_nope/cancel", json!({}), &[]).await;
    assert_eq!(r.status, 404);
    let r = http(
        a,
        "GET",
        "/v1/requests/ses_nope/events?after_seq=0&wait_ms=10",
        &[],
        "",
    )
    .await;
    assert_eq!(r.status, 404);
    s.handle.shutdown().await;
}

// vhco:test sessions.open_session -- G16: sessions.open accepts deadline_ms (capped by the host at 600000); a session whose run outlives it ends with a terminal timeout error
#[tokio::test]
async fn open_honours_a_requested_deadline() {
    let rt = runtime(None);
    let me = Principal::local();
    let mut input = open("demo.relay", &me);
    input.deadline_ms = Some(300);
    let sid = rt.open_session(input).await.unwrap().session_id;
    let started = std::time::Instant::now();
    let mut terminal = None;
    let mut after = 0;
    while terminal.is_none() && started.elapsed() < std::time::Duration::from_secs(5) {
        let b = rt.read_events(read(&sid, after, &me)).await.unwrap();
        after = b.last_seq;
        terminal = b
            .events
            .iter()
            .find(|e| e.is_terminal())
            .map(|e| e.to_json());
    }
    let t = terminal.expect("the deadline ends the session");
    assert_eq!(t["error"]["code"], "timeout.request", "{t}");
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    // A huge request is capped (the session opens; it is not rejected).
    let mut big = open("demo.relay", &me);
    big.deadline_ms = Some(u64::MAX);
    let r = rt.open_session(big).await.unwrap();
    rt.cancel_session(sref(&r.session_id, &me)).await.unwrap();
}

// vhco:test sessions.cancel_session -- G16: the background sweeper cancels a session whose idle lease expired without any session call (its run ends and releases its concurrency slot; the terminal event is cancelled.idle)
#[tokio::test]
async fn idle_sessions_expire_without_a_session_call() {
    let rt = rivet::Runtime::builder()
        .source("app.rivet", &catalog(), ".")
        .policy(
            rivet::orchestrator::runtime::policy_from_json(
                br#"{"version":1,"limits":{"max_concurrent_requests":1}}"#,
                ".",
            )
            .unwrap(),
        )
        .session_limits(rivet::domain::sessions::SessionLimits {
            idle_ms: 200,
            retention_ms: 5_000,
            ..Default::default()
        })
        .build()
        .unwrap();
    let me = Principal::local();
    let r = rt.open_session(open("demo.relay", &me)).await.unwrap();
    let probe = || rt.request("demo.add", Value::object([("a", Value::Int(1))]), None);
    // The relay holds the only request slot while it runs.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(probe().await.unwrap_err().code, "limit.concurrency");
    // No session call from here on: only the background sweeper can end the run.
    let mut freed = false;
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if probe().await.is_ok() {
            freed = true;
            break;
        }
    }
    assert!(freed, "the idle lease must expire without any session call");
    let b = rt.read_events(read(&r.session_id, 0, &me)).await.unwrap();
    assert_eq!(b.events[0].to_json()["error"]["code"], "cancelled.idle");
}

// vhco:test sessions.cancel_session -- G28: cancelling a session that already finished reports its terminal state (succeeded / failed), even after the terminal event was acknowledged and evicted
#[tokio::test]
async fn cancel_after_completion_reports_the_terminal_state() {
    let rt = runtime(None);
    let me = Principal::local();
    let mut add = open("demo.add", &me);
    add.params = Value::object([("a", Value::Int(2)), ("b", Value::Int(3))]);
    let sid = rt.open_session(add).await.unwrap().session_id;
    let b = rt.read_events(read(&sid, 0, &me)).await.unwrap();
    assert!(b.terminal);
    // Acknowledge (and evict) the terminal event, then cancel.
    rt.read_events(SessionReadInput {
        wait_ms: Some(0),
        ..read(&sid, b.last_seq, &me)
    })
    .await
    .unwrap();
    let c = rt.cancel_session(sref(&sid, &me)).await.unwrap();
    assert_eq!(c.state, "succeeded");
    let mut bad = open("demo.add", &me);
    bad.params = Value::object([("a", Value::text("x"))]);
    // Invalid params are refused at open; a failing run needs a valid open.
    assert!(rt.open_session(bad).await.is_err());
}
