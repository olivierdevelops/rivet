//! CLI surface: argument parsing and rendering only. Business logic lives in
//! the shared dispatcher; `orchestrator::setup_cli` connects the two.

use crate::domain::contracts::{Catalog, OutputReport, RegistryEntry};
use crate::domain::io_manifest::{IoQuery, PolicyDraft};
use crate::domain::outputs::{FieldSpec, ValueSpec};
use crate::domain::policy::Policy;
use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "rivet",
    version,
    about = "Run described .rivet operations over the CLI, HTTP, WebSocket, MCP and the Rust library"
)]
pub struct Cli {
    /// Entry .rivet file (policy.json beside it is discovered automatically).
    #[arg(long, global = true)]
    pub file: Option<String>,
    /// Use this policy file instead of the discovered policy.json (a path, never grants).
    #[arg(long, global = true)]
    pub policy: Option<String>,
    /// Print JSON instead of tables (every JSON output is a ResponseEnvelope).
    #[arg(long, global = true)]
    pub json: bool,
    /// Pretty-print JSON output (2-space indent, same key order); refused with --stream.
    #[arg(long, global = true)]
    pub pretty: bool,
    /// Send request/list/describe/outputs/io/trace/auth to a running `rivet serve`
    /// at this URL instead of loading a bundle (cannot be combined with --file).
    #[arg(long, global = true)]
    pub endpoint: Option<String>,
    /// With --endpoint: read the server bearer token from this file (never argv or env).
    #[arg(long, global = true)]
    pub token_file: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Invoke one operation.
    Request(RequestArgs),
    /// List public operations.
    List {
        /// Add a one-line output summary column.
        #[arg(long)]
        outputs: bool,
    },
    /// Describe operations: params, output, errors, source.
    Describe { ids: Vec<String> },
    /// Show declared outputs, emits, receives and errors.
    Outputs {
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Compile and check the bundle without running anything.
    Check {
        /// Require descriptions on public operations, params, outputs and fields.
        #[arg(long)]
        strict_docs: bool,
    },
    /// Generate the I/O manifest (every I/O site, target and access verb).
    Io(IoArgs),
    /// Static call graph of one operation: literal calls (expanded), connector
    /// calls, effect sites, DAG nodes with after edges, both `if` arms marked.
    Graph {
        id: String,
        /// Allow a private operation.
        #[arg(long)]
        all: bool,
    },
    /// Policy tools.
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    /// Serve every surface (REST, SSE, polling, WebSocket, MCP) on one listener.
    Serve(ServeArgs),
    /// Request traces recorded by this host.
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },
    /// Outbound MCP connectors.
    Connectors {
        #[command(subcommand)]
        command: ConnectorsCommand,
    },
    /// OAuth account management (rivet.auth.* built-ins); tokens are never printed.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Syntax-highlight a .rivet file from the parser's own spans (no bundle is
    /// loaded; --file is not needed). A syntax error prints the tokens before it
    /// and the diagnostic (exit 2).
    Highlight {
        /// The .rivet file to highlight.
        path: String,
        /// Output format: ansi (terminal colours), html (<span class="rv-CLASS">)
        /// or json (one {line,col,len,class,text} object per line). Default:
        /// ansi when stdout is a terminal, json otherwise.
        #[arg(long, value_parser = ["ansi", "html", "json"])]
        format: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConnectorsCommand {
    /// Discover a connector's tools/resources/prompts and write a NEW candidate
    /// snapshot (never overwrites); prints the sha256 to approve in policy.json.
    Sync {
        name: String,
        /// Where to write the candidate snapshot (must not exist; inside the bundle).
        #[arg(long)]
        output: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Start an authorization_code or device_code transaction; prints the challenge.
    Begin {
        profile: String,
        #[arg(long)]
        account: String,
    },
    /// Complete a transaction. Pass callback codes with --params-file, never argv.
    Complete {
        /// {"transaction_id": ..., "callback": {...}} or {"transaction_id": ..., "wait": true}.
        #[arg(long)]
        params: Option<String>,
        /// Read the same JSON object from a file (keeps codes out of argv and history).
        #[arg(long)]
        params_file: Option<String>,
        /// Request deadline, e.g. "2m"; a device poll still pending then returns {"state":"pending"}.
        #[arg(long)]
        timeout: Option<String>,
    },
    /// Sanitized credential status (never refreshes).
    Status {
        profile: String,
        #[arg(long)]
        account: String,
    },
    /// Forget local credentials (local_only; no provider revocation).
    Disconnect {
        profile: String,
        #[arg(long)]
        account: String,
    },
    /// Cancel an open authorization transaction.
    Cancel { transaction_id: String },
}

#[derive(Subcommand, Debug)]
pub enum TraceCommand {
    /// Show one request's broker decisions and attempts (each with its effect_id).
    Show { request_id: String },
    /// Write one request's trace as JSON to a NEW file (needs allow_write on PATH; never overwrites).
    Export {
        request_id: String,
        #[arg(long)]
        output: String,
    },
}

#[derive(Args, Debug)]
pub struct RequestArgs {
    /// Operation ID (optional with --input, whose envelope names it).
    pub id: Option<String>,
    /// Operation input as a JSON object (the envelope's `data`; default {}).
    #[arg(long)]
    pub data: Option<String>,
    /// Deprecated alias of --data (removed in 0.3.0; prints warning[deprecated.params]).
    #[arg(long)]
    pub params: Option<String>,
    /// Read a whole input envelope {operation, data, deadline_ms?, restrict?, stream?}
    /// from FILE, or from stdin with `-`.
    #[arg(long, value_name = "FILE|-")]
    pub input: Option<String>,
    /// Print NDJSON records for streamed data (one record per line).
    #[arg(long)]
    pub stream: bool,
    /// Request deadline, e.g. "5s" (default 30s).
    #[arg(long)]
    pub timeout: Option<String>,
    /// Live input for an operation that `receives`: `-` reads JSON Lines from
    /// stdin while NDJSON output drains (EOF = finish_input); needs --stream.
    #[arg(long)]
    pub input_jsonl: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct IoArgs {
    pub ids: Vec<String>,
    #[arg(long)]
    pub all: bool,
    /// Follow literal (request "id" …) calls (the default; accepted for clarity).
    #[arg(long)]
    pub transitive: bool,
    #[arg(long)]
    pub include_bootstrap: bool,
    #[arg(long, default_value = "operation")]
    pub by: String,
    #[arg(long)]
    pub kind: Option<String>,
    #[arg(long)]
    pub access: Option<String>,
    #[arg(long, default_value = "table")]
    pub format: String,
    #[arg(long)]
    pub check_policy: bool,
    #[arg(long)]
    pub strict: bool,
    #[arg(long)]
    pub trace: Option<String>,
    #[arg(long)]
    pub needs: bool,
    #[arg(long)]
    pub check_files: bool,
}

#[derive(Subcommand, Debug)]
pub enum PolicyCommand {
    /// Explain the effective policy (and, with an ID, what that operation needs).
    Explain {
        id: Option<String>,
        /// Params of one concrete call: its param-dependent targets are filled and
        /// evaluated; exit 3 when any would be denied.
        #[arg(long)]
        params: Option<String>,
    },
    /// Generate a least-privilege policy.json draft from the I/O manifest.
    Generate {
        ids: Vec<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Listen address (default 127.0.0.1:8080).
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub listen: String,
    /// Serve MCP over stdio instead of a network listener.
    #[arg(long)]
    pub stdio: bool,
}

const DASH: &str = "—";

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        format!("{s} ")
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

/// `rivet list` table.
pub fn render_list(catalog: &Catalog, outputs: bool) -> String {
    let mut out = String::new();
    let idw = catalog
        .entries
        .iter()
        .map(|e| e.id.len())
        .max()
        .unwrap_or(2)
        .max(2)
        + 2;
    let namew = catalog
        .entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(4)
        + 2;
    out.push_str(&pad("ID", idw));
    out.push_str(&pad("NAME", namew));
    if outputs {
        out.push_str(&pad("OUTPUT", 12));
    }
    out.push_str("DESCRIPTION\n");
    for e in &catalog.entries {
        out.push_str(&pad(&e.id, idw));
        out.push_str(&pad(&e.name, namew));
        if outputs {
            out.push_str(&pad(&e.output.spec.name(), 12));
        }
        out.push_str(e.description.as_deref().unwrap_or(DASH));
        out.push('\n');
    }
    out
}

fn field_rows(fields: &[FieldSpec], indent: usize, out: &mut String) {
    let w = fields.iter().map(|f| f.name.len()).max().unwrap_or(0) + 2;
    for f in fields {
        out.push_str(&" ".repeat(indent));
        out.push_str(&pad(&f.name, w.max(8)));
        out.push_str(&pad(&f.spec.name(), 9));
        out.push_str(&pad(if f.required { "required" } else { "optional" }, 10));
        out.push_str(f.description.as_deref().unwrap_or(""));
        out.push('\n');
        if let ValueSpec::Object { fields, .. } = &f.spec {
            field_rows(fields, indent + 2, out);
        }
    }
}

fn spec_line(label: &str, spec: Option<&ValueSpec>, description: Option<&str>, out: &mut String) {
    match spec {
        None => out.push_str(&format!("{}{DASH}\n", pad(label, 9))),
        Some(s) => {
            out.push_str(
                format!(
                    "{}{}{}\n",
                    pad(label, 8),
                    pad(&s.name(), 9),
                    description.unwrap_or("")
                )
                .trim_end(),
            );
            out.push('\n');
            if let ValueSpec::Object { fields, open } = s {
                field_rows(fields, 2, out);
                if *open {
                    out.push_str("  (open: extra fields allowed)\n");
                }
            }
            if let ValueSpec::List(inner) = s
                && let ValueSpec::Object { fields, .. } = inner.as_ref()
            {
                field_rows(fields, 2, out);
            }
        }
    }
}

/// `rivet outputs ID` human table (REF-2026-0002 S124).
pub fn render_outputs(reports: &[OutputReport]) -> String {
    let mut out = String::new();
    for (i, r) in reports.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{} {DASH} {}\n", r.id, r.name));
        spec_line(
            "output",
            Some(&r.output.spec),
            r.output.description.as_deref(),
            &mut out,
        );
        spec_line(
            "emits",
            r.emits.as_ref(),
            r.emits_description.as_deref(),
            &mut out,
        );
        spec_line(
            "receives",
            r.receives.as_ref(),
            r.receives_description.as_deref(),
            &mut out,
        );
        if r.errors.is_empty() {
            out.push_str(&format!("{}{DASH}\n", pad("errors", 9)));
        } else {
            out.push_str("errors\n");
            let w = r.errors.iter().map(|e| e.code.len()).max().unwrap_or(0) + 3;
            for e in &r.errors {
                out.push_str(&format!(
                    "  {}{}\n",
                    pad(&e.code, w),
                    e.description.as_deref().unwrap_or("")
                ));
            }
        }
    }
    out
}

/// `rivet describe ID` text.
pub fn render_describe(e: &RegistryEntry) -> String {
    let mut out = format!("{} {DASH} {}\n", e.id, e.name);
    if let Some(d) = &e.description {
        out.push_str(&format!("{d}\n"));
    }
    out.push_str(&format!(
        "source   {}:{}\n",
        e.source.file, e.source.start_line
    ));
    out.push_str(&format!("delivery {}\n\nparams\n", e.delivery()));
    if e.params.is_empty() {
        out.push_str(&format!("  {DASH}\n"));
    }
    let w = e.params.iter().map(|p| p.name.len()).max().unwrap_or(0) + 3;
    for p in &e.params {
        let req = match (&p.default, p.required) {
            (Some(d), _) => format!("default {}", d.to_json()),
            (None, true) => "required".into(),
            (None, false) => "optional".into(),
        };
        out.push_str(&format!(
            "  {}{}{}{}\n",
            pad(&p.name, w),
            pad(&p.spec.name(), 9),
            pad(&req, 12),
            p.description.as_deref().unwrap_or("")
        ));
    }
    out.push('\n');
    out.push_str(
        &render_outputs(&[e.output_report()])
            .lines()
            .skip(1)
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
    );
    out
}

/// `rivet policy explain` summary.
pub fn render_policy(p: &Policy) -> String {
    let mut out = String::new();
    match &p.file {
        Some(f) if p.present => out.push_str(&format!("policy   {f} ({})\n", p.sha256.as_deref().unwrap_or(""))),
        _ => out.push_str("policy   none — no policy.json: every new application effect is denied (pure operations still run)\n"),
    }
    out.push_str(&format!("base     {}\n", p.base_dir));
    out.push_str(&format!(
        "network  deny_private_ranges {}\n",
        p.network.deny_private_ranges
    ));
    out.push_str(&format!(
        "limits   {} concurrent, depth {}, {} buffered bytes\n",
        p.limits.max_concurrent_requests, p.limits.max_call_depth, p.limits.max_buffered_bytes
    ));
    for (label, list) in [("grant", &p.grants), ("deny", &p.deny)] {
        for g in list {
            let access = g
                .access
                .as_ref()
                .map(|v| {
                    format!(
                        " access [{}]",
                        v.iter().map(|a| a.as_str()).collect::<Vec<_>>().join(", ")
                    )
                })
                .unwrap_or_default();
            let broad = if g.targets.iter().any(|t| t == "*") {
                "   ⚠ broad: \"*\" allows every target"
            } else {
                ""
            };
            out.push_str(&format!(
                "{}{} {}{access}{broad}\n",
                pad(label, 9),
                g.capability.as_str(),
                g.targets.join(", ")
            ));
        }
    }
    out
}

impl IoArgs {
    /// The shared manifest query; the global `--json` is an alias for `--format json`.
    pub fn to_query(&self, json: bool) -> IoQuery {
        IoQuery {
            ids: self.ids.clone(),
            all: self.all,
            transitive: true,
            strict: self.strict,
            include_bootstrap: self.include_bootstrap,
            by: self.by.clone(),
            kind: self.kind.clone(),
            access: self
                .access
                .as_deref()
                .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            format: if json {
                "json".into()
            } else {
                self.format.clone()
            },
            check_policy: self.check_policy,
            needs: self.needs,
            check_files: self.check_files,
            trace_request_id: self.trace.clone(),
            params: None,
        }
    }
}

/// stderr lines of `policy generate`: one per review site plus a summary.
pub fn render_policy_review(d: &PolicyDraft) -> String {
    let mut out = String::new();
    for s in &d.review {
        let why = match s.knowledge.as_str() {
            "dynamic" => "not granted (dynamic target)".to_string(),
            k => format!("not granted ({k})"),
        };
        out.push_str(&format!(
            "review  {}  {} {}  {}  {}  {why}\n",
            s.effect_id,
            s.kind.as_str(),
            s.access_label(),
            s.target_display(),
            s.source_label()
        ));
    }
    let n = d.review.len();
    out.push_str(&format!(
        "policy generate: {} grants, {n} review item{}{}\n",
        d.grants.len(),
        if n == 1 { "" } else { "s" },
        if n > 0 { " — draft incomplete" } else { "" }
    ));
    out
}
