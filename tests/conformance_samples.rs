//! T-29 — sample corpus: every demo bundle compiles; every reference example parses.

use rivet::internal::domain::ports::Parser;
use rivet::internal::domain::source::{SourceBundle, SourceFile};
use rivet::internal::features::language::compile_program::compile_program;
use rivet::internal::infra::capy_parser::CapyParser;

// vhco:test language.compile_program -- every docs/demos app.rivet compiles into a program
#[test]
fn every_demo_bundle_compiles() {
    let parser = CapyParser::new().unwrap();
    let mut failures = Vec::new();
    let mut dirs: Vec<_> = std::fs::read_dir("docs/demos")
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("app.rivet").exists())
        .collect();
    dirs.sort();
    for dir in &dirs {
        let path = dir.join("app.rivet");
        let text = std::fs::read_to_string(&path).unwrap();
        let bundle = SourceBundle {
            entry: path.display().to_string(),
            root: dir.display().to_string(),
            files: vec![SourceFile {
                path: path.display().to_string(),
                text: text.clone(),
            }],
            modules: Vec::new(),
        };
        if let Err(e) = compile_program(&bundle, &parser) {
            failures.push(e.render(Some(&text)));
            for s in &e.suppressed {
                failures.push(format!("  also: {}", s.render(Some(&text))));
            }
        }
    }
    assert!(
        dirs.len() >= 12,
        "found {} demo bundles, expected at least the twelve release demos",
        dirs.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// vhco:test language.compile_program -- every rivet block in REF-2026-0002 parses with the embedded grammar
#[test]
fn every_reference_block_parses() {
    let parser = CapyParser::new().unwrap();
    let text =
        std::fs::read_to_string("docs/references/ref-2026-0002-language-and-usage.md").unwrap();
    let mut n = 0;
    let mut failures = Vec::new();
    for block in text.split("```rivet\n").skip(1) {
        let src = block.split("```").next().unwrap();
        n += 1;
        let tree = parser
            .parse(&SourceFile {
                path: format!("ref-block-{n}"),
                text: src.to_string(),
            })
            .unwrap();
        if !tree.is_clean() {
            failures.push(format!("block {n}: {:?}\n{src}", tree.diagnostics));
        }
    }
    assert!(n >= 90, "only {n} blocks found");
    assert!(failures.is_empty(), "{}", failures.join("\n---\n"));
}

/// Structural lowering errors that mean the grammar or lowering misread a
/// documented shape (as opposed to fragments referring to names defined elsewhere).
const STRUCTURAL: &[&str] = &[
    "syntax.option_expected",
    "syntax.trailing",
    "syntax.statement",
    "syntax.option_misplaced",
    "syntax.top_level",
    "syntax.unknown_statement",
    "syntax.expression",
    "syntax.with",
    "syntax.map_yield",
    "syntax.yield",
];

// vhco:test language.compile_program -- every reference body fragment lowers without structural errors inside a wrapper operation
#[test]
fn every_reference_fragment_lowers() {
    let parser = CapyParser::new().unwrap();
    let text =
        std::fs::read_to_string("docs/references/ref-2026-0002-language-and-usage.md").unwrap();
    let mut failures = Vec::new();
    for (n, block) in text.split("```rivet\n").skip(1).enumerate() {
        let src = block.split("```").next().unwrap();
        let first = src.trim_start();
        let is_bundle = ["operation ", "pipeline ", "connector ", "auth "]
            .iter()
            .any(|k| first.starts_with(k))
            && !first.starts_with("auth ")
            || first.lines().next().is_some_and(|l| l.ends_with("oauth2"));
        let wrapped = if is_bundle {
            src.to_string()
        } else {
            let body: String = src
                .lines()
                .map(|l| {
                    if l.is_empty() {
                        "\n".to_string()
                    } else {
                        format!("    {l}\n")
                    }
                })
                .collect();
            let emits = if src.contains("emit ") {
                "    emits json\n"
            } else {
                ""
            };
            let receives = if src.contains(" incoming") {
                "    receives json\n"
            } else {
                ""
            };
            format!("operation example.run\n    output json\n{emits}{receives}{body}end\n")
        };
        let bundle = SourceBundle {
            entry: format!("ref-{n}.rivet"),
            root: ".".into(),
            files: vec![SourceFile {
                path: format!("ref-{n}.rivet"),
                text: wrapped.clone(),
            }],
            modules: Vec::new(),
        };
        if let Err(e) = compile_program(&bundle, &parser) {
            for err in std::iter::once(&e).chain(e.suppressed.iter()) {
                if STRUCTURAL.contains(&err.code.as_str()) {
                    failures.push(format!(
                        "block {n}: {}\n{wrapped}",
                        err.render(Some(&wrapped))
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} structural failures:\n{}",
        failures.len(),
        failures.join("\n---\n")
    );
}
