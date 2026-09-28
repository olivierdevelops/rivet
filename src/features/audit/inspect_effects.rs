//! `audit.inspect_effects` — the I/O manifest (`rivet io`, `rivet.io`, `rt.io`).
//!
//! ```text
//!  IoQuery ──validate──▶ select entries ──▶ reachable ops (literal request edges)
//!     ──▶ sites (Registry.effect_sites, call_chain = longest chain) ──▶ kind/access filter
//!     ──▶ [check_policy: PolicyEvaluator] ──▶ [trace: TraceStore] ──▶ targets (group)
//!     ──▶ [needs] ──▶ [check_files: PolicyEvaluator stat + FileProbe] ──▶ render + exit code
//! ```

use super::support::effect_sites::norm_rel;
use super::ports::{FileProbe, PolicyEvaluator, Registry, TraceStore};
use super::support::render::{
    RenderOptions, capability_rank, render_bootstrap, render_by_capability, render_by_operation,
    render_by_target, render_csv, render_needs,
};
use crate::domain::io_manifest::{
    AttemptSummary, EffectSite, FileDigest, FileProbeInput, FileStatus, IoManifest, IoQuery,
    IoReport, Knowledge, NeededFile, OperationNeeds, Phase, SiteKind, SiteOrigin, SiteTarget,
    TargetSummary, TraceQuery,
};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::{RivetError, RivetResult};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Ports the use case needs; `trace`/`probe` may be absent on hosts without them.
pub struct AuditPorts<'a> {
    pub registry: &'a dyn Registry,
    /// Static evaluator (no decision log, no trace) over the effective policy.
    pub policy: &'a dyn PolicyEvaluator,
    pub trace: Option<&'a dyn TraceStore>,
    pub probe: Option<&'a dyn FileProbe>,
}

// vhco:usecase audit.inspect_effects(input: EffectQuery) -> IoReport needs Registry, PolicyEvaluator, TraceStore, FileProbe
// vhco:label Generate the I/O manifest (inspect effects)
// vhco:about Lists every effect site of the selected operations from the immutable compiled catalog — kind, access verbs, capability, normalized target, knowledge class, origin, phase and whether a file must already exist — grouped by operation, target or capability, optionally checked against the effective policy, joined to a request trace, or pre-flight probed on disk; nothing is executed and no source expression is evaluated.
// vhco:example input={by:"target", kind:"file"} => { "complete": true, "targets": [{ "target": "./out/notes/*.json", "capability": "allow_write", "access": ["create", "update"] }] }
pub fn inspect_effects(query: &IoQuery, ports: &AuditPorts) -> RivetResult<IoReport> {
    // vhco:step validate query -- by, format, kind and access words are checked before any work; an unknown one is validation (exit 2)
    // vhco:error bad_query -- unknown --by/--format/--kind/--access, or a verb that does not belong to --kind => validation.usage (exit 2) returns
    let (kind, verbs) = validate(query)?;
    let program = ports.registry.program();
    let catalog = ports.registry.effect_sites();

    // vhco:todo select_entries -- entries = explicit ids (each must exist and be public unless `all`; otherwise not_found without disclosing a private one), else every public operation, or every operation with `all`; with `transitive` (default) follow literal (request "id") edges breadth-first and give each reached operation its longest call chain from an entry and the set of entries reaching it; nothing is executed
    // vhco:error unknown_id -- an ID that is absent, or private without --all => not_found.operation (exit 4) returns
    let mut entries: Vec<String> = Vec::new();
    if query.ids.is_empty() {
        for op in &program.operations {
            if query.all || !op.private {
                entries.push(op.id.clone());
            }
        }
    } else {
        for id in &query.ids {
            match program.operation(id) {
                Some(op) if query.all || !op.private => entries.push(id.clone()),
                _ => {
                    return Err(RivetError::not_found(
                        "not_found.operation",
                        format!("no operation `{id}`"),
                    ));
                }
            }
        }
    }
    let edges: HashMap<&str, Vec<&str>> = catalog
        .operations
        .iter()
        .map(|o| {
            (
                o.operation_id.as_str(),
                o.calls
                    .iter()
                    .filter(|c| c.connector.is_none())
                    .map(|c| c.callee.as_str())
                    .collect(),
            )
        })
        .collect();
    // op -> (longest chain, entries reaching it)
    let mut reach: BTreeMap<String, (Vec<String>, BTreeSet<String>)> = BTreeMap::new();
    for e in &entries {
        let mut stack: Vec<Vec<String>> = vec![vec![e.clone()]];
        while let Some(path) = stack.pop() {
            let last = path.last().cloned().unwrap_or_default();
            let slot = reach
                .entry(last.clone())
                .or_insert_with(|| (path.clone(), BTreeSet::new()));
            if path.len() > slot.0.len() {
                slot.0 = path.clone();
            }
            slot.1.insert(e.clone());
            if !query.transitive || path.len() > 16 {
                continue;
            }
            for callee in edges.get(last.as_str()).into_iter().flatten() {
                if !path.iter().any(|p| p == callee) {
                    let mut next = path.clone();
                    next.push(callee.to_string());
                    stack.push(next);
                }
            }
        }
    }

    // vhco:todo walk_effects -- take each reached operation's lowered sites from Registry.effect_sites (file update = stat + update, connector calls = transport + mcp/grpc call, auth = env + credential read/write + auth use + token connect, every tls/body file/descriptor option line its own file read site), in operation-id order then effect order, stamping call_chain and entries; with include_bootstrap add the fixed runtime-internal list plus connector descriptor/schema reads as `bootstrap`
    // vhco:step walk registry.effect_sites -- immutable per-operation sites computed once at assembly; never re-parse or connect
    let mut sites: Vec<EffectSite> = Vec::new();
    let mut calls = Vec::new();
    for (op_id, (chain, reached_by)) in &reach {
        let Some(ops) = catalog.operation(op_id) else {
            continue;
        };
        for s in &ops.sites {
            let mut s = s.clone();
            s.call_chain = chain.clone();
            s.entries = reached_by.iter().cloned().collect();
            sites.push(s);
        }
        calls.extend(ops.calls.iter().cloned());
    }

    // vhco:todo derive_access -- access verbs and capability come from the fixed table (file read|list|stat|watch -> allow_read; create|update|append -> allow_write; delete -> allow_delete; network connect -> allow_network with method/protocol; bind|listen|multicast_join -> allow_listen; exec -> allow_exec; env read -> allow_env; pipe -> allow_pipe; unix -> allow_unix; mcp/grpc call -> allow_mcp/allow_grpc; auth use|manage|status -> allow_auth; credential read|write -> allow_credentials), assigned by the analysis via capability_for
    // vhco:todo classify_phase -- origin (statement or option line), phase (load | before_connect | connect | body | cleanup), requires_existing (reads, stats, lists, update, delete without `missing ok`, option reads; false for creates and for targets the same operation created earlier) and secret (tls key_file, env read by `secret`) are set per site by the analysis
    // vhco:todo normalize_targets -- URLs keep their template with {param} placeholders and carry scheme/host/port (default port explicit) and path; paths stay bundle-relative with a derived glob when param_dependent; env as `env NAME`; connector methods as connector/kind/name; a target from a non-param expression is dynamic and reported by expression only

    // vhco:step filter kind/access -- keep sites whose kind matches `kind` and whose verbs intersect `access` (call rows only when unfiltered)
    let filtered = kind.is_some() || !verbs.is_empty();
    if filtered {
        sites.retain(|s| {
            kind.is_none_or(|k| s.kind == k)
                && (verbs.is_empty() || s.access.iter().any(|v| verbs.contains(v)))
        });
        calls.clear();
    }
    let complete = !sites.iter().any(|s| s.knowledge.is_unknown());

    // vhco:todo check_policy -- when asked, evaluate every access verb of every site through PolicyEvaluator (effective policy; nothing performed): allowed when all verbs are granted; for param_dependent targets an arbitrary instance that is granted is allowed, otherwise partial when some grant selector instance falls inside the derived glob/origin and is itself allowed, else denied; bounded targets check every bound value; dynamic and opaque_native are unknown; record the policy file and sha256
    // vhco:step policy PolicyEvaluator.evaluate -- static evaluator: no decision log, no trace event
    let root = program.root.clone();
    if query.check_policy {
        for s in &mut sites {
            s.decision = Some(site_decision(s, ports.policy, &root).to_string());
        }
    }

    // vhco:todo join_trace -- with trace_request_id read that request from TraceStore (absent store or request = not_found, exit 4), join events on effect_id into attempts {count, last_decision, last_phase}; events with no planned site are kept as unplanned
    // vhco:error unknown_request -- no trace store, or the store has no such request => not_found.trace (exit 4) returns
    let mut unplanned = Vec::new();
    if let Some(req) = &query.trace_request_id {
        let store = ports.trace.ok_or_else(|| {
            RivetError::not_found(
                "not_found.trace",
                format!(
                    "no trace store holds request `{req}` (the CLI is one-shot; query a live host)"
                ),
            )
        })?;
        let trace = store.read(&TraceQuery::new(req))?;
        for s in &mut sites {
            let mut a = AttemptSummary::default();
            for e in trace
                .events
                .iter()
                .filter(|e| e.effect_id.as_deref() == Some(&s.effect_id))
            {
                a.count += 1;
                a.last_decision = Some(e.decision.clone());
                a.last_phase = Some(e.phase.clone());
            }
            s.attempts = Some(a);
        }
        for e in trace.events {
            if !sites
                .iter()
                .any(|s| Some(&s.effect_id) == e.effect_id.as_ref())
            {
                unplanned.push(e);
            }
        }
    }

    // vhco:todo group_view -- TargetSummary per (grant target, capability): union of verbs, methods, protocols, origins, phases and operations; needs_file yes/no/`yes: verbs`/null; USED BY lists each operation, `entry (via op)` for transitive users and connector `op (via method)`; the decision is the worst site decision (denied > partial > unknown > allowed); rows ordered by first appearance, capabilities of one target in vocabulary order
    let targets = group_targets(&sites, &program, target_key);
    let capability_rows = group_targets(&sites, &program, |s| {
        if s.kind == SiteKind::Network {
            s.target_display()
        } else {
            target_key(s)
        }
    });

    // vhco:todo needs_view -- with needs or check_files: one OperationNeeds per selected entry (id order) listing its requires_existing file sites and those of reachable callees (via = callee), deduplicated by path; param_dependent paths shown as templates; with include_bootstrap a leading `bundle load` group lists connector descriptor/schema files
    let mut needs = Vec::new();
    if query.needs || query.check_files {
        if query.include_bootstrap && !catalog.load_sites.is_empty() {
            needs.push(OperationNeeds {
                operation_id: "bundle load".into(),
                files: catalog.load_sites.iter().map(|s| needed(s, None)).collect(),
            });
        }
        let mut sorted_entries = entries.clone();
        sorted_entries.sort();
        for e in &sorted_entries {
            let mut files: Vec<NeededFile> = Vec::new();
            let mut ops: Vec<&String> = reach
                .iter()
                .filter(|(_, (_, by))| by.contains(e))
                .map(|(id, _)| id)
                .collect();
            ops.sort_by_key(|id| (*id != e, (*id).clone()));
            for op in ops {
                let Some(effects) = catalog.operation(op) else {
                    continue;
                };
                for s in effects
                    .sites
                    .iter()
                    .filter(|s| s.kind == SiteKind::File && s.requires_existing)
                {
                    let path = if s.target.template.is_empty() {
                        s.target_display()
                    } else {
                        s.target.template.clone()
                    };
                    if files.iter().any(|f| f.path == path) {
                        continue;
                    }
                    let via = (op != e).then(|| op.clone()).or(s.via.clone());
                    let mut f = needed(s, via);
                    f.path = path;
                    files.push(f);
                }
            }
            needs.push(OperationNeeds {
                operation_id: e.clone(),
                files,
            });
        }
    }

    // vhco:todo check_files -- with check_files: each distinct exact path is authorized as allow_read/stat through PolicyEvaluator first (denied = not_permitted, path untouched), then probed once through FileProbe (metadata + readability only, never opened); non-exact paths are not_checkable; statuses are shown under every operation needing the path
    // vhco:step probe FileProbe.stat -- only after the broker allowed stat on that path
    let mut probe_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    if query.check_files {
        let mut cache: HashMap<String, FileStatus> = HashMap::new();
        for group in &mut needs {
            for f in &mut group.files {
                let status = if f.knowledge != Knowledge::Exact {
                    FileStatus::NotCheckable
                } else if let Some(st) = cache.get(&f.path) {
                    *st
                } else {
                    let permit = ports.policy.evaluate(&EffectIntent {
                        capability: Capability::Read,
                        verb: AccessVerb::Stat,
                        target: EffectTarget::Path(f.path.clone()),
                        operation_id: group.operation_id.clone(),
                        effect_id: Some(f.effect_id.clone()),
                        span: Some(f.source.clone()),
                    });
                    let st = if permit.decision == Decision::Denied {
                        FileStatus::NotPermitted
                    } else {
                        match ports.probe {
                            Some(p) => {
                                p.stat(&FileProbeInput {
                                    path: f.path.clone(),
                                    effect_id: f.effect_id.clone(),
                                })
                                .status
                            }
                            None => FileStatus::NotCheckable,
                        }
                    };
                    cache.insert(f.path.clone(), st);
                    *probe_counts.entry(st.as_str()).or_default() += 1;
                    st
                };
                f.status = Some(status);
            }
        }
    }

    let bootstrap = if query.include_bootstrap {
        bootstrap_sites(&catalog.bundle_file, ports.policy, &catalog.load_sites)
    } else {
        Vec::new()
    };
    let policy = ports.policy.policy();
    let manifest = IoManifest {
        bundle: FileDigest {
            file: catalog.bundle_file.clone(),
            sha256: catalog
                .bundle_sha256
                .trim_start_matches("sha256:")
                .to_string(),
        },
        policy: policy.present.then(|| FileDigest {
            file: policy
                .file
                .as_deref()
                .map(|f| super::support::effect_sites::rel_file(f, &root))
                .unwrap_or_default(),
            sha256: policy.sha256.clone().unwrap_or_default(),
        }),
        complete,
        sites,
        calls,
        targets,
        needs,
        bootstrap,
        unplanned,
    };

    // vhco:todo render_inventory -- render table (by operation / target / capability, or the needs listing), markdown (same tables) or csv (one row per site) from the manifest, json = the manifest itself; stderr gets decision counts, probe counts and the incompleteness list; exit 3 when check_policy found denied/partial or check_files found not_permitted/unreadable, else 4 when a needed file is missing, else 7 when strict and incomplete, else 0
    let opts = RenderOptions {
        decision: query.check_policy,
        attempts: query.trace_request_id.is_some(),
        filtered,
        filter_label: filter_label(query),
        markdown: query.format == "markdown",
    };
    let rendered = match query.format.as_str() {
        "json" => format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest.to_json()).unwrap_or_default()
        ),
        "csv" => render_csv(&manifest),
        _ => {
            let mut out = if query.needs || query.check_files {
                render_needs(&manifest.needs, query.check_files)
            } else {
                match query.by.as_str() {
                    "target" => render_by_target(&manifest, &opts),
                    "capability" => render_by_capability(&capability_rows, &opts),
                    _ => render_by_operation(&manifest, &opts, !filtered),
                }
            };
            if query.include_bootstrap && !(query.needs || query.check_files) {
                out.push_str(&render_bootstrap(&manifest.bootstrap, opts.markdown));
            }
            out
        }
    };

    let mut diagnostics = String::new();
    let mut exit: u8 = 0;
    let count = |d: &str| {
        manifest
            .sites
            .iter()
            .filter(|s| s.decision.as_deref() == Some(d))
            .count()
    };
    if query.check_policy {
        let parts: Vec<String> = ["allowed", "partial", "denied", "unknown"]
            .iter()
            .map(|d| (d, count(d)))
            .filter(|(_, n)| *n > 0)
            .map(|(d, n)| format!("{n} {d}"))
            .collect();
        diagnostics.push_str(&format!("{}\n", parts.join(" · ")));
        if count("denied") + count("partial") > 0 {
            exit = 3;
        }
    }
    if query.check_files {
        let distinct = probe_counts.values().sum::<usize>();
        let mut parts = vec![format!(
            "{distinct} file{}",
            if distinct == 1 { "" } else { "s" }
        )];
        for st in ["present", "missing", "unreadable", "not_permitted"] {
            if let Some(n) = probe_counts.get(st) {
                parts.push(format!("{n} {st}"));
            }
        }
        diagnostics.push_str(&format!("{}\n", parts.join(" · ")));
        if probe_counts.contains_key("not_permitted") || probe_counts.contains_key("unreadable") {
            exit = 3;
        } else if probe_counts.contains_key("missing") && exit == 0 {
            exit = 4;
        }
    }
    if !manifest.complete && query.strict {
        let unknown: Vec<&EffectSite> = manifest
            .sites
            .iter()
            .filter(|s| s.knowledge.is_unknown())
            .collect();
        diagnostics.push_str(&format!(
            "io: complete=false — {} of {} sites is dynamic/opaque\n",
            unknown.len(),
            manifest.sites.len()
        ));
        for s in unknown {
            let why = match &s.expression {
                Some(x) => format!("target from expression {x}"),
                None => format!("{} ({})", s.target_display(), s.knowledge.as_str()),
            };
            diagnostics.push_str(&format!(
                "  {}  {} {}  {why}  ({})\n",
                s.effect_id,
                s.kind.as_str(),
                s.access_label(),
                s.source_label()
            ));
        }
        if exit == 0 {
            exit = 7;
        }
    }
    Ok(IoReport {
        manifest,
        by: query.by.clone(),
        format: query.format.clone(),
        rendered,
        diagnostics,
        exit_code: exit,
    })
}

fn filter_label(q: &IoQuery) -> String {
    let mut parts = Vec::new();
    if let Some(k) = &q.kind {
        parts.push(format!("kind={k}"));
    }
    if !q.access.is_empty() {
        parts.push(format!("access={}", q.access.join(",")));
    }
    parts.join(" ")
}

type Validated = (Option<SiteKind>, Vec<AccessVerb>);

fn validate(q: &IoQuery) -> RivetResult<Validated> {
    let usage = |m: String| Err(RivetError::validation("validation.usage", m));
    if !matches!(q.by.as_str(), "operation" | "target" | "capability") {
        return usage(format!(
            "--by {}: expected operation, target or capability",
            q.by
        ));
    }
    if !matches!(q.format.as_str(), "table" | "json" | "markdown" | "csv") {
        return usage(format!(
            "--format {}: expected table, json, markdown or csv",
            q.format
        ));
    }
    let kind = match &q.kind {
        None => None,
        Some(k) => match SiteKind::parse(k) {
            Some(k) => Some(k),
            None => {
                return usage(format!(
                    "--kind {k}: expected file, network, process, env, pipe, unix, mcp, grpc, auth or credential"
                ));
            }
        },
    };
    let mut verbs = Vec::new();
    for word in q
        .access
        .iter()
        .flat_map(|a| a.split(','))
        .map(str::trim)
        .filter(|w| !w.is_empty())
    {
        let Some(v) = AccessVerb::parse(word) else {
            return usage(format!("--access {word}: not an access verb"));
        };
        if let Some(k) = kind
            && !k.verbs().contains(&v)
        {
            return usage(format!(
                "--access {word}: not an access verb of kind {}",
                k.as_str()
            ));
        }
        verbs.push(v);
    }
    Ok((kind, verbs))
}

/// Grouping key of `--by target`.
fn target_key(s: &EffectSite) -> String {
    if s.target.template.is_empty() {
        return s.target_display();
    }
    match s.kind {
        SiteKind::Env => s.target_display(),
        SiteKind::Network if s.knowledge != Knowledge::Dynamic => s.grant_target(),
        _ => s
            .target
            .glob
            .clone()
            .unwrap_or_else(|| s.target.template.clone()),
    }
}

fn decision_rank(d: &str) -> u8 {
    match d {
        "denied" => 3,
        "partial" => 2,
        "unknown" => 1,
        _ => 0,
    }
}

fn group_targets(
    sites: &[EffectSite],
    program: &crate::domain::ir::CompiledProgram,
    key: impl Fn(&EffectSite) -> String,
) -> Vec<TargetSummary> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<(String, Capability), Vec<&EffectSite>> = HashMap::new();
    for s in sites {
        let k = key(s);
        if !order.contains(&k) {
            order.push(k.clone());
        }
        groups.entry((k, s.capability)).or_default().push(s);
    }
    let private = |id: &str| program.operation(id).is_some_and(|o| o.private);
    let mut out = Vec::new();
    for k in order {
        let mut caps: Vec<Capability> = groups
            .keys()
            .filter(|(t, _)| *t == k)
            .map(|(_, c)| *c)
            .collect();
        caps.sort_by_key(|c| capability_rank(*c));
        for cap in caps {
            let g = &groups[&(k.clone(), cap)];
            let mut access: Vec<AccessVerb> = g.iter().flat_map(|s| s.access.clone()).collect();
            access.sort();
            access.dedup();
            let mut methods: Vec<String> = g.iter().filter_map(|s| s.method.clone()).collect();
            methods.sort();
            methods.dedup();
            let mut protocols: Vec<String> = g.iter().filter_map(|s| s.protocol.clone()).collect();
            protocols.sort();
            protocols.dedup();
            let mut origins: Vec<SiteOrigin> = g.iter().map(|s| s.origin.clone()).collect();
            origins.sort_by(|a, b| a.name().cmp(b.name()));
            origins.dedup();
            let mut phases: Vec<Phase> = g.iter().map(|s| s.phase).collect();
            phases.sort();
            phases.dedup();
            let needs_file = if g[0].kind == SiteKind::File {
                let need: Vec<&&EffectSite> = g.iter().filter(|s| s.requires_existing).collect();
                Some(if need.is_empty() {
                    "no".to_string()
                } else if need.len() == g.len() {
                    "yes".to_string()
                } else {
                    let mut v: Vec<AccessVerb> =
                        need.iter().flat_map(|s| s.access.clone()).collect();
                    v.sort();
                    v.dedup();
                    format!(
                        "yes: {}",
                        v.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(", ")
                    )
                })
            } else {
                None
            };
            let mut operations: Vec<String> = g.iter().map(|s| s.operation_id.clone()).collect();
            operations.sort();
            operations.dedup();
            let mut used_by: Vec<String> = Vec::new();
            for op in &operations {
                let mut label = op.clone();
                if private(op) {
                    label.push_str(" (private)");
                }
                let vias: BTreeSet<&String> = g
                    .iter()
                    .filter(|s| &s.operation_id == op)
                    .filter_map(|s| s.via.as_ref())
                    .collect();
                for v in vias {
                    label.push_str(&format!(" (via {v})"));
                }
                if !used_by.contains(&label) {
                    used_by.push(label);
                }
                let transitive: BTreeSet<&String> = g
                    .iter()
                    .filter(|s| &s.operation_id == op)
                    .flat_map(|s| s.entries.iter())
                    .filter(|e| *e != op)
                    .collect();
                for e in transitive {
                    let l = format!("{e} (via {op})");
                    if !used_by.contains(&l) {
                        used_by.push(l);
                    }
                }
            }
            used_by.sort();
            let mut notes: Vec<String> = g.iter().filter_map(|s| s.note.clone()).collect();
            notes.dedup();
            let secret_site = g.iter().find(|s| s.kind == SiteKind::Env && s.secret);
            if let Some(s) = secret_site {
                let name = s
                    .secrets
                    .first()
                    .map(|n| format!("secret {n}"))
                    .unwrap_or_else(|| "secret".into());
                if s.bound_to.is_empty() {
                    notes.push(name);
                } else {
                    notes.push(format!("{name}, bound to {}", s.bound_to.join(", ")));
                }
            }
            if let Some(last) = used_by.last_mut() {
                for n in &notes {
                    last.push_str(&format!(" ({n})"));
                }
            }
            let knowledge = g
                .iter()
                .map(|s| s.knowledge)
                .max()
                .unwrap_or(Knowledge::Exact);
            let decision = g
                .iter()
                .filter_map(|s| s.decision.clone())
                .max_by_key(|d| decision_rank(d));
            out.push(TargetSummary {
                target: k.clone(),
                kind: g[0].kind,
                capability: cap,
                access,
                methods,
                protocols,
                origins,
                phases,
                needs_file,
                operations,
                used_by,
                knowledge,
                decision,
            });
        }
    }
    out
}

fn needed(s: &EffectSite, via: Option<String>) -> NeededFile {
    NeededFile {
        path: s.target.template.clone(),
        effect_id: s.effect_id.clone(),
        origin: s.origin.clone(),
        phase: s.phase,
        secret: s.secret,
        knowledge: s.knowledge,
        via,
        source: s.source.clone(),
        status: None,
    }
}

fn bootstrap_sites(
    bundle: &str,
    policy: &dyn PolicyEvaluator,
    load: &[EffectSite],
) -> Vec<EffectSite> {
    let file = |t: String| {
        let mut s = EffectSite::new("bootstrap", SiteKind::File, vec![AccessVerb::Read]);
        s.target = SiteTarget {
            template: t,
            ..SiteTarget::default()
        };
        s.origin = SiteOrigin::Statement("bootstrap".into());
        s.phase = Phase::Load;
        s.call_chain = Vec::new();
        s
    };
    let p = policy.policy();
    let mut out = vec![
        file(format!("./{bundle} (+ imports)")),
        file(match (&p.file, p.present) {
            (Some(f), true) => f.clone(),
            _ => "./policy.json (when present)".into(),
        }),
        file("system CA bundle".into()),
        file("/etc/resolv.conf / system resolver".into()),
        file("tzdata".into()),
    ];
    if load.is_empty() {
        out.push(file(
            "descriptor/schema files named by connectors (none here)".into(),
        ));
    } else {
        out.extend(load.iter().cloned());
    }
    let mut pipe = EffectSite::new(
        "bootstrap",
        SiteKind::Pipe,
        vec![AccessVerb::Read, AccessVerb::Write],
    );
    pipe.target = SiteTarget {
        template: "stdin, stdout, stderr".into(),
        ..SiteTarget::default()
    };
    pipe.origin = SiteOrigin::Statement("bootstrap".into());
    pipe.phase = Phase::Load;
    pipe.call_chain = Vec::new();
    out.push(pipe);
    out
}

// ---------------------------------------------------------------- policy check

const ANY: &str = "zzrivetanyzz";

fn intent_target(s: &EffectSite, concrete: &str, verb: AccessVerb) -> Vec<EffectTarget> {
    match s.kind {
        SiteKind::File | SiteKind::Process | SiteKind::Unix | SiteKind::Pipe => {
            vec![EffectTarget::Path(concrete.to_string())]
        }
        SiteKind::Network => vec![EffectTarget::Url(concrete.to_string())],
        SiteKind::Env => vec![EffectTarget::Env(concrete.to_string())],
        SiteKind::Auth => vec![
            EffectTarget::Logical(format!("{concrete}/{}", verb.as_str())),
            EffectTarget::Logical(concrete.to_string()),
        ],
        SiteKind::Mcp | SiteKind::Grpc | SiteKind::Credential => {
            vec![EffectTarget::Logical(concrete.to_string())]
        }
    }
}

fn allowed(s: &EffectSite, concrete: &str, verb: AccessVerb, ev: &dyn PolicyEvaluator) -> bool {
    intent_target(s, concrete, verb).into_iter().any(|t| {
        ev.evaluate(&EffectIntent {
            capability: s.capability,
            verb,
            target: t,
            operation_id: s.operation_id.clone(),
            effect_id: Some(s.effect_id.clone()),
            span: Some(s.source.clone()),
        })
        .decision
            == Decision::Allowed
    })
}

fn instance(template: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in template.chars() {
        match c {
            '{' => {
                if depth == 0 {
                    out.push_str(ANY);
                }
                depth += 1;
            }
            '}' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            c => out.push(c),
        }
    }
    out
}

/// allowed | denied | partial | unknown for one site against the effective policy.
fn site_decision(s: &EffectSite, ev: &dyn PolicyEvaluator, root: &str) -> &'static str {
    if matches!(s.knowledge, Knowledge::Dynamic | Knowledge::OpaqueNative)
        || s.target.template.is_empty()
    {
        return "unknown";
    }
    let mut worst = "allowed";
    for verb in &s.access {
        let d = match s.knowledge {
            Knowledge::Bounded if !s.target.bound.is_empty() => {
                let ok = s
                    .target
                    .bound
                    .iter()
                    .filter(|b| allowed(s, b, *verb, ev))
                    .count();
                if ok == s.target.bound.len() {
                    "allowed"
                } else if ok > 0 {
                    "partial"
                } else {
                    "denied"
                }
            }
            Knowledge::ParamDependent | Knowledge::Bounded => {
                if allowed(s, &instance(&s.target.template), *verb, ev) {
                    "allowed"
                } else if partly_covered(s, *verb, ev, root) {
                    "partial"
                } else {
                    "denied"
                }
            }
            _ => {
                if allowed(s, &s.target.template, *verb, ev) {
                    "allowed"
                } else {
                    "denied"
                }
            }
        };
        if decision_rank(d) > decision_rank(worst) {
            worst = d;
        }
    }
    worst
}

fn lexical(base: &str, p: &str) -> String {
    let joined = if p.starts_with('/') || base.is_empty() || base == "." {
        p.to_string()
    } else {
        format!("{base}/{p}")
    };
    let n = norm_rel(&joined);
    let n = if p.starts_with('/') {
        format!("/{n}")
    } else {
        n
    };
    if cfg!(any(target_os = "macos", windows)) {
        n.to_lowercase()
    } else {
        n
    }
}

/// Does some grant selector instance fall inside the site's glob (paths) or
/// origin (URLs) and is that instance itself allowed?
fn partly_covered(s: &EffectSite, verb: AccessVerb, ev: &dyn PolicyEvaluator, root: &str) -> bool {
    let policy = ev.policy();
    let grants = policy
        .grants
        .iter()
        .filter(|g| g.capability == s.capability && g.allows_verb(verb));
    match s.kind {
        SiteKind::File => {
            let Some(glob) = &s.target.glob else {
                return false;
            };
            let pattern = lexical(root, glob);
            let Ok(m) = globset::GlobBuilder::new(&pattern)
                .literal_separator(true)
                .build()
                .map(|g| g.compile_matcher())
            else {
                return false;
            };
            let root_n = lexical(root, ".");
            for g in grants {
                for t in &g.targets {
                    let inst = t.replace("**", "zz").replace(['*', '?'], "zz");
                    let abs = lexical(&policy.base_dir, &inst);
                    if !m.is_match(&abs) {
                        continue;
                    }
                    let rel = if root_n.is_empty() {
                        abs.clone()
                    } else {
                        match abs.strip_prefix(&format!("{root_n}/")) {
                            Some(r) => r.to_string(),
                            None => continue,
                        }
                    };
                    if allowed(s, &format!("./{rel}"), verb, ev) {
                        return true;
                    }
                }
            }
            false
        }
        SiteKind::Network => {
            let origin = s.grant_target();
            let Ok(site) = url::Url::parse(&origin) else {
                return false;
            };
            grants.flat_map(|g| g.targets.iter()).any(|t| {
                url::Url::parse(t).is_ok_and(|u| {
                    u.scheme() == site.scheme()
                        && u.host_str() == site.host_str()
                        && u.port_or_known_default() == site.port_or_known_default()
                })
            })
        }
        _ => false,
    }
}
