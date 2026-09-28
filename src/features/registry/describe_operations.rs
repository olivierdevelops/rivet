use super::ports::Registry;
use crate::domain::contracts::{Catalog, CatalogQuery};
use crate::domain::{RivetError, RivetResult};

// vhco:usecase registry.describe_operations(input: CatalogQuery) -> Catalog needs Registry
// vhco:label List and describe operations
// vhco:about Returns the authorized entries of the immutable catalog — IDs, display names, described parameter schemas, outputs, stream schemas and source spans — without connecting to anything.
// vhco:example input={ids:["demo.add"]} => { "entries": [{ "id": "demo.add", "name": "Add two integers" }] }
pub fn describe_operations(input: &CatalogQuery, registry: &dyn Registry) -> RivetResult<Catalog> {
    // vhco:todo filter_catalog -- read the immutable snapshot, drop private entries unless the caller is in-bundle, keep only requested IDs, and never connect to dependencies
    // vhco:step read registry.describe -- snapshot entries already filtered for privacy
    let catalog = registry.describe(input)?;
    // vhco:todo resolve_dynamic -- a requested ID that is absent (or private for this caller) is not_found without disclosing whether a private entry exists
    // vhco:error unknown_id -- a requested id is missing or hidden => not_found (exit 4) returns
    for id in &input.ids {
        if !catalog.entries.iter().any(|e| &e.id == id) {
            return Err(RivetError::not_found(
                "not_found.operation",
                format!("no operation `{id}`"),
            ));
        }
    }
    // vhco:todo project_surfaces -- the same entries back CLI list/describe, HTTP /v1/operations, MCP tools/list and the Rust registry; surfaces only re-encode them
    Ok(catalog)
}
