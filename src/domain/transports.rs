//! Transport vocabulary (PROP-2026-0001 Increments 2, 3, 5, 8): HTTP exchanges,
//! scoped sockets and argv-only processes, plus the port traits the
//! `transports` feature needs. Kept in its own module so the shared
//! `ports.rs` stays untouched; `features/transports/ports.rs` re-exports them.
//!
//! ```text
//!  EvaluatedForm ──infra parse──▶ HttpExchange / SocketPlan / ProcessPlan
//!                                        │
//!                      transports use case (authorize, retry, redirect, decode)
//!                                        │
//!               HttpClient / SocketStream / ProcessRunner / Codec  (infra ports)
//! ```

use super::errors::{ErrorKind, RivetError, RivetResult};
use super::source::SourceSpan;
use super::value::Value;
use async_trait::async_trait;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

// vhco:domain CodecKind { json | text | bytes | xml | form }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecKind {
    Json,
    Text,
    Bytes,
    Xml,
    Form,
}

impl CodecKind {
    pub fn parse(word: &str) -> Option<CodecKind> {
        Some(match word {
            "json" => CodecKind::Json,
            "text" => CodecKind::Text,
            "bytes" => CodecKind::Bytes,
            "xml" => CodecKind::Xml,
            "form" => CodecKind::Form,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CodecKind::Json => "json",
            CodecKind::Text => "text",
            CodecKind::Bytes => "bytes",
            CodecKind::Xml => "xml",
            CodecKind::Form => "form",
        }
    }

    /// Content-Type sent with an encoded request body.
    pub fn content_type(self) -> &'static str {
        match self {
            CodecKind::Json => "application/json",
            CodecKind::Text => "text/plain; charset=utf-8",
            CodecKind::Bytes => "application/octet-stream",
            CodecKind::Xml => "application/xml",
            CodecKind::Form => "application/x-www-form-urlencoded",
        }
    }
}

// vhco:domain CodecInput { kind: CodecKind; bytes?: bytes; value?: Value }
/// One encode (value → bytes) or decode (bytes → value) request.
#[derive(Clone, Debug, PartialEq)]
pub struct CodecInput {
    pub kind: CodecKind,
    pub bytes: Option<Vec<u8>>,
    pub value: Option<Value>,
}

// vhco:domain EffectOrigin { operation_id: string; span?: SourceSpan }
/// Who is asking: carried into every EffectIntent a transport use case builds.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EffectOrigin {
    pub operation_id: String,
    pub span: Option<SourceSpan>,
}

// vhco:domain HttpVersionPolicy { auto | http1 | http2 | http3 | http3_or_http2 | http3_or_http1 | http3_or_auto }
/// `version 1.1|2|3` and `version prefer [3, 2, 1.1]`. The `Http3Or*`
/// variants try HTTP/3 first and fall back only when the H3 attempt failed
/// before any request byte was written (see [`request_unsent`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HttpVersionPolicy {
    /// ALPN h2 → http/1.1 for https; HTTP/1.1 for plain http.
    #[default]
    Auto,
    Http1,
    /// h2 only (ALPN for https, prior knowledge for plain http).
    Http2,
    /// HTTP/3 only (QUIC, ALPN h3); never downgrades.
    Http3,
    /// `prefer [3, 2]`: H3, else h2 only.
    Http3OrHttp2,
    /// `prefer [3, 1.1]`: H3, else HTTP/1.1 only.
    Http3OrHttp1,
    /// `prefer [3, 2, 1.1]`: H3, else h2 → http/1.1.
    Http3OrAuto,
}

impl HttpVersionPolicy {
    /// Does the first attempt use HTTP/3?
    pub fn wants_h3(self) -> bool {
        matches!(
            self,
            HttpVersionPolicy::Http3
                | HttpVersionPolicy::Http3OrHttp2
                | HttpVersionPolicy::Http3OrHttp1
                | HttpVersionPolicy::Http3OrAuto
        )
    }

    /// The TCP policy an H3 preference may fall back to (None = strict).
    pub fn h3_fallback(self) -> Option<HttpVersionPolicy> {
        match self {
            HttpVersionPolicy::Http3OrHttp2 => Some(HttpVersionPolicy::Http2),
            HttpVersionPolicy::Http3OrHttp1 => Some(HttpVersionPolicy::Http1),
            HttpVersionPolicy::Http3OrAuto => Some(HttpVersionPolicy::Auto),
            _ => None,
        }
    }

    /// `version prefer [..]`: an ordered list of "3", "2", "1.1" (or "1").
    /// HTTP/3 may only come first (fallback happens before any request
    /// byte; nothing ever upgrades mid-request); duplicates are refused.
    pub fn from_preference(list: &[String]) -> Option<HttpVersionPolicy> {
        let norm: Vec<&str> = list
            .iter()
            .map(|v| match v.trim() {
                "1" | "1.1" => "1.1",
                other => other,
            })
            .collect();
        Some(match norm.as_slice() {
            ["3"] => HttpVersionPolicy::Http3,
            ["3", "2"] => HttpVersionPolicy::Http3OrHttp2,
            ["3", "1.1"] => HttpVersionPolicy::Http3OrHttp1,
            ["3", "2", "1.1"] => HttpVersionPolicy::Http3OrAuto,
            ["2"] => HttpVersionPolicy::Http2,
            ["1.1"] => HttpVersionPolicy::Http1,
            ["2", "1.1"] => HttpVersionPolicy::Auto,
            _ => return None,
        })
    }
}

/// Marker carried in `details.request_sent` by an HttpClient failure: an
/// error with `request_sent: false` guarantees that no request byte (no
/// HEADERS, no body) reached the wire, so a version fallback cannot replay
/// a request. Any other failure may have delivered the request.
pub fn request_unsent(e: &RivetError) -> bool {
    matches!(e.details.get("request_sent"), Some(Value::Bool(false)))
}

/// `response.version` as scripts see it: 3, 2 (integers) or 1.1 / 1.0.
pub fn version_value(v: &str) -> Value {
    match v {
        "3" => Value::Int(3),
        "2" => Value::Int(2),
        other => match other.parse::<f64>() {
            Ok(f) => Value::Float(f),
            Err(_) => Value::text(other),
        },
    }
}

// vhco:domain StreamMode { sse | jsonl | lines | bytes }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamMode {
    Sse,
    Jsonl,
    Lines,
    Bytes,
}

impl StreamMode {
    pub fn parse(word: &str) -> Option<StreamMode> {
        Some(match word {
            "sse" => StreamMode::Sse,
            "jsonl" | "ndjson" => StreamMode::Jsonl,
            "lines" => StreamMode::Lines,
            "bytes" => StreamMode::Bytes,
            _ => return None,
        })
    }
}

// vhco:domain RetryPolicy { attempts: int; on_status: int[]; base_ms: int; max_ms: int; exponential: bool; jitter: bool }
/// `retry N on status [...] backoff exponential|fixed base "D" max "D" jitter B`.
/// `attempts` counts additional attempts (retry 3 = at most four sends).
#[derive(Clone, Debug, PartialEq)]
pub struct RetryPolicy {
    pub attempts: u32,
    pub on_status: Vec<u16>,
    pub base_ms: u64,
    pub max_ms: u64,
    pub exponential: bool,
    pub jitter: bool,
}

impl RetryPolicy {
    /// Delay before retry number `n` (1-based), capped at `max_ms`.
    pub fn delay_ms(&self, n: u32) -> u64 {
        let d = if self.exponential {
            self.base_ms
                .saturating_mul(1u64 << (n.saturating_sub(1)).min(20))
        } else {
            self.base_ms
        };
        d.min(self.max_ms)
    }
}

// vhco:domain TlsMaterial { server_name?: string; ca_pem?: bytes; cert_pem?: bytes; key_pem?: bytes }
/// TLS inputs already read through the file broker (`tls ca_file|cert_file|key_file`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TlsMaterial {
    pub server_name: Option<String>,
    pub ca_pem: Option<Vec<u8>>,
    pub cert_pem: Option<Vec<u8>>,
    /// Secret: never logged or echoed in errors.
    pub key_pem: Option<Vec<u8>>,
}

// vhco:domain HttpExchange { method: string; url: string; headers: [string,string][]; query: [string,string][]; body?: CodecInput; version: HttpVersionPolicy; decode: CodecKind; accept: int[]; retry?: RetryPolicy; redirect_limit: int; tls: TlsMaterial; stream?: StreamMode; unix_socket?: string; max_body: int; origin: EffectOrigin }
/// One `http METHOD URL … end` (or `with http … as NAME`) after its options
/// were evaluated; file-valued options are already read and authorized.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpExchange {
    /// Upper-case method (GET, POST, …).
    pub method: String,
    /// Absolute URL; interpolated path/query parts were percent-encoded per component.
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub body: Option<CodecInput>,
    pub version: HttpVersionPolicy,
    /// `None` = keep the default for the mode (json-if-possible for streams, text otherwise).
    pub decode: Option<CodecKind>,
    /// Statuses that count as success; empty = any 2xx.
    pub accept: Vec<u16>,
    pub retry: Option<RetryPolicy>,
    /// 0 = redirects are returned, never followed.
    pub redirect_limit: u32,
    pub tls: TlsMaterial,
    pub stream: Option<StreamMode>,
    /// `unix "PATH"`: HTTP over a Unix socket (authorized as allow_unix, not network).
    pub unix_socket: Option<String>,
    pub max_body: u64,
    pub origin: EffectOrigin,
}

// vhco:domain WireTarget { tcp: SocketAddr | unix: string }
#[derive(Clone, Debug, PartialEq)]
pub enum WireTarget {
    /// A broker-checked address: the client dials exactly this, never re-resolving.
    Tcp(SocketAddr),
    Unix(String),
}

// vhco:domain HttpWire { method: string; url: string; headers: [string,string][]; body?: bytes; version: HttpVersionPolicy; tls: TlsMaterial; stream: bool; target: WireTarget; max_body: int }
/// One attempt on the wire; built by the use case after authorization.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpWire {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub version: HttpVersionPolicy,
    pub tls: TlsMaterial,
    pub stream: bool,
    pub target: WireTarget,
    pub max_body: u64,
}

/// Response body as the client delivered it.
pub enum HttpBody {
    Complete(Vec<u8>),
    Stream(Box<dyn ByteStream>),
}

impl std::fmt::Debug for HttpBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpBody::Complete(b) => write!(f, "Complete({} bytes)", b.len()),
            HttpBody::Stream(_) => f.write_str("Stream"),
        }
    }
}

// vhco:domain HttpReply { status: int; headers: [string,string][]; version: string; body: HttpBody }
/// `version` is the protocol actually used: "3", "2", "1.1" or "1.0".
#[derive(Debug)]
pub struct HttpReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub version: String,
    pub body: HttpBody,
}

impl HttpReply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

// vhco:domain HttpResponse { status: int; headers: object; body: Value; version: string }
#[derive(Clone, Debug, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Value,
    pub body: Value,
    pub version: String,
}

impl HttpResponse {
    /// The finite response value `{status, headers, body}` scripts see.
    pub fn to_value(&self) -> Value {
        Value::object([
            ("status", Value::Int(self.status as i64)),
            ("headers", self.headers.clone()),
            ("body", self.body.clone()),
            ("version", version_value(&self.version)),
        ])
    }
}

/// Result of `exchange_http`: the response head and, for `with http … stream`, the live body.
#[derive(Debug)]
pub struct HttpOutcome {
    pub response: HttpResponse,
    pub stream: Option<Box<dyn ByteStream>>,
    /// Decode requested for stream items (json → parse each item when possible).
    pub decode: Option<CodecKind>,
}

impl std::fmt::Debug for dyn ByteStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ByteStream")
    }
}

// vhco:domain SocketScheme { tcp | unix | ws | wss }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketScheme {
    Tcp,
    Unix,
    Ws,
    Wss,
}

impl SocketScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            SocketScheme::Tcp => "tcp",
            SocketScheme::Unix => "unix",
            SocketScheme::Ws => "ws",
            SocketScheme::Wss => "wss",
        }
    }
}

// vhco:domain Framing { newline | length32: { big_endian: bool } | delimiter: bytes | raw }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Framing {
    Newline,
    Length32 { big_endian: bool },
    Delimiter(Vec<u8>),
    Raw,
}

// vhco:domain SocketPlan { scheme: SocketScheme; endpoint: string; host?: string; port?: int; framing: Framing; max_frame: int; tls_requested: bool; tls: TlsMaterial; timeout_ms?: int; reconnect: bool; origin: EffectOrigin }
#[derive(Clone, Debug, PartialEq)]
pub struct SocketPlan {
    pub scheme: SocketScheme,
    /// `host:port`, a socket path, or a ws(s) URL, as written (after interpolation).
    pub endpoint: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub framing: Framing,
    pub max_frame: u64,
    /// `tls true` (or any tls line) on a TCP/Unix socket: Stage C, refused.
    pub tls_requested: bool,
    pub tls: TlsMaterial,
    pub timeout_ms: Option<u64>,
    /// `reconnect N …` requested: Stage C, refused.
    pub reconnect: bool,
    pub origin: EffectOrigin,
}

// vhco:domain Frame { bytes: bytes; text: bool }
/// One framed message; `text` marks WebSocket text frames (or UTF-8 line frames).
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub bytes: Vec<u8>,
    pub text: bool,
}

// vhco:domain SandboxSpec { read: string[]; write: string[]; exec: string[]; deny_read: string[]; deny_write: string[]; network: bool }
/// What a sandboxed child may touch, as absolute directory/file prefixes.
/// Children never get network (ADR-0003 v0.1.0 contract).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SandboxSpec {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub exec: Vec<String>,
    pub deny_read: Vec<String>,
    pub deny_write: Vec<String>,
}

// vhco:domain ProcessPlan { program: string; resolved: string; args: string[]; env: [string,string][]; cwd?: string; stdin?: bytes; timeout_ms: int; accept_exit: int[]; stream?: StreamMode; sandbox?: SandboxSpec; max_output: int; origin: EffectOrigin }
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessPlan {
    /// The binary as written (`/usr/bin/printf`, `./bin/worker`).
    pub program: String,
    /// Absolute path the runner executes (bundle-relative paths resolved against the root).
    pub resolved: String,
    pub args: Vec<String>,
    /// Explicit environment; the child starts from an empty environment.
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
    pub stdin: Option<CodecInput>,
    pub timeout_ms: u64,
    /// Exit codes that count as success (default [0]).
    pub accept_exit: Vec<i64>,
    pub decode_stdout: Option<CodecKind>,
    pub decode_stderr: Option<CodecKind>,
    pub stream: Option<StreamMode>,
    /// `Some` = the child must run inside an OS sandbox (policy.json present).
    pub sandbox: Option<SandboxSpec>,
    pub max_output: u64,
    pub interactive: bool,
    pub origin: EffectOrigin,
}

// vhco:domain ProcessResult { exit_status: int; stdout: bytes; stderr: bytes; duration_ms: int }
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessResult {
    pub exit_status: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration_ms: u64,
}

// vhco:domain ProcessOutcome { finished: Value | streaming: ByteStream }
/// `command …` returns `{stdout, stderr, exit, duration_ms}`; `with command … as p`
/// returns the child's live stdout.
pub enum ProcessOutcome {
    Finished(Value),
    Streaming(Box<dyn ByteStream>),
}

/// Pull-based byte source: an HTTP streaming body or a child's stdout.
#[async_trait]
pub trait ByteStream: Send {
    /// Next chunk; `Ok(None)` at end of stream.
    async fn next_chunk(&mut self) -> RivetResult<Option<Vec<u8>>>;
    /// After end of stream: surface a late failure (e.g. a nonzero exit).
    async fn finish(&mut self) -> RivetResult<()> {
        Ok(())
    }
    /// Release the source (abort the connection, kill and reap the child).
    async fn close(self: Box<Self>) -> RivetResult<()>;
}

/// Broker-dialed HTTP/1.1 + HTTP/2 + HTTP/3 client. It never follows
/// redirects and never resolves names on its own: the use case hands it a
/// checked address (for HTTP/3 the same address, over UDP).
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<IpAddr>>;
    /// One attempt at exactly `wire.version` (`Http3` = QUIC/h3, never a
    /// TCP downgrade). A failure before any request byte was written
    /// carries `details.request_sent: false` ([`request_unsent`]); an H3
    /// peer that does not speak h3 is `http.version_unavailable`.
    async fn send(&self, wire: HttpWire) -> RivetResult<HttpReply>;
    /// Backoff pause between retries (jitter applied by the adapter when asked).
    async fn wait(&self, delay_ms: u64, jitter: bool);
}

/// Value ⇄ bytes codecs (json, text, bytes, form; xml is minimal).
pub trait Codec: Send + Sync {
    fn decode(&self, input: &CodecInput) -> RivetResult<Value>;
    fn encode(&self, input: &CodecInput) -> RivetResult<Vec<u8>>;
}

/// One open, framed duplex connection (TCP, Unix, WebSocket).
#[async_trait]
pub trait SocketConnection: Send {
    async fn send(&mut self, frame: Frame) -> RivetResult<()>;
    /// Next frame; `Ok(None)` = clean end of stream at a frame boundary.
    async fn receive(&mut self) -> RivetResult<Option<Frame>>;
    /// Half-close the sending side (TCP FIN / WebSocket close frame).
    async fn finish_send(&mut self) -> RivetResult<()>;
    async fn close(self: Box<Self>) -> RivetResult<()>;
}

/// Dials sockets at checked addresses.
#[async_trait]
pub trait SocketStream: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<IpAddr>>;
    async fn connect(
        &self,
        plan: &SocketPlan,
        addr: Option<SocketAddr>,
    ) -> RivetResult<Box<dyn SocketConnection>>;
}

/// Spawns argv-only children (optionally inside an OS sandbox) and reaps them.
#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn run(&self, plan: &ProcessPlan) -> RivetResult<ProcessResult>;
    async fn spawn(&self, plan: &ProcessPlan) -> RivetResult<Box<dyn ByteStream>>;
}

/// True when an HTTP method may be replayed automatically (`retry`).
pub fn replay_safe(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "OPTIONS"
    )
}

/// RFC1918, loopback, link-local (incl. 169.254.169.254), CGNAT, broadcast,
/// unspecified, fc00::/7 and fe80::/10 — the `network.deny_private_ranges` set.
/// (Same predicate as `policy.authorize_effect`; features cannot import each other.)
pub fn is_private_ip(ip: IpAddr) -> bool {
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
                || mapped_private(a)
        }
    }
}

fn mapped_private(a: Ipv6Addr) -> bool {
    a.to_ipv4_mapped()
        .is_some_and(|v4| is_private_ip(IpAddr::V4(v4)))
}

// vhco:domain UrlPiece { literal: string | value: string }
/// One piece of a `"…${x}…"` URL template after evaluation.
#[derive(Clone, Debug, PartialEq)]
pub enum UrlPiece {
    Literal(String),
    Value(String),
}

#[derive(Clone, Copy, PartialEq)]
enum UrlPart {
    Prefix,
    Path,
    Query,
    Fragment,
}

/// Component-aware URL interpolation (PROP Increment 1/5): literal text is
/// kept; an interpolated value is percent-encoded for the component it lands
/// in, so `${id}` = `../admin?x=` stays ONE path segment (`..%2Fadmin%3Fx%3D`).
/// Values in the scheme/authority prefix are inserted verbatim (a base URL
/// parameter) and the policy still gates the resulting origin. A value that is
/// exactly `.` or `..` in the path is refused (it would be a dot segment).
pub fn assemble_url(pieces: &[UrlPiece]) -> RivetResult<String> {
    let mut out = String::new();
    let mut part = UrlPart::Prefix;
    let mut seen_scheme = false;
    let advance = |text: &str, out: &mut String, part: &mut UrlPart, seen: &mut bool| {
        for ch in text.chars() {
            out.push(ch);
            match (*part, ch) {
                (UrlPart::Prefix, _) if !*seen => {
                    if out.ends_with("://") {
                        *seen = true;
                    }
                }
                (UrlPart::Prefix, '/') => *part = UrlPart::Path,
                (UrlPart::Prefix | UrlPart::Path, '?') => *part = UrlPart::Query,
                (UrlPart::Prefix | UrlPart::Path | UrlPart::Query, '#') => {
                    *part = UrlPart::Fragment
                }
                _ => {}
            }
        }
    };
    for p in pieces {
        match p {
            UrlPiece::Literal(t) => advance(t, &mut out, &mut part, &mut seen_scheme),
            UrlPiece::Value(v) => match part {
                UrlPart::Prefix => advance(v, &mut out, &mut part, &mut seen_scheme),
                UrlPart::Path => {
                    if v == "." || v == ".." {
                        return Err(RivetError::new(
                            ErrorKind::Validation,
                            "validation.url_segment",
                            format!("an interpolated path value cannot be the dot segment `{v}`"),
                        ));
                    }
                    out.push_str(&percent_encode_component(v));
                }
                UrlPart::Query | UrlPart::Fragment => out.push_str(&percent_encode_component(v)),
            },
        }
    }
    Ok(out)
}

/// Percent-encode everything except RFC 3986 unreserved characters.
pub fn percent_encode_component(v: &str) -> String {
    let mut s = String::with_capacity(v.len());
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(pieces: &[(&str, bool)]) -> RivetResult<String> {
        let p: Vec<UrlPiece> = pieces
            .iter()
            .map(|(t, v)| {
                if *v {
                    UrlPiece::Value(t.to_string())
                } else {
                    UrlPiece::Literal(t.to_string())
                }
            })
            .collect();
        assemble_url(&p)
    }

    #[test]
    fn interpolated_path_value_stays_one_segment() {
        let u = url(&[
            ("https://api.example.com/users/", false),
            ("../admin?x=", true),
        ])
        .unwrap();
        assert_eq!(u, "https://api.example.com/users/..%2Fadmin%3Fx%3D");
        let parsed = url::Url::parse(&u).unwrap();
        assert_eq!(parsed.path(), "/users/..%2Fadmin%3Fx%3D");
        assert_eq!(parsed.query(), None);
    }

    #[test]
    fn query_values_and_base_prefix() {
        let u = url(&[("https://api.example.com/s?q=", false), ("a&b=c", true)]).unwrap();
        assert_eq!(u, "https://api.example.com/s?q=a%26b%3Dc");
        let u = url(&[
            ("http://127.0.0.1:8080", true),
            ("/users/", false),
            ("4 2", true),
        ])
        .unwrap();
        assert_eq!(u, "http://127.0.0.1:8080/users/4%202");
        assert!(url(&[("https://x.test/a/", false), ("..", true)]).is_err());
    }

    #[test]
    fn retry_delays_are_capped() {
        let r = RetryPolicy {
            attempts: 3,
            on_status: vec![503],
            base_ms: 100,
            max_ms: 250,
            exponential: true,
            jitter: false,
        };
        assert_eq!(r.delay_ms(1), 100);
        assert_eq!(r.delay_ms(2), 200);
        assert_eq!(r.delay_ms(3), 250);
        assert!(replay_safe("get") && !replay_safe("POST"));
        assert!(is_private_ip("10.1.2.3".parse().unwrap()));
        assert!(!is_private_ip("93.184.216.34".parse().unwrap()));
    }
}
