---
document_id: INC-2026-0011
title: "Windows port failures catalogued when Windows was dropped from CI"
document_type: incident
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-29T02:10:00Z
end_time: null
root_cause_status: partially-identified
systems: [Rivet]
components: [files, policy, audit, connectors, datagrams, transports, library]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: Every failure seen on windows-latest CI (run 36469534765, commit 850460c) before the maintainer dropped Windows from CI on 2026-09-29; kept open for a future Windows port.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0002, ADR-0003, INC-2026-0010, TRBL-2026-0007]
supersedes: null
superseded_by: null
tags: [rivet, incident, windows, portability, ci]
---

# Windows port failures catalogued when Windows was dropped from CI

> **Status:** Active (open until a Windows port plan resolves or closes each item)
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 → current
> **Owner:** Project maintainer
> **Affected Components:** files, policy, audit, connectors, datagrams, transports, library

## Incident Summary

Rivet first ran on Windows in CI during PLAN-2026-0002. Linux and macOS pass. On `windows-latest`, 10 test
targets and the release build failed, 29 tests in all. On 2026-09-29 the maintainer decided to **drop Windows
from CI and from the supported platforms** for v0.2.0, and to record every bug so it can be tackled later. This
incident is that record: one row per defect, with what failed, the likely cause and a proposed fix.

```text
 CI run 36469534765 (commit 850460c)
   ubuntu-latest  ✔   macos-latest  ✔   feature matrix ✔ ✔ ✔ ✔ ✔ ✔   deny ✔
   windows-latest ✘  build (LNK1201) + 10 test targets
        │
        ▼ maintainer, 2026-09-29: "drop windows … write incident report for bugs encountered"
   Windows removed from the CI matrix (ee8fc19) ─▶ this catalogue (W-01 … W-13) stays active
```

## Severity

S3: a whole platform is unsupported. There is no impact on the supported platforms, macOS and Linux.

## Status

Active. Windows is unsupported in v0.2.0, and none of the items below is fixed, except the rlib part of W-13
(INC-2026-0010).

## Discovery Context

The CI portability work in PLAN-2026-0002 (TASK-096). The failures were read through the check-run annotations
emitted by `scripts/ci_step.py` (TRBL-2026-0007).

## Start Time

2026-09-29T02:10:00Z

## End Time

Open.

## Affected Systems

Rivet on Windows (all surfaces).

## Affected Versions

All versions: v0.1.0 was never run on Windows.

## Affected Components

- files, policy, audit, modules (path handling)
- connectors (stdio MCP), transports (HTTP, process), datagrams (UDP)
- registry, packaging, build

## Customer Impact

Windows users cannot run Rivet reliably. The release notes and manuals state that only macOS and Linux are supported.

## Detection

GitHub Actions `windows-latest`, runs 36450430039 and 36469534765.

## Catalogue

| ID | Area | Failing test(s) | Observed | Likely cause | Proposed fix |
|---|---|---|---|---|---|
| W-01 | files | `tests/conformance_files.rs` `atomic_replace_never_leaves_temp_files` (:697), `update_with_version_guard` (:223); `src/infra/file_access.rs` unit `crud_round_trip_with_exclusive_create_and_version_guard` (:684) | `permission.os`: "./out/dir: Access is denied. (os error 5)"; the version-guarded replace fails | The atomic replace (write temp → rename over target) and the flock-guarded compare-and-replace use Unix semantics; on Windows a rename cannot replace an open or existing target, and directories are opened differently | Implement a Windows replace path (`ReplaceFileW`/`MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, share modes), or return `unsupported.conditional_update` explicitly as on other non-Unix platforms |
| W-02 | files / policy | `conformance_files` `dotdot_and_absolute_paths_are_refused` (:40) | An absolute or `..` path is not refused with the expected code | Path checks assume `/`-rooted paths; `C:\\…` and `\\` separators and drive letters are not recognised as absolute | Normalise both separators and treat drive and UNC prefixes as absolute before confinement |
| W-03 | policy | `conformance_policy_file` `discovery_beside_the_entry_file` (:121) | `policy().file` does not end with `services/policy.json` | The discovered policy path is stored with `\\` separators | Store and render policy paths with `/`, or compare path components |
| W-04 | audit / io manifest | `conformance_io_manifest` `demo11_json_manifest` (:330), `s144_check_policy_partial_denied_unknown` (:555), `s146_access_narrowing` (:587) | Manifest rows differ from the expected CSV/JSON | Source spans and targets carry Windows paths (`\\`, temp prefixes), so the rendered rows differ | Render every source path relative to the bundle root with `/` |
| W-05 | modules | `conformance_modules` `t17_cycle` (:400), `t20_manifest_graph_and_generate_cover_modules` (:769) | The cycle message and module spans differ | Module file names in messages and spans use `\\` | Same fix as W-04 (render paths with `/`) |
| W-06 | connectors (MCP stdio) | `conformance_mcp` `stdio_rivet_peer_sync_approve_call` (:157), `schema_drift_fails_load_or_keeps_rpc_identity` (:258), `stdio_fake_server_errors_and_sampling` (:394), `stdio_transport_is_authorized` (:459) | Stdio MCP servers do not start or are not authorized | The tests and the stdio transport launch servers through Unix paths and the process layer (sandbox refusal / program resolution without `.exe`) | Resolve Windows executables (`.exe`, `PATHEXT` rules without a PATH search), and define the stdio MCP process policy on a platform without a sandbox |
| W-07 | datagrams (UDP) | `conformance_udp` `truncated_and_oversized_datagrams` (:116) | `udp.receive_failed` instead of `udp.truncated` | On Windows, `recv` into a too-small buffer fails with `WSAEMSGSIZE` instead of silently truncating | Map `WSAEMSGSIZE` (os error 10040) to `udp.truncated` |
| W-08 | transports (HTTP) | `conformance_errors_limits_dag` `http_status_for_real_failures` (:404) | A connection to a closed local port reports `timeout.http` after 2000 ms instead of a refused-connection error | Windows retries SYN to a closed loopback port for about 2 s instead of failing fast with RST | Map the connect timeout on loopback, or give the test a Windows-specific expectation (an OS behaviour difference) |
| W-09 | sandbox | `conformance_sandbox` `child_cannot_spawn_descendants` | Expectation mismatch | There is no Windows sandbox backend (ADR-0003); the test assumes Unix process-tree semantics | Gate as macOS-only and add the Windows refusal assertion |
| W-10 | transports (process) | `src/features/transports/run_process.rs` unit `refusals_and_sandbox_spec` (:405) | Assertion mismatch in the sandbox-spec/refusal table | Program path forms (absolute vs bundle-relative) are validated with Unix rules | Accept Windows absolute forms; platform-specific expectations |
| W-11 | registry | `src/features/registry/describe_capabilities.rs` unit `lists_s102_features_and_stage_c` (:259) | Capability rows differ | The report is platform-dependent (the Unix sockets and sandbox rows on Windows) | Platform-specific expected rows |
| W-12 | packaging | `conformance_features` `package_list_ships_the_embedded_files` (:262) | `cargo package --list` output does not match | The listed paths use `\\` | Compare normalised paths |
| W-13 | build | release build: `LNK1201` on `rivet.pdb` | The link failed | The bin `rivet` and the ffi lib `rivet` share `rivet.pdb` | The rlib part is fixed (INC-2026-0010); the `.pdb` clash needs a distinct bin or lib output name on Windows |

Most items come from three root patterns:

```text
 path rendering / parsing with `\` and drive letters ─▶ W-02 W-03 W-04 W-05 W-12
 Unix-only OS semantics (rename-replace, flock, sockets, process tree) ─▶ W-01 W-06 W-09 W-10 W-11
 OS behaviour differences (WSAEMSGSIZE, loopback SYN retry) ─▶ W-07 W-08
 build naming ─▶ W-13
```

## Reproduction Steps

1. Add `windows-latest` back to `.github/workflows/ci.yml` (`matrix.os`).
2. Push, then read each job's annotations:
   `GET /repos/olivierdevelops/rivet/check-runs/<job_id>/annotations`.

## Timeline

```text
2026-09-29T02:10Z  first Windows CI failures read via annotations (run 36443517906)
2026-09-29T02:40Z  cfg/test portability fixes pushed (850460c); Windows still red in 10 targets
2026-09-29T03:00Z  maintainer decision: drop Windows, record the bugs
2026-09-29T03:40Z  Windows removed from CI (ee8fc19); catalogue written
```

## Logs and Evidence

CI runs 36450430039 and 36469534765 (job "test (windows-latest)"), annotation titles "tests: relevant lines" and
"build: last 80 lines".

## Source Files

See the catalogue: `src/infra/file_access.rs`, the path confinement in policy and files, `features/audit/*`,
`features/language/resolve_imports.rs`, `infra/mcp_client.rs`, `infra/process_adapter.rs`, `infra/udp_adapter.rs`,
`features/registry/describe_capabilities.rs`, `ffi/Cargo.toml`.

## Root Cause

Rivet was designed and validated on Unix (macOS). Path rendering, atomic file replacement, process and stdio
spawning, and some socket error mappings assume Unix semantics.

## Contributing Factors

There was no Windows runner until 2026-09-28, and ADR-0003 already refuses the process sandbox on Windows.

## Resolution

Not resolved. Windows was dropped from the supported platforms and from CI for v0.2.0.

## Verifying Tests

To be written per item by the future port. The failing tests listed above are the acceptance tests.

## Corrective Actions

- Record Windows as unsupported in REL-0.2.0, MAN-2026-0001 (Known Limitations) and MAN-2026-0002.
- A future plan (a Windows port) takes each W-item, fixes it, re-enables the CI job, and closes this incident.

## Preventive Actions

When Windows returns to CI, add it as a non-blocking job first, then make it blocking once the job is green.

## Owners

Project maintainer (decision and prioritisation); implementer (future port).

## Remaining Risks

Code that works on Unix may drift further from Windows while there is no Windows CI.

## Lessons Learned

Cross-platform claims need CI evidence from the start. The first real Windows run surfaced 13 distinct problem areas.

## Related Documents

- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)
- [ADR-0003](../../decisions/adr-0003-process-sandbox-backends.md)
- [INC-2026-0010](../resolved/inc-2026-0010-ffi-rlib-output-collision.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Catalogued W-01…W-13 when Windows was dropped from CI. |
