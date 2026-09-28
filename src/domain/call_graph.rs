//! Static call graph of one operation (`rivet graph ID [--json]`, REF S63).
//!
//! ```text
//!  users.snapshot                                      app.rivet:26   operation
//!  ├── call users.get                                  app.rivet:30   call (callee expanded)
//!  │   ├── env      read         env API_KEY           app.rivet:5    effect
//!  │   └── network  connect GET  https://…/users/{id}  app.rivet:6    effect
//!  ├── if params.save                                  app.rivet:31   branch (conditional)
//!  │   └── file     create       ./out/user.json       app.rivet:32
//!  └── dag                                             app.rivet:34
//!      ├── node a                                      app.rivet:35
//!      └── node b after a                              app.rivet:36   (edge a ─after─▶ b)
//! ```
//!
//! Nothing is evaluated: literal `(request "id" …)` targets, DAG `after` edges,
//! connector calls and effect sites come from the compiled program and its
//! effect catalog; both `if` branches are always shown and marked.

use serde_json::{Value as Json, json};

// vhco:domain GraphQuery { id: string; all: bool }
/// `rivet graph ID [--all]`: private operations need `all`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GraphQuery {
    pub id: String,
    pub all: bool,
}

// vhco:domain GraphNodeKind { operation | call | connector | effect | branch | dag | node | cycle }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphNodeKind {
    Operation,
    /// A literal `(request "id" …)`; its children are the callee's graph.
    Call,
    /// A call into an imported connector method (`crm.tools.search`).
    Connector,
    Effect,
    /// One arm of an `if` (`if COND` or `else`); everything below is conditional.
    Branch,
    Dag,
    Node,
    /// A callee already on the current chain (only reachable through dynamic
    /// dispatch; literal cycles fail `check`).
    Cycle,
}

impl GraphNodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GraphNodeKind::Operation => "operation",
            GraphNodeKind::Call => "call",
            GraphNodeKind::Connector => "connector",
            GraphNodeKind::Effect => "effect",
            GraphNodeKind::Branch => "branch",
            GraphNodeKind::Dag => "dag",
            GraphNodeKind::Node => "node",
            GraphNodeKind::Cycle => "cycle",
        }
    }
}

// vhco:domain GraphNode { id: string; kind: GraphNodeKind; label: string; source: string; condition?: string; conditional: bool; after: string[]; children: GraphNode[] }
#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    /// Stable within one graph (`n0` is the root).
    pub id: String,
    pub kind: GraphNodeKind,
    pub label: String,
    /// `file:line` of the statement, call or declaration.
    pub source: String,
    /// The branch condition (`params.save`, `not params.save` for `else`).
    pub condition: Option<String>,
    /// True when the node only runs under some `if` arm.
    pub conditional: bool,
    /// DAG node names this node runs after.
    pub after: Vec<String>,
    pub children: Vec<GraphNode>,
}

// vhco:domain CallGraph { operation_id: string; root: GraphNode }
#[derive(Clone, Debug, PartialEq)]
pub struct CallGraph {
    pub operation_id: String,
    pub root: GraphNode,
}

impl CallGraph {
    /// `{operation_id, nodes: [...], edges: [{from, to, kind: contains|after}]}`.
    pub fn to_json(&self) -> Json {
        fn walk(n: &GraphNode, nodes: &mut Vec<Json>, edges: &mut Vec<Json>) {
            nodes.push(json!({
                "id": n.id,
                "kind": n.kind.as_str(),
                "label": n.label,
                "source": n.source,
                "condition": n.condition,
                "conditional": n.conditional,
                "after": n.after,
            }));
            for c in &n.children {
                edges.push(json!({"from": n.id, "to": c.id, "kind": "contains"}));
                if n.kind == GraphNodeKind::Dag {
                    for dep in &c.after {
                        let named = |s: &&GraphNode| {
                            s.kind == GraphNodeKind::Node
                                && s.label
                                    .strip_prefix("node ")
                                    .and_then(|l| l.split(' ').next())
                                    == Some(dep.as_str())
                        };
                        if let Some(d) = n.children.iter().find(named) {
                            edges.push(json!({"from": d.id, "to": c.id, "kind": "after"}));
                        }
                    }
                }
                walk(c, nodes, edges);
            }
        }
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        walk(&self.root, &mut nodes, &mut edges);
        json!({
            "operation_id": self.operation_id,
            "root": self.root.id,
            "nodes": nodes,
            "edges": edges,
        })
    }

    /// The tree (`├──`, `└──`, `│`) with sources right-aligned in one column.
    pub fn render(&self) -> String {
        fn lines(n: &GraphNode, prefix: &str, head: &str, out: &mut Vec<(String, String)>) {
            out.push((format!("{head}{}", n.label), n.source.clone()));
            for (i, c) in n.children.iter().enumerate() {
                let last = i + 1 == n.children.len();
                let (branch, cont) = if last {
                    ("└── ", "    ")
                } else {
                    ("├── ", "│   ")
                };
                lines(
                    c,
                    &format!("{prefix}{cont}"),
                    &format!("{prefix}{branch}"),
                    out,
                );
            }
        }
        let mut rows = Vec::new();
        lines(&self.root, "", "", &mut rows);
        let width = rows
            .iter()
            .map(|(l, _)| l.chars().count())
            .max()
            .unwrap_or(0)
            + 2;
        let mut out = String::new();
        for (label, source) in rows {
            let pad = width - label.chars().count();
            out.push_str(&format!("{label}{}{source}\n", " ".repeat(pad)));
        }
        out
    }
}
