use super::ports::{Codec, PolicyEvaluator, ProcessRunner};
use crate::domain::effect_checks::authorize;
use crate::domain::policy::{AccessVerb, Capability, EffectTarget, Grant, Policy};
use crate::domain::transports::{CodecInput, CodecKind, ProcessOutcome, ProcessPlan, SandboxSpec};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use std::path::{Component, Path, PathBuf};

/// Interpreters whose `-c` flag evaluates a shell string (declined escape hatch).
const SHELLS: [&str; 10] = [
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "fish",
    "csh",
    "tcsh",
    "cmd.exe",
    "powershell",
];

// vhco:usecase transports.run_process(input: ProcessPlan) -> ProcessResult needs ProcessRunner, Codec, PolicyEvaluator
// vhco:label Run process
// vhco:about Runs one argv-only child: refuses shell strings and PATH lookups, authorizes allow_exec on the exact binary, requires an OS sandbox whenever policy.json is present (built from the policy's read/write/exec grants; unrepresentable grants or a missing backend fail before spawning), encodes stdin, and maps a nonzero exit to kind process unless `accept exit` lists it.
// vhco:example input={program:"/usr/bin/printf", args:["%s","hello; echo x"], decode_stdout:"text"} => { "stdout": "hello; echo x", "stderr": "", "exit": 0, "duration_ms": 3 }
pub async fn run_process(
    input: ProcessPlan,
    evaluator: &dyn PolicyEvaluator,
    runner: &dyn ProcessRunner,
    codec: &dyn Codec,
) -> RivetResult<ProcessOutcome> {
    let mut plan = input;
    let span = plan.origin.span.clone();
    // vhco:todo authorize_exec -- refuse a shell interpreter invoked with -c (unsupported.shell) and `interactive true` (Stage C, unsupported.interactive); require an absolute or bundle-relative path (no PATH lookup); authorize allow_exec on the path as written; env starts empty and holds only the explicit `env {…}` pairs; when policy.json is present build the SandboxSpec from its read/write/delete/exec grants and deny entries (only `DIR/**`, exact paths and `*` are representable; narrowed `access` lists or other globs fail unsupported.sandbox_backend before spawning)
    // vhco:step shell guard -- `sh -c STRING` and friends are the declined shell escape hatch
    // vhco:error shell -- shell string evaluation => unsupported.shell (501, exit 5), never spawns
    let base = plan
        .program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if SHELLS.contains(&base.as_str())
        && plan
            .args
            .iter()
            .any(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('c'))
    {
        return Err(RivetError::unsupported(
            "unsupported.shell",
            format!(
                "`{}` with -c evaluates a shell string; pass argv to the target binary instead",
                plan.program
            ),
        )
        .with_span(span));
    }
    if plan.interactive {
        return Err(RivetError::unsupported(
            "unsupported.interactive",
            "interactive processes are Stage C and not available in this build",
        )
        .with_span(span));
    }
    // vhco:error program_path -- a bare name (PATH lookup) => validation.process_program (exit 2)
    if !plan.program.contains('/') && !plan.program.contains('\\') {
        return Err(RivetError::validation(
            "validation.process_program",
            format!(
                "`{}` is not a path; use an absolute path or a bundle-relative ./path (no PATH lookup)",
                plan.program
            ),
        )
        .with_span(span));
    }
    // vhco:step authorize effect_checks::authorize -- allow_exec exec on the binary path
    authorize(
        evaluator,
        &plan.origin,
        Capability::Exec,
        AccessVerb::Exec,
        EffectTarget::Path(plan.program.clone()),
    )?;
    // vhco:step cwd effect_checks::authorize -- an explicit working directory must be inside a readable grant (allow_read stat)
    if let Some(cwd) = &plan.cwd {
        authorize(
            evaluator,
            &plan.origin,
            Capability::Read,
            AccessVerb::Stat,
            EffectTarget::Path(cwd.clone()),
        )?;
    }
    // vhco:step sandbox sandbox_spec -- policy present ⇒ the child must be confined
    let policy = evaluator.policy();
    if policy.present {
        plan.sandbox = Some(sandbox_spec(policy).map_err(|e| e.with_span(span.clone()))?);
    }
    // vhco:todo run_bounded -- encode `stdin json|text|bytes` through Codec; run through ProcessRunner with stdout/stderr drained concurrently into bounded buffers under min(timeout, request deadline); `with command … as p` + `stream stdout …` spawns instead and returns the live stdout for the scope to frame
    // vhco:step stdin codec.encode -- the only data the child receives on stdin
    if let Some(s) = &plan.stdin {
        let bytes = codec.encode(s)?;
        plan.stdin = Some(CodecInput {
            kind: CodecKind::Bytes,
            bytes: Some(bytes),
            value: None,
        });
    }
    if plan.stream.is_some() {
        // vhco:step spawn runner.spawn -- scope-owned child; cleanup closes stdin, terminates, kills after grace and reaps
        return runner
            .spawn(&plan)
            .await
            .map(ProcessOutcome::Streaming)
            .map_err(|e| e.with_span(span));
    }
    // vhco:step run runner.run -- bounded capture; the runner kills and reaps on timeout
    let result = runner
        .run(&plan)
        .await
        .map_err(|e| e.with_span(span.clone()))?;
    // vhco:todo reap -- the runner always reaps (kill_on_drop + wait, process group killed on deadline); an exit status outside `accept exit [...]` (default [0]) fails kind process with details {exit, stderr tail}; otherwise decode stdout/stderr (`decode stdout T`, default text when UTF-8 else bytes) into {stdout, stderr, exit, duration_ms}
    // vhco:error nonzero_exit -- exit not accepted => process.exit (kind process, exit 5)
    let accept = if plan.accept_exit.is_empty() {
        vec![0]
    } else {
        plan.accept_exit.clone()
    };
    if !accept.contains(&result.exit_status) {
        let tail =
            String::from_utf8_lossy(&result.stderr[result.stderr.len().saturating_sub(2048)..])
                .into_owned();
        return Err(RivetError::new(
            ErrorKind::Process,
            "process.exit",
            format!(
                "`{}` exited with status {}",
                plan.program, result.exit_status
            ),
        )
        .with_span(span)
        .with_details(Value::object([
            ("exit", Value::Int(result.exit_status)),
            ("stderr", Value::Text(tail)),
        ])));
    }
    // vhco:step decode codec.decode -- stdout/stderr per `decode stdout|stderr T`
    let stdout = decode_output(codec, plan.decode_stdout, result.stdout)?;
    let stderr = decode_output(codec, plan.decode_stderr, result.stderr)?;
    Ok(ProcessOutcome::Finished(Value::object([
        ("stdout", stdout),
        ("stderr", stderr),
        ("exit", Value::Int(result.exit_status)),
        ("duration_ms", Value::Int(result.duration_ms as i64)),
    ])))
}

fn decode_output(codec: &dyn Codec, kind: Option<CodecKind>, bytes: Vec<u8>) -> RivetResult<Value> {
    match kind {
        Some(k) => codec.decode(&CodecInput {
            kind: k,
            bytes: Some(bytes),
            value: None,
        }),
        None => Ok(match String::from_utf8(bytes) {
            Ok(s) => Value::Text(s),
            Err(e) => Value::Bytes(e.into_bytes()),
        }),
    }
}

/// Map policy grants to the child's confinement (ADR-0003 v0.1.0 contract).
/// Children never get network. Anything the backend cannot express exactly
/// fails `unsupported.sandbox_backend` instead of being widened.
pub fn sandbox_spec(policy: &Policy) -> RivetResult<SandboxSpec> {
    let mut spec = SandboxSpec::default();
    let base = &policy.base_dir;
    for g in &policy.grants {
        let bucket = match g.capability {
            Capability::Read => &mut spec.read,
            Capability::Write | Capability::Delete => &mut spec.write,
            Capability::Exec => &mut spec.exec,
            _ => continue,
        };
        check_access(g)?;
        for t in &g.targets {
            bucket.push(selector_path(base, t)?);
        }
    }
    for d in &policy.deny {
        let bucket = match d.capability {
            Capability::Read => &mut spec.deny_read,
            Capability::Write | Capability::Delete => &mut spec.deny_write,
            Capability::Exec => &mut spec.deny_read,
            _ => continue,
        };
        // A deny narrowed by access still denies at least that; deny the whole path (narrowing is safe).
        for t in &d.targets {
            bucket.push(selector_path(base, t)?);
        }
    }
    Ok(spec)
}

fn unrepresentable(detail: String) -> RivetError {
    RivetError::unsupported(
        "unsupported.sandbox_backend",
        format!("the process sandbox cannot represent this policy exactly: {detail}"),
    )
    .with_details(Value::object([("grant", Value::Text(detail))]))
}

fn check_access(g: &Grant) -> RivetResult<()> {
    if let Some(access) = &g.access
        && g.capability.verbs().iter().any(|v| !access.contains(v))
    {
        return Err(unrepresentable(format!(
            "{} {} narrowed by access",
            g.capability.as_str(),
            g.targets.join(", ")
        )));
    }
    Ok(())
}

/// `./data/**` → `<base>/data`; `*` → `/`; exact paths stay; other globs are refused.
fn selector_path(base: &str, selector: &str) -> RivetResult<String> {
    if selector == "*" {
        return Ok("/".into());
    }
    let trimmed = selector.strip_suffix("/**").unwrap_or(selector);
    if trimmed.contains(['*', '?', '[', '{']) {
        return Err(unrepresentable(format!("glob selector `{selector}`")));
    }
    let joined = if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        Path::new(base).join(trimmed)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::{Decision, EffectIntent, Permit};
    use crate::domain::transports::{ByteStream, EffectOrigin, ProcessResult};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct Allow(Policy);
    impl PolicyEvaluator for Allow {
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

    struct Fake {
        exit: i64,
        seen: Mutex<Vec<ProcessPlan>>,
    }
    #[async_trait]
    impl ProcessRunner for Fake {
        async fn run(&self, plan: &ProcessPlan) -> RivetResult<ProcessResult> {
            self.seen.lock().unwrap().push(plan.clone());
            Ok(ProcessResult {
                exit_status: self.exit,
                stdout: b"out".to_vec(),
                stderr: b"err".to_vec(),
                duration_ms: 1,
            })
        }
        async fn spawn(&self, _plan: &ProcessPlan) -> RivetResult<Box<dyn ByteStream>> {
            Err(RivetError::internal("unused"))
        }
    }

    struct Text;
    impl Codec for Text {
        fn decode(&self, i: &CodecInput) -> RivetResult<Value> {
            Ok(Value::Text(
                String::from_utf8_lossy(i.bytes.as_deref().unwrap_or(&[])).into(),
            ))
        }
        fn encode(&self, i: &CodecInput) -> RivetResult<Vec<u8>> {
            Ok(i.value
                .clone()
                .unwrap_or_default()
                .to_display()
                .into_bytes())
        }
    }

    fn plan(program: &str, args: &[&str]) -> ProcessPlan {
        ProcessPlan {
            program: program.into(),
            resolved: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: vec![],
            cwd: None,
            stdin: None,
            timeout_ms: 1000,
            accept_exit: vec![],
            decode_stdout: None,
            decode_stderr: None,
            stream: None,
            sandbox: None,
            max_output: 1 << 20,
            interactive: false,
            origin: EffectOrigin::default(),
        }
    }

    fn exec_policy(target: &str, extra: Vec<Grant>) -> Allow {
        let mut grants = vec![Grant {
            capability: Capability::Exec,
            targets: vec![target.into()],
            access: None,
        }];
        grants.extend(extra);
        Allow(Policy {
            present: true,
            base_dir: "/bundle".into(),
            grants,
            ..Policy::default()
        })
    }

    // vhco:test transports.run_process -- shell strings, bare names and ungranted binaries never reach the runner; a present policy yields a sandbox spec
    #[tokio::test]
    async fn refusals_and_sandbox_spec() {
        let f = Fake {
            exit: 0,
            seen: Mutex::new(vec![]),
        };
        let ev = exec_policy(
            "/usr/bin/printf",
            vec![Grant {
                capability: Capability::Read,
                targets: vec!["./data/**".into()],
                access: None,
            }],
        );
        let e = run_process(plan("/bin/sh", &["-c", "echo hi"]), &ev, &f, &Text)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "unsupported.shell");
        let e = run_process(plan("printf", &[]), &ev, &f, &Text)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "validation.process_program");
        let e = run_process(plan("/usr/bin/env", &[]), &ev, &f, &Text)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "permission.denied");
        assert!(f.seen.lock().unwrap().is_empty());
        let ProcessOutcome::Finished(v) =
            run_process(plan("/usr/bin/printf", &["x"]), &ev, &f, &Text)
                .await
                .unwrap()
        else {
            panic!("expected a finished process")
        };
        assert_eq!(v.get("stdout"), Some(&Value::text("out")));
        let seen = f.seen.lock().unwrap();
        let spec = seen[0].sandbox.as_ref().unwrap();
        assert_eq!(spec.read, vec!["/bundle/data".to_string()]);
        assert_eq!(spec.exec, vec!["/usr/bin/printf".to_string()]);
    }

    // vhco:test transports.run_process -- a nonzero exit fails kind process unless accept exit lists it; narrowed or glob grants cannot be sandboxed
    #[tokio::test]
    async fn exit_mapping_and_unrepresentable() {
        let f = Fake {
            exit: 3,
            seen: Mutex::new(vec![]),
        };
        let ev = exec_policy("/usr/bin/false", vec![]);
        let e = run_process(plan("/usr/bin/false", &[]), &ev, &f, &Text)
            .await
            .err()
            .unwrap();
        assert_eq!(
            (e.kind, e.code.as_str()),
            (ErrorKind::Process, "process.exit")
        );
        assert_eq!(e.details.get("exit"), Some(&Value::Int(3)));
        let mut ok = plan("/usr/bin/false", &[]);
        ok.accept_exit = vec![0, 3];
        assert!(run_process(ok, &ev, &f, &Text).await.is_ok());
        let glob = exec_policy(
            "/usr/bin/false",
            vec![Grant {
                capability: Capability::Read,
                targets: vec!["./data/*.json".into()],
                access: None,
            }],
        );
        let e = run_process(plan("/usr/bin/false", &[]), &glob, &f, &Text)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "unsupported.sandbox_backend");
    }
}
