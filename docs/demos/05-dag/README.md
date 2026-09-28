---
document_id: DEMO-2026-0005
title: "DAG dependencies and partial failures"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, cli]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable DAG demo — two independent nodes joined by a third (fail fast), and independent success, failure and a blocked descendant (fail independent), with private helpers, the static call graph and the I/O manifest.
reason: User requested sample folders with READMEs showing usage; UQ-11 asks for DAGs; UQ-17 adds declared outputs; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0003, TEST-2026-0007, TEST-2026-0016, TEST-2026-0020, TEST-2026-0024]
supersedes: null
superseded_by: null
tags: [rivet, demo, dag, pipeline, graph]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# DAG dependencies and partial failures

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
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

Each node value is the envelope `{status, result, error}`; `.result` of a node that did not succeed is null.

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request and trace IDs vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0-dev until the P5 bump
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

`outputs --all --json` lists the two public entries on one line:

```json
[{"id":"report.partial","output":{"type":"object","properties":{"good":{"type":"string","description":"Status of the independent node: succeeded."},"bad":{"type":"string","description":"Status of the failing node: failed."},"blocked":{"type":"string","description":"Status of the dependent node: blocked."}},"required":["good","bad","blocked"],"additionalProperties":false,"description":"Final status of each DAG node."},"emits":null,"receives":null,"errors":[]},{"id":"report.total","output":{"type":"object","properties":{"total":{"type":"integer","description":"(a * 2) + (b * 2)."}},"required":["total"],"additionalProperties":false,"description":"Combined DAG result."},"emits":null,"receives":null,"errors":[]}]
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

`--json` prints `{"operation_id":"report.total","root":"n0","nodes":[…],"edges":[…]}` with `contains` edges for nesting and `after` edges for dependencies, for example `{"from":"n2","to":"n6","kind":"after"}` (first → total). All exit 0.

### 3. Run both pipelines

#### Command / Request

```sh
rivet --file app.rivet request report.total --params '{"a":2,"b":3}'
rivet --file app.rivet request report.partial --params '{}'
```

#### Expected Output / Response

```json
{"request_id":"req_019f8495d5","trace_id":"tr_019f8495d5","result":{"total":10},"data_count":0,"effects":"none"}
{"request_id":"req_019e8fd9c5","trace_id":"tr_019e8fd9c5","result":{"good":"succeeded","bad":"failed","blocked":"blocked"},"data_count":0,"effects":"none"}
```

Both exit 0. `report.partial` succeeds because `fail independent` keeps unrelated nodes going; `blocked` never evaluated `bad.result`.

### 4. Failing examples

#### Command / Request

```sh
rivet --file app.rivet request report.total --params '{"a":2}'
rivet --file app.rivet request math.double --params '{"value":2}'
```

#### Expected Output / Response

A missing parameter is refused before any node runs (exit 2):

```json
{"request_id":"req_019d37359d","trace_id":"tr_019d37359d","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `b`","retryable":false,"effects":"none","operation_id":"report.total","details":{"field":"b"}}}
```

A private helper is not reachable from outside (exit 4):

```json
{"request_id":"req_019c146c95","trace_id":"tr_019c146c95","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `math.double`","retryable":false,"effects":"none","operation_id":"math.double"}}
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

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-10 | UQ-11 / R10 | `dag` with `after`, `limit`, `timeout`, `fail fast` / `fail independent`, node envelopes | Steps 2–3 | `{total:10}`; `succeeded/failed/blocked` | This README steps 2–3 (2026-09-28, 829ca43); TEST-2026-0007 |
| U-20 | UQ-15 / R20 | Several described operations, private helpers hidden | Steps 1, 4 | Two listed; helper `not_found.operation` | This README steps 1, 4; TEST-2026-0016 |
| U-23 | UQ-17 / R23 | Declared outputs | Step 1 | Two entries | This README step 1; TEST-2026-0020 |

## Cleanup

Nothing to clean up; no files, servers or sessions are created.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `list`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `graph` text and JSON | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. `report.total`, `report.partial` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. Missing param; private helper | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. `io --all --by target`, `io --check-policy` (exit 0), `io --check-files` (exit 0) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder and the output pasted from that run.

## Known Caveats

- `io --check-policy` prints `(calls X — no I/O)` for call edges to pure helpers; a callee with its own rows is marked `see above`.
- The DAG timeout (`timeout "5s"`) cannot be triggered with pure helpers; it is covered by TEST-2026-0007.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Language guide](../../manuals/man-2026-0003-language-guide.md) · [DAG tests TEST-2026-0007](../../testing/test-2026-0007-dag.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (829ca43) and pasted real output: `check` (5 declarations, no unguarded-result warning), `list`, `outputs`, `graph` text/JSON, both pipelines, missing param, private helper `not_found.operation`, manifest (`--check-policy` lists call edges); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --all --by target` shows no sites; `io --all --check-policy` exits 0 under deny-by-default. |
| 2 | 2026-09-28 | Claude | UQ-17: Capy prefix `(request …)` calls, `timeout "5s"`, declared outputs and `fixture.failure` error; removed `--sandbox ""` (no policy.json = deny-by-default); node envelope/state machine per E6; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
