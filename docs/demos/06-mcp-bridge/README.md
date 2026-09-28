---
document_id: DEMO-2026-0006
title: "Bridge a remote MCP connector into local operations"
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

# Bridge a remote MCP connector into local operations

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

Bridge a remote MCP connector into local operations. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `contacts.find` | `object {content, structuredContent?, isError}` | Forward to the pinned `crm.tools.search` import; declares `mcp.tool_failed`. |

```text
  upstream MCP client                 rivet serve (this bundle)                 remote CRM MCP server
  tools/call contacts.find  ──────>   contacts.find                     
                                        (request "crm.tools.search" …) ──────>  tools/call search
                                        needs allow_network + allow_mcp          (effects opaque = unknown)
                            <──────   Completion {result: McpResult}   <──────  result / isError
```

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17) and S52–S54 and S58–S60. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/06-mcp-bridge
```

Provide a controlled MCP server using [schemas/tools-list.fixture.json](schemas/tools-list.fixture.json) and [schemas/search-result.fixture.json](schemas/search-result.fixture.json) as protocol fixtures. They are MCP messages, not Rivet schema snapshots.

The implementation's explicit host bundle-assembly step must capture and review this tool schema and supply `schemas/crm.json`. The proposal has not finalized that on-disk snapshot envelope, so it is deliberately not fabricated here. A snapshot counts as reviewed only when its sha256 is listed in policy.json `"approved": {"snapshots": ["sha256:..."]}`. Add that entry once you have the real file; otherwise loading fails. Compilation never performs implicit discovery.

Two policy files separate normal use from schema refresh:

```text
  policy.json          (auto-discovered)  allow_network https://mcp.example.com:443
                                          allow_mcp     crm/tools/search
  policies/sync.json   (--policy)         allow_network https://mcp.example.com:443
                                          allow_mcp     crm/discover
                                          allow_write   ../schemas/crm.next.json   (relative to policies/)
```

## Steps

### Command / Request

After supplying the reviewed snapshot:

```sh
rivet --file app.rivet request contacts.find --params '{"query":"Ada"}'
rivet --file app.rivet serve --stdio
```

An upstream MCP client can now call the direct tool `contacts.find`; Rivet invokes the downstream CRM tool. To serve MCP over HTTP alongside the other surfaces instead, use `rivet --file app.rivet serve --listen 127.0.0.1:8080` (MCP at `/mcp`).

To propose a schema refresh explicitly, with the sync policy:

```sh
rivet --file app.rivet --policy ./policies/sync.json \
  connectors sync crm --output ./schemas/crm.next.json
```

View the declared output:

```sh
rivet --file app.rivet outputs contacts.find
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

`contacts.find` returns the remote McpResult with `content` and `structuredContent` preserved; `result.structuredContent.contacts` contains Ada (exit 0). A remote `isError: true` result becomes `mcp.tool_failed` (kind `application`, HTTP 502, exit 5). Without the network or MCP grant the call fails with `permission.denied` (exit 3). Refresh writes a candidate snapshot; it does not change the live catalog. Running `connectors sync` without `--policy ./policies/sync.json` is denied (exit 3), because the default policy.json has no `crm/discover` or write grant.

`rivet outputs contacts.find`:

```text
contacts.find — Find contacts
output  object   The remote MCP tool result, preserved as returned.
  content            list json  required  MCP content blocks returned by the remote tool.
  structuredContent  json       optional  Structured result; the fixture returns {contacts:[...]}.
  isError            boolean    required  False on success; a true value becomes mcp.tool_failed instead.
emits    —
receives —
errors
  mcp.tool_failed   The remote tool returned isError: true.
```

`rivet outputs --all --json`:

```json
[
  {"id": "contacts.find",
   "output": {"type": "object", "description": "The remote MCP tool result, preserved as returned.",
              "properties": {"content": {"type": "array", "items": {}, "description": "MCP content blocks returned by the remote tool."},
                             "structuredContent": {"description": "Structured result; the fixture returns {contacts:[...]}."},
                             "isError": {"type": "boolean", "description": "False on success; a true value becomes mcp.tool_failed instead."}},
              "required": ["content", "isError"], "additionalProperties": false},
   "emits": null, "receives": null,
   "errors": [{"code": "mcp.tool_failed", "description": "The remote tool returned isError: true."}]}
]
```

Imported connector tools such as `crm.tools.search` are separate catalog entries. Whether they appear in `outputs --all` depends on the connector's exposure and your authorization.

## Effects and policy

Outbound MCP needs both network authority and the logical tool grant. Serving inbound MCP confers no outbound authority. The remote server's own filesystem and database effects are opaque, and a strict local inventory must mark them unknown.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --strict --include-bootstrap --format json
rivet --file app.rivet io --needs --include-bootstrap
```

`check --strict-docs` passes: the public operation describes its param, output and fields. The body has no `fail`, and `mcp.tool_failed` is declared for documentation.

**`io --by target`**. The call to the imported `crm.tools.search` produces two sites: the HTTP transport connection declared on the connector (line 2) and the logical MCP tool call (line 17):

```text
TARGET                        ACCESS         CAPABILITY      ORIGIN           PHASE     NEEDS FILE   USED BY
https://mcp.example.com:443   connect POST   allow_network   transport http   connect   —            contacts.find (via crm.tools.search)
crm/tools/search              call tool      allow_mcp       request          body      —            contacts.find (via crm.tools.search)
```

**`io --check-policy`** with the auto-discovered [policy.json](policy.json):

```text
OPERATION       KIND      ACCESS         TARGET                        KNOWLEDGE       SOURCE         DECISION
contacts.find   (calls crm.tools.search — connector crm)                               app.rivet:17
contacts.find   network   connect POST   https://mcp.example.com/mcp   exact           app.rivet:2    allowed
contacts.find   mcp       call tool      crm/tools/search              opaque_remote   app.rivet:17   allowed
```

Both sites are allowed, so it exits 0. The tool name is exact, but what the remote server does with it is `opaque_remote`, so the manifest reports `complete: false` and `--strict` exits **7** (inspection incomplete). This is expected and is not a syntax error. Under [policies/sync.json](policies/sync.json), the tool call is `denied` (that file grants only `crm/discover`), and `io --check-policy` exits 3. `connectors sync` is a host command, not an operation, so it is not in the manifest. `--include-bootstrap` lists `./schemas/crm.json` as a descriptor/schema read.

**The connector `schema` file is a bootstrap read, not an operation site.** `schema "./schemas/crm.json"` (app.rivet:3) is a file-valued option at connector level, so it is read when the bundle is assembled. It carries the same site fields as every other site, but it is never granted by policy.json or by `policy generate`:

```text
KEY         SITE                              ORIGIN                     PHASE   REQUIRES_EXISTING   SECRET
bootstrap   file read ./schemas/crm.json      option schema              load    yes                 no
            (app.rivet:3)                     — never granted; a missing file fails the bundle load (not_found, exit 4)
```

`rivet --file app.rivet io --needs --include-bootstrap` lists it under its own heading:

```text
bundle load needs:
  ./schemas/crm.json    (schema)
contacts.find needs no existing files.
```

`io --needs` without `--include-bootstrap` prints only `contacts.find needs no existing files.`: the operation needs no existing files of its own. If the schema file were missing, the bundle would not load at all, so it can never be a per-operation need.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared output and error | `rivet outputs contacts.find` | Table above | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | Inline grants became policy.json + policies/sync.json | Commands above | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Stop your upstream host and the controlled remote fixture. Remove only the generated `crm.next.json` if you no longer need it; keep reviewed schemas.

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
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN, PHASE, NEEDS FILE; connector `schema` shown as a bootstrap `load` read with new fields; `io --needs --include-bootstrap` `bundle load needs` excerpt. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: transport + tool-call sites by target, `io --check-policy` (allowed; denied under policies/sync.json), opaque_remote → `--strict` exit 7. |
| 2 | 2026-09-28 | Claude | UQ-17: prefix `(request …)` call; declared output and `mcp.tool_failed`; `--sandbox` grants moved to policy.json and policies/sync.json; `serve --stdio` replaces `--transport mcp-stdio`; snapshot approval via policy.json; `io --strict` exit 7; View outputs and strict-docs. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
