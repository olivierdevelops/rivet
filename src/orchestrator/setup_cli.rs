//! CLI surface registration: maps each command to the shared use cases.

// vhco:surface cli kind cli calls language/compile_program, registry/describe_operations, registry/inspect_outputs, execution/request_operation, execution/cancel_request, policy/load_policy, serve/start_serve, audit/inspect_effects, audit/read_trace, policy/generate_policy, connectors/invoke_mcp, auth/begin_authorization, auth/complete_authorization, auth/credential_status, auth/disconnect_account, auth/cancel_authorization
// vhco:trigger cli auth/begin_authorization = rivet auth begin PROFILE --account ACCOUNT | rivet request rivet.auth.begin --params JSON
// vhco:trigger cli auth/complete_authorization = rivet auth complete --params-file PATH [--timeout D] | rivet auth complete --params JSON
// vhco:trigger cli auth/credential_status = rivet auth status PROFILE --account ACCOUNT
// vhco:trigger cli auth/disconnect_account = rivet auth disconnect PROFILE --account ACCOUNT
// vhco:trigger cli auth/cancel_authorization = rivet auth cancel TRANSACTION_ID
// vhco:api cli auth/begin_authorization rivet auth begin PROFILE --account ACCOUNT -- start a code (PKCE S256) or device transaction; stdout is the Completion with the AuthChallenge (no verifier, state secret or token)
// vhco:request { "profile": "string", "account": "string" }
// vhco:response { "transaction_id": "string", "authorization_url": "string?", "verification_uri": "string?", "user_code": "string?", "expires_at": "RFC 3339", "interval_seconds": "int?" }
// vhco:api cli auth/complete_authorization rivet auth complete --params-file PATH [--timeout D] -- finish a transaction; connected CredentialStatus, or {"state":"pending"} when the deadline came first (transaction kept)
// vhco:request { "transaction_id": "string", "callback": "{code, state, redirect_uri, issuer?}?", "wait": "bool" }
// vhco:response { "profile": "string", "account": "string", "state": "connected|pending", "scopes": "string[]", "expires_at": "RFC 3339", "generation": "int" }
// vhco:trigger cli execution/request_operation = rivet request ID --params JSON
// vhco:trigger cli execution/cancel_request = Ctrl-C during rivet request
// vhco:trigger cli registry/describe_operations = rivet list | rivet describe ID
// vhco:trigger cli registry/inspect_outputs = rivet outputs ID | rivet outputs --all
// vhco:trigger cli language/compile_program = rivet check [--strict-docs]
// vhco:trigger cli policy/load_policy = rivet policy explain
// vhco:trigger cli serve/start_serve = rivet serve [--listen HOST:PORT] | rivet serve --stdio
// vhco:trigger cli audit/inspect_effects = rivet io [ID ...] [--all] [--by operation|target|capability] [--kind K] [--access V,V] [--format table|json|markdown|csv] [--check-policy] [--strict] [--trace REQ] [--needs] [--check-files] [--include-bootstrap]
// vhco:trigger cli audit/read_trace = rivet trace show REQ
// vhco:trigger cli policy/generate_policy = rivet policy generate [ID ...|--all] [--output PATH]
// vhco:trigger cli connectors/invoke_mcp = rivet connectors sync NAME --output PATH | rivet request CONNECTOR.tools.NAME --params JSON
// vhco:api cli connectors/invoke_mcp rivet connectors sync NAME --output PATH -- authorized discovery (allow_mcp NAME/discover + transport grants + allow_write PATH) writing a NEW candidate snapshot; prints its sha256 to approve in policy.json approved.snapshots; exit 0, 3 denied, 4 output exists
// vhco:request { "name": "string — mcp connector", "output": "string — new snapshot path inside the bundle" }
// vhco:response { "connector": "string", "path": "string", "sha256": "sha256:…", "protocolVersion": "string", "tools": "string[]", "resources": "string[]", "prompts": "string[]" }
// vhco:api cli audit/inspect_effects rivet io [ID ...] [flags] -- the I/O manifest; stdout in the requested format, summaries on stderr; exit 0, 3 (denied/partial or not_permitted/unreadable), 4 (missing needed file / unknown id), 7 (--strict and incomplete), 2 (usage)
// vhco:request { "ids": "string[]", "all": "bool", "by": "operation|target|capability", "kind": "string?", "access": "string[]", "format": "table|json|markdown|csv", "check_policy": "bool", "strict": "bool", "needs": "bool", "check_files": "bool", "trace": "string?" }
// vhco:response { "bundle": "FileDigest", "policy": "FileDigest?", "complete": "bool", "sites": "EffectSite[]", "targets": "TargetSummary[]", "needs": "OperationNeeds[]", "bootstrap": "EffectSite[]" }
// vhco:api cli policy/generate_policy rivet policy generate [ID ...|--all] [--output PATH] -- least-privilege policy.json draft on stdout (or a new file); review items on stderr; exit 7 when review items exist, 4 conflict.exists
// vhco:request { "ids": "string[]", "all": "bool", "output": "string?" }
// vhco:response { "version": "1", "grants": "Grant[]", "network": "{deny_private_ranges: true}" }
// vhco:api cli execution/request_operation rivet request ID --params JSON -- invoke one operation; stdout is the Completion JSON, errors are an ErrorEnvelope on stderr with the registry exit code
// vhco:request { "id": "string — operation ID", "params": "JSON object" }
// vhco:response { "request_id": "string", "trace_id": "string", "result": "Value", "data_count": "int", "effects": "none|committed|partial|unknown" }

use crate::domain::contracts::error_envelope;
use crate::domain::io_manifest::IoQuery;
use crate::domain::ir::parse_duration_ms;
use crate::domain::{RivetError, Value};
use crate::features::language::lower::strict_doc_findings;
use crate::io::cli::{
    AuthCommand, Cli, Command, ConnectorsCommand, PolicyCommand, TraceCommand, render_describe,
    render_list, render_outputs, render_policy, render_policy_review,
};
use crate::orchestrator::runtime::{Runtime, RuntimeBuilder};
use clap::Parser;
use std::io::Write;

/// Entry point for the `rivet` binary; returns the process exit code.
pub fn main() -> i32 {
    let cli = Cli::parse();
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

fn fail(e: &RivetError, source: Option<&str>, json: bool) -> i32 {
    if json {
        let env = error_envelope(
            e.request_id.as_deref().unwrap_or(""),
            e.trace_id.as_deref().unwrap_or(""),
            e,
        );
        eprintln!("{env}");
    } else {
        eprintln!("{}", e.render(source));
        for s in e.suppressed.iter().take(20) {
            eprintln!("{}", s.render(source));
        }
        if e.suppressed.len() > 20 {
            eprintln!("… and {} more", e.suppressed.len() - 20);
        }
    }
    e.exit_code()
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

fn parse_params(text: &str) -> Result<Value, RivetError> {
    serde_json::from_str::<serde_json::Value>(text)
        .map(|j| Value::from_json(&j))
        .map_err(|e| {
            RivetError::validation(
                "validation.params",
                format!("--params is not valid JSON: {e}"),
            )
        })
}

async fn run(cli: Cli) -> i32 {
    let runtime = match load(&cli) {
        Ok(r) => r,
        Err((e, src)) => {
            return fail(
                &e,
                src.as_deref(),
                cli.json && !matches!(cli.command, Command::Check { .. }),
            );
        }
    };
    let source = runtime.bundle().files.first().map(|f| f.text.clone());
    let mut stdout = std::io::stdout();
    match &cli.command {
        Command::Request(args) => {
            let params = match parse_params(&args.params) {
                Ok(p) => p,
                Err(e) => return fail(&e, None, true),
            };
            let mut req = runtime.new_request(
                &args.id,
                params,
                crate::domain::contracts::Principal::local(),
            );
            if let Some(t) = &args.timeout {
                match parse_duration_ms(t) {
                    Some(ms) => req.deadline_ms = ms,
                    None => {
                        return fail(
                            &RivetError::validation(
                                "validation.usage",
                                format!("--timeout {t}: use digits plus ms, s, m or h"),
                            ),
                            None,
                            true,
                        );
                    }
                }
            }
            let sink: Option<std::sync::Arc<dyn crate::domain::ports::DataSink>> = if args.stream {
                Some(std::sync::Arc::new(NdjsonSink))
            } else {
                None
            };
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
                Ok(c) => {
                    let line = if args.stream {
                        crate::domain::contracts::Envelope::Result(c).to_json()
                    } else {
                        c.to_json()
                    };
                    let _ = writeln!(stdout, "{line}");
                    0
                }
                Err(e) => fail(&e, None, true),
            }
        }
        Command::List { outputs } => match runtime.list() {
            Ok(c) if cli.json => {
                let ops: Vec<_> = c.entries.iter().map(|e| e.summary_json()).collect();
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::json!({"operations": ops, "next_cursor": null})
                );
                0
            }
            Ok(c) => {
                let _ = write!(stdout, "{}", render_list(&c, *outputs));
                0
            }
            Err(e) => fail(&e, None, cli.json),
        },
        Command::Describe { ids } => match runtime.describe(ids) {
            Ok(c) if cli.json => {
                let items: Vec<_> = c.entries.iter().map(|e| e.describe_json()).collect();
                let _ = writeln!(
                    stdout,
                    "{}",
                    if items.len() == 1 {
                        items[0].clone()
                    } else {
                        serde_json::Value::Array(items)
                    }
                );
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
            Err(e) => fail(&e, None, cli.json),
        },
        Command::Outputs { id, all } => match runtime.outputs(id.as_deref(), *all) {
            Ok(r) if cli.json => {
                let items: Vec<_> = r.iter().map(|x| x.to_json()).collect();
                let _ = writeln!(
                    stdout,
                    "{}",
                    if !*all && items.len() == 1 {
                        items[0].clone()
                    } else {
                        serde_json::Value::Array(items)
                    }
                );
                0
            }
            Ok(r) => {
                let _ = write!(stdout, "{}", render_outputs(&r));
                0
            }
            Err(e) => fail(&e, None, cli.json),
        },
        Command::Check { strict_docs } => {
            let program = runtime.program();
            let findings = if *strict_docs {
                strict_doc_findings(&program)
            } else {
                Vec::new()
            };
            for w in &program.warnings {
                eprintln!("warning: {}", w.render(source.as_deref()));
            }
            if let Some(first) = findings.first() {
                let mut e = first.clone();
                e.suppressed = findings[1..].to_vec();
                return fail(&e, source.as_deref(), cli.json);
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
            command: PolicyCommand::Explain { id, .. },
        } => {
            let report = match id {
                Some(id) => match runtime.io(&IoQuery {
                    ids: vec![id.clone()],
                    all: true,
                    check_policy: true,
                    format: if cli.json {
                        "json".into()
                    } else {
                        "table".into()
                    },
                    ..IoQuery::default()
                }) {
                    Ok(r) => Some(r),
                    Err(e) => return fail(&e, None, cli.json),
                },
                None => None,
            };
            if cli.json {
                let p = runtime.policy();
                let mut v = serde_json::json!({"present": p.present, "file": p.file, "sha256": p.sha256, "grants": p.grants.len(), "deny": p.deny.len()});
                if let Some(r) = &report {
                    v["sites"] = r.manifest.to_json()["sites"].clone();
                }
                let _ = writeln!(stdout, "{v}");
            } else {
                let _ = write!(stdout, "{}", render_policy(runtime.policy()));
                if let Some(r) = &report {
                    let _ = write!(stdout, "\n{}", r.rendered);
                }
            }
            0
        }
        Command::Serve(args) => {
            match super::setup_serve::run_cli(runtime.clone(), &args.listen, args.stdio).await {
                Ok(code) => code,
                Err(e) => fail(&e, None, true),
            }
        }
        Command::Io(args) => match runtime.io(&args.to_query(cli.json)) {
            Ok(report) => {
                let _ = write!(stdout, "{}", report.rendered);
                if !report.diagnostics.is_empty() {
                    eprint!("{}", report.diagnostics);
                }
                report.exit_code as i32
            }
            Err(e) => fail(&e, None, cli.json),
        },
        Command::Policy {
            command: PolicyCommand::Generate { ids, all, output },
        } => match runtime.generate_policy_draft(ids, *all, output.as_deref()) {
            Ok(draft) => {
                if output.is_none() {
                    let _ = write!(stdout, "{}", draft.render());
                }
                eprint!("{}", render_policy_review(&draft));
                draft.exit_code as i32
            }
            Err(e) => fail(&e, None, true),
        },
        Command::Trace {
            command: TraceCommand::Show { request_id },
        } => match runtime.trace(request_id) {
            Ok(t) => {
                let _ = writeln!(stdout, "{}", t.to_json());
                0
            }
            Err(e) => fail(&e, None, true),
        },
        Command::Connectors {
            command: ConnectorsCommand::Sync { name, output },
        } => {
            let rel = match root_relative(output, &runtime.bundle().root) {
                Ok(r) => r,
                Err(e) => return fail(&e, None, true),
            };
            match runtime.sync_connector(name, &rel).await {
                Ok(receipt) => {
                    let _ = writeln!(stdout, "{}", receipt.to_json());
                    eprintln!(
                        "wrote candidate snapshot {output} ({}); after review, approve it in policy.json: \"approved\": {{\"snapshots\": [\"{}\"]}}",
                        receipt.sha256, receipt.sha256
                    );
                    0
                }
                Err(e) => fail(&e, None, true),
            }
        }
        Command::Auth { command } => {
            let (id, params, timeout) = match auth_request(command) {
                Ok(v) => v,
                Err(e) => return fail(&e, None, true),
            };
            let mut req =
                runtime.new_request(id, params, crate::domain::contracts::Principal::local());
            if let Some(t) = &timeout {
                match parse_duration_ms(t) {
                    Some(ms) => req.deadline_ms = ms,
                    None => {
                        return fail(
                            &RivetError::validation(
                                "validation.usage",
                                format!("--timeout {t}: use digits plus ms, s, m or h"),
                            ),
                            None,
                            true,
                        );
                    }
                }
            }
            match runtime.dispatch_request(req, None).await {
                Ok(c) => {
                    let _ = writeln!(stdout, "{}", c.to_json());
                    0
                }
                Err(e) => fail(&e, None, true),
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
fn auth_request(
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

/// `--stream`: each data item becomes one NDJSON envelope line on stdout.
struct NdjsonSink;

#[async_trait::async_trait]
impl crate::domain::ports::DataSink for NdjsonSink {
    async fn send(
        &self,
        event: crate::domain::contracts::DataEvent,
    ) -> crate::domain::RivetResult<()> {
        let line = crate::domain::contracts::Envelope::Data(event).to_json();
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
