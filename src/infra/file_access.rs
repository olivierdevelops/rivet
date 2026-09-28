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
pub(crate) fn rel(path: &str) -> RivetResult<PathBuf> {
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

pub(crate) fn io_err(path: &str, e: std::io::Error) -> RivetError {
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

pub(crate) fn refuse_hardlink(dir: &Dir, rel_path: &Path, path: &str) -> RivetResult<()> {
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

/// Refuse a path whose directory components (and, with `include_last`, the
/// final component) are symbolic links. cap-std keeps resolution inside the
/// bundle root, but a link that stays inside it would still carry a write
/// granted on `./out/**` into an ungranted sibling such as `./data`.
pub(crate) fn refuse_symlink_components(
    dir: &Dir,
    rel_path: &Path,
    path: &str,
    include_last: bool,
) -> RivetResult<()> {
    let comps: Vec<Component> = rel_path.components().collect();
    let n = if include_last {
        comps.len()
    } else {
        comps.len().saturating_sub(1)
    };
    let mut cur = PathBuf::new();
    for c in comps.iter().take(n) {
        cur.push(c);
        match dir.symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(RivetError::permission(format!(
                    "{path}: symbolic link component `{}` refused (no-follow)",
                    cur.display()
                )));
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
}

/// Write via a fresh temporary sibling then rename (atomic replacement where
/// supported). The temporary file is removed on every failure path.
fn atomic_write(dir: &Dir, rel_path: &Path, path: &str, bytes: &[u8]) -> RivetResult<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp = rel_path.with_file_name(format!(
        ".{}.{}-{}.rivet-tmp",
        rel_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut f = dir
            .open_with(&tmp, OpenOptions::new().write(true).create_new(true))
            .map_err(|e| io_err(path, e))?;
        f.write_all(bytes).map_err(|e| io_err(path, e))?;
        f.sync_all().map_err(|e| io_err(path, e))?;
        dir.rename(&tmp, dir, rel_path).map_err(|e| io_err(path, e))
    })();
    if written.is_err() {
        let _ = dir.remove_file(&tmp);
    }
    written
}

/// How long a replacement waits for another Rivet writer's lock on the same file.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Conditional (and every) replacement of an EXISTING file — G32, proposal
/// Increment 4 `if_version`:
///
/// ```text
///  open target no-follow ─▶ flock(LOCK_EX) ─▶ fd still the file at PATH? (dev+ino) ──no──▶ retry
///        │                                              │ yes
///        │                                              ▼
///        │                         nlink > 1 → file.hardlink_refused
///        │                         read via the locked fd → version ≠ V → conflict.version
///        │                         write temp sibling ─▶ fsync ─▶ rename over PATH (lock still held)
///        ▼
///   missing → not_found (update) / plain create (write without if_version)
/// ```
///
/// Guarantee (documented decision): the compare-and-replace is atomic with respect
/// to every writer that takes the same advisory `flock` on the target — all Rivet
/// runtimes (every update/write of an existing file goes through this path, so an
/// unconditional Rivet write cannot slip between another's check and rename). The
/// dev+ino re-check after locking closes the rename race (a waiter that locked the
/// replaced inode retries on the new file and then fails `conflict.version`). An
/// external process that writes WITHOUT taking the lock is not stopped by an
/// advisory lock; such writers are outside what this adapter can enforce. Where
/// `flock` is unavailable (non-Unix targets) a conditional update is refused with
/// `unsupported.conditional_update` before anything is written — never degraded to
/// read-then-write.
#[cfg(unix)]
fn locked_replace(
    dir: &Dir,
    rel_path: &Path,
    path: &str,
    if_version: Option<&str>,
    bytes: &[u8],
) -> RivetResult<()> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    use cap_std::fs::MetadataExt as _;
    use std::os::unix::io::AsRawFd;
    let started = std::time::Instant::now();
    loop {
        let mut opts = OpenOptions::new();
        opts.read(true).follow(FollowSymlinks::No);
        let mut f = dir
            .open_with(rel_path, &opts)
            .map_err(|e| io_err(path, e))?;
        // Bounded wait for the exclusive advisory lock (released on drop/close).
        loop {
            // SAFETY: flock on a descriptor this function owns for the whole scope.
            let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if rc == 0 {
                break;
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(io_err(path, err));
            }
            if started.elapsed() > LOCK_WAIT {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.file_lock",
                    format!("{path}: another writer held the file lock for too long"),
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let held = f.metadata().map_err(|e| io_err(path, e))?;
        let now = match dir.symlink_metadata(rel_path) {
            Ok(m) => m,
            // Replaced and removed meanwhile: the next open reports it.
            Err(_) => continue,
        };
        if now.file_type().is_symlink() {
            return Err(RivetError::permission(format!(
                "{path}: symbolic link refused (no-follow)"
            )));
        }
        if now.dev() != held.dev() || now.ino() != held.ino() {
            // We locked an inode that another writer already renamed away.
            continue;
        }
        if link_count(&held) > 1 {
            return Err(RivetError::new(
                ErrorKind::Permission,
                "file.hardlink_refused",
                format!(
                    "{path} has {} hard links; write/delete refused",
                    link_count(&held)
                ),
            ));
        }
        if let Some(want) = if_version {
            if held.len() > MAX_READ_BYTES {
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.file_size",
                    format!(
                        "{path} is {} bytes; the read limit is {MAX_READ_BYTES}",
                        held.len()
                    ),
                ));
            }
            let mut current = Vec::new();
            f.read_to_end(&mut current).map_err(|e| io_err(path, e))?;
            let have = version_of(&current);
            if have != want {
                return Err(RivetError::new(
                    ErrorKind::Conflict,
                    "conflict.version",
                    format!("{path} changed: version {have}, expected {want}"),
                ));
            }
        }
        // Rename while the lock is held; dropping `f` afterwards releases it.
        let out = atomic_write(dir, rel_path, path, bytes);
        drop(f);
        return out;
    }
}

#[cfg(not(unix))]
fn locked_replace(
    dir: &Dir,
    rel_path: &Path,
    path: &str,
    if_version: Option<&str>,
    bytes: &[u8],
) -> RivetResult<()> {
    if if_version.is_some() {
        return Err(RivetError::unsupported(
            "unsupported.conditional_update",
            format!("{path}: this platform cannot guard `if_version` against concurrent writers"),
        ));
    }
    refuse_hardlink(dir, rel_path, path)?;
    atomic_write(dir, rel_path, path, bytes)
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
    refuse_symlink_components(&dir, &r, path, op.verb == FileVerb::List)?;
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
            let bytes = encode(&op.content, op.codec, path)?;
            let existing = match dir.symlink_metadata(&r) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(RivetError::permission(format!(
                        "{path}: symbolic link refused (no-follow)"
                    )));
                }
                Ok(_) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => return Err(io_err(path, e)),
            };
            if existing {
                // Compare-and-replace under the file lock (see locked_replace).
                locked_replace(&dir, &r, path, op.if_version.as_deref(), &bytes)?;
            } else if op.verb == FileVerb::Update || op.if_version.is_some() {
                // Update never creates; a version guard on a missing file cannot hold.
                return Err(match &op.if_version {
                    Some(want) if op.verb == FileVerb::Write => RivetError::new(
                        ErrorKind::Conflict,
                        "conflict.version",
                        format!("{path} does not exist, expected version {want}"),
                    ),
                    _ => RivetError::not_found("not_found.file", format!("{path}: no such file")),
                });
            } else {
                atomic_write(&dir, &r, path, &bytes)?;
            }
            Ok(Value::object([
                ("path", Value::text(path)),
                (
                    if existing { "updated" } else { "created" },
                    Value::Bool(true),
                ),
                ("version", Value::text(version_of(&bytes))),
            ]))
        }
        FileVerb::Append => {
            // Append requires an existing file; it never creates one (S37).
            refuse_hardlink(&dir, &r, path)?;
            let bytes = encode(&op.content, op.codec.or(Some(Codec::Text)), path)?;
            let mut f = dir
                .open_with(&r, OpenOptions::new().append(true))
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
            refuse_symlink_components(&dir, &rt, &to, false)?;
            refuse_hardlink(&dir, &rt, &to)?;
            // Check the source before the destination is written, so a refused
            // move leaves no copy behind.
            if op.verb == FileVerb::Move {
                refuse_hardlink(&dir, &r, path)?;
            }
            atomic_write(&dir, &rt, &to, &bytes)?;
            if op.verb == FileVerb::Move {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn op(verb: FileVerb, path: &str) -> FileOperation {
        FileOperation::new(verb, path)
    }

    #[tokio::test]
    async fn crud_round_trip_with_exclusive_create_and_version_guard() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("out")).unwrap();
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

    // vhco:test files.apply_file_operation -- G32 concurrent `update … if_version V` from many threads: exactly one compare-and-replace wins per round, every other updater gets conflict.version, and the file holds the winner's bytes (no lost update)
    #[cfg(unix)]
    #[test]
    fn concurrent_conditional_updates_have_one_winner() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("out")).unwrap();
        std::fs::write(tmp.path().join("out/c.json"), "0").unwrap();
        let root = tmp.path().to_path_buf();
        for round in 0..20 {
            let v = version_of(&std::fs::read(root.join("out/c.json")).unwrap());
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
            let handles: Vec<_> = (0..8)
                .map(|i| {
                    let (root, v, barrier) = (root.clone(), v.clone(), barrier.clone());
                    std::thread::spawn(move || {
                        let mut u = FileOperation::new(FileVerb::Update, "./out/c.json");
                        u.codec = Some(Codec::Text);
                        u.content = Some(Value::text(format!("{round}-{i}")));
                        u.if_version = Some(v);
                        barrier.wait();
                        (i, apply_sync(&root, u))
                    })
                })
                .collect();
            let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            let winners: Vec<_> = results.iter().filter(|(_, r)| r.is_ok()).collect();
            assert_eq!(winners.len(), 1, "round {round}: {results:?}");
            for (_, r) in &results {
                if let Err(e) = r {
                    assert_eq!(e.code, "conflict.version", "round {round}: {e:?}");
                }
            }
            let content = std::fs::read_to_string(root.join("out/c.json")).unwrap();
            assert_eq!(content, format!("{round}-{}", winners[0].0));
        }
        // No temporary siblings are left behind.
        let leftovers: Vec<_> = std::fs::read_dir(root.join("out"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".rivet-tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    // vhco:test files.apply_file_operation -- G32 an update waits for another holder of the file lock and then re-checks the version (a writer that changed the file under the lock makes the stale guard fail)
    #[cfg(unix)]
    #[test]
    fn conditional_update_waits_for_the_lock_and_rechecks() {
        use std::os::unix::io::AsRawFd;
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("c.txt");
        std::fs::write(&p, "a").unwrap();
        let v = version_of(b"a");
        let holder = std::fs::File::open(&p).unwrap();
        assert_eq!(unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX) }, 0);
        let root = tmp.path().to_path_buf();
        let t = std::thread::spawn(move || {
            let mut u = FileOperation::new(FileVerb::Update, "./c.txt");
            u.codec = Some(Codec::Text);
            u.content = Some(Value::text("b"));
            u.if_version = Some(v);
            apply_sync(&root, u)
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!t.is_finished(), "the update must wait for the lock");
        // The lock holder changes the file in place, then releases the lock.
        std::fs::write(&p, "z").unwrap();
        drop(holder);
        let e = t.join().unwrap().unwrap_err();
        assert_eq!(e.code, "conflict.version");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "z");
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

#[cfg(not(unix))]
fn readable(_meta: &cap_std::fs::Metadata) -> bool {
    true
}
