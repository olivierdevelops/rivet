---
document_id: DEMO-2026-0008
title: "UDP request and separately authorized reply"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [registry]
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

# UDP request and separately authorized reply

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

UDP request and separately authorized reply. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `telemetry.status` | `object {state}` | Query one UDP peer. |
| `telemetry.receive` | `object {peer, value}` | Bind, receive and authorize a reply. |

```text
 telemetry.status  (policy.json)                  telemetry.receive  (policies/receive.json)

 rivet ──{"command":"status"}──> 127.0.0.1:7000    fixture 127.0.0.1:7002 ──datagram──> rivet bind 127.0.0.1:7001
       <──{"state":"ready"}────  (timeout "1s")                                          (allow_listen)
                                                   rivet ──{"received":true}──> 127.0.0.1:7002
                                                          (needs allow_network for that exact peer)
```

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17) and S81–S82. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/08-udp
```

Provide these UDP fixtures. No datagram server implementation is included.

- A peer on 127.0.0.1:7000 that accepts the JSON datagram `{"command":"status"}` and replies `{"state":"ready"}`.
- For `telemetry.receive`, a fixture bound to 127.0.0.1:7002 that sends one JSON datagram to 127.0.0.1:7001.

The two operations need different authority, so each has its own policy file:

```text
  policy.json              (auto-discovered)  allow_network udp://127.0.0.1:7000
  policies/receive.json    (--policy)         allow_listen  udp://127.0.0.1:7001
                                              allow_network udp://127.0.0.1:7002
```

`network.deny_private_ranges` is true, the default, so loopback is denied unless a grant names the IP literally. Both files do. A hostname that resolves to 127.0.0.1 would not be allowed.

## Steps

### Command / Request

```sh
rivet --file app.rivet request telemetry.status --params '{}'
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive --params '{}'
```

View the declared outputs:

```sh
rivet --file app.rivet outputs telemetry.receive
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

Status returns `{"state":"ready"}` (exit 0). Receive sends `{"received":true}`, then returns the peer and decoded value (exit 0). A datagram from an unexpected source port does not inherit reply permission, so `send_to` fails with `permission.denied` (exit 3). A lost reply times out rather than silently retrying (`timeout`, exit 6). A truncated datagram is `udp.truncated` (kind protocol, HTTP 502, exit 5). Running `telemetry.receive` without `--policy ./policies/receive.json` is denied (exit 3).

`rivet outputs telemetry.receive`:

```text
telemetry.receive — Receive one datagram
output  object   The received datagram and its sender.
  peer     text     required  Sender address as host:port.
  value    json     required  Decoded JSON datagram payload.
emits    —
receives —
errors   —
```

`rivet outputs --all --json` (first entry shown):

```json
[
  {"id": "telemetry.receive",
   "output": {"type": "object", "description": "The received datagram and its sender.",
              "properties": {"peer": {"type": "string", "description": "Sender address as host:port."},
                             "value": {"description": "Decoded JSON datagram payload."}},
              "required": ["peer", "value"], "additionalProperties": false},
   "emits": null, "receives": null, "errors": []}
]
```

## Effects and policy

The listener grant and the outbound peer grant are separate. A successful send means local acceptance, not delivery or exactly-once remote execution. `max_datagram` options come before the first body statement (leading-options rule).

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet --policy ./policies/receive.json io telemetry.receive --check-policy
```

`check --strict-docs` passes: both public operations describe their output and fields, and neither body uses `fail`.

**`io --by target`**. The reply address comes from the received datagram, so that target is `dynamic`:

```text
TARGET                   ACCESS    CAPABILITY      ORIGIN            PHASE     NEEDS FILE   USED BY
udp://127.0.0.1:7000     connect   allow_network   with udp          connect   —            telemetry.status
udp://127.0.0.1:7001     bind      allow_listen    with udp bind     connect   —            telemetry.receive
udp://{message.peer}     connect   allow_network   socket.send_to    body      —            telemetry.receive (dynamic: sender of the received datagram)
```

**`io --check-policy`** with the default [policy.json](policy.json):

```text
OPERATION           KIND      ACCESS    TARGET                 KNOWLEDGE   SOURCE         DECISION
telemetry.receive   network   bind      udp://127.0.0.1:7001   exact       app.rivet:21   denied
telemetry.receive   network   connect   udp://{message.peer}   dynamic     app.rivet:24   unknown
telemetry.status    network   connect   udp://127.0.0.1:7000   exact       app.rivet:7    allowed
```

It exits 3, because the bind is denied. Under [policies/receive.json](policies/receive.json), `io telemetry.receive --check-policy` shows the bind `allowed` and the reply `unknown`. It exits 0, because `unknown` is not a denial. The runtime still checks the actual peer, and only `udp://127.0.0.1:7002` is granted. `--strict` exits 7 in both cases because of the dynamic site. Loopback targets pass `deny_private_ranges` only because each grant names the IP literally.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared outputs | `rivet outputs --all --json` | Two entries | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | Two `--sandbox` variants became policy.json + policies/receive.json | Commands above | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Stop the UDP fixtures. Scope exit releases the bound socket; do not add `socket.close` calls.

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
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`with udp`, `with udp bind`, `socket.send_to`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: connect/bind/dynamic reply sites by target and `io --check-policy` under both policy files. |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs; quoted `timeout "1s"`/`"5s"`; `--sandbox` variants became policy.json and policies/receive.json with literal loopback grants (deny_private_ranges); View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
