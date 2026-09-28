use super::ports::PolicyFileReader;
use crate::domain::policy::{
    ALL_SURFACES, AccessVerb, ApprovedHashes, BearerTokenHash, Capability, Grant, MtlsPrincipal,
    NetworkPolicy, Policy, PolicyLimits, PrincipalGrant, ServeAuth, ServePolicy,
};
use crate::domain::ports::PolicyLocator;
use crate::domain::{RivetError, RivetResult};
use serde_json::{Map, Value as Json};
use std::path::Path;

// vhco:domain PolicyLoadInput { entry: string; explicit_path?: string }
pub type PolicyLoadInput = PolicyLocator;

// vhco:usecase policy.load_policy(input: PolicyLoadInput) -> Policy needs PolicyFileReader
// vhco:label Load policy.json
// vhco:about Finds policy.json beside the entry file (or the file named by --policy PATH), validates schema v1 strictly and returns the effective Policy. No file means deny-by-default for every new application effect; pure operations still run.
// vhco:example input={entry:"svc/app.rivet"} => { "present": true, "file": "svc/policy.json", "grants": 2 }
pub fn load_policy(input: &PolicyLoadInput, reader: &dyn PolicyFileReader) -> RivetResult<Policy> {
    // vhco:todo discover_file -- `--policy PATH` names exactly one file (a path, never grant text; a missing explicit file is policy.invalid exit 2); otherwise look for policy.json in the entry file's directory; no environment variable is consulted
    // vhco:step discover reader.discover -- PolicyDiscovery{path?, explicit}
    let discovery = reader.discover(input)?;
    let entry_dir = Path::new(&input.entry)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    // vhco:todo deny_by_default_when_absent -- no file found → Policy{present false, no grants, deny_private_ranges true, default limits 64/16/256MiB, serve loopback-only auth none}; there is no implicit allow-all
    // vhco:step absent return -- deny-by-default policy anchored at the entry directory
    let Some(_) = &discovery.path else {
        return Ok(Policy::deny_all(if entry_dir.is_empty() {
            "."
        } else {
            &entry_dir
        }));
    };
    // vhco:todo parse_and_validate_schema -- read the bytes (sha256 recorded), parse strict JSON, require version 1, accept only version/grants/deny/network/limits/serve/approved; unknown keys, capabilities, verbs outside their capability, malformed selectors, non-positive limits, bad serve.auth or surfaces are policy.invalid (exit 2) naming the JSON pointer
    // vhco:step read reader.read -- bytes + sha256 of the discovered file
    let bytes = reader.read(&discovery)?;
    let base_dir = Path::new(&bytes.path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    // vhco:step parse parse_policy -- strict schema v1 validation
    let mut policy = parse_policy(
        &bytes.bytes,
        &bytes.path,
        if base_dir.is_empty() { "." } else { &base_dir },
    )?;
    // vhco:todo resolve_targets -- relative file targets resolve against the policy file's own directory (base_dir), URL targets normalise to scheme://host:port by the evaluator; approved snapshot/overlap hashes are kept for load-time review checks
    // vhco:todo intersect_ceiling -- a library host ceiling can only narrow the file (Runtime::builder().ceiling); the file's sha256 becomes policy_hash for audit and trace records
    policy.sha256 = Some(bytes.sha256);
    // vhco:error policy_invalid -- malformed or out-of-schema policy.json => validation policy.invalid (exit 2) returns
    Ok(policy)
}

fn invalid(pointer: &str, msg: impl Into<String>) -> RivetError {
    RivetError::validation(
        "policy.invalid",
        format!("policy.json {pointer}: {}", msg.into()),
    )
    .with_details(crate::domain::Value::object([(
        "pointer",
        crate::domain::Value::text(pointer),
    )]))
}

/// Strict schema v1 parser shared by `load_policy` and `Policy::from_json`.
pub fn parse_policy(bytes: &[u8], file: &str, base_dir: &str) -> RivetResult<Policy> {
    let json: Json =
        serde_json::from_slice(bytes).map_err(|e| invalid("", format!("not valid JSON: {e}")))?;
    let obj = json
        .as_object()
        .ok_or_else(|| invalid("", "the top level must be an object"))?;
    check_keys(
        obj,
        "",
        &[
            "version", "grants", "deny", "network", "limits", "serve", "approved",
        ],
    )?;
    match obj.get("version").and_then(Json::as_i64) {
        Some(1) => {}
        _ => return Err(invalid("/version", "`version` must be 1")),
    }
    let mut policy = Policy {
        present: true,
        file: Some(file.to_string()),
        base_dir: base_dir.to_string(),
        ..Policy::default()
    };
    policy.grants = grant_list(obj.get("grants"), "/grants")?;
    policy.deny = grant_list(obj.get("deny"), "/deny")?;
    if let Some(n) = obj.get("network") {
        let m = n
            .as_object()
            .ok_or_else(|| invalid("/network", "must be an object"))?;
        check_keys(m, "/network", &["deny_private_ranges"])?;
        policy.network = NetworkPolicy {
            deny_private_ranges: match m.get("deny_private_ranges") {
                None => true,
                Some(Json::Bool(b)) => *b,
                Some(_) => {
                    return Err(invalid(
                        "/network/deny_private_ranges",
                        "must be true or false",
                    ));
                }
            },
        };
    }
    if let Some(l) = obj.get("limits") {
        let m = l
            .as_object()
            .ok_or_else(|| invalid("/limits", "must be an object"))?;
        check_keys(
            m,
            "/limits",
            &[
                "max_concurrent_requests",
                "max_call_depth",
                "max_buffered_bytes",
            ],
        )?;
        let d = PolicyLimits::default();
        let get = |k: &str, default: u64| -> RivetResult<u64> {
            match m.get(k) {
                None => Ok(default),
                Some(v) => match v.as_u64() {
                    Some(n) if n > 0 => Ok(n),
                    _ => Err(invalid(
                        &format!("/limits/{k}"),
                        "must be a positive integer",
                    )),
                },
            }
        };
        policy.limits = PolicyLimits {
            max_concurrent_requests: get(
                "max_concurrent_requests",
                d.max_concurrent_requests as u64,
            )? as u32,
            max_call_depth: get("max_call_depth", d.max_call_depth as u64)? as u32,
            max_buffered_bytes: get("max_buffered_bytes", d.max_buffered_bytes)?,
        };
    }
    if let Some(a) = obj.get("approved") {
        let m = a
            .as_object()
            .ok_or_else(|| invalid("/approved", "must be an object"))?;
        check_keys(m, "/approved", &["snapshots", "overlaps"])?;
        policy.approved = ApprovedHashes {
            snapshots: string_list(m.get("snapshots"), "/approved/snapshots")?,
            overlaps: string_list(m.get("overlaps"), "/approved/overlaps")?,
        };
    }
    if let Some(s) = obj.get("serve") {
        policy.serve = serve_policy(s)?;
    }
    Ok(policy)
}

fn check_keys(obj: &Map<String, Json>, pointer: &str, allowed: &[&str]) -> RivetResult<()> {
    for k in obj.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(invalid(
                &format!("{pointer}/{k}"),
                format!("unknown key `{k}` (allowed: {})", allowed.join(", ")),
            ));
        }
    }
    Ok(())
}

fn string_list(v: Option<&Json>, pointer: &str) -> RivetResult<Vec<String>> {
    match v {
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, x)| {
                x.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| invalid(&format!("{pointer}/{i}"), "must be a string"))
            })
            .collect(),
        Some(_) => Err(invalid(pointer, "must be a list of strings")),
    }
}

fn grant_list(v: Option<&Json>, pointer: &str) -> RivetResult<Vec<Grant>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let items = v
        .as_array()
        .ok_or_else(|| invalid(pointer, "must be a list"))?;
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let p = format!("{pointer}/{i}");
        let m = item
            .as_object()
            .ok_or_else(|| invalid(&p, "must be an object"))?;
        check_keys(m, &p, &["capability", "targets", "access"])?;
        let cap_name = m
            .get("capability")
            .and_then(Json::as_str)
            .ok_or_else(|| invalid(&format!("{p}/capability"), "is required"))?;
        let capability = Capability::parse(cap_name).ok_or_else(|| {
            invalid(
                &format!("{p}/capability"),
                format!("unknown capability `{cap_name}`"),
            )
        })?;
        let targets = string_list(m.get("targets"), &format!("{p}/targets"))?;
        if targets.is_empty() {
            return Err(invalid(
                &format!("{p}/targets"),
                "needs at least one target",
            ));
        }
        for (j, t) in targets.iter().enumerate() {
            validate_selector(capability, t, &format!("{p}/targets/{j}"))?;
        }
        let access = match m.get("access") {
            None => None,
            Some(a) => {
                let names = string_list(Some(a), &format!("{p}/access"))?;
                let mut verbs = Vec::new();
                for (j, n) in names.iter().enumerate() {
                    let verb = AccessVerb::parse(n).ok_or_else(|| {
                        invalid(
                            &format!("{p}/access/{j}"),
                            format!("unknown access verb `{n}`"),
                        )
                    })?;
                    if !capability.verbs().contains(&verb) {
                        let allowed: Vec<&str> =
                            capability.verbs().iter().map(|v| v.as_str()).collect();
                        return Err(invalid(
                            &format!("{p}/access/{j}"),
                            format!(
                                "`{n}` does not belong to {} (allowed: {})",
                                capability.as_str(),
                                allowed.join(", ")
                            ),
                        ));
                    }
                    verbs.push(verb);
                }
                Some(verbs)
            }
        };
        out.push(Grant {
            capability,
            targets,
            access,
        });
    }
    Ok(out)
}

fn validate_selector(cap: Capability, target: &str, pointer: &str) -> RivetResult<()> {
    if target == "*" {
        return Ok(());
    }
    if target.trim().is_empty() {
        return Err(invalid(pointer, "empty target"));
    }
    if (cap == Capability::Network || cap == Capability::Listen)
        && !target.contains("://")
        && target.parse::<std::net::IpAddr>().is_err()
        && !target.contains('/')
    {
        return Err(invalid(
            pointer,
            format!("`{target}` must be a URL such as https://host:443, an IP or a CIDR"),
        ));
    }
    if cap.is_path_like() && (target.contains('\0')) {
        return Err(invalid(pointer, "path contains a NUL byte"));
    }
    if globset::Glob::new(target).is_err() {
        return Err(invalid(
            pointer,
            format!("`{target}` is not a valid selector"),
        ));
    }
    Ok(())
}

fn serve_policy(v: &Json) -> RivetResult<ServePolicy> {
    let m = v
        .as_object()
        .ok_or_else(|| invalid("/serve", "must be an object"))?;
    check_keys(m, "/serve", &["surfaces", "auth", "principals"])?;
    let mut s = ServePolicy::default();
    if m.contains_key("surfaces") {
        s.surfaces = string_list(m.get("surfaces"), "/serve/surfaces")?;
        for (i, x) in s.surfaces.iter().enumerate() {
            if !ALL_SURFACES.contains(&x.as_str()) {
                return Err(invalid(
                    &format!("/serve/surfaces/{i}"),
                    format!("unknown surface `{x}` (http, sse, poll, ws, mcp)"),
                ));
            }
        }
    }
    if let Some(a) = m.get("auth") {
        let am = a
            .as_object()
            .ok_or_else(|| invalid("/serve/auth", "must be an object"))?;
        s.auth = match am.get("type").and_then(Json::as_str) {
            Some("none") => {
                check_keys(am, "/serve/auth", &["type"])?;
                ServeAuth::None
            }
            Some("bearer") => {
                check_keys(am, "/serve/auth", &["type", "tokens"])?;
                let tokens = am
                    .get("tokens")
                    .and_then(Json::as_array)
                    .ok_or_else(|| invalid("/serve/auth/tokens", "is required for bearer auth"))?;
                let mut out = Vec::new();
                for (i, t) in tokens.iter().enumerate() {
                    let p = format!("/serve/auth/tokens/{i}");
                    let principal = t
                        .get("principal")
                        .and_then(Json::as_str)
                        .ok_or_else(|| invalid(&p, "needs `principal`"))?;
                    let sha = t.get("sha256").and_then(Json::as_str).unwrap_or("");
                    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Err(invalid(
                            &format!("{p}/sha256"),
                            "must be 64 hex characters (the SHA-256 of the token)",
                        ));
                    }
                    out.push(BearerTokenHash {
                        principal: principal.into(),
                        sha256: sha.to_ascii_lowercase(),
                    });
                }
                ServeAuth::Bearer(out)
            }
            Some("mtls") => {
                check_keys(am, "/serve/auth", &["type", "client_ca", "principals"])?;
                let ca = am
                    .get("client_ca")
                    .and_then(Json::as_str)
                    .ok_or_else(|| invalid("/serve/auth/client_ca", "is required for mtls"))?;
                let mut principals = Vec::new();
                for (i, p) in am
                    .get("principals")
                    .and_then(Json::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    let ptr = format!("/serve/auth/principals/{i}");
                    principals.push(MtlsPrincipal {
                        principal: p
                            .get("principal")
                            .and_then(Json::as_str)
                            .ok_or_else(|| invalid(&ptr, "needs `principal`"))?
                            .into(),
                        subject: p
                            .get("subject")
                            .and_then(Json::as_str)
                            .ok_or_else(|| invalid(&ptr, "needs `subject`"))?
                            .into(),
                    });
                }
                ServeAuth::Mtls {
                    client_ca: ca.into(),
                    principals,
                }
            }
            _ => return Err(invalid("/serve/auth/type", "must be none, bearer or mtls")),
        };
    }
    if let Some(p) = m.get("principals") {
        let pm = p
            .as_object()
            .ok_or_else(|| invalid("/serve/principals", "must be an object"))?;
        let mut out = Vec::new();
        for (name, v) in pm {
            let ptr = format!("/serve/principals/{name}");
            let vm = v
                .as_object()
                .ok_or_else(|| invalid(&ptr, "must be an object"))?;
            check_keys(vm, &ptr, &["operations"])?;
            out.push(PrincipalGrant {
                principal: name.clone(),
                operations: string_list(vm.get("operations"), &format!("{ptr}/operations"))?,
            });
        }
        s.principals = Some(out);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{PolicyBytes, PolicyDiscovery};

    struct Fake(Option<&'static str>);
    impl PolicyFileReader for Fake {
        fn discover(&self, _: &PolicyLocator) -> RivetResult<PolicyDiscovery> {
            Ok(PolicyDiscovery {
                path: self.0.map(|_| "svc/policy.json".to_string()),
                explicit: false,
            })
        }
        fn read(&self, d: &PolicyDiscovery) -> RivetResult<PolicyBytes> {
            Ok(PolicyBytes {
                path: d.path.clone().unwrap(),
                bytes: self.0.unwrap().as_bytes().to_vec(),
                sha256: "x".into(),
            })
        }
    }

    fn load(json: Option<&'static str>) -> RivetResult<Policy> {
        load_policy(
            &PolicyLocator {
                entry: "svc/app.rivet".into(),
                explicit_path: None,
            },
            &Fake(json),
        )
    }

    // vhco:test policy.load_policy -- no policy.json yields the deny-by-default policy anchored at the entry directory
    #[test]
    fn absent_file_is_deny_by_default() {
        let p = load(None).unwrap();
        assert!(!p.present);
        assert!(p.grants.is_empty());
        assert!(p.network.deny_private_ranges);
        assert_eq!(p.base_dir, "svc");
    }

    #[test]
    fn valid_file_parses_grants_access_and_serve() {
        let p = load(Some(r#"{"version":1,"grants":[{"capability":"allow_write","targets":["./out/**"],"access":["create"]}],
            "serve":{"auth":{"type":"bearer","tokens":[{"principal":"ada","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]}}}"#))
        .unwrap();
        assert_eq!(p.grants[0].access, Some(vec![AccessVerb::Create]));
        assert_eq!(p.base_dir, "svc");
        assert!(matches!(p.serve.auth, ServeAuth::Bearer(_)));
    }

    #[test]
    fn schema_errors_are_policy_invalid_with_pointer() {
        for (json, pointer) in [
            (r#"{"version":2}"#, "/version"),
            (r#"{"version":1,"extra":1}"#, "/extra"),
            (
                r#"{"version":1,"grants":[{"capability":"allow_everything","targets":["*"]}]}"#,
                "/grants/0/capability",
            ),
            (
                r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./d"],"access":["delete"]}]}"#,
                "/grants/0/access/0",
            ),
            (
                r#"{"version":1,"limits":{"max_call_depth":0}}"#,
                "/limits/max_call_depth",
            ),
            (
                r#"{"version":1,"serve":{"surfaces":["gopher"]}}"#,
                "/serve/surfaces/0",
            ),
        ] {
            let e = parse_policy(json.as_bytes(), "p.json", ".").unwrap_err();
            assert_eq!(e.code, "policy.invalid", "{json}");
            assert_eq!(
                e.details.get("pointer").and_then(|v| v.as_str()),
                Some(pointer),
                "{json}"
            );
            assert_eq!(e.exit_code(), 2);
        }
    }
}
