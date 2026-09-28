//! Host-owned live sessions (PROP-2026-0001 Increment 14).
//!
//! ```text
//!  open ──▶ spawn run task (launch closure → shared dispatcher, DataSink → EventLog, input rx → `incoming`)
//!  send ──▶ InputSequencer check ──▶ bounded input channel (16)            read ──▶ EventLog ack + batch (long-poll)
//!  cancel ─▶ drop input, abort + join run (5 s grace) ─▶ terminal `cancelled` event, retained ≤ 60 s
//! ```

use crate::domain::contracts::{
    CatalogQuery, Completion, DataEvent, Envelope, Principal, RegistryEntry, Request,
};
use crate::domain::errors::ErrorKind;
use crate::domain::outputs::ValueSpec;
use crate::domain::ports::{DataSink, Registry, SessionDriver};
use crate::domain::sessions::{
    CancelReceipt, EventLog, InputSequencer, SendDecision, SessionAck, SessionBatch, SessionLimits,
    SessionOpenInput, SessionReadInput, SessionReceipt, SessionRef, SessionSendInput,
    terminal_envelope, terminal_state,
};
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Notify, mpsc};

/// Runs one session request through the shared dispatcher with a sink and the input feed.
pub type LaunchFn = dyn Fn(
        Request,
        Arc<dyn DataSink>,
        mpsc::Receiver<Value>,
    ) -> BoxFuture<'static, RivetResult<Completion>>
    + Send
    + Sync;
/// Mints a top-level request (IDs, principal, default deadline).
pub type MintFn = dyn Fn(&str, Value, Principal) -> Request + Send + Sync;
/// The dispatcher's parameter validation (unknown fields, types, required, defaults).
pub type ValidateFn = dyn Fn(&RegistryEntry, &Value) -> RivetResult<Value> + Send + Sync;

const ACTION_DEADLINE: Duration = Duration::from_secs(5);
const CLEANUP_GRACE: Duration = Duration::from_secs(5);

// vhco:infra session_driver satisfies SessionDriver
pub struct SessionHost {
    registry: Arc<dyn Registry>,
    launch: Arc<LaunchFn>,
    mint: Arc<MintFn>,
    validate: Arc<ValidateFn>,
    catalog_version: String,
    limits: SessionLimits,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    counter: AtomicU64,
}

struct State {
    seq: InputSequencer,
    log: EventLog,
    last_touch: Instant,
    terminal_at: Option<Instant>,
}

struct Session {
    id: String,
    request_id: String,
    trace_id: String,
    owner: String,
    connection_owned: bool,
    receives: Option<ValueSpec>,
    queue_frames: usize,
    state: Mutex<State>,
    changed: Notify,
    input: tokio::sync::Mutex<Option<mpsc::Sender<Value>>>,
    read_lock: tokio::sync::Mutex<()>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Session {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Record the terminal event once (run finished, cancelled or expired).
    fn finish(&self, outcome: RivetResult<Completion>) {
        {
            let mut st = self.st();
            if st.log.is_terminal() {
                return;
            }
            st.log
                .push(terminal_envelope(&self.request_id, &self.trace_id, outcome));
            st.terminal_at = Some(Instant::now());
        }
        self.changed.notify_waiters();
    }

    fn cancelled_error(&self, code: &str, message: &str) -> RivetError {
        let mut e = RivetError::new(ErrorKind::Cancelled, code, message);
        e.request_id = Some(self.request_id.clone());
        e.trace_id = Some(self.trace_id.clone());
        e
    }

    fn abort(&self) -> Option<tokio::task::JoinHandle<()>> {
        let h = self.task.lock().ok().and_then(|mut t| t.take());
        if let Some(h) = &h {
            h.abort();
        }
        h
    }
}

/// Data items of the run go into the session's event log; the producer waits
/// while `queue_frames` events are retained (backpressure).
struct SessionSink {
    session: Arc<Session>,
}

#[async_trait]
impl DataSink for SessionSink {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        let s = &self.session;
        loop {
            let notified = s.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut st = s.st();
                if st.log.is_terminal() {
                    return Err(s.cancelled_error("cancelled.session", "the session ended"));
                }
                if st.log.retained() < s.queue_frames {
                    st.log.push(Envelope::Data(event));
                    drop(st);
                    s.changed.notify_waiters();
                    return Ok(());
                }
            }
            notified.await;
        }
    }
}

impl SessionHost {
    pub fn new(
        registry: Arc<dyn Registry>,
        launch: Arc<LaunchFn>,
        mint: Arc<MintFn>,
        validate: Arc<ValidateFn>,
        catalog_version: String,
        limits: SessionLimits,
    ) -> SessionHost {
        SessionHost {
            registry,
            launch,
            mint,
            validate,
            catalog_version,
            limits,
            sessions: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Session>>> {
        self.sessions.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Expire retained terminal sessions and cancel idle ones.
    fn sweep(&self) {
        let now = Instant::now();
        let retention = Duration::from_millis(self.limits.retention_ms);
        let idle = Duration::from_millis(self.limits.idle_ms);
        let mut idle_sessions = Vec::new();
        self.map().retain(|_, s| {
            let st = s.st();
            match st.terminal_at {
                Some(t) => now.duration_since(t) < retention,
                None => {
                    if now.duration_since(st.last_touch) >= idle {
                        idle_sessions.push(Arc::clone(s));
                    }
                    true
                }
            }
        });
        for s in idle_sessions {
            s.abort();
            s.finish(Err(s.cancelled_error(
                "cancelled.idle",
                "the session's idle lease expired",
            )));
        }
    }

    fn get(&self, id: &str, principal: &Principal) -> RivetResult<Arc<Session>> {
        self.sweep();
        self.map()
            .get(id)
            .filter(|s| s.owner == principal.name)
            .cloned()
            .ok_or_else(|| RivetError::not_found("not_found.session", format!("no session `{id}`")))
    }

    fn touch(&self, s: &Session) {
        s.st().last_touch = Instant::now();
    }
}

#[async_trait]
impl SessionDriver for SessionHost {
    async fn open(&self, input: SessionOpenInput) -> RivetResult<SessionReceipt> {
        self.sweep();
        let entry = self
            .registry
            .describe(&CatalogQuery {
                ids: vec![input.id.clone()],
                include_private: false,
            })?
            .entries
            .into_iter()
            .next()
            .ok_or_else(|| {
                RivetError::not_found(
                    "not_found.operation",
                    format!("no operation `{}`", input.id),
                )
            })?;
        (self.validate)(&entry, &input.params).map_err(|mut e| {
            e.operation_id.get_or_insert_with(|| entry.id.clone());
            e
        })?;
        if !input.connection_owned {
            let live = self
                .map()
                .values()
                .filter(|s| {
                    s.owner == input.principal.name
                        && !s.connection_owned
                        && s.st().terminal_at.is_none()
                })
                .count();
            if live >= self.limits.per_principal {
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.sessions",
                    format!(
                        "at most {} live sessions per principal",
                        self.limits.per_principal
                    ),
                ));
            }
        }
        let mut req = (self.mint)(&entry.id, input.params.clone(), input.principal.clone());
        if let Some(d) = input.deadline_ms {
            req.deadline_ms = d;
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let tag = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let session_id = format!("ses_{n:02}{:08x}", tag & 0xffff_ffff);
        let (tx, rx) = mpsc::channel::<Value>(self.limits.queue_frames);
        let session = Arc::new(Session {
            id: session_id.clone(),
            request_id: req.request_id.clone(),
            trace_id: req.trace_id.clone(),
            owner: input.principal.name.clone(),
            connection_owned: input.connection_owned,
            receives: entry.receives.clone(),
            queue_frames: self.limits.queue_frames,
            state: Mutex::new(State {
                seq: InputSequencer {
                    closed: entry.receives.is_none(),
                    ..InputSequencer::default()
                },
                log: EventLog::default(),
                last_touch: Instant::now(),
                terminal_at: None,
            }),
            changed: Notify::new(),
            input: tokio::sync::Mutex::new(entry.receives.as_ref().map(|_| tx)),
            read_lock: tokio::sync::Mutex::new(()),
            task: Mutex::new(None),
        });
        self.map().insert(session_id.clone(), Arc::clone(&session));
        let sink: Arc<dyn DataSink> = Arc::new(SessionSink {
            session: Arc::clone(&session),
        });
        let run = (self.launch)(req.clone(), sink, rx);
        let owned = Arc::clone(&session);
        let handle = tokio::spawn(async move {
            let outcome = run.await;
            owned.finish(outcome);
        });
        if let Ok(mut t) = session.task.lock() {
            *t = Some(handle);
        }
        let lease = Duration::from_millis(self.limits.idle_ms.min(req.deadline_ms.max(1)));
        Ok(SessionReceipt {
            session_id,
            request_id: req.request_id,
            trace_id: req.trace_id,
            catalog_version: self.catalog_version.clone(),
            input_schema: entry.receives.as_ref().map(ValueSpec::to_json_schema),
            emits_schema: entry.emits.as_ref().map(ValueSpec::to_json_schema),
            next_send_seq: 1,
            expires_at: rfc3339(SystemTime::now() + lease),
            events_url: None,
        })
    }

    async fn send(&self, input: SessionSendInput) -> RivetResult<SessionAck> {
        let s = self.get(&input.session_id, &input.principal)?;
        self.touch(&s);
        let Some(spec) = &s.receives else {
            return Err(RivetError::validation(
                "validation.no_input",
                "this operation does not declare `receives`",
            ));
        };
        let mut violations = Vec::new();
        spec.check(&input.data, "", &mut violations);
        if let Some(v) = violations.first() {
            return Err(RivetError::validation(
                "validation.input",
                format!(
                    "input item at {} must be {}, got {}",
                    v.path, v.expected, v.found
                ),
            ));
        }
        let hash: String = Sha256::digest(input.data.to_json().to_string().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        // Serialize sends: the sequencer check and the enqueue happen under the input lock.
        let guard = s.input.lock().await;
        let decision = s.st().seq.check(input.send_seq, &hash)?;
        let ack = SessionAck {
            session_id: s.id.clone(),
            accepted_seq: Some(input.send_seq),
            input_closed: false,
        };
        if decision == SendDecision::Duplicate {
            return Ok(ack);
        }
        let tx = guard.as_ref().ok_or_else(|| {
            RivetError::new(
                ErrorKind::Conflict,
                "conflict.input_closed",
                "input is finished",
            )
        })?;
        match tokio::time::timeout(ACTION_DEADLINE, tx.send(input.data)).await {
            Err(_) => {
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.input_queue",
                    "the input queue stayed full for 5 s",
                ));
            }
            Ok(Err(_)) => {
                return Err(RivetError::new(
                    ErrorKind::Conflict,
                    "conflict.session_terminal",
                    "the session's run already finished",
                ));
            }
            Ok(Ok(())) => {}
        }
        s.st().seq.accept(input.send_seq, &hash);
        Ok(ack)
    }

    async fn finish_input(&self, input: SessionRef) -> RivetResult<SessionAck> {
        let s = self.get(&input.session_id, &input.principal)?;
        self.touch(&s);
        let mut guard = s.input.lock().await;
        guard.take();
        s.st().seq.closed = true;
        Ok(SessionAck {
            session_id: s.id.clone(),
            accepted_seq: None,
            input_closed: true,
        })
    }

    async fn read(&self, input: SessionReadInput) -> RivetResult<SessionBatch> {
        let s = self.get(&input.session_id, &input.principal)?;
        let _one_consumer = s.read_lock.lock().await;
        self.touch(&s);
        let max = self.limits.clamp_max_events(input.max_events) as usize;
        let wait = Duration::from_millis(self.limits.clamp_wait(input.wait_ms) as u64);
        let until = Instant::now() + wait;
        // Acknowledge once; freed space wakes a producer blocked on a full log.
        s.st().log.ack(input.after_seq)?;
        s.changed.notify_waiters();
        loop {
            let notified = s.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut st = s.st();
                let ready = st.log.has_after(input.after_seq);
                if ready || Instant::now() >= until {
                    let events = st.log.batch(max);
                    let terminal = events.last().is_some_and(|e| e.is_terminal())
                        || (st.log.is_terminal() && !st.log.has_after(input.after_seq));
                    let last_seq = events.last().map(|e| e.seq).unwrap_or(input.after_seq);
                    return Ok(SessionBatch {
                        session_id: s.id.clone(),
                        events,
                        last_seq,
                        terminal,
                    });
                }
            }
            let _ = tokio::time::timeout(until.saturating_duration_since(Instant::now()), notified)
                .await;
        }
    }

    async fn cancel(&self, input: SessionRef) -> RivetResult<CancelReceipt> {
        let s = self.get(&input.session_id, &input.principal)?;
        self.touch(&s);
        s.input.lock().await.take();
        s.st().seq.closed = true;
        if let Some(h) = s.abort() {
            let _ = tokio::time::timeout(CLEANUP_GRACE, h).await;
        }
        s.finish(Err(s.cancelled_error(
            "cancelled.session",
            "the session was cancelled",
        )));
        let state = s
            .st()
            .log
            .terminal_event()
            .map(terminal_state)
            .unwrap_or("cancelled")
            .to_string();
        Ok(CancelReceipt {
            request_id: s.request_id.clone(),
            session_id: Some(s.id.clone()),
            state,
        })
    }

    fn limits(&self) -> SessionLimits {
        self.limits.clone()
    }
}

/// UTC RFC 3339 timestamp (seconds precision).
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil_from_days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_formats_epoch_and_leap_day() {
        assert_eq!(rfc3339(SystemTime::UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_709_164_800 + 3661);
        assert_eq!(rfc3339(t), "2024-02-29T01:01:01Z");
    }
}
