//! A local fake OAuth 2.0 authorization server plus resource servers for the
//! T-11 OAuth suite (and gRPC `auth` attachment). Everything binds
//! 127.0.0.1:0 inside the test; nothing leaves the host.
//!
//! ```text
//!   auth  (A) POST /token   client_credentials | authorization_code (PKCE S256 checked)
//!                           | refresh_token (rotating) | device_code (scripted answers)
//!             POST /device  device_code + user_code
//!   api   (R) GET  /contacts   needs a live Bearer CANARY-AT-n, else 401
//!             *    /reject     always 401 (counted)
//!             GET  /redirect   302 → other/echo
//!   other (O) GET  /echo       records the Authorization header it received
//! ```
//!
//! Every token, code and device code starts with `CANARY-` so the suite can
//! prove none of them reaches a result, error, trace or CLI output.

#![allow(dead_code)]

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CLIENT_ID: &str = "rivet-service";
pub const CLIENT_SECRET: &str = "CANARY-CLIENT-SECRET";

#[derive(Default)]
pub struct FakeState {
    pub expires_in: Option<u64>,
    /// Issue tokens without `expires_in` (G20: such user tokens are never reused).
    pub omit_expiry: bool,
    pub token_delay_ms: u64,
    pub refresh_delay_ms: u64,
    pub counter: u64,
    pub access: HashSet<String>,
    pub refresh: HashSet<String>,
    /// code → (code_challenge, redirect_uri)
    pub codes: HashMap<String, (String, String)>,
    pub device_script: VecDeque<&'static str>,
    pub device_expires_in: u64,
    pub device_interval: u64,
    pub device_codes: HashSet<String>,
    pub polls: Vec<Instant>,
    /// grant_type of every /token request, in order.
    pub grants: Vec<String>,
    /// Whether each confidential request authenticated correctly.
    pub client_ok: Vec<bool>,
    pub resource_auth: Vec<Option<String>>,
    pub reject_hits: usize,
    pub other_auth: Vec<Option<String>>,
}

pub struct Fake {
    pub auth: SocketAddr,
    pub api: SocketAddr,
    pub other: SocketAddr,
    pub state: Arc<Mutex<FakeState>>,
}

#[derive(Clone)]
struct Ctx {
    state: Arc<Mutex<FakeState>>,
    other: SocketAddr,
}

fn json(status: StatusCode, v: serde_json::Value) -> Response {
    (status, axum::Json(v)).into_response()
}

fn issue(st: &mut FakeState, with_refresh: bool) -> serde_json::Value {
    st.counter += 1;
    let n = st.counter;
    let at = format!("CANARY-AT-{n}");
    st.access.insert(at.clone());
    let mut j = serde_json::json!({
        "access_token": at, "token_type": "Bearer", "scope": "contacts.read",
    });
    if !st.omit_expiry
        && let Some(e) = st.expires_in.or(Some(3600))
    {
        j["expires_in"] = serde_json::json!(e);
    }
    if with_refresh {
        let rt = format!("CANARY-RT-{n}");
        st.refresh.insert(rt.clone());
        j["refresh_token"] = serde_json::json!(rt);
    }
    j
}

fn b64url_sha256(s: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(s.as_bytes()))
}

fn form(body: &Bytes) -> HashMap<String, String> {
    url::form_urlencoded::parse(body).into_owned().collect()
}

fn client_ok(headers: &HeaderMap, f: &HashMap<String, String>) -> bool {
    if let Some(h) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let Some(b) = h.strip_prefix("Basic ") else {
            return false;
        };
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b)
            .unwrap_or_default();
        return raw == format!("{CLIENT_ID}:{CLIENT_SECRET}").into_bytes();
    }
    match f.get("client_secret") {
        Some(s) => f.get("client_id").map(String::as_str) == Some(CLIENT_ID) && s == CLIENT_SECRET,
        None => f.contains_key("client_id"),
    }
}

async fn token(State(c): State<Ctx>, headers: HeaderMap, body: Bytes) -> Response {
    let f = form(&body);
    let grant = f.get("grant_type").cloned().unwrap_or_default();
    let (delay, ok) = {
        let mut st = c.state.lock().unwrap();
        st.grants.push(grant.clone());
        let ok = client_ok(&headers, &f);
        st.client_ok.push(ok);
        let delay = match grant.as_str() {
            "refresh_token" => st.refresh_delay_ms,
            _ => st.token_delay_ms,
        };
        (delay, ok)
    };
    if delay > 0 {
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    if !ok {
        return json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "invalid_client"}),
        );
    }
    let mut st = c.state.lock().unwrap();
    let bad = |e: &str| json(StatusCode::BAD_REQUEST, serde_json::json!({"error": e}));
    match grant.as_str() {
        "client_credentials" => json(StatusCode::OK, issue(&mut st, false)),
        "authorization_code" => {
            let code = f.get("code").cloned().unwrap_or_default();
            let Some((challenge, redirect)) = st.codes.remove(&code) else {
                return bad("invalid_grant");
            };
            let verifier = f.get("code_verifier").cloned().unwrap_or_default();
            if b64url_sha256(&verifier) != challenge || f.get("redirect_uri") != Some(&redirect) {
                return bad("invalid_grant");
            }
            json(StatusCode::OK, issue(&mut st, true))
        }
        "refresh_token" => {
            let rt = f.get("refresh_token").cloned().unwrap_or_default();
            if !st.refresh.remove(&rt) {
                return bad("invalid_grant");
            }
            json(StatusCode::OK, issue(&mut st, true))
        }
        "urn:ietf:params:oauth:grant-type:device_code" => {
            st.polls.push(Instant::now());
            let dc = f.get("device_code").cloned().unwrap_or_default();
            if !st.device_codes.contains(&dc) {
                return bad("invalid_grant");
            }
            match st.device_script.pop_front().unwrap_or("ok") {
                "ok" => {
                    st.device_codes.remove(&dc);
                    json(StatusCode::OK, issue(&mut st, true))
                }
                other => bad(other),
            }
        }
        _ => bad("unsupported_grant_type"),
    }
}

async fn device(State(c): State<Ctx>, headers: HeaderMap, body: Bytes) -> Response {
    let f = form(&body);
    let mut st = c.state.lock().unwrap();
    if !client_ok(&headers, &f) {
        return json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "invalid_client"}),
        );
    }
    st.counter += 1;
    let dc = format!("CANARY-DC-{}", st.counter);
    st.device_codes.insert(dc.clone());
    json(
        StatusCode::OK,
        serde_json::json!({
            "device_code": dc, "user_code": "ABCD-EFGH",
            "verification_uri": "https://auth.example.com/activate",
            "expires_in": if st.device_expires_in == 0 { 600 } else { st.device_expires_in },
            "interval": if st.device_interval == 0 { 1 } else { st.device_interval },
        }),
    )
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

async fn contacts(State(c): State<Ctx>, headers: HeaderMap) -> Response {
    let auth = bearer(&headers);
    let mut st = c.state.lock().unwrap();
    st.resource_auth.push(auth.clone());
    let ok = auth
        .and_then(|a| a.strip_prefix("Bearer ").map(str::to_string))
        .is_some_and(|t| st.access.contains(&t));
    if ok {
        json(
            StatusCode::OK,
            serde_json::json!({"contacts": [{"id": 1, "name": "Ada"}]}),
        )
    } else {
        json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "invalid_token"}),
        )
    }
}

async fn reject(State(c): State<Ctx>) -> Response {
    c.state.lock().unwrap().reject_hits += 1;
    json(
        StatusCode::UNAUTHORIZED,
        serde_json::json!({"error": "invalid_token"}),
    )
}

async fn redirect(State(c): State<Ctx>) -> Response {
    (
        StatusCode::FOUND,
        [("location", format!("http://{}/echo", c.other))],
    )
        .into_response()
}

async fn echo(State(c): State<Ctx>, headers: HeaderMap) -> Response {
    c.state.lock().unwrap().other_auth.push(bearer(&headers));
    json(StatusCode::OK, serde_json::json!({"ok": true}))
}

async fn fallback(_uri: Uri) -> Response {
    StatusCode::NOT_FOUND.into_response()
}

impl Fake {
    pub async fn start() -> Fake {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let la = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let lr = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let lo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let other = lo.local_addr().unwrap();
        let ctx = Ctx {
            state: Arc::clone(&state),
            other,
        };
        let app = Router::new()
            .route("/token", post(token))
            .route("/device", post(device))
            .route("/contacts", get(contacts))
            .route("/reject", any(reject))
            .route("/redirect", get(redirect))
            .route("/echo", get(echo))
            .fallback(fallback)
            .with_state(ctx);
        let fake = Fake {
            auth: la.local_addr().unwrap(),
            api: lr.local_addr().unwrap(),
            other,
            state,
        };
        for l in [la, lr, lo] {
            let app = app.clone();
            tokio::spawn(async move {
                let _ = axum::serve(l, app).await;
            });
        }
        fake
    }

    pub fn auth_origin(&self) -> String {
        format!("http://{}", self.auth)
    }

    pub fn api_origin(&self) -> String {
        format!("http://{}", self.api)
    }

    pub fn other_origin(&self) -> String {
        format!("http://{}", self.other)
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut FakeState) -> R) -> R {
        f(&mut self.state.lock().unwrap())
    }

    /// Simulate the user approving at the authorization endpoint: returns the
    /// (code, state) a real provider would put on the redirect.
    pub fn approve(&self, authorization_url: &str) -> (String, String) {
        let u = url::Url::parse(authorization_url).unwrap();
        let q: HashMap<String, String> = u.query_pairs().into_owned().collect();
        assert_eq!(
            q.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(q.get("response_type").map(String::as_str), Some("code"));
        let mut st = self.state.lock().unwrap();
        st.counter += 1;
        let code = format!("CANARY-CODE-{}", st.counter);
        st.codes.insert(
            code.clone(),
            (q["code_challenge"].clone(), q["redirect_uri"].clone()),
        );
        (code, q["state"].clone())
    }
}
