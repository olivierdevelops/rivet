//! Host-owned live sessions (PROP-2026-0001 Increment 14).
//!
//! ```text
//!  open ──▶ spawn run task (launch closure → shared dispatcher, DataSink → EventLog, input rx → `incoming`)
//!  send ──▶ InputSequencer check ──▶ bounded input channel (16)            read ──▶ EventLog ack + batch (long-poll)
//!  cancel ─▶ mark cancel-requested, drop input, fire the request token ─▶ the run closes its
//!            handles and ends (joined within the 5 s grace; aborted only after it)
//!            ─▶ terminal `cancelled` event (cancel wins over a later completion), retained ≤ 60 s
//!  sweeper (background, no session call needed) ─▶ idle lease expiry cancels │ retention expiry evicts
//! ```

use crate::domain::cancel::{CancelReason, CancelToken};
use crate::domain::contracts::{
    CatalogQuery, Completion, DataEvent, Envelope, MAX_DEADLINE_MS, Principal, RegistryEntry,
    Request,
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
use std::sync::{Arc, Mutex, Weak};
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
/// Extra time after the grace before a run task that ignored its token is aborted.
const JOIN_MARGIN: Duration = Duration::from_secs(1);

type SessionMap = Mutex<HashMap<String, Arc<Session>>>;

// vhco:infra session_driver satisfies SessionDriver
pub struct SessionHost {
    registry: Arc<dyn Registry>,
    launch: Arc<LaunchFn>,
    mint: Arc<MintFn>,
    validate: Arc<ValidateFn>,
    catalog_version: String,
    limits: SessionLimits,
    sessions: Arc<SessionMap>,
    counter: AtomicU64,
    /// The background sweeper starts with the first session.
    sweeper: std::sync::Once,
}

struct State {
    seq: InputSequencer,
    log: EventLog,
    last_touch: Instant,
    terminal_at: Option<Instant>,
    /// Terminal state kept after the terminal event is acknowledged and evicted.
    terminal_state: Option<&'static str>,
    /// A cancel (caller, idle lease, shutdown) arrived before the terminal
    /// event was recorded: the session ends `cancelled` with this code.
    cancel_requested: Option<(&'static str, &'static str)>,
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
    /// The run's structured cancellation token.
    token: CancelToken,
}

impl Session {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Record the terminal event once (run finished, cancelled or expired).
    /// A cancel requested before this point wins over the run's own outcome
    /// (committed effects stay reported on the cancelled error).
    fn finish(&self, outcome: RivetResult<Completion>) {
        {
            let mut st = self.st();
            if st.log.is_terminal() {
                return;
            }
            let outcome = match (st.cancel_requested, outcome) {
                // The run's own `cancelled` becomes the session's reason
                // (cancelled.session / cancelled.idle / cancelled.shutdown),
                // keeping its effects and suppressed cleanup errors.
                (Some((code, message)), Err(run)) if run.kind == ErrorKind::Cancelled => {
                    let mut e = self.cancelled_error(code, message);
                    e.effects = run.effects;
                    e.suppressed = run.suppressed;
                    Err(e)
                }
                (Some((code, message)), other) => {
                    let effects = match &other {
                        Ok(c) => c.effects,
                        Err(e) => e.effects,
                    };
                    let mut e = self.cancelled_error(code, message);
                    e.effects = effects;
                    if let Err(cause) = other {
                        e.suppressed.push(cause);
                    }
                    Err(e)
                }
                (None, other) => other,
            };
            let envelope = terminal_envelope(&self.request_id, &self.trace_id, outcome);
            st.log.push(envelope);
            st.terminal_state = st.log.terminal_event().map(terminal_state);
            st.terminal_at = Some(Instant::now());
        }
        self.changed.notify_waiters();
    }

    /// Request cancellation: unless the terminal event is already recorded,
    /// remember the cancel (it wins the race with completion), then fire the
    /// run's token so it unwinds and closes its resources.
    fn request_cancel(&self, code: &'static str, message: &'static str) {
        {
            let mut st = self.st();
            if !st.log.is_terminal() && st.cancel_requested.is_none() {
                st.cancel_requested = Some((code, message));
            }
            st.seq.closed = true;
        }
        self.token.cancel(CancelReason::Cancelled);
    }

    /// Join the run task within the grace; abort it only after (last resort).
    async fn join(&self) {
        let handle = self.task.lock().ok().and_then(|mut t| t.take());
        if let Some(mut h) = handle
            && tokio::time::timeout(CLEANUP_GRACE + JOIN_MARGIN, &mut h)
                .await
                .is_err()
        {
            h.abort();
            let _ = h.await;
        }
    }

    fn cancelled_error(&self, code: &str, message: &str) -> RivetError {
        let mut e = RivetError::new(ErrorKind::Cancelled, code, message);
        e.request_id = Some(self.request_id.clone());
        e.trace_id = Some(self.trace_id.clone());
        e
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
            sessions: Arc::new(Mutex::new(HashMap::new())),
            counter: AtomicU64::new(0),
            sweeper: std::sync::Once::new(),
        }
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Session>>> {
        lock_map(&self.sessions)
    }

    /// Expire retained terminal sessions and cancel idle ones.
    fn sweep(&self) {
        sweep(&self.sessions, &self.limits);
    }

    /// Start the background sweeper (once, from inside the async runtime): it
    /// enforces idle leases and retention without any session call, and stops
    /// when the host is dropped.
    fn start_sweeper(&self) {
        self.sweeper.call_once(|| {
            let map: Weak<SessionMap> = Arc::downgrade(&self.sessions);
            let limits = self.limits.clone();
            let every = Duration::from_millis(
                (limits.idle_ms.min(limits.retention_ms) / 4).clamp(20, 1000),
            );
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(every).await;
                    let Some(map) = map.upgrade() else { break };
                    sweep(&map, &limits);
                }
            });
        });
    }

    /// Host shutdown: cancel every live session (each run closes its resources).
    pub async fn cancel_all(&self) {
        let live: Vec<Arc<Session>> = self.map().values().cloned().collect();
        for s in &live {
            s.request_cancel("cancelled.shutdown", "the host is shutting down");
            s.input.lock().await.take();
        }
        let joins = live.iter().map(|s| async move {
            s.join().await;
            s.finish(Err(s.cancelled_error(
                "cancelled.shutdown",
                "the host is shutting down",
            )));
        });
        futures_util::future::join_all(joins).await;
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
        // Requested total deadline, capped by the host (10 minutes).
        if let Some(d) = input.deadline_ms {
            req.deadline_ms = d.clamp(1, MAX_DEADLINE_MS);
        }
        if let Some(t) = &input.trace {
            req.trace_id = t.trace_id.clone();
            req.parent_span_id = Some(t.parent_id.clone());
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
                terminal_state: None,
                cancel_requested: None,
            }),
            changed: Notify::new(),
            input: tokio::sync::Mutex::new(entry.receives.as_ref().map(|_| tx)),
            read_lock: tokio::sync::Mutex::new(()),
            task: Mutex::new(None),
            token: req.cancel.clone(),
        });
        self.start_sweeper();
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
                    "input item {} at {} must be {}, got {}",
                    input.send_seq,
                    if v.path.is_empty() { "$" } else { &v.path },
                    v.expected,
                    v.found
                ),
            )
            .with_details(Value::object([
                ("seq", Value::Int(input.send_seq as i64)),
                ("path", Value::text(&v.path)),
                ("expected", Value::text(&v.expected)),
                ("found", Value::text(&v.found)),
            ])));
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
        // Recorded before anything else: a completion that lands after this
        // point still ends the session `cancelled`; an already-recorded
        // terminal event keeps its own state (succeeded/failed).
        s.request_cancel("cancelled.session", "the session was cancelled");
        s.input.lock().await.take();
        s.join().await;
        s.finish(Err(s.cancelled_error(
            "cancelled.session",
            "the session was cancelled",
        )));
        let state = s.st().terminal_state.unwrap_or("cancelled").to_string();
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

fn lock_map(map: &SessionMap) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Session>>> {
    map.lock().unwrap_or_else(|p| p.into_inner())
}

/// One sweep: evict terminal sessions past retention; cancel sessions whose
/// idle lease expired (their runs unwind and are joined in the background).
fn sweep(map: &SessionMap, limits: &SessionLimits) {
    let now = Instant::now();
    let retention = Duration::from_millis(limits.retention_ms);
    let idle = Duration::from_millis(limits.idle_ms);
    let mut idle_sessions = Vec::new();
    lock_map(map).retain(|_, s| {
        let st = s.st();
        match st.terminal_at {
            Some(t) => now.duration_since(t) < retention,
            None => {
                if st.cancel_requested.is_none() && now.duration_since(st.last_touch) >= idle {
                    idle_sessions.push(Arc::clone(s));
                }
                true
            }
        }
    });
    for s in idle_sessions {
        s.request_cancel("cancelled.idle", "the session's idle lease expired");
        tokio::spawn(async move {
            s.input.lock().await.take();
            s.join().await;
            s.finish(Err(s.cancelled_error(
                "cancelled.idle",
                "the session's idle lease expired",
            )));
        });
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

    struct OneOp;

    impl Registry for OneOp {
        fn describe(&self, _: &CatalogQuery) -> RivetResult<crate::domain::contracts::Catalog> {
            Ok(crate::domain::contracts::Catalog {
                entries: vec![RegistryEntry {
                    id: "t.race".into(),
                    name: "race".into(),
                    description: None,
                    kind: crate::domain::ir::OperationKind::Operation,
                    private: false,
                    params: vec![],
                    output: crate::domain::outputs::OutputSpec {
                        spec: ValueSpec::Json,
                        description: None,
                    },
                    emits: None,
                    receives: None,
                    errors: vec![],
                    source: Default::default(),
                    raw_input_schema: None,
                    raw_output_schema: None,
                }],
            })
        }
        fn program(&self) -> Arc<crate::domain::ir::CompiledProgram> {
            unreachable!("not used by the session host")
        }
    }

    // vhco:test sessions.cancel_session -- G16: a cancel that arrives before the terminal event is recorded wins the race: a run that still completes successfully afterwards ends the session `cancelled` (effects kept)
    #[tokio::test]
    async fn cancel_wins_the_race_with_completion() {
        // The run ignores cancellation and completes successfully once its
        // token fired: exactly the race a cancel must win.
        let launch: Arc<LaunchFn> = Arc::new(|req: Request, _sink, _input| {
            Box::pin(async move {
                req.cancel.cancelled().await;
                Ok(Completion {
                    request_id: req.request_id,
                    trace_id: req.trace_id,
                    result: Value::Int(1),
                    data_count: 0,
                    effects: crate::domain::errors::EffectsStatus::Committed,
                })
            })
        });
        let mint: Arc<MintFn> = Arc::new(|id: &str, params: Value, principal: Principal| Request {
            request_id: "req_race".into(),
            trace_id: "tr_race".into(),
            operation_id: id.to_string(),
            params,
            principal,
            parent_request_id: None,
            depth: 0,
            deadline_ms: 30_000,
            include_private: false,
            parent_span_id: None,
            cancel: CancelToken::new(),
        });
        let validate: Arc<ValidateFn> = Arc::new(|_: &RegistryEntry, v: &Value| Ok(v.clone()));
        let host = SessionHost::new(
            Arc::new(OneOp),
            launch,
            mint,
            validate,
            "sha256:x".into(),
            SessionLimits::default(),
        );
        let me = Principal::local();
        let r = host
            .open(SessionOpenInput {
                id: "t.race".into(),
                params: Value::Null,
                principal: me.clone(),
                connection_owned: false,
                deadline_ms: None,
                trace: None,
            })
            .await
            .unwrap();
        let c = host
            .cancel(SessionRef {
                session_id: r.session_id.clone(),
                principal: me.clone(),
            })
            .await
            .unwrap();
        assert_eq!(c.state, "cancelled");
        let b = host
            .read(SessionReadInput {
                session_id: r.session_id,
                after_seq: 0,
                max_events: None,
                wait_ms: Some(0),
                principal: me,
            })
            .await
            .unwrap();
        let t = b.events[0].to_json();
        assert_eq!(t["error"]["code"], "cancelled.session", "{t}");
        assert_eq!(t["error"]["effects"], "committed");
    }
}
