//! Loads the entry bundle and imported modules from disk (host bootstrap
//! before compilation).
//!
//! ```text
//!  load(entry)              ─▶ SourceBundle{entry, root = entry's directory, [entry file]}
//!  read_module(root, path)  ─▶ root/path, every component below root checked:
//!                                symlink ─▶ permission.import_outside_root
//!                                missing ─▶ not_found.import
//!  policy_beside(path)      ─▶ does <dir of path>/policy.json exist? (ignored module policy, R24)
//! ```

use crate::domain::errors::codes;
use crate::domain::ports::SourceLoader;
use crate::domain::source::{SourceBundle, SourceFile};
use crate::domain::{ErrorKind, RivetError, RivetResult};
use std::path::{Component, Path, PathBuf};

// vhco:infra source_loader satisfies SourceLoader
// vhco:file read *.rivet -- the entry bundle named by --file and every imported or host-loaded module below the runtime root (bootstrap; symlinks refused)
// vhco:file read policy.json -- existence probe beside each module (a module policy is ignored with a warning)
pub struct DiskSourceLoader;

impl SourceLoader for DiskSourceLoader {
    fn load(&self, entry: &str) -> RivetResult<SourceBundle> {
        let text = std::fs::read_to_string(entry).map_err(|e| {
            RivetError::not_found("not_found.source", format!("cannot read {entry}: {e}"))
        })?;
        let root = Path::new(entry)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());
        Ok(SourceBundle {
            entry: entry.to_string(),
            root,
            files: vec![SourceFile {
                path: entry.to_string(),
                text,
            }],
            modules: Vec::new(),
        })
    }

    fn read_module(&self, root: &str, path: &str) -> RivetResult<SourceFile> {
        let full = join(root, path);
        // Every component below the root must be a real directory or file: a
        // symlink could lead outside the root (R19: no symlinks).
        let mut cur = if root.is_empty() || root == "." {
            PathBuf::new()
        } else {
            PathBuf::from(root)
        };
        for c in Path::new(path).components() {
            let Component::Normal(seg) = c else {
                return Err(outside(path));
            };
            cur.push(seg);
            match std::fs::symlink_metadata(&cur) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(RivetError::new(
                        ErrorKind::Permission,
                        codes::IMPORT_OUTSIDE_ROOT,
                        format!(
                            "{full} is reached through the symlink {}; imports must stay inside the runtime root without symlinks",
                            cur.display()
                        ),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(missing(&full, &e)),
            }
        }
        let text = std::fs::read_to_string(&full).map_err(|e| missing(&full, &e))?;
        Ok(SourceFile { path: full, text })
    }

    fn policy_beside(&self, path: &str) -> bool {
        Path::new(path)
            .parent()
            .map(|d| d.join("policy.json"))
            .unwrap_or_else(|| PathBuf::from("policy.json"))
            .is_file()
    }
}

fn join(root: &str, path: &str) -> String {
    if root.is_empty() || root == "." {
        path.to_string()
    } else {
        format!("{}/{path}", root.trim_end_matches('/'))
    }
}

fn outside(path: &str) -> RivetError {
    RivetError::new(
        ErrorKind::Permission,
        codes::IMPORT_OUTSIDE_ROOT,
        format!("{path} is outside the runtime root"),
    )
}

fn missing(full: &str, e: &std::io::Error) -> RivetError {
    RivetError::new(
        ErrorKind::NotFound,
        codes::IMPORT_NOT_FOUND,
        format!("cannot read {full}: {e}"),
    )
}
