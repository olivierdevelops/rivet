use super::ports::DagNodeRunner;
use crate::domain::contracts::rfc3339_millis;
use crate::domain::dag::{DagCompletion, DagInput, NodeState, NodeStatus};
use crate::domain::errors::{EffectsStatus, ErrorKind};
use crate::domain::ir::FailurePolicy;
use crate::domain::{RivetError, Value};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::time::{Duration, Instant};

// vhco:usecase execution.run_dag(input: DagInput) -> DagCompletion needs DagNodeRunner
// vhco:label Run a DAG
// vhco:about Schedules declared nodes by their `after` edges within the dag `limit`: ready nodes run concurrently, failed or blocked dependencies block descendants, `fail fast` (the default) cancels running nodes and skips the rest, `fail independent` lets unaffected branches finish, and every node ends with a status envelope.
// vhco:example input={nodes:[{id:"a"},{id:"b",after:["a"]}], failure:"fast", limit:2} => { "nodes": [{ "id": "a", "status": "succeeded" }, { "id": "b", "status": "succeeded" }] }
pub async fn run_dag(input: DagInput, runner: &dyn DagNodeRunner) -> DagCompletion {
    let n = input.nodes.len();
    let index_of = |name: &str| input.nodes.iter().position(|x| x.id == name);
    // vhco:todo plan_nodes -- cycles, unknown `after` names and undeclared node reads were rejected at compile time; here every node starts pending, the failure policy defaults to fail fast when omitted, and node identities are their declared names
    // vhco:step init state -- all nodes pending; the dag deadline starts now
    let mut state = vec![NodeState::Pending; n];
    let mut results = vec![Value::Null; n];
    let mut errors: Vec<Option<RivetError>> = vec![None; n];
    let mut started: Vec<Option<String>> = vec![None; n];
    let mut ended: Vec<Option<String>> = vec![None; n];
    let mut fatal: Option<RivetError> = None;
    let limit = input.limit.max(1);
    let deadline = input
        .timeout_ms
        .map(|ms| Instant::now() + Duration::from_millis(ms));
    {
        let mut running = FuturesUnordered::new();
        loop {
            // vhco:todo schedule_ready -- a node whose dependency failed, was blocked, cancelled or skipped becomes blocked; a pending node whose dependencies all succeeded becomes running while fewer than `limit` nodes run; under fail fast no new node starts after the first failure
            // vhco:step block propagate -- failure spreads to descendants before scheduling
            let mut changed = true;
            while changed {
                changed = false;
                for i in 0..n {
                    let dep_failed = input.nodes[i].after.iter().any(|d| {
                        index_of(d).is_some_and(|j| {
                            matches!(
                                state[j],
                                NodeState::Failed
                                    | NodeState::Blocked
                                    | NodeState::Cancelled
                                    | NodeState::Skipped
                            )
                        })
                    });
                    if state[i] == NodeState::Pending && dep_failed {
                        state[i] = NodeState::Blocked;
                        changed = true;
                    }
                }
            }
            // vhco:step start runner.run_node -- ready nodes get their dependencies' results
            if fatal.is_none() {
                for i in 0..n {
                    if running.len() >= limit {
                        break;
                    }
                    let ready = state[i] == NodeState::Pending
                        && input.nodes[i]
                            .after
                            .iter()
                            .all(|d| index_of(d).is_some_and(|j| state[j] == NodeState::Succeeded));
                    if ready {
                        state[i] = NodeState::Running;
                        let deps: Vec<(String, Value)> = input
                            .nodes
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| state[*j] == NodeState::Succeeded)
                            .map(|(j, spec)| {
                                (
                                    spec.id.clone(),
                                    NodeStatus {
                                        id: spec.id.clone(),
                                        status: NodeState::Succeeded,
                                        result: results[j].clone(),
                                        error: None,
                                        started_at: started[j].clone(),
                                        ended_at: ended[j].clone(),
                                    }
                                    .envelope(),
                                )
                            })
                            .collect();
                        // vhco:step stamp runner.now -- started_at is the wall clock when the node starts running
                        started[i] = Some(rfc3339_millis(runner.now()));
                        running.push(async move { (i, runner.run_node(i, deps).await) });
                    }
                }
            }
            // After a fatal failure (and one propagation pass) stop scheduling;
            // still-running siblings are cancelled and joined below.
            if running.is_empty() || fatal.is_some() {
                break;
            }
            // vhco:step wait next -- the next finished node, bounded by the dag timeout
            let next = match deadline {
                Some(d) => match tokio::time::timeout(
                    d.saturating_duration_since(Instant::now()),
                    running.next(),
                )
                .await
                {
                    Ok(x) => x,
                    Err(_) => {
                        // vhco:error dag_timeout -- the dag timeout expired => timeout.dag becomes the fatal error; running nodes are cancelled
                        fatal = Some(RivetError::new(
                            ErrorKind::Timeout,
                            "timeout.dag",
                            "dag exceeded its timeout",
                        ));
                        break;
                    }
                },
                None => running.next().await,
            };
            let Some((i, r)) = next else { break };
            ended[i] = Some(rfc3339_millis(runner.now()));
            match r {
                Ok(v) => {
                    state[i] = NodeState::Succeeded;
                    results[i] = v;
                }
                Err(mut e) => {
                    state[i] = NodeState::Failed;
                    e.node_id = Some(input.nodes[i].id.clone());
                    // vhco:error node_failed_fast -- under fail fast the first failure becomes the fatal error and stops scheduling
                    if input.failure == FailurePolicy::Fast && fatal.is_none() {
                        fatal = Some(e.clone());
                    }
                    errors[i] = Some(e);
                }
            }
        }
        // vhco:step cancel runner.cancel_nodes -- after a fatal error (fail fast or dag timeout) running nodes are signalled and joined: each unwinds its own scopes (closing handles) and reports back within the cleanup grace
        if fatal.is_some() && !running.is_empty() {
            runner.cancel_nodes();
            while let Some((i, r)) = running.next().await {
                ended[i] = Some(rfc3339_millis(runner.now()));
                match r {
                    Ok(v) => {
                        state[i] = NodeState::Succeeded;
                        results[i] = v;
                    }
                    Err(mut e) => {
                        e.node_id = Some(input.nodes[i].id.clone());
                        state[i] = if matches!(e.kind, ErrorKind::Cancelled | ErrorKind::Timeout) {
                            NodeState::Cancelled
                        } else {
                            NodeState::Failed
                        };
                        errors[i] = Some(e);
                    }
                }
            }
        }
    }
    // vhco:todo report_nodes -- still-running nodes become cancelled; never-started nodes become skipped after a fatal error and blocked otherwise; each node's envelope is {status, result (null unless succeeded), error}; the fatal error carries the node list in details
    // vhco:step settle state -- final statuses and envelopes
    for s in state.iter_mut() {
        match *s {
            NodeState::Running => *s = NodeState::Cancelled,
            NodeState::Pending => {
                *s = if fatal.is_some() {
                    NodeState::Skipped
                } else {
                    NodeState::Blocked
                }
            }
            _ => {}
        }
    }
    let nodes: Vec<NodeStatus> = input
        .nodes
        .iter()
        .enumerate()
        .map(|(i, spec)| NodeStatus {
            id: spec.id.clone(),
            status: state[i],
            result: results[i].clone(),
            error: errors[i].clone(),
            started_at: started[i].clone(),
            ended_at: ended[i].clone(),
        })
        .collect();
    // The node list is attached whether or not the failing node's error already
    // carried object details (`fail "code" {}` must not hide the node statuses).
    if let Some(e) = &mut fatal {
        let list = Value::List(nodes.iter().map(NodeStatus::summary).collect());
        match &mut e.details {
            Value::Null => e.details = Value::object([("nodes", list)]),
            d @ Value::Object(_) if d.get("nodes").is_none() => d.set("nodes", list),
            _ => {}
        }
    }
    DagCompletion {
        nodes,
        fatal,
        effects: EffectsStatus::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dag::DagNodeSpec;
    use async_trait::async_trait;

    struct Runner(Vec<Result<i64, &'static str>>);

    #[async_trait]
    impl DagNodeRunner for Runner {
        async fn run_node(
            &self,
            index: usize,
            dependencies: Vec<(String, Value)>,
        ) -> Result<Value, RivetError> {
            let _ = dependencies;
            tokio::task::yield_now().await;
            match self.0[index] {
                Ok(v) => Ok(Value::Int(v)),
                Err(code) => Err(RivetError::new(ErrorKind::Application, code, "boom")),
            }
        }

        fn now(&self) -> std::time::SystemTime {
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
        }

        fn cancel_nodes(&self) {}
    }

    fn spec(id: &str, after: &[&str]) -> DagNodeSpec {
        DagNodeSpec {
            id: id.into(),
            after: after.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn statuses(c: &DagCompletion) -> Vec<&'static str> {
        c.nodes.iter().map(|n| n.status.as_str()).collect()
    }

    // vhco:test execution.run_dag -- fail independent keeps the independent success and blocks the failed node's descendant
    #[tokio::test]
    async fn independent_failure_blocks_descendants() {
        let input = DagInput {
            nodes: vec![spec("good", &[]), spec("bad", &[]), spec("child", &["bad"])],
            failure: FailurePolicy::Independent,
            limit: 2,
            timeout_ms: None,
        };
        let c = run_dag(input, &Runner(vec![Ok(8), Err("x.fail"), Ok(1)])).await;
        assert_eq!(statuses(&c), vec!["succeeded", "failed", "blocked"]);
        assert!(c.fatal.is_none());
        assert_eq!(c.nodes[0].envelope().get("result"), Some(&Value::Int(8)));
    }

    /// Node 0 is slow (would take 30 s unless cancelled); every other node fails at once.
    struct SlowSibling(crate::domain::cancel::CancelToken);

    #[async_trait]
    impl DagNodeRunner for SlowSibling {
        async fn run_node(
            &self,
            index: usize,
            _dependencies: Vec<(String, Value)>,
        ) -> Result<Value, RivetError> {
            if index == 0 {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => return Ok(Value::Int(1)),
                    _ = self.0.cancelled() => {
                        return Err(RivetError::new(ErrorKind::Cancelled, "cancelled.request", "cancelled"));
                    }
                }
            }
            Err(RivetError::new(ErrorKind::Application, "x.fail", "boom"))
        }

        fn now(&self) -> std::time::SystemTime {
            std::time::SystemTime::now()
        }

        fn cancel_nodes(&self) {
            self.0
                .cancel(crate::domain::cancel::CancelReason::Cancelled);
        }
    }

    // vhco:test execution.run_dag -- regression: under fail fast a running sibling is cancelled at the first failure instead of awaited (S138: user cancelled, enrich skipped, summary blocked)
    #[tokio::test]
    async fn fail_fast_cancels_running_siblings() {
        let input = DagInput {
            nodes: vec![
                spec("user", &[]),
                spec("orders", &[]),
                spec("enrich", &["user"]),
                spec("summary", &["user", "orders"]),
            ],
            failure: FailurePolicy::Fast,
            limit: 2,
            timeout_ms: None,
        };
        let started = Instant::now();
        let c = run_dag(input, &SlowSibling(Default::default())).await;
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "must not wait for the slow sibling"
        );
        assert_eq!(
            statuses(&c),
            vec!["cancelled", "failed", "skipped", "blocked"]
        );
        assert_eq!(c.fatal.as_ref().unwrap().code, "x.fail");
        // The cancelled sibling was joined: it has both timestamps; the skipped one none.
        assert!(c.nodes[0].started_at.is_some() && c.nodes[0].ended_at.is_some());
        assert!(c.nodes[2].started_at.is_none());
    }

    // vhco:test execution.run_dag -- G14: nodes that ran carry RFC 3339 started_at/ended_at in DagCompletion, the envelope and the fatal error's node list
    #[tokio::test]
    async fn node_timestamps_are_reported() {
        let input = DagInput {
            nodes: vec![spec("a", &[]), spec("b", &["a"]), spec("c", &["b"])],
            failure: FailurePolicy::Fast,
            limit: 2,
            timeout_ms: None,
        };
        let c = run_dag(input, &Runner(vec![Ok(1), Err("x.fail"), Ok(3)])).await;
        let a = &c.nodes[0];
        assert_eq!(a.started_at.as_deref(), Some("2023-11-14T22:13:20.000Z"));
        assert_eq!(a.ended_at.as_deref(), Some("2023-11-14T22:13:20.000Z"));
        assert_eq!(
            a.envelope().get("started_at"),
            Some(&Value::text("2023-11-14T22:13:20.000Z"))
        );
        assert_eq!(c.nodes[2].envelope().get("started_at"), Some(&Value::Null));
        let details = &c.fatal.as_ref().unwrap().details;
        let Some(Value::List(nodes)) = details.get("nodes") else {
            panic!("fatal error must list the nodes: {details:?}")
        };
        assert!(nodes[1].get("ended_at").is_some());
        assert!(nodes[2].get("started_at").is_none());
    }

    // vhco:test execution.run_dag -- fail fast (the default) makes the first failure fatal and skips never-started nodes
    #[tokio::test]
    async fn fail_fast_skips_the_rest() {
        let input = DagInput {
            nodes: vec![
                spec("bad", &[]),
                spec("later", &["bad"]),
                spec("other", &[]),
            ],
            failure: FailurePolicy::Fast,
            limit: 1,
            timeout_ms: None,
        };
        let c = run_dag(input, &Runner(vec![Err("x.fail"), Ok(1), Ok(2)])).await;
        assert_eq!(c.fatal.as_ref().unwrap().node_id.as_deref(), Some("bad"));
        assert_eq!(statuses(&c), vec!["failed", "blocked", "skipped"]);
    }
}
