//! CLI surface registration: maps each command to the shared use cases.

// vhco:surface cli kind cli calls language/compile_program, registry/describe_operations, registry/inspect_outputs, execution/request_operation, policy/load_policy
// vhco:trigger cli execution/request_operation = rivet request ID --params JSON
// vhco:trigger cli registry/describe_operations = rivet list | rivet describe ID
// vhco:trigger cli registry/inspect_outputs = rivet outputs ID | rivet outputs --all
// vhco:trigger cli language/compile_program = rivet check [--strict-docs]
// vhco:trigger cli policy/load_policy = rivet policy explain
// vhco:api cli execution/request_operation rivet request ID --params JSON -- invoke one operation; stdout is the Completion JSON, errors are an ErrorEnvelope on stderr with the registry exit code
// vhco:request { "id": "string — operation ID", "params": "JSON object" }
// vhco:response { "request_id": "string", "trace_id": "string", "result": "Value", "data_count": "int", "effects": "none|committed|partial|unknown" }

use crate::domain::contracts::error_envelope;
use crate::domain::ir::parse_duration_ms;
use crate::domain::{RivetError, Value};
use crate::features::language::lower::strict_doc_findings;
use crate::io::cli::{
    Cli, Command, PolicyCommand, render_describe, render_list, render_outputs, render_policy,
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
            match runtime.dispatch_request(req, sink).await {
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
            command: PolicyCommand::Explain { .. },
        } => {
            if cli.json {
                let p = runtime.policy();
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::json!({"present": p.present, "file": p.file, "sha256": p.sha256, "grants": p.grants.len(), "deny": p.deny.len()})
                );
            } else {
                let _ = write!(stdout, "{}", render_policy(runtime.policy()));
            }
            0
        }
        Command::Io(_)
        | Command::Policy {
            command: PolicyCommand::Generate { .. },
        }
        | Command::Serve(_) => fail(
            &RivetError::unsupported(
                "unsupported.command",
                "this command is not available in this development build yet",
            ),
            None,
            cli.json,
        ),
    }
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
