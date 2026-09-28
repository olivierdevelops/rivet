//! What this build/platform supports (`rivet.capabilities`, S79/S102).

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

// vhco:domain BuildProbe { version: string; os: string; arch: string; sandbox_backend: string; sandbox_status: SandboxStatus; sandbox_reason: string }
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
}
