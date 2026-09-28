#!/usr/bin/env python3
"""Keyword drift check (PLAN-2026-0002 T-13, R16).

    rivet.capy literals ──▶ every one classified in editors/keywords.json ?
    gen_grammar outputs  ──▶ keywords.json / rivet.tmLanguage.json (+ vscode copy) up to date ?
    Rust lowering tables ──▶ effect words, value types, built-ins == the class table ?

Exit 0 when all hold, 1 otherwise (one line per problem on stderr). Run by
`cargo test --test conformance_highlight`, so `cargo test` catches drift.
Standard library only.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_grammar as gen  # noqa: E402

ROOT = gen.ROOT


def rust_list(path: Path, const: str) -> list[str]:
    """The string items of `const NAME: &[&str] = &[ … ];` in a Rust file."""
    text = path.read_text(encoding="utf-8")
    m = re.search(rf"const {const}: &\[&str\] = &\[(.*?)\];", text, re.S)
    if not m:
        raise SystemExit(f"check_keywords: `{const}` not found in {path.relative_to(ROOT)}")
    return re.findall(r'"([^"]+)"', m.group(1))


def rust_scalar_types() -> list[str]:
    text = (ROOT / "src" / "domain" / "outputs.rs").read_text(encoding="utf-8")
    m = re.search(r"fn parse_scalar\(.*?\{(.*?)\n    \}", text, re.S)
    if not m:
        raise SystemExit("check_keywords: parse_scalar not found in src/domain/outputs.rs")
    return re.findall(r'"(\w+)" =>', m.group(1))


def main() -> int:
    problems: list[str] = []
    capy_text = gen.CAPY.read_text(encoding="utf-8")
    kw = gen.build_keywords(capy_text)

    # 1. every rivet.capy literal has a class
    for lit in kw["unclassified"]:
        problems.append(f"rivet.capy literal `{lit}` has no class (add it to CLASS_TABLE in editors/gen_grammar.py)")

    # 2. the committed keywords.json classifies every literal too
    if gen.KEYWORDS.exists():
        on_disk = json.loads(gen.KEYWORDS.read_text(encoding="utf-8"))
        classified = set().union(*map(set, on_disk.get("classes", {}).values()))
        for lit in kw["literals"]:
            if lit not in classified:
                problems.append(f"editors/keywords.json does not classify `{lit}`")
    else:
        problems.append("editors/keywords.json is missing (run python3 editors/gen_grammar.py)")

    # 3. generated files are up to date
    for path, text in gen.outputs().items():
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current != text:
            problems.append(f"{path.relative_to(ROOT)} is stale (run python3 editors/gen_grammar.py)")

    # 4. the non-literal words mirror Rivet's own tables
    lower = ROOT / "src" / "features" / "language" / "lowering" / "lower.rs"
    ir = ROOT / "src" / "domain" / "ir.rs"
    pairs = [
        ("effect", set(rust_list(lower, "EFFECT_WORDS")) | {"file"}, "lowering EFFECT_WORDS"),
        ("type", set(rust_scalar_types()) | {"object", "list"}, "ValueSpec::parse_scalar + object, list"),
        ("builtin", set(rust_list(ir, "BUILTIN_FUNCTIONS")), "domain::ir::BUILTIN_FUNCTIONS"),
    ]
    for cls, rust, where in pairs:
        table = set(kw["classes"][cls])
        if table != rust:
            problems.append(
                f"class `{cls}` drifted from {where}: only in the table {sorted(table - rust)}, only in Rust {sorted(rust - table)}"
            )

    # 5. every table class maps to one of the R17 highlight classes
    for cls in kw["classes"]:
        target = kw["highlight_class_of"].get(cls)
        if target not in kw["highlight_classes"]:
            problems.append(f"class `{cls}` has no highlight class")

    for p in problems:
        print("check_keywords: " + p, file=sys.stderr)
    if problems:
        return 1
    print(
        f"check_keywords: {len(kw['literals'])} rivet.capy literals classified; "
        f"{sum(len(v) for v in kw['classes'].values())} table words; generated files up to date"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
