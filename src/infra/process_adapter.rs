//! Process adapter (satisfies ProcessRunner) and the `command` effect kind.
//! argv only — never a shell; the environment starts empty; children are
//! confined by the platform sandbox whenever the plan asks for one, and are
//! always reaped (kill_on_drop, process-group kill on deadline/cancel).
//!
//! ```text
//!  sandbox required? ─ no ─▶ plain argv spawn (allow_exec already checked)
//!        │ yes
//!  backend for this OS ─▶ macOS Seatbelt │ Linux Landlock+seccomp (gated) │ others: refuse
//!        │ refused ─▶ unsupported.sandbox_backend, zero spawns
//!        ▼
//!  spawn (process group, piped stdio) ─▶ drain stdout/stderr concurrently (bounded)
//!        ▼
//!  exit / timeout (killpg + wait) ─▶ ProcessResult  │  StreamHandle for `with command`
//! ```

use super::codec::{StdCodec, StreamDecoder};
use super::effect_args::{
    bad, boolean, budget_ms, duration, int_list, options, origin, text, word,
};
use super::execution_driver::{EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle};
use super::http_adapter::{MAX_STREAM_ITEM, StreamHandle};
use crate::domain::ir::EffectForm;
use crate::domain::ports::PolicyEvaluator;
use crate::domain::transports::{
    ByteSink, ByteStream, ChildDuplex, Codec, CodecInput, CodecKind, ProcessOutcome, ProcessPlan,
    ProcessResult, ProcessRunner, StreamMode,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdout, Command};

#[cfg(target_os = "linux")]
use super::sandbox_linux as sandbox;
#[cfg(target_os = "macos")]
use super::sandbox_macos as sandbox;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use super::sandbox_unsupported as sandbox;

/// Default bound on captured stdout / stderr.
pub const DEFAULT_MAX_OUTPUT: u64 = 64 * 1024 * 1024;
/// Controlled PATH given to children (never the host's).
const CHILD_PATH: &str = "/usr/bin:/bin";

/// The injected `transports.run_process` use case.
pub type RunProcessFn = dyn for<'a> Fn(
        ProcessPlan,
        &'a dyn PolicyEvaluator,
        &'a dyn ProcessRunner,
        &'a dyn Codec,
    ) -> BoxFuture<'a, RivetResult<ProcessOutcome>>
    + Send
    + Sync;

// vhco:infra process_adapter satisfies ProcessRunner
// vhco:exec argv -- binaries authorized by allow_exec in transports.run_process; sandboxed (Seatbelt / Landlock+seccomp) whenever policy.json is present
pub struct TokioRunner {
    root: String,
}

fn spawn_err(program: &str, e: std::io::Error) -> RivetError {
    let (kind, code) = match e.kind() {
        std::io::ErrorKind::NotFound => (ErrorKind::NotFound, "not_found.program"),
        std::io::ErrorKind::PermissionDenied => (ErrorKind::Process, "process.not_executable"),
        _ => (ErrorKind::Process, "process.spawn"),
    };
    RivetError::new(kind, code, format!("cannot start `{program}`: {e}"))
}

impl TokioRunner {
    pub fn new(root: &str) -> TokioRunner {
        TokioRunner {
            root: root.to_string(),
        }
    }

    fn command(&self, plan: &ProcessPlan) -> RivetResult<Command> {
        let mut cmd = match &plan.sandbox {
            Some(spec) => sandbox::command(spec, &plan.resolved, &plan.args)?,
            None => {
                let mut c = Command::new(&plan.resolved);
                c.args(&plan.args);
                c
            }
        };
        cmd.env_clear();
        cmd.env("PATH", CHILD_PATH);
        for (k, v) in &plan.env {
            cmd.env(k, v);
        }
        cmd.current_dir(plan.cwd.clone().unwrap_or_else(|| self.root.clone()));
        cmd.stdin(if plan.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        Ok(cmd)
    }

    fn spawn_child(&self, plan: &ProcessPlan) -> RivetResult<Child> {
        let mut cmd = self.command(plan)?;
        let mut child = cmd.spawn().map_err(|e| spawn_err(&plan.program, e))?;
        if let (Some(mut stdin), Some(bytes)) = (
            child.stdin.take(),
            plan.stdin.as_ref().and_then(|s| s.bytes.clone()),
        ) {
            tokio::spawn(async move {
                let _ = stdin.write_all(&bytes).await;
                let _ = stdin.shutdown().await;
            });
        }
        Ok(child)
    }
}

/// A one-shot child that is killed (whole group) and reaped in the background
/// if its owner is dropped before it exited — a cancelled `command` never
/// leaves a zombie or a stray process group behind.
struct ReapOnDrop(Option<Child>);

impl Drop for ReapOnDrop {
    fn drop(&mut self) {
        let Some(mut child) = self.0.take() else {
            return;
        };
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        if let Some(pid) = child.id() {
            signal_group(pid, KILL);
        }
        let _ = child.start_kill();
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            rt.spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
}

/// Kill the child's whole process group, then the child itself, and reap it.
async fn kill_and_reap(child: &mut Child) {
    if let Some(pid) = child.id() {
        signal_group(pid, KILL);
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
const KILL: i32 = libc::SIGKILL;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TERM: i32 = libc::SIGTERM;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const KILL: i32 = 9;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const TERM: i32 = 15;

/// Signal the child's process group (created with `process_group(0)`).
fn signal_group(pid: u32, sig: i32) {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    // SAFETY: plain syscall targeting only the group this runner created.
    unsafe {
        libc::killpg(pid as libc::pid_t, sig);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = (pid, sig);
}

async fn read_bounded<R: AsyncRead + Unpin>(
    mut r: R,
    max: u64,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            return Ok((out, false));
        }
        if (out.len() + n) as u64 > max {
            return Ok((out, true));
        }
        out.extend_from_slice(&buf[..n]);
    }
}

fn exit_code(status: std::process::ExitStatus) -> i64 {
    if let Some(c) = status.code() {
        return c as i64;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig as i64;
        }
    }
    -1
}

#[async_trait]
impl ProcessRunner for TokioRunner {
    async fn run(&self, plan: &ProcessPlan) -> RivetResult<ProcessResult> {
        let start = Instant::now();
        // Reaped even when this future is dropped (cancelled one-shot command).
        let mut guard = ReapOnDrop(Some(self.spawn_child(plan)?));
        let Some(child) = guard.0.as_mut() else {
            return Err(RivetError::internal("the spawned child is missing"));
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let max = plan.max_output;
        let work = async {
            let out = async {
                match stdout {
                    Some(s) => read_bounded(s, max).await,
                    None => Ok((Vec::new(), false)),
                }
            };
            let err = async {
                match stderr {
                    Some(s) => read_bounded(s, max).await,
                    None => Ok((Vec::new(), false)),
                }
            };
            tokio::join!(out, err)
        };
        let (out, err) =
            match tokio::time::timeout(Duration::from_millis(plan.timeout_ms), work).await {
                Ok(r) => r,
                Err(_) => {
                    kill_and_reap(child).await;
                    return Err(RivetError::new(
                        ErrorKind::Timeout,
                        "timeout.process",
                        format!(
                            "`{}` exceeded {} ms and was killed",
                            plan.program, plan.timeout_ms
                        ),
                    ));
                }
            };
        let io = |e: std::io::Error| {
            RivetError::new(
                ErrorKind::Process,
                "process.io",
                format!("reading child output failed: {e}"),
            )
        };
        let (stdout, over_out) = out.map_err(io)?;
        let (stderr, over_err) = err.map_err(io)?;
        if over_out || over_err {
            kill_and_reap(child).await;
            return Err(RivetError::new(
                ErrorKind::Limit,
                "limit.process_output",
                format!(
                    "`{}` produced more than {max} bytes of output",
                    plan.program
                ),
            ));
        }
        let left = Duration::from_millis(plan.timeout_ms).saturating_sub(start.elapsed());
        let status = match tokio::time::timeout(left, child.wait()).await {
            Ok(s) => s.map_err(|e| spawn_err(&plan.program, e))?,
            Err(_) => {
                kill_and_reap(child).await;
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.process",
                    format!(
                        "`{}` exceeded {} ms and was killed",
                        plan.program, plan.timeout_ms
                    ),
                ));
            }
        };
        Ok(ProcessResult {
            exit_status: exit_code(status),
            stdout,
            stderr,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn spawn_duplex(&self, plan: &ProcessPlan) -> RivetResult<ChildDuplex> {
        let mut cmd = self.command(plan)?;
        cmd.stdin(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| spawn_err(&plan.program, e))?;
        let stdin = child.stdin.take().ok_or_else(|| {
            RivetError::new(
                ErrorKind::Process,
                "process.io",
                "child stdin is unavailable",
            )
        })?;
        let stdout = self.stream_child(plan, child);
        Ok(ChildDuplex {
            stdin: Box::new(StdinSink { stdin: Some(stdin) }),
            stdout,
        })
    }

    async fn spawn(&self, plan: &ProcessPlan) -> RivetResult<Box<dyn ByteStream>> {
        let child = self.spawn_child(plan)?;
        Ok(self.stream_child(plan, child))
    }
}

/// A child's stdin kept open for protocol traffic (MCP stdio).
struct StdinSink {
    stdin: Option<tokio::process::ChildStdin>,
}

#[async_trait]
impl ByteSink for StdinSink {
    async fn write(&mut self, bytes: &[u8]) -> RivetResult<()> {
        let io = |e: std::io::Error| {
            RivetError::new(
                ErrorKind::Process,
                "process.io",
                format!("writing child stdin failed: {e}"),
            )
        };
        let s = self
            .stdin
            .as_mut()
            .ok_or_else(|| io(std::io::Error::other("stdin closed")))?;
        s.write_all(bytes).await.map_err(io)?;
        s.flush().await.map_err(io)
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        if let Some(mut s) = self.stdin.take() {
            let _ = s.shutdown().await;
        }
        Ok(())
    }
}

impl TokioRunner {
    /// Wrap a spawned child: stdout becomes the stream, stderr is drained into a bounded tail.
    fn stream_child(&self, plan: &ProcessPlan, mut child: Child) -> Box<dyn ByteStream> {
        let stdout = child.stdout.take();
        let stderr_tail = Arc::new(tokio::sync::Mutex::new(Vec::<u8>::new()));
        if let Some(mut err) = child.stderr.take() {
            let tail = Arc::clone(&stderr_tail);
            // Drain stderr so the child can never block on a full pipe; keep a bounded tail.
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                while let Ok(n) = err.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    let mut t = tail.lock().await;
                    t.extend_from_slice(&buf[..n]);
                    let excess = t.len().saturating_sub(64 * 1024);
                    t.drain(..excess);
                }
            });
        }
        Box::new(ProcStream {
            child: Some(child),
            stdout,
            stderr_tail,
            accept: if plan.accept_exit.is_empty() {
                vec![0]
            } else {
                plan.accept_exit.clone()
            },
            program: plan.program.clone(),
        })
    }
}

/// A running child whose stdout is the stream; owned by the `with` scope.
struct ProcStream {
    child: Option<Child>,
    stdout: Option<ChildStdout>,
    stderr_tail: Arc<tokio::sync::Mutex<Vec<u8>>>,
    accept: Vec<i64>,
    program: String,
}

#[async_trait]
impl ByteStream for ProcStream {
    async fn next_chunk(&mut self) -> RivetResult<Option<Vec<u8>>> {
        let Some(out) = self.stdout.as_mut() else {
            return Ok(None);
        };
        let mut buf = vec![0u8; 64 * 1024];
        let n = out.read(&mut buf).await.map_err(|e| {
            RivetError::new(
                ErrorKind::Process,
                "process.io",
                format!("reading stdout failed: {e}"),
            )
        })?;
        if n == 0 {
            self.stdout = None;
            return Ok(None);
        }
        buf.truncate(n);
        Ok(Some(buf))
    }

    async fn finish(&mut self) -> RivetResult<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let status = child
            .wait()
            .await
            .map_err(|e| spawn_err(&self.program, e))?;
        self.child = None;
        let code = exit_code(status);
        if !self.accept.contains(&code) {
            let tail = String::from_utf8_lossy(&self.stderr_tail.lock().await).into_owned();
            return Err(RivetError::new(
                ErrorKind::Process,
                "process.exit",
                format!("`{}` exited with status {code}", self.program),
            )
            .with_details(Value::object([
                ("exit", Value::Int(code)),
                ("stderr", Value::Text(tail)),
            ])));
        }
        Ok(())
    }

    async fn close(mut self: Box<Self>) -> RivetResult<()> {
        self.stdout = None;
        if let Some(mut child) = self.child.take() {
            if let Ok(Some(_)) = child.try_wait() {
                return Ok(());
            }
            // SIGTERM the group, then SIGKILL after a short grace; always reap.
            if let Some(pid) = child.id() {
                signal_group(pid, TERM);
            }
            if tokio::time::timeout(Duration::from_millis(1500), child.wait())
                .await
                .is_err()
            {
                kill_and_reap(&mut child).await;
            }
        }
        Ok(())
    }
}

/// The `command` effect kind: one-shot `command BIN …` and `with command BIN as p`.
pub struct ProcessEffects {
    run: Arc<RunProcessFn>,
    runner: TokioRunner,
    codec: StdCodec,
    root: String,
}

impl ProcessEffects {
    pub fn new(run: Arc<RunProcessFn>, root: &str) -> ProcessEffects {
        ProcessEffects {
            run,
            runner: TokioRunner::new(root),
            codec: StdCodec,
            root: root.to_string(),
        }
    }

    fn resolve(&self, program: &str) -> String {
        if Path::new(program).is_absolute() {
            program.to_string()
        } else {
            Path::new(&self.root)
                .join(program)
                .to_string_lossy()
                .into_owned()
        }
    }

    fn plan(&self, ctx: &EffectCtx, f: &EvaluatedForm, scoped: bool) -> RivetResult<ProcessPlan> {
        let program = text(f.head.first())
            .ok_or_else(|| bad("syntax.command", "expected `command \"/path/to/binary\"`"))?;
        let mut plan = ProcessPlan {
            resolved: self.resolve(&program),
            program,
            args: Vec::new(),
            env: Vec::new(),
            cwd: None,
            stdin: None,
            timeout_ms: 0,
            accept_exit: Vec::new(),
            decode_stdout: None,
            decode_stderr: None,
            stream: None,
            sandbox: None,
            max_output: DEFAULT_MAX_OUTPUT,
            interactive: false,
            origin: origin(ctx),
        };
        let mut timeout = None;
        for (_, k, args) in options(f) {
            match k {
                "args" => match args.first() {
                    Some(EvalArg::Value(Value::List(items))) => {
                        plan.args = items.iter().map(Value::to_display).collect()
                    }
                    _ => {
                        return Err(bad(
                            "validation.command_option",
                            "expected `args [\"…\", …]`",
                        ));
                    }
                },
                "env" => match args.first() {
                    Some(EvalArg::Value(Value::Object(pairs))) => {
                        plan.env = pairs
                            .iter()
                            .map(|(k, v)| (k.clone(), v.to_display()))
                            .collect()
                    }
                    _ => {
                        return Err(bad(
                            "validation.command_option",
                            "expected `env {NAME: \"value\"}`",
                        ));
                    }
                },
                "stdin" => {
                    let kind = word(args.first())
                        .and_then(CodecKind::parse)
                        .ok_or_else(|| {
                            bad(
                                "validation.command_option",
                                "expected `stdin json|text|bytes VALUE`",
                            )
                        })?;
                    let value = match args.get(1) {
                        Some(EvalArg::Value(v)) => v.clone(),
                        _ => Value::Null,
                    };
                    plan.stdin = Some(CodecInput {
                        kind,
                        bytes: None,
                        value: Some(value),
                    });
                }
                "timeout" => timeout = duration(args.first()),
                "decode" => {
                    let which = word(args.first()).unwrap_or("");
                    let kind = word(args.get(1))
                        .and_then(CodecKind::parse)
                        .ok_or_else(|| {
                            bad(
                                "validation.command_option",
                                "expected `decode stdout|stderr json|text|bytes`",
                            )
                        })?;
                    match which {
                        "stdout" => plan.decode_stdout = Some(kind),
                        "stderr" => plan.decode_stderr = Some(kind),
                        _ => {
                            return Err(bad(
                                "validation.command_option",
                                "expected `decode stdout|stderr T`",
                            ));
                        }
                    }
                }
                "accept" => {
                    plan.accept_exit = int_list(args.get(1)).ok_or_else(|| {
                        bad(
                            "validation.command_option",
                            "expected `accept exit [CODES]`",
                        )
                    })?;
                }
                "stream" if scoped => {
                    if word(args.first()) != Some("stdout") {
                        return Err(bad(
                            "validation.command_option",
                            "only `stream stdout lines|jsonl|bytes` is available",
                        ));
                    }
                    plan.stream = Some(word(args.get(1)).and_then(StreamMode::parse).ok_or_else(
                        || {
                            bad(
                                "validation.command_option",
                                "expected `stream stdout lines|jsonl|bytes`",
                            )
                        },
                    )?);
                }
                "interactive" => plan.interactive = boolean(args.first()).unwrap_or(false),
                "cwd" => plan.cwd = text(args.first()).map(|c| self.resolve(&c)),
                other => {
                    return Err(bad(
                        "validation.command_option",
                        format!("option `{other}` is not valid for command"),
                    ));
                }
            }
        }
        if scoped && plan.stream.is_none() && !plan.interactive {
            return Err(bad(
                "validation.command_stream",
                "`with command … as NAME` needs `stream stdout lines|jsonl|bytes`",
            ));
        }
        plan.timeout_ms = budget_ms(ctx, timeout);
        Ok(plan)
    }
}

#[async_trait]
impl EffectAdapter for ProcessEffects {
    async fn run(
        &self,
        ctx: &EffectCtx,
        _form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Value> {
        let plan = self.plan(ctx, &args, false)?;
        match (self.run)(plan, ctx.policy.as_ref(), &self.runner, &self.codec).await? {
            ProcessOutcome::Finished(v) => Ok(v),
            ProcessOutcome::Streaming(s) => {
                let _ = s.close().await;
                Err(RivetError::internal("a one-shot command produced a stream"))
            }
        }
    }

    async fn open(
        &self,
        ctx: &EffectCtx,
        _form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let plan = self.plan(ctx, &args, true)?;
        let mode = plan.stream.unwrap_or(StreamMode::Lines);
        let decode_json = mode == StreamMode::Jsonl || plan.decode_stdout == Some(CodecKind::Json);
        let deadline = Instant::now() + Duration::from_millis(plan.timeout_ms);
        match (self.run)(plan, ctx.policy.as_ref(), &self.runner, &self.codec).await? {
            ProcessOutcome::Streaming(s) => Ok(Box::new(StreamHandle {
                stream: Some(s),
                decoder: StreamDecoder::new(mode, decode_json, MAX_STREAM_ITEM),
                response: None,
                done: false,
                deadline,
                member: Some("stdout"),
            })),
            ProcessOutcome::Finished(_) => {
                Err(RivetError::internal("a scoped command did not stream"))
            }
        }
    }
}
