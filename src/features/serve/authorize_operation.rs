use crate::domain::serve::{AccessDecision, OperationAccess};
use crate::domain::{RivetError, RivetResult};

/// IDs that reveal internal URLs/paths (the I/O manifest, policy drafts,
/// recorded traces) or write host files (`connectors sync`): a network
/// principal needs them listed explicitly; `*` / `demo.*` style patterns never
/// match them.
pub const SENSITIVE_IDS: [&str; 5] = [
    "rivet.io",
    "rivet.policy.generate",
    "rivet.trace.show",
    "rivet.trace.export",
    "rivet.connectors.sync",
];

/// Generic built-ins whose authorization applies to the operation they name
/// (`rivet.request`, `rivet.sessions.open`, `rivet.describe`, `rivet.outputs`)
/// or to the principal's own sessions, never to the built-in ID itself.
/// `rivet.capabilities` is here too: read-only build facts with no target and
/// no sensitive data, callable by any authenticated principal (S102).
pub const GENERIC_BUILTINS: [&str; 10] = [
    "rivet.capabilities",
    "rivet.request",
    "rivet.list",
    "rivet.describe",
    "rivet.outputs",
    "rivet.sessions.open",
    "rivet.sessions.send",
    "rivet.sessions.finish_input",
    "rivet.sessions.read",
    "rivet.sessions.cancel",
];

// vhco:usecase serve.authorize_operation(input: OperationAccess) -> AccessDecision
// vhco:label Authorize operation
// vhco:about Decides whether an authenticated principal may call (or see) one operation ID under policy.json serve.principals, identically on every surface.
// vhco:example input={principal:"ci", operation_id:"demo.add", serve:{principals:{ci:["demo.health"]}}} => { "allowed": false, "principal": "ci" }
pub fn authorize_operation(input: &OperationAccess) -> AccessDecision {
    let who = input.principal.name.clone();
    let id = input.operation_id.as_str();
    let allow = |pattern: &str| AccessDecision {
        allowed: true,
        principal: who.clone(),
        matched_pattern: Some(pattern.to_string()),
    };
    // vhco:todo public_only -- private and unknown IDs are rejected as not_found by the dispatcher/registry for every principal before or after this decision, so a denial here never reveals whether a hidden ID exists
    // vhco:todo principal_map -- the loopback principal `local` (auth none, or the CLI/library host) may call everything; generic built-ins (rivet.request/list/describe/outputs/sessions.*) are allowed here because their target ID is authorized separately; otherwise with serve.principals absent every authenticated principal may call every public ID except rivet.io/rivet.policy.generate; with the map present the principal's entry must match exactly, by `*`, or by a trailing `.*` prefix pattern (demo.* matches demo.add) — sensitive IDs match only an exact entry
    // vhco:step local return -- principal local (authenticated_by none) → allowed, pattern "local"
    if input.principal.name == "local" && input.principal.authenticated_by == "none" {
        return allow("local");
    }
    // vhco:step generic return -- a generic built-in → allowed, its target is authorized on its own
    if GENERIC_BUILTINS.contains(&id) {
        return allow("builtin");
    }
    let sensitive = SENSITIVE_IDS.contains(&id);
    let denied = AccessDecision {
        allowed: false,
        principal: who.clone(),
        matched_pattern: None,
    };
    // vhco:step no_map return -- no serve.principals → allowed unless the ID is sensitive
    let Some(grants) = &input.serve.principals else {
        return if sensitive { denied } else { allow("*") };
    };
    // vhco:step match patterns -- exact, `*` or `prefix.*`; sensitive IDs need the exact ID
    // vhco:error no_entry -- the principal has no entry or no pattern matches => allowed:false (callers return permission.denied, 403)
    let Some(entry) = grants.iter().find(|g| g.principal == who) else {
        return denied;
    };
    for pattern in &entry.operations {
        let hit = if pattern == id {
            true
        } else if sensitive {
            false
        } else if pattern == "*" {
            true
        } else if let Some(prefix) = pattern.strip_suffix(".*") {
            id.len() > prefix.len() + 1
                && id.starts_with(prefix)
                && id[prefix.len()..].starts_with('.')
        } else {
            false
        };
        if hit {
            return allow(pattern);
        }
    }
    // vhco:todo apply_everywhere -- return AccessDecision{allowed, principal, matched_pattern}; REST, SSE, polling, WebSocket, MCP and the catalog listings (GET /v1/operations, tools/list, rivet.list, rivet.outputs --all) all call this same decision and filter or refuse with permission.denied (403)
    denied
}

/// The decision as a result: a denial is `permission.denied` (403, exit 3).
pub fn require_operation(input: &OperationAccess) -> RivetResult<AccessDecision> {
    let d = authorize_operation(input);
    if d.allowed {
        Ok(d)
    } else {
        Err(RivetError::permission(format!(
            "principal `{}` may not call `{}`",
            d.principal, input.operation_id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contracts::Principal;
    use crate::domain::policy::{PrincipalGrant, ServePolicy};

    fn access(who: &str, id: &str, map: Option<Vec<(&str, Vec<&str>)>>) -> OperationAccess {
        OperationAccess {
            principal: Principal {
                name: who.into(),
                authenticated_by: "bearer".into(),
            },
            operation_id: id.into(),
            serve: ServePolicy {
                principals: map.map(|m| {
                    m.into_iter()
                        .map(|(p, ops)| PrincipalGrant {
                            principal: p.into(),
                            operations: ops.into_iter().map(String::from).collect(),
                        })
                        .collect()
                }),
                ..ServePolicy::default()
            },
        }
    }

    // vhco:test serve.authorize_operation -- prefix patterns, missing entries, the absent map and the explicit-only rule for rivet.io / rivet.policy.generate
    #[test]
    fn principal_patterns() {
        let team = || {
            Some(vec![
                ("ada", vec!["demo.*"]),
                ("ci", vec!["demo.health", "rivet.io"]),
                ("ops", vec!["*"]),
            ])
        };
        assert!(authorize_operation(&access("ada", "demo.add", team())).allowed);
        assert!(!authorize_operation(&access("ada", "demox.add", team())).allowed);
        assert!(!authorize_operation(&access("ci", "demo.add", team())).allowed);
        assert!(authorize_operation(&access("ci", "rivet.io", team())).allowed);
        assert!(!authorize_operation(&access("ops", "rivet.io", team())).allowed);
        assert!(authorize_operation(&access("ops", "x.y", team())).allowed);
        assert!(!authorize_operation(&access("eve", "demo.add", team())).allowed);
        assert!(authorize_operation(&access("eve", "demo.add", None)).allowed);
        assert!(!authorize_operation(&access("eve", "rivet.policy.generate", None)).allowed);
        assert!(!authorize_operation(&access("ops", "rivet.trace.show", team())).allowed);
        assert!(!authorize_operation(&access("eve", "rivet.connectors.sync", None)).allowed);
        assert!(authorize_operation(&access("ci", "rivet.sessions.read", team())).allowed);
        let mut local = access("local", "rivet.io", team());
        local.principal = Principal::local();
        assert!(authorize_operation(&local).allowed);
        assert_eq!(
            require_operation(&access("ci", "demo.add", team()))
                .unwrap_err()
                .code,
            "permission.denied"
        );
    }

    // vhco:test serve.authorize_operation -- rivet.capabilities is callable by any authenticated principal, even one without a serve.principals entry
    #[test]
    fn capabilities_open_to_every_principal() {
        let map = Some(vec![("ci", vec!["demo.health"])]);
        assert!(
            authorize_operation(&access("stranger", "rivet.capabilities", map.clone())).allowed
        );
        assert!(authorize_operation(&access("ci", "rivet.capabilities", map)).allowed);
        assert!(authorize_operation(&access("ci", "rivet.capabilities", None)).allowed);
    }
}
