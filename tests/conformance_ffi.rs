//! T-11, T-12 (exports), T-19 — librivet from C and Python (PROP-2026-0002 R13–R15, R23).
//!
//! ```text
//!  cargo test --workspace ──builds──▶ target/<profile>/deps/librivet.{dylib|so} + librivet.a
//!        │
//!        ├─ nm: only rivet_* exported ; macOS install name @rpath/librivet.dylib
//!        ├─ make -C examples/c TARGET=… OUT=tmp all ─▶ demo / demo_static / modules / modules_static
//!        │     each run: envelopes, stream records, live input, cancel, highlight, module objects
//!        ├─ RIVET_LIB=… python3 examples/python/{demo,modules}.py   (ctypes wrapper)
//!        └─ ffi/include/rivet.h == cbindgen output (when cbindgen is installed)
//! ```
//!
//! The library comes from the same `cargo test --workspace` build (rivet-ffi is
//! a workspace member). Run alone (`-p rivet-runtime`) without it, the suite
//! skips with a note; in CI (`CI` set) a missing library fails.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn shared_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "librivet.dylib"
    } else {
        "librivet.so"
    }
}

/// Where this build put librivet: `cargo test` leaves library artifacts in
/// `target/<profile>/deps` (next to this test binary); `cargo build` also
/// copies them to `target/<profile>`.
fn target_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let deps = exe.parent()?.to_path_buf();
    let profile = deps.parent()?.to_path_buf();
    for dir in [&deps, &profile] {
        if dir.join(shared_name()).exists() && dir.join("librivet.a").exists() {
            return Some(dir.clone());
        }
    }
    let msg = format!(
        "librivet is not built in {} — run `cargo test --workspace` (rivet-ffi is a workspace member)",
        deps.display()
    );
    if std::env::var_os("CI").is_some() {
        panic!("{msg}");
    }
    eprintln!("skipped: {msg}");
    None
}

fn run(cmd: &mut Command) -> Output {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(
        out.status.success(),
        "{cmd:?} failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The JSON after `label ` on the first line starting with it.
fn line_json(text: &str, label: &str) -> serde_json::Value {
    let prefix = format!("{label} ");
    let line = text
        .lines()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no `{label}` line in:\n{text}"));
    serde_json::from_str(&line[prefix.len()..]).unwrap_or_else(|e| panic!("{e}: {line}"))
}

fn records(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("record "))
        .map(|j| serde_json::from_str(j).unwrap())
        .collect()
}

/// Build the four C programs into a temp dir against `target`.
fn build_c(target: &Path, out: &Path) {
    run(Command::new("make")
        .arg("-s")
        .arg("-C")
        .arg(Path::new(ROOT).join("examples/c"))
        .arg(format!("TARGET={}", target.display()))
        .arg(format!("OUT={}", out.display()))
        .arg("all"));
}

// vhco:test execution.request_operation -- T-11: librivet exports only rivet_* symbols (18), and on macOS its install name is @rpath/librivet.dylib
#[test]
fn t11_exports_only_rivet_symbols() {
    let Some(target) = target_dir() else { return };
    let lib = target.join(shared_name());
    let nm = if cfg!(target_os = "macos") {
        run(Command::new("nm").arg("-gU").arg(&lib))
    } else {
        run(Command::new("nm").args(["-D", "--defined-only"]).arg(&lib))
    };
    let symbols: Vec<String> = stdout(&nm)
        .lines()
        .filter_map(|l| l.split_whitespace().nth(2))
        .map(|s| s.trim_start_matches('_').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let rivet: Vec<&String> = symbols.iter().filter(|s| s.starts_with("rivet_")).collect();
    assert_eq!(rivet.len(), 18, "{rivet:?}");
    if cfg!(target_os = "macos") {
        assert_eq!(
            symbols.len(),
            18,
            "only rivet_* may be exported: {symbols:?}"
        );
        let otool = stdout(&run(Command::new("otool").arg("-D").arg(&lib)));
        assert!(otool.contains("@rpath/librivet.dylib"), "{otool}");
    }
    for want in [
        "rivet_abi_version",
        "rivet_version",
        "rivet_runtime_new",
        "rivet_runtime_free",
        "rivet_request",
        "rivet_call_start",
        "rivet_call_next",
        "rivet_call_send",
        "rivet_call_finish_input",
        "rivet_call_cancel",
        "rivet_call_free",
        "rivet_highlight",
        "rivet_load",
        "rivet_module_operations",
        "rivet_module_call",
        "rivet_module_call_start",
        "rivet_module_free",
        "rivet_string_free",
    ] {
        assert!(symbols.iter().any(|s| s == want), "{want} missing");
    }
}

// vhco:test sessions.read_events -- T-11: the C demo linked shared and static prints envelopes, a stream (3 data + result), live input echoes, a cancelled call, highlight tokens, and refuses a double free
#[test]
fn t11_c_demo_shared_and_static() {
    let Some(target) = target_dir() else { return };
    let out = tempfile::tempdir().unwrap();
    build_c(&target, out.path());
    for program in ["demo", "demo_static"] {
        let text = stdout(&run(Command::new(out.path().join(program))
            .arg("../ffi/app.rivet")
            .current_dir(Path::new(ROOT).join("examples/c"))));
        let version = env!("CARGO_PKG_VERSION");
        assert!(
            text.starts_with(&format!("abi 1 version {version}\n")),
            "{program}: {text}"
        );
        assert_eq!(
            line_json(&text, "options-error")["error"]["code"],
            "validation.ffi_argument"
        );
        let req = line_json(&text, "request");
        assert_eq!(
            (req["status"].as_str(), req["data"].as_i64()),
            (Some("ok"), Some(5))
        );
        assert!(
            text.contains("pretty {\n  \"request_id\""),
            "pretty is indented"
        );
        assert_eq!(
            line_json(&text, "invalid")["error"]["code"],
            "validation.type"
        );
        let recs = records(&text);
        // events.count: 3 data + result; chat.echo: 2 data + result; cancelled: 1 result.
        let kinds: Vec<(&str, &str)> = recs
            .iter()
            .map(|r| {
                (
                    r["operation"].as_str().unwrap(),
                    r["type"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            kinds,
            [
                ("events.count", "data"),
                ("events.count", "data"),
                ("events.count", "data"),
                ("events.count", "result"),
                ("chat.echo", "data"),
                ("chat.echo", "data"),
                ("chat.echo", "result"),
                ("chat.echo", "result"),
            ],
            "{program}"
        );
        assert_eq!(recs[4]["data"], "hi");
        assert_eq!(recs[6]["data"], 2);
        assert_eq!(recs[7]["status"], "cancelled");
        assert!(text.contains("stream-records 4\n"));
        assert!(text.contains("chat-records 3\n"));
        assert!(text.contains("cancel-records 1\n"));
        assert_eq!(line_json(&text, "first")["data"], "ping");
        assert_eq!(
            line_json(&text, "poll"),
            serde_json::json!({"type": "timeout"})
        );
        assert_eq!(line_json(&text, "highlight")["class"], "keyword");
        assert!(text.contains("free 0\nfree-again 1\n"), "{text}");
    }
    // The static binary does not load librivet at run time.
    if cfg!(target_os = "macos") {
        let deps = stdout(&run(Command::new("otool")
            .arg("-L")
            .arg(out.path().join("demo_static"))));
        assert!(!deps.contains("librivet"), "{deps}");
        let deps = stdout(&run(Command::new("otool")
            .arg("-L")
            .arg(out.path().join("demo"))));
        assert!(deps.contains("@rpath/librivet.dylib"), "{deps}");
    }
}

// vhco:test registry.load_module -- T-19: the C module example (shared and static) loads users.rivet and lib/billing.rivet as module objects, calls them by short ID and refuses a duplicate alias
#[test]
fn t19_c_modules_shared_and_static() {
    let Some(target) = target_dir() else { return };
    let out = tempfile::tempdir().unwrap();
    build_c(&target, out.path());
    for program in ["modules", "modules_static"] {
        let text = stdout(&run(Command::new(out.path().join(program))
            .arg("../modules")
            .current_dir(Path::new(ROOT).join("examples/c"))));
        let ops = line_json(&text, "operations");
        let ids: Vec<&str> = ops
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["get", "list"], "{program}");
        let call = line_json(&text, "call");
        assert_eq!(call["operation"], "users.get");
        assert_eq!(call["data"]["id"], 42);
        assert_eq!(
            line_json(&text, "bad-call")["error"]["code"],
            "validation.min"
        );
        assert!(text.contains("\"operation\":\"billing.invoice\""), "{text}");
        assert_eq!(
            line_json(&text, "duplicate")["error"]["code"],
            "check.import_duplicate"
        );
        let recs = records(&text);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["operation"], "users.list");
        assert_eq!(line_json(&text, "request")["operation"], "users.get");
        assert!(text.contains("module-free-again 1\n"));
    }
}

fn python(script: &str, target: &Path) -> Option<String> {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipped: python3 is not installed");
        return None;
    }
    let out = run(Command::new("python3")
        .arg(Path::new(ROOT).join("examples/python").join(script))
        .env("RIVET_LIB", target.join(shared_name())));
    Some(stdout(&out))
}

// vhco:test execution.request_operation -- T-11: the Python ctypes demo requests, streams, sends live input, cancels and highlights through librivet
#[test]
fn t11_python_demo() {
    let Some(target) = target_dir() else { return };
    let Some(text) = python("demo.py", &target) else {
        return;
    };
    let want = format!(
        "abi 1 version {}\noptions-error validation.ffi_argument\n",
        env!("CARGO_PKG_VERSION")
    );
    assert!(text.starts_with(&want), "{text}");
    assert_eq!(line_json(&text, "request")["data"], 5);
    for line in [
        "invalid error validation.type",
        "stream [1, 2, 3, {'count': 3}] result ok",
        "send 1",
        "send 2",
        "chat ['hi', 'there', 2]",
        "first ping",
        "poll timeout",
        "cancel cancelled cancelled.session",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing `{line}` in:\n{text}"
        );
    }
    assert_eq!(line_json(&text, "highlight")["text"], "global");
}

// vhco:test registry.load_module -- T-19: the Python wrapper exposes a module object whose attributes are its operations (users.get(id=42))
#[test]
fn t19_python_modules() {
    let Some(target) = target_dir() else { return };
    let Some(text) = python("modules.py", &target) else {
        return;
    };
    for line in [
        "operations ['get', 'list']",
        "get users.get {\"id\": 42, \"name\": \"Hello, user 42\"}",
        "list [1, 2]",
        "invoice {\"user\": {\"id\": 7, \"name\": \"Hello, user 7\"}, \"amount\": 20.0}",
        "duplicate check.import_duplicate",
        "no-such-operation module 'users' has no operation 'missing'",
        "request Hello, user 7",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing `{line}` in:\n{text}"
        );
    }
}

// vhco:test execution.request_operation -- T-11: the checked-in ffi/include/rivet.h is exactly what cbindgen generates from ffi/src/lib.rs
#[test]
fn t11_header_matches_cbindgen() {
    if Command::new("cbindgen").arg("--version").output().is_err() {
        eprintln!("skipped: cbindgen is not installed (CI regenerates and diffs the header)");
        return;
    }
    let out = tempfile::tempdir().unwrap();
    let generated = out.path().join("rivet.h");
    run(Command::new("cbindgen")
        .args([
            "--config",
            "ffi/cbindgen.toml",
            "--crate",
            "rivet-ffi",
            "--output",
        ])
        .arg(&generated)
        .current_dir(ROOT));
    let want = std::fs::read_to_string(Path::new(ROOT).join("ffi/include/rivet.h")).unwrap();
    let got = std::fs::read_to_string(&generated).unwrap();
    assert_eq!(
        got, want,
        "ffi/include/rivet.h is stale: regenerate it with cbindgen"
    );
}

// vhco:test execution.request_operation -- T-11: rivet.pc renders with the workspace version and this OS's Libs.private (the same list the C Makefile links statically)
#[test]
fn t11_pkg_config_template() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipped: python3 is not installed");
        return;
    }
    let pc = stdout(&run(Command::new("python3")
        .args(["ffi/render_pc.py", "--prefix", "/opt/rivet"])
        .current_dir(ROOT)));
    assert!(pc.contains("prefix=/opt/rivet\n"), "{pc}");
    assert!(
        pc.contains(&format!("Version: {}\n", env!("CARGO_PKG_VERSION"))),
        "{pc}"
    );
    assert!(pc.contains("Libs: -L${libdir} -lrivet\n"), "{pc}");
    assert!(!pc.contains('@'), "unrendered placeholder: {pc}");
    let private = pc
        .lines()
        .find_map(|l| l.strip_prefix("Libs.private: "))
        .unwrap()
        .to_string();
    assert!(!private.contains("-lSystem"), "{private}");
    let makefile = std::fs::read_to_string(Path::new(ROOT).join("examples/c/Makefile")).unwrap();
    assert!(
        makefile.contains(&format!("LIBS_PRIVATE := {private}\n")),
        "examples/c/Makefile and rivet.pc disagree on Libs.private ({private})"
    );
}
