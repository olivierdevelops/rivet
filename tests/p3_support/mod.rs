//! Shared fixtures for the P3 conformance suites (T-07, T-10, T-16, T-23,
//! T-24): an instrumented slow HTTP server on 127.0.0.1:0 and small helpers
//! for building runtimes and driving the `rivet` binary.
#![allow(dead_code)]

use rivet::Runtime;
use rivet::domain::source::SourceBundle;
use rivet::domain::{RivetError, RivetResult};
use rivet::features::language::compile_program::compile_program;
use rivet::infra::capy_parser::CapyParser;
use rivet::orchestrator::runtime::policy_from_json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Compile one in-memory file (no runtime, no I/O).
pub fn compile(text: &str) -> RivetResult<rivet::domain::ir::CompiledProgram> {
    compile_program(
        &SourceBundle::single("app.rivet", text),
        &CapyParser::new().unwrap(),
    )
}

/// Every error of a failed compile: the primary one plus the suppressed ones.
pub fn all_errors(e: &RivetError) -> Vec<RivetError> {
    std::iter::once(e.clone())
        .chain(e.suppressed.iter().cloned())
        .collect()
}

/// `(line, col, end_line, end_col)` of an error's source span.
pub fn span(e: &RivetError) -> (u32, u32, u32, u32) {
    let s = e.source.as_ref().expect("error carries a source span");
    (s.start_line, s.start_col, s.end_line, s.end_col)
}

/// Build a runtime over in-memory source with a policy.json text.
pub fn runtime(src: &str, root: &str, policy: &str) -> Runtime {
    Runtime::builder()
        .source("app.rivet", src, root)
        .policy(policy_from_json(policy.as_bytes(), root).expect("policy"))
        .build()
        .expect("runtime")
}

/// A policy granting network access to one base origin.
pub fn net_policy(base: &str) -> String {
    format!(r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["{base}"]}}]}}"#)
}

/// What the slow fixture observed.
#[derive(Default)]
pub struct SlowStats {
    /// Requests currently being served (sleeping).
    pub in_flight: AtomicUsize,
    /// Highest value `in_flight` reached.
    pub max_in_flight: AtomicUsize,
    pub started: AtomicUsize,
    /// Responses fully written.
    pub finished: AtomicUsize,
    /// Clients that closed the connection before the response was due.
    pub aborted: AtomicUsize,
}

impl SlowStats {
    pub fn get(&self, f: fn(&SlowStats) -> &AtomicUsize) -> usize {
        f(self).load(Ordering::SeqCst)
    }

    /// Wait (≤ 5 s) until `aborted` reaches `n`.
    pub async fn wait_aborted(&self, n: usize) -> bool {
        for _ in 0..100 {
            if self.aborted.load(Ordering::SeqCst) >= n {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    /// Wait (≤ 5 s) until `started` reaches `n`.
    pub async fn wait_started(&self, n: usize) -> bool {
        for _ in 0..100 {
            if self.started.load(Ordering::SeqCst) >= n {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }
}

/// HTTP/1.1 fixture. `GET /sleep/<ms>/<tag>` waits `ms` then answers
/// `{"tag": tag}`; a client that closes first is counted in `aborted`.
/// `GET /status/<code>` answers that status; `GET /text` answers non-JSON text.
pub async fn slow_server() -> (u16, Arc<SlowStats>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let stats = Arc::new(SlowStats::default());
    let st = Arc::clone(&stats);
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else {
                return;
            };
            let st = Arc::clone(&st);
            tokio::spawn(async move {
                let mut s = BufReader::new(sock);
                let mut line = String::new();
                if s.read_line(&mut line).await.is_err() {
                    return;
                }
                loop {
                    let mut h = String::new();
                    if s.read_line(&mut h).await.unwrap_or(0) == 0 || h.trim().is_empty() {
                        break;
                    }
                }
                let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                let parts: Vec<&str> = target.trim_start_matches('/').split('/').collect();
                let (status, body) = match parts.as_slice() {
                    ["sleep", ms, tag] => {
                        let ms: u64 = ms.parse().unwrap_or(0);
                        st.started.fetch_add(1, Ordering::SeqCst);
                        let now = st.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                        st.max_in_flight.fetch_max(now, Ordering::SeqCst);
                        let mut probe = [0u8; 1];
                        let closed = tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(ms)) => false,
                            r = s.read(&mut probe) => matches!(r, Ok(0) | Err(_)),
                        };
                        st.in_flight.fetch_sub(1, Ordering::SeqCst);
                        if closed {
                            st.aborted.fetch_add(1, Ordering::SeqCst);
                            return;
                        }
                        (200, format!("{{\"tag\":\"{tag}\"}}"))
                    }
                    ["status", code] => (
                        code.parse().unwrap_or(500),
                        "{\"error\":\"fixture\"}".into(),
                    ),
                    ["text"] => (200, "not json at all".into()),
                    _ => (200, format!("{{\"target\":{}}}", serde_json::json!(target))),
                };
                let ctype = if parts == ["text"] {
                    "text/plain"
                } else {
                    "application/json"
                };
                let out = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let mut sock = s.into_inner();
                let _ = sock.write_all(out.as_bytes()).await;
                let _ = sock.shutdown().await;
                st.finished.fetch_add(1, Ordering::SeqCst);
            });
        }
    });
    (port, stats)
}

/// A port with nothing listening (bound, then released).
pub async fn closed_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    port
}

/// Output of one `rivet` binary run.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// The ErrorEnvelope `error` object printed on stderr.
    pub fn error(&self) -> serde_json::Value {
        let line = self
            .stderr
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('{'))
            .unwrap_or_else(|| panic!("no JSON on stderr: {}", self.stderr));
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        v.get("error").cloned().unwrap_or(v)
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.stdout.trim())
            .unwrap_or_else(|e| panic!("stdout not JSON ({e}): {}", self.stdout))
    }
}

/// Run the `rivet` binary with `args` in `dir` (blocking; call from a
/// blocking task when a local fixture must keep serving meanwhile).
pub fn rivet(dir: &std::path::Path, args: &[&str]) -> Run {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn rivet");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Async wrapper around [`rivet`] that keeps the test's fixtures serving.
pub async fn rivet_async(dir: &std::path::Path, args: &[&str]) -> Run {
    let dir = dir.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    tokio::task::spawn_blocking(move || {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        rivet(&dir, &refs)
    })
    .await
    .unwrap()
}
