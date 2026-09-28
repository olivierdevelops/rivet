---
document_id: INC-2026-0007
title: "Defects found by release demo verification"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-28T18:10:00Z
end_time: 2026-09-28T19:10:00Z
root_cause_status: identified
systems: [Rivet]
components: [execution, registry, audit, serve, cli, mcp]
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Defect found and fixed during PLAN-2026-0001 P4 (TASK-067 demo verification); never released.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0001, PROP-2026-0001, DEMO-2026-0015]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation]
---

# Defects found by release demo verification

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** execution, registry, audit, serve, cli, mcp

## Incident Summary

Executing the twelve sample READMEs step by step against the release candidate (TASK-067) turned up six defects that the conformance suites did not cover. The most serious was that the `rivet.trace.export` built-in was listed but not dispatched, so `rivet --endpoint URL trace export` always failed with `not_found.operation`.

| # | Defect | Where seen | Fix |
|---|---|---|---|
| 1 | `rivet.trace.export` built-in not dispatched; missing from MCP `tools/list` (as was `rivet.capabilities`) | remote CLI, HTTP, MCP | dispatch arm (`path`, alias `output`); both listed |
| 2 | `file delete … missing ok` on an absent file reported `effects: "committed"` | 02-file-crud | a no-op delete does not commit |
| 3 | `emits`/`receives` descriptions dropped | 04-streaming | kept in the catalog and shown by `outputs`, `describe`, JSON and MCP |
| 4 | `http.status` message dropped an explicit port | 03-http | scheme, host, port and path shown |
| 5 | `io --check-policy` printed `(calls X — see above)` for callees with no rows | 05-dag | `(calls X — no I/O)` |
| 6 | `--input-jsonl` `validation.input` errors had an empty `request_id` | 10-grpc | the cancelled request's ids are attached (local and `--endpoint`) |

```text
 12 sample READMEs ──run literally──► real output ≠ README ──► defect? ──yes──► fix + test ──► README re-run
                                                     │
                                                     └── no ──► README corrected (the code wins)
```

## Severity

S3: functional defects with no security impact; each one failed closed or only affected display.

## Status

Resolved in commit `2a751ab`, before any release.

## Discovery Context

The P4 demo-verification pass (TASK-067, DEMO-2026-0015).

## Start Time

2026-09-28T18:10:00Z

## End Time

2026-09-28T19:10:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only.

## Affected Components

- execution (`execution_driver`: effects status)
- registry / language (item descriptions)
- audit (manifest rendering)
- serve / mcp / cli (built-in dispatch, tool list, JSONL errors)

## Customer Impact

None, because the defects never shipped.

## Detection

A manual end-to-end run of each README against the build.

## Reproduction Steps

1. Start `rivet serve`, run any request, then run `rivet --endpoint URL trace export REQ --output ./t.json`. Before the fix this failed with `not_found.operation`.
2. In `docs/demos/02-file-crud`, run `notes.delete` twice. Before the fix the second run reported `effects: "committed"`.

## Timeline

```text
2026-09-28T18:10:00Z  demo agent reports six findings
2026-09-28T19:10:00Z  all fixed in 2a751ab; 398 tests pass; READMEs 05 and 10 updated
```

## Logs and Evidence

The demo Verification Records and the fix commit.

## Source Files

- `src/orchestrator/builtins.rs`, `src/io/mcp/mod.rs`
- `src/infra/execution_driver.rs`
- `src/domain/{ir,contracts}.rs`, `src/features/language/lowering/lower.rs`, `src/io/cli/mod.rs`
- `src/features/transports/exchange_http.rs`
- `src/features/audit/support/render.rs`
- `src/orchestrator/{setup_cli,remote_cli}.rs`

## Root Cause

Surface wiring and presentation paths had no end-to-end tests. For example, the built-in ID list and the dispatch table were two separate lists that nothing cross-checked.

## Contributing Factors

The conformance suites assert behaviour through the library API, which does not exercise every surface projection.

## Resolution

See the table in the Incident Summary.

## Verifying Tests

- `tests/conformance_audit.rs::trace_export_builtin_dispatches`
- `io::mcp::tests` (20 built-in tools listed)
- Demos 02, 03, 04, 05 and 10 re-run against the fixed build.

## Corrective Actions

Fixed before release.

## Preventive Actions

The demos stay in T-30, and every release re-runs them (DEMO-2026-0015).

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

Other surface projections may still lack end-to-end coverage, which the per-release demo run mitigates.

## Lessons Learned

Executing documentation literally is an effective integration test.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [DEMO-2026-0015](../../demos/demo-2026-0015-v0-1-0-release-verification.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
