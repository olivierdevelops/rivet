//! HTTP/3 leg of the HTTP client (TASK-044, PROP-2026-0001 Increment 11).
//!
//! ```text
//!  HyperClient::send(wire{version: Http3, target: Tcp(checked ip:port)})
//!        │  https only, never a Unix socket         ─┐
//!        ▼                                            │ any failure here:
//!  quinn Endpoint (ephemeral UDP) ─ QUIC v1 handshake │ details.request_sent = false
//!        │  ALPN "h3" only, 0-RTT off, same           │ (no peer h3 → http.version_unavailable,
//!        │  checked address, bounded wait             │  certificate → tls.handshake)
//!  h3::client::new (SETTINGS on the control stream) ─┘
//!        │
//!  send_request(HEADERS) → send_data(body) → finish  ─┐ from here on the request may have
//!  recv_response → recv_data …                        ─┘ reached the server: request_sent = true
//! ```
//!
//! The caller (the `transports.exchange_http` use case) decides whether a
//! `request_sent = false` failure may fall back to h2/h1; this module never
//! downgrades on its own and never dials anything but `wire.target`.

use super::net_tls::{client_config, server_name};
use crate::domain::transports::{ByteStream, HttpWire, WireTarget};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use bytes::{Buf, Bytes};
use quinn::crypto::rustls::QuicClientConfig;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

/// Longest wait for the QUIC handshake plus h3 setup. A peer that does not
/// answer QUIC at all (an h2-only server) fails within this bound.
pub const H3_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

/// Connection-specific fields that HTTP/3 forbids (RFC 9114 §4.2).
const HOP_HEADERS: [&str; 6] = [
    "host",
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
];

type H3Stream = h3::client::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>;

/// Response head and live body of one HTTP/3 exchange.
pub struct H3Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn ByteStream>,
}

/// Tag an error with whether request bytes may have reached the server.
fn with_sent(mut e: RivetError, sent: bool) -> RivetError {
    if !matches!(e.details, Value::Object(_)) {
        e.details = Value::Object(Vec::new());
    }
    e.details.set("request_sent", Value::Bool(sent));
    e.details.set("version", Value::Int(3));
    e
}

/// The peer cannot do HTTP/3 (nothing was sent).
fn unavailable(why: impl std::fmt::Display) -> RivetError {
    with_sent(
        RivetError::new(
            ErrorKind::Protocol,
            "http.version_unavailable",
            format!("HTTP/3 is not available: {why}"),
        ),
        false,
    )
}

/// Map a handshake failure: TLS alerts other than no_application_protocol
/// stay `tls.handshake`; everything else means "no HTTP/3 here".
fn handshake_error(e: &quinn::ConnectionError) -> RivetError {
    use quinn::ConnectionError as C;
    let code = match e {
        C::TransportError(t) => Some(u64::from(t.code)),
        C::ConnectionClosed(c) => Some(u64::from(c.error_code)),
        _ => None,
    };
    match code {
        Some(c) if (0x100..=0x1ff).contains(&c) && c - 0x100 != 120 => with_sent(
            RivetError::new(
                ErrorKind::Tls,
                "tls.handshake",
                format!("HTTP/3 TLS handshake failed: {e}"),
            )
            .with_details(Value::object([(
                "tls_alert",
                Value::Int((c - 0x100) as i64),
            )])),
            false,
        ),
        _ => unavailable(e),
    }
}

/// A failure after the request HEADERS may have left: never replayable.
fn sent_error(what: &str, e: impl std::fmt::Display) -> RivetError {
    with_sent(
        RivetError::new(
            ErrorKind::Connection,
            "connection.http3",
            format!("HTTP/3 {what} failed: {e}"),
        ),
        true,
    )
}

/// One HTTP/3 request at the checked address in `wire.target`.
pub async fn send(wire: &HttpWire) -> RivetResult<H3Response> {
    // HTTP/3 needs https and a TCP-style host:port target (the same checked address, dialed over UDP)
    let url = url::Url::parse(&wire.url).map_err(unavailable)?;
    if url.scheme() != "https" {
        return Err(unavailable("HTTP/3 needs an https:// URL"));
    }
    let peer: SocketAddr = match &wire.target {
        WireTarget::Tcp(a) => *a,
        WireTarget::Unix(_) => return Err(unavailable("HTTP/3 cannot run over a Unix socket")),
    };
    let host = url.host_str().unwrap_or("").to_string();

    // same trust as h1/h2 (`tls ca_file` roots or the platform verifier), ALPN exactly h3, 0-RTT disabled
    let base = client_config(&wire.tls, &[b"h3"]).map_err(|e| with_sent(e, false))?;
    let mut cc = (*base).clone();
    cc.enable_early_data = false;
    let qc = QuicClientConfig::try_from(cc).map_err(|e| {
        with_sent(
            RivetError::new(ErrorKind::Tls, "tls.config", format!("TLS setup: {e}")),
            false,
        )
    })?;
    let name = server_name(&host, &wire.tls).map_err(|e| with_sent(e, false))?;
    let name = name.to_str().into_owned();
    let config = quinn::ClientConfig::new(Arc::new(qc));

    let local: SocketAddr = if peer.is_ipv4() {
        (Ipv4Addr::UNSPECIFIED, 0).into()
    } else {
        (Ipv6Addr::UNSPECIFIED, 0).into()
    };
    let endpoint = quinn::Endpoint::client(local).map_err(|e| {
        with_sent(
            RivetError::new(
                ErrorKind::Connection,
                "connection.udp",
                format!("HTTP/3 socket: {e}"),
            ),
            false,
        )
    })?;
    // QUIC handshake to exactly the checked address, bounded by H3_HANDSHAKE_TIMEOUT; no answer or ALPN refusal => http.version_unavailable (request_sent=false)
    let connecting = endpoint
        .connect_with(config, peer, &name)
        .map_err(unavailable)?;
    let conn = match tokio::time::timeout(H3_HANDSHAKE_TIMEOUT, connecting).await {
        Err(_) => {
            endpoint.close(0u32.into(), b"handshake timeout");
            return Err(unavailable(format!(
                "no QUIC answer from {peer} within {} ms",
                H3_HANDSHAKE_TIMEOUT.as_millis()
            )));
        }
        Ok(Err(e)) => return Err(handshake_error(&e)),
        Ok(Ok(c)) => c,
    };
    let setup = h3::client::new(h3_quinn::Connection::new(conn.clone()));
    let (mut driver, mut sender) = match tokio::time::timeout(H3_HANDSHAKE_TIMEOUT, setup).await {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => {
            conn.close(0u32.into(), b"h3 setup");
            return Err(unavailable(e));
        }
        Err(_) => {
            conn.close(0u32.into(), b"h3 setup");
            return Err(unavailable("h3 setup timed out"));
        }
    };
    let driver = tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });
    let owner = Owner {
        endpoint,
        conn,
        driver: Some(driver),
    };

    let mut req = http::Request::builder()
        .method(wire.method.as_str())
        .uri(url.as_str());
    let mut has_ua = false;
    for (k, v) in &wire.headers {
        if HOP_HEADERS.iter().any(|h| k.eq_ignore_ascii_case(h)) {
            continue;
        }
        has_ua |= k.eq_ignore_ascii_case("user-agent");
        req = req.header(k.as_str(), v.as_str());
    }
    if !has_ua {
        req = req.header("user-agent", concat!("rivet/", env!("CARGO_PKG_VERSION")));
    }
    let req = req.body(()).map_err(|e| {
        with_sent(
            RivetError::validation("validation.http_request", e.to_string()),
            false,
        )
    })?;

    // from the HEADERS frame on, any failure carries request_sent=true and is never replayed
    let mut stream: H3Stream = sender
        .send_request(req)
        .await
        .map_err(|e| sent_error("request", e))?;
    if let Some(body) = &wire.body
        && !body.is_empty()
    {
        stream
            .send_data(Bytes::from(body.clone()))
            .await
            .map_err(|e| sent_error("request body", e))?;
    }
    stream
        .finish()
        .await
        .map_err(|e| sent_error("request", e))?;
    let resp = stream
        .recv_response()
        .await
        .map_err(|e| sent_error("response", e))?;
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    Ok(H3Response {
        status: resp.status().as_u16(),
        headers,
        body: Box::new(H3Body {
            stream,
            _sender: sender,
            owner,
        }),
    })
}

/// Owns the QUIC endpoint, connection and h3 driver for one exchange.
struct Owner {
    endpoint: quinn::Endpoint,
    conn: quinn::Connection,
    driver: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.conn.close(0u32.into(), b"done");
        self.endpoint.close(0u32.into(), b"done");
        if let Some(d) = self.driver.take() {
            d.abort();
        }
    }
}

/// The live HTTP/3 response body; the connection closes with it.
struct H3Body {
    stream: H3Stream,
    _sender: h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
    owner: Owner,
}

#[async_trait]
impl ByteStream for H3Body {
    async fn next_chunk(&mut self) -> RivetResult<Option<Vec<u8>>> {
        loop {
            match self.stream.recv_data().await {
                Ok(None) => return Ok(None),
                Ok(Some(mut buf)) => {
                    let n = buf.remaining();
                    if n > 0 {
                        return Ok(Some(buf.copy_to_bytes(n).to_vec()));
                    }
                }
                Err(e) => {
                    return Err(RivetError::new(
                        ErrorKind::Connection,
                        "connection.http_body",
                        format!("HTTP/3 response body failed: {e}"),
                    ));
                }
            }
        }
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        if let Some(d) = self.owner.driver.take() {
            d.abort();
            let _ = d.await;
        }
        Ok(())
    }
}
