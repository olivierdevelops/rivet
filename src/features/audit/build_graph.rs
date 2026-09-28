//! `audit.build_graph` — the static call graph of one operation (`rivet graph`).
//!
//! ```text
//!  GraphQuery ──▶ operation (public unless --all)
//!     ──▶ structure: if/else arms, dag + nodes (after edges)   ← compiled IR
//!     ──▶ leaves:    effect sites, literal calls, connector calls ← effect catalog
//!            each leaf hangs under the innermost structure whose lines contain it
//!     ──▶ call ──▶ the callee's graph, recursively (chain-guarded)
//! ```

use super::ports::Registry;
use crate::domain::call_graph::{CallGraph, GraphNode, GraphNodeKind, GraphQuery};
use crate::domain::io_manifest::{CallSite, EffectCatalog, EffectSite};
use crate::domain::ir::{CompiledProgram, Operation, Rhs, Stmt};
use crate::domain::{RivetError, RivetResult};

use super::support::effect_sites::{expr_text, rel_file};

// vhco:usecase audit.build_graph(input: GraphQuery) -> CallGraph needs Registry
// vhco:label Show the static call graph
// vhco:about Builds the static call graph of one operation from the compiled program and its effect catalog — literal (request …) calls expanded into the callee, connector calls, effect sites, DAG nodes with their after edges, and both arms of every if marked conditional; nothing is executed or evaluated.
// vhco:example input={id:"users.snapshot"} => { "operation_id": "users.snapshot", "nodes": [{"kind": "operation"}, {"kind": "call", "label": "call users.get"}], "edges": [{"kind": "contains"}] }
pub fn build_graph(query: &GraphQuery, registry: &dyn Registry) -> RivetResult<CallGraph> {
    let program = registry.program();
    let catalog = registry.effect_sites();
    // vhco:todo select_operation -- the ID must name an operation, public unless `all`; a private or absent one is not_found.operation without disclosing which
    // vhco:error unknown_id -- absent, or private without --all => not_found.operation (exit 4) returns
    let op = match program.operation(&query.id) {
        Some(op) if query.all || !op.private => op,
        _ => {
            return Err(RivetError::not_found(
                "not_found.operation",
                format!("no operation `{}`", query.id),
            ));
        }
    };
    // vhco:todo build_tree -- structure (if/else arms with their condition, dag with nodes and after lists) comes from the IR; each effect site, literal call and connector call of the operation hangs under the innermost structure whose line range contains it, in source order; a call node expands the callee's graph unless the callee is already on the chain (cycle node)
    // vhco:step tree graph_of -- ids are assigned in preorder afterwards (n0 = root)
    let mut b = Builder {
        program: &program,
        catalog: &catalog,
    };
    let mut root = b.graph_of(op, &mut vec![op.id.clone()]);
    fn renumber(n: &mut GraphNode, next: &mut usize) {
        n.id = format!("n{next}");
        *next += 1;
        n.children.iter_mut().for_each(|c| renumber(c, next));
    }
    renumber(&mut root, &mut 0);
    Ok(CallGraph {
        operation_id: op.id.clone(),
        root,
    })
}

struct Builder<'a> {
    program: &'a CompiledProgram,
    catalog: &'a EffectCatalog,
}

/// A structural region of one operation's source (lines inclusive).
struct Region {
    kind: GraphNodeKind,
    label: String,
    line: u32,
    lines: (u32, u32),
    condition: Option<String>,
    after: Vec<String>,
    children: Vec<Region>,
}

#[derive(Clone, Copy)]
enum Leaf<'a> {
    Site(&'a EffectSite),
    Call(&'a CallSite),
}

impl Leaf<'_> {
    fn line(&self) -> u32 {
        match self {
            Leaf::Site(s) => s.statement_line.max(1),
            Leaf::Call(c) => c.source.start_line,
        }
    }
}

impl<'a> Builder<'a> {
    fn graph_of(&mut self, op: &'a Operation, chain: &mut Vec<String>) -> GraphNode {
        let file = rel_file(&op.span.file, &self.program.root);
        let mut root = Region {
            kind: GraphNodeKind::Operation,
            label: op.id.clone(),
            line: op.span.start_line,
            lines: (0, u32::MAX),
            condition: None,
            after: Vec::new(),
            children: Vec::new(),
        };
        regions(&op.body, &mut root.children);
        let mut leaves: Vec<Leaf> = Vec::new();
        if let Some(effects) = self.catalog.operation(&op.id) {
            leaves.extend(
                effects
                    .sites
                    .iter()
                    .filter(|s| s.via.is_none())
                    .map(Leaf::Site),
            );
            leaves.extend(effects.calls.iter().map(Leaf::Call));
        }
        self.node(&root, &leaves, &file, false, op, chain)
    }

    /// Materialize `region` with the leaves that fall inside it and outside
    /// every child region.
    fn node(
        &mut self,
        region: &Region,
        leaves: &[Leaf<'a>],
        file: &str,
        conditional: bool,
        op: &'a Operation,
        chain: &mut Vec<String>,
    ) -> GraphNode {
        let id = String::new();
        let inside = |l: u32, r: &Region| l >= r.lines.0 && l <= r.lines.1;
        let conditional = conditional || region.kind == GraphNodeKind::Branch;
        // (line, order, child) — structure and leaves interleave in source order.
        let mut items: Vec<(u32, usize, GraphNode)> = Vec::new();
        for (i, child) in region.children.iter().enumerate() {
            let mine: Vec<Leaf> = leaves
                .iter()
                .copied()
                .filter(|l| inside(l.line(), child))
                .collect();
            let n = self.node(child, &mine, file, conditional, op, chain);
            items.push((child.line, i, n));
        }
        let own = leaves
            .iter()
            .filter(|l| !region.children.iter().any(|c| inside(l.line(), c)));
        for (i, leaf) in own.enumerate() {
            let order = 1_000_000 + i;
            match leaf {
                Leaf::Site(s) => {
                    let n = self.site(s, conditional);
                    items.push((s.statement_line, order, n));
                }
                Leaf::Call(c) => {
                    let n = self.call(c, conditional, op, chain);
                    items.push((c.source.start_line, order, n));
                }
            }
        }
        items.sort_by_key(|(line, order, _)| (*line, *order));
        GraphNode {
            id,
            kind: region.kind,
            label: region.label.clone(),
            source: format!("{file}:{}", region.line),
            condition: region.condition.clone(),
            conditional: conditional && region.kind != GraphNodeKind::Operation,
            after: region.after.clone(),
            children: items.into_iter().map(|(_, _, n)| n).collect(),
        }
    }

    fn site(&mut self, s: &EffectSite, conditional: bool) -> GraphNode {
        GraphNode {
            id: String::new(),
            kind: GraphNodeKind::Effect,
            label: format!(
                "{:<8} {:<12} {}",
                s.kind.as_str(),
                s.access_label(),
                s.target_display()
            )
            .trim_end()
            .to_string(),
            source: s.source_label(),
            condition: s.condition.clone(),
            conditional: conditional || s.condition.is_some(),
            after: Vec::new(),
            children: Vec::new(),
        }
    }

    fn call(
        &mut self,
        c: &CallSite,
        conditional: bool,
        op: &'a Operation,
        chain: &mut Vec<String>,
    ) -> GraphNode {
        let id = String::new();
        let source = format!(
            "{}:{}",
            rel_file(&c.source.file, &self.program.root),
            c.source.start_line
        );
        if let Some(conn) = &c.connector {
            // Connector expansion sites carry `via = callee`.
            let sites: Vec<&EffectSite> = self
                .catalog
                .operation(&op.id)
                .map(|e| {
                    e.sites
                        .iter()
                        .filter(|s| {
                            s.via.as_deref() == Some(c.callee.as_str())
                                && s.statement_line == c.source.start_line
                        })
                        .collect()
                })
                .unwrap_or_default();
            let children = sites
                .into_iter()
                .map(|s| self.site(s, conditional))
                .collect();
            return GraphNode {
                id,
                kind: GraphNodeKind::Connector,
                label: format!("connector {} ({conn})", c.callee),
                source,
                condition: None,
                conditional,
                after: Vec::new(),
                children,
            };
        }
        let Some(callee) = self.program.operation(&c.callee) else {
            return GraphNode {
                id,
                kind: GraphNodeKind::Call,
                label: format!("call {} (not in this bundle)", c.callee),
                source,
                condition: None,
                conditional,
                after: Vec::new(),
                children: Vec::new(),
            };
        };
        if chain.contains(&callee.id) {
            return GraphNode {
                id,
                kind: GraphNodeKind::Cycle,
                label: format!("call {} (already on this chain)", c.callee),
                source,
                condition: None,
                conditional,
                after: Vec::new(),
                children: Vec::new(),
            };
        }
        chain.push(callee.id.clone());
        let inner = self.graph_of(callee, chain);
        chain.pop();
        GraphNode {
            id,
            kind: GraphNodeKind::Call,
            label: format!("call {}", c.callee),
            source,
            condition: None,
            conditional,
            after: Vec::new(),
            children: inner.children,
        }
    }
}

/// Last line a statement (and everything nested in it) occupies.
fn last_line(s: &Stmt) -> u32 {
    let body_last = |b: &[Stmt]| b.iter().map(last_line).max().unwrap_or(0);
    let nested = match s {
        Stmt::If {
            then, otherwise, ..
        } => body_last(then).max(body_last(otherwise)),
        Stmt::While { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Iterate { body, .. }
        | Stmt::Scope { body, .. }
        | Stmt::With { body, .. } => body_last(body),
        Stmt::Try { body, handler, .. } => body_last(body).max(body_last(handler)),
        Stmt::Dag { nodes, .. } => nodes.iter().map(|n| n.span.end_line).max().unwrap_or(0),
        Stmt::Concurrent { tasks, .. } => {
            tasks.iter().map(|(_, b)| body_last(b)).max().unwrap_or(0)
        }
        Stmt::Assign {
            rhs: Rhs::Map { body, .. } | Rhs::Poll { body, .. },
            ..
        } => body_last(body),
        _ => 0,
    };
    s.span().end_line.max(nested)
}

/// Structural regions (if/else arms, dag and its nodes) of a statement list.
fn regions(body: &[Stmt], out: &mut Vec<Region>) {
    for s in body {
        match s {
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => {
                let cond_text = expr_text(cond);
                let then_end = then.iter().map(last_line).max().unwrap_or(span.start_line);
                let mut arm = Region {
                    kind: GraphNodeKind::Branch,
                    label: format!("if {cond_text}"),
                    line: span.start_line,
                    lines: (span.start_line, then_end),
                    condition: Some(cond_text.clone()),
                    after: Vec::new(),
                    children: Vec::new(),
                };
                regions(then, &mut arm.children);
                out.push(arm);
                if let Some(first) = otherwise.first() {
                    let start = first.span().start_line;
                    let end = otherwise.iter().map(last_line).max().unwrap_or(start);
                    let mut arm = Region {
                        kind: GraphNodeKind::Branch,
                        label: "else".into(),
                        line: (then_end + 1).min(start),
                        lines: (then_end + 1, end),
                        condition: Some(format!("not ({cond_text})")),
                        after: Vec::new(),
                        children: Vec::new(),
                    };
                    regions(otherwise, &mut arm.children);
                    out.push(arm);
                }
            }
            Stmt::Dag { nodes, span, .. } => {
                let mut dag = Region {
                    kind: GraphNodeKind::Dag,
                    label: "dag".into(),
                    line: span.start_line,
                    lines: (span.start_line, last_line(s)),
                    condition: None,
                    after: Vec::new(),
                    children: Vec::new(),
                };
                for n in nodes {
                    dag.children.push(Region {
                        kind: GraphNodeKind::Node,
                        label: if n.after.is_empty() {
                            format!("node {}", n.name)
                        } else {
                            format!("node {} after {}", n.name, n.after.join(", "))
                        },
                        line: n.span.start_line,
                        lines: (n.span.start_line, n.span.end_line),
                        condition: None,
                        after: n.after.clone(),
                        children: Vec::new(),
                    });
                }
                out.push(dag);
            }
            Stmt::While { body, .. }
            | Stmt::For { body, .. }
            | Stmt::Iterate { body, .. }
            | Stmt::Scope { body, .. }
            | Stmt::With { body, .. } => regions(body, out),
            Stmt::Try { body, handler, .. } => {
                regions(body, out);
                regions(handler, out);
            }
            Stmt::Concurrent { tasks, .. } => tasks.iter().for_each(|(_, b)| regions(b, out)),
            Stmt::Assign {
                rhs: Rhs::Map { body, .. } | Rhs::Poll { body, .. },
                ..
            } => regions(body, out),
            _ => {}
        }
    }
}
