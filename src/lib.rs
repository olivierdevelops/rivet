//! Rivet: protocol-visible operations, scoped resources and composable DAGs.
//!
//! Add it to another Rust project (PROP-2026-0002 R10, R11):
//!
//! ```toml
//! [dependencies]
//! rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0",
//!           default-features = false, features = ["serve"] }
//! tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
//! ```
//!
//! ```text
//!  rivet::Runtime::builder() ─ .file / .source / .root ─ .policy / .ceiling ─ .build()
//!      │
//!      ├─ rt.call(InputEnvelope)            ─▶ ResponseEnvelope   (every outcome, errors included)
//!      ├─ rt.request(id, Value, sink)       ─▶ Result<Completion>  (Rust-idiomatic `?`)
//!      ├─ rt.scope(|s| s.stream / s.duplex) ─▶ Envelope records    (emits / receives)
//!      ├─ rt.load(path) / load_as           ─▶ Module              (module.call / operations)
//!      ├─ rt.open_session / send_input / read_events / finish_input / cancel_session
//!      │                                    ─▶ rivet::types::Session*  (polling-style sessions)
//!      ├─ rt.trace(request_id)              ─▶ TraceResult          (decision/attempt trace)
//!      ├─ rivet::serve::start(rt, ServeOptions) ─▶ ServeHandle      (feature `serve`)
//!      └─ rivet::highlight::tokens(src)     ─▶ Vec<HighlightToken>
//! ```
//!
//! Cargo features: `serve`, `grpc`, `quic` (with HTTP/3), `oauth` (default) and
//! `cli` (the `rivet` binary). A bundle that uses a compiled-out adapter fails
//! at load with `unsupported.feature` (exit 5, HTTP 501, `details.feature`);
//! [`build_features`] lists what this build has.
//!
//! Everything under [`internal`] is Rivet's own five-bucket implementation
//! (AGENTS.md: `domain`, `features`, `io`, `infra`, `orchestrator`). It is
//! public only for Rivet's own tests, examples and the `rivet-ffi` crate, is
//! hidden from the docs and may change in any release; embedders use the
//! items re-exported at the crate root.

// RivetError deliberately carries the full error contract (kind, code, span,
// details, cause, suppressed errors, effects); errors are the cold path, so the
// size of `Result<_, RivetError>` is accepted instead of boxing every error.
#![allow(clippy::result_large_err)]

/// Rivet's implementation (not a stable API; see the crate documentation).
#[doc(hidden)]
pub mod internal;

// Inside the crate the buckets keep their short paths (`crate::domain::…`).
// (`io` is only used by the `cli` and `serve` surfaces.)
#[allow(unused_imports)]
pub(crate) use internal::{domain, features, infra, io, orchestrator};

// ── The facade (PROP-2026-0002 R10, R22) ──────────────────────────────────────

/// Version of the C ABI of `librivet` (`rivet_abi_version()`).
pub use domain::capabilities::ABI_VERSION;
/// A stream record from `StreamHandle::next` (data item or terminal result).
pub use domain::contracts::Envelope;
/// The result of `Runtime::request` (`ResponseEnvelope::from(completion)`).
pub use domain::contracts::{Completion, DataEvent};
/// The input envelope `{operation, data, deadline_ms?, restrict?, stream?}` (R4).
pub use domain::envelope::InputEnvelope;
/// The output envelope every surface prints (R1), and its parts.
pub use domain::envelope::{EnvelopeStatus, OutputFormat, RecordType, ResponseEnvelope};
/// The policy (policy.json schema v1): `Policy::from_file`, `Policy::from_json`.
pub use domain::policy::Policy;
/// Receives emitted items of `Runtime::request(…, Some(sink))`.
pub use domain::ports::DataSink;
/// Rivet's data model (`Value::from_json`, `to_json`).
pub use domain::value::Value;
/// The Cargo features compiled into this build (`rivet.capabilities.build_features`).
pub use orchestrator::runtime::build_features;
/// A loaded bundle: one dispatcher shared by every surface; `Clone + Send + Sync`.
pub use orchestrator::runtime::{Runtime, RuntimeBuilder};
/// A `.rivet` file loaded into a runtime as an object of operations (`rt.load`).
pub use orchestrator::setup_library::Module;
/// Syntax highlighting of `.rivet` sources (R17): `rivet::highlight::tokens(src)`.
pub use orchestrator::setup_library::highlight;
/// Structured concurrency for streams: `rt.scope(|scope| …)`, `scope.stream`, `scope.duplex`.
pub use orchestrator::setup_library::{DuplexHandle, DuplexSender, Scope, StreamHandle};

/// Every Rivet error: `kind`, `code`, `message`, `details`, `source` span, …
pub type Error = domain::errors::RivetError;
/// The error family of an [`Error`] (fixes the exit code and HTTP status).
pub use domain::errors::ErrorKind;
/// `Result<T, rivet::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// The crate version (the workspace version shared with `rivet-ffi`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The decision/attempt trace of one request (`Runtime::trace`).
pub use domain::io_manifest::{TraceEvent, TraceResult};

/// Argument and report types of `Runtime` methods (queries, catalogs,
/// manifests, drafts, sessions, requests, principals and source spans).
pub mod types {
    pub use crate::domain::call_graph::GraphQuery;
    pub use crate::domain::contracts::{Catalog, OutputReport, Principal, RegistryEntry, Request};
    pub use crate::domain::io_manifest::{IoQuery, IoReport, PolicyDraft, TraceQuery};
    pub use crate::domain::modules::ModuleSummary;
    /// `Runtime::{open_session, send_input, finish_input, read_events, cancel_session}`.
    pub use crate::domain::sessions::{
        CancelReceipt, SessionAck, SessionBatch, SessionEvent, SessionLimits, SessionOpenInput,
        SessionReadInput, SessionReceipt, SessionRef, SessionSendInput,
    };
    pub use crate::domain::source::SourceSpan;
}

/// Serve a runtime on one listener (REST, SSE, polling, WebSocket, MCP), as
/// `rivet serve` does: `rivet::serve::start(rt, ServeOptions { listen: … })`.
#[cfg(feature = "serve")]
pub mod serve {
    /// Host authentication callback of `ServeOptions::authenticator`.
    pub use crate::domain::ports::Authenticator;
    /// The listener's receipt (`ServeHandle::receipt`) and the authenticator input.
    pub use crate::domain::serve::{AuthnInput, ServeReceipt};
    pub use crate::orchestrator::setup_serve::{AccessLogSink, ServeHandle, ServeOptions, start};
}
