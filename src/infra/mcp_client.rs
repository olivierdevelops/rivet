//! MCP client adapter (PROP-2026-0001 Increment 6): satisfies `McpClient` for
//! `connector NAME mcp` declarations, protocol 2025-11-25, over stdio and
//! Streamable HTTP.
//!
//! ```text
//!  bundle load ─▶ McpPeer::load: parse options, read `schema` (bootstrap), sha256 ∈ approved.snapshots,
//!                 exposed names ⊆ snapshot, imports + alias collisions, literal calls resolve
//!
//!  open(connector) ─┬─ http    ─▶ HttpLink: every POST through the injected exchange_http (allow_network,
//!                   │                        private-range check, TLS options read through policed files)
//!                   └─ command ─▶ StdioLink: injected spawn (allow_exec + OS sandbox) with piped stdin/stdout
//!                   ▼
//!  Session: initialize ─▶ notifications/initialized ─▶ request(id) … wait for response(id)
//!            server ping → {}   sampling/elicitation/roots → -32601 + protocol.unsupported_capability
//!            notifications/progress|message → ignored (never data)
//!            drop with a pending id → notifications/cancelled, then stdin EOF + reap / HTTP DELETE
//! ```

use super::codec::StreamDecoder;
use super::effect_args::read_file;
use crate::domain::ErrorKind;
use crate::domain::ir::{Arg, CompiledProgram, Declaration, Expr, OptionLine};
use crate::domain::mcp::{
    MCP_CLIENT_VERSION, MCP_COMPATIBLE_VERSIONS, McpCatalog, McpConnectorInfo, McpContext,
    McpImport, McpImportKind, McpPeerInfo, McpReply, McpSnapshot, McpTls, McpTransport,
    declined_server_request, snapshot_hash, tool_alias, unsupported_capability,
};
use crate::domain::ports::{FileAccess, McpClient, McpSession, PolicyEvaluator};
use crate::domain::transports::{
    ByteSink, ByteStream, ChildDuplex, CodecInput, CodecKind, EffectOrigin, HttpExchange,
    HttpOutcome, HttpVersionPolicy, ProcessPlan, StreamMode, TlsMaterial,
};
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use serde_json::{Value as Json, json};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

/// Largest single MCP message (a JSON body, SSE event or stdio line).
const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// The injected `transports.exchange_http` use case (brokered HTTP client).
pub type McpHttpFn = dyn Fn(HttpExchange, Arc<dyn PolicyEvaluator>) -> BoxFuture<'static, RivetResult<HttpOutcome>>
    + Send
    + Sync;
/// The injected process path: `confine_process` (allow_exec + sandbox) then a duplex spawn.
pub type McpSpawnFn = dyn Fn(ProcessPlan, Arc<dyn PolicyEvaluator>) -> BoxFuture<'static, RivetResult<ChildDuplex>>
    + Send
    + Sync;

// vhco:infra mcp_client satisfies McpClient
// vhco:file read schema -- connector `schema` snapshot, read once at bundle load (bootstrap); its sha256 must be in policy.json approved.snapshots
// vhco:file read tls -- connector `tls ca_file|cert_file|key_file`, read through the policed FileAccess (allow_read) before connecting
// vhco:net connect mcp -- Streamable HTTP POST/DELETE through the brokered exchange_http (allow_network per attempt, private-range check)
// vhco:exec mcp -- `transport command BIN` stdio children through the process runner (allow_exec, OS sandbox required when policy.json is present)
pub struct McpPeer {
    catalog: McpCatalog,
    root: String,
    evaluator: Arc<dyn PolicyEvaluator>,
    http: Arc<McpHttpFn>,
    spawn: Arc<McpSpawnFn>,
    files: Arc<dyn FileAccess>,
}

fn decl_err(decl: &Declaration, code: &str, msg: impl Into<String>) -> RivetError {
    RivetError::validation(code, msg).with_span(Some(decl.span.clone()))
}

fn const_text(a: Option<&Arg>) -> Option<String> {
    a.and_then(|a| a.to_expr().const_text())
}

fn const_list(a: Option<&Arg>) -> Option<Vec<String>> {
    match a.map(Arg::to_expr) {
        Some(Expr::List(items)) => items.iter().map(Expr::const_text).collect(),
        _ => None,
    }
}

fn const_object(a: Option<&Arg>) -> Option<Vec<(String, String)>> {
    match a.map(Arg::to_expr) {
        Some(Expr::Object(pairs)) => pairs
            .iter()
            .map(|(k, v)| v.const_text().map(|t| (k.clone(), t)))
            .collect(),
        _ => None,
    }
}

impl McpPeer {
    pub fn new(
        catalog: McpCatalog,
        root: &str,
        evaluator: Arc<dyn PolicyEvaluator>,
        http: Arc<McpHttpFn>,
        spawn: Arc<McpSpawnFn>,
        files: Arc<dyn FileAccess>,
    ) -> McpPeer {
        McpPeer {
            catalog,
            root: root.to_string(),
            evaluator,
            http,
            spawn,
            files,
        }
    }

    /// Bootstrap: build the catalog of every `connector NAME mcp`. With
    /// `discovery` (only `connectors sync`) a missing or unreviewed snapshot is
    /// tolerated and that connector imports nothing.
    pub fn load(
        program: &CompiledProgram,
        root: &str,
        approved: &[String],
        discovery: bool,
    ) -> RivetResult<McpCatalog> {
        let mut catalog = McpCatalog::default();
        for decl in program.connectors.iter().filter(|c| c.kind == "mcp") {
            let info = connector_info(decl, root, approved, discovery)?;
            if let Some(snapshot) = &info.snapshot {
                add_imports(&mut catalog, program, decl, &info, snapshot)?;
            }
            catalog.connectors.push(info);
        }
        if let Some(c) = program
            .connectors
            .iter()
            .find(|c| c.kind != "mcp" && c.kind != "grpc")
        {
            return Err(decl_err(
                c,
                "mcp.connector",
                format!("connector kind `{}` is not supported (mcp, grpc)", c.kind),
            ));
        }
        if !discovery {
            // Literal (request "crm.…") targets must name a real import.
            for op in &program.operations {
                for call in &op.calls {
                    if catalog.owns(call) && catalog.import(call).is_none() {
                        return Err(RivetError::validation(
                            "mcp.unknown_import",
                            format!(
                                "`{}` calls `{call}`, which is not an exposed import of the reviewed snapshot",
                                op.id
                            ),
                        )
                        .with_span(Some(op.span.clone())));
                    }
                }
            }
        }
        Ok(catalog)
    }

    fn resolve(&self, program: &str) -> String {
        if Path::new(program).is_absolute() {
            program.to_string()
        } else {
            Path::new(&self.root)
                .join(program)
                .to_string_lossy()
                .into_owned()
        }
    }

    async fn tls_material(&self, tls: &McpTls) -> RivetResult<TlsMaterial> {
        let mut m = TlsMaterial {
            server_name: tls.server_name.clone(),
            ..TlsMaterial::default()
        };
        if let Some(p) = &tls.ca_file {
            m.ca_pem = Some(read_file(self.files.as_ref(), p).await?);
        }
        if let Some(p) = &tls.cert_file {
            m.cert_pem = Some(read_file(self.files.as_ref(), p).await?);
        }
        if let Some(p) = &tls.key_file {
            m.key_pem = Some(read_file(self.files.as_ref(), p).await?);
        }
        Ok(m)
    }
}

fn connector_info(
    decl: &Declaration,
    root: &str,
    approved: &[String],
    discovery: bool,
) -> RivetResult<McpConnectorInfo> {
    if decl.name == "rivet" {
        return Err(decl_err(
            decl,
            "mcp.connector",
            "`rivet` is reserved for built-in operations; rename the connector",
        ));
    }
    let mut transport = None;
    let mut info = McpConnectorInfo {
        name: decl.name.clone(),
        transport: McpTransport::Http { url: String::new() },
        tls: McpTls::default(),
        auth: None,
        schema_path: None,
        schema_hash: None,
        snapshot: None,
        expose_tools: Vec::new(),
        expose_resources: Vec::new(),
        expose_prompts: Vec::new(),
        span: decl.span.clone(),
    };
    // `transport command BIN` + indented `args`/`env` lines: the grammar has no
    // option blocks, so those lines arrive as siblings right after `transport`.
    let mut options: Vec<OptionLine> = Vec::new();
    for o in &decl.options {
        let attach = matches!(o.key.as_str(), "args" | "env")
            && options
                .last()
                .is_some_and(|t| t.key == "transport" && t.first_word() != Some("http"));
        match options.last_mut() {
            Some(t) if attach => t.children.push(o.clone()),
            _ => options.push(o.clone()),
        }
    }
    for o in &options {
        let bad =
            |m: String| RivetError::validation("mcp.connector", m).with_span(Some(o.span.clone()));
        match o.key.as_str() {
            "transport" => transport = Some(parse_transport(o, &decl.name).map_err(bad)?),
            "schema" => {
                info.schema_path = Some(
                    const_text(o.args.first())
                        .ok_or_else(|| bad("`schema` needs a constant path string".into()))?,
                )
            }
            "expose" => {
                let what = o.first_word().unwrap_or("");
                let list = const_list(o.args.get(1)).ok_or_else(|| {
                    bad(format!(
                        "expected `expose {what} [\"name\", …]` with constant strings"
                    ))
                })?;
                match what {
                    "tools" => info.expose_tools.extend(list),
                    "resources" => info.expose_resources.extend(list),
                    "prompts" => info.expose_prompts.extend(list),
                    other => {
                        return Err(bad(format!(
                            "`expose {other}`: expected tools, resources or prompts"
                        )));
                    }
                }
            }
            "auth" => {
                let profile = o.first_word().unwrap_or("").to_string();
                let account = const_text(o.args.get(2)).unwrap_or_default();
                if profile.is_empty() || o.args.get(1).and_then(Arg::word) != Some("account") {
                    return Err(bad("expected `auth PROFILE account \"ACCOUNT\"`".into()));
                }
                info.auth = Some((profile, account));
            }
            "tls" => {
                let which = o.first_word().unwrap_or("");
                let value = const_text(o.args.get(1))
                    .ok_or_else(|| bad(format!("`tls {which}` needs a constant string")))?;
                match which {
                    "server_name" => info.tls.server_name = Some(value),
                    "ca_file" => info.tls.ca_file = Some(value),
                    "cert_file" => info.tls.cert_file = Some(value),
                    "key_file" => info.tls.key_file = Some(value),
                    other => {
                        return Err(bad(format!(
                            "`tls {other}`: expected server_name, ca_file, cert_file or key_file"
                        )));
                    }
                }
            }
            other => {
                return Err(bad(format!(
                    "option `{other}` is not valid on an mcp connector (transport, schema, expose, auth, tls)"
                )));
            }
        }
    }
    info.transport = transport.ok_or_else(|| {
        decl_err(
            decl,
            "mcp.connector",
            format!(
                "connector `{}` needs `transport http URL` or `transport command BIN`",
                decl.name
            ),
        )
    })?;
    if matches!(info.transport, McpTransport::Command { .. }) && info.tls != McpTls::default() {
        return Err(decl_err(
            decl,
            "mcp.connector",
            "`tls` options apply only to `transport http`",
        ));
    }
    let Some(path) = info.schema_path.clone() else {
        if discovery {
            return Ok(info);
        }
        return Err(decl_err(
            decl,
            "mcp.connector",
            format!(
                "connector `{}` needs `schema PATH` (a reviewed snapshot; create one with `rivet connectors sync {}`)",
                decl.name, decl.name
            ),
        ));
    };
    let bytes = match std::fs::read(Path::new(root).join(&path)) {
        Ok(b) => b,
        Err(_) if discovery => return Ok(info),
        Err(e) => {
            let kind = if e.kind() == std::io::ErrorKind::NotFound {
                ErrorKind::NotFound
            } else {
                ErrorKind::Validation
            };
            return Err(RivetError::new(
                kind,
                if kind == ErrorKind::NotFound {
                    "not_found.mcp_snapshot"
                } else {
                    "mcp.snapshot"
                },
                format!("cannot read snapshot {path} of connector `{}`: {e}", decl.name),
            )
            .with_span(Some(decl.span.clone()))
            .with_hint(format!(
                "create it with `rivet connectors sync {} --output {path}`, review it and approve its sha256 in policy.json",
                decl.name
            )));
        }
    };
    let hash = snapshot_hash(&bytes);
    if !approved.iter().any(|a| a == &hash) {
        if discovery {
            return Ok(info);
        }
        return Err(decl_err(
            decl,
            "mcp.snapshot_unapproved",
            format!(
                "snapshot {path} of connector `{}` is not reviewed: {hash} is not listed in policy.json approved.snapshots",
                decl.name
            ),
        )
        .with_details(Value::object([
            ("path", Value::text(&path)),
            ("sha256", Value::text(&hash)),
        ]))
        .with_hint(format!(
            "after reviewing {path}, add \"{hash}\" to policy.json \"approved\": {{\"snapshots\": [...]}}"
        )));
    }
    let snapshot = McpSnapshot::parse(&bytes).map_err(|e| e.with_span(Some(decl.span.clone())))?;
    info.schema_hash = Some(hash);
    info.snapshot = Some(snapshot);
    Ok(info)
}

fn parse_transport(o: &OptionLine, name: &str) -> Result<McpTransport, String> {
    match o.first_word() {
        Some("http") => {
            let url = const_text(o.args.get(1))
                .ok_or_else(|| "expected `transport http \"https://host/mcp\"`".to_string())?;
            let parsed = url::Url::parse(&url).map_err(|e| format!("invalid URL `{url}`: {e}"))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(format!(
                    "connector `{name}`: MCP over HTTP needs an http(s) URL"
                ));
            }
            if !o.children.is_empty() {
                return Err("`transport http` takes no block".into());
            }
            Ok(McpTransport::Http { url })
        }
        Some("command") | Some("stdio") => {
            let program = const_text(o.args.get(1)).ok_or_else(|| {
                "expected `transport command \"/path/to/server\"` (a constant path)".to_string()
            })?;
            let mut args = Vec::new();
            let mut env = Vec::new();
            for c in &o.children {
                match c.key.as_str() {
                    "args" => {
                        args = const_list(c.args.first())
                            .ok_or_else(|| "`args` needs a list of constant strings".to_string())?
                    }
                    "env" => {
                        env = const_object(c.args.first()).ok_or_else(|| {
                            "`env` needs an object of constant strings".to_string()
                        })?
                    }
                    other => {
                        return Err(format!(
                            "`{other}` is not valid under `transport command` (args, env)"
                        ));
                    }
                }
            }
            Ok(McpTransport::Command { program, args, env })
        }
        _ => Err("expected `transport http URL` or `transport command BIN`".into()),
    }
}

fn add_imports(
    catalog: &mut McpCatalog,
    program: &CompiledProgram,
    decl: &Declaration,
    info: &McpConnectorInfo,
    snapshot: &McpSnapshot,
) -> RivetResult<()> {
    let push = |catalog: &mut McpCatalog, import: McpImport| -> RivetResult<()> {
        if program.operation(&import.id).is_some() || catalog.import(&import.id).is_some() {
            return Err(decl_err(
                decl,
                "mcp.alias_collision",
                format!(
                    "imported ID `{}` collides with an existing operation or import",
                    import.id
                ),
            ));
        }
        catalog.imports.push(import);
        Ok(())
    };
    for name in &info.expose_tools {
        // an exposed tool missing from the reviewed snapshot fails the load
        if snapshot.tool(name).is_none() {
            return Err(decl_err(
                decl,
                "mcp.unknown_tool",
                format!(
                    "connector `{}` exposes tool `{name}`, which is not in its reviewed snapshot",
                    info.name
                ),
            ));
        }
        push(
            catalog,
            McpImport {
                id: format!("{}.tools.{}", info.name, tool_alias(name)),
                connector: info.name.clone(),
                kind: McpImportKind::Tool,
                name: Some(name.clone()),
            },
        )?;
    }
    if !info.expose_resources.is_empty() {
        for uri in &info.expose_resources {
            if !snapshot.resources.iter().any(|r| &r.uri == uri) {
                return Err(decl_err(
                    decl,
                    "mcp.unknown_resource",
                    format!(
                        "connector `{}` exposes resource `{uri}`, which is not in its reviewed snapshot",
                        info.name
                    ),
                ));
            }
        }
        push(
            catalog,
            McpImport {
                id: format!("{}.resources.read", info.name),
                connector: info.name.clone(),
                kind: McpImportKind::ResourceRead,
                name: None,
            },
        )?;
    }
    if !info.expose_prompts.is_empty() {
        for p in &info.expose_prompts {
            if snapshot.prompt(p).is_none() {
                return Err(decl_err(
                    decl,
                    "mcp.unknown_prompt",
                    format!(
                        "connector `{}` exposes prompt `{p}`, which is not in its reviewed snapshot",
                        info.name
                    ),
                ));
            }
        }
        push(
            catalog,
            McpImport {
                id: format!("{}.prompts.get", info.name),
                connector: info.name.clone(),
                kind: McpImportKind::PromptGet,
                name: None,
            },
        )?;
    }
    Ok(())
}

#[async_trait]
impl McpClient for McpPeer {
    fn catalog(&self) -> &McpCatalog {
        &self.catalog
    }

    async fn open(
        &self,
        connector: &McpConnectorInfo,
        context: &McpContext,
    ) -> RivetResult<Box<dyn McpSession>> {
        let origin = EffectOrigin {
            operation_id: context.operation_id.clone(),
            span: context.span.clone(),
        };
        let link: Box<dyn Link> = match &connector.transport {
            McpTransport::Http { url } => Box::new(HttpLink {
                url: url.clone(),
                http: Arc::clone(&self.http),
                evaluator: Arc::clone(&self.evaluator),
                origin,
                tls: self.tls_material(&connector.tls).await?,
                session_id: None,
                protocol: None,
                inbox: None,
            }),
            McpTransport::Command { program, args, env } => {
                let plan = ProcessPlan {
                    program: program.clone(),
                    resolved: self.resolve(program),
                    args: args.clone(),
                    env: env.clone(),
                    cwd: None,
                    stdin: None,
                    timeout_ms: context.deadline_ms.max(1),
                    accept_exit: Vec::new(),
                    decode_stdout: None,
                    decode_stderr: None,
                    stream: None,
                    sandbox: None,
                    max_output: MAX_MESSAGE as u64,
                    interactive: false,
                    origin,
                };
                let child = (self.spawn)(plan, Arc::clone(&self.evaluator)).await?;
                Box::new(StdioLink {
                    stdin: Some(child.stdin),
                    stdout: Some(child.stdout),
                    buf: Vec::new(),
                })
            }
        };
        let mut session = Session {
            link: Some(link),
            peer: McpPeerInfo::default(),
            next_id: 0,
            pending: None,
        };
        match session.initialize().await {
            Ok(()) => Ok(Box::new(session)),
            Err(e) => {
                session.pending = None;
                Box::new(session).close().await;
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------- links

/// One MCP transport: sends messages and yields incoming ones in order.
#[async_trait]
trait Link: Send {
    /// Send one message; `reply_expected` = this is a request whose response
    /// (and any interleaved server messages) should be read next.
    async fn send(&mut self, msg: &Json, reply_expected: bool) -> RivetResult<()>;
    /// Next incoming message; `Ok(None)` when the stream ended.
    async fn recv(&mut self) -> RivetResult<Option<Json>>;
    /// After initialize: the negotiated protocol version (HTTP header).
    fn negotiated(&mut self, _version: &str) {}
    /// Release the transport (stdin EOF + reap, or HTTP DELETE).
    async fn finish(self: Box<Self>);
}

fn protocol_err(code: &str, msg: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Protocol, code, msg)
}

fn parse_message(bytes: &[u8]) -> RivetResult<Json> {
    let j: Json = serde_json::from_slice(bytes).map_err(|e| {
        protocol_err(
            "protocol.mcp_message",
            format!("MCP peer sent a non-JSON message: {e}"),
        )
    })?;
    if !j.is_object() {
        return Err(protocol_err(
            "protocol.mcp_message",
            "MCP peer sent a JSON-RPC batch or non-object message",
        ));
    }
    Ok(j)
}

struct StdioLink {
    stdin: Option<Box<dyn ByteSink>>,
    stdout: Option<Box<dyn ByteStream>>,
    buf: Vec<u8>,
}

#[async_trait]
impl Link for StdioLink {
    async fn send(&mut self, msg: &Json, _reply_expected: bool) -> RivetResult<()> {
        let mut line = msg.to_string().into_bytes();
        line.push(b'\n');
        match self.stdin.as_mut() {
            Some(s) => s.write(&line).await.map_err(|e| {
                RivetError::new(
                    ErrorKind::Connection,
                    "connection.mcp_closed",
                    format!("MCP stdio server stopped reading: {}", e.message),
                )
            }),
            None => Err(RivetError::new(
                ErrorKind::Connection,
                "connection.mcp_closed",
                "MCP stdio session is closed",
            )),
        }
    }

    async fn recv(&mut self) -> RivetResult<Option<Json>> {
        loop {
            if let Some(i) = self.buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=i).collect();
                let text = String::from_utf8_lossy(&line);
                if text.trim().is_empty() {
                    continue;
                }
                return parse_message(text.trim().as_bytes()).map(Some);
            }
            if self.buf.len() > MAX_MESSAGE {
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.mcp_message",
                    format!("an MCP message exceeded {MAX_MESSAGE} bytes"),
                ));
            }
            let Some(out) = self.stdout.as_mut() else {
                return Ok(None);
            };
            match out.next_chunk().await? {
                Some(chunk) => self.buf.extend_from_slice(&chunk),
                None => {
                    if let Some(mut s) = self.stdout.take() {
                        let late = s.finish().await;
                        let _ = s.close().await;
                        late?;
                    }
                    let rest = std::mem::take(&mut self.buf);
                    let text = String::from_utf8_lossy(&rest);
                    if text.trim().is_empty() {
                        return Ok(None);
                    }
                    return parse_message(text.trim().as_bytes()).map(Some);
                }
            }
        }
    }

    async fn finish(mut self: Box<Self>) {
        if let Some(s) = self.stdin.take() {
            let _ = s.close().await;
        }
        if let Some(mut out) = self.stdout.take() {
            // Give the server a moment to exit on EOF, then terminate and reap.
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), async {
                while let Ok(Some(_)) = out.next_chunk().await {}
            })
            .await;
            let _ = out.close().await;
        }
    }
}

enum Inbox {
    Messages(VecDeque<Json>),
    Events {
        stream: Box<dyn ByteStream>,
        decoder: StreamDecoder,
        done: bool,
    },
}

struct HttpLink {
    url: String,
    http: Arc<McpHttpFn>,
    evaluator: Arc<dyn PolicyEvaluator>,
    origin: EffectOrigin,
    tls: TlsMaterial,
    session_id: Option<String>,
    protocol: Option<String>,
    inbox: Option<Inbox>,
}

impl HttpLink {
    fn exchange(&self, method: &str, body: Option<Vec<u8>>, accept: Vec<u16>) -> HttpExchange {
        let mut headers = vec![(
            "accept".to_string(),
            "application/json, text/event-stream".to_string(),
        )];
        if body.is_some() {
            headers.push(("content-type".into(), "application/json".into()));
        }
        if let Some(s) = &self.session_id {
            headers.push(("mcp-session-id".into(), s.clone()));
        }
        if let Some(v) = &self.protocol {
            headers.push(("mcp-protocol-version".into(), v.clone()));
        }
        HttpExchange {
            method: method.into(),
            url: self.url.clone(),
            headers,
            query: Vec::new(),
            body: body.map(|b| CodecInput {
                kind: CodecKind::Bytes,
                bytes: None,
                value: Some(Value::Bytes(b)),
            }),
            version: HttpVersionPolicy::Auto,
            decode: None,
            accept,
            retry: None,
            redirect_limit: 0,
            tls: self.tls.clone(),
            stream: Some(StreamMode::Sse),
            unix_socket: None,
            max_body: MAX_MESSAGE as u64,
            origin: self.origin.clone(),
        }
    }

    async fn close_inbox(&mut self) {
        if let Some(Inbox::Events { stream, .. }) = self.inbox.take() {
            let _ = stream.close().await;
        }
    }
}

fn header(outcome: &HttpOutcome, name: &str) -> Option<String> {
    outcome
        .response
        .headers
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[async_trait]
impl Link for HttpLink {
    async fn send(&mut self, msg: &Json, reply_expected: bool) -> RivetResult<()> {
        if reply_expected {
            self.close_inbox().await;
        }
        let x = self.exchange("POST", Some(msg.to_string().into_bytes()), vec![200, 202]);
        // every POST is authorized by exchange_http (allow_network + resolved-IP check)
        let outcome = (self.http)(x, Arc::clone(&self.evaluator)).await?;
        if self.session_id.is_none()
            && let Some(sid) = header(&outcome, "mcp-session-id")
        {
            self.session_id = Some(sid);
        }
        let content_type = header(&outcome, "content-type").unwrap_or_default();
        let Some(mut stream) = outcome.stream else {
            if reply_expected {
                self.inbox = Some(Inbox::Messages(VecDeque::new()));
            }
            return Ok(());
        };
        if !reply_expected {
            let _ = stream.close().await;
            return Ok(());
        }
        if content_type.starts_with("text/event-stream") {
            self.inbox = Some(Inbox::Events {
                stream,
                decoder: StreamDecoder::new(StreamMode::Sse, false, MAX_MESSAGE),
                done: false,
            });
            return Ok(());
        }
        let mut body = Vec::new();
        while let Some(chunk) = stream.next_chunk().await? {
            body.extend_from_slice(&chunk);
            if body.len() > MAX_MESSAGE {
                let _ = stream.close().await;
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.mcp_message",
                    format!("an MCP response exceeded {MAX_MESSAGE} bytes"),
                ));
            }
        }
        let _ = stream.close().await;
        let mut queue = VecDeque::new();
        if !body.iter().all(u8::is_ascii_whitespace) {
            queue.push_back(parse_message(&body)?);
        }
        self.inbox = Some(Inbox::Messages(queue));
        Ok(())
    }

    async fn recv(&mut self) -> RivetResult<Option<Json>> {
        match self.inbox.as_mut() {
            None => Ok(None),
            Some(Inbox::Messages(q)) => Ok(q.pop_front()),
            Some(Inbox::Events {
                stream,
                decoder,
                done,
            }) => loop {
                let ev = if *done {
                    decoder.finish()?
                } else {
                    decoder.pop()?
                };
                if let Some(ev) = ev {
                    let data = ev.get("data").and_then(Value::as_str).unwrap_or("");
                    if data.trim().is_empty() {
                        continue;
                    }
                    return parse_message(data.as_bytes()).map(Some);
                }
                if *done {
                    return Ok(None);
                }
                match stream.next_chunk().await? {
                    Some(chunk) => decoder.push(&chunk),
                    None => *done = true,
                }
            },
        }
    }

    fn negotiated(&mut self, version: &str) {
        self.protocol = Some(version.to_string());
    }

    async fn finish(mut self: Box<Self>) {
        self.close_inbox().await;
        if self.session_id.is_some() {
            let x = self.exchange("DELETE", None, vec![200, 202, 204, 404, 405]);
            if let Ok(o) = (self.http)(x, Arc::clone(&self.evaluator)).await
                && let Some(s) = o.stream
            {
                let _ = s.close().await;
            }
        }
    }
}

// ---------------------------------------------------------------- session

struct Session {
    link: Option<Box<dyn Link>>,
    peer: McpPeerInfo,
    next_id: i64,
    /// The request ID awaiting a response (cancelled if the session is dropped).
    pending: Option<i64>,
}

fn cancelled_notice(id: i64, reason: &str) -> Json {
    json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": id, "reason": reason}})
}

impl Session {
    fn link(&mut self) -> RivetResult<&mut Box<dyn Link>> {
        self.link.as_mut().ok_or_else(|| {
            RivetError::new(
                ErrorKind::Connection,
                "connection.mcp_closed",
                "MCP session is closed",
            )
        })
    }

    async fn initialize(&mut self) -> RivetResult<()> {
        let reply = self
            .call(
                "initialize",
                json!({
                    "protocolVersion": MCP_CLIENT_VERSION,
                    // No sampling, elicitation or roots: the host never implements them.
                    "capabilities": {},
                    "clientInfo": {"name": "rivet", "version": env!("CARGO_PKG_VERSION")},
                }),
            )
            .await?;
        let result = match reply {
            McpReply::Result(r) => r,
            McpReply::Error { code, message, .. } => {
                return Err(protocol_err(
                    "protocol.mcp_initialize",
                    format!("MCP initialize failed with JSON-RPC error {code}: {message}"),
                )
                .with_details(Value::object([("code", Value::Int(code))])));
            }
        };
        let version = result
            .get("protocolVersion")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        // a server protocolVersion outside the compatibility range => protocol.mcp_version
        if !MCP_COMPATIBLE_VERSIONS.contains(&version.as_str()) {
            return Err(protocol_err(
                "protocol.mcp_version",
                format!(
                    "MCP server negotiated protocol `{version}`; this client supports {}",
                    MCP_COMPATIBLE_VERSIONS.join(", ")
                ),
            ));
        }
        self.peer = McpPeerInfo {
            protocol_version: version.clone(),
            capabilities: result.get("capabilities").cloned().unwrap_or(json!({})),
            server_info: result.get("serverInfo").cloned().unwrap_or(json!({})),
        };
        let link = self.link()?;
        link.negotiated(&version);
        link.send(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            false,
        )
        .await
    }

    /// One correlated request/response exchange.
    async fn call(&mut self, method: &str, params: Json) -> RivetResult<McpReply> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.pending = Some(id);
        self.link()?.send(&msg, true).await?;
        loop {
            let Some(m) = self.link()?.recv().await? else {
                return Err(RivetError::new(
                    ErrorKind::Connection,
                    "connection.mcp_closed",
                    format!("MCP server closed the connection before answering `{method}`"),
                ));
            };
            let has_method = m.get("method").and_then(Json::as_str).map(str::to_string);
            let mid = m.get("id").cloned().filter(|v| !v.is_null());
            match (has_method, mid) {
                // A response: only ours counts; stale IDs are ignored.
                (None, Some(rid)) => {
                    if rid != json!(id) {
                        continue;
                    }
                    self.pending = None;
                    if let Some(e) = m.get("error") {
                        return Ok(McpReply::Error {
                            code: e.get("code").and_then(Json::as_i64).unwrap_or(0),
                            message: e
                                .get("message")
                                .and_then(Json::as_str)
                                .unwrap_or("")
                                .to_string(),
                            data: e.get("data").cloned(),
                        });
                    }
                    return Ok(McpReply::Result(
                        m.get("result").cloned().unwrap_or(json!({})),
                    ));
                }
                // A server → client request.
                (Some(sm), Some(rid)) => {
                    if sm == "ping" {
                        self.link()?
                            .send(&json!({"jsonrpc": "2.0", "id": rid, "result": {}}), false)
                            .await?;
                        continue;
                    }
                    self.link()?
                        .send(
                            &json!({"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": format!("the client does not support `{sm}`")}}),
                            false,
                        )
                        .await?;
                    // sampling/elicitation/roots requested by the server => declined (-32601) and protocol.unsupported_capability
                    if declined_server_request(&sm) {
                        return Err(unsupported_capability(&sm));
                    }
                }
                // Notifications: progress and log messages are never data.
                (Some(sm), None) => {
                    if sm == "notifications/cancelled"
                        && m.pointer("/params/requestId") == Some(&json!(id))
                    {
                        self.pending = None;
                        return Err(protocol_err(
                            "protocol.mcp_cancelled",
                            format!("the MCP server cancelled `{method}`"),
                        ));
                    }
                }
                (None, None) => {
                    return Err(protocol_err(
                        "protocol.mcp_message",
                        "MCP peer sent a message that is neither a request, a response nor a notification",
                    ));
                }
            }
        }
    }
}

#[async_trait]
impl McpSession for Session {
    fn peer(&self) -> &McpPeerInfo {
        &self.peer
    }

    async fn request(&mut self, method: &str, params: Json) -> RivetResult<McpReply> {
        self.call(method, params).await
    }

    async fn close(mut self: Box<Self>) {
        let pending = self.pending.take();
        if let Some(mut link) = self.link.take() {
            if let Some(id) = pending {
                let _ = link
                    .send(&cancelled_notice(id, "the caller stopped waiting"), false)
                    .await;
            }
            link.finish().await;
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Dropped mid-call (deadline, scope cancel): notify the server, then release.
        let pending = self.pending.take();
        if let Some(mut link) = self.link.take()
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            handle.spawn(async move {
                if let Some(id) = pending {
                    let _ = link
                        .send(&cancelled_notice(id, "request cancelled"), false)
                        .await;
                }
                link.finish().await;
            });
        }
    }
}
