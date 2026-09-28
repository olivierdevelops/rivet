//! Developer tool: print Rivet's SyntaxTree for a `.rivet` file.
//!
//! `cargo run --example dump_ast -- path/to/app.rivet`

use rivet::internal::domain::ports::Parser;
use rivet::internal::domain::source::SourceFile;
use rivet::internal::domain::syntax_tree::SyntaxNode;
use rivet::internal::infra::capy_parser::CapyParser;

fn show(n: &SyntaxNode, depth: usize) {
    let caps: Vec<String> = n
        .captures
        .iter()
        .map(|(k, c)| {
            format!(
                "{k}={:?}@{:?}",
                c.source_text,
                c.span
                    .as_ref()
                    .map(|s| (s.start_line, s.start_col, s.end_col))
            )
        })
        .collect();
    println!("{}{} {}", "  ".repeat(depth), n.func, caps.join(" "));
    for c in n.children() {
        show(c, depth + 1);
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_ast FILE");
    let text = std::fs::read_to_string(&path).expect("readable file");
    let tree = CapyParser::new()
        .expect("grammar")
        .parse(&SourceFile { path, text })
        .expect("parse");
    for n in &tree.nodes {
        show(n, 0);
    }
    if !tree.diagnostics.is_empty() {
        println!("{:#?}", tree.diagnostics);
    }
}
