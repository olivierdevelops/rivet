use super::ports::{OAuthSessionDriver, PolicyEvaluator};
use super::support::{authorize_auth, authorize_store, check_account, resolve_profile};
use crate::domain::auth::{AuthBeginInput, AuthChallenge, OAuthFlow};
use crate::domain::policy::AccessVerb;
use crate::domain::{RivetError, RivetResult};

// vhco:usecase auth.begin_authorization(input: AuthBeginInput) -> AuthChallenge needs OAuthSessionDriver, PolicyEvaluator
// vhco:label Begin authorization
// vhco:about Starts a principal-bound authorization-code (PKCE S256) or device transaction for one profile/account after allow_auth manage and allow_credentials permits; returns only challenge data (authorization URL or user code), never a verifier, state secret or token, and never opens a browser.
// vhco:example input={profile:"crm_user", account:"ada"} => { "transaction_id": "auth_01", "authorization_url": "https://auth.example.com/authorize?response_type=code&client_id=rivet-desktop&state=…&code_challenge=…&code_challenge_method=S256", "expires_at": "2026-09-28T12:10:00Z" }
pub async fn begin_authorization(
    input: AuthBeginInput,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn OAuthSessionDriver,
) -> RivetResult<AuthChallenge> {
    // vhco:todo validate_profile -- resolve the profile by name (unknown → not_found.auth_profile, 404/exit 4); the account must be 1-128 chars without `/` (validation.auth_account); a client_credentials profile has no interactive flow → validation.auth_flow; then authorize allow_auth PROFILE/ACCOUNT/manage and allow_credentials PROFILE/ACCOUNT write (both permission.denied, 403/exit 3) before any transaction state exists
    // vhco:step profile driver.profile -- the validated, immutable profile from the program snapshot
    let profile = resolve_profile(&input.profile, |n| driver.profile(n))?;
    // vhco:step account check_account -- opaque key inside the caller's principal
    check_account(&input.account)?;
    // vhco:error auth_flow -- begin on a client_credentials profile => validation.auth_flow returns (first use acquires instead)
    if profile.flow == OAuthFlow::ClientCredentials {
        return Err(RivetError::validation(
            "validation.auth_flow",
            format!(
                "profile `{}` uses client_credentials: the first authorized use acquires a token; begin/complete are for authorization_code and device_code",
                profile.name
            ),
        ));
    }
    // vhco:step manage authorize_auth -- allow_auth PROFILE/ACCOUNT/manage
    // vhco:error permission_denied -- missing allow_auth manage or allow_credentials => permission.denied returns before any state or network
    authorize_auth(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Manage,
    )?;
    // vhco:step store authorize_store -- allow_credentials PROFILE/ACCOUNT (the later complete writes the store)
    authorize_store(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Write,
    )?;
    // vhco:todo begin_transaction -- through OAuthSessionDriver.begin: code flow mints a random state and PKCE S256 verifier (kept only in the principal-bound transaction, TTL min(provider expiry, 10 min), at most 8 open per principal) and returns {transaction_id, authorization_url, expires_at} with no network; device flow POSTs the device endpoint (allow_network through the brokered HTTP client, client auth per profile) and returns {transaction_id, verification_uri, user_code, expires_at, interval_seconds}; no browser or listener is ever opened
    // vhco:step begin driver.begin -- create the transaction and return only the challenge
    driver.begin(input, evaluator).await
}

#[cfg(test)]
mod tests {
    use super::super::support::fakes::{FakeDriver, ctx, grants};
    use super::*;
    use crate::domain::auth::AuthBeginInput;

    fn input(profile: &str, account: &str) -> AuthBeginInput {
        AuthBeginInput {
            profile: profile.into(),
            account: account.into(),
            context: ctx(),
        }
    }

    // vhco:test auth.begin_authorization -- needs allow_auth manage and allow_credentials before the driver runs; client_credentials profiles are validation.auth_flow; unknown profiles are not_found
    #[tokio::test]
    async fn begin_checks_flow_and_grants_first() {
        let d = FakeDriver::default();
        let ok = grants(&["crm_user/ada/manage"], &["crm_user/ada"]);
        let c = begin_authorization(input("crm_user", "ada"), &ok, &d)
            .await
            .unwrap();
        assert_eq!(c.transaction_id, "auth_01");
        let e = begin_authorization(input("crm_service", "svc"), &ok, &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation.auth_flow");
        let e = begin_authorization(input("nope", "ada"), &ok, &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "not_found.auth_profile");
        let no_store = grants(&["crm_user/ada/manage"], &[]);
        let e = begin_authorization(input("crm_user", "ada"), &no_store, &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let e = begin_authorization(input("crm_user", "a/b"), &ok, &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation.auth_account");
        assert_eq!(d.calls(), vec!["begin"]);
    }
}
