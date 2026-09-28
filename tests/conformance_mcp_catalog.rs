//! T-18 — incoming MCP: Streamable HTTP at /mcp and `serve --stdio` publish the
//! same catalog as direct named tools plus the built-ins; unary tools return
//! Completion, streaming tools a SessionReceipt, failures `isError`.

mod support;

use serde_json::{Value as Json, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use support::*;

fn tool<'a>(tools: &'a [Json], name: &str) -> Option<&'a Json> {
    tools.iter().find(|t| t["name"] == name)
}

// vhco:test registry.describe_operations -- tools/list mirrors the catalog: name=id, title, description, inputSchema = describe input, outputSchema = ResponseEnvelope{data: declared output}
#[tokio::test]
async fn tools_list_matches_the_catalog() {
    let s = serve(None).await;
    let a = s.addr;
    let sid = mcp_init(a, &[]).await;
    let list = std::fs::read_to_string("docs/demos/01-catalog/requests/list.mcp.json").unwrap();
    let r = mcp_raw(a, &sid, &[], serde_json::from_str(&list).unwrap()).await;
    let tools = r.json()["result"]["tools"].as_array().unwrap().clone();
    for e in s.rt.list().unwrap().entries {
        let t = tool(&tools, &e.id).unwrap_or_else(|| panic!("{} listed", e.id));
        let d = e.describe_json();
        assert_eq!(t["title"], d["name"]);
        assert_eq!(t["description"], d["description"]);
        assert_eq!(t["inputSchema"], d["input"]);
        if e.streaming() {
            assert_eq!(t["_meta"], json!({"rivet/delivery":"session"}));
            assert!(
                t["outputSchema"]["properties"]["data"]["anyOf"][0]["properties"]["session_id"]
                    .is_object()
            );
        } else {
            assert_eq!(
                t["outputSchema"]["properties"]["data"]["anyOf"][0],
                d["output"]
            );
            assert!(t.get("_meta").is_none());
        }
    }
    assert!(
        tool(&tools, "demo.secret").is_none(),
        "private helpers are not published"
    );
    for b in [
        "rivet.request",
        "rivet.list",
        "rivet.describe",
        "rivet.outputs",
        "rivet.sessions.open",
        "rivet.sessions.send",
        "rivet.sessions.finish_input",
        "rivet.sessions.read",
        "rivet.sessions.cancel",
    ] {
        assert!(tool(&tools, b).is_some(), "{b} listed");
    }
    // no-param tool: closed empty object
    assert_eq!(
        tool(&tools, "demo.health").unwrap()["inputSchema"],
        json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
    );
    s.handle.shutdown().await;
}

// vhco:test execution.request_operation -- tools/call runs the shared dispatcher: envelope (status ok) for unary, an accepted envelope with the SessionReceipt then rivet.sessions.read for streaming, isError for invalid arguments, protocol error for unknown/private tools
#[tokio::test]
async fn tools_call_unary_streaming_and_errors() {
    let s = serve(None).await;
    let a = s.addr;
    let sid = mcp_init(a, &[]).await;
    let add = std::fs::read_to_string("docs/demos/01-catalog/requests/add.mcp.json").unwrap();
    let r = mcp_raw(a, &sid, &[], serde_json::from_str(&add).unwrap())
        .await
        .json();
    assert_eq!(r["id"], 3);
    let res = &r["result"];
    assert_eq!(res["isError"], false);
    assert_eq!(res["structuredContent"]["data"], 5);
    let text: Json = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, res["structuredContent"]);
    let outputs =
        std::fs::read_to_string("docs/demos/01-catalog/requests/outputs.mcp.json").unwrap();
    let r = mcp_raw(a, &sid, &[], serde_json::from_str(&outputs).unwrap())
        .await
        .json();
    assert_eq!(
        r["result"]["structuredContent"]["data"]["output"],
        json!({"type":"integer","description":"Sum of a and b."})
    );
    // invalid arguments: tool error with the ErrorEnvelope, operation not run
    let res = mcp_call(a, &sid, &[], "demo.add", json!({"b":3})).await;
    assert_eq!(res["isError"], true);
    assert_eq!(
        res["structuredContent"]["error"]["code"],
        "validation.required"
    );
    // unknown and private tools are protocol errors
    for name in ["demo.missing", "demo.secret"] {
        let r = mcp_call(a, &sid, &[], name, json!({})).await;
        assert_eq!(r["error"]["code"], -32602);
    }
    // streaming tool → SessionReceipt, then read with the session tools
    let res = mcp_call(a, &sid, &[], "demo.count", json!({"n":2})).await;
    assert_eq!(res["isError"], false);
    assert_eq!(res["structuredContent"]["status"], "accepted");
    let receipt = res["structuredContent"]["data"].clone();
    let session = receipt["session_id"].as_str().unwrap().to_string();
    assert_eq!(receipt["next_send_seq"], 1);
    let mut after = 0;
    let mut kinds = Vec::new();
    loop {
        let b = mcp_call(
            a,
            &sid,
            &[],
            "rivet.sessions.read",
            json!({"session_id":session,"after_seq":after,"wait_ms":2000}),
        )
        .await["structuredContent"]["data"]
            .clone();
        for e in b["events"].as_array().unwrap() {
            kinds.push(e["type"].as_str().unwrap().to_string());
        }
        after = b["last_seq"].as_u64().unwrap();
        if b["terminal"] == true {
            break;
        }
    }
    assert_eq!(kinds, vec!["data", "data", "result"]);
    // the same session is reachable through the polling route for the same principal
    let r = http(
        a,
        "GET",
        &format!("/v1/requests/{session}/events?after_seq={after}&wait_ms=10"),
        &[],
        "",
    )
    .await;
    assert_eq!(r.status, 200);
    // duplex tool through the built-in session tools
    let res = mcp_call(a, &sid, &[], "demo.relay", json!({})).await;
    let session = res["structuredContent"]["data"]["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    let ack = mcp_call(
        a,
        &sid,
        &[],
        "rivet.sessions.send",
        json!({"session_id":session,"send_seq":1,"data":"hi"}),
    )
    .await;
    assert_eq!(ack["structuredContent"]["data"]["accepted_seq"], 1);
    let bad = mcp_call(
        a,
        &sid,
        &[],
        "rivet.sessions.send",
        json!({"session_id":session,"send_seq":1,"data":"changed"}),
    )
    .await;
    assert_eq!(bad["isError"], true);
    assert_eq!(
        bad["structuredContent"]["error"]["code"],
        "conflict.input_sequence"
    );
    // unknown method / missing session header
    let r = mcp_raw(
        a,
        &sid,
        &[],
        json!({"jsonrpc":"2.0","id":9,"method":"resources/list"}),
    )
    .await;
    assert_eq!(r.json()["error"]["code"], -32601);
    let r = http(
        a,
        "POST",
        "/mcp",
        &[],
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),
    )
    .await;
    assert_eq!(r.status, 422);
    let r = http(a, "GET", "/mcp", &[("mcp-session-id", sid.as_str())], "").await;
    assert_eq!(r.status, 405);
    let r = http(a, "DELETE", "/mcp", &[("mcp-session-id", sid.as_str())], "").await;
    assert_eq!(r.status, 204);
    let r = http(a, "DELETE", "/mcp", &[("mcp-session-id", sid.as_str())], "").await;
    assert_eq!(r.status, 404);
    s.handle.shutdown().await;
}

// vhco:test serve.authorize_operation -- MCP filters tools/list per principal and treats unauthorized tools as unknown
#[tokio::test]
async fn unauthorized_tools_are_hidden() {
    let team = std::fs::read_to_string("docs/demos/01-catalog/policies/team.json").unwrap();
    let s = serve(Some(&team)).await;
    let ci = [("authorization", "Bearer dev-token-ci")];
    let sid = mcp_init(s.addr, &ci).await;
    let r = mcp_raw(
        s.addr,
        &sid,
        &ci,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let names: Vec<String> = r.json()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .filter(|n| n.starts_with("demo."))
        .collect();
    assert_eq!(names, vec!["demo.health"]);
    let r = mcp_call(s.addr, &sid, &ci, "demo.add", json!({"a":1})).await;
    assert_eq!(r["error"]["code"], -32602);
    // the generic rivet.request path is authorized on its target too
    let r = mcp_call(
        s.addr,
        &sid,
        &ci,
        "rivet.request",
        json!({"operation":"demo.add","data":{"a":1}}),
    )
    .await;
    assert_eq!(r["isError"], true);
    assert_eq!(r["structuredContent"]["error"]["code"], "permission.denied");
    s.handle.shutdown().await;
}

// vhco:test serve.start_serve -- serve --stdio speaks MCP on stdin/stdout with protocol-only stdout
#[test]
fn stdio_serves_the_same_catalog() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args([
            "--file",
            "docs/demos/01-catalog/app.rivet",
            "serve",
            "--stdio",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut rpc = |msg: &str| -> Option<Json> {
        let compact: Json = serde_json::from_str(msg).unwrap();
        writeln!(stdin, "{compact}").unwrap();
        stdin.flush().unwrap();
        compact.get("id")?;
        let mut line = String::new();
        out.read_line(&mut line).unwrap();
        Some(serde_json::from_str(&line).expect("stdout carries only JSON-RPC"))
    };
    let dir = "docs/demos/01-catalog/requests";
    let read = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
    let init = rpc(&read("initialize.mcp.json")).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert!(rpc(&read("initialized.mcp.json")).is_none());
    let list = rpc(&read("list.mcp.json")).unwrap();
    let tools = list["result"]["tools"].as_array().unwrap();
    for id in [
        "demo.greet",
        "demo.add",
        "demo.health",
        "demo.countdown",
        "rivet.request",
    ] {
        assert!(tool(tools, id).is_some(), "{id}");
    }
    let add = rpc(&read("add.mcp.json")).unwrap();
    assert_eq!(add["result"]["structuredContent"]["data"], 5);
    drop(stdin);
    assert!(child.wait().unwrap().success());
}
