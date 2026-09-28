//! Live sessions: principal-owned request lifetimes with sequenced input and a
//! bounded, replayable event log (PROP-2026-0001 Increment 14).
//!
//! ```text
//!  open ─▶ active (input open) ─finish_input─▶ active (input closed) ─terminal─▶ retained (≤60 s) ─▶ expired
//!            │ send(seq) / retry same seq          │ read(after_seq)
//!            └─ cancel / idle / deadline ─▶ cancelling ─▶ retained (terminal error)
//! ```
//!
//! The sequencing rules ([`InputSequencer`]) and the replay log ([`EventLog`])
//! are pure data so every surface (HTTP session operations, polling, WebSocket,
//! MCP, library) shares one implementation.

use super::contracts::{Completion, Envelope, Principal, TraceContext};
use super::envelope::ResponseEnvelope;
use super::errors::{ErrorKind, RivetError, RivetResult};
use super::value::Value;
use serde_json::{Value as Json, json};
use std::collections::VecDeque;

// vhco:domain SessionLimits { per_principal: int; queue_frames: int; queue_bytes: int; retention_ms: int; idle_ms: int; wait_default_ms: int; wait_max_ms: int; max_events_default: int; max_events_cap: int }
/// Session limits (Increment 14 defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct SessionLimits {
    pub per_principal: usize,
    pub queue_frames: usize,
    pub queue_bytes: u64,
    pub retention_ms: u64,
    pub idle_ms: u64,
    pub wait_default_ms: u32,
    pub wait_max_ms: u32,
    pub max_events_default: u32,
    pub max_events_cap: u32,
}

impl Default for SessionLimits {
    fn default() -> Self {
        SessionLimits {
            per_principal: 8,
            queue_frames: 16,
            queue_bytes: 32 * 1024 * 1024,
            retention_ms: 60_000,
            idle_ms: 60_000,
            wait_default_ms: 1000,
            wait_max_ms: 5000,
            max_events_default: 16,
            max_events_cap: 16,
        }
    }
}

impl SessionLimits {
    /// `wait_ms` default 1000, capped at 5000.
    pub fn clamp_wait(&self, wait_ms: Option<u32>) -> u32 {
        wait_ms
            .unwrap_or(self.wait_default_ms)
            .min(self.wait_max_ms)
    }

    /// `max_events` default 16, capped by the host, at least 1.
    pub fn clamp_max_events(&self, max: Option<u32>) -> u32 {
        max.unwrap_or(self.max_events_default)
            .clamp(1, self.max_events_cap)
    }
}

/// Maximum WebSocket refs in flight per connection (same as sessions per principal).
pub const MAX_WS_REFS: usize = 8;

// vhco:domain SessionOpenInput { id: string; params: Value; principal: Principal; connection_owned: bool; deadline_ms?: int; trace?: TraceContext; restrict?: Value }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionOpenInput {
    pub id: String,
    pub params: Value,
    pub principal: Principal,
    /// WebSocket refs are owned by their connection and do not count against
    /// the per-principal session limit (the connection enforces its own 8).
    pub connection_owned: bool,
    /// Requested total operation deadline; default 30 s, capped by the host at
    /// [`super::contracts::MAX_DEADLINE_MS`].
    pub deadline_ms: Option<u64>,
    /// The caller's W3C `traceparent` (its trace-id becomes the session's trace id).
    pub trace: Option<TraceContext>,
    /// Per-request restriction carried into the session's request (see `Request::restrict`).
    pub restrict: Option<Value>,
}

// vhco:domain SessionReceipt { session_id: string; request_id: string; trace_id: string; catalog_version: string; input_schema?: Json; emits_schema?: Json; next_send_seq: int; expires_at: string; events_url?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionReceipt {
    pub session_id: String,
    pub request_id: String,
    pub trace_id: String,
    pub catalog_version: String,
    pub input_schema: Option<Json>,
    pub emits_schema: Option<Json>,
    pub next_send_seq: u64,
    pub expires_at: String,
    pub events_url: Option<String>,
}

impl SessionReceipt {
    pub fn to_json(&self) -> Json {
        let mut j = json!({
            "session_id": self.session_id,
            "request_id": self.request_id,
            "trace_id": self.trace_id,
            "catalog_version": self.catalog_version,
            "input_schema": self.input_schema,
            "emits_schema": self.emits_schema,
            "next_send_seq": self.next_send_seq,
            "expires_at": self.expires_at,
        });
        if let Some(u) = &self.events_url {
            j["events_url"] = json!(u);
        }
        j
    }

    pub fn to_value(&self) -> Value {
        Value::from_json(&self.to_json())
    }
}

// vhco:domain SessionSendInput { session_id: string; send_seq: int; data: Value; principal: Principal }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionSendInput {
    pub session_id: String,
    pub send_seq: u64,
    pub data: Value,
    pub principal: Principal,
}

// vhco:domain SessionRef { session_id: string; principal: Principal }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRef {
    pub session_id: String,
    pub principal: Principal,
}

// vhco:domain SessionAck { session_id: string; accepted_seq?: int; input_closed: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionAck {
    pub session_id: String,
    pub accepted_seq: Option<u64>,
    pub input_closed: bool,
}

impl SessionAck {
    pub fn to_json(&self) -> Json {
        json!({"session_id": self.session_id, "accepted_seq": self.accepted_seq, "input_closed": self.input_closed})
    }
}

// vhco:domain SessionReadInput { session_id: string; after_seq: int; max_events?: int; wait_ms?: int; principal: Principal }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionReadInput {
    pub session_id: String,
    pub after_seq: u64,
    pub max_events: Option<u32>,
    pub wait_ms: Option<u32>,
    pub principal: Principal,
}

// vhco:domain SessionEvent { seq: int; envelope: Envelope }
/// One retained event; `seq` is the session's output sequence (1-based).
#[derive(Clone, Debug, PartialEq)]
pub struct SessionEvent {
    pub seq: u64,
    pub envelope: Envelope,
}

impl SessionEvent {
    pub fn is_terminal(&self) -> bool {
        !matches!(self.envelope, Envelope::Data(_))
    }

    /// The event's ResponseEnvelope record with its session `seq`: `type: data`
    /// items, then one `type: result` record (status ok, error or cancelled)
    /// whose `data_count` is the number of items before it.
    pub fn record(&self) -> ResponseEnvelope {
        let r = self.envelope.record().with_seq(self.seq);
        match &self.envelope {
            Envelope::Error { .. } => r.with_data_count(self.seq.saturating_sub(1)),
            _ => r,
        }
    }

    /// Compact JSON of [`SessionEvent::record`].
    pub fn to_json(&self) -> Json {
        self.record().to_json()
    }
}

// vhco:domain SessionBatch { session_id: string; events: SessionEvent[]; last_seq: int; terminal: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct SessionBatch {
    pub session_id: String,
    pub events: Vec<SessionEvent>,
    pub last_seq: u64,
    pub terminal: bool,
}

impl SessionBatch {
    pub fn to_json(&self) -> Json {
        json!({
            "session_id": self.session_id,
            "events": self.events.iter().map(SessionEvent::to_json).collect::<Vec<_>>(),
            "last_seq": self.last_seq,
            "terminal": self.terminal,
        })
    }
}

// vhco:domain CancelReceipt { request_id: string; session_id?: string; state: string }
#[derive(Clone, Debug, PartialEq)]
pub struct CancelReceipt {
    pub request_id: String,
    pub session_id: Option<String>,
    /// `cancelled`, or the terminal state it already had (`succeeded` / `failed`).
    pub state: String,
}

impl CancelReceipt {
    pub fn to_json(&self) -> Json {
        json!({"session_id": self.session_id, "request_id": self.request_id, "state": self.state})
    }
}

/// Result of checking one `send`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendDecision {
    /// A new sequence: enqueue it.
    Enqueue,
    /// Identical retry of the most recent accepted sequence: acknowledge, do not enqueue.
    Duplicate,
}

// vhco:domain InputSequencer { last_seq: int; last_hash?: string; closed: bool }
/// Input sequencing: `send_seq` starts at 1 and increases by one; the most
/// recent sequence may be retried with an identical payload hash.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct InputSequencer {
    pub last_seq: u64,
    pub last_hash: Option<String>,
    pub closed: bool,
}

impl InputSequencer {
    pub fn check(&self, send_seq: u64, hash: &str) -> RivetResult<SendDecision> {
        if send_seq != 0 && send_seq == self.last_seq {
            if self.last_hash.as_deref() == Some(hash) {
                return Ok(SendDecision::Duplicate);
            }
            return Err(seq_conflict(format!(
                "send_seq {send_seq} was already accepted with a different payload"
            )));
        }
        if self.closed {
            return Err(RivetError::new(
                ErrorKind::Conflict,
                "conflict.input_closed",
                "input is finished; no further sends are accepted",
            ));
        }
        if send_seq != self.last_seq + 1 {
            return Err(seq_conflict(format!(
                "expected send_seq {}, got {send_seq}",
                self.last_seq + 1
            )));
        }
        Ok(SendDecision::Enqueue)
    }

    pub fn accept(&mut self, send_seq: u64, hash: &str) {
        self.last_seq = send_seq;
        self.last_hash = Some(hash.to_string());
    }

    pub fn next_seq(&self) -> u64 {
        self.last_seq + 1
    }
}

fn seq_conflict(msg: String) -> RivetError {
    RivetError::new(ErrorKind::Conflict, "conflict.input_sequence", msg)
}

// vhco:domain EventLog { events: SessionEvent[]; last_seq: int; acked: int; delivered: int; terminal: bool }
/// Retained output events with an acknowledgement cursor.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EventLog {
    events: VecDeque<SessionEvent>,
    last_seq: u64,
    acked: u64,
    delivered: u64,
    terminal: bool,
}

impl EventLog {
    /// Append an envelope; returns its sequence. Nothing is appended after the terminal event.
    pub fn push(&mut self, envelope: Envelope) -> Option<u64> {
        if self.terminal {
            return None;
        }
        self.last_seq += 1;
        let ev = SessionEvent {
            seq: self.last_seq,
            envelope,
        };
        self.terminal = ev.is_terminal();
        self.events.push_back(ev);
        Some(self.last_seq)
    }

    /// Events retained (delivered-but-unacknowledged plus undelivered).
    pub fn retained(&self) -> usize {
        self.events.len()
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn terminal_event(&self) -> Option<&SessionEvent> {
        self.events.back().filter(|e| e.is_terminal())
    }

    /// Acknowledge every event up to `after_seq` and evict them.
    /// A cursor beyond the last delivered event is a conflict; a cursor behind
    /// already-evicted events is `stream.cursor_expired` (never a silent skip).
    pub fn ack(&mut self, after_seq: u64) -> RivetResult<()> {
        if after_seq > self.delivered {
            return Err(RivetError::new(
                ErrorKind::Conflict,
                "conflict.cursor",
                format!(
                    "after_seq {after_seq} is ahead of the last delivered event {}",
                    self.delivered
                ),
            ));
        }
        if after_seq < self.acked {
            return Err(RivetError::new(
                ErrorKind::Conflict,
                "stream.cursor_expired",
                format!(
                    "events after {after_seq} were already acknowledged up to {} and evicted",
                    self.acked
                ),
            ));
        }
        while self.events.front().is_some_and(|e| e.seq <= after_seq) {
            self.events.pop_front();
        }
        self.acked = after_seq;
        Ok(())
    }

    /// Up to `max` events after the acknowledged cursor; marks them delivered.
    pub fn batch(&mut self, max: usize) -> Vec<SessionEvent> {
        let out: Vec<SessionEvent> = self
            .events
            .iter()
            .filter(|e| e.seq > self.acked)
            .take(max)
            .cloned()
            .collect();
        if let Some(last) = out.last() {
            self.delivered = self.delivered.max(last.seq);
        }
        out
    }

    /// Whether any event after `after_seq` is available.
    pub fn has_after(&self, after_seq: u64) -> bool {
        self.last_seq > after_seq
    }
}

/// Terminal state name of a finished session's last event.
pub fn terminal_state(ev: &SessionEvent) -> &'static str {
    match &ev.envelope {
        Envelope::Result(_) => "succeeded",
        Envelope::Error { error, .. } if error.kind == ErrorKind::Cancelled => "cancelled",
        _ => "failed",
    }
}

/// Build the terminal envelope for a finished run.
pub fn terminal_envelope(
    request_id: &str,
    trace_id: &str,
    operation: &str,
    outcome: RivetResult<Completion>,
) -> Envelope {
    match outcome {
        Ok(c) => Envelope::Result(c),
        Err(error) => Envelope::Error {
            request_id: request_id.to_string(),
            trace_id: trace_id.to_string(),
            operation: operation.to_string(),
            seq: 0,
            error: Box::new(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contracts::DataEvent;

    fn data(n: i64) -> Envelope {
        Envelope::Data(DataEvent {
            request_id: "r".into(),
            trace_id: "t".into(),
            operation: "demo.count".into(),
            seq: 0,
            data: Value::Int(n),
        })
    }

    // vhco:test sessions.send_input -- send_seq starts at 1, identical retry is a duplicate, changed payload or gaps conflict, closed input rejects
    #[test]
    fn sequencer_rules() {
        let mut s = InputSequencer::default();
        assert_eq!(s.check(2, "h").unwrap_err().code, "conflict.input_sequence");
        assert_eq!(s.check(1, "h").unwrap(), SendDecision::Enqueue);
        s.accept(1, "h");
        assert_eq!(s.check(1, "h").unwrap(), SendDecision::Duplicate);
        assert_eq!(
            s.check(1, "other").unwrap_err().code,
            "conflict.input_sequence"
        );
        assert_eq!(s.check(2, "x").unwrap(), SendDecision::Enqueue);
        s.closed = true;
        assert_eq!(s.check(2, "x").unwrap_err().code, "conflict.input_closed");
    }

    // vhco:test sessions.read_events -- batches are ordered, acknowledgement evicts, future and evicted cursors are typed errors
    #[test]
    fn event_log_cursor_rules() {
        let mut log = EventLog::default();
        log.push(data(3));
        log.push(data(2));
        assert_eq!(log.ack(1).unwrap_err().code, "conflict.cursor");
        let b = log.batch(16);
        assert_eq!(b.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![1, 2]);
        // replay of the same cursor returns the same events
        assert_eq!(log.batch(16).len(), 2);
        log.ack(2).unwrap();
        assert_eq!(log.retained(), 0);
        assert_eq!(log.ack(1).unwrap_err().code, "stream.cursor_expired");
        log.push(Envelope::Result(Completion {
            request_id: "r".into(),
            trace_id: "t".into(),
            operation: "demo.count".into(),
            result: Value::Int(3),
            data_count: 2,
            effects: Default::default(),
        }));
        assert!(log.push(data(9)).is_none(), "nothing after terminal");
        let b = log.batch(16);
        assert_eq!(b.len(), 1);
        assert!(b[0].is_terminal());
        assert_eq!(b[0].to_json()["seq"], 3);
        assert_eq!(terminal_state(&b[0]), "succeeded");
    }

    #[test]
    fn limits_clamp() {
        let l = SessionLimits::default();
        assert_eq!(l.clamp_wait(None), 1000);
        assert_eq!(l.clamp_wait(Some(60_000)), 5000);
        assert_eq!(l.clamp_max_events(Some(0)), 1);
        assert_eq!(l.clamp_max_events(Some(100)), 16);
    }
}
