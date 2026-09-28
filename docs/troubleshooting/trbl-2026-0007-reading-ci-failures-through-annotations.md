---
document_id: TRBL-2026-0007
title: "Reading GitHub Actions failures without admin rights (check-run annotations)"
document_type: troubleshooting
status: resolved
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [cli, library]
current_status: resolved
affected_versions:
  from: "0.2.0"
  to: null
confidentiality: internal
scope: Reusable knowledge from PLAN-2026-0002 TASK-096 — how to see why a CI step failed when job logs cannot be downloaded (no admin rights, no gh CLI).
reason: DOCUMENTATION §26 — the job-log API returns 403 "Must have admin rights to Repository"; failures were invisible until CI emitted them as annotations.
related_documents: [PLAN-2026-0002, INC-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, troubleshooting, ci, github-actions]
---

# Reading GitHub Actions failures without admin rights (check-run annotations)

> **Status:** Resolved
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** cli, library

## Problem

A CI step failed, but the job log could not be read from the terminal. `gh` was not installed, and the REST
endpoint for job logs needs admin rights.

## Symptoms

```text
$ curl -s https://api.github.com/repos/olivierdevelops/rivet/actions/jobs/<job_id>/logs
{"message": "Must have admin rights to Repository.", "status": "403"}
```

The check-run annotations only said `Process completed with exit code 101`.

## Environment and Versions

GitHub Actions (ubuntu-latest, macos-latest, windows-latest), public REST API without a token, 2026-09-29.

## Investigation

```text
 runs     GET /repos/{o}/{r}/actions/runs?per_page=N          ✔ public
 jobs     GET /repos/{o}/{r}/actions/runs/{run}/jobs          ✔ public (step names + conclusions)
 logs     GET /repos/{o}/{r}/actions/jobs/{job}/logs          ✘ 403 admin rights
 notes    GET /repos/{o}/{r}/check-runs/{job}/annotations     ✔ public  ◀── use this
```

## Possible Causes

The logs endpoint is restricted by design, while annotations are part of the public check run.

## Experiments and Attempts

- Downloading the logs without a token failed with 403.
- Annotations were readable, but held only the exit code, until a step emitted its own `::error::` lines.

## Root Cause

Nothing is broken. CI output simply has to be turned into annotations to be readable without admin rights.

## Solution or Workaround

Every CI step runs through `scripts/ci_step.py <title> -- <cmd…>`:

```text
 command output ──stream to log──▶ (kept for admins)
       └──strip ANSI──▶ relevant lines (FAILED, panicked, error, -->, assertion) + tail (no Compiling noise)
                     ──▶ ≤ 4000-char chunks ──▶ ::error title=<step>: …::<escaped text>
```

Read the annotations like this (JSON may contain control characters, so parse with `strict=False`):

```sh
curl -s "https://api.github.com/repos/olivierdevelops/rivet/actions/runs?per_page=1"
curl -s "https://api.github.com/repos/olivierdevelops/rivet/actions/runs/<run>/jobs?per_page=50"
curl -s "https://api.github.com/repos/olivierdevelops/rivet/check-runs/<job_id>/annotations?per_page=50" \
  | python3 -c "import json,sys; [print(a['title'], a['message']) for a in json.loads(sys.stdin.read(), strict=False) if a.get('title')]"
```

Every step also has `if: always()`, so a failing step does not hide the result of the next one.

## Verification

CI runs 36443517906 → 36483001760: the failing tests on Linux and Windows were identified from annotations alone
(INC-2026-0010, INC-2026-0011), and the final run is green.

## Remaining Limitations

GitHub keeps at most 10 error annotations per step and 50 per job, and truncates each one at 4096 characters.
`ci_step.py` caps its output at 4 chunks of 4000 characters.

## Current Status

Resolved.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)
- [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Recorded. |
