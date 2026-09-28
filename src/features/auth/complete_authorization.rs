use super::ports::{OAuthSessionDriver, PolicyEvaluator};
use super::support::{authorize_auth, authorize_store, resolve_profile};
use crate::domain::auth::{AuthCompleteInput, CredentialStatus, OAuthFlow};
use crate::domain::policy::AccessVerb;
use crate::domain::{RivetError, RivetResult};

// vhco:usecase auth.complete_authorization(input: AuthCompleteInput) -> CredentialStatus needs OAuthSessionDriver, PolicyEvaluator
// vhco:label Complete authorization
// vhco:about Completes the caller's own authorization transaction: a code callback is checked (state, redirect URI, issuer, TTL, single use) before the PKCE code exchange; a device transaction polls the token endpoint under the request deadline and answers {"state":"pending"} without consuming the transaction when the deadline comes first. Returns sanitized CredentialStatus, never tokens.
// vhco:example input={transaction_id:"auth_01", callback:{code:"…", state:"…", redirect_uri:"https://app.example.com/oauth/callback", issuer:"https://auth.example.com"}} => { "profile": "crm_user", "account": "ada", "state": "connected", "scopes": ["contacts.read"], "expires_at": "2026-09-28T13:00:00Z", "generation": 1 }
pub async fn complete_authorization(
    input: AuthCompleteInput,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn OAuthSessionDriver,
) -> RivetResult<CredentialStatus> {
    // vhco:todo validate_transaction -- the transaction must belong to the calling principal (unknown or foreign → not_found.auth_transaction without disclosure); its profile/account need allow_auth manage and allow_credentials write; a code-flow transaction needs a `callback` object and a device-flow transaction must not carry one (validation.auth_callback); the driver then checks expiry (auth.transaction_expired), replay/terminal state, exact redirect URI, the RFC 9207 issuer when present and the single-use state in constant time (auth.callback_invalid) before any token request; codes and device codes are never logged or echoed
    // vhco:step lookup driver.transaction -- principal-bound view (no secrets)
    // vhco:error unknown_transaction -- unknown id or another principal's transaction => not_found.auth_transaction returns
    let tx = driver
        .transaction(&input.transaction_id, &input.context.principal)
        .ok_or_else(|| {
            RivetError::not_found(
                "not_found.auth_transaction",
                format!("no authorization transaction `{}`", input.transaction_id),
            )
        })?;
    let profile = resolve_profile(&tx.profile, |n| driver.profile(n))?;
    // vhco:step manage authorize_auth -- allow_auth PROFILE/ACCOUNT/manage for the transaction's account
    authorize_auth(
        evaluator,
        &input.context,
        &tx.profile,
        &tx.account,
        AccessVerb::Manage,
    )?;
    // vhco:step store authorize_store -- allow_credentials PROFILE/ACCOUNT write (the connected credential is persisted)
    authorize_store(
        evaluator,
        &input.context,
        &tx.profile,
        &tx.account,
        AccessVerb::Write,
    )?;
    // vhco:step shape guard -- code flow requires the callback; device flow polls with transaction_id/wait only
    // vhco:error callback_shape -- missing callback for a code transaction, or a callback on a device transaction => validation.auth_callback (422/exit 2) returns
    match (profile.flow, &input.callback) {
        (OAuthFlow::AuthorizationCode, None) => {
            return Err(RivetError::validation(
                "validation.auth_callback",
                "an authorization_code transaction completes with {transaction_id, callback:{code, state, redirect_uri, issuer?}} (use --params-file, never argv)",
            ));
        }
        (OAuthFlow::DeviceCode, Some(_)) => {
            return Err(RivetError::validation(
                "validation.auth_callback",
                "a device_code transaction completes with {transaction_id, wait:true}; it takes no callback",
            ));
        }
        _ => {}
    }
    // vhco:todo exchange_and_commit -- through OAuthSessionDriver.complete: exchange the code with its PKCE verifier (grant_type=authorization_code), or poll the device token endpoint no faster than the provider interval (authorization_pending waits, slow_down adds 5 s and carries over to later calls, access_denied → auth.access_denied, expired_token → auth.transaction_expired, transport faults back off); if the request deadline arrives first the result is {"state":"pending", transaction_id, expires_at} and the transaction stays open; on success the token set is written atomically to the authorized store with generation+1, the transaction is consumed and a sanitized CredentialStatus {profile, account, state:"connected", scopes, expires_at, generation} returns; invalid_grant → auth.login_required, store failure → auth.store_failed
    // vhco:step complete driver.complete -- validate callback or poll, exchange, persist, consume; never returns tokens
    driver.complete(input, evaluator).await
}

#[cfg(test)]
mod tests {
    use super::super::support::fakes::{FakeDriver, ctx, grants};
    use super::*;
    use crate::domain::auth::{SecretCallback, SecretString};
    use crate::domain::contracts::Principal;

    fn input(id: &str, callback: bool) -> AuthCompleteInput {
        AuthCompleteInput {
            transaction_id: id.into(),
            callback: callback.then(|| SecretCallback {
                code: SecretString::new("c"),
                state: SecretString::new("s"),
                redirect_uri: "https://app.example.com/cb".into(),
                issuer: None,
                error: None,
            }),
            wait: false,
            context: ctx(),
        }
    }

    // vhco:test auth.complete_authorization -- a foreign or unknown transaction is not_found, a code transaction needs a callback, and manage + credential grants precede the driver
    #[tokio::test]
    async fn complete_is_principal_bound_and_shape_checked() {
        let d = FakeDriver::default();
        let ok = grants(&["crm_user/ada/manage"], &["crm_user/ada"]);
        let e = complete_authorization(input("auth_01", false), &ok, &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation.auth_callback");
        let mut foreign = input("auth_01", true);
        foreign.context.principal = Principal {
            name: "eve".into(),
            authenticated_by: "bearer".into(),
        };
        let e = complete_authorization(foreign, &ok, &d).await.unwrap_err();
        assert_eq!(e.code, "not_found.auth_transaction");
        let e = complete_authorization(input("auth_01", true), &grants(&[], &["crm_user/ada"]), &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let s = complete_authorization(input("auth_01", true), &ok, &d)
            .await
            .unwrap();
        assert_eq!(s.to_json()["state"], "connected");
        assert_eq!(d.calls(), vec!["complete"]);
    }
}
