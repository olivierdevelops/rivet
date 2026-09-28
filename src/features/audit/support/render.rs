//! Text renderers for the I/O manifest: table, markdown and csv. Every format
//! is rendered from the same `IoManifest`; `--by` shapes table/markdown only.
//!
//! ```text
//!  --by operation  OPERATION KIND ACCESS TARGET KNOWLEDGE SOURCE [DECISION] [ATTEMPTS]
//!  --by target     TARGET ACCESS CAPABILITY ORIGIN PHASE NEEDS FILE USED BY [DECISION]
//!  --by capability CAPABILITY / TARGET  ACCESS  USED BY  KNOWLEDGE   (under headings)
//!  --needs         <op> needs, before it can run:  path (origin[, secret])  [status]
//! ```

use crate::domain::io_manifest::{EffectSite, IoManifest, OperationNeeds, SiteKind, TargetSummary};
use crate::domain::policy::Capability;

const DASH: &str = "—";

/// Capability headings in access-vocabulary order.
pub const CAPABILITY_ORDER: [Capability; 13] = [
    Capability::Read,
    Capability::Write,
    Capability::Delete,
    Capability::Network,
    Capability::Listen,
    Capability::Exec,
    Capability::Env,
    Capability::Pipe,
    Capability::Unix,
    Capability::Mcp,
    Capability::Grpc,
    Capability::Auth,
    Capability::Credentials,
];

pub fn capability_rank(c: Capability) -> usize {
    CAPABILITY_ORDER.iter().position(|x| *x == c).unwrap_or(99)
}

/// Options that change which columns appear.
#[derive(Clone, Debug, Default)]
pub struct RenderOptions {
    pub decision: bool,
    pub attempts: bool,
    pub filtered: bool,
    pub filter_label: String,
    pub markdown: bool,
}

/// Fixed-width (or Markdown) table from rows of cells; `span` rows carry a
/// single text cell that spans the middle columns.
struct Table {
    header: Vec<String>,
    rows: Vec<Row>,
}

enum Row {
    Cells(Vec<String>),
    /// (first cell, spanning text, cells after the span, number of spanned columns)
    Span(String, String, Vec<String>, usize),
    Text(String),
}

impl Table {
    fn render(&self, markdown: bool) -> String {
        let n = self.header.len();
        if markdown {
            let mut out = format!("| {} |\n", self.header.join(" | "));
            out.push_str(&format!("|{}\n", "---|".repeat(n)));
            for r in &self.rows {
                let cells: Vec<String> = match r {
                    Row::Cells(c) => c.iter().map(|x| x.replace('|', "\\|")).collect(),
                    Row::Span(first, text, after, spanned) => {
                        let mut v = vec![first.clone(), text.clone()];
                        v.extend(std::iter::repeat_n(
                            String::new(),
                            spanned.saturating_sub(1),
                        ));
                        v.extend(after.iter().cloned());
                        v
                    }
                    Row::Text(t) => {
                        let mut v = vec![t.clone()];
                        v.extend(std::iter::repeat_n(String::new(), n - 1));
                        v
                    }
                };
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            return out;
        }
        let mut widths: Vec<usize> = self.header.iter().map(|h| h.chars().count()).collect();
        for r in &self.rows {
            match r {
                Row::Cells(c) => {
                    for (i, x) in c.iter().enumerate() {
                        if i < n {
                            widths[i] = widths[i].max(x.chars().count());
                        }
                    }
                }
                Row::Span(first, _, after, spanned) => {
                    widths[0] = widths[0].max(first.chars().count());
                    for (i, x) in after.iter().enumerate() {
                        if let Some(w) = widths.get_mut(1 + spanned + i) {
                            *w = (*w).max(x.chars().count());
                        }
                    }
                }
                Row::Text(_) => {}
            }
        }
        // A spanning text wider than its columns widens the last spanned column.
        for r in &self.rows {
            if let Row::Span(_, text, _, spanned) = r {
                let have: usize = widths[1..1 + spanned].iter().map(|w| w + 2).sum::<usize>() - 2;
                let need = text.chars().count();
                if need > have {
                    widths[*spanned] += need - have;
                }
            }
        }
        let pad = |s: &str, w: usize| {
            let k = s.chars().count();
            format!("{s}{}", " ".repeat(w.saturating_sub(k) + 2))
        };
        let line = |cells: &[String]| {
            let mut s = String::new();
            for (i, c) in cells.iter().enumerate() {
                if i + 1 == cells.len() {
                    s.push_str(c);
                } else {
                    s.push_str(&pad(c, widths[i]));
                }
            }
            s.trim_end().to_string()
        };
        let mut out = line(&self.header);
        out.push('\n');
        for r in &self.rows {
            match r {
                Row::Cells(c) => out.push_str(&line(c)),
                Row::Span(first, text, after, spanned) => {
                    let mut s = pad(first, widths[0]);
                    let span_w: usize = widths[1..1 + spanned].iter().map(|w| w + 2).sum();
                    s.push_str(&format!(
                        "{text}{}",
                        " ".repeat(span_w.saturating_sub(text.chars().count()).max(2))
                    ));
                    for (i, c) in after.iter().enumerate() {
                        let col = 1 + spanned + i;
                        if i + 1 == after.len() {
                            s.push_str(c);
                        } else {
                            s.push_str(&pad(c, widths[col]));
                        }
                    }
                    out.push_str(s.trim_end());
                }
                Row::Text(t) => out.push_str(t),
            }
            out.push('\n');
        }
        out
    }
}

/// `--by operation`: one row per site, a `(calls X)` row per literal call.
pub fn render_by_operation(m: &IoManifest, o: &RenderOptions, show_calls: bool) -> String {
    let mut header: Vec<String> = [
        "OPERATION",
        "KIND",
        "ACCESS",
        "TARGET",
        "KNOWLEDGE",
        "SOURCE",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if o.decision {
        header.push("DECISION".into());
    }
    if o.attempts {
        header.push("ATTEMPTS".into());
    }
    // (operation, statement line, call-first, index, row)
    let mut items: Vec<(String, u32, u8, usize, Row)> = Vec::new();
    for (i, s) in m.sites.iter().enumerate() {
        let mut cells = vec![
            s.operation_id.clone(),
            s.kind.as_str().to_string(),
            s.access_label(),
            s.target_display(),
            s.knowledge.as_str().to_string(),
            s.source_label(),
        ];
        if o.decision {
            cells.push(s.decision.clone().unwrap_or_default());
        }
        if o.attempts {
            cells.push(match &s.attempts {
                Some(a) if a.count > 0 => {
                    format!("{} {}", a.count, a.last_decision.as_deref().unwrap_or(DASH))
                }
                _ => format!("0 {DASH}"),
            });
        }
        items.push((
            s.operation_id.clone(),
            s.statement_line,
            1,
            i,
            Row::Cells(cells),
        ));
    }
    if show_calls {
        for (i, c) in m.calls.iter().enumerate() {
            let text = match &c.connector {
                Some(conn) => format!("(calls {} {DASH} connector {conn})", c.callee),
                None => format!("(calls {} {DASH} see above)", c.callee),
            };
            let mut after = vec![format!("{}:{}", c.source.file, c.source.start_line)];
            if o.decision {
                after.push(String::new());
            }
            if o.attempts {
                after.push(String::new());
            }
            items.push((
                c.operation_id.clone(),
                c.source.start_line,
                0,
                i,
                Row::Span(c.operation_id.clone(), text, after, 4),
            ));
        }
    }
    items.sort_by(|a, b| (&a.0, a.1, a.2, a.3).cmp(&(&b.0, b.1, b.2, b.3)));
    let mut rows: Vec<Row> = items.into_iter().map(|x| x.4).collect();
    if rows.is_empty() {
        rows.push(Row::Text(if o.filtered {
            format!("(no sites match {})", o.filter_label)
        } else {
            "(no I/O sites)".to_string()
        }));
    }
    Table { header, rows }.render(o.markdown)
}

fn access_cell(t: &TargetSummary) -> String {
    let verbs: Vec<&str> = t.access.iter().map(|v| v.as_str()).collect();
    let mut s = verbs.join(", ");
    if !t.methods.is_empty() {
        s.push(' ');
        s.push_str(&t.methods.join(", "));
    }
    let shown: Vec<&String> = t
        .protocols
        .iter()
        .filter(|p| {
            p.split('|')
                .any(|x| matches!(x, "ws" | "grpc" | "quic" | "http3" | "sse"))
        })
        .collect();
    if !shown.is_empty() {
        s.push_str(&format!(
            " ({})",
            shown
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    s
}

/// `--by target`: one row per (target, capability).
pub fn render_by_target(m: &IoManifest, o: &RenderOptions) -> String {
    let mut header: Vec<String> = [
        "TARGET",
        "ACCESS",
        "CAPABILITY",
        "ORIGIN",
        "PHASE",
        "NEEDS FILE",
        "USED BY",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if o.decision {
        header.push("DECISION".into());
    }
    let mut rows = Vec::new();
    for t in &m.targets {
        let mut origins: Vec<String> = t.origins.iter().map(|x| x.name().to_string()).collect();
        origins.dedup();
        let mut cells = vec![
            t.target.clone(),
            access_cell(t),
            t.capability.as_str().to_string(),
            origins.join(", "),
            t.phases
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            t.needs_file.clone().unwrap_or_else(|| DASH.to_string()),
            t.used_by.join(", "),
        ];
        if o.decision {
            cells.push(t.decision.clone().unwrap_or_default());
        }
        rows.push(Row::Cells(cells));
    }
    if rows.is_empty() {
        rows.push(Row::Text(if o.filtered {
            format!("(no sites match {})", o.filter_label)
        } else {
            "(no I/O sites)".to_string()
        }));
    }
    Table { header, rows }.render(o.markdown)
}

/// `--by capability`: rows under capability headings, then the unused list.
pub fn render_by_capability(rows_in: &[TargetSummary], o: &RenderOptions) -> String {
    let header: Vec<String> = ["CAPABILITY / TARGET", "ACCESS", "USED BY", "KNOWLEDGE"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut rows = Vec::new();
    let mut unused = Vec::new();
    for cap in CAPABILITY_ORDER {
        let group: Vec<&TargetSummary> = rows_in.iter().filter(|t| t.capability == cap).collect();
        if group.is_empty() {
            unused.push(cap.as_str());
            continue;
        }
        rows.push(Row::Text(cap.as_str().to_string()));
        for t in group {
            rows.push(Row::Cells(vec![
                format!("  {}", t.target),
                access_cell(t),
                t.used_by.join(", "),
                t.knowledge.as_str().to_string(),
            ]));
        }
    }
    let mut out = Table { header, rows }.render(o.markdown);
    if !unused.is_empty() && !o.markdown {
        out.push_str(&format!("\nunused: {}\n", unused.join(" ")));
    }
    out
}

/// Bootstrap appendix of a table (`--include-bootstrap`).
pub fn render_bootstrap(sites: &[EffectSite], markdown: bool) -> String {
    let rows = sites
        .iter()
        .map(|s| {
            Row::Cells(vec![
                s.kind.as_str().to_string(),
                s.access_label(),
                s.target.template.clone(),
            ])
        })
        .collect();
    let t = Table {
        header: vec!["KIND".into(), "ACCESS".into(), "TARGET".into()],
        rows,
    };
    format!(
        "\nBOOTSTRAP (runtime-internal; listed, not governed by policy.json)\n{}",
        t.render(markdown)
    )
}

/// `--needs` / `--check-files` listing, one group per operation.
pub fn render_needs(needs: &[OperationNeeds], statuses: bool) -> String {
    let label = |f: &crate::domain::io_manifest::NeededFile| {
        let mut parts = vec![f.origin.name().to_string()];
        if f.secret {
            parts.push("secret".into());
        }
        if let Some(v) = &f.via {
            parts.push(format!("via {v}"));
        }
        if f.knowledge != crate::domain::io_manifest::Knowledge::Exact {
            parts.push(f.knowledge.as_str().to_string());
        }
        format!("({})", parts.join(", "))
    };
    let pw = needs
        .iter()
        .flat_map(|n| n.files.iter().map(|f| f.path.chars().count()))
        .max()
        .unwrap_or(0)
        + 2;
    let lw = needs
        .iter()
        .flat_map(|n| n.files.iter().map(|f| label(f).chars().count()))
        .max()
        .unwrap_or(0)
        + 3;
    let mut out = String::new();
    for n in needs {
        if n.files.is_empty() {
            out.push_str(&format!("{} needs no existing files.\n", n.operation_id));
            continue;
        }
        if n.operation_id == "bundle load" {
            out.push_str("bundle load needs:\n");
        } else {
            out.push_str(&format!("{} needs, before it can run:\n", n.operation_id));
        }
        for f in &n.files {
            let path = format!("{}{}", f.path, " ".repeat(pw - f.path.chars().count()));
            let l = label(f);
            if statuses {
                let st = f.status.map(|s| s.as_str()).unwrap_or("");
                out.push_str(&format!(
                    "  {path}{l}{}{st}\n",
                    " ".repeat(lw - l.chars().count())
                ));
            } else {
                out.push_str(&format!("  {path}{l}\n"));
            }
        }
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV: one row per site (never per target).
pub fn render_csv(m: &IoManifest) -> String {
    let mut out = String::from(
        "effect_id,operation_id,kind,access,method,protocol,capability,target,glob,knowledge,call_chain,secrets,source,origin,phase,requires_existing,secret,decision\n",
    );
    for s in &m.sites {
        let target = match s.kind {
            SiteKind::Env => format!("env:{}", s.target.template),
            _ => s.target_display(),
        };
        let cells = [
            s.effect_id.clone(),
            s.operation_id.clone(),
            s.kind.as_str().to_string(),
            s.access
                .iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(";"),
            s.method.clone().unwrap_or_default(),
            s.protocol.clone().unwrap_or_default(),
            s.capability.as_str().to_string(),
            target,
            s.target.glob.clone().unwrap_or_default(),
            s.knowledge.as_str().to_string(),
            s.call_chain.join(">"),
            s.secrets.join(";"),
            format!(
                "{}:{}:{}",
                s.source.file, s.source.start_line, s.source.start_col
            ),
            s.origin.csv(),
            s.phase.as_str().to_string(),
            s.requires_existing.to_string(),
            s.secret.to_string(),
            s.decision.clone().unwrap_or_default(),
        ];
        out.push_str(
            &cells
                .iter()
                .map(|c| csv_field(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}
