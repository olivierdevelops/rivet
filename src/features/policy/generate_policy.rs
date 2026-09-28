use super::load_policy::parse_policy;
use super::ports::PolicyDraftWriter;
use crate::domain::io_manifest::{
    EffectSite, IoManifest, Knowledge, PolicyDraft, PolicyDraftFile, SiteKind, SiteOrigin,
};
use crate::domain::policy::{AccessVerb, Capability, Grant};
use crate::domain::{RivetError, RivetResult};
use std::collections::BTreeMap;

// vhco:domain PolicyGenerateInput { manifest: IoManifest; output?: string; rebase: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PolicyGenerateInput {
    pub manifest: IoManifest,
    /// `--output PATH` (written exclusively); `None` returns the draft only.
    pub output: Option<String>,
    /// Relative path from the draft's directory to the bundle root (`..` for
    /// `policies/x.json`); empty when the draft lives beside the bundle.
    pub rebase: String,
}

/// Capability order of drafts (access-vocabulary order).
const ORDER: [Capability; 13] = [
    Capability::Read,
    Capability::Write,
    Capability::Delete,
    Capability::Network,
    Capability::Listen,
    Capability::Exec,
    Capability::Env,
    Capability::Pipe,
    Capability::Unix,
    Capability::Mcp,
    Capability::Grpc,
    Capability::Auth,
    Capability::Credentials,
];

fn rank(c: Capability) -> usize {
    ORDER.iter().position(|x| *x == c).unwrap_or(99)
}

/// Why a site cannot be granted (None = grantable).
pub fn review_reason(s: &EffectSite) -> Option<String> {
    match s.knowledge {
        Knowledge::Dynamic => Some("dynamic target".into()),
        Knowledge::OpaqueRemote => Some("opaque_remote: server-side effects unknown".into()),
        Knowledge::OpaqueNative => Some("opaque_native host callback".into()),
        Knowledge::ParamDependent
            if s.kind == SiteKind::File && matches!(s.origin, SiteOrigin::Option(_)) =>
        {
            Some("param_dependent option file: no glob for option files".into())
        }
        _ => None,
    }
}

// vhco:usecase policy.generate_policy(input: PolicyGenerateInput) -> PolicyDraft needs PolicyDraftWriter
// vhco:label Generate a least-privilege policy draft
// vhco:about Turns the I/O manifest of the selected operations into a policy.json v1 draft with one grant per (capability, target) narrowed to exactly the access verbs used; dynamic and opaque sites become review items instead of grants; optionally writes the draft to a new file, never overwriting one.
// vhco:example input={ids:["users.snapshot"]} => { "policy": { "version": 1, "grants": [{ "capability": "allow_write", "targets": ["./out/user.json"], "access": ["create"] }] }, "review": [], "complete": true }
pub fn generate_policy(
    input: &PolicyGenerateInput,
    writer: &dyn PolicyDraftWriter,
) -> RivetResult<PolicyDraft> {
    // vhco:todo split_grantable -- the manifest's sites exclude bootstrap entries; exact, bounded and param_dependent sites are grantable; dynamic, opaque_remote and opaque_native sites (and param_dependent option-file sites) go to review and are never granted
    let mut review: Vec<EffectSite> = Vec::new();
    let mut pairs: BTreeMap<(usize, String), (Capability, Vec<AccessVerb>)> = BTreeMap::new();
    for s in &input.manifest.sites {
        if review_reason(s).is_some() {
            if !review.iter().any(|r| r.effect_id == s.effect_id) {
                review.push(s.clone());
            }
            continue;
        }
        // vhco:todo derive_grant_targets -- exact targets as written; param_dependent URLs collapse to their origin scheme://host:port and param_dependent paths to their glob; bounded targets expand to their bound set; option-derived files keep their exact path; auth sites use profile/account/verb; bundle-relative paths are rebased onto the draft's own directory
        let targets: Vec<String> = match (s.kind, s.knowledge) {
            (SiteKind::Network, _) => vec![s.grant_target()],
            (_, Knowledge::Bounded) if !s.target.bound.is_empty() => s.target.bound.clone(),
            (SiteKind::File, Knowledge::ParamDependent) => {
                vec![
                    s.target
                        .glob
                        .clone()
                        .unwrap_or_else(|| s.target.template.clone()),
                ]
            }
            (SiteKind::Auth, _) => s
                .access
                .iter()
                .map(|v| format!("{}/{}", s.target.template, v.as_str()))
                .collect(),
            _ => vec![s.target.template.clone()],
        };
        for t in targets {
            let t = rebase(&t, s.kind, &input.rebase);
            let e = pairs
                .entry((rank(s.capability), t))
                .or_insert((s.capability, Vec::new()));
            for v in &s.access {
                if !e.1.contains(v) {
                    e.1.push(*v);
                }
            }
        }
    }
    // vhco:todo narrow_access -- exactly one grant per (capability, target) with `access` = the union of verbs used in the capability's table order, omitted when that union is the capability's whole verb set (allow_network connect, allow_env read, allow_delete delete)
    let mut grants: Vec<Grant> = Vec::new();
    for ((_, target), (cap, mut verbs)) in pairs {
        let all = cap.verbs();
        verbs.sort_by_key(|v| all.iter().position(|x| x == v).unwrap_or(99));
        let access = if all.iter().all(|v| verbs.contains(v)) {
            None
        } else {
            Some(verbs)
        };
        grants.push(Grant {
            capability: cap,
            targets: vec![target],
            access,
        });
    }
    // vhco:todo assemble_draft -- PolicyFile{version 1, grants sorted by capability then target, network.deny_private_ranges true} with no deny, limits, serve, approved or secret material; re-parse it with the load_policy schema so the draft is guaranteed loadable; complete = review is empty
    // vhco:step validate parse_policy -- the same strict schema as policy.json; a failure is an internal error, never a silently broken draft
    let mut draft = PolicyDraft {
        grants,
        complete: review.is_empty(),
        review,
        written_to: None,
        exit_code: 0,
    };
    let text = draft.render();
    parse_policy(text.as_bytes(), "<draft>", ".").map_err(|e| {
        RivetError::internal(format!("generated draft failed validation: {}", e.message))
    })?;
    // vhco:todo write_or_return -- without output return the draft (stdout / API / rivet.policy.generate); with output ask PolicyDraftWriter.write_new which creates the file exclusively and refuses any existing path with conflict.exists (exit 4, nothing written); exit 7 when review items exist (the draft is still produced), else 0
    // vhco:step write writer.write_new -- exclusive create; an existing file, symlink or directory is conflict.exists
    // vhco:error exists -- --output names an existing path => conflict.exists (exit 4) returns
    if let Some(path) = &input.output {
        let receipt = writer.write_new(&PolicyDraftFile {
            path: path.clone(),
            bytes: text.into_bytes(),
        })?;
        draft.written_to = Some(receipt.path);
    }
    draft.exit_code = if draft.complete { 0 } else { 7 };
    Ok(draft)
}

fn rebase(target: &str, kind: SiteKind, prefix: &str) -> String {
    let path_like = matches!(
        kind,
        SiteKind::File | SiteKind::Process | SiteKind::Unix | SiteKind::Pipe
    );
    if !path_like || prefix.is_empty() || target.starts_with('/') {
        return target.to_string();
    }
    let rest = target.trim_start_matches("./");
    format!("{}/{rest}", prefix.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::io_manifest::{PolicyDraftReceipt, SiteTarget};

    struct NoWrite;
    impl PolicyDraftWriter for NoWrite {
        fn write_new(&self, f: &PolicyDraftFile) -> RivetResult<PolicyDraftReceipt> {
            Err(RivetError::new(
                crate::domain::ErrorKind::Conflict,
                "conflict.exists",
                format!("refusing to overwrite {}", f.path),
            ))
        }
    }

    fn site(id: &str, kind: SiteKind, verbs: Vec<AccessVerb>, t: &str, k: Knowledge) -> EffectSite {
        let mut s = EffectSite::new("a.b", kind, verbs);
        s.effect_id = id.into();
        s.target = SiteTarget {
            template: t.into(),
            glob: (k == Knowledge::ParamDependent).then(|| t.replace("{name}", "*")),
            ..SiteTarget::default()
        };
        s.knowledge = k;
        s
    }

    // vhco:test policy.generate_policy -- merges verbs per (capability, target), omits full verb sets, reviews dynamic sites (exit 7) and refuses to overwrite
    #[test]
    fn drafts_least_privilege() {
        let m = IoManifest {
            sites: vec![
                site(
                    "a#1",
                    SiteKind::File,
                    vec![AccessVerb::Create],
                    "./out/{name}.json",
                    Knowledge::ParamDependent,
                ),
                site(
                    "a#2",
                    SiteKind::File,
                    vec![AccessVerb::Update],
                    "./out/{name}.json",
                    Knowledge::ParamDependent,
                ),
                site(
                    "a#3",
                    SiteKind::File,
                    vec![AccessVerb::Delete],
                    "./out/x.json",
                    Knowledge::Exact,
                ),
                site(
                    "a#4",
                    SiteKind::Network,
                    vec![AccessVerb::Connect],
                    "",
                    Knowledge::Dynamic,
                ),
            ],
            ..IoManifest::default()
        };
        let d = generate_policy(
            &PolicyGenerateInput {
                manifest: m.clone(),
                ..Default::default()
            },
            &NoWrite,
        )
        .unwrap();
        assert_eq!(d.grants.len(), 2);
        assert_eq!(d.grants[0].targets, vec!["./out/*.json".to_string()]);
        assert_eq!(
            d.grants[0].access,
            Some(vec![AccessVerb::Create, AccessVerb::Update])
        );
        assert_eq!(d.grants[1].access, None);
        assert_eq!(d.review.len(), 1);
        assert_eq!(d.exit_code, 7);
        let e = generate_policy(
            &PolicyGenerateInput {
                manifest: m,
                output: Some("policy.json".into()),
                rebase: String::new(),
            },
            &NoWrite,
        )
        .unwrap_err();
        assert_eq!(e.code, "conflict.exists");
    }
}
