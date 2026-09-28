//! Local fixtures for the transport conformance suites (T-03, T-05). Every
//! server binds 127.0.0.1:0 inside the test; nothing leaves the host.

#![allow(dead_code)]

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use rivet::Runtime;
use rivet::internal::domain::contracts::DataEvent;
use rivet::internal::domain::ports::DataSink;
use rivet::internal::domain::{RivetResult, Value};
use rivet::internal::orchestrator::runtime::policy_from_json;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Build a runtime over in-memory source with a policy.json text.
pub fn runtime(src: &str, root: &str, policy: &str) -> Runtime {
    Runtime::builder()
        .source("app.rivet", src, root)
        .policy(policy_from_json(policy.as_bytes(), root).expect("policy"))
        .build()
        .expect("runtime")
}

/// Collects emitted items.
#[derive(Default)]
pub struct Collect {
    pub items: Mutex<Vec<Value>>,
    pub delay_ms: u64,
}

#[async_trait]
impl DataSink for Collect {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        self.items.lock().unwrap().push(event.data);
        Ok(())
    }
}

/// Observations made by the HTTP fixture.
#[derive(Default)]
pub struct HttpStats {
    pub requests: Mutex<Vec<String>>,
    pub flaky: AtomicUsize,
    pub streamed_bytes: AtomicU64,
    pub stream_closed: AtomicUsize,
}

struct Req {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

async fn read_req<S: tokio::io::AsyncRead + Unpin>(s: &mut BufReader<S>) -> Option<Req> {
    let mut line = String::new();
    s.read_line(&mut line).await.ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        s.read_line(&mut h).await.ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let len = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; len];
    s.read_exact(&mut body).await.ok()?;
    Some(Req {
        method,
        target,
        headers,
        body,
    })
}

fn reply(status: u16, ctype: &str, body: &str, extra: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} X\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn json(status: u16, body: &str) -> Vec<u8> {
    reply(status, "application/json", body, "")
}

/// Start the HTTP fixture. Routes mirror the reference fixture contract
/// (`/users/42`, `/users/404`, `/search`, `POST /users`, `/chat` SSE, …).
pub async fn http_server() -> (u16, Arc<HttpStats>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let stats = Arc::new(HttpStats::default());
    let st = Arc::clone(&stats);
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else {
                return;
            };
            let st = Arc::clone(&st);
            tokio::spawn(async move {
                let mut s = BufReader::new(sock);
                let Some(req) = read_req(&mut s).await else {
                    return;
                };
                st.requests
                    .lock()
                    .unwrap()
                    .push(format!("{} {}", req.method, req.target));
                let path = req.target.split('?').next().unwrap_or("").to_string();
                let query = req
                    .target
                    .split_once('?')
                    .map(|x| x.1.to_string())
                    .unwrap_or_default();
                let out: Vec<u8> = match (req.method.as_str(), path.as_str()) {
                    ("GET", "/users/42") => json(200, r#"{"id":42,"name":"Ada"}"#),
                    ("GET", "/users/404") => json(404, r#"{"error":"missing"}"#),
                    ("GET", "/users/500") => json(500, r#"{"error":"boom"}"#),
                    ("GET", "/users/7") => {
                        if st.flaky.fetch_add(1, Ordering::SeqCst) < 2 {
                            json(503, "{}")
                        } else {
                            json(200, r#"{"id":7,"name":"Retry"}"#)
                        }
                    }
                    ("POST", "/users") => {
                        let v: serde_json::Value =
                            serde_json::from_slice(&req.body).unwrap_or_default();
                        let name = v["name"].as_str().unwrap_or("").to_string();
                        json(200, &serde_json::json!({"id": 1, "name": name}).to_string())
                    }
                    ("POST", "/form") => {
                        let ct = req
                            .headers
                            .iter()
                            .find(|(k, _)| k == "content-type")
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default();
                        let body = String::from_utf8_lossy(&req.body).into_owned();
                        json(
                            200,
                            &serde_json::json!({"ct": ct, "body": body}).to_string(),
                        )
                    }
                    ("GET", "/search") => {
                        let q: Vec<(String, String)> =
                            url::form_urlencoded::parse(query.as_bytes())
                                .into_owned()
                                .collect();
                        let items: Vec<serde_json::Value> = q
                            .iter()
                            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
                            .collect();
                        json(200, &serde_json::Value::Array(items).to_string())
                    }
                    ("GET", "/redirect") => reply(302, "text/plain", "", "location: /users/42\r\n"),
                    ("GET", "/redirect-away") => {
                        reply(302, "text/plain", "", "location: http://127.0.0.2:9/x\r\n")
                    }
                    ("DELETE", "/users/42") => reply(204, "text/plain", "", ""),
                    ("GET", "/headers") => {
                        let auth = req
                            .headers
                            .iter()
                            .find(|(k, _)| k == "authorization")
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default();
                        json(200, &serde_json::json!({"auth": auth}).to_string())
                    }
                    ("POST", "/chat") => reply(
                        200,
                        "text/event-stream",
                        ": hi\n\nevent: delta\nid: 1\ndata: {\"delta\":\"Hel\"}\n\ndata: {\"delta\":\"lo\"}\n\ndata: {\"delta\":\"!\"}\n\n",
                        "",
                    ),
                    ("POST", "/api/generate") => reply(
                        200,
                        "application/x-ndjson",
                        "{\"response\":\"a\",\"done\":false}\n{\"response\":\"b\",\"done\":false}\n{\"response\":\"c\",\"done\":true}\n{\"response\":\"never\",\"done\":false}\n",
                        "",
                    ),
                    ("GET", "/logs") => reply(200, "text/plain", "one\ntwo\r\nthree", ""),
                    ("GET", "/firehose") => {
                        // Endless SSE: counts what the client accepted; a slow or gone
                        // consumer must stop it through TCP backpressure / close.
                        let mut sock = s.into_inner();
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n")
                            .await;
                        let chunk = format!("data: {{\"pad\":\"{}\"}}\n\n", "x".repeat(1000));
                        while let Ok(()) = sock.write_all(chunk.as_bytes()).await {
                            st.streamed_bytes
                                .fetch_add(chunk.len() as u64, Ordering::SeqCst);
                        }
                        st.stream_closed.fetch_add(1, Ordering::SeqCst);
                        return;
                    }
                    ("GET", "/broken") => {
                        // Chunked body cut off mid-stream: a disconnect, not a clean end.
                        let mut sock = s.into_inner();
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ntransfer-encoding: chunked\r\n\r\n4\r\nab\nc\r\n10\r\npartial")
                            .await;
                        let _ = sock.shutdown().await;
                        return;
                    }
                    (_, p) => json(200, &serde_json::json!({"path": p}).to_string()),
                };
                let mut sock = s.into_inner();
                let _ = sock.write_all(&out).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (port, stats)
}

/// Newline-framed TCP echo: replies `OK <line>`; counts closed connections.
pub async fn tcp_line_server() -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let closed = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&closed);
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else {
                return;
            };
            let c = Arc::clone(&c);
            tokio::spawn(async move {
                let (r, mut w) = sock.into_split();
                let mut r = BufReader::new(r);
                loop {
                    let mut line = String::new();
                    match r.read_line(&mut line).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let line = line.trim_end();
                            if line == "CUT" {
                                // Half a frame, then EOF.
                                let _ = w.write_all(b"partial").await;
                                let _ = w.shutdown().await;
                                continue;
                            }
                            let _ = w.write_all(format!("OK {line}\n").as_bytes()).await;
                        }
                    }
                }
                c.fetch_add(1, Ordering::SeqCst);
            });
        }
    });
    (port, closed)
}

/// Big-endian length32 echo: returns each frame reversed.
pub async fn tcp_len_server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = l.accept().await else {
                return;
            };
            tokio::spawn(async move {
                loop {
                    let mut head = [0u8; 4];
                    if sock.read_exact(&mut head).await.is_err() {
                        return;
                    }
                    let n = u32::from_be_bytes(head) as usize;
                    let mut body = vec![0u8; n];
                    if sock.read_exact(&mut body).await.is_err() {
                        return;
                    }
                    body.reverse();
                    let _ = sock.write_all(&(n as u32).to_be_bytes()).await;
                    let _ = sock.write_all(&body).await;
                }
            });
        }
    });
    port
}

/// Raw TCP server that reads until the client half-closes, then answers
/// with the byte count in two raw chunks and closes (FIN).
pub async fn tcp_fin_server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = l.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut all = Vec::new();
                let _ = sock.read_to_end(&mut all).await;
                let _ = sock
                    .write_all(format!("got {} ", all.len()).as_bytes())
                    .await;
                tokio::time::sleep(Duration::from_millis(20)).await;
                let _ = sock.write_all(b"bytes").await;
                let _ = sock.shutdown().await;
            });
        }
    });
    port
}

/// Newline-framed JSON over a Unix socket: echoes `{"echo": <input>}`.
/// Unix only: tokio has no Unix-domain listener on Windows.
#[cfg(unix)]
pub async fn unix_server(path: &std::path::Path) {
    let l = tokio::net::UnixListener::bind(path).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let (r, mut w) = sock.into_split();
                let mut r = BufReader::new(r);
                let mut line = String::new();
                while r.read_line(&mut line).await.unwrap_or(0) > 0 {
                    let v: serde_json::Value =
                        serde_json::from_str(line.trim()).unwrap_or_default();
                    let out = serde_json::json!({"echo": v}).to_string() + "\n";
                    let _ = w.write_all(out.as_bytes()).await;
                    line.clear();
                }
            });
        }
    });
}

/// WebSocket fixture: `{"type":"ping"}` → `{"type":"pong"}`; `{"type":"generate"}`
/// → three deltas and a done marker; binary frames are echoed.
pub async fn ws_server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let Ok(mut ws) = tokio_tungstenite::accept_async(sock).await else {
                    return;
                };
                use tokio_tungstenite::tungstenite::Message;
                while let Some(Ok(msg)) = ws.next().await {
                    match msg {
                        Message::Text(t) => {
                            let v: serde_json::Value =
                                serde_json::from_str(t.as_str()).unwrap_or_default();
                            if v["type"] == "ping" {
                                let _ = ws.send(Message::Text(r#"{"type":"pong"}"#.into())).await;
                            } else if v["type"] == "generate" {
                                for d in ["He", "ll", "o"] {
                                    let m =
                                        serde_json::json!({"type":"delta","delta":d}).to_string();
                                    let _ = ws.send(Message::Text(m.into())).await;
                                }
                                let _ = ws.send(Message::Text(r#"{"type":"done"}"#.into())).await;
                            }
                        }
                        Message::Binary(b) => {
                            let _ = ws.send(Message::Binary(b)).await;
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            });
        }
    });
    port
}

/// HTTPS fixture signed by a fresh test CA; returns (port, CA PEM).
pub async fn https_server() -> (u16, String) {
    use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair};
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key).unwrap();
    let leaf_key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec!["127.0.0.1".to_string()])
        .unwrap()
        .signed_by(&leaf_key, &ca)
        .unwrap();
    let certs = vec![rustls::pki_types::CertificateDer::from(leaf.der().to_vec())];
    let key = rustls::pki_types::PrivateKeyDer::try_from(leaf_key.serialize_der()).unwrap();
    let mut cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .unwrap();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(sock).await else {
                    return;
                };
                let mut s = BufReader::new(tls);
                if read_req(&mut s).await.is_none() {
                    return;
                }
                let mut tls = s.into_inner();
                let _ = tls.write_all(&json(200, r#"{"secure":true}"#)).await;
                let _ = tls.shutdown().await;
            });
        }
    });
    (port, ca.pem())
}
