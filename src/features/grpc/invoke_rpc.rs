use super::ports::{
    CredentialProvider, GrpcCall, GrpcDriver, GrpcReceiver, GrpcSender, PolicyEvaluator,
};
use crate::domain::auth::{AuthContext, CredentialInput};
use crate::domain::errors::ErrorKind;
use crate::domain::grpc::{
    GrpcCatalog, GrpcDial, GrpcEvent, GrpcMetadataValue, GrpcMethodInfo, GrpcPlan, GrpcResult,
    GrpcTerminal, GrpcUsage, RpcMode, method_ref,
};
use crate::domain::ir::{CompiledProgram, EffectForm, EffectKind, Expr, Rhs, Stmt};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::source::SourceSpan;
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

// vhco:usecase grpc.invoke_rpc(input: GrpcPlan) -> GrpcCall needs GrpcDriver, PolicyEvaluator, CredentialProvider
// vhco:label Invoke rpc
// vhco:about Checks one evaluated grpc form against the pinned descriptor catalog (method, mode, message, metadata), authorizes allow_grpc CONNECTOR/Service/Method plus allow_network on the endpoint origin (and every TLS file read) before any I/O, refuses private resolved addresses, dials the checked address through GrpcDriver and returns a scope-owned call that enforces cardinality and maps the final gRPC status.
// vhco:example input={connector:"users", method:"GetUser", usage:"one_shot", message:{id:"42"}} => { "message": {"id": "42", "name": "Ada"}, "status": "OK" }
pub async fn invoke_rpc(
    plan: GrpcPlan,
    policy: &dyn PolicyEvaluator,
    driver: &dyn GrpcDriver,
    credentials: Option<&dyn CredentialProvider>,
) -> RivetResult<Box<dyn GrpcCall>> {
    let span = plan.span.clone();
    // vhco:todo validate_method -- resolve CONNECTOR.Method in the pinned catalog (unknown → grpc.unknown_method); one-shot `grpc` needs a unary method and `with grpc` a streaming one (grpc.mode); a `message` option on a client/bidi-streaming method is grpc.mode; metadata keys must be lowercase [0-9a-z-_.], never grpc-*/reserved HTTP/2 keys, binary values only on `-bin` keys and ASCII values printable (grpc.metadata); then authorize allow_grpc call Logical(connector/Service/Method), allow_network connect Url(scheme://host:port) and allow_read read for each tls ca/cert/key file, all before any I/O
    // vhco:step lookup driver.catalog -- connector + method from the descriptor catalog (never reflection)
    let catalog = driver.catalog();
    let connector = catalog.connector(&plan.connector).ok_or_else(|| {
        RivetError::validation(
            "grpc.unknown_connector",
            format!("no grpc connector `{}`", plan.connector),
        )
        .with_span(span.clone())
    })?;
    let info = catalog
        .method(&plan.connector, &plan.method)
        .ok_or_else(|| {
            RivetError::validation(
                "grpc.unknown_method",
                format!(
                    "`{}` has no method `{}` in service {}",
                    plan.connector, plan.method, connector.service
                ),
            )
            .with_span(span.clone())
        })?
        .clone();
    // vhco:step mode check_usage -- the source form must match the descriptor cardinality
    check_usage(&info, plan.usage, plan.message.is_some())
        .map_err(|e| e.with_span(span.clone()))?;
    // vhco:step metadata check_metadata -- reject reserved and malformed metadata
    check_metadata(&plan.metadata, plan.auth.is_some()).map_err(|e| e.with_span(span.clone()))?;
    // vhco:error auth_unsupported -- `auth PROFILE account A` on a host without a credential provider => unsupported.auth returns before any effect
    if let (Some((profile, _)), None) = (&plan.auth, credentials) {
        return Err(RivetError::unsupported(
            "unsupported.auth",
            format!(
                "OAuth profile `{profile}` cannot be attached: this host has no credential provider"
            ),
        )
        .with_span(span));
    }
    // vhco:step input driver.validate_input -- ProtoJSON of the `message` option is checked against the input type before dialing
    let first = if info.mode.client_streams() {
        None
    } else {
        let m = plan.message.clone().unwrap_or(Value::Object(Vec::new()));
        driver
            .validate_input(&info, &m)
            .map_err(|e| e.with_span(span.clone()))?;
        Some(m)
    };
    let endpoint = parse_endpoint(&connector.endpoint).map_err(|e| e.with_span(span.clone()))?;
    let authorize = |capability: Capability, verb: AccessVerb, target: EffectTarget| {
        authorize(policy, &plan, capability, verb, target)
    };
    // vhco:step method policy.evaluate -- allow_grpc call CONNECTOR/Service/Method
    // vhco:error permission_denied -- a missing allow_grpc, allow_network or tls file grant => permission.denied (exit 3) returns before dialing
    authorize(
        Capability::Grpc,
        AccessVerb::Call,
        EffectTarget::Logical(info.target()),
    )?;
    // vhco:step origin policy.evaluate -- allow_network connect on the endpoint origin
    authorize(
        Capability::Network,
        AccessVerb::Connect,
        EffectTarget::Url(endpoint.origin()),
    )?;
    if endpoint.tls {
        // vhco:step tls_files policy.evaluate -- every tls ca_file/cert_file/key_file is an allow_read read before connecting
        for f in [
            &connector.tls.ca_file,
            &connector.tls.cert_file,
            &connector.tls.key_file,
        ]
        .into_iter()
        .flatten()
        {
            authorize(
                Capability::Read,
                AccessVerb::Read,
                EffectTarget::Path(f.clone()),
            )?;
        }
    }
    // vhco:todo exchange_messages -- resolve the host (IP literals are used as written; names through GrpcDriver.resolve); with deny_private_ranges a private/loopback/link-local address reached through a name must itself be granted literally or the call is refused; dial only checked addresses (in resolver order, the first that accepts TCP) with timeout = min(`timeout`, remaining request deadline); unary/server-streaming calls send their single message and half-close at once, client/bidi calls stream later rpc.send values; the returned call keeps send and receive halves under separate locks so a bidi send task and receive task progress independently
    // vhco:step resolve driver.resolve -- candidate addresses for a host name
    let candidates: Vec<IpAddr> = match endpoint.host.parse::<IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => driver
            .resolve(&endpoint.host, endpoint.port)
            .await
            .map_err(|e| e.with_span(span.clone()))?,
    };
    // vhco:step private check_private -- a private address behind a DNS name needs its own literal grant
    let literal = endpoint.host.parse::<IpAddr>().is_ok();
    let mut addresses = Vec::new();
    let mut refusal = None;
    for ip in candidates {
        if !literal && policy.policy().network.deny_private_ranges && is_private(ip) {
            let host = match ip {
                IpAddr::V4(a) => a.to_string(),
                IpAddr::V6(a) => format!("[{a}]"),
            };
            if let Err(e) = authorize(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url(format!("{}://{host}:{}", endpoint.scheme, endpoint.port)),
            ) {
                refusal.get_or_insert(e);
                continue;
            }
        }
        addresses.push(SocketAddr::new(ip, endpoint.port));
    }
    // vhco:error private_address -- every resolved address is private and none is granted literally => permission.denied before dialing
    if addresses.is_empty() {
        return Err(refusal.unwrap_or_else(|| {
            RivetError::new(
                ErrorKind::Dns,
                "dns.no_address",
                format!("{} resolved to no address", endpoint.host),
            )
            .with_span(span.clone())
        }));
    }
    // vhco:step auth credentials.acquire -- after the method, origin and TLS permits: an origin-bound lease (allow_auth use, allow_credentials, token endpoint) becomes `authorization: Bearer` metadata; the endpoint origin must be one of the profile's resource_origins
    let mut metadata = plan.metadata.clone();
    if let (Some((profile, account)), Some(provider)) = (&plan.auth, credentials) {
        let lease = provider
            .acquire(
                CredentialInput {
                    profile: profile.clone(),
                    account: account.clone(),
                    origin: endpoint.origin(),
                    audience: None,
                    scopes: Vec::new(),
                    context: AuthContext {
                        principal: plan.principal.clone(),
                        operation_id: plan.operation_id.clone(),
                        deadline_ms: plan.deadline_ms,
                    },
                },
                policy,
            )
            .await
            .map_err(|e| e.with_span(span.clone()))?;
        metadata.push((
            "authorization".into(),
            GrpcMetadataValue::Ascii(format!("Bearer {}", lease.handle.expose())),
        ));
    }
    let timeout_ms = plan
        .timeout_ms
        .map_or(plan.deadline_ms, |t| t.min(plan.deadline_ms))
        .max(1);
    // vhco:step dial driver.invoke -- start the call on the checked address; errors keep their connection/tls kinds
    let (mut tx, rx) = driver
        .invoke(GrpcDial {
            method: info.clone(),
            endpoint: endpoint.origin(),
            addresses,
            tls: connector.tls.clone(),
            metadata,
            timeout_ms,
        })
        .await
        .map_err(|e| e.with_span(span.clone()))?;
    // vhco:step first tx.send -- unary and server-streaming calls send their one message and half-close
    if let Some(m) = first {
        tx.send(m).await.map_err(|e| e.with_span(span.clone()))?;
        tx.finish().await;
    }
    // vhco:todo validate_terminal -- iteration ends normally only after final OK trailers; any non-OK status (also after emitted messages) becomes one terminal error mapped CANCELLED→cancelled, DEADLINE_EXCEEDED→timeout, NOT_FOUND→not_found, INVALID_ARGUMENT→validation, PERMISSION_DENIED→permission, UNAUTHENTICATED→auth.login_required (conflict), RESOURCE_EXHAUSTED→limit, UNIMPLEMENTED→unsupported, others grpc.<code>, keeping grpc_status, grpc_code, message and data_count in details; rpc.result needs exactly one message plus OK; sends after half-close or peer completion fail grpc.input_closed; scope exit cancels an unfinished call
    let finished = !info.mode.client_streams();
    Ok(Box::new(RpcCall {
        info,
        send: tokio::sync::Mutex::new(SendSide { tx, finished }),
        recv: tokio::sync::Mutex::new(RecvSide {
            rx,
            terminal: None,
            count: 0,
            message: None,
            failure: None,
        }),
        peer_done: AtomicBool::new(false),
    }))
}

fn authorize(
    policy: &dyn PolicyEvaluator,
    plan: &GrpcPlan,
    capability: Capability,
    verb: AccessVerb,
    target: EffectTarget,
) -> RivetResult<()> {
    let permit = policy.evaluate(&EffectIntent {
        capability,
        verb,
        target: target.clone(),
        operation_id: plan.operation_id.clone(),
        effect_id: None,
        span: plan.span.clone(),
    });
    if permit.decision == Decision::Denied {
        return Err(RivetError::permission(format!(
            "{} {} {} denied: {}",
            capability.as_str(),
            verb.as_str(),
            target.as_str(),
            permit.rule
        ))
        .with_span(plan.span.clone())
        .with_details(Value::object([
            ("capability", Value::text(capability.as_str())),
            ("access", Value::text(verb.as_str())),
            ("target", Value::text(target.as_str())),
        ])));
    }
    Ok(())
}

/// Connector endpoint split into the parts policy and dialing need.
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Endpoint {
    /// `https://host:port` with the default port explicit (the allow_network target).
    pub fn origin(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("{}://{host}:{}", self.scheme, self.port)
    }
}

/// `https://host[:port]` (TLS, HTTP/2) or development `http://host[:port]` (h2c).
pub fn parse_endpoint(text: &str) -> RivetResult<Endpoint> {
    let bad = |m: String| RivetError::validation("grpc.endpoint", m);
    let u = url::Url::parse(text).map_err(|e| bad(format!("endpoint `{text}`: {e}")))?;
    let tls = match u.scheme() {
        "https" => true,
        "http" => false,
        other => {
            return Err(RivetError::unsupported(
                "unsupported.transport",
                format!(
                    "gRPC endpoint scheme `{other}` is not supported (use https://, or http:// for development)"
                ),
            ));
        }
    };
    if !(u.path().is_empty() || u.path() == "/") || u.query().is_some() {
        return Err(bad(format!(
            "endpoint `{text}` must be an origin (scheme://host:port) without a path"
        )));
    }
    let host = match u.host() {
        Some(url::Host::Ipv6(a)) => a.to_string(),
        Some(h) => h.to_string(),
        None => return Err(bad(format!("endpoint `{text}` has no host"))),
    };
    Ok(Endpoint {
        scheme: u.scheme().to_string(),
        host,
        port: u.port_or_known_default().unwrap_or(443),
        tls,
    })
}

/// RFC1918, loopback, link-local (incl. 169.254.169.254), CGNAT, fc00::/7, fe80::/10, unspecified.
/// (Same predicate as the policy broker's; features may not import each other.)
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            a.is_private()
                || a.is_loopback()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || (a.octets()[0] == 100 && (a.octets()[1] & 0xc0) == 64)
        }
        IpAddr::V6(a) => {
            a.is_loopback()
                || a.is_unspecified()
                || (a.segments()[0] & 0xfe00) == 0xfc00
                || (a.segments()[0] & 0xffc0) == 0xfe80
                || a.to_ipv4_mapped()
                    .is_some_and(|v4| is_private(IpAddr::V4(v4)))
        }
    }
}

fn mode_error(msg: String) -> RivetError {
    RivetError::validation("grpc.mode", msg)
}

/// Descriptor cardinality versus the source form (REF S110–S113).
pub fn check_usage(info: &GrpcMethodInfo, usage: GrpcUsage, has_message: bool) -> RivetResult<()> {
    let name = format!("{}.{}", info.connector, info.method);
    match (usage, info.mode) {
        (GrpcUsage::OneShot, RpcMode::Unary) => {}
        (GrpcUsage::Scoped, RpcMode::Unary) => {
            return Err(mode_error(format!(
                "`{name}` is unary; call it as `response = grpc {name} … end`"
            )));
        }
        (GrpcUsage::OneShot, mode) => {
            return Err(mode_error(format!(
                "`{name}` is {}; open it with `with grpc {name} as rpc … end`",
                mode.as_str()
            )));
        }
        (GrpcUsage::Scoped, _) => {}
    }
    if has_message && info.mode.client_streams() {
        return Err(mode_error(format!(
            "`{name}` is {}: send messages with `rpc.send V`, not a `message` option",
            info.mode.as_str()
        )));
    }
    Ok(())
}

const RESERVED_METADATA: &[&str] = &[
    "content-type",
    "te",
    "host",
    "user-agent",
    "connection",
    "transfer-encoding",
    "upgrade",
];

/// `metadata K V` rules: lowercase legal keys, no grpc-* or HTTP/2-reserved
/// keys, binary only on `-bin`, printable ASCII otherwise, no authorization
/// entry that would conflict with an `auth` attachment.
pub fn check_metadata(entries: &[(String, GrpcMetadataValue)], has_auth: bool) -> RivetResult<()> {
    let bad = |m: String| RivetError::validation("grpc.metadata", m);
    for (k, v) in entries {
        if k.is_empty()
            || !k
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b"-_.".contains(&b))
        {
            return Err(bad(format!(
                "metadata key `{k}` must be lowercase letters, digits, `-`, `_` or `.`"
            )));
        }
        if k.starts_with("grpc-") || RESERVED_METADATA.contains(&k.as_str()) {
            return Err(bad(format!("metadata key `{k}` is reserved")));
        }
        if has_auth && (k == "authorization" || k == "proxy-authorization") {
            return Err(bad(format!(
                "metadata `{k}` conflicts with the `auth` attachment"
            )));
        }
        match v {
            GrpcMetadataValue::Binary(_) if !k.ends_with("-bin") => {
                return Err(bad(format!(
                    "binary metadata needs a key ending in `-bin` (got `{k}`)"
                )));
            }
            GrpcMetadataValue::Ascii(_) if k.ends_with("-bin") => {
                return Err(bad(format!(
                    "`{k}` is a binary key; write `metadata \"{k}\" bytes VALUE`"
                )));
            }
            GrpcMetadataValue::Ascii(s) if !s.bytes().all(|b| (0x20..0x7f).contains(&b)) => {
                return Err(bad(format!(
                    "metadata `{k}` must be printable ASCII; use a `-bin` key for other bytes"
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Map a non-OK final status to the Rivet error registry (Increment 13).
pub fn status_error(info: &GrpcMethodInfo, terminal: &GrpcTerminal, data_count: u64) -> RivetError {
    let s = &terminal.status;
    let (kind, code) = match s.code {
        1 => (ErrorKind::Cancelled, "grpc.cancelled".to_string()),
        4 => (ErrorKind::Timeout, "grpc.deadline_exceeded".to_string()),
        5 => (ErrorKind::NotFound, "grpc.not_found".to_string()),
        3 => (ErrorKind::Validation, "grpc.invalid_argument".to_string()),
        7 => (ErrorKind::Permission, "grpc.permission_denied".to_string()),
        16 => (ErrorKind::Conflict, "auth.login_required".to_string()),
        8 => (ErrorKind::Limit, "grpc.resource_exhausted".to_string()),
        12 => (ErrorKind::Unsupported, "grpc.unimplemented".to_string()),
        _ => (
            ErrorKind::Protocol,
            format!("grpc.{}", s.name.to_ascii_lowercase()),
        ),
    };
    let message: String = s.message.chars().take(512).collect();
    let mut e = RivetError::new(
        kind,
        code,
        format!(
            "gRPC {} ended with {} ({}){}{}",
            info.path(),
            s.name,
            s.code,
            if message.is_empty() { "" } else { ": " },
            message
        ),
    )
    .with_details(Value::object([
        ("grpc_status", Value::Int(s.code as i64)),
        ("grpc_code", Value::text(&s.name)),
        ("grpc_message", Value::text(message)),
        ("method", Value::text(info.target())),
        ("data_count", Value::Int(data_count as i64)),
        ("trailers", terminal.trailers.clone()),
    ]));
    if data_count > 0 {
        e = e.with_effects(crate::domain::EffectsStatus::Partial);
    }
    e
}

struct SendSide {
    tx: Box<dyn GrpcSender>,
    finished: bool,
}

struct RecvSide {
    rx: Box<dyn GrpcReceiver>,
    terminal: Option<GrpcTerminal>,
    count: u64,
    /// First response message kept for `rpc.result`.
    message: Option<Value>,
    /// A transport/decode failure; repeated on every later read.
    failure: Option<RivetError>,
}

/// Scope-owned call with cardinality and status rules applied.
struct RpcCall {
    info: GrpcMethodInfo,
    send: tokio::sync::Mutex<SendSide>,
    recv: tokio::sync::Mutex<RecvSide>,
    peer_done: AtomicBool,
}

impl RpcCall {
    fn input_closed(&self) -> RivetError {
        RivetError::new(
            ErrorKind::Conflict,
            "grpc.input_closed",
            format!(
                "{}: input is closed (finish_send was called or the server already finished)",
                self.info.path()
            ),
        )
    }

    /// Next event from the wire; `Ok(None)` once the terminal status is known.
    async fn pull(&self, r: &mut RecvSide) -> RivetResult<Option<Value>> {
        if let Some(e) = &r.failure {
            return Err(e.clone());
        }
        if r.terminal.is_some() {
            return Ok(None);
        }
        match r.rx.next_event().await {
            Ok(GrpcEvent::Message(v)) => {
                r.count += 1;
                Ok(Some(v))
            }
            Ok(GrpcEvent::End(t)) => {
                r.terminal = Some(t);
                self.peer_done.store(true, Ordering::SeqCst);
                Ok(None)
            }
            Err(e) => {
                r.rx.cancel();
                self.peer_done.store(true, Ordering::SeqCst);
                r.failure = Some(e.clone());
                Err(e)
            }
        }
    }

    fn finish_result(&self, r: &RecvSide, message: Option<Value>) -> RivetResult<GrpcResult> {
        let t = r
            .terminal
            .as_ref()
            .ok_or_else(|| RivetError::internal("gRPC call has no terminal status"))?;
        if !t.status.is_ok() {
            return Err(status_error(&self.info, t, r.count));
        }
        Ok(GrpcResult {
            message,
            initial_metadata: t.initial_metadata.clone(),
            trailers: t.trailers.clone(),
            status: t.status.clone(),
            data_count: r.count,
        })
    }

    async fn half_close(&self) {
        let mut s = self.send.lock().await;
        if !s.finished {
            s.finished = true;
            s.tx.finish().await;
        }
    }
}

#[async_trait]
impl GrpcCall for RpcCall {
    async fn send(&self, message: Value) -> RivetResult<()> {
        if !self.info.mode.client_streams() {
            return Err(mode_error(format!(
                "`{}.{}` is {}: it takes one `message` option and has no rpc.send",
                self.info.connector,
                self.info.method,
                self.info.mode.as_str()
            )));
        }
        let mut s = self.send.lock().await;
        if s.finished || self.peer_done.load(Ordering::SeqCst) {
            return Err(self.input_closed());
        }
        s.tx.send(message).await
    }

    async fn finish_send(&self) -> RivetResult<()> {
        if !self.info.mode.client_streams() {
            return Err(mode_error(format!(
                "`{}.{}` is {}: there is no input stream to finish",
                self.info.connector,
                self.info.method,
                self.info.mode.as_str()
            )));
        }
        self.half_close().await;
        Ok(())
    }

    async fn next(&self) -> RivetResult<Option<Value>> {
        if !self.info.mode.server_streams() {
            return Err(mode_error(format!(
                "`{}.{}` is {}: read its response with rpc.result",
                self.info.connector,
                self.info.method,
                self.info.mode.as_str()
            )));
        }
        let mut r = self.recv.lock().await;
        match self.pull(&mut r).await? {
            Some(v) => Ok(Some(v)),
            None => self.finish_result(&r, None).map(|_| None),
        }
    }

    async fn result(&self) -> RivetResult<GrpcResult> {
        if self.info.mode.server_streams() {
            return Err(mode_error(format!(
                "`{}.{}` is {}: iterate it with `for message in rpc` and read rpc.completion",
                self.info.connector,
                self.info.method,
                self.info.mode.as_str()
            )));
        }
        // The response only arrives after the input is complete.
        self.half_close().await;
        let mut r = self.recv.lock().await;
        while let Some(v) = self.pull(&mut r).await? {
            if r.message.is_some() {
                r.rx.cancel();
                let e = RivetError::new(
                    ErrorKind::Protocol,
                    "grpc.cardinality",
                    format!(
                        "{} returned more than one response message",
                        self.info.path()
                    ),
                );
                r.failure = Some(e.clone());
                return Err(e);
            }
            r.message = Some(v);
        }
        let msg = r.message.clone();
        let result = self.finish_result(&r, msg)?;
        if result.message.is_none() {
            return Err(RivetError::new(
                ErrorKind::Protocol,
                "grpc.missing_message",
                format!(
                    "{} finished OK without a response message",
                    self.info.path()
                ),
            ));
        }
        Ok(result)
    }

    async fn completion(&self) -> RivetResult<GrpcResult> {
        if self.info.mode.client_streams() {
            self.half_close().await;
        }
        let mut r = self.recv.lock().await;
        // Unread messages are drained (and counted) so the trailers can be read.
        while self.pull(&mut r).await?.is_some() {}
        self.finish_result(&r, None)
    }

    async fn close(&self) {
        self.half_close().await;
        let mut r = self.recv.lock().await;
        if r.terminal.is_none() && r.failure.is_none() {
            r.rx.cancel();
        }
    }
}

/// Load-time check of every `grpc` form in the program against the pinned
/// descriptor catalog: unknown connector/method and wrong mode usage fail the
/// bundle load before anything dials.
pub fn check_program(program: &CompiledProgram, catalog: &GrpcCatalog) -> RivetResult<()> {
    let mut errors = Vec::new();
    for op in &program.operations {
        walk(&op.body, catalog, &mut errors);
    }
    let mut it = errors.into_iter();
    match it.next() {
        None => Ok(()),
        Some(mut first) => {
            first.suppressed.extend(it);
            Err(first)
        }
    }
}

fn form_method(
    form: &EffectForm,
    catalog: &GrpcCatalog,
    errors: &mut Vec<RivetError>,
) -> Option<GrpcMethodInfo> {
    let span = form
        .head
        .first()
        .map(|a| a.span().clone())
        .or(Some(form.span.clone()));
    let Some((connector, method)) = method_ref(&form.head) else {
        errors.push(
            RivetError::validation("grpc.form", "expected `grpc CONNECTOR.Method`").with_span(span),
        );
        return None;
    };
    let Some(c) = catalog.connector(&connector) else {
        errors.push(
            RivetError::validation(
                "grpc.unknown_connector",
                format!("`{connector}` is not a grpc connector"),
            )
            .with_span(span),
        );
        return None;
    };
    match catalog.method(&connector, &method) {
        Some(m) => Some(m.clone()),
        None => {
            errors.push(
                RivetError::validation(
                    "grpc.unknown_method",
                    format!(
                        "service {} in {} has no method `{method}`",
                        c.service, c.descriptor
                    ),
                )
                .with_span(span),
            );
            None
        }
    }
}

fn check_form(
    form: &EffectForm,
    usage: GrpcUsage,
    catalog: &GrpcCatalog,
    errors: &mut Vec<RivetError>,
) -> Option<GrpcMethodInfo> {
    let info = form_method(form, catalog, errors)?;
    if let Err(e) = check_usage(&info, usage, form.option("message").is_some()) {
        errors.push(e.with_span(Some(form.span.clone())));
        return None;
    }
    Some(info)
}

fn walk(body: &[Stmt], catalog: &GrpcCatalog, errors: &mut Vec<RivetError>) {
    for s in body {
        match s {
            Stmt::Assign { rhs, .. }
            | Stmt::Return { value: rhs, .. }
            | Stmt::Yield { value: rhs, .. }
            | Stmt::Emit { value: rhs, .. } => walk_rhs(rhs, catalog, errors),
            Stmt::Effect { form, .. } if form.kind == EffectKind::Grpc => {
                check_form(form, GrpcUsage::OneShot, catalog, errors);
            }
            Stmt::With {
                form, bind, body, ..
            } if form.kind == EffectKind::Grpc => {
                if let (Some(info), Some(name)) =
                    (check_form(form, GrpcUsage::Scoped, catalog, errors), bind)
                {
                    check_handle_uses(body, name, &info, errors);
                }
                walk(body, catalog, errors);
            }
            _ => {
                for child in children(s) {
                    walk(child, catalog, errors);
                }
            }
        }
    }
}

fn walk_rhs(rhs: &Rhs, catalog: &GrpcCatalog, errors: &mut Vec<RivetError>) {
    match rhs {
        Rhs::Effect(form) if form.kind == EffectKind::Grpc => {
            check_form(form, GrpcUsage::OneShot, catalog, errors);
        }
        Rhs::Map { body, .. } | Rhs::Poll { body, .. } => walk(body, catalog, errors),
        _ => {}
    }
}

fn children(s: &Stmt) -> Vec<&[Stmt]> {
    match s {
        Stmt::If {
            then, otherwise, ..
        } => vec![then, otherwise],
        Stmt::Try { body, handler, .. } => vec![body, handler],
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Iterate { body, .. }
        | Stmt::Scope { body, .. }
        | Stmt::With { body, .. } => vec![body],
        Stmt::Concurrent { tasks, .. } => tasks.iter().map(|(_, b)| b.as_slice()).collect(),
        _ => vec![],
    }
}

/// Uses of the bound handle inside its `with` body must fit the mode:
/// `rpc.send`/`rpc.finish_send` need client streaming, `for m in rpc` needs
/// server streaming, `rpc.result` needs client streaming, `rpc.completion` any.
fn check_handle_uses(
    body: &[Stmt],
    name: &str,
    info: &GrpcMethodInfo,
    errors: &mut Vec<RivetError>,
) {
    let method = format!("{}.{}", info.connector, info.method);
    let mode = info.mode.as_str();
    let member = |call: &crate::domain::ir::MemberCall, errors: &mut Vec<RivetError>| {
        if call.object.len() != 1 || call.object[0] != name {
            return;
        }
        let msg = match call.method.as_str() {
            "send" | "finish_send" if info.mode.client_streams() => return,
            "send" | "finish_send" => format!(
                "`{method}` is {mode}: `{name}.{}` needs a client- or bidi-streaming method",
                call.method
            ),
            other => format!(
                "a gRPC call has no `{name}.{other}`; use send, finish_send, result or completion"
            ),
        };
        errors.push(mode_error(msg).with_span(Some(call.span.clone())));
    };
    let paths = |exprs: Vec<&Expr>, span: &SourceSpan, errors: &mut Vec<RivetError>| {
        let mut all = Vec::new();
        for e in exprs {
            e.paths(&mut all);
        }
        for p in all {
            if p.len() < 2 || p[0] != name {
                continue;
            }
            let msg = match p[1].as_str() {
                "completion" => continue,
                "result" if info.mode == RpcMode::ClientStream => continue,
                "result" => format!(
                    "`{method}` is {mode}: iterate it with `for message in {name}` and read `{name}.completion`"
                ),
                other => format!(
                    "a gRPC call has no property `{name}.{other}`; use result or completion"
                ),
            };
            errors.push(mode_error(msg).with_span(Some(span.clone())));
        }
    };
    fn rhs_exprs(r: &Rhs) -> Vec<&Expr> {
        match r {
            Rhs::Expr(e) => vec![e],
            Rhs::Map { iter, .. } => vec![iter],
            _ => vec![],
        }
    }
    for s in body {
        match s {
            Stmt::Member { call, .. } => member(call, errors),
            Stmt::Assign { rhs, span, .. }
            | Stmt::Return {
                value: rhs, span, ..
            }
            | Stmt::Yield {
                value: rhs, span, ..
            }
            | Stmt::Emit {
                value: rhs, span, ..
            } => {
                if let Rhs::Member(call) = rhs {
                    member(call, errors);
                }
                paths(rhs_exprs(rhs), span, errors);
            }
            Stmt::For { iter, span, .. } => {
                if matches!(iter, Expr::Path(p, _) if p.len() == 1 && p[0] == name)
                    && !info.mode.server_streams()
                {
                    errors.push(
                        mode_error(format!(
                            "`{method}` is {mode}: it returns one message; read it with `{name}.result`"
                        ))
                        .with_span(Some(span.clone())),
                    );
                }
                paths(vec![iter], span, errors);
            }
            Stmt::If { cond, span, .. } | Stmt::While { cond, span, .. } => {
                paths(vec![cond], span, errors)
            }
            Stmt::Append { value, span, .. } => paths(vec![value], span, errors),
            Stmt::Call { expr, span } => paths(vec![expr], span, errors),
            _ => {}
        }
        for child in children(s) {
            // A nested `with` rebinding the same name shadows it.
            if matches!(s, Stmt::With { bind: Some(b), .. } if b == name) {
                continue;
            }
            check_handle_uses(child, name, info, errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::grpc::GrpcStatus;

    fn info(mode: RpcMode) -> GrpcMethodInfo {
        GrpcMethodInfo {
            connector: "users".into(),
            service: "example.Users".into(),
            method: "M".into(),
            mode,
            input_type: "example.In".into(),
            output_type: "example.Out".into(),
        }
    }

    // vhco:test grpc.invoke_rpc -- descriptor cardinality decides the legal source form and the message option
    #[test]
    fn usage_follows_cardinality() {
        assert!(check_usage(&info(RpcMode::Unary), GrpcUsage::OneShot, true).is_ok());
        assert_eq!(
            check_usage(&info(RpcMode::Unary), GrpcUsage::Scoped, true)
                .unwrap_err()
                .code,
            "grpc.mode"
        );
        assert!(check_usage(&info(RpcMode::ServerStream), GrpcUsage::OneShot, true).is_err());
        assert!(check_usage(&info(RpcMode::ServerStream), GrpcUsage::Scoped, true).is_ok());
        assert!(check_usage(&info(RpcMode::ClientStream), GrpcUsage::Scoped, true).is_err());
        assert!(check_usage(&info(RpcMode::Bidi), GrpcUsage::Scoped, false).is_ok());
    }

    // vhco:test grpc.invoke_rpc -- reserved, malformed and conflicting metadata is rejected before any I/O
    #[test]
    fn metadata_rules() {
        let a = |k: &str, v: &str| (k.to_string(), GrpcMetadataValue::Ascii(v.into()));
        assert!(check_metadata(&[a("x-request-id", "r1"), a("x-request-id", "r2")], false).is_ok());
        assert!(check_metadata(&[a("grpc-timeout", "1S")], false).is_err());
        assert!(check_metadata(&[a("X-Upper", "v")], false).is_err());
        assert!(check_metadata(&[a("authorization", "Bearer x")], true).is_err());
        assert!(check_metadata(&[a("trace-bin", "x")], false).is_err());
        assert!(
            check_metadata(
                &[("trace-bin".into(), GrpcMetadataValue::Binary(vec![0, 1]))],
                false
            )
            .is_ok()
        );
        assert!(
            check_metadata(
                &[("trace".into(), GrpcMetadataValue::Binary(vec![0]))],
                false
            )
            .is_err()
        );
    }

    // vhco:test grpc.invoke_rpc -- final gRPC codes map onto the error registry and keep the numeric status
    #[test]
    fn status_mapping() {
        let t = |code| GrpcTerminal {
            status: GrpcStatus::from_code(code, "boom"),
            initial_metadata: Value::Object(vec![]),
            trailers: Value::Object(vec![]),
        };
        let i = info(RpcMode::ServerStream);
        let cases = [
            (1, ErrorKind::Cancelled, "grpc.cancelled"),
            (4, ErrorKind::Timeout, "grpc.deadline_exceeded"),
            (5, ErrorKind::NotFound, "grpc.not_found"),
            (3, ErrorKind::Validation, "grpc.invalid_argument"),
            (7, ErrorKind::Permission, "grpc.permission_denied"),
            (16, ErrorKind::Conflict, "auth.login_required"),
            (8, ErrorKind::Limit, "grpc.resource_exhausted"),
            (12, ErrorKind::Unsupported, "grpc.unimplemented"),
            (14, ErrorKind::Protocol, "grpc.unavailable"),
            (13, ErrorKind::Protocol, "grpc.internal"),
        ];
        for (code, kind, name) in cases {
            let e = status_error(&i, &t(code), 1);
            assert_eq!((e.kind, e.code.as_str()), (kind, name));
            assert_eq!(e.details.get("grpc_status"), Some(&Value::Int(code as i64)));
        }
        assert_eq!(status_error(&i, &t(14), 1).exit_code(), 5);
    }

    // vhco:test grpc.invoke_rpc -- endpoints normalize to the allow_network origin; other schemes are unsupported
    #[test]
    fn endpoint_origin() {
        let e = parse_endpoint("https://users.example.com").unwrap();
        assert_eq!(e.origin(), "https://users.example.com:443");
        assert!(e.tls);
        let e = parse_endpoint("http://127.0.0.1:5000").unwrap();
        assert_eq!(e.origin(), "http://127.0.0.1:5000");
        assert!(!e.tls);
        assert_eq!(
            parse_endpoint("grpc://x:1").unwrap_err().code,
            "unsupported.transport"
        );
        assert!(parse_endpoint("https://x:1/path").is_err());
    }
}
