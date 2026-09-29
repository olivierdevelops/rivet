---
document_id: INC-2026-0013
title: "rivet request --input-jsonl - waited for stdin EOF after the request ended"
document_type: incident
status: resolved
created_date: 2026-09-30
last_updated: 2026-09-30
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-30T06:10:00Z
end_time: 2026-09-30T07:10:00Z
root_cause_status: identified
systems: [Rivet]
components: [cli]
affected_versions:
  from: "0.1.0"
  to: "0.2.0"
confidentiality: internal
scope: CLI shutdown defect exposed by the v0.2.0 tag CI run; fixed on main (dd5e5ad) and shipped in v0.2.1.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0002, REL-0.2.0, REL-0.2.1, INC-2026-0012]
supersedes: null
superseded_by: null
tags: [rivet, incident, cli, tokio, stdin, ci]
---

# rivet request --input-jsonl - waited for stdin EOF after the request ended

> **Status:** Resolved (fixed in `dd5e5ad`; shipped in v0.2.1)
> **Created:** 2026-09-30
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 → 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** cli

## Incident Summary

`rivet request … --input-jsonl - --stream`, both local and with `--endpoint`, did not exit when the request ended
while stdin was still open. It kept running until stdin reached EOF.

The CLI builds a tokio runtime, and dropping that runtime waits for its blocking tasks. The stdin reader
(`tokio::io::stdin`) runs a blocking `read` that returns only at EOF. The request had already finished, but the
process stayed alive inside `Runtime::drop`.

On the v0.2.0 tag CI run (36635295229), this surfaced as a 10-second timeout in `conformance_grpc`
`cli_live_input_local_and_remote` on macOS. The same commit passed on the `main` run (36635293035).
INC-2026-0012 had listed the remote variant as a known limitation ("exits only when stdin closes").

```text
 request ends ─▶ run() returns exit code ─▶ drop(Runtime) ─▶ waits for blocking stdin read ─▶ … until EOF   ✗
 fix:        ─▶ run() returns exit code ─▶ rt.shutdown_background() ─▶ process exits                       ✓
```

## Severity

S3: a CLI hang for callers that keep stdin open. There was no data loss and no security impact.

## Status

Resolved in `dd5e5ad`. The tag `v0.2.0` (`21bb2e9`) contains the defect, and v0.2.1 ships the fix.

## Discovery Context

The v0.2.0 release (PLAN-2026-0002 P5): a tag CI run failure read through the `ci_step.py` annotations (TRBL-2026-0007).

## Start Time

2026-09-30T06:10:00Z

## End Time

2026-09-30T07:10:00Z

## Affected Systems

The Rivet CLI.

## Affected Versions

0.1.0 and 0.2.0.

## Affected Components

- cli (`src/orchestrator/setup_cli.rs`: `main`, `run_duplex`)

## Customer Impact

A host that feeds `--input-jsonl -` from a long-lived pipe saw the CLI linger after the request ended.

## Detection

A CI test timeout on the tag run. It did not reproduce locally in 25 isolated runs or in 10 loaded runs. The root
cause was then found by reading the code and confirmed by experiment.

## Reproduction Steps

1. Use an operation that receives text and returns after two items.
2. Run `rivet --file app.rivet request echo.two --input-jsonl - --stream`, write `"a"` and `"b"`, and keep stdin
   open.
3. Before the fix: no exit within 8 s, both locally and with `--endpoint`. After the fix: exit 0 at once.

## Timeline

```text
2026-09-30T06:10Z  v0.2.0 tag CI: macOS test job fails (cli_live_input_local_and_remote timeout)
2026-09-30T06:30Z  not reproducible in 25 isolated + 10 loaded runs; code reading finds the runtime-drop wait
2026-09-30T06:55Z  confirmed: without the fix local and remote hang until EOF; with it both exit at once
2026-09-30T07:10Z  fix dd5e5ad with regression tests; 495 tests pass
```

## Logs and Evidence

- CI runs 36635295229 (the tag; macOS failed) and 36635293035 (`main`, the same commit, green).
- The manual reproduction with and without the fix, above.

## Source Files

- `src/orchestrator/setup_cli.rs`

## Root Cause

`tokio::runtime::Runtime` waits on drop for blocking-pool tasks. The `--input-jsonl -` feeder reads stdin
through tokio's blocking stdin, so the process could not exit until the read returned at EOF.

## Contributing Factors

The failing test kept stdin open on purpose (it was checking cancellation). The failure depended on timing, which
made it look flaky.

## Resolution

- `main` now calls `rt.shutdown_background()` after `run()` returns, so the process does not wait for blocking tasks.
- After a rejected input line, `run_duplex` keeps re-sending the cancel every 50 ms until the run ends. This covers
  a line rejected before the session has registered its request.

## Verifying Tests

- `tests/conformance_streams.rs::input_jsonl_exits_when_the_operation_ends_without_eof`: deterministic. It fails
  without the fix (10 s timeout) and passes with it.
- `tests/conformance_streams.rs::rejected_input_line_always_cancels_the_local_request`: 20 rounds.
- `tests/conformance_grpc.rs::cli_live_input_local_and_remote`.
- The full suite passes: 495 tests.

## Corrective Actions

Patch release v0.2.1.

## Preventive Actions

The deterministic exit test guards the shutdown path.

## Owners

Implementer (fix); project maintainer (release decision: cut v0.2.1).

## Remaining Risks

None known.

## Lessons Learned

Blocking stdin readers need an explicit, non-waiting runtime shutdown in CLIs.

## Related Documents

- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)
- [INC-2026-0012](inc-2026-0012-documentation-and-demo-verification-defects.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-30 | Claude | Recorded and resolved (dd5e5ad); ships in v0.2.1. |
