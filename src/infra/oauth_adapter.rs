//! OAuth 2.0 adapter (satisfies OAuthSessionDriver and CredentialProvider).
//!
//! Hand-rolled over Rivet's own broker-dialed HTTP client (ADR-0002): every
//! token/device request goes through the injected `transports.exchange_http`
//! use case, so allow_network, DNS-rebinding and redirect rules apply to the
//! authorization server exactly as to any other origin. Tokens, codes, state,
//! PKCE verifiers and device codes live only in [`SecretString`]s inside this
//! adapter and its credential store; nothing here formats them.
//!
//! ```text
//!   begin ──code──▶ state + S256 verifier ──▶ Tx{open} ──▶ {authorization_url}
//!         └device─▶ POST device_url ────────▶ Tx{open, interval} ─▶ {user_code, verification_uri}
//!
//!   complete ─code──▶ state/redirect/issuer ok? ─▶ POST token_url (code + verifier) ─▶ commit ─▶ connected
//!            └device─▶ wait interval ─▶ POST token_url ─┬ authorization_pending ─▶ wait again
//!                        (deadline first ─▶ pending)     ├ slow_down ─▶ interval += 5 s (kept on the Tx)
//!                                                        └ 200 ─▶ commit ─▶ connected
//!
//!   acquire ─▶ per-key lock (single flight) ─▶ cached & fresh? ─▶ lease
//!                                              └ no ─▶ client_credentials exchange | refresh (rotation checked)
//!
//!   store: memory (process-local) | keychain "NS" (keyring-core platform store)
//!          entry = {generation, version, token set}; writers serialized per entry
//! ```

use super::codec::StdCodec;
use super::http_adapter::{ExchangeHttpFn, HyperClient};
use crate::domain::auth::{
    AuthBeginInput, AuthCancelInput, AuthCancelReceipt, AuthChallenge, AuthCompleteInput,
    AuthContext, AuthTransactionInfo, CredentialInput, CredentialLease, CredentialState,
    CredentialStatus, CredentialStatusInput, CredentialStoreRef, DisconnectInput,
    DisconnectReceipt, OAuthFlow, OAuthProfile, SecretRef, SecretString, rfc3339,
};
use crate::domain::contracts::Principal;
use crate::domain::effect_checks::authorize;
use crate::domain::ir::CompiledProgram;
use crate::domain::policy::{AccessVerb, Capability, EffectTarget};
use crate::domain::ports::{CredentialProvider, OAuthSessionDriver, PolicyEvaluator};
use crate::domain::transports::{
    CodecInput, CodecKind, EffectOrigin, HttpExchange, HttpVersionPolicy, TlsMaterial,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Host maximum for an authorization transaction (proposal: 10 minutes).
pub const MAX_TRANSACTION_SECS: u64 = 600;
/// Outstanding (open) transactions per principal.
pub const MAX_OPEN_PER_PRINCIPAL: usize = 8;
/// RFC 8628 §3.5: `slow_down` increases the polling interval by 5 seconds.
pub const SLOW_DOWN_MS: u64 = 5_000;
/// Cap on one token-endpoint exchange (always also bounded by the request deadline).
const TOKEN_TIMEOUT: Duration = Duration::from_secs(30);
/// Statuses whose body we read for an OAuth error code.
const TOKEN_STATUSES: [u16; 9] = [200, 201, 400, 401, 403, 429, 500, 502, 503];

/// Env lookup for `client_secret env "VAR"` (called only after an allow_env permit).
pub type EnvFn = dyn Fn(&str) -> Option<String> + Send + Sync;

/// A secure credential backend (`store keychain "NS"`): one secret blob per
/// (service = namespace, user = credential identity).
pub trait SecretBackend: Send + Sync {
    fn get(&self, service: &str, user: &str) -> Result<Option<Vec<u8>>, String>;
    fn set(&self, service: &str, user: &str, secret: &[u8]) -> Result<(), String>;
}

type BackendFactory = dyn Fn() -> RivetResult<Arc<dyn SecretBackend>> + Send + Sync;

/// keyring-core store adapter (macOS Keychain, Windows Credential Manager,
/// Secret Service). Error text is reduced to the failure class: platform
/// errors can carry raw bytes.
pub struct KeyringBackend(Arc<keyring_core::CredentialStore>);

fn keyring_err(e: keyring_core::Error) -> String {
    match e {
        keyring_core::Error::NoStorageAccess(_) => {
            "the secure store is locked or access was denied".into()
        }
        keyring_core::Error::PlatformFailure(_) => {
            "the secure store reported a platform failure".into()
        }
        keyring_core::Error::TooLong(field, _) => {
            format!("the secure store refused a long {field}")
        }
        keyring_core::Error::Invalid(field, _) => format!("the secure store refused {field}"),
        _ => "the secure store failed".into(),
    }
}

impl SecretBackend for KeyringBackend {
    fn get(&self, service: &str, user: &str) -> Result<Option<Vec<u8>>, String> {
        let entry = self.0.build(service, user, None).map_err(keyring_err)?;
        match entry.get_secret() {
            Ok(b) => Ok(Some(b)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_err(e)),
        }
    }

    fn set(&self, service: &str, user: &str, secret: &[u8]) -> Result<(), String> {
        let entry = self.0.build(service, user, None).map_err(keyring_err)?;
        entry.set_secret(secret).map_err(keyring_err)
    }
}

/// The host's secure store, or `unsupported.credential_store` (never a plaintext fallback).
pub fn platform_backend() -> RivetResult<Arc<dyn SecretBackend>> {
    let unavailable = |e: keyring_core::Error| {
        RivetError::unsupported(
            "unsupported.credential_store",
            format!(
                "the platform secure store is unavailable: {}",
                keyring_err(e)
            ),
        )
    };
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let store: Arc<keyring_core::CredentialStore> =
            apple_native_keyring_store::keychain::Store::new().map_err(unavailable)?;
        return Ok(Arc::new(KeyringBackend(store)));
    }
    #[cfg(windows)]
    {
        let store: Arc<keyring_core::CredentialStore> =
            windows_native_keyring_store::Store::new().map_err(unavailable)?;
        return Ok(Arc::new(KeyringBackend(store)));
    }
    #[cfg(target_os = "linux")]
    {
        let store: Arc<keyring_core::CredentialStore> =
            zbus_secret_service_keyring_store::Store::new().map_err(unavailable)?;
        return Ok(Arc::new(KeyringBackend(store)));
    }
    #[allow(unreachable_code)]
    {
        let _ = unavailable;
        Err(RivetError::unsupported(
            "unsupported.credential_store",
            "no secure credential store backend exists for this platform; use `store memory`",
        ))
    }
}

/// One stored credential entry (serialized into the keychain; never logged).
#[derive(Clone, Default, Serialize, Deserialize)]
struct Stored {
    generation: u64,
    /// Bumped by every write; a changed version between read and commit is a concurrent rotation.
    version: u64,
    profile_hash: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    issued_at: u64,
    scopes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TxState {
    Open,
    Exchanging,
    Connected,
    Denied,
    Failed,
    Cancelled,
    Expired,
}

impl TxState {
    fn as_str(self) -> &'static str {
        match self {
            TxState::Open | TxState::Exchanging => "open",
            TxState::Connected => "connected",
            TxState::Denied => "denied",
            TxState::Failed => "failed",
            TxState::Cancelled => "cancelled",
            TxState::Expired => "expired",
        }
    }
}

struct Tx {
    principal: String,
    profile: String,
    account: String,
    state: TxState,
    /// Unix seconds.
    expires_at: u64,
    oauth_state: SecretString,
    verifier: SecretString,
    device_code: SecretString,
    interval_ms: u64,
    next_poll: Option<Instant>,
    poll: Arc<tokio::sync::Mutex<()>>,
    created: Instant,
}

impl Tx {
    fn end(&mut self, state: TxState) {
        self.state = state;
        self.oauth_state = SecretString::default();
        self.verifier = SecretString::default();
        self.device_code = SecretString::default();
    }
}

struct TokenSet {
    access: SecretString,
    refresh: Option<SecretString>,
    expires_in: Option<u64>,
    scopes: Option<Vec<String>>,
}

struct TokenReply {
    status: u16,
    body: Value,
}

// vhco:infra oauth_adapter satisfies OAuthSessionDriver, CredentialProvider
// vhco:net connect https -- profile token_url / device_url only, through the transports.exchange_http use case (allow_network per attempt, checked addresses, no redirects)
// vhco:env <client_secret env VAR> -- the confidential client secret, read only after an allow_env permit and sent only to the profile's token/device endpoint
// vhco:db readwrite keychain -- `store keychain "NS"` entries {generation, version, token set} in the host secure store (keyring-core)
pub struct OAuthAdapter {
    profiles: HashMap<String, OAuthProfile>,
    exchange: Arc<ExchangeHttpFn>,
    client: HyperClient,
    codec: StdCodec,
    env: Arc<EnvFn>,
    memory: Mutex<HashMap<String, Stored>>,
    keychain: Mutex<Option<Arc<dyn SecretBackend>>>,
    keychain_factory: Arc<BackendFactory>,
    slots: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    transactions: Mutex<HashMap<String, Tx>>,
    /// sha256 of access tokens a resource rejected (never reused).
    revoked: Mutex<HashSet<String>>,
    counter: AtomicU64,
    rng: ring::rand::SystemRandom,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn conflict(code: &str, message: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Conflict, code, message)
}

fn store_failed(message: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Application, "auth.store_failed", message)
}

fn digest_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A provider error code, only when it is a plain RFC 6749 token (never raw text).
fn oauth_error(body: &Value) -> Option<String> {
    let e = body.get("error")?.as_str()?;
    (!e.is_empty() && e.len() <= 64 && e.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .then(|| e.to_string())
}

fn token_failure(status: u16, code: Option<&str>) -> RivetError {
    match code {
        Some("invalid_grant") => conflict(
            "auth.login_required",
            "the authorization server rejected the grant (invalid_grant); authorize the account again",
        ),
        Some("access_denied") => conflict(
            "auth.access_denied",
            "the user or authorization server denied the authorization",
        ),
        Some("expired_token") => conflict(
            "auth.transaction_expired",
            "the authorization transaction expired at the provider",
        ),
        Some("invalid_scope") => conflict(
            "auth.insufficient_scope",
            "the authorization server refused the requested scopes",
        ),
        other => {
            let mut e = RivetError::new(
                ErrorKind::Application,
                "auth.token_endpoint_failed",
                format!(
                    "the token endpoint answered {status}{}",
                    other.map(|c| format!(" ({c})")).unwrap_or_default()
                ),
            );
            e = e.with_details(Value::object([
                ("status", Value::Int(status as i64)),
                ("error", other.map(Value::text).unwrap_or(Value::Null)),
            ]));
            e
        }
    }
}

fn parse_tokens(body: &Value) -> RivetResult<TokenSet> {
    let bad = || {
        RivetError::new(
            ErrorKind::Protocol,
            "auth.token_endpoint_failed",
            "the token endpoint answered 200 without a usable bearer access_token",
        )
    };
    let access = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(bad)?;
    if let Some(t) = body.get("token_type").and_then(Value::as_str)
        && !t.eq_ignore_ascii_case("bearer")
    {
        return Err(RivetError::unsupported(
            "unsupported.token_type",
            "only Bearer access tokens are supported (DPoP/MAC are not)",
        ));
    }
    let expires_in = match body.get("expires_in") {
        Some(Value::Int(i)) if *i > 0 => Some(*i as u64),
        Some(Value::Float(f)) if *f > 0.0 => Some(*f as u64),
        Some(Value::Text(s)) => s.parse::<u64>().ok().filter(|v| *v > 0),
        _ => None,
    };
    Ok(TokenSet {
        access: SecretString::new(access),
        refresh: body
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(SecretString::new),
        expires_in,
        scopes: body
            .get("scope")
            .and_then(Value::as_str)
            .map(|s| s.split_whitespace().map(str::to_string).collect()),
    })
}

fn random_token(rng: &ring::rand::SystemRandom, bytes: usize) -> RivetResult<String> {
    use ring::rand::SecureRandom;
    let mut buf = vec![0u8; bytes];
    rng.fill(&mut buf)
        .map_err(|_| RivetError::internal("the system random generator failed"))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

fn form_escape(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Fresh enough to lease: present, not revoked, and valid beyond the skew
/// (min(30 s, half the lifetime)). A token without expiry is reused only by
/// user flows; client credentials reacquire.
fn usable(rec: &Stored, flow: OAuthFlow, revoked: &HashSet<String>) -> bool {
    let Some(tok) = &rec.access_token else {
        return false;
    };
    if revoked.contains(&digest_hex(tok)) {
        return false;
    }
    match rec.expires_at {
        Some(exp) => {
            let skew = (exp.saturating_sub(rec.issued_at) / 2).min(30);
            now() + skew < exp
        }
        None => flow != OAuthFlow::ClientCredentials,
    }
}

impl OAuthAdapter {
    /// Validate every `auth NAME oauth2` profile of the program (load fails on
    /// the first invalid profile) and wire the brokered HTTP exchange.
    pub fn load(program: &CompiledProgram, exchange: Arc<ExchangeHttpFn>) -> RivetResult<Self> {
        let mut profiles = HashMap::new();
        for d in &program.auth_profiles {
            if d.kind != "oauth2" {
                return Err(RivetError::unsupported(
                    "unsupported.auth_kind",
                    format!(
                        "auth profile `{}`: only `oauth2` profiles are supported",
                        d.name
                    ),
                )
                .with_span(Some(d.span.clone())));
            }
            let p = OAuthProfile::from_declaration(d)?;
            profiles.insert(p.name.clone(), p);
        }
        Ok(OAuthAdapter {
            profiles,
            exchange,
            client: HyperClient,
            codec: StdCodec,
            env: Arc::new(|k: &str| std::env::var(k).ok()),
            memory: Mutex::new(HashMap::new()),
            keychain: Mutex::new(None),
            keychain_factory: Arc::new(platform_backend),
            slots: Mutex::new(HashMap::new()),
            transactions: Mutex::new(HashMap::new()),
            revoked: Mutex::new(HashSet::new()),
            counter: AtomicU64::new(0),
            rng: ring::rand::SystemRandom::new(),
        })
    }

    /// Replace the environment lookup (library hosts, tests).
    pub fn with_env(mut self, env: Arc<EnvFn>) -> Self {
        self.env = env;
        self
    }

    /// Use this secure backend for `store keychain` instead of the platform store.
    pub fn with_keychain(self, backend: Arc<dyn SecretBackend>) -> Self {
        *lock(&self.keychain) = Some(backend);
        self
    }

    fn profile_ref(&self, name: &str) -> RivetResult<&OAuthProfile> {
        self.profiles.get(name).ok_or_else(|| {
            RivetError::not_found(
                "not_found.auth_profile",
                format!("no auth profile `{name}`"),
            )
        })
    }

    fn key(principal: &Principal, p: &OAuthProfile, account: &str) -> String {
        format!(
            "{}/{}#{}/{}",
            principal.name,
            p.name,
            &p.config_hash[..12],
            account
        )
    }

    fn slot(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        lock(&self.slots)
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    fn backend(&self) -> RivetResult<Arc<dyn SecretBackend>> {
        let mut g = lock(&self.keychain);
        if let Some(b) = g.as_ref() {
            return Ok(Arc::clone(b));
        }
        let b = (self.keychain_factory)()?;
        *g = Some(Arc::clone(&b));
        Ok(b)
    }

    async fn load_entry(&self, p: &OAuthProfile, key: &str) -> RivetResult<Option<Stored>> {
        let rec =
            match &p.store {
                CredentialStoreRef::Memory => lock(&self.memory).get(key).cloned(),
                CredentialStoreRef::Keychain(ns) => {
                    let b = self.backend()?;
                    let (ns, user) = (ns.clone(), key.to_string());
                    let bytes = tokio::task::spawn_blocking(move || b.get(&ns, &user))
                        .await
                        .map_err(|_| store_failed("the secure store task failed"))?
                        .map_err(store_failed)?;
                    match bytes {
                        None => None,
                        Some(b) => Some(serde_json::from_slice::<Stored>(&b).map_err(|_| {
                            store_failed("the stored credential entry is unreadable")
                        })?),
                    }
                }
            };
        Ok(rec.filter(|r| r.profile_hash == p.config_hash))
    }

    async fn save_entry(&self, p: &OAuthProfile, key: &str, rec: &Stored) -> RivetResult<()> {
        match &p.store {
            CredentialStoreRef::Memory => {
                lock(&self.memory).insert(key.to_string(), rec.clone());
                Ok(())
            }
            CredentialStoreRef::Keychain(ns) => {
                let b = self.backend()?;
                let bytes = serde_json::to_vec(rec)
                    .map_err(|_| store_failed("cannot encode the credential entry"))?;
                let (ns, user) = (ns.clone(), key.to_string());
                tokio::task::spawn_blocking(move || b.set(&ns, &user, &bytes))
                    .await
                    .map_err(|_| store_failed("the secure store task failed"))?
                    .map_err(store_failed)
            }
        }
    }

    fn origin(ctx: &AuthContext) -> EffectOrigin {
        EffectOrigin {
            operation_id: ctx.operation_id.clone(),
            span: None,
        }
    }

    fn client_secret(
        &self,
        p: &OAuthProfile,
        ev: &dyn PolicyEvaluator,
        ctx: &AuthContext,
    ) -> RivetResult<Option<SecretString>> {
        let Some(SecretRef::Env(var)) = &p.client_secret else {
            return Ok(None);
        };
        authorize(
            ev,
            &Self::origin(ctx),
            Capability::Env,
            AccessVerb::Read,
            EffectTarget::Env(var.clone()),
        )?;
        match (self.env)(var).filter(|v| !v.is_empty()) {
            Some(v) => Ok(Some(SecretString::new(v))),
            None => Err(RivetError::new(
                ErrorKind::Application,
                "auth.client_secret_missing",
                format!(
                    "auth profile `{}`: client secret env {var} is not set",
                    p.name
                ),
            )),
        }
    }

    /// One brokered `application/x-www-form-urlencoded` POST to a profile endpoint.
    async fn post_form(
        &self,
        p: &OAuthProfile,
        url: &str,
        mut form: Vec<(&'static str, String)>,
        ev: &dyn PolicyEvaluator,
        ctx: &AuthContext,
        deadline: Instant,
    ) -> RivetResult<TokenReply> {
        let mut headers = vec![("accept".to_string(), "application/json".to_string())];
        let secret = self.client_secret(p, ev, ctx)?;
        match (p.client_auth, secret) {
            (crate::domain::auth::ClientAuthMethod::Basic, Some(s)) => {
                let raw = format!("{}:{}", form_escape(&p.client_id), form_escape(s.expose()));
                headers.push((
                    "authorization".into(),
                    format!("Basic {}", STANDARD.encode(raw)),
                ));
            }
            (crate::domain::auth::ClientAuthMethod::Post, Some(s)) => {
                form.push(("client_id", p.client_id.clone()));
                form.push(("client_secret", s.expose().to_string()));
            }
            _ => form.push(("client_id", p.client_id.clone())),
        }
        let x = HttpExchange {
            method: "POST".into(),
            url: url.to_string(),
            headers,
            query: Vec::new(),
            body: Some(CodecInput {
                kind: CodecKind::Form,
                bytes: None,
                value: Some(Value::object(
                    form.into_iter().map(|(k, v)| (k, Value::Text(v))),
                )),
            }),
            version: HttpVersionPolicy::Auto,
            decode: Some(CodecKind::Json),
            accept: TOKEN_STATUSES.to_vec(),
            retry: None,
            redirect_limit: 0,
            tls: TlsMaterial::default(),
            stream: None,
            unix_socket: None,
            max_body: 1 << 20,
            origin: Self::origin(ctx),
        };
        let left = deadline
            .saturating_duration_since(Instant::now())
            .min(TOKEN_TIMEOUT);
        let timeout = || {
            RivetError::new(
                ErrorKind::Timeout,
                "timeout.auth_token",
                "the authorization server did not answer before the deadline",
            )
        };
        if left.is_zero() {
            return Err(timeout());
        }
        let fut = (self.exchange)(x, ev, &self.client, &self.codec);
        let out = tokio::time::timeout(left, fut)
            .await
            .map_err(|_| timeout())??;
        Ok(TokenReply {
            status: out.response.status,
            body: out.response.body,
        })
    }

    /// Persist a new token set (allow_credentials write first).
    #[allow(clippy::too_many_arguments)]
    async fn commit(
        &self,
        p: &OAuthProfile,
        key: &str,
        account: &str,
        prev: Option<&Stored>,
        tokens: TokenSet,
        new_login: bool,
        ev: &dyn PolicyEvaluator,
        ctx: &AuthContext,
    ) -> RivetResult<Stored> {
        authorize(
            ev,
            &Self::origin(ctx),
            Capability::Credentials,
            AccessVerb::Write,
            EffectTarget::Logical(format!("{}/{account}", p.name)),
        )?;
        let t = now();
        let prev_gen = prev.map_or(0, |r| r.generation);
        let scopes = tokens
            .scopes
            .map(|s| {
                s.into_iter()
                    .filter(|x| p.scopes.contains(x))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| p.scopes.clone());
        let rec = Stored {
            generation: if new_login {
                prev_gen + 1
            } else {
                prev_gen.max(1)
            },
            version: prev.map_or(0, |r| r.version) + 1,
            profile_hash: p.config_hash.clone(),
            access_token: Some(tokens.access.expose().to_string()),
            refresh_token: tokens.refresh.map(|r| r.expose().to_string()).or_else(|| {
                if new_login {
                    None
                } else {
                    prev.and_then(|r| r.refresh_token.clone())
                }
            }),
            expires_at: tokens.expires_in.map(|e| t + e),
            issued_at: t,
            scopes,
        };
        self.save_entry(p, key, &rec).await?;
        Ok(rec)
    }

    /// Replace the tokens of an entry with nothing (same generation).
    async fn drop_tokens(&self, p: &OAuthProfile, key: &str, rec: &Stored) {
        let mut empty = rec.clone();
        empty.access_token = None;
        empty.refresh_token = None;
        empty.version += 1;
        let _ = self.save_entry(p, key, &empty).await;
    }

    fn status_of(
        p: &OAuthProfile,
        account: &str,
        rec: Option<&Stored>,
        revoked: &HashSet<String>,
    ) -> CredentialStatus {
        let (state, scopes, expires_at, generation) = match rec {
            Some(r) if r.access_token.is_some() || r.refresh_token.is_some() => {
                let state = if usable(r, p.flow, revoked) {
                    CredentialState::Connected
                } else {
                    CredentialState::Expired
                };
                (
                    state,
                    r.scopes.clone(),
                    r.expires_at.map(rfc3339),
                    r.generation,
                )
            }
            Some(r) => (
                CredentialState::Disconnected,
                Vec::new(),
                None,
                r.generation,
            ),
            None => (CredentialState::Disconnected, Vec::new(), None, 0),
        };
        CredentialStatus {
            profile: p.name.clone(),
            account: account.to_string(),
            state,
            scopes,
            expires_at,
            generation,
            transaction_id: None,
        }
    }

    fn lease(key: &str, origin: &str, rec: &Stored) -> CredentialLease {
        CredentialLease {
            handle: SecretString::new(rec.access_token.clone().unwrap_or_default()),
            scope_id: key.to_string(),
            origin: origin.to_string(),
            generation: rec.generation,
            expires_at: rec.expires_at.map(rfc3339),
        }
    }

    fn tx_error(state: TxState) -> RivetError {
        match state {
            TxState::Expired => conflict(
                "auth.transaction_expired",
                "the authorization transaction expired; begin again",
            ),
            TxState::Denied => conflict("auth.access_denied", "the authorization was denied"),
            TxState::Exchanging => conflict(
                "auth.callback_invalid",
                "the authorization transaction is already being completed",
            ),
            _ => conflict(
                "auth.callback_invalid",
                format!(
                    "the authorization transaction is {} and cannot be completed again",
                    state.as_str()
                ),
            ),
        }
    }

    fn pending(p: &OAuthProfile, account: &str, id: &str, expires_at: u64) -> CredentialStatus {
        CredentialStatus {
            profile: p.name.clone(),
            account: account.to_string(),
            state: CredentialState::Pending,
            scopes: Vec::new(),
            expires_at: Some(rfc3339(expires_at)),
            generation: 0,
            transaction_id: Some(id.to_string()),
        }
    }

    fn prune(&self, txs: &mut HashMap<String, Tx>) {
        let t = now();
        for tx in txs.values_mut() {
            if tx.state == TxState::Open && t >= tx.expires_at {
                tx.end(TxState::Expired);
            }
        }
        txs.retain(|_, tx| {
            matches!(tx.state, TxState::Open | TxState::Exchanging)
                || tx.created.elapsed() < Duration::from_secs(2 * MAX_TRANSACTION_SECS)
        });
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish_login(
        &self,
        p: &OAuthProfile,
        id: &str,
        principal: &Principal,
        account: &str,
        tokens: TokenSet,
        ev: &dyn PolicyEvaluator,
        ctx: &AuthContext,
    ) -> RivetResult<CredentialStatus> {
        let key = Self::key(principal, p, account);
        let slot = self.slot(&key);
        let _g = slot.lock().await;
        let prev = self.load_entry(p, &key).await?;
        match self
            .commit(p, &key, account, prev.as_ref(), tokens, true, ev, ctx)
            .await
        {
            Ok(rec) => {
                if let Some(tx) = lock(&self.transactions).get_mut(id) {
                    tx.end(TxState::Connected);
                }
                let revoked = lock(&self.revoked).clone();
                Ok(Self::status_of(p, account, Some(&rec), &revoked))
            }
            Err(e) => {
                if let Some(tx) = lock(&self.transactions).get_mut(id) {
                    tx.end(TxState::Failed);
                }
                Err(e)
            }
        }
    }

    async fn complete_code(
        &self,
        p: &OAuthProfile,
        input: AuthCompleteInput,
        account: String,
        ev: &dyn PolicyEvaluator,
    ) -> RivetResult<CredentialStatus> {
        let ctx = &input.context;
        let id = input.transaction_id.clone();
        let cb = input.callback.clone().ok_or_else(|| {
            RivetError::validation(
                "validation.auth_callback",
                "an authorization_code transaction needs a callback",
            )
        })?;
        let verifier = {
            let mut txs = lock(&self.transactions);
            let tx = txs
                .get_mut(&id)
                .ok_or_else(|| Self::tx_error(TxState::Failed))?;
            if tx.state != TxState::Open {
                return Err(Self::tx_error(tx.state));
            }
            if let Some(err) = &cb.error {
                let denied = err == "access_denied";
                tx.end(if denied {
                    TxState::Denied
                } else {
                    TxState::Failed
                });
                return Err(if denied {
                    Self::tx_error(TxState::Denied)
                } else {
                    conflict(
                        "auth.callback_invalid",
                        "the authorization server returned an error instead of a code",
                    )
                });
            }
            let state_ok = !tx.oauth_state.is_empty()
                && ct_eq(
                    cb.state.expose().as_bytes(),
                    tx.oauth_state.expose().as_bytes(),
                );
            let redirect_ok = p.redirect_uri.as_deref() == Some(cb.redirect_uri.as_str());
            let issuer_ok = cb.issuer.as_deref().is_none_or(|i| i == p.issuer);
            if !(state_ok && redirect_ok && issuer_ok) {
                let what = if !state_ok {
                    "state"
                } else if !redirect_ok {
                    "redirect_uri"
                } else {
                    "issuer"
                };
                tx.end(TxState::Failed);
                return Err(conflict(
                    "auth.callback_invalid",
                    format!(
                        "the callback {what} does not match the transaction; nothing was exchanged"
                    ),
                )
                .with_details(Value::object([("mismatch", Value::text(what))])));
            }
            tx.state = TxState::Exchanging;
            std::mem::take(&mut tx.verifier)
        };
        let deadline = Instant::now() + Duration::from_millis(ctx.deadline_ms.max(1));
        let form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", cb.code.expose().to_string()),
            ("redirect_uri", cb.redirect_uri.clone()),
            ("code_verifier", verifier.expose().to_string()),
        ];
        let reply = self
            .post_form(p, &p.token_endpoint, form, ev, ctx, deadline)
            .await;
        let fail = |state: TxState| {
            if let Some(tx) = lock(&self.transactions).get_mut(&id) {
                tx.end(state);
            }
        };
        let reply = match reply {
            Ok(r) => r,
            Err(e) => {
                fail(TxState::Failed);
                return Err(e);
            }
        };
        if reply.status != 200 {
            let code = oauth_error(&reply.body);
            fail(if code.as_deref() == Some("access_denied") {
                TxState::Denied
            } else {
                TxState::Failed
            });
            return Err(token_failure(reply.status, code.as_deref()));
        }
        let tokens = match parse_tokens(&reply.body) {
            Ok(t) => t,
            Err(e) => {
                fail(TxState::Failed);
                return Err(e);
            }
        };
        // A cancel while the code was being exchanged wins: nothing is stored.
        let still = lock(&self.transactions).get(&id).map(|t| t.state);
        if still != Some(TxState::Exchanging) {
            return Err(Self::tx_error(still.unwrap_or(TxState::Cancelled)));
        }
        self.finish_login(p, &id, &ctx.principal, &account, tokens, ev, ctx)
            .await
    }

    async fn complete_device(
        &self,
        p: &OAuthProfile,
        input: AuthCompleteInput,
        account: String,
        ev: &dyn PolicyEvaluator,
    ) -> RivetResult<CredentialStatus> {
        let ctx = &input.context;
        let id = input.transaction_id.clone();
        let deadline = Instant::now() + Duration::from_millis(ctx.deadline_ms.max(1));
        let poll_lock = lock(&self.transactions)
            .get(&id)
            .map(|t| Arc::clone(&t.poll))
            .ok_or_else(|| Self::tx_error(TxState::Failed))?;
        // One poller per transaction; a second complete waits its turn (bounded by its deadline).
        let _turn = match tokio::time::timeout_at(deadline.into(), poll_lock.lock()).await {
            Ok(g) => g,
            Err(_) => {
                let exp = lock(&self.transactions)
                    .get(&id)
                    .map_or(0, |t| t.expires_at);
                return Ok(Self::pending(p, &account, &id, exp));
            }
        };
        loop {
            let (next, device_code, interval, expires_at) = {
                let mut txs = lock(&self.transactions);
                let tx = txs
                    .get_mut(&id)
                    .ok_or_else(|| Self::tx_error(TxState::Failed))?;
                if tx.state == TxState::Open && now() >= tx.expires_at {
                    tx.end(TxState::Expired);
                }
                if tx.state != TxState::Open {
                    return Err(Self::tx_error(tx.state));
                }
                (
                    tx.next_poll,
                    tx.device_code.clone(),
                    tx.interval_ms,
                    tx.expires_at,
                )
            };
            let pending = || Ok(Self::pending(p, &account, &id, expires_at));
            let t = Instant::now();
            if let Some(at) = next
                && at > t
            {
                if !input.wait || at >= deadline {
                    return pending();
                }
                tokio::time::sleep_until(at.into()).await;
                continue;
            }
            if deadline <= t {
                return pending();
            }
            let set_next = |delay_ms: u64, interval_ms: u64| {
                if let Some(tx) = lock(&self.transactions).get_mut(&id) {
                    tx.interval_ms = interval_ms;
                    tx.next_poll = Some(Instant::now() + Duration::from_millis(delay_ms));
                }
            };
            let form = vec![
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code".to_string(),
                ),
                ("device_code", device_code.expose().to_string()),
            ];
            let reply = self
                .post_form(p, &p.token_endpoint, form, ev, ctx, deadline)
                .await;
            let reply = match reply {
                Ok(r) => r,
                // Policy/config refusals: nothing was sent; the transaction stays open.
                Err(e)
                    if matches!(
                        e.kind,
                        ErrorKind::Permission | ErrorKind::Validation | ErrorKind::Application
                    ) =>
                {
                    return Err(e);
                }
                Err(e) if e.kind == ErrorKind::Timeout => {
                    set_next(interval, interval);
                    return pending();
                }
                // Transport faults back off (twice the interval) instead of busy-looping.
                Err(_) => {
                    set_next(interval * 2, interval);
                    if !input.wait {
                        return pending();
                    }
                    continue;
                }
            };
            if reply.status == 200 {
                let tokens = parse_tokens(&reply.body)?;
                if let Some(tx) = lock(&self.transactions).get_mut(&id) {
                    tx.state = TxState::Exchanging;
                }
                return self
                    .finish_login(p, &id, &ctx.principal, &account, tokens, ev, ctx)
                    .await;
            }
            match oauth_error(&reply.body).as_deref() {
                Some("authorization_pending") => set_next(interval, interval),
                Some("slow_down") => set_next(interval + SLOW_DOWN_MS, interval + SLOW_DOWN_MS),
                other => {
                    let state = match other {
                        Some("access_denied") => TxState::Denied,
                        Some("expired_token") => TxState::Expired,
                        _ => TxState::Failed,
                    };
                    if let Some(tx) = lock(&self.transactions).get_mut(&id) {
                        tx.end(state);
                    }
                    return Err(token_failure(reply.status, other));
                }
            }
            if !input.wait {
                return pending();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn refresh(
        &self,
        p: &OAuthProfile,
        key: &str,
        account: &str,
        rec: Stored,
        ev: &dyn PolicyEvaluator,
        ctx: &AuthContext,
        deadline: Instant,
    ) -> RivetResult<Stored> {
        let uncertain = |why: &str| {
            conflict(
                "auth.refresh_uncertain",
                format!(
                    "the token refresh outcome is uncertain ({why}); authorize the account again"
                ),
            )
        };
        let mut form = vec![
            ("grant_type", "refresh_token".to_string()),
            (
                "refresh_token",
                rec.refresh_token.clone().unwrap_or_default(),
            ),
        ];
        if !rec.scopes.is_empty() {
            form.push(("scope", rec.scopes.join(" ")));
        }
        let reply = match self
            .post_form(p, &p.token_endpoint, form, ev, ctx, deadline)
            .await
        {
            Ok(r) => r,
            Err(e)
                if e.kind == ErrorKind::Timeout
                    || e.code == "connection.http"
                    || e.code == "connection.http_body" =>
            {
                // The request may have reached the server: never replay the old refresh token.
                self.drop_tokens(p, key, &rec).await;
                return Err(uncertain("no answer after the refresh was sent"));
            }
            Err(e) => return Err(e),
        };
        if reply.status != 200 {
            let code = oauth_error(&reply.body);
            if code.as_deref() == Some("invalid_grant") {
                self.drop_tokens(p, key, &rec).await;
            }
            return Err(token_failure(reply.status, code.as_deref()));
        }
        let tokens = match parse_tokens(&reply.body) {
            Ok(t) => t,
            Err(_) => {
                self.drop_tokens(p, key, &rec).await;
                return Err(uncertain("the refresh answer was unusable"));
            }
        };
        // Rotation race: another writer (process sharing the keychain) committed meanwhile.
        let current = self.load_entry(p, key).await?;
        if current.as_ref().map(|c| c.version) != Some(rec.version) {
            return Err(uncertain("the credential was rotated concurrently"));
        }
        self.commit(p, key, account, Some(&rec), tokens, false, ev, ctx)
            .await
            .map_err(|e| {
                if e.code == "auth.store_failed" {
                    uncertain("the rotated token set could not be stored")
                } else {
                    e
                }
            })
    }
}

#[async_trait]
impl OAuthSessionDriver for OAuthAdapter {
    fn profile(&self, name: &str) -> Option<OAuthProfile> {
        self.profiles.get(name).cloned()
    }

    fn transaction(&self, id: &str, principal: &Principal) -> Option<AuthTransactionInfo> {
        let mut txs = lock(&self.transactions);
        self.prune(&mut txs);
        let tx = txs.get(id).filter(|t| t.principal == principal.name)?;
        Some(AuthTransactionInfo {
            transaction_id: id.to_string(),
            profile: tx.profile.clone(),
            account: tx.account.clone(),
            state: tx.state.as_str().into(),
        })
    }

    async fn begin(
        &self,
        input: AuthBeginInput,
        ev: &dyn PolicyEvaluator,
    ) -> RivetResult<AuthChallenge> {
        let p = self.profile_ref(&input.profile)?;
        if matches!(p.store, CredentialStoreRef::Keychain(_)) {
            // Fail before any authorization if the secure store cannot hold the result.
            self.backend()?;
        }
        let principal = input.context.principal.name.clone();
        {
            let mut txs = lock(&self.transactions);
            self.prune(&mut txs);
            let open = txs
                .values()
                .filter(|t| t.principal == principal && t.state == TxState::Open)
                .count();
            if open >= MAX_OPEN_PER_PRINCIPAL {
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.auth_transactions",
                    format!(
                        "at most {MAX_OPEN_PER_PRINCIPAL} authorization transactions may be open per principal; complete or cancel one"
                    ),
                ));
            }
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let id = format!(
            "auth_{n:02}{}",
            &random_token(&self.rng, 9)?
                .to_ascii_lowercase()
                .replace(['-', '_'], "x")
        );
        let t = now();
        let mut tx = Tx {
            principal,
            profile: p.name.clone(),
            account: input.account.clone(),
            state: TxState::Open,
            expires_at: t + MAX_TRANSACTION_SECS,
            oauth_state: SecretString::default(),
            verifier: SecretString::default(),
            device_code: SecretString::default(),
            interval_ms: 5_000,
            next_poll: None,
            poll: Arc::new(tokio::sync::Mutex::new(())),
            created: Instant::now(),
        };
        let challenge = match p.flow {
            OAuthFlow::AuthorizationCode => {
                let state = random_token(&self.rng, 32)?;
                let verifier = random_token(&self.rng, 32)?;
                let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
                let base = p.authorization_endpoint.clone().unwrap_or_default();
                let mut u = url::Url::parse(&base).map_err(|e| {
                    RivetError::validation(
                        "validation.auth_profile",
                        format!("authorization_url: {e}"),
                    )
                })?;
                {
                    let mut q = u.query_pairs_mut();
                    q.append_pair("response_type", "code");
                    q.append_pair("client_id", &p.client_id);
                    q.append_pair("redirect_uri", p.redirect_uri.as_deref().unwrap_or(""));
                    q.append_pair("scope", &p.scopes.join(" "));
                    q.append_pair("state", &state);
                    q.append_pair("code_challenge", &code_challenge);
                    q.append_pair("code_challenge_method", "S256");
                    if let Some(a) = &p.audience {
                        q.append_pair("audience", a);
                    }
                }
                tx.oauth_state = SecretString::new(state);
                tx.verifier = SecretString::new(verifier);
                AuthChallenge {
                    transaction_id: id.clone(),
                    authorization_url: Some(u.to_string()),
                    verification_uri: None,
                    verification_uri_complete: None,
                    user_code: None,
                    expires_at: rfc3339(tx.expires_at),
                    interval_seconds: None,
                }
            }
            OAuthFlow::DeviceCode => {
                let deadline =
                    Instant::now() + Duration::from_millis(input.context.deadline_ms.max(1));
                let mut form = vec![("scope", p.scopes.join(" "))];
                if let Some(a) = &p.audience {
                    form.push(("audience", a.clone()));
                }
                let url = p.device_endpoint.clone().unwrap_or_default();
                let reply = self
                    .post_form(p, &url, form, ev, &input.context, deadline)
                    .await?;
                if reply.status != 200 {
                    return Err(token_failure(
                        reply.status,
                        oauth_error(&reply.body).as_deref(),
                    ));
                }
                let b = &reply.body;
                let text = |k: &str| b.get(k).and_then(Value::as_str).map(str::to_string);
                let bad = || {
                    RivetError::new(
                        ErrorKind::Protocol,
                        "auth.token_endpoint_failed",
                        "the device endpoint answer lacks device_code, user_code or verification_uri",
                    )
                };
                let device_code = text("device_code").ok_or_else(bad)?;
                let user_code = text("user_code").ok_or_else(bad)?;
                let verification_uri = text("verification_uri")
                    .or_else(|| text("verification_url"))
                    .ok_or_else(bad)?;
                let expires_in = b
                    .get("expires_in")
                    .and_then(Value::as_i64)
                    .filter(|v| *v > 0)
                    .map_or(MAX_TRANSACTION_SECS, |v| v as u64);
                let interval = b
                    .get("interval")
                    .and_then(Value::as_i64)
                    .filter(|v| *v > 0)
                    .map_or(5, |v| v as u64);
                tx.expires_at = t + expires_in.min(MAX_TRANSACTION_SECS);
                tx.device_code = SecretString::new(device_code);
                tx.interval_ms = interval * 1_000;
                tx.next_poll = Some(Instant::now() + Duration::from_millis(tx.interval_ms));
                AuthChallenge {
                    transaction_id: id.clone(),
                    authorization_url: None,
                    verification_uri: Some(verification_uri),
                    verification_uri_complete: text("verification_uri_complete"),
                    user_code: Some(user_code),
                    expires_at: rfc3339(tx.expires_at),
                    interval_seconds: Some(interval as u32),
                }
            }
            OAuthFlow::ClientCredentials => {
                return Err(RivetError::validation(
                    "validation.auth_flow",
                    "client_credentials profiles have no authorization transaction",
                ));
            }
        };
        lock(&self.transactions).insert(id, tx);
        Ok(challenge)
    }

    async fn complete(
        &self,
        input: AuthCompleteInput,
        ev: &dyn PolicyEvaluator,
    ) -> RivetResult<CredentialStatus> {
        let (profile, account) = {
            let mut txs = lock(&self.transactions);
            let tx = txs
                .get_mut(&input.transaction_id)
                .filter(|t| t.principal == input.context.principal.name)
                .ok_or_else(|| {
                    RivetError::not_found(
                        "not_found.auth_transaction",
                        format!("no authorization transaction `{}`", input.transaction_id),
                    )
                })?;
            if tx.state == TxState::Open && now() >= tx.expires_at {
                tx.end(TxState::Expired);
            }
            if tx.state != TxState::Open {
                return Err(Self::tx_error(tx.state));
            }
            (tx.profile.clone(), tx.account.clone())
        };
        let p = self.profile_ref(&profile)?.clone();
        match p.flow {
            OAuthFlow::AuthorizationCode => self.complete_code(&p, input, account, ev).await,
            OAuthFlow::DeviceCode => self.complete_device(&p, input, account, ev).await,
            OAuthFlow::ClientCredentials => Err(RivetError::validation(
                "validation.auth_flow",
                "client_credentials profiles have no authorization transaction",
            )),
        }
    }

    async fn status(&self, input: CredentialStatusInput) -> RivetResult<CredentialStatus> {
        let p = self.profile_ref(&input.profile)?;
        let key = Self::key(&input.context.principal, p, &input.account);
        let rec = self.load_entry(p, &key).await?;
        let revoked = lock(&self.revoked).clone();
        Ok(Self::status_of(p, &input.account, rec.as_ref(), &revoked))
    }

    async fn disconnect(
        &self,
        input: DisconnectInput,
        _ev: &dyn PolicyEvaluator,
    ) -> RivetResult<DisconnectReceipt> {
        let p = self.profile_ref(&input.profile)?;
        {
            let mut txs = lock(&self.transactions);
            for tx in txs.values_mut() {
                if tx.principal == input.context.principal.name
                    && tx.profile == p.name
                    && tx.account == input.account
                    && matches!(tx.state, TxState::Open | TxState::Exchanging)
                {
                    tx.end(TxState::Cancelled);
                }
            }
        }
        let key = Self::key(&input.context.principal, p, &input.account);
        let slot = self.slot(&key);
        let _g = slot.lock().await;
        let prev = self.load_entry(p, &key).await?;
        let rec = Stored {
            generation: prev.as_ref().map_or(0, |r| r.generation) + 1,
            version: prev.as_ref().map_or(0, |r| r.version) + 1,
            profile_hash: p.config_hash.clone(),
            ..Stored::default()
        };
        self.save_entry(p, &key, &rec).await?;
        Ok(DisconnectReceipt {
            profile: p.name.clone(),
            account: input.account,
            local_only: true,
            generation: rec.generation,
        })
    }

    async fn cancel(&self, input: AuthCancelInput) -> RivetResult<AuthCancelReceipt> {
        let mut txs = lock(&self.transactions);
        let tx = txs
            .get_mut(&input.transaction_id)
            .filter(|t| t.principal == input.context.principal.name)
            .ok_or_else(|| {
                RivetError::not_found(
                    "not_found.auth_transaction",
                    format!("no authorization transaction `{}`", input.transaction_id),
                )
            })?;
        if tx.state == TxState::Open && now() >= tx.expires_at {
            tx.end(TxState::Expired);
        }
        if matches!(tx.state, TxState::Open | TxState::Exchanging) {
            tx.end(TxState::Cancelled);
        }
        Ok(AuthCancelReceipt {
            transaction_id: input.transaction_id,
            state: tx.state.as_str().into(),
        })
    }
}

#[async_trait]
impl CredentialProvider for OAuthAdapter {
    fn profile(&self, name: &str) -> Option<OAuthProfile> {
        self.profiles.get(name).cloned()
    }

    async fn acquire(
        &self,
        input: CredentialInput,
        ev: &dyn PolicyEvaluator,
    ) -> RivetResult<CredentialLease> {
        let p = self.profile_ref(&input.profile)?;
        let ctx = &input.context;
        let key = Self::key(&ctx.principal, p, &input.account);
        let deadline = Instant::now() + Duration::from_millis(ctx.deadline_ms.max(1));
        // Single flight per credential identity: waiters re-read the result.
        let slot = self.slot(&key);
        let _g = match tokio::time::timeout_at(deadline.into(), slot.lock()).await {
            Ok(g) => g,
            Err(_) => {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.auth_token",
                    "timed out waiting for a concurrent token refresh",
                ));
            }
        };
        let prev = self.load_entry(p, &key).await?;
        let need: Vec<String> = if input.scopes.is_empty() {
            p.scopes.clone()
        } else {
            input.scopes.clone()
        };
        let revoked = lock(&self.revoked).clone();
        let check_scopes = |rec: &Stored| -> RivetResult<()> {
            match need.iter().find(|s| !rec.scopes.contains(s)) {
                Some(s) => Err(conflict(
                    "auth.insufficient_scope",
                    format!("the credential was not granted scope `{s}`"),
                )),
                None => Ok(()),
            }
        };
        if let Some(rec) = &prev
            && usable(rec, p.flow, &revoked)
        {
            check_scopes(rec)?;
            return Ok(Self::lease(&key, &input.origin, rec));
        }
        let rec = match p.flow {
            OAuthFlow::ClientCredentials => {
                let mut form = vec![
                    ("grant_type", "client_credentials".to_string()),
                    ("scope", p.scopes.join(" ")),
                ];
                if let Some(a) = &p.audience {
                    form.push(("audience", a.clone()));
                }
                let reply = self
                    .post_form(p, &p.token_endpoint, form, ev, ctx, deadline)
                    .await?;
                if reply.status != 200 {
                    return Err(token_failure(
                        reply.status,
                        oauth_error(&reply.body).as_deref(),
                    ));
                }
                let tokens = parse_tokens(&reply.body)?;
                let fresh = prev
                    .as_ref()
                    .is_none_or(|r| r.access_token.is_none() && r.refresh_token.is_none());
                self.commit(
                    p,
                    &key,
                    &input.account,
                    prev.as_ref(),
                    tokens,
                    fresh,
                    ev,
                    ctx,
                )
                .await?
            }
            _ => match prev.filter(|r| r.refresh_token.is_some()) {
                Some(rec) => {
                    self.refresh(p, &key, &input.account, rec, ev, ctx, deadline)
                        .await?
                }
                None => {
                    return Err(conflict(
                        "auth.login_required",
                        format!(
                            "account `{}` of auth profile `{}` is not connected; run `rivet auth begin {} --account {}`",
                            input.account, p.name, p.name, input.account
                        ),
                    ));
                }
            },
        };
        check_scopes(&rec)?;
        Ok(Self::lease(&key, &input.origin, &rec))
    }

    fn invalidate(&self, lease: &CredentialLease) {
        lock(&self.revoked).insert(digest_hex(lease.handle.expose()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MapBackend(Mutex<HashMap<(String, String), Vec<u8>>>);
    impl SecretBackend for MapBackend {
        fn get(&self, s: &str, u: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(lock(&self.0).get(&(s.into(), u.into())).cloned())
        }
        fn set(&self, s: &str, u: &str, b: &[u8]) -> Result<(), String> {
            lock(&self.0).insert((s.into(), u.into()), b.to_vec());
            Ok(())
        }
    }

    #[test]
    fn freshness_uses_capped_skew_and_revocation() {
        let t = now();
        let mut r = Stored {
            access_token: Some("a".into()),
            issued_at: t,
            expires_at: Some(t + 3600),
            ..Stored::default()
        };
        let none = HashSet::new();
        assert!(usable(&r, OAuthFlow::ClientCredentials, &none));
        r.expires_at = Some(t + 20); // lifetime 20 s → skew 10 s
        assert!(usable(&r, OAuthFlow::ClientCredentials, &none));
        r.expires_at = Some(t + 5);
        r.issued_at = t - 100; // skew capped at 30 s
        assert!(!usable(&r, OAuthFlow::ClientCredentials, &none));
        r.expires_at = None;
        assert!(!usable(&r, OAuthFlow::ClientCredentials, &none));
        assert!(usable(&r, OAuthFlow::DeviceCode, &none));
        let revoked: HashSet<String> = [digest_hex("a")].into_iter().collect();
        assert!(!usable(&r, OAuthFlow::DeviceCode, &revoked));
    }

    #[test]
    fn token_errors_map_to_the_registry() {
        assert_eq!(
            token_failure(400, Some("invalid_grant")).code,
            "auth.login_required"
        );
        assert_eq!(
            token_failure(400, Some("access_denied")).code,
            "auth.access_denied"
        );
        let e = token_failure(401, Some("invalid_client"));
        assert_eq!(
            (e.code.as_str(), e.exit_code()),
            ("auth.token_endpoint_failed", 5)
        );
        assert_eq!(
            oauth_error(&Value::object([("error", Value::text("<script>"))])),
            None
        );
    }

    #[tokio::test]
    async fn keychain_entries_round_trip_without_plaintext_defaults() {
        let program = CompiledProgram::default();
        let exchange: Arc<ExchangeHttpFn> =
            Arc::new(|_, _, _, _| Box::pin(async { Err(RivetError::internal("no network")) }));
        let backend = Arc::new(MapBackend::default());
        let a = OAuthAdapter::load(&program, exchange)
            .unwrap()
            .with_keychain(backend.clone());
        let mut p = OAuthProfile {
            name: "kc".into(),
            flow: OAuthFlow::DeviceCode,
            issuer: "https://auth.example.com".into(),
            authorization_endpoint: None,
            token_endpoint: "https://auth.example.com/token".into(),
            device_endpoint: Some("https://auth.example.com/device".into()),
            client_id: "rivet".into(),
            client_secret: None,
            redirect_uri: None,
            scopes: vec!["a".into()],
            resource_origins: vec!["https://api.example.com:443".into()],
            store: CredentialStoreRef::Keychain("rivet/test".into()),
            client_auth: crate::domain::auth::ClientAuthMethod::None,
            pkce: None,
            audience: None,
            config_hash: "0123456789abcdef".into(),
        };
        let rec = Stored {
            generation: 3,
            version: 1,
            profile_hash: p.config_hash.clone(),
            access_token: Some("CANARY".into()),
            ..Stored::default()
        };
        a.save_entry(&p, "k", &rec).await.unwrap();
        let back = a.load_entry(&p, "k").await.unwrap().unwrap();
        assert_eq!(back.generation, 3);
        assert_eq!(lock(&backend.0).len(), 1);
        // A different profile configuration never reads another profile's entry.
        p.config_hash = "other".into();
        assert!(a.load_entry(&p, "k").await.unwrap().is_none());
    }
}
