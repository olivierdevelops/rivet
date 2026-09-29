---
document_id: TEST-2026-0022
title: "T-22 — e2e / security (UC-20 / R25)"
document_type: test
status: completed
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
affected_versions:
  from: "0.1.0"
  to: null
validated_plan_requirements: [PLAN-2026-0001 R25, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-22; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-22 — e2e / security (UC-20 / R25)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

One serve over REST/SSE/poll/WS/MCP; none/bearer/mTLS; non-loopback+none; ninth WS ref; `rivet.io` exposure rule

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R25 | 026–032 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_serve.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_serve
```

Tests executed (13):

- `conformance_serve::bearer_auth_and_principals` — bearer tokens map to principals on REST, WS upgrade and MCP; missing/wrong tokens are 401, unlisted operations 403
- `conformance_serve::disabled_surfaces_answer_404` — serve.surfaces narrows the mounts; disabled poll, sse and ws answer 404 while http and mcp work
- `conformance_serve::health_and_access_log` — G12: GET /v1/health answers an envelope whose data is {status:"ok", catalog_version, version} unauthenticated on loopback, and every request produces one access-log line (time, surface, method, route, principal, operation, status, duration_ms) that never contains params or tokens
- `conformance_serve::io_manifest_exposure_requires_explicit_listing` — GET /v1/io needs an explicit rivet.io listing for a network principal; wildcards do not match rivet.*; check_files is refused remotely
- `conformance_serve::io_route_returns_the_bare_manifest` — G26: GET /v1/io returns an envelope whose data is the bare IoManifest (also for format=json); format=table|markdown|csv puts the rendered report in data
- `conformance_serve::mcp_tools_list_includes_callable_builtins` — G7: MCP tools/list lists every built-in the principal may call with schemas: the local principal sees rivet.io/policy.generate/trace.show/connectors.sync and rivet.auth.*; a wildcard network principal does not see the sensitive ones unless listed exactly
- `conformance_serve::ninth_ws_ref_is_rejected` — a ninth concurrent ref gets a limit error frame; cancel ends a ref with one cancelled error frame
- `conformance_serve::non_loopback_without_auth_refuses` — a non-loopback --listen with auth none refuses to start with serve.auth_required and exit 2, binding nothing
- `conformance_serve::same_operation_on_every_surface` — one listener answers REST, SSE, polling, WebSocket and MCP for the same catalog with identical results
- `conformance_serve::sigterm_drains_and_exits_zero` — G12: SIGTERM drains like SIGINT: the in-flight request is cancelled (not dropped), its response is sent, and `rivet serve` exits 0 with one access-log line per request on stderr
- `conformance_serve::socket_close_cancels_refs` — closing the socket cancels and joins its in-flight refs (the held concurrency slot is released)
- `conformance_serve::traceparent_is_accepted_and_emitted` — G25: a valid W3C traceparent on POST /v1/request becomes the request's trace id and every answer carries a traceparent header; nested requests keep the trace id; invalid headers are ignored
- `conformance_serve::ws_refused_input_sends_the_specific_error` — G17/G27: a refused input frame on /v1/ws gets an error frame for its ref with the specific code (conflict.input_sequence), and that is the ref's only terminal frame

v0.1.0 run: `cargo test conformance_serve`.

## Expected Results

Identical Completion; `serve.auth_required` exit 2

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

13 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

13 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_serve                  test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.92s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_serve.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_serve.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:00Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-22
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
