use crate::domain::policy::{
    Capability, Decision, EffectIntent, EffectTarget, Grant, Permit, Policy,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Component, Path, PathBuf};

// vhco:usecase policy.authorize_effect(input: EffectIntent) -> Permit needs PolicyEvaluator
// vhco:label Authorize an effect attempt
// vhco:about Decides one actual effect attempt against the effective policy: deny entries win, a grant must match capability, target selector and access verb, private network ranges are refused unless named literally, and no policy.json means every effect is denied.
// vhco:example input={capability:"allow_write", verb:"update", target:"./out/a.json"} => { "decision": "denied", "rule": "grant allow_write ./out/** access [create] does not include `update`" }
pub fn authorize_effect(intent: &EffectIntent, policy: &Policy, bundle_root: &str) -> Permit {
    let permit = |decision: Decision, rule: String| Permit {
        intent: intent.clone(),
        decision,
        rule,
    };

    // vhco:todo intersect_policy -- effective policy = host ceiling ∩ policy.json (or deny-by-default when absent) ∩ per-request restriction; callers can only narrow; deny entries override grants
    // vhco:step absent guard -- no policy.json → deny every application effect with a rule naming the missing file
    if !policy.present {
        return permit(
            Decision::Denied,
            format!(
                "no policy.json: {} is denied by default (add a grant for {})",
                intent.capability.as_str(),
                intent.target.as_str()
            ),
        );
    }
    // vhco:step ceiling policy.ceiling -- a library host ceiling decides too: an attempt it denies is denied whatever policy.json grants (intersection)
    if let Some(ceiling) = &policy.ceiling {
        let c = authorize_effect(intent, ceiling, bundle_root);
        if c.decision == Decision::Denied {
            return permit(Decision::Denied, format!("host ceiling: {}", c.rule));
        }
    }
    let target = normalize_target(&intent.target, intent.capability, bundle_root);

    // vhco:step deny policy.deny -- a matching deny entry (with or without access) wins over every grant
    for d in &policy.deny {
        if d.capability == intent.capability
            && d.allows_verb(intent.verb)
            && d.targets
                .iter()
                .any(|t| selector_matches(t, &target, d.capability, &policy.base_dir))
        {
            return permit(
                Decision::Denied,
                format!("deny {} {}", d.capability.as_str(), d.targets.join(", ")),
            );
        }
    }

    // vhco:todo check_target -- validate the resolved target of every attempt (redirects, DNS results, replacements); with deny_private_ranges refuse RFC1918, loopback, link-local, 169.254.169.254, fc00::/7 and fe80::/10 unless a grant names that IP/CIDR literally
    // vhco:step private private_ip -- a literal private address needs a grant that names it
    if let Some(ip) = target_ip(&target, intent.capability)
        && policy.network.deny_private_ranges
        && is_private(ip)
        && !names_ip_literally(policy, intent.capability, ip, &target)
    {
        return permit(
            Decision::Denied,
            format!(
                "{ip} is a private/loopback/link-local address; grant it literally (e.g. \"{}\") to allow it",
                target.display
            ),
        );
    }

    // vhco:todo check_access -- a grant matches only when capability and selector match and, if the grant lists `access`, the intent's verb is listed; absent access = every verb of the capability
    // vhco:step grants policy.grants -- first grant matching capability + selector + verb allows
    let mut verb_mismatch: Option<&Grant> = None;
    for g in &policy.grants {
        if g.capability != intent.capability
            || !g
                .targets
                .iter()
                .any(|t| selector_matches(t, &target, g.capability, &policy.base_dir))
        {
            continue;
        }
        if g.allows_verb(intent.verb) {
            return permit(
                Decision::Allowed,
                format!("grant {} {}", g.capability.as_str(), g.targets.join(", ")),
            );
        }
        verb_mismatch = Some(g);
    }
    // vhco:todo check_auth_quic -- auth/credential intents need allow_auth/allow_credentials in addition to endpoint permits; QUIC/UDP permits bind protocol and destination so QUIC's internal UDP never authorizes raw UDP (enforced by distinct capability/target pairs)
    // vhco:error permission_denied -- no matching grant, a deny entry, a private address or a verb outside the grant's access => permission.denied (exit 3) handled by the caller
    match verb_mismatch {
        Some(g) => permit(
            Decision::Denied,
            format!(
                "grant {} {} access [{}] does not include `{}`",
                g.capability.as_str(),
                g.targets.join(", "),
                g.access
                    .as_ref()
                    .map(|v| v.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(", "))
                    .unwrap_or_default(),
                intent.verb.as_str()
            ),
        ),
        None => permit(
            Decision::Denied,
            format!(
                "no grant for {} {}",
                intent.capability.as_str(),
                target.display
            ),
        ),
    }
}

/// A target in comparable form.
#[derive(Debug, Clone)]
pub struct NormalTarget {
    pub display: String,
    /// Absolute-or-root-relative normalized path, URL string, env name or logical name.
    pub key: String,
    pub url: Option<url::Url>,
}

fn normalize_target(target: &EffectTarget, cap: Capability, root: &str) -> NormalTarget {
    match target {
        EffectTarget::Path(p) if cap.is_path_like() => {
            let key = normalize_path(root, p);
            NormalTarget {
                display: p.clone(),
                key,
                url: None,
            }
        }
        EffectTarget::Url(u) => NormalTarget {
            display: u.clone(),
            key: u.clone(),
            url: url::Url::parse(u).ok(),
        },
        other => NormalTarget {
            display: other.as_str().to_string(),
            key: other.as_str().to_string(),
            url: None,
        },
    }
}

/// Lexically join and normalize (`.`/`..`) a path under a base directory.
pub fn normalize_path(base: &str, p: &str) -> String {
    let joined = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        Path::new(base).join(p)
    };
    let mut out: Vec<String> = Vec::new();
    let mut absolute = false;
    for c in joined.components() {
        match c {
            Component::RootDir => absolute = true,
            Component::CurDir => {}
            Component::ParentDir => {
                if out.last().is_some_and(|l| l != "..") {
                    out.pop();
                } else if !absolute {
                    out.push("..".into());
                }
            }
            Component::Normal(s) => out.push(s.to_string_lossy().to_string()),
            Component::Prefix(p) => out.push(p.as_os_str().to_string_lossy().to_string()),
        }
    }
    let s = out.join("/");
    let s = if absolute {
        format!("/{s}")
    } else if s.is_empty() {
        ".".into()
    } else {
        s
    };
    if cfg!(any(target_os = "macos", windows)) {
        s.to_lowercase()
    } else {
        s
    }
}

fn selector_matches(
    selector: &str,
    target: &NormalTarget,
    cap: Capability,
    base_dir: &str,
) -> bool {
    if selector == "*" {
        return true;
    }
    if cap.is_path_like() {
        let norm = normalize_path(base_dir, selector);
        if let Some(prefix) = norm.strip_suffix("/**")
            && target.key == prefix
        {
            return true;
        }
        return globset::GlobBuilder::new(&norm)
            .literal_separator(true)
            .build()
            .map(|g| g.compile_matcher().is_match(&target.key))
            .unwrap_or(false);
    }
    if let (Some(turl), Ok(gurl)) = (&target.url, url::Url::parse(selector)) {
        if gurl.scheme() != turl.scheme()
            || gurl.host_str().map(str::to_lowercase) != turl.host_str().map(str::to_lowercase)
        {
            return false;
        }
        if gurl.port_or_known_default() != turl.port_or_known_default() {
            return false;
        }
        let gpath = gurl.path();
        return gpath.is_empty()
            || gpath == "/"
            || turl.path().starts_with(gpath.trim_end_matches('*'));
    }
    if let Some(turl) = &target.url {
        // IP or CIDR selector against a URL host.
        if let Some(ip) = host_ip(turl) {
            return ip_in_selector(ip, selector);
        }
        return false;
    }
    globset::Glob::new(selector)
        .map(|g| g.compile_matcher().is_match(&target.key))
        .unwrap_or(false)
}

fn host_ip(u: &url::Url) -> Option<IpAddr> {
    match u.host()? {
        url::Host::Ipv4(a) => Some(IpAddr::V4(a)),
        url::Host::Ipv6(a) => Some(IpAddr::V6(a)),
        url::Host::Domain(d) if d.eq_ignore_ascii_case("localhost") => {
            Some(IpAddr::V4(Ipv4Addr::LOCALHOST))
        }
        // Non-special schemes (udp://, quic://, tcp://) keep IP literals as opaque
        // host text; parse them so the private-range rule still applies.
        url::Host::Domain(d) => d
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .ok(),
    }
}

fn target_ip(target: &NormalTarget, cap: Capability) -> Option<IpAddr> {
    if !matches!(
        cap,
        Capability::Network | Capability::Listen | Capability::Grpc
    ) {
        return None;
    }
    target.url.as_ref().and_then(host_ip)
}

fn ip_in_selector(ip: IpAddr, selector: &str) -> bool {
    if let Ok(single) = selector.parse::<IpAddr>() {
        return single == ip;
    }
    let Some((net, bits)) = selector.split_once('/') else {
        return false;
    };
    let (Ok(net), Ok(bits)) = (net.parse::<IpAddr>(), bits.parse::<u32>()) else {
        return false;
    };
    match (net, ip) {
        (IpAddr::V4(n), IpAddr::V4(a)) if bits <= 32 => {
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            u32::from(n) & mask == u32::from(a) & mask
        }
        (IpAddr::V6(n), IpAddr::V6(a)) if bits <= 128 => {
            let mask = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - bits)
            };
            u128::from(n) & mask == u128::from(a) & mask
        }
        _ => false,
    }
}

/// RFC1918, loopback, link-local (incl. 169.254.169.254), CGNAT, fc00::/7, fe80::/10, unspecified.
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
                || v4_mapped_private(a)
        }
    }
}

fn v4_mapped_private(a: Ipv6Addr) -> bool {
    a.to_ipv4_mapped()
        .is_some_and(|v4| is_private(IpAddr::V4(v4)))
}

fn names_ip_literally(policy: &Policy, cap: Capability, ip: IpAddr, target: &NormalTarget) -> bool {
    policy
        .grants
        .iter()
        .filter(|g| g.capability == cap)
        .any(|g| {
            g.targets.iter().any(|t| {
                if ip_in_selector(ip, t) {
                    return true;
                }
                url::Url::parse(t)
                    .ok()
                    .and_then(|u| host_ip(&u))
                    .is_some_and(|gip| {
                        gip == ip && selector_matches(t, target, cap, &policy.base_dir)
                    })
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::AccessVerb;
    use crate::features::policy::load_policy::parse_policy;

    fn intent(cap: Capability, verb: AccessVerb, target: EffectTarget) -> EffectIntent {
        EffectIntent {
            capability: cap,
            verb,
            target,
            operation_id: "t.op".into(),
            effect_id: None,
            span: None,
        }
    }

    fn policy(json: &str) -> Policy {
        parse_policy(json.as_bytes(), "svc/policy.json", "svc").unwrap()
    }

    // vhco:test policy.authorize_effect -- G10: with a host ceiling an attempt must be allowed by both policies; a deny (or missing grant) in either wins and limits narrow to the smaller
    #[test]
    fn host_ceiling_intersects() {
        let file = policy(
            r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**","./etc/**"]},{"capability":"allow_write","targets":["./out/**"]}],"limits":{"max_call_depth":8}}"#,
        );
        let ceiling = policy(
            r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**","./out/**"]},{"capability":"allow_write","targets":["./out/**"]}],"deny":[{"capability":"allow_write","targets":["./out/secret/**"]}],"limits":{"max_call_depth":4}}"#,
        );
        let p = file.with_ceiling(ceiling);
        assert_eq!(p.limits.max_call_depth, 4);
        let decide = |cap, verb, path: &str| {
            authorize_effect(
                &intent(cap, verb, EffectTarget::Path(path.into())),
                &p,
                "svc",
            )
        };
        // Both allow.
        assert_eq!(
            decide(Capability::Read, AccessVerb::Read, "./data/a").decision,
            Decision::Allowed
        );
        // policy.json grants, the ceiling does not.
        let d = decide(Capability::Read, AccessVerb::Read, "./etc/x");
        assert_eq!(d.decision, Decision::Denied);
        assert!(d.rule.starts_with("host ceiling:"), "{}", d.rule);
        // The ceiling grants, policy.json does not: a ceiling never grants.
        assert_eq!(
            decide(Capability::Read, AccessVerb::Read, "./out/a").decision,
            Decision::Denied
        );
        // A ceiling deny wins over a policy.json grant.
        assert_eq!(
            decide(Capability::Write, AccessVerb::Create, "./out/secret/k").decision,
            Decision::Denied
        );
        assert_eq!(
            decide(Capability::Write, AccessVerb::Create, "./out/ok").decision,
            Decision::Allowed
        );
    }

    // vhco:test policy.authorize_effect -- no policy.json denies every effect
    #[test]
    fn absent_policy_denies() {
        let p = authorize_effect(
            &intent(
                Capability::Read,
                AccessVerb::Read,
                EffectTarget::Path("./a".into()),
            ),
            &Policy::deny_all("svc"),
            "svc",
        );
        assert_eq!(p.decision, Decision::Denied);
        assert!(p.rule.contains("no policy.json"));
    }

    #[test]
    fn globbed_paths_resolve_against_policy_and_bundle_dirs() {
        let pol = policy(
            r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"]}],"deny":[{"capability":"allow_read","targets":["./data/private/**"]}]}"#,
        );
        let ok = authorize_effect(
            &intent(
                Capability::Read,
                AccessVerb::Read,
                EffectTarget::Path("./data/in.json".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(ok.decision, Decision::Allowed);
        let escape = authorize_effect(
            &intent(
                Capability::Read,
                AccessVerb::Read,
                EffectTarget::Path("./data/../secret".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(escape.decision, Decision::Denied);
        let private = authorize_effect(
            &intent(
                Capability::Read,
                AccessVerb::Read,
                EffectTarget::Path("./data/private/k".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(private.decision, Decision::Denied);
        assert!(private.rule.starts_with("deny"));
    }

    #[test]
    fn access_narrowing_allows_create_but_not_update() {
        let pol = policy(
            r#"{"version":1,"grants":[{"capability":"allow_write","targets":["./out/**"],"access":["create"]}]}"#,
        );
        let create = authorize_effect(
            &intent(
                Capability::Write,
                AccessVerb::Create,
                EffectTarget::Path("./out/a.json".into()),
            ),
            &pol,
            "svc",
        );
        let update = authorize_effect(
            &intent(
                Capability::Write,
                AccessVerb::Update,
                EffectTarget::Path("./out/a.json".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(create.decision, Decision::Allowed);
        assert_eq!(update.decision, Decision::Denied);
        assert!(update.rule.contains("does not include `update`"));
    }

    #[test]
    fn network_origin_and_private_ranges() {
        let pol = policy(
            r#"{"version":1,"grants":[{"capability":"allow_network","targets":["https://api.example.com:443","http://127.0.0.1:8080"]}]}"#,
        );
        let ok = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("https://api.example.com/users/42".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(ok.decision, Decision::Allowed);
        let other_port = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("https://api.example.com:8443/".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(other_port.decision, Decision::Denied);
        let metadata = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("http://169.254.169.254/latest".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(metadata.decision, Decision::Denied);
        let loopback_named = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("http://127.0.0.1:8080/x".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(loopback_named.decision, Decision::Allowed);
    }

    #[test]
    fn opaque_scheme_ip_literals_get_the_private_range_rule() {
        let pol = policy(
            r#"{"version":1,"grants":[{"capability":"allow_network","targets":["*"]},{"capability":"allow_network","targets":["udp://127.0.0.1:7000"]}]}"#,
        );
        let wildcard_only = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("udp://10.0.0.5:53".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(
            wildcard_only.decision,
            Decision::Denied,
            "{}",
            wildcard_only.rule
        );
        let named = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("udp://127.0.0.1:7000".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(named.decision, Decision::Allowed, "{}", named.rule);
        let quic_v6 = authorize_effect(
            &intent(
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url("quic://[::1]:4433".into()),
            ),
            &pol,
            "svc",
        );
        assert_eq!(quic_v6.decision, Decision::Denied);
    }
}
