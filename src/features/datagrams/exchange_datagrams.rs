use super::ports::{DatagramDriver, PolicyEvaluator};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::transport::{
    DatagramMode, DatagramPlan, DatagramResult, DatagramStep, EffectContext, MAX_UDP_PAYLOAD,
    is_private_ip, join_host_port, literal_ip, split_host_port,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

// vhco:usecase datagrams.exchange_datagrams(input: DatagramPlan) -> DatagramResult needs DatagramDriver
// vhco:label Exchange datagrams
// vhco:about Validates one step of a scoped UDP socket (open, send, send_to, receive, receive_from, close), authorizes every peer, listener bind and multicast join it implies before the socket exists or a packet leaves, then runs the step on the scope's own socket through DatagramDriver.
// vhco:example input={mode:"connected", peer:"127.0.0.1:7000", max_datagram:8192, steps:["open"]} => { "messages": [], "sent_count": 0, "effects": "none" }
pub async fn exchange_datagrams(
    mut input: DatagramPlan,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn DatagramDriver,
) -> RivetResult<DatagramResult> {
    // vhco:todo authorize_datagram_plan -- reject max_datagram outside 1..=65507 (validation.udp_option); on Open: connected mode authorizes allow_network connect udp://PEER (the OS-chosen ephemeral local bind is part of that peer permit, no listen check), resolves a hostname and keeps only addresses that pass the private-range re-check (udp://IP:PORT authorized again when deny_private_ranges is on and the address is private), and records the checked peer_addr; bind mode needs an IP-literal bind and authorizes allow_listen bind udp://BIND; multicast mode needs a multicast IP-literal group and an IP-literal bind (default 0.0.0.0:PORT / [::]:PORT) and authorizes allow_network connect udp://GROUP (outbound group authority), allow_listen bind udp://BIND and allow_listen multicast_join udp://BIND; every send_to authorizes allow_network connect udp://PEER on its own (a peer taken from a received datagram inherits nothing) and records its checked addr; send/send_to payloads larger than max_datagram fail udp.message_too_large; send on a bind socket (no peer) is validation udp.no_peer and send_to on a connected socket is validation udp.connected; the first denial stops the step before any socket, bind, join or packet
    // vhco:step bounds guard -- max_datagram must fit a UDP payload (1..=65507)
    if input.max_datagram == 0 || input.max_datagram > MAX_UDP_PAYLOAD {
        return Err(RivetError::validation(
            "validation.udp_option",
            format!(
                "max_datagram {} is outside 1..={MAX_UDP_PAYLOAD}",
                input.max_datagram
            ),
        ));
    }
    let ctx = input.context.clone();
    let mut steps = std::mem::take(&mut input.steps);
    for step in steps.iter_mut() {
        match step {
            DatagramStep::Open => {
                // vhco:step open authorize_open -- peer / bind / group+bind+join intents for the socket's mode, then the checked peer address
                authorize_open(&mut input, evaluator, driver, &ctx).await?;
            }
            DatagramStep::Send { payload } => {
                // vhco:step send size_guard -- a send needs a default destination and must fit max_datagram
                if input.mode == DatagramMode::Bind {
                    return Err(RivetError::validation(
                        "udp.no_peer",
                        "a `udp bind` socket has no default peer; use send_to PEER",
                    ));
                }
                size_guard(payload.len(), input.max_datagram)?;
            }
            DatagramStep::SendTo {
                peer,
                addr,
                payload,
            } => {
                // vhco:step send_to evaluator.evaluate -- every send_to peer is authorized on its own before the packet
                if input.mode == DatagramMode::Connected {
                    return Err(RivetError::validation(
                        "udp.connected",
                        "a connected `udp` socket sends only to its peer; use send",
                    ));
                }
                size_guard(payload.len(), input.max_datagram)?;
                let (host, port) = parse_peer(peer)?;
                let target = format!("udp://{}", join_host_port(&host, port));
                authorize(
                    evaluator,
                    &ctx,
                    Capability::Network,
                    AccessVerb::Connect,
                    &target,
                )?;
                *addr = Some(checked_addr(evaluator, driver, &ctx, &host, port).await?);
            }
            DatagramStep::Receive { .. }
            | DatagramStep::ReceiveFrom { .. }
            | DatagramStep::Close => {}
        }
    }
    input.steps = steps;
    // vhco:todo exchange_scoped -- hand the authorized one-step plan to DatagramDriver on the scope's own socket: open creates/binds/connects/joins it (broadcast off), send reports local acceptance only (effects unknown, never delivery), receive/receive_from wait at most min(step timeout, resource timeout, request deadline) and fail kind timeout without retrying, a datagram longer than max_datagram fails udp.truncated (kind protocol) instead of being parsed clipped, receive_from keeps the sender as host:port, close leaves multicast groups and drops the socket; driver errors propagate unchanged
    // vhco:step exchange driver.exchange -- the driver runs the step and returns received messages, the sent count and the effect status
    // vhco:error permission_denied -- a peer, bind, group or join intent is denied, or a resolved private address is not granted literally => permission.denied (exit 3) returns with zero packets
    // vhco:error truncated -- an incoming datagram exceeds max_datagram => udp.truncated (kind protocol, exit 5) returns
    // vhco:error timeout -- no datagram before the receive deadline => timeout (exit 6) returns, nothing is retried
    driver.exchange(input).await
}

fn size_guard(len: usize, max: u32) -> RivetResult<()> {
    if len > max as usize {
        return Err(RivetError::new(
            ErrorKind::Protocol,
            "udp.message_too_large",
            format!("datagram payload of {len} bytes exceeds max_datagram {max}"),
        ));
    }
    Ok(())
}

fn parse_peer(s: &str) -> RivetResult<(String, u16)> {
    split_host_port(s).ok_or_else(|| {
        RivetError::validation(
            "validation.udp_address",
            format!("`{s}` is not HOST:PORT (IPv6 as [ADDR]:PORT)"),
        )
    })
}

fn parse_literal(s: &str, what: &str) -> RivetResult<SocketAddr> {
    let (host, port) = parse_peer(s)?;
    let ip = literal_ip(&host).ok_or_else(|| {
        RivetError::validation(
            "validation.udp_address",
            format!("{what} `{s}` must be an IP literal"),
        )
    })?;
    Ok(SocketAddr::new(ip, port))
}

async fn authorize_open(
    input: &mut DatagramPlan,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn DatagramDriver,
    ctx: &EffectContext,
) -> RivetResult<()> {
    match input.mode {
        DatagramMode::Connected => {
            let peer = input.peer.clone().ok_or_else(|| {
                RivetError::validation("validation.udp_address", "`with udp` needs HOST:PORT")
            })?;
            let (host, port) = parse_peer(&peer)?;
            let target = format!("udp://{}", join_host_port(&host, port));
            authorize(
                evaluator,
                ctx,
                Capability::Network,
                AccessVerb::Connect,
                &target,
            )?;
            input.peer_addr = Some(checked_addr(evaluator, driver, ctx, &host, port).await?);
        }
        DatagramMode::Bind => {
            let bind = input.bind.clone().ok_or_else(|| {
                RivetError::validation("validation.udp_address", "`with udp bind` needs HOST:PORT")
            })?;
            let addr = parse_literal(&bind, "bind address")?;
            authorize(
                evaluator,
                ctx,
                Capability::Listen,
                AccessVerb::Bind,
                &format!("udp://{addr}"),
            )?;
        }
        DatagramMode::Multicast => {
            let group = input.multicast.clone().ok_or_else(|| {
                RivetError::validation(
                    "validation.udp_address",
                    "`with udp multicast` needs GROUP:PORT",
                )
            })?;
            let group_addr = parse_literal(&group, "multicast group")?;
            if !group_addr.ip().is_multicast() {
                return Err(RivetError::validation(
                    "validation.udp_address",
                    format!("`{group}` is not a multicast group address"),
                ));
            }
            let bind = match &input.bind {
                Some(b) => parse_literal(b, "bind address")?,
                None => {
                    let any: IpAddr = if group_addr.is_ipv4() {
                        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                    } else {
                        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
                    };
                    SocketAddr::new(any, group_addr.port())
                }
            };
            if bind.is_ipv4() != group_addr.is_ipv4() {
                return Err(RivetError::validation(
                    "validation.udp_address",
                    "multicast bind and group must be the same IP family",
                ));
            }
            input.bind = Some(bind.to_string());
            authorize(
                evaluator,
                ctx,
                Capability::Network,
                AccessVerb::Connect,
                &format!("udp://{group_addr}"),
            )?;
            authorize(
                evaluator,
                ctx,
                Capability::Listen,
                AccessVerb::Bind,
                &format!("udp://{bind}"),
            )?;
            authorize(
                evaluator,
                ctx,
                Capability::Listen,
                AccessVerb::MulticastJoin,
                &format!("udp://{bind}"),
            )?;
        }
    }
    Ok(())
}

fn authorize(
    evaluator: &dyn PolicyEvaluator,
    ctx: &EffectContext,
    capability: Capability,
    verb: AccessVerb,
    target: &str,
) -> RivetResult<()> {
    let permit = evaluator.evaluate(&EffectIntent {
        capability,
        verb,
        target: EffectTarget::Url(target.to_string()),
        operation_id: ctx.operation_id.clone(),
        effect_id: None,
        span: ctx.span.clone(),
    });
    if permit.decision == Decision::Denied {
        return Err(RivetError::permission(format!(
            "{} {} {target} denied: {}",
            capability.as_str(),
            verb.as_str(),
            permit.rule
        ))
        .with_span(ctx.span.clone())
        .with_details(Value::object([
            ("capability", Value::text(capability.as_str())),
            ("access", Value::text(verb.as_str())),
            ("target", Value::text(target)),
        ])));
    }
    Ok(())
}

/// The address to use for an authorized `host:port`: IP literals as written;
/// names resolved once, with private results re-authorized as `udp://IP:PORT`
/// when the policy denies private ranges (DNS-rebinding guard).
async fn checked_addr(
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn DatagramDriver,
    ctx: &EffectContext,
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
                &format!("udp://{addr}"),
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
    use crate::domain::transport::DatagramResult;
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
    struct Recorder(Mutex<Vec<DatagramPlan>>);
    #[async_trait]
    impl DatagramDriver for Recorder {
        async fn resolve(&self, _h: &str, port: u16) -> RivetResult<Vec<SocketAddr>> {
            Ok(vec![SocketAddr::from(([10, 0, 0, 5], port))])
        }
        async fn exchange(&self, plan: DatagramPlan) -> RivetResult<DatagramResult> {
            self.0.lock().unwrap().push(plan);
            Ok(DatagramResult {
                messages: vec![],
                sent_count: 0,
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

    fn plan(mode: DatagramMode, steps: Vec<DatagramStep>) -> DatagramPlan {
        DatagramPlan {
            mode,
            peer: Some("127.0.0.1:7000".into()),
            peer_addr: None,
            bind: Some("127.0.0.1:7001".into()),
            multicast: Some("239.0.0.1:5000".into()),
            interface: None,
            max_datagram: 16,
            timeout_ms: None,
            steps,
            context: EffectContext {
                operation_id: "t.op".into(),
                span: None,
                deadline: Instant::now(),
            },
        }
    }

    // vhco:test datagrams.exchange_datagrams -- a granted connected peer opens with its checked address; an ungranted one never reaches the driver
    #[tokio::test]
    async fn connected_open_is_authorized_before_the_driver() {
        let d = Recorder::default();
        let ev = grants(&[(Capability::Network, "udp://127.0.0.1:7000")]);
        exchange_datagrams(
            plan(DatagramMode::Connected, vec![DatagramStep::Open]),
            &ev,
            &d,
        )
        .await
        .unwrap();
        assert_eq!(
            d.0.lock().unwrap()[0].peer_addr,
            Some("127.0.0.1:7000".parse().unwrap())
        );
        let e = exchange_datagrams(
            plan(DatagramMode::Connected, vec![DatagramStep::Open]),
            &grants(&[]),
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        assert_eq!(d.0.lock().unwrap().len(), 1);
    }

    // vhco:test datagrams.exchange_datagrams -- send_to needs its own grant, oversized payloads and a missing bind grant are refused
    #[tokio::test]
    async fn send_to_bind_and_size_rules() {
        let d = Recorder::default();
        let ev = grants(&[
            (Capability::Listen, "udp://127.0.0.1:7001"),
            (Capability::Network, "udp://127.0.0.1:7002"),
        ]);
        exchange_datagrams(plan(DatagramMode::Bind, vec![DatagramStep::Open]), &ev, &d)
            .await
            .unwrap();
        let send_to = |peer: &str, n: usize| DatagramStep::SendTo {
            peer: peer.into(),
            addr: None,
            payload: vec![0; n],
        };
        exchange_datagrams(
            plan(DatagramMode::Bind, vec![send_to("127.0.0.1:7002", 3)]),
            &ev,
            &d,
        )
        .await
        .unwrap();
        let e = exchange_datagrams(
            plan(DatagramMode::Bind, vec![send_to("127.0.0.1:7003", 3)]),
            &ev,
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let e = exchange_datagrams(
            plan(DatagramMode::Bind, vec![send_to("127.0.0.1:7002", 17)]),
            &ev,
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "udp.message_too_large");
        let e = exchange_datagrams(
            plan(DatagramMode::Bind, vec![DatagramStep::Open]),
            &grants(&[]),
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "permission.denied");
    }

    // vhco:test datagrams.exchange_datagrams -- multicast needs the group connect grant plus bind and join on the listener, and a hostname resolving to a private address needs a literal grant
    #[tokio::test]
    async fn multicast_and_private_resolution() {
        let d = Recorder::default();
        let mut p = plan(DatagramMode::Multicast, vec![DatagramStep::Open]);
        p.bind = None;
        let ev = grants(&[
            (Capability::Network, "udp://239.0.0.1:5000"),
            (Capability::Listen, "udp://0.0.0.0:5000"),
        ]);
        exchange_datagrams(p.clone(), &ev, &d).await.unwrap();
        assert_eq!(
            d.0.lock().unwrap().last().unwrap().bind.as_deref(),
            Some("0.0.0.0:5000")
        );
        let only_group = grants(&[(Capability::Network, "udp://239.0.0.1:5000")]);
        assert!(exchange_datagrams(p, &only_group, &d).await.is_err());

        let mut named = plan(DatagramMode::Connected, vec![DatagramStep::Open]);
        named.peer = Some("telemetry.internal:7000".into());
        let mut ev = grants(&[(Capability::Network, "udp://telemetry.internal:7000")]);
        ev.0.network.deny_private_ranges = true;
        let e = exchange_datagrams(named, &ev, &d).await.unwrap_err();
        assert_eq!(e.code, "permission.denied");
    }
}
