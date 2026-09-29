//! Generated I/O manifest (R6, R26; PROP-2026-0001 Increments 5 and 18).
//!
//! ```text
//!  app.rivet ──compile──▶ CompiledProgram ──effect analysis──▶ EffectCatalog
//!                                                               │ (per operation, non-transitive)
//!                               IoQuery ──select/filter/join────▼
//!                                                           IoManifest ──▶ table | json | markdown | csv
//!                                                               └──────▶ PolicyDraft (policy generate)
//! ```

use super::policy::{AccessVerb, Capability, Grant};
use super::source::SourceSpan;
use serde_json::{Value as Json, json};

// vhco:domain Knowledge { exact | bounded | param_dependent | dynamic | opaque_remote | opaque_native }
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Knowledge {
    Exact,
    Bounded,
    ParamDependent,
    Dynamic,
    OpaqueRemote,
    OpaqueNative,
}

impl Knowledge {
    pub fn as_str(self) -> &'static str {
        match self {
            Knowledge::Exact => "exact",
            Knowledge::Bounded => "bounded",
            Knowledge::ParamDependent => "param_dependent",
            Knowledge::Dynamic => "dynamic",
            Knowledge::OpaqueRemote => "opaque_remote",
            Knowledge::OpaqueNative => "opaque_native",
        }
    }

    pub fn is_unknown(self) -> bool {
        matches!(
            self,
            Knowledge::Dynamic | Knowledge::OpaqueRemote | Knowledge::OpaqueNative
        )
    }
}

// vhco:domain SiteKind { file | network | process | env | pipe | unix | mcp | grpc | auth | credential }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SiteKind {
    File,
    Network,
    Process,
    Env,
    Pipe,
    Unix,
    Mcp,
    Grpc,
    Auth,
    Credential,
}

impl SiteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SiteKind::File => "file",
            SiteKind::Network => "network",
            SiteKind::Process => "process",
            SiteKind::Env => "env",
            SiteKind::Pipe => "pipe",
            SiteKind::Unix => "unix",
            SiteKind::Mcp => "mcp",
            SiteKind::Grpc => "grpc",
            SiteKind::Auth => "auth",
            SiteKind::Credential => "credential",
        }
    }

    pub fn parse(s: &str) -> Option<SiteKind> {
        use SiteKind::*;
        [
            File, Network, Process, Env, Pipe, Unix, Mcp, Grpc, Auth, Credential,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }

    /// The access verbs a site of this kind may perform (REF "Access verbs").
    pub fn verbs(self) -> &'static [AccessVerb] {
        use AccessVerb::*;
        match self {
            SiteKind::File => &[Read, List, Stat, Watch, Create, Update, Append, Delete],
            SiteKind::Network => &[Connect, Bind, Listen, MulticastJoin],
            SiteKind::Process => &[Exec],
            SiteKind::Env => &[Read],
            SiteKind::Pipe => &[Read, Write],
            SiteKind::Unix => &[Connect, Listen],
            SiteKind::Mcp | SiteKind::Grpc => &[Call],
            SiteKind::Auth => &[Use, Manage, Status],
            SiteKind::Credential => &[Read, Write],
        }
    }
}

// vhco:domain SiteOrigin { statement: string | option: string }
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SiteOrigin {
    Statement(String),
    Option(String),
}

impl SiteOrigin {
    /// Bare name (`file read`, `tls key_file`) used in tables.
    pub fn name(&self) -> &str {
        match self {
            SiteOrigin::Statement(s) | SiteOrigin::Option(s) => s,
        }
    }

    pub fn label(&self) -> String {
        match self {
            SiteOrigin::Statement(s) => s.clone(),
            SiteOrigin::Option(o) => format!("option: {o}"),
        }
    }

    /// CSV form: `statement:file read` / `option:tls key_file`.
    pub fn csv(&self) -> String {
        match self {
            SiteOrigin::Statement(s) => format!("statement:{s}"),
            SiteOrigin::Option(o) => format!("option:{o}"),
        }
    }

    pub fn to_json(&self) -> Json {
        match self {
            SiteOrigin::Statement(s) => json!({"statement": s}),
            SiteOrigin::Option(o) => json!({"option": o}),
        }
    }
}

// vhco:domain Phase { load | before_connect | connect | body | cleanup }
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Load,
    BeforeConnect,
    Connect,
    Body,
    Cleanup,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Load => "load",
            Phase::BeforeConnect => "before_connect",
            Phase::Connect => "connect",
            Phase::Body => "body",
            Phase::Cleanup => "cleanup",
        }
    }
}

// vhco:domain SiteTarget { template: string; scheme?: string; host?: string; port?: int; path?: string; glob?: string; params: string[]; bound: string[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SiteTarget {
    pub template: String,
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub glob: Option<String>,
    pub params: Vec<String>,
    /// Every concrete target of a `bounded` site (enum params expanded).
    pub bound: Vec<String>,
}

impl SiteTarget {
    /// Grouping key for `--by target`: URL origin for network sites, glob or template otherwise.
    pub fn group_key(&self, kind: SiteKind) -> String {
        match (kind, &self.scheme, &self.host) {
            (SiteKind::Network, Some(s), Some(h)) => match self.port {
                Some(p) => format!("{s}://{h}:{p}"),
                None => format!("{s}://{h}"),
            },
            _ => self.glob.clone().unwrap_or_else(|| self.template.clone()),
        }
    }
}

// vhco:domain AttemptSummary { count: int; last_decision?: string; last_phase?: string }
/// Planned-vs-actual join of one site against a request trace (`io --trace`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AttemptSummary {
    pub count: u32,
    pub last_decision: Option<String>,
    pub last_phase: Option<String>,
}

impl AttemptSummary {
    pub fn to_json(&self) -> Json {
        json!({"count": self.count, "last_decision": self.last_decision, "last_phase": self.last_phase})
    }
}

// vhco:domain EffectSite { effect_id: string; operation_id: string; kind: SiteKind; access: AccessVerb[]; method?: string; protocol?: string; capability: Capability; target: SiteTarget; expression?: string; knowledge: Knowledge; condition?: string; call_chain: string[]; secrets: string[]; bound_to: string[]; source: SourceSpan; statement_line: int; origin: SiteOrigin; phase: Phase; requires_existing: bool; secret: bool; via?: string; note?: string; entries: string[]; decision?: string; attempts?: AttemptSummary }
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSite {
    pub effect_id: String,
    pub operation_id: String,
    pub kind: SiteKind,
    pub access: Vec<AccessVerb>,
    /// HTTP method (`GET`), gRPC call mode (`unary`) or MCP call type (`tool`).
    pub method: Option<String>,
    pub protocol: Option<String>,
    pub capability: Capability,
    pub target: SiteTarget,
    /// Source expression of a dynamic target (`config.url`); never evaluated.
    pub expression: Option<String>,
    pub knowledge: Knowledge,
    pub condition: Option<String>,
    pub call_chain: Vec<String>,
    /// Names of `secret` bindings this site uses (never values).
    pub secrets: Vec<String>,
    /// Origins a secret read is bound to (`secret … for ORIGIN`).
    pub bound_to: Vec<String>,
    pub source: SourceSpan,
    /// Line of the statement that performs the effect (an option site's
    /// block line); joins broker attempts back to sites.
    pub statement_line: u32,
    pub origin: SiteOrigin,
    pub phase: Phase,
    pub requires_existing: bool,
    pub secret: bool,
    /// Connector method (`crm.tools.search`) whose expansion produced this site.
    pub via: Option<String>,
    /// Short reviewer note shown in USED BY (`token endpoint`, `dynamic: …`).
    pub note: Option<String>,
    /// Selected entry operations that reach this site (filled by selection).
    pub entries: Vec<String>,
    pub decision: Option<String>,
    pub attempts: Option<AttemptSummary>,
}

impl EffectSite {
    /// A site with defaults; the analysis fills the rest.
    pub fn new(operation_id: &str, kind: SiteKind, access: Vec<AccessVerb>) -> EffectSite {
        let capability = access
            .first()
            .map(|v| capability_for(kind, *v))
            .unwrap_or(Capability::Read);
        EffectSite {
            effect_id: String::new(),
            operation_id: operation_id.to_string(),
            kind,
            access,
            method: None,
            protocol: None,
            capability,
            target: SiteTarget::default(),
            expression: None,
            knowledge: Knowledge::Exact,
            condition: None,
            call_chain: vec![operation_id.to_string()],
            secrets: Vec::new(),
            bound_to: Vec::new(),
            source: SourceSpan::default(),
            statement_line: 0,
            origin: SiteOrigin::Statement(String::new()),
            phase: Phase::Body,
            requires_existing: false,
            secret: false,
            via: None,
            note: None,
            entries: Vec::new(),
            decision: None,
            attempts: None,
        }
    }

    pub fn access_label(&self) -> String {
        let verbs: Vec<&str> = self.access.iter().map(|v| v.as_str()).collect();
        match &self.method {
            Some(m) => format!("{} {}", verbs.join(", "), m),
            None => verbs.join(", "),
        }
    }

    /// Display form of the target: URL/path template, `env NAME`, logical
    /// name, or `<dynamic: EXPR>`.
    pub fn target_display(&self) -> String {
        if self.target.template.is_empty() {
            return format!(
                "<dynamic: {}>",
                self.expression.as_deref().unwrap_or("expression")
            );
        }
        match self.kind {
            SiteKind::Env => format!("env {}", self.target.template),
            _ => self.target.template.clone(),
        }
    }

    /// The unit a grant uses: URL origin, glob for param_dependent paths, template otherwise.
    pub fn grant_target(&self) -> String {
        match (&self.target.scheme, &self.target.host) {
            (Some(s), Some(h)) => match self.target.port {
                Some(p) => format!("{s}://{h}:{p}"),
                None => format!("{s}://{h}"),
            },
            _ => self
                .target
                .glob
                .clone()
                .unwrap_or_else(|| self.target.template.clone()),
        }
    }

    /// `file:line` as shown in the SOURCE column.
    pub fn source_label(&self) -> String {
        format!("{}:{}", self.source.file, self.source.start_line)
    }

    pub fn to_json(&self) -> Json {
        let template = if self.target.template.is_empty() {
            Json::Null
        } else {
            Json::String(self.target.template.clone())
        };
        json!({
            "effect_id": self.effect_id,
            "operation_id": self.operation_id,
            "kind": self.kind.as_str(),
            "access": self.access.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "method": self.method,
            "protocol": self.protocol,
            "capability": self.capability.as_str(),
            "target": {
                "template": template,
                "expression": self.expression,
                "scheme": self.target.scheme,
                "host": self.target.host,
                "port": self.target.port,
                "path": self.target.path,
                "glob": self.target.glob,
                "params": self.target.params,
            },
            "knowledge": self.knowledge.as_str(),
            "condition": self.condition,
            "call_chain": self.call_chain,
            "secrets": self.secrets,
            "source": {"file": self.source.file, "line": self.source.start_line, "column": self.source.start_col},
            "origin": self.origin.to_json(),
            "phase": self.phase.as_str(),
            "requires_existing": self.requires_existing,
            "secret": self.secret,
            "via": self.via,
            "decision": self.decision,
            "attempts": self.attempts.as_ref().map(AttemptSummary::to_json),
        })
    }
}

// vhco:domain CallArg { param: string; from_param?: string; value?: Value }
/// One statically known argument of a call edge: the callee's `param` is the
/// caller's param `from_param`, or the constant `value` (a literal or a
/// global). `policy explain CALLER --params` follows these to fill the
/// callee's `{param}` targets.
#[derive(Clone, Debug, PartialEq)]
pub struct CallArg {
    pub param: String,
    pub from_param: Option<String>,
    pub value: Option<crate::domain::Value>,
}

// vhco:domain CallSite { operation_id: string; callee: string; connector?: string; source: SourceSpan; args: CallArg[] }
/// A literal `(request "id" …)` edge: shown as a `(calls X)` row, never a site.
#[derive(Clone, Debug, PartialEq)]
pub struct CallSite {
    pub operation_id: String,
    pub callee: String,
    /// Set when the callee is an imported connector method (`crm.tools.search`).
    pub connector: Option<String>,
    pub source: SourceSpan,
    /// Arguments of a `{key: …}` object literal whose value is a caller param
    /// or a constant (others are omitted: not statically known).
    pub args: Vec<CallArg>,
}

// vhco:domain OperationEffects { operation_id: string; private: bool; sites: EffectSite[]; calls: CallSite[] }
/// The lowered, non-transitive effect sites of one operation.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct OperationEffects {
    pub operation_id: String,
    pub private: bool,
    pub sites: Vec<EffectSite>,
    pub calls: Vec<CallSite>,
}

// vhco:domain EffectCatalog { operations: OperationEffects[]; load_sites: EffectSite[]; bundle_file: string; bundle_sha256: string }
/// Every operation's sites, computed once from the immutable compiled program.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EffectCatalog {
    pub operations: Vec<OperationEffects>,
    /// Connector-level `descriptor` / `schema` reads (bootstrap, phase load).
    pub load_sites: Vec<EffectSite>,
    pub bundle_file: String,
    pub bundle_sha256: String,
}

impl EffectCatalog {
    pub fn operation(&self, id: &str) -> Option<&OperationEffects> {
        self.operations.iter().find(|o| o.operation_id == id)
    }
}

// vhco:domain FileDigest { file: string; sha256: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FileDigest {
    pub file: String,
    pub sha256: String,
}

impl FileDigest {
    pub fn to_json(&self) -> Json {
        json!({"file": self.file, "sha256": self.sha256})
    }
}

// vhco:domain TargetSummary { target: string; kind: SiteKind; capability: Capability; access: AccessVerb[]; methods: string[]; protocols: string[]; origins: SiteOrigin[]; phases: Phase[]; needs_file?: string; operations: string[]; used_by: string[]; knowledge: Knowledge; decision?: string }
/// One `--by target` / `--by capability` row: sites grouped on (target, capability).
#[derive(Clone, Debug, PartialEq)]
pub struct TargetSummary {
    pub target: String,
    pub kind: SiteKind,
    pub capability: Capability,
    pub access: Vec<AccessVerb>,
    pub methods: Vec<String>,
    pub protocols: Vec<String>,
    pub origins: Vec<SiteOrigin>,
    pub phases: Vec<Phase>,
    pub needs_file: Option<String>,
    pub operations: Vec<String>,
    /// USED BY labels: `op`, `entry (via op)`, then notes.
    pub used_by: Vec<String>,
    pub knowledge: Knowledge,
    pub decision: Option<String>,
}

impl TargetSummary {
    pub fn to_json(&self) -> Json {
        json!({
            "target": self.target,
            "capability": self.capability.as_str(),
            "access": self.access.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "methods": self.methods,
            "protocols": self.protocols,
            "origins": self.origins.iter().map(SiteOrigin::to_json).collect::<Vec<_>>(),
            "phases": self.phases.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "needs_file": self.needs_file,
            "operations": self.operations,
            "used_by": self.used_by,
            "knowledge": self.knowledge.as_str(),
            "decision": self.decision,
        })
    }
}

// vhco:domain FileStatus { present | missing | unreadable | not_permitted | not_checkable }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Present,
    Missing,
    Unreadable,
    NotPermitted,
    NotCheckable,
}

impl FileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FileStatus::Present => "present",
            FileStatus::Missing => "missing",
            FileStatus::Unreadable => "unreadable",
            FileStatus::NotPermitted => "not_permitted",
            FileStatus::NotCheckable => "not_checkable",
        }
    }
}

// vhco:domain NeededFile { path: string; effect_id: string; origin: SiteOrigin; phase: Phase; secret: bool; knowledge: Knowledge; via?: string; source: SourceSpan; status?: FileStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct NeededFile {
    pub path: String,
    pub effect_id: String,
    pub origin: SiteOrigin,
    pub phase: Phase,
    pub secret: bool,
    pub knowledge: Knowledge,
    pub via: Option<String>,
    pub source: SourceSpan,
    pub status: Option<FileStatus>,
}

impl NeededFile {
    pub fn to_json(&self) -> Json {
        json!({
            "path": self.path,
            "effect_id": self.effect_id,
            "origin": self.origin.to_json(),
            "phase": self.phase.as_str(),
            "secret": self.secret,
            "knowledge": self.knowledge.as_str(),
            "via": self.via,
            "source": {"file": self.source.file, "line": self.source.start_line, "column": self.source.start_col},
            "status": self.status.map(FileStatus::as_str),
        })
    }
}

// vhco:domain OperationNeeds { operation_id: string; files: NeededFile[] }
#[derive(Clone, Debug, PartialEq)]
pub struct OperationNeeds {
    /// An operation ID, or `bundle load` for connector descriptor/schema files.
    pub operation_id: String,
    pub files: Vec<NeededFile>,
}

impl OperationNeeds {
    pub fn to_json(&self) -> Json {
        json!({
            "operation_id": self.operation_id,
            "files": self.files.iter().map(NeededFile::to_json).collect::<Vec<_>>(),
        })
    }
}

// vhco:domain IoManifest { bundle: FileDigest; policy?: FileDigest; complete: bool; sites: EffectSite[]; calls: CallSite[]; targets: TargetSummary[]; needs: OperationNeeds[]; bootstrap: EffectSite[]; unplanned: TraceEvent[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct IoManifest {
    pub bundle: FileDigest,
    pub policy: Option<FileDigest>,
    pub complete: bool,
    pub sites: Vec<EffectSite>,
    /// `(calls X)` rows of the selected operations (not sites).
    pub calls: Vec<CallSite>,
    pub targets: Vec<TargetSummary>,
    pub needs: Vec<OperationNeeds>,
    pub bootstrap: Vec<EffectSite>,
    /// Trace attempts with no matching planned site (`--trace`).
    pub unplanned: Vec<TraceEvent>,
}

impl IoManifest {
    pub fn to_json(&self) -> Json {
        let mut v = json!({
            "bundle": self.bundle.to_json(),
            "policy": self.policy.as_ref().map(FileDigest::to_json),
            "complete": self.complete,
            "sites": self.sites.iter().map(EffectSite::to_json).collect::<Vec<_>>(),
            "targets": self.targets.iter().map(TargetSummary::to_json).collect::<Vec<_>>(),
            "needs": self.needs.iter().map(OperationNeeds::to_json).collect::<Vec<_>>(),
            "bootstrap": self.bootstrap.iter().map(EffectSite::to_json).collect::<Vec<_>>(),
        });
        if !self.unplanned.is_empty() {
            v["unplanned"] = Json::Array(self.unplanned.iter().map(TraceEvent::to_json).collect());
        }
        v
    }
}

// vhco:domain IoQuery { ids: string[]; all: bool; transitive: bool; strict: bool; include_bootstrap: bool; by: string; kind?: string; access: string[]; format: string; check_policy: bool; needs: bool; check_files: bool; trace_request_id?: string; params?: Json }
/// `rivet io` / `rivet.io` / `rt.io` query (the contract's `EffectQuery`).
#[derive(Clone, Debug, PartialEq)]
pub struct IoQuery {
    pub ids: Vec<String>,
    pub all: bool,
    pub transitive: bool,
    pub strict: bool,
    pub include_bootstrap: bool,
    /// `operation` (default) | `target` | `capability`.
    pub by: String,
    pub kind: Option<String>,
    pub access: Vec<String>,
    /// `table` (default) | `json` | `markdown` | `csv`.
    pub format: String,
    pub check_policy: bool,
    pub needs: bool,
    pub check_files: bool,
    pub trace_request_id: Option<String>,
    /// `policy explain ID --params JSON`: the caller params of ONE call. When present,
    /// every site of the selected entry operation whose template placeholders are all
    /// params is filled with these values and evaluated as that exact target.
    pub params: Option<crate::domain::Value>,
}

impl Default for IoQuery {
    fn default() -> Self {
        IoQuery {
            ids: Vec::new(),
            all: false,
            transitive: true,
            strict: false,
            include_bootstrap: false,
            by: "operation".into(),
            kind: None,
            access: Vec::new(),
            format: "table".into(),
            check_policy: false,
            needs: false,
            check_files: false,
            trace_request_id: None,
            params: None,
        }
    }
}

/// Name used by the contract.
pub type EffectQuery = IoQuery;

// vhco:domain IoReport { manifest: IoManifest; by: string; format: string; rendered: string; diagnostics: string; exit_code: int }
#[derive(Clone, Debug, PartialEq)]
pub struct IoReport {
    pub manifest: IoManifest,
    pub by: String,
    pub format: String,
    /// stdout text in the requested format.
    pub rendered: String,
    /// stderr summary lines (counts, incompleteness, probe summary).
    pub diagnostics: String,
    pub exit_code: u8,
}

// vhco:domain TraceEvent { request_id: string; trace_id: string; node_id?: string; attempt: int; effect_id?: string; operation_id: string; phase: string; capability: string; access: string; target: string; decision: string; policy_hash: string; source?: SourceSpan; outcome: Json }
/// One broker decision/attempt recorded for a request (secret values never included).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceEvent {
    pub request_id: String,
    pub trace_id: String,
    pub node_id: Option<String>,
    pub attempt: u32,
    pub effect_id: Option<String>,
    pub operation_id: String,
    pub phase: String,
    pub capability: String,
    pub access: String,
    pub target: String,
    pub decision: String,
    pub policy_hash: String,
    pub source: Option<SourceSpan>,
    pub outcome: Json,
}

impl TraceEvent {
    pub fn to_json(&self) -> Json {
        json!({
            "request_id": self.request_id,
            "trace_id": self.trace_id,
            "node_id": self.node_id,
            "attempt": self.attempt,
            "effect_id": self.effect_id,
            "operation_id": self.operation_id,
            "phase": self.phase,
            "capability": self.capability,
            "access": self.access,
            "target": self.target,
            "decision": self.decision,
            "policy_hash": self.policy_hash,
            "source": self.source.as_ref().map(|s| json!({"file": s.file, "line": s.start_line, "column": s.start_col})),
            "outcome": self.outcome,
        })
    }
}

// vhco:domain TraceQuery { request_id: string; cursor?: string; limit: int }
#[derive(Clone, Debug, PartialEq)]
pub struct TraceQuery {
    pub request_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

impl TraceQuery {
    pub fn new(request_id: &str) -> TraceQuery {
        TraceQuery {
            request_id: request_id.to_string(),
            cursor: None,
            limit: 1000,
        }
    }
}

// vhco:domain TraceResult { request_id: string; events: TraceEvent[]; complete: bool; next_cursor?: string; gaps: int }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TraceResult {
    pub request_id: String,
    pub events: Vec<TraceEvent>,
    /// False when the bounded store evicted part of this request's events.
    pub complete: bool,
    pub next_cursor: Option<String>,
    pub gaps: u32,
}

impl TraceResult {
    pub fn to_json(&self) -> Json {
        json!({
            "request_id": self.request_id,
            "attempts": self.events.iter().map(TraceEvent::to_json).collect::<Vec<_>>(),
            "complete": self.complete,
            "next_cursor": self.next_cursor,
            "gaps": self.gaps,
        })
    }
}

// vhco:domain FileProbeInput { path: string; effect_id: string }
#[derive(Clone, Debug, PartialEq)]
pub struct FileProbeInput {
    /// Bundle-root-relative path as written in source.
    pub path: String,
    pub effect_id: String,
}

// vhco:domain FileProbeResult { path: string; status: FileStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct FileProbeResult {
    pub path: String,
    pub status: FileStatus,
}

// vhco:domain PolicyDraft { grants: Grant[]; review: EffectSite[]; complete: bool; written_to?: string; exit_code: int }
/// A least-privilege policy.json v1 draft (`policy generate`, `rt.generate_policy`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PolicyDraft {
    pub grants: Vec<Grant>,
    pub review: Vec<EffectSite>,
    pub complete: bool,
    pub written_to: Option<String>,
    pub exit_code: u8,
}

impl PolicyDraft {
    fn grant_json(g: &Grant) -> Json {
        let mut o = serde_json::Map::new();
        o.insert(
            "capability".into(),
            Json::String(g.capability.as_str().into()),
        );
        o.insert("targets".into(), json!(g.targets));
        if let Some(a) = &g.access {
            o.insert(
                "access".into(),
                Json::Array(a.iter().map(|v| Json::String(v.as_str().into())).collect()),
            );
        }
        Json::Object(o)
    }

    /// The policy.json v1 document itself.
    pub fn policy_json(&self) -> Json {
        let grants: Vec<Json> = self.grants.iter().map(Self::grant_json).collect();
        json!({"version": 1, "grants": grants, "network": {"deny_private_ranges": true}})
    }

    /// `{policy, review, complete}` as returned by `rivet.policy.generate`.
    pub fn to_json(&self) -> Json {
        json!({
            "policy": self.policy_json(),
            "review": self.review.iter().map(EffectSite::to_json).collect::<Vec<_>>(),
            "complete": self.complete,
        })
    }

    /// policy.json text with one grant per line (the reference layout).
    pub fn render(&self) -> String {
        let mut out = String::from("{\n  \"version\": 1,\n  \"grants\": [\n");
        let n = self.grants.len();
        for (i, g) in self.grants.iter().enumerate() {
            let line = serde_json::to_string(&Self::grant_json(g)).unwrap_or_default();
            out.push_str("    ");
            out.push_str(&line.replace("\":", "\": ").replace(",\"", ", \""));
            if i + 1 < n {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ],\n  \"network\": {\"deny_private_ranges\": true}\n}\n");
        out
    }
}

// vhco:domain PolicyDraftFile { path: string; bytes: bytes }
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyDraftFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

// vhco:domain PolicyDraftReceipt { path: string; sha256: string }
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyDraftReceipt {
    pub path: String,
    pub sha256: String,
}

/// The fixed runtime-internal bootstrap list (PROP-2026-0001 Increment 5).
pub fn bootstrap_list() -> Vec<(String, String)> {
    [
        ("entry bundle + declared imports", "read at load"),
        ("policy.json (or --policy PATH)", "read at load"),
        ("CA bundle / system trust store", "read by TLS adapters"),
        ("resolv.conf / system resolver", "read by DNS"),
        ("tzdata", "read for timestamps"),
        (
            "descriptor / schema snapshot files",
            "read at load (gRPC descriptors, MCP snapshots)",
        ),
        ("stdin / stdout / stderr", "invocation channels only"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

/// Capability that owns a verb for a given site kind.
pub fn capability_for(kind: SiteKind, verb: AccessVerb) -> Capability {
    match (kind, verb) {
        (
            SiteKind::File,
            AccessVerb::Read | AccessVerb::List | AccessVerb::Stat | AccessVerb::Watch,
        ) => Capability::Read,
        (SiteKind::File, AccessVerb::Delete) => Capability::Delete,
        (SiteKind::File, _) => Capability::Write,
        (SiteKind::Network, AccessVerb::Bind | AccessVerb::Listen | AccessVerb::MulticastJoin) => {
            Capability::Listen
        }
        (SiteKind::Network, _) => Capability::Network,
        (SiteKind::Process, _) => Capability::Exec,
        (SiteKind::Env, _) => Capability::Env,
        (SiteKind::Pipe, _) => Capability::Pipe,
        (SiteKind::Unix, _) => Capability::Unix,
        (SiteKind::Mcp, _) => Capability::Mcp,
        (SiteKind::Grpc, _) => Capability::Grpc,
        (SiteKind::Auth, _) => Capability::Auth,
        (SiteKind::Credential, _) => Capability::Credentials,
    }
}
