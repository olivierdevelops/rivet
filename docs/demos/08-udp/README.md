---
document_id: DEMO-2026-0008
title: "UDP request and separately authorized reply"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 6
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, datagrams, policy, audit, cli, serve]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable UDP demo — one unicast request/reply, one bound receive whose reply needs its own peer grant, the policy denials, and the I/O manifest under two policy files, run against a shipped stdlib UDP fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate; TASK-067: executed against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, PROP-2026-0001, REF-2026-0002, MAN-2026-0008, MAN-2026-0005, API-2026-0005, TEST-2026-0012, TEST-2026-0020, TEST-2026-0021, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, examples, udp, datagrams, policy]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# UDP request and separately authorized reply

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, datagrams, policy, audit, cli, serve

## Purpose

UDP request and separately authorized reply. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `telemetry.status` | `object {state}` | Send `{"command":"status"}` to one UDP peer and wait up to `1s` for the reply. |
| `telemetry.receive` | `object {peer, value}` | Bind, receive one datagram (up to `5s`), reply `{"received":true}` to its sender, return sender + payload. |

```text
 telemetry.status  (policy.json)                   telemetry.receive  (policies/receive.json)

 rivet ──{"command":"status"}──> 127.0.0.1:7000     peer 127.0.0.1:7002 ──{"reading":42}──> rivet bind 127.0.0.1:7001
       <──{"state":"ready"}────  (timeout "1s")                                              (allow_listen)
       needs allow_network udp://127.0.0.1:7000     rivet ──{"received":true}──> 127.0.0.1:7002
                                                          (send_to: needs allow_network for THAT exact peer;
                                                           receiving from it grants nothing)
```

The two capabilities are independent. Being allowed to *listen* never implies being allowed to *answer*:

```text
             allow_listen udp://127.0.0.1:7001          allow_network udp://<peer>
                         │                                         │
   datagram ──> [ bind + receive_from ] ── message.peer ──> [ send_to message.peer ] ──> reply
                         │                                         │
                   denied → exit 3                     peer not granted → exit 3
                   (nothing bound)                     (datagram already consumed)
```

This bundle has no multicast operation. Multicast (`with udp multicast …`) and the B3 guarantee that
`io --check-policy` multicast decisions mirror the runtime permit are covered by
[tests/conformance_udp.rs](../../../tests/conformance_udp.rs) (`multicast_manifest_agrees_with_runtime_permits`),
which also asserts the manifest verdicts for this bundle shown in steps 4–5.

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit 829ca43, the release candidate (the version bump to 0.1.0 happens at release, P5), with target/release/rivet on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0-dev until the P5 bump
```

- `python3` (3.8+, standard library only) for the fixture [fixtures/udp_fixture.py](fixtures/udp_fixture.py).
- `curl` for the optional HTTP check in step 13.
- Free loopback UDP ports. The bundle names 7000–7002; the local fixture run remaps them to 18800–18803
  (see [Local fixture run](#local-fixture-run)) so it does not collide with anything on your machine.

Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/08-udp
```

The two operations need different authority, so each has its own policy file:

```text
  policy.json              (auto-discovered)  allow_network udp://127.0.0.1:7000
  policies/receive.json    (--policy)         allow_listen  udp://127.0.0.1:7001
                                              allow_network udp://127.0.0.1:7002
```

`network.deny_private_ranges` is true, the default, so loopback is denied unless a grant names the IP literally. Both files do. A hostname that resolves to 127.0.0.1 would not be allowed. Note that the two files are
disjoint: `receive.json` does **not** grant the status peer, so under it `telemetry.status` is denied (step 5).

Steps 1–6 inspect the unmodified folder and perform no network I/O. Steps 7–13 run the operations against the
shipped fixture in a scratch copy.

## Steps

### 1. Check the bundle

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
```

#### Expected Output / Response

```text
ok: 2 operations, 0 connectors, 0 auth profiles
```

Exit 0. Both public operations describe their output and fields, and neither body uses `fail`. Plain `check` prints the same line.

### 2. View the declared outputs

#### Command / Request

```sh
rivet --file app.rivet outputs telemetry.receive
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
telemetry.receive — Receive one datagram
output  object   The received datagram and its sender.
  peer    text     required  Sender address as host:port.
  value   json     required  Decoded JSON datagram payload.
emits    —
receives —
errors   —
```

`outputs --all --json` prints one line; reformatted here, both entries:

```json
[
  {"id": "telemetry.receive",
   "output": {"type": "object",
              "properties": {"peer": {"type": "string", "description": "Sender address as host:port."},
                             "value": {"description": "Decoded JSON datagram payload."}},
              "required": ["peer", "value"], "additionalProperties": false,
              "description": "The received datagram and its sender."},
   "emits": null, "receives": null, "errors": []},
  {"id": "telemetry.status",
   "output": {"type": "object",
              "properties": {"state": {"type": "string", "description": "Peer state; the fixture answers \"ready\"."}},
              "required": ["state"], "additionalProperties": false,
              "description": "Status reported by the UDP peer."},
   "emits": null, "receives": null, "errors": []}
]
```

Both exit 0.

### 3. I/O manifest by target

#### Command / Request

```sh
rivet --file app.rivet io --by target
```

#### Expected Output / Response

The reply address comes from the received datagram, so that target is `dynamic`:

```text
TARGET                ACCESS   CAPABILITY     ORIGIN          PHASE    NEEDS FILE  USED BY
udp://127.0.0.1:7001  bind     allow_listen   with udp bind   connect  —           telemetry.receive
udp://{message.peer}  connect  allow_network  socket.send_to  body     —           telemetry.receive (dynamic: sender of the received datagram)
udp://127.0.0.1:7000  connect  allow_network  with udp        connect  —           telemetry.status
```

Exit 0.

### 4. `io --check-policy` under the default policy.json

#### Command / Request

```sh
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

```text
OPERATION          KIND     ACCESS   TARGET                KNOWLEDGE  SOURCE        DECISION
telemetry.receive  network  bind     udp://127.0.0.1:7001  exact      app.rivet:21  denied
telemetry.receive  network  connect  udp://{message.peer}  dynamic    app.rivet:24  unknown
telemetry.status   network  connect  udp://127.0.0.1:7000  exact      app.rivet:7   allowed
1 allowed · 1 denied · 1 unknown
```

Exit 3, because the bind is denied. This is exactly what the runtime does in step 8.

`io --check-files` (exit 0):

```text
telemetry.receive needs no existing files.
telemetry.status needs no existing files.
0 files
```

### 5. `io --check-policy` under policies/receive.json

#### Command / Request

```sh
rivet --file app.rivet --policy ./policies/receive.json io telemetry.receive --check-policy
rivet --file app.rivet --policy ./policies/receive.json io --check-policy
rivet --file app.rivet --policy ./policies/receive.json io telemetry.receive --check-policy --strict
```

#### Expected Output / Response

Only `telemetry.receive` (exit 0 — `unknown` is not a denial):

```text
OPERATION          KIND     ACCESS   TARGET                KNOWLEDGE  SOURCE        DECISION
telemetry.receive  network  bind     udp://127.0.0.1:7001  exact      app.rivet:21  allowed
telemetry.receive  network  connect  udp://{message.peer}  dynamic    app.rivet:24  unknown
1 allowed · 1 unknown
```

The whole bundle under receive.json (exit 3 — the status peer is not granted by this file):

```text
OPERATION          KIND     ACCESS   TARGET                KNOWLEDGE  SOURCE        DECISION
telemetry.receive  network  bind     udp://127.0.0.1:7001  exact      app.rivet:21  allowed
telemetry.receive  network  connect  udp://{message.peer}  dynamic    app.rivet:24  unknown
telemetry.status   network  connect  udp://127.0.0.1:7000  exact      app.rivet:7   denied
1 allowed · 1 denied · 1 unknown
```

With `--strict` (exit 7, because of the dynamic reply site):

```text
OPERATION          KIND     ACCESS   TARGET                KNOWLEDGE  SOURCE        DECISION
telemetry.receive  network  bind     udp://127.0.0.1:7001  exact      app.rivet:21  allowed
telemetry.receive  network  connect  udp://{message.peer}  dynamic    app.rivet:24  unknown
1 allowed · 1 unknown
io: complete=false — 1 of 2 sites is dynamic/opaque
  telemetry.receive#2  network connect  target from expression message.peer  (app.rivet:24)
```

The runtime still checks the actual peer at `send_to`, and only `udp://127.0.0.1:7002` is granted (steps 9–10).
When a site is both denied and dynamic, the denial wins: `rivet --file app.rivet io --check-policy --strict`
under the default policy prints the step 4 table plus the `complete=false` lines and exits **3**, not 7.

```text
  exit-code precedence for io --check-policy [--strict]
  ┌──────────────────────────────┬──────┐
  │ any reachable site denied    │  3   │  ← checked first
  │ --strict and complete=false  │  7   │
  │ otherwise                    │  0   │
  └──────────────────────────────┴──────┘
```

`--include-bootstrap` adds the fixed runtime-internal list under a separate `BOOTSTRAP` section (listed, not governed by policy.json):

```text
BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet (+ imports)
file  read         ./policy.json
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
```

Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression.

### 6. Explain the policy decision

#### Command / Request

```sh
rivet --file app.rivet policy explain telemetry.receive --params '{}'
rivet --file app.rivet --policy ./policies/receive.json policy explain telemetry.receive --params '{}'
```

#### Expected Output / Response

Default policy (exit 3):

```text
policy   ./policy.json (sha256:01e2cdbd…)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_network udp://127.0.0.1:7000

OPERATION          KIND     ACCESS   TARGET                KNOWLEDGE  SOURCE        DECISION
telemetry.receive  network  bind     udp://127.0.0.1:7001  exact      app.rivet:21  denied
telemetry.receive  network  connect  udp://{message.peer}  dynamic    app.rivet:24  unknown
denied: telemetry.receive#1 allow_listen udp://127.0.0.1:7001 (bind)
```

Under receive.json (exit 0): `policy ./policies/receive.json …`, `base ./policies`, grants
`allow_listen udp://127.0.0.1:7001` and `allow_network udp://127.0.0.1:7002`, bind `allowed`, reply `unknown`.
Hashes vary with file content.

## Local fixture run

The bundle pins loopback ports 7000–7002. Do not edit the folder (tests pin its I/O-manifest rows); copy it to a
scratch directory and remap the ports there, in `app.rivet` and in both policy files, keeping the literal
`127.0.0.1` so `deny_private_ranges` still passes:

```text
  folder (unmodified)          scratch copy ($WORK)
  127.0.0.1:7000  status peer  ──>  127.0.0.1:18800   fixture: udp_fixture.py status 18800
  127.0.0.1:7001  rivet bind   ──>  127.0.0.1:18801   rivet telemetry.receive
  127.0.0.1:7002  reply peer   ──>  127.0.0.1:18802   fixture: udp_fixture.py ping 18802 18801
  (not granted)                     127.0.0.1:18803   fixture: udp_fixture.py ping 18803 18801 (step 10)
```

```sh
WORK="$(mktemp -d)"
cp -R app.rivet policy.json policies fixtures "$WORK/"
cd "$WORK"
sed -i.bak -e 's/127\.0\.0\.1:7000/127.0.0.1:18800/g' \
           -e 's/127\.0\.0\.1:7001/127.0.0.1:18801/g' \
           -e 's/127\.0\.0\.1:7002/127.0.0.1:18802/g' \
           app.rivet policy.json policies/receive.json
python3 fixtures/udp_fixture.py status 18800 & FX=$!
```

The fixture prints `status fixture on udp://127.0.0.1:18800` and answers every `{"command":"status"}` with
`{"state":"ready"}`. `ping FROM TO` binds `FROM`, sends `{"reading":42}` to `TO` every 100 ms until one reply
arrives (covering the moment before Rivet binds), prints it and exits. Request and trace IDs below vary.

```text
 sequence (steps 7 and 9)

  rivet (status)            fixture :18800             fixture ping :18802        rivet (receive) :18801
      │ {"command":"status"}      │                           │                           │
      │──────────────────────────>│                           │                    bind (allow_listen)
      │     {"state":"ready"}     │                           │ {"reading":42} (every 100 ms)
      │<──────────────────────────│                           │──────────────────────────>│ receive_from
   exit 0                         │                           │                           │ peer granted?
                                  │                           │    {"received":true}      │ allow_network
                                  │                           │<──────────────────────────│ udp://…:18802
                                  │                     prints reply, exit 0           exit 0
```

### 7. `telemetry.status` — unicast request/reply

#### Command / Request

```sh
rivet --file app.rivet request telemetry.status --params '{}'
```

#### Expected Output / Response

```json
{"request_id":"req_01ce8db4a5","trace_id":"tr_01ce8db4a5","result":{"state":"ready"},"data_count":0,"effects":"committed"}
```

Exit 0.

### 8. `telemetry.receive` under the default policy — denied

#### Command / Request

```sh
rivet --file app.rivet request telemetry.receive --params '{}'
```

#### Expected Output / Response

```json
{"request_id":"req_01cd37b24d","trace_id":"tr_01cd37b24d","error":{"kind":"permission","code":"permission.denied","message":"allow_listen bind udp://127.0.0.1:18801 denied: 127.0.0.1 is a private/loopback/link-local address; grant it literally (e.g. \"udp://127.0.0.1:18801\") to allow it","retryable":false,"effects":"none","source":{"file":"app.rivet","line":21,"column":5,"end_line":26,"end_column":8},"operation_id":"telemetry.receive","details":{"capability":"allow_listen","access":"bind","target":"udp://127.0.0.1:18801"}}}
```

Exit 3, `effects: "none"` — nothing was bound. This matches the step 4 manifest. (The same command in the
unmodified folder gives the same error for `udp://127.0.0.1:7001`; no socket is opened.)

### 9. `telemetry.receive` under policies/receive.json — bind, receive, authorized reply

#### Command / Request

```sh
python3 fixtures/udp_fixture.py ping 18802 18801 & PG=$!
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive --params '{}'
wait $PG
```

#### Expected Output / Response

Rivet (exit 0):

```json
{"request_id":"req_01cc5276ad","trace_id":"tr_01cc5276ad","result":{"peer":"127.0.0.1:18802","value":{"reading":42}},"data_count":0,"effects":"committed"}
```

The ping fixture (exit 0):

```text
reply from 127.0.0.1:18801: {"received":true}
```

### 10. Datagram from an unexpected source port — reply denied

#### Command / Request

```sh
python3 fixtures/udp_fixture.py ping 18803 18801 & PG=$!
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive --params '{}'
kill $PG
```

#### Expected Output / Response

Receiving from `:18803` does not grant replying to it; `send_to` fails:

```json
{"request_id":"req_01cab6e0bd","trace_id":"tr_01cab6e0bd","error":{"kind":"permission","code":"permission.denied","message":"allow_network connect udp://127.0.0.1:18803 denied: 127.0.0.1 is a private/loopback/link-local address; grant it literally (e.g. \"udp://127.0.0.1:18803\") to allow it","retryable":false,"effects":"none","source":{"file":"app.rivet","line":24,"column":9,"end_line":24,"end_column":58},"operation_id":"telemetry.receive","details":{"capability":"allow_network","access":"connect","target":"udp://127.0.0.1:18803"}}}
```

Exit 3. The source points at line 24 (`socket.send_to`). The ping fixture never gets a reply.

### 11. No datagram arrives — receive timeout

#### Command / Request

```sh
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive --params '{}'
```

#### Expected Output / Response

After 5 s (exit 6):

```json
{"request_id":"req_01c7e76ed5","trace_id":"tr_01c7e76ed5","error":{"kind":"timeout","code":"timeout","message":"no datagram within 5000 ms","retryable":false,"effects":"none","source":{"file":"app.rivet","line":23,"column":9,"end_line":23,"end_column":56},"operation_id":"telemetry.receive"}}
```

### 12. Lost reply — status timeout, no silent retry

#### Command / Request

```sh
kill $FX
python3 fixtures/udp_fixture.py status 18800 --silent & FX=$!
rivet --file app.rivet request telemetry.status --params '{}'
```

#### Expected Output / Response

After 1 s (exit 6). `effects` is `committed` because the status datagram was sent; Rivet does not resend it:

```json
{"request_id":"req_0183664665","trace_id":"tr_0183664665","error":{"kind":"timeout","code":"timeout","message":"no datagram within 1000 ms","retryable":false,"effects":"committed","source":{"file":"app.rivet","line":10,"column":9,"end_line":10,"end_column":48},"operation_id":"telemetry.status"}}
```

### 13. Oversized reply — `udp.truncated` (CLI exit 5, HTTP 502)

#### Command / Request

```sh
kill $FX
python3 fixtures/udp_fixture.py status 18800 --big 9000 & FX=$!
rivet --file app.rivet request telemetry.status --params '{}'

# Same failure through one `rivet serve` listener:
rivet serve --file app.rivet --listen 127.0.0.1:18810 & SV=$!
curl -s -w '\n%{http_code}\n' -X POST http://127.0.0.1:18810/v1/request \
  -H 'content-type: application/json' -d '{"id":"telemetry.status","params":{}}'
kill $SV
```

#### Expected Output / Response

A 9000-byte datagram exceeds `max_datagram 8192`; it is rejected, never parsed clipped (exit 5):

```json
{"request_id":"req_012ee1280d","trace_id":"tr_012ee1280d","error":{"kind":"protocol","code":"udp.truncated","message":"a datagram from 127.0.0.1:18800 exceeds max_datagram 8192; it is not parsed clipped","retryable":false,"effects":"committed","source":{"file":"app.rivet","line":10,"column":9,"end_line":10,"end_column":48},"operation_id":"telemetry.status","details":{"peer":"127.0.0.1:18800"}}}
```

The `curl` call prints the same error body followed by `502`; the serve access log on stderr shows
`"route":"/v1/request",…,"operation":"telemetry.status","status":502`.

## Effects and policy

The listener grant and the outbound peer grant are separate. A successful send means local acceptance, not
delivery or exactly-once remote execution. `max_datagram` options come before the first body statement
(leading-options rule). Scope exit releases the bound socket; there is no `socket.close` call to add.

```text
  outcome table (verified in steps 7–13)
  ┌────────────────────────────────────────────┬───────────────────┬──────┬───────────┐
  │ situation                                  │ code              │ exit │ effects   │
  ├────────────────────────────────────────────┼───────────────────┼──────┼───────────┤
  │ status reply received                      │ —                 │  0   │ committed │
  │ receive + reply to granted peer            │ —                 │  0   │ committed │
  │ bind not granted (default policy)          │ permission.denied │  3   │ none      │
  │ reply to a peer that is not granted        │ permission.denied │  3   │ none      │
  │ nothing received in 5s                     │ timeout           │  6   │ none      │
  │ status sent, no reply in 1s                │ timeout           │  6   │ committed │
  │ datagram larger than max_datagram          │ udp.truncated     │  5   │ committed │
  │   … through serve                          │ HTTP 502          │      │           │
  └────────────────────────────────────────────┴───────────────────┴──────┴───────────┘
```

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-15 | UQ-14 / R15 | UDP `with udp`, `with udp bind`, `send`/`receive`/`receive_from`/`send_to`, `max_datagram`, per-peer reply authorization | Steps 7–13 | Reply `{"state":"ready"}`; reply to granted peer only; `timeout` exit 6; `udp.truncated` exit 5 / HTTP 502 | This README steps 7–13 (2026-09-28, 829ca43); TEST-2026-0012 |
| U-23 | UQ-17 / R23 | Declared outputs | Step 2: `rivet outputs --all --json` | Two entries | This README step 2 (2026-09-28, 829ca43); TEST-2026-0020 |
| U-24 | UQ-17 / R24 | Two `--sandbox` variants became policy.json + policies/receive.json | Steps 6, 8, 9 | Default policy denies the bind; receive.json allows bind and the 18802 reply | This README steps 6, 8, 9 (2026-09-28, 829ca43); TEST-2026-0021 |
| U-26 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`); B3: manifest decisions mirror the runtime permit | Steps 3–5 | Tables above; exits 3 / 0 / 3 / 7 | This README steps 3–5 (2026-09-28, 829ca43); TEST-2026-0025 |

## Cleanup

```sh
kill $FX 2>/dev/null; kill $PG 2>/dev/null; kill $SV 2>/dev/null
cd - && rm -rf "$WORK"
```

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| `check --strict-docs` / `check` (step 1) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. View the declared outputs | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. I/O manifest by target | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| `io --check-policy` (steps 4–5: exit 3 default; 0 / 3 / 7 under receive.json) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| `io --check-files` (step 4) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. Explain the policy decision | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. `telemetry.status` unicast request/reply | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 8. `telemetry.receive` denied under default policy | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 9. `telemetry.receive` under receive.json with authorized reply | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 10. Unexpected source port — reply denied | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 11. Receive timeout | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 12. Lost status reply — timeout | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 13. Oversized reply — `udp.truncated` (CLI and serve) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| Multicast | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | NOT APPLICABLE — this bundle declares no multicast operation; see tests/conformance_udp.rs |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder (or the scratch copy named in Local fixture run) and the output pasted from that run.

## Known Caveats

- Request IDs, trace IDs, policy hashes and ports vary; compare application results, codes and exits rather than literal IDs.
- The fixture run uses ports 18800–18803 and 18810 instead of the bundle's 7000–7002; any free loopback ports work as long as `app.rivet` and both policy files are rewritten consistently.
- `permission.denied` for a loopback target that no grant names reads "… is a private/loopback/link-local address; grant it literally …" even when the fix is to add the grant at all; the hint is correct either way.
- A lost reply is not retried: `telemetry.status` times out with `effects: "committed"` because the request datagram already left.
- In step 10 the received datagram is consumed before `send_to` is refused; the error reports `effects: "none"` because no outbound effect happened.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [Implementation plan PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [Protocols and connectors manual](../../manuals/man-2026-0008-protocols-and-connectors.md)
- [Policy and I/O manifest guide](../../manuals/man-2026-0005-policy-and-io-manifest-guide.md)
- [Error registry](../../api/api-2026-0005-error-registry.md)
- [UDP test plan TEST-2026-0012](../../testing/test-2026-0012-udp.md)
- [Declared outputs tests TEST-2026-0020](../../testing/test-2026-0020-outputs.md)
- [Policy file tests TEST-2026-0021](../../testing/test-2026-0021-policy-file.md)
- [I/O manifest tests TEST-2026-0025](../../testing/test-2026-0025-io-manifest.md)
- [UDP conformance tests](../../../tests/conformance_udp.rs)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-28 | Claude | TASK-067 re-verification at 829ca43 (after the INC-2026-0005/0006 fixes): every step re-run, output identical except IDs and hashes; commit references updated; linked the release verification guide DEMO-2026-0015 |
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (073d944); added fixtures/udp_fixture.py and a Local fixture run (ports remapped to 18800–18803); pasted real output for check, outputs (both entries), `io --by target` (row order), `io --check-policy` (summary line, receive.json whole-bundle denial of status, `--strict` exit 7, denied-beats-dynamic exit 3), `policy explain`, and every request outcome (status, denial, authorized reply, unexpected peer, both timeouts, `udp.truncated` exit 5 / HTTP 502); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`with udp`, `with udp bind`, `socket.send_to`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: connect/bind/dynamic reply sites by target and `io --check-policy` under both policy files. |
