//! T-25 I/O manifest conformance: golden rows derived from the docs/demos
//! READMEs (02-file-crud, 03-http, 11-sandbox) and the REF-2026-0002 fixture
//! bundles (S62–S65, S141–S147, S153–S157), exit codes 0/2/3/4/7.

use rivet::Runtime;
use rivet::domain::Value;
use rivet::domain::io_manifest::{FileStatus, IoQuery, Knowledge};
use std::path::Path;
use std::process::Command;

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

/// Split a fixed-width table line on runs of 2+ spaces.
fn cells(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut spaces = 0;
    for c in line.trim().chars() {
        if c == ' ' {
            spaces += 1;
            continue;
        }
        if spaces >= 2 && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        } else if spaces == 1 {
            cur.push(' ');
        }
        spaces = 0;
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Table body rows (header skipped) as cell vectors.
fn rows(table: &str) -> Vec<Vec<String>> {
    table
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(cells)
        .collect()
}

fn expect(table: &str) -> Vec<Vec<String>> {
    table
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(cells)
        .collect()
}

fn demo(name: &str) -> String {
    format!("docs/demos/{name}/app.rivet")
}

/// Copy a fixture bundle (and an optional policy.json) into a temp dir.
fn bundle(src: &str, file: &str, policy: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(src, dir.path().join(file)).unwrap();
    if let Some(p) = policy {
        std::fs::write(dir.path().join("policy.json"), p).unwrap();
    }
    dir
}

fn path(d: &tempfile::TempDir, f: &str) -> String {
    d.path().join(f).to_string_lossy().to_string()
}

// ---------------------------------------------------------------- 02-file-crud

// vhco:test audit.inspect_effects -- 02-file-crud `io --by target` rows (README table, one row per capability) and `io --check-policy` table exit 0
#[test]
fn demo02_by_target_and_check_policy() {
    let (code, out, _) = rivet(&["--file", &demo("02-file-crud"), "io", "--by", "target"]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "./out/note.json  read, stat  allow_read  file read, file update  body  yes  notes.read, notes.update
             ./out/note.json  create, update  allow_write  file create, file update  body  yes: update  notes.create, notes.update
             ./out/note.json  delete  allow_delete  file delete  body  no  notes.delete
             ./out  list  allow_read  file list  body  yes  notes.list"
        )
    );
    let (code, out, _) = rivet(&["--file", &demo("02-file-crud"), "io", "--check-policy"]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "notes.create   file   create   ./out/note.json   exact       app.rivet:9    allowed
             notes.delete   file   delete   ./out/note.json   exact       app.rivet:40   allowed
             notes.list     file   list     ./out             exact       app.rivet:50   allowed
             notes.read     file   read     ./out/note.json   exact       app.rivet:19   allowed
             notes.update   file   stat     ./out/note.json   exact       app.rivet:30   allowed
             notes.update   file   update   ./out/note.json   exact       app.rivet:30   allowed"
        )
    );
}

// vhco:test audit.inspect_effects -- 02-file-crud `io --needs` listing and `--access create,update,delete` keeps only mutating rows
#[test]
fn demo02_needs_and_access_filter() {
    let (code, out, _) = rivet(&["--file", &demo("02-file-crud"), "io", "--needs"]);
    assert_eq!(code, 0);
    assert_eq!(
        expect(&out),
        expect(
            "notes.create needs no existing files.
             notes.delete needs no existing files.
             notes.list needs, before it can run:
               ./out                 (file list)
             notes.read needs, before it can run:
               ./out/note.json       (file read)
             notes.update needs, before it can run:
               ./out/note.json       (file update)"
        )
    );
    let (code, out, _) = rivet(&[
        "--file",
        &demo("02-file-crud"),
        "io",
        "--access",
        "create,update,delete",
    ]);
    assert_eq!(code, 0);
    let ops: Vec<(String, String)> = rows(&out)
        .into_iter()
        .map(|r| (r[0].clone(), r[2].clone()))
        .collect();
    assert_eq!(
        ops,
        vec![
            ("notes.create".into(), "create".into()),
            ("notes.delete".into(), "delete".into()),
            ("notes.update".into(), "update".into())
        ]
    );
}

// ---------------------------------------------------------------- 03-http

// vhco:test audit.inspect_effects -- 03-http origin-grouped `--by target` row and per-path `--check-policy` rows (query template included), exit 0
#[test]
fn demo03_http_rows() {
    let (code, out, _) = rivet(&["--file", &demo("03-http"), "io", "--by", "target"]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "https://api.example.com:443   connect GET, POST   allow_network   http get, http post   connect   —            users.create, users.get, users.search"
        )
    );
    let (code, out, _) = rivet(&["--file", &demo("03-http"), "io", "--check-policy"]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "users.create   network   connect POST   https://api.example.com/users                       exact             app.rivet:29   allowed
             users.get      network   connect GET    https://api.example.com/users/{id}                  param_dependent   app.rivet:10   allowed
             users.search   network   connect GET    https://api.example.com/search?q={query}&limit=10   param_dependent   app.rivet:41   allowed"
        )
    );
    let rt = Runtime::builder().file(&demo("03-http")).build().unwrap();
    let m = rt
        .io(&IoQuery {
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap()
        .manifest;
    let get = m
        .sites
        .iter()
        .find(|s| s.operation_id == "users.get")
        .unwrap();
    assert_eq!(get.target.params, vec!["id".to_string()]);
    assert_eq!(get.target.port, Some(443));
    assert_eq!(get.method.as_deref(), Some("GET"));
}

// ---------------------------------------------------------------- 11-sandbox

// vhco:test audit.inspect_effects -- 11-sandbox check-policy (deny overrides grant, exit 3), by target, by capability and an empty delete filter
#[test]
fn demo11_views() {
    let f = demo("11-sandbox");
    let (code, out, _) = rivet(&["--file", &f, "io", "--check-policy"]);
    assert_eq!(code, 3);
    assert_eq!(
        rows(&out),
        expect(
            "data.private    file   read     ./data/private/secret.json   exact       app.rivet:26   denied
             data.read       file   read     ./data/public.json           exact       app.rivet:15   allowed
             data.snapshot   (calls data.read — see above)                                        app.rivet:37
             data.snapshot   file   create   ./out/snapshot.json          exact       app.rivet:38   allowed"
        )
    );
    let (code, out, _) = rivet(&["--file", &f, "io", "--by", "target"]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "./data/private/secret.json   read     allow_read    file read     body    yes          data.private
             ./data/public.json           read     allow_read    file read     body    yes          data.read, data.snapshot (via data.read)
             ./out/snapshot.json          create   allow_write   file create   body    no           data.snapshot"
        )
    );
    let (_, out, _) = rivet(&["--file", &f, "io", "--by", "capability"]);
    let body: Vec<Vec<String>> = rows(&out)
        .into_iter()
        .filter(|r| !r[0].starts_with("unused"))
        .map(|r| r[..r.len().min(3)].to_vec())
        .collect();
    assert_eq!(
        body,
        expect(
            "allow_read
               ./data/private/secret.json   read     data.private
               ./data/public.json           read     data.read, data.snapshot (via data.read)
             allow_write
               ./out/snapshot.json          create   data.snapshot"
        )
    );
    let (code, out, _) = rivet(&["--file", &f, "io", "--kind", "file", "--access", "delete"]);
    assert_eq!(code, 0);
    assert!(out.contains("(no sites match kind=file access=delete)"));
    let (code, _, _) = rivet(&["--file", &f, "io", "--kind", "file", "--access", "connect"]);
    assert_eq!(code, 2);
    let (code, _, _) = rivet(&["--file", &f, "io", "--access", "remove"]);
    assert_eq!(code, 2);
    let (code, _, _) = rivet(&["--file", &f, "io", "--by", "file"]);
    assert_eq!(code, 2);
}

// vhco:test audit.inspect_effects -- 11-sandbox --needs (callee via) and --check-files under policy.json (exit 3), for allowed ops (exit 0) and create-only.json (all not_permitted, exit 3)
#[test]
fn demo11_needs_and_check_files() {
    let f = demo("11-sandbox");
    let (code, out, _) = rivet(&["--file", &f, "io", "--needs"]);
    assert_eq!(code, 0);
    assert_eq!(
        expect(&out),
        expect(
            "data.private needs, before it can run:
               ./data/private/secret.json   (file read)
             data.read needs, before it can run:
               ./data/public.json           (file read)
             data.snapshot needs, before it can run:
               ./data/public.json           (file read, via data.read)
             demo.echo needs no existing files."
        )
    );
    let (code, out, _) = rivet(&["--file", &f, "io", "--check-files"]);
    assert_eq!(code, 3);
    assert_eq!(
        expect(&out),
        expect(
            "data.private needs, before it can run:
               ./data/private/secret.json   (file read)                  not_permitted
             data.read needs, before it can run:
               ./data/public.json           (file read)                  present
             data.snapshot needs, before it can run:
               ./data/public.json           (file read, via data.read)   present
             demo.echo needs no existing files."
        )
    );
    let (code, _, _) = rivet(&[
        "--file",
        &f,
        "io",
        "data.read",
        "data.snapshot",
        "--check-files",
    ]);
    assert_eq!(code, 0);
    let (code, out, _) = rivet(&[
        "--file",
        &f,
        "--policy",
        "docs/demos/11-sandbox/policies/create-only.json",
        "io",
        "--check-files",
    ]);
    assert_eq!(code, 3);
    assert_eq!(out.matches("not_permitted").count(), 3);
    let (code, _, _) = rivet(&[
        "--file",
        &f,
        "--policy",
        "docs/demos/11-sandbox/policies/create-only.json",
        "io",
        "data.snapshot",
        "--check-policy",
    ]);
    assert_eq!(code, 0);
}

// vhco:test audit.inspect_effects -- 11-sandbox JSON manifest: effect ids, call chain through data.read, origin/phase/requires_existing, policy digest, complete=true and --strict exit 0
#[test]
fn demo11_json_manifest() {
    let (code, out, _) = rivet(&[
        "--file",
        &demo("11-sandbox"),
        "io",
        "data.snapshot",
        "--check-policy",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0);
    // `--format json` is an envelope (operation rivet.io) whose data is the manifest.
    let env: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(env["operation"], "rivet.io");
    let j = &env["data"];
    assert_eq!(j["complete"], true);
    assert_eq!(j["policy"]["file"], "policy.json");
    let sites = j["sites"].as_array().unwrap();
    assert_eq!(sites.len(), 2);
    assert_eq!(sites[0]["effect_id"], "data.read#1");
    assert_eq!(
        sites[0]["call_chain"],
        serde_json::json!(["data.snapshot", "data.read"])
    );
    assert_eq!(
        sites[0]["origin"],
        serde_json::json!({"statement": "file read"})
    );
    assert_eq!(sites[0]["requires_existing"], true);
    assert_eq!(sites[0]["decision"], "allowed");
    assert_eq!(sites[1]["effect_id"], "data.snapshot#1");
    assert_eq!(sites[1]["requires_existing"], false);
    assert_eq!(j["needs"], serde_json::json!([]));
    let (code, _, _) = rivet(&[
        "--file",
        &demo("11-sandbox"),
        "io",
        "--strict",
        "--include-bootstrap",
        "--json",
    ]);
    assert_eq!(code, 0);
}

// ---------------------------------------------------------------- REF fixture bundle

const FIXTURE: &str = "tests/fixtures/io_manifest/app.rivet";

const S144_POLICY: &str = r#"{
  "version": 1,
  "grants": [
    {"capability": "allow_env",     "targets": ["API_KEY"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_read",    "targets": ["./data/**", "./out/notes/**"]},
    {"capability": "allow_write",   "targets": ["./out/user.json", "./out/notes/2026-*.json"]}
  ]
}"#;

// vhco:test audit.inspect_effects -- S65 every site of --all (13) with call row, dynamic config.url reported by expression, --strict exits 7 and plain run exits 0
#[test]
fn s65_strict_incomplete_exits_7() {
    let (code, out, err) = rivet(&["--file", FIXTURE, "io", "--all", "--strict"]);
    assert_eq!(code, 7);
    let r = rows(&out);
    assert_eq!(r.len(), 14);
    assert!(r.iter().any(|x| x[1] == "(calls users.get — see above)"));
    assert!(
        r.iter()
            .any(|x| x.contains(&"<dynamic: config.url>".to_string()))
    );
    assert!(r.iter().any(|x| x[0] == "archive.purge"));
    assert!(err.contains("1 of 13 sites is dynamic/opaque"));
    assert!(err.contains("sync.push#2"));
    let (code, _, _) = rivet(&["--file", FIXTURE, "io", "--all"]);
    assert_eq!(code, 0);
    // without --all the private helper is absent
    let (_, out, _) = rivet(&["--file", FIXTURE, "io"]);
    assert!(!out.contains("archive.purge"));
    // private id without --all is not_found (exit 4)
    let (code, _, _) = rivet(&["--file", FIXTURE, "io", "archive.purge"]);
    assert_eq!(code, 4);
}

// vhco:test audit.inspect_effects -- S62 JSON: deterministic effect ids, param templates/globs, longest call chain, secrets by name, complete=false, policy null
#[test]
fn s62_json_sites() {
    let rt = Runtime::builder().file(FIXTURE).build().unwrap();
    let r = rt
        .io(&IoQuery {
            all: true,
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap();
    let m = &r.manifest;
    assert_eq!(m.sites.len(), 13);
    assert!(!m.complete);
    assert!(m.policy.is_none());
    let get = m
        .sites
        .iter()
        .find(|s| s.effect_id == "users.get#2")
        .unwrap();
    assert_eq!(get.target.template, "https://api.example.com/users/{id}");
    assert_eq!(get.target.path.as_deref(), Some("/users/{id}"));
    assert_eq!(get.knowledge, Knowledge::ParamDependent);
    assert_eq!(get.call_chain, vec!["users.snapshot", "users.get"]);
    assert_eq!(get.secrets, vec!["api_key"]);
    assert_eq!(get.source.start_line, 6);
    let del = m
        .sites
        .iter()
        .find(|s| s.effect_id == "notes.delete#1")
        .unwrap();
    assert_eq!(del.target.glob.as_deref(), Some("./out/notes/*.json"));
    let dynamic = m
        .sites
        .iter()
        .find(|s| s.effect_id == "sync.push#2")
        .unwrap();
    assert_eq!(dynamic.knowledge, Knowledge::Dynamic);
    assert_eq!(dynamic.expression.as_deref(), Some("config.url"));
    let j = m.to_json();
    assert!(
        j["sites"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["target"]["template"].is_null())
    );
    let upd: Vec<_> = m
        .sites
        .iter()
        .filter(|s| s.operation_id == "notes.update")
        .collect();
    assert_eq!(upd.len(), 2);
    assert_eq!(upd[0].effect_id, "notes.update#1");
    assert_eq!(upd[0].capability.as_str(), "allow_read");
    assert_eq!(upd[1].capability.as_str(), "allow_write");
}

// vhco:test audit.inspect_effects -- S63/S64: one entry follows its literal call; --kind network with --include-bootstrap lists the fixed bootstrap list
#[test]
fn s63_s64_entry_and_bootstrap() {
    let (code, out, _) = rivet(&["--file", FIXTURE, "io", "users.snapshot"]);
    assert_eq!(code, 0);
    let r = rows(&out);
    assert_eq!(r.len(), 4);
    let (code, out, _) = rivet(&[
        "--file",
        FIXTURE,
        "io",
        "--all",
        "--kind",
        "network",
        "--include-bootstrap",
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("BOOTSTRAP (runtime-internal; listed, not governed by policy.json)"));
    assert!(out.contains("stdin, stdout, stderr"));
    assert!(out.contains("./app.rivet (+ imports)"));
    let net: Vec<_> = out.lines().take_while(|l| !l.is_empty()).skip(1).collect();
    assert_eq!(net.len(), 3);
}

// vhco:test audit.inspect_effects -- S141/S143 by-target rows: origin grouping, glob rows per capability, NEEDS FILE yes/no/`yes: update`, private helper labelled
#[test]
fn s141_s143_by_target() {
    let (code, out, _) = rivet(&[
        "--file",
        FIXTURE,
        "io",
        "--all",
        "--access",
        "create,update,append,delete",
        "--by",
        "target",
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "./out/archive/old.json  delete          allow_delete  file delete               body   no           archive.purge (private)
             ./out/notes/*.json      create, update  allow_write   file create, file update  body   yes: update  notes.create, notes.update
             ./out/notes/*.json      delete          allow_delete  file delete               body   yes          notes.delete
             ./out/user.json         create          allow_write   file create               body   no           users.snapshot"
        )
    );
    let (_, out, _) = rivet(&["--file", FIXTURE, "io", "--by", "target"]);
    let net = rows(&out)
        .into_iter()
        .find(|r| r[0] == "https://api.example.com:443")
        .unwrap();
    assert_eq!(net[1], "connect GET, POST");
    assert_eq!(net[3], "http get, http post");
    let (code, out, _) = rivet(&[
        "--file", FIXTURE, "io", "--all", "--kind", "file", "--access", "delete",
    ]);
    assert_eq!(code, 0);
    assert_eq!(rows(&out).len(), 2);
}

// vhco:test audit.inspect_effects -- S144 check-policy against the fixture policy: partial for 2026-* globs, denied without allow_delete, unknown for dynamic, exit 3; S145 CSV row shape
#[test]
fn s144_check_policy_partial_denied_unknown() {
    let d = bundle(FIXTURE, "app.rivet", Some(S144_POLICY));
    let app = path(&d, "app.rivet");
    let (code, out, err) = rivet(&["--file", &app, "io", "--check-policy"]);
    assert_eq!(code, 3, "{out}{err}");
    let decisions: Vec<(String, String, String)> = rows(&out)
        .into_iter()
        .filter(|r| r.len() == 7)
        .map(|r| (r[0].clone(), r[2].clone(), r[6].clone()))
        .collect();
    let find = |op: &str, acc: &str| {
        decisions
            .iter()
            .find(|(o, a, _)| o == op && a == acc)
            .map(|x| x.2.clone())
            .unwrap()
    };
    assert_eq!(find("users.get", "connect GET"), "allowed");
    assert_eq!(find("users.get", "read"), "allowed");
    assert_eq!(find("notes.create", "create"), "partial");
    assert_eq!(find("notes.update", "stat"), "allowed");
    assert_eq!(find("notes.update", "update"), "partial");
    assert_eq!(find("notes.delete", "delete"), "denied");
    assert_eq!(find("sync.push", "connect POST"), "unknown");
    assert_eq!(find("users.snapshot", "create"), "allowed");
    assert!(err.contains("2 partial"));
    assert!(err.contains("1 denied"));
    let (code, out, _) = rivet(&["--file", &app, "io", "--check-policy", "--format", "csv"]);
    assert_eq!(code, 3);
    let mut lines = out.lines();
    assert_eq!(
        lines.next().unwrap(),
        "effect_id,operation_id,kind,access,method,protocol,capability,target,glob,knowledge,call_chain,secrets,source,origin,phase,requires_existing,secret,decision"
    );
    assert!(out.contains("users.get#1,users.get,env,read,,,allow_env,env:API_KEY,,exact,users.snapshot>users.get,api_key,app.rivet:5:5,statement:secret,body,false,true,allowed"));
    assert!(out.contains("notes.update#2,notes.update,file,update,,,allow_write,./out/notes/{name}.json,./out/notes/*.json,param_dependent,notes.update,,app.rivet:56:5,statement:file update,body,true,false,partial"));
    let (_, md, _) = rivet(&[
        "--file", &app, "io", "--by", "target", "--format", "markdown",
    ]);
    assert!(
        md.starts_with("| TARGET | ACCESS | CAPABILITY | ORIGIN | PHASE | NEEDS FILE | USED BY |")
    );
    assert!(md.contains(
        "| ./out/user.json | create | allow_write | file create | body | no | users.snapshot |"
    ));
}

// vhco:test audit.inspect_effects -- S146 create-only narrowing: stat allowed, update denied, exit 3
#[test]
fn s146_access_narrowing() {
    let d = bundle(
        FIXTURE,
        "app.rivet",
        Some(
            r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./out/notes/*.json"],"access":["stat"]},{"capability":"allow_write","targets":["./out/notes/*.json"],"access":["create"]}]}"#,
        ),
    );
    let (code, out, _) = rivet(&[
        "--file",
        &path(&d, "app.rivet"),
        "io",
        "notes.create",
        "notes.update",
        "--check-policy",
    ]);
    assert_eq!(code, 3);
    assert_eq!(
        rows(&out),
        expect(
            "notes.create  file  create  ./out/notes/{name}.json  param_dependent  app.rivet:47  allowed
             notes.update  file  stat    ./out/notes/{name}.json  param_dependent  app.rivet:56  allowed
             notes.update  file  update  ./out/notes/{name}.json  param_dependent  app.rivet:56  denied"
        )
    );
}

// ---------------------------------------------------------------- TLS fixture: option-derived sites

const TLS: &str = "tests/fixtures/tls/tls.rivet";

// vhco:test audit.inspect_effects -- S154/S155: tls options are before_connect file sites; key_file is secret; body file of a file created earlier is not a need
#[test]
fn s154_s155_option_sites_and_needs() {
    let (code, out, _) = rivet(&["--file", TLS, "io", "status.get", "--by", "target"]);
    assert_eq!(code, 0);
    assert_eq!(
        rows(&out),
        expect(
            "https://status.example.com:443  connect GET  allow_network  http get     connect         —    status.get
             ./certs/ca.pem                  read         allow_read     tls ca_file  before_connect  yes  status.get"
        )
    );
    let (code, out, _) = rivet(&["--file", TLS, "io", "--needs"]);
    assert_eq!(code, 0);
    assert_eq!(
        expect(&out),
        expect(
            "render.status needs, before it can run:
               ./certs/ca.pem        (tls ca_file)
               ./certs/client.pem    (tls cert_file)
               ./certs/client.key    (tls key_file, secret)
             report.upload needs, before it can run:
               ./data/template.json  (file read)
             status.get needs, before it can run:
               ./certs/ca.pem        (tls ca_file)"
        )
    );
    let rt = Runtime::builder().file(TLS).build().unwrap();
    let m = rt
        .io(&IoQuery {
            ids: vec!["render.status".into()],
            needs: true,
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap()
        .manifest;
    let files = &m.needs[0].files;
    assert_eq!(files[2].effect_id, "render.status#4");
    assert!(files[2].secret);
    assert_eq!(files[2].source.start_line, 17);
    let upload = rt
        .io(&IoQuery {
            ids: vec!["report.upload".into()],
            ..IoQuery::default()
        })
        .unwrap()
        .manifest;
    let body = upload
        .sites
        .iter()
        .find(|s| s.origin.name() == "body file")
        .unwrap();
    assert!(!body.requires_existing);
}

const S156_POLICY: &str = r#"{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./certs/ca.pem", "./certs/client.pem", "./certs/client.key", "./data/template.json"], "access": ["read", "stat"]},
    {"capability": "allow_read", "targets": ["./out/report.json"], "access": ["read"]},
    {"capability": "allow_write", "targets": ["./out/report.json"], "access": ["create"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443", "https://render.example.com:443", "https://status.example.com:443"]}
  ],
  "network": {"deny_private_ranges": true}
}"#;

fn tls_bundle(with_key: bool) -> tempfile::TempDir {
    let d = bundle(TLS, "tls.rivet", Some(S156_POLICY));
    std::fs::create_dir_all(d.path().join("certs")).unwrap();
    std::fs::create_dir_all(d.path().join("data")).unwrap();
    for f in ["certs/ca.pem", "certs/client.pem", "data/template.json"] {
        std::fs::write(d.path().join(f), "x").unwrap();
    }
    if with_key {
        std::fs::write(d.path().join("certs/client.key"), "SECRET-KEY-MATERIAL").unwrap();
    }
    d
}

// vhco:test audit.inspect_effects -- S156/S157 --check-files: all present exit 0, missing key exit 4, read-only policy without stat = not_permitted exit 3
#[test]
fn s156_s157_check_files() {
    let d = tls_bundle(true);
    let app = path(&d, "tls.rivet");
    let (code, out, err) = rivet(&["--file", &app, "io", "--check-files"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out.matches("present").count(), 5);
    assert!(err.contains("4 files · 4 present"));
    assert!(!out.contains("SECRET-KEY-MATERIAL"));

    let d = tls_bundle(false);
    let app = path(&d, "tls.rivet");
    let (code, out, _) = rivet(&["--file", &app, "io", "render.status", "--check-files"]);
    assert_eq!(code, 4);
    assert!(out.contains("missing"));

    let d = tls_bundle(true);
    std::fs::create_dir_all(d.path().join("policies")).unwrap();
    std::fs::write(
        d.path().join("policies/read-only.json"),
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["../certs/ca.pem","../certs/client.pem","../certs/client.key"],"access":["read"]}]}"#,
    )
    .unwrap();
    let (code, out, _) = rivet(&[
        "--file",
        &path(&d, "tls.rivet"),
        "--policy",
        &path(&d, "policies/read-only.json"),
        "io",
        "render.status",
        "--check-files",
    ]);
    assert_eq!(code, 3);
    assert_eq!(out.matches("not_permitted").count(), 3);

    // library: statuses are filled on the manifest
    let keep = tls_bundle(true);
    let rt = Runtime::builder()
        .file(&path(&keep, "tls.rivet"))
        .build()
        .unwrap();
    let m = rt
        .io(&IoQuery {
            check_files: true,
            ..IoQuery::default()
        })
        .unwrap()
        .manifest;
    assert!(
        m.needs
            .iter()
            .flat_map(|n| &n.files)
            .all(|f| f.status == Some(FileStatus::Present))
    );
}

// ---------------------------------------------------------------- trace join

// vhco:test audit.inspect_effects -- S153 planned vs actual: a real request's broker decisions carry effect_ids that --trace joins into attempts; unknown request is not_found; the one-shot CLI trace store is empty (exit 4)
#[tokio::test]
async fn s153_trace_join() {
    let d = tempfile::tempdir().unwrap();
    std::fs::copy(
        "docs/demos/11-sandbox/app.rivet",
        d.path().join("app.rivet"),
    )
    .unwrap();
    std::fs::copy(
        "docs/demos/11-sandbox/policy.json",
        d.path().join("policy.json"),
    )
    .unwrap();
    std::fs::create_dir_all(d.path().join("data")).unwrap();
    std::fs::write(d.path().join("data/public.json"), r#"{"message":"hi"}"#).unwrap();
    let rt = Runtime::builder()
        .file(&path(&d, "app.rivet"))
        .build()
        .unwrap();
    let c = rt.request("data.read", Value::Null, None).await.unwrap();
    let t = rt.trace(&c.request_id).unwrap();
    assert_eq!(t.events.len(), 1);
    assert_eq!(t.events[0].effect_id.as_deref(), Some("data.read#1"));
    assert_eq!(t.events[0].decision, "allowed");
    let r = rt
        .io(&IoQuery {
            ids: vec!["data.read".into()],
            trace_request_id: Some(c.request_id.clone()),
            ..IoQuery::default()
        })
        .unwrap();
    let a = r.manifest.sites[0].attempts.clone().unwrap();
    assert_eq!(a.count, 1);
    assert_eq!(a.last_decision.as_deref(), Some("allowed"));
    assert!(r.rendered.contains("ATTEMPTS"));
    assert!(r.rendered.contains("1 allowed"));
    let e = rt
        .io(&IoQuery {
            trace_request_id: Some("req_nope".into()),
            ..IoQuery::default()
        })
        .unwrap_err();
    assert_eq!(e.exit_code(), 4);
    let (code, _, _) = rivet(&["--file", &demo("11-sandbox"), "trace", "show", "req_01"]);
    assert_eq!(code, 4);
    let (code, _, _) = rivet(&["--file", &demo("11-sandbox"), "io", "--trace", "req_01"]);
    assert_eq!(code, 4);
}

#[test]
fn fixtures_exist() {
    assert!(Path::new(FIXTURE).exists() && Path::new(TLS).exists());
}
