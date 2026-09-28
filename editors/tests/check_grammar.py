#!/usr/bin/env python3
"""Apply the generated TextMate grammar to every Rivet sample (PLAN-2026-0002 T-13).

Replaces the Node-based `vscode-tmgrammar-test` (Node.js is not installed; see
the PLAN-2026-0002 deviation): a small TextMate engine in Python `re` runs
`editors/rivet.tmLanguage.json` over

    every ```rivet block of docs/references/ref-2026-0002-language-and-usage.md
    every docs/demos/**/app.rivet

and asserts, per line:

    1. nothing crashes (every regex compiles in Python `re`, i.e. no
       Oniguruma-only syntax, and the engine always advances);
    2. a line that starts with a declaration keyword is scoped
       storage.type.declaration.rivet, a control keyword keyword.control.rivet,
       an option keyword.other.option.rivet and `file` support.function.effect.rivet
       (unless the line assigns to a variable of that name: `message = …`);
    3. no unknown keyword: a statement line (outside brackets and strings)
       starts with a scoped keyword, an assignment, a method call or a call.

Engine subset: `match` + `captures`, `begin`/`end` (+ `beginCaptures`,
`patterns`), `include` of `#repository` items and `$self`; the earliest match
wins, ties go to the first pattern, and an open `begin` rule's `end` is tried
alongside its patterns (end wins ties), exactly as vscode-textmate does.

    python3 editors/tests/check_grammar.py        # exit 0 ok, 1 on a failure
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GRAMMAR = ROOT / "editors" / "rivet.tmLanguage.json"
KEYWORDS = ROOT / "editors" / "keywords.json"
REF = ROOT / "docs" / "references" / "ref-2026-0002-language-and-usage.md"


@dataclass
class Rule:
    name: str | None = None
    match: re.Pattern | None = None
    captures: dict = field(default_factory=dict)
    begin: re.Pattern | None = None
    end: str | None = None
    begin_captures: dict = field(default_factory=dict)
    patterns: list = field(default_factory=list)
    include: str | None = None


class Grammar:
    def __init__(self, raw: dict):
        self.raw = raw
        self.repo = {k: self._rule(v) for k, v in raw.get("repository", {}).items()}
        self.top = [self._rule(p) for p in raw["patterns"]]
        self.scope = raw["scopeName"]

    def _rule(self, r: dict) -> Rule:
        return Rule(
            name=r.get("name"),
            match=re.compile(r["match"]) if "match" in r else None,
            captures=r.get("captures", {}),
            begin=re.compile(r["begin"]) if "begin" in r else None,
            end=r.get("end"),
            begin_captures=r.get("beginCaptures", r.get("captures", {})),
            patterns=[self._rule(p) for p in r.get("patterns", [])],
            include=r.get("include"),
        )

    def expand(self, patterns: list[Rule], seen=None) -> list[Rule]:
        """Flatten includes into the concrete match/begin rules, in order."""
        seen = seen or set()
        out: list[Rule] = []
        for p in patterns:
            if p.include:
                if p.include == "$self":
                    key = "$self"
                    target = self.top
                else:
                    key = p.include
                    item = self.repo[p.include.lstrip("#")]
                    target = [item] if (item.match or item.begin) else item.patterns
                if key in seen:
                    continue
                out.extend(self.expand(target, seen | {key}))
            elif p.match or p.begin:
                out.append(p)
            elif p.patterns:
                out.extend(self.expand(p.patterns, seen))
        return out


def assign(scopes: list[list[str]], start: int, end: int, name: str | None):
    if name:
        for i in range(start, min(end, len(scopes))):
            scopes[i].append(name)


def apply_captures(scopes, m: re.Match, captures: dict):
    for k, v in captures.items():
        g = int(k)
        if g <= (m.re.groups or 0) and m.start(g) >= 0:
            assign(scopes, m.start(g), m.end(g), v.get("name"))


def tokenize(g: Grammar, lines: list[str]) -> list[list[list[str]]]:
    """Per line, per character: the list of scopes (outermost first)."""
    stack: list[tuple[Rule, re.Pattern]] = []  # open begin rules + compiled end
    result = []
    for line in lines:
        scopes = [[g.scope] + [r.name for r, _ in stack if r.name] for _ in line]
        pos = 0
        guard = 0
        while pos <= len(line):
            guard += 1
            if guard > 10_000:
                raise RuntimeError(f"engine did not advance on line {line!r}")
            candidates = g.expand(stack[-1][0].patterns if stack else g.top)
            best = None  # (start, order, kind, rule, match)
            if stack:
                em = stack[-1][1].search(line, pos)
                if em:
                    best = (em.start(), -1, "end", stack[-1][0], em)
            for order, rule in enumerate(candidates):
                rx = rule.match or rule.begin
                m = rx.search(line, pos)
                if m and (best is None or m.start() < best[0]):
                    best = (m.start(), order, "match" if rule.match else "begin", rule, m)
            if best is None:
                break
            start, _, kind, rule, m = best
            if kind == "end":
                closed = stack.pop()[0]
                if closed.name:  # the region's scope stops after its end match
                    for i in range(m.end(), len(line)):
                        if closed.name in scopes[i]:
                            scopes[i].reverse()
                            scopes[i].remove(closed.name)
                            scopes[i].reverse()
                pos = m.end() if m.end() > pos else pos + 1
                continue
            if kind == "match":
                assign(scopes, m.start(), m.end(), rule.name)
                apply_captures(scopes, m, rule.captures)
                pos = m.end() if m.end() > pos else pos + 1
                continue
            # begin: the region opens at the match start
            assign(scopes, m.start(), m.end(), rule.name)
            apply_captures(scopes, m, rule.begin_captures)
            stack.append((rule, re.compile(rule.end)))
            pos = m.end() if m.end() > m.start() else pos + 1
            # scopes of the rest of the line are extended below
            for i in range(pos, len(line)):
                if rule.name:
                    scopes[i].append(rule.name)
        result.append(scopes)
        # a `$`-terminated region closes at the end of its line
        while stack and stack[-1][0].end == "$":
            stack.pop()
    return result


def samples() -> list[tuple[str, str]]:
    out = []
    text = REF.read_text(encoding="utf-8")
    for i, m in enumerate(re.finditer(r"```rivet\n(.*?)```", text, re.S), 1):
        out.append((f"REF-2026-0002 block {i}", m.group(1)))
    for path in sorted((ROOT / "docs" / "demos").glob("**/app.rivet")):
        out.append((str(path.relative_to(ROOT)), path.read_text(encoding="utf-8")))
    return out


def has(scopes: list[str], prefix: str) -> bool:
    return any(s.startswith(prefix) for s in scopes)


def check(g: Grammar, kw: dict, label: str, text: str) -> list[str]:
    problems = []
    lines = text.split("\n")
    declaration = set(kw["classes"]["declaration"])
    control = set(kw["classes"]["control"])
    option = set(kw["classes"]["option"])
    effect = set(kw["classes"]["effect"])
    tokens = tokenize(g, lines)
    depth = 0
    for n, (line, scopes) in enumerate(zip(lines, tokens), 1):
        m = re.match(r"^(\s*)([A-Za-z_][A-Za-z0-9_]*)", line)
        in_string = bool(scopes) and m is not None and has(scopes[len(m.group(1))], "string")
        if m and depth == 0 and not in_string:
            col = len(m.group(1))
            word = m.group(2)
            rest = line[m.end():]
            assigns = re.match(r"\s*(=(?!=)|\+=|\.)", rest) is not None
            here = scopes[col]
            if not assigns and word in declaration and not has(here, "storage.type.declaration"):
                problems.append(f"{label}:{n}: declaration `{word}` not scoped (got {here})")
            elif not assigns and word in control and not has(here, "keyword.control"):
                problems.append(f"{label}:{n}: control `{word}` not scoped (got {here})")
            elif not assigns and word in option and not has(here, "keyword.other.option"):
                problems.append(f"{label}:{n}: option `{word}` not scoped (got {here})")
            elif not assigns and word in effect - option and not has(here, "support.function.effect"):
                problems.append(f"{label}:{n}: effect statement `{word}` not scoped (got {here})")
            elif (
                not assigns
                and word not in declaration | control | option | effect
                and not has(here, "keyword")
                and not has(here, "storage")
                and not has(here, "support")
            ):
                problems.append(f"{label}:{n}: unknown statement keyword `{word}` (got {here})")
            elif assigns and not (has(here, "variable") or has(here, "keyword") or has(here, "storage")):
                problems.append(f"{label}:{n}: assigned name `{word}` not scoped (got {here})")
        # bracket depth outside strings and comments (continuation lines)
        for ch, sc in zip(line, scopes):
            if has(sc, "string") or has(sc, "comment"):
                continue
            if ch in "([{":
                depth += 1
            elif ch in ")]}":
                depth = max(0, depth - 1)
    return problems


def main() -> int:
    g = Grammar(json.loads(GRAMMAR.read_text(encoding="utf-8")))
    kw = json.loads(KEYWORDS.read_text(encoding="utf-8"))
    problems: list[str] = []
    items = samples()
    lines = 0
    for label, text in items:
        try:
            problems.extend(check(g, kw, label, text))
        except Exception as e:  # a crash is a failure, reported with its sample
            problems.append(f"{label}: engine crashed: {e!r}")
        lines += text.count("\n")
    for p in problems:
        print("check_grammar: " + p, file=sys.stderr)
    if problems:
        print(f"check_grammar: {len(problems)} problem(s)", file=sys.stderr)
        return 1
    print(f"check_grammar: {len(items)} samples, {lines} lines tokenized; every declaration/control keyword scoped")
    return 0


if __name__ == "__main__":
    sys.exit(main())
