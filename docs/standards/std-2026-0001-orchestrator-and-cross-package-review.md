---
document_id: STD-2026-0001
title: "Orchestrator and cross-package implementation standard"
document_type: standard
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Codex]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [domain, features, io, infra, orchestrator]
affected_versions:
  from: "0.1.0-dev"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Define how orchestrator code composes Rivet dependencies and how reviewers detect logic or boundary violations across every package.
reason: The orchestrator currently combines dependency wiring with runtime, routing and lifecycle logic; this standard makes the intended boundaries explicit and prevents similar drift in other packages.
dependencies: [AGENTS.md, DOCUMENTATION.md, vhco-contract.json]
related_documents: ["ARCH-2026-0001", "REF-2026-0009"]
supersedes: null
superseded_by: null
tags: [standard, vhco, orchestrator, dependency-injection, review]
confidentiality: internal
review_cycle: on-architecture-change
next_review_date: 2026-10-29
---

# Orchestrator and cross-package implementation standard

This is a review standard for all Rust changes. It supplements `AGENTS.md`; VHCO validation remains authoritative for structural legality, while this document defines the stricter design expectation for maintainable code.

## 1. Boundary rule

The orchestrator is the composition root. Its primary job is to create concrete adapters, bind them to feature ports, register surfaces, and expose the assembled runtime.

```text
domain       vocabulary and ports; no internal imports
features     pure use cases; capabilities arrive as port arguments
io           protocol parsing and encoding only
infra        concrete I/O adapters and external edges
orchestrator composition, surface registration, lifecycle wiring, runtime façade
```

The orchestrator may import every bucket because it composes the application. The reverse is not allowed: features, IO and infrastructure must not import the orchestrator, and features must not construct infrastructure.

## 2. What belongs in the orchestrator

Allowed responsibilities:

- construct concrete adapters and shared state;
- pass adapters into feature functions or port-bearing structs;
- register interpreter, transport and surface adapters;
- assemble routers, CLI entrypoints, library façades and FFI handles;
- translate lifecycle events such as shutdown, cancellation and connection close;
- add tracing, metrics or access logging around a delegated operation;
- provide a small façade that forwards to an already-composed runtime.

The code should read like a dependency graph. A reviewer should be able to identify the concrete implementation of each port and the point where it is injected without following business rules through the surface code.

## 3. What does not belong in the orchestrator

Move or redesign these responsibilities when they become substantial:

- policy decisions, authorization rules or operation selection;
- domain validation that is shared by multiple surfaces;
- session rules, retry policy, output semantics or business branching;
- use-case sequencing that is not required by a protocol adapter;
- reusable request parsing or envelope semantics;
- concrete file, network, process, database or credential access;
- feature-specific behavior hidden behind a “builtins” or “runtime” helper.

Surface-specific decoding, response formatting and cancellation translation are appropriate. The surface must delegate after that translation; it must not reimplement the use case.

## 4. Dependency-injection rules

1. Every external capability is represented by a domain port or feature port.
2. A feature use case receives its ports as parameters; it never constructs an adapter.
3. Concrete adapters are created once in the composition root and shared explicitly with `Arc`, references or owned values according to their lifetime.
4. Closures used as adapter callbacks must be thin adapters to a named use case. They must not become anonymous business services.
5. If a runtime has meaningful alternate implementations or test doubles, expose a dependency bundle/factory rather than hard-coding every adapter inside a large constructor.
6. A test must be able to exercise a feature with fake ports without starting a listener or touching the filesystem/network.
7. Adapter edges (`file`, `net`, `db`, `env`, `arg`) are annotated on the adapter that performs them.

## 5. Thin-surface rules

Each `setup_<surface>` module should follow this shape:

```text
receive -> decode -> call use case/runtime façade -> encode -> return
```

It may own protocol concerns such as HTTP status mapping, CLI exit codes, WebSocket frame framing, MCP JSON-RPC envelopes, FFI handle validation, access logging and connection shutdown. It should not decide domain outcomes, duplicate authorization, or contain a second implementation of a feature.

Warning signs requiring review:

- a surface directly calls several feature use cases and branches on their domain results;
- the same validation or authorization condition appears in two surfaces;
- a handler reads files, opens sockets or creates processes directly;
- a surface has a large state machine unrelated to protocol framing;
- a “runtime” module contains policy semantics, catalog mutation and request execution in one file;
- a dispatcher selects business behavior by string IDs instead of delegating to a feature-level operation.

## 6. Cross-package review: inspect all consumers

Never review a changed package in isolation. For every change, inspect:

1. the changed module and its imports;
2. every caller and implementer of changed traits/functions;
3. every surface that exposes the behavior;
4. every adapter that supplies or consumes the affected port;
5. tests, examples, FFI shims and remote clients;
6. `vhco-contract.json`, generated `vhco.json`, README and relevant `docs/` pages.

Useful commands from the repository root:

```sh
rg -n "TraitName|function_name|operation/id" src tests ffi examples
rg -n "use crate::(domain|features|io|infra|orchestrator)" src
rg -n "vhco:(surface|trigger|usecase|port|infra|todo|test)" src ffi tests
rg --files src/features src/io src/infra src/orchestrator
```

For a changed port, inspect both directions: all feature callers and all infrastructure implementations. For a changed surface, inspect all sibling surfaces for duplicated behavior. For a changed domain type, inspect serialization, FFI, examples and fixtures.

## 7. Required review gates

Run the narrowest useful checks while editing, then the full gates before handoff:

```sh
vhco validate .
vhco sync .
vhco check .
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
python3 scripts/check_docs.py
```

Interpretation matters:

- `validate` checks structural VHCO rules;
- `sync` checks that code and the hand-authored contract agree;
- `check` checks declared application guarantees;
- Rust tests check behavior, not architecture;
- documentation checks ensure the human model is current.

Do not use `vhco spec . > vhco-contract.json` or `vhco sync . --update-spec` to silence a design mismatch. If the intended design changed, edit `vhco-contract.json` by hand first, obtain review, then implement the code.

## 8. Review questions

Before approving a change, answer yes to all applicable questions:

- Is the contract updated before implementation?
- Is each new use case pure and port-driven?
- Is the orchestrator only composing, translating protocol boundaries, or managing lifecycle?
- Can shared logic be found in one feature/domain location rather than copied across surfaces?
- Are all package consumers and implementations inspected?
- Are all external edges declared on their adapters?
- Does every use case have detailed todos, execution steps and a test?
- Are all surfaces registered and documented?
- Are `vhco validate`, `vhco sync`, `vhco check`, tests and docs green?
- Does README and `docs/` describe the current behavior?

## 9. Exception process

An orchestrator exception is acceptable only when the logic is inherently about composition, protocol lifecycle or resource ownership and moving it would obscure the boundary. Record the reason in a code comment or architecture document, keep the exception local, and add a test that protects the boundary. A large file, historical placement or convenience import is not by itself an exception.

