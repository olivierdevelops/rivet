//! Built-in reserved `rivet.*` operations, reachable through the shared
//! dispatcher from every surface (CLI `request`, `/v1/request`, MCP tools, WS,
//! library): catalog inspection, generic dispatch and session control.
//!
//! ```text
//!  rivet.list / describe / outputs ─▶ registry use cases, filtered by serve.authorize_operation
//!  rivet.request {id, params}       ─▶ unary: nested dispatch │ streaming: sessions.open receipt
//!  rivet.sessions.*                 ─▶ sessions use cases ─▶ SessionHost
//! ```

use super::runtime::Runtime;
use crate::domain::contracts::{Completion, Request};
use crate::domain::errors::EffectsStatus;
use crate::domain::ports::DataSink;
use crate::domain::serve::OperationAccess;
use crate::domain::sessions::{SessionOpenInput, SessionReadInput, SessionRef, SessionSendInput};
use crate::domain::{RivetError, RivetResult, Value};
use crate::features::serve::authorize_operation::{authorize_operation, require_operation};
use crate::features::sessions::cancel_session::cancel_session;
use crate::features::sessions::finish_input::finish_input;
use crate::features::sessions::open_session::open_session;
use crate::features::sessions::read_events::read_events;
use crate::features::sessions::send_input::send_input;
use futures_util::future::BoxFuture;
use serde_json::{Value as Json, json};
use std::sync::Arc;

/// Every built-in operation ID this build serves.
pub const BUILTIN_IDS: [&str; 11] = [
    "rivet.request",
    "rivet.io",
    "rivet.policy.generate",
    "rivet.list",
    "rivet.describe",
    "rivet.outputs",
    "rivet.sessions.open",
    "rivet.sessions.send",
    "rivet.sessions.finish_input",
    "rivet.sessions.read",
    "rivet.sessions.cancel",
];

/// Whether `principal` may see/call `id` (hidden IDs look like unknown ones).
pub fn visible(rt: &Runtime, principal: &crate::domain::contracts::Principal, id: &str) -> bool {
    authorize_operation(&OperationAccess {
        principal: principal.clone(),
        operation_id: id.to_string(),
        serve: rt.policy().serve.clone(),
    })
    .allowed
}

fn text(params: &Value, key: &str) -> RivetResult<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            RivetError::validation(
                "validation.required",
                format!("missing required parameter `{key}`"),
            )
            .with_details(Value::object([("field", Value::text(key))]))
        })
}

fn uint(params: &Value, key: &str) -> RivetResult<Option<u64>> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Int(i)) if *i >= 0 => Ok(Some(*i as u64)),
        Some(Value::Float(f)) if *f >= 0.0 && f.fract() == 0.0 => Ok(Some(*f as u64)),
        Some(other) => Err(RivetError::validation(
            "validation.type",
            format!(
                "parameter `{key}` must be a non-negative integer, got {}",
                other.type_name()
            ),
        )),
    }
}

fn hidden(id: &str) -> RivetError {
    RivetError::not_found("not_found.operation", format!("no operation `{id}`"))
}

/// Dispatch one `rivet.*` request; the outer request was already authorized.
pub fn dispatch_builtin<'a>(
    rt: &'a Runtime,
    req: Request,
    sink: Option<Arc<dyn DataSink>>,
) -> BoxFuture<'a, RivetResult<Completion>> {
    Box::pin(dispatch_builtin_inner(rt, req, sink))
}

async fn dispatch_builtin_inner(
    rt: &Runtime,
    req: Request,
    sink: Option<Arc<dyn DataSink>>,
) -> RivetResult<Completion> {
    let tag = |mut e: RivetError| {
        e.request_id.get_or_insert_with(|| req.request_id.clone());
        e.trace_id.get_or_insert_with(|| req.trace_id.clone());
        e.operation_id
            .get_or_insert_with(|| req.operation_id.clone());
        e
    };
    let done = |result: Json| Completion {
        request_id: req.request_id.clone(),
        trace_id: req.trace_id.clone(),
        result: Value::from_json(&result),
        data_count: 0,
        effects: EffectsStatus::None,
    };
    let p = &req.params;
    let who = req.principal.clone();
    let out: RivetResult<Completion> = async {
        match req.operation_id.as_str() {
            "rivet.list" => {
                let with_outputs = p.get("outputs").and_then(Value::as_bool).unwrap_or(false);
                let ops: Vec<Json> = rt
                    .list()?
                    .entries
                    .iter()
                    .filter(|e| visible(rt, &who, &e.id))
                    .map(|e| {
                        let mut j = e.summary_json();
                        if with_outputs {
                            j["output"] = e.output_schema();
                        }
                        j
                    })
                    .collect();
                Ok(done(json!({"operations": ops, "next_cursor": null})))
            }
            "rivet.io" => {
                // Inventories reveal internal URLs and paths: only an explicit
                // `rivet.io` listing (or the local principal) reaches this arm.
                let list = |k: &str| -> Vec<String> {
                    match p.get(k) {
                        Some(Value::List(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                        Some(Value::Text(s)) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                        _ => Vec::new(),
                    }
                };
                let flag = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
                let ids = list("ids");
                for id in &ids {
                    if !visible(rt, &who, id) {
                        return Err(hidden(id));
                    }
                }
                if flag("check_files") {
                    return Err(RivetError::validation(
                        "validation.check_files_remote",
                        "check_files probes the host's files and is available from the CLI and library only",
                    ));
                }
                let query = crate::domain::io_manifest::IoQuery {
                    ids,
                    all: flag("all"),
                    by: p.get("by").and_then(Value::as_str).unwrap_or("operation").to_string(),
                    kind: p.get("kind").and_then(Value::as_str).map(str::to_string),
                    access: list("access"),
                    format: "json".into(),
                    check_policy: flag("check_policy"),
                    needs: flag("needs"),
                    strict: flag("strict"),
                    include_bootstrap: flag("include_bootstrap"),
                    ..crate::domain::io_manifest::IoQuery::default()
                };
                let report = rt.io(&query)?;
                Ok(done(report.manifest.to_json()))
            }
            "rivet.policy.generate" => {
                let ids: Vec<String> = match p.get("ids") {
                    Some(Value::List(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                    _ => Vec::new(),
                };
                for id in &ids {
                    if !visible(rt, &who, id) {
                        return Err(hidden(id));
                    }
                }
                let all = p.get("all").and_then(Value::as_bool).unwrap_or(false);
                // Never writes files from a surface: the draft is returned.
                let draft = rt.generate_policy_draft(&ids, all, None)?;
                Ok(done(json!({
                    "policy": draft.policy_json(),
                    "review": draft.review.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
                    "complete": draft.complete,
                })))
            }
            "rivet.describe" => {
                let id = text(p, "id")?;
                if !visible(rt, &who, &id) {
                    return Err(hidden(&id));
                }
                let c = rt.describe(std::slice::from_ref(&id))?;
                Ok(done(c.entries[0].describe_json()))
            }
            "rivet.outputs" => {
                let all = p.get("all").and_then(Value::as_bool).unwrap_or(false);
                let id = p.get("id").and_then(Value::as_str).map(str::to_string);
                if let Some(id) = &id
                    && !visible(rt, &who, id)
                {
                    return Err(hidden(id));
                }
                let reports = rt.outputs(id.as_deref(), all || id.is_none())?;
                let items: Vec<Json> = reports
                    .iter()
                    .filter(|r| visible(rt, &who, &r.id))
                    .map(|r| r.to_json())
                    .collect();
                Ok(done(if id.is_some() && items.len() == 1 {
                    items[0].clone()
                } else {
                    Json::Array(items)
                }))
            }
            "rivet.request" => {
                let id = text(p, "id")?;
                let params = p.get("params").cloned().unwrap_or(Value::Null);
                if id.starts_with("rivet.") && id != "rivet.request" {
                    let inner = rt
                        .request_as(who.clone(), &id, params, sink.clone())
                        .await?;
                    return Ok(Completion {
                        request_id: req.request_id.clone(),
                        trace_id: req.trace_id.clone(),
                        ..inner
                    });
                }
                require_operation(&OperationAccess {
                    principal: who.clone(),
                    operation_id: id.clone(),
                    serve: rt.policy().serve.clone(),
                })?;
                let entry = rt.describe(std::slice::from_ref(&id))?.entries.remove(0);
                if entry.streaming() {
                    let r = open_session(
                        SessionOpenInput {
                            id,
                            params,
                            principal: who.clone(),
                            connection_owned: false,
                            deadline_ms: None,
                        },
                        rt.sessions().as_ref(),
                    )
                    .await?;
                    return Ok(done(r.to_json()));
                }
                let inner = rt
                    .request_as(who.clone(), &id, params, sink.clone())
                    .await?;
                Ok(Completion {
                    request_id: req.request_id.clone(),
                    trace_id: req.trace_id.clone(),
                    ..inner
                })
            }
            "rivet.sessions.open" => {
                let id = text(p, "id")?;
                require_operation(&OperationAccess {
                    principal: who.clone(),
                    operation_id: id.clone(),
                    serve: rt.policy().serve.clone(),
                })?;
                let r = open_session(
                    SessionOpenInput {
                        id,
                        params: p.get("params").cloned().unwrap_or(Value::Null),
                        principal: who.clone(),
                        connection_owned: false,
                        deadline_ms: None,
                    },
                    rt.sessions().as_ref(),
                )
                .await?;
                Ok(done(r.to_json()))
            }
            "rivet.sessions.send" => {
                let a = send_input(
                    SessionSendInput {
                        session_id: text(p, "session_id")?,
                        send_seq: uint(p, "send_seq")?.unwrap_or(0),
                        data: p.get("data").cloned().unwrap_or(Value::Null),
                        principal: who.clone(),
                    },
                    rt.sessions().as_ref(),
                )
                .await?;
                Ok(done(a.to_json()))
            }
            "rivet.sessions.finish_input" => {
                let a = finish_input(
                    SessionRef {
                        session_id: text(p, "session_id")?,
                        principal: who.clone(),
                    },
                    rt.sessions().as_ref(),
                )
                .await?;
                Ok(done(a.to_json()))
            }
            "rivet.sessions.read" => {
                let b = read_events(
                    SessionReadInput {
                        session_id: text(p, "session_id")?,
                        after_seq: uint(p, "after_seq")?.unwrap_or(0),
                        max_events: uint(p, "max_events")?.map(|v| v.min(u32::MAX as u64) as u32),
                        wait_ms: uint(p, "wait_ms")?.map(|v| v.min(u32::MAX as u64) as u32),
                        principal: who.clone(),
                    },
                    rt.sessions().as_ref(),
                )
                .await?;
                Ok(done(b.to_json()))
            }
            "rivet.sessions.cancel" => {
                let c = cancel_session(
                    SessionRef {
                        session_id: text(p, "session_id")?,
                        principal: who.clone(),
                    },
                    rt.sessions().as_ref(),
                )
                .await?;
                Ok(done(c.to_json()))
            }
            other => Err(hidden(other)),
        }
    }
    .await;
    out.map_err(tag)
}
