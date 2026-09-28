//! The single error model and error registry shared by every surface
//! (PROP-2026-0001 Increment 2; REF-2026-0002 "Error registry").

use super::source::SourceSpan;
use super::value::Value;
use std::fmt;

// vhco:domain ErrorKind { syntax | validation | auth | permission | not_found | conflict | limit | timeout | cancelled | connection | dns | tls | http | protocol | process | parse | output_invalid | consumer_failed | cleanup | unsupported | internal | application }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Syntax,
    Validation,
    Auth,
    Permission,
    NotFound,
    Conflict,
    Limit,
    Timeout,
    Cancelled,
    Connection,
    Dns,
    Tls,
    Http,
    Protocol,
    Process,
    Parse,
    OutputInvalid,
    ConsumerFailed,
    Cleanup,
    Unsupported,
    Internal,
    Application,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "syntax",
            ErrorKind::Validation => "validation",
            ErrorKind::Auth => "auth",
            ErrorKind::Permission => "permission",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Conflict => "conflict",
            ErrorKind::Limit => "limit",
            ErrorKind::Timeout => "timeout",
            ErrorKind::Cancelled => "cancelled",
            ErrorKind::Connection => "connection",
            ErrorKind::Dns => "dns",
            ErrorKind::Tls => "tls",
            ErrorKind::Http => "http",
            ErrorKind::Protocol => "protocol",
            ErrorKind::Process => "process",
            ErrorKind::Parse => "parse",
            ErrorKind::OutputInvalid => "output_invalid",
            ErrorKind::ConsumerFailed => "consumer_failed",
            ErrorKind::Cleanup => "cleanup",
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::Internal => "internal",
            ErrorKind::Application => "application",
        }
    }

    pub fn parse(s: &str) -> Option<ErrorKind> {
        ALL_KINDS.iter().copied().find(|k| k.as_str() == s)
    }

    /// CLI exit code from the registry.
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorKind::Syntax | ErrorKind::Validation => 2,
            ErrorKind::Auth | ErrorKind::Permission => 3,
            ErrorKind::NotFound | ErrorKind::Conflict => 4,
            ErrorKind::Timeout => 6,
            ErrorKind::Cancelled => 130,
            _ => 5,
        }
    }

    /// HTTP status from the registry.
    pub fn http_status(self) -> u16 {
        match self {
            ErrorKind::Syntax | ErrorKind::Validation => 422,
            ErrorKind::Auth => 401,
            ErrorKind::Permission => 403,
            ErrorKind::NotFound => 404,
            ErrorKind::Conflict | ErrorKind::Cancelled => 409,
            ErrorKind::Limit => 429,
            ErrorKind::Timeout => 504,
            ErrorKind::Unsupported => 501,
            ErrorKind::Connection
            | ErrorKind::Dns
            | ErrorKind::Tls
            | ErrorKind::Http
            | ErrorKind::Protocol
            | ErrorKind::Application
            | ErrorKind::Process
            | ErrorKind::Parse => 502,
            ErrorKind::OutputInvalid
            | ErrorKind::Internal
            | ErrorKind::Cleanup
            | ErrorKind::ConsumerFailed => 500,
        }
    }

    pub fn retryable(self) -> bool {
        matches!(self, ErrorKind::Limit)
    }
}

pub const ALL_KINDS: [ErrorKind; 22] = [
    ErrorKind::Syntax,
    ErrorKind::Validation,
    ErrorKind::Auth,
    ErrorKind::Permission,
    ErrorKind::NotFound,
    ErrorKind::Conflict,
    ErrorKind::Limit,
    ErrorKind::Timeout,
    ErrorKind::Cancelled,
    ErrorKind::Connection,
    ErrorKind::Dns,
    ErrorKind::Tls,
    ErrorKind::Http,
    ErrorKind::Protocol,
    ErrorKind::Process,
    ErrorKind::Parse,
    ErrorKind::OutputInvalid,
    ErrorKind::ConsumerFailed,
    ErrorKind::Cleanup,
    ErrorKind::Unsupported,
    ErrorKind::Internal,
    ErrorKind::Application,
];

// vhco:domain EffectsStatus { none | committed | partial | unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EffectsStatus {
    #[default]
    None,
    Committed,
    Partial,
    Unknown,
}

impl EffectsStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            EffectsStatus::None => "none",
            EffectsStatus::Committed => "committed",
            EffectsStatus::Partial => "partial",
            EffectsStatus::Unknown => "unknown",
        }
    }
}

// vhco:domain RivetError { kind: ErrorKind; code: string; message: string; retryable: bool; source?: SourceSpan; operation_id?: string; request_id?: string; trace_id?: string; node_id?: string; hint?: string; details: Value; effects: EffectsStatus; cause?: RivetError; suppressed: RivetError[] }
#[derive(Clone, Debug, PartialEq)]
pub struct RivetError {
    pub kind: ErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub source: Option<SourceSpan>,
    pub operation_id: Option<String>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub node_id: Option<String>,
    pub hint: Option<String>,
    pub details: Value,
    pub effects: EffectsStatus,
    pub cause: Option<Box<RivetError>>,
    pub suppressed: Vec<RivetError>,
}

pub type RivetResult<T> = Result<T, RivetError>;

impl RivetError {
    pub fn new(kind: ErrorKind, code: impl Into<String>, message: impl Into<String>) -> RivetError {
        RivetError {
            kind,
            code: code.into(),
            message: message.into(),
            retryable: kind.retryable(),
            source: None,
            operation_id: None,
            request_id: None,
            trace_id: None,
            node_id: None,
            hint: None,
            details: Value::Null,
            effects: EffectsStatus::None,
            cause: None,
            suppressed: Vec::new(),
        }
    }

    pub fn syntax(code: &str, message: impl Into<String>, span: Option<SourceSpan>) -> RivetError {
        let mut e = RivetError::new(ErrorKind::Syntax, code, message);
        e.source = span;
        e
    }

    pub fn validation(code: &str, message: impl Into<String>) -> RivetError {
        RivetError::new(ErrorKind::Validation, code, message)
    }

    pub fn permission(message: impl Into<String>) -> RivetError {
        RivetError::new(ErrorKind::Permission, "permission.denied", message)
    }

    pub fn not_found(code: &str, message: impl Into<String>) -> RivetError {
        RivetError::new(ErrorKind::NotFound, code, message)
    }

    pub fn internal(message: impl Into<String>) -> RivetError {
        RivetError::new(ErrorKind::Internal, "internal", message)
    }

    pub fn unsupported(code: &str, message: impl Into<String>) -> RivetError {
        RivetError::new(ErrorKind::Unsupported, code, message)
    }

    pub fn with_span(mut self, span: Option<SourceSpan>) -> RivetError {
        if self.source.is_none() {
            self.source = span;
        }
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> RivetError {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_details(mut self, details: Value) -> RivetError {
        self.details = details;
        self
    }

    pub fn with_effects(mut self, effects: EffectsStatus) -> RivetError {
        self.effects = effects;
        self
    }

    pub fn exit_code(&self) -> i32 {
        self.kind.exit_code()
    }

    pub fn http_status(&self) -> u16 {
        self.kind.http_status()
    }

    /// The `error` object of an ErrorEnvelope.
    pub fn to_value(&self) -> Value {
        let mut v = Value::object([
            ("kind", Value::text(self.kind.as_str())),
            ("code", Value::text(&self.code)),
            ("message", Value::text(&self.message)),
            ("retryable", Value::Bool(self.retryable)),
            ("effects", Value::text(self.effects.as_str())),
        ]);
        if let Some(s) = &self.source {
            v.set("source", s.to_value());
        }
        if let Some(op) = &self.operation_id {
            v.set("operation_id", Value::text(op));
        }
        if let Some(node) = &self.node_id {
            v.set("node_id", Value::text(node));
        }
        if let Some(h) = &self.hint {
            v.set("hint", Value::text(h));
        }
        if self.details != Value::Null {
            v.set("details", self.details.clone());
        }
        if let Some(c) = &self.cause {
            v.set("cause", c.to_value());
        }
        if !self.suppressed.is_empty() {
            v.set(
                "suppressed",
                Value::List(self.suppressed.iter().map(|e| e.to_value()).collect()),
            );
        }
        v
    }

    /// Render for a terminal, with the offending source line when available.
    pub fn render(&self, source_text: Option<&str>) -> String {
        let mut out = format!("error[{}]: {}", self.code, self.message);
        if let Some(span) = &self.source {
            out.push_str(&format!(
                "\n  --> {}:{}:{}",
                span.file, span.start_line, span.start_col
            ));
            if let Some(text) = source_text
                && let Some(line) = text.lines().nth(span.start_line.saturating_sub(1) as usize)
            {
                let width = if span.end_line == span.start_line && span.end_col > span.start_col {
                    (span.end_col - span.start_col) as usize
                } else {
                    1
                };
                out.push_str(&format!(
                    "\n   |\n{:>3}| {}\n   | {}{}",
                    span.start_line,
                    line,
                    " ".repeat(span.start_col.saturating_sub(1) as usize),
                    "^".repeat(width.max(1))
                ));
            }
        }
        if let Some(h) = &self.hint {
            out.push_str(&format!("\n  = hint: {h}"));
        }
        out
    }
}

impl fmt::Display for RivetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.code,
            self.kind.as_str(),
            self.message
        )
    }
}

impl std::error::Error for RivetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_maps_every_kind_to_exit_and_http() {
        for k in ALL_KINDS {
            assert!(k.exit_code() > 0);
            assert!(k.http_status() >= 400);
            assert_eq!(ErrorKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(ErrorKind::Syntax.exit_code(), 2);
        assert_eq!(ErrorKind::Permission.exit_code(), 3);
        assert_eq!(ErrorKind::NotFound.exit_code(), 4);
        assert_eq!(ErrorKind::OutputInvalid.exit_code(), 5);
        assert_eq!(ErrorKind::Timeout.exit_code(), 6);
        assert_eq!(ErrorKind::Cancelled.exit_code(), 130);
        assert_eq!(ErrorKind::Validation.http_status(), 422);
        assert_eq!(ErrorKind::Unsupported.http_status(), 501);
    }
}
