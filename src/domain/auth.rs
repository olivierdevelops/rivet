//! OAuth 2.0 vocabulary (PROP-2026-0001 Increment 9): immutable profiles,
//! authorization transactions, sanitized credential status and opaque leases.
//!
//! ```text
//!  auth NAME oauth2 … end ──from_declaration──▶ OAuthProfile (validated, config_hash pinned)
//!
//!  rivet.auth.begin    AuthBeginInput      ──▶ AuthChallenge      {transaction_id, authorization_url | user_code…}
//!  rivet.auth.complete AuthCompleteInput   ──▶ CredentialStatus   {state: connected | pending}
//!  rivet.auth.status   CredentialStatusInput ─▶ CredentialStatus  (no refresh, no network)
//!  rivet.auth.disconnect DisconnectInput   ──▶ DisconnectReceipt  {local_only: true, generation+1}
//!  rivet.auth.cancel   AuthCancelInput     ──▶ AuthCancelReceipt  {state: cancelled | <terminal>}
//!  http/grpc `auth P account A` CredentialInput ─▶ CredentialLease (opaque; adapter-only)
//! ```
//!
//! Secrets (client secret, codes, state, verifier, device code, tokens) are
//! held in [`SecretString`], whose `Debug`/`Display` never print the value.

use super::contracts::Principal;
use super::ir::{Arg, Declaration, Expr, OptionLine};
use super::value::Value;
use super::{RivetError, RivetResult};
use serde_json::{Value as Json, json};

// vhco:domain SecretString { redacted: string }
/// A secret value. Formatting prints `[redacted]`; only `expose` reveals it.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: impl Into<String>) -> SecretString {
        SecretString(s.into())
    }

    /// The raw secret, for the one place that must send it (a header or form field).
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

impl std::fmt::Display for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

/// The opaque bearer handle an adapter attaches; never returned to scripts.
pub type OpaqueSecretHandle = SecretString;

// vhco:domain OAuthFlow { client_credentials | authorization_code | device_code }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OAuthFlow {
    ClientCredentials,
    AuthorizationCode,
    DeviceCode,
}

impl OAuthFlow {
    pub fn as_str(self) -> &'static str {
        match self {
            OAuthFlow::ClientCredentials => "client_credentials",
            OAuthFlow::AuthorizationCode => "authorization_code",
            OAuthFlow::DeviceCode => "device_code",
        }
    }
}

// vhco:domain ClientAuthMethod { basic | post | none }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientAuthMethod {
    /// `Authorization: Basic base64(urlencode(id):urlencode(secret))` (RFC 6749 §2.3.1).
    Basic,
    /// `client_id` + `client_secret` form fields.
    Post,
    /// Public client: `client_id` form field only.
    None,
}

// vhco:domain PkceMethod { s256 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PkceMethod {
    S256,
}

// vhco:domain SecretRef { env: string }
/// Where a client secret comes from (read only after an allow_env permit).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SecretRef {
    Env(String),
}

// vhco:domain CredentialStoreRef { memory | keychain: string }
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CredentialStoreRef {
    /// Process-local; lost at exit.
    Memory,
    /// Host secure store (macOS Keychain, Windows Credential Manager, Secret Service) under a namespace.
    Keychain(String),
}

// vhco:domain OAuthProfile { name: string; flow: OAuthFlow; issuer: string; authorization_endpoint?: string; token_endpoint: string; device_endpoint?: string; client_id: string; client_secret?: SecretRef; redirect_uri?: string; scopes: string[]; resource_origins: string[]; store: CredentialStoreRef; client_auth: ClientAuthMethod; pkce?: PkceMethod; audience?: string; config_hash: string }
/// One immutable `auth NAME oauth2 … end` profile after validation.
#[derive(Clone, Debug, PartialEq)]
pub struct OAuthProfile {
    pub name: String,
    pub flow: OAuthFlow,
    pub issuer: String,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: String,
    pub device_endpoint: Option<String>,
    pub client_id: String,
    pub client_secret: Option<SecretRef>,
    pub redirect_uri: Option<String>,
    pub scopes: Vec<String>,
    /// Exact `scheme://host:port` origins (default port explicit).
    pub resource_origins: Vec<String>,
    pub store: CredentialStoreRef,
    pub client_auth: ClientAuthMethod,
    pub pkce: Option<PkceMethod>,
    pub audience: Option<String>,
    /// sha256 over the canonical configuration: the profile identity in cache keys.
    pub config_hash: String,
}

fn profile_err(name: &str, line: Option<&OptionLine>, msg: impl Into<String>) -> RivetError {
    RivetError::validation(
        "validation.auth_profile",
        format!("auth profile `{name}`: {}", msg.into()),
    )
    .with_span(line.map(|l| l.span.clone()))
}

fn arg_text(a: &Arg) -> Option<String> {
    match a {
        Arg::Word(w, _) => Some(w.clone()),
        Arg::Expr(e, _) => e.const_text(),
    }
}

fn arg_list(a: &Arg) -> Option<Vec<String>> {
    match a {
        Arg::Expr(Expr::List(items), _) => items.iter().map(Expr::const_text).collect(),
        Arg::Expr(Expr::Lit(Value::List(items)), _) => items
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => None,
    }
}

/// `scheme://host:port` with the default port explicit; `None` if not an http(s) URL.
pub fn origin_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    let host = match u.host()? {
        url::Host::Ipv6(a) => format!("[{a}]"),
        h => h.to_string().to_ascii_lowercase(),
    };
    Some(format!(
        "{}://{host}:{}",
        u.scheme(),
        u.port_or_known_default()?
    ))
}

/// Loopback hosts may use plain http (RFC 8252 §7.3); everything else needs https.
fn secure_enough(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else {
        return false;
    };
    match u.scheme() {
        "https" => true,
        "http" => match u.host() {
            Some(url::Host::Ipv4(a)) => a.is_loopback(),
            Some(url::Host::Ipv6(a)) => a.is_loopback(),
            Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
            None => false,
        },
        _ => false,
    }
}

impl OAuthProfile {
    /// Validate one lowered `auth NAME oauth2` declaration (the REF auth
    /// profile option rules). Unsupported flows/methods are refused explicitly.
    pub fn from_declaration(d: &Declaration) -> RivetResult<OAuthProfile> {
        let name = d.name.as_str();
        let one =
            |key: &str| -> RivetResult<Option<String>> {
                match d.option(key) {
                    None => Ok(None),
                    Some(l) => l.args.first().and_then(arg_text).map(Some).ok_or_else(|| {
                        profile_err(name, Some(l), format!("`{key}` needs a value"))
                    }),
                }
            };
        let list = |key: &str| -> RivetResult<Option<Vec<String>>> {
            match d.option(key) {
                None => Ok(None),
                Some(l) => l.args.first().and_then(arg_list).map(Some).ok_or_else(|| {
                    profile_err(name, Some(l), format!("`{key}` needs a list of strings"))
                }),
            }
        };
        const KNOWN: [&str; 15] = [
            "flow",
            "pkce",
            "issuer",
            "authorization_url",
            "device_url",
            "token_url",
            "client_id",
            "client_secret",
            "client_auth",
            "redirect_uri",
            "scopes",
            "resource_origins",
            "store",
            "audience",
            "description",
        ];
        for o in &d.options {
            if !KNOWN.contains(&o.key.as_str()) {
                return Err(profile_err(
                    name,
                    Some(o),
                    format!("`{}` is not an oauth2 profile option", o.key),
                ));
            }
        }
        let flow_line = d.option("flow");
        let flow = match one("flow")?.as_deref() {
            Some("client_credentials") => OAuthFlow::ClientCredentials,
            Some("authorization_code") => OAuthFlow::AuthorizationCode,
            Some("device_code") => OAuthFlow::DeviceCode,
            Some(f @ ("password" | "implicit")) => {
                return Err(RivetError::unsupported(
                    "unsupported.auth_flow",
                    format!("auth profile `{name}`: the `{f}` flow is refused (RFC 9700)"),
                )
                .with_span(flow_line.map(|l| l.span.clone())));
            }
            Some(other) => {
                return Err(profile_err(
                    name,
                    flow_line,
                    format!(
                        "unknown flow `{other}` (client_credentials, authorization_code, device_code)"
                    ),
                ));
            }
            None => return Err(profile_err(name, None, "`flow` is required")),
        };
        let client_auth = match one("client_auth")?.as_deref() {
            Some("basic") => ClientAuthMethod::Basic,
            Some("post") => ClientAuthMethod::Post,
            Some("none") => ClientAuthMethod::None,
            Some(other) => {
                return Err(RivetError::unsupported(
                    "unsupported.auth_method",
                    format!("auth profile `{name}`: client_auth `{other}` is not supported (basic, post, none)"),
                )
                .with_span(d.option("client_auth").map(|l| l.span.clone())));
            }
            None => {
                return Err(profile_err(
                    name,
                    None,
                    "`client_auth basic|post|none` is required",
                ));
            }
        };
        let pkce = match one("pkce")?.as_deref() {
            None => None,
            Some("s256") | Some("S256") => Some(PkceMethod::S256),
            Some(other) => {
                return Err(profile_err(
                    name,
                    d.option("pkce"),
                    format!("pkce `{other}` is refused; only `pkce s256` is supported"),
                ));
            }
        };
        let client_secret = match d.option("client_secret") {
            None => None,
            Some(l) => match (
                l.args.first().and_then(Arg::word),
                l.args.get(1).and_then(arg_text),
            ) {
                (Some("env"), Some(var)) if !var.is_empty() => Some(SecretRef::Env(var)),
                _ => {
                    return Err(profile_err(
                        name,
                        Some(l),
                        "expected `client_secret env \"VAR\"` (secrets never appear literally in source)",
                    ));
                }
            },
        };
        let store = match d.option("store") {
            None => {
                return Err(profile_err(
                    name,
                    None,
                    "`store memory|keychain \"NS\"` is required",
                ));
            }
            Some(l) => match (
                l.args.first().and_then(Arg::word),
                l.args.get(1).and_then(arg_text),
            ) {
                (Some("memory"), None) => CredentialStoreRef::Memory,
                (Some("keychain"), Some(ns)) if !ns.is_empty() => CredentialStoreRef::Keychain(ns),
                _ => {
                    return Err(profile_err(
                        name,
                        Some(l),
                        "expected `store memory` or `store keychain \"NAMESPACE\"`",
                    ));
                }
            },
        };
        let issuer = one("issuer")?;
        let token_endpoint = one("token_url")?;
        let authorization_endpoint = one("authorization_url")?;
        let device_endpoint = one("device_url")?;
        let redirect_uri = one("redirect_uri")?;
        let client_id = one("client_id")?;
        let scopes = list("scopes")?;
        let origins = list("resource_origins")?;
        let audience = one("audience")?;

        let need = |v: &Option<String>, key: &str| -> RivetResult<String> {
            v.clone().ok_or_else(|| {
                profile_err(
                    name,
                    None,
                    format!("`{key}` is required for flow {}", flow.as_str()),
                )
            })
        };
        let issuer = need(&issuer, "issuer")?;
        let token_endpoint = need(&token_endpoint, "token_url")?;
        let client_id = need(&client_id, "client_id")?;
        let scopes = scopes.ok_or_else(|| profile_err(name, None, "`scopes [...]` is required"))?;
        let origins = origins
            .ok_or_else(|| profile_err(name, None, "`resource_origins [...]` is required"))?;
        match flow {
            OAuthFlow::AuthorizationCode => {
                need(&authorization_endpoint, "authorization_url")?;
                need(&redirect_uri, "redirect_uri")?;
                if pkce.is_none() {
                    return Err(profile_err(
                        name,
                        flow_line,
                        "flow authorization_code requires `pkce s256`",
                    ));
                }
            }
            OAuthFlow::DeviceCode => {
                need(&device_endpoint, "device_url")?;
            }
            OAuthFlow::ClientCredentials => {
                if client_auth == ClientAuthMethod::None || client_secret.is_none() {
                    return Err(profile_err(
                        name,
                        d.option("client_auth"),
                        "flow client_credentials needs a confidential client (`client_auth basic|post` and `client_secret env \"VAR\"`)",
                    ));
                }
            }
        }
        if client_auth != ClientAuthMethod::None && client_secret.is_none() {
            return Err(profile_err(
                name,
                d.option("client_auth"),
                "client_auth basic|post needs `client_secret env \"VAR\"`",
            ));
        }
        for (key, u) in [
            ("issuer", Some(&issuer)),
            ("token_url", Some(&token_endpoint)),
            ("authorization_url", authorization_endpoint.as_ref()),
            ("device_url", device_endpoint.as_ref()),
        ] {
            if let Some(u) = u
                && !secure_enough(u)
            {
                return Err(profile_err(
                    name,
                    d.option(key),
                    format!("`{key}` must be an https:// URL (plain http only on loopback)"),
                ));
            }
        }
        let mut resource_origins = Vec::new();
        for o in &origins {
            let norm = origin_of(o).filter(|n| {
                url::Url::parse(o)
                    .map(|u| u.path() == "/" || u.path().is_empty())
                    .unwrap_or(false)
                    && !n.is_empty()
            });
            match norm {
                Some(n) => resource_origins.push(n),
                None => {
                    return Err(profile_err(
                        name,
                        d.option("resource_origins"),
                        format!("resource origin `{o}` must be scheme://host:port"),
                    ));
                }
            }
        }
        if resource_origins.is_empty() || scopes.is_empty() {
            return Err(profile_err(
                name,
                None,
                "resource_origins and scopes must each name at least one entry",
            ));
        }
        let canonical = json!({
            "name": name, "flow": flow.as_str(), "issuer": issuer,
            "authorization_url": authorization_endpoint, "device_url": device_endpoint,
            "token_url": token_endpoint, "client_id": client_id,
            "client_secret": client_secret.as_ref().map(|SecretRef::Env(v)| format!("env:{v}")),
            "client_auth": format!("{client_auth:?}"), "redirect_uri": redirect_uri,
            "scopes": scopes, "resource_origins": resource_origins,
            "store": format!("{store:?}"), "pkce": pkce.map(|_| "S256"), "audience": audience,
        });
        let config_hash = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(canonical.to_string().as_bytes());
            digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        Ok(OAuthProfile {
            name: name.to_string(),
            flow,
            issuer,
            authorization_endpoint,
            token_endpoint,
            device_endpoint,
            client_id,
            client_secret,
            redirect_uri,
            scopes,
            resource_origins,
            store,
            client_auth,
            pkce,
            audience,
            config_hash,
        })
    }
}

// vhco:domain AuthContext { principal: Principal; operation_id: string; deadline_ms: int }
/// Who is asking and how long they may wait (the request context the auth
/// use cases need).
#[derive(Clone, Debug, PartialEq)]
pub struct AuthContext {
    pub principal: Principal,
    /// The operation on whose behalf effects are authorized (`rivet.auth.*` for built-ins).
    pub operation_id: String,
    /// Remaining request budget in milliseconds.
    pub deadline_ms: u64,
}

// vhco:domain AuthBeginInput { profile: string; account: string; context: AuthContext }
#[derive(Clone, Debug, PartialEq)]
pub struct AuthBeginInput {
    pub profile: String,
    pub account: String,
    pub context: AuthContext,
}

// vhco:domain AuthChallenge { transaction_id: string; authorization_url?: string; verification_uri?: string; user_code?: string; expires_at: string; interval_seconds?: int }
/// Challenge data returned deliberately to the authorized initiator.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthChallenge {
    pub transaction_id: String,
    pub authorization_url: Option<String>,
    pub verification_uri: Option<String>,
    pub verification_uri_complete: Option<String>,
    pub user_code: Option<String>,
    /// RFC 3339 UTC.
    pub expires_at: String,
    pub interval_seconds: Option<u32>,
}

impl AuthChallenge {
    pub fn to_json(&self) -> Json {
        let mut j = json!({"transaction_id": self.transaction_id, "expires_at": self.expires_at});
        if let Some(u) = &self.authorization_url {
            j["authorization_url"] = json!(u);
        }
        if let Some(u) = &self.verification_uri {
            j["verification_uri"] = json!(u);
        }
        if let Some(u) = &self.verification_uri_complete {
            j["verification_uri_complete"] = json!(u);
        }
        if let Some(c) = &self.user_code {
            j["user_code"] = json!(c);
        }
        if let Some(i) = self.interval_seconds {
            j["interval_seconds"] = json!(i);
        }
        j
    }
}

// vhco:domain SecretCallback { code: SecretString; state: SecretString; redirect_uri: string; issuer?: string; error?: string }
/// The host-collected redirect parameters (never from argv).
#[derive(Clone, Debug, PartialEq)]
pub struct SecretCallback {
    pub code: SecretString,
    pub state: SecretString,
    pub redirect_uri: String,
    pub issuer: Option<String>,
    /// Provider `error` (e.g. access_denied) instead of a code.
    pub error: Option<String>,
}

// vhco:domain AuthCompleteInput { transaction_id: string; callback?: SecretCallback; wait: bool; context: AuthContext }
#[derive(Clone, Debug, PartialEq)]
pub struct AuthCompleteInput {
    pub transaction_id: String,
    pub callback: Option<SecretCallback>,
    pub wait: bool,
    pub context: AuthContext,
}

// vhco:domain CredentialStatusInput { profile: string; account: string; context: AuthContext }
#[derive(Clone, Debug, PartialEq)]
pub struct CredentialStatusInput {
    pub profile: String,
    pub account: String,
    pub context: AuthContext,
}

// vhco:domain CredentialState { connected | disconnected | expired | pending }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialState {
    Connected,
    Disconnected,
    /// Access token expired; the next use refreshes or reacquires.
    Expired,
    /// `complete` hit the request deadline before the provider answered.
    Pending,
}

impl CredentialState {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialState::Connected => "connected",
            CredentialState::Disconnected => "disconnected",
            CredentialState::Expired => "expired",
            CredentialState::Pending => "pending",
        }
    }
}

// vhco:domain CredentialStatus { profile: string; account: string; state: CredentialState; scopes: string[]; expires_at?: string; generation: int; transaction_id?: string }
/// Sanitized account state: never token material.
#[derive(Clone, Debug, PartialEq)]
pub struct CredentialStatus {
    pub profile: String,
    pub account: String,
    pub state: CredentialState,
    pub scopes: Vec<String>,
    pub expires_at: Option<String>,
    pub generation: u64,
    /// Set only for `pending`: the transaction to complete again.
    pub transaction_id: Option<String>,
}

impl CredentialStatus {
    pub fn to_json(&self) -> Json {
        if self.state == CredentialState::Pending {
            return json!({
                "state": "pending",
                "transaction_id": self.transaction_id,
                "expires_at": self.expires_at,
            });
        }
        json!({
            "profile": self.profile,
            "account": self.account,
            "state": self.state.as_str(),
            "scopes": self.scopes,
            "expires_at": self.expires_at,
            "generation": self.generation,
        })
    }
}

// vhco:domain DisconnectInput { profile: string; account: string; context: AuthContext }
#[derive(Clone, Debug, PartialEq)]
pub struct DisconnectInput {
    pub profile: String,
    pub account: String,
    pub context: AuthContext,
}

// vhco:domain DisconnectReceipt { profile: string; account: string; local_only: bool; generation: int }
#[derive(Clone, Debug, PartialEq)]
pub struct DisconnectReceipt {
    pub profile: String,
    pub account: String,
    pub local_only: bool,
    pub generation: u64,
}

impl DisconnectReceipt {
    pub fn to_json(&self) -> Json {
        json!({"profile": self.profile, "account": self.account, "local_only": self.local_only, "generation": self.generation})
    }
}

// vhco:domain AuthCancelInput { transaction_id: string; context: AuthContext }
#[derive(Clone, Debug, PartialEq)]
pub struct AuthCancelInput {
    pub transaction_id: String,
    pub context: AuthContext,
}

// vhco:domain AuthCancelReceipt { transaction_id: string; state: string }
#[derive(Clone, Debug, PartialEq)]
pub struct AuthCancelReceipt {
    pub transaction_id: String,
    pub state: String,
}

impl AuthCancelReceipt {
    pub fn to_json(&self) -> Json {
        json!({"transaction_id": self.transaction_id, "state": self.state})
    }
}

// vhco:domain AuthTransactionInfo { transaction_id: string; profile: string; account: string; state: string }
/// What the use cases may know about a transaction (no secrets).
#[derive(Clone, Debug, PartialEq)]
pub struct AuthTransactionInfo {
    pub transaction_id: String,
    pub profile: String,
    pub account: String,
    /// open | connected | failed | cancelled | expired
    pub state: String,
}

// vhco:domain CredentialInput { profile: string; account: string; origin: string; audience?: string; scopes: string[]; context: AuthContext }
/// One attachment request from a transport adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct CredentialInput {
    pub profile: String,
    pub account: String,
    /// `scheme://host:port` of the resource the token will be sent to.
    pub origin: String,
    pub audience: Option<String>,
    /// Required scopes; empty = the profile's configured scopes.
    pub scopes: Vec<String>,
    pub context: AuthContext,
}

// vhco:domain CredentialLease { handle: OpaqueSecretHandle; scope_id: string; origin: string; generation: int; expires_at?: string }
/// An adapter-only bearer lease bound to one origin.
#[derive(Clone, Debug, PartialEq)]
pub struct CredentialLease {
    pub handle: OpaqueSecretHandle,
    /// Cache identity (principal/profile hash/account), used to invalidate after a 401.
    pub scope_id: String,
    pub origin: String,
    pub generation: u64,
    pub expires_at: Option<String>,
}

/// `YYYY-MM-DDTHH:MM:SSZ` for seconds since the Unix epoch (proleptic Gregorian, UTC).
pub fn rfc3339(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_formats_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_790_596_200), "2026-09-28T11:50:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn secrets_never_format() {
        let s = SecretString::new("tok-123");
        assert_eq!(format!("{s:?} {s}"), "[redacted] [redacted]");
        assert_eq!(s.expose(), "tok-123");
    }

    #[test]
    fn origins_make_the_default_port_explicit() {
        assert_eq!(
            origin_of("https://API.example.com/x").as_deref(),
            Some("https://api.example.com:443")
        );
        assert_eq!(
            origin_of("http://127.0.0.1:8080").as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(origin_of("quic://a:1"), None);
    }
}
