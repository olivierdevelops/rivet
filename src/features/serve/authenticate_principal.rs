use super::ports::Authenticator;
use crate::domain::contracts::Principal;
use crate::domain::policy::ServeAuth;
use crate::domain::serve::AuthnInput;
use crate::domain::{ErrorKind, RivetError, RivetResult};
use sha2::{Digest, Sha256};

// vhco:usecase serve.authenticate_principal(input: AuthnInput) -> Principal needs Authenticator
// vhco:label Authenticate principal
// vhco:about Turns one incoming HTTP/SSE/poll request, WebSocket upgrade or MCP HTTP request into a Principal using policy.json serve.auth (or a library host's Authenticator).
// vhco:example input={auth:{type:"bearer"}, authorization:"Bearer dev-token-ada"} => { "name": "ada", "authenticated_by": "bearer" }
pub fn authenticate_principal(
    input: &AuthnInput,
    host: Option<&dyn Authenticator>,
) -> RivetResult<Principal> {
    // vhco:todo pick_method -- a library host's Authenticator callback wins when supplied; otherwise serve.auth from policy.json applies; the same function runs for every surface (REST/SSE/poll requests, the WS upgrade and each MCP HTTP request)
    // vhco:step host host.authenticate -- delegate entirely to the host callback when present
    if let Some(h) = host {
        return h.authenticate(input);
    }
    match &input.auth {
        // vhco:todo none_loopback_only -- auth none yields principal `local` (authenticated_by none) only on a loopback bind; start_serve already refused non-loopback binds, so a non-loopback here is auth.required (401)
        // vhco:step none return -- loopback → Principal{local, none}
        ServeAuth::None => {
            if input.bind_is_loopback {
                Ok(Principal::local())
            } else {
                Err(unauthenticated(
                    "auth.required",
                    "this listener requires authentication",
                ))
            }
        }
        // vhco:todo bearer -- read `Authorization: Bearer TOKEN` (scheme case-insensitive), sha256 the token to lowercase hex and compare in constant time against every tokens[].sha256; a match yields that principal (authenticated_by bearer); a missing header is auth.required and an unmatched token auth.invalid, both 401 with no hint about which tokens exist; the raw token is never logged
        // vhco:error missing_token -- no Authorization header => auth.required (401) returns
        // vhco:error bad_token -- token hash matches no entry => auth.invalid (401) returns
        ServeAuth::Bearer(tokens) => {
            // vhco:step parse header -- `Bearer <token>`
            let token = input
                .authorization
                .as_deref()
                .and_then(bearer_token)
                .ok_or_else(|| unauthenticated("auth.required", "missing bearer token"))?;
            // vhco:step hash sha256 -- hex digest of the presented token
            let digest = hex(&Sha256::digest(token.as_bytes()));
            // vhco:step compare constant_time_eq -- check every entry without early exit
            let mut found: Option<&str> = None;
            for t in tokens {
                if constant_time_eq(t.sha256.to_ascii_lowercase().as_bytes(), digest.as_bytes()) {
                    found = Some(t.principal.as_str());
                }
            }
            found
                .map(|name| Principal {
                    name: name.to_string(),
                    authenticated_by: "bearer".into(),
                })
                .ok_or_else(|| unauthenticated("auth.invalid", "invalid bearer token"))
        }
        // vhco:todo mtls -- auth mtls requires a verified client certificate subject (the TLS listener verifies the chain against client_ca) mapped exactly to principals[].subject; no subject or no mapping is auth.required/auth.invalid (401)
        // vhco:error no_cert -- no verified client certificate subject => auth.required (401) returns
        ServeAuth::Mtls { principals, .. } => {
            let subject = input.client_cert_subject.as_deref().ok_or_else(|| {
                unauthenticated("auth.required", "a client certificate is required")
            })?;
            principals
                .iter()
                .find(|p| p.subject == subject)
                .map(|p| Principal {
                    name: p.principal.clone(),
                    authenticated_by: "mtls".into(),
                })
                .ok_or_else(|| {
                    unauthenticated(
                        "auth.invalid",
                        "client certificate is not mapped to a principal",
                    )
                })
        }
    }
}

fn unauthenticated(code: &str, message: &str) -> RivetError {
    RivetError::new(ErrorKind::Auth, code, message)
}

fn bearer_token(header: &str) -> Option<&str> {
    let (scheme, rest) = header.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let t = rest.trim();
    (!t.is_empty()).then_some(t)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::BearerTokenHash;

    fn input(auth: ServeAuth, header: Option<&str>, loopback: bool) -> AuthnInput {
        AuthnInput {
            surface: "http".into(),
            remote_addr: "127.0.0.1:5555".into(),
            bind_is_loopback: loopback,
            authorization: header.map(String::from),
            client_cert_subject: None,
            auth,
        }
    }

    // vhco:test serve.authenticate_principal -- none is local on loopback only; bearer matches the demo token hashes and rejects missing/wrong tokens with 401
    #[test]
    fn none_and_bearer() {
        assert_eq!(
            authenticate_principal(&input(ServeAuth::None, None, true), None).unwrap(),
            Principal::local()
        );
        assert_eq!(
            authenticate_principal(&input(ServeAuth::None, None, false), None)
                .unwrap_err()
                .http_status(),
            401
        );
        // docs/demos/01-catalog/policies/team.json fixture hashes
        let team = ServeAuth::Bearer(vec![
            BearerTokenHash {
                principal: "ada".into(),
                sha256: "bf7e9889975e8d9483fed456e9651d9c73470c82857bc3e6384222a1d5f7f9c6".into(),
            },
            BearerTokenHash {
                principal: "ci".into(),
                sha256: "a6c6871b8f3568d985f17c7458aff2582992a5007442b589ed5af28854bb8501".into(),
            },
        ]);
        let p = authenticate_principal(
            &input(team.clone(), Some("Bearer dev-token-ada"), false),
            None,
        )
        .unwrap();
        assert_eq!(p.name, "ada");
        let p = authenticate_principal(
            &input(team.clone(), Some("bearer dev-token-ci"), true),
            None,
        )
        .unwrap();
        assert_eq!(p.name, "ci");
        assert_eq!(
            authenticate_principal(&input(team.clone(), None, true), None)
                .unwrap_err()
                .code,
            "auth.required"
        );
        assert_eq!(
            authenticate_principal(&input(team, Some("Bearer nope"), true), None)
                .unwrap_err()
                .code,
            "auth.invalid"
        );
    }
}
