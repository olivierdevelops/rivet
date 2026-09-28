---
document_id: RUN-2026-0002
title: "Roll out a policy.json change to rivet serve"
document_type: runbook
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
related_documents: [OPS-2026-0001, RUN-2026-0001, PLAN-2026-0001, REF-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, runbook, policy, serve, rollout, rollback]
---

# Roll out a policy.json change to rivet serve

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
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

- `rivet` 0.1.0 on the host and the served bundle (entry `.rivet` file) readable.
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
- Stop with SIGINT (`kill -INT`), not SIGTERM.

## Procedure

Validated example: the 02-file-crud bundle served on `127.0.0.1:18481` with `auth none` (loopback). The live policy
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
$ rivet --endpoint http://127.0.0.1:18481 request notes.create --params '{"text":"hello"}'
{"request_id":"req_0272b72452","trace_id":"tr_0272b72452","error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no grant for allow_write ./out/note.json","retryable":false,"effects":"none","source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}}}
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
flip means the edit is wrong — go back to step 1. `--json` gives the same data with the candidate's `sha256`.

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
rivet --file app.rivet serve --listen 127.0.0.1:18481 2> serve.log & echo $! > serve.pid
cat serve.log
```

```text
{"listen_addr":"127.0.0.1:18481","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:6809a1f4…","policy_hash":"sha256:8cd42eb254449e0522b3cdfbb201cae88b3573689ccbb701db9adb9077152d18"}
```

### 5. Verify real decisions

```sh
rivet --endpoint http://127.0.0.1:18481 request notes.create --params '{"text":"hello"}'
rivet --endpoint http://127.0.0.1:18481 request notes.read
rivet --endpoint http://127.0.0.1:18481 request notes.delete; echo "exit=$?"
rivet --endpoint http://127.0.0.1:18481 trace show <request_id of the create>
```

## Expected Results

```text
{"request_id":"req_01e22cbc65","trace_id":"tr_01e22cbc65","result":{"created":true},"data_count":0,"effects":"committed"}
{"request_id":"req_0260474222","trace_id":"tr_0260474222","result":{"text":"hello"},"data_count":0,"effects":"none"}
{"request_id":"req_03e2a6bdff",…,"error":{"kind":"permission","code":"permission.denied","message":"allow_delete delete on ./out/note.json denied: no grant for allow_delete ./out/note.json",…}}
exit=3
{"request_id":"req_01e22cbc65","attempts":[{…,"effect_id":"notes.create#1","operation_id":"notes.create","phase":"decision","capability":"allow_write","access":"create","target":"./out/note.json","decision":"allowed","policy_hash":"sha256:8cd42eb2…","source":null,"outcome":{"rule":"grant allow_write ./out/**"}}],"complete":true,"next_cursor":null,"gaps":0}
```

## Verification

| Check | Pass condition |
|---|---|
| Receipt | `policy_hash` equals the candidate `sha256` from step 3 |
| Intended allow | `notes.create` returns a result with `"effects":"committed"` |
| Intended deny | `notes.delete` → `permission.denied`, exit 3, rule `no grant for allow_delete …` |
| Trace | `trace show` names the rule that matched (`grant allow_write ./out/**`) and the new `policy_hash` |
| Offline agrees with live | the decisions in step 2 match what step 5 observed |

## Rollback

```sh
cp policy.json.prev policy.json
kill -INT "$(cat serve.pid)"
rivet --file app.rivet serve --listen 127.0.0.1:18481 2> serve.log & echo $! > serve.pid
rivet --endpoint http://127.0.0.1:18481 request notes.update --params '{"text":"again"}'; echo "exit=$?"
```

Validated output after rollback: the receipt shows the previous `policy_hash`
(`sha256:0c9b4b04…`), writes are denied again and reads still work:

```text
{"request_id":"req_0103e62f35",…,"error":{"kind":"permission","code":"permission.denied","message":"allow_write update on ./out/note.json denied: no grant for allow_write ./out/note.json",…}}
exit=3
```

Rollback does **not** undo effects committed while the new policy was live (the note file created in step 5 stays),
and the in-memory traces from before the restart are gone (`trace show req_01e22cbc65` → `not_found.trace`, exit 4).

## Failure Scenarios

| Symptom | Cause | Action |
|---|---|---|
| Step 2 prints e.g. `error[policy.invalid]: policy.json /grants/0/access/0: … does not belong to allow_read (allowed: read, list, stat, watch)` (exit 2) | schema error in the candidate | fix the key/verb named by the JSON pointer; repeat step 2 |
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
restart, and spot-check `trace show` for denied requests. Rivet 0.1.0 has no metrics endpoint or access log; see
[OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md#logs-and-exit-codes).

## Last Validation Date

2026-09-28 — executed end to end (steps 0–5 and rollback) against `target/debug/rivet` 0.1.0-dev (commit `f40d4aa`)
on macOS arm64, listener `127.0.0.1:18481`, using a temporary copy of the 02-file-crud bundle. Hashes and request IDs
shown are from that run.

## Related Documents

- [OPS-2026-0001 Operating rivet serve](../operations/ops-2026-0001-operating-rivet-serve.md)
- [RUN-2026-0001 Rotate serve bearer tokens](run-2026-0001-rotate-serve-bearer-tokens.md)
- [REF-2026-0001 Request and evidence](../references/ref-2026-0001-request-and-evidence.md)
- [Demo 02 file CRUD](../demos/02-file-crud/README.md)
- [Runbooks index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial runbook, validated against 0.1.0-dev (f40d4aa). |
