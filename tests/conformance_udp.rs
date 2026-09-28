//! T-12 — UDP: connected unicast (IPv4, IPv6 when available), timeouts,
//! truncation, oversized sends, listener binds, separately authorized replies,
//! multicast on loopback and the docs/demos/08-udp bundle. Local fixtures only.

use rivet::domain::{ErrorKind, RivetError, Value};
use rivet::orchestrator::runtime::{Runtime, policy_from_json};
use std::net::{SocketAddr, UdpSocket as StdUdp};
use std::time::Duration;
use tokio::net::UdpSocket;

fn runtime(src: &str, policy: &str) -> Runtime {
    let root = std::env::temp_dir();
    let root = root.to_str().unwrap();
    Runtime::builder()
        .source("app.rivet", src, root)
        .policy(policy_from_json(policy.as_bytes(), root).unwrap())
        .build()
        .unwrap()
}

fn grants(entries: &[(&str, String)]) -> String {
    let g: Vec<String> = entries
        .iter()
        .map(|(c, t)| format!(r#"{{"capability": "{c}", "targets": ["{t}"]}}"#))
        .collect();
    format!(
        r#"{{"version": 1, "grants": [{}], "network": {{"deny_private_ranges": true}}}}"#,
        g.join(", ")
    )
}

async fn run(rt: &Runtime, id: &str) -> Result<Value, RivetError> {
    rt.request(id, Value::Null, None).await.map(|c| c.result)
}

/// A peer that answers every datagram with `reply` (or stays silent when `None`)
/// and counts what it received.
async fn peer(
    bind: &str,
    reply: Option<Vec<u8>>,
) -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let s = UdpSocket::bind(bind).await.unwrap();
    let addr = s.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 70_000];
        while let Ok((n, from)) = s.recv_from(&mut buf).await {
            let _ = tx.send(buf[..n].to_vec());
            if let Some(r) = &reply {
                let _ = s.send_to(r, from).await;
            }
        }
    });
    (addr, rx)
}

fn status_op(target: &str, max: u32, timeout: &str) -> String {
    format!(
        "operation t.status\n    output json\n    with udp \"{target}\" as socket\n        max_datagram {max}\n        socket.send json {{command: \"status\"}}\n        return socket.receive json timeout \"{timeout}\"\n    end\nend\n"
    )
}

// vhco:test datagrams.exchange_datagrams -- a connected loopback socket sends one JSON datagram and decodes the reply
#[tokio::test]
async fn connected_ipv4_round_trip() {
    let (addr, mut seen) = peer("127.0.0.1:0", Some(br#"{"state":"ready"}"#.to_vec())).await;
    let rt = runtime(
        &status_op(&addr.to_string(), 8192, "1s"),
        &grants(&[("allow_network", format!("udp://{addr}"))]),
    );
    let v = run(&rt, "t.status").await.unwrap();
    assert_eq!(v, Value::object([("state", Value::text("ready"))]));
    assert_eq!(seen.recv().await.unwrap(), br#"{"command":"status"}"#);
}

#[tokio::test]
async fn connected_ipv6_round_trip_when_available() {
    if StdUdp::bind("[::1]:0").is_err() {
        eprintln!("IPv6 loopback unavailable on this host; skipping");
        return;
    }
    let (addr, _seen) = peer("[::1]:0", Some(br#"{"state":"ready"}"#.to_vec())).await;
    let rt = runtime(
        &status_op(&addr.to_string(), 8192, "1s"),
        &grants(&[("allow_network", format!("udp://{addr}"))]),
    );
    assert_eq!(
        run(&rt, "t.status").await.unwrap(),
        Value::object([("state", Value::text("ready"))])
    );
}

#[tokio::test]
async fn lost_reply_times_out_without_retry() {
    let (addr, mut seen) = peer("127.0.0.1:0", None).await;
    let rt = runtime(
        &status_op(&addr.to_string(), 8192, "150ms"),
        &grants(&[("allow_network", format!("udp://{addr}"))]),
    );
    let e = run(&rt, "t.status").await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Timeout);
    assert_eq!(seen.recv().await.unwrap(), br#"{"command":"status"}"#);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(seen.try_recv().is_err(), "no retry was sent");
}

// vhco:test datagrams.exchange_datagrams -- a reply longer than max_datagram is udp.truncated, an oversized send is refused before sending
#[tokio::test]
async fn truncated_and_oversized_datagrams() {
    let (addr, mut seen) = peer("127.0.0.1:0", Some(vec![b'x'; 100])).await;
    let rt = runtime(
        &status_op(&addr.to_string(), 40, "1s"),
        &grants(&[("allow_network", format!("udp://{addr}"))]),
    );
    let e = run(&rt, "t.status").await.unwrap_err();
    assert_eq!(e.code, "udp.truncated");
    assert_eq!(e.kind, ErrorKind::Protocol);
    seen.recv().await.unwrap();

    let rt = runtime(
        &status_op(&addr.to_string(), 8, "1s"),
        &grants(&[("allow_network", format!("udp://{addr}"))]),
    );
    let e = run(&rt, "t.status").await.unwrap_err();
    assert_eq!(e.code, "udp.message_too_large");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(seen.try_recv().is_err(), "oversized payload was not sent");
}

// vhco:test datagrams.exchange_datagrams -- a denied peer sends zero datagrams
#[tokio::test]
async fn denied_peer_sends_nothing() {
    let (addr, mut seen) = peer("127.0.0.1:0", Some(b"{}".to_vec())).await;
    let rt = runtime(
        &status_op(&addr.to_string(), 8192, "1s"),
        &grants(&[("allow_network", "udp://127.0.0.1:1".to_string())]),
    );
    let e = run(&rt, "t.status").await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(seen.try_recv().is_err());
}

fn free_port() -> u16 {
    StdUdp::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn receive_op(bind: u16) -> String {
    format!(
        "operation t.receive\n    output json\n    with udp bind \"127.0.0.1:{bind}\" as socket\n        max_datagram 1024\n        message = socket.receive_from json timeout \"3s\"\n        socket.send_to message.peer json {{received: true}}\n        return {{peer: message.peer, value: message.data}}\n    end\nend\n"
    )
}

/// Keep sending `hello` to `to` until a reply arrives (the listener binds
/// asynchronously); returns the reply.
async fn sender(to: SocketAddr) -> (SocketAddr, tokio::task::JoinHandle<Option<Vec<u8>>>) {
    let s = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let me = s.local_addr().unwrap();
    let h = tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        for _ in 0..60 {
            s.send_to(br#"{"hello":1}"#, to).await.unwrap();
            if let Ok(Ok((n, _))) =
                tokio::time::timeout(Duration::from_millis(50), s.recv_from(&mut buf)).await
            {
                return Some(buf[..n].to_vec());
            }
        }
        None
    });
    (me, h)
}

// vhco:test datagrams.exchange_datagrams -- a bound listener reports the sender and replies only with that peer's own grant
#[tokio::test]
async fn listener_reply_needs_its_own_grant() {
    let port = free_port();
    let bind: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let (me, reply) = sender(bind).await;
    let rt = runtime(
        &receive_op(port),
        &grants(&[
            ("allow_listen", format!("udp://{bind}")),
            ("allow_network", format!("udp://{me}")),
        ]),
    );
    let v = run(&rt, "t.receive").await.unwrap();
    assert_eq!(v.get("peer"), Some(&Value::text(me.to_string())));
    assert_eq!(
        v.get("value"),
        Some(&Value::object([("hello", Value::Int(1))]))
    );
    assert_eq!(reply.await.unwrap().unwrap(), br#"{"received":true}"#);

    // Same exchange without a grant for the sender: send_to is refused.
    let port = free_port();
    let bind: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let (_me, reply) = sender(bind).await;
    let rt = runtime(
        &receive_op(port),
        &grants(&[("allow_listen", format!("udp://{bind}"))]),
    );
    let e = run(&rt, "t.receive").await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
    reply.abort();
}

#[tokio::test]
async fn bind_without_listen_grant_is_denied() {
    let port = free_port();
    let rt = runtime(
        &receive_op(port),
        &grants(&[("allow_network", format!("udp://127.0.0.1:{port}"))]),
    );
    let e = run(&rt, "t.receive").await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
    // Nothing was bound: the port is still free.
    assert!(StdUdp::bind(format!("127.0.0.1:{port}")).is_ok());
}

/// Whether this host loops IPv4 multicast back over 127.0.0.1.
fn multicast_loopback_works(group: std::net::Ipv4Addr) -> bool {
    use socket2::{Domain, Protocol, Socket, Type};
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    let _ = s.set_reuse_address(true);
    if s.bind(&SocketAddr::from(([0, 0, 0, 0], 0)).into()).is_err()
        || s.join_multicast_v4(&group, &std::net::Ipv4Addr::LOCALHOST)
            .is_err()
        || s.set_multicast_if_v4(&std::net::Ipv4Addr::LOCALHOST)
            .is_err()
    {
        return false;
    }
    let port = s.local_addr().unwrap().as_socket().unwrap().port();
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = s.send_to(b"probe", &SocketAddr::from((group, port)).into());
    let mut buf = [std::mem::MaybeUninit::<u8>::uninit(); 16];
    s.recv(&mut buf).is_ok()
}

// vhco:test datagrams.exchange_datagrams -- multicast joins a group on the loopback interface, receives a datagram and leaves on scope exit
#[tokio::test]
async fn multicast_on_loopback() {
    let group = std::net::Ipv4Addr::new(239, 255, 42, 98);
    if !multicast_loopback_works(group) {
        eprintln!("multicast loopback is not delivered on this host; skipping (OS routing)");
        return;
    }
    let port = free_port();
    let src = format!(
        "operation t.mc\n    output json\n    with udp multicast \"{group}:{port}\" as socket\n        bind \"0.0.0.0:{port}\"\n        interface \"127.0.0.1\"\n        max_datagram 4096\n        message = socket.receive_from text timeout \"3s\"\n        return message.data\n    end\nend\n"
    );
    let rt = runtime(
        &src,
        &grants(&[
            ("allow_network", format!("udp://{group}:{port}")),
            ("allow_listen", format!("udp://0.0.0.0:{port}")),
        ]),
    );
    let sender = tokio::spawn(async move {
        let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        socket2::SockRef::from(&s)
            .set_multicast_if_v4(&std::net::Ipv4Addr::LOCALHOST)
            .ok();
        for _ in 0..60 {
            let _ = s.send_to(b"tick", (group, port));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    let v = run(&rt, "t.mc").await.unwrap();
    assert_eq!(v, Value::text("tick"));
    sender.abort();

    // Without the listener grant the join never happens.
    let rt = runtime(
        &src,
        &grants(&[("allow_network", format!("udp://{group}:{port}"))]),
    );
    assert_eq!(
        run(&rt, "t.mc").await.unwrap_err().code,
        "permission.denied"
    );
}

// vhco:test datagrams.exchange_datagrams -- docs/demos/08-udp telemetry.status and telemetry.receive behave as the README describes against local fixtures
#[tokio::test]
async fn demo_08_udp_bundle() {
    // The demo pins ports 7000–7002; skip when the host already uses them.
    let Ok(fixture) = UdpSocket::bind("127.0.0.1:7000").await else {
        eprintln!("127.0.0.1:7000 is busy; skipping the demo run");
        return;
    };
    tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        while let Ok((n, from)) = fixture.recv_from(&mut buf).await {
            if &buf[..n] == br#"{"command":"status"}"# {
                let _ = fixture.send_to(br#"{"state":"ready"}"#, from).await;
            }
        }
    });
    let rt = Runtime::builder()
        .file("docs/demos/08-udp/app.rivet")
        .build()
        .unwrap();
    let c = rt
        .request("telemetry.status", Value::Null, None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::object([("state", Value::text("ready"))]));
    // Default policy has no listener grant: receive is denied (exit 3).
    let e = rt
        .request("telemetry.receive", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");

    let Ok(peer) = UdpSocket::bind("127.0.0.1:7002").await else {
        eprintln!("127.0.0.1:7002 is busy; skipping the receive demo");
        return;
    };
    let rt = Runtime::builder()
        .file("docs/demos/08-udp/app.rivet")
        .policy_file("docs/demos/08-udp/policies/receive.json")
        .build()
        .unwrap();
    let pinger = tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        for _ in 0..60 {
            peer.send_to(br#"{"reading":42}"#, "127.0.0.1:7001")
                .await
                .unwrap();
            if let Ok(Ok((n, _))) =
                tokio::time::timeout(Duration::from_millis(50), peer.recv_from(&mut buf)).await
            {
                return Some(buf[..n].to_vec());
            }
        }
        None
    });
    let c = rt
        .request("telemetry.receive", Value::Null, None)
        .await
        .unwrap();
    assert_eq!(c.result.get("peer"), Some(&Value::text("127.0.0.1:7002")));
    assert_eq!(pinger.await.unwrap().unwrap(), br#"{"received":true}"#);
}

/// The static `io --check-policy` verdict of one operation: denied when any site is.
fn manifest_denies(rt: &Runtime, id: &str) -> bool {
    let q = rivet::domain::io_manifest::IoQuery {
        ids: vec![id.to_string()],
        check_policy: true,
        format: "json".into(),
        ..Default::default()
    };
    let report = rt.io(&q).unwrap();
    assert!(!report.manifest.sites.is_empty());
    report
        .manifest
        .sites
        .iter()
        .any(|s| s.decision.as_deref() == Some("denied"))
}

// vhco:test audit.inspect_effects -- B3: for UDP multicast (default and explicit bind) the manifest's check-policy decision matches the runtime permit (group connect + bind/multicast_join on the bind address) for every grant combination, and agrees for the docs/demos/08-udp bundle
#[tokio::test]
async fn multicast_manifest_agrees_with_runtime_permits() {
    let group = "239.255.42.97";
    let port = free_port();
    let default_bind = format!(
        "operation t.mc\n    output json\n    with udp multicast \"{group}:{port}\" as socket\n        max_datagram 512\n        return socket.receive text timeout \"50ms\"\n    end\nend\n"
    );
    let explicit_bind = format!(
        "operation t.mc\n    output json\n    with udp multicast \"{group}:{port}\" as socket\n        bind \"0.0.0.0:{port}\"\n        max_datagram 512\n        return socket.receive text timeout \"50ms\"\n    end\nend\n"
    );
    let net = ("allow_network", format!("udp://{group}:{port}"));
    let listen_any = ("allow_listen", format!("udp://0.0.0.0:{port}"));
    let listen_group = ("allow_listen", format!("udp://{group}:{port}"));
    let combos: Vec<Vec<(&str, String)>> = vec![
        vec![],
        vec![net.clone()],
        vec![listen_any.clone()],
        vec![listen_group.clone()],
        vec![net.clone(), listen_group.clone()],
        vec![net.clone(), listen_any.clone()],
    ];
    for src in [&default_bind, &explicit_bind] {
        for g in &combos {
            let policy = if g.is_empty() {
                r#"{"version": 1}"#.to_string()
            } else {
                grants(g)
            };
            let rt = runtime(src, &policy);
            let static_denied = manifest_denies(&rt, "t.mc");
            let runtime_denied = matches!(
                run(&rt, "t.mc").await,
                Err(e) if e.code == "permission.denied"
            );
            assert_eq!(
                static_denied, runtime_denied,
                "manifest and runtime disagree for grants {g:?}"
            );
        }
    }

    // docs/demos/08-udp: the default policy denies only the listener; receive.json
    // grants the listener and the reply peer but not the status peer.
    for policy in [None, Some("docs/demos/08-udp/policies/receive.json")] {
        let mut b = Runtime::builder().file("docs/demos/08-udp/app.rivet");
        if let Some(p) = policy {
            b = b.policy_file(p);
        }
        let rt = b.build().unwrap();
        let receive = manifest_denies(&rt, "telemetry.receive");
        let status = manifest_denies(&rt, "telemetry.status");
        match policy {
            None => assert!(!status && receive, "demo default policy"),
            Some(_) => assert!(status && !receive, "demo receive.json policy"),
        }
    }
}
