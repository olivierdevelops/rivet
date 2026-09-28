//! Scoped UDP sockets (`with udp …`, PROP-2026-0001 Increment 10).
//!
//! ```text
//!  with udp "H:P" as socket ─▶ UdpAdapter::open ─▶ exchange_datagrams(plan{Open}) ─▶ UdpSession::exchange
//!      socket.send json V   ─▶ UdpHandle::call  ─▶ exchange_datagrams(plan{Send}) ─▶ socket.send
//!  end                      ─▶ UdpHandle::close ─▶ exchange_datagrams(plan{Close}) ─▶ leave groups, drop
//! ```
//!
//! Every step runs the `datagrams.exchange_datagrams` use case (injected by
//! the orchestrator), so each peer, bind and join is authorized before the
//! socket exists or a packet leaves.

use crate::domain::errors::EffectsStatus;
use crate::domain::ir::EffectForm;
use crate::domain::ports::{DatagramDriver, PolicyEvaluator};
use crate::domain::transport::{
    DEFAULT_MAX_DATAGRAM, DatagramMessage, DatagramMode, DatagramPlan, DatagramResult,
    DatagramStep, EffectContext, MAX_UDP_PAYLOAD,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use crate::infra::execution_driver::{
    EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle,
};
use crate::infra::wire_codec::{
    codec_at, decode, duration_value, encode, text_value, timeout_arg, uint_value, value_at,
};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

/// The `datagrams.exchange_datagrams` use case, injected by the orchestrator.
pub type DatagramExchangeFn = dyn Fn(
        DatagramPlan,
        Arc<dyn PolicyEvaluator>,
        Arc<dyn DatagramDriver>,
    ) -> BoxFuture<'static, RivetResult<DatagramResult>>
    + Send
    + Sync;

/// `EffectAdapter` for kind `udp`.
pub struct UdpAdapter {
    exchange: Arc<DatagramExchangeFn>,
}

impl UdpAdapter {
    pub fn new(exchange: Arc<DatagramExchangeFn>) -> UdpAdapter {
        UdpAdapter { exchange }
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
    RivetError::validation("validation.udp_option", what)
}

/// Build the socket's base plan from `with udp [bind|multicast] "H:P"` and its options.
fn base_plan(ctx: &EffectCtx, args: &EvaluatedForm) -> RivetResult<DatagramPlan> {
    let (mode, target) = match args.head.first() {
        Some(EvalArg::Word(w)) if w == "bind" => (DatagramMode::Bind, args.head.get(1)),
        Some(EvalArg::Word(w)) if w == "multicast" => (DatagramMode::Multicast, args.head.get(1)),
        other => (DatagramMode::Connected, other),
    };
    let target = text_value(target, "with udp")?;
    let mut plan = DatagramPlan {
        mode,
        peer: None,
        peer_addr: None,
        bind: None,
        multicast: None,
        interface: None,
        max_datagram: DEFAULT_MAX_DATAGRAM,
        timeout_ms: None,
        steps: Vec::new(),
        context: context(ctx),
    };
    match mode {
        DatagramMode::Connected => plan.peer = Some(target),
        DatagramMode::Bind => plan.bind = Some(target),
        DatagramMode::Multicast => plan.multicast = Some(target),
    }
    for (key, vals) in &args.options {
        match key.as_str() {
            "max_datagram" => {
                let n = uint_value(vals.first(), "max_datagram")?;
                plan.max_datagram = u32::try_from(n).unwrap_or(u32::MAX);
            }
            "timeout" => plan.timeout_ms = Some(duration_value(vals.first())?),
            "bind" if mode == DatagramMode::Multicast => {
                plan.bind = Some(text_value(vals.first(), "bind")?)
            }
            "interface" if mode == DatagramMode::Multicast => {
                plan.interface = Some(text_value(vals.first(), "interface")?)
            }
            other => {
                return Err(option_error(format!(
                    "`{other}` is not a valid option here; udp takes max_datagram, timeout (multicast adds bind, interface)"
                )));
            }
        }
    }
    Ok(plan)
}

#[async_trait]
impl EffectAdapter for UdpAdapter {
    async fn open(
        &self,
        ctx: &EffectCtx,
        _form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let mut plan = base_plan(ctx, &args)?;
        plan.steps = vec![DatagramStep::Open];
        let session: Arc<UdpSession> = Arc::new(UdpSession::default());
        (self.exchange)(plan.clone(), Arc::clone(&ctx.policy), session.clone()).await?;
        plan.steps.clear();
        Ok(Box::new(UdpHandle {
            exchange: Arc::clone(&self.exchange),
            session,
            plan,
        }))
    }
}

/// The scope's handle: every method is one authorized plan step.
struct UdpHandle {
    exchange: Arc<DatagramExchangeFn>,
    session: Arc<UdpSession>,
    plan: DatagramPlan,
}

impl UdpHandle {
    async fn step(&self, ctx: &EffectCtx, step: DatagramStep) -> RivetResult<DatagramResult> {
        let mut plan = self.plan.clone();
        plan.context = context(ctx);
        plan.steps = vec![step];
        (self.exchange)(plan, Arc::clone(&ctx.policy), self.session.clone()).await
    }

    fn first(r: DatagramResult) -> RivetResult<DatagramMessage> {
        r.messages
            .into_iter()
            .next()
            .ok_or_else(|| RivetError::internal("udp receive returned no datagram"))
    }
}

#[async_trait]
impl ResourceHandle for UdpHandle {
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
                self.step(ctx, DatagramStep::Send { payload }).await?;
                Ok(Value::Null)
            }
            "send_to" => {
                let peer = match value_at(&args, 0, method)? {
                    Value::Text(s) => s,
                    other => {
                        return Err(RivetError::validation(
                            "validation.udp_address",
                            format!(
                                "send_to needs a \"HOST:PORT\" peer, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };
                let codec = codec_at(&args, 1, method)?;
                let payload = encode(codec, &value_at(&args, 2, method)?)?;
                self.step(
                    ctx,
                    DatagramStep::SendTo {
                        peer,
                        addr: None,
                        payload,
                    },
                )
                .await?;
                Ok(Value::Null)
            }
            "receive" => {
                let codec = codec_at(&args, 0, method)?;
                let timeout_ms = timeout_arg(&args)?;
                let m = Self::first(self.step(ctx, DatagramStep::Receive { timeout_ms }).await?)?;
                decode(codec, m.data)
            }
            "receive_from" => {
                let codec = codec_at(&args, 0, method)?;
                let timeout_ms = timeout_arg(&args)?;
                let m = Self::first(
                    self.step(ctx, DatagramStep::ReceiveFrom { timeout_ms })
                        .await?,
                )?;
                Ok(Value::object([
                    ("peer", Value::text(m.peer)),
                    ("data", decode(codec, m.data)?),
                ]))
            }
            other => Err(RivetError::unsupported(
                "unsupported.method",
                format!(
                    "a udp socket has no `{other}` method (send, send_to, receive, receive_from)"
                ),
            )),
        }
    }

    /// `for packet in socket` — each datagram as `{peer, data: bytes}`.
    async fn next(&mut self, ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        let m = Self::first(
            self.step(ctx, DatagramStep::ReceiveFrom { timeout_ms: None })
                .await?,
        )?;
        Ok(Some(Value::object([
            ("peer", Value::text(m.peer)),
            ("data", Value::Bytes(m.data)),
        ])))
    }

    async fn close(self: Box<Self>) -> RivetResult<()> {
        let mut plan = self.plan.clone();
        plan.steps = vec![DatagramStep::Close];
        self.session.exchange(plan).await.map(|_| ())
    }
}

struct Joined {
    group: IpAddr,
    v4_iface: Option<Ipv4Addr>,
    v6_index: u32,
}

struct Socket {
    udp: UdpSocket,
    /// Default destination of `send` (the connected peer or the multicast group).
    dest: Option<SocketAddr>,
    connected: bool,
    joined: Option<Joined>,
}

// vhco:infra udp_adapter satisfies DatagramDriver
// vhco:net udp <granted HOST:PORT> -- connected peers, send_to peers and multicast groups named by policy.json
// vhco:net udp <granted bind HOST:PORT> -- explicit listener binds and multicast memberships (allow_listen)
#[derive(Default)]
pub struct UdpSession {
    socket: Mutex<Option<Arc<Socket>>>,
}

fn io_err(code: &str, what: &str, e: std::io::Error) -> RivetError {
    RivetError::new(ErrorKind::Connection, code, format!("{what}: {e}"))
}

fn wait_for(plan: &DatagramPlan, step_ms: Option<u64>) -> Duration {
    let remaining = plan
        .context
        .deadline
        .saturating_duration_since(Instant::now());
    match step_ms.or(plan.timeout_ms) {
        Some(ms) => remaining.min(Duration::from_millis(ms)),
        None => remaining,
    }
}

/// Interface option → (IPv4 interface address, interface index).
fn interface(name: Option<&str>) -> RivetResult<(Option<Ipv4Addr>, u32)> {
    let Some(name) = name else {
        return Ok((Some(Ipv4Addr::UNSPECIFIED), 0));
    };
    if let Ok(IpAddr::V4(a)) = name.parse::<IpAddr>() {
        return Ok((Some(a), 0));
    }
    #[cfg(unix)]
    {
        let c = std::ffi::CString::new(name).map_err(|_| option_error("bad interface name"))?;
        // SAFETY: `c` is a valid NUL-terminated string for the duration of the call.
        let idx = unsafe { libc::if_nametoindex(c.as_ptr()) };
        if idx != 0 {
            return Ok((None, idx));
        }
    }
    Err(RivetError::unsupported(
        "unsupported.udp_option",
        format!("multicast interface `{name}` is not available on this host"),
    ))
}

impl UdpSession {
    fn current(&self) -> RivetResult<Arc<Socket>> {
        self.socket
            .lock()
            .map_err(|_| RivetError::internal("udp socket lock poisoned"))?
            .clone()
            .ok_or_else(|| {
                RivetError::new(
                    ErrorKind::Cleanup,
                    "cleanup.closed",
                    "the udp socket is closed",
                )
            })
    }

    async fn open(&self, plan: &DatagramPlan) -> RivetResult<Socket> {
        match plan.mode {
            DatagramMode::Connected => {
                let peer = plan
                    .peer_addr
                    .ok_or_else(|| RivetError::internal("udp open without a checked peer"))?;
                let local: SocketAddr = if peer.is_ipv4() {
                    (Ipv4Addr::UNSPECIFIED, 0).into()
                } else {
                    (Ipv6Addr::UNSPECIFIED, 0).into()
                };
                let udp = UdpSocket::bind(local)
                    .await
                    .map_err(|e| io_err("udp.bind_failed", "ephemeral bind", e))?;
                udp.connect(peer)
                    .await
                    .map_err(|e| io_err("udp.connect_failed", "connect", e))?;
                Ok(Socket {
                    udp,
                    dest: Some(peer),
                    connected: true,
                    joined: None,
                })
            }
            DatagramMode::Bind => {
                let addr: SocketAddr = plan
                    .bind
                    .as_deref()
                    .and_then(|b| b.parse().ok())
                    .ok_or_else(|| RivetError::internal("udp bind without an address"))?;
                let udp = UdpSocket::bind(addr)
                    .await
                    .map_err(|e| io_err("udp.bind_failed", &format!("bind {addr}"), e))?;
                Ok(Socket {
                    udp,
                    dest: None,
                    connected: false,
                    joined: None,
                })
            }
            DatagramMode::Multicast => self.open_multicast(plan),
        }
    }

    fn open_multicast(&self, plan: &DatagramPlan) -> RivetResult<Socket> {
        use socket2::{Domain, Protocol, Socket as RawSocket, Type};
        let group: SocketAddr = plan
            .multicast
            .as_deref()
            .and_then(|g| g.parse().ok())
            .ok_or_else(|| RivetError::internal("multicast without a group"))?;
        let bind: SocketAddr = plan
            .bind
            .as_deref()
            .and_then(|b| b.parse().ok())
            .ok_or_else(|| RivetError::internal("multicast without a bind"))?;
        let (v4_iface, v6_index) = interface(plan.interface.as_deref())?;
        let domain = if bind.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };
        let s = RawSocket::new(domain, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|e| io_err("udp.socket_failed", "socket", e))?;
        s.set_reuse_address(true)
            .map_err(|e| io_err("udp.socket_failed", "reuse_address", e))?;
        #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
        s.set_reuse_port(true)
            .map_err(|e| io_err("udp.socket_failed", "reuse_port", e))?;
        s.set_broadcast(false)
            .map_err(|e| io_err("udp.socket_failed", "broadcast off", e))?;
        s.bind(&bind.into())
            .map_err(|e| io_err("udp.bind_failed", &format!("bind {bind}"), e))?;
        let unsupported = |e: std::io::Error| {
            RivetError::unsupported(
                "unsupported.udp_option",
                format!("joining multicast group {} failed: {e}", group.ip()),
            )
        };
        match group.ip() {
            IpAddr::V4(g) => {
                match v4_iface {
                    Some(a) => s.join_multicast_v4(&g, &a).map_err(unsupported)?,
                    None => {
                        #[cfg(any(
                            target_os = "linux",
                            target_os = "android",
                            target_os = "macos",
                            target_os = "freebsd",
                            windows
                        ))]
                        s.join_multicast_v4_n(
                            &g,
                            &socket2::InterfaceIndexOrAddress::Index(v6_index),
                        )
                        .map_err(unsupported)?;
                        #[cfg(not(any(
                            target_os = "linux",
                            target_os = "android",
                            target_os = "macos",
                            target_os = "freebsd",
                            windows
                        )))]
                        return Err(unsupported(std::io::Error::other(
                            "interface names are unsupported",
                        )));
                    }
                }
                if let Some(a) = v4_iface.filter(|a| !a.is_unspecified()) {
                    s.set_multicast_if_v4(&a).map_err(unsupported)?;
                }
            }
            IpAddr::V6(g) => {
                s.join_multicast_v6(&g, v6_index).map_err(unsupported)?;
            }
        }
        s.set_nonblocking(true)
            .map_err(|e| io_err("udp.socket_failed", "nonblocking", e))?;
        let udp = UdpSocket::from_std(s.into())
            .map_err(|e| io_err("udp.socket_failed", "register", e))?;
        Ok(Socket {
            udp,
            dest: Some(group),
            connected: false,
            joined: Some(Joined {
                group: group.ip(),
                v4_iface,
                v6_index,
            }),
        })
    }

    async fn receive(
        &self,
        plan: &DatagramPlan,
        timeout_ms: Option<u64>,
    ) -> RivetResult<DatagramMessage> {
        let sock = self.current()?;
        let max = plan.max_datagram.min(MAX_UDP_PAYLOAD) as usize;
        // One spare byte detects truncation portably: a datagram that fills it was clipped.
        let mut buf = vec![0u8; max + 1];
        let wait = wait_for(plan, timeout_ms);
        let got = tokio::time::timeout(wait, async {
            if sock.connected {
                let n = sock.udp.recv(&mut buf).await?;
                Ok::<_, std::io::Error>((n, sock.dest.unwrap_or_else(|| ([0, 0, 0, 0], 0).into())))
            } else {
                sock.udp.recv_from(&mut buf).await
            }
        })
        .await;
        let (n, peer) = match got {
            Err(_) => {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout",
                    format!("no datagram within {} ms", wait.as_millis()),
                )
                .with_effects(EffectsStatus::Unknown));
            }
            Ok(Err(e)) => {
                return Err(io_err("udp.receive_failed", "receive", e));
            }
            Ok(Ok(v)) => v,
        };
        if n > max {
            return Err(RivetError::new(
                ErrorKind::Protocol,
                "udp.truncated",
                format!(
                    "a datagram from {peer} exceeds max_datagram {max}; it is not parsed clipped"
                ),
            )
            .with_details(Value::object([("peer", Value::text(peer.to_string()))])));
        }
        buf.truncate(n);
        Ok(DatagramMessage {
            peer: peer.to_string(),
            data: buf,
        })
    }

    fn close(&self) {
        let taken = self.socket.lock().ok().and_then(|mut g| g.take());
        if let Some(sock) = taken
            && let Some(j) = &sock.joined
        {
            {
                let raw = socket2::SockRef::from(&sock.udp);
                let _ = match (j.group, j.v4_iface) {
                    (IpAddr::V4(g), Some(a)) => raw.leave_multicast_v4(&g, &a),
                    #[cfg(any(
                        target_os = "linux",
                        target_os = "android",
                        target_os = "macos",
                        target_os = "freebsd",
                        windows
                    ))]
                    (IpAddr::V4(g), None) => raw.leave_multicast_v4_n(
                        &g,
                        &socket2::InterfaceIndexOrAddress::Index(j.v6_index),
                    ),
                    (IpAddr::V6(g), _) => raw.leave_multicast_v6(&g, j.v6_index),
                    #[allow(unreachable_patterns)]
                    _ => Ok(()),
                };
            }
        }
    }
}

#[async_trait]
impl DatagramDriver for UdpSession {
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<SocketAddr>> {
        tokio::net::lookup_host((host, port))
            .await
            .map(|a| a.collect())
            .map_err(|e| RivetError::new(ErrorKind::Dns, "dns.resolve", format!("{host}: {e}")))
    }

    async fn exchange(&self, plan: DatagramPlan) -> RivetResult<DatagramResult> {
        let mut out = DatagramResult {
            messages: Vec::new(),
            sent_count: 0,
            effects: EffectsStatus::None,
        };
        for step in &plan.steps {
            match step {
                DatagramStep::Open => {
                    let sock = self.open(&plan).await?;
                    *self
                        .socket
                        .lock()
                        .map_err(|_| RivetError::internal("udp socket lock poisoned"))? =
                        Some(Arc::new(sock));
                }
                DatagramStep::Send { payload } => {
                    let sock = self.current()?;
                    let r = if sock.connected {
                        sock.udp.send(payload).await
                    } else {
                        let dest = sock.dest.ok_or_else(|| {
                            RivetError::validation("udp.no_peer", "this socket has no default peer")
                        })?;
                        sock.udp.send_to(payload, dest).await
                    };
                    r.map_err(|e| io_err("udp.send_failed", "send", e))?;
                    out.sent_count += 1;
                    out.effects = EffectsStatus::Unknown;
                }
                DatagramStep::SendTo { addr, payload, .. } => {
                    let sock = self.current()?;
                    let dest = addr.ok_or_else(|| {
                        RivetError::internal("udp send_to without a checked address")
                    })?;
                    sock.udp
                        .send_to(payload, dest)
                        .await
                        .map_err(|e| io_err("udp.send_failed", "send_to", e))?;
                    out.sent_count += 1;
                    out.effects = EffectsStatus::Unknown;
                }
                DatagramStep::Receive { timeout_ms } | DatagramStep::ReceiveFrom { timeout_ms } => {
                    out.messages.push(self.receive(&plan, *timeout_ms).await?);
                }
                DatagramStep::Close => self.close(),
            }
        }
        Ok(out)
    }
}
