# 9 · Codebase intelligence — optional `.vhco.json` layers

[← Commands](08-commands.md) · [wiki index](README.md)

Beyond modeling, the tool can **enforce code policy and capabilities** from `.vhco.json`. All of this is
**optional** — add only the blocks you want, then run `vhco scan .`. None of it changes `validate`
(architecture rules) or `sync` (design drift); it is a **separate, code-hygiene / capability layer**. A
project with no `.vhco.json` blocks is simply *described, not constrained* — the descriptive-by-default
posture holds, and a first run never fails for lack of config.

> **Three layers, three jobs — don't conflate them.**
> - `vhco validate` → the **architecture** (five folders, closed-world imports, per-surface registration).
> - `vhco sync` → the **design** (does the code match `vhco-contract.json` / `vhco.json`?).
> - `vhco scan` / `vhco sandbox` → **code policy & capabilities** (this page). Driven by `.vhco.json`.
> A green `validate` says nothing about your `scan` rules, and vice versa.

```jsonc
// .vhco.json — every block is optional; add only what you need
{
  "comments": { ".myext": "//" },              // custom comment symbol for an extension

  "extract":  {                                // lexical extractors → located signals
    "todo-markers": { "match": ["TODO", "FIXME"], "tags": ["debt"] }
  },

  "rules": [                                   // scoped assertions over the code
    { "name": "no-eval", "assert": "deny",      "match": ["eval("],       "in": ["features/**"], "why": "no dynamic exec in business logic" },
    { "name": "license", "assert": "require",   "match": ["SPDX-License"], "in": ["**/*.go"] }
  ],

  "conventions": {                             // every listed edge kind must be annotated
    "document-edges": { "require": ["net", "db"], "severity": "error" }
  },

  "sandbox": {                                 // capability manifest — what the program is ALLOWED to touch
    "deny-undeclared": true, "on-violation": "error",
    "allow": { "net": ["api.example.com"], "file": ["~/.app/**"], "db": ["*"] }
  }
}
```

`vhco scan .` runs **all** of these in one pass: extractors → signals, rules → findings, edge detection
(annotated + inferred), the documentation convention, and the sandbox check. `vhco sandbox .` is a
separate command that *compiles* the `sandbox` block into a runtime profile (it does not check the code).

---

## The model behind it (so the blocks make sense)

The intelligence layer reasons about the program's **edges** — the places the code touches the outside
world — each pinned to a `file:line` and tagged with a **provenance**:

- **annotated** — a human (or the LLM that wrote the code) declared it with a `vhco:` edge comment.
- **inferred** — vhco's detectors recognized a well-known sink (e.g. `os.Getenv` → `env`, `http.*` →
  `net`) and emitted a *candidate* edge. Honest about being a guess.

Edge kinds you'll see: `env`, `arg`, `file`, `net`, and (where modeled) `db`, `exec`, and more. The
extractors and rules below are **lexical** (pattern-based) and so are also best-effort/inferred — never
trust them as complete; that's why everything carries provenance.

---

## What each block does

### `comments` — custom comment symbols

Maps a file extension to the comment marker vhco should look for `vhco:` directives in. The built-in
symbols cover common languages; add this only for an extension vhco doesn't know.

```jsonc
{ "comments": { ".myext": "//", ".cfg": "#" } }
```

### `language` / `dirs` — make `validate` work for any language

Modeling is language-agnostic, but `vhco validate`'s two code-reading checks — **registration**
(`orchestrator/setup_<surface>.<ext>`) and the **import-boundary** check — are built in only for
Go/Python/Rust/TypeScript. Teach it any other language (Dart, Kotlin, Swift, …) here — **config-driven, no
tool change** — and that language becomes a first-class VHCO project.

```jsonc
{
  "language": {
    "name": "dart",                                        // routes off the built-in Go path; any label
    "ext":  ".dart",                                       // use-case files + setup_<surface>.dart
    "import": "^\\s*import\\s+['\"](?:package:[^/]+/)?(?:\\.\\./)*(?:lib/)?(domain|features|surfaces|infra|orchestrator)/"
  },
  "dirs": { "io": "surfaces" }
}
```

- **`language.ext`** — the source extension; used for use-case files and the `setup_<surface>.<ext>`
  registration convention.
- **`language.import`** — a regex whose **first capture group is the imported bucket**; the closed-world
  boundary check reads it to catch e.g. a `features/` file importing `infra`. (The bucket name it captures is
  the on-disk folder name — `surfaces` resolves to the logical `io` bucket.)
- **`language.name`** — any label except `go` (which has a dedicated toolchain path); `language.srcRoot` — a
  sub-path holding the buckets if not the project root.
- **`dirs`** — rename a **logical** bucket's on-disk folder (independent of language). `{ "io": "surfaces" }`
  puts the io bucket in `surfaces/` (Python's stdlib owns the name `io`; Dart projects often do the same).
  Absent config, the io bucket auto-resolves to `surfaces/` when that folder exists and `io/` doesn't — a
  general rule, not a per-language special case.

This is separate from `validate`/`sync` semantics — it only tells the enforcement layer *how* to read your
language. ⛔ Don't fake another language (a `setup_*.py` stub in a Dart project) to force `validate` green.

### `extract` — match code with regex, tag it, locate it

This is the layer for **"find parts of the code with a pattern (regex or literal), tag/label them, and get
each hit located with its `file:line`."** Each named entry under `extract` is one **extractor**; it scans the
source line by line and emits one **signal** per match. The map key is the signal's **category**.

```jsonc
{
  "extract": {
    "secrets":   { "match": ["api_key", "password", "/AKIA[0-9A-Z]{16}/"], "tags": ["secret"] },
    "tech-debt": { "match": ["TODO", "FIXME", "HACK", "XXX"], "tags": ["debt"] },
    "risky":     { "match": ["os/exec", "eval(", "unsafe."], "tags": ["external", "destructive"], "in": ["features/**"] },
    "handlers":  { "match": ["/func (?P<name>\\w+)Handler\\(/"], "kind": "symbol", "capture": "name", "after": 2 }
  }
}
```

**Every field of an extractor block:**

| Field | Type | What it does |
|---|---|---|
| `match` | `[]string` | the patterns. Each is a **literal substring**, or a **`/regex/`** when wrapped in slashes — a Go [RE2] regular expression, named groups allowed: `(?P<name>…)`. An invalid regex is silently skipped (it never fails the whole scan). |
| `tags` | `[]string` | labels attached to **every** hit, so it feeds the same risk picture as edges (e.g. `secret`, `external`). This is how you "tag parts of the code." |
| `kind` | `string` | a free label put on each signal's `Kind` (e.g. `symbol`, `import`, `keyword`) — use it to say *what sort of thing* the category collects. |
| `capture` | `string` | the name of a regex group whose text becomes the signal's **name**. With `(?P<name>\w+)Handler` + `"capture": "name"`, a match of `loginHandler` is named `login`. Without a capture, the signal's name is the whole matched text. |
| `after` | `int` | grab **N lines after** the match as a context snippet (so a reader/agent sees a declaration without opening the file). `0`/omitted = no snippet. |
| `in` | `[]string` | **scope** — only search files matching these globs. Omit to search the whole tree. Tighten this to kill false positives. **Glob rule:** a pattern is tested against both the file's full relative path **and** its basename; `*` stays within a path segment, `**` crosses `/`. So `*.go` matches Go files **anywhere** (root + nested), `**/*.go` matches only files **at least one directory deep** (it skips root-level files), and `features/**` matches everything under `features/`. |

[RE2]: Go's regexp syntax — no backreferences, linear-time; `(?P<name>…)` for named groups, `(?i)` for
case-insensitive, etc.

**Each match becomes one located signal** carrying: `category` (the block key), `kind`, `name` (the capture
or the matched text), `value` (the matched line), `tags`, the `after`-window snippet, and `file:line`.

**Literal vs regex — quick rules:**
- `"eval("` → matches any line **containing** `eval(`.
- `"/AKIA[0-9A-Z]{16}/"` → regex (slashes) — an AWS-key shape.
- `"/(?i)todo/"` → case-insensitive `todo`.
- `"/func (?P<name>\\w+)Handler\\(/"` → regex with a **named capture**; pair with `"capture": "name"`. (In
  JSON, backslashes are escaped: `\\w`, `\\(`.)

**What scan reports for `extract`:** a count of signals **by category** — e.g. `● 12 signal(s):` then
`secrets 3`, `tech-debt 8`, `handlers 5`. With `--json`, each signal is a full located record (category,
kind, name, value, tags, file, line, snippet). Extractors alone *surface* things; they don't fail the build
unless you attach a **rule** (next) — which is literally an extractor pattern plus an assertion.

### `rules` — scoped assertions (the linter)

A rule is an extractor pattern plus an **assertion**. Findings carry `file:line`, the rule name, the
message, and the `why`. Each rule takes:

- `name` — the rule id (shown in output and SARIF `ruleId`).
- `assert` — one of the four types below.
- `match` — the literal(s)/regex(es) to look for.
- `in: [<globs>]` — the path scope (anchoring a rule to a folder is what turns a lint into an
  *architectural fitness function*).
- `why` — the human explanation shown with the finding.
- (severity defaults to error; warn-only findings still exit 0.)

**The four `assert` types, each with an example:**

| `assert` | Meaning | Example |
|---|---|---|
| `deny` | the pattern **must not** appear in scope | ban `eval(` under `features/**` |
| `allow-only` | the pattern may appear **only** in the given scope; flagged everywhere else | `unsafe.` only in `infra/lowlevel/**` |
| `require` | every scoped file **must** contain the pattern | a license header in `**/*.go` |
| `max <n>` | at most *n* hits across scope | a TODO budget of `max 50` |

```jsonc
{
  "rules": [
    { "name": "no-eval",      "assert": "deny",       "match": ["eval(", "exec("], "in": ["features/**"], "why": "use cases must not run dynamic code" },
    { "name": "unsafe-quarantine", "assert": "allow-only", "match": ["unsafe."], "in": ["infra/lowlevel/**"], "why": "unsafe is quarantined to one folder" },
    { "name": "must-authorize", "assert": "require",    "match": ["authorize("], "in": ["io/*/handler.*"], "why": "every handler must authorize" },
    { "name": "debt-budget",  "assert": "max 50",     "match": ["TODO"], "why": "keep tech debt bounded" }
  ]
}
```

**What scan reports for `rules`:** `N finding(s):` then one line per finding —
`✗ [no-eval] features/x.go:42 — denied pattern "eval(" (use cases must not run dynamic code)`. `✗` =
error (exit 1), `⚠` = warn (exit 0).

### `conventions` — the documentation convention (legibility, not restriction)

A convention **requires legibility**: *if you touch the network/db/etc., you must annotate it.* It forbids
nothing — it just makes every edge mandatory to declare, so the model can never silently miss one.

```jsonc
{
  "conventions": {
    "document-edges": {
      "require":  ["net", "file", "exec", "db", "env"],  // these interactions must be annotated
      "in":       ["features/**", "io/**", "infra/**"],
      "severity": "error"                                // fail the build until it's documented
    }
  }
}
```

**What scan reports:** for every *detected* edge of a required kind that has **no matching annotation** on
its enclosing function, a finding — e.g. *"features/sync/push.go:42 — network call is undocumented; add a
`vhco:net` describing the endpoint and purpose."* The fix is to annotate the edge, not to remove it. This
is the durable cure for coverage decay: not "we measured 84%" but "100% of detected edges are documented,
or the build is red."

### `sandbox` — the capability manifest (restriction)

The manifest names the edges the program is **allowed** to have. Detection finds the edges it **actually**
has. The set-difference is the sandbox:

```
violations  = detected_edges − allowed_edges     # capability taken but not granted
dead_grants = allowed_edges − detected_edges      # capability granted but unused (prune it)
```

```jsonc
{
  "sandbox": {
    "deny-undeclared": true,        // any detected edge with no matching annotation = violation
    "on-violation": "error",        // error (CI gate) | warn (advisory)
    "allow": {
      "net":  ["api.stripe.com", "*.svc.internal"],   // egress allow-list; anything else flagged
      "file": ["./config/**", "~/.app/**"],           // paths the program may touch
      "db":   ["*"],                                   // tables it may access
      "env":  ["APP_*", "PATH", "HOME"],
      "arg":  ["*"]
    }
  }
}
```

**What scan reports:** any detected/annotated edge **outside** the `allow` lists is a violation with its
`file:line`. With `deny-undeclared: true`, any detected edge with no matching annotation is *also* a
violation (it forces every side effect to be explicit). `on-violation: warn` makes it advisory (exit 0)
while you tune; `error` gates the build.

**Trust boundary (read this):** this is a **static, advisory** check, not a runtime sandbox. It is only
meaningful when the **manifest is authored by a more trusted party** than the code (a human, a protected
branch, a governance repo) and the **code is semi-trusted** (AI- or contributor-generated). If the same
party writes both the code and its annotations, the manifest is worthless. It does not confine a running
process — for that, *generate* a runtime profile from the same manifest (next).

---

## Running it

```sh
vhco scan . [--sarif|--json]                          # extract + rules + edge detection + conventions + sandbox check
vhco sandbox . --profile summary|netpolicy|seccomp    # compile the sandbox manifest into a runtime profile
```

- `vhco scan .` — the one-pass **check**. Text by default; `--sarif` for CI/security dashboards; `--json`
  for agents. Exit code is non-zero when any error-severity finding is present.
- `vhco sandbox . --profile <kind>` — **compiles** the `sandbox` manifest; it does not check the code.
  - `summary` (default) — a human-readable summary of the allowed capabilities.
  - `netpolicy` — a network egress allow-list shape (from the `net` allow entries).
  - `seccomp` — a seccomp-style syscall profile (drops syscalls the manifest doesn't grant).
  - With no `sandbox` block it prints `no sandbox manifest — add a "sandbox" block to .vhco.json`.

The two halves reinforce each other: the **static** `scan` keeps the source honest and the manifest
current; the **generated** runtime profile enforces it against code that lies or that detection couldn't
fully see.

---

## Sidecar edges for code you can't annotate

**`.vhco.overlay.json`** adds **sidecar edges** for vendored / generated / third-party code you can't put
comments in — a **zero-touch** way to complete the "What it touches" picture without editing the source.
The scanner merges overlay edges on top of whatever it finds in-source, so the program can be fully modeled
while the repository's source stays byte-for-byte unchanged. Overlays can even live in a separate repo, so
you can map a dependency you don't own.

---

## Common confusion

- **`scan` ≠ `validate`.** `validate` enforces the *architecture* (folders, imports, registration) and
  reads no `.vhco.json` policy. `scan` enforces *code policy / capabilities* from `.vhco.json`. Both can
  exit non-zero; they check different things.
- **`scan` ≠ `sync`.** `sync` compares code to a design *spec*. `scan` has nothing to do with the design;
  it lints patterns and checks edges against a manifest.
- **`scan` vs `sandbox`.** `scan` **checks** the manifest against the code. `sandbox` **compiles** the
  manifest into a runtime profile — it doesn't inspect the code at all.
- **`conventions` vs `sandbox`.** A convention *requires every edge to be declared* (legibility — forbids
  nothing). The sandbox *restricts which declared edges are allowed* (capability budget). Documentation
  first, restriction second. They are often used together.
- **inferred ≠ confirmed.** Extractor signals and detected edges are best-effort/`inferred`. A capability
  check over low coverage is a *weak* check — the tool says so rather than imply a clean bill of health.
- **`extract` finds; `rules` enforce.** An `extract` entry only surfaces signals. To fail the build, write
  a `rules` entry with an `assert`.

---

## FAQ

**Do I need any of this to use vhco?**
No. The whole layer is opt-in. Modeling, `validate`, `sync`, `doc`, `flow`, `query`, etc. all work with no
`.vhco.json` policy blocks at all.

**Where do findings show up in CI?**
Run `vhco scan . --sarif` and upload the SARIF — findings appear in GitHub code-scanning like any linter.
Each finding carries `ruleId`, `level` (error/warning), the message + `why`, and the `file:line`.

**What's the difference between a `rules` deny and a `sandbox` deny?**
A `rules` `deny` bans a **textual pattern** (e.g. `eval(`) in a scope. The `sandbox` denies an **edge/capability**
(e.g. a network egress to an un-allowed host). Rules are about *code smells*; the sandbox is about *what
the program touches*.

**How do I allow a network call to exactly one host?**
`"sandbox": { "allow": { "net": ["api.stripe.com"] } }` — everything else is flagged. Annotate the call
with a `vhco:net` edge so it's also documented (and so `deny-undeclared` is satisfied).

**My extractor regex catches the word in a comment, not the real call.**
Lexical extraction is best-effort and inferred — that false positive is expected. Tighten the `match`,
scope with `in`, or prefer an edge annotation for the real call site. Don't treat extractor output as
ground truth.

**Can I map a dependency I'm not allowed to edit?**
Yes — use `.vhco.overlay.json` to attach sidecar edges without touching the source.

## Canonical references

- Extractors, rules, diff, coverage, agent queries: `docs/wwd/codebase-intelligence.md`.
- The capability sandbox for AI-generated code (threat model, manifest, runtime profiles):
  `docs/wwd/sandboxing-and-ai-code.md`.
