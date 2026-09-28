//! UDP datagram and native QUIC plans and results (PROP-2026-0001 Increments
//! 10–11). A scoped `with udp …` / `with quic …` resource turns each attempt
//! (open, send, receive, child stream, close) into a one-step plan that the
//! datagrams/quic use cases validate and authorize before the driver runs it.
//!
//! ```text
//!  with udp "127.0.0.1:7000" as socket        ──▶ DatagramPlan{Connected, steps:[Open]}
//!      socket.send json {…}                   ──▶ DatagramPlan{…, steps:[Send{payload}]}
//!      socket.receive json timeout "1s"       ──▶ DatagramPlan{…, steps:[Receive{1000}]}
//!  end                                        ──▶ DatagramPlan{…, steps:[Close]}
//! ```

use super::errors::EffectsStatus;
use super::source::SourceSpan;
use super::value::Value;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Instant;

// vhco:domain EffectContext { operation_id: string; span?: SourceSpan; deadline: Instant }
/// Who is asking and until when; every intent the use cases evaluate carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectContext {
    pub operation_id: String,
    pub span: Option<SourceSpan>,
    pub deadline: Instant,
}

// vhco:domain DatagramMode { connected | bind | multicast }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatagramMode {
    /// `with udp "host:port"` — connected unicast; replies only from that peer.
    Connected,
    /// `with udp bind "host:port"` — explicit listener; `receive_from` / `send_to`.
    Bind,
    /// `with udp multicast "group:port"` + `bind`/`interface` options.
    Multicast,
}

// vhco:domain DatagramStep { open | send | send_to | receive | receive_from | close }
#[derive(Clone, Debug, PartialEq)]
pub enum DatagramStep {
    Open,
    /// Send to the connected peer (or the multicast group).
    Send {
        payload: Vec<u8>,
    },
    /// Send to an explicit peer; `addr` is filled by the use case after the
    /// peer is authorized and resolved (the driver never re-resolves).
    SendTo {
        peer: String,
        addr: Option<SocketAddr>,
        payload: Vec<u8>,
    },
    Receive {
        timeout_ms: Option<u64>,
    },
    ReceiveFrom {
        timeout_ms: Option<u64>,
    },
    Close,
}

// vhco:domain DatagramPlan { mode: DatagramMode; peer?: string; peer_addr?: SocketAddr; bind?: string; multicast?: string; interface?: string; max_datagram: u32; timeout_ms?: u64; steps: DatagramStep[]; context: EffectContext }
#[derive(Clone, Debug, PartialEq)]
pub struct DatagramPlan {
    pub mode: DatagramMode,
    /// Connected peer as written (`host:port`, `[v6]:port`).
    pub peer: Option<String>,
    /// Checked address of `peer` (filled by the use case on `Open`).
    pub peer_addr: Option<SocketAddr>,
    /// Local bind (`with udp bind`, or the multicast `bind` option).
    pub bind: Option<String>,
    /// Multicast group `group:port`.
    pub multicast: Option<String>,
    /// Multicast interface: an interface name (`eth0`) or an IP literal.
    pub interface: Option<String>,
    pub max_datagram: u32,
    /// Resource-level `timeout "D"` (default for receives without their own).
    pub timeout_ms: Option<u64>,
    pub steps: Vec<DatagramStep>,
    pub context: EffectContext,
}

// vhco:domain DatagramMessage { peer: string; data: bytes }
#[derive(Clone, Debug, PartialEq)]
pub struct DatagramMessage {
    /// Sender as `host:port` (IPv6 in brackets).
    pub peer: String,
    pub data: Vec<u8>,
}

// vhco:domain DatagramResult { messages: DatagramMessage[]; sent_count: u64; effects: EffectsStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct DatagramResult {
    pub messages: Vec<DatagramMessage>,
    pub sent_count: u64,
    /// A send is local acceptance only: `unknown` once anything was sent.
    pub effects: EffectsStatus,
}

/// Largest UDP payload over IPv4 (65 535 − 8 − 20).
pub const MAX_UDP_PAYLOAD: u32 = 65_507;
/// `max_datagram` when the block does not set one.
pub const DEFAULT_MAX_DATAGRAM: u32 = 8192;

// vhco:domain StreamDir { bidi | uni }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamDir {
    Bidi,
    Uni,
}

// vhco:domain FramingKind { raw | newline | length32 | delimiter }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FramingKind {
    /// No framing: a receive yields one chunk as it arrives.
    Raw,
    Newline,
    Length32 {
        big_endian: bool,
    },
    Delimiter(Vec<u8>),
}

// vhco:domain Framing { kind: FramingKind; max_frame: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framing {
    pub kind: FramingKind,
    pub max_frame: u32,
}

/// Default frame bound (PROP-2026-0001 defaults: 1 MiB).
pub const DEFAULT_MAX_FRAME: u32 = 1024 * 1024;

impl Default for Framing {
    fn default() -> Self {
        Framing {
            kind: FramingKind::Raw,
            max_frame: DEFAULT_MAX_FRAME,
        }
    }
}

// vhco:domain QuicTls { server_name?: string; ca_file?: string; cert_file?: string; key_file?: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct QuicTls {
    pub server_name: Option<String>,
    pub ca_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
}

// vhco:domain QuicStep { connect | open_stream | accept_stream | send | receive | finish_send | close_stream | send_datagram | receive_datagram | close }
#[derive(Clone, Debug, PartialEq)]
pub enum QuicStep {
    Connect,
    OpenStream {
        dir: StreamDir,
        framing: Framing,
    },
    AcceptStream {
        dir: StreamDir,
        framing: Framing,
        timeout_ms: Option<u64>,
    },
    Send {
        stream: u64,
        payload: Vec<u8>,
    },
    /// One frame (or one raw chunk); `Value::Null` at the peer's FIN.
    Receive {
        stream: u64,
        timeout_ms: Option<u64>,
    },
    FinishSend {
        stream: u64,
    },
    /// Scope exit of a child stream: FIN if still open, stop reading.
    CloseStream {
        stream: u64,
    },
    SendDatagram {
        payload: Vec<u8>,
    },
    ReceiveDatagram {
        timeout_ms: Option<u64>,
    },
    Close,
}

// vhco:domain QuicPlan { endpoint: string; server_name: string; alpn: string; max_streams: u32; datagrams: bool; migration: bool; early_data: bool; timeout_ms?: u64; tls: QuicTls; peer_addr?: SocketAddr; steps: QuicStep[]; context: EffectContext }
#[derive(Clone, Debug, PartialEq)]
pub struct QuicPlan {
    /// `quic://host:port` or `https://host:port` as written.
    pub endpoint: String,
    /// TLS identity to verify (`tls server_name`, else the URL host).
    pub server_name: String,
    pub alpn: String,
    pub max_streams: u32,
    pub datagrams: bool,
    pub migration: bool,
    pub early_data: bool,
    pub timeout_ms: Option<u64>,
    pub tls: QuicTls,
    /// Checked UDP destination (filled by the use case on `Connect`).
    pub peer_addr: Option<SocketAddr>,
    pub steps: Vec<QuicStep>,
    pub context: EffectContext,
}

/// Default `max_streams` (PROP-2026-0001 Increment 11: at most 8 active child streams).
pub const DEFAULT_MAX_STREAMS: u32 = 8;

// vhco:domain QuicResult { value: Value; stream_count: u32; negotiated_alpn: string; effects: EffectsStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct QuicResult {
    /// Step value: a stream key (`open`/`accept`), a frame/datagram (`Bytes`), or `Null`.
    pub value: Value,
    pub stream_count: u32,
    pub negotiated_alpn: String,
    pub effects: EffectsStatus,
}

/// Split `host:port` / `[v6]:port` into host (brackets removed) and port.
pub fn split_host_port(s: &str) -> Option<(String, u16)> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('[') {
        let (host, port) = rest.split_once("]:")?;
        return Some((host.to_string(), port.parse().ok()?));
    }
    let (host, port) = s.rsplit_once(':')?;
    if host.is_empty() || host.contains(':') {
        return None;
    }
    Some((host.to_string(), port.parse().ok()?))
}

/// `host:port` with IPv6 bracket notation normalized.
pub fn join_host_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// An IP literal, with `localhost` meaning 127.0.0.1 (the policy's reading too).
pub fn literal_ip(host: &str) -> Option<IpAddr> {
    if host.eq_ignore_ascii_case("localhost") {
        return Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    host.parse().ok()
}

/// RFC1918, loopback, link-local (incl. 169.254.169.254), CGNAT, fc00::/7,
/// fe80::/10 and unspecified — the ranges `network.deny_private_ranges` refuses.
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
                || a.to_ipv4_mapped()
                    .is_some_and(|v4| is_private_ip(IpAddr::V4(v4)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_port_forms() {
        assert_eq!(
            split_host_port("127.0.0.1:7000"),
            Some(("127.0.0.1".into(), 7000))
        );
        assert_eq!(split_host_port("[::1]:9"), Some(("::1".into(), 9)));
        assert_eq!(split_host_port("::1:9"), None);
        assert_eq!(split_host_port("host"), None);
        assert_eq!(join_host_port("::1", 9), "[::1]:9");
        assert!(is_private_ip("10.1.2.3".parse().unwrap()));
        assert!(!is_private_ip("239.0.0.1".parse().unwrap()));
        assert_eq!(literal_ip("LOCALHOST"), Some("127.0.0.1".parse().unwrap()));
    }
}
