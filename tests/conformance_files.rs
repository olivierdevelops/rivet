//! T-04 — file CRUD through the Runtime (PROP-2026-0001 Increment 4,
//! REF-2026-0002 S33–S40, S66, S139, S146): verb semantics, confinement
//! (`..`, absolute paths, symlinks, hard links), policy interplay and atomic
//! replacement. Every test works in its own temporary bundle root.
//!
//! ```text
//!  op source ──▶ Runtime.request ──▶ files.apply_file_operation ──▶ policy (per intent)
//!                                            │ allowed
//!                                            ▼
//!                                   ConfinedFiles (cap-std Dir, no-follow)
//!                                            │
//!         create (O_EXCL) · read · update (tmp + rename) · write · append
//!         delete · list · stat · copy/move (overwrite false by default)
//! ```

use rivet::Runtime;
use rivet::domain::{ErrorKind, RivetError, Value};
use rivet::orchestrator::runtime::policy_from_json;
use std::path::Path;

/// Read, write and delete granted under `./out/**`, read under `./data/**`.
const FULL: &str = r#"{"version":1,"grants":[
    {"capability":"allow_read","targets":["./out","./out/**","./data","./data/**"]},
    {"capability":"allow_write","targets":["./out/**"]},
    {"capability":"allow_delete","targets":["./out/**"]}]}"#;

fn op(body: &str) -> String {
    let indented: String = body.lines().map(|l| format!("    {l}\n")).collect();
    format!("operation t.run\n    output json\n{indented}end\n\noperation t.pure\n    output integer\n    return 7\nend\n")
}

fn runtime(root: &Path, src: &str, policy: Option<&str>) -> Runtime {
    let root = root.to_str().unwrap();
    let mut b = Runtime::builder().source("app.rivet", src, root);
    if let Some(p) = policy {
        b = b.policy(policy_from_json(p.as_bytes(), root).expect("policy"));
    }
    b.build().expect("runtime")
}

async fn run_with(root: &Path, body: &str, policy: Option<&str>) -> Result<Value, RivetError> {
    runtime(root, &op(body), policy)
        .request("t.run", Value::Null, None)
        .await
        .map(|c| c.result)
}

async fn run(root: &Path, body: &str) -> Result<Value, RivetError> {
    run_with(root, body, Some(FULL)).await
}

/// A bundle root with `out/` and `data/` (nothing else).
fn bundle() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("out")).unwrap();
    std::fs::create_dir(tmp.path().join("data")).unwrap();
    tmp
}

fn read(p: impl AsRef<Path>) -> String {
    std::fs::read_to_string(p).unwrap()
}

fn json_file(p: impl AsRef<Path>) -> serde_json::Value {
    serde_json::from_str(&read(p)).unwrap()
}

fn code(e: &RivetError) -> (&str, i32) {
    (e.code.as_str(), e.exit_code())
}

/// Every entry name below `dir`, recursively (temp files included).
fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().to_string();
        if e.file_type().unwrap().is_dir() {
            for sub in tree(&e.path()) {
                out.push(format!("{name}/{sub}"));
            }
        }
        out.push(name);
    }
    out.sort();
    out
}

// ---------------------------------------------------------------- verbs

// vhco:test files.apply_file_operation -- S33 create is exclusive: an existing destination is conflict.already_exists and stays unchanged
#[tokio::test]
async fn create_is_exclusive() {
    let b = bundle();
    let v = run(
        b.path(),
        "r = file create \"./out/config.json\" json {enabled: true}\nreturn r",
    )
    .await
    .unwrap();
    assert_eq!(v.get("created"), Some(&Value::Bool(true)));
    assert!(v.get("version").and_then(Value::as_str).is_some());
    assert_eq!(
        json_file(b.path().join("out/config.json")),
        serde_json::json!({"enabled": true})
    );
    let e = run(
        b.path(),
        "file create \"./out/config.json\" json {enabled: false}\nreturn null",
    )
    .await
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(e.code, "conflict.already_exists");
    assert_eq!(
        json_file(b.path().join("out/config.json")),
        serde_json::json!({"enabled": true})
    );
}

// vhco:test files.apply_file_operation -- Increment 4 parent directory creation is explicit: create/write/copy into a missing directory fail not_found and create nothing
#[tokio::test]
async fn parent_directories_are_never_created_implicitly() {
    let b = bundle();
    for body in [
        "file create \"./out/new/a.json\" json {a: 1}\nreturn null",
        "file write \"./out/new/a.txt\" text \"x\"\nreturn null",
    ] {
        let e = run(b.path(), body).await.unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound, "{body}: {e:?}");
    }
    std::fs::write(b.path().join("data/in.txt"), "in").unwrap();
    let e = run(
        b.path(),
        "file copy \"./data/in.txt\" to \"./out/new/in.txt\"\nreturn null",
    )
    .await
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert!(!b.path().join("out/new").exists());
}

// vhco:test files.apply_file_operation -- S34 read decodes json/text/bytes; missing is not_found; malformed JSON and non-UTF-8 text are parse errors
#[tokio::test]
async fn read_codecs_and_failures() {
    let b = bundle();
    std::fs::write(b.path().join("data/config.json"), r#"{"enabled":true}"#).unwrap();
    std::fs::write(b.path().join("data/notes.txt"), "héllo\n").unwrap();
    std::fs::write(b.path().join("data/blob.bin"), [0u8, 159, 146, 150]).unwrap();
    std::fs::write(b.path().join("data/bad.json"), "{nope").unwrap();

    let v = run(
        b.path(),
        "config = file read \"./data/config.json\" as json\nreturn config.enabled",
    )
    .await
    .unwrap();
    assert_eq!(v, Value::Bool(true));
    let v = run(b.path(), "return file read \"./data/notes.txt\" as text")
        .await
        .unwrap();
    assert_eq!(v, Value::text("héllo\n"));
    // Default codec follows the extension: .json decodes, anything else is text.
    let v = run(b.path(), "return file read \"./data/config.json\"")
        .await
        .unwrap();
    assert_eq!(v.get("enabled"), Some(&Value::Bool(true)));
    let v = run(b.path(), "return file read \"./data/blob.bin\" as bytes")
        .await
        .unwrap();
    assert_eq!(v, Value::Bytes(vec![0, 159, 146, 150]));

    let e = run(b.path(), "return file read \"./data/missing.json\" as json")
        .await
        .unwrap_err();
    assert_eq!((e.kind, e.exit_code()), (ErrorKind::NotFound, 4));
    let e = run(b.path(), "return file read \"./data/bad.json\" as json")
        .await
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Parse);
    let e = run(b.path(), "return file read \"./data/blob.bin\" as text")
        .await
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Parse);
}

// vhco:test files.apply_file_operation -- S35 update replaces an existing file only; a missing target is not_found and nothing is created
#[tokio::test]
async fn update_replaces_existing_only() {
    let b = bundle();
    let e = run(
        b.path(),
        "file update \"./out/config.json\" json {enabled: false}\nreturn null",
    )
    .await
    .unwrap_err();
    assert_eq!((e.kind, e.exit_code()), (ErrorKind::NotFound, 4));
    assert!(tree(&b.path().join("out")).is_empty(), "no implicit create");

    std::fs::write(b.path().join("out/config.json"), r#"{"enabled":true}"#).unwrap();
    let v = run(
        b.path(),
        "r = file update \"./out/config.json\" json {enabled: false}\nreturn r",
    )
    .await
    .unwrap();
    assert_eq!(v.get("updated"), Some(&Value::Bool(true)));
    assert_eq!(
        json_file(b.path().join("out/config.json")),
        serde_json::json!({"enabled": false})
    );
    assert_eq!(tree(&b.path().join("out")), vec!["config.json"]);
}

// vhco:test files.apply_file_operation -- S36 if_version: the stat version guards the update; a changed file is conflict.version and stays unchanged
#[tokio::test]
async fn update_with_version_guard() {
    let b = bundle();
    std::fs::write(b.path().join("out/config.json"), r#"{"enabled":false}"#).unwrap();
    let body = "info = file stat \"./out/config.json\"\nfile update \"./out/config.json\" json {enabled: true}\n    if_version info.version\nend\nreturn {updated: true, seen: info.version}";
    let v = run(b.path(), body).await.unwrap();
    assert_eq!(v.get("updated"), Some(&Value::Bool(true)));
    assert_eq!(
        json_file(b.path().join("out/config.json")),
        serde_json::json!({"enabled": true})
    );
    // A stale version (the file changed since it was observed) is a conflict.
    let stale = v.get("seen").and_then(Value::as_str).unwrap().to_string();
    let e = run(
        b.path(),
        &format!(
            "file update \"./out/config.json\" json {{enabled: false}}\n    if_version \"{stale}\"\nend\nreturn null"
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(e.code, "conflict.version");
    assert_eq!(
        json_file(b.path().join("out/config.json")),
        serde_json::json!({"enabled": true})
    );
}

// vhco:test files.apply_file_operation -- S37 write creates or replaces explicitly; append adds to an existing file and never creates one
#[tokio::test]
async fn write_upserts_and_append_needs_existing() {
    let b = bundle();
    let e = run(
        b.path(),
        "file append \"./out/events.txt\" text \"early\\n\"\nreturn null",
    )
    .await
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound, "append must not create: {e:?}");
    assert!(!b.path().join("out/events.txt").exists());

    let v = run(
        b.path(),
        "a = file write \"./out/events.txt\" text \"started\\n\"\nb = file append \"./out/events.txt\" text \"finished\\n\"\nreturn {a: a, b: b}",
    )
    .await
    .unwrap();
    assert_eq!(
        v.get("a").unwrap().get("created"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        v.get("b").unwrap().get("appended"),
        Some(&Value::Int("finished\n".len() as i64))
    );
    assert_eq!(read(b.path().join("out/events.txt")), "started\nfinished\n");
    let v = run(
        b.path(),
        "return file write \"./out/events.txt\" text \"again\\n\"",
    )
    .await
    .unwrap();
    assert_eq!(v.get("updated"), Some(&Value::Bool(true)));
    assert_eq!(read(b.path().join("out/events.txt")), "again\n");
}

// vhco:test files.apply_file_operation -- S38 delete removes one file; missing fails unless `missing ok`; directories are not deleted
#[tokio::test]
async fn delete_with_missing_policy() {
    let b = bundle();
    std::fs::write(b.path().join("out/obsolete.json"), "{}").unwrap();
    let body = "r = file delete \"./out/obsolete.json\"\n    missing ok\nend\nreturn r";
    let v = run(b.path(), body).await.unwrap();
    assert_eq!(v.get("deleted"), Some(&Value::Bool(true)));
    assert!(!b.path().join("out/obsolete.json").exists());
    let v = run(b.path(), body).await.unwrap();
    assert_eq!(v.get("deleted"), Some(&Value::Bool(false)));
    let e = run(
        b.path(),
        "file delete \"./out/obsolete.json\"\nreturn null",
    )
    .await
    .unwrap_err();
    assert_eq!((e.kind, e.exit_code()), (ErrorKind::NotFound, 4));

    std::fs::create_dir(b.path().join("out/dir")).unwrap();
    std::fs::write(b.path().join("out/dir/keep.txt"), "k").unwrap();
    assert!(
        run(b.path(), "file delete \"./out/dir\"\nreturn null")
            .await
            .is_err()
    );
    assert_eq!(read(b.path().join("out/dir/keep.txt")), "k");
}

// vhco:test files.apply_file_operation -- S39 list is sorted by name and stat reports size/version without reading contents into the result
#[tokio::test]
async fn list_and_stat() {
    let b = bundle();
    std::fs::write(b.path().join("data/config.json"), r#"{"enabled":true}"#).unwrap();
    std::fs::write(b.path().join("data/b.txt"), "bb").unwrap();
    std::fs::create_dir(b.path().join("data/a")).unwrap();
    let v = run(
        b.path(),
        "entries = file list \"./data\"\ninfo = file stat \"./data/config.json\"\nreturn {entries: entries, info: info}",
    )
    .await
    .unwrap();
    let names: Vec<String> = match v.get("entries").unwrap() {
        Value::List(items) => items
            .iter()
            .map(|e| e.get("name").unwrap().to_display())
            .collect(),
        other => panic!("{other:?}"),
    };
    assert_eq!(names, vec!["a", "b.txt", "config.json"]);
    let info = v.get("info").unwrap();
    assert_eq!(info.get("size"), Some(&Value::Int(16)));
    assert_eq!(info.get("type"), Some(&Value::text("file")));
    assert!(
        info.get("version")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
    );
    assert!(!info.to_display().contains("enabled"), "no contents in stat");
    let e = run(b.path(), "return file stat \"./data/none.json\"")
        .await
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
}

// vhco:test files.apply_file_operation -- S40 copy/move never overwrite by default or with `overwrite false`; the destination stays unchanged
#[tokio::test]
async fn copy_and_move_guard_the_destination() {
    let b = bundle();
    std::fs::write(b.path().join("data/input.txt"), "input").unwrap();
    let v = run(
        b.path(),
        "file copy \"./data/input.txt\" to \"./out/copy.txt\"\n    overwrite false\nend\nfile move \"./out/copy.txt\" to \"./out/final.txt\"\n    overwrite false\nend\nreturn {path: \"./out/final.txt\"}",
    )
    .await
    .unwrap();
    assert_eq!(v.get("path"), Some(&Value::text("./out/final.txt")));
    assert_eq!(read(b.path().join("out/final.txt")), "input");
    assert!(!b.path().join("out/copy.txt").exists());
    assert_eq!(read(b.path().join("data/input.txt")), "input");

    std::fs::write(b.path().join("out/keep.txt"), "keep").unwrap();
    for body in [
        // `overwrite false` spelled out …
        "file copy \"./data/input.txt\" to \"./out/keep.txt\"\n    overwrite false\nend\nreturn null",
        // … and the default: no overwrite unless asked for.
        "file copy \"./data/input.txt\" to \"./out/keep.txt\"\nreturn null",
        "file move \"./out/final.txt\" to \"./out/keep.txt\"\nreturn null",
    ] {
        let e = run(b.path(), body).await.unwrap_err();
        assert_eq!(e.code, "conflict.already_exists", "{body}");
        assert_eq!(read(b.path().join("out/keep.txt")), "keep");
    }
    assert_eq!(read(b.path().join("out/final.txt")), "input");
    // Asking for it explicitly replaces the destination.
    run(
        b.path(),
        "file copy \"./data/input.txt\" to \"./out/keep.txt\"\n    overwrite true\nend\nreturn null",
    )
    .await
    .unwrap();
    assert_eq!(read(b.path().join("out/keep.txt")), "input");
}

// ---------------------------------------------------------------- confinement

// vhco:test files.apply_file_operation -- `..` escapes and absolute paths are refused (permission, exit 3) and nothing outside the root is touched
#[tokio::test]
async fn dotdot_and_absolute_paths_are_refused() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("root");
    std::fs::create_dir_all(root.join("out")).unwrap();
    std::fs::write(outer.path().join("outside.txt"), "outside").unwrap();
    let star = r#"{"version":1,"grants":[
        {"capability":"allow_read","targets":["*"]},
        {"capability":"allow_write","targets":["*"]},
        {"capability":"allow_delete","targets":["*"]}]}"#;
    let abs = outer.path().join("outside.txt");
    let abs = abs.to_str().unwrap();
    for body in [
        "return file read \"../outside.txt\" as text".to_string(),
        "return file read \"./out/../../outside.txt\" as text".to_string(),
        "return file write \"../outside.txt\" text \"pwned\"".to_string(),
        "return file create \"../new.txt\" text \"pwned\"".to_string(),
        "return file delete \"../outside.txt\"".to_string(),
        format!("return file read \"{abs}\" as text"),
        format!("return file write \"{abs}\" text \"pwned\""),
        format!("return file copy \"{abs}\" to \"./out/stolen.txt\""),
    ] {
        let e = run_with(&root, &body, Some(star)).await.unwrap_err();
        assert_eq!(
            (e.kind, e.exit_code()),
            (ErrorKind::Permission, 3),
            "{body}: {e:?}"
        );
    }
    assert_eq!(read(outer.path().join("outside.txt")), "outside");
    assert!(!outer.path().join("new.txt").exists());
    assert!(tree(&root.join("out")).is_empty());
    // `..` that stays inside the root is ordinary lexical normalization.
    std::fs::write(root.join("out/in.txt"), "in").unwrap();
    let v = run_with(
        &root,
        "return file read \"./out/../out/in.txt\" as text",
        Some(star),
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("in"));
}

// vhco:test files.apply_file_operation -- a symlinked final component is refused for read/update/write/append/delete and its target is unchanged
#[cfg(unix)]
#[tokio::test]
async fn symlinked_file_is_refused() {
    let b = bundle();
    std::fs::write(b.path().join("data/secret.json"), r#"{"s":1}"#).unwrap();
    std::os::unix::fs::symlink(
        b.path().join("data/secret.json"),
        b.path().join("out/link.json"),
    )
    .unwrap();
    for body in [
        "return file read \"./out/link.json\" as json",
        "return file update \"./out/link.json\" json {s: 2}",
        "return file write \"./out/link.json\" json {s: 2}",
        "return file append \"./out/link.json\" text \"x\"",
        "return file copy \"./out/link.json\" to \"./out/copy.json\"",
        "return file create \"./out/link.json\" json {s: 2}",
    ] {
        let e = run(b.path(), body).await.unwrap_err();
        assert!(
            matches!(e.kind, ErrorKind::Permission | ErrorKind::Conflict),
            "{body}: {e:?}"
        );
    }
    assert_eq!(read(b.path().join("data/secret.json")), r#"{"s":1}"#);
    assert!(!b.path().join("out/copy.json").exists());
    // Deleting through the link never touches its target.
    let _ = run(b.path(), "return file delete \"./out/link.json\"").await;
    assert_eq!(read(b.path().join("data/secret.json")), r#"{"s":1}"#);
}

// vhco:test files.apply_file_operation -- a symlinked directory component (inside or outside the root) is refused: a grant on ./out/** never reaches ./data through out/link
#[cfg(unix)]
#[tokio::test]
async fn symlinked_directory_component_is_refused() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("root");
    std::fs::create_dir_all(root.join("out")).unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::create_dir_all(outer.path().join("elsewhere")).unwrap();
    std::fs::write(root.join("data/secret.json"), r#"{"s":1}"#).unwrap();
    std::fs::write(outer.path().join("elsewhere/x.txt"), "x").unwrap();
    // A relative link that stays inside the root (cap-std would follow it) …
    std::os::unix::fs::symlink("../data", root.join("out/inner")).unwrap();
    // … and an absolute link that leaves it.
    std::os::unix::fs::symlink(outer.path().join("elsewhere"), root.join("out/outer")).unwrap();
    // Only ./out is granted; ./data is not.
    let out_only = r#"{"version":1,"grants":[
        {"capability":"allow_read","targets":["./out","./out/**"]},
        {"capability":"allow_write","targets":["./out/**"]},
        {"capability":"allow_delete","targets":["./out/**"]}]}"#;
    for dir in ["inner", "outer"] {
        for body in [
            format!("return file read \"./out/{dir}/secret.json\" as text"),
            format!("return file read \"./out/{dir}/x.txt\" as text"),
            format!("return file create \"./out/{dir}/new.txt\" text \"pwned\""),
            format!("return file write \"./out/{dir}/secret.json\" text \"pwned\""),
            format!("return file update \"./out/{dir}/secret.json\" text \"pwned\""),
            format!("return file delete \"./out/{dir}/secret.json\""),
            format!("return file list \"./out/{dir}\""),
            format!("return file stat \"./out/{dir}/secret.json\""),
        ] {
            let e = run_with(&root, &body, Some(out_only)).await.unwrap_err();
            assert_eq!(e.kind, ErrorKind::Permission, "{body}: {e:?}");
        }
    }
    assert_eq!(read(root.join("data/secret.json")), r#"{"s":1}"#);
    assert_eq!(tree(&root.join("data")), vec!["secret.json"]);
    assert_eq!(tree(&outer.path().join("elsewhere")), vec!["x.txt"]);
}

// vhco:test files.apply_file_operation -- S139 a hard-linked target is refused for update/write/append/delete/copy destination (file.hardlink_refused, exit 3); neither path changes
#[cfg(unix)]
#[tokio::test]
async fn hard_links_are_refused() {
    let b = bundle();
    std::fs::write(b.path().join("data/secret.json"), r#"{"s":1}"#).unwrap();
    std::fs::write(b.path().join("data/in.txt"), "in").unwrap();
    std::fs::hard_link(
        b.path().join("data/secret.json"),
        b.path().join("out/report.json"),
    )
    .unwrap();
    for body in [
        "file update \"./out/report.json\" json {ok: true}\nreturn null",
        "file write \"./out/report.json\" json {ok: true}\nreturn null",
        "file append \"./out/report.json\" text \"x\"\nreturn null",
        "file delete \"./out/report.json\"\nreturn null",
        "file copy \"./data/in.txt\" to \"./out/report.json\"\n    overwrite true\nend\nreturn null",
        "file move \"./out/report.json\" to \"./out/moved.json\"\nreturn null",
    ] {
        let e = run(b.path(), body).await.unwrap_err();
        assert_eq!(code(&e), ("file.hardlink_refused", 3), "{body}: {e:?}");
        assert_eq!(e.kind, ErrorKind::Permission);
    }
    assert_eq!(read(b.path().join("data/secret.json")), r#"{"s":1}"#);
    assert_eq!(read(b.path().join("out/report.json")), r#"{"s":1}"#);
    assert_eq!(tree(&b.path().join("out")), vec!["report.json"]);
}

// ---------------------------------------------------------------- policy interplay

// vhco:test files.apply_file_operation -- S146 an access-narrowed create-only grant allows create but denies update/write/append before any byte is written
#[tokio::test]
async fn access_narrowed_create_only() {
    let b = bundle();
    std::fs::create_dir(b.path().join("out/notes")).unwrap();
    let policy = r#"{"version":1,"grants":[
        {"capability":"allow_read","targets":["./out/notes/*.json"],"access":["stat"]},
        {"capability":"allow_write","targets":["./out/notes/*.json"],"access":["create"]}]}"#;
    let v = run_with(
        b.path(),
        "file create \"./out/notes/a.json\" json {body: \"hi\"}\nreturn {created: \"a\"}",
        Some(policy),
    )
    .await
    .unwrap();
    assert_eq!(v.get("created"), Some(&Value::text("a")));
    for body in [
        "file update \"./out/notes/a.json\" json {body: \"bye\"}\nreturn null",
        "file write \"./out/notes/a.json\" json {body: \"bye\"}\nreturn null",
        "file append \"./out/notes/a.json\" text \"bye\"\nreturn null",
        "file delete \"./out/notes/a.json\"\nreturn null",
        "return file read \"./out/notes/a.json\"",
        // `file write` needs create AND update, so even a new path is refused.
        "file write \"./out/notes/b.json\" json {body: \"x\"}\nreturn null",
    ] {
        let e = run_with(b.path(), body, Some(policy)).await.unwrap_err();
        assert_eq!(code(&e), ("permission.denied", 3), "{body}");
    }
    assert_eq!(
        json_file(b.path().join("out/notes/a.json")),
        serde_json::json!({"body": "hi"})
    );
    assert_eq!(tree(&b.path().join("out/notes")), vec!["a.json"]);
    // The denial names the verb and the narrowed grant.
    let e = run_with(
        b.path(),
        "file update \"./out/notes/a.json\" json {body: \"bye\"}\nreturn null",
        Some(policy),
    )
    .await
    .unwrap_err();
    let d = &e.details;
    assert_eq!(d.get("access"), Some(&Value::text("update")));
    assert_eq!(d.get("capability"), Some(&Value::text("allow_write")));
    assert!(e.message.contains("create"), "{}", e.message);
}

// vhco:test files.apply_file_operation -- S66 without policy.json every file verb is denied with zero effects while pure operations still run
#[tokio::test]
async fn no_policy_denies_every_file_verb() {
    let b = bundle();
    std::fs::write(b.path().join("data/in.txt"), "in").unwrap();
    for body in [
        "return file read \"./data/in.txt\" as text",
        "return file stat \"./data/in.txt\"",
        "return file list \"./data\"",
        "return file create \"./out/a.txt\" text \"a\"",
        "return file delete \"./data/in.txt\"",
    ] {
        let e = run_with(b.path(), body, None).await.unwrap_err();
        assert_eq!(code(&e), ("permission.denied", 3), "{body}");
    }
    assert!(tree(&b.path().join("out")).is_empty());
    assert_eq!(read(b.path().join("data/in.txt")), "in");
    let rt = runtime(b.path(), &op("return null"), None);
    assert_eq!(
        rt.request("t.pure", Value::Null, None)
            .await
            .unwrap()
            .result,
        Value::Int(7)
    );
}

// vhco:test files.apply_file_operation -- delete needs allow_delete (write alone is insufficient); `./out/**` never matches the sibling `./out2`
#[tokio::test]
async fn delete_grant_and_sibling_prefix() {
    let b = bundle();
    std::fs::create_dir(b.path().join("out2")).unwrap();
    std::fs::write(b.path().join("out/a.txt"), "a").unwrap();
    let write_only = r#"{"version":1,"grants":[{"capability":"allow_write","targets":["./out/**"]}]}"#;
    let e = run_with(
        b.path(),
        "return file delete \"./out/a.txt\"",
        Some(write_only),
    )
    .await
    .unwrap_err();
    assert_eq!(code(&e), ("permission.denied", 3));
    assert!(b.path().join("out/a.txt").exists());
    let e = run(b.path(), "return file create \"./out2/x.txt\" text \"x\"")
        .await
        .unwrap_err();
    assert_eq!(code(&e), ("permission.denied", 3));
    assert!(tree(&b.path().join("out2")).is_empty());
}

// ---------------------------------------------------------------- durability

// vhco:test files.apply_file_operation -- atomic replacement leaves no temporary sibling behind, after success and after a failed replace
#[tokio::test]
async fn atomic_replace_never_leaves_temp_files() {
    let b = bundle();
    std::fs::write(b.path().join("out/a.json"), "{}").unwrap();
    std::fs::write(b.path().join("data/in.txt"), "in").unwrap();
    for _ in 0..3 {
        run(
            b.path(),
            "file update \"./out/a.json\" json {n: 1}\nfile write \"./out/b.txt\" text \"b\"\nreturn null",
        )
        .await
        .unwrap();
    }
    assert_eq!(tree(&b.path().join("out")), vec!["a.json", "b.txt"]);

    // A replace that fails at rename time (the destination is a directory).
    std::fs::create_dir(b.path().join("out/dir")).unwrap();
    std::fs::write(b.path().join("out/dir/keep.txt"), "k").unwrap();
    let e = run(
        b.path(),
        "file copy \"./data/in.txt\" to \"./out/dir\"\n    overwrite true\nend\nreturn null",
    )
    .await
    .unwrap_err();
    assert_ne!(e.kind, ErrorKind::Permission, "{e:?}");
    assert_eq!(
        tree(&b.path().join("out")),
        vec!["a.json", "b.txt", "dir", "dir/keep.txt"],
        "no temp file may survive a failed replace"
    );

    // Concurrent updates of the same file: every one succeeds or fails cleanly, no temp left.
    let rt = runtime(
        b.path(),
        &op("file update \"./out/a.json\" json {n: 2}\nreturn null"),
        Some(FULL),
    );
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let rt = rt.clone();
        tasks.push(tokio::spawn(async move {
            rt.request("t.run", Value::Null, None).await.map(|_| ())
        }));
    }
    for t in tasks {
        t.await.unwrap().unwrap();
    }
    assert_eq!(
        json_file(b.path().join("out/a.json")),
        serde_json::json!({"n": 2})
    );
    assert_eq!(
        tree(&b.path().join("out")),
        vec!["a.json", "b.txt", "dir", "dir/keep.txt"]
    );
}
