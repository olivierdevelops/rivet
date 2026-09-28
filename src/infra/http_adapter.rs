//! HTTP client adapter (satisfies HttpClient) and the `http` effect kind.
//!
//! Rivet's own connection-level client over `hyper::client::conn` (ADR-0002):
//! it dials exactly the address the use case checked, speaks HTTP/1.1 or h2
//! (ALPN for https) or HTTP/3 over QUIC (`h3_client`, for `version 3` /
//! `version prefer [3, …]`), never follows redirects and never resolves
//! names itself.
//! One connection per attempt (no pooling yet).
//!
//! ```text
//!  x = http METHOD URL … end        with http METHOD URL as NAME … stream sse|jsonl|lines|bytes
//!        │                                     │
//!   parse options → HttpExchange  (files for tls/body read via the policed FileAccess)
//!        │
//!   transports.exchange_http (injected)  ──▶ HyperClient.resolve / send / wait
//!        │                                     │
//!   {status, headers, body}              HttpStreamHandle: for event in NAME
//! ```

use super::codec::{StdCodec, StreamDecoder};
use super::effect_args::{
    apply_tls, bad, budget_ms, duration, int, int_list, key, options, origin, read_file, text, word,
};
use super::execution_driver::{EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle};
use super::h3_client;
use super::net_tls::{client_config, connect_err, handshake_err, resolve, server_name};
use crate::domain::ir::EffectForm;
use crate::domain::ports::{FileAccess, PolicyEvaluator};
use crate::domain::transports::{
    ByteStream, Codec, CodecInput, CodecKind, HttpBody, HttpClient, HttpExchange, HttpOutcome,
    HttpReply, HttpResponse, HttpVersionPolicy, HttpWire, RetryPolicy, StreamMode, TlsMaterial,
    WireTarget, version_value,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncWrite};

/// Default cap on a buffered response body (overridden by `max_body N`).
pub const DEFAULT_MAX_BODY: u64 = 64 * 1024 * 1024;
/// Largest single stream item (SSE event, JSON line, text line).
pub const MAX_STREAM_ITEM: usize = 16 * 1024 * 1024;

/// The injected `transports.exchange_http` use case.
pub type ExchangeHttpFn = dyn for<'a> Fn(
        HttpExchange,
        &'a dyn PolicyEvaluator,
        &'a dyn HttpClient,
        &'a dyn Codec,
    ) -> BoxFuture<'a, RivetResult<HttpOutcome>>
    + Send
    + Sync;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

// vhco:infra http_adapter satisfies HttpClient
// vhco:net connect http+https -- every origin (and each resolved IP / redirect hop) authorized by the transports.exchange_http use case before dialing
// vhco:net connect udp -- HTTP/3 (QUIC v1, ALPN h3, 0-RTT off) only to the same checked host:port the https origin grant covers
pub struct HyperClient;

fn http_err(e: impl std::fmt::Display) -> RivetError {
    RivetError::new(
        ErrorKind::Connection,
        "connection.http",
        format!("HTTP exchange failed: {e}"),
    )
}

#[async_trait]
impl HttpClient for HyperClient {
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<IpAddr>> {
        resolve(host, port).await
    }

    async fn send(&self, wire: HttpWire) -> RivetResult<HttpReply> {
        if wire.version.wants_h3() {
            // HTTP/3 at the same checked address; never downgrades here (the
            // use case alone decides a pre-send fallback).
            let r = h3_client::send(&wire).await?;
            return buffer_reply(r.status, r.headers, "3".into(), r.body, &wire).await;
        }
        let url = url::Url::parse(&wire.url).map_err(|e| bad("validation.url", e.to_string()))?;
        let host = url.host_str().unwrap_or("").to_string();
        let https = url.scheme() == "https";
        let raw: Box<dyn Io> = match &wire.target {
            WireTarget::Tcp(addr) => {
                let s = tokio::net::TcpStream::connect(addr)
                    .await
                    .map_err(|e| connect_err(&format!("{host} ({addr})"), e))?;
                let _ = s.set_nodelay(true);
                Box::new(s)
            }
            #[cfg(unix)]
            WireTarget::Unix(path) => Box::new(
                tokio::net::UnixStream::connect(path)
                    .await
                    .map_err(|e| connect_err(path, e))?,
            ),
            #[cfg(not(unix))]
            WireTarget::Unix(_) => {
                return Err(RivetError::unsupported(
                    "unsupported.unix",
                    "Unix sockets are not available on this platform",
                ));
            }
        };
        let (io, h2): (Box<dyn Io>, bool) = if https {
            let alpn: &[&[u8]] = match wire.version {
                HttpVersionPolicy::Http1 => &[b"http/1.1"],
                HttpVersionPolicy::Http2 => &[b"h2"],
                _ => &[b"h2", b"http/1.1"],
            };
            let cfg = client_config(&wire.tls, alpn)?;
            let name = server_name(&host, &wire.tls)?;
            let tls = tokio_rustls::TlsConnector::from(cfg)
                .connect(name, raw)
                .await
                .map_err(handshake_err)?;
            let h2 = tls.get_ref().1.alpn_protocol() == Some(b"h2");
            if wire.version == HttpVersionPolicy::Http2 && !h2 {
                return Err(RivetError::new(
                    ErrorKind::Protocol,
                    "http.version_unavailable",
                    "the server did not negotiate HTTP/2",
                ));
            }
            (Box::new(tls), h2)
        } else {
            (raw, wire.version == HttpVersionPolicy::Http2)
        };

        let authority = match url.port() {
            Some(p) => format!("{host}:{p}"),
            None => host.clone(),
        };
        let path_q = match url.query() {
            Some(q) => format!("{}?{q}", url.path()),
            None => url.path().to_string(),
        };
        let uri = if h2 { url.to_string() } else { path_q };
        let mut req = hyper::Request::builder()
            .method(wire.method.as_str())
            .uri(uri);
        if !h2 {
            req = req.header("host", &authority);
        }
        let mut has_ua = false;
        for (k, v) in &wire.headers {
            if k.eq_ignore_ascii_case("host") && !h2 {
                continue;
            }
            has_ua |= k.eq_ignore_ascii_case("user-agent");
            req = req.header(k.as_str(), v.as_str());
        }
        if !has_ua {
            req = req.header("user-agent", concat!("rivet/", env!("CARGO_PKG_VERSION")));
        }
        let body = Full::new(Bytes::from(wire.body.clone().unwrap_or_default()));
        let req = req
            .body(body)
            .map_err(|e| bad("validation.http_request", e.to_string()))?;

        let io = TokioIo::new(io);
        let (resp, conn) = if h2 {
            let (mut sender, conn) =
                hyper::client::conn::http2::handshake(TokioExecutor::new(), io)
                    .await
                    .map_err(http_err)?;
            let task = tokio::spawn(async move {
                let _ = conn.await;
            });
            let resp = sender.send_request(req).await.map_err(http_err);
            (resp, task)
        } else {
            let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
                .await
                .map_err(http_err)?;
            let task = tokio::spawn(async move {
                let _ = conn.await;
            });
            let resp = sender.send_request(req).await.map_err(http_err);
            (resp, task)
        };
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                conn.abort();
                return Err(e);
            }
        };
        let status = resp.status().as_u16();
        let version = match resp.version() {
            hyper::Version::HTTP_2 => "2",
            hyper::Version::HTTP_10 => "1.0",
            _ => "1.1",
        }
        .to_string();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        let body = Box::new(HyperStream {
            body: resp.into_body(),
            conn: Some(conn),
        });
        buffer_reply(status, headers, version, body, &wire).await
    }

    async fn wait(&self, delay_ms: u64, jitter: bool) {
        let ms = if jitter && delay_ms > 1 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(0);
            delay_ms / 2 + nanos % (delay_ms / 2 + 1)
        } else {
            delay_ms
        };
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}

/// Hand a live body back for `stream …`, or buffer it up to `max_body`.
async fn buffer_reply(
    status: u16,
    headers: Vec<(String, String)>,
    version: String,
    mut body: Box<dyn ByteStream>,
    wire: &HttpWire,
) -> RivetResult<HttpReply> {
    let body = if wire.stream {
        HttpBody::Stream(body)
    } else {
        let mut buf = Vec::new();
        while let Some(chunk) = body.next_chunk().await? {
            if (buf.len() + chunk.len()) as u64 > wire.max_body {
                let _ = body.close().await;
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.http_body",
                    format!("response body exceeded {} bytes", wire.max_body),
                ));
            }
            buf.extend_from_slice(&chunk);
        }
        let _ = body.close().await;
        HttpBody::Complete(buf)
    };
    Ok(HttpReply {
        status,
        headers,
        version,
        body,
    })
}

/// A live hyper response body; the connection task is aborted on close.
struct HyperStream {
    body: Incoming,
    conn: Option<tokio::task::JoinHandle<()>>,
}

#[async_trait]
impl ByteStream for HyperStream {
    async fn next_chunk(&mut self) -> RivetResult<Option<Vec<u8>>> {
        loop {
            match self.body.frame().await {
                None => return Ok(None),
                Some(Err(e)) => {
                    return Err(RivetError::new(
                        ErrorKind::Connection,
                        "connection.http_body",
                        format!("response body failed: {e}"),
                    ));
                }
                Some(Ok(frame)) => {
                    if let Ok(data) = frame.into_data()
                        && !data.is_empty()
                    {
                        return Ok(Some(data.to_vec()));
                    }
                }
            }
        }
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        if let Some(c) = self.conn.take() {
            c.abort();
            let _ = c.await;
        }
        Ok(())
    }
}

impl Drop for HyperStream {
    fn drop(&mut self) {
        if let Some(c) = self.conn.take() {
            c.abort();
        }
    }
}

/// The `http` effect kind: `x = http …` and `with http … as NAME`.
pub struct HttpEffects {
    exchange: Arc<ExchangeHttpFn>,
    files: Arc<dyn FileAccess>,
    client: HyperClient,
    codec: StdCodec,
}

impl HttpEffects {
    pub fn new(exchange: Arc<ExchangeHttpFn>, files: Arc<dyn FileAccess>) -> HttpEffects {
        HttpEffects {
            exchange,
            files,
            client: HyperClient,
            codec: StdCodec,
        }
    }

    /// Turn the evaluated form into an HttpExchange (reading file options first).
    async fn parse(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        f: &EvaluatedForm,
        scoped: bool,
    ) -> RivetResult<(HttpExchange, u64)> {
        let method = word(f.head.first())
            .ok_or_else(|| bad("syntax.http", "expected `http METHOD URL`"))?
            .to_ascii_uppercase();
        let url = text(f.head.get(1))
            .ok_or_else(|| bad("syntax.http", "the URL after the method must be a string"))?;
        let mut x = HttpExchange {
            method,
            url,
            headers: Vec::new(),
            query: Vec::new(),
            body: None,
            version: HttpVersionPolicy::Auto,
            decode: None,
            accept: Vec::new(),
            retry: None,
            redirect_limit: 0,
            tls: TlsMaterial::default(),
            stream: None,
            unix_socket: None,
            max_body: DEFAULT_MAX_BODY,
            origin: origin(ctx),
        };
        let mut timeout = None;
        for (i, k, args) in options(f) {
            match k {
                "query" | "header" => {
                    let name = key(form, i, 0, args).ok_or_else(|| {
                        bad("validation.http_option", format!("`{k}` needs a name"))
                    })?;
                    let value = text(args.get(1)).ok_or_else(|| {
                        bad(
                            "validation.http_option",
                            format!("`{k} {name}` needs a value"),
                        )
                    })?;
                    if k == "query" {
                        x.query.push((name, value));
                    } else {
                        x.headers.push((name, value));
                    }
                }
                "body" => {
                    let kind = word(args.first()).unwrap_or("");
                    let value = match args.get(1) {
                        Some(EvalArg::Value(v)) => v.clone(),
                        _ => Value::Null,
                    };
                    x.body = Some(match kind {
                        "file" => {
                            let path = text(args.get(1)).ok_or_else(|| {
                                bad("validation.http_option", "`body file` needs a path")
                            })?;
                            CodecInput {
                                kind: CodecKind::Bytes,
                                bytes: None,
                                value: Some(Value::Bytes(read_file(self.files.as_ref(), &path).await?)),
                            }
                        }
                        "multipart" => {
                            return Err(RivetError::unsupported(
                                "unsupported.http_body",
                                "`body multipart` is not available in this build",
                            ));
                        }
                        other => CodecInput {
                            kind: CodecKind::parse(other).ok_or_else(|| {
                                bad(
                                    "validation.http_option",
                                    format!("unknown body kind `{other}` (json, text, form, bytes, file)"),
                                )
                            })?,
                            bytes: None,
                            value: Some(value),
                        },
                    });
                }
                "decode" => {
                    let w = word(args.first()).unwrap_or("");
                    x.decode = Some(CodecKind::parse(w).ok_or_else(|| {
                        bad("validation.http_option", format!("unknown decode `{w}`"))
                    })?);
                }
                "accept" => {
                    let list = int_list(args.get(1)).ok_or_else(|| {
                        bad("validation.http_option", "expected `accept status [CODES]`")
                    })?;
                    x.accept = list.into_iter().map(|c| c as u16).collect();
                }
                "timeout" => timeout = duration(args.first()),
                "redirect" => {
                    // redirect follow limit N
                    x.redirect_limit = int(args.get(2)).unwrap_or(0).max(0) as u32;
                    if word(args.first()) != Some("follow") {
                        return Err(bad(
                            "validation.http_option",
                            "expected `redirect follow limit N`",
                        ));
                    }
                }
                "retry" => x.retry = Some(parse_retry(args)?),
                "version" => x.version = parse_version(args)?,
                "auth" => {
                    return Err(RivetError::unsupported(
                        "unsupported.oauth",
                        "`auth PROFILE account …` needs the OAuth adapter, which is not available in this build",
                    ));
                }
                "unix" => x.unix_socket = text(args.first()),
                "tls" => apply_tls(self.files.as_ref(), args, &mut x.tls).await?,
                "max_body" => {
                    x.max_body = int(args.first()).unwrap_or(DEFAULT_MAX_BODY as i64).max(0) as u64
                }
                "stream" if scoped => {
                    let w = word(args.first()).unwrap_or("");
                    x.stream = Some(StreamMode::parse(w).ok_or_else(|| {
                        bad(
                            "validation.http_option",
                            format!("unknown stream mode `{w}` (sse, jsonl, lines, bytes)"),
                        )
                    })?);
                }
                other => {
                    return Err(bad(
                        "validation.http_option",
                        format!("option `{other}` is not valid for http"),
                    ));
                }
            }
        }
        if scoped && x.stream.is_none() {
            return Err(bad(
                "validation.http_stream",
                "`with http … as NAME` needs `stream sse|jsonl|lines|bytes`",
            ));
        }
        Ok((x, budget_ms(ctx, timeout)))
    }

    async fn exchange(
        &self,
        ctx: &EffectCtx,
        x: HttpExchange,
        ms: u64,
    ) -> RivetResult<HttpOutcome> {
        let fut = (self.exchange)(x, ctx.policy.as_ref(), &self.client, &self.codec);
        match tokio::time::timeout(Duration::from_millis(ms), fut).await {
            Ok(r) => r,
            Err(_) => Err(RivetError::new(
                ErrorKind::Timeout,
                "timeout.http",
                format!("the HTTP exchange exceeded {ms} ms"),
            )),
        }
    }
}

/// `version 1.1|2|3` or `version prefer [3, 2, 1.1]` (HTTP/3 first, if at all).
fn parse_version(args: &[EvalArg]) -> RivetResult<HttpVersionPolicy> {
    let bad_version = || {
        bad(
            "validation.http_version",
            "expected `version 1.1|2|3` or `version prefer [3, 2]` (HTTP/3 may only come first; no repeats)",
        )
    };
    let list: Vec<String> = if word(args.first()) == Some("prefer") {
        match args.get(1) {
            Some(EvalArg::Value(Value::List(items))) => {
                items.iter().map(|v| version_text(v.to_display())).collect()
            }
            _ => return Err(bad_version()),
        }
    } else {
        match text(args.first()) {
            Some(v) if args.len() == 1 => vec![version_text(v)],
            _ => return Err(bad_version()),
        }
    };
    HttpVersionPolicy::from_preference(&list).ok_or_else(bad_version)
}

/// `1.1` may print as `1.1`, `2.0` as `2`: normalise to the version name.
fn version_text(v: String) -> String {
    match v.as_str() {
        "2.0" => "2".into(),
        "3.0" => "3".into(),
        "1.0" | "1" => "1.1".into(),
        _ => v,
    }
}

fn parse_retry(args: &[EvalArg]) -> RivetResult<RetryPolicy> {
    let mut r = RetryPolicy {
        attempts: int(args.first()).unwrap_or(0).max(0) as u32,
        on_status: vec![429, 502, 503, 504],
        base_ms: 100,
        max_ms: 2_000,
        exponential: true,
        jitter: false,
    };
    let mut i = 1;
    while i < args.len() {
        match word(args.get(i)) {
            Some("on") => {
                if word(args.get(i + 1)) == Some("status") {
                    i += 1;
                }
                r.on_status = int_list(args.get(i + 1))
                    .ok_or_else(|| bad("validation.http_option", "expected `on status [CODES]`"))?
                    .into_iter()
                    .map(|c| c as u16)
                    .collect();
                i += 2;
            }
            Some("backoff") => {
                r.exponential = word(args.get(i + 1)) != Some("fixed");
                i += 2;
            }
            Some("base") => {
                r.base_ms = duration(args.get(i + 1)).unwrap_or(r.base_ms);
                i += 2;
            }
            Some("max") => {
                r.max_ms = duration(args.get(i + 1)).unwrap_or(r.max_ms);
                i += 2;
            }
            Some("jitter") => {
                r.jitter = super::effect_args::boolean(args.get(i + 1)).unwrap_or(true);
                i += 2;
            }
            _ => {
                return Err(bad(
                    "validation.http_option",
                    "expected `retry N on status [...] backoff exponential|fixed base \"D\" max \"D\" jitter B`",
                ));
            }
        }
    }
    Ok(r)
}

#[async_trait]
impl EffectAdapter for HttpEffects {
    async fn run(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Value> {
        let (x, ms) = self.parse(ctx, form, &args, false).await?;
        let out = self.exchange(ctx, x, ms).await?;
        Ok(out.response.to_value())
    }

    async fn open(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let (x, ms) = self.parse(ctx, form, &args, true).await?;
        let mode = x.stream.unwrap_or(StreamMode::Bytes);
        let deadline = Instant::now() + Duration::from_millis(ms);
        let out = self.exchange(ctx, x, ms).await?;
        let decode_json = match mode {
            StreamMode::Sse => !matches!(out.decode, Some(CodecKind::Text | CodecKind::Bytes)),
            _ => true,
        };
        Ok(Box::new(StreamHandle {
            stream: out.stream,
            decoder: StreamDecoder::new(mode, decode_json, MAX_STREAM_ITEM),
            response: Some(out.response),
            done: false,
            deadline,
            member: None,
        }))
    }
}

/// A scope-owned stream of items (HTTP body or child stdout), framed by a
/// StreamDecoder. Bounded: one chunk is pulled only when the body asks for
/// the next item, so a slow consumer applies backpressure to the source.
pub struct StreamHandle {
    pub stream: Option<Box<dyn ByteStream>>,
    pub decoder: StreamDecoder,
    pub response: Option<HttpResponse>,
    pub done: bool,
    pub deadline: Instant,
    /// For processes: the member that iterates (`stdout`).
    pub member: Option<&'static str>,
}

impl StreamHandle {
    pub async fn pull(&mut self) -> RivetResult<Option<Value>> {
        loop {
            if let Some(v) = self.decoder.pop()? {
                return Ok(Some(v));
            }
            if self.done {
                return Ok(None);
            }
            let Some(stream) = self.stream.as_mut() else {
                self.done = true;
                continue;
            };
            let left = self.deadline.saturating_duration_since(Instant::now());
            let chunk = match tokio::time::timeout(left, stream.next_chunk()).await {
                Ok(r) => r?,
                Err(_) => {
                    return Err(RivetError::new(
                        ErrorKind::Timeout,
                        "timeout.stream",
                        "the stream did not produce data before its deadline",
                    ));
                }
            };
            match chunk {
                Some(c) => self.decoder.push(&c),
                None => {
                    self.done = true;
                    stream.finish().await?;
                    if let Some(v) = self.decoder.finish()? {
                        return Ok(Some(v));
                    }
                    return Ok(None);
                }
            }
        }
    }
}

#[async_trait]
impl ResourceHandle for StreamHandle {
    async fn next(&mut self, _ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        self.pull().await
    }

    async fn next_of(&mut self, _ctx: &EffectCtx, member: &str) -> RivetResult<Option<Value>> {
        match self.member {
            Some(m) if m == member => self.pull().await,
            _ => Err(RivetError::unsupported(
                "unsupported.iterate",
                format!("`{member}` cannot be iterated on this resource"),
            )),
        }
    }

    async fn property(&mut self, _ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        match (&self.response, name) {
            (Some(r), "status") => Ok(Value::Int(r.status as i64)),
            (Some(r), "headers") => Ok(r.headers.clone()),
            (Some(r), "version") => Ok(version_value(&r.version)),
            _ => Err(RivetError::unsupported(
                "unsupported.property",
                format!("this stream has no `{name}` property"),
            )),
        }
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        match self.stream.take() {
            Some(s) => s.close().await,
            None => Ok(()),
        }
    }
}
