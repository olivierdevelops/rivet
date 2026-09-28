//! Confined file CRUD with directory-relative, no-follow handles (cap-std).
//!
//! Every path resolves under the bundle root through a capability `Dir`, so
//! `..` escapes, absolute-path tricks and symlink escapes fail closed instead
//! of relying on canonicalize-then-open checks (PROP-2026-0001 Increment 4).

use crate::domain::files::{Codec, FileOperation, FileVerb};
use crate::domain::ports::FileAccess;
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// Default read bound (PROP-2026-0001 defaults: 8 MiB frame/body limit).
pub const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;

// vhco:infra file_access satisfies FileAccess
// vhco:file readwrite <bundle root>/** -- only paths granted by policy.json, resolved under the bundle root
pub struct ConfinedFiles {
    root: PathBuf,
}

impl ConfinedFiles {
    pub fn new(root: &str) -> ConfinedFiles {
        ConfinedFiles {
            root: PathBuf::from(root),
        }
    }

    fn dir(&self) -> RivetResult<Dir> {
        Dir::open_ambient_dir(&self.root, ambient_authority()).map_err(|e| {
            RivetError::new(
                ErrorKind::NotFound,
                "file.root",
                format!("bundle root {}: {e}", self.root.display()),
            )
        })
    }
}

/// Root-relative form of a source path; absolute or escaping paths are refused.
fn rel(path: &str) -> RivetResult<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(RivetError::permission(format!(
            "{path}: absolute paths are outside the bundle root"
        )));
    }
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::Normal(s) => out.push(s),
            Component::ParentDir => {
                if !out.pop() {
                    return Err(RivetError::permission(format!(
                        "{path}: escapes the bundle root"
                    )));
                }
            }
            _ => {
                return Err(RivetError::permission(format!(
                    "{path}: unsupported path form"
                )));
            }
        }
    }
    Ok(out)
}

fn io_err(path: &str, e: std::io::Error) -> RivetError {
    use std::io::ErrorKind as K;
    match e.kind() {
        K::NotFound => RivetError::not_found("not_found.file", format!("{path}: no such file")),
        K::AlreadyExists => RivetError::new(
            ErrorKind::Conflict,
            "conflict.already_exists",
            format!("{path} already exists"),
        ),
        K::PermissionDenied => RivetError::new(
            ErrorKind::Permission,
            "permission.os",
            format!("{path}: {e}"),
        ),
        _ if e.raw_os_error() == Some(40) || e.raw_os_error() == Some(62) => {
            RivetError::permission(format!("{path}: symbolic link refused (no-follow)"))
        }
        _ => RivetError::new(ErrorKind::Internal, "file.io", format!("{path}: {e}")),
    }
}

fn version_of(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    format!(
        "v:{}",
        d.iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn encode(content: &Option<Value>, codec: Option<Codec>, path: &str) -> RivetResult<Vec<u8>> {
    let Some(v) = content else {
        return Ok(Vec::new());
    };
    Ok(match (codec, v) {
        (Some(Codec::Bytes), Value::Bytes(b)) => b.clone(),
        (Some(Codec::Bytes), other) => {
            return Err(RivetError::validation(
                "file.codec",
                format!(
                    "{path}: bytes codec needs a bytes value, got {}",
                    other.type_name()
                ),
            ));
        }
        (Some(Codec::Text), Value::Text(s)) => s.clone().into_bytes(),
        (Some(Codec::Text), other) => other.to_display().into_bytes(),
        (Some(Codec::Json) | None, v) => {
            let mut s = serde_json::to_string_pretty(&v.to_json()).unwrap_or_default();
            s.push('\n');
            s.into_bytes()
        }
    })
}

fn decode(bytes: Vec<u8>, codec: Option<Codec>, path: &str) -> RivetResult<Value> {
    let codec = codec.unwrap_or(if path.ends_with(".json") {
        Codec::Json
    } else {
        Codec::Text
    });
    match codec {
        Codec::Bytes => Ok(Value::Bytes(bytes)),
        Codec::Text => String::from_utf8(bytes).map(Value::Text).map_err(|_| {
            RivetError::new(
                ErrorKind::Parse,
                "parse.utf8",
                format!("{path} is not UTF-8 text"),
            )
        }),
        Codec::Json => serde_json::from_slice::<serde_json::Value>(&bytes)
            .map(|j| Value::from_json(&j))
            .map_err(|e| {
                RivetError::new(
                    ErrorKind::Parse,
                    "parse.json",
                    format!("{path}: invalid JSON: {e}"),
                )
            }),
    }
}

#[cfg(unix)]
fn link_count(meta: &cap_std::fs::Metadata) -> u64 {
    use cap_std::fs::MetadataExt;
    meta.nlink()
}

#[cfg(not(unix))]
fn link_count(_meta: &cap_std::fs::Metadata) -> u64 {
    1
}

fn refuse_hardlink(dir: &Dir, rel_path: &Path, path: &str) -> RivetResult<()> {
    if let Ok(meta) = dir.symlink_metadata(rel_path) {
        if meta.file_type().is_symlink() {
            return Err(RivetError::permission(format!(
                "{path}: symbolic link refused (no-follow)"
            )));
        }
        if meta.is_file() && link_count(&meta) > 1 {
            return Err(RivetError::new(
                ErrorKind::Permission,
                "file.hardlink_refused",
                format!(
                    "{path} has {} hard links; write/delete refused",
                    link_count(&meta)
                ),
            ));
        }
    }
    Ok(())
}

fn read_all(dir: &Dir, rel_path: &Path, path: &str) -> RivetResult<Vec<u8>> {
    let meta = dir
        .symlink_metadata(rel_path)
        .map_err(|e| io_err(path, e))?;
    if meta.file_type().is_symlink() {
        return Err(RivetError::permission(format!(
            "{path}: symbolic link refused (no-follow)"
        )));
    }
    if meta.len() > MAX_READ_BYTES {
        return Err(RivetError::new(
            ErrorKind::Limit,
            "limit.file_size",
            format!(
                "{path} is {} bytes; the read limit is {MAX_READ_BYTES}",
                meta.len()
            ),
        ));
    }
    let mut f = dir.open(rel_path).map_err(|e| io_err(path, e))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| io_err(path, e))?;
    Ok(buf)
}

fn ensure_parent(dir: &Dir, rel_path: &Path, path: &str) -> RivetResult<()> {
    if let Some(parent) = rel_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        dir.create_dir_all(parent).map_err(|e| io_err(path, e))?;
    }
    Ok(())
}

/// Write via a temporary sibling then rename (atomic replacement where supported).
fn atomic_write(dir: &Dir, rel_path: &Path, path: &str, bytes: &[u8]) -> RivetResult<()> {
    let tmp = rel_path.with_file_name(format!(
        ".{}.rivet-tmp",
        rel_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    ));
    {
        let mut f = dir
            .open_with(
                &tmp,
                OpenOptions::new().write(true).create(true).truncate(true),
            )
            .map_err(|e| io_err(path, e))?;
        f.write_all(bytes).map_err(|e| io_err(path, e))?;
        f.sync_all().map_err(|e| io_err(path, e))?;
    }
    dir.rename(&tmp, dir, rel_path).map_err(|e| io_err(path, e))
}

fn apply_sync(root: &Path, op: FileOperation) -> RivetResult<Value> {
    let dir = Dir::open_ambient_dir(root, ambient_authority()).map_err(|e| {
        RivetError::new(
            ErrorKind::NotFound,
            "file.root",
            format!("bundle root {}: {e}", root.display()),
        )
    })?;
    let path = op.path.as_str();
    let r = rel(path)?;
    match op.verb {
        FileVerb::Read => decode(read_all(&dir, &r, path)?, op.codec, path),
        FileVerb::Stat => {
            let meta = dir.symlink_metadata(&r).map_err(|e| io_err(path, e))?;
            let version = if meta.is_file() {
                version_of(&read_all(&dir, &r, path)?)
            } else {
                String::new()
            };
            Ok(Value::object([
                ("path", Value::text(path)),
                (
                    "type",
                    Value::text(if meta.is_dir() {
                        "dir"
                    } else if meta.file_type().is_symlink() {
                        "symlink"
                    } else {
                        "file"
                    }),
                ),
                ("size", Value::Int(meta.len() as i64)),
                ("version", Value::text(version)),
            ]))
        }
        FileVerb::List => {
            let target = if r.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                r.clone()
            };
            let mut entries = Vec::new();
            for e in dir.read_dir(&target).map_err(|e| io_err(path, e))? {
                let e = e.map_err(|e| io_err(path, e))?;
                let ft = e.file_type().map_err(|e| io_err(path, e))?;
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                entries.push(Value::object([
                    ("name", Value::text(e.file_name().to_string_lossy())),
                    (
                        "type",
                        Value::text(if ft.is_dir() {
                            "dir"
                        } else if ft.is_symlink() {
                            "symlink"
                        } else {
                            "file"
                        }),
                    ),
                    ("size", Value::Int(size as i64)),
                ]));
            }
            entries.sort_by(|a, b| {
                a.get("name")
                    .map(|v| v.to_display())
                    .cmp(&b.get("name").map(|v| v.to_display()))
            });
            Ok(Value::List(entries))
        }
        FileVerb::Create => {
            ensure_parent(&dir, &r, path)?;
            let bytes = encode(&op.content, op.codec, path)?;
            let mut f = dir
                .open_with(&r, OpenOptions::new().write(true).create_new(true))
                .map_err(|e| io_err(path, e))?;
            f.write_all(&bytes).map_err(|e| io_err(path, e))?;
            f.sync_all().map_err(|e| io_err(path, e))?;
            Ok(Value::object([
                ("path", Value::text(path)),
                ("created", Value::Bool(true)),
                ("version", Value::text(version_of(&bytes))),
            ]))
        }
        FileVerb::Update | FileVerb::Write => {
            let existing = match read_all(&dir, &r, path) {
                Ok(b) => Some(b),
                Err(e) if e.kind == ErrorKind::NotFound && op.verb == FileVerb::Write => None,
                Err(e) => return Err(e),
            };
            if let (Some(want), Some(bytes)) = (&op.if_version, &existing) {
                let have = version_of(bytes);
                if &have != want {
                    return Err(RivetError::new(
                        ErrorKind::Conflict,
                        "conflict.version",
                        format!("{path} changed: version {have}, expected {want}"),
                    ));
                }
            }
            refuse_hardlink(&dir, &r, path)?;
            ensure_parent(&dir, &r, path)?;
            let bytes = encode(&op.content, op.codec, path)?;
            atomic_write(&dir, &r, path, &bytes)?;
            Ok(Value::object([
                ("path", Value::text(path)),
                (
                    if existing.is_some() {
                        "updated"
                    } else {
                        "created"
                    },
                    Value::Bool(true),
                ),
                ("version", Value::text(version_of(&bytes))),
            ]))
        }
        FileVerb::Append => {
            refuse_hardlink(&dir, &r, path)?;
            ensure_parent(&dir, &r, path)?;
            let bytes = encode(&op.content, op.codec.or(Some(Codec::Text)), path)?;
            let mut f = dir
                .open_with(&r, OpenOptions::new().append(true).create(true))
                .map_err(|e| io_err(path, e))?;
            f.write_all(&bytes).map_err(|e| io_err(path, e))?;
            Ok(Value::object([
                ("path", Value::text(path)),
                ("appended", Value::Int(bytes.len() as i64)),
            ]))
        }
        FileVerb::Delete => {
            refuse_hardlink(&dir, &r, path)?;
            match dir.remove_file(&r) {
                Ok(()) => Ok(Value::object([
                    ("path", Value::text(path)),
                    ("deleted", Value::Bool(true)),
                ])),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && op.missing_ok => {
                    Ok(Value::object([
                        ("path", Value::text(path)),
                        ("deleted", Value::Bool(false)),
                    ]))
                }
                Err(e) => Err(io_err(path, e)),
            }
        }
        FileVerb::Copy | FileVerb::Move => {
            let to = op.to.clone().ok_or_else(|| {
                RivetError::validation(
                    "file.to",
                    format!("file {} needs `to PATH`", op.verb.as_str()),
                )
            })?;
            let rt = rel(&to)?;
            let bytes = read_all(&dir, &r, path)?;
            if !op.overwrite && dir.symlink_metadata(&rt).is_ok() {
                return Err(RivetError::new(
                    ErrorKind::Conflict,
                    "conflict.already_exists",
                    format!("{to} already exists (overwrite false)"),
                ));
            }
            refuse_hardlink(&dir, &rt, &to)?;
            ensure_parent(&dir, &rt, &to)?;
            atomic_write(&dir, &rt, &to, &bytes)?;
            if op.verb == FileVerb::Move {
                refuse_hardlink(&dir, &r, path)?;
                dir.remove_file(&r).map_err(|e| io_err(path, e))?;
            }
            Ok(Value::object([
                ("path", Value::text(&to)),
                ("version", Value::text(version_of(&bytes))),
            ]))
        }
    }
}

#[async_trait]
impl FileAccess for ConfinedFiles {
    async fn apply(&self, op: FileOperation) -> RivetResult<Value> {
        let _ = self.dir()?;
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || apply_sync(&root, op))
            .await
            .map_err(|e| RivetError::internal(format!("file task failed: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(verb: FileVerb, path: &str) -> FileOperation {
        FileOperation::new(verb, path)
    }

    #[tokio::test]
    async fn crud_round_trip_with_exclusive_create_and_version_guard() {
        let tmp = tempfile::tempdir().unwrap();
        let files = ConfinedFiles::new(tmp.path().to_str().unwrap());
        let mut c = op(FileVerb::Create, "./out/a.json");
        c.content = Some(Value::object([("n", Value::Int(1))]));
        let created = files.apply(c.clone()).await.unwrap();
        assert_eq!(
            files.apply(c).await.unwrap_err().code,
            "conflict.already_exists"
        );
        let v = files
            .apply(op(FileVerb::Read, "./out/a.json"))
            .await
            .unwrap();
        assert_eq!(v.get("n"), Some(&Value::Int(1)));
        let mut u = op(FileVerb::Update, "./out/a.json");
        u.content = Some(Value::Int(2));
        u.if_version = Some("v:stale".into());
        assert_eq!(
            files.apply(u.clone()).await.unwrap_err().code,
            "conflict.version"
        );
        u.if_version = created
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        files.apply(u).await.unwrap();
        let mut d = op(FileVerb::Delete, "./out/a.json");
        files.apply(d.clone()).await.unwrap();
        assert_eq!(
            files.apply(d.clone()).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
        d.missing_ok = true;
        assert_eq!(
            files.apply(d).await.unwrap().get("deleted"),
            Some(&Value::Bool(false))
        );
    }

    #[tokio::test]
    async fn escapes_and_symlinks_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(tmp.path().join("secret"), "s").unwrap();
        let files = ConfinedFiles::new(root.to_str().unwrap());
        assert_eq!(
            files
                .apply(op(FileVerb::Read, "../secret"))
                .await
                .unwrap_err()
                .kind,
            ErrorKind::Permission
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(tmp.path().join("secret"), root.join("link")).unwrap();
            assert_eq!(
                files
                    .apply(op(FileVerb::Read, "./link"))
                    .await
                    .unwrap_err()
                    .kind,
                ErrorKind::Permission
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn hard_linked_files_refuse_write_and_delete() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a"), "x").unwrap();
        std::fs::hard_link(tmp.path().join("a"), tmp.path().join("b")).unwrap();
        let files = ConfinedFiles::new(tmp.path().to_str().unwrap());
        assert_eq!(
            files
                .apply(op(FileVerb::Delete, "./a"))
                .await
                .unwrap_err()
                .code,
            "file.hardlink_refused"
        );
    }
}

// vhco:infra file_access satisfies FileProbe
// vhco:file stat <bundle root>/** -- `io --check-files` metadata probes of needed paths (never opened or read)
impl crate::domain::ports::FileProbe for ConfinedFiles {
    fn stat(
        &self,
        input: &crate::domain::io_manifest::FileProbeInput,
    ) -> crate::domain::io_manifest::FileProbeResult {
        use crate::domain::io_manifest::{FileProbeResult, FileStatus};
        let status = match (rel(&input.path), self.dir()) {
            (Ok(p), Ok(dir)) => match dir.metadata(&p) {
                Ok(meta) => {
                    if readable(&meta) {
                        FileStatus::Present
                    } else {
                        FileStatus::Unreadable
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileStatus::Missing,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    FileStatus::Unreadable
                }
                Err(_) => FileStatus::Missing,
            },
            (Err(_), _) => FileStatus::NotPermitted,
            (_, Err(_)) => FileStatus::Missing,
        };
        FileProbeResult {
            path: input.path.clone(),
            status,
        }
    }
}

/// Readability from permission bits only (the file is never opened).
#[cfg(unix)]
fn readable(meta: &cap_std::fs::Metadata) -> bool {
    use cap_std::fs::PermissionsExt;
    meta.permissions().mode() & 0o444 != 0
}

#[cfg(not(unix))]
fn readable(_meta: &cap_std::fs::Metadata) -> bool {
    true
}
