//! T-26 `policy generate` conformance (REF-2026-0002 S148, S149, S152, S158;
//! docs/demos/11-sandbox "Generate a least-privilege draft").

use rivet::Runtime;
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

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap()
}

const FIXTURE: &str = "tests/fixtures/io_manifest/app.rivet";

// vhco:test policy.generate_policy -- S148 full draft: one grant per (capability, target), narrowed access, origin/glob collapse, dynamic site on stderr, exit 7
#[test]
fn s148_draft_with_review_item() {
    let (code, out, err) = rivet(&["--file", FIXTURE, "policy", "generate"]);
    assert_eq!(code, 7);
    assert_eq!(
        json(&out),
        json(
            r#"{
  "version": 1,
  "grants": [
    {"capability": "allow_read",    "targets": ["./data/endpoint.json"],        "access": ["read"]},
    {"capability": "allow_read",    "targets": ["./data/input.json"],           "access": ["read"]},
    {"capability": "allow_read",    "targets": ["./out/notes/*.json"],          "access": ["stat"]},
    {"capability": "allow_write",   "targets": ["./out/notes/*.json"],          "access": ["create", "update"]},
    {"capability": "allow_write",   "targets": ["./out/user.json"],             "access": ["create"]},
    {"capability": "allow_delete",  "targets": ["./out/notes/*.json"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_env",     "targets": ["API_KEY"]}
  ],
  "network": {"deny_private_ranges": true}
}"#
        )
    );
    assert!(err.contains(
        "review  sync.push#2  network connect POST  <dynamic: config.url>  app.rivet:72  not granted (dynamic target)"
    ));
    assert!(err.contains("policy generate: 8 grants, 1 review item — draft incomplete"));
    let (code, out, _) = rivet(&["--file", FIXTURE, "policy", "generate", "users.snapshot"]);
    assert_eq!(code, 0);
    assert_eq!(
        json(&out),
        json(
            r#"{"version": 1, "grants": [
    {"capability": "allow_write",   "targets": ["./out/user.json"], "access": ["create"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_env",     "targets": ["API_KEY"]}],
  "network": {"deny_private_ranges": true}}"#
        )
    );
}

// vhco:test policy.generate_policy -- S149 --output refuses an existing policy.json (conflict.exists, exit 4, unchanged) and writes a new draft exclusively (exit 7)
#[test]
fn s149_output_never_overwrites() {
    let d = tempfile::tempdir().unwrap();
    std::fs::copy(FIXTURE, d.path().join("app.rivet")).unwrap();
    let policy = r#"{"version": 1}"#;
    std::fs::write(d.path().join("policy.json"), policy).unwrap();
    let app = d.path().join("app.rivet").to_string_lossy().to_string();
    let existing = d.path().join("policy.json").to_string_lossy().to_string();
    let (code, out, err) = rivet(&["--file", &app, "policy", "generate", "--output", &existing]);
    assert_eq!(code, 4);
    assert!(out.is_empty());
    assert!(err.contains("conflict.exists"));
    assert_eq!(std::fs::read_to_string(&existing).unwrap(), policy);
    let draft = d
        .path()
        .join("policy.draft.json")
        .to_string_lossy()
        .to_string();
    let (code, _, err) = rivet(&["--file", &app, "policy", "generate", "--output", &draft]);
    assert_eq!(code, 7);
    assert!(err.contains("sync.push#2"));
    let written = json(&std::fs::read_to_string(&draft).unwrap());
    assert_eq!(written["grants"].as_array().unwrap().len(), 8);
    // the draft is loadable with the same schema
    Runtime::builder()
        .file(&app)
        .policy_file(&draft)
        .build()
        .unwrap();
}

// vhco:test policy.generate_policy -- --output in a subdirectory rebases bundle-relative paths onto the draft's own directory
#[test]
fn output_rebases_relative_paths() {
    let d = tempfile::tempdir().unwrap();
    std::fs::copy(
        "docs/demos/11-sandbox/app.rivet",
        d.path().join("app.rivet"),
    )
    .unwrap();
    std::fs::create_dir_all(d.path().join("policies")).unwrap();
    let app = d.path().join("app.rivet").to_string_lossy().to_string();
    let out = d
        .path()
        .join("policies/draft.json")
        .to_string_lossy()
        .to_string();
    let (code, _, _) = rivet(&[
        "--file",
        &app,
        "policy",
        "generate",
        "data.read",
        "--output",
        &out,
    ]);
    assert_eq!(code, 0);
    let j = json(&std::fs::read_to_string(&out).unwrap());
    assert_eq!(j["grants"][0]["targets"][0], "../data/public.json");
}

// vhco:test policy.generate_policy -- S158 option-derived files granted exactly with access [read], ca.pem once, no directory glob, exit 0
#[test]
fn s158_option_files_exact() {
    let (code, out, _) = rivet(&[
        "--file",
        "tests/fixtures/tls/tls.rivet",
        "policy",
        "generate",
    ]);
    assert_eq!(code, 0);
    let j = json(&out);
    let grants = j["grants"].as_array().unwrap();
    assert_eq!(grants.len(), 9);
    let ca: Vec<_> = grants
        .iter()
        .filter(|g| g["targets"][0] == "./certs/ca.pem")
        .collect();
    assert_eq!(ca.len(), 1);
    assert_eq!(ca[0]["access"], serde_json::json!(["read"]));
    assert!(!out.contains("./certs/**"));
    assert!(!out.contains("serve"));
}

// vhco:test policy.generate_policy -- a param_dependent tls key_file path is a review item (exit 7), never widened to a glob
#[test]
fn param_dependent_option_file_is_review() {
    let rt = Runtime::builder()
        .source(
            "app.rivet",
            "operation t.key\n    param tenant text required\n    output json\n    response = http get \"https://render.example.com/s\"\n        tls key_file \"./certs/${tenant}.key\"\n        decode json\n    end\n    return response.body\nend\n",
            ".",
        )
        .build()
        .unwrap();
    let d = rt.generate_policy(&[]).unwrap();
    assert_eq!(d.exit_code, 7);
    assert!(!d.complete);
    assert_eq!(d.review.len(), 1);
    assert_eq!(d.review[0].origin.name(), "tls key_file");
    assert!(!d.policy_json().to_string().contains("*.key"));
}

// vhco:test policy.generate_policy -- S152 library: rt.generate_policy(&["users.snapshot"]) is complete and matches S148's second draft; demo 11 draft for the intended ops
#[test]
fn s152_library_and_demo11() {
    let rt = Runtime::builder().file(FIXTURE).build().unwrap();
    let d = rt.generate_policy(&["users.snapshot"]).unwrap();
    assert!(d.complete && d.review.is_empty());
    assert_eq!(d.grants.len(), 3);
    let j = d.to_json();
    assert_eq!(j["complete"], true);
    assert_eq!(j["policy"]["network"]["deny_private_ranges"], true);
    let rt = Runtime::builder()
        .file("docs/demos/11-sandbox/app.rivet")
        .build()
        .unwrap();
    let d = rt
        .generate_policy(&["demo.echo", "data.read", "data.snapshot"])
        .unwrap();
    assert_eq!(
        d.policy_json(),
        json(
            r#"{"version": 1,
 "grants": [{"capability": "allow_read",  "targets": ["./data/public.json"],  "access": ["read"]},
            {"capability": "allow_write", "targets": ["./out/snapshot.json"], "access": ["create"]}],
 "network": {"deny_private_ranges": true}}"#
        )
    );
}
