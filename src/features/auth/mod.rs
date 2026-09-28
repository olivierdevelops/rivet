//! Auth feature (PROP-2026-0001 Increment 9): OAuth 2.0 authorization
//! transactions, sanitized account status and brokered credential leases.
//!
//! ```text
//!  rivet.auth.begin ──▶ begin_authorization ──allow_auth manage + allow_credentials──▶ OAuthSessionDriver.begin
//!  rivet.auth.complete ─▶ complete_authorization ─(owner, manage)──▶ driver.complete (state/issuer/PKCE, device polling)
//!  rivet.auth.status ──▶ credential_status ──allow_auth status──▶ driver.status (no refresh)
//!  rivet.auth.disconnect ▶ disconnect_account ─allow_auth manage─▶ driver.disconnect (local_only, generation+1)
//!  rivet.auth.cancel ──▶ cancel_authorization ─(owner, manage)──▶ driver.cancel
//!  http/grpc `auth P account A` ─▶ acquire_credential ─origin bound + allow_auth use─▶ CredentialProvider.acquire
//! ```

pub mod acquire_credential;
pub mod begin_authorization;
pub mod cancel_authorization;
pub mod complete_authorization;
pub mod credential_status;
pub mod disconnect_account;
pub mod ports;
pub mod support;
