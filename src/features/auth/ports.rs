//! Ports needed by the auth feature.

// vhco:port OAuthSessionDriver { begin(AuthBeginInput) -> AuthChallenge; complete(AuthCompleteInput) -> CredentialStatus; status(CredentialStatusInput) -> CredentialStatus; disconnect(DisconnectInput) -> DisconnectReceipt; cancel(AuthCancelInput) -> AuthCancelReceipt }
pub use crate::domain::ports::OAuthSessionDriver;
// vhco:port CredentialProvider { acquire(CredentialInput) -> CredentialLease }
pub use crate::domain::ports::CredentialProvider;
// vhco:port PolicyEvaluator { evaluate(EffectIntent) -> Permit }
pub use crate::domain::ports::PolicyEvaluator;
