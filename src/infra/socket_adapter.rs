//! Socket adapter (satisfies SocketStream) and the `tcp`, `unix` and
//! `websocket` effect kinds. Every connection is scope-owned:
//!
//! ```text
//!  with tcp "h:p" as conn        with unix PATH as conn        with websocket URL as socket
//!        │ parse options (framing, max_frame, timeout; tls/reconnect → Stage C)
//!        ▼
//!  transports.exchange_socket (injected): authorize → resolve → checked address
//!        ▼
//!  FramedConn (newline | length32 | delimiter | raw)   WsConn (message boundaries)
//!        ▼
//!  SocketHandle: conn.send KIND V · conn.receive KIND [timeout "D"] · for x in conn
//!               conn.finish_send (half-close) · scope exit → close within grace
//! ```

use super::codec::{StdCodec, decode_json, encode_frame, take_frame};
use super::effect_args::{apply_tls, bad, budget_ms, duration, int, options, origin, text, word};
use super::execution_driver::{EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle};
use super::net_tls::{client_config, connect_err, handshake_err, resolve, server_name};
use crate::domain::ir::{EffectForm, EffectKind};
use crate::domain::ports::PolicyEvaluator;
use crate::domain::transports::{
    Codec, CodecInput, CodecKind, Frame, Framing, SocketConnection, SocketPlan, SocketScheme,
    SocketStream, TlsMaterial,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use futures_util::{SinkExt, StreamExt};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

/// Default `max_frame`: the proposal frame limit, 8 MiB (overridden by `max_frame N`).
pub const DEFAULT_MAX_FRAME: u64 = 8 * 1024 * 1024;

/// The injected `transports.exchange_socket` use case.
pub type ExchangeSocketFn = dyn for<'a> Fn(
        SocketPlan,
        &'a dyn PolicyEvaluator,
        &'a dyn SocketStream,
    ) -> BoxFuture<'a, RivetResult<Box<dyn SocketConnection>>>
    + Send
    + Sync;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

// vhco:infra socket_adapter satisfies SocketStream
// vhco:net connect tcp+unix+ws+wss -- endpoints authorized by transports.exchange_socket (allow_network / allow_unix) before dialing a checked address
pub struct Dialer;

#[async_trait]
impl SocketStream for Dialer {
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<IpAddr>> {
        resolve(host, port).await
    }

    async fn connect(
        &self,
        plan: &SocketPlan,
        addr: Option<SocketAddr>,
    ) -> RivetResult<Box<dyn SocketConnection>> {
        let raw: Box<dyn Io> = match (plan.scheme, addr) {
            (SocketScheme::Unix, _) => unix_connect(&plan.endpoint).await?,
            (_, Some(addr)) => {
                let s = tokio::net::TcpStream::connect(addr)
                    .await
                    .map_err(|e| connect_err(&plan.endpoint, e))?;
                let _ = s.set_nodelay(true);
                Box::new(s)
            }
            (_, None) => {
                return Err(RivetError::internal(
                    "socket dial without a checked address",
                ));
            }
        };
        match plan.scheme {
            SocketScheme::Tcp | SocketScheme::Unix => Ok(Box::new(FramedConn {
                io: raw,
                buf: Vec::new(),
                framing: plan.framing.clone(),
                max: plan.max_frame,
                eof: false,
            })),
            SocketScheme::Ws | SocketScheme::Wss => {
                let io: Box<dyn Io> = if plan.scheme == SocketScheme::Wss {
                    let host = plan.host.clone().unwrap_or_default();
                    let cfg = client_config(&plan.tls, &[b"http/1.1"])?;
                    Box::new(
                        tokio_rustls::TlsConnector::from(cfg)
                            .connect(server_name(&host, &plan.tls)?, raw)
                            .await
                            .map_err(handshake_err)?,
                    )
                } else {
                    raw
                };
                let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
                    .max_message_size(Some(plan.max_frame as usize))
                    .max_frame_size(Some(plan.max_frame as usize));
                let (ws, _resp) = tokio_tungstenite::client_async_with_config(
                    plan.endpoint.as_str(),
                    io,
                    Some(config),
                )
                .await
                .map_err(|e| {
                    RivetError::new(
                        ErrorKind::Protocol,
                        "protocol.websocket_handshake",
                        format!("WebSocket upgrade failed: {e}"),
                    )
                })?;
                Ok(Box::new(WsConn { ws, closed: false }))
            }
        }
    }
}

#[cfg(unix)]
async fn unix_connect(path: &str) -> RivetResult<Box<dyn Io>> {
    Ok(Box::new(
        tokio::net::UnixStream::connect(path)
            .await
            .map_err(|e| connect_err(path, e))?,
    ))
}

#[cfg(not(unix))]
async fn unix_connect(_path: &str) -> RivetResult<Box<dyn Io>> {
    Err(RivetError::unsupported(
        "unsupported.unix",
        "Unix sockets are not available on this platform",
    ))
}

fn eof_err() -> RivetError {
    RivetError::new(
        ErrorKind::Protocol,
        "protocol.unexpected_eof",
        "the peer closed the connection before a complete frame",
    )
}

fn io_err(e: std::io::Error) -> RivetError {
    RivetError::new(
        ErrorKind::Connection,
        "connection.io",
        format!("socket I/O failed: {e}"),
    )
}

/// TCP / Unix stream with explicit framing.
struct FramedConn {
    io: Box<dyn Io>,
    buf: Vec<u8>,
    framing: Framing,
    max: u64,
    eof: bool,
}

#[async_trait]
impl SocketConnection for FramedConn {
    async fn send(&mut self, frame: Frame) -> RivetResult<()> {
        let wire = encode_frame(&self.framing, frame.bytes, self.max)?;
        self.io.write_all(&wire).await.map_err(io_err)?;
        self.io.flush().await.map_err(io_err)
    }

    async fn receive(&mut self) -> RivetResult<Option<Frame>> {
        loop {
            if let Some(bytes) = take_frame(&mut self.buf, &self.framing, self.max)? {
                let text = self.framing == Framing::Newline && std::str::from_utf8(&bytes).is_ok();
                return Ok(Some(Frame { bytes, text }));
            }
            if self.eof {
                return if self.buf.is_empty() {
                    Ok(None)
                } else {
                    Err(eof_err())
                };
            }
            let mut tmp = vec![0u8; 64 * 1024];
            let n = self.io.read(&mut tmp).await.map_err(io_err)?;
            if n == 0 {
                self.eof = true;
            } else {
                self.buf.extend_from_slice(&tmp[..n]);
            }
        }
    }

    async fn finish_send(&mut self) -> RivetResult<()> {
        self.io.shutdown().await.map_err(io_err)
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        let _ = self.io.shutdown().await;
        Ok(())
    }
}

/// WebSocket client connection; message boundaries are preserved.
struct WsConn {
    ws: tokio_tungstenite::WebSocketStream<Box<dyn Io>>,
    closed: bool,
}

fn ws_err(e: tokio_tungstenite::tungstenite::Error) -> RivetError {
    RivetError::new(
        ErrorKind::Connection,
        "connection.websocket",
        format!("WebSocket failed: {e}"),
    )
}

#[async_trait]
impl SocketConnection for WsConn {
    async fn send(&mut self, frame: Frame) -> RivetResult<()> {
        let msg = if frame.text {
            Message::Text(
                String::from_utf8(frame.bytes)
                    .map_err(|_| bad("validation.codec", "text frames must be UTF-8"))?
                    .into(),
            )
        } else {
            Message::Binary(frame.bytes.into())
        };
        self.ws.send(msg).await.map_err(ws_err)
    }

    async fn receive(&mut self) -> RivetResult<Option<Frame>> {
        loop {
            match self.ws.next().await {
                None => return Ok(None),
                Some(Err(e)) => return Err(ws_err(e)),
                Some(Ok(Message::Text(t))) => {
                    return Ok(Some(Frame {
                        bytes: t.as_bytes().to_vec(),
                        text: true,
                    }));
                }
                Some(Ok(Message::Binary(b))) => {
                    return Ok(Some(Frame {
                        bytes: b.to_vec(),
                        text: false,
                    }));
                }
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(_)) => continue,
            }
        }
    }

    async fn finish_send(&mut self) -> RivetResult<()> {
        if !self.closed {
            self.closed = true;
            self.ws.close(None).await.map_err(ws_err)?;
        }
        Ok(())
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        if !self.closed {
            let _ = self.ws.close(None).await;
        }
        // Drain until the peer's close frame (bounded by the scope's cleanup grace).
        let _ = tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(Ok(_)) = self.ws.next().await {}
        })
        .await;
        Ok(())
    }
}

/// The `tcp`, `unix` and `websocket` effect kinds (scoped only).
pub struct SocketEffects {
    exchange: Arc<ExchangeSocketFn>,
    files: Arc<dyn crate::domain::ports::FileAccess>,
    dialer: Dialer,
}

impl SocketEffects {
    pub fn new(
        exchange: Arc<ExchangeSocketFn>,
        files: Arc<dyn crate::domain::ports::FileAccess>,
    ) -> SocketEffects {
        SocketEffects {
            exchange,
            files,
            dialer: Dialer,
        }
    }

    async fn plan(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        f: &EvaluatedForm,
    ) -> RivetResult<SocketPlan> {
        let endpoint = text(f.head.first())
            .ok_or_else(|| bad("syntax.with", "the endpoint must be a string"))?;
        let (scheme, host, port) = match form.kind {
            EffectKind::Tcp => {
                let (h, p) = endpoint.rsplit_once(':').ok_or_else(|| {
                    bad(
                        "validation.endpoint",
                        format!("`{endpoint}` must be HOST:PORT"),
                    )
                })?;
                let port = p.parse::<u16>().map_err(|_| {
                    bad(
                        "validation.endpoint",
                        format!("`{endpoint}` has an invalid port"),
                    )
                })?;
                (
                    SocketScheme::Tcp,
                    Some(h.trim_start_matches('[').trim_end_matches(']').to_string()),
                    Some(port),
                )
            }
            EffectKind::Unix => (SocketScheme::Unix, None, None),
            _ => {
                let u = url::Url::parse(&endpoint)
                    .map_err(|e| bad("validation.url", format!("invalid URL `{endpoint}`: {e}")))?;
                let scheme = match u.scheme() {
                    "ws" => SocketScheme::Ws,
                    "wss" => SocketScheme::Wss,
                    other => {
                        return Err(bad(
                            "validation.url",
                            format!("websocket needs ws:// or wss://, got `{other}`"),
                        ));
                    }
                };
                (
                    scheme,
                    u.host_str()
                        .map(|h| h.trim_start_matches('[').trim_end_matches(']').to_string()),
                    u.port_or_known_default(),
                )
            }
        };
        let mut plan = SocketPlan {
            scheme,
            endpoint,
            host,
            port,
            framing: Framing::Raw,
            max_frame: DEFAULT_MAX_FRAME,
            tls_requested: false,
            tls: TlsMaterial::default(),
            timeout_ms: None,
            reconnect: false,
            origin: origin(ctx),
        };
        let ws = matches!(scheme, SocketScheme::Ws | SocketScheme::Wss);
        for (_, k, args) in options(f) {
            match k {
                "framing" if !ws => parse_framing(args, &mut plan)?,
                "max_frame" => plan.max_frame = int(args.first()).unwrap_or(0).max(1) as u64,
                "timeout" => plan.timeout_ms = duration(args.first()),
                "tls" if ws => apply_tls(self.files.as_ref(), args, &mut plan.tls).await?,
                // Stage C on raw sockets: flagged, refused by the use case before any file read or dial.
                "tls" => plan.tls_requested = true,
                "reconnect" => plan.reconnect = true,
                "resume" => {}
                other => {
                    return Err(bad(
                        "validation.socket_option",
                        format!("option `{other}` is not valid for {}", form.kind.as_str()),
                    ));
                }
            }
        }
        Ok(plan)
    }
}

fn parse_framing(args: &[EvalArg], plan: &mut SocketPlan) -> RivetResult<()> {
    let mut i = 0;
    while i < args.len() {
        match word(args.get(i)) {
            Some("newline") => plan.framing = Framing::Newline,
            Some("raw") => plan.framing = Framing::Raw,
            Some("length32") => plan.framing = Framing::Length32 { big_endian: true },
            Some("endian") => {
                let big = match word(args.get(i + 1)) {
                    Some("big") => true,
                    Some("little") => false,
                    _ => return Err(bad("validation.framing", "expected `endian big|little`")),
                };
                plan.framing = Framing::Length32 { big_endian: big };
                i += 1;
            }
            Some("delimiter") => {
                let d = text(args.get(i + 1))
                    .ok_or_else(|| bad("validation.framing", "expected `delimiter \"S\"`"))?;
                plan.framing = Framing::Delimiter(d.into_bytes());
                i += 1;
            }
            Some("max_frame") => {
                plan.max_frame = int(args.get(i + 1)).unwrap_or(0).max(1) as u64;
                i += 1;
            }
            _ => {
                return Err(bad(
                    "validation.framing",
                    "expected `framing newline|length32 endian big|little|delimiter \"S\"|raw [max_frame N]`",
                ));
            }
        }
        i += 1;
    }
    Ok(())
}

#[async_trait]
impl EffectAdapter for SocketEffects {
    async fn open(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let plan = self.plan(ctx, form, &args).await?;
        let ms = budget_ms(ctx, plan.timeout_ms);
        let deadline = Instant::now() + Duration::from_millis(ms);
        let ws = matches!(plan.scheme, SocketScheme::Ws | SocketScheme::Wss);
        let fut = (self.exchange)(plan, ctx.policy.as_ref(), &self.dialer);
        let conn = match tokio::time::timeout(Duration::from_millis(ms), fut).await {
            Ok(r) => r?,
            Err(_) => {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.connect",
                    format!("connecting exceeded {ms} ms"),
                ));
            }
        };
        Ok(Box::new(SocketHandle {
            conn: Some(conn),
            deadline,
            ws,
        }))
    }
}

/// `with tcp|unix|websocket … as NAME`: the scope-owned connection.
struct SocketHandle {
    conn: Option<Box<dyn SocketConnection>>,
    deadline: Instant,
    ws: bool,
}

fn timeout_err(what: &str) -> RivetError {
    RivetError::new(
        ErrorKind::Timeout,
        "timeout.receive",
        format!("{what} did not complete before its deadline"),
    )
}

impl SocketHandle {
    fn conn(&mut self) -> RivetResult<&mut Box<dyn SocketConnection>> {
        self.conn.as_mut().ok_or_else(|| {
            RivetError::new(
                ErrorKind::Cleanup,
                "cleanup.closed",
                "the connection is closed",
            )
        })
    }

    fn left(&self, per_call: Option<u64>) -> Duration {
        let scope = self.deadline.saturating_duration_since(Instant::now());
        match per_call {
            Some(ms) => scope.min(Duration::from_millis(ms)),
            None => scope,
        }
    }

    async fn frame(&mut self, per_call: Option<u64>) -> RivetResult<Option<Frame>> {
        let left = self.left(per_call);
        let conn = self.conn()?;
        match tokio::time::timeout(left, conn.receive()).await {
            Ok(r) => r,
            Err(_) => Err(timeout_err("receive")),
        }
    }
}

fn frame_value(f: Frame) -> Value {
    if f.text {
        match String::from_utf8(f.bytes) {
            Ok(s) => Value::Text(s),
            Err(e) => Value::Bytes(e.into_bytes()),
        }
    } else {
        Value::Bytes(f.bytes)
    }
}

#[async_trait]
impl ResourceHandle for SocketHandle {
    async fn call(
        &mut self,
        _ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
    ) -> RivetResult<Value> {
        match method {
            "send" => {
                let kind = word(args.first())
                    .and_then(CodecKind::parse)
                    .ok_or_else(|| {
                        bad(
                            "validation.socket_send",
                            "expected `send json|text|bytes VALUE`",
                        )
                    })?;
                let value = match args.get(1) {
                    Some(EvalArg::Value(v)) => v.clone(),
                    Some(EvalArg::Word(w)) => Value::Text(w.clone()),
                    None => return Err(bad("validation.socket_send", "`send` needs a value")),
                };
                let bytes = StdCodec.encode(&CodecInput {
                    kind,
                    bytes: None,
                    value: Some(value),
                })?;
                let frame = Frame {
                    bytes,
                    text: kind != CodecKind::Bytes,
                };
                let left = self.left(None);
                let conn = self.conn()?;
                match tokio::time::timeout(left, conn.send(frame)).await {
                    Ok(r) => r.map(|_| Value::Null),
                    Err(_) => Err(timeout_err("send")),
                }
            }
            "receive" => {
                let mut kind = CodecKind::Text;
                let mut per_call = None;
                let mut i = 0;
                while i < args.len() {
                    match word(args.get(i)) {
                        Some("timeout") => {
                            per_call = duration(args.get(i + 1));
                            i += 1;
                        }
                        Some(w) if CodecKind::parse(w).is_some() => {
                            kind = CodecKind::parse(w).unwrap_or(kind)
                        }
                        _ => {
                            return Err(bad(
                                "validation.socket_receive",
                                "expected `receive json|text|bytes [timeout \"D\"]`",
                            ));
                        }
                    }
                    i += 1;
                }
                let frame = self.frame(per_call).await?.ok_or_else(eof_err)?;
                match kind {
                    CodecKind::Json => decode_json(&frame.bytes),
                    CodecKind::Bytes => Ok(Value::Bytes(frame.bytes)),
                    _ => String::from_utf8(frame.bytes)
                        .map(Value::Text)
                        .map_err(|_| {
                            RivetError::new(
                                ErrorKind::Parse,
                                "parse.utf8",
                                "received frame is not UTF-8",
                            )
                        }),
                }
            }
            "finish_send" => {
                self.conn()?.finish_send().await?;
                Ok(Value::Null)
            }
            "close" if self.ws => {
                if let Some(c) = self.conn.take() {
                    c.close().await?;
                }
                Ok(Value::Null)
            }
            other => Err(RivetError::unsupported(
                "unsupported.method",
                format!("sockets have no `{other}` method (send, receive, finish_send)"),
            )),
        }
    }

    async fn next(&mut self, _ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        if self.conn.is_none() {
            return Ok(None);
        }
        Ok(self.frame(None).await?.map(frame_value))
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        match self.conn.take() {
            Some(c) => c.close().await,
            None => Ok(()),
        }
    }
}
