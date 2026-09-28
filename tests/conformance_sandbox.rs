//! T-08 — the sandbox guarantee (PROP-2026-0001 Increment 5, ADR-0003,
//! REF-2026-0002 S66, S129, S139, S140): brokered effects are checked per
//! attempt, child processes are confined by the OS backend whenever a policy
//! is present (macOS Seatbelt; other platforms refuse before spawning), the
//! bootstrap list is published, and secrets stay bound to their origins.
//!
//! ```text
//!  command … ─▶ allow_exec? ─no─▶ permission.denied (no spawn)
//!                  │ yes
//!                  ▼
//!         policy present ─▶ SandboxSpec ─▶ macOS: sandbox-exec (deny default, no network, no fork)
//!                                      └▶ Linux (uncertified) / Windows: unsupported.sandbox_backend, no spawn
//! ```

#[path = "transport_support/mod.rs"]
mod support;

use rivet::domain::io_manifest::IoQuery;
use rivet::domain::{ErrorKind, RivetError, Value};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn op(body: &str) -> String {
    let indented: String = body.lines().map(|l| format!("    {l}\n")).collect();
    format!("operation t.run\n    output json\n{indented}end\n")
}

fn p(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

async fn run(root: &Path, body: &str, policy: &str) -> Result<Value, RivetError> {
    support::runtime(&op(body), &p(root), policy)
        .request("t.run", Value::Null, None)
        .await
        .map(|c| c.result)
}

/// Whether this platform has a certified process sandbox backend (ADR-0003).
fn sandbox_certified() -> bool {
    cfg!(target_os = "macos")
}

/// On platforms without a certified backend the only acceptable outcome is the
/// typed refusal before spawning.
fn assert_refused_before_spawn(e: &RivetError) {
    assert_eq!(e.code, "unsupported.sandbox_backend", "{e:?}");
    assert_eq!((e.kind, e.exit_code()), (ErrorKind::Unsupported, 5));
}

fn bundle() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for d in ["out", "data", "data/private"] {
        std::fs::create_dir_all(tmp.path().join(d)).unwrap();
    }
    std::fs::write(tmp.path().join("data/private/secret.txt"), "secret").unwrap();
    tmp
}

// vhco:test transports.run_process -- S66 without policy.json exec is denied before spawning (the marker file is never created); pure operations still run
#[tokio::test]
async fn no_policy_denies_exec() {
    let b = bundle();
    let marker = b.path().join("out/marker");
    let src = format!(
        "operation t.run\n    output json\n    r = command \"/usr/bin/touch\"\n        args [\"{}\"]\n    end\n    return r.exit\nend\n\noperation t.pure\n    output integer\n    return 3\nend\n",
        p(&marker)
    );
    let rt = rivet::Runtime::builder()
        .source("app.rivet", &src, &p(b.path()))
        .build()
        .unwrap();
    assert!(!rt.policy().present);
    let e = rt.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("permission.denied", 3));
    assert!(!marker.exists(), "no spawn without policy.json");
    assert_eq!(
        rt.request("t.pure", Value::Null, None)
            .await
            .unwrap()
            .result,
        Value::Int(3)
    );
}

fn exec_policy(extra: &str) -> String {
    format!(
        r#"{{"version":1,"grants":[
            {{"capability":"allow_exec","targets":["/usr/bin/touch","/usr/bin/curl","/usr/bin/xargs","/bin/cat"]}},
            {{"capability":"allow_write","targets":["./out/**"]}},
            {{"capability":"allow_read","targets":["./data/**"]}}{extra}]}}"#
    )
}

// vhco:test transports.run_process -- with a policy present a child writes inside granted paths only: writes to ./data, the bundle root and /tmp are refused by the OS
#[tokio::test]
async fn child_cannot_write_outside_granted_paths() {
    let b = bundle();
    let policy = exec_policy("");
    let inside = b.path().join("out/ok.txt");
    let touch = |target: &Path| {
        format!(
            "r = command \"/usr/bin/touch\"\n    args [\"{}\"]\nend\nreturn r.exit",
            p(target)
        )
    };
    let outside_tmp = std::env::temp_dir().join(format!("rivet-t08-{}", std::process::id()));
    let outside = [
        b.path().join("data/pwned.txt"),
        b.path().join("pwned.txt"),
        outside_tmp.clone(),
    ];
    if !sandbox_certified() {
        let e = run(b.path(), &touch(&inside), &policy).await.unwrap_err();
        assert_refused_before_spawn(&e);
        assert!(!inside.exists());
        return;
    }
    let v = run(b.path(), &touch(&inside), &policy).await.unwrap();
    assert_eq!(v, Value::Int(0));
    assert!(inside.exists(), "granted write works");
    for target in &outside {
        let e = run(b.path(), &touch(target), &policy).await.unwrap_err();
        assert_eq!(e.code, "process.exit", "{target:?}: {e:?}");
        assert!(!target.exists(), "{target:?} must not be created");
    }
    // A deny entry carves a hole in a granted tree for the child too.
    let deny = r#"{"version":1,"grants":[
            {"capability":"allow_exec","targets":["/usr/bin/touch"]},
            {"capability":"allow_write","targets":["./out/**"]}],
            "deny":[{"capability":"allow_write","targets":["./out/locked/**"]}]}"#;
    std::fs::create_dir(b.path().join("out/locked")).unwrap();
    let locked = b.path().join("out/locked/x");
    let e = run(b.path(), &touch(&locked), deny).await.unwrap_err();
    assert_eq!(e.code, "process.exit");
    assert!(!locked.exists());
}

// vhco:test transports.run_process -- a sandboxed child cannot reach the network, even an origin the script itself may connect to (zero connections accepted)
#[tokio::test]
async fn child_cannot_reach_the_network() {
    let b = bundle();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&accepted);
    tokio::spawn(async move {
        while listener.accept().await.is_ok() {
            seen.fetch_add(1, Ordering::SeqCst);
        }
    });
    let policy = exec_policy(&format!(
        r#",{{"capability":"allow_network","targets":["http://127.0.0.1:{port}"]}}"#
    ));
    let body = format!(
        "r = command \"/usr/bin/curl\"\n    args [\"-s\", \"-m\", \"3\", \"http://127.0.0.1:{port}/\"]\n    timeout \"10s\"\nend\nreturn r.exit"
    );
    let e = run(b.path(), &body, &policy).await.unwrap_err();
    if !sandbox_certified() {
        assert_refused_before_spawn(&e);
    } else {
        assert_eq!(e.code, "process.exit", "{e:?}");
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        0,
        "no connection from the child"
    );
}

// vhco:test transports.run_process -- process descendants: a sandboxed child cannot fork a grandchild to escape (xargs → touch never runs)
#[tokio::test]
async fn child_cannot_spawn_descendants() {
    let b = bundle();
    let marker = b.path().join("out/grandchild");
    let body = format!(
        "r = command \"/usr/bin/xargs\"\n    args [\"/usr/bin/touch\"]\n    stdin text \"{}\"\n    accept exit [0, 1, 123, 124, 125, 126, 127]\nend\nreturn r.exit",
        p(&marker)
    );
    let out = run(b.path(), &body, &exec_policy("")).await;
    if !sandbox_certified() {
        assert_refused_before_spawn(&out.unwrap_err());
    } else {
        let exit = out.unwrap();
        assert_ne!(exit, Value::Int(0), "xargs must fail to fork");
    }
    assert!(!marker.exists(), "no grandchild ran");
}

// vhco:test transports.run_process -- policies the backend cannot represent exactly (narrowed access, globs) fail unsupported.sandbox_backend before spawning
#[tokio::test]
async fn unrepresentable_policy_refuses_before_spawn() {
    let b = bundle();
    let marker = b.path().join("out/never");
    let body = format!(
        "r = command \"/usr/bin/touch\"\n    args [\"{}\"]\nend\nreturn r.exit",
        p(&marker)
    );
    for extra in [
        r#",{"capability":"allow_write","targets":["./scratch/**"],"access":["create"]}"#,
        r#",{"capability":"allow_read","targets":["./data/*.txt"]}"#,
    ] {
        let e = run(b.path(), &body, &exec_policy(extra)).await.unwrap_err();
        assert_refused_before_spawn(&e);
        assert!(!marker.exists());
    }
}

// vhco:test policy.authorize_effect -- brokered file effects: case variants of a denied path are still denied on case-insensitive volumes, and a hard link inside a granted tree is refused (S139)
#[tokio::test]
async fn case_variants_and_hard_links() {
    let b = bundle();
    let policy = r#"{"version":1,
        "grants":[{"capability":"allow_read","targets":["./data/**","./out/**"]},
                  {"capability":"allow_write","targets":["./out/**"]}],
        "deny":[{"capability":"allow_read","targets":["./data/private/**"]}]}"#;
    let e = run(
        b.path(),
        "return file read \"./data/private/secret.txt\" as text",
        policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    if cfg!(target_os = "macos") {
        // APFS is case-insensitive by default: ./DATA/Private is the same directory.
        let e = run(
            b.path(),
            "return file read \"./DATA/Private/secret.txt\" as text",
            policy,
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Permission, "{e:?}");
    }
    #[cfg(unix)]
    {
        std::fs::hard_link(
            b.path().join("data/private/secret.txt"),
            b.path().join("out/report.txt"),
        )
        .unwrap();
        let e = run(
            b.path(),
            "file update \"./out/report.txt\" text \"pwned\"\nreturn null",
            policy,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "file.hardlink_refused");
        assert_eq!(
            std::fs::read_to_string(b.path().join("data/private/secret.txt")).unwrap(),
            "secret"
        );
    }
}

// vhco:test audit.inspect_effects -- `io --include-bootstrap` publishes the fixed runtime-internal list (bundle, policy.json, CA bundle, resolver, tzdata, descriptors, stdio) and it grants nothing
#[tokio::test]
async fn bootstrap_list_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        "operation config.read\n    output json\n    return file read \"./fixtures/config.json\" as json\nend\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("policy.json"),
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./fixtures/**"]}]}"#,
    )
    .unwrap();
    let rt = rivet::Runtime::builder()
        .file(&p(&dir.path().join("app.rivet")))
        .build()
        .unwrap();
    let report = rt
        .io(&IoQuery {
            include_bootstrap: true,
            format: "json".into(),
            ..IoQuery::default()
        })
        .unwrap();
    let targets: Vec<String> = report
        .manifest
        .bootstrap
        .iter()
        .map(|s| s.target.template.clone())
        .collect();
    let joined = targets.join(" | ");
    for needle in [
        "app.rivet",
        "policy.json",
        "CA bundle",
        "resolv",
        "tzdata",
        "descriptor/schema",
        "stdin, stdout, stderr",
    ] {
        assert!(joined.contains(needle), "{needle} missing from {joined}");
    }
    assert!(
        targets
            .iter()
            .any(|t| t.ends_with("policy.json") && Path::new(t).is_absolute()
                || t.contains(&p(dir.path()))),
        "the discovered policy.json is named: {joined}"
    );
    // Without the flag the bootstrap list is not part of the manifest.
    let plain = rt.io(&IoQuery::default()).unwrap();
    assert!(plain.manifest.bootstrap.is_empty());
    // Bootstrap reads never grant the script anything: the entry file stays unreadable.
    let e = rt
        .request("config.read", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(
        e.kind,
        ErrorKind::NotFound,
        "fixtures missing, but authorized: {e:?}"
    );
    let src =
        "operation t.run\n    output text\n    return file read \"./app.rivet\" as text\nend\n";
    let rt2 = rivet::Runtime::builder()
        .source("app.rivet", src, &p(dir.path()))
        .policy(
            rivet::orchestrator::runtime::policy_from_json(
                br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./fixtures/**"]}]}"#,
                &p(dir.path()),
            )
            .unwrap(),
        )
        .build()
        .unwrap();
    let e = rt2.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
}

const CANARY: &str = "CANARY-T08-SECRET-VALUE";
const SECRET_VAR: &str = "RIVET_T08_API_KEY";

fn set_secret_env() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: set once, before any runtime in this binary reads this variable.
        unsafe { std::env::set_var(SECRET_VAR, CANARY) };
    });
}

// vhco:test transports.exchange_http -- S140 a secret bound with `for ORIGIN` reaches that origin only: another granted origin is denied before connecting, and the value never appears in results, errors or traces
#[tokio::test]
async fn secret_destination_binding() {
    set_secret_env();
    let (bound_port, bound) = support::http_server().await;
    let (other_port, other) = support::http_server().await;
    let bound_origin = format!("http://127.0.0.1:{bound_port}");
    let other_origin = format!("http://127.0.0.1:{other_port}");
    let policy = format!(
        r#"{{"version":1,"grants":[
            {{"capability":"allow_env","targets":["{SECRET_VAR}"]}},
            {{"capability":"allow_network","targets":["{bound_origin}","{other_origin}"]}}]}}"#
    );
    let src = format!(
        "operation account.use\n    output json\n    secret API_KEY from env \"{SECRET_VAR}\" for \"{bound_origin}\"\n    r = http get \"{bound_origin}/users/42\"\n        header \"Authorization\" \"Bearer ${{API_KEY}}\"\n        decode json\n    end\n    return r.body\nend\n\noperation account.mirror\n    output json\n    secret API_KEY from env \"{SECRET_VAR}\" for \"{bound_origin}\"\n    r = http get \"{other_origin}/users/42\"\n        header \"Authorization\" \"Bearer ${{API_KEY}}\"\n        decode json\n    end\n    return r.body\nend\n\noperation account.leak\n    output json\n    secret API_KEY from env \"{SECRET_VAR}\" for \"{bound_origin}\"\n    return {{key: API_KEY}}\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let rt = support::runtime(&src, &p(tmp.path()), &policy);

    let ok = rt.request("account.use", Value::Null, None).await.unwrap();
    assert_eq!(ok.result.get("name"), Some(&Value::text("Ada")));
    assert!(!ok.to_json().to_string().contains(CANARY));
    assert_eq!(bound.requests.lock().unwrap().len(), 1);

    let e = rt
        .request("account.mirror", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("permission.denied", 3),
        "{e:?}"
    );
    assert!(
        other.requests.lock().unwrap().is_empty(),
        "denied before connecting to the other origin"
    );

    // Returning a secret (explicit flow through object construction) is an error.
    let leak = rt.request("account.leak", Value::Null, None).await;
    let leak_err = leak.as_ref().expect_err("returning a secret must fail");
    assert_eq!(leak_err.code, "permission.denied");

    for e in [Some(e), leak.err()].into_iter().flatten() {
        let text = format!("{} {e:?}", e.to_value().to_json());
        assert!(!text.contains(CANARY), "error leaked the secret: {text}");
        if let Some(req) = &e.request_id
            && let Ok(t) = rt.trace(req)
        {
            let t = t.to_json().to_string();
            assert!(!t.contains(CANARY), "trace leaked the secret: {t}");
        }
    }
    let t = rt.trace(&ok.request_id).unwrap().to_json().to_string();
    assert!(!t.contains(CANARY), "trace leaked the secret: {t}");
}

/// A TCP/UDP sink that records every byte it receives.
async fn byte_sinks() -> (u16, u16, Arc<std::sync::Mutex<Vec<u8>>>) {
    use tokio::io::AsyncReadExt;
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_port = tcp.local_addr().unwrap().port();
    let s = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Ok((mut c, _)) = tcp.accept().await {
            let s = Arc::clone(&s);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                while let Ok(n) = c.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    s.lock().unwrap().extend_from_slice(&buf[..n]);
                }
            });
        }
    });
    let udp = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let udp_port = udp.local_addr().unwrap().port();
    let s = Arc::clone(&seen);
    tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        while let Ok((n, _)) = udp.recv_from(&mut buf).await {
            s.lock().unwrap().extend_from_slice(&buf[..n]);
        }
    });
    (tcp_port, udp_port, seen)
}

// vhco:test transports.exchange_http -- G23b secret taint reaches every sink: assignment, interpolation, list/object construction and (chained) base64 encoding are tracked; http headers/body toward an unbound origin, file writes, process args, TCP/UDP sends and nested request params are denied (files/processes always), a bound TCP origin and the bound HTTP origin (even base64-encoded) are allowed, returning an encoded secret is an error, and the canary never reaches a sink, result, error or trace
#[tokio::test]
async fn secret_taint_covers_every_sink() {
    use base64::Engine;
    set_secret_env();
    let (bound_port, bound) = support::http_server().await;
    let (other_port, other) = support::http_server().await;
    let (tcp_port, udp_port, seen) = byte_sinks().await;
    let bound_origin = format!("http://127.0.0.1:{bound_port}");
    let other_origin = format!("http://127.0.0.1:{other_port}");
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("out")).unwrap();
    let policy = format!(
        r#"{{"version":1,"grants":[
            {{"capability":"allow_env","targets":["{SECRET_VAR}"]}},
            {{"capability":"allow_write","targets":["./out/**"]}},
            {{"capability":"allow_exec","targets":["/bin/echo"]}},
            {{"capability":"allow_network","targets":["{bound_origin}","{other_origin}","tcp://127.0.0.1:{tcp_port}","udp://127.0.0.1:{udp_port}"]}}]}}"#
    );
    let secret = format!("    secret K from env \"{SECRET_VAR}\" for \"{bound_origin}\"\n");
    let op =
        |id: &str, body: &str| format!("operation {id}\n    output json\n{secret}{body}end\n\n");
    let mut src = String::new();
    src += &op(
        "s.bound_b64",
        &format!(
            "    enc = (base64.encode \"u:${{K}}\")\n    r = http get \"{bound_origin}/users/42\"\n        header \"Authorization\" \"Basic ${{enc}}\"\n        decode json\n    end\n    return r.body\n"
        ),
    );
    src += &op(
        "s.header_b64_other",
        &format!(
            "    enc = (base64.encode \"u:${{K}}\")\n    r = http get \"{other_origin}/users/42\"\n        header \"X-Key\" enc\n    end\n    return r.status\n"
        ),
    );
    src += &op(
        "s.chain_other",
        &format!(
            "    a = K\n    b = (base64.encode a)\n    c = (base64.encode b)\n    r = http get \"{other_origin}/users/42\"\n        header \"X-Key\" \"v=${{c}}\"\n    end\n    return r.status\n"
        ),
    );
    src += &op(
        "s.body_other",
        &format!(
            "    payload = {{items: [\"x\", K]}}\n    r = http post \"{other_origin}/users\"\n        body json payload\n    end\n    return r.status\n"
        ),
    );
    src += &op(
        "s.query_other",
        &format!("    r = http get \"{other_origin}/search?q=${{K}}\"\n    return r.status\n"),
    );
    src += &op(
        "s.file",
        "    file write \"./out/k.txt\" text \"k=${K}\"\n    return 1\n",
    );
    src += &op(
        "s.process",
        "    r = command \"/bin/echo\"\n        args [\"x\", K]\n    end\n    return r.exit\n",
    );
    src += &op(
        "s.tcp",
        &format!(
            "    with tcp \"127.0.0.1:{tcp_port}\" as c\n        framing newline\n        c.send text \"k=${{K}}\"\n    end\n    return 1\n"
        ),
    );
    src += &op(
        "s.udp",
        &format!(
            "    enc = (base64.encode K)\n    with udp \"127.0.0.1:{udp_port}\" as u\n        u.send text enc\n    end\n    return 1\n"
        ),
    );
    src += &op("s.nested", "    return (request \"s.echo\" {v: K})\n");
    src += "operation s.echo\n    param v text required\n    output json\n    return v\nend\n\n";
    src += &op("s.return_b64", "    return {v: (base64.encode K)}\n");
    src += &format!(
        "operation s.tcp_bound\n    output json\n    secret T from env \"{SECRET_VAR}\" for \"tcp://127.0.0.1:{tcp_port}\"\n    with tcp \"127.0.0.1:{tcp_port}\" as c\n        framing newline\n        c.send text \"t=${{T}}\"\n    end\n    return 1\nend\n"
    );
    let rt = support::runtime(&src, &p(tmp.path()), &policy);

    // Bound origin: the (encoded) secret travels.
    let ok = rt.request("s.bound_b64", Value::Null, None).await.unwrap();
    assert_eq!(ok.result.get("name"), Some(&Value::text("Ada")));
    assert_eq!(bound.requests.lock().unwrap().len(), 1);

    let mut errors = Vec::new();
    for id in [
        "s.header_b64_other",
        "s.chain_other",
        "s.body_other",
        "s.query_other",
        "s.file",
        "s.process",
        "s.tcp",
        "s.udp",
        "s.nested",
        "s.return_b64",
    ] {
        let e = rt.request(id, Value::Null, None).await.unwrap_err();
        assert_eq!(e.code, "permission.denied", "{id}: {e:?}");
        assert!(e.message.contains("secret `K`"), "{id}: {}", e.message);
        errors.push(e);
    }
    assert!(
        other.requests.lock().unwrap().is_empty(),
        "nothing reached the unbound origin"
    );
    assert!(!tmp.path().join("out/k.txt").exists(), "file never written");

    // A secret bound to the TCP origin may be sent there.
    rt.request("s.tcp_bound", Value::Null, None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let got = seen.lock().unwrap().clone();
    let text = String::from_utf8_lossy(&got).to_string();
    assert!(text.contains(&format!("t={CANARY}")), "{text}");
    assert!(!text.contains(&format!("k={CANARY}")), "{text}");
    let b64 = base64::engine::general_purpose::STANDARD.encode(CANARY);
    assert!(!text.contains(&b64), "udp leak: {text}");

    for e in errors {
        let text = format!("{} {e:?}", e.to_value().to_json());
        assert!(!text.contains(CANARY) && !text.contains(&b64), "{text}");
        if let Some(req) = &e.request_id
            && let Ok(t) = rt.trace(req)
        {
            let t = t.to_json().to_string();
            assert!(!t.contains(CANARY), "trace leaked the secret: {t}");
        }
    }
}
