use super::ports::{OAuthSessionDriver, PolicyEvaluator};
use super::support::{authorize_auth, authorize_store, check_account, resolve_profile};
use crate::domain::RivetResult;
use crate::domain::auth::{CredentialStatus, CredentialStatusInput};
use crate::domain::policy::AccessVerb;

// vhco:usecase auth.credential_status(input: CredentialStatusInput) -> CredentialStatus needs OAuthSessionDriver, PolicyEvaluator
// vhco:label Credential status
// vhco:about Reports the caller's own account state for one profile (connected, expired or disconnected, granted scopes, expiry, generation) after allow_auth status and allow_credentials read permits; never refreshes, discovers endpoints or returns token material.
// vhco:example input={profile:"crm_user", account:"ada"} => { "profile": "crm_user", "account": "ada", "state": "connected", "scopes": ["contacts.read"], "expires_at": "2026-09-28T13:00:00Z", "generation": 1 }
pub async fn credential_status(
    input: CredentialStatusInput,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn OAuthSessionDriver,
) -> RivetResult<CredentialStatus> {
    // vhco:todo authorize_status -- resolve the profile (unknown → not_found.auth_profile) and validate the account key; require allow_auth PROFILE/ACCOUNT/status and allow_credentials PROFILE/ACCOUNT read (permission.denied otherwise); the driver scopes the entry by the authenticated principal, so another tenant's account of the same name is simply "disconnected" for this caller and its existence is never disclosed
    // vhco:step profile driver.profile -- validated profile from the program snapshot
    let profile = resolve_profile(&input.profile, |n| driver.profile(n))?;
    check_account(&input.account)?;
    // vhco:step status authorize_auth -- allow_auth PROFILE/ACCOUNT/status
    // vhco:error permission_denied -- missing allow_auth status or allow_credentials read => permission.denied returns before the store is read
    authorize_auth(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Status,
    )?;
    // vhco:step read authorize_store -- allow_credentials PROFILE/ACCOUNT read
    authorize_store(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Read,
    )?;
    // vhco:todo read_sanitized_status -- through OAuthSessionDriver.status read only this principal's entry from the profile's store (memory or keychain) and report {profile, account, state: connected|expired|disconnected, scopes, expires_at, generation}; no refresh, no network, no token material; a store read failure is auth.store_failed
    // vhco:step read driver.status -- sanitized state only
    driver.status(input).await
}

#[cfg(test)]
mod tests {
    use super::super::support::fakes::{FakeDriver, ctx, grants};
    use super::*;

    // vhco:test auth.credential_status -- status needs allow_auth status (manage alone is not enough) and returns sanitized JSON without tokens
    #[tokio::test]
    async fn status_needs_status_grant() {
        let d = FakeDriver::default();
        let input = || CredentialStatusInput {
            profile: "crm_user".into(),
            account: "ada".into(),
            context: ctx(),
        };
        let e = credential_status(
            input(),
            &grants(&["crm_user/ada/manage"], &["crm_user/ada"]),
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let s = credential_status(
            input(),
            &grants(&["crm_user/ada/status"], &["crm_user/ada"]),
            &d,
        )
        .await
        .unwrap();
        let j = s.to_json();
        assert_eq!(j["state"], "connected");
        assert_eq!(j["generation"], 1);
        assert!(j.get("access_token").is_none());
    }
}
