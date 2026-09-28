---
document_id: DEMO-2026-0005
title: "DAG dependencies and partial failures"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 6
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, cli]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable DAG demo — two independent nodes joined by a third (fail fast), and independent success, failure and a blocked descendant (fail independent), with private helpers, the static call graph and the I/O manifest.
reason: User requested sample folders with READMEs showing usage; UQ-11 asks for DAGs; UQ-17 adds declared outputs; TASK-067 executed every step against the 0.1.0 release candidate. TASK-076 (PLAN-2026-0002) re-executed it against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0003, TEST-2026-0007, TEST-2026-0016, TEST-2026-0020, TEST-2026-0024, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, demo, dag, pipeline, graph]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# DAG dependencies and partial failures

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, cli

## Purpose

DAG dependencies and partial failures. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `report.total` | `object {total}` | Two parallel pure nodes followed by a join (`fail fast`). |
| `report.partial` | `object {good, bad, blocked}` | Independent success, failure and blocked descendant (`fail independent`). |

The private helpers `math.double`, `math.sum` and `fixture.fail` are not in the external catalog. `fixture.fail` still declares its `fixture.failure` error.

```text
 report.total  (dag limit 2 timeout "5s" fail fast)       report.partial (dag limit 2 fail independent)

   first = (request "math.double" {value: a})               good = (request "math.double" {value: 4})
   second = (request "math.double" {value: b})              bad  = (request "fixture.fail" {})
          \              /                                            |
           v            v                                             v
   total after [first, second] =                           blocked after [bad]   -> never evaluated
     (request "math.sum" {a: first.result, b: second.result})

 node state machine:
   pending --deps ok--> ready --> running --> succeeded
      |                             |------> failed
      |--dep failed/blocked--> blocked       |------> cancelled
      |--fail fast before ready--> skipped
```

Each node value is the script-level node record `{status, result, error}`; `.result` of a node that did not succeed is null. This in-language shape is unchanged in 0.2.0; only what the operation finally returns is wrapped in a ResponseEnvelope on the wire.

## Verified Against Version

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped to 0.2.0 at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Since 0.2.0 results and errors are ResponseEnvelopes and input is given with `--data` ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md)). Every output block below was pasted from that run. Request and trace IDs vary.

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root
export PATH="$PWD/target/release:$PATH"     # the release candidate prints rivet 0.1.0 until the P5 bump
```

## Setup

```sh
cd docs/demos/05-dag
```

No external services, data or grants are needed. **This folder deliberately has no policy.json.** Absence means deny-by-default for application I/O, and every node here is pure, so nothing is denied.

## Steps

### 1. Check, list and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet list
rivet --file app.rivet outputs report.partial
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

`check` counts all five declarations, including the private helpers:

```text
ok: 5 operations, 0 connectors, 0 auth profiles
```

`list` shows only the two public pipelines:

```text
ID              NAME                   DESCRIPTION
report.total    Compute a DAG total    Run two nodes independently and combine their results.
report.partial  Inspect a partial DAG  Keep independent success and report a blocked dependent node.
```

```text
report.partial — Inspect a partial DAG
output  object   Final status of each DAG node.
  good     text     required  Status of the independent node: succeeded.
  bad      text     required  Status of the failing node: failed.
  blocked  text     required  Status of the dependent node: blocked.
emits    —
receives —
errors   —
```

`outputs --all --json` prints one `rivet.outputs` envelope whose `data` lists the two public entries:

```json
{"request_id":"req_015bd2079d","trace_id":"tr_015bd2079d","operation":"rivet.outputs","type":"result","status":"ok","data":[{"id":"report.partial","output":{"type":"object","properties":{"good":{"type":"string","description":"Status of the independent node: succeeded."},"bad":{"type":"string","description":"Status of the failing node: failed."},"blocked":{"type":"string","description":"Status of the dependent node: blocked."}},"required":["good","bad","blocked"],"additionalProperties":false,"description":"Final status of each DAG node."},"emits":null,"receives":null,"errors":[]},{"id":"report.total","output":{"type":"object","properties":{"total":{"type":"integer","description":"(a * 2) + (b * 2)."}},"required":["total"],"additionalProperties":false,"description":"Combined DAG result."},"emits":null,"receives":null,"errors":[]}],"error":null,"effects":"none","data_count":0}
```

All exit 0. `check` prints no warnings: literal `(request "id" …)` targets form no cycle, the only `fail` code is declared, and `report.total` reads `total.result` inside a `fail fast` DAG, where any failed node already fails the pipeline.

### 2. Static call graph

#### Command / Request

```sh
rivet --file app.rivet graph report.total
rivet --file app.rivet graph report.partial
rivet --file app.rivet graph report.total --json
```

#### Expected Output / Response

```text
report.total                            app.rivet:29
└── dag                                 app.rivet:37
    ├── node first                      app.rivet:38
    │   └── call math.double            app.rivet:38
    ├── node second                     app.rivet:39
    │   └── call math.double            app.rivet:39
    └── node total after first, second  app.rivet:40
        └── call math.sum               app.rivet:40
```

```text
report.partial                  app.rivet:45
└── dag                         app.rivet:53
    ├── node good               app.rivet:54
    │   └── call math.double    app.rivet:54
    ├── node bad                app.rivet:55
    │   └── call fixture.fail   app.rivet:55
    └── node blocked after bad  app.rivet:56
        └── call math.double    app.rivet:56
```

`--json` prints a `rivet.graph` envelope whose `data` is `{"operation_id":"report.total","root":"n0","nodes":[…],"edges":[…]}` with `contains` edges for nesting and `after` edges for dependencies, for example `{"from":"n2","to":"n6","kind":"after"}` (first → total). All exit 0.

### 3. Run both pipelines

#### Command / Request

```sh
rivet --file app.rivet request report.total --data '{"a":2,"b":3}'
rivet --file app.rivet request report.partial
```

#### Expected Output / Response

```json
{"request_id":"req_0158d36105","trace_id":"tr_0158d36105","operation":"report.total","type":"result","status":"ok","data":{"total":10},"error":null,"effects":"none","data_count":0}
{"request_id":"req_01579880b5","trace_id":"tr_01579880b5","operation":"report.partial","type":"result","status":"ok","data":{"good":"succeeded","bad":"failed","blocked":"blocked"},"error":null,"effects":"none","data_count":0}
```

Both exit 0 with `status: "ok"`. `report.partial` succeeds because `fail independent` keeps unrelated nodes going; `blocked` never evaluated `bad.result`.

### 4. Failing examples

#### Command / Request

```sh
rivet --file app.rivet request report.total --data '{"a":2}'
rivet --file app.rivet request math.double --data '{"value":2}'
```

#### Expected Output / Response

A missing parameter is refused before any node runs (exit 2):

```json
{"request_id":"req_0156fd3a1d","trace_id":"tr_0156fd3a1d","operation":"report.total","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `b`","retryable":false,"operation_id":"report.total","details":{"field":"b"}},"effects":"none","data_count":0}
```

A private helper is not reachable from outside (exit 4):

```json
{"request_id":"req_0155cdee55","trace_id":"tr_0155cdee55","operation":"math.double","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `math.double`","retryable":false,"operation_id":"math.double"},"effects":"none","data_count":0}
```

### 5. I/O manifest

#### Command / Request

```sh
rivet --file app.rivet io --all --by target
rivet --file app.rivet io --all --check-policy
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

Following every DAG node transitively finds no effect site:

```text
TARGET  ACCESS  CAPABILITY  ORIGIN  PHASE  NEEDS FILE  USED BY
(no I/O sites)
```

`--check-policy` lists each call edge instead of a site (no policy.json, deny-by-default, nothing to deny):

```text
OPERATION       KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE        DECISION
report.partial  (calls math.double — no I/O)     app.rivet:54
report.partial  (calls fixture.fail — no I/O)    app.rivet:55
report.partial  (calls math.double — no I/O)     app.rivet:56
report.total    (calls math.double — no I/O)     app.rivet:38
report.total    (calls math.double — no I/O)     app.rivet:39
report.total    (calls math.sum — no I/O)        app.rivet:40
```

`io --check-files` prints `report.partial needs no existing files.`, `report.total needs no existing files.` and `0 files`. All exit 0. If a helper later gains an effect, its sites appear under every pipeline that reaches it.

## Effects and policy

```text
  outcome table (verified in steps 3–4)
  ┌───────────────────────────────────┬─────────────────────┬──────┐
  │ situation                         │ code                │ exit │
  ├───────────────────────────────────┼─────────────────────┼──────┤
  │ report.total {a:2, b:3}           │ — → {total:10}      │  0   │
  │ report.partial                    │ — → statuses        │  0   │
  │ missing b                         │ validation.required │  2   │
  │ calling a private helper          │ not_found.operation │  4   │
  └───────────────────────────────────┴─────────────────────┴──────┘
```

All DAGs and nested calls share the host-wide budget `limits.max_concurrent_requests` (default 64). Nested calls deeper than `limits.max_call_depth` (default 16) fail with `limit.call_depth`. If a helper becomes effectful, parent deadlines, cancellation and policy propagate to it.

## Release Updates

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 / U-02 | UQ-03/05 / R1, R2 | Results and errors are ResponseEnvelopes | Steps 3–4 | `data` `{total:10}`; `status: error` for the two failures | This README steps 3–4 (2026-09-29, 8031baa) |
| U-03 | UQ-03 / R3 | `outputs --json` and `graph --json` are envelopes | Steps 1–2 | `rivet.outputs`, `rivet.graph` | This README steps 1–2 |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-10 | UQ-11 / R10 | `dag` with `after`, `limit`, `timeout`, `fail fast` / `fail independent`, node envelopes | Steps 2–3 | `{total:10}`; `succeeded/failed/blocked` | This README steps 2–3 (re-run 2026-09-29, 8031baa); TEST-2026-0007 |
| U-20 | UQ-15 / R20 | Several described operations, private helpers hidden | Steps 1, 4 | Two listed; helper `not_found.operation` | This README steps 1, 4; TEST-2026-0016 |
| U-23 | UQ-17 / R23 | Declared outputs | Step 1 | Two entries | This README step 1; TEST-2026-0020 |

## Cleanup

Nothing to clean up; no files, servers or sessions are created.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `list`, `outputs`, `outputs --all --json` (envelope) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. `graph` text and JSON (envelope) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. `report.total`, `report.partial` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 4. Missing param; private helper | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 5. `io --all --by target`, `io --check-policy` (exit 0), `io --check-files` (exit 0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`); every command above was executed from this folder and the output pasted from that run. The 0.1.0 verification (TASK-067, commit 829ca43) is recorded in revision 5 below.

## Known Caveats

- `io --check-policy` prints `(calls X — no I/O)` for call edges to pure helpers; a callee with its own rows is marked `see above`.
- The DAG timeout (`timeout "5s"`) cannot be triggered with pure helpers; it is covered by TEST-2026-0007.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Language guide](../../manuals/man-2026-0003-language-guide.md) · [DAG tests TEST-2026-0007](../../testing/test-2026-0007-dag.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-54): re-executed every step against the 0.2.0 release candidate (8031baa); `--params` → `--data`; results, errors, `outputs --all --json` and `graph --json` shown as 0.2.0 envelopes; node records noted as unchanged in-language shapes; 0.2.0 Release Updates; verified_against 0.2.0 |
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (829ca43) and pasted real output: `check` (5 declarations, no unguarded-result warning), `list`, `outputs`, `graph` text/JSON, both pipelines, missing param, private helper `not_found.operation`, manifest (`--check-policy` lists call edges); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --all --by target` shows no sites; `io --all --check-policy` exits 0 under deny-by-default. |
| 2 | 2026-09-28 | Claude | UQ-17: Capy prefix `(request …)` calls, `timeout "5s"`, declared outputs and `fixture.failure` error; removed `--sandbox ""` (no policy.json = deny-by-default); node envelope/state machine per E6; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
