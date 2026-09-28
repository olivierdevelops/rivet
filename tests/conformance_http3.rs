//! T-14 — HTTP/3 through the ordinary `http` effect (UC-14, S99–S102).
//!
//! ```text
//!   127.0.0.1:PORT/udp  ── h3 fixture (quinn + h3, ALPN h3)      GET /items, GET /lines, POST /mutate (response lost)
//!                       ── or a QUIC peer with ALPN "not-h3"      (H3 handshake fails fast)
//!                       ── or nothing                             (no QUIC answer)
//!   127.0.0.1:PORT/tcp  ── h2-only TLS fixture (hyper, ALPN h2)   GET /items, POST /mutate
//! ```
//!
//! One self-signed certificate (rcgen, SAN 127.0.0.1 + localhost) is
//! trusted through `tls ca_file "./ca.pem"`. Every test counts what each
//! server actually received, so "no downgrade", "one mutation" and "zero
//! packets" are observed on the wire, not inferred from the error.

use bytes::{Buf, Bytes};
use http_body_util::Full;
use hyper_util::rt::{TokioExecutor, TokioIo};
use quinn::crypto::rustls::QuicServerConfig;
use rivet::domain::{ErrorKind, RivetError, Value};
use rivet::orchestrator::runtime::{Runtime, policy_from_json};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Quic {
    /// A real HTTP/3 server.
    H3,
    /// A QUIC server that only offers ALPN `not-h3`.
    WrongAlpn,
    /// Nothing listens on UDP.
    None,
}

#[derive(Default)]
struct Counts {
    h3_requests: AtomicUsize,
    h2_requests: AtomicUsize,
    mutations: AtomicUsize,
}

struct Fixture {
    port: u16,
    root: tempfile::TempDir,
    counts: Arc<Counts>,
    _quic: Option<quinn::Endpoint>,
}

/// Start the fixtures on one port number; `ca.pem` goes into the bundle root.
async fn fixture(quic: Quic, h2: bool) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let counts = Arc::new(Counts::default());
    // One key pair for both servers so one ca.pem trusts either.
    let cert =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".into(), "localhost".into()]).unwrap();
    std::fs::write(root.path().join("ca.pem"), cert.cert.pem()).unwrap();
    let identity = || {
        (
            vec![CertificateDer::from(cert.cert.der().to_vec())],
            PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
        )
    };
    let provider = || Arc::new(rustls::crypto::ring::default_provider());

    let tcp = if h2 {
        Some(tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap())
    } else {
        None
    };
    let port = match &tcp {
        Some(l) => l.local_addr().unwrap().port(),
        None => 0,
    };
    if let Some(listener) = tcp {
        let (chain, key) = identity();
        let mut sc = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .unwrap();
        sc.alpn_protocols = vec![b"h2".to_vec()];
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(sc));
        let counts = counts.clone();
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let acceptor = acceptor.clone();
                let counts = counts.clone();
                tokio::spawn(async move {
                    let Ok(tls) = acceptor.accept(sock).await else {
                        return;
                    };
                    let svc = hyper::service::service_fn(move |req: hyper::Request<_>| {
                        let counts = counts.clone();
                        async move {
                            counts.h2_requests.fetch_add(1, Ordering::SeqCst);
                            if req.method() == hyper::Method::POST {
                                counts.mutations.fetch_add(1, Ordering::SeqCst);
                            }
                            let body = r#"{"items":[],"via":"h2"}"#;
                            Ok::<_, std::convert::Infallible>(
                                hyper::Response::builder()
                                    .header("content-type", "application/json")
                                    .body(Full::new(Bytes::from(body)))
                                    .unwrap(),
                            )
                        }
                    });
                    let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                        .serve_connection(TokioIo::new(tls), svc)
                        .await;
                });
            }
        });
    }

    let quic_server = match quic {
        Quic::None => None,
        Quic::H3 | Quic::WrongAlpn => {
            let (chain, key) = identity();
            let mut sc = rustls::ServerConfig::builder_with_provider(provider())
                .with_protocol_versions(&[&rustls::version::TLS13])
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(chain, key)
                .unwrap();
            sc.alpn_protocols = vec![if quic == Quic::H3 {
                b"h3".to_vec()
            } else {
                b"not-h3".to_vec()
            }];
            sc.max_early_data_size = 0;
            let cfg =
                quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(sc).unwrap()));
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let ep = quinn::Endpoint::server(cfg, addr).unwrap();
            let server = ep.clone();
            let counts = counts.clone();
            tokio::spawn(async move {
                while let Some(incoming) = server.accept().await {
                    let counts = counts.clone();
                    tokio::spawn(async move {
                        let Ok(conn) = incoming.await else { return };
                        serve_h3(conn, counts).await;
                    });
                }
            });
            Some(ep)
        }
    };
    let port = match &quic_server {
        Some(ep) if port == 0 => ep.local_addr().unwrap().port(),
        _ => port,
    };
    Fixture {
        port,
        root,
        counts,
        _quic: quic_server,
    }
}

/// The h3 fixture: `GET /items`, `GET /lines` and `POST /mutate`, whose
/// response is lost (the connection is closed after the body arrived).
async fn serve_h3(conn: quinn::Connection, counts: Arc<Counts>) {
    let Ok(mut h3conn) =
        h3::server::Connection::<_, Bytes>::new(h3_quinn::Connection::new(conn.clone())).await
    else {
        return;
    };
    while let Ok(Some(resolver)) = h3conn.accept().await {
        let Ok((req, mut stream)) = resolver.resolve_request().await else {
            continue;
        };
        counts.h3_requests.fetch_add(1, Ordering::SeqCst);
        let mut body = Vec::new();
        while let Ok(Some(mut chunk)) = stream.recv_data().await {
            let n = chunk.remaining();
            body.extend_from_slice(&chunk.copy_to_bytes(n));
        }
        match (req.method().as_str(), req.uri().path()) {
            ("POST", "/mutate") => {
                counts.mutations.fetch_add(1, Ordering::SeqCst);
                // The mutation happened; the response never arrives.
                conn.close(0x102u32.into(), b"lost");
                return;
            }
            (_, path) => {
                let (ctype, text) = if path == "/lines" {
                    ("text/plain", "alpha\nbeta\n".to_string())
                } else {
                    ("application/json", r#"{"items":[]}"#.to_string())
                };
                let resp = http::Response::builder()
                    .status(200)
                    .header("content-type", ctype)
                    .body(())
                    .unwrap();
                let _ = stream.send_response(resp).await;
                let _ = stream.send_data(Bytes::from(text)).await;
                let _ = stream.finish().await;
            }
        }
    }
}

impl Fixture {
    fn origin(&self) -> String {
        format!("https://127.0.0.1:{}", self.port)
    }

    fn runtime(&self, src: &str, network: bool) -> Runtime {
        let root = self.root.path().to_str().unwrap();
        let mut grants =
            vec![r#"{"capability": "allow_read", "targets": ["./ca.pem"]}"#.to_string()];
        if network {
            grants.push(format!(
                r#"{{"capability": "allow_network", "targets": ["{}"]}}"#,
                self.origin()
            ));
        }
        let policy = format!(
            r#"{{"version": 1, "grants": [{}], "network": {{"deny_private_ranges": true}}}}"#,
            grants.join(", ")
        );
        Runtime::builder()
            .source("app.rivet", src, root)
            .policy(policy_from_json(policy.as_bytes(), root).unwrap())
            .build()
            .unwrap()
    }

    fn h3(&self) -> usize {
        self.counts.h3_requests.load(Ordering::SeqCst)
    }
    fn h2(&self) -> usize {
        self.counts.h2_requests.load(Ordering::SeqCst)
    }
    fn mutations(&self) -> usize {
        self.counts.mutations.load(Ordering::SeqCst)
    }
}

/// `GET /items` with the given `version …` line.
fn get_items(origin: &str, version: &str) -> String {
    format!(
        "operation t.op\n    output json\n    response = http get \"{origin}/items\"\n        {version}\n        tls ca_file \"./ca.pem\"\n        decode json\n    end\n    return {{items: response.body.items, version: response.version}}\nend\n"
    )
}

async fn run(rt: &Runtime) -> Result<Value, RivetError> {
    rt.request("t.op", Value::Null, None)
        .await
        .map(|c| c.result)
}

fn items(version: Value) -> Value {
    Value::object([("items", Value::List(vec![])), ("version", version)])
}

// vhco:test transports.exchange_http -- strict `version 3` completes over HTTP/3 against the h3 fixture and response.version is 3
#[tokio::test]
async fn strict_h3_success() {
    let f = fixture(Quic::H3, true).await;
    let rt = f.runtime(&get_items(&f.origin(), "version 3"), true);
    assert_eq!(run(&rt).await.unwrap(), items(Value::Int(3)));
    assert_eq!((f.h3(), f.h2()), (1, 0));
}

// vhco:test transports.exchange_http -- strict `version 3` to an h2-only server fails http.version_unavailable (exit 5) and never downgrades
#[tokio::test]
async fn strict_h3_to_h2_only_server_does_not_downgrade() {
    for quic in [Quic::None, Quic::WrongAlpn] {
        let f = fixture(quic, true).await;
        let rt = f.runtime(&get_items(&f.origin(), "version 3"), true);
        let started = Instant::now();
        let e = run(&rt).await.unwrap_err();
        assert_eq!(e.code, "http.version_unavailable", "{}", e.message);
        assert_eq!(e.kind, ErrorKind::Protocol);
        assert_eq!(e.exit_code(), 5);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(f.h2(), 0, "strict HTTP/3 must not fall back to h2");
    }
}

// vhco:test transports.exchange_http -- `version prefer [3, 2]` uses H3 when available and falls back to h2 when the H3 handshake fails before any request byte; response.version reports 3 or 2
#[tokio::test]
async fn prefer_falls_back_only_when_h3_handshake_fails() {
    let f = fixture(Quic::H3, true).await;
    let rt = f.runtime(&get_items(&f.origin(), "version prefer [3, 2]"), true);
    assert_eq!(run(&rt).await.unwrap(), items(Value::Int(3)));
    assert_eq!((f.h3(), f.h2()), (1, 0));

    let f = fixture(Quic::WrongAlpn, true).await;
    let rt = f.runtime(&get_items(&f.origin(), "version prefer [3, 2]"), true);
    assert_eq!(run(&rt).await.unwrap(), items(Value::Int(2)));
    assert_eq!(f.h2(), 1);

    let f = fixture(Quic::None, true).await;
    let rt = f.runtime(&get_items(&f.origin(), "version prefer [3, 2]"), true);
    assert_eq!(run(&rt).await.unwrap(), items(Value::Int(2)));
    assert_eq!(f.h2(), 1);

    // Plain `version 2` against the same server reports 2 as well.
    let rt = f.runtime(&get_items(&f.origin(), "version 2"), true);
    assert_eq!(run(&rt).await.unwrap(), items(Value::Int(2)));

    // HTTP/3 may only come first in a preference list.
    let rt = f.runtime(&get_items(&f.origin(), "version prefer [2, 3]"), true);
    assert_eq!(run(&rt).await.unwrap_err().code, "validation.http_version");
}

// vhco:test transports.exchange_http -- a POST whose H3 response is lost fails (exit 5) and is not re-sent over h2: exactly one mutation
#[tokio::test]
async fn post_with_lost_response_is_not_replayed() {
    let f = fixture(Quic::H3, true).await;
    let src = format!(
        "operation t.op\n    output json\n    response = http post \"{}/mutate\"\n        version prefer [3, 2]\n        tls ca_file \"./ca.pem\"\n        body json {{name: \"widget\"}}\n    end\n    return response.status\nend\n",
        f.origin()
    );
    let rt = f.runtime(&src, true);
    let e = run(&rt).await.unwrap_err();
    assert_eq!(e.exit_code(), 5, "{}: {}", e.code, e.message);
    assert_eq!(e.code, "connection.http3");
    assert_eq!(e.details.get("request_sent"), Some(&Value::Bool(true)));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(f.mutations(), 1, "exactly one mutation");
    assert_eq!(f.h2(), 0, "an ambiguous POST is never replayed over h2");
}

// vhco:test transports.exchange_http -- without an https grant the H3 request is denied (permission.denied, exit 3) and zero UDP packets are sent
#[tokio::test]
async fn policy_denial_sends_zero_packets() {
    let sink = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = sink.local_addr().unwrap().port();
    let f = Fixture {
        port,
        root: tempfile::tempdir().unwrap(),
        counts: Arc::new(Counts::default()),
        _quic: None,
    };
    std::fs::write(f.root.path().join("ca.pem"), "not used").unwrap();
    for version in ["version 3", "version prefer [3, 2]"] {
        let rt = f.runtime(&get_items(&f.origin(), version), false);
        let e = run(&rt).await.unwrap_err();
        assert_eq!(e.code, "permission.denied");
        assert_eq!(e.exit_code(), 3);
    }
    let mut buf = [0u8; 2048];
    let got = tokio::time::timeout(Duration::from_millis(300), sink.recv_from(&mut buf)).await;
    assert!(got.is_err(), "a denied HTTP/3 target received a packet");
}

// vhco:test transports.exchange_http -- `with http … version 3 stream lines` iterates an HTTP/3 body and exposes response.version 3
#[tokio::test]
async fn h3_stream_scope() {
    let f = fixture(Quic::H3, false).await;
    let src = format!(
        "operation t.op\n    output json\n    out = []\n    with http get \"{}/lines\" as s\n        version 3\n        tls ca_file \"./ca.pem\"\n        stream lines\n        for line in s\n            out += [line]\n        end\n        return {{lines: out, version: s.version}}\n    end\nend\n",
        f.origin()
    );
    let rt = f.runtime(&src, true);
    assert_eq!(
        run(&rt).await.unwrap(),
        Value::object([
            (
                "lines",
                Value::List(vec![Value::text("alpha"), Value::text("beta")])
            ),
            ("version", Value::Int(3)),
        ])
    );
}

// vhco:test transports.exchange_http -- docs/demos/09-quic items.http3 returns {items: [], version: 3} against the local h3 fixture; the bundle's QUIC-only policy.json denies it before any I/O
#[tokio::test]
async fn demo_09_quic_items_http3() {
    let f = fixture(Quic::H3, false).await;
    let text = std::fs::read_to_string("docs/demos/09-quic/app.rivet").unwrap();
    assert!(text.contains("\"https://api.example.com/items\""));
    let local = text
        .replace(
            "\"https://api.example.com/items\"",
            &format!("\"{}/items\"", f.origin()),
        )
        .replace(
            "        version 3\n",
            "        version 3\n        tls ca_file \"./ca.pem\"\n",
        );
    let rt = f.runtime(&local, true);
    let c = rt.request("items.http3", Value::Null, None).await.unwrap();
    assert_eq!(c.result, items(Value::Int(3)));
    assert_eq!(f.h3(), 1);

    // The unmodified bundle under its QUIC-only policy.json: refused before DNS.
    let rt = Runtime::builder()
        .file("docs/demos/09-quic/app.rivet")
        .policy_file("docs/demos/09-quic/policy.json")
        .build()
        .unwrap();
    let e = rt
        .request("items.http3", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
}
