---
document_id: RES-2026-0003
title: "Process sandbox backends for sandboxed allow_exec and stdio MCP"
document_type: research
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, transports, connectors, execution]
affected_versions:
  from: not-applicable
  to: "0.1.0"
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, reviewers, security-reviewers]
scope: Which OS mechanisms can confine a child process started by allow_exec or a stdio MCP connector on Linux, macOS and Windows; what each can and cannot enforce; which platforms ship a sandbox backend in v0.1.0 and which refuse with unsupported.sandbox_backend.
reason: PLAN-2026-0001 TASK-011 / PROP-2026-0001 F-14 and OQ-1 — the proposal marks the sandboxed subprocess worker "blocked per platform" until a backend passes conformance, and requires exact src/infra/sandbox_<platform>.rs paths.
related_documents: [PROP-2026-0001, PLAN-2026-0001, ADR-0001, ADR-0003, RES-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, sandbox, seatbelt, landlock, seccomp, appcontainer, job-object, security]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Process sandbox backends for sandboxed allow_exec and stdio MCP

> **Status:** Completed — macOS Seatbelt backend runtime-verified (27/27 cases PASS); Linux and Windows build-verified only
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** policy, transports, connectors, execution

## Summary

A Rivet child process (`allow_exec`, or a stdio MCP server) can make any syscall once it is running. The broker
cannot see those syscalls, so PROP-2026-0001 Increment 5 requires an OS-enforced worker boundary. If none has
passed conformance, the spawn must fail with `unsupported.sandbox_backend` **before** it happens. This research
answers OQ-1 per platform:

```text
                  mechanism                         evidence here            v0.1.0 recommendation
  ┌──────────┬─────────────────────────────────┬──────────────────────────┬───────────────────────────────────┐
  │ macOS    │ Seatbelt via /usr/bin/sandbox-  │ RUNTIME: 27/27 PASS on   │ SHIP  src/infra/sandbox_macos.rs  │
  │          │ exec, static SBPL + -D params   │ macOS 26.4.1 (arm64)     │ (no-fork, no-network children)    │
  ├──────────┼─────────────────────────────────┼──────────────────────────┼───────────────────────────────────┤
  │ Linux    │ Landlock ABI ≥ 6 (kernel 6.12)  │ BUILD ONLY (zigbuild     │ BUILD src/infra/sandbox_linux.rs; │
  │          │ + seccomp-BPF + no_new_privs +  │ x86_64-unknown-linux-gnu)│ advertise only after T-08 passes  │
  │          │ per-scope subreaper supervisor  │ not run: no Linux host   │ in Linux CI; else refuse          │
  ├──────────┼─────────────────────────────────┼──────────────────────────┼───────────────────────────────────┤
  │ Windows  │ AppContainer (0 capabilities) + │ BUILD ONLY (zigbuild     │ REFUSE: unsupported.sandbox_      │
  │          │ Job Object (kill-on-close)      │ x86_64-pc-windows-gnu)   │ backend (design kept for later)   │
  ├──────────┼─────────────────────────────────┼──────────────────────────┼───────────────────────────────────┤
  │ other    │ —                               │ —                        │ REFUSE                            │
  └──────────┴─────────────────────────────────┴──────────────────────────┴───────────────────────────────────┘
```

Two limits hold on every platform, and they shape the v0.1.0 contract:

- **Network allow-lists at host granularity cannot be enforced by any of these mechanisms.**
  - Seatbelt accepts only `*` or `localhost` as a host. This was verified here: `host must be * or localhost`.
  - Landlock filters TCP by **port** only.
  - AppContainer network capabilities are all-or-nothing.

  So in v0.1.0 sandboxed children get **no network**. A policy that grants network to a sandboxed child fails
  `unsupported.sandbox_backend`.
- **Environment confinement is the spawner's job.** `env_clear()` plus the `allow_env` allow-list does it. The
  OS sandbox only adds protection against reading other processes' memory or environment.

The decision is recorded in [ADR-0003](../decisions/adr-0003-process-sandbox-backends.md).

## Question

1. For each OS, which primitive can confine the following for a spawned child and **all of its descendants**?
   - file reads and writes to granted paths (symlinks, hard links, `..`, case variants, renames);
   - network (deny, and allow-list);
   - other IPC (UNIX sockets, signals);
   - execution;
   - environment;
   - process-tree lifetime.
2. Which platforms can therefore run sandboxed `allow_exec` or stdio MCP in v0.1.0, and which must refuse?
3. What exact `src/infra/sandbox_<platform>.rs` files does TASK-035 create?

## Method

```text
 macOS (this machine)   : scratch crate sandbox-proto spawns children under sandbox-exec with a deny-default
                          profile; every case is run twice — unsandboxed CONTROL (must succeed) and SANDBOXED —
                          so a denial counts only if the control proved the operation possible.
 Linux, Windows         : desk research (kernel/Win32 documentation, crate sources) + a compile-only scratch
                          crate sandbox-xplat implementing the backend sketch, cross-built with
                          cargo zigbuild for x86_64-unknown-linux-gnu and x86_64-pc-windows-gnu.
                          No Linux kernel or Windows host was available: NOT runtime-verified.
 crates                 : landlock 0.4.7, seccompiler 0.5.0, windows-sys 0.61.2, rappct 0.13.3 (evaluated),
                          nono 0.78.0 / birdcage 0.8.1 / hakoniwa 1.8.0 / gaol 0.2.1 (surveyed)
```

All prototypes live in the session scratch directory, never in the repository. The profile and the key code are
reproduced inline below so the result can be rebuilt.

## Results

### Capability matrix

The cell legend:
- **✔ R** — enforced, runtime-verified here;
- **✔ B** — enforced by design, build-verified only;
- **◐** — partial (see the notes after the table);
- **✘** — not enforceable by the mechanism.

| Property | macOS Seatbelt (`sandbox_macos.rs`) | Linux Landlock + seccomp (`sandbox_linux.rs`) | Windows AppContainer + Job (not shipped) |
|---|---|---|---|
| Read confined to granted paths | ✔ R (allow-list of system dirs + grants) | ✔ B (`AccessFs::from_read` per `PathBeneath`) | ◐ B (ACL-based: only objects granting the container SID or ALL APPLICATION PACKAGES; system dirs readable) |
| Write/create/delete confined to granted paths | ✔ R | ✔ B | ✔ B (ACE for container SID on WORK only) |
| Symlink / `..` / rename / hard-link escapes | ✔ R (5 cases) | ✔ B (Landlock checks the resolved hierarchy; `Refer` right governs cross-dir link/rename) | ✔ B (object ACLs, path-independent) |
| Case-variant paths | ✔ R (APFS case-insensitive; deny holds) | n/a (case-sensitive fs) | ✔ B (ACLs are object-based) |
| Exec only from allowed dirs | ✔ R (`process-exec` filter) | ✔ B (`AccessFs::Execute` only on exec grants) | ◐ B (any readable+executable image) |
| Metadata (`stat`) outside grants | ◐ R visible (`file-read-metadata` allowed; needed for path resolution) | ◐ visible (Landlock does not restrict `stat`) | ◐ ACL-dependent |
| Network: deny all | ✔ R (TCP, UDP, DNS, UNIX) | ✔ B (Landlock TCP + seccomp `socket()` for UDP/raw/netlink/packet) | ✔ B (zero capabilities; loopback blocked for AppContainers) |
| Network: allow-list by host | ✘ (`host must be * or localhost`) | ✘ (port-only TCP rules) | ✘ (capabilities are all-or-nothing) |
| Network: allow `localhost:PORT` only | ✔ R | ◐ (port rule matches any host) | ✘ |
| UNIX-socket connect outside grants | ✔ R | ◐ B (Landlock `ResolveUnix` needs ABI 9 / Linux 7.1; else seccomp denies `socket(AF_UNIX)`) | ✔ B (ACL on pipe/socket objects) |
| Signals to processes outside | ✔ R (`signal (target same-sandbox)`) | ✔ B (Landlock `Scope::Signal`, ABI 6) | ✔ B (AppContainer token cannot open others' processes) |
| Nested sandbox cannot loosen | ✔ R (`sandbox_apply: Operation not permitted`) | ✔ B (Landlock layers only restrict; `no_new_privs`) | ✔ B (token cannot be raised) |
| Applies to all descendants | ✔ R (grandchild case) | ✔ B (Landlock and seccomp inherited across fork/exec) | ✔ B (token + job inherited; breakaway not allowed) |
| Whole tree killed at scope end | ◐ R: process-group kill misses a `setsid` double-fork → **fork denied in v0.1.0 profile**, verified | ✔ B via per-scope subreaper supervisor + `PR_SET_PDEATHSIG` (design) | ✔ B (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) |
| Environment | ✔ R by spawner (`env_clear` + allow-list); other processes' env not exposed by macOS 26 (`KERN_PROCARGS2` omits it) | ✔ by spawner; `/proc/*/environ` covered by fs rules; `ptrace`/`process_vm_readv` denied by seccomp | ✔ by spawner (explicit env block) |
| Other processes' argv visible | ◐ R yes (`sysctl-read` is required: without it Rust binaries abort creating the stack guard page) | ◐ `/proc` readable only if granted (it is not) | ✘ not visible to AppContainer |
| Kernel/OS availability | Every supported macOS (`sandbox-exec` deprecated since 10.8, still shipped on 26.4.1) | Landlock ABI 6 = Linux ≥ 6.12 **and** Landlock in the active LSM list | Windows 8+ |

### macOS evidence (runtime)

The profile below is a static SBPL text. Host paths arrive **only** as `-D` parameters, so a path cannot inject
profile syntax. The only spliced value is a validated `u16` port. Seatbelt matches resolved vnode paths, so
granted paths must be canonical (`/tmp` → `/private/tmp`).

```scheme
(version 1)
(deny default)
(allow process-fork)                       ; REMOVED in the v0.1.0 profile (see "process tree" below)
(allow process-exec (subpath "/bin") (subpath "/usr/bin") (subpath (param "WORK")))
(allow signal (target same-sandbox))
(allow sysctl-read)
(allow file-read-metadata)
(allow file-read*
    (literal "/")
    (subpath "/usr") (subpath "/bin") (subpath "/System") (subpath "/Library/Apple")
    (subpath "/private/var/db/timezone") (subpath "/dev")
    (subpath (param "WORK")) (subpath (param "RO")))
(allow file-write* (subpath (param "WORK")) (literal "/dev/null"))
(allow file-ioctl (literal "/dev/null") (literal "/dev/tty"))
;; only when a loopback port is granted:  (allow network-outbound (remote ip "localhost:PORT"))
```

Spawn shape: `sandbox-exec -p <profile> -D WORK=<canon> -D RO=<canon> -- <argv…>`, with `env_clear()`, an
explicit env allow-list, `stdin` null or piped, `process_group(0)` and `kill_on_drop(true)`.

The output below is verbatim from `sandbox-proto`. Scratch paths are shortened to `…`. `control_exit` is the
same command run without a sandbox.

```text
PASS write inside WORK                           expect=allow control_exit=0 sandboxed_exit=0  ok
PASS read RO input                               expect=allow control_exit=0 sandboxed_exit=0  readonly-input
PASS write RO dir                                expect=deny  control_exit=0 sandboxed_exit=1  …/ro/w-sbx.txt: Operation not permitted
PASS read secret outside grants                  expect=deny  control_exit=0 sandboxed_exit=1  cat: …/secret/secret.txt: Operation not permitted
PASS write outside grants                        expect=deny  control_exit=0 sandboxed_exit=1  …/outside/w-sbx.txt: Operation not permitted
PASS write /tmp                                  expect=deny  control_exit=0 sandboxed_exit=1  /tmp/rivet-sbx-escape-sbx-…: Operation not permitted
PASS write via symlink WORK->outside             expect=deny  control_exit=0 sandboxed_exit=1  …/work/link: Operation not permitted
PASS hard link outside file into WORK            expect=deny  control_exit=0 sandboxed_exit=1  ln: …/work/hl-sbx.txt: Operation not permitted
PASS grandchild writes outside                   expect=deny  control_exit=0 sandboxed_exit=1  …/outside/gc-sbx.txt: Operation not permitted
PASS exec binary outside exec allow-list         expect=deny  control_exit=0 sandboxed_exit=71 execvp() of '…/secret/run.sh' failed: Operation not permitted
PASS nested sandbox-exec (allow default)         expect=deny  control_exit=0 sandboxed_exit=71 sandbox-exec: sandbox_apply: Operation not permitted
PASS tcp connect loopback (no net grant)         expect=deny  control_exit=0 sandboxed_exit=1
PASS tcp connect allowed localhost:port          expect=allow control_exit=0 sandboxed_exit=0  Connection to 127.0.0.1 port 61958 [tcp/*] succeeded!
PASS tcp connect other port (grant is one port)  expect=deny  control_exit=0 sandboxed_exit=1
PASS tcp connect public IP 1.1.1.1:443           expect=deny  control_exit=0 sandboxed_exit=1
PASS udp send public IP                          expect=deny  control_exit=0 sandboxed_exit=1
PASS unix socket connect (outside dir)           expect=deny  control_exit=0 sandboxed_exit=1  connect: Operation not permitted
PASS stat() metadata outside grants (INFO)       expect=allow control_exit=0 sandboxed_exit=0
PASS DNS lookup (mDNSResponder)                  expect=deny  control_exit=0 sandboxed_exit=1
PASS read secret via case-variant path           expect=deny  control_exit=0 sandboxed_exit=1  cat: …/SECRET/secret.txt: Operation not permitted
PASS write WORK via case-variant path            expect=allow control_exit=0 sandboxed_exit=0
PASS write outside via WORK/../outside           expect=deny  control_exit=0 sandboxed_exit=1  …/work/../outside/dotdot-sbx.txt: Operation not permitted
PASS rename WORK file to outside                 expect=deny  control_exit=0 sandboxed_exit=1  mv: rename … Operation not permitted
PASS create symlink in WORK then read secret     expect=deny  control_exit=0 sandboxed_exit=1  cat: …/work/sl-sbx: Operation not permitted
PASS signal process outside sandbox              expect=deny  control_exit=0 sandboxed_exit=1  kill: (96788) - Operation not permitted
PASS env = spawner allow-list only               PATH=/usr/bin:/bin RIVET_ALLOWED=1
INFO process-tree: before killpg 2 sleepers [97674, 97680]; after killpg 1 survivors [97680]
     (setsid escapee survives process-group kill; it stays sandboxed)
PASS no-fork profile: fork() refused             fork denied: Operation not permitted
SUMMARY 27 cases, 0 failed
```

Additional manual checks on the same host:

| Check | Result |
|---|---|
| `(remote ip "1.1.1.1:443")` in a profile | Rejected by the profile compiler: `host must be * or localhost in network address` |
| `(remote ip "*:443")` | Allows **any** host on 443, so it is useless as an allow-list |
| No-fork profile running `/bin/echo` directly and `/bin/sh -c '/bin/echo …'` | Both succeed. `sh` execs its last command without forking, so single-command wrappers still work |
| Profile without `sysctl-read` | Rust binaries abort (`failed to allocate a guard page`). `sysctl-read` stays |
| `sysctl-name` / `process-info*` filters | `illegal function` on 26.4.1 (not available in `sandbox-exec` SBPL). Argv of same-user processes stays visible |
| `KERN_PROCARGS2` of another same-user process, **unsandboxed** | argv only. macOS 26 omits the environment, so Rivet's env secrets are not exposed this way |

**Process tree.** Seatbelt is inherited by every descendant, and the grandchild case proves it. Cleanup is a
different matter. A child can double-fork and call `setsid`, which moves the grandchild out of the process
group. The escapee survives `killpg`, still sandboxed, and outlives the scope. macOS has no subreaper and no job
object, so the v0.1.0 profile **omits `(allow process-fork)`**: the child is then the whole tree and killing
one pid ends it. A command that needs to fork (for example `npx`-launched servers) fails inside the sandbox
with `EPERM`. That is a documented platform limitation, not silent weakening.

### Linux design (build-verified)

```text
 runtime (tokio)                        per-scope supervisor (re-exec: rivet, hidden internal argv)
 ───────────────                        ───────────────────────────────────────────────────────────
 broker: grants → SandboxSpec ──pipe──► prctl(PR_SET_CHILD_SUBREAPER)      ◄── orphans reparent HERE
 probe: landlock ABI ≥ V6?                prepare Landlock ruleset (PathFd per grant) + BPF program
        no  → unsupported.sandbox_backend fork → child: setsid; PR_SET_PDEATHSIG(SIGKILL);
        yes → spawn supervisor                          restrict_self (HardRequirement, no_new_privs);
 scope end ──SIGTERM/grace/SIGKILL──►                   seccomp apply; execve(argv, env allow-list)
                                          scope end: kill every child until waitpid → ECHILD
```

The sketch (`sandbox-xplat/src/linux.rs`) cross-compiles. Its key choices:

- **Landlock** (`landlock` 0.4.7):
  - `Ruleset::default().set_compatibility(CompatLevel::HardRequirement)`;
  - `handle_access(AccessFs::from_all(V6))`, `handle_access(AccessNet::from_all(V6))` and
    `scope(Scope::from_all(V6))`;
  - rules: `PathBeneath(read grants, from_read)`, `PathBeneath(exec grants, from_read | Execute)` and
    `PathBeneath(write grants, from_all & !Execute)`;
  - no `NetPort` rules in v0.1.0, which means no TCP.

  `HardRequirement` makes a kernel below ABI 6 an **error** (turned into `unsupported.sandbox_backend`) instead
  of a silent partial sandbox. Rivet also rejects `RulesetStatus != FullyEnforced`.
- **seccomp** (`seccompiler` 0.5.0): default allow, `EPERM` for:
  - `socket()` with domain `AF_UNIX`, `AF_NETLINK`, `AF_PACKET`, `AF_INET` or `AF_INET6`. Landlock ≤ V8
    covers neither UDP, raw sockets nor pathname UNIX connect;
  - `ptrace`, `process_vm_readv`/`process_vm_writev`, `bpf`, `perf_event_open`, `io_uring_setup` (io_uring can
    bypass syscall filtering);
  - `mount`, `umount2`, `pivot_root`, `unshare`, `setns`;
  - `keyctl`, `add_key`, `request_key`, `kexec_load`, `init_module`.
- **Tree cleanup:** a per-scope supervisor is the child subreaper, so double-forked orphans reparent to it and
  not to init. Attribution is exact (the supervisor only ever has this scope's processes), which is why a
  single runtime-wide subreaper was rejected.
- **Kernel floor:** ABI 6 = Linux 6.12 (signal and abstract-socket scoping), for example Debian 13 or RHEL 10.
  Ubuntu 24.04 GA (6.8) and the current GitHub `ubuntu-24.04` runner kernels (6.8/6.11) are **below** it. T-08
  on Linux therefore needs a runner with kernel ≥ 6.12, such as a KVM guest on a hosted runner or a self-hosted
  runner.

Not verified here: none of this has run. The v0.1.0 rule is that Linux advertises the backend only when T-08
passes in Linux CI **and** the startup probe finds ABI ≥ 6. Until then it fails `unsupported.sandbox_backend`.

### Windows design (build-verified, not shipped)

The sketch (`sandbox-xplat/src/windows.rs`, `windows-sys` 0.61.2) works as follows:

1. `CreateAppContainerProfile` (or `DeriveAppContainerSidFromAppContainerName`).
2. `SECURITY_CAPABILITIES { CapabilityCount: 0 }`, passed through
   `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`.
3. `CreateProcessW(CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT)` with an
   explicit env block.
4. `AssignProcessToJobObject` on a job with `KILL_ON_JOB_CLOSE`, `DIE_ON_UNHANDLED_EXCEPTION` and
   `ACTIVE_PROCESS` limits, then `ResumeThread`.

Why Windows does not ship in v0.1.0:

- **File grants mutate host ACLs.** Giving the container write access to `WORK` means adding an ACE for the
  container SID to the directory (`SetNamedSecurityInfoW`). That is a persistent change to the user's
  filesystem. A crash leaves stale ACEs, and Rivet would have to journal and repair them. That is new,
  security-sensitive state management with no test host available.
- **Reads are ACL-driven, not allow-listed.** Anything readable by ALL APPLICATION PACKAGES is readable. LPAC
  tightens this but breaks many executables. Behaviour depends on the host's ACLs, so it needs real-host
  conformance.
- `rappct` 0.13.3 (MIT, MSRV 1.90) wraps these APIs. It has a single maintainer, about 24k downloads and a last
  release on 2025-10-23. It is not selected; `windows-sys` covers the six calls needed.
- No runtime evidence: no Windows host was available.

### Crates surveyed

| Crate | Version | Licence | Verdict |
|---|---|---|---|
| `landlock` | 0.4.7 | MIT OR Apache-2.0 | **Selected** (Linux). Knows ABI V1–V9. `HardRequirement` compatibility mode |
| `seccompiler` | 0.5.0 | Apache-2.0 OR BSD-3-Clause | **Selected** (Linux). Pure Rust BPF compiler (Firecracker), no libseccomp C dependency |
| `libseccomp` | 0.4.0 | MIT OR Apache-2.0 | Not selected: needs the system C library |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | Selected for the future Windows backend; already in the dependency block |
| `rappct` | 0.13.3 | MIT | Evaluated, not selected (maturity) |
| `nono` | 0.78.0 | Apache-2.0 | Landlock + Seatbelt wrapper; **MSRV 1.95 > 1.90**, rejected |
| `birdcage` | 0.8.1 | **GPL-3.0-or-later** | Rejected (licence; last release 2024-04) |
| `hakoniwa` | 1.8.0 | LGPL-3.0 WITH linking exception | Linux namespaces; rejected (licence, namespaces need unprivileged userns) |
| `gaol` | 0.2.1 | MIT/Apache-2.0 | Unmaintained since 2019 |
| `extrasafe` | 0.5.1 | MIT | In-process seccomp helper; not a child-process sandbox |
| macOS: none | — | — | No maintained crate; `sandbox-exec` via `tokio::process` needs no FFI (`sandbox_init` is also deprecated) |

## Recommendation

```text
 policy grants for the child ──► SandboxSpec{read[], write[], exec[], env[], net: none, unix: none}
                                         │
                   spawn request ─► backend for this OS? ── none ──► unsupported.sandbox_backend (before spawn)
                                         │ yes
                              spec representable? ── no (net/unix/child-fork on macOS) ──► unsupported.sandbox_backend
                                         │ yes
                              backend self-probe OK? ── no (e.g. Landlock ABI < 6) ──► unsupported.sandbox_backend
                                         │ yes
                                     spawn confined child; scope owns the tree; audit records backend+profile hash
```

| File (TASK-035) | Platform | v0.1.0 |
|---|---|---|
| `src/infra/sandbox_macos.rs` | `cfg(target_os = "macos")` | **Created and shipped.** Seatbelt through `/usr/bin/sandbox-exec`, the no-fork profile above, grants as `-D` parameters |
| `src/infra/sandbox_linux.rs` | `cfg(target_os = "linux")` | **Created.** Landlock V6 + seccomp + per-scope subreaper supervisor. Advertised in `rivet.capabilities` only after T-08 passes in Linux CI on kernel ≥ 6.12; otherwise every sandboxed spawn fails `unsupported.sandbox_backend` |
| `src/infra/sandbox_unsupported.rs` | `cfg(not(any(target_os = "linux", target_os = "macos")))`, including Windows | **Created.** Always `unsupported.sandbox_backend`, before spawn |
| `src/infra/sandbox_windows.rs` | Windows | **Not created in v0.1.0.** AppContainer + Job design recorded above for a later ADR |

`src/infra/process_adapter.rs` (the `ProcessRunner` adapter) selects exactly one of these with `cfg`. Stdio MCP
connectors (`src/infra/mcp_client.rs`) hand their `tokio::process::Command` to the same selector before rmcp's
`TokioChildProcess` takes it (RES-2026-0002 finding 4).

v0.1.0 child sandbox contract (all platforms that ship):
- **Allowed:** reads of the granted paths plus a fixed system allow-list, writes to the granted paths, exec from
  the granted executables, env from `allow_env`.
- **Refused:** network of any kind (including loopback), UNIX sockets and pipes outside grants, signals to
  others, and (macOS) forking.
- **Refused with `unsupported.sandbox_backend`:** a policy that grants a sandboxed child anything outside that
  set.
- **Future work:** a Rivet-owned loopback egress proxy (macOS `localhost:PORT` rule; Linux network namespace)
  would restore brokered network for children.

## Limitations and open items

- Linux and Windows are **build-verified only**. No kernel or Windows host was available here. The Linux claims
  come from kernel ABI documentation and the crate sources, and must be proven by T-08 before the backend is
  advertised.
- macOS leaks file **metadata** (existence, size, times) outside grants and **argv** of same-user processes.
  Rivet must keep secrets out of argv (it already does: secrets come from env or the credential store).
- `sandbox-exec` is deprecated. If Apple removes it, the macOS backend's probe fails and Rivet refuses. That is
  fail-closed. A `sandbox_init` FFI fallback is equally deprecated and is not planned.
- The macOS no-fork rule excludes wrapper launchers (for example `npx`) as stdio MCP servers under a sandbox.
  They still work unsandboxed where the policy does not require sandboxing.
- The T-08 conformance suite must port the 27 macOS cases above into `cargo test conformance_sandbox` and add
  the Linux equivalents (including `/proc/<pid>/environ` and `AF_UNIX` cases).

## Conclusion

OQ-1 is answered:
- **macOS** has a passing backend (27/27).
- **Linux** has a buildable, well-specified backend that must pass T-08 in CI before it is advertised.
- **Windows** has none in v0.1.0 and refuses sandboxed exec with `unsupported.sandbox_backend`.

No platform can enforce host-level network allow-lists for a child, so v0.1.0 sandboxed children have no
network.

## Related Documents

- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md) — Increment 5 "Enforcement limit", F-14, OQ-1, T-08
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — TASK-011, TASK-035, PF-25
- [ADR-0003](../decisions/adr-0003-process-sandbox-backends.md), [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md)
- [RES-2026-0002](res-2026-0002-rust-crate-feasibility.md) — crate selection, rmcp child-process seam

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | macOS Seatbelt prototype (27 cases), Linux/Windows backend sketches cross-built, capability matrix and v0.1.0 recommendation. |
