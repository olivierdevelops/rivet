//! MCP surface encoding (protocol 2025-11-25): JSON-RPC messages, tool
//! descriptors and tool results. Hand-rolled so the documented shapes are
//! exact; the dispatch loop lives in `orchestrator::setup_mcp`.
//!
//! ```text
//!  tools/list ─▶ one direct tool per authorized public operation
//!               {name:id, title:name, description, inputSchema:params, outputSchema:ResponseEnvelope{data:output}}
//!               streaming ops add _meta {"rivet/delivery":"session"} and return an accepted envelope
//!               whose data is the SessionReceipt
//!             + every built-in the principal may call (rivet.request / list / describe /
//!               outputs / sessions.* / io / policy.generate / trace.show /
//!               connectors.sync / auth.*), filtered by the caller's authorization
//!  tools/call ─▶ {content:[{type:text,text:<envelope JSON>}], structuredContent:<envelope>,
//!               isError: status is error or cancelled}   (JSON-RPC framing unchanged)
//!  rivet.request / rivet.sessions.open arguments: {operation, data} (deprecated {id, params})
//! ```

use crate::domain::contracts::RegistryEntry;
use crate::domain::serve::MCP_PROTOCOL_VERSION;
use serde_json::{Value as Json, json};

/// JSON-RPC error codes.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// One incoming JSON-RPC message.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcMessage {
    /// Present on requests (and responses); absent on notifications.
    pub id: Option<Json>,
    pub method: Option<String>,
    pub params: Json,
}

impl RpcMessage {
    pub fn is_request(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }
}

/// Parse one JSON-RPC 2.0 message; the error is a ready-to-send error response.
pub fn parse_message(bytes: &[u8]) -> Result<RpcMessage, Json> {
    let j: Json = serde_json::from_slice(bytes)
        .map_err(|e| rpc_error(&Json::Null, PARSE_ERROR, &format!("parse error: {e}")))?;
    let Some(obj) = j.as_object() else {
        return Err(rpc_error(
            &Json::Null,
            INVALID_REQUEST,
            "expected one JSON-RPC message object (batches are not supported)",
        ));
    };
    let id = obj.get("id").cloned().filter(|v| !v.is_null());
    if obj.get("jsonrpc").and_then(Json::as_str) != Some("2.0") {
        return Err(rpc_error(
            id.as_ref().unwrap_or(&Json::Null),
            INVALID_REQUEST,
            "jsonrpc must be \"2.0\"",
        ));
    }
    Ok(RpcMessage {
        id,
        method: obj.get("method").and_then(Json::as_str).map(str::to_string),
        params: obj.get("params").cloned().unwrap_or(json!({})),
    })
}

pub fn rpc_result(id: &Json, result: Json) -> Json {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub fn rpc_error(id: &Json, code: i64, message: &str) -> Json {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// `initialize` result.
pub fn initialize_result(version: &str) -> Json {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "rivet", "version": version},
    })
}

/// ResponseEnvelope schema (PROP-2026-0002 R1) whose `data` is `data_schema`
/// when the call succeeds (`data` is null on error/cancelled).
pub fn envelope_schema(data_schema: Json) -> Json {
    json!({
        "type": "object",
        "properties": {
            "request_id": {"type": "string"},
            "trace_id": {"type": "string"},
            "operation": {"type": ["string", "null"]},
            "type": {"type": "string", "enum": ["result"]},
            "status": {"type": "string", "enum": ["ok", "error", "cancelled", "accepted"]},
            "data": {"anyOf": [data_schema, {"type": "null"}]},
            "error": {"type": ["object", "null"]},
            "effects": {"type": "string", "enum": ["none", "committed", "partial", "unknown"]},
            "data_count": {"type": "integer", "minimum": 0},
        },
        "required": ["request_id", "trace_id", "operation", "type", "status", "data", "error", "effects", "data_count"],
    })
}

/// SessionReceipt schema (the `data` of a streaming tool's accepted envelope).
pub fn receipt_schema() -> Json {
    json!({
        "type": "object",
        "properties": {
            "session_id": {"type": "string"},
            "request_id": {"type": "string"},
            "trace_id": {"type": "string"},
            "catalog_version": {"type": "string"},
            "input_schema": {},
            "emits_schema": {},
            "next_send_seq": {"type": "integer"},
            "expires_at": {"type": "string"},
        },
        "required": ["session_id", "request_id", "catalog_version", "next_send_seq", "expires_at"],
    })
}

/// Input envelope properties for `rivet.request` / `rivet.sessions.open`:
/// `operation` + `data`, with the deprecated 0.1.0 `id` / `params` aliases.
fn envelope_input_props(extra: Json) -> Json {
    let mut props = json!({
        "operation": {"type": "string", "description": "Operation ID."},
        "data": {"type": "object", "description": "Operation input (default {})."},
        "id": {"type": "string", "deprecated": true, "description": "Deprecated alias of `operation` (removed in 0.3.0)."},
        "params": {"type": "object", "deprecated": true, "description": "Deprecated alias of `data` (removed in 0.3.0)."},
    });
    if let (Json::Object(p), Json::Object(x)) = (&mut props, extra) {
        p.extend(x);
    }
    props
}

/// Direct named tool for one public operation.
pub fn tool_descriptor(e: &RegistryEntry) -> Json {
    let mut t = json!({
        "name": e.id,
        "title": e.name,
        "inputSchema": e.input_schema(),
    });
    if let Some(d) = &e.description {
        t["description"] = json!(d);
    }
    if e.streaming() {
        t["outputSchema"] = envelope_schema(receipt_schema());
        t["_meta"] = json!({"rivet/delivery": "session"});
    } else {
        t["outputSchema"] = envelope_schema(e.output_schema());
    }
    t
}

fn builtin(name: &str, title: &str, description: &str, props: Json, required: &[&str]) -> Json {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {"type": "object", "properties": props, "required": required, "additionalProperties": false},
        "outputSchema": envelope_schema(json!({})),
    })
}

/// Built-in generic tools listed beside the direct tools.
pub fn builtin_tools() -> Vec<Json> {
    let sid = json!({"type": "string", "description": "Session ID from sessions.open."});
    vec![
        builtin(
            "rivet.request",
            "Request an operation",
            "Generic dispatch of an input envelope {operation, data}: the operation's envelope for unary operations, an accepted envelope with a SessionReceipt for streaming ones.",
            envelope_input_props(json!({})),
            &[],
        ),
        builtin(
            "rivet.list",
            "List operations",
            "Authorized operation summaries.",
            json!({"cursor": {"type": "string"}, "limit": {"type": "integer"}, "outputs": {"type": "boolean"}}),
            &[],
        ),
        builtin(
            "rivet.describe",
            "Describe an operation",
            "Full descriptor: params, output, emits, receives, errors.",
            json!({"id": {"type": "string"}}),
            &["id"],
        ),
        builtin(
            "rivet.outputs",
            "Show declared outputs",
            "Output, emits, receives and errors JSON Schema for one ID or all.",
            json!({"id": {"type": "string"}, "all": {"type": "boolean"}}),
            &[],
        ),
        builtin(
            "rivet.sessions.open",
            "Open a session",
            "Open a live session for an input envelope {operation, data}; returns a SessionReceipt.",
            envelope_input_props(json!({"deadline_ms": {"type": "integer", "minimum": 1}})),
            &[],
        ),
        builtin(
            "rivet.sessions.send",
            "Send session input",
            "Enqueue one input item (send_seq starts at 1).",
            json!({"session_id": sid, "send_seq": {"type": "integer", "minimum": 1}, "data": {}}),
            &["session_id", "send_seq", "data"],
        ),
        builtin(
            "rivet.sessions.finish_input",
            "Finish session input",
            "Half-close the session's input.",
            json!({"session_id": sid}),
            &["session_id"],
        ),
        builtin(
            "rivet.sessions.read",
            "Read session events",
            "Events after after_seq (acknowledges earlier ones); waits up to wait_ms (max 5000).",
            json!({"session_id": sid, "after_seq": {"type": "integer", "minimum": 0}, "max_events": {"type": "integer"}, "wait_ms": {"type": "integer"}}),
            &["session_id"],
        ),
        builtin(
            "rivet.sessions.cancel",
            "Cancel a session",
            "Cancel the session and await cleanup.",
            json!({"session_id": sid}),
            &["session_id"],
        ),
        builtin(
            "rivet.io",
            "Show the I/O manifest",
            "Every effect site of the selected operations (targets, access, capability, policy decision). Reveals internal URLs and paths.",
            json!({
                "ids": {"type": "array", "items": {"type": "string"}},
                "all": {"type": "boolean"},
                "by": {"type": "string", "enum": ["operation", "target", "capability"]},
                "kind": {"type": "string"},
                "access": {"type": "array", "items": {"type": "string"}},
                "check_policy": {"type": "boolean"},
                "needs": {"type": "boolean"},
                "strict": {"type": "boolean"},
                "include_bootstrap": {"type": "boolean"},
                "trace": {"type": "string", "description": "Join the recorded attempts of this request ID."},
                "format": {"type": "string", "enum": ["json", "table", "markdown", "csv"]},
                "report": {"type": "boolean", "description": "Return the rendered IoReport instead of the bare manifest."}
            }),
            &[],
        ),
        builtin(
            "rivet.policy.generate",
            "Generate a policy draft",
            "Least-privilege policy.json draft for the selected operations; never writes files.",
            json!({"ids": {"type": "array", "items": {"type": "string"}}, "all": {"type": "boolean"}}),
            &[],
        ),
        builtin(
            "rivet.trace.show",
            "Show a request trace",
            "This host's recorded broker decisions and attempts for one request.",
            json!({"request_id": {"type": "string"}}),
            &["request_id"],
        ),
        builtin(
            "rivet.trace.export",
            "Export a request trace",
            "Write one request's recorded trace JSON to a new bundle-relative file (needs allow_write on the path; never overwrites).",
            json!({"request_id": {"type": "string"}, "path": {"type": "string", "description": "Bundle-relative path of the new trace file."}}),
            &["request_id", "path"],
        ),
        builtin(
            "rivet.capabilities",
            "Describe build capabilities",
            "Protocols, OAuth flows, sandbox backend, serve surfaces, refused Stage C forms and the version of this build.",
            json!({}),
            &[],
        ),
        builtin(
            "rivet.connectors.sync",
            "Sync an MCP connector snapshot",
            "Discover a connector's tools/resources/prompts and create a candidate snapshot file (never overwrites).",
            json!({"name": {"type": "string"}, "output": {"type": "string", "description": "Bundle-relative path of the new snapshot file."}}),
            &["name", "output"],
        ),
        builtin(
            "rivet.auth.begin",
            "Begin OAuth authorization",
            "Start an authorization transaction for a profile and account.",
            json!({"profile": {"type": "string"}, "account": {"type": "string"}}),
            &["profile", "account"],
        ),
        builtin(
            "rivet.auth.complete",
            "Complete OAuth authorization",
            "Finish a transaction with the redirect callback (or wait for a device grant).",
            json!({
                "transaction_id": {"type": "string"},
                "callback": {"type": "object", "properties": {
                    "code": {"type": "string"}, "state": {"type": "string"},
                    "redirect_uri": {"type": "string"}, "issuer": {"type": "string"},
                    "error": {"type": "string"}
                }},
                "wait": {"type": "boolean"}
            }),
            &["transaction_id"],
        ),
        builtin(
            "rivet.auth.status",
            "Show credential status",
            "Whether an account is connected (never returns token material).",
            json!({"profile": {"type": "string"}, "account": {"type": "string"}}),
            &["profile", "account"],
        ),
        builtin(
            "rivet.auth.disconnect",
            "Disconnect an account",
            "Revoke (where supported) and delete an account's stored credentials.",
            json!({"profile": {"type": "string"}, "account": {"type": "string"}}),
            &["profile", "account"],
        ),
        builtin(
            "rivet.auth.cancel",
            "Cancel an authorization",
            "Cancel a pending authorization transaction.",
            json!({"transaction_id": {"type": "string"}}),
            &["transaction_id"],
        ),
    ]
}

/// Tool result carrying one ResponseEnvelope: the text content is its JSON and
/// `structuredContent` the object itself.
pub fn tool_result(structured: Json, is_error: bool) -> Json {
    json!({
        "content": [{"type": "text", "text": structured.to_string()}],
        "structuredContent": structured,
        "isError": is_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_errors() {
        let m = parse_message(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .unwrap();
        assert!(m.is_request());
        let n =
            parse_message(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
        assert!(!n.is_request());
        assert_eq!(
            parse_message(b"{").unwrap_err()["error"]["code"],
            PARSE_ERROR
        );
        assert_eq!(
            parse_message(b"[]").unwrap_err()["error"]["code"],
            INVALID_REQUEST
        );
        let names: Vec<String> = builtin_tools()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names.len(), 20);
        for t in builtin_tools() {
            assert_eq!(t["inputSchema"]["type"], "object");
            assert!(t["outputSchema"].is_object());
        }
    }
}
