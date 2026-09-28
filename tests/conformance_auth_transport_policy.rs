//! T-15 — the same registered fixture operations (pure, HTTP, UDP, OAuth
//! attached HTTP) give identical values and error codes over every access
//! point — the CLI binary, HTTP `/v1/request`, MCP `tools/call` and the
//! library — under an absent, an empty and a restricted policy.json
//! (PROP-2026-0001 Increments 9–11, 16; REF-2026-0002 S66, S84, S140).
//!
//! ```text
//!                      ┌── rivet --file app.rivet request ID   (CLI, own process)
//!  app.rivet ──────────┼── Runtime::builder().file(..).request (library)
//!  (+ policy.json?)    ├── POST /v1/request                    (rivet serve, HTTP)
//!                      └── MCP tools/call                      (rivet serve, /mcp)
//!                                   │
//!               normalize ─▶ Ok(result JSON) | Err(error.code) ─▶ all four equal
//! ```

#[path = "oauth_support/mod.rs"]
mod oauth_support;
#[path = "support/mod.rs"]
mod serve_support;
#[path = "transport_support/mod.rs"]
mod transport;

use oauth_support::{CLIENT_SECRET, Fake};
use rivet::Runtime;
use rivet::domain::Value;
use rivet::orchestrator::setup_serve::{ServeOptions, start};
use serde_json::{Value as Json, json};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Once;

const RIVET: &str = env!("CARGO_BIN_EXE_rivet");
const SECRET_VAR: &str = "RIVET_T15_CLIENT_SECRET";

type Outcome = Result<Json, String>;

fn set_env() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: set once, before any runtime of this test binary reads the environment.
        unsafe { std::env::set_var(SECRET_VAR, CLIENT_SECRET) };
    });
}

/// A UDP peer answering every datagram with `{"state":"ready"}`.
async fn udp_peer() -> SocketAddr {
    let s = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = s.local_addr().unwrap();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        while let Ok((_, from)) = s.recv_from(&mut buf).await {
            let _ = s.send_to(br#"{"state":"ready"}"#, from).await;
        }
    });
    addr
}

struct Fixtures {
    http: String,
    udp: SocketAddr,
    oauth: Fake,
}

fn app(f: &Fixtures) -> String {
    let a = f.oauth.auth_origin();
    let r = f.oauth.api_origin();
    let o = f.oauth.other_origin();
    let http = &f.http;
    let udp = f.udp;
    format!(
        r#"auth crm_service oauth2
    flow client_credentials
    issuer "{a}"
    token_url "{a}/token"
    client_id "rivet-service"
    client_secret env "{SECRET_VAR}"
    client_auth basic
    scopes ["contacts.read"]
    resource_origins ["{r}"]
    store memory
end

operation fx.echo
    param value text required
    output text
    return value
end

operation fx.http
    output json
    response = http get "{http}/users/42"
        decode json
    end
    return response.body
end

operation fx.udp
    output json
    with udp "{udp}" as socket
        socket.send json {{command: "status"}}
        return socket.receive json timeout "2s"
    end
end

operation fx.oauth
    output json
    response = http get "{r}/contacts"
        auth crm_service account "service"
        decode json
    end
    return response.body
end

operation fx.oauth_other
    output json
    response = http get "{o}/echo"
        auth crm_service account "service"
        decode json
    end
    return response.body
end
"#
    )
}

/// Grants the HTTP fixture, the OAuth profile (token endpoint + resource
/// origin) and network access to the "other" origin, but not the UDP peer: the
/// bearer must still never reach the other origin (resource_origins binding).
fn restricted(f: &Fixtures) -> String {
    format!(
        r#"{{"version": 1, "grants": [
            {{"capability": "allow_auth", "targets": ["crm_service/service/use"]}},
            {{"capability": "allow_credentials", "targets": ["crm_service/service"]}},
            {{"capability": "allow_env", "targets": ["{SECRET_VAR}"]}},
            {{"capability": "allow_network", "targets": ["{}", "{}", "{}", "{}"]}}
        ]}}"#,
        f.http,
        f.oauth.auth_origin(),
        f.oauth.api_origin(),
        f.oauth.other_origin()
    )
}

const CALLS: [(&str, &str); 5] = [
    ("fx.echo", r#"{"value":"ok"}"#),
    ("fx.http", "{}"),
    ("fx.udp", "{}"),
    ("fx.oauth", "{}"),
    ("fx.oauth_other", "{}"),
];

fn cli(dir: &Path, id: &str, params: &str) -> Outcome {
    let out = std::process::Command::new(RIVET)
        .current_dir(dir)
        .env(SECRET_VAR, CLIENT_SECRET)
        .args([
            "--file",
            "app.rivet",
            "--json",
            "request",
            id,
            "--params",
            params,
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stdout.contains("CANARY") && !stderr.contains("CANARY"));
    if out.status.success() {
        let j: Json =
            serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
        Ok(
            if j.get("request_id").is_some() && j.get("result").is_some() {
                j["result"].clone()
            } else {
                j
            },
        )
    } else {
        let line = stderr
            .lines()
            .rev()
            .find(|l| l.starts_with('{'))
            .unwrap_or_else(|| panic!("{stderr}"));
        let env: Json = serde_json::from_str(line).unwrap();
        let code = env["error"]["code"].as_str().unwrap().to_string();
        // The exit code is the registry's for that error kind.
        assert!(out.status.code().unwrap() > 0);
        Err(code)
    }
}

async fn library(rt: &Runtime, id: &str, params: &str) -> Outcome {
    let p: Json = serde_json::from_str(params).unwrap();
    match rt.request(id, Value::from_json(&p), None).await {
        Ok(c) => {
            assert!(!c.to_json().to_string().contains("CANARY"));
            Ok(c.result.to_json())
        }
        Err(e) => {
            assert!(!format!("{e:?}").contains("CANARY"));
            Err(e.code)
        }
    }
}

async fn http(addr: SocketAddr, id: &str, params: &str) -> Outcome {
    let p: Json = serde_json::from_str(params).unwrap();
    let r = serve_support::post(addr, "/v1/request", json!({"id": id, "params": p}), &[]).await;
    assert!(!r.text.contains("CANARY"));
    let j = r.json();
    if r.status == 200 {
        Ok(j["result"].clone())
    } else {
        Err(j["error"]["code"]
            .as_str()
            .unwrap_or_else(|| panic!("{j}"))
            .to_string())
    }
}

async fn mcp(addr: SocketAddr, sid: &str, id: &str, params: &str) -> Outcome {
    let p: Json = serde_json::from_str(params).unwrap();
    let res = serve_support::mcp_call(addr, sid, &[], id, p).await;
    assert!(!res.to_string().contains("CANARY"));
    if res["isError"] == true {
        let s = &res["structuredContent"];
        let code = s["error"]["code"]
            .as_str()
            .or_else(|| s["code"].as_str())
            .unwrap_or_else(|| panic!("{res}"));
        Err(code.to_string())
    } else {
        Ok(res["structuredContent"]["result"].clone())
    }
}

/// Run every call over every surface for one policy; returns the agreed outcomes.
async fn all_surfaces(f: &Fixtures, policy: Option<&str>) -> Vec<(String, Outcome)> {
    set_env();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.rivet"), app(f)).unwrap();
    if let Some(p) = policy {
        std::fs::write(dir.path().join("policy.json"), p).unwrap();
    }
    let entry = dir.path().join("app.rivet");
    let rt = Runtime::builder()
        .file(entry.to_str().unwrap())
        .build()
        .unwrap();
    assert_eq!(rt.policy().present, policy.is_some());
    let served = Runtime::builder()
        .file(entry.to_str().unwrap())
        .build()
        .unwrap();
    let handle = start(
        served,
        ServeOptions {
            listen: Some("127.0.0.1:0".into()),
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    let addr = handle.addr.unwrap();
    let sid = serve_support::mcp_init(addr, &[]).await;
    let mut agreed = Vec::new();
    for (id, params) in CALLS {
        let lib = library(&rt, id, params).await;
        let over_http = http(addr, id, params).await;
        let over_mcp = mcp(addr, &sid, id, params).await;
        let over_cli = cli(dir.path(), id, params);
        assert_eq!(lib, over_http, "{id}: library vs HTTP ({policy:?})");
        assert_eq!(lib, over_mcp, "{id}: library vs MCP ({policy:?})");
        assert_eq!(lib, over_cli, "{id}: library vs CLI ({policy:?})");
        agreed.push((id.to_string(), lib));
    }
    agreed
}

fn outcome<'a>(all: &'a [(String, Outcome)], id: &str) -> &'a Outcome {
    &all.iter().find(|(i, _)| i == id).unwrap().1
}

async fn fixtures() -> Fixtures {
    let (port, _) = transport::http_server().await;
    Fixtures {
        http: format!("http://127.0.0.1:{port}"),
        udp: udp_peer().await,
        oauth: Fake::start().await,
    }
}

// vhco:test transports.exchange_http -- absent policy.json: every surface runs the pure operation and denies HTTP, UDP and OAuth-attached calls with the same permission.denied, contacting nothing
#[tokio::test(flavor = "multi_thread")]
async fn absent_policy_same_on_every_surface() {
    let f = fixtures().await;
    let all = all_surfaces(&f, None).await;
    assert_eq!(outcome(&all, "fx.echo"), &Ok(json!("ok")));
    for id in ["fx.http", "fx.udp", "fx.oauth", "fx.oauth_other"] {
        assert_eq!(outcome(&all, id), &Err("permission.denied".into()), "{id}");
    }
    assert!(
        f.oauth
            .with(|s| s.grants.is_empty() && s.resource_auth.is_empty())
    );
}

// vhco:test datagrams.exchange_datagrams -- `{"version":1}` behaves like an absent file on every surface: pure runs, every effect is permission.denied
#[tokio::test(flavor = "multi_thread")]
async fn empty_policy_same_on_every_surface() {
    let f = fixtures().await;
    let all = all_surfaces(&f, Some(r#"{"version":1}"#)).await;
    assert_eq!(outcome(&all, "fx.echo"), &Ok(json!("ok")));
    for id in ["fx.http", "fx.udp", "fx.oauth", "fx.oauth_other"] {
        assert_eq!(outcome(&all, id), &Err("permission.denied".into()), "{id}");
    }
    assert!(f.oauth.with(|s| s.grants.is_empty()));
}

// vhco:test auth.acquire_credential -- restricted policy: granted HTTP and OAuth-attached calls succeed identically everywhere, the ungranted UDP peer is denied, and the bearer never goes to an origin outside resource_origins
#[tokio::test(flavor = "multi_thread")]
async fn restricted_policy_same_on_every_surface() {
    let f = fixtures().await;
    let policy = restricted(&f);
    let all = all_surfaces(&f, Some(&policy)).await;
    assert_eq!(outcome(&all, "fx.echo"), &Ok(json!("ok")));
    assert_eq!(
        outcome(&all, "fx.http"),
        &Ok(json!({"id": 42, "name": "Ada"}))
    );
    assert_eq!(outcome(&all, "fx.udp"), &Err("permission.denied".into()));
    let contacts = outcome(&all, "fx.oauth").as_ref().unwrap();
    assert!(!contacts.is_null());
    assert_eq!(
        outcome(&all, "fx.oauth_other"),
        &Err("auth.origin_not_bound".into())
    );
    // The token was only ever presented to the resource origin; the other
    // origin never received a request, with or without Authorization.
    assert!(f.oauth.with(|s| s.other_auth.is_empty()));
    assert!(f.oauth.with(|s| {
        !s.resource_auth.is_empty()
            && s.resource_auth
                .iter()
                .all(|a| a.as_deref().is_some_and(|h| h.starts_with("Bearer ")))
    }));
}

// vhco:test datagrams.exchange_datagrams -- granting the UDP peer literally makes the UDP fixture succeed with the same value on every surface
#[tokio::test(flavor = "multi_thread")]
async fn granted_udp_same_on_every_surface() {
    let f = fixtures().await;
    let policy = format!(
        r#"{{"version": 1, "grants": [{{"capability": "allow_network", "targets": ["udp://{}", "{}"]}}]}}"#,
        f.udp, f.http
    );
    let all = all_surfaces(&f, Some(&policy)).await;
    assert_eq!(outcome(&all, "fx.udp"), &Ok(json!({"state": "ready"})));
    assert_eq!(
        outcome(&all, "fx.http"),
        &Ok(json!({"id": 42, "name": "Ada"}))
    );
    // Network alone does not authorize the OAuth profile.
    assert_eq!(outcome(&all, "fx.oauth"), &Err("permission.denied".into()));
}
