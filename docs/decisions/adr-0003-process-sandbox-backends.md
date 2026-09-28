---
document_id: ADR-0003
title: "Process sandbox backends per platform for v0.1.0"
document_type: decision
status: approved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
approval:
  decision_date: 2026-09-28
  approvers: [Project maintainer]
  basis: "ADR-0001 blanket approval — maintainer, 2026-09-28: \"everything else is approved\" (covers the P1 research-derived ADRs of PLAN-2026-0001)"
systems: [Rivet]
components: [policy, transports, connectors, execution]
affected_versions:
  from: not-applicable
  to: "0.1.0"
reason: PLAN-2026-0001 TASK-012 / PROP-2026-0001 F-14 and OQ-1 — decide which platforms confine sandboxed allow_exec and stdio MCP children in v0.1.0, with which mechanism and which exact source files, and which platforms refuse with unsupported.sandbox_backend.
related_documents: [RES-2026-0003, RES-2026-0002, PROP-2026-0001, PLAN-2026-0001, ADR-0001, ADR-0002]
supersedes: null
superseded_by: null
tags: [rivet, sandbox, security, seatbelt, landlock, seccomp, decision]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-12-28
---

# Process sandbox backends per platform for v0.1.0

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** policy, transports, connectors, execution

## Status

Approved on 2026-09-28. The approver is the project maintainer, under the blanket approval recorded in
[ADR-0001](adr-0001-approve-rivet-runtime-design.md) (*"everything else is approved"*). PLAN-2026-0001 names
this ADR as a P1 output (TASK-012). Adding a platform, or widening what a sandboxed child may do, needs a new
ADR with conformance evidence.

## Context

PROP-2026-0001 Increment 5 ("Enforcement limit") says a child process can make any syscall once it is running.
Sandboxed process and stdio-MCP execution therefore need a tested OS worker boundary. Without one, Rivet fails
`unsupported.sandbox_backend` **before spawning**. OQ-1 asks which backends pass on Linux, macOS and Windows.
F-14 asks for the exact `src/infra/sandbox_<platform>.rs` paths.

[RES-2026-0003](../research/res-2026-0003-process-sandbox-backends.md) found the following:

- **macOS Seatbelt** (`sandbox-exec`, deny-default profile) passed **27/27** runtime cases on macOS 26.4.1. The
  cases cover:
  - read, write and exec confinement;
  - symlink, hard-link, `..`, rename and case-variant escapes;
  - TCP, UDP, DNS and UNIX-socket denial, and a `localhost:PORT` allow rule;
  - signals, nested-sandbox loosening, grandchild inheritance and env;
  - the no-fork profile.

  Process-group cleanup misses a `setsid` double-fork. A profile without `process-fork` closes that gap.
- **Linux** Landlock (ABI ≥ 6, kernel ≥ 6.12) + seccomp + a per-scope subreaper supervisor is specified and
  cross-builds. It was **not run**: no Linux host was available.
- **Windows** AppContainer + Job Object cross-builds. It was **not run**. File grants would need persistent ACL
  changes to the user's directories.
- **No mechanism on any OS enforces a network allow-list by host for a child.** Seatbelt accepts only
  `*`/`localhost`, Landlock filters TCP ports only, and AppContainer capabilities are all-or-nothing.

## Decision

```text
 ┌──────────┬───────────────────────────────────┬────────────────────────────────────────────────────────┐
 │ Platform │ Source file (TASK-035)            │ v0.1.0 behaviour                                        │
 ├──────────┼───────────────────────────────────┼────────────────────────────────────────────────────────┤
 │ macOS    │ src/infra/sandbox_macos.rs        │ SUPPORTED. Seatbelt via /usr/bin/sandbox-exec; static   │
 │          │                                   │ deny-default SBPL; grants only as -D params; NO fork    │
 ├──────────┼───────────────────────────────────┼────────────────────────────────────────────────────────┤
 │ Linux    │ src/infra/sandbox_linux.rs        │ IMPLEMENTED, GATED. Landlock V6 (HardRequirement) +     │
 │          │                                   │ seccomp + per-scope subreaper supervisor. Advertised    │
 │          │                                   │ only after T-08 passes in Linux CI (kernel ≥ 6.12) AND  │
 │          │                                   │ the startup probe finds ABI ≥ 6; else refuse            │
 ├──────────┼───────────────────────────────────┼────────────────────────────────────────────────────────┤
 │ Windows, │ src/infra/sandbox_unsupported.rs  │ REFUSE: every sandboxed spawn fails                     │
 │ others   │ cfg(not(any(linux, macos)))       │ unsupported.sandbox_backend before spawning             │
 ├──────────┼───────────────────────────────────┼────────────────────────────────────────────────────────┤
 │ Windows  │ src/infra/sandbox_windows.rs      │ NOT CREATED in v0.1.0 (AppContainer + Job design kept   │
 │ (later)  │                                   │ in RES-2026-0003 for a future ADR)                      │
 └──────────┴───────────────────────────────────┴────────────────────────────────────────────────────────┘
```

**The v0.1.0 sandboxed-child contract** is the same on every platform that ships a backend. The policy's
grants for the child are mapped to:

- **read:** granted paths plus a fixed, published system allow-list (macOS: `/usr`, `/bin`, `/System`,
  `/Library/Apple`, `/private/var/db/timezone`, `/dev`);
- **write/create/delete:** granted paths only;
- **exec:** granted executables only;
- **env:** the `allow_env` allow-list, applied by the spawner (`env_clear` first);
- **network, including loopback:** none;
- **UNIX sockets or pipes outside grants:** none;
- **signals to processes outside the sandbox:** none;
- **macOS only — `fork`:** none. The child must be a single process.

A policy that grants a sandboxed child anything outside this set fails `unsupported.sandbox_backend`. The error
detail names the unrepresentable grant (for example `network`, `unix`, or `fork` on macOS). The failure comes
before spawn, and nothing is silently weakened.

**Selection order in `src/infra/process_adapter.rs`:**

```text
 sandbox required? ─ no ─► plain argv spawn (policy still checks allow_exec)
        │ yes
 backend compiled for this OS? ─ no ─► unsupported.sandbox_backend
        │ yes
 spec representable by backend? ─ no ─► unsupported.sandbox_backend (detail: which grant)
        │ yes
 backend self-probe passes? ─ no ─► unsupported.sandbox_backend (detail: e.g. landlock ABI 5 < 6, sandbox-exec missing)
        │ yes
 spawn confined child; scope owns and kills the whole tree; audit records backend id + profile hash
```

The stdio MCP connectors (`src/infra/mcp_client.rs`) build their `tokio::process::Command` through the same
selector before handing it to rmcp's `TokioChildProcess`.

## Rationale

- **Only claim what was proven.** The proposal marks the sandbox worker "blocked until a backend passes
  conformance". macOS passed a real adversarial suite. Linux has a strong, standard design with no run yet, so
  it ships behind a conformance gate instead of a claim. Windows has neither a run nor an acceptable file-grant
  story, so it refuses.
- **No network for children is the honest v0.1.0 answer.** No available mechanism can express "only
  `api.example.com:443`". Allowing a port or `*` would hand the child far more authority than the policy
  wrote.
- **No fork on macOS** is the only verified way to guarantee the scope's cleanup promise without a subreaper or
  job object. The trade-off is documented and fails loudly with `EPERM`.
- **Grants as `-D` parameters** make profile injection through a crafted path impossible. Linux uses `PathFd`
  handles, which have the same property.

## Alternatives Considered

| Alternative | Why rejected |
|---|---|
| Ship the Linux backend unconditionally | No runtime evidence yet. The proposal forbids advertising without conformance |
| Landlock best-effort (`CompatLevel::BestEffort`) on older kernels | Silent partial sandboxes violate fail-closed. `HardRequirement` + refusal instead |
| Single runtime-wide subreaper on Linux | Cannot attribute reparented orphans to a scope. A per-scope supervisor makes attribution exact |
| Allow `fork` on macOS with process-group kill | Verified escape: a `setsid` double-fork survives `killpg` |
| Allow child network by port (`*:443`, Landlock `ConnectTcp 443`) | Grants every host on that port. Not the policy's meaning |
| Windows AppContainer in v0.1.0 | Persistent ACL mutation on user directories, and no test host |
| Third-party wrappers (`nono`, `birdcage`, `hakoniwa`, `gaol`, `rappct`) | MSRV 1.95, GPL-3.0, LGPL/namespaces, unmaintained, and single-maintainer maturity respectively. Direct `landlock`/`seccompiler`/`windows-sys` and `sandbox-exec` are smaller and auditable |
| `sandbox_init` FFI on macOS | Equally deprecated. `sandbox-exec` needs no unsafe FFI and is what the prototype proved |

## Consequences

### Positive Consequences

- OQ-1 and F-14 are closed. TASK-035 and PF-25 have exact file names.
- macOS users get a proven sandbox for single-process workers and stdio MCP servers in v0.1.0.
- Every other case fails closed with a specific, documented reason.

### Negative Consequences

- Sandboxed children cannot use the network on any platform in v0.1.0.
- On macOS, sandboxed children cannot fork, which excludes launcher-style servers such as `npx`-spawned MCP
  servers under a sandbox.
- Windows users have no sandboxed exec in v0.1.0.
- Linux support depends on setting up a CI runner with kernel ≥ 6.12. Current GitHub-hosted Ubuntu images run
  6.8/6.11.

### Risks

- **Apple removes `sandbox-exec`.** The backend's self-probe fails and Rivet refuses (fail-closed). A new ADR
  would be needed.
- **macOS metadata and argv visibility.** A sandboxed child can `stat` paths outside its grants and read argv of
  same-user processes. Rivet never places secrets in argv, and the release notes and manuals must state this.
- **The Linux design has an unknown interaction with real workloads.** An example is programs that need
  `AF_UNIX` for name-service lookups. T-08 and the manuals must cover it.

## Implementation Impact

- **TASK-035** creates `src/infra/sandbox_macos.rs`, `src/infra/sandbox_linux.rs` and
  `src/infra/sandbox_unsupported.rs`, selected by `cfg` in `src/infra/process_adapter.rs`.
  `src/infra/sandbox_windows.rs` is not created. PF-25 in PLAN-2026-0001 should list these three exact files.
- **T-08** (`cargo test conformance_sandbox`) must:
  - port the 27 macOS cases of RES-2026-0003;
  - add the Linux equivalents: `/proc/<pid>/environ`, `AF_UNIX`, UDP, `setsid` orphan cleanup, and a kernel
    below ABI 6 refusing;
  - on Windows, assert `unsupported.sandbox_backend` before any process is created.
- **`rivet.capabilities`** reports the following per platform, and advertises the Linux backend only after
  T-08 passes:
  - `sandbox_backend: seatbelt | landlock-v6 | none`;
  - `sandbox_child_network: false`;
  - `sandbox_child_fork: true | false`.
- **The error registry** keeps the single code `unsupported.sandbox_backend` (HTTP 501, exit 5). The detail
  field names the reason.
- **Documentation:** the sandbox demo (`docs/demos/11-sandbox`), the operator manual and the release notes must
  state the platform table and the child contract above.

## Superseded Decisions

None.

## Related Documents

- [RES-2026-0003](../research/res-2026-0003-process-sandbox-backends.md) — evidence and capability matrix
- [ADR-0002](adr-0002-rust-crate-selection.md) — `landlock`, `seccompiler`, `windows-sys` in the dependency block
- [ADR-0001](adr-0001-approve-rivet-runtime-design.md) — approval basis
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md) — Increment 5, F-14, OQ-1, T-08
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — TASK-011, TASK-035, PF-25

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded per-platform sandbox backends and the v0.1.0 child contract from RES-2026-0003 under ADR-0001's blanket approval. |
