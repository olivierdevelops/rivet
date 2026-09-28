//! File CRUD request/result data (R5).

use super::source::SourceSpan;
use super::value::Value;

// vhco:domain FileVerb { read | list | stat | create | update | write | append | delete | copy | move }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileVerb {
    Read,
    List,
    Stat,
    Create,
    Update,
    Write,
    Append,
    Delete,
    Copy,
    Move,
}

impl FileVerb {
    pub fn parse(s: &str) -> Option<FileVerb> {
        Some(match s {
            "read" => FileVerb::Read,
            "list" => FileVerb::List,
            "stat" => FileVerb::Stat,
            "create" => FileVerb::Create,
            "update" => FileVerb::Update,
            "write" => FileVerb::Write,
            "append" => FileVerb::Append,
            "delete" => FileVerb::Delete,
            "copy" => FileVerb::Copy,
            "move" => FileVerb::Move,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FileVerb::Read => "read",
            FileVerb::List => "list",
            FileVerb::Stat => "stat",
            FileVerb::Create => "create",
            FileVerb::Update => "update",
            FileVerb::Write => "write",
            FileVerb::Append => "append",
            FileVerb::Delete => "delete",
            FileVerb::Copy => "copy",
            FileVerb::Move => "move",
        }
    }
}

// vhco:domain Codec { json | text | bytes }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Json,
    Text,
    Bytes,
}

impl Codec {
    pub fn parse(s: &str) -> Option<Codec> {
        Some(match s {
            "json" => Codec::Json,
            "text" => Codec::Text,
            "bytes" => Codec::Bytes,
            _ => return None,
        })
    }
}

// vhco:domain FileOperation { verb: FileVerb; path: string; to?: string; codec?: Codec; content?: Value; if_version?: string; missing_ok: bool; overwrite: bool }
/// Paths are as written in source, relative to the bundle root. The adapter
/// resolves them with no-follow handles under the root.
#[derive(Clone, Debug, PartialEq)]
pub struct FileOperation {
    pub verb: FileVerb,
    pub path: String,
    pub to: Option<String>,
    pub codec: Option<Codec>,
    pub content: Option<Value>,
    pub if_version: Option<String>,
    pub missing_ok: bool,
    pub overwrite: bool,
}

impl FileOperation {
    pub fn new(verb: FileVerb, path: impl Into<String>) -> FileOperation {
        FileOperation {
            verb,
            path: path.into(),
            to: None,
            codec: None,
            content: None,
            if_version: None,
            missing_ok: false,
            // copy/move never replace an existing destination unless asked (S40).
            overwrite: false,
        }
    }
}

// vhco:domain FileStreamMode { read | write | append }
/// Mode of a scoped `with file open PATH mode M as NAME` handle (G35).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStreamMode {
    /// `for chunk in NAME` yields bytes chunks of at most `chunk_size`.
    Read,
    /// Create or truncate; `NAME.write bytes|text V`.
    Write,
    /// Existing file only (like `file append`, S37); `NAME.write …` appends.
    Append,
}

impl FileStreamMode {
    pub fn parse(s: &str) -> Option<FileStreamMode> {
        Some(match s {
            "read" => FileStreamMode::Read,
            "write" => FileStreamMode::Write,
            "append" => FileStreamMode::Append,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FileStreamMode::Read => "read",
            FileStreamMode::Write => "write",
            FileStreamMode::Append => "append",
        }
    }
}

/// Default and upper bound of `chunk_size` (PROP-2026-0001 8 MiB frame limit).
pub const DEFAULT_CHUNK_SIZE: u64 = 64 * 1024;
pub const MAX_CHUNK_SIZE: u64 = 8 * 1024 * 1024;

// vhco:domain FileStreamRequest { path: string; mode: string; chunk_size?: int; operation_id: string; span?: SourceSpan }
/// The evaluated head/options of `with file open …` before validation.
#[derive(Clone, Debug, PartialEq)]
pub struct FileStreamRequest {
    pub path: String,
    /// As written (`read`, `write`, `append`); validated by the use case.
    pub mode: String,
    pub chunk_size: Option<i64>,
    pub operation_id: String,
    pub span: Option<SourceSpan>,
}

// vhco:domain FileStreamPlan { path: string; mode: FileStreamMode; chunk_size: int }
/// A validated, authorized handle plan the confined adapter opens.
#[derive(Clone, Debug, PartialEq)]
pub struct FileStreamPlan {
    pub path: String,
    pub mode: FileStreamMode,
    pub chunk_size: u64,
}
