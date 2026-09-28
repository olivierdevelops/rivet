---
document_id: DEMO-2026-0006
title: "Bridge a remote MCP connector into local operations"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [connectors, registry, policy, mcp, cli, serve]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable MCP bridge demo — the unreviewed bundle refusing to load, `connectors sync`, review and sha256 approval of the snapshot, a local operation that calls the imported remote tool, `mcp.tool_failed`, `mcp.schema_drift`, the opaque-remote manifest and re-export over incoming MCP, against a shipped stdlib MCP Streamable HTTP fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-06 asks for an MCP bridge; UQ-17 adds declared outputs and policy.json-only policy; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0008, API-2026-0003, TEST-2026-0006, TEST-2026-0018, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, demo, mcp, connectors, snapshot]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# Bridge a remote MCP connector into local operations

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** connectors, registry, policy, mcp, cli, serve

## Purpose

Bridge a remote MCP connector into local operations. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `contacts.find` | `object {content, structuredContent?, isError}` | Forward to the imported `crm.tools.search`; declares `mcp.tool_failed`. |
| `crm.tools.search` | McpResult (from the snapshot) | Imported connector tool (`expose tools ["search"]`); a separate catalog entry. |

```text
  upstream MCP client                 rivet serve (this bundle)                 remote CRM MCP server
  tools/call contacts.find  ──────>   contacts.find
                                        (request "crm.tools.search" …) ──────>  initialize, tools/list (drift check),
                                        needs allow_network + allow_mcp          tools/call search
                            <──────   Completion {result: McpResult}   <──────  result / isError
                                        effects "unknown" (remote side is opaque)
```

The connector's tool schemas come from a **reviewed snapshot**, never from discovery at compile time:

```text
  connectors sync ──> schemas/crm.next.json ──review──> schemas/crm.json ──sha256──> policy.json
  (sync policy:        (candidate, never         (mv)       (connector        "approved": {"snapshots": [...]}
   crm/discover +       overwrites)                          `schema` option)
   allow_write)
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request, trace and MCP session IDs, hashes and ports vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0-dev until the P5 bump
```

- `python3` (standard library only) for [fixtures/crm_mcp.py](fixtures/crm_mcp.py), an MCP Streamable HTTP server that replays [schemas/tools-list.fixture.json](schemas/tools-list.fixture.json) and [schemas/search-result.fixture.json](schemas/search-result.fixture.json) (query `Ada` → one contact; any other query → `isError: true`). `--drift` serves a changed `search` input schema.
- `curl` for step 8. Loopback ports 18860 (fixture) and 18861 (serve) free.

## Setup

```sh
cd docs/demos/06-mcp-bridge
```

No reviewed snapshot ships with this folder: `schemas/crm.json` is created by `connectors sync`, reviewed, and approved by its sha256 in policy.json (steps 2–3). The two policy files separate normal use from schema refresh:

```text
  policy.json          (auto-discovered)  allow_network https://mcp.example.com:443
                                          allow_mcp     crm/tools/search
  policies/sync.json   (--policy)         allow_network https://mcp.example.com:443
                                          allow_mcp     crm/discover
                                          allow_write   ../schemas/crm.next.json   (relative to policies/)
```

`https://mcp.example.com/mcp` is a placeholder; steps 2–8 run against the fixture in a scratch copy.

## Steps

### 1. The unreviewed bundle does not load

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
```

#### Expected Output / Response

Compilation never performs discovery, so without a reviewed snapshot the bundle does not load (exit 4). `outputs`, `io` and `request` fail the same way:

```text
error[not_found.mcp_snapshot]: cannot read snapshot ./schemas/crm.json of connector `crm`: No such file or directory (os error 2)
  --> app.rivet:1:1
   |
  1| connector crm mcp
   | ^
  = hint: create it with `rivet connectors sync crm --output ./schemas/crm.json`, review it and approve its sha256 in policy.json
```

### 2. Local fixture run: sync a candidate snapshot

#### Command / Request

```sh
WORK="$(mktemp -d)"; cp -R app.rivet policy.json policies schemas fixtures "$WORK/"; cd "$WORK"
sed -i.bak -e 's#https://mcp\.example\.com/mcp#http://127.0.0.1:18860/mcp#' app.rivet
sed -i.bak -e 's#https://mcp\.example\.com:443#http://127.0.0.1:18860#' policy.json policies/sync.json
python3 fixtures/crm_mcp.py 18860 2>fixture.log & FX=$!
rivet --file app.rivet connectors sync crm --output ./schemas/crm.next.json
rivet --file app.rivet --policy ./policies/sync.json connectors sync crm --output ./schemas/crm.next.json
rivet --file app.rivet --policy ./policies/sync.json connectors sync crm --output ./schemas/crm.next.json
```

#### Expected Output / Response

Under the default policy there is no discover grant, and nothing is contacted (exit 3):

```json
{"request_id":"","trace_id":"","error":{"kind":"permission","code":"permission.denied","message":"allow_mcp call crm/discover denied: no grant for allow_mcp crm/discover","retryable":false,"effects":"none","details":{"capability":"allow_mcp","access":"call","target":"crm/discover"}}}
```

Under sync.json it writes the candidate (exit 0; the second line is on stderr):

```text
{"connector":"crm","path":"./schemas/crm.next.json","sha256":"sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805","protocolVersion":"2025-11-25","tools":["search"],"resources":[],"prompts":[]}
wrote candidate snapshot ./schemas/crm.next.json (sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805); after review, approve it in policy.json: "approved": {"snapshots": ["sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805"]}
```

Sync never overwrites (exit 4):

```json
{"request_id":"","trace_id":"","error":{"kind":"conflict","code":"conflict.already_exists","message":"./schemas/crm.next.json already exists; connectors sync never overwrites a snapshot","retryable":false,"effects":"none","details":{"path":"./schemas/crm.next.json"}}}
```

The candidate holds `"format": "rivet.mcp.snapshot/1"`, the server's `protocolVersion` and `serverInfo`, and the `search` tool with its `inputSchema` and `outputSchema`. Its hash does not depend on the endpoint URL. `fixture.log` shows `MCP initialize`, `MCP notifications/initialized`, `MCP tools/list`, `MCP DELETE (session end)`.

### 3. Review, promote and approve

#### Command / Request

```sh
mv schemas/crm.next.json schemas/crm.json
rivet --file app.rivet check
python3 - <<'PY'
import hashlib, json
digest = "sha256:" + hashlib.sha256(open("schemas/crm.json", "rb").read()).hexdigest()
for f in ("policy.json", "policies/sync.json"):
    p = json.load(open(f)); p["approved"] = {"snapshots": [digest]}
    open(f, "w").write(json.dumps(p, indent=2) + "\n")
print("approved", digest)
PY
rivet --file app.rivet check --strict-docs
rivet --file app.rivet list
rivet --file app.rivet outputs contacts.find
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

A present but unapproved snapshot is refused (exit 2):

```text
error[mcp.snapshot_unapproved]: snapshot ./schemas/crm.json of connector `crm` is not reviewed: sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805 is not listed in policy.json approved.snapshots
  --> app.rivet:1:1
   |
  1| connector crm mcp
   | ^
  = hint: after reviewing ./schemas/crm.json, add "sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805" to policy.json "approved": {"snapshots": [...]}
```

After approval (in both files, so the bundle also loads under the sync policy), everything exits 0:

```text
approved sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805
ok: 1 operations, 1 connectors, 0 auth profiles
```

```text
ID                NAME           DESCRIPTION
contacts.find     Find contacts  Call a reviewed remote MCP search tool through the shared registry.
crm.tools.search  search         Search the controlled contact fixture.
```

```text
contacts.find — Find contacts
output  object   The remote MCP tool result, preserved as returned.
  content            list json required  MCP content blocks returned by the remote tool.
  structuredContent  json     optional  Structured result; the fixture returns {contacts:[...]}.
  isError            boolean  required  False on success; a true value becomes mcp.tool_failed instead.
emits    —
receives —
errors
  mcp.tool_failed   The remote tool returned isError: true.
```

`outputs --all --json` has two entries. The imported tool's output comes from the snapshot's `outputSchema`:

```json
{"id":"crm.tools.search","output":{"type":"object","description":"MCP tool result: content blocks and structuredContent, unchanged.","properties":{"content":{"type":"array"},"structuredContent":{"type":"object","properties":{"contacts":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"name":{"type":"string"}},"required":["id","name"],"additionalProperties":false}}},"required":["contacts"],"additionalProperties":false},"isError":{"type":"boolean"}},"required":["content","isError"],"additionalProperties":true},"emits":null,"receives":null,"errors":[]}
```

### 4. I/O manifest: opaque remote effects

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --strict
rivet --file app.rivet --policy ./policies/sync.json io --check-policy
rivet --file app.rivet io --check-files
rivet --file app.rivet io --needs --include-bootstrap
```

#### Expected Output / Response

Two sites: the connector's HTTP transport (line 2) and the logical tool call (line 17):

```text
TARGET                  ACCESS        CAPABILITY     ORIGIN          PHASE    NEEDS FILE  USED BY
http://127.0.0.1:18860  connect POST  allow_network  transport http  connect  —           contacts.find (via crm.tools.search)
crm/tools/search        call tool     allow_mcp      request         body     —           contacts.find (via crm.tools.search)
```

`--check-policy` (exit 0):

```text
OPERATION      KIND     ACCESS        TARGET                      KNOWLEDGE      SOURCE        DECISION
contacts.find  (calls crm.tools.search — connector crm)                          app.rivet:17
contacts.find  network  connect POST  http://127.0.0.1:18860/mcp  exact          app.rivet:2   allowed
contacts.find  mcp      call tool     crm/tools/search            opaque_remote  app.rivet:17  allowed
2 allowed
```

`--strict` exits **7** (inspection incomplete) because what the remote server does is `opaque_remote`; this is expected:

```text
io: complete=false — 1 of 2 sites is dynamic/opaque
  contacts.find#2  mcp call tool  crm/tools/search (opaque_remote)  (app.rivet:17)
```

Under policies/sync.json the tool call is `denied` (`1 allowed · 1 denied`, exit 3): that file grants only `crm/discover`. `io --check-files` prints `contacts.find needs no existing files.` and `0 files` (exit 0). The snapshot is a bootstrap read, never granted by policy.json (exit 0):

```text
bundle load needs:
  ./schemas/crm.json  (schema)
contacts.find needs no existing files.
```

### 5. Explain the policy decision

#### Command / Request

```sh
rivet --file app.rivet policy explain contacts.find --params '{"query":"Ada"}'
```

#### Expected Output / Response

```text
policy   ./policy.json (sha256:e23300ef25c0d491249c44be73aa78bd78e449058adc95beeb070319f9f58475)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_network http://127.0.0.1:18860
grant    allow_mcp crm/tools/search

OPERATION      KIND     ACCESS        TARGET                      KNOWLEDGE      SOURCE        DECISION
contacts.find  (calls crm.tools.search — connector crm)                          app.rivet:17
contacts.find  network  connect POST  http://127.0.0.1:18860/mcp  exact          app.rivet:2   allowed
contacts.find  mcp      call tool     crm/tools/search            opaque_remote  app.rivet:17  allowed
```

Exit 0.

### 6. Call the bridged tool

#### Command / Request

```sh
rivet --file app.rivet request contacts.find --params '{"query":"Ada"}'
rivet --file app.rivet request crm.tools.search --params '{"query":"Ada"}'
rivet --file app.rivet request contacts.find --params '{"query":"Bob"}'
```

#### Expected Output / Response

The McpResult is preserved; `effects` is `unknown` because the remote side is opaque (exit 0). The imported tool called directly returns the same `result`:

```json
{"request_id":"req_01ba199a0d","trace_id":"tr_01ba199a0d","result":{"content":[{"type":"text","text":"{\"contacts\":[{\"id\":\"42\",\"name\":\"Ada\"}]}"}],"structuredContent":{"contacts":[{"id":"42","name":"Ada"}]},"isError":false},"data_count":0,"effects":"unknown"}
```

A remote `isError: true` becomes `mcp.tool_failed` (kind `application`, exit 5). The nested request ID carries a `.1` suffix:

```json
{"request_id":"req_01b996604d.1","trace_id":"tr_01b996604d","error":{"kind":"application","code":"mcp.tool_failed","message":"MCP tool `crm.tools.search` returned isError: true","retryable":false,"effects":"unknown","source":{"file":"app.rivet","line":17,"column":13,"end_line":17,"end_column":20},"operation_id":"crm.tools.search","details":{"tool":"search","content":[{"type":"text","text":"no match"}]}}}
```

Each call logs `initialize`, `notifications/initialized`, `tools/list` (the drift check), `tools/call`, `DELETE (session end)` in `fixture.log`.

### 7. Schema drift is refused before the call

#### Command / Request

```sh
kill $FX; python3 fixtures/crm_mcp.py 18860 --drift 2>>fixture.log & FX=$!
rivet --file app.rivet request contacts.find --params '{"query":"Ada"}'
```

#### Expected Output / Response

The live `tools/list` no longer matches the approved snapshot, so no `tools/call` is sent (exit 5):

```json
{"request_id":"req_017a59d985.1","trace_id":"tr_017a59d985","error":{"kind":"protocol","code":"mcp.schema_drift","message":"connector `crm`: the live server no longer matches the approved snapshot (search: inputSchema changed); run `rivet connectors sync` and review the new snapshot","retryable":false,"effects":"none","source":{"file":"app.rivet","line":17,"column":13,"end_line":17,"end_column":20},"operation_id":"crm.tools.search","details":{"connector":"crm","tools":[{"name":"search","difference":"inputSchema changed"}]}}}
```

The drift session in `fixture.log` ends after `MCP tools/list` with `MCP DELETE (session end)`.

### 8. Re-export over incoming MCP

#### Command / Request

```sh
kill $FX; python3 fixtures/crm_mcp.py 18860 2>>fixture.log & FX=$!
rivet --file app.rivet serve --listen 127.0.0.1:18861 2>serve.err & SV=$!
curl -si http://127.0.0.1:18861/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  --data-binary @"$OLDPWD/../01-catalog/requests/initialize.mcp.json"        # note the mcp-session-id header
SID='returned-session-id'
H=(-H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' -H "MCP-Session-Id: $SID")
curl -s http://127.0.0.1:18861/mcp "${H[@]}" -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'
curl -s http://127.0.0.1:18861/mcp "${H[@]}" -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
curl -s http://127.0.0.1:18861/mcp "${H[@]}" -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"contacts.find","arguments":{"query":"Ada"}}}'
curl -s http://127.0.0.1:18861/mcp "${H[@]}" -d '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"contacts.find","arguments":{"query":"Bob"}}}'
curl -s -o /dev/null -w '%{http_code}\n' -X DELETE http://127.0.0.1:18861/mcp -H "MCP-Session-Id: $SID"
kill $SV $FX
```

#### Expected Output / Response

`tools/list` names `contacts.find` and `crm.tools.search` first, then the `rivet.*` built-ins (`rivet.request` … `rivet.auth.cancel`). The success call (abbreviated) returns the Completion in `structuredContent` with `"isError":false`:

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"…"}],"structuredContent":{"request_id":"req_01fb04fc8d","trace_id":"tr_01fb04fc8d","result":{"content":[{"type":"text","text":"{\"contacts\":[{\"id\":\"42\",\"name\":\"Ada\"}]}"}],"structuredContent":{"contacts":[{"id":"42","name":"Ada"}]},"isError":false},"data_count":0,"effects":"unknown"},"isError":false}}
```

The failure is an MCP tool error (`"isError":true`) whose `structuredContent` is the `mcp.tool_failed` error envelope:

```json
{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"…"}],"structuredContent":{"request_id":"req_027b5cecea.2","trace_id":"tr_027b5cecea","error":{"kind":"application","code":"mcp.tool_failed", …,"details":{"tool":"search","content":[{"type":"text","text":"no match"}]}}},"isError":true}}
```

`DELETE /mcp` returns `204`.

## Effects and policy

```text
  outcome table (verified in steps 1–8)
  ┌──────────────────────────────────────────┬─────────────────────────┬──────┬─────────┐
  │ situation                                │ code                    │ exit │ effects │
  ├──────────────────────────────────────────┼─────────────────────────┼──────┼─────────┤
  │ tool result                              │ —                       │  0   │ unknown │
  │ remote isError: true                     │ mcp.tool_failed         │  5   │ unknown │
  │ live tools/list differs from snapshot    │ mcp.schema_drift        │  5   │ none    │
  │ snapshot file missing                    │ not_found.mcp_snapshot  │  4   │ —       │
  │ snapshot sha256 not approved             │ mcp.snapshot_unapproved │  2   │ —       │
  │ sync without crm/discover grant          │ permission.denied       │  3   │ none    │
  │ sync onto an existing file               │ conflict.already_exists │  4   │ none    │
  └──────────────────────────────────────────┴─────────────────────────┴──────┴─────────┘
```

Outbound MCP needs both network authority and the logical tool grant. Serving inbound MCP confers no outbound authority. The remote server's own effects are opaque, so the manifest marks them `opaque_remote` and results report `effects: "unknown"`.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-07 | UQ-06 / R7 | MCP client connector over Streamable HTTP with sync, reviewed snapshots and a drift check | Steps 1–3, 6–7 | Load refused until approved; Ada found; `mcp.tool_failed`; `mcp.schema_drift` | This README steps 1–3, 6–7 (2026-09-28, 829ca43); TEST-2026-0006 |
| U-21 | UQ-15 / R21 | Incoming MCP re-exports local and imported tools | Step 8 | `contacts.find` listed and callable; `isError` mapping | This README step 8; TEST-2026-0018 |
| U-26 | UQ-18 / R26 | Manifest marks remote effects `opaque_remote`; snapshot is a bootstrap read | Step 4 | `--strict` exit 7; `bundle load needs` lists `./schemas/crm.json` | This README step 4; TEST-2026-0025 |

## Cleanup

```sh
kill $SV $FX 2>/dev/null
cd - && rm -rf "$WORK"
```

Nothing is written to this folder; the candidate and the approved snapshot exist only in the scratch copy.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs` in this folder → `not_found.mcp_snapshot` (exit 4, by design) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `connectors sync` denied, written, never overwritten | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. `mcp.snapshot_unapproved`; approval; `check --strict-docs`, `list`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. `io --by target`, `io --check-policy` (0; 3 under sync.json), `--strict` (7), `--check-files` (0), bootstrap needs | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. `policy explain` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. `contacts.find` Ada, direct `crm.tools.search`, `mcp.tool_failed` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. `mcp.schema_drift` with `--drift` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 8. Incoming MCP: tools/list, success, tool error, DELETE 204 | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder or the scratch copy and the output pasted from that run.

## Known Caveats

- The folder deliberately ships without `schemas/crm.json`; `tests/conformance_mcp.rs` asserts that the unmodified bundle refuses to load. `check`, `io --check-policy` and `io --check-files` therefore exit 4 in this folder and pass in the approved scratch copy (steps 3–4).
- The snapshot here is synced from the shipped fixture, not a real CRM. For a real server: replace the endpoint and grants, sync, review, approve the new sha256.
- The stdio MCP client transport (`transport command …`) is not exercised here; it is covered by TEST-2026-0006.
- Nested request IDs carry a suffix (`req_….1`) that identifies the inner call.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Protocols and connectors manual](../../manuals/man-2026-0008-protocols-and-connectors.md) · [MCP server tools API](../../api/api-2026-0003-mcp-server-tools.md)
- [MCP client tests TEST-2026-0006](../../testing/test-2026-0006-mcp.md) · [MCP catalog tests TEST-2026-0018](../../testing/test-2026-0018-mcp-catalog.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/crm_mcp.py (port 18860) and a scratch-copy run; the snapshot format is now real (`rivet.mcp.snapshot/1`), created by `connectors sync`, promoted and approved in the walkthrough; executed every step against 0.1.0-dev (829ca43) and pasted real output: `not_found.mcp_snapshot`, sync (denied / written / never overwritten), `mcp.snapshot_unapproved`, check/list/outputs (imported tool listed), manifest, `policy explain`, success, `mcp.tool_failed`, `mcp.schema_drift`, incoming MCP; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN, PHASE, NEEDS FILE; connector `schema` shown as a bootstrap `load` read with new fields; `io --needs --include-bootstrap` `bundle load needs` excerpt. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: transport + tool-call sites by target, `io --check-policy` (allowed; denied under policies/sync.json), opaque_remote → `--strict` exit 7. |
| 2 | 2026-09-28 | Claude | UQ-17: prefix `(request …)` call; declared output and `mcp.tool_failed`; `--sandbox` grants moved to policy.json and policies/sync.json; `serve --stdio` replaces `--transport mcp-stdio`; snapshot approval via policy.json; `io --strict` exit 7; View outputs and strict-docs. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
