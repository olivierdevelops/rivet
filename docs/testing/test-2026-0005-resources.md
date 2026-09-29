---
document_id: TEST-2026-0005
title: "T-05 — integration / fault (UC-05 / R3, R12, R13)"
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
validated_plan_requirements: [PLAN-2026-0001 R3, PLAN-2026-0001 R12, PLAN-2026-0001 R13, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-05; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-05 — integration / fault (UC-05 / R3, R12, R13)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

HTTP/socket/process fixtures; early return; EOF; forced cleanup faults; Stage C refusal (TASK-094)

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R3 | 022, 033–035 |
| PLAN-2026-0001 R12 | 033–035, 094 |
| PLAN-2026-0001 R13 | 021, 035, 037, 093 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_resources.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_resources
```

Tests executed (21):

- `conformance_resources::cancel_closes_handles_gracefully_in_reverse_order` — G15: cancelling a running request closes its open WebSockets with a close handshake in reverse acquisition order (inner, then outer) within the grace, and the caller gets exactly one terminal `cancelled` error (exit 130)
- `conformance_resources::cancel_terminates_and_reaps_child_processes`
- `conformance_resources::deadline_expiry_closes_handles_gracefully` — G15: when the request deadline expires while the body waits, the open WebSockets are closed with a close handshake (not dropped) and the caller gets one terminal timeout.request error (exit 6)
- `conformance_resources::demo_03_http` — docs/demos/03-http behaves as its README describes against a local fixture (42 → user, 404 → users.not_found, retry on 503, encoded search)
- `conformance_resources::demo_04_socket_ping` — docs/demos/04-streaming socket.ping returns the fixture's pong under a WebSocket grant and is denied under the default policy
- `conformance_resources::http_authorization` — no policy denies, an ungranted port denies, and a private hostname needs a literal IP grant
- `conformance_resources::http_interpolation_keeps_segments` — an interpolated `../admin?x=` stays one encoded path segment
- `conformance_resources::http_multipart_and_xml_bodies` — `body multipart … end` sends field and file parts (the file read is authorized by allow_read before connecting; without the grant nothing is sent) and `body xml (xml.element …)` sends escaped application/xml
- `conformance_resources::http_one_shot_methods` — GET/POST/DELETE against a local fixture return {status, headers, body}; query values are encoded as data
- `conformance_resources::http_status_retry_redirect` — non-2xx fails http.status with details.status unless accepted; retries replay GET; redirects are opt-in and re-authorized
- `conformance_resources::https_with_ca_file` — HTTPS validates against `tls ca_file` (read through the broker first); without it the system trust store rejects the test CA
- `conformance_resources::process_one_shot`
- `conformance_resources::process_refusals` — exec without a grant, bare names and shell strings never spawn
- `conformance_resources::process_sandbox_confines_reads` — with a policy present the child is sandboxed: granted reads work, other paths are refused by the OS
- `conformance_resources::process_stream_break_reaps`
- `conformance_resources::stage_c_refusals` — Stage C forms (TCP TLS, interactive process, FIFO, reconnect, HTTP/3) fail with typed unsupported before any effect
- `conformance_resources::tcp_finish_send_raw_iteration` — finish_send half-closes while the response stays readable until the peer FIN ends iteration
- `conformance_resources::tcp_length32` — length32 big-endian frames round-trip bytes
- `conformance_resources::tcp_newline_and_cleanup` — newline TCP send/receive, EOF mid-frame is protocol.unexpected_eof, early return closes the connection
- `conformance_resources::unix_json`
- `conformance_resources::websocket_exchange` — WebSocket request/response, a receive loop to a terminal marker, binary echo

v0.1.0 run: `cargo test conformance_resources`.

## Expected Results

No live handles after 5 s; primary error kept; `unsupported.*` for Stage C

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

21 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

21 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_resources              test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.38s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_resources.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_resources.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:20Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-05
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
