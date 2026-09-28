use super::ports::{PolicyEvaluator, QuicDriver};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::transport::{
    EffectContext, QuicPlan, QuicResult, QuicStep, is_private_ip, join_host_port, literal_ip,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use std::net::SocketAddr;

/// Upper bound for `max_streams` (host-wide task budget).
pub const MAX_STREAMS_CEILING: u32 = 1024;

/// Parse `quic://host:port` / `https://host[:port]` into (scheme, host, port).
pub fn parse_quic_endpoint(endpoint: &str) -> RivetResult<(String, String, u16)> {
    let bad = |why: &str| {
        RivetError::validation(
            "validation.quic_endpoint",
            format!("`{endpoint}` {why}; expected quic://HOST:PORT or https://HOST[:PORT]"),
        )
    };
    let url = url::Url::parse(endpoint).map_err(|_| bad("is not a URL"))?;
    let scheme = url.scheme().to_string();
    if scheme != "quic" && scheme != "https" {
        return Err(bad("has an unsupported scheme"));
    }
    let host = match url.host() {
        Some(url::Host::Ipv6(a)) => a.to_string(),
        Some(h) => h.to_string(),
        None => return Err(bad("has no host")),
    };
    let port = url
        .port()
        .or(if scheme == "https" { Some(443) } else { None })
        .ok_or_else(|| bad("needs an explicit port"))?;
    if !(url.path().is_empty() || url.path() == "/") || url.query().is_some() {
        return Err(bad("must not carry a path or query"));
    }
    Ok((scheme, host, port))
}

// vhco:usecase quic.exchange_quic(input: QuicPlan) -> QuicResult needs QuicDriver
// vhco:label Exchange quic
// vhco:about Validates one step of a scoped native QUIC connection (connect, open/accept a stream, send, receive, finish_send, datagrams, close), authorizes the logical quic origin, its resolved UDP destination and any TLS files before the handshake, and runs the step through QuicDriver with 0-RTT and migration off.
// vhco:example input={endpoint:"quic://engine.example.com:4433", alpn:"rivet-rpc/1", max_streams:8, steps:["connect"]} => { "value": null, "stream_count": 0, "negotiated_alpn": "rivet-rpc/1", "effects": "none" }
pub async fn exchange_quic(
    mut input: QuicPlan,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn QuicDriver,
) -> RivetResult<QuicResult> {
    // vhco:todo validate_quic -- parse the endpoint (quic://HOST:PORT with an explicit port, or https://HOST[:PORT] defaulting to 443; anything else is validation.quic_endpoint); an empty alpn is validation quic.alpn_required; max_streams outside 1..=1024 is validation.quic_option; `early_data true` is unsupported.quic_early_data; tls cert_file without key_file (or the reverse) is validation.quic_option; on Connect authorize allow_read read for each tls ca_file/cert_file/key_file path, then allow_network connect SCHEME://HOST:PORT (the logical QUIC origin; its UDP transport is covered by that permit and never by a raw udp grant), then resolve a hostname and keep the first address that is public or, when deny_private_ranges is on, re-authorized as SCHEME://IP:PORT (a hostname never inherits a private address), recording it as peer_addr so the driver dials exactly that address; datagram steps without `datagrams true` are validation quic.datagrams_disabled; a framing max_frame of 0 is validation.quic_option; every denial happens before the handshake, so a denied target sends zero packets
    // vhco:step bounds guard -- static option checks (alpn, max_streams, early data, cert/key pairing)
    if input.alpn.is_empty() {
        return Err(RivetError::validation(
            "quic.alpn_required",
            "`with quic` needs `alpn \"ID\"`; ALPN is required and must match the peer",
        ));
    }
    if input.max_streams == 0 || input.max_streams > MAX_STREAMS_CEILING {
        return Err(RivetError::validation(
            "validation.quic_option",
            format!(
                "max_streams {} is outside 1..={MAX_STREAMS_CEILING}",
                input.max_streams
            ),
        ));
    }
    if input.early_data {
        return Err(RivetError::unsupported(
            "unsupported.quic_early_data",
            "0-RTT early data is disabled: replayed early data could repeat an operation",
        ));
    }
    if input.tls.cert_file.is_some() != input.tls.key_file.is_some() {
        return Err(RivetError::validation(
            "validation.quic_option",
            "`tls cert_file` and `tls key_file` must be given together",
        ));
    }
    // vhco:todo authorize_path_cleanup -- `migration true` is refused with unsupported.quic_migration before any packet (no per-path policy hook exists, so a peer-suggested address can never be probed); the client never rebinds, and a close step closes the connection after its streams (children close first because their scopes are inner), waiting a bounded time for the peer; application writes are never replayed on reconnect or fallback
    // vhco:step migration guard -- migration true fails closed
    if input.migration {
        return Err(RivetError::unsupported(
            "unsupported.quic_migration",
            "QUIC connection migration is not available: every new path would need its own policy approval before probing; use `migration false`",
        ));
    }
    let ctx = input.context.clone();
    let steps = input.steps.clone();
    for step in &steps {
        match step {
            QuicStep::Connect => {
                // vhco:step files evaluator.evaluate -- tls ca/cert/key files are read intents checked before connecting
                for path in [
                    &input.tls.ca_file,
                    &input.tls.cert_file,
                    &input.tls.key_file,
                ]
                .into_iter()
                .flatten()
                {
                    authorize(
                        evaluator,
                        &ctx,
                        Capability::Read,
                        AccessVerb::Read,
                        EffectTarget::Path(path.clone()),
                    )?;
                }
                // vhco:step origin evaluator.evaluate -- allow_network connect for the logical quic origin
                let (scheme, host, port) = parse_quic_endpoint(&input.endpoint)?;
                authorize(
                    evaluator,
                    &ctx,
                    Capability::Network,
                    AccessVerb::Connect,
                    EffectTarget::Url(format!("{scheme}://{}", join_host_port(&host, port))),
                )?;
                // vhco:step resolve driver.resolve -- resolved UDP destination re-checked against private ranges
                input.peer_addr =
                    Some(checked_addr(evaluator, driver, &ctx, &scheme, &host, port).await?);
            }
            QuicStep::OpenStream { framing, .. } | QuicStep::AcceptStream { framing, .. } => {
                if framing.max_frame == 0 {
                    return Err(RivetError::validation(
                        "validation.quic_option",
                        "framing max_frame must be at least 1",
                    ));
                }
            }
            QuicStep::SendDatagram { .. } | QuicStep::ReceiveDatagram { .. } => {
                if !input.datagrams {
                    return Err(RivetError::validation(
                        "quic.datagrams_disabled",
                        "QUIC DATAGRAM frames need `datagrams true` on the connection",
                    ));
                }
            }
            _ => {}
        }
    }
    // vhco:todo drive_streams -- hand the authorized step to QuicDriver: connect dials peer_addr with server_name as the verified TLS identity (ca_file replaces the trust roots), the single ALPN, 0-RTT off and migration off, fails a mismatched ALPN or certificate as kind tls and requires negotiated DATAGRAM when `datagrams true` (quic.datagrams_unavailable); open/accept create a child stream only while fewer than max_streams are active (limit.quic_streams); send frames the payload (raw, newline, length32 with explicit endian, delimiter) and a uni-accepted stream cannot send (validation quic.direction); receive returns one frame bounded by max_frame (quic.frame_too_large) or Null at the peer's FIN; finish_send sends FIN only; a peer reset is quic.stream_reset and STOP_SENDING is quic.stop_sending on that stream only; send_datagram rejects payloads above the negotiated size (quic.datagram_too_large); every wait is bounded by min(step timeout, resource timeout, request deadline) and fails kind timeout
    // vhco:step exchange driver.exchange -- the driver runs the step on the scope's connection
    // vhco:error permission_denied -- the origin, a resolved private address or a tls file is not granted => permission.denied (exit 3) returns before any packet
    // vhco:error tls -- certificate or ALPN mismatch during the handshake => kind tls (quic.tls / quic.alpn_mismatch, exit 5) returns
    // vhco:error unsupported -- migration true or early data => unsupported.quic_migration / unsupported.quic_early_data (exit 5) returns before connecting
    driver.exchange(input).await
}

fn authorize(
    evaluator: &dyn PolicyEvaluator,
    ctx: &EffectContext,
    capability: Capability,
    verb: AccessVerb,
    target: EffectTarget,
) -> RivetResult<()> {
    let permit = evaluator.evaluate(&EffectIntent {
        capability,
        verb,
        target: target.clone(),
        operation_id: ctx.operation_id.clone(),
        effect_id: None,
        span: ctx.span.clone(),
    });
    if permit.decision == Decision::Denied {
        return Err(RivetError::permission(format!(
            "{} {} {} denied: {}",
            capability.as_str(),
            verb.as_str(),
            target.as_str(),
            permit.rule
        ))
        .with_span(ctx.span.clone())
        .with_details(Value::object([
            ("capability", Value::text(capability.as_str())),
            ("access", Value::text(verb.as_str())),
            ("target", Value::text(target.as_str())),
        ])));
    }
    Ok(())
}

async fn checked_addr(
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn QuicDriver,
    ctx: &EffectContext,
    scheme: &str,
    host: &str,
    port: u16,
) -> RivetResult<SocketAddr> {
    if let Some(ip) = literal_ip(host) {
        return Ok(SocketAddr::new(ip, port));
    }
    let addrs = driver.resolve(host, port).await?;
    let deny_private = evaluator.policy().network.deny_private_ranges;
    let mut refusal = None;
    for addr in addrs {
        if deny_private && is_private_ip(addr.ip()) {
            match authorize(
                evaluator,
                ctx,
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url(format!("{scheme}://{addr}")),
            ) {
                Ok(()) => return Ok(addr),
                Err(e) => refusal = Some(e),
            }
        } else {
            return Ok(addr);
        }
    }
    Err(refusal.unwrap_or_else(|| {
        RivetError::new(
            ErrorKind::Dns,
            "dns.no_address",
            format!("{host} resolved to no address"),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::errors::EffectsStatus;
    use crate::domain::policy::{Grant, Permit, Policy};
    use crate::domain::transport::{DEFAULT_MAX_STREAMS, QuicTls};
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::time::Instant;

    struct Grants(Policy);
    impl PolicyEvaluator for Grants {
        fn evaluate(&self, i: &EffectIntent) -> Permit {
            let ok = self.0.grants.iter().any(|g| {
                g.capability == i.capability && g.targets.iter().any(|t| t == i.target.as_str())
            });
            Permit {
                intent: i.clone(),
                decision: if ok {
                    Decision::Allowed
                } else {
                    Decision::Denied
                },
                rule: "test".into(),
            }
        }
        fn policy(&self) -> &Policy {
            &self.0
        }
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<QuicPlan>>);
    #[async_trait]
    impl QuicDriver for Recorder {
        async fn resolve(&self, _h: &str, port: u16) -> RivetResult<Vec<SocketAddr>> {
            Ok(vec![
                SocketAddr::from(([10, 0, 0, 9], port)),
                SocketAddr::from(([93, 184, 216, 34], port)),
            ])
        }
        async fn exchange(&self, plan: QuicPlan) -> RivetResult<QuicResult> {
            self.0.lock().unwrap().push(plan);
            Ok(QuicResult {
                value: Value::Null,
                stream_count: 0,
                negotiated_alpn: "x".into(),
                effects: EffectsStatus::None,
            })
        }
    }

    fn grants(g: &[(Capability, &str)]) -> Grants {
        Grants(Policy {
            present: true,
            grants: g
                .iter()
                .map(|(c, t)| Grant {
                    capability: *c,
                    targets: vec![t.to_string()],
                    access: None,
                })
                .collect(),
            ..Policy::default()
        })
    }

    fn plan(endpoint: &str) -> QuicPlan {
        QuicPlan {
            endpoint: endpoint.into(),
            server_name: "engine.example.com".into(),
            alpn: "rivet-rpc/1".into(),
            max_streams: DEFAULT_MAX_STREAMS,
            datagrams: false,
            migration: false,
            early_data: false,
            timeout_ms: None,
            tls: QuicTls::default(),
            peer_addr: None,
            steps: vec![QuicStep::Connect],
            context: EffectContext {
                operation_id: "t.op".into(),
                span: None,
                deadline: Instant::now(),
            },
        }
    }

    // vhco:test quic.exchange_quic -- the logical quic origin is authorized, private resolutions are skipped and the checked address reaches the driver
    #[tokio::test]
    async fn connect_authorizes_origin_and_resolved_address() {
        let d = Recorder::default();
        let mut ev = grants(&[(Capability::Network, "quic://engine.example.com:4433")]);
        ev.0.network.deny_private_ranges = true;
        exchange_quic(plan("quic://engine.example.com:4433"), &ev, &d)
            .await
            .unwrap();
        assert_eq!(
            d.0.lock().unwrap()[0].peer_addr,
            Some("93.184.216.34:4433".parse().unwrap())
        );
        let e = exchange_quic(plan("quic://engine.example.com:4433"), &grants(&[]), &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        // A raw UDP grant never authorizes a QUIC origin.
        let udp = grants(&[(Capability::Network, "udp://engine.example.com:4433")]);
        assert!(
            exchange_quic(plan("quic://engine.example.com:4433"), &udp, &d)
                .await
                .is_err()
        );
        assert_eq!(d.0.lock().unwrap().len(), 1);
    }

    // vhco:test quic.exchange_quic -- migration, early data, missing ALPN, ungranted tls files and disabled datagrams fail before the driver
    #[tokio::test]
    async fn static_refusals() {
        let d = Recorder::default();
        let ev = grants(&[(Capability::Network, "quic://127.0.0.1:4433")]);
        let mut p = plan("quic://127.0.0.1:4433");
        p.migration = true;
        assert_eq!(
            exchange_quic(p, &ev, &d).await.unwrap_err().code,
            "unsupported.quic_migration"
        );
        let mut p = plan("quic://127.0.0.1:4433");
        p.early_data = true;
        assert_eq!(
            exchange_quic(p, &ev, &d).await.unwrap_err().code,
            "unsupported.quic_early_data"
        );
        let mut p = plan("quic://127.0.0.1:4433");
        p.alpn.clear();
        assert_eq!(
            exchange_quic(p, &ev, &d).await.unwrap_err().code,
            "quic.alpn_required"
        );
        let mut p = plan("quic://127.0.0.1:4433");
        p.tls.ca_file = Some("./ca.pem".into());
        assert_eq!(
            exchange_quic(p, &ev, &d).await.unwrap_err().code,
            "permission.denied"
        );
        let mut p = plan("quic://127.0.0.1:4433");
        p.steps = vec![QuicStep::SendDatagram { payload: vec![1] }];
        assert_eq!(
            exchange_quic(p, &ev, &d).await.unwrap_err().code,
            "quic.datagrams_disabled"
        );
        assert!(parse_quic_endpoint("quic://h").is_err());
        assert_eq!(
            parse_quic_endpoint("https://h").unwrap(),
            ("https".into(), "h".into(), 443)
        );
        assert!(d.0.lock().unwrap().is_empty());
    }
}
