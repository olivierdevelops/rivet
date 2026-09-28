//! T-22 — unified serve: one listener, the same operation over REST, SSE,
//! polling, WebSocket and MCP; auth none/bearer; non-loopback + none refusal;
//! disabled surfaces; ninth WS ref; socket close cancels its refs.

mod support;

use serde_json::json;
use support::*;

// vhco:test serve.start_serve -- one listener answers REST, SSE, polling, WebSocket and MCP for the same catalog with identical results
#[tokio::test]
async fn same_operation_on_every_surface() {
    let s = serve(None).await;
    assert_eq!(
        s.handle.receipt.surfaces,
        vec!["http", "sse", "poll", "ws", "mcp"]
    );
    let a = s.addr;
    // REST
    let add = json!({"id":"demo.add","params":{"a":2,"b":3}});
    let r = post(a, "/v1/request", add.clone(), &[]).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.json()["result"], 5);
    assert_eq!(r.json()["effects"], "none");
    let r = post(
        a,
        "/v1/request",
        json!({"id":"demo.add","params":{"b":3}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"]["code"], "validation.required");
    let r = post(
        a,
        "/v1/request",
        json!({"id":"demo.secret","params":{}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 404, "private helpers are not reachable");
    let r = post(
        a,
        "/v1/request",
        json!({"id":"demo.count","params":{"n":2}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"]["code"], "stream.required");
    let r = http(a, "POST", "/v1/request", &[], "{nope").await;
    assert_eq!(r.status, 400);
    let r = http(a, "GET", "/v1/operations", &[], "").await;
    let ids: Vec<_> = r.json()["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"demo.add".to_string()) && !ids.contains(&"demo.secret".to_string()));
    let r = http(a, "GET", "/v1/operations/demo.add", &[], "").await;
    assert_eq!(r.json()["output"]["description"], "Sum of a and b.");
    let r = http(a, "GET", "/v1/operations/demo.add/outputs", &[], "").await;
    assert_eq!(
        r.json(),
        json!({"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]})
    );
    // built-in through the same route (S06)
    let r = post(
        a,
        "/v1/request",
        json!({"id":"rivet.describe","params":{"id":"demo.add"}}),
        &[],
    )
    .await;
    assert_eq!(r.json()["result"]["id"], "demo.add");

    // SSE (S133)
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"id":"demo.count","params":{"n":2}}).to_string(),
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.headers["content-type"], "text/event-stream");
    let ev = sse_events(&r.text);
    assert_eq!(ev.len(), 3, "{}", r.text);
    assert_eq!(
        (ev[0].0, ev[0].1.as_str(), &ev[0].2["data"]),
        (1, "data", &json!(1))
    );
    assert_eq!(
        (ev[1].0, ev[1].1.as_str(), &ev[1].2["data"]),
        (2, "data", &json!(2))
    );
    assert_eq!((ev[2].0, ev[2].1.as_str()), (3, "result"));
    assert_eq!(ev[2].2["result"], 2);
    assert_eq!(ev[2].2["seq"], 3);
    // unary over SSE: one terminal result event; validation before headers keeps 422
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &add.to_string(),
    )
    .await;
    let ev = sse_events(&r.text);
    assert_eq!((ev.len(), ev[0].2["result"].clone()), (1, json!(5)));
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"id":"demo.add","params":{}}).to_string(),
    )
    .await;
    assert_eq!(r.status, 422);

    // Polling (S134)
    let r = post(
        a,
        "/v1/requests",
        json!({"id":"demo.count","params":{"n":2}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 202, "{}", r.text);
    let url = r.json()["events_url"].as_str().unwrap().to_string();
    let r = http(
        a,
        "GET",
        &format!("{url}?after_seq=0&wait_ms=5000"),
        &[],
        "",
    )
    .await;
    let b = r.json();
    let mut events = b["events"].as_array().unwrap().clone();
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
    let kinds: Vec<_> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["data", "data", "result"]);
    assert_eq!(events[2]["result"], 2);
    // unary job: a single terminal result event
    let r = post(a, "/v1/requests", add.clone(), &[]).await;
    let url = r.json()["events_url"].as_str().unwrap().to_string();
    let b = http(
        a,
        "GET",
        &format!("{url}?after_seq=0&wait_ms=5000"),
        &[],
        "",
    )
    .await
    .json();
    assert_eq!(b["terminal"], true);
    assert_eq!(b["events"][0]["result"], 5);

    // WebSocket (docs/demos/01-catalog/requests/ws-frames.jsonl)
    let mut ws = ws_connect(a, &[]).await.unwrap();
    for line in std::fs::read_to_string("docs/demos/01-catalog/requests/ws-frames.jsonl")
        .unwrap()
        .lines()
    {
        ws_send(&mut ws, serde_json::from_str(line).unwrap()).await;
    }
    let frames = ws_until_terminal(&mut ws, &["c1", "c2", "c3"]).await;
    let of = |r: &str| {
        frames
            .iter()
            .filter(|f| f["ref"] == r)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(of("c1").len(), 1);
    assert_eq!(of("c1")[0]["completion"]["result"], 5);
    let c2: Vec<_> = of("c2");
    assert_eq!(
        c2.iter()
            .map(|f| f["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["data", "data", "data", "result"]
    );
    assert_eq!(c2[0]["data"], 3);
    assert_eq!(c2[3]["completion"]["result"], json!({"count": 3}));
    assert_eq!(of("c3")[0]["error"]["code"], "validation.required");
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"c4","id":"demo.missing","params":{}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["ref"].clone(), f["error"]["kind"].clone()),
        (json!("c4"), json!("not_found"))
    );
    ws_send(&mut ws, json!({"type":"bogus"})).await;
    assert_eq!(ws_recv(&mut ws).await["error"]["code"], "validation.frame");

    // MCP
    let sid = mcp_init(a, &[]).await;
    let res = mcp_call(a, &sid, &[], "demo.add", json!({"a":2,"b":3})).await;
    assert_eq!(res["isError"], false);
    assert_eq!(res["structuredContent"]["result"], 5);
    s.handle.shutdown().await;
}

// vhco:test serve.authenticate_principal -- bearer tokens map to principals on REST, WS upgrade and MCP; missing/wrong tokens are 401, unlisted operations 403
#[tokio::test]
async fn bearer_auth_and_principals() {
    let team = std::fs::read_to_string("docs/demos/01-catalog/policies/team.json").unwrap();
    let s = serve(Some(&team)).await;
    let a = s.addr;
    let add = json!({"id":"demo.add","params":{"a":2,"b":3}});
    let r = post(
        a,
        "/v1/request",
        add.clone(),
        &[("authorization", "Bearer dev-token-ada")],
    )
    .await;
    assert_eq!((r.status, r.json()["result"].clone()), (200, json!(5)));
    let r = post(
        a,
        "/v1/request",
        add.clone(),
        &[("authorization", "Bearer dev-token-ci")],
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(r.json()["error"]["code"], "permission.denied");
    let r = post(a, "/v1/request", add.clone(), &[]).await;
    assert_eq!(r.status, 401);
    let r = post(
        a,
        "/v1/request",
        add.clone(),
        &[("authorization", "Bearer wrong")],
    )
    .await;
    assert_eq!(r.status, 401);
    // ws is not in team.json serve.surfaces
    assert_eq!(http(a, "GET", "/v1/ws", &[], "").await.status, 404);
    // ci sees only demo.health
    let r = http(
        a,
        "GET",
        "/v1/operations",
        &[("authorization", "Bearer dev-token-ci")],
        "",
    )
    .await;
    let ids: Vec<_> = r.json()["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].clone())
        .collect();
    assert_eq!(ids, vec![json!("demo.health")]);
    // MCP needs the token on every request
    let ada = [("authorization", "Bearer dev-token-ada")];
    let sid = mcp_init(a, &ada).await;
    let res = mcp_call(a, &sid, &ada, "demo.add", json!({"a":2,"b":3})).await;
    assert_eq!(res["structuredContent"]["result"], 5);
    let r = mcp_raw(
        a,
        &sid,
        &[],
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(r.status, 401);
    // another principal cannot use ada's MCP session
    let r = mcp_raw(
        a,
        &sid,
        &[("authorization", "Bearer dev-token-ci")],
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await;
    assert_eq!(r.status, 404);
    // polling sessions are principal-owned and survive reconnects (every call here is a new connection)
    let r = post(
        a,
        "/v1/requests",
        json!({"id":"demo.countdown","params":{}}),
        &ada,
    )
    .await;
    let url = r.json()["events_url"].as_str().unwrap().to_string();
    let q = format!("{url}?after_seq=0&wait_ms=2000");
    let r = http(
        a,
        "GET",
        &q,
        &[("authorization", "Bearer dev-token-ci")],
        "",
    )
    .await;
    assert_eq!(r.status, 404, "cross-principal access is not_found");
    let r = http(a, "GET", &q, &ada, "").await;
    assert_eq!(r.status, 200);
    s.handle.shutdown().await;

    // WS upgrade carries the bearer token when ws is enabled
    let hash = sha256_hex("ws-token");
    let policy = json!({"version":1,"serve":{"auth":{"type":"bearer","tokens":[{"principal":"ada","sha256":hash}]}}}).to_string();
    let s = serve(Some(&policy)).await;
    assert!(ws_connect(s.addr, &[]).await.unwrap_err().contains("401"));
    let mut ws = ws_connect(s.addr, &[("authorization", "Bearer ws-token")])
        .await
        .unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"a","id":"demo.add","params":{"a":1,"b":1}}),
    )
    .await;
    assert_eq!(ws_recv(&mut ws).await["completion"]["result"], 2);
    s.handle.shutdown().await;
}

// vhco:test serve.start_serve -- a non-loopback --listen with auth none refuses to start with serve.auth_required and exit 2, binding nothing
#[tokio::test]
async fn non_loopback_without_auth_refuses() {
    let rt = runtime(None);
    let e = rivet::orchestrator::setup_serve::start(
        rt,
        rivet::orchestrator::setup_serve::ServeOptions {
            listen: Some("0.0.0.0:0".into()),
            ..Default::default()
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(e.code, "serve.auth_required");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args([
            "--file",
            "docs/demos/01-catalog/app.rivet",
            "serve",
            "--listen",
            "0.0.0.0:0",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("serve.auth_required"));
    assert!(out.stdout.is_empty());
}

// vhco:test serve.start_serve -- serve.surfaces narrows the mounts; disabled poll, sse and ws answer 404 while http and mcp work
#[tokio::test]
async fn disabled_surfaces_answer_404() {
    let s = serve(Some(r#"{"version":1,"serve":{"surfaces":["http","mcp"]}}"#)).await;
    let a = s.addr;
    let add = json!({"id":"demo.add","params":{"a":2,"b":3}});
    assert_eq!(post(a, "/v1/requests", add.clone(), &[]).await.status, 404);
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"id":"demo.count","params":{}}).to_string(),
    )
    .await;
    assert_eq!(r.status, 404);
    assert_eq!(http(a, "GET", "/v1/ws", &[], "").await.status, 404);
    assert_eq!(post(a, "/v1/request", add, &[]).await.status, 200);
    mcp_init(a, &[]).await;
    s.handle.shutdown().await;
}

// vhco:test serve.multiplex_ws -- a ninth concurrent ref gets a limit error frame; cancel ends a ref with one cancelled error frame
#[tokio::test]
async fn ninth_ws_ref_is_rejected() {
    let s = serve(None).await;
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    for i in 0..8 {
        ws_send(
            &mut ws,
            json!({"type":"request","ref":format!("r{i}"),"id":"demo.relay","params":{}}),
        )
        .await;
    }
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r8","id":"demo.relay","params":{}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["ref"], "r8");
    assert_eq!(f["error"]["kind"], "limit");
    // duplicate in-flight ref
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r0","id":"demo.add","params":{"a":1}}),
    )
    .await;
    assert_eq!(ws_recv(&mut ws).await["error"]["code"], "conflict.ref");
    // relay: input → data, finish_input → result
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"r1","seq":1,"data":"hi"}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["type"].clone(), f["data"].clone()),
        (json!("data"), json!("hi"))
    );
    ws_send(&mut ws, json!({"type":"finish_input","ref":"r1"})).await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["ref"].clone(), f["completion"]["result"].clone()),
        (json!("r1"), json!(1))
    );
    // cancel ends r0 with exactly one cancelled error frame
    ws_send(&mut ws, json!({"type":"cancel","ref":"r0"})).await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["ref"].clone(), f["error"]["kind"].clone()),
        (json!("r0"), json!("cancelled"))
    );
    // a slot is free again
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r9","id":"demo.add","params":{"a":1,"b":2}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["ref"].clone(), f["completion"]["result"].clone()),
        (json!("r9"), json!(3))
    );
    s.handle.shutdown().await;
}

// vhco:test serve.multiplex_ws -- closing the socket cancels and joins its in-flight refs (the held concurrency slot is released)
#[tokio::test]
async fn socket_close_cancels_refs() {
    let s = serve(Some(
        r#"{"version":1,"limits":{"max_concurrent_requests":1}}"#,
    ))
    .await;
    let a = s.addr;
    let mut ws = ws_connect(a, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"live","id":"demo.relay","params":{}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"live","seq":1,"data":"x"}),
    )
    .await;
    assert_eq!(ws_recv(&mut ws).await["data"], "x");
    // the relay holds the only request slot
    let r = post(
        a,
        "/v1/request",
        json!({"id":"demo.add","params":{"a":1}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 429, "{}", r.text);
    drop(ws);
    let mut ok = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let r = post(
            a,
            "/v1/request",
            json!({"id":"demo.add","params":{"a":1}}),
            &[],
        )
        .await;
        if r.status == 200 {
            ok = true;
            break;
        }
    }
    assert!(ok, "closing the socket must cancel its refs");
    s.handle.shutdown().await;
}

// vhco:test audit.inspect_effects -- GET /v1/io needs an explicit rivet.io listing for a network principal; wildcards do not match rivet.*; check_files is refused remotely
#[tokio::test]
async fn io_manifest_exposure_requires_explicit_listing() {
    let wild = sha256_hex("tok-wild");
    let listed = sha256_hex("tok-listed");
    let policy = json!({"version":1,"serve":{
        "auth":{"type":"bearer","tokens":[{"principal":"wild","sha256":wild},{"principal":"auditor","sha256":listed}]},
        "principals":{"wild":{"operations":["*"]},"auditor":{"operations":["rivet.io","rivet.policy.generate","demo.*"]}}}})
    .to_string();
    let s = serve(Some(&policy)).await;
    let r = http(
        s.addr,
        "GET",
        "/v1/io?by=target",
        &[("authorization", "Bearer tok-wild")],
        "",
    )
    .await;
    assert_eq!(r.status, 403, "{}", r.text);
    let r = http(
        s.addr,
        "GET",
        "/v1/io?by=target",
        &[("authorization", "Bearer tok-listed")],
        "",
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert!(r.json()["sites"].is_array());
    let r = http(
        s.addr,
        "GET",
        "/v1/io?check_files=true",
        &[("authorization", "Bearer tok-listed")],
        "",
    )
    .await;
    assert_eq!(r.status, 422);
    let r = post(
        s.addr,
        "/v1/policy/generate",
        json!({"all": true}),
        &[("authorization", "Bearer tok-listed")],
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.json()["policy"]["version"], 1);
    let r = post(
        s.addr,
        "/v1/policy/generate",
        json!({"all": true}),
        &[("authorization", "Bearer tok-wild")],
    )
    .await;
    assert_eq!(r.status, 403);
}
