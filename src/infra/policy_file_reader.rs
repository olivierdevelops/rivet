//! Finds and reads policy.json (host bootstrap I/O).

use crate::domain::ports::{PolicyBytes, PolicyDiscovery, PolicyFileReader, PolicyLocator};
use crate::domain::{RivetError, RivetResult};
use sha2::{Digest, Sha256};
use std::path::Path;

// vhco:infra policy_file_reader satisfies PolicyFileReader
// vhco:file read policy.json -- beside the entry .rivet file, or the file named by --policy PATH
pub struct DiskPolicyReader;

impl PolicyFileReader for DiskPolicyReader {
    fn discover(&self, locator: &PolicyLocator) -> RivetResult<PolicyDiscovery> {
        if let Some(explicit) = &locator.explicit_path {
            if !Path::new(explicit).is_file() {
                return Err(RivetError::validation(
                    "policy.invalid",
                    format!("--policy {explicit}: no such file"),
                ));
            }
            return Ok(PolicyDiscovery {
                path: Some(explicit.clone()),
                explicit: true,
            });
        }
        let dir = Path::new(&locator.entry)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let candidate = dir.join("policy.json");
        Ok(PolicyDiscovery {
            path: candidate
                .is_file()
                .then(|| candidate.to_string_lossy().to_string()),
            explicit: false,
        })
    }

    fn read(&self, discovery: &PolicyDiscovery) -> RivetResult<PolicyBytes> {
        let path = discovery
            .path
            .clone()
            .ok_or_else(|| RivetError::internal("read called without a discovered policy file"))?;
        let bytes = std::fs::read(&path)
            .map_err(|e| RivetError::validation("policy.invalid", format!("{path}: {e}")))?;
        let digest = Sha256::digest(&bytes);
        Ok(PolicyBytes {
            path,
            sha256: format!(
                "sha256:{}",
                digest
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            ),
            bytes,
        })
    }
}
