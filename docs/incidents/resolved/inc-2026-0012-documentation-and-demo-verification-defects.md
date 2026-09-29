---
document_id: INC-2026-0012
title: "Defects found by v0.2.0 documentation and demo verification"
document_type: incident
status: resolved
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-29T06:00:00Z
end_time: 2026-09-29T12:00:00Z
root_cause_status: identified
systems: [Rivet]
components: [envelope, serve, ws, mcp, poll, library, ffi, cli, language, highlight_source, audit, policy, sessions]
affected_versions:
  from: "0.2.0-rc"
  to: "0.2.0-rc"
confidentiality: internal
scope: Nineteen defects (plus one missing deprecation signal) found while verifying the 0.2.0 documentation, demos and references (PLAN-2026-0002 P4, before P3/P5), fixed before release.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0002, PROP-2026-0002, DEMO-2026-0020, API-2026-0002, API-2026-0006, INC-2026-0007, INC-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation, v0.2.0, envelope, websocket, mcp]
---

# Defects found by v0.2.0 documentation and demo verification

> **Status:** Resolved
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 release candidate (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** envelope, serve, ws, mcp, poll, library, ffi, cli, language, highlight_source, audit, policy, sessions

## Incident Summary

Running the 0.2.0 demos, the release verification guide (DEMO-2026-0020) and the system/reference docs literally
against the release candidate turned up defects that the conformance suites did not cover. Most are envelope
inconsistencies between surfaces (missing `seq`, empty IDs, a wrong `operation`); the rest are CLI, language and
test-harness gaps. None has a security impact. All were fixed before the release; each has a regression test in
`tests/conformance_verification_defects.rs` (or the named existing suite).

| # | Defect | Where seen | Fix | Commit | Verifying test |
|---|---|---|---|---|---|
| 1 | T-29 compiled each demo without resolving imports, so `17-modules` failed | `cargo test` | demos load through `DiskSourceLoader` + `resolve_imports` (demo folder = root), like `rivet check` | `fad2940` | `conformance_samples::every_demo_bundle_compiles` |
| 2 | WS terminal record of a refused input had no `seq` and `data_count: 0` | API-2026-0002 | the ref's lane counts data records; a terminal record sent outside the pump gets the next `seq` and the real `data_count` | `4122353` | `ws_refused_input_terminal_is_numbered` |
| 3 | Library terminal records (`Envelope::record()`) had no `seq` | 12-library | `Result` → `seq = data_count + 1`; scope-stream errors are numbered after the last item | `4122353` | `library_terminal_records_carry_seq` |
| 4 | A rejected input envelope answered `operation: null` although it named an operation | API-2026-0006 | `serve.parse_input` refusals carry the named operation string (`operation`, else `id`) | `4122353` | `rejected_envelope_echoes_the_operation` |
| 5 | MCP `rivet.request` error envelopes named `rivet.request` | DEMO-2026-0020 | errors name the target operation, like successes | `4122353` | `mcp_errors_name_the_target_or_null` |
| 6 | `mcp.session_required` put the JSON-RPC method in `operation` | API-2026-0003 | protocol-level MCP refusals have `operation: null` | `4122353` | `mcp_errors_name_the_target_or_null` |
| 7 | Nested `suppressed[]` / `cause` errors still carried `effects` | API-2026-0006 | `error_object` strips `effects` recursively | `4122353` | `nested_errors_have_no_effects` |
| 8 | WS (and polling) refusals at open had empty IDs | DEMO-2026-0020 | the session request is minted before validation; refusals carry its IDs | `4122353` | `ws_open_refusals_carry_ids` |
| 9 | `rivet highlight` dropped the header tokens of an unclosed block | 16-editor | with diagnostics, uncovered lines before the error are lexed as statement lines | `1034636` | `highlight_keeps_tokens_of_an_unclosed_block` |
| 10 | Inline `output object … open true` passed `check` but was ignored | REF-2026-0002 | honoured (same as the block's `open true`); `open` on a non-object is `syntax.type` | `1034636` | `inline_open_object_output_is_honoured` |
| 11 | Remote CLI sent no `deadline_ms` over WS (`--timeout` dropped with `--input-jsonl -`) | API-2026-0002 | the request frame carries `deadline_ms` | `4a34537` | `remote_ws_sends_the_timeout` |
| 12 | `policy explain`: no `--data`; `--json` said `ok` while exiting 3; params not passed into modules | 17-modules | `--data` (alias `--params`, no warning; same for `auth complete --data/--data-file`); a denial prints a `status: error`, kind `permission` envelope on stderr (exit 3); call edges record static arguments, so callee targets are filled | `4a34537` | `policy_explain_data_denial_and_modules` |
| 13 | `check.import_duplicate` from `Runtime::load` / `rivet_load` had empty IDs | 15-ffi, 17-modules | the refusal carries minted request/trace IDs; kind stays the registry's (`syntax`, exit 2, as `rivet check`) | `4a34537` | `load_refusal_has_ids_and_registry_kind` |
| 14 | `check.module_policy_ignored` printed only by `rivet check` | 17-modules | `rivet request` and `rivet serve` print it on stderr once at load | `4a34537` | `request_prints_the_module_policy_warning` |
| 15 | Session methods' types, `TraceResult`, serve `start`/`ServeOptions` only under `rivet::internal` | API-2026-0004 | facade: `rivet::types::Session*`/`Request`/`TraceQuery`, `rivet::{TraceResult, TraceEvent}`, `rivet::serve::{start, ServeOptions, ServeHandle, …}` (feature `serve`) | `4a34537` | `facade_exposes_sessions_trace_and_serve` |
| 16 | WS `conflict.ref` for an in-flight ref looked like that ref's terminal record | 01-catalog | refusal sent with `ref: ""` and `error.details.ref` | `4122353` | `ws_duplicate_ref_refusal_is_detached` |
| 17 | Compiled-out gRPC call reported at the connector's `endpoint` line | 10-grpc (lean build) | reported at the call site (the connector is reported separately) | `1034636` | `compiled_out_grpc_points_at_the_call` |
| 18 | `import "https://…"` read as a path (`not_found.import`) | REF-2026-0002 | any `scheme://` import is `syntax.import` "imports are local paths; URL imports are not supported" | `1034636` | `url_imports_are_refused_at_check` |
| 19 | T-29 treated reference blocks starting with `global`/`import` as fragments | `cargo test` | recognised as whole files | `fad2940` | `conformance_samples::every_reference_fragment_lowers` |
| + | Legacy WS frames (`id`/`params`) — deprecation signal | coordinator | already leaves the trace note (phase `input`, decision `deprecated`); now tested. No per-frame field was added: the record shape is fixed by `stream-record.schema.json` | — | `ws_legacy_frame_leaves_a_deprecation_note` |

Commits: `fad2940` (tests), `4122353` (envelopes), `1034636` (language), `4a34537` (CLI, library, regression suite).

```text
 docs + demos run literally ──▶ real output ≠ contract (API-2026-0002/0006, schemas) ──▶ defect ──▶ fix + regression test
          │                                                                                │
          └── by design (not changed): an error inside a module names the internal         └── docs updated to the
              operation in error.operation_id while the envelope's operation is the caller      fixed behaviour
```

## Decisions

- **Item 8 — `seq` on unary WebSocket results: kept.** API-2026-0002 makes every ref a connection-owned session
  ("unary, streaming and duplex operations all run the same way") and lists `seq` among the keys of every terminal
  record; API-2026-0006 gives `seq` to "stream terminal records". A WS terminal record is therefore a stream
  terminal record and carries `seq` (a unary ref ends with `seq: 1`, `data_count: 0`). What was inconsistent was the
  refused-input terminal (item 2), which now carries `seq` too. Frame refusals for refs that never opened (and, from
  item 16, refusals detached from an in-flight ref, `ref: ""`) are not terminal records and have no `seq`. Unary
  results on REST, MCP, the CLI, the library and the C ABI keep no `seq`.
- **Item 8 — IDs on refusals at open:** a WS/polling request whose operation or params are refused has request and
  trace IDs (it became a request). A frame or envelope that never became a request (not JSON, no ref, a bad
  envelope shape) keeps empty IDs, as API-2026-0006 states for REST.
- **Item 10 — honoured,** not rejected: `open true|false` on the header line is equivalent to the field block's
  `open true` line; either one opens the object.
- **Item 12 — `policy explain --json` on a denial** prints an error envelope (API-2026-0006: `status` `error`,
  kind `permission` → exit 3) on stderr, like every other failing `--json` command; its `error.details` holds the
  full explanation (`present`, `file`, `sha256`, `grants`, `deny`, `broad`, `sites`) plus `denied[]`. Without a
  denial the `ok` envelope is unchanged on stdout. `--data` is the primary flag; `--params` stays an alias with no
  warning (it is this command's own flag, not the deprecated `request --params`).
- **Item 16 — detached refusal:** no new frame type (the `type` vocabulary is fixed by the schema); API-2026-0002
  already says a record with `ref: ""` is a refused frame, never a terminal record, so the refusal uses `ref: ""` and
  names the ref in `error.details.ref`.

## Severity

S3: envelope and CLI inconsistencies with no security impact; nothing shipped.

## Status

Resolved before the 0.2.0 release (commits `fad2940`, `4122353`, `1034636`, `4a34537`); 493 tests pass.

## Discovery Context

PLAN-2026-0002 P4: the demo re-verification (TASK-075…077, DEMO-2026-0020 Known Caveats) and the system/reference
documentation writer (items 16–19).

## Start Time

2026-09-29T06:00:00Z

## End Time

2026-09-29T12:00:00Z

## Affected Systems

Rivet.

## Affected Versions

0.2.0 release candidate only.

## Affected Components

- envelope (`domain/envelope.rs`, `domain/contracts.rs`), serve (`parse_input`, `multiplex_ws`, `serve_listener`, `setup_serve`, `setup_mcp`)
- sessions (`infra/session_driver.rs`), library (`setup_library.rs`, `lib.rs`)
- language (`lowering/lower.rs`, `highlight_source.rs`), capabilities (`domain/capabilities.rs`)
- audit (`inspect_effects.rs`, `effect_sites.rs`, `io_manifest.rs`), cli (`setup_cli.rs`, `io/cli`), remote client

## Customer Impact

None; unreleased.

## Detection

Manual, literal execution of documentation against the build.

## Reproduction Steps

1. WS: `{"type":"request","ref":"c3","operation":"chat.echo"}`, input seq 1 `"hi"`, then input seq 3. Before the fix
   the terminal record had no `seq` and `data_count: 0`.
2. `printf 'operation x.y\n    name "X"\n    return (\n' > b.rivet; rivet highlight b.rivet --format json` printed
   only line 2's tokens.
3. `rivet --file docs/demos/17-modules/app.rivet policy explain report.remote --params '{"id":3}'` kept
   `users.fetch`'s target `param_dependent`.

## Timeline

```text
2026-09-29  P4 demo verification + DEMO-2026-0020 list items 1–15 as caveats
2026-09-29  system/reference writer adds items 16–19 and the WS deprecation question
2026-09-29  fad2940 tests: T-29 import-aware demo compile, whole-file global/import blocks
2026-09-29  4122353 envelopes and surfaces (items 2–8, 16)
2026-09-29  1034636 language (items 9, 10, 17, 18)
2026-09-29  4a34537 CLI, loads and facade (items 11–15) + regression suite
2026-09-29  docs updated to the fixed behaviour; this record
```

## Logs and Evidence

The regression suite, the updated demo READMEs and API documents (real output after the fix).

## Source Files

See Affected Components; tests in `tests/conformance_verification_defects.rs` and `tests/conformance_samples.rs`.

## Root Cause

Each surface builds its envelopes on its own path (WS lanes, session driver, MCP tool results, CLI commands,
library streams); the shared `ResponseEnvelope` constructors fix the shape but not the IDs, `seq` and `operation`
each path supplies. The conformance suites validated the schema shape, not cross-surface equality of these fields.

## Contributing Factors

The demos and T-29 were written before file modules; `policy explain` predates both the 0.2.0 `--data` flag and
modules.

## Resolution

See the table in the Incident Summary.

## Verifying Tests

`tests/conformance_verification_defects.rs` (17 tests, one or more per item) and `tests/conformance_samples.rs`
(items 1 and 19).

## Corrective Actions

Fixed before release; the documents that listed these defects as caveats now show the fixed output.

## Preventive Actions

The regression suite asserts cross-surface equality of `operation`, IDs, `seq` and `data_count`; T-29 compiles
demos the way `rivet check` does.

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

Other envelope fields may still differ between surfaces in paths no demo exercises. The remote CLI's duplex path
exits only when stdin reaches EOF, even after the terminal record (pre-existing; not changed here).

## Lessons Learned

Cross-surface envelope invariants need one test that drives the same request through every surface.

## Related Documents

- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)
- [DEMO-2026-0020](../../demos/demo-2026-0020-v0-2-0-release-verification.md)
- [API-2026-0002](../../api/api-2026-0002-websocket-rivet-v1.md) · [API-2026-0006](../../api/api-2026-0006-envelopes.md)
- [INC-2026-0007](inc-2026-0007-demo-verification-defects.md) · [INC-2026-0008](inc-2026-0008-documentation-verification-defects.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Recorded and resolved (items 1–19 and the WS deprecation signal). |
