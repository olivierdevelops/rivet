use super::ports::{DataSink, ExecutionDriver, Registry};
use super::validate_output::{OutputCheck, validate_output};
use crate::domain::contracts::{
    CatalogQuery, Completion, DataEvent, ExecutionPlan, RegistryEntry, Request,
};
use crate::domain::errors::ErrorKind;
use crate::domain::outputs::{ParamSpec, ValueSpec};
use crate::domain::policy::PolicyLimits;
use crate::domain::{RivetError, RivetResult, Value};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// Checks every emitted item against the declared `emits` spec before it
/// reaches the consumer; the first violation stops the producer and becomes
/// the request's `output.invalid` error (with the item's seq).
struct EmitCheck {
    spec: ValueSpec,
    inner: Option<Arc<dyn DataSink>>,
    violation: Mutex<Option<RivetError>>,
}

#[async_trait]
impl DataSink for EmitCheck {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        let mut violations = Vec::new();
        self.spec.check(&event.data, "", &mut violations);
        if let Some(v) = violations.first() {
            let e = RivetError::new(
                ErrorKind::OutputInvalid,
                "output.invalid",
                format!(
                    "emitted item {} at {} must be {}, got {}",
                    event.seq,
                    if v.path.is_empty() { "$" } else { &v.path },
                    v.expected,
                    v.found
                ),
            )
            .with_details(Value::object([
                ("seq", Value::Int(event.seq as i64)),
                ("path", Value::text(&v.path)),
                ("expected", Value::text(&v.expected)),
                ("found", Value::text(&v.found)),
            ]));
            if let Ok(mut slot) = self.violation.lock() {
                slot.get_or_insert(e.clone());
            }
            return Err(e);
        }
        match &self.inner {
            Some(sink) => sink.send(event).await,
            None => Ok(()),
        }
    }
}

// vhco:usecase execution.request_operation(input: Request) -> Completion needs ExecutionDriver, ScopeSupervisor
// vhco:label Request an operation
// vhco:about The one dispatcher every surface uses: resolves the ID, rejects unknown/invalid params before injecting defaults, enforces call depth, drives the compiled body in a supervised scope, validates the result against the declared output and returns exactly one Completion or error.
// vhco:example input={operation_id:"demo.add", params:{a:2, b:3}} => { "result": 5, "data_count": 0, "effects": "none" }
pub async fn request_operation(
    input: Request,
    registry: &dyn Registry,
    driver: &dyn ExecutionDriver,
    limits: &PolicyLimits,
    sink: Option<Arc<dyn DataSink>>,
) -> RivetResult<Completion> {
    // vhco:todo validate_request -- resolve the ID in the pinned catalog (private IDs only for in-bundle calls), reject unknown fields and wrong types before injecting defaults, enforce limits.max_call_depth (limit.call_depth) before scheduling; concurrency/byte budgets are reserved by the host dispatcher
    // vhco:step resolve registry.describe -- a hidden or unknown ID is not_found without revealing private entries
    let entry = resolve(&input, registry)?;
    // vhco:error call_depth -- nesting deeper than limits.max_call_depth => limit.call_depth (429, exit 5) returns before any effect
    if input.depth > limits.max_call_depth {
        return Err(RivetError::new(
            ErrorKind::Limit,
            "limit.call_depth",
            format!(
                "call depth {} exceeds limits.max_call_depth {}",
                input.depth, limits.max_call_depth
            ),
        ));
    }
    // vhco:step params validate_params -- unknown fields, missing required, type, min/max/enum; then defaults
    let params = validate_params(&entry, &input.params).map_err(|e| tag(e, &input))?;
    // vhco:error streaming_unary -- an operation that `receives` input cannot run as a plain unary request => stream.input_required (422, exit 2) returns
    if entry.receives.is_some() && sink.is_none() {
        return Err(tag(
            RivetError::validation(
                "stream.input_required",
                format!(
                    "`{}` receives live input; open a session or use --input-jsonl",
                    entry.id
                ),
            ),
            &input,
        ));
    }

    // vhco:todo drive_scoped -- drive the compiled body through the injected driver; every emitted item is checked against the declared `emits` spec before it reaches the sink (first violation → output.invalid with details.seq, producer stopped); items flow to the sink with awaited demand; cancellation (the request's structured token: caller cancel or deadline) propagates without replaying emitted data
    // vhco:step emits EmitCheck -- wrap the sink so each data item is validated (declared json items are opaque)
    let check = entry.emits.clone().map(|spec| {
        Arc::new(EmitCheck {
            spec,
            inner: sink.clone(),
            violation: Mutex::new(None),
        })
    });
    let sink: Option<Arc<dyn DataSink>> = match &check {
        Some(c) => Some(c.clone()),
        None => sink,
    };
    // vhco:step drive driver.drive -- the driver owns the scope: resources close in reverse order on success, error, break, cancellation and deadline expiry
    let plan = ExecutionPlan {
        request: input.clone(),
        params,
    };
    // vhco:error emit_invalid -- an emitted item violates `emits` => output.invalid (500, exit 5) with details.seq and the run's effects returns
    let outcome = driver.drive(plan, sink).await.map_err(|e| {
        let violation = check
            .as_ref()
            .and_then(|c| c.violation.lock().ok().and_then(|mut v| v.take()));
        match violation {
            Some(mut v) => {
                v.effects = e.effects;
                tag(v, &input)
            }
            None => tag(e, &input),
        }
    })?;

    // vhco:todo finish_scope -- the scope is joined inside drive; validate the final result against the declared OutputSpec BEFORE building the Completion; mismatch → output.invalid (500, exit 5) with effects preserved; exactly one completion or primary error
    // vhco:step check validate_output -- structural check of the result
    let verdict = validate_output(&OutputCheck {
        operation_id: entry.id.clone(),
        spec: entry.output.clone(),
        result: outcome.result.clone(),
        effects: outcome.effects,
    });
    if let Some(err) = verdict.error {
        return Err(tag(err, &input));
    }
    Ok(Completion {
        request_id: input.request_id,
        trace_id: input.trace_id,
        result: outcome.result,
        data_count: outcome.data_count,
        effects: outcome.effects,
    })
}

fn tag(mut e: RivetError, req: &Request) -> RivetError {
    if e.operation_id.is_none() {
        e.operation_id = Some(req.operation_id.clone());
    }
    e.request_id.get_or_insert_with(|| req.request_id.clone());
    e.trace_id.get_or_insert_with(|| req.trace_id.clone());
    e
}

fn resolve(input: &Request, registry: &dyn Registry) -> RivetResult<RegistryEntry> {
    let catalog = registry.describe(&CatalogQuery {
        ids: vec![input.operation_id.clone()],
        include_private: input.include_private,
    })?;
    catalog.entries.into_iter().next().ok_or_else(|| {
        tag(
            RivetError::not_found(
                "not_found.operation",
                format!("no operation `{}`", input.operation_id),
            ),
            input,
        )
    })
}

/// Validate caller params against the declared parameters; returns an object in declaration order.
pub fn validate_params(entry: &RegistryEntry, params: &Value) -> RivetResult<Value> {
    let given: &[(String, Value)] = match params {
        Value::Object(pairs) => pairs,
        Value::Null => &[],
        other => {
            return Err(RivetError::validation(
                "validation.params",
                format!("params must be an object, got {}", other.type_name()),
            ));
        }
    };
    for (k, _) in given {
        if !entry.params.iter().any(|p| &p.name == k) {
            let known: Vec<&str> = entry.params.iter().map(|p| p.name.as_str()).collect();
            return Err(RivetError::validation(
                "validation.unknown_field",
                format!("unknown parameter `{k}` for `{}`", entry.id),
            )
            .with_details(Value::object([
                ("field", Value::text(k)),
                (
                    "known",
                    Value::List(known.iter().map(|s| Value::text(*s)).collect()),
                ),
            ])));
        }
    }
    let mut out = Value::Object(Vec::new());
    for p in &entry.params {
        let value = given
            .iter()
            .find(|(k, _)| k == &p.name)
            .map(|(_, v)| v.clone());
        let value = match value {
            Some(Value::Null) | None => match (&p.default, p.required) {
                (Some(d), _) => d.clone(),
                (None, true) => {
                    return Err(RivetError::validation(
                        "validation.required",
                        format!("missing required parameter `{}`", p.name),
                    )
                    .with_details(Value::object([("field", Value::text(&p.name))])));
                }
                (None, false) => Value::Null,
            },
            Some(v) => check_param(p, v)?,
        };
        out.set(&p.name, value);
    }
    Ok(out)
}

fn check_param(p: &ParamSpec, v: Value) -> RivetResult<Value> {
    let bad = |expected: &str, v: &Value| {
        RivetError::validation(
            "validation.type",
            format!(
                "parameter `{}` must be {expected}, got {}",
                p.name,
                v.type_name()
            ),
        )
        .with_details(Value::object([("field", Value::text(&p.name))]))
    };
    let v = match (&p.spec, v) {
        (ValueSpec::Integer, Value::Float(f)) if f.fract() == 0.0 && f.abs() < 9.0e15 => {
            Value::Int(f as i64)
        }
        (ValueSpec::Integer, v @ Value::Int(_)) => v,
        (ValueSpec::Integer, v) => return Err(bad("an integer", &v)),
        // A `number` parameter is a number even when the caller wrote `2`:
        // `(a + b) / 2` with a=2, b=5 is 3.5 (S121), not integer division.
        (ValueSpec::Number, Value::Int(i)) => Value::Float(i as f64),
        (ValueSpec::Number, v @ Value::Float(_)) => v,
        (ValueSpec::Number, v) => return Err(bad("a number", &v)),
        (ValueSpec::Text, v @ Value::Text(_)) => v,
        (ValueSpec::Text, v) => return Err(bad("text", &v)),
        (ValueSpec::Boolean, v @ Value::Bool(_)) => v,
        (ValueSpec::Boolean, v) => return Err(bad("a boolean", &v)),
        (ValueSpec::Bytes, v @ Value::Bytes(_)) => v,
        (ValueSpec::Bytes, v) => return Err(bad("bytes ({\"$type\":\"bytes\",\"base64\":…})", &v)),
        (ValueSpec::Json, v) => v,
        (spec, v) => {
            let mut violations = Vec::new();
            spec.check(&v, &p.name, &mut violations);
            if let Some(first) = violations.first() {
                return Err(RivetError::validation(
                    "validation.type",
                    format!(
                        "parameter at {} must be {}, got {}",
                        first.path, first.expected, first.found
                    ),
                ));
            }
            v
        }
    };
    let num = match &v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    };
    if let (Some(n), Some(min)) = (num, p.min)
        && n < min
    {
        return Err(RivetError::validation(
            "validation.min",
            format!("parameter `{}` must be ≥ {min}", p.name),
        ));
    }
    if let (Some(n), Some(max)) = (num, p.max)
        && n > max
    {
        return Err(RivetError::validation(
            "validation.max",
            format!("parameter `{}` must be ≤ {max}", p.name),
        ));
    }
    if let Some(allowed) = &p.enum_values
        && !allowed.contains(&v)
    {
        return Err(RivetError::validation(
            "validation.enum",
            format!(
                "parameter `{}` must be one of {}",
                p.name,
                Value::List(allowed.clone())
            ),
        ));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ir::OperationKind;
    use crate::domain::outputs::OutputSpec;
    use crate::domain::source::SourceSpan;

    fn entry() -> RegistryEntry {
        RegistryEntry {
            id: "demo.add".into(),
            name: "Add".into(),
            description: None,
            kind: OperationKind::Operation,
            private: false,
            params: vec![
                ParamSpec {
                    name: "a".into(),
                    spec: ValueSpec::Integer,
                    required: true,
                    default: None,
                    min: None,
                    max: None,
                    enum_values: None,
                    description: None,
                },
                ParamSpec {
                    name: "b".into(),
                    spec: ValueSpec::Integer,
                    required: false,
                    default: Some(Value::Int(0)),
                    min: None,
                    max: None,
                    enum_values: None,
                    description: None,
                },
            ],
            output: OutputSpec {
                spec: ValueSpec::Integer,
                description: None,
            },
            emits: None,
            receives: None,
            errors: vec![],
            source: SourceSpan::default(),
            raw_input_schema: None,
            raw_output_schema: None,
        }
    }

    // vhco:test execution.request_operation -- params are checked for unknown fields and types before defaults are injected
    #[test]
    fn params_validation_and_defaults() {
        let e = entry();
        assert_eq!(
            validate_params(&e, &Value::object([("a", Value::Int(2))])).unwrap(),
            Value::object([("a", Value::Int(2)), ("b", Value::Int(0))])
        );
        assert_eq!(
            validate_params(&e, &Value::object([("b", Value::Int(1))]))
                .unwrap_err()
                .code,
            "validation.required"
        );
        assert_eq!(
            validate_params(
                &e,
                &Value::object([("a", Value::Int(1)), ("c", Value::Int(1))])
            )
            .unwrap_err()
            .code,
            "validation.unknown_field"
        );
        assert_eq!(
            validate_params(&e, &Value::object([("a", Value::text("2"))]))
                .unwrap_err()
                .code,
            "validation.type"
        );
    }

    struct Emitter;

    #[async_trait]
    impl ExecutionDriver for Emitter {
        async fn drive(
            &self,
            plan: ExecutionPlan,
            sink: Option<Arc<dyn DataSink>>,
        ) -> RivetResult<crate::domain::contracts::RunOutcome> {
            let sink = sink.expect("an emitting operation always gets a checking sink");
            for (seq, data) in [(1, Value::Int(1)), (2, Value::text("two"))] {
                sink.send(DataEvent {
                    request_id: plan.request.request_id.clone(),
                    trace_id: plan.request.trace_id.clone(),
                    seq,
                    data,
                })
                .await
                .map_err(|e| {
                    RivetError::new(ErrorKind::ConsumerFailed, "consumer_failed", e.message)
                })?;
            }
            Ok(crate::domain::contracts::RunOutcome {
                result: Value::Int(2),
                data_count: 2,
                effects: Default::default(),
            })
        }
    }

    struct OneEntry(RegistryEntry);

    impl Registry for OneEntry {
        fn describe(&self, _: &CatalogQuery) -> RivetResult<crate::domain::contracts::Catalog> {
            Ok(crate::domain::contracts::Catalog {
                entries: vec![self.0.clone()],
            })
        }
        fn program(&self) -> Arc<crate::domain::ir::CompiledProgram> {
            unreachable!()
        }
    }

    // vhco:test execution.request_operation -- G13: an emitted item that violates the declared `emits` spec fails the request with output.invalid carrying the item's seq (even without a consumer)
    #[tokio::test]
    async fn emitted_items_are_validated() {
        let mut e = entry();
        e.params.clear();
        e.emits = Some(ValueSpec::Integer);
        let req = Request {
            request_id: "req_1".into(),
            trace_id: "tr_1".into(),
            operation_id: "demo.add".into(),
            params: Value::Null,
            principal: crate::domain::contracts::Principal::local(),
            parent_request_id: None,
            depth: 0,
            deadline_ms: 1000,
            include_private: false,
            parent_span_id: None,
            cancel: Default::default(),
        };
        let err = request_operation(req, &OneEntry(e), &Emitter, &PolicyLimits::default(), None)
            .await
            .unwrap_err();
        assert_eq!(err.code, "output.invalid");
        assert_eq!(err.details.get("seq"), Some(&Value::Int(2)));
        assert_eq!(err.request_id.as_deref(), Some("req_1"));
    }

    // vhco:test execution.request_operation -- regression (S121): an integer argument for a `number` parameter becomes a number, so arithmetic on it is not integer division
    #[test]
    fn number_params_are_numbers() {
        let mut e = entry();
        e.params[0].spec = ValueSpec::Number;
        let v = validate_params(&e, &Value::object([("a", Value::Int(5))])).unwrap();
        assert_eq!(v.get("a"), Some(&Value::Float(5.0)));
    }
}
