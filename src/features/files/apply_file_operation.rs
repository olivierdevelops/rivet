use super::ports::{FileAccess, PolicyEvaluator};
use crate::domain::files::{FileOperation, FileVerb};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::source::SourceSpan;
use crate::domain::{RivetError, RivetResult, Value};

// vhco:domain FileRequest { op: FileOperation; operation_id: string; span?: SourceSpan; effect_id?: string }
#[derive(Clone, Debug, PartialEq)]
pub struct FileRequest {
    pub op: FileOperation,
    pub operation_id: String,
    pub span: Option<SourceSpan>,
    pub effect_id: Option<String>,
}

/// The (capability, verb, path) intents one file verb needs (REF-2026-0002 "DSL → verb").
pub fn file_intents(op: &FileOperation) -> Vec<(Capability, AccessVerb, String)> {
    use AccessVerb as V;
    use Capability as C;
    let p = op.path.clone();
    match op.verb {
        FileVerb::Read => vec![(C::Read, V::Read, p)],
        FileVerb::List => vec![(C::Read, V::List, p)],
        FileVerb::Stat => vec![(C::Read, V::Stat, p)],
        FileVerb::Create => vec![(C::Write, V::Create, p)],
        FileVerb::Update => vec![(C::Read, V::Stat, p.clone()), (C::Write, V::Update, p)],
        FileVerb::Write => vec![(C::Write, V::Create, p.clone()), (C::Write, V::Update, p)],
        FileVerb::Append => vec![(C::Write, V::Append, p)],
        FileVerb::Delete => vec![(C::Delete, V::Delete, p)],
        FileVerb::Copy | FileVerb::Move => {
            let to = op.to.clone().unwrap_or_default();
            let mut v = vec![
                (C::Read, V::Read, p.clone()),
                (C::Write, V::Create, to.clone()),
            ];
            if op.overwrite {
                v.push((C::Write, V::Update, to));
            }
            if op.verb == FileVerb::Move {
                v.push((C::Delete, V::Delete, p));
            }
            v
        }
    }
}

// vhco:usecase files.apply_file_operation(input: FileOperation) -> FileResult needs FileAccess
// vhco:label Apply a file operation
// vhco:about Authorizes every path the verb touches (source, destination) with its access verb before any handle opens, then performs the confined create/read/update/append/delete/list/stat/copy/move through FileAccess.
// vhco:example input={verb:"create", path:"./out/user.json", codec:"json", content:{id:42}} => { "path": "./out/user.json", "created": true }
pub async fn apply_file_operation(
    input: FileRequest,
    evaluator: &dyn PolicyEvaluator,
    files: &dyn FileAccess,
) -> RivetResult<Value> {
    // vhco:todo validate_file -- derive the intents of the verb (update = stat + update; write = create + update; copy/move = read source + create destination [+ update when overwriting] [+ delete source]) and authorize each through the evaluator before opening any handle
    // vhco:step authorize evaluator.evaluate -- one permit per intent; the first denial stops the operation with zero effects
    // vhco:error permission_denied -- any intent denied => permission.denied (403, exit 3) returns with the rule and zero effects
    for (capability, verb, path) in file_intents(&input.op) {
        let permit = evaluator.evaluate(&EffectIntent {
            capability,
            verb,
            target: EffectTarget::Path(path.clone()),
            operation_id: input.operation_id.clone(),
            effect_id: input.effect_id.clone(),
            span: input.span.clone(),
        });
        if permit.decision == Decision::Denied {
            return Err(RivetError::permission(format!(
                "{} {} on {path} denied: {}",
                capability.as_str(),
                verb.as_str(),
                permit.rule
            ))
            .with_span(input.span.clone())
            .with_details(Value::object([
                ("capability", Value::text(capability.as_str())),
                ("access", Value::text(verb.as_str())),
                ("target", Value::text(&path)),
            ])));
        }
    }
    // vhco:todo perform_file -- apply through FileAccess with root-relative no-follow confinement: create is exclusive, update requires an existing file (atomic replace), delete names one entry (missing ok when asked), hard-linked files are refused for write/delete (file.hardlink_refused); not_found/conflict keep their kinds
    // vhco:step apply files.apply -- the adapter returns the verb's result value
    files
        .apply(input.op)
        .await
        .map_err(|e| e.with_span(input.span))
}
