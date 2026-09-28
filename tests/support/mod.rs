//! Shared helpers for the serve/session/MCP conformance suites: an in-process
//! `rivet serve` on 127.0.0.1:0 and real HTTP / WebSocket clients.
#![allow(dead_code)]

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use rivet::Runtime;
use rivet::orchestrator::runtime::policy_from_json;
use rivet::orchestrator::setup_serve::{ServeHandle, ServeOptions, start};
use serde_json::{Value as Json, json};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

/// docs/demos/01-catalog plus the S133 streaming/duplex operations and a private helper.
pub fn catalog() -> String {
    let demo = std::fs::read_to_string("docs/demos/01-catalog/app.rivet").unwrap();
    format!(
        "{demo}
operation demo.count
    description \"Emit the integers 1..n, then return n.\"
    param n integer default 3 min 1 max 50 description \"How many integers to emit.\"
    output integer description \"The last integer emitted.\"
    emits integer
    i = 0
    iterate max 50
        if i == n
            break
        end
        i = i + 1
        emit i
    end
    return n
end

operation demo.relay
    description \"Emit each received text item back, then return how many were relayed.\"
    output integer description \"Number of items relayed.\"
    emits text
    receives text
    count = 0
    for item in incoming
        emit item
        count = count + 1
    end
    return count
end

operation demo.secret
    private true
    output integer
    return 7
end
"
    )
}

pub fn runtime(policy: Option<&str>) -> Runtime {
    let mut b = Runtime::builder().source("app.rivet", &catalog(), ".");
    if let Some(p) = policy {
        b = b.policy(policy_from_json(p.as_bytes(), ".").unwrap());
    }
    b.build().unwrap()
}

pub struct Server {
    pub rt: Runtime,
    pub addr: SocketAddr,
    pub handle: ServeHandle,
}

pub async fn serve(policy: Option<&str>) -> Server {
    let rt = runtime(policy);
    let handle = start(
        rt.clone(),
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = handle.addr.unwrap();
    Server { rt, addr, handle }
}

pub struct Reply {
    pub status: u16,
    pub headers: hyper::HeaderMap,
    pub text: String,
}

impl Reply {
    pub fn json(&self) -> Json {
        serde_json::from_str(&self.text).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.text))
    }
}

/// One HTTP/1.1 exchange on a fresh connection.
pub async fn http(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .unwrap();
    tokio::spawn(conn);
    let mut b = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("host", addr.to_string());
    if !body.is_empty() {
        b = b.header("content-type", "application/json");
    }
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let resp = sender
        .send_request(b.body(Full::new(Bytes::from(body.to_string()))).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        headers,
        text: String::from_utf8_lossy(&bytes).to_string(),
    }
}

pub async fn post(addr: SocketAddr, path: &str, body: Json, headers: &[(&str, &str)]) -> Reply {
    http(addr, "POST", path, headers, &body.to_string()).await
}

/// Parse an SSE body into (id, event, data JSON) triples.
pub fn sse_events(text: &str) -> Vec<(u64, String, Json)> {
    text.split("\n\n")
        .filter(|b| !b.trim().is_empty())
        .map(|block| {
            let mut id = 0;
            let mut ev = String::new();
            let mut data = Json::Null;
            for line in block.lines() {
                if let Some(v) = line.strip_prefix("id: ") {
                    id = v.parse().unwrap();
                } else if let Some(v) = line.strip_prefix("event: ") {
                    ev = v.to_string();
                } else if let Some(v) = line.strip_prefix("data: ") {
                    data = serde_json::from_str(v).unwrap();
                }
            }
            (id, ev, data)
        })
        .collect()
}

pub type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

/// Connect to /v1/ws with subprotocol rivet.v1 (and optional extra headers).
pub async fn ws_connect(addr: SocketAddr, headers: &[(&str, &str)]) -> Result<Ws, String> {
    let mut req = format!("ws://{addr}/v1/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert("sec-websocket-protocol", "rivet.v1".parse().unwrap());
    for (k, v) in headers {
        req.headers_mut().insert(
            hyper::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            v.parse().unwrap(),
        );
    }
    match tokio_tungstenite::connect_async(req).await {
        Ok((ws, resp)) => {
            assert_eq!(
                resp.headers()
                    .get("sec-websocket-protocol")
                    .and_then(|v| v.to_str().ok()),
                Some("rivet.v1")
            );
            Ok(ws)
        }
        Err(e) => Err(e.to_string()),
    }
}

pub async fn ws_send(ws: &mut Ws, frame: Json) {
    ws.send(Message::Text(frame.to_string().into()))
        .await
        .unwrap();
}

/// Next JSON text frame (5 s timeout).
pub async fn ws_recv(ws: &mut Ws) -> Json {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("frame within 5 s")
            .expect("socket open")
            .unwrap();
        if let Message::Text(t) = m {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

/// Collect frames until every listed ref has its terminal frame.
pub async fn ws_until_terminal(ws: &mut Ws, refs: &[&str]) -> Vec<Json> {
    let mut frames = Vec::new();
    let mut open: Vec<String> = refs.iter().map(|s| s.to_string()).collect();
    while !open.is_empty() {
        let f = ws_recv(ws).await;
        if f["type"] == "result" || f["type"] == "error" {
            let r = f["ref"].as_str().unwrap().to_string();
            open.retain(|x| *x != r);
        }
        frames.push(f);
    }
    frames
}

/// MCP initialize + initialized over HTTP; returns the session ID.
pub async fn mcp_init(addr: SocketAddr, auth: &[(&str, &str)]) -> String {
    let body =
        std::fs::read_to_string("docs/demos/01-catalog/requests/initialize.mcp.json").unwrap();
    let mut h = vec![("accept", "application/json, text/event-stream")];
    h.extend_from_slice(auth);
    let r = http(addr, "POST", "/mcp", &h, &body).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.json()["result"]["protocolVersion"], "2025-11-25");
    let sid = r.headers["mcp-session-id"].to_str().unwrap().to_string();
    let r = mcp_raw(
        addr,
        &sid,
        auth,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
    assert_eq!(r.status, 202);
    assert!(r.text.is_empty());
    sid
}

pub async fn mcp_raw(addr: SocketAddr, sid: &str, auth: &[(&str, &str)], msg: Json) -> Reply {
    let mut h = vec![
        ("accept", "application/json, text/event-stream"),
        ("mcp-session-id", sid),
        ("mcp-protocol-version", "2025-11-25"),
    ];
    h.extend_from_slice(auth);
    http(addr, "POST", "/mcp", &h, &msg.to_string()).await
}

/// tools/call → the JSON-RPC `result` (or the whole message on a protocol error).
pub async fn mcp_call(
    addr: SocketAddr,
    sid: &str,
    auth: &[(&str, &str)],
    name: &str,
    args: Json,
) -> Json {
    let r = mcp_raw(
        addr,
        sid,
        auth,
        json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":name,"arguments":args}}),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let j = r.json();
    if j.get("error").is_some() {
        j
    } else {
        j["result"].clone()
    }
}

/// sha256 hex of a token (policy.json bearer entries).
pub fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
