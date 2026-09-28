//! Scoped native QUIC v1 connections (`with quic …`, PROP-2026-0001 Increment 11).
//!
//! ```text
//!  with quic "quic://H:P" as connection ─▶ QuicAdapter::open ─▶ exchange_quic(plan{Connect})
//!      alpn / max_streams / datagrams / tls …           (authorize files + origin + resolved IP,
//!                                                         then quinn handshake: 1 ALPN, 0-RTT off)
//!      with connection.open bidi as stream ─▶ open_child ─▶ exchange_quic(plan{OpenStream})
//!          stream.send json V              ─▶ QuicStream::call ─▶ plan{Send}  (framed)
//!          stream.receive json timeout "5s"─▶ plan{Receive}                  (one frame)
//!      end                                 ─▶ plan{CloseStream}  FIN + stop reading
//!  end                                     ─▶ plan{Close}        connection closed, bounded idle wait
//! ```
//!
//! Every step runs the `quic.exchange_quic` use case (injected by the
//! orchestrator). The client never rebinds, so the socket stays on the
//! authorized path; migration and early data are refused by the use case.

use crate::domain::errors::EffectsStatus;
use crate::domain::files::{Codec, FileOperation, FileVerb};
use crate::domain::ir::EffectForm;
use crate::domain::ports::{FileAccess, PolicyEvaluator, QuicDriver};
use crate::domain::transport::{
    DEFAULT_MAX_STREAMS, EffectContext, Framing, FramingKind, QuicPlan, QuicResult, QuicStep,
    QuicTls, StreamDir,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use crate::infra::execution_driver::{
    EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle,
};
use crate::infra::wire_codec::{
    bool_value, codec_at, decode, duration_value, encode, text_value, timeout_arg, uint_value,
    value_at,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use quinn::crypto::rustls::{HandshakeData, QuicClientConfig};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The `quic.exchange_quic` use case, injected by the orchestrator.
pub type QuicExchangeFn = dyn Fn(
        QuicPlan,
        Arc<dyn PolicyEvaluator>,
        Arc<dyn QuicDriver>,
    ) -> BoxFuture<'static, RivetResult<QuicResult>>
    + Send
    + Sync;

/// `EffectAdapter` for kind `quic`.
pub struct QuicAdapter {
    exchange: Arc<QuicExchangeFn>,
    /// Confined reads of `tls ca_file|cert_file|key_file` (authorized by the use case).
    files: Arc<dyn FileAccess>,
}

impl QuicAdapter {
    pub fn new(exchange: Arc<QuicExchangeFn>, files: Arc<dyn FileAccess>) -> QuicAdapter {
        QuicAdapter { exchange, files }
    }
}

fn context(ctx: &EffectCtx) -> EffectContext {
    EffectContext {
        operation_id: ctx.operation_id.clone(),
        span: Some(ctx.span.clone()),
        deadline: ctx.deadline,
    }
}

fn option_error(what: impl Into<String>) -> RivetError {
    RivetError::validation("validation.quic_option", what)
}

fn base_plan(ctx: &EffectCtx, args: &EvaluatedForm) -> RivetResult<QuicPlan> {
    let endpoint = text_value(args.head.first(), "with quic")?;
    let mut plan = QuicPlan {
        endpoint: endpoint.clone(),
        server_name: String::new(),
        alpn: String::new(),
        max_streams: DEFAULT_MAX_STREAMS,
        datagrams: false,
        migration: false,
        early_data: false,
        timeout_ms: None,
        tls: QuicTls::default(),
        peer_addr: None,
        steps: Vec::new(),
        context: context(ctx),
    };
    for (key, vals) in &args.options {
        match key.as_str() {
            "alpn" => plan.alpn = text_value(vals.first(), "alpn")?,
            "max_streams" => {
                plan.max_streams =
                    u32::try_from(uint_value(vals.first(), "max_streams")?).unwrap_or(u32::MAX)
            }
            "migration" => plan.migration = bool_value(vals.first(), "migration")?,
            "datagrams" => plan.datagrams = bool_value(vals.first(), "datagrams")?,
            "early_data" => plan.early_data = bool_value(vals.first(), "early_data")?,
            "timeout" => plan.timeout_ms = Some(duration_value(vals.first())?),
            "tls" => {
                let which = vals.first().and_then(EvalArg::word).unwrap_or_default();
                let v = text_value(vals.get(1), &format!("tls {which}"))?;
                match which {
                    "server_name" => plan.tls.server_name = Some(v),
                    "ca_file" => plan.tls.ca_file = Some(v),
                    "cert_file" => plan.tls.cert_file = Some(v),
                    "key_file" => plan.tls.key_file = Some(v),
                    other => {
                        return Err(option_error(format!(
                            "`tls {other}` is not valid; use server_name, ca_file, cert_file or key_file"
                        )));
                    }
                }
            }
            other => {
                return Err(option_error(format!(
                    "`{other}` is not a valid quic option (alpn, max_streams, migration, datagrams, timeout, tls)"
                )));
            }
        }
    }
    plan.server_name = match &plan.tls.server_name {
        Some(n) => n.clone(),
        None => url::Url::parse(&endpoint)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.trim_matches(['[', ']']).to_string()))
            .unwrap_or_default(),
    };
    Ok(plan)
}

/// `framing …` and `timeout "D"` options of a child stream.
fn stream_options(options: &[(String, Vec<EvalArg>)]) -> RivetResult<(Framing, Option<u64>)> {
    let mut framing = Framing::default();
    let mut timeout = None;
    for (key, vals) in options {
        match key.as_str() {
            "framing" => framing = parse_framing(vals)?,
            "timeout" => timeout = Some(duration_value(vals.first())?),
            other => {
                return Err(option_error(format!(
                    "`{other}` is not a valid stream option (framing, timeout)"
                )));
            }
        }
    }
    Ok((framing, timeout))
}

fn parse_framing(vals: &[EvalArg]) -> RivetResult<Framing> {
    let mut f = Framing::default();
    let mut i = 1;
    f.kind = match vals.first().and_then(EvalArg::word) {
        Some("raw") => FramingKind::Raw,
        Some("newline") => FramingKind::Newline,
        Some("length32") => FramingKind::Length32 { big_endian: true },
        Some("delimiter") => {
            i = 2;
            FramingKind::Delimiter(text_value(vals.get(1), "framing delimiter")?.into_bytes())
        }
        _ => {
            return Err(option_error(
                "framing is newline, length32 endian big|little, delimiter \"S\" or raw",
            ));
        }
    };
    while i < vals.len() {
        match vals[i].word() {
            Some("endian") => {
                let big = match vals.get(i + 1).and_then(EvalArg::word) {
                    Some("big") => true,
                    Some("little") => false,
                    _ => return Err(option_error("endian is big or little")),
                };
                if let FramingKind::Length32 { big_endian } = &mut f.kind {
                    *big_endian = big;
                }
                i += 2;
            }
            Some("max_frame") => {
                f.max_frame =
                    u32::try_from(uint_value(vals.get(i + 1), "max_frame")?).unwrap_or(u32::MAX);
                i += 2;
            }
            _ => return Err(option_error("unexpected framing argument")),
        }
    }
    if matches!(f.kind, FramingKind::Delimiter(ref d) if d.is_empty()) {
        return Err(option_error("the framing delimiter must not be empty"));
    }
    Ok(f)
}

#[async_trait]
impl EffectAdapter for QuicAdapter {
    async fn open(
        &self,
        ctx: &EffectCtx,
        _form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let mut plan = base_plan(ctx, &args)?;
        plan.steps = vec![QuicStep::Connect];
        let session = Arc::new(QuicSession::new(Arc::clone(&self.files)));
        let r = (self.exchange)(plan.clone(), Arc::clone(&ctx.policy), session.clone()).await?;
        plan.steps.clear();
        plan.peer_addr = session.peer();
        Ok(Box::new(QuicConn {
            exchange: Arc::clone(&self.exchange),
            session,
            plan,
            alpn: r.negotiated_alpn,
        }))
    }
}

async fn run_step(
    exchange: &Arc<QuicExchangeFn>,
    session: &Arc<QuicSession>,
    base: &QuicPlan,
    ctx: &EffectCtx,
    step: QuicStep,
) -> RivetResult<QuicResult> {
    let mut plan = base.clone();
    plan.context = context(ctx);
    plan.steps = vec![step];
    exchange(plan, Arc::clone(&ctx.policy), session.clone()).await
}

/// The connection handle (`connection.open …`, `connection.send_datagram …`).
struct QuicConn {
    exchange: Arc<QuicExchangeFn>,
    session: Arc<QuicSession>,
    plan: QuicPlan,
    alpn: String,
}

#[async_trait]
impl ResourceHandle for QuicConn {
    async fn call(
        &mut self,
        ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
    ) -> RivetResult<Value> {
        match method {
            "send_datagram" => {
                let codec = codec_at(&args, 0, method)?;
                let payload = encode(codec, &value_at(&args, 1, method)?)?;
                run_step(
                    &self.exchange,
                    &self.session,
                    &self.plan,
                    ctx,
                    QuicStep::SendDatagram { payload },
                )
                .await?;
                Ok(Value::Null)
            }
            "receive_datagram" => {
                let codec = codec_at(&args, 0, method)?;
                let timeout_ms = timeout_arg(&args)?;
                let r = run_step(
                    &self.exchange,
                    &self.session,
                    &self.plan,
                    ctx,
                    QuicStep::ReceiveDatagram { timeout_ms },
                )
                .await?;
                match r.value {
                    Value::Bytes(b) => decode(codec, b),
                    other => Ok(other),
                }
            }
            other => Err(RivetError::unsupported(
                "unsupported.method",
                format!(
                    "a quic connection has no `{other}` method (open, accept, send_datagram, receive_datagram)"
                ),
            )),
        }
    }

    async fn property(&mut self, _ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        match name {
            "alpn" => Ok(Value::text(&self.alpn)),
            other => Err(RivetError::unsupported(
                "unsupported.property",
                format!("a quic connection has no `{other}` property (alpn)"),
            )),
        }
    }

    async fn open_child(
        &mut self,
        ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
        options: Vec<(String, Vec<EvalArg>)>,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let dir = match args.first().and_then(EvalArg::word) {
            Some("bidi") => StreamDir::Bidi,
            Some("uni") => StreamDir::Uni,
            _ => {
                return Err(RivetError::validation(
                    "quic.direction",
                    format!("`connection.{method}` takes bidi or uni"),
                ));
            }
        };
        let (framing, timeout_ms) = stream_options(&options)?;
        let step = match method {
            "open" => QuicStep::OpenStream { dir, framing },
            "accept" => QuicStep::AcceptStream {
                dir,
                framing,
                timeout_ms,
            },
            other => {
                return Err(RivetError::unsupported(
                    "unsupported.child",
                    format!("a quic connection cannot `{other}` a stream (open, accept)"),
                ));
            }
        };
        let r = run_step(&self.exchange, &self.session, &self.plan, ctx, step).await?;
        let key = r
            .value
            .as_i64()
            .ok_or_else(|| RivetError::internal("quic stream step returned no stream key"))?
            as u64;
        Ok(Box::new(QuicStreamHandle {
            exchange: Arc::clone(&self.exchange),
            session: Arc::clone(&self.session),
            plan: self.plan.clone(),
            key,
            timeout_ms,
        }))
    }

    async fn close(self: Box<Self>) -> RivetResult<()> {
        let mut plan = self.plan.clone();
        plan.steps = vec![QuicStep::Close];
        self.session.exchange(plan).await.map(|_| ())
    }
}

/// A child stream handle (`with connection.open bidi as stream`).
struct QuicStreamHandle {
    exchange: Arc<QuicExchangeFn>,
    session: Arc<QuicSession>,
    plan: QuicPlan,
    key: u64,
    timeout_ms: Option<u64>,
}

impl QuicStreamHandle {
    async fn receive(&self, ctx: &EffectCtx, timeout_ms: Option<u64>) -> RivetResult<Value> {
        let r = run_step(
            &self.exchange,
            &self.session,
            &self.plan,
            ctx,
            QuicStep::Receive {
                stream: self.key,
                timeout_ms: timeout_ms.or(self.timeout_ms),
            },
        )
        .await?;
        Ok(r.value)
    }
}

#[async_trait]
impl ResourceHandle for QuicStreamHandle {
    async fn call(
        &mut self,
        ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
    ) -> RivetResult<Value> {
        match method {
            "send" => {
                let codec = codec_at(&args, 0, method)?;
                let payload = encode(codec, &value_at(&args, 1, method)?)?;
                run_step(
                    &self.exchange,
                    &self.session,
                    &self.plan,
                    ctx,
                    QuicStep::Send {
                        stream: self.key,
                        payload,
                    },
                )
                .await?;
                Ok(Value::Null)
            }
            "receive" => {
                let codec = codec_at(&args, 0, method)?;
                match self.receive(ctx, timeout_arg(&args)?).await? {
                    Value::Bytes(b) => decode(codec, b),
                    _ => Err(RivetError::new(
                        ErrorKind::Protocol,
                        "quic.stream_finished",
                        "the peer finished the stream before a complete message arrived",
                    )),
                }
            }
            "finish_send" => {
                run_step(
                    &self.exchange,
                    &self.session,
                    &self.plan,
                    ctx,
                    QuicStep::FinishSend { stream: self.key },
                )
                .await?;
                Ok(Value::Null)
            }
            other => Err(RivetError::unsupported(
                "unsupported.method",
                format!("a quic stream has no `{other}` method (send, receive, finish_send)"),
            )),
        }
    }

    /// `for chunk in stream` — frames (or raw chunks) until the peer's FIN.
    async fn next(&mut self, ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        match self.receive(ctx, None).await? {
            Value::Null => Ok(None),
            v => Ok(Some(v)),
        }
    }

    async fn close(self: Box<Self>) -> RivetResult<()> {
        let mut plan = self.plan.clone();
        plan.steps = vec![QuicStep::CloseStream { stream: self.key }];
        self.session.exchange(plan).await.map(|_| ())
    }
}

struct Conn {
    endpoint: quinn::Endpoint,
    conn: quinn::Connection,
    alpn: String,
    peer: SocketAddr,
}

struct StreamState {
    send: Option<quinn::SendStream>,
    recv: Option<quinn::RecvStream>,
    framing: Framing,
    buf: Vec<u8>,
    eof: bool,
}

// vhco:infra quic_adapter satisfies QuicDriver
// vhco:net quic <granted quic://HOST:PORT> -- native QUIC v1 to the policy-granted origin over its broker-checked UDP path
// vhco:file read <bundle root>/<tls ca_file|cert_file|key_file> -- trust roots and client identity, read only after an allow_read permit
pub struct QuicSession {
    files: Arc<dyn FileAccess>,
    conn: Mutex<Option<Arc<Conn>>>,
    streams: Mutex<HashMap<u64, Arc<tokio::sync::Mutex<StreamState>>>>,
    next_key: AtomicU64,
}

fn lock_err() -> RivetError {
    RivetError::internal("quic session lock poisoned")
}

fn timeout_err(what: &str, wait: Duration) -> RivetError {
    RivetError::new(
        ErrorKind::Timeout,
        "timeout",
        format!("{what} did not complete within {} ms", wait.as_millis()),
    )
}

fn wait_for(plan: &QuicPlan, step_ms: Option<u64>) -> Duration {
    let remaining = plan
        .context
        .deadline
        .saturating_duration_since(Instant::now());
    match step_ms.or(plan.timeout_ms) {
        Some(ms) => remaining.min(Duration::from_millis(ms)),
        None => remaining,
    }
}

/// Map a quinn connection failure to the registry: TLS alerts (bad
/// certificate, unknown CA, no_application_protocol) are kind `tls`.
fn connection_error(e: &quinn::ConnectionError) -> RivetError {
    use quinn::ConnectionError as C;
    let code = match e {
        C::TransportError(t) => Some(u64::from(t.code)),
        C::ConnectionClosed(c) => Some(u64::from(c.error_code)),
        _ => None,
    };
    match (e, code) {
        (_, Some(c)) if (0x100..=0x1ff).contains(&c) => {
            let alert = c - 0x100;
            let code = if alert == 120 {
                "quic.alpn_mismatch"
            } else {
                "quic.tls"
            };
            RivetError::new(ErrorKind::Tls, code, format!("QUIC handshake failed: {e}"))
                .with_details(Value::object([("tls_alert", Value::Int(alert as i64))]))
        }
        (C::TimedOut, _) => RivetError::new(
            ErrorKind::Timeout,
            "quic.idle_timeout",
            "the QUIC connection timed out",
        ),
        (C::ApplicationClosed(_), _) | (C::LocallyClosed, _) => RivetError::new(
            ErrorKind::Connection,
            "quic.closed",
            format!("the QUIC connection is closed: {e}"),
        ),
        (C::TransportError(_) | C::ConnectionClosed(_), _) => RivetError::new(
            ErrorKind::Protocol,
            "quic.transport",
            format!("QUIC transport error: {e}"),
        ),
        _ => RivetError::new(
            ErrorKind::Connection,
            "quic.connection",
            format!("QUIC connection failed: {e}"),
        ),
    }
}

fn read_error(e: quinn::ReadError) -> RivetError {
    match e {
        quinn::ReadError::Reset(code) => RivetError::new(
            ErrorKind::Protocol,
            "quic.stream_reset",
            format!("the peer reset the stream (code {code})"),
        )
        .with_details(Value::object([(
            "code",
            Value::Int(code.into_inner() as i64),
        )])),
        quinn::ReadError::ConnectionLost(c) => connection_error(&c),
        other => RivetError::new(
            ErrorKind::Protocol,
            "quic.stream",
            format!("stream read failed: {other}"),
        ),
    }
}

fn write_error(e: quinn::WriteError) -> RivetError {
    match e {
        quinn::WriteError::Stopped(code) => RivetError::new(
            ErrorKind::Protocol,
            "quic.stop_sending",
            format!("the peer stopped reading the stream (code {code})"),
        )
        .with_details(Value::object([(
            "code",
            Value::Int(code.into_inner() as i64),
        )])),
        quinn::WriteError::ConnectionLost(c) => connection_error(&c),
        quinn::WriteError::ClosedStream => RivetError::validation(
            "quic.stream_finished",
            "the send direction is already finished",
        ),
        other => RivetError::new(
            ErrorKind::Protocol,
            "quic.stream",
            format!("stream write failed: {other}"),
        ),
    }
    .with_effects(EffectsStatus::Unknown)
}

fn direction_error(what: &str) -> RivetError {
    RivetError::validation("quic.direction", format!("this stream cannot {what}"))
}

fn frame(framing: &Framing, payload: Vec<u8>) -> RivetResult<Vec<u8>> {
    if payload.len() > framing.max_frame as usize {
        return Err(RivetError::new(
            ErrorKind::Protocol,
            "quic.frame_too_large",
            format!(
                "a {}-byte message exceeds max_frame {}",
                payload.len(),
                framing.max_frame
            ),
        ));
    }
    Ok(match &framing.kind {
        FramingKind::Raw => payload,
        FramingKind::Newline => {
            if payload.contains(&b'\n') {
                return Err(RivetError::validation(
                    "framing.delimiter_in_payload",
                    "newline framing cannot carry a message that contains a newline",
                ));
            }
            let mut p = payload;
            p.push(b'\n');
            p
        }
        FramingKind::Delimiter(d) => {
            if payload.windows(d.len()).any(|w| w == d.as_slice()) {
                return Err(RivetError::validation(
                    "framing.delimiter_in_payload",
                    "the message contains the framing delimiter",
                ));
            }
            let mut p = payload;
            p.extend_from_slice(d);
            p
        }
        FramingKind::Length32 { big_endian } => {
            let n = payload.len() as u32;
            let mut p = if *big_endian {
                n.to_be_bytes().to_vec()
            } else {
                n.to_le_bytes().to_vec()
            };
            p.extend(payload);
            p
        }
    })
}

fn too_large(max: u32) -> RivetError {
    RivetError::new(
        ErrorKind::Protocol,
        "quic.frame_too_large",
        format!("an incoming frame exceeds max_frame {max}"),
    )
}

/// One complete frame from the buffer, if present.
fn take_frame(st: &mut StreamState) -> RivetResult<Option<Vec<u8>>> {
    let max = st.framing.max_frame as usize;
    match &st.framing.kind {
        FramingKind::Raw => {
            if st.buf.is_empty() {
                Ok(None)
            } else {
                Ok(Some(std::mem::take(&mut st.buf)))
            }
        }
        FramingKind::Newline | FramingKind::Delimiter(_) => {
            let delim: Vec<u8> = match &st.framing.kind {
                FramingKind::Delimiter(d) => d.clone(),
                _ => vec![b'\n'],
            };
            match st
                .buf
                .windows(delim.len())
                .position(|w| w == delim.as_slice())
            {
                Some(p) if p <= max => {
                    let f = st.buf[..p].to_vec();
                    st.buf.drain(..p + delim.len());
                    Ok(Some(f))
                }
                Some(_) => Err(too_large(st.framing.max_frame)),
                None if st.buf.len() > max + delim.len() => Err(too_large(st.framing.max_frame)),
                None => Ok(None),
            }
        }
        FramingKind::Length32 { big_endian } => {
            if st.buf.len() < 4 {
                return Ok(None);
            }
            let h = [st.buf[0], st.buf[1], st.buf[2], st.buf[3]];
            let n = if *big_endian {
                u32::from_be_bytes(h)
            } else {
                u32::from_le_bytes(h)
            } as usize;
            if n > max {
                return Err(too_large(st.framing.max_frame));
            }
            if st.buf.len() < 4 + n {
                return Ok(None);
            }
            let f = st.buf[4..4 + n].to_vec();
            st.buf.drain(..4 + n);
            Ok(Some(f))
        }
    }
}

async fn receive_frame(st: &mut StreamState) -> RivetResult<Value> {
    loop {
        if let Some(f) = take_frame(st)? {
            return Ok(Value::Bytes(f));
        }
        if st.eof {
            if st.buf.is_empty() {
                return Ok(Value::Null);
            }
            return Err(RivetError::new(
                ErrorKind::Protocol,
                "framing.truncated",
                "the peer finished the stream in the middle of a frame",
            ));
        }
        let recv = st.recv.as_mut().ok_or_else(|| direction_error("receive"))?;
        match recv.read_chunk(64 * 1024, true).await.map_err(read_error)? {
            Some(chunk) => st.buf.extend_from_slice(&chunk.bytes),
            None => st.eof = true,
        }
    }
}

fn load_certs(pem: &[u8], what: &str) -> RivetResult<Vec<CertificateDer<'static>>> {
    let certs: Vec<_> = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<_, _>>()
        .map_err(|e| RivetError::new(ErrorKind::Tls, "tls.pem", format!("{what}: {e}")))?;
    if certs.is_empty() {
        return Err(RivetError::new(
            ErrorKind::Tls,
            "tls.pem",
            format!("{what} contains no certificate"),
        ));
    }
    Ok(certs)
}

impl QuicSession {
    pub fn new(files: Arc<dyn FileAccess>) -> QuicSession {
        QuicSession {
            files,
            conn: Mutex::new(None),
            streams: Mutex::new(HashMap::new()),
            next_key: AtomicU64::new(1),
        }
    }

    fn peer(&self) -> Option<SocketAddr> {
        self.conn.lock().ok()?.as_ref().map(|c| c.peer)
    }

    fn conn(&self) -> RivetResult<Arc<Conn>> {
        self.conn
            .lock()
            .map_err(|_| lock_err())?
            .clone()
            .ok_or_else(|| {
                RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.closed",
                    "the quic connection is closed",
                )
            })
    }

    fn stream(&self, key: u64) -> RivetResult<Arc<tokio::sync::Mutex<StreamState>>> {
        self.streams
            .lock()
            .map_err(|_| lock_err())?
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.closed",
                    "the quic stream is closed",
                )
            })
    }

    fn active(&self) -> u32 {
        self.streams.lock().map(|m| m.len() as u32).unwrap_or(0)
    }

    async fn read_file(&self, path: &str) -> RivetResult<Vec<u8>> {
        let mut op = FileOperation::new(FileVerb::Read, path);
        op.codec = Some(Codec::Bytes);
        match self.files.apply(op).await? {
            Value::Bytes(b) => Ok(b),
            Value::Text(t) => Ok(t.into_bytes()),
            _ => Err(RivetError::internal("tls file read returned no bytes")),
        }
    }

    async fn client_config(&self, plan: &QuicPlan) -> RivetResult<quinn::ClientConfig> {
        let tls_err = |e: rustls::Error| {
            RivetError::new(ErrorKind::Tls, "quic.tls_config", format!("TLS setup: {e}"))
        };
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(tls_err)?;
        let builder = match &plan.tls.ca_file {
            // `tls ca_file` replaces the default trust roots; hostname verification stays on.
            Some(path) => {
                let pem = self.read_file(path).await?;
                let mut roots = rustls::RootCertStore::empty();
                for c in load_certs(&pem, path)? {
                    roots.add(c).map_err(tls_err)?;
                }
                builder.with_root_certificates(roots)
            }
            None => {
                use rustls_platform_verifier::BuilderVerifierExt;
                builder.with_platform_verifier().map_err(tls_err)?
            }
        };
        let mut cc = match (&plan.tls.cert_file, &plan.tls.key_file) {
            (Some(cert), Some(key)) => {
                let chain = load_certs(&self.read_file(cert).await?, cert)?;
                let key_pem = self.read_file(key).await?;
                let key = PrivateKeyDer::from_pem_slice(&key_pem).map_err(|_| {
                    RivetError::new(
                        ErrorKind::Tls,
                        "tls.pem",
                        "tls key_file holds no private key",
                    )
                })?;
                builder.with_client_auth_cert(chain, key).map_err(tls_err)?
            }
            _ => builder.with_no_client_auth(),
        };
        cc.alpn_protocols = vec![plan.alpn.as_bytes().to_vec()];
        cc.enable_early_data = false;
        let qc = QuicClientConfig::try_from(cc).map_err(|e| {
            RivetError::new(ErrorKind::Tls, "quic.tls_config", format!("TLS setup: {e}"))
        })?;
        let mut config = quinn::ClientConfig::new(Arc::new(qc));
        let mut transport = quinn::TransportConfig::default();
        let streams = quinn::VarInt::from_u32(plan.max_streams);
        transport.max_concurrent_bidi_streams(streams);
        transport.max_concurrent_uni_streams(streams);
        transport.datagram_receive_buffer_size(if plan.datagrams { Some(65_536) } else { None });
        config.transport_config(Arc::new(transport));
        Ok(config)
    }

    async fn connect(&self, plan: &QuicPlan) -> RivetResult<String> {
        let peer = plan
            .peer_addr
            .ok_or_else(|| RivetError::internal("quic connect without a checked address"))?;
        let config = self.client_config(plan).await?;
        let local: SocketAddr = if peer.is_ipv4() {
            (Ipv4Addr::UNSPECIFIED, 0).into()
        } else {
            (Ipv6Addr::UNSPECIFIED, 0).into()
        };
        let endpoint = quinn::Endpoint::client(local).map_err(|e| {
            RivetError::new(
                ErrorKind::Connection,
                "quic.socket",
                format!("QUIC socket: {e}"),
            )
        })?;
        let connecting = endpoint
            .connect_with(config, peer, &plan.server_name)
            .map_err(|e| {
                RivetError::new(
                    ErrorKind::Connection,
                    "quic.connect",
                    format!("QUIC connect: {e}"),
                )
            })?;
        let wait = wait_for(plan, None);
        let conn = match tokio::time::timeout(wait, connecting).await {
            Err(_) => {
                endpoint.close(0u32.into(), b"deadline");
                return Err(timeout_err("the QUIC handshake", wait));
            }
            Ok(Err(e)) => return Err(connection_error(&e)),
            Ok(Ok(c)) => c,
        };
        let alpn = conn
            .handshake_data()
            .and_then(|h| h.downcast::<HandshakeData>().ok())
            .and_then(|h| h.protocol)
            .map(|p| String::from_utf8_lossy(&p).to_string())
            .unwrap_or_default();
        let fail = |e: RivetError| {
            conn.close(0u32.into(), b"refused");
            Err(e)
        };
        if alpn != plan.alpn {
            return fail(RivetError::new(
                ErrorKind::Tls,
                "quic.alpn_mismatch",
                format!("the peer negotiated ALPN `{alpn}`, not `{}`", plan.alpn),
            ));
        }
        if plan.datagrams && conn.max_datagram_size().is_none() {
            return fail(RivetError::new(
                ErrorKind::Protocol,
                "quic.datagrams_unavailable",
                "the peer did not negotiate the QUIC DATAGRAM extension",
            ));
        }
        *self.conn.lock().map_err(|_| lock_err())? = Some(Arc::new(Conn {
            endpoint,
            conn,
            alpn: alpn.clone(),
            peer,
        }));
        Ok(alpn)
    }

    fn insert(&self, st: StreamState) -> RivetResult<u64> {
        let key = self.next_key.fetch_add(1, Ordering::SeqCst);
        self.streams
            .lock()
            .map_err(|_| lock_err())?
            .insert(key, Arc::new(tokio::sync::Mutex::new(st)));
        Ok(key)
    }

    fn stream_budget(&self, plan: &QuicPlan) -> RivetResult<()> {
        if self.active() >= plan.max_streams {
            return Err(RivetError::new(
                ErrorKind::Limit,
                "limit.quic_streams",
                format!(
                    "max_streams {} child streams are already open",
                    plan.max_streams
                ),
            ));
        }
        Ok(())
    }

    async fn step(
        &self,
        plan: &QuicPlan,
        step: &QuicStep,
        out: &mut QuicResult,
    ) -> RivetResult<()> {
        match step {
            QuicStep::Connect => {
                out.negotiated_alpn = self.connect(plan).await?;
            }
            QuicStep::OpenStream { dir, framing } => {
                self.stream_budget(plan)?;
                let c = self.conn()?;
                let wait = wait_for(plan, None);
                let (send, recv) = match dir {
                    StreamDir::Bidi => {
                        let (s, r) = tokio::time::timeout(wait, c.conn.open_bi())
                            .await
                            .map_err(|_| timeout_err("opening a stream", wait))?
                            .map_err(|e| connection_error(&e))?;
                        (Some(s), Some(r))
                    }
                    StreamDir::Uni => {
                        let s = tokio::time::timeout(wait, c.conn.open_uni())
                            .await
                            .map_err(|_| timeout_err("opening a stream", wait))?
                            .map_err(|e| connection_error(&e))?;
                        (Some(s), None)
                    }
                };
                let key = self.insert(StreamState {
                    send,
                    recv,
                    framing: framing.clone(),
                    buf: Vec::new(),
                    eof: false,
                })?;
                out.value = Value::Int(key as i64);
            }
            QuicStep::AcceptStream {
                dir,
                framing,
                timeout_ms,
            } => {
                self.stream_budget(plan)?;
                let c = self.conn()?;
                let wait = wait_for(plan, *timeout_ms);
                let (send, recv) = match dir {
                    StreamDir::Bidi => {
                        let (s, r) = tokio::time::timeout(wait, c.conn.accept_bi())
                            .await
                            .map_err(|_| timeout_err("accepting a stream", wait))?
                            .map_err(|e| connection_error(&e))?;
                        (Some(s), Some(r))
                    }
                    StreamDir::Uni => {
                        let r = tokio::time::timeout(wait, c.conn.accept_uni())
                            .await
                            .map_err(|_| timeout_err("accepting a stream", wait))?
                            .map_err(|e| connection_error(&e))?;
                        (None, Some(r))
                    }
                };
                let key = self.insert(StreamState {
                    send,
                    recv,
                    framing: framing.clone(),
                    buf: Vec::new(),
                    eof: false,
                })?;
                out.value = Value::Int(key as i64);
            }
            QuicStep::Send { stream, payload } => {
                let s = self.stream(*stream)?;
                let mut st = s.lock().await;
                let bytes = frame(&st.framing, payload.clone())?;
                let wait = wait_for(plan, None);
                let send = st.send.as_mut().ok_or_else(|| direction_error("send"))?;
                tokio::time::timeout(wait, send.write_all(&bytes))
                    .await
                    .map_err(|_| {
                        timeout_err("writing to the stream", wait)
                            .with_effects(EffectsStatus::Unknown)
                    })?
                    .map_err(write_error)?;
                out.effects = EffectsStatus::Unknown;
            }
            QuicStep::Receive { stream, timeout_ms } => {
                let s = self.stream(*stream)?;
                let mut st = s.lock().await;
                let wait = wait_for(plan, *timeout_ms);
                out.value = tokio::time::timeout(wait, receive_frame(&mut st))
                    .await
                    .map_err(|_| timeout_err("receiving from the stream", wait))??;
            }
            QuicStep::FinishSend { stream } => {
                let s = self.stream(*stream)?;
                let mut st = s.lock().await;
                let send = st.send.as_mut().ok_or_else(|| direction_error("send"))?;
                send.finish().map_err(|_| {
                    RivetError::validation(
                        "quic.stream_finished",
                        "the send direction is already finished",
                    )
                })?;
            }
            QuicStep::CloseStream { stream } => {
                let removed = self.streams.lock().map_err(|_| lock_err())?.remove(stream);
                if let Some(s) = removed {
                    let mut st = s.lock().await;
                    if let Some(send) = st.send.as_mut() {
                        // FIN (a no-op when finish_send already ran).
                        let _ = send.finish();
                    }
                    if let (false, Some(recv)) = (st.eof, st.recv.as_mut()) {
                        let _ = recv.stop(0u32.into());
                    }
                }
            }
            QuicStep::SendDatagram { payload } => {
                let c = self.conn()?;
                let max = c.conn.max_datagram_size().ok_or_else(|| {
                    RivetError::new(
                        ErrorKind::Protocol,
                        "quic.datagrams_unavailable",
                        "the peer did not negotiate the QUIC DATAGRAM extension",
                    )
                })?;
                let too_large = || {
                    RivetError::new(
                        ErrorKind::Protocol,
                        "quic.datagram_too_large",
                        format!(
                            "a {}-byte datagram exceeds the negotiated maximum {max}",
                            payload.len()
                        ),
                    )
                };
                if payload.len() > max {
                    return Err(too_large());
                }
                c.conn
                    .send_datagram(Bytes::from(payload.clone()))
                    .map_err(|e| match e {
                        quinn::SendDatagramError::TooLarge => too_large(),
                        quinn::SendDatagramError::ConnectionLost(c) => connection_error(&c),
                        other => RivetError::new(
                            ErrorKind::Protocol,
                            "quic.datagrams_unavailable",
                            format!("{other}"),
                        ),
                    })?;
                out.effects = EffectsStatus::Unknown;
            }
            QuicStep::ReceiveDatagram { timeout_ms } => {
                let c = self.conn()?;
                let wait = wait_for(plan, *timeout_ms);
                let d = tokio::time::timeout(wait, c.conn.read_datagram())
                    .await
                    .map_err(|_| timeout_err("receiving a datagram", wait))?
                    .map_err(|e| connection_error(&e))?;
                out.value = Value::Bytes(d.to_vec());
            }
            QuicStep::Close => {
                if let Ok(mut m) = self.streams.lock() {
                    m.clear();
                }
                let taken = self.conn.lock().map_err(|_| lock_err())?.take();
                if let Some(c) = taken {
                    c.conn.close(0u32.into(), b"scope end");
                    let _ =
                        tokio::time::timeout(Duration::from_secs(1), c.endpoint.wait_idle()).await;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl QuicDriver for QuicSession {
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<SocketAddr>> {
        tokio::net::lookup_host((host, port))
            .await
            .map(|a| a.collect())
            .map_err(|e| RivetError::new(ErrorKind::Dns, "dns.resolve", format!("{host}: {e}")))
    }

    async fn exchange(&self, plan: QuicPlan) -> RivetResult<QuicResult> {
        let mut out = QuicResult {
            value: Value::Null,
            stream_count: 0,
            negotiated_alpn: self
                .conn
                .lock()
                .ok()
                .and_then(|c| c.as_ref().map(|c| c.alpn.clone()))
                .unwrap_or_default(),
            effects: EffectsStatus::None,
        };
        for step in &plan.steps {
            self.step(&plan, step, &mut out).await?;
        }
        out.stream_count = self.active();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(kind: FramingKind, max: u32, buf: &[u8], eof: bool) -> StreamState {
        StreamState {
            send: None,
            recv: None,
            framing: Framing {
                kind,
                max_frame: max,
            },
            buf: buf.to_vec(),
            eof,
        }
    }

    #[tokio::test]
    async fn framing_encodes_and_splits_frames() {
        let f = Framing {
            kind: FramingKind::Length32 { big_endian: true },
            max_frame: 8,
        };
        assert_eq!(frame(&f, b"ab".to_vec()).unwrap(), b"\0\0\0\x02ab");
        assert!(frame(&f, vec![0; 9]).is_err());
        let mut st = state(
            FramingKind::Length32 { big_endian: true },
            8,
            b"\0\0\0\x02ab\0\0\0\x01c",
            true,
        );
        assert_eq!(
            receive_frame(&mut st).await.unwrap(),
            Value::Bytes(b"ab".to_vec())
        );
        assert_eq!(
            receive_frame(&mut st).await.unwrap(),
            Value::Bytes(b"c".to_vec())
        );
        assert_eq!(receive_frame(&mut st).await.unwrap(), Value::Null);
        let mut st = state(FramingKind::Newline, 8, b"one\ntw", true);
        assert_eq!(
            receive_frame(&mut st).await.unwrap(),
            Value::Bytes(b"one".to_vec())
        );
        assert_eq!(
            receive_frame(&mut st).await.unwrap_err().code,
            "framing.truncated"
        );
        let mut st = state(
            FramingKind::Length32 { big_endian: false },
            2,
            b"\x05\0\0\0",
            false,
        );
        assert_eq!(
            receive_frame(&mut st).await.unwrap_err().code,
            "quic.frame_too_large"
        );
    }

    #[test]
    fn framing_option_parses() {
        let w = |s: &str| EvalArg::Word(s.into());
        let f = parse_framing(&[
            w("length32"),
            w("endian"),
            w("little"),
            w("max_frame"),
            EvalArg::Value(Value::Int(64)),
        ])
        .unwrap();
        assert_eq!(f.kind, FramingKind::Length32 { big_endian: false });
        assert_eq!(f.max_frame, 64);
        assert!(parse_framing(&[w("bogus")]).is_err());
    }
}
