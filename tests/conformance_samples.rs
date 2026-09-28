//! T-29 — sample corpus: every demo bundle compiles; every reference example parses.

use rivet::domain::ports::Parser;
use rivet::domain::source::{SourceBundle, SourceFile};
use rivet::features::language::compile_program::compile_program;
use rivet::infra::capy_parser::CapyParser;

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
        };
        if let Err(e) = compile_program(&bundle, &parser) {
            failures.push(e.render(Some(&text)));
            for s in &e.suppressed {
                failures.push(format!("  also: {}", s.render(Some(&text))));
            }
        }
    }
    assert_eq!(dirs.len(), 12);
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
