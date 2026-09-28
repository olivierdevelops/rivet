//! Process sandbox backend for platforms without a tested worker boundary
//! (Windows and others, ADR-0003): every sandboxed spawn is refused with
//! `unsupported.sandbox_backend` before any process starts.

use crate::domain::transports::SandboxSpec;
use crate::domain::{RivetError, RivetResult, Value};

pub const BACKEND: &str = "none";

pub fn command(
    _spec: &SandboxSpec,
    _program: &str,
    _args: &[String],
) -> RivetResult<tokio::process::Command> {
    Err(RivetError::unsupported(
        "unsupported.sandbox_backend",
        "no process sandbox backend exists for this platform; sandboxed execution is refused",
    )
    .with_details(Value::object([("backend", Value::text(BACKEND))])))
}
