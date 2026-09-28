---
document_id: SYS-2026-0002
title: "Rivet execution, scopes and DAG runtime"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [execution, files]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: How one request is dispatched, bounded, interpreted, scoped, cancelled and validated, including file operations and DAG scheduling, as implemented in the Rivet 0.1.0 runtime.
reason: Every surface (CLI, HTTP, SSE, polling, WebSocket, MCP, library) funnels into one dispatcher and one interpreter; maintainers need the current lifecycle, limits, scope cleanup, DAG states, cancellation paths and exit codes in one verified place (PLAN-2026-0001 D-16).
related_documents: [PROP-2026-0001, PLAN-2026-0001, SYS-2026-0001, SYS-2026-0003, SYS-2026-0004, SYS-2026-0005, SYS-2026-0007, SYS-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, system, execution, dag, scopes, cancellation, files, errors]
---

# Rivet execution, scopes and DAG runtime

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** execution, files
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

## Summary

Rivet runs every operation through **one dispatcher** (`Runtime::dispatch_request` in
`src/orchestrator/runtime.rs`) that calls **one use case** (`execution.request_operation` in
`src/features/execution/request_operation.rs`) and **one interpreter** (`Interpreter` in
`src/infra/execution_driver.rs`). The dispatcher enforces principal access, the host-wide
concurrency budget, the call-depth limit, the request deadline and cancellation. The
interpreter walks the compiled operation body inside a single request scope. Resource
handles opened with `with … as NAME` belong to the scope that opened them. DAG blocks are
scheduled by the `execution.run_dag` use case (`src/features/execution/run_dag.rs`). File
effects go through `files.apply_file_operation` (`src/features/files/apply_file_operation.rs`)
into a confined, no-follow adapter (`src/infra/file_access.rs`). Every outcome is exactly one
`Completion` or one `RivetError`, whose kind fixes the CLI exit code and HTTP status
(`src/domain/errors.rs`).

```text
   CLI  HTTP  SSE  poll  WS  MCP  library          (surfaces: SYS-2026-0004)
     \    \    |    |    |   /    /
      \    \   |    |    |  /    /
       v    v  v    v    v v    v
   +-----------------------------------------------+
   | Runtime::dispatch_request   (orchestrator)    |
   |   principal check · rivet.* built-ins ·       |
   |   concurrency permit · cancel registration    |
   +----------------------+------------------------+
                          v
   +-----------------------------------------------+
   | execution.request_operation (use case)        |
   |   resolve · depth · params · drive · output   |
   +----------------------+------------------------+
                          v
   +-----------------------------------------------+
   | Interpreter::drive (infra, ExecutionDriver)   |
   |   deadline · frame/scopes · with-handles ·    |
   |   dag / concurrent / map / poll · effects     |
   +---+-----------+-------------+-----------------+
       |           |             |
       v           v             v
   run_dag     FileAccess     EffectAdapter (HTTP, sockets, process,
   use case    (PolicedFiles   gRPC, QUIC, UDP ... SYS-2026-0005)
               -> apply_file_operation -> ConfinedFiles)
```

## Responsibilities

| Responsibility | Where | Notes |
|---|---|---|
| Accept a fully formed `Request` from any surface | `src/orchestrator/runtime.rs` `dispatch_request` | IDs minted by `new_request` (`req_…`, `tr_…`) |
| Authorize the operation for the principal (top level only) | `require_operation` (`src/features/serve/authorize_operation.rs`) | `local` principal always allowed |
| Route `rivet.*` built-ins | `src/orchestrator/builtins.rs` | before any budget is taken |
| Host-wide concurrency budget | `Inner.concurrency` (`tokio::sync::Semaphore`) | top-level requests only |
| Resolve ID, check depth, validate params, inject defaults | `request_operation`, `validate_params` | unknown fields rejected before defaults |
| Drive the compiled body under the request deadline | `Interpreter::drive` | `timeout.request` |
| Own and close scoped resources | `Machine::run_with`, `CLEANUP_GRACE` | 5 s per handle |
| Schedule DAG nodes | `run_dag` use case + `NodeRunner` | fail fast default, envelopes |
| Cancel a running top-level request by ID | `execution.cancel_request` + `RunningRequests` | Ctrl-C → exit 130 |
| Validate the final result against the declared output | `execution.validate_output` | `output.invalid` |
| Confined file CRUD with per-intent authorization | `files.apply_file_operation`, `ConfinedFiles` | cap-std, no-follow |
| Single error model, exit codes, HTTP statuses | `src/domain/errors.rs` | 22 kinds |

## Boundaries and Non-Responsibilities

- **Parsing, lowering and catalog building** belong to the compiler (SYS-2026-0001). The
  interpreter receives an already checked `CompiledProgram`.
- **Policy decisions** belong to the policy broker (SYS-2026-0003). Execution only builds
  `EffectIntent`s and honours `Permit`s.
- **Wire protocols** (HTTP, sockets, processes, gRPC, QUIC, UDP, MCP client) are
  `EffectAdapter`s documented in SYS-2026-0005 and SYS-2026-0009.
- **Transport framing of results** (JSON, SSE, WS frames, polling batches, MCP tool results)
  belongs to the surfaces (SYS-2026-0004). Live input sessions are in SYS-2026-0007.
- Execution does **not** retry effects, persist state, resume after restart, or run
  compensations. There is no `finally` block in 0.1.0 (a known limitation).

## Architecture

### Module map

```text
 src/
 ├─ orchestrator/runtime.rs        Runtime, RuntimeBuilder, dispatch_request, cancel,
 │                                 NestedDispatcher (nested (request …) re-entry), DagUseCase
 ├─ features/execution/
 │   ├─ request_operation.rs       use case: resolve → depth → params → drive → validate
 │   ├─ validate_output.rs         use case: structural result check
 │   ├─ run_dag.rs                 use case: DAG scheduling and node states
 │   ├─ cancel_request.rs          use case: owner-checked cancel signal
 │   └─ ports.rs                   ExecutionDriver, RequestControl, DagNodeRunner
 ├─ features/files/
 │   ├─ apply_file_operation.rs    use case: intents → evaluator → FileAccess
 │   └─ open_file_stream.rs        use case: `with file open` mode/chunk checks + authorization
 ├─ domain/
 │   ├─ cancel.rs                  CancelToken (tree of tokens), CancelReason cancelled|timeout
 │   ├─ dag.rs                     NodeState, DagInput, NodeStatus.envelope() (+ started_at/ended_at), DagCompletion
 │   ├─ files.rs                   FileVerb, Codec, FileOperation
 │   ├─ errors.rs                  ErrorKind registry, RivetError, EffectsStatus
 │   ├─ contracts.rs               Request, Completion, DEFAULT_DEADLINE_MS = 30 000
 │   └─ policy.rs                  PolicyLimits (64 / 16 / 256 MiB)
 └─ infra/
     ├─ execution_driver.rs        Interpreter (ExecutionDriver), Machine, Frame,
     │                             ResourceHandle, CLEANUP_GRACE = 5 s, DEADLINE_BACKSTOP = 250 ms,
     │                             NodeRunner, secret taint (SecretTaint, sink_guard)
     ├─ file_stream.rs             FileStreams: `with file open` handles (read chunks / write / append)
     ├─ request_control.rs         RunningRequests (RequestControl), recent ring of 1024
     └─ file_access.rs             ConfinedFiles (FileAccess), MAX_READ_BYTES = 8 MiB,
                                   locked compare-and-replace (flock, LOCK_WAIT 10 s)
```

### Request lifecycle (sequence)

```text
 surface          Runtime::dispatch_request     request_operation      Interpreter::drive      validate_output
    |  Request{id,params,principal,deadline_ms,depth=0}                                              |
    |------------------>|                            |                         |                     |
    |                   | depth==0: require_operation (serve.principals)       |                     |
    |                   |   denied -> permission.denied (403 / exit 3)         |                     |
    |                   | id starts "rivet." -> builtins::dispatch_builtin     |                     |
    |                   | depth==0: concurrency.try_acquire_owned()            |                     |
    |                   |   none left -> limit.concurrency (429 / exit 5)      |                     |
    |                   | depth==0: RunningRequests.start(id, owner, token)    |                     |
    |                   |---- select { run , token fired + grace (last resort) } |                   |
    |                   |                            | registry.describe(id)   |                     |
    |                   |                            |   miss -> not_found.operation                 |
    |                   |                            | depth > max_call_depth -> limit.call_depth    |
    |                   |                            | validate_params (unknown/required/type/       |
    |                   |                            |   min/max/enum, then defaults)                |
    |                   |                            | receives && no sink -> stream.input_required  |
    |                   |                            |------ drive(plan, sink) --->|                 |
    |                   |                            |                         | deadline = now+deadline_ms
    |                   |                            |                         | params -> frame scope 0
    |                   |                            |                         | exec body; deadline+250 ms
    |                   |                            |                         |   fires the token (timeout)
    |                   |                            |                         |  emit -> check `emits` -> sink.send
    |                   |                            |                         |  return -> secret-taint check
    |                   |                            |<---- RunOutcome{result,data_count,effects}   |
    |                   |                            |---------------------------------------------->|
    |                   |                            |<-------- verdict (valid | output.invalid) ----|
    |                   |<-- Completion | RivetError-|                         |                     |
    |                   | RunningRequests.finish(id, succeeded|failed|cancelled)                     |
    |<-- exactly one ---|                            |                         |                     |
```

Nested `(request "ID" {…})` and `with (request.stream "ID" {…}) as NAME` build a child
`Request` (`depth + 1`, `include_private: true`, same principal and trace ID, request ID
`<parent>.<suffix>`, a **child cancellation token** of the caller's) and re-enter
`dispatch_request` through `NestedDispatcher`. Nested calls skip the principal check and the
concurrency permit (they run inside the parent's permit), but pass the depth check and
parameter validation, and inherit every per-request `restrict` on the task's restriction stack. Their `deadline_ms` is the parent's
**remaining** time, so children never outlive the parent deadline.

### Scope tree

One request is one task. Everything below it runs cooperatively inside that task and is
dropped with it.

```text
 request req_01…  (Interpreter::drive, deadline D)
 └─ Frame  scopes[0] = params + operation-level assignments
    ├─ block scope (if / else / for / while / iterate / try / scope timeout "…")
    │   └─ loop variables, `error` in catch — block-local (Frame::define)
    ├─ with KIND … as a                       handle a  (Frame.handles, innermost last;
    │                                                    KIND includes `file open PATH mode M`)
    │   └─ with KIND … as b                   handle b
    │       └─ body …                         exit order: close b, then close a
    ├─ dag limit N [timeout "T"] [fail fast|fail independent]
    │   ├─ node x  (NodeRunner: copy of frame + dependency envelopes)
    │   └─ node y after [x]
    ├─ concurrent limit N fail fast|independent [timeout]
    │   ├─ task send     (frame clone, own block scope)
    │   └─ task receive
    ├─ map item in LIST limit N  (yield per item, default limit 4)
    ├─ poll every "E" timeout "T"  (until COND, yield)
    └─ (request "child" {…})   ──▶ child request req_01….N   (depth+1, remaining deadline)
```

Assignment rules (`Frame::assign` / `Frame::define`): parameters, loop variables and `error`
are block bindings; a new name assigned inside a block becomes an operation-level variable,
visible after the block ends.

### Resource handles and with-block cleanup

`with KIND … as NAME … end` (`Machine::run_with`) opens a `Box<dyn ResourceHandle>`
through the adapter registered for `KIND` (or the built-in `request.stream`, or
`with PARENT.child … as NAME` via `open_child`). The handle is pushed on
`Frame.handles`, the body runs in a new block scope, then the handle is **always** closed:

```text
 open ──▶ push handle ──▶ run body ──┬─ Ok(flow)      ─┐
                                     ├─ Err(e)         ├─▶ remove handle ─▶ close() within 5 s
                                     ├─ break / return ┘        │
                                     │                          ├─ closed ok, body ok   -> flow
                                     │                          ├─ closed ok, body err  -> e
                                     │                          ├─ close err, body ok   -> cleanup.failed
                                     │                          ├─ close > 5 s          -> cleanup.timeout
                                     │                          └─ both failed          -> e (+ suppressed[close err])
```

- Reverse acquisition order is structural: an inner `with` finishes (and closes) before the
  outer one resumes, so handles close innermost first.
- Handles are not values: they cannot be returned, stored or passed to DAG nodes.
- `CLEANUP_GRACE` (5 s) bounds each `close()` on normal exit, error, `break` and `return`.
- When the request is **cancelled or its deadline fires**, cancellation is structured: the
  request's `CancelToken` fires, every await point and handle operation observes it, the
  body unwinds through its `with` blocks and each `close()` runs with the **rest of the
  grace** (`RunState::close_budget`, at least 50 ms); child processes are reaped. Only if the
  body is still running when the grace (+ 250 ms backstop) ends is the future dropped, and
  the terminal error then carries `cleanup.timeout` under `suppressed`.

```text
 cancel / deadline ─▶ token.cancel(Cancelled|Timeout) ─▶ children tokens fire too
        │                (nested requests, DAG nodes, concurrent tasks, request.stream)
        ▼
 body unwinds ─▶ with c: close()  ─▶ with b: close() ─▶ with a: close()   (reverse, ≤ grace)
        │
        ├─ finished within 5 s ─▶ one error: cancelled.request | timeout.request
        └─ still running        ─▶ future dropped (last resort) + suppressed cleanup.timeout
```

`with file open PATH mode read|write|append as NAME` (`src/infra/file_stream.rs`) is
authorized by `files.open_file_stream` per mode (read → `allow_read` read; write →
`allow_write` create + update; append → `allow_write` append), opens a no-follow handle
under the bundle root (reads need a regular file, writes refuse hard links, append never
creates), yields bytes chunks of at most `chunk_size` (default 64 KiB, 1 … 8 MiB) for
`for chunk in NAME`, accepts `NAME.write text|bytes V`, and flushes + syncs on close.
`with file watch` is Stage C (`unsupported.stage_c`).

### DAG node state machine

`execution.run_dag` (`src/features/execution/run_dag.rs`) owns scheduling; the interpreter
supplies a `NodeRunner` that evaluates one node's expression in a copy of the enclosing
frame with its completed dependencies' envelopes in scope.

```text
                    all `after` deps succeeded
                    and running < limit
                    and no fatal error yet
  ┌─────────┐ ─────────────────────────────▶ ┌─────────┐  Ok(v)   ┌───────────┐
  │ pending │                                │ running │ ───────▶ │ succeeded │
  └─────────┘                                └─────────┘          └───────────┘
    │     │                                    │     │  Err(e)    ┌────────┐
    │     │ a dep is failed / blocked /        │     └──────────▶ │ failed │ (fail fast: first
    │     │ cancelled / skipped                │                  └────────┘  one becomes fatal)
    │     ▼                                    │ still running when the
    │  ┌─────────┐                             │ loop stops (fatal / dag timeout)
    │  │ blocked │                             ▼
    │  └─────────┘                          ┌───────────┐
    │  never started, fatal error set       │ cancelled │
    └────────────────────────────────────▶  └───────────┘
                    ┌─────────┐
                    │ skipped │   (never started, fatal error set)
                    └─────────┘
  never started, no fatal error (unreachable deps) ─▶ blocked
```

Scheduling loop (per iteration):

```text
 1. propagate: pending node with a failed/blocked/cancelled/skipped dep -> blocked (to fixpoint)
 2. start:     if no fatal: pending nodes whose deps all succeeded -> running (while running < limit)
 3. stop:      nothing running -> exit loop
 4. wait:      next finished node, bounded by dag timeout (timeout.dag -> fatal)
 5. record:    Ok -> succeeded + result ; Err -> failed + error(node_id) ;
               fail fast && first failure -> fatal = that error
 6. settle:    dropping the running set cancels in-flight nodes; running -> cancelled;
               pending -> skipped (fatal) | blocked (no fatal)
```

- Default `limit` is 4 (`options.limit.unwrap_or(4)` in `Machine::run_dag`); default failure
  policy is `fail fast`.
- After the block, **every node name evaluates to its envelope**
  `{status, result, error, started_at, ended_at}` (`NodeStatus::envelope` in
  `src/domain/dag.rs`; RFC 3339 timestamps with milliseconds, `null` for a node that never
  started); `result` is `null` unless `status` is `succeeded` (`rivet check` warns with
  `check.unguarded_result` when a `return` reads it unguarded).
- Under `fail fast` the fatal error (the first failing node's error, with `node_id` set) is
  raised from the `dag` statement, and the scheduler **merges** `details.nodes =
  [{id, status, started_at, ended_at}…]` into that error's existing `details`.
  `fail independent` raises nothing; the body inspects the envelopes.
- A dag `timeout "T"` expiry is `timeout.dag` (exit 6).

### Cancellation propagation

```text
  Ctrl-C (CLI)        rivet.sessions.cancel / WS cancel / poll cancel     SSE disconnect    deadline      serve drain
      │                          │  (SYS-2026-0007)                          │                │               │
      ▼                          ▼                                           ▼                │               ▼
  Runtime::cancel(id, local)  SessionHost.cancel: mark cancel-requested,  AbortOnDrop        │     Runtime::shutdown:
      │                       drop input, fire the session's token        │                  │     cancel_all requests
      ▼                          │                                        │                  │     + sessions
  execution.cancel_request       │                                        │                  │     (cancelled.shutdown)
   lookup(id) owner == principal?│                                        │                  │
   no  -> not_found.request      │                                        │                  │
   terminal -> {state: succeeded|failed|cancelled}                        │                  │
   running -> token.cancel(Cancelled)                                     │     token.cancel(Timeout)
      │                          │                                        │                  │
      ▼                          ▼                                        ▼                  ▼
  the request's CancelToken fires ──▶ every child token fires (nested requests, DAG nodes,
      │                              concurrent tasks, map items, request.stream children)
      ├─ the interpreter observes it at its next await point / handle operation
      ├─ `with` scopes close handles in reverse order within the 5 s grace; processes reaped
      ▼
  one error: cancelled.request | timeout.request   (effects = what actually happened)
      → exit 130 / HTTP 409  (timeout: exit 6 / HTTP 504)
  only after the grace (+ backstop): the remaining future is dropped, cleanup.timeout suppressed
  RunningRequests.finish(id, "cancelled")  (kept in a 1024-entry recent ring for late cancels)
```

## Interfaces

### Ports and use cases

| Item | Signature (from `vhco:` annotations) | File |
|---|---|---|
| use case | `execution.request_operation(Request) -> Completion needs ExecutionDriver` | `src/features/execution/request_operation.rs` |
| use case | `execution.validate_output(OutputCheck) -> OutputVerdict` | `src/features/execution/validate_output.rs` |
| use case | `execution.run_dag(DagInput) -> DagCompletion needs DagNodeRunner` | `src/features/execution/run_dag.rs` |
| use case | `execution.cancel_request(CancelRequest) -> CancelReceipt needs RequestControl` | `src/features/execution/cancel_request.rs` |
| use case | `files.apply_file_operation(FileOperation) -> FileResult needs FileAccess` | `src/features/files/apply_file_operation.rs` |
| port | `ExecutionDriver { drive(ExecutionPlan) -> Completion }` | `src/features/execution/ports.rs` |
| port | `RequestControl { lookup(string) -> RequestState; signal(string) -> bool }` | same |
| port | `DagNodeRunner { run_node(usize, List<(String, Value)>) -> Value }` | same |
| port | `FileAccess { apply(FileOperation) -> FileResult }` | `src/features/files/ports.rs` |
| infra | `Interpreter` satisfies `ExecutionDriver` | `src/infra/execution_driver.rs` |
| infra | `RunningRequests` satisfies `RequestControl` | `src/infra/request_control.rs` |
| infra | `ConfinedFiles` satisfies `FileAccess` | `src/infra/file_access.rs` |

### Library entry points (`src/orchestrator/runtime.rs`)

```text
 Runtime::builder().file(p) | .source(path, text, root)
                   .policy_file(p) | .policy(Policy)   [.ceiling(Policy)]
                   .build()                         -> Runtime
 rt.request(id, params, sink?)                      -> Completion | RivetError  (principal local)
 rt.request_restricted(id, params, restrict, sink?) -> Completion | RivetError  (narrowed)
 rt.scope(|scope| … scope.stream / scope.duplex …)  -> owned handles, cancelled + joined at the end
 rt.new_request(id, params, principal)              -> Request (deadline_ms 30 000)
 rt.dispatch_request(request, sink?)                -> Completion | RivetError
 rt.cancel(request_id, principal)                   -> CancelReceipt {request_id, state}
 rt.shutdown(drain)                                 -> cancel every request and session
```

### CLI

```text
 rivet [--file F] [--policy P] request ID --params JSON [--stream] [--timeout D]
        │                                     │          │           └─ digits + ms|s|m|h → deadline_ms
        │                                     │          └─ NDJSON data envelopes, then a result envelope
        │                                     └─ object; unknown fields rejected
        └─ stdout: Completion JSON (exit 0) · stderr: ErrorEnvelope (registry exit code)
 --timeout above 600000 ms (10m) → validation.usage (exit 2), the same host cap as HTTP deadline_ms
 Ctrl-C while running → execution.cancel_request → cancelled.request → exit 130
```

### HTTP

`POST /v1/request {id, params, deadline_ms?, restrict?}` (SYS-2026-0004). `deadline_ms` is
clamped to `1..=600000` (`MAX_REQUEST_DEADLINE_MS` in `src/io/http/mod.rs`); the CLI's
`--timeout` in `--endpoint` mode is sent as this field (and refused above the cap locally too).

### Completion and error envelopes

```text
 Completion                                    ErrorEnvelope
 {                                             {
   "request_id": "req_…",                        "request_id": "req_…", "trace_id": "tr_…",
   "trace_id":   "tr_…",                         "error": {
   "result":     <validated value>,                "kind": "<ErrorKind>", "code": "<dotted.code>",
   "data_count": <items emitted>,                  "message": "…", "retryable": bool,
   "effects":    "none|committed|unknown"          "effects": "none|committed|partial|unknown",
 }                                                 "source"?, "operation_id"?, "node_id"?,
                                                   "hint"?, "details"?, "cause"?, "suppressed"?
                                                 }
                                               }
```

`effects` only ever rises (`none < committed < partial < unknown`, folded with `fetch_max`):
a nested call's status is folded into its caller's, so an opaque remote MCP tool call
(`unknown`) keeps the wrapping request `unknown` — never `committed` merely because the
remote call returned.

## Configuration

### Limits and where they come from

| Limit | Default | Source | Enforced in | Error |
|---|---|---|---|---|
| `limits.max_concurrent_requests` | 64 | `policy.json` `limits` → `PolicyLimits` (`src/domain/policy.rs`, parsed in `src/features/policy/load_policy.rs`) | `Semaphore` in `dispatch_request` (top-level only, `try_acquire`, no queue) | `limit.concurrency` (429 / 5) |
| `limits.max_call_depth` | 16 | same | `request_operation` (`depth > max`) | `limit.call_depth` (429 / 5) |
| `limits.max_buffered_bytes` | 268 435 456 (256 MiB) | same | host-wide `BufferBudget` reserved by every retained session event (SYS-2026-0007) | `limit.buffered_bytes` (429 / 5) |
| Request deadline | 30 000 ms | `DEFAULT_DEADLINE_MS` (`src/domain/contracts.rs`) | `Interpreter::drive`: deadline + 250 ms fires the token (`Timeout`); a per-statement check in `exec_block` | `timeout.request` (504 / 6) |
| CLI `--timeout D` | — | `request --timeout` → `deadline_ms`, at most 600 000 ms | same | `validation.usage` for a bad duration or above the cap |
| HTTP `deadline_ms` | — | request body, clamped to 1..600 000 | same | — |
| Cleanup grace | 5 s | `CLEANUP_GRACE` (`src/infra/execution_driver.rs`) | `run_with`; after cancellation the remaining grace bounds each close | `cleanup.timeout` |
| `with file open` chunk | 64 KiB (1 … 8 MiB) | `chunk_size N` option | `files.open_file_stream` | `limit.chunk_size` |
| File lock wait (`update`/`write` of an existing file) | 10 s | `LOCK_WAIT` (`src/infra/file_access.rs`) | `locked_replace` | `timeout.file_lock` |
| DAG / map concurrency | 4 | `limit N` option, else 4 | `run_dag`, `run_map` | — |
| `concurrent` concurrency | number of tasks | `limit N` option | `run_concurrent` | — |
| `poll` interval / timeout | 1 s / 30 s | `every` / `timeout` options | `run_poll` | `timeout.poll` |
| `scope timeout "D"` / dag / concurrent `timeout` | none | source | `Stmt::Scope`, `run_dag`, `run_concurrent` | `timeout.scope` / `timeout.dag` / `timeout.concurrent` |
| `request.stream` channel | 16 items | constant in `open_request_stream` | bounded `mpsc` (backpressure) | — |
| File read size | 8 MiB | `MAX_READ_BYTES` (`src/infra/file_access.rs`) | `read_all` | `limit.file_size` |
| Recent-request ring | 1024 | `RECENT` (`src/infra/request_control.rs`) | late `cancel` answers | — |

`policy.json` `limits` keys must be positive integers; unknown keys are `policy.invalid`
(schema: SYS-2026-0008). Verified effect of lowering `max_call_depth`:

```sh
# scratch bundle: chain.a -> (request "chain.b") -> (request "chain.c")
# policy.json: {"version":1,"grants":[],"deny":[],"limits":{"max_call_depth":1}}
$ rivet --file app.rivet request chain.a --params '{}'
{"request_id":"req_0197e00f65.1","trace_id":"tr_0197e00f65","error":{"kind":"limit","code":"limit.call_depth","message":"call depth 2 exceeds limits.max_call_depth 1","retryable":true,"effects":"none","source":{"file":"app.rivet","line":11,"column":13,"end_line":11,"end_column":20},"operation_id":"chain.b"}}
exit=5
$ rivet --file app.rivet policy explain chain.a --params '{}'
policy   ./policy.json (sha256:b2c04073986cca43ac35cebf62717a7db34703d914fd9d219ce3716389abe76c)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 1, 268435456 buffered bytes
```

(Request and trace IDs vary per run; the `.1` suffix marks the nested child request.)

## Runtime Behaviour

### Parameter validation

`validate_params` (`src/features/execution/request_operation.rs`) runs before any effect:

```text
 params not object/null  -> validation.params
 unknown key             -> validation.unknown_field  details {field, known[]}
 missing required        -> validation.required       details {field}
 wrong type              -> validation.type           (integers accept whole floats < 9e15)
 below min / above max   -> validation.min / validation.max
 not in enum             -> validation.enum
 absent optional         -> default, else null        (output object is in declaration order)
```

### Deadlines and timeouts

```text
 deadline_ms (default 30 000; CLI --timeout; HTTP deadline_ms ≤ 600 000)
   │
   ├─ Interpreter::drive: tokio::time::timeout(deadline_ms, body) ──▶ timeout.request
   ├─ exec_block: checked before every statement                  ──▶ timeout.request
   ├─ nested (request …): child deadline_ms = parent remaining     (never longer)
   ├─ EffectCtx.deadline / remaining(): adapters bound their own I/O (SYS-2026-0005)
   └─ inner bounds: scope timeout · dag timeout · concurrent timeout · poll timeout
```

Every timeout is kind `timeout` → exit 6, HTTP 504. Verified (scratch bundle; `slow.wait`
polls forever, `scoped.timeout` wraps it in `scope timeout "100ms"`):

```sh
$ rivet --file app.rivet request slow.wait --params {} --timeout 300ms
{"request_id":"req_01bc72a1a5","trace_id":"tr_01bc72a1a5","error":{"kind":"timeout","code":"timeout.request","message":"`slow.wait` exceeded its 300 ms deadline","retryable":false,"effects":"none","operation_id":"slow.wait"}}
exit=6
$ rivet --file app.rivet request scoped.timeout --params {}
{"request_id":"req_01a6d14255","trace_id":"tr_01a6d14255","error":{"kind":"timeout","code":"timeout.scope","message":"scope exceeded 100 ms","retryable":false,"effects":"none","source":{"file":"app.rivet","line":47,"column":5,"end_line":52,"end_column":8},"operation_id":"scoped.timeout"}}
exit=6
$ rivet --file app.rivet request slow.wait --params {} --timeout 2x
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.usage","message":"--timeout 2x: use digits plus ms, s, m or h","retryable":false,"effects":"none"}}
exit=2
```

Over HTTP (`rivet serve` on 127.0.0.1:18422):

```sh
$ curl -s -X POST http://127.0.0.1:18422/v1/request -H 'content-type: application/json' \
       -d '{"id":"slow.wait","params":{},"deadline_ms":200}'
{"request_id":"req_0129730775","trace_id":"tr_0129730775","error":{"kind":"timeout","code":"timeout.request","message":"`slow.wait` exceeded its 200 ms deadline","retryable":false,"effects":"none","operation_id":"slow.wait"}}   # HTTP 504
```

### Cancellation (Ctrl-C)

The CLI races the request against `tokio::signal::ctrl_c()`; on the signal it calls
`Runtime::cancel` (which fires the request's token) and then awaits the request, which unwinds
and ends with one `cancelled.request` error. Verified at commit `829ca43` by sending SIGINT one
second into `slow.wait` (a `poll every "100ms" timeout "60s"` that never completes):

```sh
$ rivet --file slow.rivet request slow.wait > int.out 2> int.err &   # then: kill -INT $!
exit=130
stdout: (empty)
stderr: {"request_id":"req_0100f8330d","trace_id":"tr_0100f8330d","error":{"kind":"cancelled","code":"cancelled.request","message":"`slow.wait` was cancelled","retryable":false,"effects":"none","source":{"file":"slow.rivet","line":5,"column":5,"end_line":8,"end_column":8},"operation_id":"slow.wait"}}
```

The error now carries the statement span where the run was interrupted, and `effects`
reports what actually happened (here `none`); `unknown` is reported only when the grace ran
out and the future had to be dropped.

`cancel_request` semantics: unknown IDs and IDs owned by another principal are both
`not_found.request` (ownership is not disclosed); an already-terminal request returns its
terminal state (`succeeded`, `failed` or `cancelled`) instead of pretending to cancel; a
second signal is a no-op.

### DAG runs — verified with `docs/demos/05-dag`

```sh
$ cd docs/demos/05-dag
$ rivet --file app.rivet request report.total --params '{"a":2,"b":3}'
{"request_id":"req_01a64911bd","trace_id":"tr_01a64911bd","result":{"total":10},"data_count":0,"effects":"none"}
exit=0
$ rivet --file app.rivet request report.partial --params '{}'
{"request_id":"req_01a5ec79c5","trace_id":"tr_01a5ec79c5","result":{"good":"succeeded","bad":"failed","blocked":"blocked"},"data_count":0,"effects":"none"}
exit=0
$ rivet --file app.rivet request report.total --params '{"a":2}'
{"request_id":"req_01a35727d5","trace_id":"tr_01a35727d5","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `b`","retryable":false,"effects":"none","operation_id":"report.total","details":{"field":"b"}}}
exit=2
```

```text
 report.partial  (dag limit 2 fail independent)

   good ──────────────▶ succeeded   result 8
   bad  ──────────────▶ failed      error fixture.failure (node_id "bad")
     └─▶ blocked after [bad] ─▶ blocked   (never evaluated; bad.result is null)

   return {good: good.status, bad: bad.status, blocked: blocked.status}
```

Node envelopes and the fatal node list, verified at commit `829ca43` (scratch `dag.rivet`:
`w.one` returns 1, `w.boom` fails `w.boom`):

```sh
$ rivet --file dag.rivet --json request w.env        # dag fail independent; return {a: a, b: b}
{"request_id":"req_016054125d","trace_id":"tr_016054125d","result":{"a":{"status":"succeeded","result":1,"error":null,"started_at":"2026-09-28T09:52:42.060Z","ended_at":"2026-09-28T09:52:42.066Z"},"b":{"status":"failed","result":null,"error":{"kind":"application","code":"w.boom","message":"always","retryable":false,"effects":"none","source":{"file":"dag.rivet","line":15,"column":5,"end_line":15,"end_column":21},"operation_id":"w.boom","node_id":"b","details":{}},"started_at":"2026-09-28T09:52:42.060Z","ended_at":"2026-09-28T09:52:42.066Z"}},"data_count":0,"effects":"none"}
$ rivet --file dag.rivet --json request w.fast       # dag fail fast
{"request_id":"req_015ff5ca95.2","trace_id":"tr_015ff5ca95","error":{"kind":"application","code":"w.boom","message":"always","retryable":false,"effects":"none","source":{"file":"dag.rivet","line":15,"column":5,"end_line":15,"end_column":21},"operation_id":"w.boom","node_id":"b","details":{"nodes":[{"id":"a","status":"succeeded","started_at":"2026-09-28T09:52:42.085Z","ended_at":"2026-09-28T09:52:42.085Z"},{"id":"b","status":"failed","started_at":"2026-09-28T09:52:42.085Z","ended_at":"2026-09-28T09:52:42.085Z"}]}}}
exit=5
```

`fail fast` (the default) makes the first failure fatal. Scratch bundle `dag.fast`
(`dag limit 1`, nodes `bad`, `later after [bad]`, `other`), captured at `f40d4aa`:

```sh
$ rivet --file app.rivet request dag.fast --params {}
{"request_id":"req_01be5c5ccd.1","trace_id":"tr_01be5c5ccd","error":{"kind":"application","code":"fixture.failure","message":"Always fails.","retryable":false,"effects":"none","source":{"file":"app.rivet","line":6,"column":5,"end_line":6,"end_column":44},"operation_id":"fixture.fail","node_id":"bad","details":{"reason":"demo"}}}
exit=5
```

Here `later` ends `blocked` and `other` ends `skipped` (unit test
`fail_fast_skips_the_rest` in `run_dag.rs`). The raised error is the failing node's own
error (its request ID is the nested child's), carrying `node_id`; since commit `829ca43` the
node list is merged into its existing `details` (`merge into existing details`,
G14); the capture above predates that change.

### Streams and nested streams

`emit` requires a declared `emits` type (`stream.emits_undeclared` otherwise), **validates
the item against it** (a mismatch is `output.invalid` "emitted item N at PATH must be …" with
`details {seq, path, expected, found}`), refuses a secret-tainted value (`permission.denied`
"cannot be emitted") and awaits the sink, so a slow consumer applies backpressure. A sink that
returns `RivetError::consumer_stop()` stops the request cleanly (`cancelled` /
`consumer.stop`); any other sink error is `consumer_failed`. Items received through
`incoming` are validated against `receives` by the session driver (`validation.input`,
SYS-2026-0007). `with (request.stream "ID" {…}) as events` runs the child in a task owned
by the scope and delivers items through a 16-slot channel:

```sh
$ rivet --file app.rivet request events.count --params {} --stream
{"request_id":"req_019e985a15","trace_id":"tr_019e985a15","seq":1,"type":"data","data":1}
{"request_id":"req_019e985a15","trace_id":"tr_019e985a15","seq":2,"type":"data","data":2}
{"request_id":"req_019e985a15","trace_id":"tr_019e985a15","seq":3,"type":"data","data":3}
{"request_id":"req_019e985a15","trace_id":"tr_019e985a15","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
exit=0
$ rivet --file app.rivet request events.consume --params {}     # sums events.count via request.stream
{"request_id":"req_019cfd397d","trace_id":"tr_019cfd397d","result":{"total":6},"data_count":0,"effects":"none"}
exit=0
$ rivet --file app.rivet --json request demo.typed --stream          # emits integer; emit "not a number" (829ca43)
{"request_id":"req_01b8df1f9d","trace_id":"tr_01b8df1f9d","error":{"kind":"output_invalid","code":"output.invalid","message":"emitted item 1 at $ must be integer, got text","retryable":false,"effects":"none","operation_id":"demo.typed","details":{"seq":1,"path":"$","expected":"integer","found":"text"}}}
exit=5
```

An operation that declares `receives` cannot run unary (`stream.input_required`, exit 2);
live input is described in SYS-2026-0007.

### Output validation

`execution.validate_output` runs once on the final value, before the `Completion` exists.
Output type `json` (the default when an operation has no `output` line) is opaque. Otherwise
scalars must match, required fields must be present and non-null, closed objects reject
extra keys, lists and nested objects recurse; at most 32 violations are listed, plus
`details.truncated`. Committed effects are preserved on the error.

```sh
$ rivet --file app.rivet request bad.output --params {}      # output integer; return "five"
{"request_id":"req_01a861d59d","trace_id":"tr_01a861d59d","error":{"kind":"output_invalid","code":"output.invalid","message":"`bad.output` returned a result that does not match its declared output: at $ expected integer, found text","retryable":false,"effects":"none","operation_id":"bad.output","details":{"violations":[{"path":"$","expected":"integer","found":"text"}]}}}
exit=5
```

### File operations

```text
 source:  file VERB "PATH" [as CODEC | CODEC VALUE] [to "PATH"]   options: if_version, missing ok, overwrite
            │
            ▼  Machine::run_file builds FileOperation {verb, path, to?, codec?, content?, if_version?, missing_ok, overwrite}
 PolicedFiles.apply (runtime.rs)
            │
            ▼  files.apply_file_operation
     file_intents(op) ── one (capability, access, path) per touched path
            │            evaluator.evaluate each; first denial -> permission.denied (exit 3), zero effects
            ▼
     ConfinedFiles.apply ── spawn_blocking(apply_sync): cap-std Dir at the bundle root,
                            relative paths only, `..` may not escape, symlinks refused
```

| Verb | Intents authorized | Behaviour | Result value |
|---|---|---|---|
| `read` | read/read | decode by codec; > 8 MiB → `limit.file_size` | decoded value |
| `list` | read/list | entries sorted by name | `[{name, type, size}]` |
| `stat` | read/stat | no-follow metadata | `{path, type, size, version}` |
| `create` | write/create | exclusive (`create_new`) → `conflict.already_exists` | `{path, created, version}` |
| `update` | read/stat + write/update | must exist (`not_found.file`); locked compare-and-replace (below); `if_version` mismatch → `conflict.version` | `{path, updated, version}` |
| `write` | write/create + write/update | create or replace atomically | `{path, created\|updated, version}` |
| `append` | write/append | default codec text | `{path, appended}` |
| `delete` | delete/delete | `missing ok` → `deleted:false` and no effect recorded (`effects` stays `none`) | `{path, deleted}` |
| `copy` / `move` | read/read src + write/create dst (+ write/update if overwrite) (+ delete/delete src for move) | `overwrite false` → `conflict.already_exists` | `{path, version}` |

`version` is `v:` plus the first 8 bytes of the SHA-256 of the content, hex. Hard-linked
targets (link count > 1) are refused for write, delete, copy destination and move source
(`file.hardlink_refused`). A mutating file verb marks the request's effects `committed`.
Every file effect runs inside its effect scope, so the broker intent — and therefore the
trace entry and any `permission.denied` — names the operation ID and the statement's source
span.

**Replacing an existing file** (`update`, `write`, `copy`/`move` with overwrite) is a
compare-and-replace under an exclusive advisory lock (Unix `flock`):

```text
 open target no-follow ─▶ flock(LOCK_EX) (wait ≤ 10 s, else timeout.file_lock)
   ─▶ fd still the file at PATH? (dev+ino) ── no ─▶ retry on the new file
   ─▶ nlink > 1 → file.hardlink_refused
   ─▶ read via the locked fd; if_version given and ≠ current → conflict.version (file unchanged)
   ─▶ write temp sibling ─▶ fsync ─▶ rename over PATH (lock still held) ─▶ release
```

This is atomic against every writer that takes the same lock (all Rivet runtimes); a process
that writes without the lock is outside what an advisory lock can stop. On platforms without
`flock` (non-Unix) a conditional update is refused with `unsupported.conditional_update`
before anything is written. A variable named like a codec never replaces the codec keyword
(`file append P text text` appends the variable `text`).

Verified with `docs/demos/02-file-crud` (copied to a scratch folder so the demo tree stays
clean):

```sh
$ rivet --file app.rivet request notes.create --params '{"text":"first draft"}'
{"request_id":"req_01c2a76edd","trace_id":"tr_01c2a76edd","result":{"created":true},"data_count":0,"effects":"committed"}
exit=0
$ rivet --file app.rivet request notes.create --params '{"text":"again"}'
{"request_id":"req_01bf4a5ffd","trace_id":"tr_01bf4a5ffd","error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/note.json already exists","retryable":false,"effects":"none","source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create"}}
exit=4
$ rivet --file app.rivet request notes.read --params '{}'
{"request_id":"req_01be7b80dd","trace_id":"tr_01be7b80dd","result":{"text":"first draft"},"data_count":0,"effects":"none"}
exit=0
$ rivet --file app.rivet request notes.update --params '{"text":"reviewed draft"}'
{"request_id":"req_01bd27379d","trace_id":"tr_01bd27379d","result":{"updated":true},"data_count":0,"effects":"committed"}
exit=0
$ rivet --file app.rivet request notes.list --params '{}'
{"request_id":"req_01bca3ce4d","trace_id":"tr_01bca3ce4d","result":[{"name":"note.json","type":"file","size":31}],"data_count":0,"effects":"none"}
exit=0
$ rivet --file app.rivet request notes.delete --params '{}'
{"request_id":"req_01ba5f62a5","trace_id":"tr_01ba5f62a5","result":{"absent":true},"data_count":0,"effects":"committed"}
exit=0
$ rivet --file app.rivet request notes.delete --params '{}'      # missing ok (f40d4aa capture)
{"request_id":"req_01b9184a6d","trace_id":"tr_01b9184a6d","result":{"absent":true},"data_count":0,"effects":"committed"}
exit=0
# since 2a751ab a delete that finds nothing reports "effects":"none":
{"request_id":"req_0122609e5d","trace_id":"tr_0122609e5d","result":{"absent":true},"data_count":0,"effects":"none"}
$ rivet --file app.rivet request notes.update --params '{"text":"x"}'
{"request_id":"req_01b831df0d","trace_id":"tr_01b831df0d","error":{"kind":"not_found","code":"not_found.file","message":"./out/note.json: no such file","retryable":false,"effects":"none","source":{"file":"app.rivet","line":30,"column":5,"end_line":30,"end_column":52},"operation_id":"notes.update"}}
exit=4
```

Without `policy.json` beside the bundle (deny-by-default):

```sh
$ rivet --file app.rivet request notes.create --params '{"text":"x"}'
{"request_id":"req_01b83d6425","trace_id":"tr_01b83d6425","error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no policy.json: allow_write is denied by default (add a grant for ./out/note.json)","retryable":false,"effects":"none","source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}}}
exit=3
```

### Error model and exit codes

`ErrorKind` (`src/domain/errors.rs`) is the single registry; codes are dotted refinements
(`timeout.request`, `conflict.version` …).

| Kind(s) | CLI exit | HTTP | Retryable |
|---|---|---|---|
| `syntax`, `validation` | 2 | 422 | no |
| `auth` | 3 | 401 | no |
| `permission` | 3 | 403 | no |
| `not_found` | 4 | 404 | no |
| `conflict` | 4 | 409 | no |
| `limit` | 5 | 429 | **yes** |
| `timeout` | 6 | 504 | no |
| `cancelled` | 130 | 409 | no |
| `unsupported` | 5 | 501 | no |
| `connection`, `dns`, `tls`, `http`, `protocol`, `application`, `process`, `parse` | 5 | 502 | no |
| `output_invalid`, `internal`, `cleanup`, `consumer_failed` | 5 | 500 | no |

Exit 7 (inspection incomplete) is produced by `io --strict` / `policy generate`, not by the
error registry. `EffectsStatus` is `none | committed | partial | unknown`. A primary error
wins over a cleanup error, which is attached under `suppressed`.

```text
 RivetError
 ├─ kind · code · message · retryable (= kind is limit)
 ├─ source {file,line,column,end_line,end_column}?   operation_id?  node_id?  hint?
 ├─ request_id? · trace_id?      (set by the dispatcher's tag())
 ├─ details (Value)              (violations, field, nodes, capability/access/target …)
 ├─ effects                      none | committed | partial | unknown
 ├─ cause?                       (e.g. consumer_failed wraps the sink error)
 └─ suppressed[]                 (cleanup failures behind a primary error)
```

## Data and Storage

- No execution state is persisted. `RunningRequests` holds running request IDs, owners and
  `Notify` signals in memory, plus the last 1024 terminal states for late cancel calls.
- The trace store is in memory (`MemoryTraceStore`, see Observability); it disappears with
  the process.
- File operations touch only paths under the bundle root; `update`, `write`, `copy` and
  `move` write a sibling temporary file `.<name>.rivet-tmp` and rename it over the target
  (under the file lock when the target exists).

## Dependencies

| Crate / module | Used for |
|---|---|
| `tokio` (`time::timeout`, `select!`, `sync::Semaphore`, `sync::Notify`, `mpsc`, `spawn_blocking`) | deadlines, cancellation, budget, channels, blocking file I/O |
| `futures-util` (`FuturesUnordered`) | DAG, `map` and `concurrent` scheduling in the request task |
| `cap-std` | root-confined, no-follow file handles |
| `sha2` | file `version` tokens |
| `async-trait` | ports and adapters |
| Policy broker (SYS-2026-0003) | `PolicyEvaluator` behind every file and adapter effect |

Crate selection rationale: `../../decisions/adr-0002-rust-crate-selection.md` (see Related
Documents).

## Deployment

The runtime is linked into the single `rivet` binary and into the `rivet` library crate. It
runs wherever the CLI, `rivet serve` or a library host runs; there is no separate service.
A `serve` process shares one `Runtime` (one semaphore, one `RunningRequests`, one trace
store) across all surfaces and principals.

## Security Boundaries

```text
 principal ──▶ require_operation (serve.principals; local always allowed) ── top level only
 bundle    ──▶ private operations reachable only by nested calls (include_private)
 effects   ──▶ EffectIntent ─▶ PolicyEvaluator ─▶ Permit (deny wins; no policy = deny)
 files     ──▶ every touched path authorized first; cap-std root; no absolute paths,
               no `..` escape, symlinks refused, hard links refused for mutation
 cancel    ──▶ only the owning principal; foreign IDs look like unknown IDs
 budgets   ──▶ max_concurrent_requests / max_call_depth / max_buffered_bytes / deadlines
 secrets   ──▶ taint: a `secret … for ORIGIN` value (and forms derived by explicit flows)
               reaches only a network sink whose scheme://host:port is a bound origin
 restrict  ──▶ per-request restriction stack: every decision must pass policy ∩ restrict…
```

**Secret taint** (`execution_driver.rs`, G23b). A `secret NAME from env "VAR" for ORIGIN…`
records the value as a taint *form*. Explicit flows keep it recognisable: assignment,
interpolation, list/object construction (the form appears as a substring), and the encoding
helpers (`base64.encode`, `text`, …) whose **output** is recorded as a new derived form
(≥ 4 bytes, at most 64 forms per secret). Every sink is checked: the head, options and child
parts of every effect form (URL, headers, body, query), `with` opens, handle member calls
(socket/stream/UDP sends), nested `(request …)` params (so MCP and gRPC calls), `return` and
`emit`. A sink passes only when it is a network destination whose `scheme://host:port` is one
of the bound origins; files, processes, Unix sockets, pipes, nested requests, `return` and
`emit` never pass. Refusals are `permission.denied` with `details {secret, origin?}`. Implicit
flows (branching on a secret, lengths, timing) are out of scope.

- Nested calls inherit the caller's principal and cannot widen authority.
- File errors name the source path, never file content; the CLI stdin feeder never echoes
  input lines (SYS-2026-0007).

## Observability

- **Completion / error envelopes** carry `request_id`, `trace_id`, `effects` and, for
  errors, `source`, `operation_id` and `node_id`.
- **Trace store** (`src/infra/trace_store.rs`, `MemoryTraceStore`): every authorized or
  denied effect decision is recorded with request ID, effect ID, capability, access, target,
  decision and matching rule. Read with `rivet trace show REQUEST_ID` against a running
  `serve` (the CLI's own process exits with its trace). Verified with the 02-file-crud bundle:

```sh
$ rivet --endpoint http://127.0.0.1:18421 trace show req_01693e07ad
{"request_id":"req_01693e07ad","attempts":[{"request_id":"req_01693e07ad","trace_id":"tr_01693e07ad","node_id":null,"attempt":1,"effect_id":"notes.create#1","operation_id":"notes.create","phase":"decision","capability":"allow_write","access":"create","target":"./out/note.json","decision":"allowed","policy_hash":"sha256:deccf2027323af83c9798a05d6cac1adb657a81852e54ac7b99ce3eca4b0bef3","source":null,"outcome":{"rule":"grant allow_write ./out/**"}}],"complete":true,"next_cursor":null,"gaps":0}
```

- `rivet policy explain ID` prints the effective `limits` line (see Configuration).
- `Runtime::export_trace` (and the `rivet.trace.export` built-in behind `rivet --endpoint … trace export`,
  dispatched since commit `2a751ab`) writes one request's trace to a new file through the broker.

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations), the
ones that concern execution:

- No `finally` blocks; Stage C forms (`with file watch`, `with pipe`, `reconnect`,
  `interactive true`) are refused at run time.
- CLI `--timeout` is not forwarded over the WebSocket duplex path (`--endpoint` with
  `--input-jsonl -`).
- No persistent trace store (and no persistence or resume after restart).

Behaviour by design: the concurrency budget does not queue (the 65th simultaneous top-level
request fails immediately with `limit.concurrency`, retryable); effects are never retried
automatically; secret taint covers explicit flows only.

## Last Verified Version

`0.1.0-dev (commit 829ca43)`, 2026-09-28, macOS, `target/debug/rivet`. First verified at
`f40d4aa`: commands were run from `docs/demos/05-dag`, a scratch copy of
`docs/demos/02-file-crud`, and scratch bundles for timeouts, cancellation, output validation
and call depth. Re-verified at `829ca43` for structured cancellation (SIGINT and deadline),
DAG timestamps and `details.nodes`, `emits` item validation, `with file open`, `if_version`
under the lock and secret taint, using scratch bundles. `rivet serve` was used on
127.0.0.1:18421 and 127.0.0.1:18422 and stopped afterwards. Request and trace IDs differ
on every run.

## Related Documents

- [PROP-2026-0001 Rivet runtime proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001 v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [ADR-0002 Rust crate selection](../../decisions/adr-0002-rust-crate-selection.md)
- [REF-2026-0002 Language and usage](../../references/ref-2026-0002-language-and-usage.md)
- [SYS-2026-0001 Compiler and catalog](../components/sys-2026-0001-compiler-and-catalog.md)
- [SYS-2026-0003 Policy broker and I/O manifest](../components/sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004 Surfaces and serve](../components/sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0005 Protocol adapters](../integrations/sys-2026-0005-protocol-adapters.md)
- [SYS-2026-0007 Duplex sessions](sys-2026-0007-sessions.md)
- [SYS-2026-0008 policy.json reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [Demo folders](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-16). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43, 2a751ab): structured cancellation (CancelToken, graceful close within the grace), DAG `started_at`/`ended_at` and merged `details.nodes`, `emits` validation, typed sink stop, enforced `max_buffered_bytes`, `--timeout` cap, `effects` folding (`unknown` for opaque MCP), file effect scopes, `with file open`, flock-guarded compare-and-replace, secret taint on every sink, per-request restriction; limitations reduced to the current ones. |
