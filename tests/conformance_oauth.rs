//! T-11 OAuth 2.0 conformance (PROP-2026-0001 Increment 9, REF S84–S93,
//! docs/demos/07-oauth2) against a local fake authorization server.
//!
//! ```text
//!  rivet.auth.begin ─▶ fake /authorize (simulated user) ─▶ rivet.auth.complete ─▶ connected
//!  http get … auth P account A ─▶ acquire (single flight) ─▶ fake /token ─▶ Bearer ─▶ fake /contacts
//!  every Completion, error envelope, trace and CLI output is scanned for CANARY-* secrets
//! ```

mod oauth_support;

use oauth_support::{CLIENT_SECRET, Fake};
use rivet::Runtime;
use rivet::internal::domain::contracts::Principal;
use rivet::internal::domain::{RivetError, Value};
use rivet::internal::orchestrator::runtime::policy_from_json;
use std::sync::Once;
use std::time::{Duration, Instant};

const SECRET_VAR: &str = "RIVET_T11_CLIENT_SECRET";

fn set_env() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: set once, before any runtime of this test binary reads the environment.
        unsafe {
            std::env::set_var(SECRET_VAR, CLIENT_SECRET);
            std::env::set_var("CRM_CLIENT_SECRET", CLIENT_SECRET);
        }
    });
}

fn profiles(f: &Fake, secret_var: &str) -> String {
    let a = f.auth_origin();
    let r = f.api_origin();
    format!(
        r#"auth crm_service oauth2
    flow client_credentials
    issuer "{a}"
    token_url "{a}/token"
    client_id "rivet-service"
    client_secret env "{secret_var}"
    client_auth basic
    scopes ["contacts.read"]
    resource_origins ["{r}"]
    store memory
end

auth crm_post oauth2
    flow client_credentials
    issuer "{a}"
    token_url "{a}/token"
    client_id "rivet-service"
    client_secret env "{secret_var}"
    client_auth post
    scopes ["contacts.read"]
    resource_origins ["{r}"]
    store memory
end

auth crm_user oauth2
    flow authorization_code
    pkce s256
    issuer "{a}"
    authorization_url "{a}/authorize"
    token_url "{a}/token"
    client_id "rivet-desktop"
    client_auth none
    redirect_uri "https://app.example.com/oauth/callback"
    scopes ["contacts.read"]
    resource_origins ["{r}"]
    store memory
end

auth crm_device oauth2
    flow device_code
    issuer "{a}"
    device_url "{a}/device"
    token_url "{a}/token"
    client_id "rivet-cli"
    client_auth none
    scopes ["contacts.read"]
    resource_origins ["{r}"]
    store memory
end

"#
    )
}

fn operations(f: &Fake) -> String {
    let r = f.api_origin();
    let o = f.other_origin();
    let op = |id: &str, method: &str, url: String, profile: &str, account: &str, extra: &str| {
        format!(
            "operation {id}\n    output json\n    response = http {method} \"{url}\"\n        auth {profile} account \"{account}\"\n        decode json\n{extra}    end\n    return response.body\nend\n\n"
        )
    };
    [
        op(
            "svc.contacts",
            "get",
            format!("{r}/contacts"),
            "crm_service",
            "service",
            "",
        ),
        op(
            "post.contacts",
            "get",
            format!("{r}/contacts"),
            "crm_post",
            "service",
            "",
        ),
        op(
            "user.contacts",
            "get",
            format!("{r}/contacts"),
            "crm_user",
            "ada",
            "",
        ),
        op(
            "device.contacts",
            "get",
            format!("{r}/contacts"),
            "crm_device",
            "ada",
            "",
        ),
        op(
            "svc.other",
            "get",
            format!("{o}/echo"),
            "crm_service",
            "service",
            "",
        ),
        op(
            "svc.redirect",
            "get",
            format!("{r}/redirect"),
            "crm_service",
            "service",
            "        redirect follow limit 2\n",
        ),
        op(
            "svc.reject_get",
            "get",
            format!("{r}/reject"),
            "crm_service",
            "service",
            "",
        ),
        op(
            "svc.reject_post",
            "post",
            format!("{r}/reject"),
            "crm_service",
            "service",
            "",
        ),
    ]
    .concat()
}

fn policy(f: &Fake, network: &[String]) -> String {
    let net: Vec<String> = network.iter().map(|n| format!("\"{n}\"")).collect();
    format!(
        r#"{{"version": 1, "grants": [
            {{"capability": "allow_auth", "targets": ["crm_service/service/use", "crm_post/service/use", "crm_user/ada/*", "crm_device/ada/*"]}},
            {{"capability": "allow_credentials", "targets": ["crm_service/service", "crm_post/service", "crm_user/ada", "crm_device/ada"]}},
            {{"capability": "allow_env", "targets": ["{SECRET_VAR}"]}},
            {{"capability": "allow_network", "targets": [{}]}}
        ]}}"#,
        net.join(", ")
    )
    .replace("{AUTH}", &f.auth_origin())
}

fn all_network(f: &Fake) -> Vec<String> {
    vec![f.auth_origin(), f.api_origin(), f.other_origin()]
}

fn runtime_with(src: &str, policy: &str) -> Runtime {
    set_env();
    Runtime::builder()
        .source("app.rivet", src, ".")
        .policy(policy_from_json(policy.as_bytes(), ".").expect("policy"))
        .build()
        .expect("runtime")
}

fn runtime(f: &Fake) -> Runtime {
    runtime_with(
        &format!("{}{}", profiles(f, SECRET_VAR), operations(f)),
        &policy(f, &all_network(f)),
    )
}

fn obj(j: serde_json::Value) -> Value {
    Value::from_json(&j)
}

/// No secret material in any text the caller can observe.
fn assert_clean(what: &str, text: &str) {
    assert!(!text.contains("CANARY"), "{what} leaked a secret: {text}");
}

fn err_text(e: &RivetError) -> String {
    format!("{} {:?}", e.to_value().to_json(), e)
}

async fn call(
    rt: &Runtime,
    id: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, RivetError> {
    let req = rt.new_request(id, obj(params), Principal::local());
    let request_id = req.request_id.clone();
    let out = rt.dispatch_request(req, None).await;
    if let Ok(t) = rt.trace(&request_id) {
        assert_clean("trace", &t.to_json().to_string());
    }
    match out {
        Ok(c) => {
            let j = c.envelope().to_json();
            assert_clean("completion", &j.to_string());
            Ok(j["data"].clone())
        }
        Err(e) => {
            assert_clean("error", &err_text(&e));
            Err(e)
        }
    }
}

async fn call_with_deadline(
    rt: &Runtime,
    id: &str,
    params: serde_json::Value,
    deadline_ms: u64,
) -> Result<serde_json::Value, RivetError> {
    let mut req = rt.new_request(id, obj(params), Principal::local());
    req.deadline_ms = deadline_ms;
    match rt.dispatch_request(req, None).await {
        Ok(c) => {
            let j = c.envelope().to_json();
            assert_clean("completion", &j.to_string());
            Ok(j["data"].clone())
        }
        Err(e) => {
            assert_clean("error", &err_text(&e));
            Err(e)
        }
    }
}

fn grants_of(f: &Fake, g: &str) -> usize {
    f.with(|s| s.grants.iter().filter(|x| x.as_str() == g).count())
}

async fn connect_user(f: &Fake, rt: &Runtime) -> serde_json::Value {
    let c = call(
        rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    let (code, state) = f.approve(c["authorization_url"].as_str().unwrap());
    call(
        rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": c["transaction_id"], "callback": {
            "code": code, "state": state,
            "redirect_uri": "https://app.example.com/oauth/callback",
            "issuer": f.auth_origin()}}),
    )
    .await
    .unwrap()
}

// vhco:test auth.acquire_credential -- client_credentials: first use exchanges (Basic client auth from allow_env), later uses hit the cache, the token reaches only the bound origin and never a result or trace
#[tokio::test]
async fn client_credentials_acquire_cache_and_no_leak() {
    let f = Fake::start().await;
    let rt = runtime(&f);
    let r = call(&rt, "svc.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(r["contacts"][0]["name"], "Ada");
    call(&rt, "svc.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(
        grants_of(&f, "client_credentials"),
        1,
        "second call reuses the cache"
    );
    assert!(f.with(|s| s.client_ok.iter().all(|ok| *ok)));
    // client_auth post sends the secret in the form body instead.
    call(&rt, "post.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(grants_of(&f, "client_credentials"), 2);
    assert!(f.with(|s| s.client_ok.iter().all(|ok| *ok)));
    let s = call(
        &rt,
        "rivet.auth.status",
        serde_json::json!({"profile": "crm_service", "account": "service"}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        s.code, "permission.denied",
        "status needs allow_auth …/status"
    );
}

// vhco:test auth.acquire_credential -- two concurrent first uses cause exactly one token request (single flight per credential identity)
#[tokio::test]
async fn concurrent_acquires_single_flight() {
    let f = Fake::start().await;
    f.with(|s| s.token_delay_ms = 300);
    let rt = runtime(&f);
    let (a, b) = tokio::join!(
        call(&rt, "svc.contacts", serde_json::json!({})),
        call(&rt, "svc.contacts", serde_json::json!({}))
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(grants_of(&f, "client_credentials"), 1);
}

// vhco:test auth.acquire_credential -- S93: without the token-endpoint network grant the use is denied before contacting the provider and no resource call follows; a missing secret env is auth.client_secret_missing (exit 5)
#[tokio::test]
async fn missing_grants_and_secret_fail_before_effects() {
    let f = Fake::start().await;
    let rt = runtime_with(
        &format!("{}{}", profiles(&f, SECRET_VAR), operations(&f)),
        &policy(&f, &[f.api_origin()]),
    );
    let e = call(&rt, "svc.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("permission.denied", 3));
    assert!(f.with(|s| s.grants.is_empty() && s.resource_auth.is_empty()));

    let rt = runtime_with(
        &format!(
            "{}{}",
            profiles(&f, "RIVET_T11_UNSET_SECRET"),
            operations(&f)
        ),
        &policy(&f, &all_network(&f)).replace(SECRET_VAR, "RIVET_T11_UNSET_SECRET"),
    );
    let e = call(&rt, "svc.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("auth.client_secret_missing", 5)
    );
    assert!(f.with(|s| s.grants.is_empty()));
}

// vhco:test auth.acquire_credential -- a Bearer is attached only to resource_origins: another origin is auth.origin_not_bound, and a redirect to another origin strips it
#[tokio::test]
async fn origin_binding_and_redirect_strip() {
    let f = Fake::start().await;
    let rt = runtime(&f);
    let e = call(&rt, "svc.other", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("auth.origin_not_bound", 3)
    );
    assert!(f.with(|s| s.grants.is_empty() && s.other_auth.is_empty()));
    let r = call(&rt, "svc.redirect", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(r["ok"], true);
    assert_eq!(f.with(|s| s.other_auth.clone()), vec![None]);
}

// vhco:test auth.acquire_credential -- a 401 on a mutation is not retried; a replay-safe GET invalidates the lease and retries once with a fresh token
#[tokio::test]
async fn unauthorized_mutation_is_not_retried() {
    let f = Fake::start().await;
    let rt = runtime(&f);
    let e = call(&rt, "svc.reject_post", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "http.status");
    assert_eq!(f.with(|s| s.reject_hits), 1);
    let e = call(&rt, "svc.reject_get", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "http.status");
    assert_eq!(f.with(|s| s.reject_hits), 3, "GET retried exactly once");
    // POST: token 1 (rejected, invalidated); GET: token 2 (rejected), retry with token 3.
    assert_eq!(
        grants_of(&f, "client_credentials"),
        3,
        "a rejected token is never reused"
    );
}

// vhco:test auth.complete_authorization -- authorization_code + PKCE S256: begin returns only a challenge, complete checks state/issuer and returns connected status; the account is then usable; replay fails
#[tokio::test]
async fn authorization_code_flow_end_to_end() {
    let f = Fake::start().await;
    let rt = runtime(&f);
    let e = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("auth.login_required", 4));
    let begin = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    let url = begin["authorization_url"].as_str().unwrap();
    assert!(url.starts_with(&format!("{}/authorize?", f.auth_origin())));
    assert!(begin["expires_at"].as_str().unwrap().ends_with('Z'));
    assert!(
        f.with(|s| s.grants.is_empty()),
        "code begin performs no token HTTP call"
    );
    let (code, state) = f.approve(url);
    let cb = serde_json::json!({"transaction_id": begin["transaction_id"], "callback": {
        "code": code, "state": state, "redirect_uri": "https://app.example.com/oauth/callback",
        "issuer": f.auth_origin()}});
    let s = call(&rt, "rivet.auth.complete", cb.clone()).await.unwrap();
    assert_eq!(s["state"], "connected");
    assert_eq!(s["generation"], 1);
    assert_eq!(s["scopes"], serde_json::json!(["contacts.read"]));
    let e = call(&rt, "rivet.auth.complete", cb).await.unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("auth.callback_invalid", 4)
    );
    let r = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(r["contacts"][0]["id"], 1);
    let st = call(
        &rt,
        "rivet.auth.status",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    assert_eq!(st["state"], "connected");
    // disconnect: local_only, generation advances, no more leases
    let d = call(
        &rt,
        "rivet.auth.disconnect",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    assert_eq!(
        d,
        serde_json::json!({"profile":"crm_user","account":"ada","local_only":true,"generation":2})
    );
    let st = call(
        &rt,
        "rivet.auth.status",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    assert_eq!(
        (st["state"].as_str(), st["generation"].as_u64()),
        (Some("disconnected"), Some(2))
    );
    let e = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "auth.login_required");
    let d = call(
        &rt,
        "rivet.auth.disconnect",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    assert_eq!(
        d["generation"], 3,
        "disconnect is idempotent and still advances"
    );
}

// vhco:test auth.complete_authorization -- state and RFC 9207 issuer mismatches are auth.callback_invalid before any exchange and consume the transaction; foreign principals see not_found
#[tokio::test]
async fn state_and_issuer_mismatch() {
    let f = Fake::start().await;
    let rt = runtime(&f);
    for (field, bad) in [
        ("state", "forged-state"),
        ("issuer", "https://evil.example.com"),
        ("redirect_uri", "https://evil.example.com/cb"),
    ] {
        let begin = call(
            &rt,
            "rivet.auth.begin",
            serde_json::json!({"profile": "crm_user", "account": "ada"}),
        )
        .await
        .unwrap();
        let (code, state) = f.approve(begin["authorization_url"].as_str().unwrap());
        let mut cb = serde_json::json!({"code": code, "state": state,
            "redirect_uri": "https://app.example.com/oauth/callback", "issuer": f.auth_origin()});
        cb[field] = serde_json::json!(bad);
        let e = call(
            &rt,
            "rivet.auth.complete",
            serde_json::json!({"transaction_id": begin["transaction_id"], "callback": cb}),
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "auth.callback_invalid", "{field}");
        assert_eq!(e.details.get("mismatch"), Some(&Value::text(field)));
        // The transaction is failed: even the right callback is refused now.
        cb[field] = if field == "issuer" {
            serde_json::json!(f.auth_origin())
        } else if field == "state" {
            serde_json::json!(state)
        } else {
            serde_json::json!("https://app.example.com/oauth/callback")
        };
        let e = call(
            &rt,
            "rivet.auth.complete",
            serde_json::json!({"transaction_id": begin["transaction_id"], "callback": cb}),
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "auth.callback_invalid");
    }
    assert_eq!(
        grants_of(&f, "authorization_code"),
        0,
        "nothing was exchanged"
    );

    let begin = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    let mut req = rt.new_request(
        "rivet.auth.cancel",
        obj(serde_json::json!({"transaction_id": begin["transaction_id"]})),
        Principal {
            name: "eve".into(),
            authenticated_by: "bearer".into(),
        },
    );
    req.depth = 0;
    let e = rt.dispatch_request(req, None).await.unwrap_err();
    assert_eq!(e.code, "not_found.auth_transaction");
    // missing callback on a code transaction
    let e = call(
        &rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": begin["transaction_id"]}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("validation.auth_callback", 2)
    );
}

// vhco:test auth.cancel_authorization -- cancel is idempotent, a cancelled transaction cannot complete, and a completed one reports connected
#[tokio::test]
async fn cancel_is_idempotent() {
    let f = Fake::start().await;
    let rt = runtime(&f);
    let begin = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    let id = begin["transaction_id"].clone();
    for _ in 0..2 {
        let r = call(
            &rt,
            "rivet.auth.cancel",
            serde_json::json!({"transaction_id": id}),
        )
        .await
        .unwrap();
        assert_eq!(
            r,
            serde_json::json!({"transaction_id": id, "state": "cancelled"})
        );
    }
    let (code, state) = f.approve(begin["authorization_url"].as_str().unwrap());
    let e = call(
        &rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": id, "callback": {
        "code": code, "state": state, "redirect_uri": "https://app.example.com/oauth/callback"}}),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "auth.callback_invalid");
    let e = call(
        &rt,
        "rivet.auth.cancel",
        serde_json::json!({"transaction_id": "auth_nope"}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("not_found.auth_transaction", 4)
    );
    let e = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_service", "account": "service"}),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "validation.auth_flow");
}

// vhco:test auth.complete_authorization -- device flow: pending then slow_down; the deadline returns {"state":"pending"} without consuming the transaction; the next complete keeps the increased interval and connects
#[tokio::test]
async fn device_flow_pending_slow_down_and_carry_over() {
    let f = Fake::start().await;
    f.with(|s| {
        s.device_interval = 1;
        s.device_script = ["authorization_pending", "slow_down", "ok"]
            .into_iter()
            .collect();
    });
    let rt = runtime(&f);
    let begin = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_device", "account": "ada"}),
    )
    .await
    .unwrap();
    assert_eq!(begin["user_code"], "ABCD-EFGH");
    assert_eq!(begin["interval_seconds"], 1);
    assert!(begin.get("device_code").is_none());
    let id = begin["transaction_id"].clone();
    let t0 = Instant::now();
    let r = call_with_deadline(
        &rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": id, "wait": true}),
        3_000,
    )
    .await
    .unwrap();
    assert_eq!(r["state"], "pending");
    assert_eq!(r["transaction_id"], id);
    assert!(t0.elapsed() < Duration::from_millis(3_500));
    assert_eq!(f.with(|s| s.polls.len()), 2);
    let r = call_with_deadline(
        &rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": id, "wait": true}),
        15_000,
    )
    .await
    .unwrap();
    assert_eq!(r["state"], "connected");
    let polls = f.with(|s| s.polls.clone());
    assert_eq!(polls.len(), 3);
    assert!(
        polls[2] - polls[1] >= Duration::from_millis(5_900),
        "slow_down carried over: {:?}",
        polls[2] - polls[1]
    );
    assert!(polls[1] - polls[0] >= Duration::from_millis(900));
    let r = call(&rt, "device.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(r["contacts"][0]["id"], 1);
}

// vhco:test auth.complete_authorization -- device denial is auth.access_denied and an expired challenge is auth.transaction_expired
#[tokio::test]
async fn device_denied_and_expired() {
    let f = Fake::start().await;
    f.with(|s| {
        s.device_interval = 1;
        s.device_script = ["access_denied"].into_iter().collect();
    });
    let rt = runtime(&f);
    let begin = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_device", "account": "ada"}),
    )
    .await
    .unwrap();
    let e = call(
        &rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": begin["transaction_id"], "wait": true}),
    )
    .await
    .unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("auth.access_denied", 4));
    let r = call(
        &rt,
        "rivet.auth.cancel",
        serde_json::json!({"transaction_id": begin["transaction_id"]}),
    )
    .await
    .unwrap();
    assert_eq!(r["state"], "denied");

    f.with(|s| s.device_expires_in = 1);
    let begin = call(
        &rt,
        "rivet.auth.begin",
        serde_json::json!({"profile": "crm_device", "account": "ada"}),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let e = call(
        &rt,
        "rivet.auth.complete",
        serde_json::json!({"transaction_id": begin["transaction_id"], "wait": true}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("auth.transaction_expired", 4)
    );
}

// vhco:test auth.acquire_credential -- an expired user token refreshes once for two concurrent uses (rotation race), rotation is committed, invalid_grant becomes auth.login_required
#[tokio::test]
async fn refresh_rotation_race_and_invalid_grant() {
    let f = Fake::start().await;
    f.with(|s| s.expires_in = Some(1));
    let rt = runtime(&f);
    connect_user(&f, &rt).await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    f.with(|s| s.refresh_delay_ms = 200);
    let (a, b) = tokio::join!(
        call(&rt, "user.contacts", serde_json::json!({})),
        call(&rt, "user.contacts", serde_json::json!({}))
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(
        grants_of(&f, "refresh_token"),
        1,
        "one refresh for concurrent callers"
    );
    // the rotated refresh token is the one used next time
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(grants_of(&f, "refresh_token"), 2);
    // provider revokes the grant
    f.with(|s| s.refresh.clear());
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    let e = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!((e.code.as_str(), e.exit_code()), ("auth.login_required", 4));
    let st = call(
        &rt,
        "rivet.auth.status",
        serde_json::json!({"profile": "crm_user", "account": "ada"}),
    )
    .await
    .unwrap();
    assert_eq!(st["state"], "disconnected");
    let e = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "auth.login_required");
    assert_eq!(
        grants_of(&f, "refresh_token"),
        3,
        "no refresh loop after invalid_grant"
    );
}

// vhco:test auth.acquire_credential -- G20: a user-flow access token without expiry is never reused: every use refreshes (the account stays connected), and without a refresh token it is login_required
#[tokio::test]
async fn user_token_without_expiry_is_never_reused() {
    let f = Fake::start().await;
    f.with(|s| s.omit_expiry = true);
    let rt = runtime(&f);
    let st = connect_user(&f, &rt).await;
    assert_eq!(st["state"], "connected");
    assert_eq!(grants_of(&f, "refresh_token"), 0);
    call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(grants_of(&f, "refresh_token"), 1, "first use refreshes");
    call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(
        grants_of(&f, "refresh_token"),
        2,
        "no reuse across requests"
    );
    // Provider drops the refresh grant: no silent reuse of the old access token.
    f.with(|s| s.refresh.clear());
    let e = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "auth.login_required");
}

// vhco:test auth.acquire_credential -- a refresh whose answer never arrives is auth.refresh_uncertain and the old refresh token is never replayed
#[tokio::test]
async fn uncertain_refresh_is_not_replayed() {
    let f = Fake::start().await;
    f.with(|s| s.expires_in = Some(1));
    let rt = runtime(&f);
    connect_user(&f, &rt).await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    f.with(|s| s.refresh_delay_ms = 2_000);
    let e = call_with_deadline(&rt, "user.contacts", serde_json::json!({}), 500)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("auth.refresh_uncertain", 4)
    );
    f.with(|s| s.refresh_delay_ms = 0);
    let e = call(&rt, "user.contacts", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "auth.login_required");
    assert_eq!(grants_of(&f, "refresh_token"), 1);
}

// vhco:test auth.begin_authorization -- profile rules are enforced at load: code flow without pkce s256, password flow and missing store are refused
#[tokio::test]
async fn profile_rules_at_load() {
    let base = "auth p oauth2\n    flow authorization_code\n    issuer \"https://auth.example.com\"\n    authorization_url \"https://auth.example.com/a\"\n    token_url \"https://auth.example.com/t\"\n    client_id \"c\"\n    client_auth none\n    redirect_uri \"https://app.example.com/cb\"\n    scopes [\"s\"]\n    resource_origins [\"https://api.example.com:443\"]\n    store memory\nend\n";
    let build = |src: &str| Runtime::builder().source("app.rivet", src, ".").build();
    let e = build(base).err().unwrap();
    assert_eq!(e.code, "validation.auth_profile");
    assert!(
        build(&base.replace("    store memory\n", "    pkce s256\n    store memory\n")).is_ok()
    );
    let e = build(&base.replace("flow authorization_code", "flow password"))
        .err()
        .unwrap();
    assert_eq!(e.code, "unsupported.auth_flow");
    let e = build(&base.replace("    store memory\n", "    pkce s256\n"))
        .err()
        .unwrap();
    assert_eq!(e.code, "validation.auth_profile");
    let e = build(
        &base
            .replace("https://auth.example.com/t", "http://auth.example.com/t")
            .replace("    store memory\n", "    pkce s256\n    store memory\n"),
    )
    .err()
    .unwrap();
    assert_eq!(
        e.code, "validation.auth_profile",
        "plain http token endpoints only on loopback"
    );
}

// vhco:test auth.acquire_credential -- docs/demos/07-oauth2 runs against the fake provider: contacts.list returns the payload twice with one token exchange, and removing the auth-origin grant is permission.denied
#[tokio::test]
async fn demo_07_flows() {
    set_env();
    let f = Fake::start().await;
    let src = std::fs::read_to_string("docs/demos/07-oauth2/app.rivet")
        .unwrap()
        .replace("https://auth.example.com", &f.auth_origin())
        .replace("https://api.example.com:443", &f.api_origin())
        .replace("https://api.example.com", &f.api_origin());
    let pol = std::fs::read_to_string("docs/demos/07-oauth2/policy.json")
        .unwrap()
        .replace("https://auth.example.com:443", &f.auth_origin())
        .replace("https://api.example.com:443", &f.api_origin());
    let rt = runtime_with(&src, &pol);
    for _ in 0..2 {
        let r = call(&rt, "contacts.list", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(r["contacts"][0]["name"], "Ada");
    }
    assert_eq!(grants_of(&f, "client_credentials"), 1);
    let outputs = rt.outputs(Some("contacts.list"), false).unwrap();
    assert_eq!(
        outputs[0].to_json()["output"]["required"],
        serde_json::json!(["contacts"])
    );
    let denied = pol.replace(&format!("\"{}\", ", f.auth_origin()), "");
    let rt = runtime_with(&src, &denied);
    let e = call(&rt, "contacts.list", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.exit_code(), e.http_status()),
        ("permission.denied", 3, 403)
    );
    assert_eq!(grants_of(&f, "client_credentials"), 1);
}

// vhco:test auth.begin_authorization -- the CLI `rivet auth begin|status|cancel|complete` maps to rivet.auth.* and prints no secrets
#[tokio::test]
async fn cli_auth_commands() {
    set_env();
    let f = Fake::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        format!("{}{}", profiles(&f, SECRET_VAR), operations(&f)),
    )
    .unwrap();
    std::fs::write(dir.path().join("policy.json"), policy(&f, &all_network(&f))).unwrap();
    let file = dir.path().join("app.rivet");
    let bin = env!("CARGO_BIN_EXE_rivet");
    let run = |args: &[&str]| {
        let out = std::process::Command::new(bin)
            .arg("--file")
            .arg(&file)
            .args(args)
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_clean("cli", &text);
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            text,
        )
    };
    let (code, out, _) = run(&["auth", "begin", "crm_user", "--account", "ada"]);
    assert_eq!(code, 0);
    let j: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(
        j["data"]["authorization_url"]
            .as_str()
            .unwrap()
            .contains("code_challenge_method=S256")
    );
    let (code, out, _) = run(&["auth", "status", "crm_user", "--account", "ada"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"disconnected\""));
    let (code, _, text) = run(&["auth", "cancel", "auth_unknown"]);
    assert_eq!(code, 4, "{text}");
    let cb = dir.path().join("callback.json");
    std::fs::write(&cb, r#"{"transaction_id":"auth_x","callback":{"code":"CANARY-CODE-X","state":"s","redirect_uri":"https://app.example.com/oauth/callback"}}"#).unwrap();
    let (code, _, text) = run(&["auth", "complete", "--params-file", cb.to_str().unwrap()]);
    assert_eq!(code, 4, "{text}");
    let (code, _, text) = run(&["auth", "complete", "--params", "{not json CANARY-CODE-Y"]);
    assert_eq!(code, 2, "{text}");
    let (code, _, _) = run(&["auth", "begin", "crm_service", "--account", "service"]);
    assert_eq!(code, 2);
}
