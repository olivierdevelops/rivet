---
document_id: INC-2026-0008
title: "Defects found by documentation verification"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-28T19:20:00Z
end_time: 2026-09-28T20:10:00Z
root_cause_status: identified
systems: [Rivet]
components: [language, execution, policy, sessions, serve, cli]
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Defect found and fixed during PLAN-2026-0001 P4 (documentation refresh, TASK-059–066); never released.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0001, PROP-2026-0001, INC-2026-0007]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation]
---

# Defects found by documentation verification

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** language, execution, policy, sessions, serve, cli

## Incident Summary

While re-verifying every command and example in the current-state documentation against the build, the documentation writer found seven more defects, fixed here.

| # | Defect | Fix |
|---|---|---|
| 1 | Words after `with … as NAME` were silently dropped, so a trailing `chunk_size 4` did nothing | kept as resource arguments |
| 2 | `policy.invalid` always said `policy.json`, even with `--policy big.json` | names the file actually loaded |
| 3 | `check` warnings rendered as `warning: error[code]` | `warning[code]` |
| 4 | Refusal of a returned secret had no source span | points at the `return` statement |
| 5 | `rivet.trace.export` / `rivet.connectors.sync` reported `effects: "none"` although they create a file | `committed` |
| 6 | A top-level request's error could carry a nested request's id (DAG fail-fast, generic `rivet.request`) | top-level dispatch stamps its own ids |
| 7 | Refused WebSocket input frames had no `request_id` / `trace_id` | the session's ids are attached |

One further finding is not fixed: `io --include-bootstrap` shows `./app.rivet (+ imports)` although 0.1.0 has no import form. The wording is kept as the designed placeholder and is listed as a known issue in the release notes.

```text
 docs claim ──run against build──► mismatch ──► defect? ──yes──► fix + test ──► doc shows real output
```

## Severity

S3: no security impact. Item 4 already refused; only its location was missing.

## Status

Resolved in commit `1f59098`, before any release.

## Discovery Context

Documentation refresh for Fix-A…D (TASK-059–066). Every command was verified against the build.

## Start Time

2026-09-28T19:20:00Z

## End Time

2026-09-28T20:10:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only.

## Affected Components

- language (`lower_with`)
- files (`file_stream` chunk size)
- policy (`load_policy`)
- execution (return guard, top-level ids)
- sessions / serve (WebSocket refusal ids)
- cli (warning label)

## Customer Impact

None, because the defects never shipped.

## Detection

Manual verification of the documentation examples.

## Reproduction Steps

1. `with file open "./data/x.txt" mode read as src chunk_size 4` over a 12-byte file: before the fix it yielded 1 chunk; the expected result is 3.
2. `rivet --file app.rivet --policy big.json check` with an invalid limit: before the fix the message said `policy.json /limits/…`.

## Timeline

```text
2026-09-28T19:20:00Z  documentation writer reports eight findings
2026-09-28T20:10:00Z  seven fixed in 1f59098 (400 tests pass); one kept as a known issue
```

## Logs and Evidence

The regression tests and the fix commit.

## Source Files

- `src/features/language/lowering/lower.rs`, `src/infra/file_stream.rs`
- `src/features/policy/load_policy.rs`
- `src/orchestrator/{setup_cli,builtins,runtime}.rs`
- `src/infra/{execution_driver,session_driver}.rs`, `src/domain/serve.rs`

## Root Cause

Each defect was an unexercised edge of a surface. For example, the `with` lowering assumed nothing follows `as NAME`, and the error-id stamping happened only at the innermost request.

## Contributing Factors

The same as INC-2026-0007: the suites assert behaviour through the library API, not every rendering.

## Resolution

See the table in the Incident Summary.

## Verifying Tests

- `tests/conformance_files.rs::trailing_chunk_size_on_the_with_line_applies`
- `tests/conformance_policy_file.rs::invalid_policy_message_names_the_loaded_file`
- `tests/conformance_errors_limits_dag.rs::check_warnings_for_unguarded_result_and_undeclared_codes`
- The full suite passes: 400 passed, 0 failed.

## Corrective Actions

Fixed before release.

## Preventive Actions

Documentation examples are re-verified against every release build (T-30, DEMO-2026-0015).

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

The `(+ imports)` placeholder wording remains, as noted above.

## Lessons Learned

Verifying documentation against the build keeps finding real defects, so it stays a release step.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [INC-2026-0007](inc-2026-0007-demo-verification-defects.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
