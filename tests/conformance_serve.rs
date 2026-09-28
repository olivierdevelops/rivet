//! T-22 — unified serve: one listener, the same operation over REST, SSE,
//! polling, WebSocket and MCP; auth none/bearer; non-loopback + none refusal;
//! disabled surfaces; ninth WS ref; socket close cancels its refs.

mod support;

use serde_json::{Value as Json, json};
use std::net::SocketAddr;
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
    let add = json!({"operation":"demo.add","data":{"a":2,"b":3}});
    let r = post(a, "/v1/request", add.clone(), &[]).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.json()["data"], 5);
    assert_eq!(r.json()["effects"], "none");
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"demo.add","data":{"b":3}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"]["code"], "validation.required");
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"demo.secret","data":{}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 404, "private helpers are not reachable");
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"demo.count","data":{"n":2}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"]["code"], "stream.required");
    let r = http(a, "POST", "/v1/request", &[], "{nope").await;
    assert_eq!(r.status, 400);
    let r = http(a, "GET", "/v1/operations", &[], "").await;
    let ids: Vec<_> = r.json()["data"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"demo.add".to_string()) && !ids.contains(&"demo.secret".to_string()));
    let r = http(a, "GET", "/v1/operations/demo.add", &[], "").await;
    assert_eq!(r.json()["data"]["output"]["description"], "Sum of a and b.");
    let r = http(a, "GET", "/v1/operations/demo.add/outputs", &[], "").await;
    assert_eq!(
        r.json()["data"],
        json!({"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]})
    );
    // built-in through the same route (S06)
    let r = post(
        a,
        "/v1/request",
        json!({"operation":"rivet.describe","data":{"id":"demo.add"}}),
        &[],
    )
    .await;
    assert_eq!(r.json()["data"]["id"], "demo.add");

    // SSE (S133)
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"operation":"demo.count","data":{"n":2}}).to_string(),
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
    assert_eq!(ev[2].2["data"], 2);
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
    assert_eq!((ev.len(), ev[0].2["data"].clone()), (1, json!(5)));
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"operation":"demo.add","data":{}}).to_string(),
    )
    .await;
    assert_eq!(r.status, 422);

    // Polling (S134)
    let r = post(
        a,
        "/v1/requests",
        json!({"operation":"demo.count","data":{"n":2}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 202, "{}", r.text);
    let url = r.json()["data"]["events_url"].as_str().unwrap().to_string();
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
    assert_eq!(events[2]["data"], 2);
    // unary job: a single terminal result event
    let r = post(a, "/v1/requests", add.clone(), &[]).await;
    let url = r.json()["data"]["events_url"].as_str().unwrap().to_string();
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
    assert_eq!(b["events"][0]["data"], 5);

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
    assert_eq!(of("c1")[0]["data"], 5);
    let c2: Vec<_> = of("c2");
    assert_eq!(
        c2.iter()
            .map(|f| f["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["data", "data", "data", "result"]
    );
    assert_eq!(c2[0]["data"], 3);
    assert_eq!(c2[3]["data"], json!({"count": 3}));
    assert_eq!(of("c3")[0]["error"]["code"], "validation.required");
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"c4","operation":"demo.missing","data":{}}),
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
    assert_eq!(res["structuredContent"]["data"], 5);
    s.handle.shutdown().await;
}

// vhco:test serve.authenticate_principal -- bearer tokens map to principals on REST, WS upgrade and MCP; missing/wrong tokens are 401, unlisted operations 403
#[tokio::test]
async fn bearer_auth_and_principals() {
    let team = std::fs::read_to_string("docs/demos/01-catalog/policies/team.json").unwrap();
    let s = serve(Some(&team)).await;
    let a = s.addr;
    let add = json!({"operation":"demo.add","data":{"a":2,"b":3}});
    let r = post(
        a,
        "/v1/request",
        add.clone(),
        &[("authorization", "Bearer dev-token-ada")],
    )
    .await;
    assert_eq!((r.status, r.json()["data"].clone()), (200, json!(5)));
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
    let ids: Vec<_> = r.json()["data"]["operations"]
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
    assert_eq!(res["structuredContent"]["data"], 5);
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
        json!({"operation":"demo.countdown","data":{}}),
        &ada,
    )
    .await;
    let url = r.json()["data"]["events_url"].as_str().unwrap().to_string();
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
        json!({"type":"request","ref":"a","operation":"demo.add","data":{"a":1,"b":1}}),
    )
    .await;
    assert_eq!(ws_recv(&mut ws).await["data"], 2);
    s.handle.shutdown().await;
}

// vhco:test serve.start_serve -- a non-loopback --listen with auth none refuses to start with serve.auth_required and exit 2, binding nothing
#[tokio::test]
async fn non_loopback_without_auth_refuses() {
    let rt = runtime(None);
    let e = rivet::internal::orchestrator::setup_serve::start(
        rt,
        rivet::internal::orchestrator::setup_serve::ServeOptions {
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
    let add = json!({"operation":"demo.add","data":{"a":2,"b":3}});
    assert_eq!(post(a, "/v1/requests", add.clone(), &[]).await.status, 404);
    let r = http(
        a,
        "POST",
        "/v1/request",
        &[("accept", "text/event-stream")],
        &json!({"operation":"demo.count","data":{}}).to_string(),
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
            json!({"type":"request","ref":format!("r{i}"),"operation":"demo.relay","data":{}}),
        )
        .await;
    }
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r8","operation":"demo.relay","data":{}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["ref"], "r8");
    assert_eq!(f["error"]["kind"], "limit");
    // duplicate in-flight ref
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r0","operation":"demo.add","data":{"a":1}}),
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
        (f["ref"].clone(), f["data"].clone()),
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
        json!({"type":"request","ref":"r9","operation":"demo.add","data":{"a":1,"b":2}}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["ref"].clone(), f["data"].clone()),
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
        json!({"type":"request","ref":"live","operation":"demo.relay","data":{}}),
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
        json!({"operation":"demo.add","data":{"a":1}}),
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
            json!({"operation":"demo.add","data":{"a":1}}),
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
    assert!(r.json()["data"]["sites"].is_array());
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
    assert_eq!(r.json()["data"]["policy"]["version"], 1);
    let r = post(
        s.addr,
        "/v1/policy/generate",
        json!({"all": true}),
        &[("authorization", "Bearer tok-wild")],
    )
    .await;
    assert_eq!(r.status, 403);
}

// ---------------------------------------------------------------------------
// Fix-C: MCP built-ins (G7), health/access log/SIGTERM (G12), W3C
// traceparent (G25), bare /v1/io (G26), WS refused input frames (G17/G27).
// ---------------------------------------------------------------------------

async fn serve_logged(
    policy: Option<&str>,
) -> (Server, std::sync::Arc<std::sync::Mutex<Vec<Json>>>) {
    let rt = runtime(policy);
    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Json>::new()));
    let sink = std::sync::Arc::clone(&lines);
    let handle = rivet::internal::orchestrator::setup_serve::start(
        rt.clone(),
        rivet::internal::orchestrator::setup_serve::ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            access_log: Some(std::sync::Arc::new(move |line: &str| {
                sink.lock()
                    .unwrap()
                    .push(serde_json::from_str(line).unwrap());
            })),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let addr = handle.addr.unwrap();
    (Server { rt, addr, handle }, lines)
}

fn tool_names(list: &Json) -> Vec<String> {
    list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

// vhco:test serve.authorize_operation -- G7: MCP tools/list lists every built-in the principal may call with schemas: the local principal sees rivet.io/policy.generate/trace.show/connectors.sync and rivet.auth.*; a wildcard network principal does not see the sensitive ones unless listed exactly
#[tokio::test]
async fn mcp_tools_list_includes_callable_builtins() {
    let s = serve(None).await;
    let sid = mcp_init(s.addr, &[]).await;
    let r = mcp_raw(
        s.addr,
        &sid,
        &[],
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let list = r.json()["result"].clone();
    let names = tool_names(&list);
    for want in [
        "rivet.request",
        "rivet.io",
        "rivet.policy.generate",
        "rivet.trace.show",
        "rivet.connectors.sync",
        "rivet.auth.begin",
        "rivet.auth.status",
        "demo.add",
    ] {
        assert!(names.iter().any(|n| n == want), "{want} missing: {names:?}");
    }
    for t in list["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{t}");
        assert!(t["outputSchema"].is_object(), "{t}");
    }
    s.handle.shutdown().await;

    let wild = sha256_hex("tok-wild");
    let listed = sha256_hex("tok-listed");
    let policy = json!({"version":1,"serve":{
        "auth":{"type":"bearer","tokens":[{"principal":"wild","sha256":wild},{"principal":"auditor","sha256":listed}]},
        "principals":{"wild":{"operations":["*"]},"auditor":{"operations":["rivet.io","demo.*"]}}}})
    .to_string();
    let s = serve(Some(&policy)).await;
    for (token, sees_io) in [("Bearer tok-wild", false), ("Bearer tok-listed", true)] {
        let auth = [("authorization", token)];
        let sid = mcp_init(s.addr, &auth).await;
        let r = mcp_raw(
            s.addr,
            &sid,
            &auth,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .await;
        let names = tool_names(&r.json()["result"]);
        assert_eq!(
            names.iter().any(|n| n == "rivet.io"),
            sees_io,
            "{token}: {names:?}"
        );
        assert!(!names.iter().any(|n| n == "rivet.trace.show"), "{token}");
        assert!(
            !names.iter().any(|n| n == "rivet.connectors.sync"),
            "{token}"
        );
        assert!(names.iter().any(|n| n == "rivet.request"), "{token}");
    }
    s.handle.shutdown().await;
}

// vhco:test serve.start_serve -- G12: GET /v1/health answers an envelope whose data is {status:"ok", catalog_version, version} unauthenticated on loopback, and every request produces one access-log line (time, surface, method, route, principal, operation, status, duration_ms) that never contains params or tokens
#[tokio::test]
async fn health_and_access_log() {
    let (s, lines) = serve_logged(None).await;
    let r = http(s.addr, "GET", "/v1/health", &[], "").await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.json()["data"]["status"], "ok");
    assert_eq!(r.json()["data"]["catalog_version"], s.rt.catalog_version());
    let r = post(
        s.addr,
        "/v1/request",
        json!({"operation":"demo.greet","data":{"person":"S3CRET-PARAM"}}),
        &[],
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = http(s.addr, "GET", "/v1/operations/demo.add?x=QUERYVAL", &[], "").await;
    assert_eq!(r.status, 200);
    let log = lines.lock().unwrap().clone();
    assert_eq!(log.len(), 3, "{log:?}");
    let req = &log[1];
    for k in [
        "time",
        "surface",
        "method",
        "route",
        "principal",
        "operation",
        "status",
        "duration_ms",
    ] {
        assert!(req.get(k).is_some(), "{k} missing: {req}");
    }
    assert_eq!(req["surface"], "http");
    assert_eq!(req["method"], "POST");
    assert_eq!(req["route"], "/v1/request");
    assert_eq!(req["principal"], "local");
    assert_eq!(req["operation"], "demo.greet");
    assert_eq!(req["status"], 200);
    assert_eq!(log[0]["route"], "/v1/health");
    assert_eq!(log[2]["route"], "/v1/operations/{id}");
    let text = serde_json::to_string(&log).unwrap();
    assert!(
        !text.contains("S3CRET-PARAM") && !text.contains("QUERYVAL"),
        "{text}"
    );
    s.handle.shutdown().await;

    // Non-loopback semantics are covered by serve.auth: with bearer auth on a
    // loopback bind health stays open.
    let hash = sha256_hex("tok");
    let policy = json!({"version":1,"serve":{"auth":{"type":"bearer","tokens":[{"principal":"ada","sha256":hash}]}}}).to_string();
    let s = serve(Some(&policy)).await;
    assert_eq!(http(s.addr, "GET", "/v1/health", &[], "").await.status, 200);
    s.handle.shutdown().await;
}

// vhco:test execution.request_operation -- G25: a valid W3C traceparent on POST /v1/request becomes the request's trace id and every answer carries a traceparent header; nested requests keep the trace id; invalid headers are ignored
#[tokio::test]
async fn traceparent_is_accepted_and_emitted() {
    let s = serve(None).await;
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let r = post(
        s.addr,
        "/v1/request",
        json!({"operation":"demo.add","data":{"a":2,"b":3}}),
        &[("traceparent", tp)],
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.json()["trace_id"], "4bf92f3577b34da6a3ce929d0e0e4736");
    let out = r.headers["traceparent"].to_str().unwrap().to_string();
    let parts: Vec<&str> = out.split('-').collect();
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1], "4bf92f3577b34da6a3ce929d0e0e4736");
    assert_ne!(
        parts[2], "00f067aa0ba902b7",
        "our own span, not the caller's"
    );
    // rivet.request dispatches a nested request that keeps the trace id.
    let r = post(
        s.addr,
        "/v1/request",
        json!({"operation":"rivet.request","data":{"operation":"demo.add","data":{"a":1}}}),
        &[("traceparent", tp)],
    )
    .await;
    assert_eq!(r.json()["trace_id"], "4bf92f3577b34da6a3ce929d0e0e4736");
    // An invalid header is ignored: Rivet mints its own trace and still emits one.
    let r = post(
        s.addr,
        "/v1/request",
        json!({"operation":"demo.add","data":{"a":1}}),
        &[("traceparent", "00-zz-00f067aa0ba902b7-01")],
    )
    .await;
    assert!(r.json()["trace_id"].as_str().unwrap().starts_with("tr_"));
    let emitted = r.headers["traceparent"].to_str().unwrap();
    assert_eq!(emitted.len(), 55, "{emitted}");
    // Polling and MCP accept it too.
    let r = post(
        s.addr,
        "/v1/requests",
        json!({"operation":"demo.add","data":{"a":1}}),
        &[("traceparent", tp)],
    )
    .await;
    assert_eq!(r.status, 202);
    assert_eq!(r.json()["trace_id"], "4bf92f3577b34da6a3ce929d0e0e4736");
    assert!(r.headers.get("traceparent").is_some());
    let sid = mcp_init(s.addr, &[]).await;
    let r = mcp_raw(
        s.addr,
        &sid,
        &[("traceparent", tp)],
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":1}}}),
    )
    .await;
    assert_eq!(
        r.json()["result"]["structuredContent"]["trace_id"],
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    assert!(
        r.headers["traceparent"]
            .to_str()
            .unwrap()
            .contains("4bf92f3577b34da6a3ce929d0e0e4736")
    );
    s.handle.shutdown().await;
}

// vhco:test audit.inspect_effects -- G26: GET /v1/io returns an envelope whose data is the bare IoManifest (also for format=json); format=table|markdown|csv puts the rendered report in data
#[tokio::test]
async fn io_route_returns_the_bare_manifest() {
    let s = serve(None).await;
    for q in ["/v1/io", "/v1/io?format=json"] {
        let r = http(s.addr, "GET", q, &[], "").await;
        assert_eq!(r.status, 200, "{}", r.text);
        let j = r.json()["data"].clone();
        assert!(j["sites"].is_array(), "{q}: {j}");
        assert!(j.get("rendered").is_none(), "{q}");
    }
    let r = http(s.addr, "GET", "/v1/io?format=table", &[], "").await;
    let j = r.json()["data"].clone();
    assert!(j["rendered"].is_string(), "{j}");
    assert!(j["manifest"]["sites"].is_array());
    s.handle.shutdown().await;
}

// vhco:test serve.multiplex_ws -- G17/G27: a refused input frame on /v1/ws gets an error frame for its ref with the specific code (conflict.input_sequence), and that is the ref's only terminal frame
#[tokio::test]
async fn ws_refused_input_sends_the_specific_error() {
    let s = serve(None).await;
    let mut ws = ws_connect(s.addr, &[]).await.unwrap();
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r1","operation":"demo.relay","data":{}}),
    )
    .await;
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"r1","seq":1,"data":"a"}),
    )
    .await;
    assert_eq!(ws_recv(&mut ws).await["data"], "a");
    ws_send(
        &mut ws,
        json!({"type":"input","ref":"r1","seq":5,"data":"b"}),
    )
    .await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(
        (f["type"].clone(), f["status"].clone()),
        (json!("result"), json!("error")),
        "{f}"
    );
    assert_eq!(f["ref"], "r1");
    assert_eq!(f["error"]["code"], "conflict.input_sequence", "{f}");
    // No second terminal frame for r1; a new ref still works on the socket.
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r2","operation":"demo.add","data":{"a":1}}),
    )
    .await;
    let frames = ws_until_terminal(&mut ws, &["r2"]).await;
    assert!(
        frames.iter().all(|f| f["ref"] != "r1"),
        "r1 already ended: {frames:?}"
    );
    // A schema-invalid input item is refused with validation.input and its seq.
    ws_send(
        &mut ws,
        json!({"type":"request","ref":"r3","operation":"demo.relay","data":{}}),
    )
    .await;
    ws_send(&mut ws, json!({"type":"input","ref":"r3","seq":1,"data":7})).await;
    let f = ws_recv(&mut ws).await;
    assert_eq!(f["error"]["code"], "validation.input", "{f}");
    assert_eq!(f["error"]["details"]["seq"], 1, "{f}");
    s.handle.shutdown().await;
}

// vhco:test serve.start_serve -- G12: SIGTERM drains like SIGINT: the in-flight request is cancelled (not dropped), its response is sent, and `rivet serve` exits 0 with one access-log line per request on stderr
#[cfg(unix)]
#[tokio::test]
async fn sigterm_drains_and_exits_zero() {
    use tokio::io::AsyncBufReadExt;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("app.rivet"),
        "operation slow.op\n    param note text default \"x\"\n    output json\n    status = poll every \"50ms\" timeout \"60s\"\n        until false\n        yield 1\n    end\n    return status\nend\n",
    )
    .unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(["--file", "app.rivet", "serve", "--listen", "127.0.0.1:0"])
        .current_dir(tmp.path())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut err = tokio::io::BufReader::new(child.stderr.take().unwrap()).lines();
    let receipt: Json = serde_json::from_str(&err.next_line().await.unwrap().unwrap()).unwrap();
    let addr: SocketAddr = receipt["listen_addr"].as_str().unwrap().parse().unwrap();
    let slow = tokio::spawn(async move {
        post(
            addr,
            "/v1/request",
            json!({"operation":"slow.op","data":{"note":"PARAM-VALUE"}}),
            &[],
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    // SAFETY: signalling our own child process.
    unsafe {
        libc::kill(child.id().unwrap() as i32, libc::SIGTERM);
    }
    let status = tokio::time::timeout(std::time::Duration::from_secs(12), child.wait())
        .await
        .expect("serve exits after draining")
        .unwrap();
    assert_eq!(status.code(), Some(0));
    let r = slow.await.unwrap();
    assert_eq!(r.status, 409, "{}", r.text);
    assert_eq!(r.json()["error"]["kind"], "cancelled");
    let mut rest = Vec::new();
    while let Ok(Some(l)) = err.next_line().await {
        rest.push(l);
    }
    let access: Vec<Json> = rest
        .iter()
        .filter_map(|l| serde_json::from_str::<Json>(l).ok())
        .filter(|j| j.get("route").is_some())
        .collect();
    assert_eq!(access.len(), 1, "{rest:?}");
    assert_eq!(access[0]["operation"], "slow.op");
    assert_eq!(access[0]["status"], 409);
    assert!(!rest.join("\n").contains("PARAM-VALUE"));
}
