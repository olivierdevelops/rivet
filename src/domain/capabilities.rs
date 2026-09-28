//! What this build/platform supports (`rivet.capabilities`, S79/S102), and the
//! load-time refusal of adapters whose Cargo feature was compiled out.

use super::errors::codes;
use super::io_manifest::EffectCatalog;
use super::ir::CompiledProgram;
use super::{RivetError, RivetResult, Value};

// vhco:domain SandboxStatus { active | gated | unsupported }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxStatus {
    /// The backend confines sandboxed processes on this host.
    Active,
    /// Implemented but not yet certified here; sandboxed spawns are refused.
    Gated,
    /// No backend exists for this OS; sandboxed spawns are refused.
    Unsupported,
}

impl SandboxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SandboxStatus::Active => "active",
            SandboxStatus::Gated => "gated",
            SandboxStatus::Unsupported => "unsupported",
        }
    }
}

// vhco:domain BuildProbe { version: string; os: string; arch: string; sandbox_backend: string; sandbox_status: SandboxStatus; sandbox_reason: string; build_features: string[]; abi_version: int }
/// Facts only the composition root knows (crate version, compiled platform,
/// the process sandbox backend selected for this OS).
#[derive(Clone, Debug, PartialEq)]
pub struct BuildProbe {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub sandbox_backend: String,
    pub sandbox_status: SandboxStatus,
    pub sandbox_reason: String,
    /// Cargo features compiled into this build (`serve`, `grpc`, `quic`, `oauth`, `cli`).
    pub build_features: Vec<String>,
    /// The C ABI major version (`rivet_abi_version()`, PROP-2026-0002 R13).
    pub abi_version: u32,
}

// vhco:domain CapabilityReport { version: string; platform: Json; stages: Json; features: Json[]; sandbox: Json; serve: Json; build_features: string[]; abi_version: int }
/// The shape of `rivet.capabilities` (`registry.describe_capabilities` builds it
/// as a [`Value`](super::Value) object with exactly these keys, in this order):
///
/// ```text
///  version · platform{os,arch} · stages{A,B,C} · features[{name,stage,support,…}]
///  · sandbox{backend,status,reason} · serve{surfaces,auth} · build_features[…] · abi_version
/// ```
///
/// `features` is the protocol support table (0.1.0); the compiled Cargo
/// features are `build_features` (ADR-0005 decision 5).
pub const CAPABILITY_REPORT_KEYS: [&str; 8] = [
    "version",
    "platform",
    "stages",
    "features",
    "sandbox",
    "serve",
    "build_features",
    "abi_version",
];

/// Version of the C ABI (`rivet_abi_version()`); bumped only on a breaking
/// change to `rivet.h` (PROP-2026-0002 R13).
pub const ABI_VERSION: u32 = 1;

/// Every optional Cargo feature of `rivet-runtime`, in report order
/// (PROP-2026-0002 R11). `default = serve, grpc, quic, oauth`.
pub const ALL_BUILD_FEATURES: [&str; 5] = ["serve", "grpc", "quic", "oauth", "cli"];

/// `unsupported.feature` (kind unsupported → exit 5, HTTP 501) naming the
/// Cargo feature in `details.feature`.
pub fn unsupported_feature(feature: &str, what: &str) -> RivetError {
    RivetError::unsupported(
        codes::UNSUPPORTED_FEATURE,
        format!(
            "{what} needs the `{feature}` feature, which this build was compiled without \
             (rebuild rivet-runtime with `--features {feature}`)"
        ),
    )
    .with_details(Value::object([("feature", Value::text(feature))]))
}

/// Refuse a bundle that uses an adapter whose Cargo feature is not in
/// `compiled` (checked at load, before anything runs): a `grpc` connector or
/// effect needs `grpc`; a `quic` effect or HTTP/3 (`version 3`,
/// `version prefer [3, …]`) needs `quic`; an `auth NAME oauth2` profile needs
/// `oauth`. The first use in source order is reported with its span; the
/// others are attached as suppressed errors.
pub fn require_build_features(
    program: &CompiledProgram,
    effects: &EffectCatalog,
    compiled: &[&str],
) -> RivetResult<()> {
    let has = |f: &str| compiled.contains(&f);
    let mut errors: Vec<RivetError> = Vec::new();
    if !has("grpc") {
        for c in program.connectors.iter().filter(|c| c.kind == "grpc") {
            errors.push(
                unsupported_feature("grpc", &format!("gRPC connector `{}`", c.name))
                    .with_span(Some(c.span.clone())),
            );
        }
    }
    if !has("oauth") {
        for a in &program.auth_profiles {
            errors.push(
                unsupported_feature("oauth", &format!("auth profile `{}`", a.name))
                    .with_span(Some(a.span.clone())),
            );
        }
    }
    let sites = effects
        .operations
        .iter()
        .flat_map(|o| o.sites.iter())
        .chain(effects.load_sites.iter());
    for site in sites {
        let protocol = site.protocol.as_deref().unwrap_or("");
        let parts: Vec<&str> = protocol.split('|').collect();
        let need = if parts.contains(&"grpc") {
            Some(("grpc", "a gRPC call"))
        } else if parts.contains(&"quic") {
            Some(("quic", "a QUIC exchange"))
        } else if parts.contains(&"http3") {
            Some(("quic", "HTTP/3 (`version 3` or `version prefer [3, …]`)"))
        } else {
            None
        };
        if let Some((feature, what)) = need
            && !has(feature)
        {
            errors.push(
                unsupported_feature(feature, &format!("{what} in `{}`", site.operation_id))
                    .with_span(Some(site.source.clone())),
            );
        }
    }
    errors.sort_by(|a, b| {
        let key = |e: &RivetError| {
            e.source
                .as_ref()
                .map(|s| (s.file.clone(), s.start_line, s.start_col))
        };
        key(a).cmp(&key(b))
    });
    let mut it = errors.into_iter();
    match it.next() {
        None => Ok(()),
        Some(mut first) => {
            first.suppressed.extend(it);
            Err(first)
        }
    }
}
