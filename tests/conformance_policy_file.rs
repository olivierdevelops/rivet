//! T-21 — policy.json is the only policy source (PROP-2026-0001 Increment 16,
//! REF-2026-0002 S66–S69, S129, S130, S146, S147).
//!
//! ```text
//!  entry dir/app.rivet ──▶ entry dir/policy.json (discovered)      ─┐
//!  --policy PATH       ──▶ exactly that file (never grant text)     ├─▶ strict schema v1 ──error──▶ policy.invalid, exit 2
//!  neither             ──▶ deny-by-default (pure ops still run)    ─┘            │
//!                                                                               ▼
//!        deny entries ▶ private ranges (unless named literally) ▶ grant(capability, selector, access)
//! ```

#[path = "support/mod.rs"]
mod serve_support;
#[path = "transport_support/mod.rs"]
mod transport;

use rivet::Runtime;
use rivet::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use rivet::domain::{ErrorKind, Value};
use rivet::orchestrator::runtime::policy_from_json;
use rivet::orchestrator::setup_serve::{ServeOptions, start};
use serde_json::{Value as Json, json};
use std::path::Path;
use std::process::Command;

const RIVET: &str = env!("CARGO_BIN_EXE_rivet");

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Out {
    /// The ErrorEnvelope `error` object printed on stderr under `--json`.
    fn error(&self) -> Json {
        let line = self.stderr.lines().rev().find(|l| l.starts_with('{'));
        let env: Json = serde_json::from_str(line.unwrap_or_else(|| panic!("{}", self.stderr)))
            .unwrap_or_else(|e| panic!("{e}: {}", self.stderr));
        env["error"].clone()
    }
}

fn rivet(cwd: &Path, args: &[&str]) -> Out {
    let out = Command::new(RIVET)
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run rivet");
    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

const CONFIG_APP: &str = "operation config.read
    description \"Read the service configuration file.\"
    output json
    config = file read \"./fixtures/config.json\" as json
    return config
end

operation fixture.echo
    param value text required
    output text
    return value
end
";

// ---------------------------------------------------------------- discovery

// vhco:test policy.load_policy -- S129 policy.json is discovered beside the entry file (never the current directory); removing it denies the same call (exit 3) while pure operations run
#[test]
fn discovery_beside_the_entry_file() {
    let repo = tempfile::tempdir().unwrap();
    let r = repo.path();
    // A broad policy in the current directory must NOT be read.
    write(
        r,
        "policy.json",
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["*"]}]}"#,
    );
    write(r, "services/app.rivet", CONFIG_APP);
    write(r, "services/fixtures/config.json", r#"{"port":8080}"#);
    write(r, "fixtures/config.json", r#"{"port":1}"#);
    write(
        r,
        "services/policy.json",
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./fixtures/**"]}]}"#,
    );
    let ok = rivet(
        r,
        &[
            "--file",
            "services/app.rivet",
            "--json",
            "request",
            "config.read",
            "--params",
            "{}",
        ],
    );
    assert_eq!(ok.code, 0, "{}", ok.stderr);
    assert!(ok.stdout.contains("8080"), "{}", ok.stdout);

    // Library hosts discover the same file.
    let rt = Runtime::builder()
        .file(r.join("services/app.rivet").to_str().unwrap())
        .build()
        .unwrap();
    assert!(rt.policy().present);
    assert!(
        rt.policy()
            .file
            .as_deref()
            .unwrap()
            .ends_with("services/policy.json")
    );

    std::fs::rename(
        r.join("services/policy.json"),
        r.join("services/policy.off"),
    )
    .unwrap();
    let denied = rivet(
        r,
        &[
            "--file",
            "services/app.rivet",
            "--json",
            "request",
            "config.read",
            "--params",
            "{}",
        ],
    );
    assert_eq!(denied.code, 3, "{}", denied.stderr);
    assert_eq!(denied.error()["code"], "permission.denied");
    let pure = rivet(
        r,
        &[
            "--file",
            "services/app.rivet",
            "request",
            "fixture.echo",
            "--params",
            r#"{"value":"ok"}"#,
        ],
    );
    assert_eq!(pure.code, 0, "{}", pure.stderr);
    assert!(pure.stdout.contains("ok"));
    let rt = Runtime::builder()
        .file(r.join("services/app.rivet").to_str().unwrap())
        .build()
        .unwrap();
    assert!(!rt.policy().present);
}

// vhco:test policy.load_policy -- S69 --policy PATH uses only that file (targets resolve from its directory, deny wins); grant text or a missing file is policy.invalid exit 2
#[test]
fn explicit_policy_path() {
    let p = tempfile::tempdir().unwrap();
    let d = p.path();
    write(
        d,
        "app.rivet",
        "operation data.read\n    param name text required\n    output json\n    return file read \"./data/${name}\" as json\nend\n\noperation fixture.echo\n    param value text required\n    output text\n    return value\nend\n",
    );
    // The default file is malformed: it must not be read when --policy is given.
    write(d, "policy.json", "{ not json");
    write(
        d,
        "policies/ci.json",
        r#"{"version":1,
            "grants":[{"capability":"allow_read","targets":["../data/**"]},{"capability":"allow_write","targets":["../out/**"]}],
            "deny":[{"capability":"allow_read","targets":["../data/private/**"]}]}"#,
    );
    write(d, "data/public.json", r#"{"ok":1}"#);
    write(d, "data/private/secret.json", r#"{"s":1}"#);

    let read = |name: &str| {
        rivet(
            d,
            &[
                "--file",
                "app.rivet",
                "--policy",
                "./policies/ci.json",
                "--json",
                "request",
                "data.read",
                "--params",
                &format!(r#"{{"name":"{name}"}}"#),
            ],
        )
    };
    let ok = read("public.json");
    assert_eq!(ok.code, 0, "{}", ok.stderr);
    let denied = read("private/secret.json");
    assert_eq!(denied.code, 3, "deny overrides grants: {}", denied.stderr);
    assert_eq!(denied.error()["code"], "permission.denied");

    // Without --policy the malformed default is loaded and rejected.
    let bad = rivet(
        d,
        &[
            "--file",
            "app.rivet",
            "--json",
            "request",
            "fixture.echo",
            "--params",
            r#"{"value":"ok"}"#,
        ],
    );
    assert_eq!(bad.code, 2);
    assert_eq!(bad.error()["code"], "policy.invalid");

    for policy in ["allow_read=./data/**", "./policies/missing.json"] {
        let o = rivet(
            d,
            &[
                "--file",
                "app.rivet",
                "--policy",
                policy,
                "--json",
                "request",
                "fixture.echo",
                "--params",
                r#"{"value":"ok"}"#,
            ],
        );
        assert_eq!(o.code, 2, "{policy}: {}", o.stderr);
        assert_eq!(o.error()["code"], "policy.invalid");
        assert_eq!(o.error()["kind"], "validation");
        assert!(o.stdout.is_empty(), "nothing runs");
    }

    // Library: policy_file(PATH) behaves the same.
    let e = Runtime::builder()
        .file(d.join("app.rivet").to_str().unwrap())
        .policy_file(d.join("nope.json").to_str().unwrap())
        .build()
        .err()
        .unwrap();
    assert_eq!((e.code.as_str(), e.exit_code()), ("policy.invalid", 2));
}

// vhco:test policy.load_policy -- `{"version":1}` is a present, empty policy: every effect is denied, pure operations run, private ranges stay denied
#[tokio::test]
async fn version_only_policy_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    std::fs::write(tmp.path().join("in.txt"), "x").unwrap();
    let src = "operation t.read\n    output text\n    return file read \"./in.txt\" as text\nend\n\noperation t.pure\n    output integer\n    return 1 + 1\nend\n";
    let p = policy_from_json(br#"{"version":1}"#, root).unwrap();
    assert!(p.present && p.grants.is_empty() && p.deny.is_empty());
    assert!(p.network.deny_private_ranges);
    let rt = Runtime::builder()
        .source("app.rivet", src, root)
        .policy(p)
        .build()
        .unwrap();
    let e = rt.request("t.read", Value::Null, None).await.unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("permission.denied", 3));
    assert_eq!(
        rt.request("t.pure", Value::Null, None)
            .await
            .unwrap()
            .result,
        Value::Int(2)
    );
}

// ---------------------------------------------------------------- schema errors

// vhco:test policy.load_policy -- every schema error class is policy.invalid (validation, exit 2) naming the JSON pointer of the offending value
#[test]
fn schema_errors_name_their_json_pointer() {
    let g = |extra: &str| format!(r#"{{"version":1,"grants":[{extra}]}}"#);
    let cases: Vec<(String, &str)> = vec![
        ("{ not json".into(), ""),
        ("[]".into(), ""),
        ("{}".into(), "/version"),
        (r#"{"version":2}"#.into(), "/version"),
        (r#"{"version":"1"}"#.into(), "/version"),
        (r#"{"version":1,"sandbox":true}"#.into(), "/sandbox"),
        (r#"{"version":1,"grants":{}}"#.into(), "/grants"),
        (g("7"), "/grants/0"),
        (
            g(r#"{"capability":"allow_read","targets":["./x"],"mode":"rw"}"#),
            "/grants/0/mode",
        ),
        (g(r#"{"targets":["./x"]}"#), "/grants/0/capability"),
        (
            g(r#"{"capability":"allow_everything","targets":["./x"]}"#),
            "/grants/0/capability",
        ),
        (g(r#"{"capability":"allow_read"}"#), "/grants/0/targets"),
        (
            g(r#"{"capability":"allow_read","targets":[]}"#),
            "/grants/0/targets",
        ),
        (
            g(r#"{"capability":"allow_read","targets":[42]}"#),
            "/grants/0/targets/0",
        ),
        (
            g(r#"{"capability":"allow_read","targets":[""]}"#),
            "/grants/0/targets/0",
        ),
        (
            g(r#"{"capability":"allow_read","targets":["./data/[a"]}"#),
            "/grants/0/targets/0",
        ),
        (
            g(r#"{"capability":"allow_network","targets":["api.example.com"]}"#),
            "/grants/0/targets/0",
        ),
        // S147: a verb of another capability, and an unknown verb.
        (
            g(r#"{"capability":"allow_read","targets":["./data/**"],"access":["read","delete"]}"#),
            "/grants/0/access/1",
        ),
        (
            g(r#"{"capability":"allow_write","targets":["./out/**"],"access":["wirte"]}"#),
            "/grants/0/access/0",
        ),
        (
            g(r#"{"capability":"allow_write","targets":["./out/**"],"access":"create"}"#),
            "/grants/0/access",
        ),
        (
            r#"{"version":1,"deny":[{"capability":"allow_write","targets":["./out/**"],"access":["connect"]}]}"#.into(),
            "/deny/0/access/0",
        ),
        (r#"{"version":1,"network":true}"#.into(), "/network"),
        (
            r#"{"version":1,"network":{"deny_private_ranges":"yes"}}"#.into(),
            "/network/deny_private_ranges",
        ),
        (
            r#"{"version":1,"network":{"allow_private":true}}"#.into(),
            "/network/allow_private",
        ),
        (
            r#"{"version":1,"limits":{"max_call_depth":0}}"#.into(),
            "/limits/max_call_depth",
        ),
        (
            r#"{"version":1,"limits":{"max_concurrent_requests":-1}}"#.into(),
            "/limits/max_concurrent_requests",
        ),
        (r#"{"version":1,"limits":{"cpu":1}}"#.into(), "/limits/cpu"),
        (
            r#"{"version":1,"approved":{"snapshots":[1]}}"#.into(),
            "/approved/snapshots/0",
        ),
        (
            r#"{"version":1,"serve":{"surfaces":["grpc"]}}"#.into(),
            "/serve/surfaces/0",
        ),
        (r#"{"version":1,"serve":{"tls":true}}"#.into(), "/serve/tls"),
    ];
    for (text, pointer) in &cases {
        let e = policy_from_json(text.as_bytes(), ".")
            .err()
            .unwrap_or_else(|| panic!("{text} must be rejected"));
        assert_eq!(e.code, "policy.invalid", "{text}");
        assert_eq!(e.kind, ErrorKind::Validation, "{text}");
        assert_eq!(e.exit_code(), 2, "{text}");
        assert_eq!(
            e.details.get("pointer").and_then(Value::as_str),
            Some(*pointer),
            "{text}: {}",
            e.message
        );
    }

    // The CLI refuses to run anything, even a pure operation (S147), exit 2.
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "app.rivet", CONFIG_APP);
    write(
        tmp.path(),
        "policy.json",
        &g(r#"{"capability":"allow_read","targets":["./data/**"],"access":["read","delete"]}"#),
    );
    for args in [
        vec!["request", "fixture.echo", "--params", r#"{"value":"ok"}"#],
        vec!["io", "--check-policy"],
    ] {
        let mut all = vec!["--file", "app.rivet", "--json"];
        all.extend(args.iter().copied());
        let o = rivet(tmp.path(), &all);
        assert_eq!(o.code, 2, "{args:?}: {}", o.stderr);
        let err = o.error();
        assert_eq!(err["code"], "policy.invalid");
        assert_eq!(err["details"]["pointer"], "/grants/0/access/1");
        assert!(err["message"].as_str().unwrap().contains("delete"));
        assert!(!o.stdout.contains("ok"), "nothing ran: {}", o.stdout);
    }
}

// ---------------------------------------------------------------- decisions

fn decide(
    rt: &Runtime,
    capability: Capability,
    verb: AccessVerb,
    target: EffectTarget,
) -> Decision {
    rt.evaluator()
        .evaluate(&EffectIntent {
            capability,
            verb,
            target,
            operation_id: "t.run".into(),
            effect_id: None,
            span: None,
        })
        .decision
}

fn rt_with(root: &str, policy: &str) -> Runtime {
    Runtime::builder()
        .source(
            "app.rivet",
            "operation t.run\n    output integer\n    return 1\nend\n",
            root,
        )
        .policy(policy_from_json(policy.as_bytes(), root).unwrap())
        .build()
        .unwrap()
}

// vhco:test policy.authorize_effect -- deny entries override grants, including access-narrowed deny entries (deny append on the audit log, update still allowed)
#[tokio::test]
async fn deny_overrides_grants() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    let rt = rt_with(
        root,
        r#"{"version":1,
            "grants":[{"capability":"allow_read","targets":["./data/**"]},
                      {"capability":"allow_write","targets":["./out/**"]},
                      {"capability":"allow_network","targets":["https://api.example.com:443","https://internal.example.com:443"]}],
            "deny":[{"capability":"allow_read","targets":["./data/private/**"]},
                    {"capability":"allow_write","targets":["./out/audit.log"],"access":["append"]},
                    {"capability":"allow_network","targets":["https://internal.example.com:443"]}]}"#,
    );
    use AccessVerb as V;
    use Capability as C;
    use Decision::{Allowed, Denied};
    let p = |s: &str| EffectTarget::Path(s.into());
    let u = |s: &str| EffectTarget::Url(s.into());
    let cases = [
        (C::Read, V::Read, p("./data/public.json"), Allowed),
        (C::Read, V::Read, p("./data/private/secret.json"), Denied),
        (
            C::Read,
            V::Read,
            p("./data/x/../private/secret.json"),
            Denied,
        ),
        (C::Read, V::List, p("./data/private"), Denied),
        (C::Write, V::Append, p("./out/audit.log"), Denied),
        (C::Write, V::Update, p("./out/audit.log"), Allowed),
        (C::Write, V::Append, p("./out/other.log"), Allowed),
        (
            C::Network,
            V::Connect,
            u("https://api.example.com/logs"),
            Allowed,
        ),
        (
            C::Network,
            V::Connect,
            u("https://internal.example.com/admin"),
            Denied,
        ),
        (
            C::Network,
            V::Connect,
            u("https://INTERNAL.example.com:443/x"),
            Denied,
        ),
    ];
    for (c, v, t, want) in cases {
        let label = format!("{c:?} {v:?} {t:?}");
        assert_eq!(decide(&rt, c, v, t), want, "{label}");
    }
}

// vhco:test policy.authorize_effect -- access verbs narrow a grant (stat-only read denies read/list); absent access allows every verb of the capability only
#[tokio::test]
async fn access_verbs_narrow_grants() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    use AccessVerb as V;
    use Capability as C;
    let p = |s: &str| EffectTarget::Path(s.into());
    let narrowed = rt_with(
        root,
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"],"access":["stat"]}]}"#,
    );
    assert_eq!(
        decide(&narrowed, C::Read, V::Stat, p("./data/a")),
        Decision::Allowed
    );
    assert_eq!(
        decide(&narrowed, C::Read, V::Read, p("./data/a")),
        Decision::Denied
    );
    assert_eq!(
        decide(&narrowed, C::Read, V::List, p("./data")),
        Decision::Denied
    );
    let full = rt_with(
        root,
        r#"{"version":1,"grants":[{"capability":"allow_write","targets":["./out/**"]}]}"#,
    );
    for v in [V::Create, V::Update, V::Append] {
        assert_eq!(
            decide(&full, C::Write, v, p("./out/a")),
            Decision::Allowed,
            "{v:?}"
        );
    }
    // allow_write never implies delete or read.
    assert_eq!(
        decide(&full, C::Delete, V::Delete, p("./out/a")),
        Decision::Denied
    );
    assert_eq!(
        decide(&full, C::Read, V::Read, p("./out/a")),
        Decision::Denied
    );
}

// vhco:test policy.authorize_effect -- private ranges (RFC1918, 127/8, ::1, 169.254.169.254, fc00::/7, fe80::/10, IPv4-mapped IPv6, localhost) are denied under "*" and allowed only when named literally
#[tokio::test]
async fn private_ranges_denied_unless_named_literally() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    let star = rt_with(
        root,
        r#"{"version":1,"grants":[{"capability":"allow_network","targets":["*"]}]}"#,
    );
    let private = [
        ("http://10.0.0.7/", "10.0.0.0/8"),
        ("http://172.16.4.2:8080/", "172.16.4.2"),
        ("http://192.168.1.1/", "http://192.168.1.1:80"),
        ("http://127.0.0.1:9/", "127.0.0.1"),
        ("http://127.8.9.10/", "127.0.0.0/8"),
        ("http://[::1]:8080/", "::1"),
        (
            "http://169.254.169.254/latest/meta-data/",
            "169.254.169.254",
        ),
        ("http://[fc00::1]/", "fc00::/7"),
        ("http://[fd12:3456::1]/", "fc00::/7"),
        ("http://[fe80::1]/", "fe80::/10"),
        ("http://[::ffff:127.0.0.1]/", "::ffff:127.0.0.1"),
        ("http://[::ffff:10.1.2.3]/", "::ffff:10.1.2.3"),
        ("http://localhost:8080/", "127.0.0.1"),
        ("udp://192.168.1.20:5000", "192.168.1.20"),
    ];
    let net = |u: &str| EffectTarget::Url(u.to_string());
    for (url, _) in private {
        assert_eq!(
            decide(&star, Capability::Network, AccessVerb::Connect, net(url)),
            Decision::Denied,
            "{url} under \"*\""
        );
    }
    // Public addresses are allowed under "*".
    for url in [
        "http://93.184.216.34/",
        "https://api.example.com/x",
        "http://[2606:4700::1]/",
    ] {
        assert_eq!(
            decide(&star, Capability::Network, AccessVerb::Connect, net(url)),
            Decision::Allowed,
            "{url}"
        );
    }
    for (url, literal) in private {
        let named = rt_with(
            root,
            &format!(
                r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["*","{literal}"]}}]}}"#
            ),
        );
        assert_eq!(
            decide(&named, Capability::Network, AccessVerb::Connect, net(url)),
            Decision::Allowed,
            "{url} with literal {literal}"
        );
    }
    // Naming one private address does not open its neighbours.
    let one = rt_with(
        root,
        r#"{"version":1,"grants":[{"capability":"allow_network","targets":["*","10.0.0.7"]}]}"#,
    );
    assert_eq!(
        decide(
            &one,
            Capability::Network,
            AccessVerb::Connect,
            net("http://10.0.0.8/")
        ),
        Decision::Denied
    );
    // deny_private_ranges false restores plain "*" semantics.
    let off = rt_with(
        root,
        r#"{"version":1,"grants":[{"capability":"allow_network","targets":["*"]}],"network":{"deny_private_ranges":false}}"#,
    );
    assert_eq!(
        decide(
            &off,
            Capability::Network,
            AccessVerb::Connect,
            net("http://10.0.0.7/")
        ),
        Decision::Allowed
    );
}

// vhco:test policy.authorize_effect -- S130 end to end: a loopback fixture is unreachable under "*" (zero requests reach it) and reachable when its origin is granted literally
#[tokio::test]
async fn private_range_end_to_end() {
    let (port, stats) = transport::http_server().await;
    let src = format!(
        "operation t.run\n    output json\n    r = http get \"http://127.0.0.1:{port}/users/42\"\n        decode json\n    end\n    return r.body\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    let star = transport::runtime(
        &src,
        root,
        r#"{"version":1,"grants":[{"capability":"allow_network","targets":["*"]}]}"#,
    );
    let e = star.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("permission.denied", 3));
    assert!(stats.requests.lock().unwrap().is_empty(), "no connection");
    let named = transport::runtime(
        &src,
        root,
        &format!(
            r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["http://127.0.0.1:{port}"]}}]}}"#
        ),
    );
    let v = named.request("t.run", Value::Null, None).await.unwrap();
    assert_eq!(v.result.get("name"), Some(&Value::text("Ada")));
}

const RESTRICT_APP: &str = "operation t.a\n    output text\n    return file read \"./data/a.txt\" as text\nend\n\noperation t.b\n    output text\n    return file read \"./data/b.txt\" as text\nend\n\noperation t.nested_b\n    output text\n    return (request \"t.b\" {})\nend\n\noperation t.secret\n    output text\n    return file read \"./secret.txt\" as text\nend\n";

fn restrict_rt(tmp: &tempfile::TempDir) -> Runtime {
    let root = tmp.path().to_str().unwrap();
    std::fs::create_dir_all(tmp.path().join("data")).unwrap();
    std::fs::write(tmp.path().join("data/a.txt"), "A").unwrap();
    std::fs::write(tmp.path().join("data/b.txt"), "B").unwrap();
    std::fs::write(tmp.path().join("secret.txt"), "S").unwrap();
    Runtime::builder()
        .source("app.rivet", RESTRICT_APP, root)
        .policy(
            policy_from_json(
                br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"]}]}"#,
                root,
            )
            .unwrap(),
        )
        .build()
        .unwrap()
}

// vhco:test policy.authorize_effect -- G31 library request option: `restrict {grants}` narrows one request (and its nested calls) to the intersection with policy.json, a restriction naming ungranted targets never widens, other requests are unaffected, and a malformed restriction is policy.invalid with a /restrict pointer
#[tokio::test]
async fn request_restriction_narrows_never_widens_library() {
    let tmp = tempfile::tempdir().unwrap();
    let rt = restrict_rt(&tmp);
    let only_a = json!({"grants": [{"capability": "allow_read", "targets": ["./data/a.txt"]}]});
    let only_a = Value::from_json(&only_a);
    let c = rt
        .request_restricted("t.a", Value::Null, only_a.clone(), None)
        .await
        .unwrap();
    assert_eq!(c.result, Value::text("A"));
    for id in ["t.b", "t.nested_b"] {
        let e = rt
            .request_restricted(id, Value::Null, only_a.clone(), None)
            .await
            .unwrap_err();
        assert_eq!(e.code, "permission.denied", "{id}: {e:?}");
        assert!(e.message.contains("request restriction"), "{}", e.message);
    }
    // Unrestricted requests keep the full policy.
    assert_eq!(
        rt.request("t.b", Value::Null, None).await.unwrap().result,
        Value::text("B")
    );
    // A restriction that "grants" more than policy.json does not widen.
    let wide =
        Value::from_json(&json!({"grants": [{"capability": "allow_read", "targets": ["*"]}]}));
    let e = rt
        .request_restricted("t.secret", Value::Null, wide.clone(), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    // An empty restriction denies every effect.
    let none = Value::from_json(&json!({"grants": []}));
    assert_eq!(
        rt.request_restricted("t.a", Value::Null, none, None)
            .await
            .unwrap_err()
            .code,
        "permission.denied"
    );
    // Malformed restrictions are rejected before anything runs.
    for (bad, pointer) in [
        (json!({"grants": [], "deny": []}), "/restrict/deny"),
        (
            json!({"grants": [{"capability": "allow_everything", "targets": ["*"]}]}),
            "/restrict/grants/0/capability",
        ),
    ] {
        let e = rt
            .request_restricted("t.a", Value::Null, Value::from_json(&bad), None)
            .await
            .unwrap_err();
        assert_eq!(e.code, "policy.invalid", "{bad}");
        assert_eq!(
            e.details.get("pointer").and_then(|v| v.as_str()),
            Some(pointer),
            "{bad}"
        );
    }
}

// vhco:test policy.authorize_effect -- G31 surfaces: `restrict` on POST /v1/request, MCP tools/call and a WebSocket request frame narrows that call only (403 / isError / error frame), and a widening restriction stays denied
#[tokio::test]
async fn request_restriction_on_http_mcp_ws() {
    let tmp = tempfile::tempdir().unwrap();
    let rt = restrict_rt(&tmp);
    let handle = start(
        rt.clone(),
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = handle.addr.unwrap();
    let only_a = json!({"grants": [{"capability": "allow_read", "targets": ["./data/a.txt"]}]});
    let wide = json!({"grants": [{"capability": "allow_read", "targets": ["*"]}]});

    // HTTP
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id": "t.a", "params": {}, "restrict": only_a}),
        &[],
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id": "t.b", "params": {}, "restrict": only_a}),
        &[],
    )
    .await;
    assert_eq!(r.status, 403, "{}", r.text);
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id": "t.secret", "params": {}, "restrict": wide}),
        &[],
    )
    .await;
    assert_eq!(r.status, 403, "{}", r.text);
    let r = serve_support::post(addr, "/v1/request", json!({"id": "t.b", "params": {}}), &[]).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id": "t.a", "params": {}, "restrict": {"grants": "x"}}),
        &[],
    )
    .await;
    assert_eq!(r.json()["error"]["code"], "policy.invalid", "{}", r.text);

    // MCP tools/call
    let sid = serve_support::mcp_init(addr, &[]).await;
    let call = |name: &str, restrict: &Json| {
        json!({"jsonrpc":"2.0","id":7,"method":"tools/call",
               "params":{"name": name, "arguments": {}, "restrict": restrict}})
    };
    let ok = serve_support::mcp_raw(addr, &sid, &[], call("t.a", &only_a))
        .await
        .json();
    assert_eq!(ok["result"]["isError"], json!(false), "{ok}");
    let denied = serve_support::mcp_raw(addr, &sid, &[], call("t.b", &only_a))
        .await
        .json();
    assert_eq!(denied["result"]["isError"], json!(true), "{denied}");
    assert!(denied.to_string().contains("permission.denied"), "{denied}");
    let widened = serve_support::mcp_raw(addr, &sid, &[], call("t.secret", &wide))
        .await
        .json();
    assert_eq!(widened["result"]["isError"], json!(true), "{widened}");

    // WebSocket request frames
    let mut ws = serve_support::ws_connect(addr, &[]).await.unwrap();
    serve_support::ws_send(
        &mut ws,
        json!({"type": "request", "ref": "r1", "id": "t.a", "params": {}, "restrict": only_a}),
    )
    .await;
    serve_support::ws_send(
        &mut ws,
        json!({"type": "request", "ref": "r2", "id": "t.b", "params": {}, "restrict": only_a}),
    )
    .await;
    let frames = serve_support::ws_until_terminal(&mut ws, &["r1", "r2"]).await;
    let terminal = |r: &str| {
        frames
            .iter()
            .find(|f| f["ref"] == r && (f["type"] == "result" || f["type"] == "error"))
            .cloned()
            .unwrap()
    };
    assert_eq!(terminal("r1")["type"], "result", "{frames:?}");
    let r2 = terminal("r2");
    assert_eq!(r2["type"], "error", "{frames:?}");
    assert!(r2.to_string().contains("permission.denied"), "{r2}");
}

// vhco:test policy.authorize_effect -- per-request data cannot widen authority: policy/grant fields in /v1/request bodies and MCP arguments are ignored and the call stays denied
#[tokio::test]
async fn per_request_restriction_cannot_widen() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    std::fs::write(tmp.path().join("secret.txt"), "s").unwrap();
    let src =
        "operation t.read\n    output text\n    return file read \"./secret.txt\" as text\nend\n";
    let rt = Runtime::builder()
        .source("app.rivet", src, root)
        .policy(policy_from_json(br#"{"version":1}"#, root).unwrap())
        .build()
        .unwrap();
    let handle = start(
        rt.clone(),
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = handle.addr.unwrap();
    let widen = json!({"version": 1, "grants": [{"capability": "allow_read", "targets": ["*"]}]});
    let r = serve_support::post(
        addr,
        "/v1/request",
        json!({"id": "t.read", "params": {}, "policy": widen, "grants": widen["grants"], "sandbox": "*"}),
        &[],
    )
    .await;
    assert_eq!(r.status, 403, "{}", r.text);
    assert_eq!(r.json()["error"]["code"], "permission.denied");
    let sid = serve_support::mcp_init(addr, &[]).await;
    let res = serve_support::mcp_raw(
        addr,
        &sid,
        &[],
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
               "params":{"name":"t.read","arguments":{},"_meta":{"policy": widen}}}),
    )
    .await;
    let j = res.json();
    let text = j.to_string();
    assert!(!text.contains("\"s\""), "{text}");
    assert!(text.contains("permission.denied"), "{text}");
    assert_eq!(rt.policy().grants.len(), 0);
}

// vhco:test policy.load_policy -- S67 `policy explain` flags every "*" grant as broad (text and --json); narrow grants are not flagged
#[test]
fn policy_explain_flags_star() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "app.rivet", CONFIG_APP);
    write(
        tmp.path(),
        "policy.json",
        r#"{"version":1,"grants":[
            {"capability":"allow_read","targets":["*"]},
            {"capability":"allow_write","targets":["*"]},
            {"capability":"allow_network","targets":["*"]},
            {"capability":"allow_env","targets":["HOME"]}]}"#,
    );
    let o = rivet(tmp.path(), &["--file", "app.rivet", "policy", "explain"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let broad: Vec<&str> = o.stdout.lines().filter(|l| l.contains("broad")).collect();
    assert_eq!(broad.len(), 3, "{}", o.stdout);
    assert!(
        !o.stdout
            .lines()
            .any(|l| l.contains("HOME") && l.contains("broad"))
    );
    let o = rivet(
        tmp.path(),
        &[
            "--file",
            "app.rivet",
            "--json",
            "policy",
            "explain",
            "config.read",
        ],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let j: Json = serde_json::from_str(o.stdout.trim()).unwrap();
    let caps: Vec<&str> = j["broad"]
        .as_array()
        .unwrap_or_else(|| panic!("no `broad` in {j}"))
        .iter()
        .map(|b| b["capability"].as_str().unwrap())
        .collect();
    assert_eq!(caps, vec!["allow_read", "allow_write", "allow_network"]);
}

// vhco:test audit.inspect_effects -- G9 `policy explain ID --params JSON` fills param-dependent targets with that call's params, prints per-site decisions and exits 3 when any concrete target would be denied (0 when all are allowed; no --params keeps exit 0)
#[test]
fn policy_explain_params_evaluates_concrete_targets() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        tmp.path(),
        "app.rivet",
        "operation users.get\n    param name text required\n    param id integer required\n    output json\n    local = file read \"./data/${name}.json\" as json\n    r = http get \"https://api.example.com/users/${id}\"\n        decode json\n    end\n    return r.body\nend\n",
    );
    write(
        tmp.path(),
        "policy.json",
        r#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/ada.json"]},{"capability":"allow_network","targets":["https://api.example.com:443/users/42"]}]}"#,
    );
    let explain = |params: Option<&str>| {
        let mut args = vec![
            "--file",
            "app.rivet",
            "--json",
            "policy",
            "explain",
            "users.get",
        ];
        if let Some(p) = params {
            args.extend(["--params", p]);
        }
        rivet(tmp.path(), &args)
    };
    let decisions = |o: &Out| -> Vec<(String, String)> {
        let j: Json = serde_json::from_str(o.stdout.trim()).unwrap();
        j["sites"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["target"].to_string(),
                    s["decision"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect()
    };
    let ok = explain(Some(r#"{"name":"ada","id":42}"#));
    assert_eq!(ok.code, 0, "{}\n{}", ok.stdout, ok.stderr);
    let d = decisions(&ok);
    assert!(d.iter().all(|(_, x)| x == "allowed"), "{d:?}");
    assert!(
        d.iter().any(|(t, _)| t.contains("./data/ada.json")),
        "{d:?}"
    );
    assert!(d.iter().any(|(t, _)| t.contains("/users/42")), "{d:?}");

    let bad = explain(Some(r#"{"name":"bob","id":42}"#));
    assert_eq!(bad.code, 3, "{}\n{}", bad.stdout, bad.stderr);
    let d = decisions(&bad);
    assert!(
        d.iter()
            .any(|(t, x)| t.contains("./data/bob.json") && x == "denied"),
        "{d:?}"
    );
    assert!(bad.stderr.contains("./data/bob.json"), "{}", bad.stderr);

    // Another id is a different concrete URL: denied.
    let steer = explain(Some(r#"{"name":"ada","id":7}"#));
    assert_eq!(steer.code, 3, "{}", steer.stdout);

    // Without --params the generic view is printed and the exit stays 0.
    assert_eq!(explain(None).code, 0);
}
