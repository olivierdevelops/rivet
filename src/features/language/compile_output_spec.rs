use super::lowering::lower::Lowerer;
use crate::domain::RivetResult;
use crate::domain::outputs::OutputSpec;
use crate::domain::syntax_tree::SyntaxNode;

// vhco:usecase language.compile_output_spec(input: OutputDeclaration) -> OutputSpec
// vhco:label Compile an output declaration
// vhco:about Lowers one `output`/`emits`/`receives` header (scalar or `object … end` field block) into a typed OutputSpec with descriptions; JSON Schema is projected from it on demand.
// vhco:example input="output integer description \"Sum of a and b.\"" => { "spec": "integer", "description": "Sum of a and b." }
pub fn compile_output_spec(input: &SyntaxNode) -> RivetResult<OutputSpec> {
    let mut lowerer = Lowerer::default();
    // vhco:todo read_scalar_or_block -- read TYPE from the header (text|integer|number|boolean|bytes|json|object|list T) and an optional `description "…"`; an `object` type reads its nested `field NAME TYPE [required|optional] [description "…"]` lines (nested objects recurse) and `open true`
    // vhco:step lower lowerer.typed_decl -- the same lowering the compiler uses for output, emits and receives
    let lowered = lowerer.typed_decl(input);
    // vhco:todo validate_fields -- unknown types, duplicate field names at one level, field blocks on non-object types and unknown field modifiers are kind-syntax errors with the field's span
    // vhco:todo lower_item_and_errors -- emits/receives item schemas use this same lowering; declared `error` lines are collected by the operation lowering into DeclaredError{code, description}
    // vhco:error invalid_output -- malformed output declaration => syntax error returns (first error, others suppressed)
    if !lowerer.errors.is_empty() {
        let mut errors = lowerer.errors;
        let mut first = errors.remove(0);
        first.suppressed = errors;
        return Err(first);
    }
    // vhco:todo emit_json_schema -- JSON Schema is derived from the ValueSpec tree by ValueSpec::to_json_schema (text→string, bytes→tagged base64 object, object→closed properties/required unless open) whenever a surface projects it
    let (spec, description) = lowered.expect("typed_decl without errors returns a spec");
    Ok(OutputSpec { spec, description })
}
