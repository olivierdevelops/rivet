//! T-16–T-18, T-20 — file modules (PROP-2026-0002 R19–R22, R24; UC-10, UC-11).
//!
//! ```text
//!  T-16  import compiles each file once · ALIAS.ID (transitive) · internal vs public · (alias.id …) and request
//!        · calls inside a module use its own IDs · per-module globals, connectors
//!  T-17  syntax.import · not_found.import (4) · permission.import_outside_root (3) · check.import_cycle
//!        · check.import_duplicate · check.import_collision · limit.imports (5) — each with its span
//!  T-18  Runtime::builder().root(dir) · load / load_as → Module · operations/describe/outputs/call/stream
//!        · concurrent loads while requests run · in-flight requests keep their snapshot
//!  T-20  the loader's policy only · module policy.json ignored with a warning · io/graph/generate cover modules
//! ```
#![allow(clippy::result_large_err)]

use rivet::internal::domain::contracts::Envelope;
use rivet::internal::domain::envelope::{EnvelopeStatus, InputEnvelope};
use rivet::internal::domain::io_manifest::IoQuery;
use rivet::internal::domain::policy::Capability;
use rivet::internal::domain::{RivetError, RivetResult, Value};
use rivet::internal::orchestrator::runtime::policy_from_json;
use rivet::{Module, Runtime};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Write `files` (relative path → text) below a fresh temporary root.
fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let p = dir.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

fn entry(dir: &Path) -> String {
    dir.join("app.rivet").display().to_string()
}

fn build(dir: &Path) -> RivetResult<Runtime> {
    Runtime::builder().file(&entry(dir)).build()
}

/// Every diagnostic of a failed build (first + suppressed).
fn all(e: &RivetError) -> Vec<&RivetError> {
    std::iter::once(e).chain(e.suppressed.iter()).collect()
}

/// The one diagnostic with `code`: (file name, line, column, exit code).
fn diag(dir: &Path, code: &str) -> (String, u32, u32, i32) {
    let e = build(dir).err().expect("the bundle must fail");
    let found: Vec<_> = all(&e).into_iter().filter(|d| d.code == code).collect();
    assert_eq!(found.len(), 1, "{code}: {:#?}", all(&e));
    let s = found[0].source.clone().expect("span");
    let file = PathBuf::from(&s.file)
        .strip_prefix(dir)
        .map(|p| p.display().to_string())
        .unwrap_or(s.file.clone());
    (file, s.start_line, s.start_col, found[0].exit_code())
}

fn rivet(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(args)
        .output()
        .expect("run rivet");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

const USERS: &str = r#"global api = "users-module"

operation get
    param id integer required min 1
    output json
    return {id: id, from: api}
end

operation list
    output json
    return [(get {id: 1}), (request "get" {id: 2})]
end

operation helper
    private true
    output json
    return 7
end

operation count
    param n integer default 3
    output integer
    emits integer
    i = 0
    iterate max 100
        if i == n
            break
        end
        i = i + 1
        emit i
    end
    return n
end
"#;

const BILLING: &str = r#"import "../users.rivet" as people
import "./tax.rivet" as tax public
import "./audit.rivet" as audit

global api = "billing-module"

operation invoices
    param user integer required
    output json
    who = (people.get {id: user})
    return {user: who, total: (tax.rate {}), log: (audit.note {}), from: api}
end

operation pay
    output json
    return (request "invoices" {user: 1})
end
"#;

const TAX: &str = r#"operation rate
    output number
    return 0.2
end
"#;

const AUDIT: &str = r#"operation note
    output text
    return "noted"
end
"#;

const APP: &str = r#"import "./users.rivet" as users
import "./lib/billing.rivet" as billing public

global api = "entry"

operation report.user
    param id integer required
    output json
    u = (users.get {id: id})
    same = (request "users.get" {id: id})
    inv = (billing.invoices {user: id})
    return {user: u, same: same, invoices: inv, from: api}
end
"#;

fn modules_tree() -> tempfile::TempDir {
    tree(&[
        ("app.rivet", APP),
        ("users.rivet", USERS),
        ("lib/billing.rivet", BILLING),
        ("lib/tax.rivet", TAX),
        ("lib/audit.rivet", AUDIT),
    ])
}

// ------------------------------------------------------------------ T-16

// vhco:test language.resolve_imports -- each file compiles once (users.rivet imported by app and billing), namespaces are ALIAS.ID and transitive (billing.tax.rate), bootstrap files listed
#[test]
fn t16_imports_compile_once_and_namespace() {
    let dir = modules_tree();
    let rt = build(dir.path()).unwrap();
    let p = rt.program();
    let mut ids: Vec<&str> = p.operations.iter().map(|o| o.id.as_str()).collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            "billing.audit.note",
            "billing.invoices",
            "billing.pay",
            "billing.tax.rate",
            "report.user",
            "users.count",
            "users.get",
            "users.helper",
            "users.list",
        ]
    );
    // users.rivet is imported twice (app as `users`, billing as `people`): one module.
    assert_eq!(p.modules.len(), 5);
    let users = p.modules.iter().find(|m| m.alias == "users").unwrap();
    assert_eq!(users.depth, 1);
    let billing = p.modules.iter().find(|m| m.alias == "billing").unwrap();
    assert_eq!(billing.imports[0].target, "users");
    assert_eq!(p.operation("users.get").unwrap().module, "users");
}

// vhco:test language.compile_program -- internal imports are callable but not listed; `public` (transitively) lists them; `(alias.id …)`, `(request "alias.id")`, own-ID calls and per-module globals all work
#[tokio::test]
async fn t16_visibility_calls_and_module_globals() {
    let dir = modules_tree();
    let rt = build(dir.path()).unwrap();
    let mut listed: Vec<String> = rt
        .list()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.id)
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            "billing.invoices",
            "billing.pay",
            "billing.tax.rate",
            "report.user"
        ]
    );
    let out = rt
        .call(InputEnvelope::new("report.user").data(serde_json::json!({"id": 5})))
        .await;
    assert_eq!(out.status, Some(EnvelopeStatus::Ok), "{:?}", out.error);
    let data = out.data.unwrap();
    assert_eq!(
        data["user"],
        serde_json::json!({"id": 5, "from": "users-module"})
    );
    assert_eq!(data["same"], data["user"]);
    assert_eq!(data["invoices"]["total"], 0.2);
    assert_eq!(data["invoices"]["log"], "noted");
    assert_eq!(data["invoices"]["from"], "billing-module");
    assert_eq!(data["from"], "entry");
    // Calls inside a module use its own IDs: (request "invoices") in billing.
    let pay = rt.call(InputEnvelope::new("billing.pay")).await;
    assert_eq!(pay.status, Some(EnvelopeStatus::Ok), "{:?}", pay.error);
    // Internal: not reachable from a surface.
    let hidden = rt
        .call(InputEnvelope::new("users.get").data(serde_json::json!({"id": 1})))
        .await;
    assert_eq!(hidden.error.unwrap().code, "not_found.operation");
    let internal = rt.call(InputEnvelope::new("billing.audit.note")).await;
    assert_eq!(internal.status, Some(EnvelopeStatus::Error));
}

// vhco:test language.compile_program -- a module call that resolves nowhere and a call to another module's private operation are check.unknown_operation
#[test]
fn t16_module_calls_are_checked() {
    let dir = tree(&[
        (
            "app.rivet",
            "import \"./users.rivet\" as users\n\noperation a.b\n    output json\n    return (users.helper {})\nend\n",
        ),
        ("users.rivet", USERS),
    ]);
    let e = build(dir.path()).err().unwrap();
    assert!(
        all(&e).iter().any(|d| d.code == "check.unknown_operation"
            && d.message.contains("private to module `users`")),
        "{e:#?}"
    );
    let dir = tree(&[
        ("app.rivet", "import \"./m.rivet\" as m\n"),
        (
            "m.rivet",
            "operation x\n    output json\n    return (request \"report.user\" {})\nend\n",
        ),
    ]);
    let (file, line, col, exit) = diag(dir.path(), "check.unknown_operation");
    assert_eq!((file.as_str(), line, col, exit), ("m.rivet", 3, 13, 2));
}

// vhco:test language.compile_program -- connectors stay inside their module: the importer cannot reference a module's connector, and a connector name declared in two files collides
#[test]
fn t16_module_connectors_stay_inside() {
    let conn = "connector svc grpc\n    endpoint \"http://127.0.0.1:1\"\n    descriptor \"./svc.pb\"\n    service \"example.Users\"\nend\n";
    let dir = tree(&[
        (
            "app.rivet",
            "import \"./m.rivet\" as m\n\noperation a.b\n    output json\n    r = grpc svc.GetUser\n        message {id: \"1\"}\n    end\n    return r\nend\n",
        ),
        ("m.rivet", conn),
    ]);
    let e = build(dir.path()).err().unwrap();
    assert!(
        all(&e).iter().any(|d| d.code == "check.unknown_connector"),
        "{e:#?}"
    );
    let dir = tree(&[
        ("app.rivet", &format!("import \"./m.rivet\" as m\n\n{conn}")),
        ("m.rivet", conn),
    ]);
    let (file, line, col, _) = diag(dir.path(), "check.import_collision");
    assert_eq!((file.as_str(), line, col), ("m.rivet", 1, 1));
}

// ------------------------------------------------------------------ T-17

// vhco:test language.resolve_imports -- syntax.import: malformed, after a declaration, absolute path and reserved alias, each at the import line (exit 2)
#[test]
fn t17_syntax_import() {
    for (text, line) in [
        ("import users\n", 1),
        ("import \"./a.rivet\" users\n", 1),
        ("import \"./a.rivet\" as a public now\n", 1),
        (
            "operation x\n    output json\n    return 1\nend\nimport \"./a.rivet\" as a\n",
            5,
        ),
        ("import \"/etc/a.rivet\" as a\n", 1),
        ("import \"./a.rivet\" as rivet\n", 1),
        ("global g = 1\nimport \"./a.rivet\" as a\n", 2),
    ] {
        let dir = tree(&[("app.rivet", text), ("a.rivet", AUDIT)]);
        let (file, l, col, exit) = diag(dir.path(), "syntax.import");
        assert_eq!(
            (file.as_str(), l, col, exit),
            ("app.rivet", line, 1, 2),
            "{text}"
        );
    }
    // Inside an operation.
    let dir = tree(&[(
        "app.rivet",
        "operation x\n    output json\n    import \"./a.rivet\" as a\n    return 1\nend\n",
    )]);
    let (_, l, col, _) = diag(dir.path(), "syntax.import");
    assert_eq!((l, col), (3, 5));
}

// vhco:test language.resolve_imports -- not_found.import at the import (exit 4); `rivet check` renders it and exits 4
#[test]
fn t17_not_found() {
    let dir = tree(&[("app.rivet", "\nimport \"./nope.rivet\" as n\n")]);
    assert_eq!(
        diag(dir.path(), "not_found.import"),
        ("app.rivet".into(), 2, 1, 4)
    );
    let (code, _, err) = rivet(&["--file", &entry(dir.path()), "check"]);
    assert_eq!(code, 4);
    assert!(err.contains("error[not_found.import]"), "{err}");
    assert!(err.contains("app.rivet:2:1"), "{err}");
}

// vhco:test language.resolve_imports -- permission.import_outside_root for a `..` escape (in a nested module too) and for a symlink below the root (exit 3)
#[test]
fn t17_outside_root() {
    let dir = tree(&[("app.rivet", "import \"../x.rivet\" as x\n")]);
    assert_eq!(
        diag(dir.path(), "permission.import_outside_root"),
        ("app.rivet".into(), 1, 1, 3)
    );
    let dir = tree(&[
        ("app.rivet", "import \"./lib/a.rivet\" as a\n"),
        ("lib/a.rivet", "import \"../../x.rivet\" as x\n"),
    ]);
    assert_eq!(
        diag(dir.path(), "permission.import_outside_root"),
        ("lib/a.rivet".into(), 1, 1, 3)
    );
    let outside = tree(&[("x.rivet", AUDIT)]);
    let dir = tree(&[("app.rivet", "import \"./link.rivet\" as x\n")]);
    std::os::unix::fs::symlink(
        outside.path().join("x.rivet"),
        dir.path().join("link.rivet"),
    )
    .unwrap();
    let (file, line, col, exit) = diag(dir.path(), "permission.import_outside_root");
    assert_eq!((file.as_str(), line, col, exit), ("app.rivet", 1, 1, 3));
}

// vhco:test language.resolve_imports -- check.import_cycle names the cycle path at the import that starts it
#[test]
fn t17_cycle() {
    let dir = tree(&[
        ("app.rivet", "import \"./a.rivet\" as a\n"),
        ("a.rivet", "import \"./b.rivet\" as b\n"),
        ("b.rivet", "import \"./a.rivet\" as a\n"),
    ]);
    let e = build(dir.path()).err().unwrap();
    let c = all(&e)
        .into_iter()
        .find(|d| d.code == "check.import_cycle")
        .unwrap();
    assert_eq!(c.message, "import cycle: a.rivet → b.rivet → a.rivet");
    assert_eq!(
        diag(dir.path(), "check.import_cycle"),
        ("a.rivet".into(), 1, 1, 2)
    );
    // UC-10 sample: cyc.rivet → a.rivet → cyc.rivet, reported at cyc.rivet:1:1.
    let dir = tree(&[
        ("app.rivet", "import \"./a.rivet\" as a\n"),
        ("a.rivet", "import \"./app.rivet\" as back\n"),
    ]);
    let e = build(dir.path()).err().unwrap();
    assert_eq!(e.message, "import cycle: app.rivet → a.rivet → app.rivet");
    assert_eq!(e.source.unwrap().start_line, 1);
}

// vhco:test language.resolve_imports -- check.import_duplicate when one file reuses an alias
#[test]
fn t17_duplicate_alias() {
    let dir = tree(&[
        (
            "app.rivet",
            "import \"./a.rivet\" as a\nimport \"./b.rivet\" as a\n",
        ),
        ("a.rivet", AUDIT),
        ("b.rivet", TAX),
    ]);
    assert_eq!(
        diag(dir.path(), "check.import_duplicate"),
        ("app.rivet".into(), 2, 1, 2)
    );
}

// vhco:test language.compile_program -- check.import_collision when a namespaced ID equals a local operation ID, at the import
#[test]
fn t17_collision() {
    let dir = tree(&[
        (
            "app.rivet",
            "import \"./users.rivet\" as users\n\noperation users.get\n    output json\n    return 1\nend\n",
        ),
        ("users.rivet", USERS),
    ]);
    let (file, line, col, exit) = diag(dir.path(), "check.import_collision");
    assert_eq!((file.as_str(), line, col, exit), ("app.rivet", 1, 1, 2));
}

// vhco:test language.resolve_imports -- limit.imports for imports deeper than 16 levels and for more than 256 files (exit 5)
#[test]
fn t17_limits() {
    let mut files: Vec<(String, String)> =
        vec![("app.rivet".into(), "import \"./m1.rivet\" as m1\n".into())];
    for i in 1..=17 {
        files.push((
            format!("m{i}.rivet"),
            format!("import \"./m{}.rivet\" as m{}\n", i + 1, i + 1),
        ));
    }
    files.push(("m18.rivet".into(), AUDIT.into()));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let dir = tree(&refs);
    assert_eq!(
        diag(dir.path(), "limit.imports"),
        ("m16.rivet".into(), 1, 1, 5)
    );

    let mut app = String::new();
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..256 {
        app.push_str(&format!("import \"./f{i}.rivet\" as f{i}\n"));
        files.push((format!("f{i}.rivet"), AUDIT.into()));
    }
    files.push(("app.rivet".into(), app));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let dir = tree(&refs);
    assert_eq!(
        diag(dir.path(), "limit.imports"),
        ("app.rivet".into(), 256, 1, 5)
    );
}

// ------------------------------------------------------------------ T-18

fn host(dir: &Path) -> Runtime {
    Runtime::builder()
        .root(dir.to_str().unwrap())
        .policy(policy_from_json(br#"{"version": 1}"#, dir.to_str().unwrap()).unwrap())
        .build()
        .unwrap()
}

// vhco:test registry.load_module -- a runtime without an entry file loads two modules; Module operations/describe/outputs/call use short IDs and answer envelopes named ALIAS.ID
#[tokio::test]
async fn t18_load_and_call_modules() {
    let dir = modules_tree();
    let rt = host(dir.path());
    assert!(rt.list().unwrap().entries.is_empty());
    let users = rt.load("./users.rivet").unwrap();
    assert_eq!(users.alias(), "users");
    let mut ops: Vec<String> = users.operations().into_iter().map(|e| e.id).collect();
    ops.sort();
    assert_eq!(ops, ["count", "get", "list"]);
    assert_eq!(users.describe("get").unwrap().params[0].name, "id");
    assert_eq!(users.outputs("count").unwrap().id, "users.count");
    let out = users.call("get", serde_json::json!({"id": 42})).await;
    assert_eq!(out.operation.as_deref(), Some("users.get"));
    assert_eq!(out.data.unwrap()["id"], 42);
    let billing = rt.load_as("./lib/billing.rivet", "money").unwrap();
    assert_eq!(billing.alias(), "money");
    let paid = billing.call("pay", serde_json::json!({})).await;
    assert_eq!(paid.status, Some(EnvelopeStatus::Ok), "{:?}", paid.error);
    assert_eq!(paid.operation.as_deref(), Some("money.pay"));
    // The runtime's own dispatcher sees the same catalog.
    let same = rt
        .call(InputEnvelope::new("users.get").data(serde_json::json!({"id": 42})))
        .await;
    assert_eq!(same.data.unwrap()["from"], "users-module");
    // Public nested imports of a loaded module are listed; internal ones are not.
    let listed: Vec<String> = rt
        .list()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert!(listed.contains(&"money.tax.rate".to_string()));
    assert!(!listed.contains(&"money.audit.note".to_string()));
    // Load errors: an alias twice, a bad stem, outside the root, a missing file.
    assert_eq!(
        rt.load("./users.rivet").err().unwrap().code,
        "check.import_duplicate"
    );
    std::fs::write(dir.path().join("my-mod.rivet"), AUDIT).unwrap();
    assert_eq!(
        rt.load("./my-mod.rivet").err().unwrap().code,
        "syntax.import"
    );
    assert_eq!(
        rt.load("../elsewhere.rivet").err().unwrap().code,
        "permission.import_outside_root"
    );
    let missing = rt.load("./missing.rivet").err().unwrap();
    assert_eq!(
        (missing.code.as_str(), missing.exit_code()),
        ("not_found.import", 4)
    );
    // A failed load leaves the catalog as it was.
    assert!(rt.describe(&["users.get".to_string()]).is_ok());
}

// vhco:test registry.load_module -- a module operation streams items through a scope (Module::stream)
#[tokio::test]
async fn t18_module_streams() {
    let dir = modules_tree();
    let rt = host(dir.path());
    let users: Module = rt.load("./users.rivet").unwrap();
    let (items, result) = rt
        .scope(|scope| async move {
            let mut s = users
                .stream(&scope, "count", Value::object([("n", Value::Int(3))]))
                .await?;
            let mut items = 0;
            let mut result = None;
            while let Some(env) = s.next().await? {
                match env {
                    Envelope::Result(c) => result = Some(c.result),
                    _ => items += 1,
                }
            }
            Ok((items, result))
        })
        .await
        .unwrap();
    assert_eq!(items, 3);
    assert_eq!(result, Some(Value::Int(3)));
}

const SLOW: &str = r#"operation slow.call
    param target text required
    output json
    n = 0
    r = poll every "20ms" timeout "5s"
        n = n + 1
        until n == 10
        yield n
    end
    return (request target {})
end

operation quick
    output integer
    return 1
end
"#;

// vhco:test registry.load_module -- loads run while requests are in flight: running requests finish on their snapshot, new ones see every loaded module
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t18_concurrent_loads_during_requests() {
    let mut files: Vec<(String, String)> = vec![("app.rivet".into(), SLOW.into())];
    for i in 0..6 {
        files.push((
            format!("m{i}.rivet"),
            format!("operation ping\n    output integer\n    return {i}\nend\n"),
        ));
    }
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let dir = tree(&refs);
    let rt = build(dir.path()).unwrap();
    // Requests started before the loads: the dynamic target does not exist in
    // their snapshot, so the nested call is not_found even after m0 loads.
    let mut running = Vec::new();
    for _ in 0..8 {
        let r = rt.clone();
        running.push(tokio::spawn(async move {
            r.call(InputEnvelope::new("slow.call").data(serde_json::json!({"target": "m0.ping"})))
                .await
        }));
    }
    // Plain requests keep flowing during the loads.
    let mut quick = Vec::new();
    for _ in 0..16 {
        let r = rt.clone();
        quick.push(tokio::spawn(async move {
            for _ in 0..10 {
                let out = r.call(InputEnvelope::new("quick")).await;
                assert_eq!(out.status, Some(EnvelopeStatus::Ok));
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }));
    }
    tokio::time::sleep(Duration::from_millis(30)).await;
    let mut loads = Vec::new();
    for i in 0..6 {
        let r = rt.clone();
        loads.push(tokio::task::spawn_blocking(move || {
            r.load(&format!("./m{i}.rivet"))
                .map(|m| m.alias().to_string())
        }));
    }
    for l in loads {
        assert!(l.await.unwrap().is_ok());
    }
    for q in quick {
        q.await.unwrap();
    }
    for t in running {
        let out = t.await.unwrap();
        assert_eq!(out.status, Some(EnvelopeStatus::Error));
        assert_eq!(out.error.unwrap().code, "not_found.operation");
    }
    // A request started after the loads sees every module.
    for i in 0..6 {
        let out = rt
            .call(
                InputEnvelope::new("slow.call")
                    .data(serde_json::json!({"target": format!("m{i}.ping")})),
            )
            .await;
        assert_eq!(out.data, Some(serde_json::json!(i)), "{:?}", out.error);
    }
}

// ------------------------------------------------------------------ T-20

const DATA_MODULE: &str = r#"operation public_file
    output json
    v = file read "./data/public.json" as json
    return v
end

operation secret_file
    output json
    v = file read "./secret/key.json" as json
    return v
end
"#;

fn policy_tree() -> tempfile::TempDir {
    tree(&[
        ("app.rivet", "import \"./lib/data.rivet\" as data public\n"),
        ("lib/data.rivet", DATA_MODULE),
        (
            "lib/policy.json",
            r#"{"version": 1, "grants": [{"capability": "allow_read", "targets": ["./**"]}]}"#,
        ),
        (
            "policy.json",
            r#"{"version": 1, "grants": [{"capability": "allow_read", "targets": ["./data/**"]}]}"#,
        ),
        ("data/public.json", r#"{"ok": true}"#),
        ("secret/key.json", r#"{"key": "s3cr3t"}"#),
    ])
}

// vhco:test language.resolve_imports -- modules run under the loader's policy only: the module's own policy.json is ignored (warning check.module_policy_ignored at the import) and cannot widen access
#[tokio::test]
async fn t20_loader_policy_only() {
    let dir = policy_tree();
    let rt = build(dir.path()).unwrap();
    let program = rt.program();
    let w: Vec<&RivetError> = program
        .warnings
        .iter()
        .filter(|w| w.code == "check.module_policy_ignored")
        .collect();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].source.as_ref().unwrap().start_line, 1);
    let ok = rt.call(InputEnvelope::new("data.public_file")).await;
    assert_eq!(
        ok.data,
        Some(serde_json::json!({"ok": true})),
        "{:?}",
        ok.error
    );
    let denied = rt.call(InputEnvelope::new("data.secret_file")).await;
    assert_eq!(denied.error.unwrap().code, "permission.denied");
    // `rivet check` prints the warning and still exits 0.
    let (code, _, err) = rivet(&["--file", &entry(dir.path()), "check"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("warning[check.module_policy_ignored]"),
        "{err}"
    );
    // A host-loaded module with its own policy.json: same rule, warning in the summary.
    let rt = Runtime::builder()
        .root(dir.path().to_str().unwrap())
        .policy(
            policy_from_json(
                br#"{"version": 1, "grants": [{"capability": "allow_read", "targets": ["./data/**"]}]}"#,
                dir.path().to_str().unwrap(),
            )
            .unwrap(),
        )
        .build()
        .unwrap();
    let m = rt.load("./lib/data.rivet").unwrap();
    assert_eq!(m.warnings().len(), 1);
    assert_eq!(
        m.call("secret_file", serde_json::json!({}))
            .await
            .error
            .unwrap()
            .code,
        "permission.denied"
    );
}

// vhco:test audit.inspect_effects -- io, graph and policy generate cover every module with module spans (lib/data.rivet:3), namespaced IDs and one bootstrap row per file
#[test]
fn t20_manifest_graph_and_generate_cover_modules() {
    let dir = policy_tree();
    let rt = build(dir.path()).unwrap();
    let m = rt
        .io(&IoQuery {
            all: true,
            include_bootstrap: true,
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap()
        .manifest;
    let site = m
        .sites
        .iter()
        .find(|s| s.operation_id == "data.public_file")
        .unwrap();
    assert_eq!(site.source.short(), "lib/data.rivet:3");
    let boot: Vec<&str> = m
        .bootstrap
        .iter()
        .map(|s| s.target.template.as_str())
        .collect();
    assert_eq!(&boot[..2], ["./app.rivet", "./lib/data.rivet"]);
    let g = rt
        .graph(&rivet::internal::domain::call_graph::GraphQuery {
            id: "data.public_file".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(g.render().contains("lib/data.rivet:3"), "{}", g.render());
    let d = rt.generate_policy(&["data.public_file"]).unwrap();
    let read = d
        .grants
        .iter()
        .find(|g| g.capability == Capability::Read)
        .unwrap();
    assert_eq!(read.targets, vec!["./data/public.json"]);
    let (code, out, _) = rivet(&["--file", &entry(dir.path()), "io", "--include-bootstrap"]);
    assert_eq!(code, 0);
    assert!(out.contains("./lib/data.rivet"), "{out}");
    assert!(out.contains("data.secret_file"), "{out}");
    let (_, out, _) = rivet(&[
        "--file",
        &entry(dir.path()),
        "policy",
        "explain",
        "data.secret_file",
    ]);
    assert!(out.contains("lib/data.rivet:9"), "{out}");
}
