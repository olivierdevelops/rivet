//! T-13, T-14 — syntax highlighting (PROP-2026-0002 R16, R17; UC-07, UC-08).
//!
//! ```text
//!  T-13  python3 editors/check_keywords.py      every rivet.capy literal classified; generated files fresh
//!        python3 editors/tests/check_grammar.py TextMate regexes over every REF-2026-0002 block + demo app
//!        python3 editors/vscode/package_vsix.py .vsix layout; package.json version == Cargo version
//!  T-14  rivet highlight --format ansi|html|json   goldens in tests/fixtures/highlight/
//!        syntax error → tokens before the error + diagnostic (exit 2)
//!        every token of every REF block and demo lies inside the source, sorted, non-overlapping
//! ```
//!
//! Goldens are refreshed with `RIVET_BLESS=1 cargo test --test conformance_highlight`.
#![allow(clippy::result_large_err)]

use rivet::highlight::{self, HighlightFormat, HighlightToken, TOKEN_CLASSES};
use std::path::{Path, PathBuf};
use std::process::Command;

const RIVET: &str = env!("CARGO_BIN_EXE_rivet");
const FIXTURES: &str = "tests/fixtures/highlight";

/// `python3`, else `python` (Windows installs of CPython ship only `python.exe`).
fn python(args: &[&str]) -> std::process::Output {
    ["python3", "python"]
        .iter()
        .find_map(|exe| Command::new(exe).args(args).output().ok())
        .expect("python3 (or python) is required for the editor checks (T-13)")
}

fn assert_ok(out: &std::process::Output, what: &str) {
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rivet-hl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------- T-13

// vhco:test language.highlight_source -- T-13 drift: every rivet.capy literal is classified in editors/keywords.json, the class table mirrors lowering's effect words, value types and built-ins, and keywords.json + rivet.tmLanguage.json are exactly what gen_grammar.py generates
#[test]
fn t13_keyword_table_has_no_drift() {
    let out = python(&["editors/check_keywords.py"]);
    assert_ok(&out, "editors/check_keywords.py");
}

// vhco:test language.highlight_source -- T-13 grammar: the generated TextMate regexes run (Python re) over every REF-2026-0002 rivet block and docs/demos/**/app.rivet without crashing, and every declaration, control, option and effect statement keyword is scoped
#[test]
fn t13_textmate_grammar_scopes_every_sample() {
    let out = python(&["editors/tests/check_grammar.py"]);
    assert_ok(&out, "editors/tests/check_grammar.py");
    let text = String::from_utf8_lossy(&out.stdout);
    let samples: usize = text
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    assert!(samples >= 100, "too few samples checked: {text}");
}

// vhco:test language.highlight_source -- T-13 packaging: package_vsix.py builds rivet-<Cargo version>.vsix with [Content_Types].xml, extension.vsixmanifest and extension/{package.json, README.md, language-configuration.json, syntaxes/rivet.tmLanguage.json}, deterministically, and nothing from .vscodeignore
#[test]
fn t13_vsix_packages_with_the_workspace_version() {
    let pkg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("editors/vscode/package.json").unwrap())
            .unwrap();
    assert_eq!(pkg["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        pkg["contributes"]["languages"][0]["extensions"][0],
        ".rivet"
    );
    assert_eq!(
        pkg["contributes"]["grammars"][0]["scopeName"],
        "source.rivet"
    );
    let conf: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("editors/vscode/language-configuration.json").unwrap(),
    )
    .unwrap();
    assert_eq!(conf["comments"]["lineComment"], "#");
    assert_eq!(conf["brackets"].as_array().unwrap().len(), 3);

    let dir = scratch("vsix");
    let dir_s = dir.to_string_lossy().to_string();
    let first = python(&["editors/vscode/package_vsix.py", "--out", &dir_s]);
    assert_ok(&first, "package_vsix.py");
    let vsix = dir.join(format!("rivet-{}.vsix", env!("CARGO_PKG_VERSION")));
    let bytes = std::fs::read(&vsix).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04", "a zip archive");
    let listing = String::from_utf8_lossy(&bytes);
    for name in [
        "[Content_Types].xml",
        "extension.vsixmanifest",
        "extension/package.json",
        "extension/README.md",
        "extension/language-configuration.json",
        "extension/syntaxes/rivet.tmLanguage.json",
    ] {
        assert!(listing.contains(name), "{name} missing from the .vsix");
    }
    for excluded in ["package_vsix.py", ".vscodeignore"] {
        assert!(
            !listing.contains(excluded),
            "{excluded} must not be packaged"
        );
    }
    let again = python(&["editors/vscode/package_vsix.py", "--out", &dir_s]);
    assert_ok(&again, "package_vsix.py (second run)");
    assert_eq!(
        std::fs::read(&vsix).unwrap(),
        bytes,
        "deterministic archive"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- T-14

fn run_highlight(file: &str, format: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(RIVET);
    cmd.arg("highlight").arg(file);
    if let Some(f) = format {
        cmd.args(["--format", f]);
    }
    cmd.output().unwrap()
}

fn golden(name: &str, actual: &str) {
    let path = Path::new(FIXTURES).join(name);
    if std::env::var_os("RIVET_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {}", path.display()));
    assert_eq!(
        actual, expected,
        "golden {name} differs (RIVET_BLESS=1 to refresh)"
    );
}

// vhco:test language.highlight_source -- T-14 goldens: `rivet highlight app.rivet --format ansi|html|json` match tests/fixtures/highlight/app.{ansi,html,jsonl}; the library renders the same bytes; json is the default when stdout is not a terminal
#[test]
fn t14_cli_goldens_for_ansi_html_and_json() {
    let file = format!("{FIXTURES}/app.rivet");
    let source = std::fs::read_to_string(&file).unwrap();
    let tokens = highlight::tokens_of(&file, &source).expect("app.rivet parses");
    for (format, ext) in [("ansi", "ansi"), ("html", "html"), ("json", "jsonl")] {
        let out = run_highlight(&file, Some(format));
        assert_ok(&out, &format!("rivet highlight --format {format}"));
        let text = String::from_utf8(out.stdout).unwrap();
        golden(&format!("app.{ext}"), &text);
        let lib = highlight::render(&source, &tokens, HighlightFormat::parse(format).unwrap());
        assert_eq!(lib, text, "library and CLI render the same {format}");
    }
    // piped stdout (not a TTY) → json lines
    let piped = run_highlight(&file, None);
    assert_ok(&piped, "rivet highlight (default format)");
    assert_eq!(
        String::from_utf8(piped.stdout).unwrap(),
        std::fs::read_to_string(format!("{FIXTURES}/app.jsonl")).unwrap()
    );
    // stripping the ANSI codes gives back the source
    let ansi = std::fs::read_to_string(format!("{FIXTURES}/app.ansi")).unwrap();
    let mut plain = String::new();
    let mut chars = ansi.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for d in chars.by_ref() {
                if d == 'm' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    assert_eq!(plain, source);
}

// vhco:test language.highlight_source -- T-14 classes: the golden source yields every R17 class (keyword, option, type, effect, string, interpolation, number, comment, operation_id, global, variable, operator) at the expected positions, e.g. global api at 3:8 and the effect verb `get`
#[test]
fn t14_every_token_class_appears_where_expected() {
    let source = std::fs::read_to_string(format!("{FIXTURES}/app.rivet")).unwrap();
    let tokens = highlight::tokens(&source).unwrap();
    for class in TOKEN_CLASSES {
        assert!(
            tokens.iter().any(|t| t.class == *class),
            "no `{class}` token"
        );
    }
    let at = |line: u32, col: u32| -> (&str, &str) {
        let t = tokens
            .iter()
            .find(|t| t.line == line && t.col == col)
            .unwrap_or_else(|| panic!("no token at {line}:{col}"));
        (t.class.as_str(), t.text.as_str())
    };
    assert_eq!(
        at(1, 1),
        (
            "comment",
            "# Golden source for T-14 (tests/conformance_highlight.rs): every token class once or more."
        )
    );
    assert_eq!(at(2, 1), ("keyword", "import"));
    assert_eq!(at(2, 33), ("keyword", "public"));
    assert_eq!(at(3, 8), ("global", "api"));
    assert_eq!(at(3, 14), ("string", "\"https://api.example.com\""));
    assert_eq!(at(6, 11), ("operation_id", "users.get"));
    assert_eq!(at(8, 14), ("type", "integer"));
    assert_eq!(at(8, 22), ("option", "required"));
    assert_eq!(at(11, 9), ("effect", "http"));
    assert_eq!(at(11, 14), ("effect", "get"));
    assert_eq!(at(11, 19), ("interpolation", "${api}"));
    assert_eq!(at(12, 9), ("option", "header"));
    assert_eq!(at(16, 5), ("variable", "message"));
    // `é` is one character (two bytes): the interpolation after it starts at column 18
    assert_eq!(at(16, 18), ("interpolation", "${id}"));
    assert_eq!(at(18, 37), ("global", "retry_on.0"));
    assert_eq!(at(18, 53), ("keyword", "length"));
    assert_eq!(at(19, 5), ("keyword", "else"));
    assert_eq!(at(20, 43), ("operation_id", "users.list"));
    assert_eq!(at(22, 1), ("keyword", "end"));
}

// vhco:test language.highlight_source -- T-14 partial tokens: a syntax error prints the tokens that start before it (none after), the syntax.expression diagnostic at 5:5 on stderr, exit 2; the library returns the same tokens with the error
#[test]
fn t14_syntax_error_gives_partial_tokens_and_exit_2() {
    let file = format!("{FIXTURES}/broken.rivet");
    let out = run_highlight(&file, Some("json"));
    assert_eq!(out.status.code(), Some(2));
    let stdout = String::from_utf8(out.stdout).unwrap();
    golden("broken.jsonl", &stdout);
    let lines: Vec<serde_json::Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.first().unwrap()["text"], "global");
    assert_eq!(lines.last().unwrap()["text"], "json");
    assert!(lines.iter().all(|l| l["line"].as_u64().unwrap() < 5));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[syntax.expression]"), "{stderr}");
    assert!(stderr.contains("broken.rivet:5:5"), "{stderr}");

    let source = std::fs::read_to_string(&file).unwrap();
    let (tokens, err) = highlight::tokens_of(&file, &source).unwrap_err();
    assert_eq!(tokens.len(), lines.len());
    assert_eq!(err.code, "syntax.expression");
    assert_eq!(err.exit_code(), 2);
    let (partial, err2) = highlight::highlight(&source, HighlightFormat::Html).unwrap_err();
    assert!(
        partial
            .starts_with("<pre class=\"rv-source\"><code><span class=\"rv-keyword\">global</span>")
    );
    assert!(
        partial.contains("    return {x: }"),
        "text after the error is copied plain"
    );
    assert_eq!(err2.code, "syntax.expression");

    // --json: the diagnostic is an error envelope naming rivet.highlight
    let out = Command::new(RIVET)
        .args(["--json", "highlight", &file, "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let env: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(env["operation"], "rivet.highlight");
    assert_eq!(env["status"], "error");
}

// vhco:test language.highlight_source -- T-14 usage: an unreadable FILE is validation.usage (exit 2) and an unknown --format is refused by the CLI
#[test]
fn t14_usage_errors() {
    let out = run_highlight("tests/fixtures/highlight/no-such-file.rivet", Some("json"));
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("validation.usage"));
    assert!(out.stdout.is_empty());
    let out = run_highlight(&format!("{FIXTURES}/app.rivet"), Some("svg"));
    assert!(!out.status.success());
    assert!(HighlightFormat::parse("svg").is_err());
}

fn corpus() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let reference =
        std::fs::read_to_string("docs/references/ref-2026-0002-language-and-usage.md").unwrap();
    let mut rest = reference.as_str();
    let mut n = 0;
    while let Some(i) = rest.find("```rivet\n") {
        let body = &rest[i + 9..];
        let end = body.find("```").unwrap();
        n += 1;
        out.push((format!("REF-2026-0002 block {n}"), body[..end].to_string()));
        rest = &body[end + 3..];
    }
    let mut stack = vec![PathBuf::from("docs/demos")];
    let mut demos = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|f| f == "app.rivet") {
                demos.push(p);
            }
        }
    }
    demos.sort();
    for p in demos {
        out.push((
            p.display().to_string(),
            std::fs::read_to_string(&p).unwrap(),
        ));
    }
    out
}

fn check_spans(label: &str, source: &str, tokens: &[HighlightToken]) {
    let lines: Vec<&str> = source.split('\n').collect();
    let mut last = (0u32, 0u32);
    for t in tokens {
        assert!(
            TOKEN_CLASSES.contains(&t.class.as_str()),
            "{label}: class {}",
            t.class
        );
        assert!(t.len > 0, "{label}: empty token {t:?}");
        assert!(
            (t.line, t.col) >= last,
            "{label}: tokens out of order at {t:?}"
        );
        let line = lines
            .get(t.line as usize - 1)
            .unwrap_or_else(|| panic!("{label}: line {} outside the source", t.line));
        let slice: String = line
            .chars()
            .skip(t.col as usize - 1)
            .take(t.len as usize)
            .collect();
        assert_eq!(
            slice, t.text,
            "{label}: token {t:?} does not slice the source"
        );
        last = (t.line, t.col + t.len);
    }
}

// vhco:test language.highlight_source -- T-14 corpus: every REF-2026-0002 rivet block and every demo app.rivet tokenizes; parses cleanly; every token lies inside the source (line/col/len slice exactly its text), tokens are sorted and non-overlapping, and each statement's first word is a keyword, option or effect token
#[test]
fn t14_every_reference_and_demo_token_lies_inside_the_source() {
    let items = corpus();
    assert!(items.len() >= 100, "corpus too small: {}", items.len());
    let mut total = 0;
    for (label, source) in &items {
        let tokens = match highlight::tokens_of(label, source) {
            Ok(t) => t,
            Err((_, e)) => panic!("{label}: every REF block and demo parses: {e:?}"),
        };
        check_spans(label, source, &tokens);
        total += tokens.len();
        if label.starts_with("docs/demos") {
            let keywords = ["operation", "param", "output", "return", "end"];
            for (n, line) in source.lines().enumerate() {
                let word = line.split_whitespace().next().unwrap_or("");
                if keywords.contains(&word) {
                    let col = (line.len() - line.trim_start().len()) as u32 + 1;
                    assert!(
                        tokens.iter().any(|t| t.line == n as u32 + 1
                            && t.col == col
                            && t.class == "keyword"),
                        "{label}:{}: `{word}` is not a keyword token",
                        n + 1
                    );
                }
            }
        }
    }
    assert!(total > 4_000, "suspiciously few tokens: {total}");
}
