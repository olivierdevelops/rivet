//! DAG execution contract (R10; PROP-2026-0001 Increment 7).
//!
//! ```text
//!  pending ──deps succeeded──▶ ready ──▶ running ──▶ succeeded
//!     │                                     ├──────▶ failed
//!     ├──dep failed/blocked──▶ blocked      └──────▶ cancelled (fail fast / timeout)
//!     └──fail fast before ready──▶ skipped
//! ```

use super::errors::{EffectsStatus, RivetError};
use super::ir::FailurePolicy;
use super::value::Value;
use async_trait::async_trait;

// vhco:domain NodeState { pending | running | succeeded | failed | cancelled | blocked | skipped }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Blocked,
    Skipped,
}

impl NodeState {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeState::Pending => "pending",
            NodeState::Running => "running",
            NodeState::Succeeded => "succeeded",
            NodeState::Failed => "failed",
            NodeState::Cancelled => "cancelled",
            NodeState::Blocked => "blocked",
            NodeState::Skipped => "skipped",
        }
    }
}

// vhco:domain DagNodeSpec { id: String; after: List<String> }
#[derive(Clone, Debug, PartialEq)]
pub struct DagNodeSpec {
    pub id: String,
    pub after: Vec<String>,
}

// vhco:domain DagInput { nodes: List<DagNodeSpec>; failure: FailurePolicy; limit: usize; timeout_ms: Option<u64> }
#[derive(Clone, Debug, PartialEq)]
pub struct DagInput {
    pub nodes: Vec<DagNodeSpec>,
    pub failure: FailurePolicy,
    pub limit: usize,
    pub timeout_ms: Option<u64>,
}

// vhco:domain NodeStatus { id: String; status: NodeState; result: Value; error: Option<RivetError>; started_at: Option<String>; ended_at: Option<String> }
#[derive(Clone, Debug, PartialEq)]
pub struct NodeStatus {
    pub id: String,
    pub status: NodeState,
    pub result: Value,
    pub error: Option<RivetError>,
    /// RFC 3339 (UTC, ms) when the node started running; absent if it never ran.
    pub started_at: Option<String>,
    /// RFC 3339 (UTC, ms) when the node finished (succeeded, failed or cancelled).
    pub ended_at: Option<String>,
}

impl NodeStatus {
    /// `{id, status, started_at?, ended_at?}` — the per-node summary carried by
    /// a fatal DAG error's details and by traces.
    pub fn summary(&self) -> Value {
        let mut v = Value::object([
            ("id", Value::text(&self.id)),
            ("status", Value::text(self.status.as_str())),
        ]);
        if let Some(t) = &self.started_at {
            v.set("started_at", Value::text(t));
        }
        if let Some(t) = &self.ended_at {
            v.set("ended_at", Value::text(t));
        }
        v
    }

    /// The `{status, result, error, started_at, ended_at}` envelope a node name evaluates to.
    pub fn envelope(&self) -> Value {
        let time = |t: &Option<String>| t.as_ref().map(Value::text).unwrap_or(Value::Null);
        Value::object([
            ("status", Value::text(self.status.as_str())),
            (
                "result",
                if self.status == NodeState::Succeeded {
                    self.result.clone()
                } else {
                    Value::Null
                },
            ),
            (
                "error",
                self.error
                    .as_ref()
                    .map(|e| e.to_value())
                    .unwrap_or(Value::Null),
            ),
            ("started_at", time(&self.started_at)),
            ("ended_at", time(&self.ended_at)),
        ])
    }
}

// vhco:domain DagCompletion { nodes: List<NodeStatus>; fatal: Option<RivetError>; effects: EffectsStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct DagCompletion {
    pub nodes: Vec<NodeStatus>,
    /// Set under fail fast (first failure) or when the dag timeout expired.
    pub fatal: Option<RivetError>,
    pub effects: EffectsStatus,
}

/// Runs one node's expression with the envelopes of its completed dependencies
/// in scope. Implemented by the interpreter; the DAG use case owns scheduling.
#[async_trait]
pub trait DagNodeRunner: Send + Sync {
    /// Evaluate node `index`. After [`DagNodeRunner::cancel_nodes`] a running
    /// node unwinds (closing its resources) and returns within the cleanup grace.
    async fn run_node(
        &self,
        index: usize,
        dependencies: Vec<(String, Value)>,
    ) -> Result<Value, RivetError>;
    /// Wall clock for the nodes' `started_at` / `ended_at`.
    fn now(&self) -> std::time::SystemTime;
    /// Signal every running node to cancel (fail fast, dag timeout).
    fn cancel_nodes(&self);
}

/// The DAG scheduler as seen by the interpreter (the orchestrator injects the
/// `execution.run_dag` use case behind it).
#[async_trait]
pub trait DagExecutor: Send + Sync {
    async fn run(&self, input: DagInput, runner: &dyn DagNodeRunner) -> DagCompletion;
}
