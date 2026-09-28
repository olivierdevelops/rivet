//! T-02 — one operation, every access point: the real `rivet` binary run
//! locally (`--file`) and as a thin client of an in-process `rivet serve`
//! (`--endpoint`) prints the same results and exits with the same registry
//! codes; OAuth transactions survive across CLI invocations against one host.
//!
//! ```text
//!  rivet --file app.rivet  request/list/describe/outputs/io/trace  ─┐
//!                                                                   ├─ same stdout (IDs aside) + exit code
//!  rivet --endpoint http://127.0.0.1:N  (same commands) ─▶ serve ──┘
//!  auth begin (invocation 1) ─▶ fake provider approve ─▶ auth complete (invocation 2) ─▶ connected
//! ```

mod oauth_support;
mod support;

use rivet::Runtime;
use rivet::orchestrator::setup_serve::{ServeHandle, ServeOptions, start};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const RIVET: &str = env!("CARGO_BIN_EXE_rivet");

#[derive(Debug, Clone, PartialEq)]
struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Run the binary with `args`, feeding `stdin` (then EOF) when given.
async fn cli(args: &[&str], stdin: Option<&str>) -> Out {
    let mut cmd = tokio::process::Command::new(RIVET);
    cmd.args(args)
        .stdin(if stdin.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(text) = stdin {
        let mut s = child.stdin.take().unwrap();
        s.write_all(text.as_bytes()).await.unwrap();
        drop(s);
    }
    let out = child.wait_with_output().await.unwrap();
    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Drop the generated request/trace IDs so two runs compare by content.
fn strip_ids(j: &mut Json) {
    match j {
        Json::Object(m) => {
            m.remove("request_id");
            m.remove("trace_id");
            for v in m.values_mut() {
                strip_ids(v);
            }
        }
        Json::Array(a) => a.iter_mut().for_each(strip_ids),
        _ => {}
    }
}

fn normalize(text: &str) -> String {
    text.lines()
        .map(|l| match serde_json::from_str::<Json>(l) {
            Ok(mut j) => {
                strip_ids(&mut j);
                j.to_string()
            }
            Err(_) => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct Host {
    _dir: tempfile::TempDir,
    file: PathBuf,
    url: String,
    handle: ServeHandle,
}

async fn host_for(dir: tempfile::TempDir, file: PathBuf) -> Host {
    let rt = Runtime::builder()
        .file(file.to_str().unwrap())
        .build()
        .unwrap();
    let handle = start(
        rt,
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let url = format!("http://{}", handle.addr.unwrap());
    Host {
        _dir: dir,
        file,
        url,
        handle,
    }
}

fn bundle(source: &str, policy: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("app.rivet");
    std::fs::write(&file, source).unwrap();
    if let Some(p) = policy {
        std::fs::write(dir.path().join("policy.json"), p).unwrap();
    }
    (dir, file)
}

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.to_str().unwrap().to_string()
}

// vhco:test execution.request_operation -- the CLI run locally (--file) and against a running serve (--endpoint) prints the same Completion, NDJSON stream, validation ErrorEnvelope, tables and JSON for request/list/describe/outputs/io/trace, with identical exit codes
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_and_remote_cli_agree() {
    let (dir, file) = bundle(&support::catalog(), None);
    let host = host_for(dir, file).await;
    let local = ["--file", host.file.to_str().unwrap()];
    let remote = ["--endpoint", host.url.as_str()];
    let cases: Vec<(Vec<&str>, Option<&str>)> = vec![
        (
            vec!["request", "demo.add", "--data", r#"{"a":2,"b":3}"#],
            None,
        ),
        (vec!["request", "demo.add", "--data", r#"{"b":3}"#], None),
        // G19: the 600000 ms host cap applies to local and --endpoint alike.
        (
            vec![
                "request",
                "demo.add",
                "--data",
                r#"{"a":1,"b":1}"#,
                "--timeout",
                "10m",
            ],
            None,
        ),
        (
            vec![
                "--json",
                "request",
                "demo.add",
                "--data",
                r#"{"a":1,"b":1}"#,
                "--timeout",
                "601s",
            ],
            None,
        ),
        (vec!["request", "nope.op"], None),
        (
            vec!["request", "demo.count", "--data", r#"{"n":3}"#, "--stream"],
            None,
        ),
        (
            vec!["request", "demo.relay", "--input-jsonl", "-", "--stream"],
            Some("\"a\"\n\n\"b\"\n"),
        ),
        (
            vec!["request", "demo.relay", "--input-jsonl", "-", "--stream"],
            Some("\"a\"\n{not json\n"),
        ),
        (
            vec!["request", "demo.relay", "--input-jsonl", "-", "--stream"],
            Some("42\n"),
        ),
        (
            vec!["request", "demo.relay", "--input-jsonl", "-"],
            Some(""),
        ),
        (vec!["list"], None),
        (vec!["list", "--outputs"], None),
        (vec!["list", "--json"], None),
        (vec!["describe", "demo.add"], None),
        (vec!["describe", "demo.count", "demo.add", "--json"], None),
        (vec!["describe", "nope.op"], None),
        (vec!["outputs", "demo.add"], None),
        (vec!["outputs", "demo.add", "--json"], None),
        (vec!["outputs", "--all", "--json"], None),
        (vec!["outputs", "--all"], None),
        (vec!["outputs"], None),
        (vec!["io", "demo.add"], None),
        (vec!["io", "demo.add", "--trace", "req_unknown"], None),
        (vec!["io", "--by", "target", "--format", "json"], None),
        (vec!["trace", "show", "req_unknown"], None),
    ];
    for (args, stdin) in cases {
        let l = cli(&[&local[..], &args[..]].concat(), stdin).await;
        let r = cli(&[&remote[..], &args[..]].concat(), stdin).await;
        assert_eq!(l.code, r.code, "{args:?}\nlocal {l:?}\nremote {r:?}");
        // After a cancelled live input, how many items were echoed first is a race.
        let cancelled_input = stdin.is_some_and(|s| !s.contains("\"b\"")) && l.code != 0;
        if !cancelled_input {
            assert_eq!(
                normalize(&l.stdout),
                normalize(&r.stdout),
                "{args:?}\nlocal {l:?}\nremote {r:?}"
            );
        }
        if l.code != 0 {
            // The terminal record of a cancelled live input carries `seq` and
            // `data_count`, which count those racing items: compare without them.
            let err = |s: &str| {
                if cancelled_input {
                    normalize(s)
                        .lines()
                        .map(|l| match serde_json::from_str::<Json>(l) {
                            Ok(Json::Object(mut m)) => {
                                m.remove("seq");
                                m.remove("data_count");
                                Json::Object(m).to_string()
                            }
                            _ => l.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    normalize(s)
                }
            };
            assert_eq!(
                err(&l.stderr),
                err(&r.stderr),
                "{args:?}\nlocal {l:?}\nremote {r:?}"
            );
        }
        match args.as_slice() {
            [_, "demo.add", "--data", p] if p.contains("\"a\"") => {
                assert_eq!(l.code, 0);
                let j: Json = serde_json::from_str(r.stdout.trim()).unwrap();
                assert_eq!(j["data"], 5);
            }
            [_, "demo.add", "--data", _] => {
                assert_eq!(r.code, 2);
                assert!(r.stderr.contains("validation.required"), "{r:?}");
            }
            [_, "demo.relay", "--input-jsonl", "-", "--stream"] => match stdin {
                Some(s) if s.contains("\"b\"") => {
                    assert_eq!(r.code, 0, "{r:?}");
                    let lines: Vec<Json> = r
                        .stdout
                        .lines()
                        .map(|l| serde_json::from_str(l).unwrap())
                        .collect();
                    assert_eq!(lines.len(), 3, "{r:?}");
                    assert_eq!(
                        (&lines[0]["data"], &lines[1]["data"]),
                        (&json!("a"), &json!("b"))
                    );
                    assert!(lines[0]["request_id"].as_str().unwrap().starts_with("req_"));
                    assert_eq!(
                        (&lines[2]["type"], &lines[2]["data"]),
                        (&json!("result"), &json!(2))
                    );
                }
                _ => {
                    assert_eq!(r.code, 2, "{r:?}");
                    assert!(r.stderr.contains("validation.input"), "{r:?}");
                }
            },
            ["request", "demo.count", ..] => assert_eq!(r.stdout.lines().count(), 4, "{r:?}"),
            ["request", "nope.op"] | ["describe", "nope.op"] | ["trace", "show", _] => {
                assert_eq!(r.code, 4, "{r:?}")
            }
            ["outputs"] => assert_eq!(r.code, 2),
            [.., "--timeout", "10m"] => assert_eq!(l.code, 0, "{l:?}"),
            [.., "--timeout", "601s"] => {
                assert_eq!(l.code, 2, "{l:?}");
                assert!(l.stderr.contains("validation.usage"), "{l:?}");
                assert!(l.stderr.contains("600000"), "{l:?}");
            }
            _ => {}
        }
    }

    // --endpoint and --file together are a usage error; a closed port is a connection error.
    let both = cli(&[&local[..], &remote[..], &["list"]].concat(), None).await;
    assert_eq!(both.code, 2, "{both:?}");
    assert!(both.stderr.contains("validation.usage"));
    let closed = cli(&["--endpoint", "http://127.0.0.1:9", "list"], None).await;
    assert_eq!(closed.code, 5, "{closed:?}");
    host.handle.shutdown().await;
}

// vhco:test serve.authenticate_principal -- --token-file sends the bearer (never on argv or env); without it the server answers 401 (exit 3); a network principal may not read traces or the I/O manifest unless listed explicitly (exit 3)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_file_and_sensitive_builtins() {
    let token = "t0ken-for-ada";
    let sha: String = Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let policy = format!(
        r#"{{"version":1,"serve":{{"auth":{{"type":"bearer","tokens":[{{"principal":"ada","sha256":"{sha}"}},{{"principal":"ops","sha256":"{}"}}]}},
            "principals":{{"ada":{{"operations":["demo.*"]}},"ops":{{"operations":["demo.*","rivet.trace.show","rivet.io"]}}}}}}}}"#,
        Sha256::digest(b"ops-token")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let (dir, file) = bundle(&support::catalog(), Some(&policy));
    let ada = write(dir.path(), "ada.token", &format!("{token}\n"));
    let ops = write(dir.path(), "ops.token", "ops-token");
    let host = host_for(dir, file).await;
    let url = host.url.as_str();
    let add = ["request", "demo.add", "--data", r#"{"a":1,"b":1}"#];
    let anon = cli(&[&["--endpoint", url][..], &add[..]].concat(), None).await;
    assert_eq!(anon.code, 3, "{anon:?}");
    assert!(anon.stderr.contains("\"kind\":\"auth\""), "{anon:?}");
    let ok = cli(
        &[&["--endpoint", url, "--token-file", &ada][..], &add[..]].concat(),
        None,
    )
    .await;
    assert_eq!(ok.code, 0, "{ok:?}");
    assert!(!ok.stdout.contains(token) && !ok.stderr.contains(token));
    let rid = serde_json::from_str::<Json>(ok.stdout.trim()).unwrap()["request_id"]
        .as_str()
        .unwrap()
        .to_string();
    for args in [vec!["trace", "show", rid.as_str()], vec!["io", "demo.add"]] {
        let denied = cli(
            &[&["--endpoint", url, "--token-file", &ada][..], &args[..]].concat(),
            None,
        )
        .await;
        assert_eq!(denied.code, 3, "{args:?} {denied:?}");
    }
    // An explicitly listed principal reads the host's io manifest; pure demo.add
    // made no effect attempts, so its trace is not_found (exit 4) rather than 403.
    let io = cli(
        &["--endpoint", url, "--token-file", &ops, "io", "demo.add"],
        None,
    )
    .await;
    assert_eq!(io.code, 0, "{io:?}");
    assert!(io.stdout.contains("OPERATION"), "{io:?}");
    let tr = cli(
        &[
            "--endpoint",
            url,
            "--token-file",
            &ops,
            "trace",
            "show",
            &rid,
        ],
        None,
    )
    .await;
    assert_eq!(tr.code, 4, "{tr:?}");
    // --token-file without --endpoint is a usage error.
    let bad = cli(&["--token-file", &ops, "list"], None).await;
    assert_eq!(bad.code, 2, "{bad:?}");
    host.handle.shutdown().await;
}

// vhco:test auth.complete_authorization -- OAuth begin and complete run as two separate CLI invocations against one running host (its memory transaction store), then a protected request uses the connected account; a standalone local CLI cannot complete that transaction; no token or code is printed
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oauth_begin_complete_across_invocations() {
    let f = oauth_support::Fake::start().await;
    let (a, r) = (f.auth_origin(), f.api_origin());
    let source = format!(
        "auth crm_user oauth2\n    flow authorization_code\n    pkce s256\n    issuer \"{a}\"\n    authorization_url \"{a}/authorize\"\n    token_url \"{a}/token\"\n    client_id \"rivet-desktop\"\n    client_auth none\n    redirect_uri \"https://app.example.com/oauth/callback\"\n    scopes [\"contacts.read\"]\n    resource_origins [\"{r}\"]\n    store memory\nend\n\noperation user.contacts\n    output json\n    response = http get \"{r}/contacts\"\n        auth crm_user account \"ada\"\n        decode json\n    end\n    return response.body\nend\n"
    );
    let policy = format!(
        r#"{{"version": 1, "grants": [
            {{"capability": "allow_auth", "targets": ["crm_user/ada/*"]}},
            {{"capability": "allow_credentials", "targets": ["crm_user/ada"]}},
            {{"capability": "allow_network", "targets": ["{a}", "{r}"]}}]}}"#
    );
    let (dir, file) = bundle(&source, Some(&policy));
    let dir_path = dir.path().to_path_buf();
    let host = host_for(dir, file).await;
    let ep = ["--endpoint", host.url.as_str()];
    let begin = cli(
        &[&ep[..], &["auth", "begin", "crm_user", "--account", "ada"]].concat(),
        None,
    )
    .await;
    assert_eq!(begin.code, 0, "{begin:?}");
    let challenge: Json = serde_json::from_str(begin.stdout.trim()).unwrap();
    let tx = challenge["data"]["transaction_id"].as_str().unwrap();
    let (code, state) = f.approve(challenge["data"]["authorization_url"].as_str().unwrap());
    let cb = write(
        &dir_path,
        "callback.json",
        &json!({"transaction_id": tx, "callback": {"code": code, "state": state,
            "redirect_uri": "https://app.example.com/oauth/callback", "issuer": a}})
        .to_string(),
    );
    // A separate local process has its own (empty) memory store.
    let local = cli(
        &[
            "--file",
            host.file.to_str().unwrap(),
            "auth",
            "complete",
            "--params-file",
            &cb,
        ],
        None,
    )
    .await;
    assert_eq!(local.code, 4, "{local:?}");
    let done = cli(
        &[
            &ep[..],
            &["auth", "complete", "--params-file", &cb, "--timeout", "10s"],
        ]
        .concat(),
        None,
    )
    .await;
    assert_eq!(done.code, 0, "{done:?}");
    let status: Json = serde_json::from_str(done.stdout.trim()).unwrap();
    assert_eq!(status["data"]["state"], "connected");
    let st = cli(
        &[&ep[..], &["auth", "status", "crm_user", "--account", "ada"]].concat(),
        None,
    )
    .await;
    assert!(st.stdout.contains("\"connected\""), "{st:?}");
    let contacts = cli(&[&ep[..], &["request", "user.contacts"]].concat(), None).await;
    assert_eq!(contacts.code, 0, "{contacts:?}");
    let j: Json = serde_json::from_str(contacts.stdout.trim()).unwrap();
    assert_eq!(j["data"]["contacts"][0]["name"], "Ada");
    let cancel = cli(
        &[&ep[..], &["auth", "cancel", "auth_unknown"]].concat(),
        None,
    )
    .await;
    assert_eq!(cancel.code, 4, "{cancel:?}");
    for o in [&begin, &done, &st, &contacts, &cancel, &local] {
        let all = format!("{}{}", o.stdout, o.stderr);
        assert!(!all.contains("CANARY"), "leaked a secret: {all}");
    }
    host.handle.shutdown().await;
}
