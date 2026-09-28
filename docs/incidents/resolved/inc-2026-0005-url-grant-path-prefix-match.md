---
document_id: INC-2026-0005
title: "URL grant paths matched as raw string prefixes"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S2"
start_time: 2026-09-28T18:00:00Z
end_time: 2026-09-28T18:20:00Z
root_cause_status: identified
systems: [Rivet]
components: [policy, authorize_effect]
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Defect found and fixed during PLAN-2026-0001 validation; never released.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0001, PROP-2026-0001, SYS-2026-0003, SYS-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation, security]
---

# URL grant paths matched as raw string prefixes

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** policy, authorize_effect

## Incident Summary

An `allow_network` grant with a URL path authorized every target path that merely *started with the same characters*. A grant for `https://api.example.com/users/42` also allowed `https://api.example.com/users/420` and `…/users/42-admin`, which is wider than the grant's author intended.

```text
 grant   https://api.example.com/users/42
 target  https://api.example.com/users/420
                                  ▲
 before  "/users/420".starts_with("/users/42")  → allowed   ✗
 after   equal, or continues with "/"            → denied    ✓
```

## Severity

S2: an authorization control was wider than written, in unreleased code.

## Status

Resolved in commit `2d581b8`, before any release.

## Discovery Context

The coordinator noticed it while reviewing the policy matcher after the Fix-A…D merges (PLAN-2026-0001 P3).

## Start Time

2026-09-28T18:00:00Z

## End Time

2026-09-28T18:20:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only; no release contained the defect.

## Affected Components

- policy
- authorize_effect

## Customer Impact

None, because the defect never shipped.

## Detection

Code review, then confirmed with a unit test.

## Reproduction Steps

1. Use this policy.json: `{"version":1,"grants":[{"capability":"allow_network","targets":["https://api.example.com/users/42"]}]}`.
2. Authorize `allow_network connect https://api.example.com/users/420`.
3. Before the fix the result was `allowed`. The expected result is `denied`.

## Timeline

```text
2026-09-28T18:00:00Z  defect observed (review of selector_matches)
2026-09-28T18:20:00Z  fix committed in 2d581b8, full suite green (396 passed)
```

## Logs and Evidence

The test run is recorded under Verifying Tests, and the fix is commit `2d581b8`.

## Source Files

- `src/features/policy/authorize_effect.rs` (`url_path_matches`, `selector_matches`)

## Root Cause

`selector_matches` compared URL paths with `str::starts_with` after trimming a trailing `*`, so it ignored path-segment boundaries.

## Contributing Factors

File-path selectors use `globset` with `literal_separator(true)`, which is segment-aware, and the URL branch looked equivalent to it when it was not.

## Resolution

URL paths now match on whole segments:

| Selector path | Covers | Does not cover |
|---|---|---|
| empty or `/` | every path | — |
| `/users/42` | `/users/42`, `/users/42/posts` | `/users/420` |
| `/users/` | `/users`, `/users/7`, `/users/7/x` | `/usersx` |
| `/users/4*` (explicit raw prefix) | `/users/4`, `/users/420` | `/users/5` |

## Verifying Tests

- `features::policy::authorize_effect::tests::url_grant_paths_match_whole_segments`
- The full `cargo test` run passes: 396 passed, 0 failed.

## Corrective Actions

The fix was merged before release, and SYS-2026-0003 and SYS-2026-0008 were updated to describe the segment rule.

## Preventive Actions

The unit test covers boundary cases for every selector shape.

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

None known. Query strings are not part of URL selectors.

## Lessons Learned

Prefix checks on hierarchical names need a boundary check.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [SYS-2026-0003](../../system/components/sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0008](../../system/configuration/sys-2026-0008-policy-json-reference.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
