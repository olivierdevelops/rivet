//! CLI surface registration: maps each command to the shared use cases.

// vhco:surface cli kind cli calls language/compile_program, language/compile_globals, language/resolve_imports, registry/describe_operations, registry/inspect_outputs, execution/request_operation, execution/cancel_request, policy/load_policy, serve/start_serve, serve/parse_input, audit/inspect_effects, audit/build_graph, audit/read_trace, policy/generate_policy, connectors/invoke_mcp, auth/begin_authorization, auth/complete_authorization, auth/credential_status, auth/disconnect_account, auth/cancel_authorization, language/highlight_source
// vhco:trigger cli auth/begin_authorization = rivet auth begin PROFILE --account ACCOUNT | rivet request rivet.auth.begin --data JSON
// vhco:trigger cli auth/complete_authorization = rivet auth complete --params-file PATH [--timeout D] | rivet auth complete --params JSON
// vhco:trigger cli auth/credential_status = rivet auth status PROFILE --account ACCOUNT
// vhco:trigger cli auth/disconnect_account = rivet auth disconnect PROFILE --account ACCOUNT
// vhco:trigger cli auth/cancel_authorization = rivet auth cancel TRANSACTION_ID
// vhco:api cli auth/begin_authorization rivet auth begin PROFILE --account ACCOUNT -- start a code (PKCE S256) or device transaction; stdout is the ResponseEnvelope (operation rivet.auth.begin) whose data is the AuthChallenge (no verifier, state secret or token)
// vhco:request { "profile": "string", "account": "string" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.auth.begin", "type": "result", "status": "ok", "data": "{transaction_id, authorization_url?, verification_uri?, user_code?, expires_at, interval_seconds?}", "error": "null", "effects": "none|committed", "data_count": "0" }
// vhco:api cli auth/complete_authorization rivet auth complete --params-file PATH [--timeout D] -- finish a transaction; envelope whose data is the connected CredentialStatus, or {"state":"pending"} when the deadline came first (transaction kept)
// vhco:request { "transaction_id": "string", "callback": "{code, state, redirect_uri, issuer?}?", "wait": "bool" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.auth.complete", "type": "result", "status": "ok", "data": "{profile, account, state: connected|pending, scopes, expires_at, generation}", "error": "null", "effects": "none|committed", "data_count": "0" }
// vhco:trigger cli execution/request_operation = rivet request ID --data JSON [--stream] [--input-jsonl - --stream] | rivet request --input FILE|- | rivet --endpoint URL [--token-file PATH] request ID …
// vhco:trigger cli serve/parse_input = rivet request --input FILE|- | rivet request ID --data JSON (--params: deprecated alias, warning[deprecated.params])
// vhco:trigger cli execution/cancel_request = Ctrl-C during rivet request
// vhco:trigger cli registry/describe_operations = rivet list | rivet describe ID | rivet --endpoint URL list | describe ID
// vhco:trigger cli registry/inspect_outputs = rivet outputs ID | rivet outputs --all
// vhco:trigger cli language/compile_program = rivet check [--strict-docs]
// vhco:trigger cli language/compile_globals = rivet check (globals are evaluated when the bundle loads)
// vhco:trigger cli language/resolve_imports = rivet check (every import is resolved when the bundle loads)
// vhco:trigger cli language/highlight_source = rivet highlight FILE [--format ansi|html|json]
// vhco:api cli language/highlight_source rivet highlight FILE [--format ansi|html|json] -- syntax-highlight one .rivet file from the parser's spans (reads only FILE; no bundle, policy or --file); stdout: ANSI colours (default on a terminal), an HTML <pre> with <span class="rv-CLASS">, or JSON lines (default when piped); a syntax error prints the tokens before it on stdout and the diagnostic on stderr (exit 2); an unreadable FILE is validation.usage (exit 2)
// vhco:request { "path": "string — the .rivet file", "format": "ansi|html|json — default ansi on a TTY, json otherwise" }
// vhco:response { "line": "int — 1-based", "col": "int — 1-based, characters", "len": "int — characters", "class": "keyword|option|type|effect|string|interpolation|number|comment|operation_id|global|variable|operator", "text": "string — exact source text" }
// vhco:trigger cli policy/load_policy = rivet policy explain
// vhco:trigger cli serve/start_serve = rivet serve [--listen HOST:PORT] | rivet serve --stdio
// vhco:trigger cli audit/inspect_effects = rivet io [ID ...] [--all] [--by operation|target|capability] [--kind K] [--access V,V] [--format table|json|markdown|csv] [--check-policy] [--strict] [--trace REQ] [--needs] [--check-files] [--include-bootstrap]
// vhco:trigger cli audit/build_graph = rivet graph ID [--all] [--json]
// vhco:trigger cli audit/read_trace = rivet trace show REQ | rivet trace export REQ --output PATH | rivet --endpoint URL trace show|export … (rivet.trace.show / rivet.trace.export)
// vhco:trigger cli policy/generate_policy = rivet policy generate [ID ...|--all] [--output PATH]
// vhco:trigger cli connectors/invoke_mcp = rivet connectors sync NAME --output PATH | rivet request CONNECTOR.tools.NAME --data JSON
// vhco:api cli connectors/invoke_mcp rivet connectors sync NAME --output PATH -- authorized discovery (allow_mcp NAME/discover + transport grants + allow_write PATH) writing a NEW candidate snapshot; prints an envelope (operation rivet.connectors.sync) whose data is the receipt; the sha256 to approve in policy.json approved.snapshots goes to stderr; exit 0, 3 denied, 4 output exists
// vhco:request { "name": "string — mcp connector", "output": "string — new snapshot path inside the bundle" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.connectors.sync", "type": "result", "status": "ok", "data": "{connector, path, sha256, protocolVersion, tools, resources, prompts}", "error": "null", "effects": "committed", "data_count": "0" }
// vhco:api cli audit/inspect_effects rivet io [ID ...] [flags] -- the I/O manifest; stdout in the requested format (json / --json: an envelope, operation rivet.io, whose data is the manifest), summaries on stderr; exit 0, 3 (denied/partial or not_permitted/unreadable), 4 (missing needed file / unknown id), 7 (--strict and incomplete), 2 (usage)
// vhco:request { "ids": "string[]", "all": "bool", "by": "operation|target|capability", "kind": "string?", "access": "string[]", "format": "table|json|markdown|csv", "check_policy": "bool", "strict": "bool", "needs": "bool", "check_files": "bool", "trace": "string?" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "rivet.io", "type": "result", "status": "ok", "data": "{bundle: FileDigest, policy: FileDigest?, complete: bool, sites: EffectSite[], targets: TargetSummary[], needs: OperationNeeds[], bootstrap: EffectSite[]}", "error": "null", "effects": "none", "data_count": "0" }
// vhco:api cli policy/generate_policy rivet policy generate [ID ...|--all] [--output PATH] -- least-privilege policy.json draft on stdout (or a new file); with --json an envelope (operation rivet.policy.generate) whose data is {policy, review, complete}; review items on stderr; exit 7 when review items exist, 4 conflict.exists
// vhco:request { "ids": "string[]", "all": "bool", "output": "string?" }
// vhco:response { "version": "1", "grants": "Grant[]", "network": "{deny_private_ranges: true}" }
// vhco:api cli execution/request_operation rivet request ID --data JSON | rivet request --input FILE|- -- invoke one operation; stdout is the ResponseEnvelope (compact, or 2-space indented with --pretty); errors are the same envelope with status error on stderr and the registry exit code; --stream prints NDJSON records (type data … then one type result) and refuses --pretty (validation.usage); --params is a deprecated alias of --data (warning[deprecated.params] on stderr)
// vhco:request { "operation": "string — operation ID (positional ID or the --input envelope)", "data": "JSON object (--data; deprecated --params)", "input": "FILE|- — a whole input envelope {operation, data, deadline_ms?, restrict?, stream?}", "pretty": "bool", "stream": "bool" }
// vhco:response { "request_id": "string", "trace_id": "string", "operation": "string", "type": "result", "status": "ok|error|cancelled", "data": "Value|null", "error": "{kind, code, message, retryable, …}|null", "effects": "none|committed|partial|unknown", "data_count": "int" }

use crate::domain::envelope::{InputEnvelope, OutputFormat, RawInput, ResponseEnvelope};
use crate::domain::io_manifest::IoQuery;
use crate::domain::{RivetError, Value};
use crate::features::language::lowering::lower::strict_doc_findings;
use crate::features::serve::parse_input::parse_input;
use crate::io::cli::{
    AuthCommand, Cli, Command, ConnectorsCommand, PolicyCommand, RequestArgs, TraceCommand,
    render_describe, render_list, render_outputs, render_policy, render_policy_review,
};
use crate::orchestrator::runtime::{Runtime, RuntimeBuilder};
use clap::Parser;
use std::io::Write;

/// The CLI's JSON format (`--pretty`), fixed once per process.
static FORMAT: std::sync::OnceLock<OutputFormat> = std::sync::OnceLock::new();

/// Compact unless `--pretty` was given.
pub(super) fn format() -> OutputFormat {
    FORMAT.get().copied().unwrap_or_default()
}

/// Print one envelope on stdout in the CLI format.
pub(super) fn print_envelope(env: &ResponseEnvelope) {
    let _ = writeln!(std::io::stdout(), "{}", env.render(format()));
}

/// An envelope for a payload the CLI produced itself (list, describe, io, …):
/// IDs are minted like a request's so every JSON output carries them.
fn payload(runtime: &Runtime, operation: &str, data: serde_json::Value) -> ResponseEnvelope {
    let req = runtime.new_request(
        operation,
        Value::Object(Vec::new()),
        crate::domain::contracts::Principal::local(),
    );
    ResponseEnvelope::payload(&req.request_id, &req.trace_id, operation, data)
}

/// Entry point for the `rivet` binary; returns the process exit code.
pub fn main() -> i32 {
    let cli = Cli::parse();
    let _ = FORMAT.set(OutputFormat::pretty(cli.pretty));
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: cannot start the async runtime: {e}");
            return 5;
        }
    };
    rt.block_on(run(cli))
}

pub(super) fn fail(e: &RivetError, source: Option<&str>, json: bool) -> i32 {
    fail_as(None, e, source, json)
}

/// Report a failure: with JSON output an error envelope (naming `operation`
/// when known) on stderr, otherwise the rendered diagnostic; returns the
/// registry exit code.
pub(super) fn fail_as(
    operation: Option<&str>,
    e: &RivetError,
    source: Option<&str>,
    json: bool,
) -> i32 {
    if json {
        eprintln!(
            "{}",
            ResponseEnvelope::from_error(operation, e).render(format())
        );
    } else {
        eprintln!("{}", e.render(own_source(e).as_deref().or(source)));
        for s in e.suppressed.iter().take(20) {
            eprintln!("{}", s.render(own_source(s).as_deref().or(source)));
        }
        if e.suppressed.len() > 20 {
            eprintln!("… and {} more", e.suppressed.len() - 20);
        }
    }
    e.exit_code()
}

/// The text of the file a diagnostic points into (an imported module's own
/// source rather than the entry file's).
pub(super) fn own_source(e: &RivetError) -> Option<String> {
    e.source
        .as_ref()
        .and_then(|s| std::fs::read_to_string(&s.file).ok())
}

fn load(cli: &Cli) -> Result<Runtime, (RivetError, Option<String>)> {
    let file = cli.file.clone().ok_or_else(|| {
        (
            RivetError::validation(
                "validation.usage",
                "--file PATH is required (the entry .rivet file)",
            ),
            None,
        )
    })?;
    let source = std::fs::read_to_string(&file).ok();
    let mut b: RuntimeBuilder = Runtime::builder().file(&file);
    if let Some(p) = &cli.policy {
        b = b.policy_file(p);
    }
    if matches!(cli.command, Command::Connectors { .. }) {
        // Refreshing a snapshot must work before one exists or is approved.
        b = b.connector_discovery();
    }
    b.build().map_err(|e| (e, source))
}

pub(super) fn parse_params(text: &str) -> Result<Value, RivetError> {
    parse_json_flag("--params", text)
}

fn parse_json_flag(flag: &str, text: &str) -> Result<Value, RivetError> {
    serde_json::from_str::<serde_json::Value>(text)
        .map(|j| Value::from_json(&j))
        .map_err(|e| {
            RivetError::validation(
                "validation.params",
                format!("{flag} is not valid JSON: {e}"),
            )
        })
}

/// `rivet request [ID] [--data JSON | --params JSON | --input FILE|-]` → the
/// one InputEnvelope (`serve.parse_input` for `--input`). Deprecated spellings
/// print `warning[deprecated.params]` / `warning[deprecated.input]` on stderr.
pub(super) fn resolve_input(args: &RequestArgs) -> Result<InputEnvelope, RivetError> {
    let usage = |m: &str| RivetError::validation("validation.usage", m.to_string());
    if args.data.is_some() && args.params.is_some() {
        return Err(usage("use --data (or the deprecated --params), not both"));
    }
    if let Some(src) = &args.input {
        if args.data.is_some() || args.params.is_some() {
            return Err(usage(
                "--input carries the whole envelope; drop --data/--params",
            ));
        }
        if src == "-" && args.input_jsonl.as_deref() == Some("-") {
            return Err(usage(
                "--input - and --input-jsonl - both read stdin; pass the envelope with --input FILE",
            ));
        }
        let text = if src == "-" {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).map_err(|e| {
                usage(&format!(
                    "--input -: stdin is not readable UTF-8 text: {}",
                    e.kind()
                ))
            })?;
            s
        } else {
            std::fs::read_to_string(src)
                .map_err(|e| usage(&format!("--input {src}: {}", e.kind())))?
        };
        let body = serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
            RivetError::validation(
                crate::domain::envelope::INPUT_ENVELOPE,
                format!("--input {src} is not valid JSON: {e}"),
            )
        })?;
        let input = parse_input(RawInput::new(body))?;
        if let Some(id) = &args.id
            && id != &input.operation
        {
            return Err(usage(&format!(
                "the operation is named twice: `{id}` on the command line and `{}` in --input",
                input.operation
            )));
        }
        if input.is_legacy() {
            eprintln!(
                "warning[deprecated.input]: input keys {} are deprecated; use `operation` and `data` (removed in 0.3.0)",
                input
                    .aliases
                    .iter()
                    .map(|a| format!("`{a}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            );
        }
        return Ok(input);
    }
    let id = args.id.clone().ok_or_else(|| {
        usage("rivet request needs an operation ID (or --input FILE|- with an envelope)")
    })?;
    let data = match (&args.data, &args.params) {
        (Some(d), _) => parse_json_flag("--data", d)?,
        (None, Some(p)) => {
            eprintln!(
                "warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)"
            );
            parse_params(p)?
        }
        (None, None) => Value::Object(Vec::new()),
    };
    let mut input = InputEnvelope::new(&id).data_value(data);
    if args.params.is_some() {
        input.aliases.push("params".into());
    }
    Ok(input)
}

/// The operation an error envelope names for a CLI command (the built-in ID
/// the command maps to, or the requested operation).
fn command_operation(c: &Command) -> Option<String> {
    Some(
        match c {
            Command::Request(a) => return a.id.clone(),
            Command::List { .. } => "rivet.list",
            Command::Describe { .. } => "rivet.describe",
            Command::Outputs { .. } => "rivet.outputs",
            Command::Check { .. } => "rivet.check",
            Command::Io(_) => "rivet.io",
            Command::Graph { .. } => "rivet.graph",
            Command::Policy {
                command: PolicyCommand::Explain { .. },
            } => "rivet.policy.explain",
            Command::Policy {
                command: PolicyCommand::Generate { .. },
            } => "rivet.policy.generate",
            Command::Serve(_) => "rivet.serve",
            Command::Trace {
                command: TraceCommand::Show { .. },
            } => "rivet.trace.show",
            Command::Trace {
                command: TraceCommand::Export { .. },
            } => "rivet.trace.export",
            Command::Connectors { .. } => "rivet.connectors.sync",
            Command::Highlight { .. } => "rivet.highlight",
            Command::Auth { .. } => return None,
        }
        .to_string(),
    )
}

/// `--token-file PATH`: the server bearer token (trimmed); never echoed.
fn read_token(path: &str) -> Result<String, RivetError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        RivetError::validation(
            "validation.usage",
            format!("--token-file {path}: {}", e.kind()),
        )
    })?;
    let token = text.trim().to_string();
    if token.is_empty() || token.contains(['\r', '\n']) {
        return Err(RivetError::validation(
            "validation.usage",
            format!("--token-file {path} must hold exactly one token line"),
        ));
    }
    Ok(token)
}

/// `--endpoint URL`: a thin client of a running `rivet serve`
/// (host bootstrap I/O through the remote_client adapter).
async fn run_endpoint(cli: &Cli, url: &str) -> i32 {
    if cli.file.is_some() || cli.policy.is_some() {
        return fail(
            &RivetError::validation(
                "validation.usage",
                "--endpoint cannot be combined with --file or --policy: the server owns the bundle and its policy",
            ),
            None,
            cli.json,
        );
    }
    let token = match cli.token_file.as_deref().map(read_token).transpose() {
        Ok(t) => t,
        Err(e) => return fail(&e, None, cli.json),
    };
    match crate::infra::remote_client::RemoteClient::new(url, token) {
        Ok(client) => super::remote_cli::run_remote(cli, &client).await,
        Err(e) => fail(&e, None, cli.json),
    }
}

/// `rivet highlight FILE [--format ansi|html|json]` (language.highlight_source):
/// reads only FILE (bootstrap I/O, like `check`), prints the rendering on
/// stdout and, on a syntax error, the partial rendering plus the diagnostic
/// (exit 2).
pub(super) fn highlight(path: &str, format: Option<&str>, json: bool) -> i32 {
    use crate::orchestrator::setup_library::highlight::{HighlightFormat, render, tokens_of};
    use std::io::IsTerminal;
    let format = match format {
        Some(f) => match HighlightFormat::parse(f) {
            Ok(f) => f,
            Err(e) => return fail_as(Some("rivet.highlight"), &e, None, json),
        },
        None if std::io::stdout().is_terminal() => HighlightFormat::Ansi,
        None => HighlightFormat::Json,
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            let e = RivetError::validation(
                "validation.usage",
                format!("rivet highlight {path}: {}", e.kind()),
            );
            return fail_as(Some("rivet.highlight"), &e, None, json);
        }
    };
    let (tokens, error) = match tokens_of(path, &text) {
        Ok(t) => (t, None),
        Err((t, e)) => (t, Some(e)),
    };
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(render(&text, &tokens, format).as_bytes());
    let _ = stdout.flush();
    match error {
        Some(e) => fail_as(Some("rivet.highlight"), &e, Some(&text), json),
        None => 0,
    }
}

async fn run(cli: Cli) -> i32 {
    if let Command::Highlight { path, format } = &cli.command {
        return highlight(path, format.as_deref(), cli.json);
    }
    if let Some(url) = cli.endpoint.clone() {
        return run_endpoint(&cli, &url).await;
    }
    if cli.token_file.is_some() {
        return fail(
            &RivetError::validation(
                "validation.usage",
                "--token-file authenticates to a server and needs --endpoint URL",
            ),
            None,
            cli.json,
        );
    }
    let runtime = match load(&cli) {
        Ok(r) => r,
        Err((e, src)) => {
            // Load diagnostics (syntax, policy) are rendered with their source
            // line unless --json asked for an error envelope.
            return fail_as(
                command_operation(&cli.command).as_deref(),
                &e,
                src.as_deref(),
                cli.json,
            );
        }
    };
    let source = runtime.bundle().files.first().map(|f| f.text.clone());
    let mut stdout = std::io::stdout();
    match &cli.command {
        Command::Request(args) => {
            // serve.parse_input: --data / --input (or the deprecated --params).
            let input = match resolve_input(args) {
                Ok(i) => i,
                Err(e) => return fail_as(args.id.as_deref(), &e, None, true),
            };
            let op = input.operation.clone();
            let stream = args.stream || input.stream == Some(true);
            if stream && cli.pretty {
                return fail_as(
                    Some(&op),
                    &RivetError::validation(
                        "validation.usage",
                        "--pretty cannot be used with --stream: NDJSON records must stay one per line",
                    ),
                    None,
                    true,
                );
            }
            let mut req = runtime.new_request(
                &op,
                input.data,
                crate::domain::contracts::Principal::local(),
            );
            if let Some(ms) = input.deadline_ms {
                req.deadline_ms = ms;
            }
            req.restrict = input.restrict;
            match super::remote_cli::timeout_ms(&args.timeout) {
                Ok(Some(ms)) => req.deadline_ms = ms,
                Ok(None) => {}
                Err(e) => return fail_as(Some(&op), &e, None, true),
            }
            runtime.note_deprecated_input(&req.request_id, &req.trace_id, &op, &input.aliases);
            match super::remote_cli::check_input_flags(args.input_jsonl.as_deref(), stream) {
                Ok(true) => {
                    let sink = std::sync::Arc::new(NdjsonSink::default());
                    return match run_duplex(&runtime, req, sink.clone()).await {
                        Ok(c) => {
                            let r = ResponseEnvelope::from_completion(&c).with_seq(sink.next_seq());
                            let _ = writeln!(stdout, "{}", r.to_json_string());
                            0
                        }
                        Err(e) => sink.fail(&op, &e),
                    };
                }
                Ok(false) => {}
                Err(e) => return fail_as(Some(&op), &e, None, true),
            }
            let ndjson = std::sync::Arc::new(NdjsonSink::default());
            let sink: Option<std::sync::Arc<dyn crate::domain::ports::DataSink>> =
                if stream { Some(ndjson.clone()) } else { None };
            // Ctrl-C cancels the request through execution.cancel_request; the
            // request then ends with one `cancelled` error (exit 130).
            let request_id = req.request_id.clone();
            let run = runtime.dispatch_request(req, sink);
            tokio::pin!(run);
            let outcome = tokio::select! {
                r = &mut run => r,
                _ = tokio::signal::ctrl_c() => {
                    let _ = runtime.cancel(&request_id, crate::domain::contracts::Principal::local());
                    run.await
                }
            };
            match outcome {
                Ok(c) if stream => {
                    let r = ResponseEnvelope::from_completion(&c).with_seq(ndjson.next_seq());
                    let _ = writeln!(stdout, "{}", r.to_json_string());
                    0
                }
                Ok(c) => {
                    print_envelope(&ResponseEnvelope::from_completion(&c));
                    0
                }
                Err(e) if stream => ndjson.fail(&op, &e),
                Err(e) => fail_as(Some(&op), &e, None, true),
            }
        }
        Command::List { outputs } => match runtime.list() {
            Ok(c) if cli.json => {
                let ops: Vec<_> = c
                    .entries
                    .iter()
                    .map(|e| {
                        let mut j = e.summary_json();
                        if *outputs {
                            j["output"] = e.output_schema();
                        }
                        j
                    })
                    .collect();
                print_envelope(&payload(
                    &runtime,
                    "rivet.list",
                    serde_json::json!({"operations": ops, "next_cursor": null}),
                ));
                0
            }
            Ok(c) => {
                let _ = write!(stdout, "{}", render_list(&c, *outputs));
                0
            }
            Err(e) => fail_as(Some("rivet.list"), &e, None, cli.json),
        },
        Command::Describe { ids } => match runtime.describe(ids) {
            Ok(c) if cli.json => {
                let items: Vec<_> = c.entries.iter().map(|e| e.describe_json()).collect();
                let data = if items.len() == 1 {
                    items[0].clone()
                } else {
                    serde_json::Value::Array(items)
                };
                print_envelope(&payload(&runtime, "rivet.describe", data));
                0
            }
            Ok(c) => {
                for (i, e) in c.entries.iter().enumerate() {
                    if i > 0 {
                        let _ = writeln!(stdout);
                    }
                    let _ = write!(stdout, "{}", render_describe(e));
                }
                0
            }
            Err(e) => fail_as(Some("rivet.describe"), &e, None, cli.json),
        },
        Command::Outputs { id, all } => match runtime.outputs(id.as_deref(), *all) {
            Ok(r) if cli.json => {
                let items: Vec<_> = r.iter().map(|x| x.to_json()).collect();
                let data = if !*all && items.len() == 1 {
                    items[0].clone()
                } else {
                    serde_json::Value::Array(items)
                };
                print_envelope(&payload(&runtime, "rivet.outputs", data));
                0
            }
            Ok(r) => {
                let _ = write!(stdout, "{}", render_outputs(&r));
                0
            }
            Err(e) => fail_as(Some("rivet.outputs"), &e, None, cli.json),
        },
        Command::Check { strict_docs } => {
            let program = runtime.program();
            let findings = if *strict_docs {
                strict_doc_findings(&program)
            } else {
                Vec::new()
            };
            // With --strict-docs an undeclared `fail` code is reported once, as an error.
            for w in program
                .warnings
                .iter()
                .filter(|w| !(*strict_docs && w.code == "docs.undeclared_error"))
            {
                // Same diagnostic shape as an error, labelled `warning[code]`.
                eprintln!(
                    "{}",
                    w.render(own_source(w).as_deref().or(source.as_deref()))
                        .replacen("error[", "warning[", 1)
                );
            }
            if let Some(first) = findings.first() {
                let mut e = first.clone();
                e.suppressed = findings[1..].to_vec();
                return fail_as(Some("rivet.check"), &e, source.as_deref(), cli.json);
            }
            if cli.json {
                print_envelope(&payload(
                    &runtime,
                    "rivet.check",
                    serde_json::json!({
                        "operations": program.operations.len(),
                        "connectors": program.connectors.len(),
                        "auth_profiles": program.auth_profiles.len(),
                        "warnings": program.warnings.len(),
                    }),
                ));
                return 0;
            }
            let _ = writeln!(
                stdout,
                "ok: {} operations, {} connectors, {} auth profiles",
                program.operations.len(),
                program.connectors.len(),
                program.auth_profiles.len()
            );
            0
        }
        Command::Policy {
            command: PolicyCommand::Explain { id, params },
        } => {
            let params = match params.as_deref().map(parse_params).transpose() {
                Ok(p) => p,
                Err(e) => return fail_as(Some("rivet.policy.explain"), &e, None, cli.json),
            };
            let report = match id {
                Some(id) => match runtime.io(&IoQuery {
                    ids: vec![id.clone()],
                    all: true,
                    check_policy: true,
                    params: params.clone(),
                    format: if cli.json {
                        "json".into()
                    } else {
                        "table".into()
                    },
                    ..IoQuery::default()
                }) {
                    Ok(r) => Some(r),
                    Err(e) => return fail_as(Some("rivet.policy.explain"), &e, None, cli.json),
                },
                None => None,
            };
            if cli.json {
                let p = runtime.policy();
                // `"*"` is an explicit broad target; flag every grant that uses it (S67).
                let broad: Vec<serde_json::Value> = p
                    .grants
                    .iter()
                    .filter(|g| g.targets.iter().any(|t| t == "*"))
                    .map(
                        |g| serde_json::json!({"capability": g.capability.as_str(), "target": "*"}),
                    )
                    .collect();
                let mut v = serde_json::json!({"present": p.present, "file": p.file, "sha256": p.sha256, "grants": p.grants.len(), "deny": p.deny.len(), "broad": broad});
                if let Some(r) = &report {
                    v["sites"] = r.manifest.to_json()["sites"].clone();
                }
                print_envelope(&payload(&runtime, "rivet.policy.explain", v));
            } else {
                let _ = write!(stdout, "{}", render_policy(runtime.policy()));
                if let Some(r) = &report {
                    let _ = write!(stdout, "\n{}", r.rendered);
                }
            }
            // G9: with --params the sites are that call's concrete targets; any denial
            // means the call would be refused → exit 3 (permission).
            let denied = report.as_ref().is_some_and(|r| {
                r.manifest
                    .sites
                    .iter()
                    .any(|s| s.decision.as_deref() == Some("denied"))
            });
            if params.is_some() && denied {
                if let Some(r) = &report {
                    for s in &r.manifest.sites {
                        if s.decision.as_deref() == Some("denied") {
                            eprintln!(
                                "denied: {} {} {} ({})",
                                s.effect_id,
                                s.capability.as_str(),
                                s.target.template,
                                s.access_label()
                            );
                        }
                    }
                }
                3
            } else {
                0
            }
        }
        Command::Serve(args) => {
            match super::setup_serve::run_cli(runtime.clone(), &args.listen, args.stdio).await {
                Ok(code) => code,
                Err(e) => fail_as(Some("rivet.serve"), &e, None, true),
            }
        }
        Command::Graph { id, all } => {
            let query = crate::domain::call_graph::GraphQuery {
                id: id.clone(),
                all: *all,
            };
            match runtime.graph(&query) {
                Ok(g) if cli.json => {
                    print_envelope(&payload(&runtime, "rivet.graph", g.to_json()));
                    0
                }
                Ok(g) => {
                    let _ = write!(stdout, "{}", g.render());
                    0
                }
                Err(e) => fail_as(Some("rivet.graph"), &e, None, cli.json),
            }
        }
        Command::Io(args) => {
            let query = args.to_query(cli.json);
            let json_out = query.format == "json";
            match runtime.io(&query) {
                Ok(report) => {
                    if json_out {
                        // The JSON format is an envelope whose data is the manifest.
                        print_envelope(&payload(&runtime, "rivet.io", report.manifest.to_json()));
                    } else {
                        let _ = write!(stdout, "{}", report.rendered);
                    }
                    if !report.diagnostics.is_empty() {
                        eprint!("{}", report.diagnostics);
                    }
                    report.exit_code as i32
                }
                Err(e) => fail_as(Some("rivet.io"), &e, None, cli.json || json_out),
            }
        }
        Command::Policy {
            command: PolicyCommand::Generate { ids, all, output },
        } => match runtime.generate_policy_draft(ids, *all, output.as_deref()) {
            Ok(draft) => {
                if cli.json {
                    // Same payload as rivet.policy.generate / POST /v1/policy/generate.
                    print_envelope(&payload(
                        &runtime,
                        "rivet.policy.generate",
                        serde_json::json!({
                            "policy": draft.policy_json(),
                            "review": draft.review.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
                            "complete": draft.complete,
                        }),
                    ));
                } else if output.is_none() {
                    let _ = write!(stdout, "{}", draft.render());
                }
                eprint!("{}", render_policy_review(&draft));
                draft.exit_code as i32
            }
            Err(e) => fail_as(Some("rivet.policy.generate"), &e, None, true),
        },
        Command::Trace {
            command: TraceCommand::Show { request_id },
        } => match runtime.trace(request_id) {
            Ok(t) => {
                print_envelope(&payload(&runtime, "rivet.trace.show", t.to_json()));
                0
            }
            Err(e) => fail_as(Some("rivet.trace.show"), &e, None, true),
        },
        Command::Trace {
            command: TraceCommand::Export { request_id, output },
        } => {
            let rel = match root_relative(output, &runtime.bundle().root) {
                Ok(r) => r,
                Err(e) => return fail_as(Some("rivet.trace.export"), &e, None, true),
            };
            match runtime.export_trace(request_id, &rel).await {
                Ok(receipt) => {
                    let mut env = payload(&runtime, "rivet.trace.export", receipt.to_json());
                    env.effects = Some(crate::domain::EffectsStatus::Committed);
                    print_envelope(&env);
                    0
                }
                Err(e) => fail_as(Some("rivet.trace.export"), &e, None, true),
            }
        }
        Command::Connectors {
            command: ConnectorsCommand::Sync { name, output },
        } => {
            let rel = match root_relative(output, &runtime.bundle().root) {
                Ok(r) => r,
                Err(e) => return fail_as(Some("rivet.connectors.sync"), &e, None, true),
            };
            match runtime.sync_connector(name, &rel).await {
                Ok(receipt) => {
                    let mut env = payload(&runtime, "rivet.connectors.sync", receipt.to_json());
                    env.effects = Some(crate::domain::EffectsStatus::Committed);
                    print_envelope(&env);
                    eprintln!(
                        "wrote candidate snapshot {output} ({}); after review, approve it in policy.json: \"approved\": {{\"snapshots\": [\"{}\"]}}",
                        receipt.sha256, receipt.sha256
                    );
                    0
                }
                Err(e) => fail_as(Some("rivet.connectors.sync"), &e, None, true),
            }
        }
        Command::Auth { command } => {
            let (id, params, timeout) = match auth_request(command) {
                Ok(v) => v,
                Err(e) => return fail(&e, None, true),
            };
            let mut req =
                runtime.new_request(id, params, crate::domain::contracts::Principal::local());
            match super::remote_cli::timeout_ms(&timeout) {
                Ok(Some(ms)) => req.deadline_ms = ms,
                Ok(None) => {}
                Err(e) => return fail_as(Some(id), &e, None, true),
            }
            match runtime.dispatch_request(req, None).await {
                Ok(c) => {
                    print_envelope(&ResponseEnvelope::from_completion(&c));
                    0
                }
                Err(e) => fail_as(Some(id), &e, None, true),
            }
        }
        #[allow(unreachable_patterns)]
        _ => fail(
            &RivetError::unsupported(
                "unsupported.command",
                "this command is not available in this development build yet",
            ),
            None,
            cli.json,
        ),
    }
}

/// `request ID --input-jsonl - --stream` on a local bundle: the stdin feeder
/// enqueues validated items into the session input (`Runtime::dispatch_session`
/// attaches it as `incoming`) while data envelopes drain to stdout. EOF closes
/// the input; a malformed line or Ctrl-C cancels the request through
/// execution.cancel_request (a malformed line then reports its validation error).
async fn run_duplex(
    runtime: &Runtime,
    req: crate::domain::contracts::Request,
    sink: std::sync::Arc<NdjsonSink>,
) -> Result<crate::domain::contracts::Completion, RivetError> {
    use super::remote_cli::{INPUT_QUEUE, feed_stdin_jsonl, receives_of};
    let entry = runtime
        .describe(std::slice::from_ref(&req.operation_id))?
        .entries
        .remove(0);
    let receives = receives_of(&entry)?;
    let (tx, rx) = tokio::sync::mpsc::channel::<Value>(INPUT_QUEUE);
    let (failed_tx, mut failed_rx) = tokio::sync::oneshot::channel::<RivetError>();
    let feeder = tokio::spawn(feed_stdin_jsonl(tx, receives, failed_tx));
    let request_id = req.request_id.clone();
    let trace_id = req.trace_id.clone();
    let principal = req.principal.clone();
    let run = runtime.dispatch_session(req, sink, rx);
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
                    let _ = runtime.cancel(&request_id, principal.clone());
                }
            }
            _ = tokio::signal::ctrl_c() => {
                let _ = runtime.cancel(&request_id, principal.clone());
            }
        }
    };
    feeder.abort();
    match input_error.or_else(|| failed_rx.try_recv().ok()) {
        Some(mut e) => {
            // The input error belongs to the request it cancelled.
            e.request_id = Some(request_id);
            e.trace_id = Some(trace_id);
            Err(e)
        }
        None => outcome,
    }
}

/// A cwd-relative output path as a bundle-root-relative `./path` (files are
/// confined to the bundle root); a path outside the bundle is refused.
fn root_relative(output: &str, root: &str) -> Result<String, RivetError> {
    let norm = |p: &std::path::Path| -> Vec<String> {
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        };
        let mut out: Vec<String> = Vec::new();
        for c in abs.components() {
            match c {
                std::path::Component::ParentDir => {
                    out.pop();
                }
                std::path::Component::Normal(s) => out.push(s.to_string_lossy().to_string()),
                _ => {}
            }
        }
        out
    };
    let o = norm(std::path::Path::new(output));
    let r = norm(std::path::Path::new(root));
    if o.len() <= r.len() || o[..r.len()] != r[..] {
        return Err(RivetError::validation(
            "validation.output",
            format!("--output {output} must be inside the bundle directory {root}"),
        ));
    }
    Ok(format!("./{}", o[r.len()..].join("/")))
}

/// Map `rivet auth …` onto the `rivet.auth.*` built-in (ID, params, --timeout).
pub(super) fn auth_request(
    command: &AuthCommand,
) -> Result<(&'static str, Value, Option<String>), RivetError> {
    let pa = |profile: &str, account: &str| {
        Value::object([
            ("profile", Value::text(profile)),
            ("account", Value::text(account)),
        ])
    };
    Ok(match command {
        AuthCommand::Begin { profile, account } => ("rivet.auth.begin", pa(profile, account), None),
        AuthCommand::Status { profile, account } => {
            ("rivet.auth.status", pa(profile, account), None)
        }
        AuthCommand::Disconnect { profile, account } => {
            ("rivet.auth.disconnect", pa(profile, account), None)
        }
        AuthCommand::Cancel { transaction_id } => (
            "rivet.auth.cancel",
            Value::object([("transaction_id", Value::text(transaction_id))]),
            None,
        ),
        AuthCommand::Complete {
            params,
            params_file,
            timeout,
        } => {
            let text = match (params, params_file) {
                (Some(p), None) => p.clone(),
                (None, Some(f)) => std::fs::read_to_string(f).map_err(|e| {
                    RivetError::validation(
                        "validation.usage",
                        format!("--params-file {f}: {}", e.kind()),
                    )
                })?,
                _ => {
                    return Err(RivetError::validation(
                        "validation.usage",
                        "auth complete needs exactly one of --params JSON or --params-file PATH",
                    ));
                }
            };
            // Parse errors never echo the input (it may hold a callback code).
            let v = serde_json::from_str::<serde_json::Value>(&text)
                .map(|j| Value::from_json(&j))
                .map_err(|e| {
                    RivetError::validation(
                        "validation.params",
                        format!(
                            "auth complete params are not valid JSON (line {}, column {})",
                            e.line(),
                            e.column()
                        ),
                    )
                })?;
            ("rivet.auth.complete", v, timeout.clone())
        }
    })
}

/// `--stream`: each data item becomes one NDJSON record (`type: data`) on
/// stdout; the terminal record then carries the next sequence. Records are
/// always compact (one per line).
#[derive(Default)]
pub(super) struct NdjsonSink {
    sent: std::sync::atomic::AtomicU64,
}

impl NdjsonSink {
    /// The sequence of the terminal record (items sent + 1).
    pub(super) fn next_seq(&self) -> u64 {
        self.sent.load(std::sync::atomic::Ordering::SeqCst) + 1
    }

    /// A failed stream: the terminal error record on stderr (its sequence and
    /// the number of items before it); returns the registry exit code.
    pub(super) fn fail(&self, operation: &str, e: &RivetError) -> i32 {
        let r = ResponseEnvelope::from_error(Some(operation), e)
            .with_seq(self.next_seq())
            .with_data_count(self.next_seq() - 1);
        eprintln!("{}", r.to_json_string());
        e.exit_code()
    }
}

#[async_trait::async_trait]
impl crate::domain::ports::DataSink for NdjsonSink {
    async fn send(
        &self,
        event: crate::domain::contracts::DataEvent,
    ) -> crate::domain::RivetResult<()> {
        self.sent.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let line = ResponseEnvelope::from_data(&event).to_json_string();
        let mut out = std::io::stdout();
        writeln!(out, "{line}").map_err(|e| {
            RivetError::new(
                crate::domain::ErrorKind::ConsumerFailed,
                "consumer_failed",
                format!("stdout closed: {e}"),
            )
        })
    }
}
