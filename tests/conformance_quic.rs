//! T-13 — native QUIC: a local quinn fixture with a self-signed certificate
//! trusted through `tls ca_file` (rcgen). Bidi and uni streams, stream limits
//! and resets, wrong ALPN, untrusted certificates, DATAGRAM round trip,
//! migration refusal, denied targets sending zero packets, and the
//! docs/demos/09-quic bundle against the fixture.

use quinn::crypto::rustls::QuicServerConfig;
use rivet::internal::domain::{ErrorKind, RivetError, Value};
use rivet::internal::orchestrator::runtime::{Runtime, policy_from_json};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

struct Fixture {
    addr: SocketAddr,
    root: tempfile::TempDir,
    _server: quinn::Endpoint,
}

const ALPNS: [&[u8]; 4] = [
    b"rivet-rpc/1",
    b"rivet-uni/1",
    b"rivet-telemetry/1",
    b"rivet-reset/1",
];

/// Start a loopback QUIC server; `ca.pem` (its self-signed cert for
/// `localhost`) is written into the returned bundle root.
fn fixture() -> Fixture {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ca.pem"), cert.cert.pem()).unwrap();
    let der = CertificateDer::from(cert.cert.der().to_vec());
    let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let mut sc = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![der], key.into())
    .unwrap();
    sc.alpn_protocols = ALPNS.iter().map(|a| a.to_vec()).collect();
    sc.max_early_data_size = 0;
    let mut cfg =
        quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(sc).unwrap()));
    let mut tc = quinn::TransportConfig::default();
    tc.datagram_receive_buffer_size(Some(65_536));
    cfg.transport_config(Arc::new(tc));
    cfg.migration(false);
    let server = quinn::Endpoint::server(cfg, "127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = server.local_addr().unwrap();
    let ep = server.clone();
    tokio::spawn(async move {
        while let Some(incoming) = ep.accept().await {
            tokio::spawn(async move {
                let Ok(conn) = incoming.await else { return };
                let alpn = conn
                    .handshake_data()
                    .and_then(|h| h.downcast::<quinn::crypto::rustls::HandshakeData>().ok())
                    .and_then(|h| h.protocol)
                    .unwrap_or_default();
                match alpn.as_slice() {
                    b"rivet-rpc/1" => {
                        while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                            let mut len = [0u8; 4];
                            if recv.read_exact(&mut len).await.is_err() {
                                continue;
                            }
                            let mut body = vec![0u8; u32::from_be_bytes(len) as usize];
                            let _ = recv.read_exact(&mut body).await;
                            let reply = br#"{"state":"ready"}"#;
                            let _ = send.write_all(&(reply.len() as u32).to_be_bytes()).await;
                            let _ = send.write_all(reply).await;
                            let _ = send.finish();
                        }
                    }
                    b"rivet-uni/1" => {
                        while let Ok(mut recv) = conn.accept_uni().await {
                            let data = recv.read_to_end(1 << 16).await.unwrap_or_default();
                            if let Ok(mut out) = conn.open_uni().await {
                                let mut msg = b"got:".to_vec();
                                msg.extend(data);
                                let _ = out.write_all(&msg).await;
                                let _ = out.finish();
                            }
                        }
                    }
                    b"rivet-telemetry/1" => {
                        while let Ok(d) = conn.read_datagram().await {
                            let _ = conn.send_datagram(d);
                        }
                    }
                    b"rivet-reset/1" => {
                        while let Ok((mut send, _recv)) = conn.accept_bi().await {
                            let _ = send.reset(7u32.into());
                        }
                    }
                    _ => {}
                }
                conn.closed().await;
            });
        }
    });
    Fixture {
        addr,
        root,
        _server: server,
    }
}

impl Fixture {
    fn origin(&self) -> String {
        format!("quic://{}", self.addr)
    }

    fn runtime(&self, src: &str, grants: &[(&str, String)]) -> Runtime {
        let root = self.root.path().to_str().unwrap();
        let g: Vec<String> = grants
            .iter()
            .map(|(c, t)| format!(r#"{{"capability": "{c}", "targets": ["{t}"]}}"#))
            .collect();
        let policy = format!(
            r#"{{"version": 1, "grants": [{}], "network": {{"deny_private_ranges": true}}}}"#,
            g.join(", ")
        );
        Runtime::builder()
            .source("app.rivet", src, root)
            .policy(policy_from_json(policy.as_bytes(), root).unwrap())
            .build()
            .unwrap()
    }

    fn full_grants(&self) -> Vec<(&'static str, String)> {
        vec![
            ("allow_network", self.origin()),
            ("allow_read", "./ca.pem".to_string()),
        ]
    }
}

/// `with quic` block trusting the fixture CA; `body` is indented 8 spaces.
fn op(origin: &str, alpn: &str, extra: &str, body: &str) -> String {
    format!(
        "operation q.op\n    output json\n    with quic \"{origin}\" as connection\n        alpn \"{alpn}\"\n        tls server_name \"localhost\"\n        tls ca_file \"./ca.pem\"\n{extra}{body}    end\nend\n"
    )
}

async fn run(rt: &Runtime) -> Result<Value, RivetError> {
    rt.request("q.op", Value::Null, None)
        .await
        .map(|c| c.result)
}

const BIDI: &str = "        with connection.open bidi as stream\n            framing length32 endian big max_frame 1048576\n            stream.send json {action: \"status\"}\n            return stream.receive json timeout \"5s\"\n        end\n";

// vhco:test quic.exchange_quic -- a bidi stream with length32 framing returns the fixture's status frame over a CA-pinned handshake
#[tokio::test]
async fn bidi_stream_round_trip() {
    let f = fixture();
    let rt = f.runtime(
        &op(
            &f.origin(),
            "rivet-rpc/1",
            "        max_streams 8\n        migration false\n",
            BIDI,
        ),
        &f.full_grants(),
    );
    assert_eq!(
        run(&rt).await.unwrap(),
        Value::object([("state", Value::text("ready"))])
    );
}

// vhco:test quic.exchange_quic -- a uni stream finishes at scope exit and a peer-opened uni stream is accepted read-only
#[tokio::test]
async fn uni_streams_open_and_accept() {
    let f = fixture();
    let body = "        with connection.open uni as out\n            out.send text \"hi\"\n        end\n        with connection.accept uni as inbound\n            timeout \"5s\"\n            return inbound.receive text\n        end\n";
    let rt = f.runtime(&op(&f.origin(), "rivet-uni/1", "", body), &f.full_grants());
    assert_eq!(run(&rt).await.unwrap(), Value::text("got:hi"));

    // A send-only stream cannot receive.
    let body = "        with connection.open uni as out\n            return out.receive text\n        end\n";
    let rt = f.runtime(&op(&f.origin(), "rivet-uni/1", "", body), &f.full_grants());
    assert_eq!(run(&rt).await.unwrap_err().code, "quic.direction");
}

// vhco:test quic.exchange_quic -- a mismatched ALPN fails the handshake as kind tls
#[tokio::test]
async fn wrong_alpn_is_a_tls_error() {
    let f = fixture();
    let rt = f.runtime(&op(&f.origin(), "wrong/1", "", BIDI), &f.full_grants());
    let e = run(&rt).await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Tls);
    assert_eq!(e.code, "quic.alpn_mismatch");
}

#[tokio::test]
async fn untrusted_certificate_is_a_tls_error() {
    let f = fixture();
    let src = format!(
        "operation q.op\n    output json\n    with quic \"{}\" as connection\n        alpn \"rivet-rpc/1\"\n        tls server_name \"localhost\"\n{BIDI}    end\nend\n",
        f.origin()
    );
    let rt = f.runtime(&src, &[("allow_network", f.origin())]);
    let e = run(&rt).await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Tls, "{e:?}");
}

// vhco:test quic.exchange_quic -- a negotiated DATAGRAM round-trips one message
#[tokio::test]
async fn datagram_round_trip() {
    let f = fixture();
    let body = "        connection.send_datagram bytes (base64.decode \"AAEC\")\n        return connection.receive_datagram bytes timeout \"2s\"\n";
    let rt = f.runtime(
        &op(
            &f.origin(),
            "rivet-telemetry/1",
            "        datagrams true\n",
            body,
        ),
        &f.full_grants(),
    );
    assert_eq!(run(&rt).await.unwrap(), Value::Bytes(vec![0, 1, 2]));

    // Without `datagrams true` the step is refused before sending.
    let rt = f.runtime(
        &op(&f.origin(), "rivet-telemetry/1", "", body),
        &f.full_grants(),
    );
    assert_eq!(run(&rt).await.unwrap_err().code, "quic.datagrams_disabled");
}

#[tokio::test]
async fn stream_limit_reset_and_migration() {
    let f = fixture();
    let nested = "        with connection.open bidi as a\n            with connection.open bidi as b\n                return 1\n            end\n        end\n";
    let rt = f.runtime(
        &op(
            &f.origin(),
            "rivet-rpc/1",
            "        max_streams 1\n",
            nested,
        ),
        &f.full_grants(),
    );
    assert_eq!(run(&rt).await.unwrap_err().code, "limit.quic_streams");

    let reset = "        with connection.open bidi as s\n            s.send text \"x\"\n            return s.receive text timeout \"5s\"\n        end\n";
    let rt = f.runtime(
        &op(&f.origin(), "rivet-reset/1", "", reset),
        &f.full_grants(),
    );
    let e = run(&rt).await.unwrap_err();
    assert_eq!(e.code, "quic.stream_reset");
    assert_eq!(e.kind, ErrorKind::Protocol);

    let rt = f.runtime(
        &op(&f.origin(), "rivet-rpc/1", "        migration true\n", BIDI),
        &f.full_grants(),
    );
    assert_eq!(
        run(&rt).await.unwrap_err().code,
        "unsupported.quic_migration"
    );
}

// vhco:test quic.exchange_quic -- a denied origin or an ungranted ca_file sends zero packets
#[tokio::test]
async fn denied_target_sends_zero_packets() {
    let sink = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sink.local_addr().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ca.pem"), "not used").unwrap();
    let f = Fixture {
        addr,
        root,
        _server: quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap(),
    };
    // No network grant.
    let rt = f.runtime(
        &op(&f.origin(), "rivet-rpc/1", "", BIDI),
        &[("allow_read", "./ca.pem".to_string())],
    );
    assert_eq!(run(&rt).await.unwrap_err().code, "permission.denied");
    // Network granted but the CA file is not.
    let rt = f.runtime(
        &op(&f.origin(), "rivet-rpc/1", "", BIDI),
        &[("allow_network", f.origin())],
    );
    assert_eq!(run(&rt).await.unwrap_err().code, "permission.denied");
    // A raw UDP grant does not authorize QUIC.
    let rt = f.runtime(
        &op(&f.origin(), "rivet-rpc/1", "", BIDI),
        &[
            ("allow_network", format!("udp://{addr}")),
            ("allow_read", "./ca.pem".to_string()),
        ],
    );
    assert_eq!(run(&rt).await.unwrap_err().code, "permission.denied");
    let mut buf = [0u8; 2048];
    let got = tokio::time::timeout(Duration::from_millis(300), sink.recv_from(&mut buf)).await;
    assert!(got.is_err(), "a denied QUIC target received a packet");
}

// vhco:test quic.exchange_quic -- docs/demos/09-quic engine.status returns the README's status frame against a local fixture; the other policy file denies it
#[tokio::test]
async fn demo_09_quic_bundle() {
    let f = fixture();
    let text = std::fs::read_to_string("docs/demos/09-quic/app.rivet").unwrap();
    assert!(text.contains("\"quic://engine.example.com:4433\""));
    let local = text
        .replace("\"quic://engine.example.com:4433\"", &format!("\"{}\"", f.origin()))
        .replace(
            "        migration false\n",
            "        migration false\n        tls server_name \"localhost\"\n        tls ca_file \"./ca.pem\"\n",
        );
    let rt = f.runtime(&local, &f.full_grants());
    let c = rt
        .request("engine.status", Value::Null, None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::object([("state", Value::text("ready"))]));

    // The unmodified bundle under policies/http3.json: denied before DNS or any packet.
    let rt = Runtime::builder()
        .file("docs/demos/09-quic/app.rivet")
        .policy_file("docs/demos/09-quic/policies/http3.json")
        .build()
        .unwrap();
    let e = rt
        .request("engine.status", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
}
