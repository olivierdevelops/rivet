---
document_id: DEMO-2026-0008
title: "UDP request and separately authorized reply"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 10
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, datagrams, policy, audit, cli, serve]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable UDP demo — one unicast request/reply, one bound receive whose reply needs its own peer grant, the policy denials, and the I/O manifest under two policy files, run against a shipped stdlib UDP fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate; TASK-067: executed against the 0.1.0 release candidate. TASK-076 (PLAN-2026-0002) re-executed it against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, PROP-2026-0001, REF-2026-0002, MAN-2026-0008, MAN-2026-0005, API-2026-0005, TEST-2026-0012, TEST-2026-0020, TEST-2026-0021, TEST-2026-0025, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, examples, udp, datagrams, policy]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# UDP request and separately authorized reply

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0 and later
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

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped to 0.2.0 at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Since 0.2.0 results and errors are ResponseEnvelopes and input is given with `--data` ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md)); UDP behaviour, grants and codes are unchanged.

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root
export PATH="$PWD/target/release:$PATH"     # rivet --version prints rivet 0.2.1
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

`outputs --all --json` prints one `rivet.outputs` envelope on one line; reformatted here, the two entries of its `data`:

```json
{"request_id": "req_01c7d66e65", "trace_id": "tr_01c7d66e65", "operation": "rivet.outputs", "type": "result", "status": "ok", "data": [
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
 ], "error": null, "effects": "none", "data_count": 0}
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

`--include-bootstrap` adds the fixed runtime-internal list under a separate `BOOTSTRAP` section (listed, not governed by policy.json). Since 0.2.0 the bundle row names the entry file and one row per imported module (0.1.0 printed `./app.rivet (+ imports)`); this bundle imports nothing:

```text
BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet
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
rivet --file app.rivet policy explain telemetry.receive --data '{}'
rivet --file app.rivet --policy ./policies/receive.json policy explain telemetry.receive --data '{}'
```

#### Expected Output / Response

Default policy (exit 3):

```text
policy   ./policy.json (sha256:01e2cdbd5f9655d2c033432b5388019d4ac9eb1f893997d26a5c0d63eae411c7)
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
Hashes vary with file content. `--data` (alias `--params`, no warning) is what makes `policy explain` evaluate one concrete call and exit 3 on a denial; without it the same table prints and the exit is 0. With `--json`, the denial is a `status: error` envelope of kind `permission` on stderr (exit 3).

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
rivet --file app.rivet request telemetry.status
```

#### Expected Output / Response

```json
{"request_id":"req_014a068a85","trace_id":"tr_014a068a85","operation":"telemetry.status","type":"result","status":"ok","data":{"state":"ready"},"error":null,"effects":"committed","data_count":0}
```

Exit 0.

### 8. `telemetry.receive` under the default policy — denied

#### Command / Request

```sh
rivet --file app.rivet request telemetry.receive
```

#### Expected Output / Response

```json
{"request_id":"req_014939d3dd","trace_id":"tr_014939d3dd","operation":"telemetry.receive","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_listen bind udp://127.0.0.1:18801 denied: 127.0.0.1 is a private/loopback/link-local address; grant it literally (e.g. \"udp://127.0.0.1:18801\") to allow it","retryable":false,"source":{"file":"app.rivet","line":21,"column":5,"end_line":26,"end_column":8},"operation_id":"telemetry.receive","details":{"capability":"allow_listen","access":"bind","target":"udp://127.0.0.1:18801"}},"effects":"none","data_count":0}
```

Exit 3, `effects: "none"` — nothing was bound. This matches the step 4 manifest. (The same command in the
unmodified folder gives the same error for `udp://127.0.0.1:7001`; no socket is opened.)

### 9. `telemetry.receive` under policies/receive.json — bind, receive, authorized reply

#### Command / Request

```sh
python3 fixtures/udp_fixture.py ping 18802 18801 & PG=$!
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive
wait $PG
```

#### Expected Output / Response

Rivet (exit 0):

```json
{"request_id":"req_0148cbc08d","trace_id":"tr_0148cbc08d","operation":"telemetry.receive","type":"result","status":"ok","data":{"peer":"127.0.0.1:18802","value":{"reading":42}},"error":null,"effects":"committed","data_count":0}
```

The ping fixture (exit 0):

```text
reply from 127.0.0.1:18801: {"received":true}
```

### 10. Datagram from an unexpected source port — reply denied

#### Command / Request

```sh
python3 fixtures/udp_fixture.py ping 18803 18801 & PG=$!
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive
kill $PG
```

#### Expected Output / Response

Receiving from `:18803` does not grant replying to it; `send_to` fails:

```json
{"request_id":"req_0145d057f5","trace_id":"tr_0145d057f5","operation":"telemetry.receive","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_network connect udp://127.0.0.1:18803 denied: 127.0.0.1 is a private/loopback/link-local address; grant it literally (e.g. \"udp://127.0.0.1:18803\") to allow it","retryable":false,"source":{"file":"app.rivet","line":24,"column":9,"end_line":24,"end_column":58},"operation_id":"telemetry.receive","details":{"capability":"allow_network","access":"connect","target":"udp://127.0.0.1:18803"}},"effects":"none","data_count":0}
```

Exit 3. The source points at line 24 (`socket.send_to`). The ping fixture never gets a reply.

### 11. No datagram arrives — receive timeout

#### Command / Request

```sh
rivet --file app.rivet --policy ./policies/receive.json request telemetry.receive
```

#### Expected Output / Response

After 5 s (exit 6):

```json
{"request_id":"req_01423cc2e5","trace_id":"tr_01423cc2e5","operation":"telemetry.receive","type":"result","status":"error","data":null,"error":{"kind":"timeout","code":"timeout","message":"no datagram within 5000 ms","retryable":false,"source":{"file":"app.rivet","line":23,"column":9,"end_line":23,"end_column":56},"operation_id":"telemetry.receive"},"effects":"none","data_count":0}
```

### 12. Lost reply — status timeout, no silent retry

#### Command / Request

```sh
kill $FX
python3 fixtures/udp_fixture.py status 18800 --silent & FX=$!
rivet --file app.rivet request telemetry.status
```

#### Expected Output / Response

After 1 s (exit 6). `effects` is `committed` because the status datagram was sent; Rivet does not resend it:

```json
{"request_id":"req_01d8454a5d","trace_id":"tr_01d8454a5d","operation":"telemetry.status","type":"result","status":"error","data":null,"error":{"kind":"timeout","code":"timeout","message":"no datagram within 1000 ms","retryable":false,"source":{"file":"app.rivet","line":10,"column":9,"end_line":10,"end_column":48},"operation_id":"telemetry.status"},"effects":"committed","data_count":0}
```

### 13. Oversized reply — `udp.truncated` (CLI exit 5, HTTP 502)

#### Command / Request

```sh
kill $FX
python3 fixtures/udp_fixture.py status 18800 --big 9000 & FX=$!
rivet --file app.rivet request telemetry.status

# Same failure through one `rivet serve` listener:
rivet serve --file app.rivet --listen 127.0.0.1:18810 & SV=$!
curl -s -w '\n%{http_code}\n' -X POST http://127.0.0.1:18810/v1/request \
  -H 'content-type: application/json' -d '{"operation":"telemetry.status","data":{}}'
kill $SV
```

#### Expected Output / Response

A 9000-byte datagram exceeds `max_datagram 8192`; it is rejected, never parsed clipped (exit 5):

```json
{"request_id":"req_015e4ccd2d","trace_id":"tr_015e4ccd2d","operation":"telemetry.status","type":"result","status":"error","data":null,"error":{"kind":"protocol","code":"udp.truncated","message":"a datagram from 127.0.0.1:18800 exceeds max_datagram 8192; it is not parsed clipped","retryable":false,"source":{"file":"app.rivet","line":10,"column":9,"end_line":10,"end_column":48},"operation_id":"telemetry.status","details":{"peer":"127.0.0.1:18800"}},"effects":"committed","data_count":0}
```

The `curl` call prints the same error envelope (its own request ID) followed by `502`; the serve access log on stderr shows
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

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 / U-02 | UQ-03/05 / R1, R2 | Results and errors are ResponseEnvelopes on the CLI and HTTP | Steps 7–13 | Same codes, exits, `effects` and HTTP 502 as 0.1.0 | This README steps 7–13 (2026-09-29, 8031baa) |
| U-19 | UQ-09 / R19 | The bootstrap list names the entry file (and each imported module) instead of `(+ imports)` | Step 5 `--include-bootstrap` | `file read ./app.rivet` | This README step 5 |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-15 | UQ-14 / R15 | UDP `with udp`, `with udp bind`, `send`/`receive`/`receive_from`/`send_to`, `max_datagram`, per-peer reply authorization | Steps 7–13 | Reply `{"state":"ready"}`; reply to granted peer only; `timeout` exit 6; `udp.truncated` exit 5 / HTTP 502 | This README steps 7–13 (re-run 2026-09-29, 8031baa); TEST-2026-0012 |
| U-23 | UQ-17 / R23 | Declared outputs | Step 2: `rivet outputs --all --json` | Two entries | This README step 2; TEST-2026-0020 |
| U-24 | UQ-17 / R24 | Two `--sandbox` variants became policy.json + policies/receive.json | Steps 6, 8, 9 | Default policy denies the bind; receive.json allows bind and the 18802 reply | This README steps 6, 8, 9; TEST-2026-0021 |
| U-26 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`); B3: manifest decisions mirror the runtime permit | Steps 3–5 | Tables above; exits 3 / 0 / 3 / 7 | This README steps 3–5; TEST-2026-0025 |

## Cleanup

```sh
kill $FX 2>/dev/null; kill $PG 2>/dev/null; kill $SV 2>/dev/null
cd - && rm -rf "$WORK"
```

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| `check --strict-docs` / `check` (step 1) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. View the declared outputs (envelope) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. I/O manifest by target | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `io --check-policy` (steps 4–5: exit 3 default; 0 / 3 / 7 under receive.json; 3 for `--check-policy --strict` under the default) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `io --check-files` (step 4); `--include-bootstrap` (no `(+ imports)`) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. Explain the policy decision (`--params '{}'`: exit 3 / 0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. (re-run after INC-2026-0012) `policy explain … --data '{}'`: default policy exit 3 (same table and `denied:` line), receive.json exit 0; `--json` denial envelope on stderr (exit 3); without `--data` exit 0 | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 7. `telemetry.status` unicast request/reply | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 8. `telemetry.receive` denied under default policy | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 9. `telemetry.receive` under receive.json with authorized reply | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 10. Unexpected source port — reply denied | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 11. Receive timeout | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 12. Lost status reply — timeout | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 13. Oversized reply — `udp.truncated` (CLI and serve, HTTP 502) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| Multicast | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | NOT APPLICABLE — this bundle declares no multicast operation; see tests/conformance_udp.rs |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`); every command above was executed from this folder (or the scratch copy named in Local fixture run) and the output pasted from that run. The 0.1.0 verification (TASK-067, commit 829ca43) is recorded in an earlier revision below. Step 6 was re-run with `policy explain --data` on 2026-09-29 at commit `7c25175` (source = `14750b8`) after the INC-2026-0012 fixes; its output above is from that run.

## Known Caveats

- Request IDs, trace IDs, policy hashes and ports vary; compare application results, codes and exits rather than literal IDs.
- The fixture run uses ports 18800–18803 and 18810 instead of the bundle's 7000–7002; any free loopback ports work as long as `app.rivet` and both policy files are rewritten consistently.
- `permission.denied` for a loopback target that no grant names reads "… is a private/loopback/link-local address; grant it literally …" even when the fix is to add the grant at all; the hint is correct either way.
- A lost reply is not retried: `telemetry.status` times out with `effects: "committed"` because the request datagram already left.
- In step 10 the received datagram is consumed before `send_to` is refused; the error reports `effects: "none"` because no outbound effect happened.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
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
| 10 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 9 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 8 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 6 uses `policy explain --data` (alias `--params`) and was re-run (output unchanged); the `--json` denial envelope on stderr noted; one Verification Record row. |
| 7 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-57): re-executed every step against the 0.2.0 release candidate (8031baa) with the local UDP fixture; `--params` dropped on `request` (kept on `policy explain`), HTTP body `{operation, data}`; results and errors replaced by 0.2.0 envelopes; `outputs --all --json` as an envelope; the bootstrap row now names `./app.rivet` (INC-2026-0008 placeholder gone); full policy hash; 0.2.0 Release Updates; verified_against 0.2.0 |
| 6 | 2026-09-28 | Claude | TASK-067 re-verification at 829ca43 (after the INC-2026-0005/0006 fixes): every step re-run, output identical except IDs and hashes; commit references updated; linked the release verification guide DEMO-2026-0015 |
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (073d944); added fixtures/udp_fixture.py and a Local fixture run (ports remapped to 18800–18803); pasted real output for check, outputs (both entries), `io --by target` (row order), `io --check-policy` (summary line, receive.json whole-bundle denial of status, `--strict` exit 7, denied-beats-dynamic exit 3), `policy explain`, and every request outcome (status, denial, authorized reply, unexpected peer, both timeouts, `udp.truncated` exit 5 / HTTP 502); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`with udp`, `with udp bind`, `socket.send_to`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: connect/bind/dynamic reply sites by target and `io --check-policy` under both policy files. |
