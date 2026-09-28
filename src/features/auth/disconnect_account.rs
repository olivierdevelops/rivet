use super::ports::{OAuthSessionDriver, PolicyEvaluator};
use super::support::{authorize_auth, authorize_store, check_account, resolve_profile};
use crate::domain::RivetResult;
use crate::domain::auth::{DisconnectInput, DisconnectReceipt};
use crate::domain::policy::AccessVerb;

// vhco:usecase auth.disconnect_account(input: DisconnectInput) -> DisconnectReceipt needs OAuthSessionDriver, PolicyEvaluator
// vhco:label Disconnect account
// vhco:about Forgets the caller's local credentials for one profile/account: cancels its open transactions, removes stored tokens, advances the generation so no new lease uses the old one, and reports local_only true (no provider revocation is claimed).
// vhco:example input={profile:"crm_user", account:"ada"} => { "profile": "crm_user", "account": "ada", "local_only": true, "generation": 2 }
pub async fn disconnect_account(
    input: DisconnectInput,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn OAuthSessionDriver,
) -> RivetResult<DisconnectReceipt> {
    // vhco:todo authorize_disconnect -- resolve the profile (unknown → not_found.auth_profile), validate the account key and require allow_auth PROFILE/ACCOUNT/manage plus allow_credentials PROFILE/ACCOUNT write (permission.denied otherwise); the driver then cancels every open authorization transaction of this principal/profile/account and bumps the account generation so leases of the old generation are never issued again
    // vhco:step profile driver.profile -- validated profile
    let profile = resolve_profile(&input.profile, |n| driver.profile(n))?;
    check_account(&input.account)?;
    // vhco:step manage authorize_auth -- allow_auth PROFILE/ACCOUNT/manage
    // vhco:error permission_denied -- missing manage or credential-store grant => permission.denied returns; nothing is deleted
    authorize_auth(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Manage,
    )?;
    // vhco:step store authorize_store -- allow_credentials PROFILE/ACCOUNT write (the store entry is replaced by a token-free tombstone)
    authorize_store(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Write,
    )?;
    // vhco:todo forget_local_credentials -- through OAuthSessionDriver.disconnect replace the stored entry with a token-free record carrying generation+1 (idempotent when nothing was stored), and return {profile, account, local_only:true, generation}; if the durable store write fails the call fails with auth.store_failed (502/exit 5) rather than reporting success
    // vhco:step forget driver.disconnect -- cancel transactions, drop tokens, advance generation
    driver.disconnect(input, evaluator).await
}

#[cfg(test)]
mod tests {
    use super::super::support::fakes::{FakeDriver, ctx, grants};
    use super::*;

    // vhco:test auth.disconnect_account -- disconnect needs manage + credentials and reports local_only with the new generation
    #[tokio::test]
    async fn disconnect_is_local_only() {
        let d = FakeDriver::default();
        let input = || DisconnectInput {
            profile: "crm_user".into(),
            account: "ada".into(),
            context: ctx(),
        };
        let e = disconnect_account(
            input(),
            &grants(&["crm_user/ada/status"], &["crm_user/ada"]),
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let r = disconnect_account(
            input(),
            &grants(&["crm_user/ada/manage"], &["crm_user/ada"]),
            &d,
        )
        .await
        .unwrap();
        assert_eq!(
            r.to_json(),
            serde_json::json!({"profile":"crm_user","account":"ada","local_only":true,"generation":2})
        );
    }
}
