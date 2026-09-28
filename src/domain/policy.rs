//! Policy file model, capabilities, access verbs and effect intents (R11, R24, R26).

use super::source::SourceSpan;

// vhco:domain Capability { allow_read | allow_write | allow_delete | allow_network | allow_exec | allow_env | allow_unix | allow_pipe | allow_listen | allow_mcp | allow_grpc | allow_auth | allow_credentials }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    Read,
    Write,
    Delete,
    Network,
    Exec,
    Env,
    Unix,
    Pipe,
    Listen,
    Mcp,
    Grpc,
    Auth,
    Credentials,
}

pub const ALL_CAPABILITIES: [Capability; 13] = [
    Capability::Read,
    Capability::Write,
    Capability::Delete,
    Capability::Network,
    Capability::Exec,
    Capability::Env,
    Capability::Unix,
    Capability::Pipe,
    Capability::Listen,
    Capability::Mcp,
    Capability::Grpc,
    Capability::Auth,
    Capability::Credentials,
];

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Read => "allow_read",
            Capability::Write => "allow_write",
            Capability::Delete => "allow_delete",
            Capability::Network => "allow_network",
            Capability::Exec => "allow_exec",
            Capability::Env => "allow_env",
            Capability::Unix => "allow_unix",
            Capability::Pipe => "allow_pipe",
            Capability::Listen => "allow_listen",
            Capability::Mcp => "allow_mcp",
            Capability::Grpc => "allow_grpc",
            Capability::Auth => "allow_auth",
            Capability::Credentials => "allow_credentials",
        }
    }

    pub fn parse(s: &str) -> Option<Capability> {
        ALL_CAPABILITIES.iter().copied().find(|c| c.as_str() == s)
    }

    /// The access verbs that belong to this capability.
    pub fn verbs(self) -> &'static [AccessVerb] {
        use AccessVerb::*;
        match self {
            Capability::Read => &[Read, List, Stat, Watch],
            Capability::Write => &[Create, Update, Append],
            Capability::Delete => &[Delete],
            Capability::Network => &[Connect],
            Capability::Listen => &[Bind, Listen, MulticastJoin],
            Capability::Exec => &[Exec],
            Capability::Env => &[Read],
            Capability::Pipe => &[Read, Write],
            Capability::Unix => &[Connect, Listen],
            Capability::Mcp => &[Call],
            Capability::Grpc => &[Call],
            Capability::Auth => &[Use, Manage, Status],
            Capability::Credentials => &[Read, Write],
        }
    }

    /// True when the capability's targets are filesystem paths.
    pub fn is_path_like(self) -> bool {
        matches!(
            self,
            Capability::Read
                | Capability::Write
                | Capability::Delete
                | Capability::Exec
                | Capability::Unix
                | Capability::Pipe
        )
    }
}

// vhco:domain AccessVerb { read | list | stat | watch | create | update | append | delete | connect | bind | listen | multicast_join | exec | write | call | use | manage | status }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccessVerb {
    Read,
    List,
    Stat,
    Watch,
    Create,
    Update,
    Append,
    Delete,
    Connect,
    Bind,
    Listen,
    MulticastJoin,
    Exec,
    Write,
    Call,
    Use,
    Manage,
    Status,
}

impl AccessVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessVerb::Read => "read",
            AccessVerb::List => "list",
            AccessVerb::Stat => "stat",
            AccessVerb::Watch => "watch",
            AccessVerb::Create => "create",
            AccessVerb::Update => "update",
            AccessVerb::Append => "append",
            AccessVerb::Delete => "delete",
            AccessVerb::Connect => "connect",
            AccessVerb::Bind => "bind",
            AccessVerb::Listen => "listen",
            AccessVerb::MulticastJoin => "multicast_join",
            AccessVerb::Exec => "exec",
            AccessVerb::Write => "write",
            AccessVerb::Call => "call",
            AccessVerb::Use => "use",
            AccessVerb::Manage => "manage",
            AccessVerb::Status => "status",
        }
    }

    pub fn parse(s: &str) -> Option<AccessVerb> {
        use AccessVerb::*;
        [
            Read,
            List,
            Stat,
            Watch,
            Create,
            Update,
            Append,
            Delete,
            Connect,
            Bind,
            Listen,
            MulticastJoin,
            Exec,
            Write,
            Call,
            Use,
            Manage,
            Status,
        ]
        .into_iter()
        .find(|v| v.as_str() == s)
    }
}

// vhco:domain Grant { capability: Capability; targets: string[]; access?: AccessVerb[] }
#[derive(Clone, Debug, PartialEq)]
pub struct Grant {
    pub capability: Capability,
    pub targets: Vec<String>,
    pub access: Option<Vec<AccessVerb>>,
}

impl Grant {
    pub fn allows_verb(&self, verb: AccessVerb) -> bool {
        match &self.access {
            None => true,
            Some(v) => v.contains(&verb),
        }
    }
}

// vhco:domain NetworkPolicy { deny_private_ranges: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkPolicy {
    pub deny_private_ranges: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        NetworkPolicy {
            deny_private_ranges: true,
        }
    }
}

// vhco:domain PolicyLimits { max_concurrent_requests: int; max_call_depth: int; max_buffered_bytes: int }
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyLimits {
    pub max_concurrent_requests: u32,
    pub max_call_depth: u32,
    pub max_buffered_bytes: u64,
}

impl Default for PolicyLimits {
    fn default() -> Self {
        PolicyLimits {
            max_concurrent_requests: 64,
            max_call_depth: 16,
            max_buffered_bytes: 256 * 1024 * 1024,
        }
    }
}

// vhco:domain ApprovedHashes { snapshots: string[]; overlaps: string[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ApprovedHashes {
    pub snapshots: Vec<String>,
    pub overlaps: Vec<String>,
}

// vhco:domain ServeAuth { none | bearer: BearerTokenHash[] | mtls: { client_ca: string; principals: MtlsPrincipal[] } }
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ServeAuth {
    #[default]
    None,
    Bearer(Vec<BearerTokenHash>),
    Mtls {
        client_ca: String,
        principals: Vec<MtlsPrincipal>,
    },
}

// vhco:domain BearerTokenHash { principal: string; sha256: string }
#[derive(Clone, Debug, PartialEq)]
pub struct BearerTokenHash {
    pub principal: String,
    pub sha256: String,
}

// vhco:domain MtlsPrincipal { principal: string; subject: string }
#[derive(Clone, Debug, PartialEq)]
pub struct MtlsPrincipal {
    pub principal: String,
    pub subject: String,
}

// vhco:domain PrincipalGrant { principal: string; operations: string[] }
#[derive(Clone, Debug, PartialEq)]
pub struct PrincipalGrant {
    pub principal: String,
    pub operations: Vec<String>,
}

// vhco:domain ServePolicy { surfaces: string[]; auth: ServeAuth; principals?: PrincipalGrant[] }
#[derive(Clone, Debug, PartialEq)]
pub struct ServePolicy {
    pub surfaces: Vec<String>,
    pub auth: ServeAuth,
    pub principals: Option<Vec<PrincipalGrant>>,
}

pub const ALL_SURFACES: [&str; 5] = ["http", "sse", "poll", "ws", "mcp"];

impl Default for ServePolicy {
    fn default() -> Self {
        ServePolicy {
            surfaces: ALL_SURFACES.iter().map(|s| s.to_string()).collect(),
            auth: ServeAuth::None,
            principals: None,
        }
    }
}

// vhco:domain Policy { present: bool; file?: string; base_dir: string; sha256?: string; grants: Grant[]; deny: Grant[]; network: NetworkPolicy; limits: PolicyLimits; approved: ApprovedHashes; serve: ServePolicy }
/// Effective policy. `present == false` means no policy file was found:
/// deny-by-default for every new application effect.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Policy {
    pub present: bool,
    pub file: Option<String>,
    pub base_dir: String,
    pub sha256: Option<String>,
    pub grants: Vec<Grant>,
    pub deny: Vec<Grant>,
    pub network: NetworkPolicy,
    pub limits: PolicyLimits,
    pub approved: ApprovedHashes,
    pub serve: ServePolicy,
}

impl Policy {
    /// Deny-by-default policy used when no file exists.
    pub fn deny_all(base_dir: &str) -> Policy {
        Policy {
            base_dir: base_dir.to_string(),
            ..Policy::default()
        }
    }
}

// vhco:domain EffectTarget { path: string | url: string | env: string | operation: string | profile: string | text: string }
#[derive(Clone, Debug, PartialEq)]
pub enum EffectTarget {
    /// Filesystem path as written (resolved relative to the bundle root by the broker).
    Path(String),
    /// Absolute URL (`https://host:port/path`, `tcp://host:port`, `udp://…`).
    Url(String),
    Env(String),
    /// Logical target such as `CONNECTOR/Service/Method` or `crm.tools.search`.
    Logical(String),
}

impl EffectTarget {
    pub fn as_str(&self) -> &str {
        match self {
            EffectTarget::Path(s)
            | EffectTarget::Url(s)
            | EffectTarget::Env(s)
            | EffectTarget::Logical(s) => s,
        }
    }
}

// vhco:domain EffectIntent { capability: Capability; verb: AccessVerb; target: EffectTarget; operation_id: string; effect_id?: string; span?: SourceSpan }
#[derive(Clone, Debug, PartialEq)]
pub struct EffectIntent {
    pub capability: Capability,
    pub verb: AccessVerb,
    pub target: EffectTarget,
    pub operation_id: String,
    pub effect_id: Option<String>,
    pub span: Option<SourceSpan>,
}

// vhco:domain Decision { allowed | denied }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Denied,
}

// vhco:domain Permit { intent: EffectIntent; decision: Decision; rule: string }
#[derive(Clone, Debug, PartialEq)]
pub struct Permit {
    pub intent: EffectIntent,
    pub decision: Decision,
    /// Human-readable reason: the matching grant/deny entry or "no policy.json".
    pub rule: String,
}
