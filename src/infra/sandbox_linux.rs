//! Linux process sandbox backend (ADR-0003, RES-2026-0003): Landlock ABI V6
//! (HardRequirement) + a seccomp deny-list, applied in the child between fork
//! and exec. IMPLEMENTED, GATED: it is advertised only after the T-08
//! conformance suite passes in Linux CI on a kernel ≥ 6.12; until then every
//! sandboxed spawn fails `unsupported.sandbox_backend` before spawning.
//!
//! ```text
//!  SandboxSpec ──▶ Landlock ruleset (PathFd per grant, fs+net+scope handled)
//!              ──▶ seccomp BPF (socket AF_UNIX/INET/INET6/NETLINK/PACKET, ptrace, bpf,
//!                   io_uring, mount/unshare/setns, keyctl, module/kexec → EPERM)
//!  pre_exec: restrict_self (no_new_privs) → apply_filter → execve(argv, env allow-list)
//! ```

use crate::domain::transports::SandboxSpec;
use crate::domain::{RivetError, RivetResult, Value};
use landlock::{
    ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, Ruleset, RulesetAttr,
    RulesetCreatedAttr, Scope, path_beneath_rules,
};
use seccompiler::{
    SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule,
    TargetArch,
};
use std::collections::BTreeMap;
use std::sync::Mutex;

pub const BACKEND: &str = "linux-landlock-seccomp";

/// Flipped to true only when T-08 passes on the Linux CI matrix (ADR-0003).
const CERTIFIED: bool = false;

const SYSTEM_READ: [&str; 6] = ["/usr", "/bin", "/lib", "/lib64", "/etc/ld.so.cache", "/dev"];

/// Advertised state for `rivet.capabilities`.
pub fn status() -> (crate::domain::capabilities::SandboxStatus, &'static str) {
    use crate::domain::capabilities::SandboxStatus;
    if CERTIFIED {
        (
            SandboxStatus::Active,
            "Landlock ABI 6 + seccomp deny-list applied before exec",
        )
    } else {
        (
            SandboxStatus::Gated,
            "implemented, gated until the T-08 conformance suite passes on Linux CI (kernel >= 6.12, Landlock ABI 6); sandboxed spawns are refused",
        )
    }
}

fn refuse(detail: &str) -> RivetError {
    RivetError::unsupported(
        "unsupported.sandbox_backend",
        format!("the Linux sandbox backend is unavailable: {detail}"),
    )
    .with_details(Value::object([
        ("backend", Value::text(BACKEND)),
        ("reason", Value::text(detail)),
    ]))
}

fn seccomp_program() -> RivetResult<seccompiler::BpfProgram> {
    let eperm = SeccompAction::Errno(libc::EPERM as u32);
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    let mut socket_rules = Vec::new();
    for domain in [
        libc::AF_UNIX,
        libc::AF_INET,
        libc::AF_INET6,
        libc::AF_NETLINK,
        libc::AF_PACKET,
    ] {
        let cond =
            SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, domain as u64)
                .map_err(|e| refuse(&format!("seccomp condition: {e}")))?;
        socket_rules
            .push(SeccompRule::new(vec![cond]).map_err(|e| refuse(&format!("seccomp rule: {e}")))?);
    }
    rules.insert(libc::SYS_socket, socket_rules);
    for sys in [
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_io_uring_setup,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_kexec_load,
        libc::SYS_init_module,
    ] {
        rules.insert(sys, Vec::new());
    }
    let arch: TargetArch = std::env::consts::ARCH
        .try_into()
        .map_err(|_| refuse("unsupported CPU architecture for seccomp"))?;
    let filter = SeccompFilter::new(rules, SeccompAction::Allow, eperm, arch)
        .map_err(|e| refuse(&format!("seccomp filter: {e}")))?;
    filter
        .try_into()
        .map_err(|e: seccompiler::BackendError| refuse(&format!("seccomp compile: {e}")))
}

/// Build the confined command. Fails before spawning while the backend is gated
/// or when the spec cannot be expressed (deny entries inside granted trees).
pub fn command(
    spec: &SandboxSpec,
    program: &str,
    args: &[String],
) -> RivetResult<tokio::process::Command> {
    if !CERTIFIED {
        return Err(refuse(
            "gated until the T-08 conformance suite passes on Linux CI (kernel >= 6.12, Landlock ABI 6)",
        ));
    }
    if !spec.deny_read.is_empty() || !spec.deny_write.is_empty() {
        return Err(refuse(
            "policy deny entries cannot be expressed by Landlock",
        ));
    }
    let abi = ABI::V6;
    let read: Vec<String> = SYSTEM_READ
        .iter()
        .map(|s| s.to_string())
        .chain(spec.read.iter().cloned())
        .collect();
    let mut exec = spec.exec.clone();
    exec.push(program.to_string());
    let ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .and_then(|r| r.handle_access(AccessNet::from_all(abi)))
        .and_then(|r| r.scope(Scope::from_all(abi)))
        .and_then(|r| r.create())
        .and_then(|r| r.add_rules(path_beneath_rules(&read, AccessFs::from_read(abi))))
        .and_then(|r| {
            r.add_rules(path_beneath_rules(
                &exec,
                AccessFs::from_read(abi) | AccessFs::Execute,
            ))
        })
        .and_then(|r| {
            r.add_rules(path_beneath_rules(
                &spec.write,
                AccessFs::from_all(abi) & !AccessFs::Execute,
            ))
        })
        .map_err(|e| refuse(&format!("landlock: {e}")))?;
    let program_bpf = seccomp_program()?;
    let pending = Mutex::new(Some((ruleset, program_bpf)));
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args);
    // SAFETY: runs in the forked child before exec; it only takes the prepared
    // ruleset/filter out of a mutex no other thread in the child can hold.
    unsafe {
        cmd.pre_exec(move || {
            let taken = pending.lock().ok().and_then(|mut g| g.take());
            let Some((ruleset, bpf)) = taken else {
                return Err(std::io::Error::from_raw_os_error(libc::EPERM));
            };
            ruleset
                .restrict_self()
                .map_err(|_| std::io::Error::from_raw_os_error(libc::EPERM))?;
            seccompiler::apply_filter(&bpf)
                .map_err(|_| std::io::Error::from_raw_os_error(libc::EPERM))?;
            Ok(())
        });
    }
    Ok(cmd)
}
