//! Writes `policy generate --output PATH` drafts exclusively (host CLI file
//! creation, not an application effect). An existing file, symlink or
//! directory is never replaced.

use crate::domain::io_manifest::{PolicyDraftFile, PolicyDraftReceipt};
use crate::domain::ports::PolicyDraftWriter;
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};
use sha2::{Digest, Sha256};
use std::io::Write;

// vhco:infra policy_draft_writer satisfies PolicyDraftWriter
// vhco:file write <--output PATH> -- created exclusively; an existing path is conflict.exists
pub struct ExclusiveDraftWriter;

impl PolicyDraftWriter for ExclusiveDraftWriter {
    fn write_new(&self, file: &PolicyDraftFile) -> RivetResult<PolicyDraftReceipt> {
        let exists = || {
            RivetError::new(
                ErrorKind::Conflict,
                "conflict.exists",
                format!("refusing to overwrite {}", file.path),
            )
            .with_details(Value::object([("path", Value::text(&file.path))]))
        };
        if std::fs::symlink_metadata(&file.path).is_ok() {
            return Err(exists());
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file.path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => exists(),
                _ => RivetError::new(
                    ErrorKind::NotFound,
                    "not_found.file",
                    format!("cannot create {}: {e}", file.path),
                ),
            })?;
        f.write_all(&file.bytes)
            .map_err(|e| RivetError::internal(format!("writing {}: {e}", file.path)))?;
        let digest = Sha256::digest(&file.bytes);
        Ok(PolicyDraftReceipt {
            path: file.path.clone(),
            sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // vhco:test policy.generate_policy -- --output creates the draft once and refuses to overwrite it (conflict.exists)
    #[test]
    fn creates_once_then_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("draft.json").to_string_lossy().to_string();
        let f = PolicyDraftFile {
            path: p.clone(),
            bytes: b"{}".to_vec(),
        };
        ExclusiveDraftWriter.write_new(&f).unwrap();
        let e = ExclusiveDraftWriter.write_new(&f).unwrap_err();
        assert_eq!(e.code, "conflict.exists");
        assert_eq!(std::fs::read(&p).unwrap(), b"{}");
    }
}
