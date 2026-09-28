---
document_id: REF-2026-0001
title: "Rivet request and evidence"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 7
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Proposed Rivet behavior and design review; no implementation or release claim.
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet request and evidence

Source date: 2026-09-27. This is a requirements record, not an approval.

## Original user request

| ID | Request, preserved from this conversation |
|---|---|
| UQ-01 | "write a proposal for this project given /Users/oliverlaleau/Documents/projects/rivet/PROJECT.md" |
| UQ-02 | "dont want socket.close, thing should close in context" |
| UQ-03 | "want good error handling"; "clean and easy syntax" |
| UQ-04 | "can CRUD file" |
| UQ-05 | "should be auditable and traceable so we should have commands to see everywhere that can do IO opterations" |
| UQ-06 | "support mcp connector to connector mcp" |
| UQ-07 | "allow all apis to be reached from same interface such that we can access items with cli, http, library" |
| UQ-08 | "we can have something like request(ID, PARAMS, on_data)"; reference to `internal/functions/function.go` |
| UQ-09 | "must support usage as library" |
| UQ-10 | "use rust and capy (https://github.com/olivierdevelops/capy) for parsing" |
| UQ-11 | "be allow be be used as a dag for more complex flows" |
| UQ-12 | "give the best possible usage and syntax and cosider all use cases"; "write areference docs to show 50+ samples" |
| UQ-13 | "must have a sandbox option where if passed, all IO operation fail unless specified"; `--sandbox "allow_read, allow_write, allow_network, ..."` — **status (2026-09-28):** the deny-by-default *behaviour* is retained; the `--sandbox "..."` grant-string *syntax* is superseded by [UQ-17](#policy-outputs-and-unified-serve-request) (policy comes only from `policy.json`). |

## Follow-up request

| ID | Source and date | Request and scope |
|---|---|---|
| UQ-14 | User conversation, 2026-09-28 | "add support for udp, oath2.0, quic". Interpret `oath2.0` as OAuth 2.0, confirmed by the preceding OAuth discussion. The preceding QUIC discussion included native streams, datagrams and HTTP/3. Update the current proposal/contract/reference; no implementation approval is inferred. |

Revision 2 adds R15–R18, concrete protocol and auth contracts, and S81–S102. UDP was already loosely staged; it is now required Stage B scope alongside OAuth, QUIC and HTTP/3.

Protocol baselines consulted on 2026-09-28: [UDP RFC 768](https://www.rfc-editor.org/rfc/rfc768.html), [OAuth RFC 6749](https://www.rfc-editor.org/rfc/rfc6749.html), [PKCE RFC 7636](https://www.rfc-editor.org/rfc/rfc7636.html), [OAuth security RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html), [device authorization RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html), [QUIC RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html), [QUIC TLS RFC 9001](https://www.rfc-editor.org/rfc/rfc9001.html), [QUIC DATAGRAM RFC 9221](https://www.rfc-editor.org/rfc/rfc9221.html), [HTTP/3 RFC 9114](https://www.rfc-editor.org/rfc/rfc9114.html). These specify protocols; Rivet syntax, defaults, lifecycle and sandbox bindings are proposed project decisions, not claims made by the RFCs.

## gRPC and operation catalog request

| ID | Source and date | Request and scope |
|---|---|---|
| UQ-15 | User conversation, 2026-09-28 | "add grpc"; "we need to add another access point, mcp"; "1 file may contain many opertations"; "each operation can define name, params + description"; "we can call them through mcp, http, cli, ...". Preserve the existing Rust library access too. Update the design artifacts; no implementation approval is inferred. |

MCP was already a named surface; revision 3 strengthens it to required direct publication of described operations as tools. R19–R22 specify gRPC, file/catalog metadata, incoming MCP and streaming access. S103–S120 add 18 examples.

Sources consulted: [gRPC concepts](https://grpc.io/docs/what-is-grpc/core-concepts/), [status codes](https://grpc.io/docs/guides/status-codes/), [ProtoJSON mapping](https://protobuf.dev/programming-guides/json/), [MCP tool schemas/names](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), and [MCP transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports). Rivet syntax, catalog defaults, session delivery and sandbox choices are proposed project contracts rather than behavior mandated by these protocols.

## Sample folder request

**UQ-16 — user, 2026-09-28:** “can you write some samples files in folders with readmes to show usage”.

The [sample corpus](../demos/README.md) materializes the already-proposed interface in twelve independent folders. This extends R14 documentation coverage with physical source/fixture assets, not runtime implementation or design approval.

## Policy, outputs and unified serve request

**UQ-17 — user, 2026-09-28** (verbatim, including typos):

> operations must be able to define their outputs as well such that we can have commands to view them
> use a po;icy.json file to set policy instead of args
> rivet serve should serve with http, mcp, polling, ws, ... access at at once
> also, fix the gaps you found

Interpretation (recorded here; the proposal turns it into R23–R25):

```text
 UQ-17 line                         interpretation                                  proposal
 ---------------------------------  ----------------------------------------------  -----------------
 (a) define outputs + view them  -> declared, described output schemas plus         R23, UC-19
                                    inspection commands (rivet outputs, describe,
                                    HTTP /v1/operations/{id}/outputs, MCP)
 (b) po;icy.json instead of args -> policy comes only from a JSON file, never from  R24, UC-08
                                    grant strings on the command line
 (c) serve http, mcp, polling,   -> one `rivet serve` listener exposes REST, SSE,   R25, UC-20
     ws ... at once                 polling, WebSocket and MCP simultaneously
 (d) fix the gaps you found      -> apply the design-review fixes (licence gate,     E1-E16 in
                                    MCP canonical form, sandbox scoping, SSRF,       revision 5
                                    DAG semantics, error registry, syntax ...)
```

Relationship to UQ-13:

```text
 UQ-13 (2026-09-27)                         UQ-17 (2026-09-28)
 +--------------------------------------+   +--------------------------------------+
 | behaviour: all I/O fails unless      |==>| KEPT: no policy.json => deny-by-      |
 |            specified                 |   |       default for new application I/O |
 | syntax:    --sandbox "allow_read,..."|X  | SUPERSEDED: --sandbox removed;        |
 |                                      |   |       grants live in policy.json only |
 +--------------------------------------+   +--------------------------------------+
```

"po;icy.json" is read as `policy.json` (adjacent-key typo). "at at once" is read as "all at once". R22's
live polling/WebSocket access now also traces to this request. No implementation approval is inferred.

## I/O manifest request

**UQ-18 — user, 2026-09-28** (verbatim, including typos):

> we should have a way to generate all io and what they use
> like url, path
>
> file permissions like read, write, delete, et...

Interpretation (recorded here; the proposal turns it into R26, UC-21 and UC-22): `rivet io` gets a fully
specified, generated **I/O manifest**: every I/O site, the concrete target it uses (URL, path, host:port, argv,
env var, connector method…), and the fine-grained **access** it performs (read, write, delete, …), mapped to the
policy capability that permits it. The manifest can be viewed by target, by operation or by capability,
exported, checked against `policy.json`, and turned into a least-privilege policy draft.

```text
 UQ-18 phrase                         interpretation                                   proposal
 -----------------------------------  -----------------------------------------------  -----------------
 "generate all io"                 -> generated IoManifest of every effect site          R26, UC-21
                                      (`rivet io`, JSON / table / markdown / csv)
 "what they use / like url, path"  -> normalized target per site: URL template,          R26, UC-21
                                      host:port, path + derived glob, argv, env var,
                                      connector method; `--by target` view
 "file permissions like read,      -> access verbs per site (read, list, stat, watch,    R26, UC-21
  write, delete, et..."               create, update, append, delete, connect, bind,
                                      exec, call, ...) mapped to allow_* capabilities
 (implied: act on it)              -> `io --check-policy` against policy.json and        R26, UC-22
                                      `rivet policy generate` least-privilege draft
```

```text
 UQ-05 (2026-09-27)                         UQ-18 (2026-09-28)
 +--------------------------------------+   +--------------------------------------+
 | "commands to see everywhere that     |==>| KEPT: R6 / UC-09 static inventory     |
 |  can do IO"                          |   |       and runtime trace               |
 |                                      |   | REFINED: output contract = IoManifest |
 |                                      |   |       (targets + access verbs +       |
 |                                      |   |       capability + policy decision)   |
 +--------------------------------------+   +--------------------------------------+
```

"et..." is read as "etc." — the access vocabulary covers every capability, not only file verbs. R6 and UC-09
remain; R26 refines their output contract. The generated policy is a draft for human review, never an
approval. No implementation approval is inferred.

## Evidence and provenance

- [PROJECT.md](../../PROJECT.md), all 84 sections: original protocol and pipeline design. It is an input brief, not a shipped language manual.
- [AGENTS.md](../../AGENTS.md): contract-first workflow, architecture and proposal requirements. Its project-facts block was inherited template text describing a Go `email_provider` module; it was never evidence of an existing email application. PLAN-2026-0001 TASK-006 replaced it on 2026-09-28 with Rivet's facts (Rust module `rivet`, 14 features, six surfaces).
- [DOCUMENTATION.md](../../DOCUMENTATION.md), revision 4, §§4.2, 12.1, 21: authoritative proposal structure and traceability.
- Local Go reference snapshot (`.ignore/references/ai_manager/function.go`), copied from the exact user-supplied path on the source date. `Function.Execute` validates input, iterates output and observes callback stop. Input filtering precedes unknown-field validation; output validation is a stub; an empty filtered frame can be skipped. Rivet should preserve streaming/cancellation ergonomics while defining strict validation and visible completion.
- Capy source snapshot (`.ignore/references/capy/rust/src/capy.rs`), commit `84f984c64e0811ef2bfff7835167d6630422ecaa`; embedding guide (`.ignore/references/capy/docs/embedding.md`); lexical reference (`.ignore/references/capy/docs/language-reference.md`). The public `Library::parse` returns a recovering tree, diagnostics and spans. Rivet must check `is_clean()` before lowering. Four-space indentation, object values and library-defined shapes fit the proposed DSL; complete grammar feasibility still needs a spike.
- [Upstream Capy repository](https://github.com/olivierdevelops/capy/tree/84f984c64e0811ef2bfff7835167d6630422ecaa). Local snapshots are discovery evidence only, excluded from the Rivet code model by their dot-directory location.
- **Capy licence (approval gate G-LIC) — closed 2026-09-28 by owner statement.** The root LICENSE (`.ignore/references/capy/LICENSE`) and crate LICENSE (`.ignore/references/capy/rust/LICENSE`) are a *Capy Source-Available License (v1)* whose terms forbid bundling, commercial use and derivative works, while the crate manifest (`.ignore/references/capy/rust/Cargo.toml`) declares `license = "MIT"`. Under the LICENSE text alone, Rivet could not depend on or redistribute Capy, so the proposal gated implementation on **G-LIC**. The project maintainer owns Capy and closed the gate on 2026-09-28 with the statement *"use git, i own capy so dont worry — everything else is approved"*, recorded in [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md). The licence holder has authorized Rivet to depend on and ship Capy. Aligning the upstream `LICENSE` text with the manifest is the owner's own housekeeping and no longer blocks Rivet.

```text
 LICENSE (source-available) --conflicts--> Cargo.toml (MIT)
            |                                     |
            +---- owner authorizes use in Rivet --+--> G-LIC closed 2026-09-28 (ADR-0001)
                  ("i own capy so dont worry")          |
                                                        +--> upstream LICENSE cleanup: owner housekeeping, not a gate
```

- MCP reference baseline: [transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), [resources](https://modelcontextprotocol.io/specification/2025-11-25/server/resources), [prompts](https://modelcontextprotocol.io/specification/2025-11-25/server/prompts). Pin and negotiate this version initially; do not call it the latest version.
- [Tokio JoinSet](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html): task ownership is useful for scope joins; resource protocol shutdown remains Rivet's responsibility.

## Missing baseline artifacts

At discovery there was no Cargo manifest, runtime source, README, docs tree, contract, release or Git repository. The CUDA proposal and implemented documentation example named in AGENTS.md were absent. The proposal follows the available authoritative documentation template and adds the AGENTS-specific sections. No implementation or measured performance is asserted.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-28 | Claude | Licence evidence: G-LIC closed by the Capy owner's statement (ADR-0001); AGENTS.md project-facts note updated after TASK-006. |
| 6 | 2026-09-28 | Claude | Recorded UQ-18 verbatim with interpretation (generated I/O manifest with targets and access verbs, policy check and least-privilege policy draft; R26, UC-21, UC-22) and its relationship to UQ-05. |
| 5 | 2026-09-28 | Claude | Recorded UQ-17 verbatim with interpretation; marked UQ-13 flag syntax superseded (deny-by-default kept); corrected the Capy licence note to a real source-available/MIT conflict gated by G-LIC. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Recorded request and inspected sources. |
