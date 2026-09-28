---
document_id: SYS-2026-0002
title: "Rivet execution, scopes and DAG runtime"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [execution, files]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.2.0-rc (main at 8031baa)"
next_review_date: 2026-10-29
review_cycle: on-release
confidentiality: internal
scope: How one request is dispatched, bounded, interpreted, scoped, cancelled and validated, including file operations and DAG scheduling, as implemented in the Rivet 0.1.0 and 0.2.0 runtime (0.2.0: the global scope of each file, envelope outcomes at the surfaces, module operations and catalog snapshots).
reason: Every surface (CLI, HTTP, SSE, polling, WebSocket, MCP, library) funnels into one dispatcher and one interpreter; maintainers need the current lifecycle, limits, scope cleanup, DAG states, cancellation paths and exit codes in one verified place (PLAN-2026-0001 D-16).
related_documents: [PROP-2026-0001, PLAN-2026-0001, PROP-2026-0002, PLAN-2026-0002, API-2026-0006, SYS-2026-0001, SYS-2026-0003, SYS-2026-0004, SYS-2026-0005, SYS-2026-0007, SYS-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, system, execution, dag, scopes, cancellation, files, errors]
---

# Rivet execution, scopes and DAG runtime

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** execution, files
> **Last Verified Version:** 0.2.0-rc (main at 8031baa)

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
(`src/domain/errors.rs`). From 0.2.0 the surfaces render that outcome as one
[ResponseEnvelope](../../api/api-2026-0006-envelopes.md) (`status` `ok`, `error` or `cancelled`), and a frame
reads the operation file's frozen **global scope** after its locals and params.

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
  compensations. There is no `finally` block (0.1.0 and 0.2.0; a known limitation).
- **Rendering the outcome** as an envelope (`ResponseEnvelope::from_outcome`, `serve.parse_input` for the
  input) happens at the surface edge (SYS-2026-0004). Execution still returns `Completion | RivetError`.

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
 │   ├─ envelope.rs                ResponseEnvelope, InputEnvelope, Envelope records (0.2.0; surfaces only)
 │   └─ policy.rs                  PolicyLimits (64 / 16 / 256 MiB)
 └─ infra/
     ├─ execution_driver.rs        Interpreter (ExecutionDriver, holds globals: file → Arc<Value>), Machine, Frame,
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
    |                   |                            |                         | frame.globals = scope of op.file
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
 └─ Frame  globals  = Arc<Value> of the operation's file (read-only, frozen at load; 0.2.0)
    │      scopes[0] = params + operation-level assignments
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

### Global scope lookup order (0.2.0)

`Interpreter::new` turns each `GlobalScope` of the program (SYS-2026-0001) into an `Arc<Value>` keyed by
file. `drive` gives the frame the scope of the **operation's own file** (`frame.globals =
globals.get(&op.file)`), so each module sees only its own globals. `Frame::get` resolves a name in this order:

```text
 name ─▶ block scopes, innermost first (loop vars, catch `error`, block locals)
      ─▶ scopes[0] (params + operation-level assignments)
      ─▶ frame.globals (the file's frozen constants)          ─▶ none: unknown name
```

The order matters only for speed: `check.global_shadow` and `check.global_assign` refuse, at compile time,
every binding or assignment that reuses a global's name. At run time, therefore, a name is never both a local
and a global. `Frame::assign` never writes to `globals`. DAG `NodeRunner`s, `concurrent` tasks and `map`
items clone the frame, and with it the same `Arc`. A nested `(request …)` builds a new frame for the callee,
with the callee's file scope. Globals are shared read-only across concurrent requests and are not copied per
request.

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
 Runtime::builder().file(p) | .source(path, text, root) | .root(dir)   (root: 0.2.0, empty catalog)
                   .policy_file(p) | .policy(Policy)   [.ceiling(Policy)]
                   .build()                         -> Runtime
 rt.call(InputEnvelope) / rt.call_json(&str)        -> ResponseEnvelope   (0.2.0 facade; setup_library.rs)
 rt.load(path) / rt.load_as(path, alias)            -> Module             (0.2.0; new catalog snapshot)
 module.call(id, data)                              -> ResponseEnvelope   (dispatches "alias.id")
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
 rivet [--file F] [--policy P] [--pretty] request ID [--data JSON] [--stream] [--timeout D]
 rivet [--file F] [--policy P] request --input FILE|-          (a whole InputEnvelope; 0.2.0)
        │                                     │                  │           └─ digits + ms|s|m|h → deadline_ms
        │                                     │                  └─ NDJSON records (type data …, then type result)
        │                                     └─ object (default {}); unknown fields rejected
        │                                        --params JSON: deprecated alias (warning[deprecated.params], removed in 0.3.0)
        └─ stdout: ResponseEnvelope status ok (exit 0) · stderr: status error|cancelled (registry exit code)
 --timeout above 600000 ms (10m) → validation.usage (exit 2), the same host cap as HTTP deadline_ms
 Ctrl-C while running → execution.cancel_request → cancelled.request → exit 130
```

### HTTP

`POST /v1/request {operation, data, deadline_ms?, restrict?}` (SYS-2026-0004; the 0.1.0 keys `id` and
`params` are accepted as deprecated aliases through 0.2.x and answered with `deprecation: true`). `deadline_ms` is
clamped to `1..=600000` (`MAX_REQUEST_DEADLINE_MS` in `src/io/http/mod.rs`); the CLI's
`--timeout` in `--endpoint` mode is sent as this field (and refused above the cap locally too).

### Outcome and envelope

Execution produces a `Completion {result, data_count, effects}` or a `RivetError` (with its own `effects`). In
0.2.0 every surface renders both as one ResponseEnvelope (full reference:
[API-2026-0006](../../api/api-2026-0006-envelopes.md); 0.1.0 shapes: [MIG-2026-0001](../../migrations/mig-2026-0001-response-and-input-envelopes.md)):

```text
 Completion{result, data_count, effects}            RivetError{kind, code, message, …, effects}
            │                                                 │
            └────────────── ResponseEnvelope ─────────────────┘
 {"request_id":"req_…","trace_id":"tr_…","operation":"<id>","type":"result",
  "status":"ok" | "error" | "cancelled",
  "data":  <validated result> | null,
  "error": null | {kind, code, message, retryable, operation_id?, node_id?, details?, source?, hint?, cause?, suppressed?},
  "effects":"none|committed|partial|unknown",        ← top level (was error.effects in 0.1.0)
  "data_count": <items emitted>}
```

A nested failure keeps the child's error object (its `operation_id`, `node_id` and `source`). The envelope's
`request_id` is the **top-level** request's; in 0.1.0 the error envelope carried the child's `req_….N`.

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
$ rivet --file app.rivet request chain.a
{"request_id":"req_013bed2b5d","trace_id":"tr_013bed2b5d","operation":"chain.a","type":"result","status":"error","data":null,"error":{"kind":"limit","code":"limit.call_depth","message":"call depth 2 exceeds limits.max_call_depth 1","retryable":true,"source":{"file":"app.rivet","line":8,"column":13,"end_line":8,"end_column":20},"operation_id":"chain.b"},"effects":"none","data_count":0}
exit=5
$ rivet --file app.rivet policy explain chain.a
policy   ./policy.json (sha256:b2c04073986cca43ac35cebf62717a7db34703d914fd9d219ce3716389abe76c)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 1, 268435456 buffered bytes

OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE       DECISION
chain.a    (calls chain.b — no I/O)         app.rivet:3
chain.b    (calls chain.c — no I/O)         app.rivet:8
exit=0
```

(Request and trace IDs vary per run. The error object is the nested `chain.b` call's, as its `operation_id`
and `source` show; the envelope `request_id` is the top-level request's.)

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
$ rivet --file app.rivet request slow.wait --timeout 300ms
{"request_id":"req_01383b0ba5","trace_id":"tr_01383b0ba5","operation":"slow.wait","type":"result","status":"error","data":null,"error":{"kind":"timeout","code":"timeout.request","message":"`slow.wait` exceeded its 300 ms deadline","retryable":false,"source":{"file":"app.rivet","line":4,"column":5,"end_line":8,"end_column":8},"operation_id":"slow.wait"},"effects":"none","data_count":0}
exit=6
$ rivet --file app.rivet request scoped.timeout
{"request_id":"req_011568756d","trace_id":"tr_011568756d","operation":"scoped.timeout","type":"result","status":"error","data":null,"error":{"kind":"timeout","code":"timeout.scope","message":"scope exceeded 100 ms","retryable":false,"source":{"file":"app.rivet","line":14,"column":5,"end_line":16,"end_column":8},"operation_id":"scoped.timeout"},"effects":"none","data_count":0}
exit=6
$ rivet --file app.rivet request slow.wait --timeout 2x
{"request_id":"","trace_id":"","operation":"slow.wait","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.usage","message":"--timeout 2x: use digits plus ms, s, m or h","retryable":false},"effects":"none","data_count":0}
exit=2
```

Over HTTP (`rivet serve --listen 127.0.0.1:18900`, stopped after the capture):

```sh
$ curl -s -i -X POST http://127.0.0.1:18900/v1/request -H 'content-type: application/json' \
       -d '{"operation":"slow.wait","data":{},"deadline_ms":200}'
HTTP/1.1 504 Gateway Timeout
content-type: application/json
traceparent: 00-68713364eb604a2dc6e2ebed68b69fa0-d6913ec122d9af75-01

{"request_id":"req_01614fa83d","trace_id":"tr_01614fa83d","operation":"slow.wait","type":"result","status":"error","data":null,"error":{"kind":"timeout","code":"timeout.request","message":"`slow.wait` exceeded its 200 ms deadline","retryable":false,"source":{"file":"app.rivet","line":4,"column":5,"end_line":8,"end_column":8},"operation_id":"slow.wait"},"effects":"none","data_count":0}
```

The access log line shows the 250 ms backstop on top of the 200 ms deadline:
`{"time":"2026-09-28T21:26:25.019Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"slow.wait","status":504,"duration_ms":452}`.
The same body sent with the deprecated keys `{"id":"slow.wait","params":{},…}` gets the same 504 envelope plus
the response header `deprecation: true`, and its log line ends in `"deprecated":1`.

### Cancellation (Ctrl-C)

The CLI races the request against `tokio::signal::ctrl_c()`; on the signal it calls
`Runtime::cancel` (which fires the request's token) and then awaits the request, which unwinds
and ends with one `cancelled.request` error. Verified at commit `829ca43` by sending SIGINT one
second into `slow.wait` (a `poll every "100ms" timeout "60s"` that never completes), and re-verified
on the 0.2.0-rc (`status: "cancelled"`):

```sh
$ rivet --file app.rivet request slow.wait > int.out 2> int.err &   # then: kill -INT $!
exit=130
stdout: (empty)
stderr: {"request_id":"req_01275c018d","trace_id":"tr_01275c018d","operation":"slow.wait","type":"result","status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.request","message":"`slow.wait` was cancelled","retryable":false,"source":{"file":"app.rivet","line":4,"column":5,"end_line":8,"end_column":8},"operation_id":"slow.wait"},"effects":"none","data_count":0}
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
$ rivet --file app.rivet request report.total --data '{"a":2,"b":3}'
{"request_id":"req_01e9f53b15","trace_id":"tr_01e9f53b15","operation":"report.total","type":"result","status":"ok","data":{"total":10},"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request report.partial
{"request_id":"req_01e8e4f515","trace_id":"tr_01e8e4f515","operation":"report.partial","type":"result","status":"ok","data":{"good":"succeeded","bad":"failed","blocked":"blocked"},"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request report.total --data '{"a":2}'
{"request_id":"req_01e7e80235","trace_id":"tr_01e7e80235","operation":"report.total","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `b`","retryable":false,"operation_id":"report.total","details":{"field":"b"}},"effects":"none","data_count":0}
exit=2
```

```text
 report.partial  (dag limit 2 fail independent)

   good ──────────────▶ succeeded   result 8
   bad  ──────────────▶ failed      error fixture.failure (node_id "bad")
     └─▶ blocked after [bad] ─▶ blocked   (never evaluated; bad.result is null)

   return {good: good.status, bad: bad.status, blocked: blocked.status}
```

Node envelopes and the fatal node list (scratch `app.rivet`: `w.one` returns 1, `w.boom` fails `w.boom`):

```sh
$ rivet --file app.rivet request w.env        # dag fail independent; return {a: a, b: b}
{"request_id":"req_010d8e524d","trace_id":"tr_010d8e524d","operation":"w.env","type":"result","status":"ok","data":{"a":{"status":"succeeded","result":1,"error":null,"started_at":"2026-09-28T21:25:55.924Z","ended_at":"2026-09-28T21:25:55.924Z"},"b":{"status":"failed","result":null,"error":{"kind":"application","code":"w.boom","message":"always","retryable":false,"effects":"none","source":{"file":"app.rivet","line":28,"column":5,"end_line":28,"end_column":18},"operation_id":"w.boom","node_id":"b"},"started_at":"2026-09-28T21:25:55.924Z","ended_at":"2026-09-28T21:25:55.924Z"}},"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request w.fast       # dag fail fast
{"request_id":"req_010cf46f15","trace_id":"tr_010cf46f15","operation":"w.fast","type":"result","status":"error","data":null,"error":{"kind":"application","code":"w.boom","message":"always","retryable":false,"source":{"file":"app.rivet","line":28,"column":5,"end_line":28,"end_column":18},"operation_id":"w.boom","node_id":"b","details":{"nodes":[{"id":"a","status":"succeeded","started_at":"2026-09-28T21:25:55.940Z","ended_at":"2026-09-28T21:25:55.940Z"},{"id":"b","status":"failed","started_at":"2026-09-28T21:25:55.940Z","ended_at":"2026-09-28T21:25:55.941Z"}]}},"effects":"none","data_count":0}
exit=5
```

A node value inside `data` is a **language value** (`{status, result, error, started_at, ended_at}`), not a
wire envelope. Its `error` object is the script-visible error and still carries `effects`, as in 0.1.0. Only
the top-level envelope moved `effects` out of the error.

`fail fast` (the default) makes the first failure fatal. Scratch bundle `df.rivet` (`dag limit 1`, nodes
`bad`, `later after [bad]`, `other`):

```sh
$ rivet --file df.rivet request dag.fast
{"request_id":"req_0160be859d","trace_id":"tr_0160be859d","operation":"dag.fast","type":"result","status":"error","data":null,"error":{"kind":"application","code":"fixture.failure","message":"Always fails.","retryable":false,"source":{"file":"df.rivet","line":4,"column":5,"end_line":4,"end_column":44},"operation_id":"fixture.fail","node_id":"bad","details":{"reason":"demo","nodes":[{"id":"bad","status":"failed","started_at":"2026-09-28T21:26:11.715Z","ended_at":"2026-09-28T21:26:11.715Z"},{"id":"later","status":"blocked"},{"id":"other","status":"skipped"}]}},"effects":"none","data_count":0}
exit=5
```

`later` ends `blocked` and `other` ends `skipped` (unit test `fail_fast_skips_the_rest` in `run_dag.rs`).
The raised error is the failing node's own error and carries `node_id`. The node list is merged into its
existing `details` (`reason` is kept; G14, since `829ca43`).

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
$ rivet --file app.rivet request events.count --stream
{"request_id":"req_010bf80d8d","trace_id":"tr_010bf80d8d","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
{"request_id":"req_010bf80d8d","trace_id":"tr_010bf80d8d","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_010bf80d8d","trace_id":"tr_010bf80d8d","operation":"events.count","type":"data","seq":3,"data":3,"error":null}
{"request_id":"req_010bf80d8d","trace_id":"tr_010bf80d8d","operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
exit=0
$ rivet --file app.rivet request events.consume     # sums events.count via request.stream
{"request_id":"req_010aedf38d","trace_id":"tr_010aedf38d","operation":"events.consume","type":"result","status":"ok","data":{"total":6},"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request demo.typed --stream          # emits integer; emit "not a number"
{"request_id":"req_0109f96c05","trace_id":"tr_0109f96c05","operation":"demo.typed","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"output_invalid","code":"output.invalid","message":"emitted item 1 at $ must be integer, got text","retryable":false,"operation_id":"demo.typed","details":{"seq":1,"path":"$","expected":"integer","found":"text"}},"effects":"none","data_count":0}
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
$ rivet --file app.rivet request bad.output      # output integer; return "five"
{"request_id":"req_0108e76bf5","trace_id":"tr_0108e76bf5","operation":"bad.output","type":"result","status":"error","data":null,"error":{"kind":"output_invalid","code":"output.invalid","message":"`bad.output` returned a result that does not match its declared output: at $ expected integer, found text","retryable":false,"operation_id":"bad.output","details":{"violations":[{"path":"$","expected":"integer","found":"text"}]}},"effects":"none","data_count":0}
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
$ mkdir out
$ rivet --file app.rivet request notes.create --data '{"text":"first draft"}'
{"request_id":"req_0159a36f7d","trace_id":"tr_0159a36f7d","operation":"notes.create","type":"result","status":"ok","data":{"created":true},"error":null,"effects":"committed","data_count":0}
exit=0
$ rivet --file app.rivet request notes.create --data '{"text":"again"}'
{"request_id":"req_015768642d","trace_id":"tr_015768642d","operation":"notes.create","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/note.json already exists","retryable":false,"source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create"},"effects":"none","data_count":0}
exit=4
$ rivet --file app.rivet request notes.read
{"request_id":"req_01564d87dd","trace_id":"tr_01564d87dd","operation":"notes.read","type":"result","status":"ok","data":{"text":"first draft"},"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request notes.update --data '{"text":"reviewed draft"}'
{"request_id":"req_0156b8303d","trace_id":"tr_0156b8303d","operation":"notes.update","type":"result","status":"ok","data":{"updated":true},"error":null,"effects":"committed","data_count":0}
exit=0
$ rivet --file app.rivet request notes.list
{"request_id":"req_01546b46e5","trace_id":"tr_01546b46e5","operation":"notes.list","type":"result","status":"ok","data":[{"name":"note.json","type":"file","size":31}],"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request notes.delete
{"request_id":"req_015351c7cd","trace_id":"tr_015351c7cd","operation":"notes.delete","type":"result","status":"ok","data":{"absent":true},"error":null,"effects":"committed","data_count":0}
exit=0
$ rivet --file app.rivet request notes.delete      # missing ok: nothing deleted, effects none (since 2a751ab)
{"request_id":"req_01525389f5","trace_id":"tr_01525389f5","operation":"notes.delete","type":"result","status":"ok","data":{"absent":true},"error":null,"effects":"none","data_count":0}
exit=0
$ rivet --file app.rivet request notes.update --data '{"text":"x"}'
{"request_id":"req_01515a5d7d","trace_id":"tr_01515a5d7d","operation":"notes.update","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.file","message":"./out/note.json: no such file","retryable":false,"source":{"file":"app.rivet","line":30,"column":5,"end_line":30,"end_column":52},"operation_id":"notes.update"},"effects":"none","data_count":0}
exit=4
```

Without the `out/` directory, every verb except `delete … missing ok` fails `not_found.file` (the demo README
creates it first).

Without `policy.json` beside the bundle (deny-by-default):

```sh
$ rivet --file app.rivet request notes.create --data '{"text":"x"}'
{"request_id":"req_0161980a65","trace_id":"tr_0161980a65","operation":"notes.create","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no policy.json: allow_write is denied by default (add a grant for ./out/note.json)","retryable":false,"source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}},"effects":"none","data_count":0}
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

- **Response envelopes** (0.2.0) carry `request_id`, `trace_id`, `operation`, `status`, `effects` and
  `data_count`, and for errors `error.source`, `error.operation_id` and `error.node_id`.
- **Trace store** (`src/infra/trace_store.rs`, `MemoryTraceStore`): every authorized or
  denied effect decision is recorded with request ID, effect ID, capability, access, target,
  decision and matching rule. Read with `rivet trace show REQUEST_ID` against a running
  `serve` (the CLI's own process exits with its trace). The answer is a `rivet.trace.show` envelope.
  Verified with a scratch copy of the 02-file-crud bundle on `127.0.0.1:18901`:

```sh
$ rivet --endpoint http://127.0.0.1:18901 trace show req_01087ca4cd
{"request_id":"req_0282643a62","trace_id":"tr_0282643a62","operation":"rivet.trace.show","type":"result","status":"ok","data":{"request_id":"req_01087ca4cd","attempts":[{"request_id":"req_01087ca4cd","trace_id":"tr_01087ca4cd","node_id":null,"attempt":1,"effect_id":"notes.create#1","operation_id":"notes.create","phase":"decision","capability":"allow_write","access":"create","target":"./out/note.json","decision":"allowed","policy_hash":"sha256:deccf2027323af83c9798a05d6cac1adb657a81852e54ac7b99ce3eca4b0bef3","source":{"file":"app.rivet","line":9,"column":5},"outcome":{"rule":"grant allow_write ./out/**"}}],"complete":true,"next_cursor":null,"gaps":0},"error":null,"effects":"none","data_count":0}
```

- **Deprecated input** (0.2.0). A request that arrived with the aliases `id`/`params` gets one extra trace row
  with phase `input`, access `deprecated` and no effect ID (`Runtime::note_deprecated_input`). Manifests
  ignore it:

```text
{"request_id":"req_030031b587",…,"attempt":0,"effect_id":null,"operation_id":"notes.read","phase":"input","capability":"input","access":"deprecated","target":"id,params","decision":"deprecated",…,"outcome":{"deprecated":["id","params"],"hint":"send `operation` and `data`; `id` and `params` are removed in 0.3.0"}}
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

`0.2.0-rc (main at 8031baa)`, 2026-09-29, macOS, `target/release/rivet` (`cargo build --release --features cli`).
Every capture was re-run: `docs/demos/05-dag`, a scratch copy of `docs/demos/02-file-crud`, and scratch
bundles for timeouts, SIGINT, DAG envelopes, streams, output validation and call depth. `rivet serve` ran on
127.0.0.1:18900 and 127.0.0.1:18901 and was stopped after each capture. The global lookup order was read from
`Frame::get` in `execution_driver.rs`. Request and trace IDs, timestamps and trace-parent headers differ on every
run.

History: `0.1.0-dev (commit 829ca43)`, 2026-09-28, macOS, `target/debug/rivet`. First verified at
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
- [API-2026-0006 Envelopes](../../api/api-2026-0006-envelopes.md) and [MIG-2026-0001](../../migrations/mig-2026-0001-response-and-input-envelopes.md)
- [PLAN-2026-0002 v0.2.0 plan](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (D-32, D-47)
- [Demo folders](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-16). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43, 2a751ab): structured cancellation (CancelToken, graceful close within the grace), DAG `started_at`/`ended_at` and merged `details.nodes`, `emits` validation, typed sink stop, enforced `max_buffered_bytes`, `--timeout` cap, `effects` folding (`unknown` for opaque MCP), file effect scopes, `with file open`, flock-guarded compare-and-replace, secret taint on every sink, per-request restriction; limitations reduced to the current ones. |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-32/D-47 (TASK-073, TASK-070): frame global scope and lookup order (locals → params → file globals); outcome rendered as a ResponseEnvelope at the surfaces; CLI `--data`/`--input` (`--params` deprecated); HTTP `{operation, data}`; library `call`/`load`; every capture re-run on the 0.2.0-rc (envelopes); deprecated-input trace row. |
