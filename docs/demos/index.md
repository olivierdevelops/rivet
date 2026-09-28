---
document_id: REF-2026-0010
title: "Rivet demos index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [registry]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Navigation and status for the sample folders and the v0.1.0 release verification guide.
reason: Every documentation directory needs an index.md; the walkthroughs live in README.md.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: ["DEMO-2026-0013", "DEMO-2026-0015", "PLAN-2026-0001", "PROP-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, examples, design]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-27
---

# Rivet demos index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** registry

This directory holds `.rivet` bundles, `policy.json` files, fixtures, request bodies and one usage README per folder. They illustrate [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md). The new [13-real-world-apis](13-real-world-apis/README.md) cookbook uses public APIs where possible and labels credentialed, local and private-gateway setup explicitly. The walkthroughs, commands and expected results are in [README.md](README.md); this page gives status, reading order and open work only.

## What belongs here

```text
docs/demos/
├── index.md          this page: status, reading order, open work
├── README.md         DEMO-2026-0013: walkthrough table, conventions, shortest usage
├── manifest.json     inventory of entry operations (documentation, not a loader format)
├── demo-2026-0015-v0-1-0-release-verification.md   release verification guide (U-01…U-26)
└── NN-topic/         one independent sample
    ├── app.rivet     full proposed bundle
    ├── policy.json   policy for that sample (absent = deny-by-default)
    ├── README.md     setup, exact commands, expected results, failures, cleanup
    └── data/ requests/ schemas/ policies/   supporting inputs where used
```

Belongs here: sample bundles and their inputs. Does not belong here: design decisions (proposals), syntax reference (REF-2026-0002) or anything claimed as executed. Folders are named `NN-topic` in reading order; each folder README carries its own `DEMO-2026-NNNN` ID.

## Recommended reading order

```text
  01-catalog ──> 12-library ──> 11-sandbox ──> 02-file-crud ──> 03-http ──> 04-streaming ──> 05-dag
  (one file,      (same catalog   (policy.json,    (file CRUD:
   every surface)  from Rust)      I/O manifest,    create/update/
                                   io --needs,      delete verbs)
                                   policy generate)
        │
        └──> protocol samples: 06-mcp-bridge, 07-oauth2, 08-udp, 09-quic, 10-grpc
```

1. [01-catalog](01-catalog/README.md) — one file, several described operations, every access point, including `rivet serve`.
2. [12-library](12-library/README.md) — the same catalog embedded as a Rust library.
3. [11-sandbox](11-sandbox/README.md) — `policy.json`, deny-by-default, the generated I/O manifest (`rivet io`), the files each operation needs (`io --needs`, `io --check-files`) and `rivet policy generate`.
4. [02-file-crud](02-file-crud/README.md), [03-http](03-http/README.md), [04-streaming](04-streaming/README.md), [05-dag](05-dag/README.md).
5. Protocol samples: [06-mcp-bridge](06-mcp-bridge/README.md), [07-oauth2](07-oauth2/README.md), [08-udp](08-udp/README.md), [09-quic](09-quic/README.md), [10-grpc](10-grpc/README.md).
6. [Release verification guide DEMO-2026-0015](demo-2026-0015-v0-1-0-release-verification.md) — one row per released update (U-01…U-26), success and failure per surface.
7. [13-real-world-apis](13-real-world-apis/README.md) — public APIs, OpenAI, Ollama, WebSocket, MCP and private transport gateways.

## Status

| Item | State |
|---|---|
| Samples | Thirteen folders. 01–12 are **active**, executed step by step against 0.1.0 (release candidate 829ca43, 2026-09-28, macOS arm64) with local fixtures; each README has a Verification Record and `manifest.json` marks them `runtime_verified: true`. 13-real-world-apis is a separately owned draft |
| Release guide | [DEMO-2026-0015](demo-2026-0015-v0-1-0-release-verification.md), active; tag and commit are filled at P5 |
| Design revision | Updated to proposal revision 8: prefix calls, declared outputs, `policy.json` only (the `--sandbox` flag was removed), one `rivet serve` for every surface, and the generated I/O manifest (every README shows `io --by target` with ORIGIN, PHASE and NEEDS FILE columns, and `io --check-policy`, for its own app.rivet). Revision 8 (TASK-005, ADR-0001) adds option-derived file sites (`tls ca_file`/`cert_file`/`key_file`, `body file`), `io --needs` and `io --check-files`; connector `descriptor`/`schema` files (10-grpc, 06-mcp-bridge) stay bootstrap reads |
| Recently added | Release verification guide DEMO-2026-0015; local fixtures under `fixtures/` in 01, 03, 04, 06, 07, 08, 09, 10 (TASK-067, 2026-09-28); earlier:  `io --needs` / `io --check-files` walkthrough in 11-sandbox and `bundle load needs` examples in 06-mcp-bridge and 10-grpc; this index; `11-sandbox/policies/create-only.json` (`access` narrowing) and the `policy generate` comparison (2026-09-28) |
| Deprecated / superseded / archived | None |

## Open work and risks

- **I/O manifest tables are generated output.** Since TASK-067 every README's `io` tables are pasted from `rivet io` runs; `tests/conformance_io_manifest.rs` pins the rows of 02, 03 and 11.
- **Platform coverage.** Only macOS arm64 was available for TASK-067; Linux (Landlock/seccomp sandbox) verification is open.
- **Release artifact.** Rerun DEMO-2026-0015 against the tagged v0.1.0 artifact (TASK-081).

- **Capy parser:** every `.rivet` file here compiles (`tests/conformance_samples.rs`), along with every numbered example in REF-2026-0002.
- **G-LIC — Capy licence:** closed 2026-09-28. The maintainer owns Capy and authorized its use in Rivet ([ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md)).

## Related

- [Sample walkthroughs (README.md)](README.md) and [manifest.json](manifest.json)
- [Release verification guide (DEMO-2026-0015)](demo-2026-0015-v0-1-0-release-verification.md)
- [Numbered usage examples](../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [Documentation current state](../README.md) and [navigation](../index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | TASK-067/068: folders 01–12 verified against 0.1.0 (829ca43) and active; added the release verification guide DEMO-2026-0015 to the layout, reading order and status; open work now lists platform coverage and the release-artifact rerun; §7 header. |
| 3 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): status now proposal revision 8 (option-derived file sites, `io --needs`, `io --check-files`); reading order and recently added updated; G-LIC marked closed. |
| 2 | 2026-09-28 | Claude | UQ-18/R26: design revision 6; reading order and status mention the I/O manifest, `policy generate` and policies/create-only.json; open-work note that manifest tables are hand-derived. |
| 1 | 2026-09-28 | Claude | Created the required directory index: purpose, reading order, status and approval gates for the sample folders. |
