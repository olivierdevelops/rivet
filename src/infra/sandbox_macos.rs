//! macOS process sandbox backend (ADR-0003, RES-2026-0003): Seatbelt through
//! `/usr/bin/sandbox-exec` with a static deny-default SBPL profile.
//!
//! ```text
//!  sandbox-exec -p <profile> -D R0=/canon/read … -D W0=… -D X0=… -D DR0=… -- PROGRAM ARGV…
//!  profile text only ever names parameters, never paths → no profile injection
//!  children: no network (incl. loopback), no fork, signals only inside the sandbox
//! ```

use crate::domain::transports::SandboxSpec;
use crate::domain::{RivetError, RivetResult, Value};
use std::path::{Path, PathBuf};

pub const BACKEND: &str = "macos-seatbelt";
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// Fixed, published system read allow-list (ADR-0003).
const SYSTEM_READ: [&str; 6] = [
    "/usr",
    "/bin",
    "/System",
    "/Library/Apple",
    "/private/var/db/timezone",
    "/dev",
];

fn refuse(detail: &str) -> RivetError {
    RivetError::unsupported(
        "unsupported.sandbox_backend",
        format!("the macOS sandbox backend is unavailable: {detail}"),
    )
    .with_details(Value::object([
        ("backend", Value::text(BACKEND)),
        ("reason", Value::text(detail)),
    ]))
}

/// Seatbelt matches resolved vnode paths: canonicalize the longest existing
/// prefix (`/tmp` → `/private/tmp`) and re-append the rest.
fn canonical(p: &str) -> String {
    let path = Path::new(p);
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(c) = existing.canonicalize() {
            let mut out: PathBuf = c;
            for r in rest.iter().rev() {
                out.push(r);
            }
            return out.to_string_lossy().into_owned();
        }
        match (
            existing.file_name().map(|f| f.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return p.to_string(),
        }
    }
}

/// The static profile for `n` parameters of each kind.
pub fn profile(
    read: usize,
    write: usize,
    exec: usize,
    deny_read: usize,
    deny_write: usize,
) -> String {
    let params = |prefix: &str, n: usize| -> String {
        (0..n)
            .map(|i| format!(" (subpath (param \"{prefix}{i}\"))"))
            .collect()
    };
    let exec_literals: String = (0..exec)
        .map(|i| format!(" (subpath (param \"X{i}\"))"))
        .collect();
    let system: String = SYSTEM_READ
        .iter()
        .map(|p| format!(" (subpath \"{p}\")"))
        .collect();
    let mut s = String::from("(version 1)\n(deny default)\n");
    if exec > 0 {
        s.push_str(&format!("(allow process-exec{exec_literals})\n"));
    }
    s.push_str(
        "(allow signal (target same-sandbox))\n(allow sysctl-read)\n(allow file-read-metadata)\n",
    );
    s.push_str(&format!(
        "(allow file-read* (literal \"/\"){system}{}{})\n",
        params("R", read),
        params("X", exec)
    ));
    s.push_str(&format!(
        "(allow file-write* (literal \"/dev/null\"){})\n",
        params("W", write)
    ));
    s.push_str("(allow file-ioctl (literal \"/dev/null\") (literal \"/dev/tty\"))\n");
    if deny_read > 0 {
        s.push_str(&format!("(deny file-read*{})\n", params("DR", deny_read)));
    }
    if deny_write > 0 {
        s.push_str(&format!("(deny file-write*{})\n", params("DW", deny_write)));
    }
    s
}

/// Build the confined command. Fails before spawning when the backend is missing.
pub fn command(
    spec: &SandboxSpec,
    program: &str,
    args: &[String],
) -> RivetResult<tokio::process::Command> {
    if !Path::new(SANDBOX_EXEC).exists() {
        return Err(refuse("/usr/bin/sandbox-exec is missing"));
    }
    let mut exec = spec.exec.clone();
    if !exec.iter().any(|e| e == program) {
        exec.push(program.to_string());
    }
    let text = profile(
        spec.read.len(),
        spec.write.len(),
        exec.len(),
        spec.deny_read.len(),
        spec.deny_write.len(),
    );
    let mut cmd = tokio::process::Command::new(SANDBOX_EXEC);
    cmd.arg("-p").arg(text);
    let mut define = |prefix: &str, list: &[String]| {
        for (i, p) in list.iter().enumerate() {
            cmd.arg("-D").arg(format!("{prefix}{i}={}", canonical(p)));
        }
    };
    define("R", &spec.read);
    define("W", &spec.write);
    define("X", &exec);
    define("DR", &spec.deny_read);
    define("DW", &spec.deny_write);
    cmd.arg("--").arg(program).args(args);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    // vhco:test transports.run_process -- the Seatbelt profile names only parameters (no paths), denies by default and grants no network or fork
    #[test]
    fn profile_is_static_and_closed() {
        let p = profile(1, 1, 1, 1, 0);
        assert!(p.contains("(deny default)"));
        assert!(p.contains("(param \"R0\")") && p.contains("(param \"DR0\")"));
        assert!(!p.contains("network"));
        assert!(!p.contains("process-fork"));
        assert!(canonical("/tmp/x/does-not-exist").starts_with("/private/tmp"));
    }
}
