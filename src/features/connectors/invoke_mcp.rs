use super::ports::{CredentialProvider, McpClient, McpSession, PolicyEvaluator};
use crate::domain::auth::{AuthContext, CredentialInput, origin_of};
use crate::domain::contracts::Principal;
use crate::domain::effect_checks::authorize;
use crate::domain::mcp::{
    MAX_MCP_HOPS, META_CHAIN, META_HOPS, McpConnectorInfo, McpImport, McpImportKind, McpReply,
    McpRequest, McpResult, McpSnapshot, McpTransport, check_schema,
};
use crate::domain::policy::{AccessVerb, Capability, EffectTarget};
use crate::domain::transports::EffectOrigin;
use crate::domain::{EffectsStatus, ErrorKind, RivetError, RivetResult, Value};
use serde_json::{Value as Json, json};

/// Upper bound on `*/list` pages read by one discovery.
const MAX_LIST_PAGES: usize = 100;

// vhco:usecase connectors.invoke_mcp(input: McpRequest) -> McpResult needs McpClient, PolicyEvaluator, CredentialProvider
// vhco:label Invoke mcp
// vhco:about Resolves an imported MCP operation (crm.tools.search, crm.resources.read, crm.prompts.get) or a `discover` against the reviewed snapshot pinned at bundle load, refuses schema changes, bridge recursion and invalid params, authorizes allow_mcp on the logical target before any I/O, acquires an origin-bound OAuth lease for `auth PROFILE account A` (http transport), then runs one scoped session (the adapter authorizes the transport and attaches the bearer), preserving content blocks and structuredContent and mapping isError, JSON-RPC errors and declined server callbacks to distinct typed errors.
// vhco:example input={connector:"crm", method:"tools.search", params:{query:"Ada"}} => { "content": [{"type": "text", "text": "{\"contacts\":[…]}"}], "structuredContent": {"contacts": [{"id": "42", "name": "Ada"}]}, "isError": false }
pub async fn invoke_mcp(
    input: McpRequest,
    evaluator: &dyn PolicyEvaluator,
    client: &dyn McpClient,
    credentials: Option<&dyn CredentialProvider>,
) -> RivetResult<McpResult> {
    let ctx = &input.context;
    let origin = EffectOrigin {
        operation_id: ctx.operation_id.clone(),
        span: ctx.span.clone(),
    };
    // vhco:todo authorize_mcp -- resolve CONNECTOR.METHOD in the pinned McpCatalog (unknown connector → not_found.mcp_connector; an ID that is not an exposed import of the reviewed snapshot → not_found.operation); the request's schema_hash must equal the loaded snapshot hash (else protocol.mcp_schema_changed); `auth PROFILE account A` without a credential provider is unsupported.auth (with one, an origin-bound lease is acquired after allow_mcp and before the session opens); hops ≥ MAX_MCP_HOPS is limit.mcp_hops and an identity already in the bridge chain is limit.mcp_recursion; params are validated against the snapshot (tool inputSchema → validation.mcp_params, resource URI must be exposed, prompt name exposed with its required arguments); finally authorize allow_mcp call Logical(connector/tools/NAME | resources/read | prompts/get | discover) — all before any transport I/O
    // vhco:step lookup client.catalog -- connector + import from the snapshot catalog read at bundle load (never live discovery)
    let catalog = client.catalog();
    let conn = catalog.connector(&input.connector).ok_or_else(|| {
        RivetError::not_found(
            "not_found.mcp_connector",
            format!("no MCP connector `{}`", input.connector),
        )
        .with_span(ctx.span.clone())
    })?;
    let discover = input.method == "discover";
    let import = if discover {
        None
    } else {
        let id = format!("{}.{}", conn.name, input.method);
        Some(catalog.import(&id).cloned().ok_or_else(|| {
            RivetError::not_found(
                "not_found.operation",
                format!(
                    "no operation `{id}`: it is not an exposed import of connector `{}`'s reviewed snapshot",
                    conn.name
                ),
            )
            .with_span(ctx.span.clone())
        })?)
    };
    // vhco:error schema_changed -- the caller pinned a different snapshot hash than the one loaded => protocol.mcp_schema_changed before any I/O
    // vhco:step pin compare -- the program's snapshot hash is the only schema a call may use
    if !discover && conn.schema_hash.as_deref() != Some(input.schema_hash.as_str()) {
        return Err(RivetError::new(
            ErrorKind::Protocol,
            "protocol.mcp_schema_changed",
            format!(
                "connector `{}` snapshot changed since this call was compiled; reload the bundle",
                conn.name
            ),
        ));
    }
    // vhco:error auth_unsupported -- `auth PROFILE account A` on a host without a credential provider => unsupported.auth returns before any effect
    if let (Some((profile, account)), None) = (&conn.auth, credentials) {
        return Err(RivetError::unsupported(
            "unsupported.auth",
            format!(
                "connector `{}` uses `auth {profile} account \"{account}\"`, but this host has no credential provider",
                conn.name
            ),
        )
        .with_span(ctx.span.clone()));
    }
    // vhco:step bridge compare -- bounded hop count and identity chain (server/operation) stop recursive bridges
    // vhco:error hop_limit -- hops already at MAX_MCP_HOPS => limit.mcp_hops (429, exit 5) before any effect
    if ctx.bridge.hops >= MAX_MCP_HOPS {
        return Err(RivetError::new(
            ErrorKind::Limit,
            "limit.mcp_hops",
            format!(
                "MCP bridge hop limit ({MAX_MCP_HOPS}) reached calling `{}.{}`",
                conn.name, input.method
            ),
        )
        .with_details(bridge_details(&ctx.bridge.chain, ctx.bridge.hops)));
    }
    let entry = format!("{}/{}.{}", ctx.identity, conn.name, input.method);
    // vhco:error recursion -- this host already called the same import earlier in the chain => limit.mcp_recursion
    if ctx.bridge.chain.contains(&entry) {
        return Err(RivetError::new(
            ErrorKind::Limit,
            "limit.mcp_recursion",
            format!("recursive MCP bridge: `{entry}` is already in the call chain"),
        )
        .with_details(bridge_details(&ctx.bridge.chain, ctx.bridge.hops)));
    }
    // vhco:step params check_params -- the snapshot decides what a valid call looks like
    let call = match &import {
        Some(i) => Some(check_params(i, conn, &input.params)?),
        None => None,
    };
    // vhco:step authorize effect_checks::authorize -- allow_mcp call on the logical target
    let target = match &import {
        Some(i) => i.target(),
        None => format!("{}/discover", conn.name),
    };
    authorize(
        evaluator,
        &origin,
        Capability::Mcp,
        AccessVerb::Call,
        EffectTarget::Logical(target),
    )?;

    // vhco:todo invoke_peer -- open one scoped session through McpClient (the adapter authorizes allow_network per POST or allow_exec + sandbox for stdio, then runs initialize with a compatible protocolVersion and notifications/initialized); the negotiated capability for tools/resources/prompts must be present (protocol.mcp_capability); send the call with a fresh correlation ID and `_meta` {rivet/hops: hops+1, rivet/chain: chain+[identity/import]}; a JSON-RPC error keeps its code (protocol.mcp_error, details.code); tools isError:true → application mcp.tool_failed with content/structuredContent in details and effects unknown; a structuredContent that fails the snapshot outputSchema → protocol.mcp_output_schema; content blocks and structuredContent are returned unchanged; discovery pages tools/resources/prompts lists into a candidate snapshot; the session is always closed, and dropping it mid-call sends notifications/cancelled
    // vhco:step auth credentials.acquire -- `auth PROFILE account A` (http transport only; stdio + auth fails at bundle load): authorize allow_network on the endpoint first so a denied resource never costs a token request, then acquire an origin-bound lease (allow_auth use, allow_credentials, token endpoint; the endpoint origin must be one of the profile's resource_origins) that the HTTP transport attaches as `Authorization: Bearer` on every POST/DELETE of this session
    let mut ctx_owned;
    let ctx = match (&conn.auth, credentials, &conn.transport) {
        (Some((profile, account)), Some(provider), McpTransport::Http { url }) => {
            let endpoint_origin = origin_of(url).ok_or_else(|| {
                RivetError::validation(
                    "mcp.auth_transport",
                    format!(
                        "connector `{}`: `auth` needs an http(s) endpoint",
                        conn.name
                    ),
                )
            })?;
            authorize(
                evaluator,
                &origin,
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url(url.clone()),
            )?;
            let lease = provider
                .acquire(
                    CredentialInput {
                        profile: profile.clone(),
                        account: account.clone(),
                        origin: endpoint_origin,
                        audience: None,
                        scopes: Vec::new(),
                        context: AuthContext {
                            principal: ctx.principal.clone().unwrap_or_else(Principal::local),
                            operation_id: ctx.operation_id.clone(),
                            deadline_ms: ctx.deadline_ms,
                        },
                    },
                    evaluator,
                )
                .await
                .map_err(|e| e.with_span(ctx.span.clone()))?;
            ctx_owned = ctx.clone();
            ctx_owned.bearer = Some(lease);
            &ctx_owned
        }
        (Some(_), _, McpTransport::Command { .. }) => {
            // vhco:error auth_stdio -- `auth` on a stdio connector => validation.mcp_auth_transport (also refused at load)
            return Err(RivetError::validation(
                "mcp.auth_transport",
                format!(
                    "connector `{}`: `auth` applies only to `transport http` (credentials bind to network origins)",
                    conn.name
                ),
            ));
        }
        _ => ctx,
    };
    // vhco:step open client.open -- spawn/connect + initialize (transport authorized by the adapter per attempt)
    let opened = client.open(conn, ctx).await;
    // vhco:error resource_401 -- the MCP endpoint answers 401 to a bearer => the lease is invalidated (next call reacquires) and http.status returns
    let rejected = |e: &RivetError| {
        e.code == "http.status" && e.details.get("status") == Some(&Value::Int(401))
    };
    let mut session = match opened {
        Ok(s) => s,
        Err(e) => {
            if let (Some(lease), Some(p), true) = (&ctx.bearer, credentials, rejected(&e)) {
                p.invalidate(lease);
            }
            return Err(e);
        }
    };
    // vhco:step drift check_drift -- first use of the session: live tools/list vs the approved snapshot for every exposed tool (name + inputSchema); any difference is mcp.schema_drift and the call is never sent with a live schema
    let drift = match &import {
        Some(_) => check_drift(session.as_mut(), conn).await,
        None => Ok(()),
    };
    if let Err(e) = drift {
        session.close().await;
        return Err(e);
    }
    let outcome = match (&import, call) {
        (Some(i), Some(params)) => {
            let mut params = params;
            if i.kind == McpImportKind::Tool {
                let mut chain = ctx.bridge.chain.clone();
                chain.push(entry);
                params["_meta"] = json!({META_HOPS: ctx.bridge.hops + 1, META_CHAIN: chain});
            }
            call_import(session.as_mut(), i, conn, params).await
        }
        _ => discover_snapshot(session.as_mut()).await,
    };
    // vhco:step close session.close -- owned sessions end with the call (stdin EOF + reap, or HTTP DELETE)
    session.close().await;
    if let (Err(e), Some(lease), Some(p)) = (&outcome, &ctx.bearer, credentials)
        && rejected(e)
    {
        p.invalidate(lease);
    }
    outcome
}

fn bridge_details(chain: &[String], hops: u32) -> Value {
    Value::object([
        ("hops", Value::Int(hops as i64)),
        (
            "chain",
            Value::List(chain.iter().map(Value::text).collect()),
        ),
    ])
}

fn params_error(msg: impl Into<String>, problems: &[String]) -> RivetError {
    RivetError::validation("validation.mcp_params", msg).with_details(Value::object([(
        "problems",
        Value::List(problems.iter().map(Value::text).collect()),
    )]))
}

/// Build the JSON-RPC params for one import after checking them against the snapshot.
fn check_params(import: &McpImport, conn: &McpConnectorInfo, params: &Value) -> RivetResult<Json> {
    let snapshot: &McpSnapshot = conn.snapshot.as_ref().ok_or_else(|| {
        RivetError::validation(
            "mcp.snapshot_missing",
            format!("connector `{}` has no reviewed snapshot", conn.name),
        )
    })?;
    let args = match params {
        Value::Null => json!({}),
        v @ Value::Object(_) => v.to_json(),
        other => {
            return Err(params_error(
                format!("params must be an object, got {}", other.type_name()),
                &[],
            ));
        }
    };
    match import.kind {
        McpImportKind::Tool => {
            let name = import.name.clone().unwrap_or_default();
            let tool = snapshot.tool(&name).ok_or_else(|| {
                RivetError::not_found(
                    "not_found.operation",
                    format!("tool `{name}` is not in the reviewed snapshot"),
                )
            })?;
            let mut problems = Vec::new();
            check_schema(&tool.input_schema, &args, "params", &mut problems);
            if !problems.is_empty() {
                return Err(params_error(
                    format!(
                        "invalid params for `{}`: {}",
                        import.id,
                        problems.join("; ")
                    ),
                    &problems,
                ));
            }
            Ok(json!({"name": name, "arguments": args}))
        }
        McpImportKind::ResourceRead => {
            let obj = args.as_object().cloned().unwrap_or_default();
            if let Some(k) = obj.keys().find(|k| k.as_str() != "uri") {
                return Err(params_error(
                    format!("unknown parameter `{k}` for `{}` (only uri)", import.id),
                    &[],
                ));
            }
            let uri = obj
                .get("uri")
                .and_then(Json::as_str)
                .ok_or_else(|| params_error(format!("`{}` needs {{uri: text}}", import.id), &[]))?;
            if !conn.expose_resources.iter().any(|u| u == uri) {
                return Err(RivetError::not_found(
                    "not_found.mcp_resource",
                    format!(
                        "resource `{uri}` is not exposed by connector `{}`",
                        conn.name
                    ),
                ));
            }
            Ok(json!({"uri": uri}))
        }
        McpImportKind::PromptGet => {
            let obj = args.as_object().cloned().unwrap_or_default();
            if let Some(k) = obj
                .keys()
                .find(|k| !matches!(k.as_str(), "name" | "arguments"))
            {
                return Err(params_error(
                    format!(
                        "unknown parameter `{k}` for `{}` (name, arguments)",
                        import.id
                    ),
                    &[],
                ));
            }
            let name = obj.get("name").and_then(Json::as_str).ok_or_else(|| {
                params_error(format!("`{}` needs {{name: text}}", import.id), &[])
            })?;
            if !conn.expose_prompts.iter().any(|p| p == name) {
                return Err(RivetError::not_found(
                    "not_found.mcp_prompt",
                    format!(
                        "prompt `{name}` is not exposed by connector `{}`",
                        conn.name
                    ),
                ));
            }
            let arguments = obj.get("arguments").cloned().unwrap_or(json!({}));
            let Some(given) = arguments.as_object() else {
                return Err(params_error("`arguments` must be an object", &[]));
            };
            let mut problems = Vec::new();
            for (k, v) in given {
                if !v.is_string() {
                    problems.push(format!("arguments.{k}: prompt arguments are text"));
                }
            }
            if let Some(spec) = snapshot.prompt(name) {
                for a in spec.arguments.iter().filter(|a| a.required) {
                    if !given.contains_key(&a.name) {
                        problems.push(format!("arguments.{}: required", a.name));
                    }
                }
                for k in given.keys() {
                    if !spec.arguments.iter().any(|a| &a.name == k) {
                        problems.push(format!("arguments.{k}: unknown argument"));
                    }
                }
            }
            if !problems.is_empty() {
                return Err(params_error(
                    format!("invalid prompt arguments: {}", problems.join("; ")),
                    &problems,
                ));
            }
            Ok(json!({"name": name, "arguments": arguments}))
        }
    }
}

fn protocol(code: &str, msg: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Protocol, code, msg)
}

/// JSON-RPC error → protocol.mcp_error keeping the JSON-RPC code, message and data.
fn rpc_error(method: &str, code: i64, message: String, data: Option<Json>) -> RivetError {
    let mut details = Value::object([
        ("code", Value::Int(code)),
        ("message", Value::text(&message)),
        ("method", Value::text(method)),
    ]);
    if let Some(d) = data {
        details.set("data", Value::from_json(&d));
    }
    protocol(
        "protocol.mcp_error",
        format!("MCP {method} failed with JSON-RPC error {code}: {message}"),
    )
    .with_details(details)
    .with_effects(EffectsStatus::Unknown)
}

async fn call_import(
    session: &mut dyn McpSession,
    import: &McpImport,
    conn: &McpConnectorInfo,
    params: Json,
) -> RivetResult<McpResult> {
    // vhco:step capability session.peer -- the negotiated server capabilities must include tools/resources/prompts
    // vhco:error capability -- the server did not negotiate the needed capability => protocol.mcp_capability
    if !session.peer().supports(import.capability()) {
        return Err(protocol(
            "protocol.mcp_capability",
            format!(
                "MCP server of connector `{}` did not negotiate the `{}` capability",
                conn.name,
                import.capability()
            ),
        ));
    }
    let method = import.rpc_method();
    // vhco:step call session.request -- one correlated request; progress/log notifications never become data
    let result = match session.request(method, params).await? {
        McpReply::Result(r) => r,
        // vhco:error jsonrpc -- a JSON-RPC error response => protocol.mcp_error with details.code (the JSON-RPC identity)
        McpReply::Error {
            code,
            message,
            data,
        } => return Err(rpc_error(method, code, message, data)),
    };
    let list = |key: &str| -> RivetResult<Vec<Value>> {
        match result.get(key) {
            Some(Json::Array(a)) => Ok(a.iter().map(Value::from_json).collect()),
            _ => Err(protocol(
                "protocol.mcp_result",
                format!("MCP {method} result has no `{key}` array"),
            )),
        }
    };
    match import.kind {
        McpImportKind::Tool => {
            let content = list("content")?;
            let structured = result.get("structuredContent").cloned();
            // vhco:step decide isError -- a tool execution failure is application data, not a protocol fault
            // vhco:error tool_failed -- result.isError true => mcp.tool_failed (kind application, 502, exit 5) with content and structuredContent in details
            if result.get("isError").and_then(Json::as_bool) == Some(true) {
                let mut details = Value::object([
                    ("tool", Value::text(import.name.as_deref().unwrap_or(""))),
                    ("content", Value::List(content)),
                ]);
                if let Some(s) = &structured {
                    details.set("structuredContent", Value::from_json(s));
                }
                return Err(RivetError::new(
                    ErrorKind::Application,
                    "mcp.tool_failed",
                    format!("MCP tool `{}` returned isError: true", import.id),
                )
                .with_details(details)
                .with_effects(EffectsStatus::Unknown));
            }
            // vhco:error output_schema -- structuredContent that violates the snapshot outputSchema => protocol.mcp_output_schema
            let tool = conn
                .snapshot
                .as_ref()
                .and_then(|s| s.tool(import.name.as_deref().unwrap_or("")));
            if let (Some(schema), Some(s)) =
                (tool.and_then(|t| t.output_schema.as_ref()), &structured)
            {
                let mut problems = Vec::new();
                check_schema(schema, s, "structuredContent", &mut problems);
                if !problems.is_empty() {
                    return Err(protocol(
                        "protocol.mcp_output_schema",
                        format!(
                            "MCP tool `{}` returned structuredContent outside its reviewed outputSchema: {}",
                            import.id,
                            problems.join("; ")
                        ),
                    )
                    .with_effects(EffectsStatus::Unknown));
                }
            }
            let mut metadata = Value::object([("method", Value::text(method))]);
            if let Some(m) = result.get("_meta") {
                metadata.set("_meta", Value::from_json(m));
            }
            Ok(McpResult {
                content,
                structured_content: structured.as_ref().map(Value::from_json),
                metadata,
            })
        }
        McpImportKind::ResourceRead => Ok(McpResult {
            content: list("contents")?,
            structured_content: None,
            metadata: Value::object([("method", Value::text(method))]),
        }),
        McpImportKind::PromptGet => {
            let mut metadata = Value::object([("method", Value::text(method))]);
            if let Some(d) = result.get("description") {
                metadata.set("description", Value::from_json(d));
            }
            Ok(McpResult {
                content: list("messages")?,
                structured_content: None,
                metadata,
            })
        }
    }
}

/// G29 (proposal Increment 6: "refreshed schemas never change a running registry
/// silently"): at first use of each connector session, compare the server's live
/// `tools/list` with the approved snapshot. Every exposed tool must still exist
/// under the same name with an identical `inputSchema` (JSON equality, key order
/// ignored); otherwise the call fails `mcp.schema_drift` before anything is sent
/// and the live schema is never used. Servers that did not negotiate `tools` are
/// left to the capability check of the call itself.
async fn check_drift(session: &mut dyn McpSession, conn: &McpConnectorInfo) -> RivetResult<()> {
    let Some(snapshot) = &conn.snapshot else {
        return Ok(());
    };
    if conn.expose_tools.is_empty() || !session.peer().supports("tools") {
        return Ok(());
    }
    let live = list_all(session, "tools/list", "tools").await?;
    let mut drifted: Vec<(String, &'static str)> = Vec::new();
    for name in &conn.expose_tools {
        let approved = snapshot.tools.iter().find(|t| &t.name == name);
        let current = live
            .iter()
            .find(|t| t.get("name").and_then(Json::as_str) == Some(name.as_str()));
        match (approved, current) {
            (Some(a), Some(c)) => {
                if c.get("inputSchema") != Some(&a.input_schema) {
                    drifted.push((name.clone(), "inputSchema changed"));
                }
            }
            (Some(_), None) => drifted.push((name.clone(), "missing from the live server")),
            (None, _) => drifted.push((name.clone(), "missing from the approved snapshot")),
        }
    }
    if drifted.is_empty() {
        return Ok(());
    }
    let list: Vec<String> = drifted.iter().map(|(n, w)| format!("{n}: {w}")).collect();
    Err(protocol(
        "mcp.schema_drift",
        format!(
            "connector `{}`: the live server no longer matches the approved snapshot ({}); run `rivet connectors sync` and review the new snapshot",
            conn.name,
            list.join("; ")
        ),
    )
    .with_details(Value::object([
        ("connector", Value::text(&conn.name)),
        (
            "tools",
            Value::List(
                drifted
                    .iter()
                    .map(|(n, w)| {
                        Value::object([("name", Value::text(n)), ("difference", Value::text(*w))])
                    })
                    .collect(),
            ),
        ),
    ])))
}

/// Page one `*/list` method (cursor pagination, bounded).
async fn list_all(session: &mut dyn McpSession, method: &str, key: &str) -> RivetResult<Vec<Json>> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_LIST_PAGES {
        let params = match &cursor {
            Some(c) => json!({"cursor": c}),
            None => json!({}),
        };
        let page = match session.request(method, params).await? {
            McpReply::Result(r) => r,
            McpReply::Error {
                code,
                message,
                data,
            } => return Err(rpc_error(method, code, message, data)),
        };
        if let Some(Json::Array(items)) = page.get(key) {
            out.extend(items.iter().cloned());
        }
        match page.get("nextCursor").and_then(Json::as_str) {
            Some(c) if !c.is_empty() => cursor = Some(c.to_string()),
            _ => return Ok(out),
        }
    }
    Err(RivetError::new(
        ErrorKind::Limit,
        "limit.mcp_pages",
        format!("{method} returned more than {MAX_LIST_PAGES} pages"),
    ))
}

/// Keep only the fields a snapshot records.
fn pick(item: &Json, keys: &[&str]) -> Json {
    let mut o = serde_json::Map::new();
    for k in keys {
        if let Some(v) = item.get(*k) {
            o.insert((*k).to_string(), v.clone());
        }
    }
    Json::Object(o)
}

async fn discover_snapshot(session: &mut dyn McpSession) -> RivetResult<McpResult> {
    // vhco:step discover list_all -- tools/list, resources/list and prompts/list (only negotiated capabilities), every page
    let peer = session.peer().clone();
    let tools = if peer.supports("tools") {
        list_all(session, "tools/list", "tools").await?
    } else {
        Vec::new()
    };
    let resources = if peer.supports("resources") {
        list_all(session, "resources/list", "resources").await?
    } else {
        Vec::new()
    };
    let prompts = if peer.supports("prompts") {
        list_all(session, "prompts/list", "prompts").await?
    } else {
        Vec::new()
    };
    let raw = json!({
        "protocolVersion": peer.protocol_version,
        "serverInfo": peer.server_info,
        "tools": tools.iter().map(|t| pick(t, &["name", "title", "description", "inputSchema", "outputSchema"])).collect::<Vec<_>>(),
        "resources": resources.iter().map(|r| pick(r, &["uri", "name", "description", "mimeType"])).collect::<Vec<_>>(),
        "prompts": prompts.iter().map(|p| pick(p, &["name", "description", "arguments"])).collect::<Vec<_>>(),
    });
    // vhco:error bad_listing -- a listing that does not form a valid snapshot (missing names/schemas, duplicates) => mcp.snapshot
    let snapshot = McpSnapshot::parse(raw.to_string().as_bytes())?;
    Ok(McpResult {
        content: Vec::new(),
        structured_content: Some(Value::from_json(&snapshot.to_json())),
        metadata: Value::object([
            ("tools", Value::Int(snapshot.tools.len() as i64)),
            ("resources", Value::Int(snapshot.resources.len() as i64)),
            ("prompts", Value::Int(snapshot.prompts.len() as i64)),
        ]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::{
        BridgeHops, McpCatalog, McpContext, McpPeerInfo, McpTls, McpTransport,
    };
    use crate::domain::policy::{Decision, EffectIntent, Permit, Policy};
    use crate::domain::source::SourceSpan;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct Allow(Policy, bool);
    impl PolicyEvaluator for Allow {
        fn evaluate(&self, i: &EffectIntent) -> Permit {
            Permit {
                intent: i.clone(),
                decision: if self.1 {
                    Decision::Allowed
                } else {
                    Decision::Denied
                },
                rule: "test".into(),
            }
        }
        fn policy(&self) -> &Policy {
            &self.0
        }
    }

    struct Fake {
        catalog: McpCatalog,
        replies: Mutex<Vec<McpReply>>,
        sent: std::sync::Arc<Mutex<Vec<(String, Json)>>>,
    }

    struct FakeSession {
        peer: McpPeerInfo,
        replies: Vec<McpReply>,
        sent: std::sync::Arc<Mutex<Vec<(String, Json)>>>,
    }

    #[async_trait]
    impl McpSession for FakeSession {
        fn peer(&self) -> &McpPeerInfo {
            &self.peer
        }
        async fn request(&mut self, method: &str, params: Json) -> RivetResult<McpReply> {
            self.sent.lock().unwrap().push((method.into(), params));
            Ok(self.replies.remove(0))
        }
        async fn close(self: Box<Self>) {}
    }

    #[async_trait]
    impl McpClient for Fake {
        fn catalog(&self) -> &McpCatalog {
            &self.catalog
        }
        async fn open(
            &self,
            _: &McpConnectorInfo,
            _: &McpContext,
        ) -> RivetResult<Box<dyn McpSession>> {
            Ok(Box::new(FakeSession {
                peer: McpPeerInfo {
                    protocol_version: "2025-11-25".into(),
                    capabilities: json!({"tools": {}}),
                    server_info: json!({"name": "fake"}),
                },
                replies: std::mem::take(&mut *self.replies.lock().unwrap()),
                sent: self.sent.clone(),
            }))
        }
    }

    const SEARCH_SCHEMA: &str =
        r#"{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}"#;

    /// The live `tools/list` answer the drift check reads first.
    fn live_tools(input_schema: &str) -> McpReply {
        let schema: Json = serde_json::from_str(input_schema).unwrap();
        McpReply::Result(json!({"tools": [{"name": "search", "inputSchema": schema}]}))
    }

    /// A peer whose live tools/list matches the approved snapshot.
    fn fake(replies: Vec<McpReply>) -> Fake {
        let mut all = vec![live_tools(SEARCH_SCHEMA)];
        all.extend(replies);
        fake_raw(all)
    }

    fn fake_raw(replies: Vec<McpReply>) -> Fake {
        let snapshot = McpSnapshot::parse(br#"{"protocolVersion":"2025-11-25","tools":[{"name":"search","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]}"#).unwrap();
        Fake {
            catalog: McpCatalog {
                connectors: vec![McpConnectorInfo {
                    name: "crm".into(),
                    transport: McpTransport::Http {
                        url: "https://mcp.example.com/mcp".into(),
                    },
                    tls: McpTls::default(),
                    auth: None,
                    schema_path: Some("./schemas/crm.json".into()),
                    schema_hash: Some("sha256:abc".into()),
                    snapshot: Some(snapshot),
                    expose_tools: vec!["search".into()],
                    expose_resources: vec![],
                    expose_prompts: vec![],
                    span: SourceSpan::default(),
                }],
                imports: vec![McpImport {
                    id: "crm.tools.search".into(),
                    connector: "crm".into(),
                    kind: McpImportKind::Tool,
                    name: Some("search".into()),
                }],
            },
            replies: Mutex::new(replies),
            sent: Default::default(),
        }
    }

    fn req(params: Json, bridge: BridgeHops) -> McpRequest {
        McpRequest {
            connector: "crm".into(),
            method: "tools.search".into(),
            params: Value::from_json(&params),
            schema_hash: "sha256:abc".into(),
            context: McpContext {
                operation_id: "contacts.find".into(),
                identity: "rivet:test".into(),
                bridge,
                ..McpContext::default()
            },
        }
    }

    fn allow() -> Allow {
        Allow(Policy::deny_all("."), true)
    }

    // vhco:test connectors.invoke_mcp -- a tool result keeps content and structuredContent, and the call carries bridge _meta
    #[tokio::test]
    async fn tool_success_preserves_result_and_sends_hops() {
        let f = fake(vec![McpReply::Result(
            json!({"content": [{"type": "text", "text": "hi"}], "structuredContent": {"contacts": []}, "isError": false}),
        )]);
        let r = invoke_mcp(
            req(json!({"query": "Ada"}), BridgeHops::default()),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap();
        let v = r.to_value(McpImportKind::Tool);
        assert_eq!(v.get("isError"), Some(&Value::Bool(false)));
        assert!(v.get("structuredContent").is_some());
        let sent = f.sent.lock().unwrap();
        assert_eq!(sent[0].0, "tools/list", "drift check first");
        assert_eq!(sent[1].0, "tools/call");
        assert_eq!(sent[1].1["_meta"]["rivet/hops"], json!(1));
        assert_eq!(
            sent[1].1["_meta"]["rivet/chain"][0],
            json!("rivet:test/crm.tools.search")
        );
    }

    // vhco:test connectors.invoke_mcp -- isError maps to mcp.tool_failed and a JSON-RPC error keeps its code
    #[tokio::test]
    async fn tool_failures_are_typed() {
        let f = fake(vec![McpReply::Result(
            json!({"content": [{"type": "text", "text": "boom"}], "isError": true}),
        )]);
        let e = invoke_mcp(
            req(json!({"query": "x"}), BridgeHops::default()),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(
            (e.kind, e.code.as_str()),
            (ErrorKind::Application, "mcp.tool_failed")
        );
        let f = fake(vec![McpReply::Error {
            code: -32602,
            message: "Unknown tool".into(),
            data: None,
        }]);
        let e = invoke_mcp(
            req(json!({"query": "x"}), BridgeHops::default()),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "protocol.mcp_error");
        assert_eq!(e.details.get("code"), Some(&Value::Int(-32602)));
    }

    // vhco:test connectors.invoke_mcp -- G29 a live tools/list whose exposed tool changed its inputSchema or disappeared fails mcp.schema_drift and the tool call is never sent; key order alone is not drift
    #[tokio::test]
    async fn live_schema_drift_is_refused() {
        let ok = McpReply::Result(json!({"content": [], "isError": false}));
        for live in [
            live_tools(
                r#"{"type":"object","properties":{"query":{"type":"integer"}},"required":["query"]}"#,
            ),
            McpReply::Result(
                json!({"tools": [{"name": "other", "inputSchema": {"type": "object"}}]}),
            ),
        ] {
            let f = fake_raw(vec![live, ok.clone()]);
            let e = invoke_mcp(
                req(json!({"query": "x"}), BridgeHops::default()),
                &allow(),
                &f,
                None,
            )
            .await
            .unwrap_err();
            assert_eq!(e.code, "mcp.schema_drift");
            let sent = f.sent.lock().unwrap();
            assert_eq!(sent.len(), 1, "only tools/list was sent");
            assert_eq!(sent[0].0, "tools/list");
        }
        // Same schema with keys in another order: not drift.
        let f = fake_raw(vec![
            live_tools(
                r#"{"required":["query"],"properties":{"query":{"type":"string"}},"type":"object"}"#,
            ),
            ok,
        ]);
        invoke_mcp(
            req(json!({"query": "x"}), BridgeHops::default()),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap();
    }

    // vhco:test connectors.invoke_mcp -- invalid params, denied allow_mcp, hop limit and recursion all fail before the session opens
    #[tokio::test]
    async fn guards_run_before_io() {
        let f = fake(vec![]);
        let e = invoke_mcp(
            req(json!({"q": 1}), BridgeHops::default()),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "validation.mcp_params");
        let e = invoke_mcp(
            req(json!({"query": "x"}), BridgeHops::default()),
            &Allow(Policy::deny_all("."), false),
            &f,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Permission);
        let e = invoke_mcp(
            req(
                json!({"query": "x"}),
                BridgeHops {
                    hops: MAX_MCP_HOPS,
                    chain: vec![],
                },
            ),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "limit.mcp_hops");
        let e = invoke_mcp(
            req(
                json!({"query": "x"}),
                BridgeHops {
                    hops: 1,
                    chain: vec!["rivet:test/crm.tools.search".into()],
                },
            ),
            &allow(),
            &f,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "limit.mcp_recursion");
        let mut r = req(json!({"query": "x"}), BridgeHops::default());
        r.schema_hash = "sha256:other".into();
        assert_eq!(
            invoke_mcp(r, &allow(), &f, None).await.unwrap_err().code,
            "protocol.mcp_schema_changed"
        );
        assert!(f.sent.lock().unwrap().is_empty());
    }
}
