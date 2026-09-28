//! File CRUD request/result data (R5).

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
