//! MCP client connector vocabulary (PROP-2026-0001 Increment 6; REF S52–S61).
//!
//! ```text
//!  connector crm mcp … end ──bundle load──▶ McpConnectorInfo (transport, exposure, reviewed snapshot)
//!                                               │  sha256(schema file) ∈ policy.approved.snapshots
//!                                               ▼
//!  imports: crm.tools.<alias>  crm.resources.read  crm.prompts.get   (only exposed names)
//!                                               │
//!  (request "crm.tools.search" {…}) ──▶ McpRequest ──connectors.invoke_mcp──▶ McpResult
//! ```
//!
//! Snapshot file format (`schema PATH`, written by `rivet connectors sync`):
//!
//! ```json
//! {"format": "rivet.mcp.snapshot/1", "protocolVersion": "2025-11-25",
//!  "serverInfo": {"name": "crm", "version": "1.0"},
//!  "tools": [{"name", "title"?, "description"?, "inputSchema", "outputSchema"?}],
//!  "resources": [{"uri", "name"?, "description"?, "mimeType"?}],
//!  "prompts": [{"name", "description"?, "arguments": [{"name", "description"?, "required"?}]}]}
//! ```
//! Only `protocolVersion` and `tools` are required. The file is hashed as
//! written; a snapshot is *reviewed* when `sha256:<hex>` of its bytes is listed
//! in policy.json `approved.snapshots`.

use super::errors::{ErrorKind, RivetError, RivetResult};
use super::source::SourceSpan;
use super::value::Value;
use serde_json::{Value as Json, json};

/// Protocol version this client offers first.
pub const MCP_CLIENT_VERSION: &str = "2025-11-25";
/// Server versions the client accepts in the initialize response.
pub const MCP_COMPATIBLE_VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];
/// Maximum bridge hops (Rivet → MCP → Rivet → …) before a call is refused.
pub const MAX_MCP_HOPS: u32 = 8;
/// `format` marker written into snapshots.
pub const SNAPSHOT_FORMAT: &str = "rivet.mcp.snapshot/1";
/// `_meta` keys that carry the bridge hop count and identity chain.
pub const META_HOPS: &str = "rivet/hops";
pub const META_CHAIN: &str = "rivet/chain";

// vhco:domain McpTransport { http: {url: string} | command: {program: string; args: string[]; env: [string,string][]} }
#[derive(Clone, Debug, PartialEq)]
pub enum McpTransport {
    /// Streamable HTTP endpoint (`transport http URL`).
    Http { url: String },
    /// stdio child (`transport command BIN` + `args`/`env`).
    Command {
        program: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
}

// vhco:domain McpTls { server_name?: string; ca_file?: string; cert_file?: string; key_file?: string }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct McpTls {
    pub server_name: Option<String>,
    pub ca_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
}

// vhco:domain McpToolSpec { name: string; title?: string; description?: string; input_schema: Json; output_schema?: Json }
#[derive(Clone, Debug, PartialEq)]
pub struct McpToolSpec {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub input_schema: Json,
    pub output_schema: Option<Json>,
}

// vhco:domain McpResourceSpec { uri: string; name?: string; description?: string; mime_type?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct McpResourceSpec {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

// vhco:domain McpPromptArg { name: string; description?: string; required: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct McpPromptArg {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

// vhco:domain McpPromptSpec { name: string; description?: string; arguments: McpPromptArg[] }
#[derive(Clone, Debug, PartialEq)]
pub struct McpPromptSpec {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<McpPromptArg>,
}

// vhco:domain McpSnapshot { protocol_version: string; server_info?: Json; tools: McpToolSpec[]; resources: McpResourceSpec[]; prompts: McpPromptSpec[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct McpSnapshot {
    pub protocol_version: String,
    pub server_info: Option<Json>,
    pub tools: Vec<McpToolSpec>,
    pub resources: Vec<McpResourceSpec>,
    pub prompts: Vec<McpPromptSpec>,
}

fn snap_err(msg: impl Into<String>) -> RivetError {
    RivetError::validation("mcp.snapshot", msg)
}

fn opt_str(o: &serde_json::Map<String, Json>, key: &str) -> Option<String> {
    o.get(key).and_then(Json::as_str).map(str::to_string)
}

impl McpSnapshot {
    /// Parse the snapshot file format (strict top-level keys).
    pub fn parse(bytes: &[u8]) -> RivetResult<McpSnapshot> {
        let j: Json = serde_json::from_slice(bytes)
            .map_err(|e| snap_err(format!("snapshot is not valid JSON: {e}")))?;
        let obj = j
            .as_object()
            .ok_or_else(|| snap_err("snapshot must be a JSON object"))?;
        for k in obj.keys() {
            if !matches!(
                k.as_str(),
                "format" | "protocolVersion" | "serverInfo" | "tools" | "resources" | "prompts"
            ) {
                return Err(snap_err(format!("unknown snapshot key `{k}`")));
            }
        }
        if let Some(f) = obj.get("format")
            && f.as_str() != Some(SNAPSHOT_FORMAT)
        {
            return Err(snap_err(format!(
                "snapshot format must be \"{SNAPSHOT_FORMAT}\""
            )));
        }
        let protocol_version = opt_str(obj, "protocolVersion")
            .ok_or_else(|| snap_err("snapshot needs \"protocolVersion\""))?;
        let list = |key: &str, required: bool| -> RivetResult<Vec<Json>> {
            match obj.get(key) {
                None if !required => Ok(Vec::new()),
                None => Err(snap_err(format!("snapshot needs \"{key}\""))),
                Some(Json::Array(a)) => Ok(a.clone()),
                Some(_) => Err(snap_err(format!("\"{key}\" must be an array"))),
            }
        };
        let mut snap = McpSnapshot {
            protocol_version,
            server_info: obj.get("serverInfo").cloned(),
            ..McpSnapshot::default()
        };
        for (i, t) in list("tools", true)?.iter().enumerate() {
            let o = t
                .as_object()
                .ok_or_else(|| snap_err(format!("tools[{i}] must be an object")))?;
            let name =
                opt_str(o, "name").ok_or_else(|| snap_err(format!("tools[{i}] needs a name")))?;
            let input_schema = o
                .get("inputSchema")
                .cloned()
                .ok_or_else(|| snap_err(format!("tool `{name}` needs an inputSchema")))?;
            if !input_schema.is_object() {
                return Err(snap_err(format!(
                    "tool `{name}` inputSchema must be an object"
                )));
            }
            if snap.tools.iter().any(|x| x.name == name) {
                return Err(snap_err(format!("tool `{name}` is listed twice")));
            }
            snap.tools.push(McpToolSpec {
                title: opt_str(o, "title"),
                description: opt_str(o, "description"),
                output_schema: o.get("outputSchema").cloned(),
                input_schema,
                name,
            });
        }
        for (i, r) in list("resources", false)?.iter().enumerate() {
            let o = r
                .as_object()
                .ok_or_else(|| snap_err(format!("resources[{i}] must be an object")))?;
            snap.resources.push(McpResourceSpec {
                uri: opt_str(o, "uri")
                    .ok_or_else(|| snap_err(format!("resources[{i}] needs a uri")))?,
                name: opt_str(o, "name"),
                description: opt_str(o, "description"),
                mime_type: opt_str(o, "mimeType"),
            });
        }
        for (i, p) in list("prompts", false)?.iter().enumerate() {
            let o = p
                .as_object()
                .ok_or_else(|| snap_err(format!("prompts[{i}] must be an object")))?;
            let arguments = o
                .get("arguments")
                .and_then(Json::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Json::as_object)
                        .filter_map(|x| {
                            Some(McpPromptArg {
                                name: opt_str(x, "name")?,
                                description: opt_str(x, "description"),
                                required: x
                                    .get("required")
                                    .and_then(Json::as_bool)
                                    .unwrap_or(false),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            snap.prompts.push(McpPromptSpec {
                name: opt_str(o, "name")
                    .ok_or_else(|| snap_err(format!("prompts[{i}] needs a name")))?,
                description: opt_str(o, "description"),
                arguments,
            });
        }
        Ok(snap)
    }

    /// The snapshot as JSON in the documented key order.
    pub fn to_json(&self) -> Json {
        let mut out = serde_json::Map::new();
        out.insert("format".into(), json!(SNAPSHOT_FORMAT));
        out.insert("protocolVersion".into(), json!(self.protocol_version));
        if let Some(s) = &self.server_info {
            out.insert("serverInfo".into(), s.clone());
        }
        let tools: Vec<Json> = self
            .tools
            .iter()
            .map(|t| {
                let mut o = serde_json::Map::new();
                o.insert("name".into(), json!(t.name));
                if let Some(x) = &t.title {
                    o.insert("title".into(), json!(x));
                }
                if let Some(x) = &t.description {
                    o.insert("description".into(), json!(x));
                }
                o.insert("inputSchema".into(), t.input_schema.clone());
                if let Some(x) = &t.output_schema {
                    o.insert("outputSchema".into(), x.clone());
                }
                Json::Object(o)
            })
            .collect();
        out.insert("tools".into(), Json::Array(tools));
        let resources: Vec<Json> = self
            .resources
            .iter()
            .map(|r| {
                let mut o = serde_json::Map::new();
                o.insert("uri".into(), json!(r.uri));
                for (k, v) in [
                    ("name", &r.name),
                    ("description", &r.description),
                    ("mimeType", &r.mime_type),
                ] {
                    if let Some(v) = v {
                        o.insert(k.into(), json!(v));
                    }
                }
                Json::Object(o)
            })
            .collect();
        out.insert("resources".into(), Json::Array(resources));
        let prompts: Vec<Json> = self
            .prompts
            .iter()
            .map(|p| {
                let mut o = serde_json::Map::new();
                o.insert("name".into(), json!(p.name));
                if let Some(d) = &p.description {
                    o.insert("description".into(), json!(d));
                }
                let args: Vec<Json> = p
                    .arguments
                    .iter()
                    .map(|a| {
                        let mut x = serde_json::Map::new();
                        x.insert("name".into(), json!(a.name));
                        if let Some(d) = &a.description {
                            x.insert("description".into(), json!(d));
                        }
                        x.insert("required".into(), json!(a.required));
                        Json::Object(x)
                    })
                    .collect();
                o.insert("arguments".into(), Json::Array(args));
                Json::Object(o)
            })
            .collect();
        out.insert("prompts".into(), Json::Array(prompts));
        Json::Object(out)
    }

    /// Exact bytes `connectors sync` writes (pretty JSON + trailing newline).
    pub fn serialize(&self) -> Vec<u8> {
        let mut s = serde_json::to_string_pretty(&self.to_json()).unwrap_or_default();
        s.push('\n');
        s.into_bytes()
    }

    pub fn tool(&self, name: &str) -> Option<&McpToolSpec> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn prompt(&self, name: &str) -> Option<&McpPromptSpec> {
        self.prompts.iter().find(|p| p.name == name)
    }
}

// vhco:domain McpImportKind { tool | resource_read | prompt_get }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpImportKind {
    Tool,
    ResourceRead,
    PromptGet,
}

// vhco:domain McpImport { id: string; connector: string; kind: McpImportKind; name?: string }
/// One imported operation ID (`crm.tools.search`, `crm.resources.read`, `crm.prompts.get`).
#[derive(Clone, Debug, PartialEq)]
pub struct McpImport {
    pub id: String,
    pub connector: String,
    pub kind: McpImportKind,
    /// Original remote tool name (tools only), sent on the wire unchanged.
    pub name: Option<String>,
}

impl McpImport {
    /// The allow_mcp logical target: `crm/tools/search`, `crm/resources/read`,
    /// `crm/prompts/get`. Tools use the alias from the import ID (what source
    /// and the I/O manifest show), which equals the remote name unless the
    /// name needed an alias.
    pub fn target(&self) -> String {
        match self.kind {
            McpImportKind::Tool => {
                let prefix = format!("{}.tools.", self.connector);
                format!(
                    "{}/tools/{}",
                    self.connector,
                    self.id.strip_prefix(&prefix).unwrap_or(&self.id)
                )
            }
            McpImportKind::ResourceRead => format!("{}/resources/read", self.connector),
            McpImportKind::PromptGet => format!("{}/prompts/get", self.connector),
        }
    }

    /// The MCP method this import sends.
    pub fn rpc_method(&self) -> &'static str {
        match self.kind {
            McpImportKind::Tool => "tools/call",
            McpImportKind::ResourceRead => "resources/read",
            McpImportKind::PromptGet => "prompts/get",
        }
    }

    /// The server capability the method needs (`tools`, `resources`, `prompts`).
    pub fn capability(&self) -> &'static str {
        match self.kind {
            McpImportKind::Tool => "tools",
            McpImportKind::ResourceRead => "resources",
            McpImportKind::PromptGet => "prompts",
        }
    }
}

/// Deterministic alias of a remote tool name for its import ID: every
/// character outside `[A-Za-z0-9_.]` becomes `_`, empty dot segments become `_`.
/// Collisions are detected by the loader, never silently merged.
pub fn tool_alias(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    mapped
        .split('.')
        .map(|s| if s.is_empty() { "_" } else { s })
        .collect::<Vec<_>>()
        .join(".")
}

// vhco:domain McpConnectorInfo { name: string; transport: McpTransport; tls: McpTls; auth?: (string, string); schema_path?: string; schema_hash?: string; snapshot?: McpSnapshot; expose_tools: string[]; expose_resources: string[]; expose_prompts: string[]; span: SourceSpan }
#[derive(Clone, Debug, PartialEq)]
pub struct McpConnectorInfo {
    pub name: String,
    pub transport: McpTransport,
    pub tls: McpTls,
    /// `auth PROFILE account "A"` (OAuth; unsupported.auth until it exists).
    pub auth: Option<(String, String)>,
    pub schema_path: Option<String>,
    /// `sha256:<hex>` of the reviewed snapshot file bytes.
    pub schema_hash: Option<String>,
    /// `None` only in discovery mode (`connectors sync` before a snapshot exists).
    pub snapshot: Option<McpSnapshot>,
    pub expose_tools: Vec<String>,
    pub expose_resources: Vec<String>,
    pub expose_prompts: Vec<String>,
    pub span: SourceSpan,
}

// vhco:domain McpCatalog { connectors: McpConnectorInfo[]; imports: McpImport[] }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct McpCatalog {
    pub connectors: Vec<McpConnectorInfo>,
    pub imports: Vec<McpImport>,
}

impl McpCatalog {
    pub fn connector(&self, name: &str) -> Option<&McpConnectorInfo> {
        self.connectors.iter().find(|c| c.name == name)
    }

    pub fn import(&self, id: &str) -> Option<&McpImport> {
        self.imports.iter().find(|i| i.id == id)
    }

    /// One catalog entry per imported operation, with its reviewed snapshot
    /// schemas: a tool keeps its `inputSchema` verbatim and its result schema
    /// wraps the snapshot `outputSchema` as `structuredContent`; resources and
    /// prompts accept only the exposed URIs / names. `list`, `describe`,
    /// `outputs`, `/v1/operations` and MCP `tools/list` show these entries.
    pub fn import_entries(&self) -> Vec<super::contracts::RegistryEntry> {
        use super::contracts::RegistryEntry;
        use super::ir::OperationKind;
        use super::outputs::{OutputSpec, ParamSpec, ValueSpec};
        let mut out = Vec::new();
        for i in &self.imports {
            let Some(conn) = self.connector(&i.connector) else {
                continue;
            };
            let snapshot = conn.snapshot.as_ref();
            let (name, description, input, output) = match i.kind {
                McpImportKind::Tool => {
                    let tool = snapshot.and_then(|s| s.tool(i.name.as_deref().unwrap_or("")));
                    let structured = tool
                        .and_then(|t| t.output_schema.clone())
                        .unwrap_or_else(|| json!({}));
                    (
                        tool.and_then(|t| t.title.clone())
                            .or_else(|| i.name.clone())
                            .unwrap_or_else(|| i.id.clone()),
                        tool.and_then(|t| t.description.clone()),
                        tool.map(|t| t.input_schema.clone())
                            .unwrap_or_else(|| json!({"type": "object"})),
                        json!({"type": "object",
                               "description": "MCP tool result: content blocks and structuredContent, unchanged.",
                               "properties": {"content": {"type": "array"}, "structuredContent": structured, "isError": {"type": "boolean"}},
                               "required": ["content", "isError"], "additionalProperties": true}),
                    )
                }
                McpImportKind::ResourceRead => (
                    format!("Read a {} resource", conn.name),
                    Some(format!(
                        "Read one resource exposed by MCP connector `{}`.",
                        conn.name
                    )),
                    json!({"type": "object",
                           "properties": {"uri": {"type": "string", "enum": conn.expose_resources, "description": "An exposed resource URI."}},
                           "required": ["uri"], "additionalProperties": false}),
                    json!({"type": "object", "description": "MCP resources/read result.",
                           "properties": {"contents": {"type": "array"}},
                           "required": ["contents"], "additionalProperties": true}),
                ),
                McpImportKind::PromptGet => (
                    format!("Get a {} prompt", conn.name),
                    Some(format!(
                        "Render one prompt exposed by MCP connector `{}`.",
                        conn.name
                    )),
                    json!({"type": "object",
                           "properties": {"name": {"type": "string", "enum": conn.expose_prompts, "description": "An exposed prompt name."},
                                          "arguments": {"type": "object", "additionalProperties": {"type": "string"}, "description": "Prompt arguments (text)."}},
                           "required": ["name"], "additionalProperties": false}),
                    json!({"type": "object", "description": "MCP prompts/get result.",
                           "properties": {"messages": {"type": "array"}},
                           "required": ["messages"], "additionalProperties": true}),
                ),
            };
            out.push(RegistryEntry {
                id: i.id.clone(),
                name,
                description,
                kind: OperationKind::Operation,
                private: false,
                params: ParamSpec::list_from_schema(&input),
                output: OutputSpec {
                    spec: ValueSpec::from_json_schema(&output),
                    description: output
                        .get("description")
                        .and_then(Json::as_str)
                        .map(str::to_string),
                },
                emits: None,
                receives: None,
                emits_description: None,
                receives_description: None,
                errors: Vec::new(),
                source: conn.span.clone(),
                raw_input_schema: Some(input),
                raw_output_schema: Some(output),
            });
        }
        out
    }

    /// True when `id` is under an MCP connector's namespace (`crm.…`).
    pub fn owns(&self, id: &str) -> bool {
        id.split_once('.')
            .is_some_and(|(c, _)| self.connector(c).is_some())
    }
}

// vhco:domain BridgeHops { hops: int; chain: string[] }
/// Bridge recursion state carried in `_meta` across MCP hops.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BridgeHops {
    pub hops: u32,
    pub chain: Vec<String>,
}

impl BridgeHops {
    /// Read `_meta` {"rivet/hops", "rivet/chain"} from tools/call params.
    pub fn from_meta(params: &Json) -> BridgeHops {
        let meta = params.get("_meta");
        BridgeHops {
            hops: meta
                .and_then(|m| m.get(META_HOPS))
                .and_then(Json::as_u64)
                .unwrap_or(0)
                .min(u32::MAX as u64) as u32,
            chain: meta
                .and_then(|m| m.get(META_CHAIN))
                .and_then(Json::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Json::as_str)
                        .take(64)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

// vhco:domain McpContext { operation_id: string; request_id: string; deadline_ms: int; bridge: BridgeHops; identity: string; span?: SourceSpan; principal?: Principal; bearer?: CredentialLease }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct McpContext {
    /// The calling operation (or the import ID for a surface call).
    pub operation_id: String,
    pub request_id: String,
    pub deadline_ms: u64,
    pub bridge: BridgeHops,
    /// This host's identity in bridge chains (`rivet:<bundle hash prefix>`).
    pub identity: String,
    pub span: Option<SourceSpan>,
    /// The caller (credential cache identity for `auth PROFILE account A`).
    pub principal: Option<super::contracts::Principal>,
    /// An origin-bound OAuth lease acquired by `connectors.invoke_mcp`; the
    /// HTTP transport attaches it as `Authorization: Bearer` (never logged).
    pub bearer: Option<super::auth::CredentialLease>,
}

// vhco:domain McpRequest { connector: string; method: string; params: Value; schema_hash: string; context: McpContext }
/// `method` is the import suffix (`tools.search`, `resources.read`,
/// `prompts.get`) or `discover` for `connectors sync`.
#[derive(Clone, Debug, PartialEq)]
pub struct McpRequest {
    pub connector: String,
    pub method: String,
    pub params: Value,
    pub schema_hash: String,
    pub context: McpContext,
}

// vhco:domain McpResult { content: Value[]; structured_content?: Value; metadata: Value }
/// Tools: content blocks + structuredContent. Resources: `contents` in
/// `content`. Prompts: `messages` in `content`, description in metadata.
/// Discovery: the candidate snapshot in `structured_content`.
#[derive(Clone, Debug, PartialEq)]
pub struct McpResult {
    pub content: Vec<Value>,
    pub structured_content: Option<Value>,
    pub metadata: Value,
}

impl McpResult {
    /// The value scripts and surfaces receive, preserving the MCP shape.
    pub fn to_value(&self, kind: McpImportKind) -> Value {
        match kind {
            McpImportKind::Tool => {
                let mut v = Value::object([("content", Value::List(self.content.clone()))]);
                if let Some(s) = &self.structured_content {
                    v.set("structuredContent", s.clone());
                }
                v.set("isError", Value::Bool(false));
                if let Some(m) = self.metadata.get("_meta") {
                    v.set("_meta", m.clone());
                }
                v
            }
            McpImportKind::ResourceRead => {
                Value::object([("contents", Value::List(self.content.clone()))])
            }
            McpImportKind::PromptGet => {
                let mut v = Value::Object(Vec::new());
                if let Some(d) = self.metadata.get("description") {
                    v.set("description", d.clone());
                }
                v.set("messages", Value::List(self.content.clone()));
                v
            }
        }
    }
}

// vhco:domain McpPeerInfo { protocol_version: string; capabilities: Json; server_info: Json }
/// What the server negotiated in `initialize`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct McpPeerInfo {
    pub protocol_version: String,
    pub capabilities: Json,
    pub server_info: Json,
}

impl McpPeerInfo {
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities
            .get(capability)
            .is_some_and(|c| !c.is_null())
    }
}

// vhco:domain McpReply { result: Json | error: {code: int; message: string; data?: Json} }
/// One correlated JSON-RPC response (notifications and server requests are
/// handled by the session and never surface here).
#[derive(Clone, Debug, PartialEq)]
pub enum McpReply {
    Result(Json),
    Error {
        code: i64,
        message: String,
        data: Option<Json>,
    },
}

/// Server→client requests the host never implements (REF S61).
pub fn declined_server_request(method: &str) -> bool {
    matches!(
        method,
        "sampling/createMessage" | "elicitation/create" | "roots/list"
    )
}

/// The typed error for a declined sampling/elicitation/roots request.
pub fn unsupported_capability(method: &str) -> RivetError {
    RivetError::new(
        ErrorKind::Protocol,
        "protocol.unsupported_capability",
        format!("the MCP server requested `{method}`, which this host does not enable"),
    )
    .with_details(Value::object([("method", Value::text(method))]))
}

/// Minimal JSON Schema check (type, enum, const, properties, required,
/// additionalProperties, items, min/max, minLength/maxLength, anyOf/oneOf/allOf).
/// Unknown keywords are ignored. Violations are appended as `path: problem`.
pub fn check_schema(schema: &Json, value: &Json, path: &str, out: &mut Vec<String>) {
    let Some(s) = schema.as_object() else {
        if schema == &Json::Bool(false) {
            out.push(format!("{path}: no value is allowed here"));
        }
        return;
    };
    if let Some(t) = s.get("type") {
        let types: Vec<&str> = match t {
            Json::String(x) => vec![x.as_str()],
            Json::Array(a) => a.iter().filter_map(Json::as_str).collect(),
            _ => vec![],
        };
        if !types.is_empty() && !types.iter().any(|ty| type_matches(ty, value)) {
            out.push(format!(
                "{path}: expected {}, got {}",
                types.join(" or "),
                json_type(value)
            ));
            return;
        }
    }
    if let Some(Json::Array(options)) = s.get("enum")
        && !options.contains(value)
    {
        out.push(format!(
            "{path}: must be one of {}",
            Json::Array(options.clone())
        ));
    }
    if let Some(c) = s.get("const")
        && c != value
    {
        out.push(format!("{path}: must equal {c}"));
    }
    if let Some(n) = value.as_f64() {
        if let Some(min) = s.get("minimum").and_then(Json::as_f64)
            && n < min
        {
            out.push(format!("{path}: must be ≥ {min}"));
        }
        if let Some(max) = s.get("maximum").and_then(Json::as_f64)
            && n > max
        {
            out.push(format!("{path}: must be ≤ {max}"));
        }
    }
    if let Some(text) = value.as_str() {
        let len = text.chars().count() as u64;
        if let Some(min) = s.get("minLength").and_then(Json::as_u64)
            && len < min
        {
            out.push(format!("{path}: shorter than {min} characters"));
        }
        if let Some(max) = s.get("maxLength").and_then(Json::as_u64)
            && len > max
        {
            out.push(format!("{path}: longer than {max} characters"));
        }
    }
    if let Some(obj) = value.as_object() {
        let props = s.get("properties").and_then(Json::as_object);
        if let Some(Json::Array(req)) = s.get("required") {
            for r in req.iter().filter_map(Json::as_str) {
                if !obj.contains_key(r) {
                    out.push(format!("{path}.{r}: required"));
                }
            }
        }
        for (k, v) in obj {
            let child = format!("{path}.{k}");
            match props.and_then(|p| p.get(k)) {
                Some(ps) => check_schema(ps, v, &child, out),
                None => match s.get("additionalProperties") {
                    Some(Json::Bool(false)) => out.push(format!("{child}: unknown field")),
                    Some(extra @ Json::Object(_)) => check_schema(extra, v, &child, out),
                    _ => {}
                },
            }
        }
    }
    if let (Some(items), Some(arr)) = (s.get("items"), value.as_array()) {
        for (i, v) in arr.iter().enumerate() {
            check_schema(items, v, &format!("{path}[{i}]"), out);
        }
    }
    if let Some(Json::Array(all)) = s.get("allOf") {
        for sub in all {
            check_schema(sub, value, path, out);
        }
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(Json::Array(alts)) = s.get(key) {
            let ok = alts.iter().any(|sub| {
                let mut v = Vec::new();
                check_schema(sub, value, path, &mut v);
                v.is_empty()
            });
            if !ok {
                out.push(format!("{path}: matches none of the {key} alternatives"));
            }
        }
    }
}

fn type_matches(ty: &str, v: &Json) -> bool {
    match ty {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        "number" => v.is_number(),
        "integer" => {
            v.as_i64().is_some()
                || v.as_u64().is_some()
                || v.as_f64().is_some_and(|f| f.fract() == 0.0)
        }
        _ => true,
    }
}

fn json_type(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

/// `sha256:<hex>` of raw bytes (the snapshot review identity).
pub fn snapshot_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAP: &str = r#"{"protocolVersion":"2025-11-25","tools":[{"name":"search","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}],"prompts":[{"name":"p","arguments":[{"name":"who","required":true}]}]}"#;

    // vhco:test connectors.invoke_mcp -- the snapshot file format parses strictly and re-serializes to an equal snapshot
    #[test]
    fn snapshot_round_trips_and_rejects_unknown_keys() {
        let s = McpSnapshot::parse(SNAP.as_bytes()).unwrap();
        assert_eq!(s.tools[0].name, "search");
        assert!(s.prompts[0].arguments[0].required);
        let again = McpSnapshot::parse(&s.serialize()).unwrap();
        assert_eq!(again, s);
        assert_eq!(
            McpSnapshot::parse(br#"{"protocolVersion":"x","tools":[],"extra":1}"#)
                .unwrap_err()
                .code,
            "mcp.snapshot"
        );
        assert!(McpSnapshot::parse(br#"{"tools":[]}"#).is_err());
    }

    // vhco:test connectors.invoke_mcp -- params are checked against a tool inputSchema with path-named violations
    #[test]
    fn schema_check_reports_paths() {
        let s = McpSnapshot::parse(SNAP.as_bytes()).unwrap();
        let schema = &s.tools[0].input_schema;
        let mut v = Vec::new();
        check_schema(schema, &json!({"query": "Ada"}), "params", &mut v);
        assert!(v.is_empty());
        check_schema(schema, &json!({"query": 3, "x": 1}), "params", &mut v);
        assert_eq!(v.len(), 2, "{v:?}");
        let mut v = Vec::new();
        check_schema(schema, &json!({}), "params", &mut v);
        assert_eq!(v, vec!["params.query: required"]);
    }

    // vhco:test connectors.invoke_mcp -- tool aliases are deterministic and bridge hop metadata parses from _meta
    #[test]
    fn aliases_are_deterministic_and_bridge_meta_parses() {
        assert_eq!(tool_alias("search"), "search");
        assert_eq!(tool_alias("create-contact"), "create_contact");
        assert_eq!(tool_alias("a..b"), "a._.b");
        let b = BridgeHops::from_meta(&json!({"_meta": {"rivet/hops": 3, "rivet/chain": ["x"]}}));
        assert_eq!(b.hops, 3);
        assert_eq!(b.chain, vec!["x"]);
        assert_eq!(BridgeHops::from_meta(&json!({})), BridgeHops::default());
    }
}
