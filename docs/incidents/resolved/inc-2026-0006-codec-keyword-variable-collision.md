---
document_id: INC-2026-0006
title: "Codec keywords replaced by same-named variables at run time"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-28T17:20:00Z
end_time: 2026-09-28T17:50:00Z
root_cause_status: identified
systems: [Rivet]
components: [execution, execution_driver]
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Defect found and fixed during PLAN-2026-0001 P4 (README quickstart verification); never released.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0001, PROP-2026-0001, INC-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation]
---

# Codec keywords replaced by same-named variables at run time

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** execution, execution_driver

## Incident Summary

When a variable in scope had the same name as a codec or format keyword (`text`, `json`, `bytes`, `lines`, …), the interpreter replaced the keyword with the variable's value while evaluating effect arguments. The effect then failed at run time, although `rivet check` passed. `param text text required` is a very common declaration.

```text
 param text text required
 file append "./out/log.txt" text "${text}\n"
                             ▲
 before  eval_arg("text") → Value("hello")  → file.form "unexpected argument"   ✗
 after   keyword slot     → Word("text")    → codec text, content "hello\n"      ✓
```

The same collision broke `decode text` in `http` forms (`validation.http_option`) and `conn.send text text` on sockets (`validation.socket_send`).

## Severity

S3: a functional defect with no security impact. The request failed closed with `effects: none`.

## Status

Resolved in commit `79e6318`, before any release.

## Discovery Context

The coordinator found it while verifying the README quickstart for TASK-069 (`param text text` together with `file append … text "${text}\n"`).

## Start Time

2026-09-28T17:20:00Z

## End Time

2026-09-28T17:50:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only.

## Affected Components

- execution
- execution_driver

## Customer Impact

None, because the defect never shipped.

## Detection

Manual quickstart verification; minimal repros in file, HTTP and socket forms.

## Reproduction Steps

1. Declare `operation t.a` with `param text text required` and `file append "./out/e.txt" text text`, and grant `allow_write` on `./out/**`.
2. Run `rivet --file app.rivet request t.a --params '{"text":"x"}'`.
3. Before the fix the request failed with `file.form` (exit 2). The expected result is that `x` is appended.

## Timeline

```text
2026-09-28T17:20:00Z  observed while verifying the README quickstart
2026-09-28T17:35:00Z  reproduced in file, http (decode) and socket (send) forms
2026-09-28T17:50:00Z  fix committed in 79e6318, full suite green (397 passed)
```

## Logs and Evidence

The failing and passing runs are recorded in the fix commit message and in the regression test.

## Source Files

- `src/infra/execution_driver.rs` (`keyword_slot`, `eval_arg_at`)

## Root Cause

`eval_arg` resolved every bare word that named a variable in scope, without regard to the word's grammatical position. Static analysis (check, io manifest) works on unevaluated arguments, so it never saw the problem.

## Contributing Factors

The same family as [INC-2026-0002](inc-2026-0002-option-keyword-variable-misparse.md), where option keywords doubled as variable names in the grammar. The earlier fix covered parsing, not evaluation.

## Resolution

A keyword stays a word when it is in a keyword slot:

| Slot | Example | Treated as |
|---|---|---|
| first word of an option or call | `decode text`, `conn.send text …` | keyword |
| after `as` | `file read P as text` | keyword |
| codec followed by a value | `file append P text VALUE` | keyword |
| directly after a codec | `… text text` (second) | value (variable) |
| anywhere else | `header "X-Note" text` | value (variable) |

## Verifying Tests

- `tests/conformance_files.rs::codec_named_variables_keep_keywords`
- The full `cargo test` run passes: 397 passed, 0 failed.

## Corrective Actions

Fixed before release.

## Preventive Actions

The regression test covers codec-named variables in both keyword and value positions.

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

A variable named after a format word cannot be passed as the first argument of an option (for example `timeout text`). Such an option has no realistic use.

## Lessons Learned

Positional keywords need to keep their meaning at evaluation time, not just at parse time.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [INC-2026-0002](inc-2026-0002-option-keyword-variable-misparse.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
