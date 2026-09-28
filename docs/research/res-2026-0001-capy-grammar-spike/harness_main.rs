use capy_core::capy::Library;
use capy_core::domain::ast_json;
use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().collect();
    let lib_src = fs::read_to_string(&args[1]).unwrap();
    let lib = match Library::new(&lib_src) { Ok(l) => l, Err(e) => { eprintln!("LIB ERROR: {:?}", e); std::process::exit(2) } };
    let corpus = fs::read_to_string(&args[2]).unwrap();
    // corpus: blocks separated by lines "=====SRC <name>"
    let mut ok = 0; let mut bad = 0;
    let dump = args.get(3).map(|s| s.as_str());
    for chunk in corpus.split("=====SRC ").filter(|c| !c.trim().is_empty()) {
        let (name, src) = chunk.split_once('\n').unwrap();
        let r = lib.parse(src);
        if Some(name) == dump { println!("{}", ast_json::to_json_pretty(&r)); }
        if dump == Some("ALL") { println!("@@{}\t{}", name, ast_json::to_json(&r)); }
        if r.is_clean() { ok += 1; } else {
            bad += 1;
            println!("--- {}", name);
            for d in &r.diagnostics {
                let line = src.lines().nth(d.primary.start_line.saturating_sub(1) as usize).unwrap_or("");
                println!("  {}:{} {} | {}", d.primary.start_line, d.primary.start_col, d.msg, line.trim());
            }
        }
    }
    println!("CLEAN {} / {}", ok, ok + bad);
}
