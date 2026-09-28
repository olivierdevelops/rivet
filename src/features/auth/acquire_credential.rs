use super::ports::{CredentialProvider, PolicyEvaluator};
use super::support::{authorize_auth, authorize_store, check_account, resolve_profile};
use crate::domain::auth::{CredentialInput, CredentialLease, origin_of};
use crate::domain::policy::AccessVerb;
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};

// vhco:usecase auth.acquire_credential(input: CredentialInput) -> CredentialLease needs CredentialProvider, PolicyEvaluator
// vhco:label Acquire credential
// vhco:about Gives a transport adapter an opaque bearer lease for one profile/account, only for an exact resource origin the profile binds and only after allow_auth use and allow_credentials permits; a cached token is reused, otherwise one coordinated acquisition/refresh runs per credential identity. Never starts an interactive flow.
// vhco:example input={profile:"crm_service", account:"service", origin:"https://api.example.com:443"} => { "scope_id": "local/crm_service#…/service", "origin": "https://api.example.com:443", "generation": 1 }
pub async fn acquire_credential(
    input: CredentialInput,
    evaluator: &dyn PolicyEvaluator,
    provider: &dyn CredentialProvider,
) -> RivetResult<CredentialLease> {
    // vhco:todo bind_destination -- resolve the profile (unknown → not_found.auth_profile) and account key; the destination must normalize to exactly one of the profile's resource_origins scheme://host:port (otherwise auth.origin_not_bound, permission 403/exit 3, before any token work); requested scopes must be a subset of the configured scopes and an audience must equal the profile's (validation.auth_scope); then require allow_auth PROFILE/ACCOUNT/use and allow_credentials PROFILE/ACCOUNT read; an account with no login is auth.login_required from the provider, never an interactive flow
    // vhco:step profile provider.profile -- validated profile from the program snapshot
    let profile = resolve_profile(&input.profile, |n| provider.profile(n))?;
    check_account(&input.account)?;
    // vhco:step origin origin_of -- exact scheme://host:port match against resource_origins
    // vhco:error origin_not_bound -- destination origin not listed in resource_origins => auth.origin_not_bound (permission) returns; no token is fetched or attached
    let origin = origin_of(&input.origin).unwrap_or_else(|| input.origin.clone());
    if !profile.resource_origins.contains(&origin) {
        return Err(RivetError::new(
            ErrorKind::Permission,
            "auth.origin_not_bound",
            format!(
                "auth profile `{}` is not bound to {origin}; add it to resource_origins to attach this credential there",
                profile.name
            ),
        )
        .with_details(Value::object([("origin", Value::text(&origin))])));
    }
    // vhco:error scope_expansion -- scopes outside the profile or a different audience => validation.auth_scope returns
    if let Some(s) = input.scopes.iter().find(|s| !profile.scopes.contains(s)) {
        return Err(RivetError::validation(
            "validation.auth_scope",
            format!(
                "scope `{s}` is not configured on auth profile `{}`",
                profile.name
            ),
        ));
    }
    if input.audience.is_some() && input.audience != profile.audience {
        return Err(RivetError::validation(
            "validation.auth_scope",
            format!(
                "audience is not the one configured on auth profile `{}`",
                profile.name
            ),
        ));
    }
    // vhco:step use authorize_auth -- allow_auth PROFILE/ACCOUNT/use (rechecked on every use, cached or not)
    // vhco:error permission_denied -- missing allow_auth use or allow_credentials => permission.denied returns before the store or token endpoint is touched
    authorize_auth(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Use,
    )?;
    // vhco:step read authorize_store -- allow_credentials PROFILE/ACCOUNT read
    authorize_store(
        evaluator,
        &input.context,
        &profile.name,
        &input.account,
        AccessVerb::Read,
    )?;
    // vhco:todo acquire_single_flight -- CredentialProvider.acquire serializes per principal/profile-hash/account: a cached access token valid beyond the skew (min(30 s, half its lifetime)) is leased; otherwise client_credentials exchanges again (allow_env for the secret, allow_network for token_url, allow_credentials write for the commit) and user flows refresh once with their refresh token, committing the rotated token set before reporting success; concurrent callers wait for that one refresh; invalid_grant → auth.login_required (tokens dropped), ambiguous refresh outcomes or a concurrent rotation → auth.refresh_uncertain without replaying the old refresh token; the lease is opaque and origin-bound
    // vhco:step lease provider.acquire -- cached token or one coordinated acquisition/refresh
    let mut input = input;
    input.origin = origin;
    provider.acquire(input, evaluator).await
}

#[cfg(test)]
mod tests {
    use super::super::support::fakes::{FakeDriver, ctx, grants};
    use super::*;

    fn input(origin: &str) -> CredentialInput {
        CredentialInput {
            profile: "crm_service".into(),
            account: "service".into(),
            origin: origin.into(),
            audience: None,
            scopes: vec![],
            context: ctx(),
        }
    }

    // vhco:test auth.acquire_credential -- only bound resource origins get a lease; use + credentials grants are required first
    #[tokio::test]
    async fn lease_is_origin_bound_and_authorized() {
        let d = FakeDriver::default();
        let ok = grants(&["crm_service/service/use"], &["crm_service/service"]);
        let e = acquire_credential(input("https://evil.example.net"), &ok, &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "auth.origin_not_bound");
        assert_eq!(e.exit_code(), 3);
        let e = acquire_credential(input("https://api.example.com"), &grants(&[], &[]), &d)
            .await
            .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let mut wide = input("https://api.example.com");
        wide.scopes = vec!["admin".into()];
        assert_eq!(
            acquire_credential(wide, &ok, &d).await.unwrap_err().code,
            "validation.auth_scope"
        );
        let lease = acquire_credential(input("https://api.example.com/x"), &ok, &d)
            .await
            .unwrap();
        assert_eq!(lease.origin, "https://api.example.com:443");
        assert_eq!(d.calls(), vec!["acquire"]);
    }
}
