use super::ports::{OAuthSessionDriver, PolicyEvaluator};
use super::support::authorize_auth;
use crate::domain::auth::{AuthCancelInput, AuthCancelReceipt};
use crate::domain::policy::AccessVerb;
use crate::domain::{RivetError, RivetResult};

// vhco:usecase auth.cancel_authorization(input: AuthCancelInput) -> AuthCancelReceipt needs OAuthSessionDriver, PolicyEvaluator
// vhco:label Cancel a pending authorization
// vhco:about Invalidates one of the caller's open authorization transactions (state, verifier and device code are discarded, the provider is never contacted); idempotent, and a transaction that already ended reports its terminal state unchanged.
// vhco:example input={transaction_id:"auth_02"} => { "transaction_id": "auth_02", "state": "cancelled" }
pub async fn cancel_authorization(
    input: AuthCancelInput,
    evaluator: &dyn PolicyEvaluator,
    driver: &dyn OAuthSessionDriver,
) -> RivetResult<AuthCancelReceipt> {
    // vhco:todo authorize_cancel -- resolve the transaction for the authenticated principal through OAuthSessionDriver.transaction; unknown ids and other principals' transactions are both not_found.auth_transaction (404/exit 4) without disclosing existence; then require allow_auth PROFILE/ACCOUNT/manage for the transaction's profile/account (permission.denied otherwise)
    // vhco:step lookup driver.transaction -- principal-bound view (no secrets)
    // vhco:error unknown_transaction -- unknown or foreign transaction => not_found.auth_transaction returns
    let tx = driver
        .transaction(&input.transaction_id, &input.context.principal)
        .ok_or_else(|| {
            RivetError::not_found(
                "not_found.auth_transaction",
                format!("no authorization transaction `{}`", input.transaction_id),
            )
        })?;
    // vhco:step manage authorize_auth -- allow_auth PROFILE/ACCOUNT/manage
    authorize_auth(
        evaluator,
        &input.context,
        &tx.profile,
        &tx.account,
        AccessVerb::Manage,
    )?;
    // vhco:todo invalidate_transaction -- OAuthSessionDriver.cancel marks an open transaction cancelled and drops its state, PKCE verifier and device code so a later rivet.auth.complete fails (auth.callback_invalid) and a device poll waiting in this runtime stops at its next check; the provider is never contacted; returns AuthCancelReceipt{transaction_id, state:"cancelled"}
    // vhco:todo idempotent_terminal -- cancelling a cancelled transaction returns the same receipt; one that already ended returns {transaction_id, state: connected|denied|failed|expired} unchanged and stored credentials are untouched (disconnect removes those); no token, state or verifier appears in the receipt or trace
    // vhco:step cancel driver.cancel -- discard secrets, report the (terminal) state
    driver.cancel(input).await
}

#[cfg(test)]
mod tests {
    use super::super::support::fakes::{FakeDriver, ctx, grants};
    use super::*;

    // vhco:test auth.cancel_authorization -- unknown transactions are not_found; the owner's transaction is cancelled after the manage grant
    #[tokio::test]
    async fn cancel_is_principal_bound() {
        let d = FakeDriver::default();
        let ok = grants(&["crm_user/ada/manage"], &[]);
        let e = cancel_authorization(
            AuthCancelInput {
                transaction_id: "auth_99".into(),
                context: ctx(),
            },
            &ok,
            &d,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "not_found.auth_transaction");
        let r = cancel_authorization(
            AuthCancelInput {
                transaction_id: "auth_01".into(),
                context: ctx(),
            },
            &ok,
            &d,
        )
        .await
        .unwrap();
        assert_eq!(r.state, "cancelled");
    }
}
