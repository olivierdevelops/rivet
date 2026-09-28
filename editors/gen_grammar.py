#!/usr/bin/env python3
"""Generate the Rivet keyword table and TextMate grammar from the Capy grammar.

    src/infra/rivet.capy ──extract `arg literal "…"` / `block_sections …`──┐
    CLASS_TABLE (below: declaration, control, option, type, effect, …) ─────┤
                                                                             ▼
                                             editors/keywords.json         (GENERATED)
                                                                             ▼
                                             editors/rivet.tmLanguage.json (GENERATED)
                                             editors/vscode/syntaxes/rivet.tmLanguage.json (copy)

Both outputs are generated: never hand-edit them. Re-run after changing
`rivet.capy` or the class table:

    python3 editors/gen_grammar.py          # write the outputs
    python3 editors/gen_grammar.py --check  # exit 1 when an output is stale

`editors/check_keywords.py` (run by `cargo test --test conformance_highlight`)
fails when a `rivet.capy` literal has no class or an output is out of date.
The Rust highlighter (`src/features/language/highlight_source.rs`) embeds
`keywords.json`, so the editor grammar and `rivet highlight` share one table.

Standard library only (PLAN-2026-0002 PF-14; Node.js is not required).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CAPY = ROOT / "src" / "infra" / "rivet.capy"
KEYWORDS = ROOT / "editors" / "keywords.json"
GRAMMAR = ROOT / "editors" / "rivet.tmLanguage.json"
VSCODE_GRAMMAR = ROOT / "editors" / "vscode" / "syntaxes" / "rivet.tmLanguage.json"

SCOPE_NAME = "source.rivet"

# ---------------------------------------------------------------------------
# The token-class table. Every literal of rivet.capy must land in at least one
# class (checked); the words that are not Capy literals (types, effects, param
# modifiers, built-ins) mirror Rivet's lowering tables and are cross-checked
# against the Rust sources by editors/check_keywords.py.
# ---------------------------------------------------------------------------
CLASS_TABLE: dict[str, list[str]] = {
    # top-level and header statements
    "declaration": [
        "import", "global", "operation", "pipeline", "connector", "auth",
        "name", "description", "private", "param", "output", "emits",
        "receives", "field", "error", "secret", "public",
    ],
    # body statements, block closers and block glue words
    "control": [
        "end", "return", "yield", "emit", "fail", "break", "if", "else",
        "while", "for", "try", "catch", "dag", "node", "after", "concurrent",
        "task", "scope", "iterate", "with", "map", "poll", "every",
    ],
    # option lines: every `opt_*` literal is added automatically (derived from
    # rivet.capy); these are the param/field modifiers on the same footing
    "option": ["required", "optional", "default", "min", "max", "enum"],
    # value types (domain::outputs::ValueSpec::parse_scalar + object, list)
    "type": ["text", "integer", "number", "boolean", "bytes", "json", "object", "list"],
    # effect heads (lowering EFFECT_WORDS) and statements
    "effect": [
        "http", "file", "grpc", "command", "tcp", "unix", "pipe", "udp",
        "quic", "websocket",
    ],
    "boolean_null": ["true", "false", "null"],
    # word operators and the punctuation literals of rivet.capy
    "operator": ["in", "and", "or", "not", "as", "=", "+=", "."],
    # prefix-call built-ins (domain::ir::BUILTIN_FUNCTIONS)
    "builtin": [
        "request", "request.stream", "length", "base64.encode", "base64.decode",
        "text", "keys", "xml.element",
    ],
}

SYMBOL_OPERATORS = ["==", "!=", "<=", ">=", "+=", "=", "+", "-", "*", "/", "%", "<", ">"]

# TextMate scope per class (standard scope roots so every theme colours them).
SCOPES: dict[str, str] = {
    "declaration": "storage.type.declaration.rivet",
    "control": "keyword.control.rivet",
    "option": "keyword.other.option.rivet",
    "modifier": "storage.modifier.rivet",
    "type": "support.type.rivet",
    "effect": "support.function.effect.rivet",
    "effect_verb": "entity.name.function.effect-verb.rivet",
    "builtin": "support.function.builtin.rivet",
    "boolean_null": "constant.language.rivet",
    "number": "constant.numeric.rivet",
    "string": "string.quoted.double.rivet",
    "string_single": "string.quoted.single.rivet",
    "string_backtick": "string.quoted.other.backtick.rivet",
    "interpolation": "meta.interpolation.rivet",
    "interpolation_begin": "punctuation.section.interpolation.begin.rivet",
    "interpolation_end": "punctuation.section.interpolation.end.rivet",
    "interpolation_path": "variable.other.interpolated.rivet",
    "escape": "constant.character.escape.rivet",
    "comment": "comment.line.number-sign.rivet",
    "comment_punctuation": "punctuation.definition.comment.rivet",
    "operation_id": "entity.name.function.operation-id.rivet",
    "global": "variable.other.constant.global.rivet",
    "variable": "variable.other.rivet",
    "parameter": "variable.parameter.rivet",
    "assigned": "variable.other.assignment.rivet",
    "operator": "keyword.operator.rivet",
    "operator_word": "keyword.operator.word.rivet",
    "assignment": "keyword.operator.assignment.rivet",
}

# The class `rivet highlight` reports for each table class (PROP-2026-0002 R17).
HIGHLIGHT_CLASS = {
    "declaration": "keyword",
    "control": "keyword",
    "boolean_null": "keyword",
    "builtin": "keyword",
    "option": "option",
    "type": "type",
    "effect": "effect",
    "operator": "operator",
}

TOKEN_CLASSES = [
    "keyword", "option", "type", "effect", "string", "interpolation", "number",
    "comment", "operation_id", "global", "variable", "operator",
]


def parse_capy(text: str) -> dict:
    """Every function of rivet.capy with its literals and block sections."""
    functions: list[dict] = []
    current: dict | None = None
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].rstrip() if not raw.lstrip().startswith("#") else ""
        m = re.match(r"^function\s+(\w+)\s*$", line)
        if m:
            current = {"name": m.group(1), "literals": [], "sections": []}
            functions.append(current)
            continue
        if line.strip() == "end" and not line.startswith(" "):
            current = None
            continue
        if current is None:
            continue
        for lit in re.findall(r'\barg\s+literal\s+"((?:[^"\\]|\\.)*)"', line):
            current["literals"].append(lit)
        m = re.search(r"\bblock_sections\s+(\w+)\s+closer\s+(\w+)", line)
        if m:
            current["sections"].append(m.group(1))
            current["literals"].append(m.group(1))
    return {"functions": functions}


def build_keywords(capy_text: str) -> dict:
    parsed = parse_capy(capy_text)
    literals: set[str] = set()
    option_literals: set[str] = set()
    for f in parsed["functions"]:
        literals.update(f["literals"])
        if f["name"].startswith("opt_") and f["literals"]:
            option_literals.add(f["literals"][0])
        if f["name"] == "end":
            literals.add("end")
    classes = {k: sorted(set(v)) for k, v in CLASS_TABLE.items()}
    classes["option"] = sorted(set(classes["option"]) | option_literals)
    classified = set().union(*classes.values())
    unclassified = sorted(literals - classified)
    statements = {}
    for f in parsed["functions"]:
        if f["literals"]:
            statements[f["name"]] = f["literals"][0]
    return {
        "$comment": "GENERATED by editors/gen_grammar.py from src/infra/rivet.capy and its class table; do not edit",
        "source": "src/infra/rivet.capy",
        "scope_name": SCOPE_NAME,
        "literals": sorted(literals),
        "unclassified": unclassified,
        "classes": classes,
        "option_modifiers": sorted(CLASS_TABLE["option"]),
        "statements": dict(sorted(statements.items())),
        "symbol_operators": SYMBOL_OPERATORS,
        "highlight_classes": TOKEN_CLASSES,
        "highlight_class_of": HIGHLIGHT_CLASS,
        "scopes": SCOPES,
    }


def words(ws: list[str]) -> str:
    """A regex alternation of words, longest first (so `request.stream` beats `request`)."""
    return "|".join(re.escape(w) for w in sorted(ws, key=lambda w: (-len(w), w)))


def build_grammar(kw: dict) -> dict:
    c = kw["classes"]
    s = SCOPES
    decl = [w for w in c["declaration"] if w not in ("public",)]
    control = c["control"]
    options = [w for w in c["option"] if w not in kw["option_modifiers"]]
    modifiers = kw["option_modifiers"]
    types = c["type"]
    effects = c["effect"]
    builtins = c["builtin"]
    ident = r"[A-Za-z_][A-Za-z0-9_]*"
    # A statement word is a keyword unless the line assigns to / calls a
    # method on a variable of the same name (INC-2026-0002): `message = …`,
    # `stream.send …`, `count += 1`.
    not_var = r"(?=\s|$)(?!\s*(?:=(?!=)|\+=))"
    ws_lead = r"^\s*"

    repository = {
        "comment": {
            "match": r"(#).*$",
            "name": s["comment"],
            "captures": {"1": {"name": s["comment_punctuation"]}},
        },
        "interpolation": {
            "match": r"(\$\{)\s*([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)*)\s*(\})",
            "name": s["interpolation"],
            "captures": {
                "1": {"name": s["interpolation_begin"]},
                "2": {"name": s["interpolation_path"]},
                "3": {"name": s["interpolation_end"]},
            },
        },
        "escape": {"match": r"\\(?:[nrt0\\\"'`$]|u\{[0-9A-Fa-f]{1,6}\}|x[0-9A-Fa-f]{2})", "name": s["escape"]},
        "string_double": {
            "begin": '"',
            "end": '"',
            "name": s["string"],
            "patterns": [{"include": "#escape"}, {"include": "#interpolation"}],
        },
        "string_single": {
            "begin": "'",
            "end": "'",
            "name": s["string_single"],
            "patterns": [{"include": "#escape"}, {"include": "#interpolation"}],
        },
        "string_backtick": {
            "begin": "`",
            "end": "`",
            "name": s["string_backtick"],
            "patterns": [{"include": "#escape"}, {"include": "#interpolation"}],
        },
        "strings": {
            "patterns": [
                {"include": "#string_double"},
                {"include": "#string_single"},
                {"include": "#string_backtick"},
            ]
        },
        "number": {"match": r"(?<![A-Za-z0-9_.])-?[0-9][0-9_]*(?:\.[0-9]+)?(?![A-Za-z0-9_])", "name": s["number"]},
        "constant": {"match": rf"\b(?:{words(c['boolean_null'])})\b", "name": s["boolean_null"]},
        "builtin_call": {
            "match": rf"(\()\s*({words(builtins)})(?![A-Za-z0-9_.])",
            "captures": {"2": {"name": s["builtin"]}},
        },
        "operation_call": {
            "match": rf"(\()\s*({ident}(?:\.{ident})+)(?=\s|\))",
            "captures": {"2": {"name": s["operation_id"]}},
        },
        "effect": {
            "match": rf"\b({words(effects)})\b(?:\s+(?!(?:in|as|and|or|not)\b)([a-z_][a-z0-9_]*)\b(?![.(]))?(?=\s+[\"'`A-Za-z_\[{{(])",
            "captures": {"1": {"name": s["effect"]}, "2": {"name": s["effect_verb"]}},
        },
        "operator_word": {"match": r"\b(?:and|or|not|in|as)\b", "name": s["operator_word"]},
        "operator": {"match": r"==|!=|<=|>=|\+=|[-+*/%<>=]", "name": s["operator"]},
        "variable": {"match": rf"\b{ident}(?:\.[A-Za-z0-9_]+)*\b", "name": s["variable"]},
        "expression": {
            "patterns": [
                {"include": "#comment"},
                {"include": "#strings"},
                {"include": "#builtin_call"},
                {"include": "#operation_call"},
                {"include": "#constant"},
                {"include": "#number"},
                {"include": "#effect"},
                {"include": "#operator_word"},
                {"include": "#operator"},
                {"include": "#variable"},
            ]
        },
        "type_words": {"match": rf"\b(?:{words(types)})\b", "name": s["type"]},
        "modifiers": {
            "match": rf"\b(?:{words(modifiers + ['description'])})\b",
            "name": s["modifier"],
        },
        # import "PATH" as ALIAS [public]
        "import": {
            "match": rf"{ws_lead}(import)\s+(\"[^\"]*\")\s+(as)\s+({ident})(?:\s+(public))?",
            "captures": {
                "1": {"name": s["declaration"]},
                "2": {"name": s["string"]},
                "3": {"name": s["operator_word"]},
                "4": {"name": s["variable"]},
                "5": {"name": s["declaration"]},
            },
        },
        # global NAME = EXPR
        "global": {
            "match": rf"{ws_lead}(global)\s+({ident})\s*(=)?",
            "captures": {
                "1": {"name": s["declaration"]},
                "2": {"name": s["global"]},
                "3": {"name": s["assignment"]},
            },
        },
        # operation ID / pipeline ID
        "operation": {
            "match": rf"{ws_lead}(operation|pipeline)\s+({ident}(?:\.{ident})*)",
            "captures": {"1": {"name": s["declaration"]}, "2": {"name": s["operation_id"]}},
        },
        # connector NAME KIND / auth NAME KIND
        "connector": {
            "match": rf"{ws_lead}(connector|auth)\s+({ident})\s+({ident})\s*$",
            "captures": {
                "1": {"name": s["declaration"]},
                "2": {"name": s["variable"]},
                "3": {"name": s["type"]},
            },
        },
        # param NAME [list …] TYPE modifiers… / field NAME TYPE …
        "typed_member": {
            "begin": rf"{ws_lead}(param|field|secret)\s+({ident})",
            "beginCaptures": {"1": {"name": s["declaration"]}, "2": {"name": s["parameter"]}},
            "end": "$",
            "patterns": [
                {"include": "#comment"},
                {"include": "#strings"},
                {"include": "#type_words"},
                {"include": "#modifiers"},
                {"include": "#expression"},
            ],
        },
        # output TYPE / emits TYPE / receives TYPE [description "…"]
        "typed_decl": {
            "begin": rf"{ws_lead}(output|emits|receives){not_var}",
            "beginCaptures": {"1": {"name": s["declaration"]}},
            "end": "$",
            "patterns": [
                {"include": "#comment"},
                {"include": "#strings"},
                {"include": "#type_words"},
                {"include": "#modifiers"},
                {"include": "#expression"},
            ],
        },
        # for VAR in ITER
        "for": {
            "match": rf"{ws_lead}(for)\s+({ident})\s+(in)\b",
            "captures": {
                "1": {"name": s["control"]},
                "2": {"name": s["variable"]},
                "3": {"name": s["operator_word"]},
            },
        },
        # node NAME [after DEPS] =
        "node": {
            "match": rf"{ws_lead}(node)\s+({ident})(?:\s+(after)\b)?",
            "captures": {
                "1": {"name": s["control"]},
                "2": {"name": s["variable"]},
                "3": {"name": s["control"]},
            },
        },
        # VAR = map ITEM in … / VAR = poll … / VAR = … / VAR += …
        "assignment": {
            "match": rf"{ws_lead}({ident})\s*(=(?!=)|\+=)\s*(?:(map|poll)\b(?:\s+({ident})\s+(in)\b)?)?",
            "captures": {
                "1": {"name": s["assigned"]},
                "2": {"name": s["assignment"]},
                "3": {"name": s["control"]},
                "4": {"name": s["variable"]},
                "5": {"name": s["operator_word"]},
            },
        },
        "declaration_statement": {
            "match": rf"{ws_lead}({words(decl)}){not_var}",
            "captures": {"1": {"name": s["declaration"]}},
        },
        "control_statement": {
            "match": rf"{ws_lead}({words(control)}){not_var}",
            "captures": {"1": {"name": s["control"]}},
        },
        # decode json / timeout "5s" / header "accept" value … (an option tail)
        "option_statement": {
            "begin": rf"{ws_lead}({words(options)}){not_var}",
            "beginCaptures": {"1": {"name": s["option"]}},
            "end": "$",
            "patterns": [
                {"include": "#comment"},
                {"include": "#strings"},
                {"include": "#type_words"},
                {"include": "#expression"},
            ],
        },
        "effect_statement": {
            "match": rf"{ws_lead}(file){not_var}(?:\s+([a-z_][a-z0-9_]*)\b(?![.(]))?",
            "captures": {"1": {"name": s["effect"]}, "2": {"name": s["effect_verb"]}},
        },
        # `as json`, `limit 10`, `until done`, `every "1s"` inside a statement tail
        "tail_option": {
            "match": rf"(?<=\s)(as)\s+({words(types)})\b",
            "captures": {"1": {"name": s["operator_word"]}, "2": {"name": s["type"]}},
        },
    }
    patterns = [
        {"include": "#comment"},
        {"include": "#import"},
        {"include": "#global"},
        {"include": "#operation"},
        {"include": "#connector"},
        {"include": "#typed_member"},
        {"include": "#typed_decl"},
        {"include": "#for"},
        {"include": "#node"},
        {"include": "#assignment"},
        {"include": "#effect_statement"},
        {"include": "#declaration_statement"},
        {"include": "#control_statement"},
        {"include": "#option_statement"},
        {"include": "#tail_option"},
        {"include": "#expression"},
    ]
    return {
        "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
        "$comment": "GENERATED by editors/gen_grammar.py from editors/keywords.json (src/infra/rivet.capy); do not edit",
        "name": "Rivet",
        "scopeName": SCOPE_NAME,
        "fileTypes": ["rivet"],
        "patterns": patterns,
        "repository": repository,
    }


def render(obj: dict) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False) + "\n"


def outputs() -> dict[Path, str]:
    kw = build_keywords(CAPY.read_text(encoding="utf-8"))
    grammar = render(build_grammar(kw))
    return {KEYWORDS: render(kw), GRAMMAR: grammar, VSCODE_GRAMMAR: grammar}


def main(argv: list[str]) -> int:
    check = "--check" in argv
    kw = build_keywords(CAPY.read_text(encoding="utf-8"))
    if kw["unclassified"]:
        print(
            "gen_grammar: rivet.capy literals with no class: "
            + ", ".join(kw["unclassified"])
            + " — add them to CLASS_TABLE in editors/gen_grammar.py",
            file=sys.stderr,
        )
        return 1
    stale = []
    for path, text in outputs().items():
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current != text:
            stale.append(path)
            if not check:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text, encoding="utf-8")
    rel = [str(p.relative_to(ROOT)) for p in stale]
    if check:
        if stale:
            print("gen_grammar: stale generated files: " + ", ".join(rel)
                  + " — run python3 editors/gen_grammar.py", file=sys.stderr)
            return 1
        print("gen_grammar: generated files are up to date")
        return 0
    print("gen_grammar: wrote " + (", ".join(rel) if rel else "nothing (up to date)"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
