---
document_id: RES-2026-0001
title: "Capy grammar spike (G-SPIKE)"
document_type: research
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language]
affected_versions:
  from: not-applicable
  to: "0.1.0"
scope: Prove that the pinned Capy commit can parse every Rivet example in the approved design, and fix how Rivet consumes Capy's output.
reason: PLAN-2026-0001 TASK-010 / PROP-2026-0001 gate G-SPIKE.
related_documents: [PROP-2026-0001, PLAN-2026-0001, REF-2026-0002, ADR-0001]
supersedes: null
superseded_by: null
tags: [rivet, capy, grammar, spike]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Capy grammar spike (G-SPIKE)

> **Status:** Completed — **G-SPIKE PASS**
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language

## Summary

Capy at commit `84f984c64e0811ef2bfff7835167d6630422ecaa` (capy-core 0.22.0), driven by a 111-function
`rivet.capy` library, parses **119 of 119** Rivet source blocks cleanly. The blocks are:
- the 106 `rivet` code blocks of REF-2026-0002, which contain all 159 numbered examples plus the unnumbered variants;
- the 12 `docs/demos/*/app.rivet` files;
- the 13 blocks of PROP-2026-0001.

A structural check over every tree found zero problems: no stray `end`, every `catch` pairs with a `try`, and
every body is indented under its parent. Invalid sources are rejected. Rivet consumes Capy only through
`Library::parse`, then the **versioned AST JSON** (`capy_core::domain::ast_json::to_json`, `schema_version: 1`),
so no Capy-internal Rust types leak into Rivet.

```text
 .rivet source ──► Library::parse (rivet.capy) ──► ParseResult ──► ast_json::to_json  (schema_version 1)
                                                                        │
                                                    serde ──► Rivet SyntaxTree (spans, func, captures)
                                                                        │
                                              Rivet lowering: option tails, value expressions, pairing
                                                  (try+catch), semantic checks ──► typed IR
```

## Question

Can the pinned Capy commit express every statement shape in the approved design? Specifically: prefix calls,
quoted durations, `${dotted.path}` strings, nested `output`/`field` blocks, `with`/`dag`/`concurrent`/`task`
blocks, `try`/`catch`, method-call statements, assignments with option blocks, and multi-line object literals.
It must also report invalid input with source positions.

## Method

1. Cloned `https://github.com/olivierdevelops/capy` and checked out the pinned commit.
2. Wrote the spike harness ([harness_main.rs](res-2026-0001-capy-grammar-spike/harness_main.rs)). It compiles
   `rivet.capy` with `Library::new`, parses every corpus block with `Library::parse`, and reports diagnostics and
   `CLEAN n / total`. With `ALL` it prints each tree as AST JSON.
3. Extracted the corpus with [extract_corpus.py](res-2026-0001-capy-grammar-spike/extract_corpus.py).
4. Iterated [rivet.capy](res-2026-0001-capy-grammar-spike/rivet.capy) until the corpus was clean, then ran a
   structural walker over every AST and a [negative corpus](res-2026-0001-capy-grammar-spike/negative-corpus.txt).

Reproduce:

```sh
git clone https://github.com/olivierdevelops/capy capy-src && git -C capy-src checkout 84f984c64e0811ef2bfff7835167d6630422ecaa
cargo new spike && cp docs/research/res-2026-0001-capy-grammar-spike/harness_main.rs spike/src/main.rs
# spike/Cargo.toml:  capy-core = { path = "../capy-src/rust" }
python3 docs/research/res-2026-0001-capy-grammar-spike/extract_corpus.py corpus.txt
cargo run --manifest-path spike/Cargo.toml -- docs/research/res-2026-0001-capy-grammar-spike/rivet.capy corpus.txt
# expected last line: CLEAN 119 / 119
```

## Results

| Check | Result |
|---|---|
| Positive corpus (119 blocks) | **CLEAN 119 / 119** |
| Structure walk: stray `end`, orphan `catch`, `try` without `catch`, child not indented under parent | 0 problems |
| Negative: keyword typo `operaton` | Rejected (E0001 at 1:1) |
| Negative: unknown statement `frobnicate the thing` | Rejected (2:5) |
| Negative: missing `end` | Rejected ("expected closer end") |
| Negative: bare `return` | Rejected |
| Negative: unclosed `{` | Rejected |
| Negative: f-call style `x = request("a", {…})` | **Parses**: `(…)` lands in the assignment's trailing `rest`. Rivet lowering must reject it (`syntax.fcall_style`, with a prefix-call hint) |
| Negative: 2-space indent | **Parses**: the lexer accepted it. Rivet must add its own indentation check (`syntax.indent`) |

Statement functions used by the corpus (top 10):

```text
return 153 · operation 83 · description 75 · param 61 · output 57 · field 57
with 53 · name 53 · assign_block 46 · assign 44 · … 111 functions defined
```

## Findings that shape the implementation

1. **Use AST JSON, not internal types.** `ParseResult.tree` is a `domain::ast::Block`, an internal type. The
   AST JSON schema is a documented, versioned contract: fields may be added without a version bump, and
   removals bump `schema_version`. `capy_parser.rs` serializes with `ast_json::to_json` and deserializes into
   Rivet's own `SyntaxTree` with serde. It rejects any `schema_version` other than 1.
2. **Capture `text` is normalized, not raw.** `{order_id: x}` comes back as `{"order_id": x}`. Rivet must slice
   the **original source by the capture span** (1-indexed, `end_col` exclusive, multi-line allowed) to keep
   exact text for diagnostics and for its expression parser.
3. **Rivet owns value-expression parsing.** Capy identifies where an expression begins and ends
   (`is_expr: true`). Rivet parses the sliced text into its IR: literals, lists, objects, dotted paths, prefix
   calls, infix `+ - * / % == != < <= > >= and or`, and `not`. The grammar is small and mirrors Capy's value
   grammar.
4. **Option lines carry a `tail`.** 60 option keywords (`decode`, `timeout`, `framing`, `retry`, `tls`, …) are
   declared individually. Unknown keywords still fail inside Capy, but their arguments arrive as a `tail`.
   Rivet validates each tail against the per-resource option table (REF-2026-0002 §options), including the
   quoted-duration rule and the leading-options rule.
5. **`try`/`catch`.** Capy's `block_sections` cannot put arguments on a section header (`catch error kind
   http`), and a `block_closer` target cannot open its own body. Final shape: `try` is `block_dedent` and
   `catch <filter>` is a sibling block closed by `end`. Rivet lowering pairs them; a `try` without an
   immediately following `catch` is `syntax.try_without_catch`. The surface syntax is unchanged.
6. **Custom `pattern` types are not enforced at parse time** in this commit, so shape constraints use literal
   tokens instead. For example, method calls are `ident "." dotted_ident tail`, which correctly rejects
   `foo bar`.
7. **One-line vs block forms** of the same keyword need `when_followed_by indent` /
   `when_not_followed_by indent` on **both** variants. Without the guard, the one-line form wins and leaves a
   stray `end`.
8. **Diagnostics wording.** On an unknown keyword, Capy reports the furthest attempt (for example "expected `+=`,
   `.`, or `=` … in `append_assign`"). Rivet's diagnostic layer rewrites an E0001 at a statement's first token
   into "unknown statement `X`", with a did-you-mean from the function list. It keeps Capy's span and code as
   the cause.

9. **Priority matters for keyword reuse (found during implementation).** Option keywords such as `message`
   and `stream` are also legal variable names. With equal priority, `message = socket.receive_from json` matched
   the `message` option with a tail of `= …`. Assignments, `+=` and method calls now carry `priority 10`, above
   every option line. This is safe because option lines never have `=`, `+=` or `.` as their second token.
   Structural checks alone did not catch it; lowering did.

## Conclusion

**G-SPIKE: PASS.** The design needs no grammar change. The eight findings above become implementation
requirements for TASK-016 (`capy_parser.rs`, `rivet.capy`) and TASK-017 (`compile_program`), and T-23 gains
cases for `syntax.fcall_style`, `syntax.indent` and `syntax.try_without_catch`. The spike grammar is the seed
for `src/infra/rivet.capy`.

## Related Documents

- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md): Increment 1 and gate G-SPIKE
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md): TASK-010, TASK-016, TASK-017
- [REF-2026-0002](../references/ref-2026-0002-language-and-usage.md): the example corpus
- Capy docs at the pinned commit: `docs/ast-json.md`, `docs/diagnostics.md`, `docs/block-functions.md`

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | Finding 9 (keyword reuse needs priority) recorded from implementation. |
| 1 | 2026-09-28 | Claude | Spike executed: 119/119 clean (re-run after S154–S159), structure 0 problems, negatives reviewed; eight implementation findings. |
