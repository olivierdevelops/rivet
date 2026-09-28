//! Generated I/O manifest (R6, R26; PROP-2026-0001 Increment 18).

use super::policy::{AccessVerb, Capability};
use super::source::SourceSpan;
use serde_json::{Value as Json, json};

// vhco:domain Knowledge { exact | bounded | param_dependent | dynamic | opaque_remote | opaque_native }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

// vhco:domain SiteOrigin { statement: string | option: string }
#[derive(Clone, Debug, PartialEq)]
pub enum SiteOrigin {
    Statement(String),
    Option(String),
}

impl SiteOrigin {
    pub fn label(&self) -> String {
        match self {
            SiteOrigin::Statement(s) => s.clone(),
            SiteOrigin::Option(o) => format!("option: {o}"),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

// vhco:domain SiteTarget { template: string; scheme?: string; host?: string; port?: int; path?: string; glob?: string; params: string[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SiteTarget {
    pub template: String,
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub glob: Option<String>,
    pub params: Vec<String>,
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

// vhco:domain EffectSite { effect_id: string; operation_id: string; kind: SiteKind; access: AccessVerb[]; method?: string; protocol?: string; capability: Capability; target: SiteTarget; knowledge: Knowledge; call_chain: string[]; secrets: string[]; source: SourceSpan; origin: SiteOrigin; phase: Phase; requires_existing: bool; secret: bool; decision?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSite {
    pub effect_id: String,
    pub operation_id: String,
    pub kind: SiteKind,
    pub access: Vec<AccessVerb>,
    pub method: Option<String>,
    pub protocol: Option<String>,
    pub capability: Capability,
    pub target: SiteTarget,
    pub knowledge: Knowledge,
    pub condition: Option<String>,
    pub call_chain: Vec<String>,
    pub secrets: Vec<String>,
    pub source: SourceSpan,
    pub origin: SiteOrigin,
    pub phase: Phase,
    pub requires_existing: bool,
    pub secret: bool,
    pub decision: Option<String>,
}

impl EffectSite {
    pub fn access_label(&self) -> String {
        let verbs: Vec<&str> = self.access.iter().map(|v| v.as_str()).collect();
        match &self.method {
            Some(m) => format!("{} {}", verbs.join(", "), m),
            None => verbs.join(", "),
        }
    }

    pub fn to_json(&self) -> Json {
        json!({
            "effect_id": self.effect_id,
            "operation_id": self.operation_id,
            "kind": self.kind.as_str(),
            "access": self.access.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "method": self.method,
            "protocol": self.protocol,
            "capability": self.capability.as_str(),
            "target": {
                "template": self.target.template,
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
            "decision": self.decision,
        })
    }
}

// vhco:domain IoManifest { bundle_file: string; bundle_sha256: string; policy_file?: string; policy_sha256?: string; complete: bool; sites: EffectSite[]; bootstrap: string[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct IoManifest {
    pub bundle_file: String,
    pub bundle_sha256: String,
    pub policy_file: Option<String>,
    pub policy_sha256: Option<String>,
    pub complete: bool,
    pub sites: Vec<EffectSite>,
    pub bootstrap: Vec<(String, String)>,
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
