//! Loads the entry bundle from disk (host bootstrap before compilation).

use crate::domain::ports::SourceLoader;
use crate::domain::source::{SourceBundle, SourceFile};
use crate::domain::{RivetError, RivetResult};
use std::path::Path;

// vhco:infra source_loader satisfies SourceLoader
// vhco:file read *.rivet -- the entry bundle named by --file (bootstrap)
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
}
