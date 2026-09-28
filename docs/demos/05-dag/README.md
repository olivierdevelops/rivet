---
document_id: DEMO-2026-0005
title: "DAG dependencies and partial failures"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [examples]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development]
audience: [developers, reviewers]
scope: Illustrate the existing proposed interface with sample files; no runtime implementation or release claim.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PROP-2026-0001, REF-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, examples, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: not-implemented
---

# DAG dependencies and partial failures

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

DAG dependencies and partial failures. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `report.total` | `object {total}` | Parallel pure nodes followed by a join (`fail fast`). |
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

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17, review fix E6) and S44–S47 and S109. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/05-dag
```

No external services, data or grants are needed. **This folder deliberately has no policy.json.** Absence means deny-by-default for new application I/O, and every node here is pure, so nothing is denied.

## Steps

### Command / Request

```sh
rivet --file app.rivet request report.total --params '{"a":2,"b":3}'
rivet --file app.rivet request report.partial --params '{}'
rivet --file app.rivet graph report.total --json
```

View the declared outputs:

```sh
rivet --file app.rivet outputs report.partial
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

`report.total` returns `{"total":10}` (exit 0). `report.partial` returns `{"good":"succeeded","bad":"failed","blocked":"blocked"}` (exit 0, because `fail independent` keeps the pipeline going). Each node value is the envelope `{status, result, error}`, and the `.result` of a node that did not succeed is null. The blocked descendant never evaluates `bad.result`. The graph shows two independent nodes feeding one join.

`rivet outputs report.partial`:

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

`rivet outputs --all --json` lists only the two public pipelines (abbreviated):

```json
[
  {"id": "report.partial", "output": {"type": "object", "description": "Final status of each DAG node.",
    "properties": {"good": {"type": "string", "description": "Status of the independent node: succeeded."},
                   "bad": {"type": "string", "description": "Status of the failing node: failed."},
                   "blocked": {"type": "string", "description": "Status of the dependent node: blocked."}},
    "required": ["good", "bad", "blocked"], "additionalProperties": false},
   "emits": null, "receives": null, "errors": []},
  {"id": "report.total", "output": {"type": "object", "description": "Combined DAG result.",
    "properties": {"total": {"type": "integer", "description": "(a * 2) + (b * 2)."}},
    "required": ["total"], "additionalProperties": false},
   "emits": null, "receives": null, "errors": []}
]
```

| Condition | Code | CLI exit |
|---|---|---|
| Success | — | 0 |
| Missing `a` or `b` | `validation.required` | 2 |
| DAG exceeds `timeout "5s"` | `timeout` | 6 |

## Effects and policy

Pure helpers need no I/O. If a helper is later replaced with an effectful operation, parent deadlines, cancellation and policy propagate to it. All DAGs and nested calls share the host-wide budget `limits.max_concurrent_requests` (default 64), which a policy.json `limits` block can lower. Private helpers are absent from external catalogs and cannot be called via generic request.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --all --by target
rivet --file app.rivet io --all --check-policy
```

`check --strict-docs` passes:

- Public pipelines describe their params, output and fields.
- The only `fail` code, `fixture.failure`, is declared on `fixture.fail`.
- Literal `(request "id" …)` targets form no cycle (`check.call_cycle` would reject one).

`report.total` reads `total.result` without a status guard, so `check` reports a warning (not a strict-docs error). At run time `fail fast` fails the pipeline before `return` when any node fails. `report.partial` reads only `.status` and needs no guard. Nested calls deeper than `limits.max_call_depth` (default 16) fail with `limit.call_depth`.

**I/O manifest.** `--all` adds the three private helpers. Following every DAG node transitively still finds no effect site:

```text
TARGET   ACCESS   CAPABILITY   ORIGIN   PHASE   NEEDS FILE   USED BY
(no application effect sites: report.partial, report.total and private
 helpers fixture.fail, math.double, math.sum are pure)
```

`io --all --check-policy` runs with no policy.json (deny-by-default). There are no sites to deny, so it exits 0. If a helper later gains an effect, it appears under every pipeline that reaches it, for example `report.total (via math.double)`.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared outputs and helper error | `rivet outputs --all --json` | Two public entries | Not run — runtime does not exist |
| U-02 | UQ-17 (review D) | Capy prefix calls and quoted durations in DAG nodes | `check --strict-docs` | Parses cleanly | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Nothing to clean up; no files, servers or sessions are created.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags or bare durations | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --all --by target` shows no sites; `io --all --check-policy` exits 0 under deny-by-default. |
| 2 | 2026-09-28 | Claude | UQ-17: Capy prefix `(request …)` calls, `timeout "5s"`, declared outputs and `fixture.failure` error; removed `--sandbox ""` (no policy.json = deny-by-default); node envelope/state machine per E6; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
