//! Authorization helpers shared by the transport use cases: one permit per
//! actual attempt, and the post-DNS private-range (rebinding) check.
//!
//! ```text
//!  target URL ──authorize(allow_network connect)──▶ host is IP literal? ── yes ─▶ dial it
//!                                                        │ no
//!                                                  resolve(host) ──▶ each IP: private & deny_private_ranges?
//!                                                        │                 yes ─▶ authorize scheme://IP:port/ (needs literal grant)
//!                                                        ▼                 no  ─▶ dial this IP (never re-resolve)
//! ```

use super::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use super::ports::PolicyEvaluator;
use super::transports::{EffectOrigin, is_private_ip};
use super::{ErrorKind, RivetError, RivetResult, Value};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};

/// Authorize one intent; a denial is `permission.denied` naming the rule.
pub fn authorize(
    evaluator: &dyn PolicyEvaluator,
    origin: &EffectOrigin,
    capability: Capability,
    verb: AccessVerb,
    target: EffectTarget,
) -> RivetResult<()> {
    let permit = evaluator.evaluate(&EffectIntent {
        capability,
        verb,
        target: target.clone(),
        operation_id: origin.operation_id.clone(),
        effect_id: None,
        span: origin.span.clone(),
    });
    if permit.decision == Decision::Denied {
        return Err(RivetError::permission(format!(
            "{} {} {} denied: {}",
            capability.as_str(),
            verb.as_str(),
            target.as_str(),
            permit.rule
        ))
        .with_span(origin.span.clone())
        .with_details(Value::object([
            ("capability", Value::text(capability.as_str())),
            ("access", Value::text(verb.as_str())),
            ("target", Value::text(target.as_str())),
        ])));
    }
    Ok(())
}

fn ip_text(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(a) => a.to_string(),
        IpAddr::V6(a) => format!("[{a}]"),
    }
}

/// Pick the address to dial for `host:port` after the origin was authorized.
/// IP literals were already checked by the origin permit (the broker applies
/// the private-range rule to literals). Names are resolved once; a private
/// result is used only when a grant names that IP literally, so a hostname
/// grant cannot be rebound to an internal address.
pub async fn checked_addr<F, Fut>(
    evaluator: &dyn PolicyEvaluator,
    origin: &EffectOrigin,
    scheme: &str,
    host: &str,
    port: u16,
    resolve: F,
) -> RivetResult<SocketAddr>
where
    F: FnOnce(String, u16) -> Fut,
    Fut: Future<Output = RivetResult<Vec<IpAddr>>>,
{
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    if bare.eq_ignore_ascii_case("localhost") {
        // The origin permit already treated `localhost` as the loopback literal.
        return Ok(SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port));
    }
    let ips = resolve(bare.to_string(), port).await?;
    if ips.is_empty() {
        return Err(RivetError::new(
            ErrorKind::Dns,
            "dns.no_address",
            format!("{bare} resolved to no address"),
        ));
    }
    let deny_private = evaluator.policy().network.deny_private_ranges;
    let mut last: Option<RivetError> = None;
    for ip in ips {
        if deny_private && is_private_ip(ip) {
            let target = EffectTarget::Url(format!("{scheme}://{}:{port}/", ip_text(ip)));
            match authorize(
                evaluator,
                origin,
                Capability::Network,
                AccessVerb::Connect,
                target,
            ) {
                Ok(()) => return Ok(SocketAddr::new(ip, port)),
                Err(e) => last = Some(e),
            }
        } else {
            return Ok(SocketAddr::new(ip, port));
        }
    }
    Err(last
        .map(|mut e| {
            e.message = format!("{bare} resolved to a private address: {}", e.message);
            e
        })
        .unwrap_or_else(|| RivetError::permission(format!("{bare}: no permitted address"))))
}
