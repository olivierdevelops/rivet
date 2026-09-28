//! Shared checks for the auth use cases: account names, profile lookup and
//! the allow_auth / allow_credentials permits.

use super::ports::PolicyEvaluator;
use crate::domain::auth::{AuthContext, OAuthProfile};
use crate::domain::effect_checks::authorize;
use crate::domain::policy::{AccessVerb, Capability, EffectTarget};
use crate::domain::transports::EffectOrigin;
use crate::domain::{ErrorKind, RivetError, RivetResult};

pub fn origin(ctx: &AuthContext) -> EffectOrigin {
    EffectOrigin {
        operation_id: ctx.operation_id.clone(),
        span: None,
    }
}

/// OAuth lifecycle failures are `conflict` (409, exit 4).
pub fn conflict(code: &str, message: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Conflict, code, message)
}

/// Accounts are opaque keys inside the principal: non-empty, no `/`, bounded.
pub fn check_account(account: &str) -> RivetResult<()> {
    if account.is_empty()
        || account.len() > 128
        || account.contains('/')
        || account.chars().any(char::is_control)
    {
        return Err(RivetError::validation(
            "validation.auth_account",
            "account must be 1-128 characters without `/` or control characters",
        ));
    }
    Ok(())
}

pub fn resolve_profile(
    name: &str,
    lookup: impl Fn(&str) -> Option<OAuthProfile>,
) -> RivetResult<OAuthProfile> {
    lookup(name).ok_or_else(|| {
        RivetError::not_found(
            "not_found.auth_profile",
            format!("no auth profile `{name}`"),
        )
    })
}

/// `allow_auth PROFILE/ACCOUNT/VERB`; a grant on `PROFILE/ACCOUNT` whose
/// `access` list names VERB is equivalent.
pub fn authorize_auth(
    evaluator: &dyn PolicyEvaluator,
    ctx: &AuthContext,
    profile: &str,
    account: &str,
    verb: AccessVerb,
) -> RivetResult<()> {
    let o = origin(ctx);
    authorize(
        evaluator,
        &o,
        Capability::Auth,
        verb,
        EffectTarget::Logical(format!("{profile}/{account}/{}", verb.as_str())),
    )
    .or_else(|e| {
        authorize(
            evaluator,
            &o,
            Capability::Auth,
            verb,
            EffectTarget::Logical(format!("{profile}/{account}")),
        )
        .map_err(|_| e)
    })
}

/// `allow_credentials PROFILE/ACCOUNT` for a store read or write.
pub fn authorize_store(
    evaluator: &dyn PolicyEvaluator,
    ctx: &AuthContext,
    profile: &str,
    account: &str,
    verb: AccessVerb,
) -> RivetResult<()> {
    authorize(
        evaluator,
        &origin(ctx),
        Capability::Credentials,
        verb,
        EffectTarget::Logical(format!("{profile}/{account}")),
    )
}

#[cfg(test)]
pub mod fakes {
    //! In-memory driver/provider and exact-match evaluator for the use-case tests.
    use crate::domain::auth::*;
    use crate::domain::contracts::Principal;
    use crate::domain::ir::{Arg, Declaration, Expr, OptionLine};
    use crate::domain::policy::{
        Capability, Decision, EffectIntent, Grant, NetworkPolicy, Permit, Policy,
    };
    use crate::domain::ports::{CredentialProvider, OAuthSessionDriver, PolicyEvaluator};
    use crate::domain::source::SourceSpan;
    use crate::domain::{RivetResult, Value};
    use async_trait::async_trait;
    use std::sync::Mutex;

    pub struct Exact(Policy);
    impl PolicyEvaluator for Exact {
        fn evaluate(&self, i: &EffectIntent) -> Permit {
            let ok = self.0.grants.iter().any(|g| {
                g.capability == i.capability && g.targets.iter().any(|t| t == i.target.as_str())
            });
            Permit {
                intent: i.clone(),
                decision: if ok {
                    Decision::Allowed
                } else {
                    Decision::Denied
                },
                rule: "test".into(),
            }
        }
        fn policy(&self) -> &Policy {
            &self.0
        }
    }

    pub fn grants(auth: &[&str], creds: &[&str]) -> Exact {
        let g = |c, t: &[&str]| Grant {
            capability: c,
            targets: t.iter().map(|s| s.to_string()).collect(),
            access: None,
        };
        Exact(Policy {
            present: true,
            grants: vec![g(Capability::Auth, auth), g(Capability::Credentials, creds)],
            network: NetworkPolicy {
                deny_private_ranges: true,
            },
            ..Policy::default()
        })
    }

    pub fn ctx() -> AuthContext {
        AuthContext {
            principal: Principal::local(),
            operation_id: "rivet.auth".into(),
            deadline_ms: 30_000,
        }
    }

    fn opt(key: &str, args: Vec<Arg>) -> OptionLine {
        OptionLine {
            key: key.into(),
            args,
            span: SourceSpan::default(),
            children: vec![],
        }
    }
    fn w(s: &str) -> Arg {
        Arg::Word(s.into(), SourceSpan::default())
    }
    fn t(s: &str) -> Arg {
        Arg::Expr(Expr::Lit(Value::text(s)), SourceSpan::default())
    }
    fn l(items: &[&str]) -> Arg {
        Arg::Expr(
            Expr::List(items.iter().map(|s| Expr::Lit(Value::text(*s))).collect()),
            SourceSpan::default(),
        )
    }

    pub fn profile(name: &str, flow: &str) -> OAuthProfile {
        let mut options = vec![
            opt("flow", vec![w(flow)]),
            opt("issuer", vec![t("https://auth.example.com")]),
            opt("token_url", vec![t("https://auth.example.com/token")]),
            opt("client_id", vec![t("rivet")]),
            opt("scopes", vec![l(&["contacts.read"])]),
            opt(
                "resource_origins",
                vec![l(&["https://api.example.com:443"])],
            ),
            opt("store", vec![w("memory")]),
        ];
        match flow {
            "client_credentials" => {
                options.push(opt("client_auth", vec![w("basic")]));
                options.push(opt("client_secret", vec![w("env"), t("SECRET")]));
            }
            "authorization_code" => {
                options.push(opt("client_auth", vec![w("none")]));
                options.push(opt("pkce", vec![w("s256")]));
                options.push(opt(
                    "authorization_url",
                    vec![t("https://auth.example.com/authorize")],
                ));
                options.push(opt("redirect_uri", vec![t("https://app.example.com/cb")]));
            }
            _ => {
                options.push(opt("client_auth", vec![w("none")]));
                options.push(opt(
                    "device_url",
                    vec![t("https://auth.example.com/device")],
                ));
            }
        }
        OAuthProfile::from_declaration(&Declaration {
            name: name.into(),
            kind: "auth".into(),
            options,
            span: SourceSpan::default(),
        })
        .unwrap()
    }

    /// Records which driver calls happened; answers with fixed shapes.
    #[derive(Default)]
    pub struct FakeDriver {
        calls: Mutex<Vec<&'static str>>,
    }

    impl FakeDriver {
        pub fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
        fn hit(&self, c: &'static str) {
            self.calls.lock().unwrap().push(c);
        }
        fn lookup(&self, name: &str) -> Option<OAuthProfile> {
            match name {
                "crm_user" => Some(profile(name, "authorization_code")),
                "crm_device" => Some(profile(name, "device_code")),
                "crm_service" => Some(profile(name, "client_credentials")),
                _ => None,
            }
        }
        fn status_of(profile: &str, account: &str) -> CredentialStatus {
            CredentialStatus {
                profile: profile.into(),
                account: account.into(),
                state: CredentialState::Connected,
                scopes: vec!["contacts.read".into()],
                expires_at: None,
                generation: 1,
                transaction_id: None,
            }
        }
    }

    #[async_trait]
    impl OAuthSessionDriver for FakeDriver {
        fn profile(&self, name: &str) -> Option<OAuthProfile> {
            self.lookup(name)
        }
        fn transaction(&self, id: &str, principal: &Principal) -> Option<AuthTransactionInfo> {
            (id == "auth_01" && principal.name == "local").then(|| AuthTransactionInfo {
                transaction_id: id.into(),
                profile: "crm_user".into(),
                account: "ada".into(),
                state: "open".into(),
            })
        }
        async fn begin(
            &self,
            _i: AuthBeginInput,
            _e: &dyn PolicyEvaluator,
        ) -> RivetResult<AuthChallenge> {
            self.hit("begin");
            Ok(AuthChallenge {
                transaction_id: "auth_01".into(),
                authorization_url: Some("https://auth.example.com/authorize".into()),
                verification_uri: None,
                verification_uri_complete: None,
                user_code: None,
                expires_at: rfc3339(0),
                interval_seconds: None,
            })
        }
        async fn complete(
            &self,
            _i: AuthCompleteInput,
            _e: &dyn PolicyEvaluator,
        ) -> RivetResult<CredentialStatus> {
            self.hit("complete");
            Ok(Self::status_of("crm_user", "ada"))
        }
        async fn status(&self, i: CredentialStatusInput) -> RivetResult<CredentialStatus> {
            self.hit("status");
            Ok(Self::status_of(&i.profile, &i.account))
        }
        async fn disconnect(
            &self,
            i: DisconnectInput,
            _e: &dyn PolicyEvaluator,
        ) -> RivetResult<DisconnectReceipt> {
            self.hit("disconnect");
            Ok(DisconnectReceipt {
                profile: i.profile,
                account: i.account,
                local_only: true,
                generation: 2,
            })
        }
        async fn cancel(&self, i: AuthCancelInput) -> RivetResult<AuthCancelReceipt> {
            self.hit("cancel");
            Ok(AuthCancelReceipt {
                transaction_id: i.transaction_id,
                state: "cancelled".into(),
            })
        }
    }

    #[async_trait]
    impl CredentialProvider for FakeDriver {
        fn profile(&self, name: &str) -> Option<OAuthProfile> {
            self.lookup(name)
        }
        async fn acquire(
            &self,
            i: CredentialInput,
            _e: &dyn PolicyEvaluator,
        ) -> RivetResult<CredentialLease> {
            self.hit("acquire");
            Ok(CredentialLease {
                handle: SecretString::new("tok"),
                scope_id: format!("local/{}/{}", i.profile, i.account),
                origin: i.origin,
                generation: 1,
                expires_at: None,
            })
        }
        fn invalidate(&self, _l: &CredentialLease) {}
    }
}
