//! Stand-ins for adapters whose Cargo feature is compiled out
//! (PROP-2026-0002 R11, ADR-0005 decision 4).
//!
//! ```text
//!  feature   compiled in                       compiled out (this file)
//!  quic      infra::h3_client (HTTP/3)          h3_client::send  ─▶ unsupported.feature {feature: quic}
//!  oauth     infra::oauth_adapter::OAuthAdapter NoOAuth: every flow ─▶ unsupported.feature {feature: oauth}
//! ```
//!
//! A bundle that uses such an adapter is refused at load
//! (`domain::capabilities::require_build_features`); these stand-ins keep the
//! runtime assembly identical and refuse again if anything reaches them.

// vhco:infra unsupported_features satisfies OAuthSessionDriver, CredentialProvider

/// HTTP/3 without the `quic` feature: the HTTP adapter's `version 3` branch
/// refuses instead of silently downgrading.
#[cfg(not(feature = "quic"))]
pub mod h3_client {
    use crate::domain::RivetResult;
    use crate::domain::capabilities::unsupported_feature;
    use crate::domain::transports::{ByteStream, HttpWire};

    /// Response head and live body of one HTTP/3 exchange (never built here).
    pub struct H3Response {
        pub status: u16,
        pub headers: Vec<(String, String)>,
        pub body: Box<dyn ByteStream>,
    }

    pub async fn send(_wire: &HttpWire) -> RivetResult<H3Response> {
        Err(unsupported_feature("quic", "HTTP/3"))
    }
}

/// OAuth 2.0 without the `oauth` feature: no profile exists and every
/// authorization or credential request is `unsupported.feature`.
#[cfg(not(feature = "oauth"))]
pub mod oauth {
    use crate::domain::RivetResult;
    use crate::domain::auth::{
        AuthBeginInput, AuthCancelInput, AuthCancelReceipt, AuthChallenge, AuthCompleteInput,
        AuthTransactionInfo, CredentialInput, CredentialLease, CredentialStatus,
        CredentialStatusInput, DisconnectInput, DisconnectReceipt, OAuthProfile,
    };
    use crate::domain::capabilities::unsupported_feature;
    use crate::domain::contracts::Principal;
    use crate::domain::ir::CompiledProgram;
    use crate::domain::ports::{CredentialProvider, OAuthSessionDriver, PolicyEvaluator};
    use crate::infra::http_adapter::ExchangeHttpFn;
    use async_trait::async_trait;
    use std::sync::Arc;

    fn refuse<T>() -> RivetResult<T> {
        Err(unsupported_feature("oauth", "OAuth 2.0"))
    }

    /// The OAuth adapter of a build without `oauth`.
    pub struct NoOAuth;

    impl NoOAuth {
        /// Same signature as `OAuthAdapter::load`; profiles were already
        /// refused by the load-time feature check.
        pub fn load(
            _program: &CompiledProgram,
            _exchange: Arc<ExchangeHttpFn>,
        ) -> RivetResult<Self> {
            Ok(NoOAuth)
        }
    }

    #[async_trait]
    impl OAuthSessionDriver for NoOAuth {
        fn profile(&self, _name: &str) -> Option<OAuthProfile> {
            None
        }
        fn transaction(&self, _id: &str, _principal: &Principal) -> Option<AuthTransactionInfo> {
            None
        }
        async fn begin(
            &self,
            _input: AuthBeginInput,
            _evaluator: &dyn PolicyEvaluator,
        ) -> RivetResult<AuthChallenge> {
            refuse()
        }
        async fn complete(
            &self,
            _input: AuthCompleteInput,
            _evaluator: &dyn PolicyEvaluator,
        ) -> RivetResult<CredentialStatus> {
            refuse()
        }
        async fn status(&self, _input: CredentialStatusInput) -> RivetResult<CredentialStatus> {
            refuse()
        }
        async fn disconnect(
            &self,
            _input: DisconnectInput,
            _evaluator: &dyn PolicyEvaluator,
        ) -> RivetResult<DisconnectReceipt> {
            refuse()
        }
        async fn cancel(&self, _input: AuthCancelInput) -> RivetResult<AuthCancelReceipt> {
            refuse()
        }
    }

    #[async_trait]
    impl CredentialProvider for NoOAuth {
        fn profile(&self, _name: &str) -> Option<OAuthProfile> {
            None
        }
        async fn acquire(
            &self,
            _input: CredentialInput,
            _evaluator: &dyn PolicyEvaluator,
        ) -> RivetResult<CredentialLease> {
            refuse()
        }
        fn invalidate(&self, _lease: &CredentialLease) {}
    }
}
