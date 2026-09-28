//! `rivet --endpoint URL …`: the CLI as a thin client of a running
//! `rivet serve`, plus the `--input-jsonl -` stdin feeder shared with local
//! runs. Every command maps onto the server's public surfaces, so a remote
//! run prints the same Completion / tables / ErrorEnvelope and exits with the
//! same registry code as a local one.
//!
//! ```text
//!  request ID [--stream] ─────────▶ POST /v1/request (SSE with --stream)
//!  request ID --input-jsonl - --stream
//!       stdin JSONL ─validate─▶ input frames ─▶ GET /v1/ws ─▶ data frames ─▶ NDJSON stdout
//!       EOF ─▶ finish_input · malformed line / Ctrl-C ─▶ cancel
//!  auth … / trace show REQ / connectors sync ─▶ POST /v1/request rivet.auth.* / rivet.trace.show / rivet.connectors.sync
//!  list / describe / outputs ────▶ GET /v1/operations[/{id}[/outputs]] (rivet.list / rivet.outputs for --outputs / --all)
//!  io [flags] [--trace REQ] ─────▶ GET /v1/io?…&format=F  (rendered IoReport; exit code from the server)
//! ```

use super::setup_cli::{NdjsonSink, auth_request, fail, parse_params};
use crate::domain::contracts::{Catalog, Envelope, RegistryEntry};
use crate::domain::ir::parse_duration_ms;
use crate::domain::outputs::ValueSpec;
use crate::domain::ports::{DataSink, RemoteCall, RemoteEndpoint};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use crate::io::cli::{
    Cli, Command, ConnectorsCommand, RequestArgs, TraceCommand, render_describe, render_list,
    render_outputs,
};
use serde_json::{Value as Json, json};
use std::io::Write;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;

/// Items the stdin feeder may queue ahead of the consumer (backpressure).
pub const INPUT_QUEUE: usize = 16;

fn usage(msg: impl Into<String>) -> RivetError {
    RivetError::validation("validation.usage", msg)
}

/// `--timeout D` as milliseconds, local and `--endpoint` alike: above the
/// host cap (the HTTP `deadline_ms` cap, 600000 ms) is a usage error (G19).
pub fn timeout_ms(t: &Option<String>) -> RivetResult<Option<u64>> {
    let cap = crate::io::http::MAX_REQUEST_DEADLINE_MS;
    match t {
        None => Ok(None),
        Some(t) => match parse_duration_ms(t) {
            None => Err(usage(format!(
                "--timeout {t}: use digits plus ms, s, m or h"
            ))),
            Some(ms) if ms > cap => Err(usage(format!(
                "--timeout {t} is {ms} ms; the host cap is {cap} ms (10m)"
            ))
            .with_details(Value::object([
                ("timeout_ms", Value::Int(ms as i64)),
                ("max_ms", Value::Int(cap as i64)),
            ]))),
            Some(ms) => Ok(Some(ms)),
        },
    }
}

/// `--input-jsonl SRC --stream` preconditions: only `-` (stdin) is an input
/// channel; other paths would be file reads the CLI does not broker.
pub fn check_input_flags(args: &RequestArgs) -> RivetResult<bool> {
    match args.input_jsonl.as_deref() {
        None => Ok(false),
        Some("-") if args.stream => Ok(true),
        Some("-") => Err(usage(
            "--input-jsonl - needs --stream (output is NDJSON envelopes)",
        )),
        Some(other) => Err(usage(format!(
            "--input-jsonl {other}: only `-` (stdin) is supported"
        ))),
    }
}

/// The operation must declare `receives` to accept live input.
pub fn receives_of(entry: &RegistryEntry) -> RivetResult<ValueSpec> {
    entry.receives.clone().ok_or_else(|| {
        RivetError::validation(
            "validation.no_input",
            format!(
                "`{}` does not declare `receives`; drop --input-jsonl",
                entry.id
            ),
        )
    })
}

/// Read JSON Lines from stdin and enqueue each item as it arrives (the
/// bounded channel applies backpressure; nothing is buffered up front).
/// Blank lines are skipped; EOF drops the sender (finish_input). A line that
/// is not JSON or does not match `receives` is `validation.input`: it is sent
/// on `failed` while the input stays open (so the run cannot finish
/// normally first), and the caller cancels the request. Line content is never
/// echoed.
pub async fn feed_stdin_jsonl(
    tx: tokio::sync::mpsc::Sender<Value>,
    receives: ValueSpec,
    failed: tokio::sync::oneshot::Sender<RivetError>,
) {
    if let Err(e) = read_stdin_jsonl(&tx, &receives).await {
        let _ = failed.send(e);
        // Keep `tx` (the input) open until the caller cancels and aborts us.
        std::future::pending::<()>().await;
    }
}

async fn read_stdin_jsonl(
    tx: &tokio::sync::mpsc::Sender<Value>,
    receives: &ValueSpec,
) -> RivetResult<()> {
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut n = 0u64;
    let mut seq = 0u64;
    loop {
        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) => return Ok(()),
            Err(e) => {
                return Err(RivetError::validation(
                    "validation.input",
                    format!("stdin is not readable UTF-8 text: {}", e.kind()),
                ));
            }
        };
        n += 1;
        if line.trim().is_empty() {
            continue;
        }
        seq += 1;
        let at = Value::object([
            ("seq", Value::Int(seq as i64)),
            ("line", Value::Int(n as i64)),
        ]);
        let j: Json = serde_json::from_str(&line).map_err(|e| {
            RivetError::validation(
                "validation.input",
                format!(
                    "stdin line {n} is not JSON (column {}); the request was cancelled",
                    e.column()
                ),
            )
            .with_details(at.clone())
        })?;
        let v = Value::from_json(&j);
        let mut violations = Vec::new();
        receives.check(&v, "", &mut violations);
        if let Some(x) = violations.first() {
            return Err(RivetError::validation(
                "validation.input",
                format!(
                    "stdin line {n}: input item at {} must be {}, got {}; the request was cancelled",
                    if x.path.is_empty() { "$" } else { &x.path },
                    x.expected,
                    x.found
                ),
            )
            .with_details(at));
        }
        if tx.send(v).await.is_err() {
            // The request already ended; stop reading.
            return Ok(());
        }
    }
}

fn cancelled() -> RivetError {
    RivetError::new(
        ErrorKind::Cancelled,
        "cancelled.request",
        "the request was cancelled by its caller",
    )
}

/// Percent-encode one path segment (operation IDs are dotted words).
fn seg(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn protocol(msg: &str) -> RivetError {
    RivetError::new(ErrorKind::Protocol, "protocol.endpoint", msg.to_string())
}

fn entry_of(j: &Json) -> RivetResult<RegistryEntry> {
    RegistryEntry::from_json(j).ok_or_else(|| protocol("the server sent a malformed descriptor"))
}

async fn ids_in_order(client: &dyn RemoteEndpoint) -> RivetResult<Vec<String>> {
    let j = client.get("/v1/operations").await?;
    Ok(j.get("operations")
        .and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|o| o.get("id").and_then(Json::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

async fn builtin(
    client: &dyn RemoteEndpoint,
    id: &str,
    params: Json,
    deadline_ms: Option<u64>,
) -> RivetResult<Json> {
    let c = client
        .request(RemoteCall {
            id: id.to_string(),
            params: Value::from_json(&params),
            deadline_ms,
        })
        .await
        .map_err(|mut e| {
            // The CLI command, not the built-in it maps to, is what the user ran.
            if e.operation_id.as_deref() == Some(id) {
                e.operation_id = None;
            }
            e
        })?;
    Ok(c.result.to_json())
}

/// Run one CLI command against the server; returns the process exit code.
pub async fn run_remote(cli: &Cli, client: &dyn RemoteEndpoint) -> i32 {
    let mut stdout = std::io::stdout();
    match &cli.command {
        Command::Request(args) => request(args, client).await,
        Command::Auth { command } => {
            let (id, params, timeout) = match auth_request(command) {
                Ok(v) => v,
                Err(e) => return fail(&e, None, true),
            };
            let deadline_ms = match timeout_ms(&timeout) {
                Ok(d) => d,
                Err(e) => return fail(&e, None, true),
            };
            let call = RemoteCall {
                id: id.to_string(),
                params,
                deadline_ms,
            };
            match client.request(call).await {
                Ok(c) => {
                    let _ = writeln!(stdout, "{}", c.to_json());
                    0
                }
                Err(e) => fail(&e, None, true),
            }
        }
        Command::Trace {
            command: TraceCommand::Show { request_id },
        } => match builtin(
            client,
            "rivet.trace.show",
            json!({"request_id": request_id}),
            None,
        )
        .await
        {
            Ok(t) => {
                let _ = writeln!(stdout, "{t}");
                0
            }
            Err(e) => fail(&e, None, true),
        },
        Command::Connectors {
            command: ConnectorsCommand::Sync { name, output },
        } => match builtin(
            client,
            "rivet.connectors.sync",
            json!({"name": name, "output": output}),
            None,
        )
        .await
        {
            Ok(r) => {
                let _ = writeln!(stdout, "{r}");
                let sha = r.get("sha256").and_then(Json::as_str).unwrap_or("");
                eprintln!(
                    "wrote candidate snapshot {output} ({sha}); after review, approve it in policy.json: \"approved\": {{\"snapshots\": [\"{sha}\"]}}"
                );
                0
            }
            Err(e) => fail(&e, None, true),
        },
        Command::List { outputs } => match list(cli.json, *outputs, client).await {
            Ok(text) => {
                let _ = write!(stdout, "{text}");
                0
            }
            Err(e) => fail(&e, None, cli.json),
        },
        Command::Describe { ids } => match describe(cli.json, ids, client).await {
            Ok(text) => {
                let _ = write!(stdout, "{text}");
                0
            }
            Err(e) => fail(&e, None, cli.json),
        },
        Command::Outputs { id, all } => {
            match outputs(cli.json, id.as_deref(), *all, client).await {
                Ok(text) => {
                    let _ = write!(stdout, "{text}");
                    0
                }
                Err(e) => fail(&e, None, cli.json),
            }
        }
        Command::Io(args) => {
            let q = args.to_query(cli.json);
            let mut pairs: Vec<(&str, String)> = vec![
                ("by", q.by.clone()),
                ("format", q.format.clone()),
                // The rendered report (not the bare manifest) for every format.
                ("report", "true".into()),
            ];
            if !q.ids.is_empty() {
                pairs.push(("ids", q.ids.join(",")));
            }
            if !q.access.is_empty() {
                pairs.push(("access", q.access.join(",")));
            }
            if let Some(k) = &q.kind {
                pairs.push(("kind", k.clone()));
            }
            if let Some(t) = &q.trace_request_id {
                pairs.push(("trace", t.clone()));
            }
            for (k, on) in [
                ("all", q.all),
                ("check_policy", q.check_policy),
                ("needs", q.needs),
                ("strict", q.strict),
                ("include_bootstrap", q.include_bootstrap),
                ("check_files", q.check_files),
            ] {
                if on {
                    pairs.push((k, "true".into()));
                }
            }
            let query: String = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(pairs)
                .finish();
            match client.get(&format!("/v1/io?{query}")).await {
                Ok(r) => {
                    let _ = write!(
                        stdout,
                        "{}",
                        r.get("rendered").and_then(Json::as_str).unwrap_or("")
                    );
                    let diag = r.get("diagnostics").and_then(Json::as_str).unwrap_or("");
                    if !diag.is_empty() {
                        eprint!("{diag}");
                    }
                    r.get("exit_code").and_then(Json::as_i64).unwrap_or(0) as i32
                }
                Err(e) => fail(&e, None, cli.json),
            }
        }
        Command::Check { .. }
        | Command::Graph { .. }
        | Command::Policy { .. }
        | Command::Serve(_) => fail(
            &usage(
                "check, graph, policy and serve work on a local bundle (--file); they are not available with --endpoint",
            ),
            None,
            cli.json,
        ),
    }
}

async fn request(args: &RequestArgs, client: &dyn RemoteEndpoint) -> i32 {
    let params = match parse_params(&args.params) {
        Ok(p) => p,
        Err(e) => return fail(&e, None, true),
    };
    let (deadline_ms, live) = match (timeout_ms(&args.timeout), check_input_flags(args)) {
        (Ok(d), Ok(l)) => (d, l),
        (Err(e), _) | (_, Err(e)) => return fail(&e, None, true),
    };
    let call = RemoteCall {
        id: args.id.clone(),
        params,
        deadline_ms,
    };
    let outcome = if live {
        duplex(call, client).await
    } else {
        // Ctrl-C drops the exchange: the server cancels a request whose
        // client disconnected (SSE) and the CLI reports `cancelled` (130).
        let run = async {
            if args.stream {
                client.request_stream(call, Arc::new(NdjsonSink)).await
            } else {
                client.request(call).await
            }
        };
        tokio::select! {
            r = run => r,
            _ = tokio::signal::ctrl_c() => Err(cancelled()),
        }
    };
    match outcome {
        Ok(c) => {
            let line = if args.stream {
                Envelope::Result(c).to_json()
            } else {
                c.to_json()
            };
            let _ = writeln!(std::io::stdout(), "{line}");
            0
        }
        Err(e) => fail(&e, None, true),
    }
}

/// `--input-jsonl - --stream` over `/v1/ws`: the stdin feeder and the frame
/// reader run concurrently; a malformed line or Ctrl-C sends `cancel`.
async fn duplex(
    call: RemoteCall,
    client: &dyn RemoteEndpoint,
) -> RivetResult<crate::domain::contracts::Completion> {
    let described = client
        .get(&format!("/v1/operations/{}", seg(&call.id)))
        .await?;
    let receives = receives_of(&entry_of(&described)?)?;
    let (tx, rx) = tokio::sync::mpsc::channel::<Value>(INPUT_QUEUE);
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let mut cancel_tx = Some(cancel_tx);
    let (failed_tx, mut failed_rx) = tokio::sync::oneshot::channel::<RivetError>();
    let feeder = tokio::spawn(feed_stdin_jsonl(tx, receives, failed_tx));
    let sink: Arc<dyn DataSink> = Arc::new(NdjsonSink);
    let run = client.duplex(call, rx, cancel_rx, sink);
    tokio::pin!(run);
    let mut watching = true;
    let mut input_error: Option<RivetError> = None;
    let outcome = loop {
        tokio::select! {
            r = &mut run => break r,
            e = &mut failed_rx, if watching => {
                watching = false;
                if let Ok(e) = e {
                    input_error = Some(e);
                    if let Some(c) = cancel_tx.take() {
                        let _ = c.send(());
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                if let Some(c) = cancel_tx.take() {
                    let _ = c.send(());
                }
            }
        }
    };
    feeder.abort();
    match input_error.or_else(|| failed_rx.try_recv().ok()) {
        Some(e) => Err(e),
        None => outcome,
    }
}

async fn list(
    json_out: bool,
    with_outputs: bool,
    client: &dyn RemoteEndpoint,
) -> RivetResult<String> {
    if json_out {
        return Ok(format!("{}\n", client.get("/v1/operations").await?));
    }
    let j = if with_outputs {
        builtin(client, "rivet.list", json!({"outputs": true}), None).await?
    } else {
        client.get("/v1/operations").await?
    };
    let entries = j
        .get("operations")
        .and_then(Json::as_array)
        .map(|a| a.iter().map(entry_of).collect::<RivetResult<Vec<_>>>())
        .transpose()?
        .unwrap_or_default();
    Ok(render_list(&Catalog { entries }, with_outputs))
}

async fn describe(
    json_out: bool,
    ids: &[String],
    client: &dyn RemoteEndpoint,
) -> RivetResult<String> {
    let order = ids_in_order(client).await?;
    let wanted: Vec<String> = if ids.is_empty() {
        order.clone()
    } else {
        ids.to_vec()
    };
    let mut found: Vec<(String, Json)> = Vec::new();
    for id in &wanted {
        let j = client.get(&format!("/v1/operations/{}", seg(id))).await?;
        found.push((id.clone(), j));
    }
    // Same order as a local run: catalog order, not argv order.
    found.sort_by_key(|(id, _)| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
    if json_out {
        let items: Vec<Json> = found.into_iter().map(|(_, j)| j).collect();
        let out = if items.len() == 1 {
            items[0].clone()
        } else {
            Json::Array(items)
        };
        return Ok(format!("{out}\n"));
    }
    let mut text = String::new();
    for (i, (_, j)) in found.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        text.push_str(&render_describe(&entry_of(j)?));
    }
    Ok(text)
}

async fn outputs(
    json_out: bool,
    id: Option<&str>,
    all: bool,
    client: &dyn RemoteEndpoint,
) -> RivetResult<String> {
    if id.is_some() == all {
        return Err(RivetError::validation(
            "validation.query",
            "pass exactly one of an operation ID or --all",
        ));
    }
    if json_out {
        let j = match id {
            Some(id) => {
                client
                    .get(&format!("/v1/operations/{}/outputs", seg(id)))
                    .await?
            }
            None => builtin(client, "rivet.outputs", json!({"all": true}), None).await?,
        };
        return Ok(format!("{j}\n"));
    }
    let ids: Vec<String> = match id {
        Some(id) => vec![id.to_string()],
        None => ids_in_order(client).await?,
    };
    let mut reports = Vec::new();
    for id in &ids {
        let j = client.get(&format!("/v1/operations/{}", seg(id))).await?;
        reports.push(entry_of(&j)?.output_report());
    }
    reports.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(render_outputs(&reports))
}
