use crate::domain::errors::{EffectsStatus, ErrorKind};
use crate::domain::outputs::{OutputSpec, SchemaViolation, ValueSpec};
use crate::domain::{RivetError, Value};

// vhco:domain OutputCheck { operation_id: string; spec: OutputSpec; result: Value; effects: EffectsStatus }
#[derive(Clone, Debug, PartialEq)]
pub struct OutputCheck {
    pub operation_id: String,
    pub spec: OutputSpec,
    pub result: Value,
    pub effects: EffectsStatus,
}

// vhco:domain OutputVerdict { valid: bool; violations: SchemaViolation[]; error?: RivetError }
#[derive(Clone, Debug, PartialEq)]
pub struct OutputVerdict {
    pub valid: bool,
    pub violations: Vec<SchemaViolation>,
    pub error: Option<RivetError>,
}

const MAX_VIOLATIONS: usize = 32;

// vhco:usecase execution.validate_output(input: OutputCheck) -> OutputVerdict
// vhco:label Validate a result against its declared output
// vhco:about Pure structural check of a final result against the operation's OutputSpec before any Completion is built; a mismatch becomes output.invalid with committed effects preserved.
// vhco:example input={spec:"integer", result:"five"} => { "valid": false, "violations": [{ "path": "$", "expected": "integer", "found": "text" }] }
pub fn validate_output(input: &OutputCheck) -> OutputVerdict {
    // vhco:todo skip_opaque -- output type json (the default when no output line exists) is valid without a structural check
    // vhco:step opaque return -- json accepts any value
    if input.spec.spec == ValueSpec::Json {
        return OutputVerdict {
            valid: true,
            violations: Vec::new(),
            error: None,
        };
    }
    // vhco:todo check_structure -- scalar types must match, required fields present and non-null, optional fields may be absent, closed objects reject extra keys, list elements and nested objects recurse; at most 32 violations then a truncation count
    // vhco:step walk spec.check -- collects {path, expected, found}
    let mut violations = Vec::new();
    input.spec.spec.check(&input.result, "", &mut violations);
    if violations.is_empty() {
        return OutputVerdict {
            valid: true,
            violations,
            error: None,
        };
    }
    let total = violations.len();
    violations.truncate(MAX_VIOLATIONS);
    // vhco:todo verdict -- invalid → RivetError{kind output_invalid, code output.invalid, retryable false, effects copied from the check so committed effects stay reported}; never mutates the result
    // vhco:error output_invalid -- result violates the declared output => output.invalid (HTTP 500, exit 5) returned in the verdict
    let first = &violations[0];
    let mut details = Value::object([(
        "violations",
        Value::List(violations.iter().map(SchemaViolation::to_value).collect()),
    )]);
    if total > MAX_VIOLATIONS {
        details.set("truncated", Value::Int((total - MAX_VIOLATIONS) as i64));
    }
    let mut error = RivetError::new(
        ErrorKind::OutputInvalid,
        "output.invalid",
        format!(
            "`{}` returned a result that does not match its declared output: at {} expected {}, found {}",
            input.operation_id, first.path, first.expected, first.found
        ),
    )
    .with_details(details)
    .with_effects(input.effects);
    error.operation_id = Some(input.operation_id.clone());
    OutputVerdict {
        valid: false,
        violations,
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // vhco:test execution.validate_output -- a text result for an integer output is output.invalid with effects preserved
    #[test]
    fn mismatch_is_output_invalid() {
        let v = validate_output(&OutputCheck {
            operation_id: "demo.add".into(),
            spec: OutputSpec {
                spec: ValueSpec::Integer,
                description: None,
            },
            result: Value::text("five"),
            effects: EffectsStatus::Committed,
        });
        assert!(!v.valid);
        let e = v.error.unwrap();
        assert_eq!(e.code, "output.invalid");
        assert_eq!(e.exit_code(), 5);
        assert_eq!(e.effects, EffectsStatus::Committed);
    }

    #[test]
    fn json_output_is_opaque() {
        let v = validate_output(&OutputCheck {
            operation_id: "x".into(),
            spec: OutputSpec::default(),
            result: Value::Int(1),
            effects: EffectsStatus::None,
        });
        assert!(v.valid);
    }
}
