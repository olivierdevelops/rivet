---
document_id: RUN-2026-0002
title: "Roll out a policy.json change to rivet serve"
document_type: runbook
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [policy, audit, serve, cli]
affected_versions:
  from: "0.1.0"
  to: null
last_validation_date: 2026-09-28
applicable_environments: [development, server]
escalation_path: "Operator on duty → Project maintainer (owner of policy.json and the serve deployment)"
audience: [operators, maintainers]
confidentiality: internal
scope: Change the effect grants (grants, deny, network, limits) of a running `rivet serve` safely, with an offline preview and a tested rollback.
reason: PLAN-2026-0001 D-32 — policy.json is the security boundary and is read only at startup, so every change needs a reviewed, verifiable rollout.
related_documents: [PLAN-2026-0002, API-2026-0006, SYS-2026-0003, OPS-2026-0001, RUN-2026-0001, PLAN-2026-0001, REF-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, runbook, policy, serve, rollout, rollback]
---

# Roll out a policy.json change to rivet serve

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, audit, serve, cli

## Purpose

Change what a served bundle may touch (files, network, processes, …) by editing `policy.json`, prove the effect of
the change **before** it is live, apply it with a restart, verify real decisions, and roll back if needed.

```text
 ┌────────┐   ┌──────────────────┐   ┌────────────────┐   ┌───────────┐   ┌──────────────┐   ┌──────────┐
 │1 stage │──▶│2 io --check-policy│──▶│3 policy explain│──▶│4 install +│──▶│5 verify real │──▶│ done     │
 │  edit  │   │  (offline diff)  │   │  (per operation)│   │  restart  │   │  decisions   │   │          │
 └────────┘   └──────────────────┘   └────────────────┘   └───────────┘   └──────┬───────┘   └──────────┘
      ▲                  │ unexpected decision                                   │ wrong
      └──────────────────┘                                     ┌─────────────────▼──────┐
                                                               │ R rollback: restore     │
                                                               │ policy.json.prev+restart│
                                                               └────────────────────────┘
```

## When to Use This Runbook

- Granting a served operation a new effect (for example allowing `file create`), narrowing `access` verbs, adding a
  `deny`, changing `network.deny_private_ranges` or `limits`.
- Changing `serve.surfaces` or `serve.principals` (same flow; for token changes use
  [RUN-2026-0001](run-2026-0001-rotate-serve-bearer-tokens.md)).

## Preconditions

- A `rivet` binary on the host (0.2.0: built or installed with `--features cli`) and the served bundle readable:
  the entry `.rivet` file **and every module it imports** (0.2.0). Supported hosts: macOS and Linux (Windows is not
  supported in 0.2.0, INC-2026-0011).
- A bundle of several files has **one** policy, the entry's. Change that file only; a `policy.json` beside an
  imported module is ignored (`check.module_policy_ignored`), and `io --check-policy` already covers every module's
  sites.
- The server's start command is known; the process can be signalled.
- **`policy.json` is read once at startup — a change is live only after a restart.**
- Relative file targets resolve against the **policy file's own directory**, so stage the candidate file **in the same
  directory** as the live `policy.json`.

## Required Access

Write access to the bundle directory, permission to restart `rivet serve`, and a reviewer for any change that widens
grants.

## Safety Warnings

- An invalid file (unknown key, bad verb for a capability, non-positive limit) makes the server **refuse to start**
  (`policy.invalid`, exit 2) — always preview with `--policy <candidate>` first; both preview commands parse the file
  strictly.
- `deny` always wins over `grants`. Private address ranges stay denied unless named literally.
- `io --check-policy` exits **3 when any site is denied**, including sites you intend to keep denied. Restrict it to
  the operations the change is for, or read the table.
- Stop with SIGINT (`kill -INT`) or SIGTERM: both drain in-flight work and exit 0 (since commit `829ca43`).
- A value in `limits` wider than its field (for example `4294967297`) is now `policy.invalid` too; the preview catches it.

## Procedure

Validated example (0.2.0-rc): the 02-file-crud bundle served on `127.0.0.1:18931` with `auth none` (loopback). The live policy
grants only reads; the change lets `notes.create` and `notes.update` write, while `notes.delete` stays denied.

### 0. Record the baseline

```sh
rivet --file app.rivet io --check-policy; echo "exit=$?"
```

```text
OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
notes.create  file  create  ./out/note.json  exact      app.rivet:9   denied
notes.delete  file  delete  ./out/note.json  exact      app.rivet:40  denied
notes.list    file  list    ./out            exact      app.rivet:50  allowed
notes.read    file  read    ./out/note.json  exact      app.rivet:19  allowed
notes.update  file  stat    ./out/note.json  exact      app.rivet:30  allowed
notes.update  file  update  ./out/note.json  exact      app.rivet:30  denied
3 allowed · 3 denied
exit=3
```

The running server confirms the denial:

```text
$ rivet --endpoint http://127.0.0.1:18931 request notes.create --data '{"text":"hello"}'
{"request_id":"req_0145e519d5","trace_id":"tr_0145e519d5","operation":"notes.create","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no grant for allow_write ./out/note.json","retryable":false,"source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}},"effects":"none","data_count":0}
exit 3
```

### 1. Stage the edit next to the live file

```sh
cp policy.json policy.json.prev
cat > policy.json.next <<'EOF'
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./out", "./out/**"]},
    {"capability": "allow_write", "targets": ["./out/**"], "access": ["create", "update"]}
  ]
}
EOF
```

### 2. Preview every I/O site under the candidate

```sh
rivet --file app.rivet --policy policy.json.next io --check-policy
rivet --file app.rivet --policy policy.json.next io notes.create notes.update notes.read --check-policy; echo "exit=$?"
```

```text
OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
notes.create  file  create  ./out/note.json  exact      app.rivet:9   allowed
notes.delete  file  delete  ./out/note.json  exact      app.rivet:40  denied
notes.list    file  list    ./out            exact      app.rivet:50  allowed
notes.read    file  read    ./out/note.json  exact      app.rivet:19  allowed
notes.update  file  stat    ./out/note.json  exact      app.rivet:30  allowed
notes.update  file  update  ./out/note.json  exact      app.rivet:30  allowed
5 allowed · 1 denied
…
4 allowed
exit=0
```

Compare with step 0: exactly the intended rows flipped (`create`, `update`); `delete` is still denied. Any other
flip means the edit is wrong — go back to step 1. `--json` gives the same data inside a `rivet.io` envelope
(0.2.0); `rivet … --json policy explain` answers a `rivet.policy.explain` envelope whose `data` carries the
candidate's `sha256`:

```text
{"request_id":"req_0142af133d","trace_id":"tr_0142af133d","operation":"rivet.policy.explain","type":"result","status":"ok","data":{"present":true,"file":"policy.json.next","sha256":"sha256:8cd42eb254449e0522b3cdfbb201cae88b3573689ccbb701db9adb9077152d18","grants":2,"deny":0,"broad":[]},"error":null,"effects":"none","data_count":0}
```

### 3. Explain the decision for the changed operation

```sh
rivet --file app.rivet --policy policy.json.next policy explain notes.create
```

```text
policy   policy.json.next (sha256:8cd42eb254449e0522b3cdfbb201cae88b3573689ccbb701db9adb9077152d18)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ./out, ./out/**
grant    allow_write ./out/** access [create, update]

OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE       DECISION
notes.create  file  create  ./out/note.json  exact      app.rivet:9  allowed
```

Note the candidate `sha256` — it becomes the server's `policy_hash`.

### 4. Install and restart

```sh
cp policy.json.next policy.json
kill -INT "$(cat serve.pid)"
rivet --file app.rivet serve --listen 127.0.0.1:18931 2> serve.log & echo $! > serve.pid
cat serve.log
```

```text
{"listen_addr":"127.0.0.1:18931","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:6809a1f48a4c2a6aada8d0dd5df34f53134dd936111851ae519237d9b011b701","policy_hash":"sha256:8cd42eb254449e0522b3cdfbb201cae88b3573689ccbb701db9adb9077152d18"}
```

### 5. Verify real decisions

```sh
rivet --endpoint http://127.0.0.1:18931 request notes.create --data '{"text":"hello"}'
rivet --endpoint http://127.0.0.1:18931 request notes.read
rivet --endpoint http://127.0.0.1:18931 request notes.delete; echo "exit=$?"
rivet --endpoint http://127.0.0.1:18931 trace show <request_id of the create>
```

## Expected Results

```text
{"request_id":"req_0110986efd","trace_id":"tr_0110986efd","operation":"notes.create","type":"result","status":"ok","data":{"created":true},"error":null,"effects":"committed","data_count":0}
{"request_id":"req_028e3d45f2","trace_id":"tr_028e3d45f2","operation":"notes.read","type":"result","status":"ok","data":{"text":"hello"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_030cca104f","trace_id":"tr_030cca104f","operation":"notes.delete","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_delete delete on ./out/note.json denied: no grant for allow_delete ./out/note.json","retryable":false,"source":{"file":"app.rivet","line":40,"column":5,"end_line":42,"end_column":8},"operation_id":"notes.delete","details":{"capability":"allow_delete","access":"delete","target":"./out/note.json"}},"effects":"none","data_count":0}
exit=3
{"request_id":"req_04880f3664","trace_id":"tr_04880f3664","operation":"rivet.trace.show","type":"result","status":"ok","data":{"request_id":"req_0110986efd","attempts":[{…,"effect_id":"notes.create#1","operation_id":"notes.create","phase":"decision","capability":"allow_write","access":"create","target":"./out/note.json","decision":"allowed","policy_hash":"sha256:8cd42eb254449e0522b3cdfbb201cae88b3573689ccbb701db9adb9077152d18","source":{"file":"app.rivet","line":9,"column":5},"outcome":{"rule":"grant allow_write ./out/**"}}],"complete":true,"next_cursor":null,"gaps":0},"error":null,"effects":"none","data_count":0}
```

## Verification

| Check | Pass condition |
|---|---|
| Receipt | `policy_hash` equals the candidate `sha256` from step 3 |
| Intended allow | `notes.create` returns `"status":"ok"` with `"effects":"committed"` |
| Intended deny | `notes.delete` → `"status":"error"`, `error.code` `permission.denied`, exit 3, rule `no grant for allow_delete …` |
| Trace | `trace show` names the rule that matched (`grant allow_write ./out/**`) and the new `policy_hash` |
| Offline agrees with live | the decisions in step 2 match what step 5 observed |

## Rollback

```sh
cp policy.json.prev policy.json
kill -INT "$(cat serve.pid)"
rivet --file app.rivet serve --listen 127.0.0.1:18931 2> serve.log & echo $! > serve.pid
rivet --endpoint http://127.0.0.1:18931 request notes.update --data '{"text":"again"}'; echo "exit=$?"
```

Validated output after rollback: the receipt shows the previous `policy_hash`
(`sha256:05c2e270…`), writes are denied again and reads still work:

```text
{"request_id":"req_01d935febd","trace_id":"tr_01d935febd","operation":"notes.update","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_write update on ./out/note.json denied: no grant for allow_write ./out/note.json","retryable":false,"source":{"file":"app.rivet","line":30,"column":5,"end_line":30,"end_column":52},"operation_id":"notes.update","details":{"capability":"allow_write","access":"update","target":"./out/note.json"}},"effects":"none","data_count":0}
exit=3
{"request_id":"req_02586eccfa","trace_id":"tr_02586eccfa","operation":"notes.read","type":"result","status":"ok","data":{"text":"hello"},"error":null,"effects":"none","data_count":0}
```

Rollback does **not** undo effects committed while the new policy was live (the note file created in step 5 stays),
and the in-memory traces from before the restart are gone (`trace show req_0110986efd` → an error envelope with
`not_found.trace`, exit 4).

## Failure Scenarios

| Symptom | Cause | Action |
|---|---|---|
| Step 2 prints e.g. ``error[policy.invalid]: bad.json /grants/0/access/0: `create` does not belong to allow_read (allowed: read, list, stat, watch)`` (exit 2; verified on the RC) | schema error in the candidate | fix the key/verb named by the JSON pointer; repeat step 2 |
| Step 2 shows unexpected `allowed` rows | grant too broad (`**`, `*` host, missing `access`) | narrow targets or add `access` verbs |
| Step 2 still shows the new grant's sites `denied` | candidate staged in another directory: its `./out/**` resolves against that directory (validated: `--policy ../stage/p.json` left `notes.create` denied) | stage next to the live `policy.json` |
| Server exits 2 after install | the installed file differs from the previewed candidate | roll back, re-run steps 2–3 on the exact file |
| Live request still denied after install | server not restarted, or started with a different `--policy` | compare receipt `policy_hash` with step 3; restart |
| Server exits 5 `connection.bind` | old process still holds the port | `kill -INT` and wait, then start |

## Escalation

Operator on duty → Project maintainer. Escalate before rollout for any change that widens network access, adds
`allow_exec`, turns off `deny_private_ranges`, or grants a sensitive built-in; escalate immediately if live
decisions differ from the step 2 preview.

## Related Monitoring

Watch the startup receipt (`policy_hash`), client error rates for `permission.denied` (HTTP 403 / exit 3) after the
restart, and spot-check `trace show` for denied requests. The per-request access log on stderr shows each
request's `operation` and `status` (403 lines are denials at the principal or broker level); `GET /v1/health`
confirms the restarted listener. There is no metrics endpoint; see
[OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md#logs-and-exit-codes). Before rolling out, a
single concrete call can be checked offline with `rivet policy explain ID --params '{…}'` (exit 3 when denied; this
`--params` is `policy explain`'s own flag in 0.2.0, not the deprecated `request --params`). During the rollout also
watch `"deprecated":1` access-log lines: clients still sending 0.1.0 input keep working through 0.2.x
([OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md#monitoring-deprecated-input-020)).

## Last Validation Date

2026-09-29: executed end to end (steps 0–5, the verification table, rollback and the bad-verb failure scenario)
against the 0.2.0 release candidate. The binary was `target/release/rivet`, built with
`cargo build --release --features cli` at main `8031baa`, on macOS arm64, with listener `127.0.0.1:18931` and a
temporary copy of the 02-file-crud bundle. The server was stopped afterwards. The procedure is unchanged; the
outputs are now envelopes.

History: 2026-09-28 — executed end to end (steps 0–5 and rollback) against `target/debug/rivet` 0.1.0-dev (commit `f40d4aa`)
on macOS arm64, listener `127.0.0.1:18481`, using a temporary copy of the 02-file-crud bundle. Its request IDs
are no longer shown. The fix-batch changes that touch this runbook (SIGTERM drain, access log, `/v1/health`,
`policy explain --params`, limit widths) were verified separately at commit `829ca43`; the procedure's commands are
unchanged.

## Related Documents

- [OPS-2026-0001 Operating rivet serve](../operations/ops-2026-0001-operating-rivet-serve.md)
- [RUN-2026-0001 Rotate serve bearer tokens](run-2026-0001-rotate-serve-bearer-tokens.md)
- [REF-2026-0001 Request and evidence](../references/ref-2026-0001-request-and-evidence.md)
- [Demo 02 file CRUD](../demos/02-file-crud/README.md)
- [SYS-2026-0003 Policy broker and I/O manifest](../system/components/sys-2026-0003-policy-broker-and-io-manifest.md) (one policy across modules)
- [API-2026-0006 Envelopes](../api/api-2026-0006-envelopes.md)
- [Runbooks index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial runbook, validated against 0.1.0-dev (f40d4aa). |
| 2 | 2026-09-28 | Claude | Fix batch (829ca43): SIGTERM drains like SIGINT; monitoring uses the access log and `/v1/health`; `policy explain --params` pre-check; limit widths. Procedure unchanged. |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-38/D-48 (TASK-079): re-executed on the 0.2.0-rc; outputs as envelopes (`--data`, `rivet.trace.show`, `rivet.policy.explain --json`), one policy for multi-file bundles, deprecated-input watch, `cli` feature prerequisite, macOS/Linux only. |
