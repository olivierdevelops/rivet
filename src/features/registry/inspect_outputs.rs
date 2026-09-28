use super::ports::Registry;
use crate::domain::contracts::{CatalogQuery, OutputReport};
use crate::domain::{RivetError, RivetResult};

// vhco:domain OutputQuery { id?: string; all: bool }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct OutputQuery {
    pub id: Option<String>,
    pub all: bool,
}

// vhco:usecase registry.inspect_outputs(input: OutputQuery) -> OutputReport needs Registry
// vhco:label Inspect declared outputs
// vhco:about Returns each selected operation's declared output (type, description, fields), emits and receives schemas and declared errors from the immutable catalog, for `rivet outputs`, GET /v1/operations/{id}/outputs, MCP rivet.outputs and Runtime::outputs.
// vhco:example input={id:"demo.add"} => [{ "id": "demo.add", "output": { "type": "integer", "description": "Sum of a and b." } }]
pub fn inspect_outputs(
    input: &OutputQuery,
    registry: &dyn Registry,
) -> RivetResult<Vec<OutputReport>> {
    // vhco:todo select_operations -- `all` selects every public operation; otherwise `id` is required; both or neither is validation (exit 2); an unknown or private id is not_found (exit 4)
    // vhco:error bad_query -- id and all together, or neither => validation.query returns
    let ids = match (&input.id, input.all) {
        (Some(_), true) | (None, false) => {
            return Err(RivetError::validation(
                "validation.query",
                "pass exactly one of an operation ID or --all",
            ));
        }
        (Some(id), false) => vec![id.clone()],
        (None, true) => Vec::new(),
    };
    // vhco:todo read_immutable_spec -- read RegistryEntry.output/emits/receives/errors from the compiled catalog; never re-parse, connect or execute
    // vhco:step read registry.describe -- public entries only
    let catalog = registry.describe(&CatalogQuery {
        ids: ids.clone(),
        include_private: false,
    })?;
    // vhco:error unknown_id -- selected id is absent or private => not_found (exit 4) returns
    if let Some(id) = ids.first() {
        if catalog.entries.is_empty() {
            return Err(RivetError::not_found(
                "not_found.operation",
                format!("no operation `{id}`"),
            ));
        }
    }
    // vhco:todo shape_report -- one OutputReport per operation ordered by id: output spec (type, description, fields), emits|null, receives|null and declared errors
    let mut reports: Vec<OutputReport> = catalog
        .entries
        .into_iter()
        .map(|e| OutputReport {
            id: e.id,
            name: e.name,
            output: e.output,
            emits: e.emits,
            receives: e.receives,
            errors: e.errors,
        })
        .collect();
    reports.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(reports)
}
