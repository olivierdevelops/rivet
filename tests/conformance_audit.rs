//! T-09 audit conformance (portions owned by WS-C): every lowered effect site
//! is reported with a deterministic effect_id, and secret values never appear
//! in manifests, drafts or traces.

use rivet::Runtime;
use rivet::domain::Value;
use rivet::domain::io_manifest::{IoQuery, SiteKind};

const SRC: &str = r#"operation a.all
    param name text required
    param mode text required enum ["x", "y"]
    output json
    secret token from env "RIVET_TEST_TOKEN" for "https://api.example.com:443"
    if name == "z"
        r = http post "https://api.example.com/items/${name}"
            header "Authorization" "Bearer ${token}"
            body file "./data/upload.bin"
            decode json
        end
    end
    file update "./out/${mode}.json" json {n: 1}
    file write "./out/w.json" json {n: 1}
    file copy "./in/a.json" to "./out/b.json"
    file move "./in/c.json" to "./out/d.json"
    entries = file list "./in"
    info = file stat "./in/a.json"
    file append "./log/a.log" text "x"
    file delete "./tmp/x.json"
        missing ok
    end
    out = command "/usr/bin/printf"
        args ["%s", "x"]
    end
    with tcp "db.example.com:5432" as conn
        conn.send text "ping"
    end
    with unix "/tmp/s.sock" as u
        u.send text "x"
    end
    return {ok: true}
end
"#;

fn site_ids(rt: &Runtime) -> Vec<(String, String, String)> {
    rt.io(&IoQuery {
        format: "json".into(),
        ..IoQuery::default()
    })
    .unwrap()
    .manifest
    .sites
    .iter()
    .map(|s| {
        (
            s.effect_id.clone(),
            s.kind.as_str().to_string(),
            s.access_label(),
        )
    })
    .collect()
}

// vhco:test audit.inspect_effects -- every site kind of one operation is reported in source order with op#N ids, conditions, bounded enum targets and verb expansion (update, write, copy, move)
#[test]
fn every_site_is_reported() {
    let rt = Runtime::builder()
        .source("app.rivet", SRC, ".")
        .build()
        .unwrap();
    let ids = site_ids(&rt);
    let want: Vec<(&str, &str, &str)> = vec![
        ("a.all#1", "env", "read"),
        ("a.all#2", "network", "connect POST"),
        ("a.all#3", "file", "read"),
        ("a.all#4", "file", "stat"),
        ("a.all#5", "file", "update"),
        ("a.all#6", "file", "create, update"),
        ("a.all#7", "file", "read"),
        ("a.all#8", "file", "create"),
        ("a.all#9", "file", "read"),
        ("a.all#10", "file", "create"),
        ("a.all#11", "file", "delete"),
        ("a.all#12", "file", "list"),
        ("a.all#13", "file", "stat"),
        ("a.all#14", "file", "append"),
        ("a.all#15", "file", "delete"),
        ("a.all#16", "process", "exec"),
        ("a.all#17", "network", "connect"),
        ("a.all#18", "unix", "connect"),
    ];
    let got: Vec<(&str, &str, &str)> = ids
        .iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();
    assert_eq!(got, want);
    let m = rt.io(&IoQuery::default()).unwrap().manifest;
    let post = &m.sites[1];
    assert_eq!(post.condition.as_deref(), Some("name == \"z\""));
    assert_eq!(post.secrets, vec!["token"]);
    let upd = &m.sites[4];
    assert_eq!(upd.knowledge.as_str(), "bounded");
    assert_eq!(upd.target.bound, vec!["./out/x.json", "./out/y.json"]);
    assert!(
        !m.sites[14].requires_existing,
        "missing ok tolerates absence"
    );
    let tcp = &m.sites[16];
    assert_eq!(tcp.target.template, "tcp://db.example.com:5432");
    assert!(m.sites.iter().any(|s| s.kind == SiteKind::Unix));
    // policy generate expands a bounded target to its bound set
    let d = rt.generate_policy(&[]).unwrap();
    let j = d.policy_json().to_string();
    assert!(j.contains("./out/x.json") && j.contains("./out/y.json"));
}

// vhco:test audit.inspect_effects -- secret values never appear in io output (any format), policy drafts or traces; only names and paths
#[tokio::test]
async fn secret_values_never_printed() {
    // SAFETY: test-local variable name, set before any thread reads it.
    unsafe { std::env::set_var("RIVET_TEST_TOKEN", "s3cr3t-value-123") };
    let rt = Runtime::builder()
        .source("app.rivet", SRC, ".")
        .build()
        .unwrap();
    for format in ["table", "json", "markdown", "csv"] {
        for by in ["operation", "target", "capability"] {
            let r = rt
                .io(&IoQuery {
                    format: format.into(),
                    by: by.into(),
                    check_policy: true,
                    ..IoQuery::default()
                })
                .unwrap();
            assert!(!r.rendered.contains("s3cr3t"), "{format}/{by}");
            assert!(!r.diagnostics.contains("s3cr3t"));
            assert!(r.rendered.contains("RIVET_TEST_TOKEN"), "{format}/{by}");
        }
    }
    let d = rt.generate_policy(&[]).unwrap();
    assert!(!d.policy_json().to_string().contains("s3cr3t"));
    assert!(d.policy_json().to_string().contains("RIVET_TEST_TOKEN"));
    // a real request is denied (no policy) and its trace holds no secret value
    let c = rt
        .request(
            "a.all",
            Value::object([("name", Value::text("q")), ("mode", Value::text("x"))]),
            None,
        )
        .await;
    if let Err(e) = &c {
        assert!(!e.message.contains("s3cr3t"));
        if let Some(req) = &e.request_id
            && let Ok(t) = rt.trace(req)
        {
            for ev in t.events {
                assert!(!ev.to_json().to_string().contains("s3cr3t"));
            }
        }
    }
    // TLS key file path shown, content never read
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy("tests/fixtures/tls/tls.rivet", dir.path().join("tls.rivet")).unwrap();
    std::fs::create_dir_all(dir.path().join("certs")).unwrap();
    std::fs::write(dir.path().join("certs/client.key"), "PRIVATE-KEY-BYTES").unwrap();
    let rt = Runtime::builder()
        .file(&dir.path().join("tls.rivet").to_string_lossy())
        .build()
        .unwrap();
    let r = rt
        .io(&IoQuery {
            check_files: true,
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap();
    assert!(r.rendered.contains("./certs/client.key"));
    assert!(!r.rendered.contains("PRIVATE-KEY-BYTES"));
}

// vhco:test audit.read_trace -- broker decisions of a nested request are traced with the callee's effect_id and denied attempts are recorded too
#[tokio::test]
async fn trace_records_denials_with_effect_ids() {
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
    // The demo README runs `mkdir -p out`: parent directories are never created implicitly.
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    std::fs::write(d.path().join("data/public.json"), r#"{"message":"hi"}"#).unwrap();
    let rt = Runtime::builder()
        .file(&d.path().join("app.rivet").to_string_lossy())
        .build()
        .unwrap();
    let e = rt
        .request("data.private", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    let req = e.request_id.clone().expect("request id on error");
    let t = rt.trace(&req).unwrap();
    assert_eq!(t.events[0].effect_id.as_deref(), Some("data.private#1"));
    assert_eq!(t.events[0].decision, "denied");
    let ok = rt
        .request("data.snapshot", Value::Null, None)
        .await
        .unwrap();
    let t = rt.trace(&ok.request_id).unwrap();
    let ids: Vec<_> = t
        .events
        .iter()
        .filter_map(|e| e.effect_id.clone())
        .collect();
    assert!(ids.contains(&"data.snapshot#1".to_string()));
}

// vhco:test audit.read_trace -- G5: trace export writes the request's trace JSON to a new file only with allow_write on the path, refuses to overwrite, reports unknown requests as not_found, and works from the CLI command too
#[tokio::test]
async fn trace_export_needs_write_grant_and_never_overwrites() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("app.rivet"),
        "operation t.read\n    output json\n    return file read \"./data/a.json\" as json\nend\n",
    )
    .unwrap();
    std::fs::create_dir_all(d.path().join("data")).unwrap();
    std::fs::create_dir_all(d.path().join("audit")).unwrap();
    std::fs::write(d.path().join("data/a.json"), "{\"n\":1}").unwrap();
    let load = |policy: &str| {
        std::fs::write(d.path().join("policy.json"), policy).unwrap();
        Runtime::builder()
            .file(&d.path().join("app.rivet").to_string_lossy())
            .build()
            .unwrap()
    };
    // Without a write grant the export is denied and nothing is written.
    let rt =
        load(r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"]}]}"#);
    let c = rt.request("t.read", Value::Null, None).await.unwrap();
    let e = rt
        .export_trace(&c.request_id, "./audit/t.json")
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    assert!(!d.path().join("audit/t.json").exists());

    let rt = load(
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"]},{"capability":"allow_write","targets":["./audit/**"]}]}"#,
    );
    let c = rt.request("t.read", Value::Null, None).await.unwrap();
    let receipt = rt
        .export_trace(&c.request_id, "./audit/t.json")
        .await
        .unwrap();
    assert!(receipt.events >= 1);
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("audit/t.json")).unwrap()).unwrap();
    assert_eq!(written, rt.trace(&c.request_id).unwrap().to_json());
    // Never overwrites.
    let e = rt
        .export_trace(&c.request_id, "./audit/t.json")
        .await
        .unwrap_err();
    assert_eq!(e.code, "conflict.already_exists");
    // Unknown request: not_found, nothing written.
    let e = rt
        .export_trace("req_missing", "./audit/missing.json")
        .await
        .unwrap_err();
    assert!(e.code.starts_with("not_found"), "{}", e.code);
    assert!(!d.path().join("audit/missing.json").exists());

    // CLI: a fresh process has no recorded request → not_found (exit 4).
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args([
            "--file",
            &d.path().join("app.rivet").to_string_lossy(),
            "trace",
            "export",
            "req_missing",
            "--output",
            &d.path().join("audit/cli.json").to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(!d.path().join("audit/cli.json").exists());
}
