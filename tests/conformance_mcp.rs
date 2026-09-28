//! T-06 MCP client connector conformance (PROP-2026-0001 Increment 6; REF S52–S61;
//! docs/demos/06-mcp-bridge).
//!
//! ```text
//!  client bundle ──(request "peer.tools.X")──▶ connectors.invoke_mcp
//!        │                                         │
//!        │   stdio:  rivet serve --stdio  (the real binary, OS-sandboxed: policy.json present)
//!        │           /bin/sh fake.sh      (scripted fake: isError, JSON-RPC error, sampling)
//!        │   http:   rivet serve /mcp     (in-process, 127.0.0.1)
//!        │           axum fake            (the demo's protocol fixtures, SSE responses, sampling)
//!        ▼
//!  snapshots: `connectors sync` writes a new file → its sha256 must be approved before load
//! ```
//!
//! Stdio fixtures run inside the OS sandbox (Seatbelt on macOS, Landlock on
//! Linux). Where the platform has no sandbox backend the stdio tests assert the
//! typed `unsupported.sandbox_backend` refusal instead.
#![allow(clippy::result_large_err)]

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use rivet::Runtime;
use rivet::domain::Value;
use rivet::domain::mcp::snapshot_hash;
use rivet::orchestrator::setup_serve::{ServeHandle, ServeOptions, start};
use serde_json::{Value as Json, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

mod oauth_support;

const RIVET: &str = env!("CARGO_BIN_EXE_rivet");

fn canon(p: &Path) -> String {
    p.canonicalize().unwrap().to_string_lossy().into_owned()
}

fn write(dir: &Path, rel: &str, text: &str) -> PathBuf {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    p
}

fn policy(grants: Json, approved: &[String]) -> String {
    let mut p = json!({"version": 1, "grants": grants});
    if !approved.is_empty() {
        p["approved"] = json!({"snapshots": approved});
    }
    serde_json::to_string_pretty(&p).unwrap()
}

fn load(dir: &Path) -> Result<Runtime, rivet::domain::RivetError> {
    Runtime::builder()
        .file(dir.join("app.rivet").to_str().unwrap())
        .build()
}

fn load_discovery(dir: &Path) -> Runtime {
    Runtime::builder()
        .file(dir.join("app.rivet").to_str().unwrap())
        .connector_discovery()
        .build()
        .unwrap()
}

fn sandbox_missing(e: &rivet::domain::RivetError) -> bool {
    e.code == "unsupported.sandbox_backend"
}

/// Serialized error (with nested details) for substring checks.
fn error_text(e: &rivet::domain::RivetError) -> String {
    e.to_value().to_json().to_string()
}

async fn serve_on(rt: Runtime, listen: &str) -> ServeHandle {
    start(
        rt,
        ServeOptions {
            listen: Some(listen.into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

const PEER: &str = r#"operation demo.add
    description "Add two integers."
    param a integer required
    param b integer default 0
    output integer
    return a + b
end

operation demo.bad
    description "Declares an integer but returns text: a tool execution error."
    output integer
    return "not a number"
end
"#;

// ------------------------------------------------------------------ stdio: rivet serve --stdio

fn stdio_client(dir: &Path, peer_dir: &Path, expose: &str, body: &str) {
    write(
        dir,
        "app.rivet",
        &format!(
            "connector peer mcp\n    transport command \"{RIVET}\"\n        args [\"--file\", \"{}/app.rivet\", \"serve\", \"--stdio\"]\n    end\n    schema \"./schemas/peer.json\"\n    expose tools {expose}\nend\n\n{body}",
            canon(peer_dir)
        ),
    );
}

fn stdio_grants(peer_dir: &Path) -> Json {
    json!([
        {"capability": "allow_exec", "targets": [RIVET]},
        {"capability": "allow_read", "targets": [format!("{}/**", canon(peer_dir))]},
        {"capability": "allow_mcp", "targets": ["peer/discover", "peer/tools/demo.add", "peer/tools/demo.bad", "peer/tools/ghost"]},
        {"capability": "allow_write", "targets": ["./schemas/**"]}
    ])
}

// vhco:test connectors.invoke_mcp -- stdio peer (rivet serve --stdio, sandboxed): sync writes a new snapshot, load fails until its sha256 is approved, then tool success, isError → mcp.tool_failed, and a snapshot tool the server lacks → protocol.mcp_error keeping JSON-RPC -32602
#[tokio::test(flavor = "multi_thread")]
async fn stdio_rivet_peer_sync_approve_call() {
    let peer = tempfile::tempdir().unwrap();
    write(peer.path(), "app.rivet", PEER);
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("schemas")).unwrap(); // `connectors sync --output` never creates directories
    stdio_client(
        dir.path(),
        peer.path(),
        "[\"demo.add\", \"demo.bad\"]",
        "operation bridge.add\n    param a integer required\n    output json\n    r = (request \"peer.tools.demo.add\" {a: a, b: 3})\n    return r.structuredContent.result\nend\n",
    );
    write(
        dir.path(),
        "policy.json",
        &policy(stdio_grants(peer.path()), &[]),
    );

    // No snapshot yet: a normal load fails (not_found, exit 4); discovery loads.
    let e = load(dir.path()).err().unwrap();
    assert_eq!(e.code, "not_found.mcp_snapshot");
    assert_eq!(e.exit_code(), 4);
    let rt = load_discovery(dir.path());
    let receipt = match rt.sync_connector("peer", "./schemas/peer.json").await {
        Ok(r) => r,
        Err(e) if sandbox_missing(&e) => return,
        Err(e) => panic!("{e:?}"),
    };
    assert!(receipt.tools.contains(&"demo.add".to_string()));
    let bytes = std::fs::read(dir.path().join("schemas/peer.json")).unwrap();
    assert_eq!(snapshot_hash(&bytes), receipt.sha256);
    // Never overwrites: a second sync to the same path is a conflict.
    let again = rt
        .sync_connector("peer", "./schemas/peer.json")
        .await
        .unwrap_err();
    assert_eq!(again.code, "conflict.already_exists");

    // Unapproved hash → load failure naming the hash.
    let e = load(dir.path()).err().unwrap();
    assert_eq!(e.code, "mcp.snapshot_unapproved");
    assert!(e.message.contains(&receipt.sha256));
    write(
        dir.path(),
        "policy.json",
        &policy(
            stdio_grants(peer.path()),
            std::slice::from_ref(&receipt.sha256),
        ),
    );
    let rt = load(dir.path()).unwrap();
    assert!(
        rt.connector_imports()
            .contains(&"peer.tools.demo.add".to_string())
    );

    let c = rt
        .request("bridge.add", Value::object([("a", Value::Int(2))]), None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::Int(5));

    // Direct import call from a surface: content blocks + structuredContent preserved.
    let c = rt
        .request(
            "peer.tools.demo.add",
            Value::object([("a", Value::Int(1)), ("b", Value::Int(1))]),
            None,
        )
        .await
        .unwrap();
    let j = c.result.to_json();
    assert_eq!(j["isError"], json!(false));
    assert_eq!(j["content"][0]["type"], json!("text"));
    assert_eq!(j["structuredContent"]["result"], json!(2));

    // Params are checked against the snapshot inputSchema before any I/O.
    let e = rt
        .request(
            "peer.tools.demo.add",
            Value::object([("a", Value::text("x"))]),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "validation.mcp_params");

    // A remote tool execution error → mcp.tool_failed (kind application, exit 5).
    let e = rt
        .request("peer.tools.demo.bad", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "mcp.tool_failed");
    assert_eq!(e.kind, rivet::domain::ErrorKind::Application);
    assert_eq!(e.exit_code(), 5);
    assert!(
        error_text(&e).contains("output.invalid"),
        "{}",
        error_text(&e)
    );
}

// vhco:test connectors.invoke_mcp -- schema drift: an edited snapshot no longer matches its approved hash, and an exposed tool missing from the snapshot, both fail the load; a snapshot tool the server no longer has keeps its JSON-RPC error identity
#[tokio::test(flavor = "multi_thread")]
async fn schema_drift_fails_load_or_keeps_rpc_identity() {
    let peer = tempfile::tempdir().unwrap();
    write(peer.path(), "app.rivet", PEER);
    let dir = tempfile::tempdir().unwrap();
    let snap = json!({"protocolVersion": "2025-11-25", "tools": [
        {"name": "demo.add", "inputSchema": {"type": "object"}},
        {"name": "ghost", "inputSchema": {"type": "object"}}
    ]});
    let text = serde_json::to_string_pretty(&snap).unwrap();
    write(dir.path(), "schemas/peer.json", &text);
    let hash = snapshot_hash(text.as_bytes());
    stdio_client(dir.path(), peer.path(), "[\"demo.add\", \"ghost\"]", "");
    write(
        dir.path(),
        "policy.json",
        &policy(stdio_grants(peer.path()), std::slice::from_ref(&hash)),
    );
    let rt = load(dir.path()).unwrap();
    match rt.request("peer.tools.ghost", Value::Null, None).await {
        Err(e) if sandbox_missing(&e) => {}
        Err(e) => {
            assert_eq!(e.code, "protocol.mcp_error", "{e:?}");
            assert_eq!(e.details.get("code"), Some(&Value::Int(-32602)));
        }
        Ok(c) => panic!("ghost tool succeeded: {c:?}"),
    }

    // Drift: the reviewed file changed after approval → hash no longer approved.
    write(
        dir.path(),
        "schemas/peer.json",
        &text.replace("ghost", "ghost2"),
    );
    let e = load(dir.path()).err().unwrap();
    assert_eq!(e.code, "mcp.snapshot_unapproved");

    // Exposure names a tool the reviewed snapshot does not contain.
    write(dir.path(), "schemas/peer.json", &text);
    stdio_client(dir.path(), peer.path(), "[\"demo.add\", \"missing\"]", "");
    let e = load(dir.path()).err().unwrap();
    assert_eq!(e.code, "mcp.unknown_tool");

    // A literal call to a non-imported ID fails the load; an alias collision too.
    stdio_client(
        dir.path(),
        peer.path(),
        "[\"demo.add\"]",
        "operation x.y\n    output json\n    return (request \"peer.tools.ghost\" {})\nend\n",
    );
    assert_eq!(load(dir.path()).err().unwrap().code, "mcp.unknown_import");
    stdio_client(
        dir.path(),
        peer.path(),
        "[\"demo.add\"]",
        "operation peer.tools.demo.add\n    output json\n    return 1\nend\n",
    );
    assert_eq!(load(dir.path()).err().unwrap().code, "mcp.alias_collision");
}

// ------------------------------------------------------------------ stdio: scripted fake

const FAKE_SH: &str = r#"log="$1"
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"1"}}}' ;;
    *'"method":"tools/call"'*)
      case "$line" in
        *'"name":"sample"'*)
          printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":1,"progress":1}}'
          printf '%s\n' '{"jsonrpc":"2.0","id":"s1","method":"sampling/createMessage","params":{"messages":[],"maxTokens":1}}' ;;
        *'"name":"fail"'*)
          printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"nope"}],"isError":true}}' ;;
        *'"name":"broken"'*)
          printf '%s\n' '{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"internal","data":{"why":"fixture"}}}' ;;
        *)
          printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","data":"not data"}}'
          printf '%s\n' '{"jsonrpc":"2.0","id":"p1","method":"ping"}'
          printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"ok"},{"type":"image","data":"AAEC","mimeType":"image/png"}],"structuredContent":{"ok":true},"isError":false}}' ;;
      esac ;;
    *) printf '%s\n' "$line" >> "$log" ;;
  esac
done
"#;

fn fake_bundle(dir: &Path, tools: &[&str]) -> String {
    let script = write(dir, "fixture/fake.sh", FAKE_SH);
    let log = dir.join("logs/peer.log");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(&log, "").unwrap();
    let list: Vec<Json> = tools
        .iter()
        .map(|t| json!({"name": t, "inputSchema": {"type": "object"}}))
        .collect();
    let text =
        serde_json::to_string(&json!({"protocolVersion": "2025-11-25", "tools": list})).unwrap();
    write(dir, "schemas/fake.json", &text);
    let names: Vec<String> = tools.iter().map(|t| format!("\"{t}\"")).collect();
    let sampling = if tools.contains(&"sample") {
        "\noperation fixture.mcp_sampling\n    output json\n    return (request \"fake.tools.sample\" {})\nend\n"
    } else {
        ""
    };
    write(
        dir,
        "app.rivet",
        &format!(
            "connector fake mcp\n    transport command \"/bin/sh\"\n        args [\"{}\", \"{}\"]\n    end\n    schema \"./schemas/fake.json\"\n    expose tools [{}]\nend\n{sampling}",
            canon(&script),
            canon(&log),
            names.join(", ")
        ),
    );
    let targets: Vec<String> = tools.iter().map(|t| format!("fake/tools/{t}")).collect();
    write(
        dir,
        "policy.json",
        &policy(
            json!([
                {"capability": "allow_exec", "targets": ["/bin/sh", "/bin/bash", "/bin/dash"]},
                {"capability": "allow_read", "targets": ["./fixture/**"]},
                {"capability": "allow_write", "targets": ["./logs/**"]},
                {"capability": "allow_mcp", "targets": targets}
            ]),
            &[snapshot_hash(text.as_bytes())],
        ),
    );
    canon(&log)
}

// vhco:test connectors.invoke_mcp -- scripted stdio fake: interleaved log/progress notifications and a server ping are not data, binary content blocks stay tagged, isError → mcp.tool_failed, JSON-RPC error keeps code and data, sampling is declined (-32601) with protocol.unsupported_capability and notifications/cancelled
#[tokio::test(flavor = "multi_thread")]
async fn stdio_fake_server_errors_and_sampling() {
    let dir = tempfile::tempdir().unwrap();
    let log = fake_bundle(dir.path(), &["echo", "fail", "broken", "sample"]);
    let rt = load(dir.path()).unwrap();
    let ok = match rt.request("fake.tools.echo", Value::Null, None).await {
        Err(e) if sandbox_missing(&e) => return,
        other => other.unwrap(),
    };
    let j = ok.result.to_json();
    assert_eq!(j["structuredContent"], json!({"ok": true}));
    assert_eq!(j["content"].as_array().unwrap().len(), 2);
    assert_eq!(
        j["content"][1],
        json!({"type": "image", "data": "AAEC", "mimeType": "image/png"})
    );

    let e = rt
        .request("fake.tools.fail", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "mcp.tool_failed");
    assert_eq!(e.effects, rivet::domain::EffectsStatus::Unknown);

    let e = rt
        .request("fake.tools.broken", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "protocol.mcp_error");
    assert_eq!(e.details.get("code"), Some(&Value::Int(-32603)));
    assert_eq!(
        e.details.get("data").map(|d| d.to_json()),
        Some(json!({"why": "fixture"}))
    );

    // S61: the fixture requests sampling; the host declines, no model call.
    let e = rt
        .request("fixture.mcp_sampling", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "protocol.unsupported_capability");
    assert_eq!(e.exit_code(), 5);
    let seen = std::fs::read_to_string(&log).unwrap();
    assert!(
        seen.contains("\"id\":\"s1\"") && seen.contains("-32601"),
        "sampling was not declined: {seen}"
    );
    assert!(
        seen.contains("notifications/cancelled"),
        "no cancellation notice: {seen}"
    );
    assert!(seen.contains("notifications/initialized"));
}

// vhco:test connectors.invoke_mcp -- stdio transport needs allow_exec and allow_mcp: without them the call is permission.denied before anything is spawned
#[tokio::test(flavor = "multi_thread")]
async fn stdio_transport_is_authorized() {
    let dir = tempfile::tempdir().unwrap();
    let log = fake_bundle(dir.path(), &["echo"]);
    let text = std::fs::read_to_string(dir.path().join("schemas/fake.json")).unwrap();
    let hash = snapshot_hash(text.as_bytes());
    write(
        dir.path(),
        "policy.json",
        &policy(
            json!([{"capability": "allow_mcp", "targets": ["fake/tools/echo"]}]),
            std::slice::from_ref(&hash),
        ),
    );
    let rt = load(dir.path()).unwrap();
    let e = rt
        .request("fake.tools.echo", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    assert!(e.message.contains("allow_exec"), "{}", e.message);
    write(
        dir.path(),
        "policy.json",
        &policy(
            json!([{"capability": "allow_exec", "targets": ["/bin/sh"]}]),
            &[hash],
        ),
    );
    let rt = load(dir.path()).unwrap();
    let e = rt
        .request("fake.tools.echo", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    assert!(e.message.contains("allow_mcp"), "{}", e.message);
    assert_eq!(
        std::fs::read_to_string(log).unwrap(),
        "",
        "nothing may be spawned"
    );
}

// ------------------------------------------------------------------ HTTP: rivet serve /mcp

// vhco:test connectors.invoke_mcp -- Streamable HTTP against Rivet's own serve /mcp: sync over HTTP, tool success, isError → mcp.tool_failed, and allow_network is required per POST
#[tokio::test(flavor = "multi_thread")]
async fn http_rivet_serve_peer() {
    let peer_rt = Runtime::builder()
        .source("app.rivet", PEER, ".")
        .build()
        .unwrap();
    let server = serve_on(peer_rt, "127.0.0.1:0").await;
    let addr = server.addr.unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("schemas")).unwrap(); // `connectors sync --output` never creates directories
    write(
        dir.path(),
        "app.rivet",
        &format!(
            "connector peer mcp\n    transport http \"http://{addr}/mcp\"\n    schema \"./schemas/peer.json\"\n    expose tools [\"demo.add\", \"demo.bad\"]\nend\n\noperation bridge.add\n    param a integer required\n    output json\n    r = (request \"peer.tools.demo.add\" {{a: a, b: 40}})\n    return r.structuredContent.result\nend\n"
        ),
    );
    let grants = json!([
        {"capability": "allow_network", "targets": [format!("http://{addr}")]},
        {"capability": "allow_mcp", "targets": ["peer/discover", "peer/tools/demo.add", "peer/tools/demo.bad"]},
        {"capability": "allow_write", "targets": ["./schemas/**"]}
    ]);
    write(dir.path(), "policy.json", &policy(grants.clone(), &[]));
    let receipt = load_discovery(dir.path())
        .sync_connector("peer", "./schemas/peer.json")
        .await
        .unwrap();
    assert_eq!(receipt.protocol_version, "2025-11-25");
    write(
        dir.path(),
        "policy.json",
        &policy(grants, std::slice::from_ref(&receipt.sha256)),
    );
    let rt = load(dir.path()).unwrap();
    let c = rt
        .request("bridge.add", Value::object([("a", Value::Int(2))]), None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::Int(42));
    let e = rt
        .request("peer.tools.demo.bad", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "mcp.tool_failed");

    // Without the network grant every POST is refused before connecting.
    write(
        dir.path(),
        "policy.json",
        &policy(
            json!([{"capability": "allow_mcp", "targets": ["peer/tools/demo.add"]}]),
            std::slice::from_ref(&receipt.sha256),
        ),
    );
    let rt = load(dir.path()).unwrap();
    let e = rt
        .request("bridge.add", Value::object([("a", Value::Int(2))]), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    assert!(e.message.contains("allow_network"), "{}", e.message);
    server.shutdown().await;
}

// vhco:test connectors.invoke_mcp -- recursion: a bundle bridged to itself over MCP stops at the repeated identity (limit.mcp_recursion), and a chain of distinct bridged operations stops at MAX_MCP_HOPS (limit.mcp_hops)
#[tokio::test(flavor = "multi_thread")]
async fn bridge_recursion_is_bounded() {
    let port = free_port();
    let dir = tempfile::tempdir().unwrap();
    let steps = 12;
    let mut src = format!(
        "connector self mcp\n    transport http \"http://127.0.0.1:{port}/mcp\"\n    schema \"./schemas/self.json\"\n    expose tools [\"loop.again\""
    );
    for i in 1..=steps {
        src.push_str(&format!(", \"step.s{i}\""));
    }
    src.push_str("]\nend\n\noperation loop.again\n    output json\n    return (request \"self.tools.loop.again\" {})\nend\n");
    for i in 0..steps {
        src.push_str(&format!(
            "\noperation step.s{i}\n    output json\n    return (request \"self.tools.step.s{}\" {{}})\nend\n",
            i + 1
        ));
    }
    src.push_str(&format!(
        "\noperation step.s{steps}\n    output json\n    return {{done: true}}\nend\n"
    ));
    write(dir.path(), "app.rivet", &src);
    let mut tools = vec![json!({"name": "loop.again", "inputSchema": {"type": "object"}})];
    for i in 1..=steps {
        tools.push(json!({"name": format!("step.s{i}"), "inputSchema": {"type": "object"}}));
    }
    let snap =
        serde_json::to_string(&json!({"protocolVersion": "2025-11-25", "tools": tools})).unwrap();
    write(dir.path(), "schemas/self.json", &snap);
    write(
        dir.path(),
        "policy.json",
        &policy(
            json!([
                {"capability": "allow_network", "targets": [format!("http://127.0.0.1:{port}")]},
                {"capability": "allow_mcp", "targets": ["self/tools/*"]}
            ]),
            &[snapshot_hash(snap.as_bytes())],
        ),
    );
    let rt = load(dir.path()).unwrap();
    let server = serve_on(rt.clone(), &format!("127.0.0.1:{port}")).await;

    let e = rt
        .request("loop.again", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "mcp.tool_failed");
    assert!(
        error_text(&e).contains("limit.mcp_recursion"),
        "{}",
        error_text(&e)
    );

    let e = rt.request("step.s0", Value::Null, None).await.unwrap_err();
    let text = error_text(&e);
    assert!(text.contains("limit.mcp_hops"), "{text}");
    assert!(
        !text.contains("\"done\""),
        "the chain must stop before the last step"
    );
    server.shutdown().await;
}

// ------------------------------------------------------------------ HTTP: axum fake with the demo fixtures

#[derive(Clone, Default)]
struct FakeHttp {
    seen: Arc<Mutex<Vec<Json>>>,
}

fn sse(messages: &[Json]) -> Response {
    let body: String = messages
        .iter()
        .map(|m| format!("event: message\ndata: {m}\n\n"))
        .collect();
    ([(header::CONTENT_TYPE, "text/event-stream")], body).into_response()
}

async fn fake_mcp(
    axum::extract::State(st): axum::extract::State<FakeHttp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let msg: Json = serde_json::from_slice(&body).unwrap();
    st.seen.lock().unwrap().push(msg.clone());
    let id = msg["id"].clone();
    let method = msg["method"].as_str().unwrap_or("");
    if method != "initialize"
        && headers.get("mcp-session-id").and_then(|v| v.to_str().ok()) != Some("fake-session")
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let fixture = |name: &str| -> Json {
        let text =
            std::fs::read_to_string(format!("docs/demos/06-mcp-bridge/schemas/{name}")).unwrap();
        let mut j: Json = serde_json::from_str(&text).unwrap();
        j["id"] = id.clone();
        j
    };
    match method {
        "initialize" => (
            [("mcp-session-id", "fake-session")],
            axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": {
                "protocolVersion": "2025-11-25",
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "crm-fixture", "version": "1.0"}}})),
        )
            .into_response(),
        "tools/list" => axum::Json(fixture("tools-list.fixture.json")).into_response(),
        "tools/call" if msg["params"]["arguments"]["query"] == json!("sample") => sse(&[
            json!({"jsonrpc": "2.0", "method": "notifications/message", "params": {"level": "info", "data": "x"}}),
            json!({"jsonrpc": "2.0", "id": 99, "method": "elicitation/create", "params": {"message": "?", "requestedSchema": {"type": "object"}}}),
        ]),
        "tools/call" => sse(&[
            json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {"progressToken": 7, "progress": 0.5}}),
            fixture("search-result.fixture.json"),
        ]),
        _ if id.is_null() => StatusCode::ACCEPTED.into_response(),
        _ => axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": {}})).into_response(),
    }
}

async fn start_fake() -> (std::net::SocketAddr, FakeHttp) {
    let state = FakeHttp::default();
    let app = Router::new()
        .route(
            "/mcp",
            post(fake_mcp).delete(|| async { StatusCode::NO_CONTENT }),
        )
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (addr, state)
}

/// docs/demos/06-mcp-bridge copied into a temp dir with the connector URL
/// pointed at the local fixture server.
fn demo_bundle(dir: &Path, url: &str) {
    let app = std::fs::read_to_string("docs/demos/06-mcp-bridge/app.rivet").unwrap();
    write(
        dir,
        "app.rivet",
        &app.replace("https://mcp.example.com/mcp", url),
    );
    let sync = std::fs::read_to_string("docs/demos/06-mcp-bridge/policies/sync.json").unwrap();
    write(dir, "policies/sync.json", &sync);
    let default = std::fs::read_to_string("docs/demos/06-mcp-bridge/policy.json").unwrap();
    write(dir, "policy.json", &default);
}

// vhco:test connectors.invoke_mcp -- docs/demos/06-mcp-bridge flows: load fails without the reviewed snapshot (exit 4), the I/O manifest shows the transport + opaque_remote tool sites, sync is denied by the default policy (exit 3), sync with policies/sync.json writes a candidate whose approval makes contacts.find return Ada over Streamable HTTP (SSE), with the upstream MCP session id reused and progress not treated as data
#[tokio::test(flavor = "multi_thread")]
async fn demo_06_readme_flows() {
    let (addr, fake) = start_fake().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("schemas")).unwrap(); // `connectors sync --output` never creates directories
    // Unmodified demo: no snapshot → bundle load fails with not_found (exit 4).
    let e = Runtime::builder()
        .file("docs/demos/06-mcp-bridge/app.rivet")
        .build()
        .err()
        .unwrap();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("not_found.mcp_snapshot", 4)
    );

    demo_bundle(dir.path(), &format!("http://{addr}/mcp"));
    // Default policy has no crm/discover or write grant → denied (exit 3), nothing written.
    let e = load_discovery(dir.path())
        .sync_connector("crm", "./schemas/crm.next.json")
        .await
        .unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("permission.denied", 3));
    assert!(fake.seen.lock().unwrap().is_empty());

    // With policies/sync.json (grants rewritten to the local fixture origin).
    let sync_policy = dir.path().join("policies/sync.json");
    let text = std::fs::read_to_string(&sync_policy)
        .unwrap()
        .replace("https://mcp.example.com:443", &format!("http://{addr}"));
    std::fs::write(&sync_policy, text).unwrap();
    let rt = Runtime::builder()
        .file(dir.path().join("app.rivet").to_str().unwrap())
        .policy_file(sync_policy.to_str().unwrap())
        .connector_discovery()
        .build()
        .unwrap();
    let receipt = rt
        .sync_connector("crm", "./schemas/crm.next.json")
        .await
        .unwrap();
    assert_eq!(receipt.tools, vec!["search".to_string()]);
    // The candidate never replaces the reviewed path; promote + approve explicitly.
    assert!(!dir.path().join("schemas/crm.json").exists());
    std::fs::copy(
        dir.path().join("schemas/crm.next.json"),
        dir.path().join("schemas/crm.json"),
    )
    .unwrap();
    let mut p: Json =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("policy.json")).unwrap())
            .unwrap();
    p["grants"][0]["targets"] = json!([format!("http://{addr}")]);
    p["approved"] = json!({"snapshots": [receipt.sha256]});
    write(
        dir.path(),
        "policy.json",
        &serde_json::to_string_pretty(&p).unwrap(),
    );
    let rt = load(dir.path()).unwrap();

    // io --check-policy: transport + opaque_remote MCP call, both allowed.
    let report = rt
        .io(&rivet::domain::io_manifest::IoQuery {
            ids: vec!["contacts.find".into()],
            check_policy: true,
            format: "json".into(),
            ..Default::default()
        })
        .unwrap();
    let m = report.manifest.to_json();
    let sites = m["sites"].as_array().unwrap();
    assert!(
        sites
            .iter()
            .any(|s| s["capability"] == json!("allow_network"))
    );
    assert!(sites.iter().any(|s| s["capability"] == json!("allow_mcp")
        && s["knowledge"] == json!("opaque_remote")
        && s["target"]["template"] == json!("crm/tools/search")));
    assert_eq!(report.exit_code, 0);

    fake.seen.lock().unwrap().clear();
    let c = rt
        .request(
            "contacts.find",
            Value::object([("query", Value::text("Ada"))]),
            None,
        )
        .await
        .unwrap();
    let j = c.result.to_json();
    assert_eq!(j["structuredContent"]["contacts"][0]["name"], json!("Ada"));
    assert_eq!(j["isError"], json!(false));
    assert_eq!(j["content"][0]["type"], json!("text"));
    let seen = fake.seen.lock().unwrap().clone();
    let methods: Vec<&str> = seen.iter().filter_map(|m| m["method"].as_str()).collect();
    assert_eq!(
        methods,
        vec!["initialize", "notifications/initialized", "tools/call"]
    );
    assert_eq!(seen[2]["params"]["name"], json!("search"));
    assert_eq!(seen[2]["params"]["_meta"]["rivet/hops"], json!(1));

    // Elicitation over SSE is declined like sampling.
    let e = rt
        .request(
            "contacts.find",
            Value::object([("query", Value::text("sample"))]),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "protocol.unsupported_capability");
    let seen = fake.seen.lock().unwrap().clone();
    assert!(
        seen.iter()
            .any(|m| m["id"] == json!(99) && m["error"]["code"] == json!(-32601))
    );
    assert!(
        seen.iter()
            .any(|m| m["method"] == json!("notifications/cancelled"))
    );

    // Without the MCP grant: permission.denied before any transport I/O.
    p["grants"] = json!([{"capability": "allow_network", "targets": [format!("http://{addr}")]}]);
    write(
        dir.path(),
        "policy.json",
        &serde_json::to_string_pretty(&p).unwrap(),
    );
    let rt = load(dir.path()).unwrap();
    fake.seen.lock().unwrap().clear();
    let e = rt
        .request(
            "contacts.find",
            Value::object([("query", Value::text("Ada"))]),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("permission.denied", 3));
    assert!(fake.seen.lock().unwrap().is_empty());
}

// ------------------------------------------------------------------ OAuth on an MCP connector (S92)

const MCP_SECRET_VAR: &str = "RIVET_T06_MCP_CLIENT_SECRET";

/// An MCP endpoint that answers only requests bearing a fake-issued token
/// (`Bearer CANARY-AT-n`); `whoami` reports whether one arrived.
async fn protected_mcp(headers: HeaderMap, body: Bytes) -> Response {
    let authed = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer CANARY-AT-"));
    if !authed {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let msg: Json = serde_json::from_slice(&body).unwrap();
    let id = msg["id"].clone();
    match msg["method"].as_str().unwrap_or("") {
        "initialize" => axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": {
            "protocolVersion": "2025-11-25",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "protected", "version": "1"}}}))
        .into_response(),
        "tools/list" => axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": {"tools": [
            {"name": "whoami", "description": "Who called", "inputSchema": {"type": "object", "properties": {"verbose": {"type": "boolean", "description": "More detail."}}}}]}}))
        .into_response(),
        "tools/call" => axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": {
            "content": [{"type": "text", "text": "bearer"}],
            "structuredContent": {"bearer": true}, "isError": false}}))
        .into_response(),
        _ if id.is_null() => StatusCode::ACCEPTED.into_response(),
        _ => axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": {}})).into_response(),
    }
}

// vhco:test connectors.invoke_mcp -- `auth PROFILE account A` on an http MCP connector: an origin-bound client_credentials lease is attached as a bearer (never returned), imported tools appear in list/describe/outputs and MCP tools/list with their snapshot inputSchema, rivet.connectors.sync runs for the local principal, and stdio + auth fails at load
#[tokio::test(flavor = "multi_thread")]
async fn http_connector_oauth_and_catalog() {
    // SAFETY: set once for this test binary before any runtime reads the environment.
    unsafe { std::env::set_var(MCP_SECRET_VAR, oauth_support::CLIENT_SECRET) };
    let oauth = oauth_support::Fake::start().await;
    let app = Router::new().route(
        "/mcp",
        post(protected_mcp).delete(|| async { StatusCode::NO_CONTENT }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let origin = format!("http://{addr}");
    let a = oauth.auth_origin();
    let dir = tempfile::tempdir().unwrap();
    let snapshot = r#"{"format": "rivet.mcp.snapshot/1", "protocolVersion": "2025-11-25", "tools": [{"name": "whoami", "description": "Who called", "inputSchema": {"type": "object", "properties": {"verbose": {"type": "boolean", "description": "More detail."}}}}]}"#;
    write(dir.path(), "schemas/crm.json", snapshot);
    let hash = snapshot_hash(snapshot.as_bytes());
    write(
        dir.path(),
        "app.rivet",
        &format!(
            "auth crm_service oauth2\n    flow client_credentials\n    issuer \"{a}\"\n    token_url \"{a}/token\"\n    client_id \"rivet-service\"\n    client_secret env \"{MCP_SECRET_VAR}\"\n    client_auth basic\n    scopes [\"contacts.read\"]\n    resource_origins [\"{origin}\"]\n    store memory\nend\n\nconnector crm mcp\n    transport http \"{origin}/mcp\"\n    schema \"./schemas/crm.json\"\n    auth crm_service account \"service\"\n    expose tools [\"whoami\"]\nend\n"
        ),
    );
    let grants = json!([
        {"capability": "allow_network", "targets": [origin, a]},
        {"capability": "allow_mcp", "targets": ["crm/tools/whoami", "crm/discover"]},
        {"capability": "allow_auth", "targets": ["crm_service/service/use"]},
        {"capability": "allow_credentials", "targets": ["crm_service/service"]},
        {"capability": "allow_env", "targets": [MCP_SECRET_VAR]},
        {"capability": "allow_write", "targets": ["./schemas/**"]}
    ]);
    write(
        dir.path(),
        "policy.json",
        &policy(grants.clone(), std::slice::from_ref(&hash)),
    );
    let rt = load(dir.path()).unwrap();
    let c = rt
        .request("crm.tools.whoami", Value::Null, None)
        .await
        .unwrap();
    let text = c.to_json().to_string();
    assert_eq!(c.to_json()["result"]["structuredContent"]["bearer"], true);
    assert!(!text.contains("CANARY"), "{text}");
    // A second call reuses the cached lease (one token grant).
    rt.request("crm.tools.whoami", Value::Null, None)
        .await
        .unwrap();
    assert_eq!(
        oauth.with(|s| s
            .grants
            .iter()
            .filter(|g| g.as_str() == "client_credentials")
            .count()),
        1
    );

    // The import is a catalog entry with its snapshot schema on every listing.
    let ids: Vec<String> = rt
        .list()
        .unwrap()
        .entries
        .iter()
        .map(|e| e.id.clone())
        .collect();
    assert!(ids.contains(&"crm.tools.whoami".to_string()), "{ids:?}");
    let d = rt.describe(&["crm.tools.whoami".into()]).unwrap().entries[0].describe_json();
    assert_eq!(
        d["input"]["properties"]["verbose"]["description"],
        "More detail."
    );
    assert_eq!(d["name"], "whoami");
    let o = rt.outputs(Some("crm.tools.whoami"), false).unwrap()[0].to_json();
    assert_eq!(o["output"]["required"], json!(["content", "isError"]));
    let server = serve_on(rt.clone(), "127.0.0.1:0").await;
    let mcp = reqwest_like_post(
        server.addr.unwrap(),
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
    )
    .await;
    let tool = mcp["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "crm.tools.whoami")
        .cloned()
        .expect("imported tool listed");
    assert_eq!(
        tool["inputSchema"]["properties"]["verbose"]["type"],
        "boolean"
    );
    server.shutdown().await;

    // rivet.connectors.sync mirrors `rivet connectors sync` (local principal).
    let c = rt
        .request(
            "rivet.connectors.sync",
            Value::from_json(&json!({"name": "crm", "output": "./schemas/crm.next.json"})),
            None,
        )
        .await
        .unwrap();
    assert!(
        c.result
            .get("sha256")
            .and_then(Value::as_str)
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(dir.path().join("schemas/crm.next.json").exists());

    // stdio transport + auth is refused at load.
    let source = std::fs::read_to_string(dir.path().join("app.rivet")).unwrap();
    let profile = source
        .split("connector crm mcp")
        .next()
        .unwrap()
        .to_string();
    write(
        dir.path(),
        "app.rivet",
        &format!(
            "{profile}connector crm mcp\n    transport command \"/bin/cat\"\n    schema \"./schemas/crm.json\"\n    auth crm_service account \"service\"\n    expose tools [\"whoami\"]\nend\n"
        ),
    );
    let e = load(dir.path()).err().unwrap();
    assert_eq!((e.code.as_str(), e.exit_code()), ("mcp.auth_transport", 2));
}

/// One JSON-RPC POST to a serve's /mcp (initialize first for a session).
async fn reqwest_like_post(addr: std::net::SocketAddr, msg: Json) -> Json {
    use http_body_util::{BodyExt, Full};
    use hyper_util::rt::TokioIo;
    async fn post(
        addr: std::net::SocketAddr,
        body: Json,
        session: Option<&str>,
    ) -> (Option<String>, Json) {
        let io = TokioIo::new(tokio::net::TcpStream::connect(addr).await.unwrap());
        let (mut s, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();
        tokio::spawn(conn);
        let mut b = hyper::Request::post("/mcp")
            .header("host", addr.to_string())
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25");
        if let Some(sid) = session {
            b = b.header("mcp-session-id", sid);
        }
        let resp = s
            .send_request(
                b.body(Full::new(axum::body::Bytes::from(body.to_string())))
                    .unwrap(),
            )
            .await
            .unwrap();
        let sid = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (sid, serde_json::from_slice(&bytes).unwrap_or(Json::Null))
    }
    let (sid, _) = post(
        addr,
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}),
        None,
    )
    .await;
    post(addr, msg, sid.as_deref()).await.1
}

// vhco:test connectors.invoke_mcp -- G21: `connectors sync --output` refuses an existing path (conflict.already_exists) before any connection reaches the server
#[tokio::test(flavor = "multi_thread")]
async fn sync_refuses_existing_output_before_contacting_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(Mutex::new(0u32));
    let h = hits.clone();
    tokio::spawn(async move {
        while let Ok((_s, _)) = listener.accept().await {
            *h.lock().unwrap() += 1;
        }
    });
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "app.rivet",
        &format!(
            "connector peer mcp\n    transport http \"http://{addr}/mcp\"\n    schema \"./schemas/peer.json\"\n    expose tools [\"demo.add\"]\nend\n"
        ),
    );
    write(
        dir.path(),
        "policy.json",
        &policy(
            json!([
                {"capability": "allow_network", "targets": [format!("http://{addr}")]},
                {"capability": "allow_mcp", "targets": ["peer/discover"]},
                {"capability": "allow_write", "targets": ["./schemas/**"]}
            ]),
            &[],
        ),
    );
    write(dir.path(), "schemas/peer.json", "{\"keep\": true}");
    let rt = load_discovery(dir.path());
    let e = rt
        .sync_connector("peer", "./schemas/peer.json")
        .await
        .unwrap_err();
    assert_eq!(e.code, "conflict.already_exists");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(*hits.lock().unwrap(), 0, "the server was contacted");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("schemas/peer.json")).unwrap(),
        "{\"keep\": true}"
    );
}
