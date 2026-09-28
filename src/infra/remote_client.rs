//! The CLI's `--endpoint URL` client: host bootstrap I/O that talks to an
//! existing `rivet serve` over its public surfaces (never a script effect, so
//! it is not brokered by policy.json — the server authorizes every call).
//!
//! ```text
//!  rivet --endpoint http://127.0.0.1:8080 [--token-file F] …
//!     request / auth / trace show ─▶ POST /v1/request {operation, data, deadline_ms?}
//!     request --stream            ─▶ POST /v1/request  Accept: text/event-stream (data records, one result record)
//!     request --input-jsonl -     ─▶ GET /v1/ws (rivet.v1): request {operation, data}, input*, finish_input | cancel
//!     list / describe / outputs / io ─▶ GET /v1/operations[/{id}[/outputs]] · GET /v1/io (envelopes)
//!  2xx ─▶ ResponseEnvelope status ok ─▶ Completion / JSON
//!  non-2xx or status error|cancelled ─▶ the same RivetError (registry exit code)
//!  a 0.1.0 server (no envelope `type`/`status`) ─▶ protocol.endpoint with a version hint from /v1/health
//! ```

use super::net_tls::{client_config, connect_err, handshake_err, server_name};
use crate::domain::contracts::{Completion, DataEvent};
use crate::domain::envelope::{RecordType, ResponseEnvelope};
use crate::domain::ports::{DataSink, RemoteCall, RemoteEndpoint};
use crate::domain::transports::TlsMaterial;
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use serde_json::{Value as Json, json};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

/// Largest response body / SSE event / WS frame the client accepts: the 32 MiB
/// per-request collection budget (a Completion may carry collected items).
const MAX_BODY: usize = 32 * 1024 * 1024;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// `http(s)://host:port[/base]` of a running `rivet serve`.
#[derive(Clone, Debug, PartialEq)]
struct Endpoint {
    tls: bool,
    host: String,
    port: u16,
    base: String,
}

// vhco:infra remote_client satisfies RemoteEndpoint
// vhco:net connect http+https+ws -- the `--endpoint URL` of a running `rivet serve` (REST, SSE and /v1/ws), host bootstrap only
// vhco:file read <--token-file PATH> -- the caller's bearer token for the server (read by setup_cli, never echoed)
pub struct RemoteClient {
    endpoint: Endpoint,
    /// `Authorization: Bearer …` from `--token-file`; absent → no header.
    token: Option<String>,
}

fn endpoint_err(msg: impl Into<String>) -> RivetError {
    RivetError::validation("validation.endpoint", msg)
}

fn io_err(msg: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Connection, "connection.endpoint", msg)
}

impl RemoteClient {
    /// Parse `--endpoint URL` (http or https, no userinfo, query or fragment).
    pub fn new(url: &str, token: Option<String>) -> RivetResult<RemoteClient> {
        let u = url::Url::parse(url).map_err(|e| endpoint_err(format!("--endpoint {url}: {e}")))?;
        let tls = match u.scheme() {
            "http" => false,
            "https" => true,
            other => {
                return Err(endpoint_err(format!(
                    "--endpoint must be http:// or https://, not {other}://"
                )));
            }
        };
        if !u.username().is_empty() || u.password().is_some() {
            return Err(endpoint_err(
                "--endpoint must not carry credentials; use --token-file PATH",
            ));
        }
        if u.query().is_some() || u.fragment().is_some() {
            return Err(endpoint_err("--endpoint takes no query or fragment"));
        }
        let host = match u.host() {
            Some(url::Host::Ipv6(a)) => format!("[{a}]"),
            Some(h) => h.to_string(),
            None => return Err(endpoint_err("--endpoint needs a host")),
        };
        Ok(RemoteClient {
            endpoint: Endpoint {
                tls,
                host,
                port: u.port_or_known_default().unwrap_or(80),
                base: u.path().trim_end_matches('/').to_string(),
            },
            token,
        })
    }

    fn authority(&self) -> String {
        format!("{}:{}", self.endpoint.host, self.endpoint.port)
    }

    async fn connect(&self) -> RivetResult<Box<dyn Io>> {
        let target = self.authority();
        let host = self.endpoint.host.trim_matches(['[', ']']);
        let tcp = tokio::net::TcpStream::connect((host, self.endpoint.port))
            .await
            .map_err(|e| connect_err(&target, e))?;
        let _ = tcp.set_nodelay(true);
        if !self.endpoint.tls {
            return Ok(Box::new(tcp));
        }
        let material = TlsMaterial::default();
        let cfg = client_config(&material, &[b"http/1.1"])?;
        let name = server_name(&self.endpoint.host, &material)?;
        let tls = tokio_rustls::TlsConnector::from(cfg)
            .connect(name, tcp)
            .await
            .map_err(handshake_err)?;
        Ok(Box::new(tls))
    }

    /// One HTTP/1.1 exchange; returns (status, content-type, body stream).
    async fn send(
        &self,
        method: &str,
        path: &str,
        accept: &str,
        body: Option<Json>,
    ) -> RivetResult<(u16, String, hyper::body::Incoming)> {
        let io = self.connect().await?;
        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(io))
            .await
            .map_err(|e| io_err(format!("HTTP handshake with {}: {e}", self.authority())))?;
        tokio::spawn(async move {
            let _ = conn.await;
        });
        let mut b = hyper::Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.endpoint.base))
            .header(hyper::header::HOST, self.authority())
            .header(hyper::header::ACCEPT, accept);
        if let Some(t) = &self.token {
            b = b.header(hyper::header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let bytes = match body {
            Some(j) => {
                b = b.header(hyper::header::CONTENT_TYPE, "application/json");
                Bytes::from(j.to_string())
            }
            None => Bytes::new(),
        };
        let req = b
            .body(Full::new(bytes))
            .map_err(|e| io_err(format!("cannot build the request: {e}")))?;
        let resp = sender
            .send_request(req)
            .await
            .map_err(|e| io_err(format!("{} did not answer: {e}", self.authority())))?;
        let status = resp.status().as_u16();
        let ctype = resp
            .headers()
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        Ok((status, ctype, resp.into_body()))
    }

    async fn read_all(mut body: hyper::body::Incoming) -> RivetResult<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|e| io_err(format!("response body: {e}")))?;
            if let Some(d) = frame.data_ref() {
                out.extend_from_slice(d);
                if out.len() > MAX_BODY {
                    return Err(RivetError::new(
                        ErrorKind::Limit,
                        "limit.response_body",
                        "the server response exceeds 32 MiB",
                    ));
                }
            }
        }
        Ok(out)
    }

    /// The whole JSON body of a 2xx answer, or the server's error. A 2xx body
    /// that is not a 0.2 ResponseEnvelope (a 0.1.0 server) is protocol.endpoint
    /// with a version hint.
    async fn json_call(&self, method: &str, path: &str, body: Option<Json>) -> RivetResult<Json> {
        let (status, _, stream) = self.send(method, path, "application/json", body).await?;
        let bytes = Self::read_all(stream).await?;
        if (200..300).contains(&status) {
            let j: Json = serde_json::from_slice(&bytes).map_err(|e| {
                RivetError::new(
                    ErrorKind::Protocol,
                    "protocol.endpoint",
                    format!("the server answered {status} with a non-JSON body: {e}"),
                )
            })?;
            if ResponseEnvelope::from_json(&j).is_none() {
                return Err(self.legacy_server().await);
            }
            return Ok(j);
        }
        Err(status_error(status, &bytes))
    }

    /// The error for a server that answered without a 0.2 envelope: GET
    /// /v1/health tells a 0.1.x server (bare `{status, catalog_version}`) apart.
    async fn legacy_server(&self) -> RivetError {
        let health = match self
            .send("GET", "/v1/health", "application/json", None)
            .await
        {
            Ok((_, _, b)) => Self::read_all(b).await.ok(),
            Err(_) => None,
        };
        let version = health
            .and_then(|b| serde_json::from_slice::<Json>(&b).ok())
            .map(|h| match ResponseEnvelope::from_json(&h) {
                Some(env) => env
                    .data
                    .as_ref()
                    .and_then(|d| d.get("version"))
                    .and_then(Json::as_str)
                    .unwrap_or("0.2.x")
                    .to_string(),
                None => "0.1.x".to_string(),
            })
            .unwrap_or_else(|| "unknown".to_string());
        RivetError::new(
            ErrorKind::Protocol,
            "protocol.endpoint",
            format!(
                "{} did not answer with a 0.2 response envelope (server version: {version})",
                self.authority()
            ),
        )
        .with_hint(
            "this client speaks the 0.2.0 envelopes; upgrade the server to rivet 0.2.x, or use a rivet 0.1.x client",
        )
        .with_details(Value::object([("server_version", Value::text(version))]))
    }

    /// The 0.2.0 input envelope for one call.
    fn request_body(call: &RemoteCall) -> Json {
        let mut j = json!({"operation": call.id, "data": call.params.to_json()});
        if let Some(d) = call.deadline_ms {
            j["deadline_ms"] = json!(d);
        }
        j
    }
}

/// A non-2xx answer: the error envelope decoded back into the same
/// RivetError (a 0.1.0 `{request_id, trace_id, error}` body decodes too); a
/// body that is not an envelope becomes the registry kind of its status.
fn status_error(status: u16, body: &[u8]) -> RivetError {
    if let Ok(j) = serde_json::from_slice::<Json>(body)
        && let Some(e) = j.get("error").filter(|e| e.is_object())
    {
        return envelope_error(&j, e);
    }
    let kind = match status {
        400 | 422 => ErrorKind::Validation,
        401 => ErrorKind::Auth,
        403 => ErrorKind::Permission,
        404 => ErrorKind::NotFound,
        409 => ErrorKind::Conflict,
        429 => ErrorKind::Limit,
        501 => ErrorKind::Unsupported,
        504 => ErrorKind::Timeout,
        _ => ErrorKind::Connection,
    };
    RivetError::new(
        kind,
        format!("{}.endpoint", kind.as_str()),
        format!("the server answered HTTP {status}"),
    )
}

/// `{request_id, trace_id, …, error:{…}, effects}` → RivetError with its IDs
/// (0.2 envelopes carry `effects` at the top level; 0.1.0 inside `error`).
fn envelope_error(envelope: &Json, error: &Json) -> RivetError {
    let mut e = RivetError::from_value(&Value::from_json(error));
    if let Some(fx) = envelope.get("effects").and_then(Json::as_str) {
        e.effects = crate::domain::EffectsStatus::parse(fx);
    }
    let id = |k: &str| {
        envelope
            .get(k)
            .and_then(Json::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    e.request_id = id("request_id");
    e.trace_id = id("trace_id");
    e
}

/// A terminal `type: result` record → the Completion (status ok) or the
/// RivetError (status error/cancelled).
fn completion(j: &Json) -> RivetResult<Completion> {
    match ResponseEnvelope::from_json(j).filter(|e| e.record_type == RecordType::Result) {
        Some(env) => env.into_outcome(),
        None => Err(RivetError::new(
            ErrorKind::Protocol,
            "protocol.endpoint",
            "the server answered without a result envelope",
        )),
    }
}

/// Incremental `text/event-stream` decoder: complete events only.
#[derive(Default)]
struct SseBuffer {
    buf: Vec<u8>,
}

impl SseBuffer {
    fn push(&mut self, chunk: &[u8]) -> Vec<(String, String)> {
        // Work on bytes: a chunk may end inside a multi-byte character, so only
        // a complete event (ending in a blank line) is decoded as UTF-8.
        self.buf
            .extend(chunk.iter().copied().filter(|b| *b != b'\r'));
        let mut events = Vec::new();
        loop {
            let Some(end) = self.buf.windows(2).position(|w| w == b"\n\n") else {
                break;
            };
            let raw = String::from_utf8_lossy(&self.buf[..end]).into_owned();
            self.buf.drain(..end + 2);
            let (mut event, mut data) = ("message".to_string(), String::new());
            for line in raw.lines() {
                if let Some(v) = line.strip_prefix("event:") {
                    event = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(v.strip_prefix(' ').unwrap_or(v));
                }
            }
            if !data.is_empty() {
                events.push((event, data));
            }
        }
        events
    }
}

#[async_trait]
impl RemoteEndpoint for RemoteClient {
    async fn request(&self, call: RemoteCall) -> RivetResult<Completion> {
        let j = self
            .json_call("POST", "/v1/request", Some(Self::request_body(&call)))
            .await?;
        completion(&j)
    }

    async fn request_stream(
        &self,
        call: RemoteCall,
        sink: Arc<dyn DataSink>,
    ) -> RivetResult<Completion> {
        let (status, ctype, mut body) = self
            .send(
                "POST",
                "/v1/request",
                "text/event-stream",
                Some(Self::request_body(&call)),
            )
            .await?;
        if !(200..300).contains(&status) {
            let bytes = Self::read_all(body).await?;
            return Err(status_error(status, &bytes));
        }
        if !ctype.starts_with("text/event-stream") {
            // A server may answer a unary operation with a plain envelope.
            let bytes = Self::read_all(body).await?;
            let j: Json = serde_json::from_slice(&bytes).map_err(|e| {
                RivetError::new(ErrorKind::Protocol, "protocol.endpoint", e.to_string())
            })?;
            if ResponseEnvelope::from_json(&j).is_none() {
                return Err(self.legacy_server().await);
            }
            return completion(&j);
        }
        let mut sse = SseBuffer::default();
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|e| io_err(format!("event stream: {e}")))?;
            let Some(chunk) = frame.data_ref() else {
                continue;
            };
            for (_event, data) in sse.push(chunk) {
                let j: Json = serde_json::from_str(&data).map_err(|e| {
                    RivetError::new(
                        ErrorKind::Protocol,
                        "protocol.endpoint",
                        format!("bad SSE event: {e}"),
                    )
                })?;
                // The record's `type` decides (the SSE event name mirrors it).
                match j.get("type").and_then(Json::as_str) {
                    Some("data") => {
                        if let Some(ev) = DataEvent::from_json(&j) {
                            sink.send(ev).await?;
                        }
                    }
                    Some("result") => return completion(&j),
                    _ if j.get("error").is_some_and(|e| e.is_object()) => {
                        // a 0.1.0 `event: error`
                        return Err(envelope_error(&j, &j["error"]));
                    }
                    _ => {}
                }
            }
            if sse.buf.len() > MAX_BODY {
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.response_body",
                    "an SSE event exceeds 32 MiB",
                ));
            }
        }
        Err(io_err(
            "the event stream ended without a terminal result or error",
        ))
    }

    async fn get(&self, path: &str) -> RivetResult<Json> {
        self.json_call("GET", path, None).await
    }

    async fn duplex(
        &self,
        call: RemoteCall,
        mut input: tokio::sync::mpsc::Receiver<Value>,
        mut cancel: tokio::sync::oneshot::Receiver<()>,
        sink: Arc<dyn DataSink>,
    ) -> RivetResult<Completion> {
        let io = self.connect().await?;
        let scheme = if self.endpoint.tls { "wss" } else { "ws" };
        let mut req = format!(
            "{scheme}://{}{}/v1/ws",
            self.authority(),
            self.endpoint.base
        )
        .into_client_request()
        .map_err(|e| endpoint_err(format!("WebSocket URL: {e}")))?;
        req.headers_mut().insert(
            "sec-websocket-protocol",
            http::HeaderValue::from_static(crate::domain::serve::WS_SUBPROTOCOL),
        );
        if let Some(t) = &self.token {
            let v = http::HeaderValue::from_str(&format!("Bearer {t}"))
                .map_err(|_| endpoint_err("the --token-file token is not a valid header value"))?;
            req.headers_mut().insert(http::header::AUTHORIZATION, v);
        }
        let (ws, _) = tokio_tungstenite::client_async(req, io)
            .await
            .map_err(|e| match e {
                tokio_tungstenite::tungstenite::Error::Http(resp) => {
                    let body = resp.body().clone().unwrap_or_default();
                    status_error(resp.status().as_u16(), &body)
                }
                other => io_err(format!("WebSocket upgrade: {other}")),
            })?;
        let (mut tx, mut rx) = ws.split();
        const REF: &str = "cli";
        let send = |j: Json| Message::Text(j.to_string().into());
        let ws_err = |e: tokio_tungstenite::tungstenite::Error| io_err(format!("WebSocket: {e}"));
        tx.send(send(
            json!({"type": "request", "ref": REF, "operation": call.id, "data": call.params.to_json()}),
        ))
        .await
        .map_err(ws_err)?;
        let mut seq = 0u64;
        let mut input_open = true;
        let mut cancel_open = true;
        loop {
            tokio::select! {
                item = input.recv(), if input_open => match item {
                    Some(v) => {
                        seq += 1;
                        tx.send(send(json!({"type": "input", "ref": REF, "seq": seq, "data": v.to_json()})))
                            .await
                            .map_err(ws_err)?;
                    }
                    None => {
                        input_open = false;
                        tx.send(send(json!({"type": "finish_input", "ref": REF})))
                            .await
                            .map_err(ws_err)?;
                    }
                },
                fired = &mut cancel, if cancel_open => {
                    cancel_open = false;
                    if fired.is_ok() {
                        input_open = false;
                        tx.send(send(json!({"type": "cancel", "ref": REF})))
                            .await
                            .map_err(ws_err)?;
                    }
                },
                msg = rx.next() => {
                    let text = match msg {
                        Some(Ok(Message::Text(t))) => t.to_string(),
                        Some(Ok(Message::Close(_))) | None => {
                            return Err(io_err("the server closed the WebSocket before a terminal frame"));
                        }
                        Some(Ok(_)) => continue,
                        Some(Err(e)) => return Err(ws_err(e)),
                    };
                    let j: Json = serde_json::from_str(&text).map_err(|e| {
                        RivetError::new(ErrorKind::Protocol, "protocol.endpoint", format!("bad frame: {e}"))
                    })?;
                    if j.get("ref").and_then(Json::as_str) != Some(REF) {
                        continue;
                    }
                    match j.get("type").and_then(Json::as_str) {
                        Some("data") => {
                            if let Some(ev) = DataEvent::from_json(&j) {
                                sink.send(ev).await?;
                            }
                        }
                        Some("result") => {
                            // The ref's one terminal record: status ok, error or cancelled.
                            let c = completion(&j);
                            let _ = tx.send(Message::Close(None)).await;
                            return c;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_parsing_and_sse_framing() {
        let c = RemoteClient::new("http://127.0.0.1:8080", None).unwrap();
        assert_eq!(c.authority(), "127.0.0.1:8080");
        assert!(RemoteClient::new("ftp://x", None).is_err());
        assert!(RemoteClient::new("http://u:p@x", None).is_err());
        let mut s = SseBuffer::default();
        assert!(s.push(b"id: 1\nevent: data\ndata: {\"a\"").is_empty());
        let ev = s.push(b":1}\n\nevent: result\ndata: {}\n\n");
        assert_eq!(
            ev,
            vec![
                ("data".to_string(), "{\"a\":1}".to_string()),
                ("result".to_string(), "{}".to_string())
            ]
        );
        let e = status_error(
            403,
            br#"{"request_id":"r1","trace_id":"t1","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"no","retryable":false},"effects":"partial","data_count":0}"#,
        );
        assert_eq!((e.exit_code(), e.request_id.as_deref()), (3, Some("r1")));
        assert_eq!(e.effects, crate::domain::EffectsStatus::Partial);
        // a 0.1.0 error body still decodes
        let old = status_error(
            404,
            br#"{"request_id":"r2","trace_id":"t2","error":{"kind":"not_found","code":"not_found.operation","message":"no","retryable":false,"effects":"none"}}"#,
        );
        assert_eq!(old.code, "not_found.operation");
        assert_eq!(status_error(500, b"oops").kind, ErrorKind::Connection);
        let ok = completion(&json!({"request_id":"r","trace_id":"t","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0})).unwrap();
        assert_eq!(ok.result, Value::Int(5));
        assert_eq!(
            completion(&json!({"request_id":"r","result":5}))
                .unwrap_err()
                .code,
            "protocol.endpoint"
        );
    }
}
