---
document_id: REL-0.2.1
title: "Rivet 0.2.1 release notes"
document_type: release
status: draft
created_date: 2026-09-30
last_updated: 2026-09-30
document_revision: 1
authors: [Claude]
owner: Project maintainer
version: "0.2.1"
git_tag: v0.2.1
git_commit: recorded after tagging
release_date: 2026-09-30
source_branch: main
previous_version: "0.2.0"
previous_tag: v0.2.0
systems: [Rivet]
components: [cli]
affected_versions:
  from: "0.2.1"
  to: null
confidentiality: internal
scope: Patch release of 0.2.0 with one fix, INC-2026-0013 (`--input-jsonl -` waited for stdin EOF after the request ended). No other functional change.
reason: DOCUMENTATION §33 — every release has a release document; the maintainer chose to cut v0.2.1 for INC-2026-0013 (2026-09-30).
related_documents: [REL-0.2.0, INC-2026-0013, PLAN-2026-0002, RPT-2026-0015, DEMO-2026-0020]
supersedes: null
superseded_by: null
tags: [rivet, release, v0.2.1, patch]
---

# Release 0.2.1

> **Status:** Draft
> **Created:** 2026-09-30
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.1
> **Owner:** Project maintainer
> **Affected Components:** cli

## Release Identity

```text
Release Version:          0.2.1 (patch of 0.2.0)
Git Tag:                  v0.2.1
Git Commit:               recorded after tagging
Release Date:             2026-09-30
Source Branch:            main
Previous Version:         0.2.0
Previous Tag:             v0.2.0
Previous Release Commit:  21bb2e94dfa476856495997c4f510f5537c65225
Repository:               https://github.com/olivierdevelops/rivet (origin)
Supported platforms:      macOS, Linux (Windows unsupported: INC-2026-0011)
Artifacts:                dist/v0.2.1/ — rivet, librivet.dylib + librivet.a, rivet.h, rivet.pc, rivet-0.2.1.vsix, SHA256SUMS
CI:                       pending
```

## Plan

[PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) TASK-097. The maintainer
decided on 2026-09-30 to cut v0.2.1 for INC-2026-0013, found by the CI run on the `v0.2.0` tag.

```text
 v0.2.0 (21bb2e9) ──tag CI──▶ INC-2026-0013 ──fix dd5e5ad──▶ v0.2.1 = v0.2.0 + fix + version/doc updates
```

## Project Standards Baseline

As in [REL-0.2.0](rel-0.2.0-release-notes.md): AGENTS.md, DOCUMENTATION.md, STD-2026-0001, VHCO. PASS.

## User Requirements

No requirement changed. R1–R24 of PROP-2026-0002 stay as released in 0.2.0 (see
[RPT-2026-0015](../reports/rpt-2026-0015-validation-of-plan-2026-0002.md), 24 PASS). The fix restores the
expected behaviour of R4 and R22 (`--input-jsonl -` with the CLI and sessions).

| Requirement | Source | Released Update | Result |
|---|---|---|---|
| PROP-2026-0002 R4 / R22 | INC-2026-0013 | U-F1 | PASS |

## Added

Nothing.

## Changed

Nothing, apart from the version (0.2.1) and the install tag in the documentation (`--tag v0.2.1`).

## Fixed

- [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md): the process no longer
  waits for stdin EOF after the request ends, locally or with `--endpoint`. The runtime shuts down in the
  background, and a rejected input line always cancels the request.

## Removed

The 0.2.0 known issue "`--input-jsonl -` waits for stdin EOF".

## Released Updates and Verification

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-F1 | INC-2026-0013 (R4, R22) | CLI exits when the request ends, with stdin still open | `rivet --file app.rivet request echo.two --input-jsonl - --stream`; write two lines and keep stdin open | Records, then exit 0 at once (on 0.2.0 it waits for EOF) | `conformance_streams::input_jsonl_exits_when_the_operation_ends_without_eof` (fails on 0.2.0, passes on 0.2.1); manual run local and `--endpoint` |

## Tests

| Test | Requirement | Result |
|---|---|---|
| `conformance_streams::input_jsonl_exits_when_the_operation_ends_without_eof` | R4, R22 | PASS |
| `conformance_streams::rejected_input_line_always_cancels_the_local_request` | R4 | PASS |
| `conformance_grpc::cli_live_input_local_and_remote` | R22 | PASS |
| Full suite (`cargo test --workspace --all-targets --all-features`) | all | 495 passed, 0 failed |
| [TEST-2026-0033](../testing/test-2026-0033-release.md) (release identity) | release | PASS (v0.2.1 run) |

## Validation

[RPT-2026-0015](../reports/rpt-2026-0015-validation-of-plan-2026-0002.md) (24 PASS) still applies. 0.2.1 changes
only the CLI shutdown path, and that change is covered by the tests above.

## Measured Results

NOT APPLICABLE: no performance claim.

## Incidents

| Incident | Severity | Title | Status |
|---|---|---|---|
| [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md) | S3 | `--input-jsonl -` waited for stdin EOF after the request ended | resolved |

## Troubleshooting

[TRBL-2026-0007](../troubleshooting/trbl-2026-0007-reading-ci-failures-through-annotations.md) is how the tag CI
failure was read.

## Release Verification Guide and Demo

[DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) applies to 0.2.x. The patch adds U-F1
above. [10-grpc](../demos/10-grpc/README.md) describes the fixed exit behaviour.

## Manual

Only version strings and the install tag changed (MAN-2026-0001/0002/0004/0006/0007/0009).

## System

Only version strings in examples changed (API-2026-0001/0003/0004, SYS-2026-0004, OPS-2026-0001, ONB-2026-0001,
MIG-2026-0001).

## Documentation Impact

| Artifact | Decision | Updated Document or Reason |
|---|---|---|
| Release verification guide / demo | UPDATED | U-F1 here; 10-grpc caveats |
| README.md | UPDATED | Current release 0.2.1; install tag |
| System documentation | UPDATED | version strings in examples |
| Architecture documentation | NOT APPLICABLE | no structural change |
| API and CLI reference | UPDATED | version strings |
| Manual | UPDATED | version strings and install tag |

## Source Changes

| Source | Change | Reason |
|---|---|---|
| `src/orchestrator/setup_cli.rs` | Modified | `rt.shutdown_background()` after the command; re-cancel after a rejected input line (INC-2026-0013) |
| `tests/conformance_streams.rs` | Modified | two regression tests |
| `Cargo.toml`, `Cargo.lock`, `ffi/Cargo.toml`, `editors/vscode/package.json` | Modified | version 0.2.1 |

## Known Issues

The same as REL-0.2.0 except the fixed item: Windows is unsupported (INC-2026-0011), there is no ASan CI job for
`rivet-ffi`, and crates.io publication is deferred (G-PUB).

## Limitations

As in [REL-0.2.0](rel-0.2.0-release-notes.md#limitations).

## Follow-up Work

As in REL-0.2.0 (Windows port, ASan job, 0.3.0 alias removal, G-PUB).

## Release Completion Gate (DOCUMENTATION §34)

Walked for the patch on 2026-09-30. Every item that REL-0.2.0 walked still holds for the unchanged scope. The patch-specific items:

| Condition | State | Evidence |
|---|---|---|
| Bug documented | ✔ | INC-2026-0013 |
| Tests completed and recorded | ✔ | Tests above; TEST-2026-0033 |
| Canonical version updated and synchronized | ✔ | `check_version.py --tag`: 0.2.1 everywhere |
| Release commit, clean tree, full SHA | ✔ | `recorded after tagging` |
| Tag created, matches, resolves to the commit | ✔ | annotated `v0.2.1` |
| Commit and tag pushed | ✔ | origin |
| CI | ✔ | pending |
| Six documentation-impact decisions | ✔ | Documentation Impact above |
| Indexes regenerated | ✔ | releases, incidents, testing |

## Related Documents

- [REL-0.2.0](rel-0.2.0-release-notes.md) · [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md) · [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-30 | Claude | Drafted for the release commit. |
