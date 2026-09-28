use super::ports::PolicyEvaluator;
use crate::domain::files::{
    DEFAULT_CHUNK_SIZE, FileStreamMode, FileStreamPlan, FileStreamRequest, MAX_CHUNK_SIZE,
};
use crate::domain::policy::{AccessVerb, Capability, Decision, EffectIntent, EffectTarget};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};

/// The (capability, verb) intents one handle mode needs — the same DSL → verb
/// table as the one-shot verbs (`file read` / `file write` / `file append`).
///
/// ```text
///   mode read   ─▶ allow_read  read
///   mode write  ─▶ allow_write create + update   (create or truncate)
///   mode append ─▶ allow_write append            (existing file only)
/// ```
pub fn stream_intents(mode: FileStreamMode) -> Vec<(Capability, AccessVerb)> {
    match mode {
        FileStreamMode::Read => vec![(Capability::Read, AccessVerb::Read)],
        FileStreamMode::Write => vec![
            (Capability::Write, AccessVerb::Create),
            (Capability::Write, AccessVerb::Update),
        ],
        FileStreamMode::Append => vec![(Capability::Write, AccessVerb::Append)],
    }
}

// vhco:usecase files.open_file_stream(input: FileStreamRequest) -> FileStreamPlan needs PolicyEvaluator
// vhco:label Open a scoped file handle
// vhco:about Validates `with file open PATH mode read|write|append as NAME` (+ `chunk_size N`) and authorizes the mode's access verbs on the path before the confined adapter opens a no-follow, root-relative handle that yields bytes chunks (read) or accepts `NAME.write` (write/append) and closes on scope exit.
// vhco:example input={path:"./data/lines.txt", mode:"read", chunk_size:65536} => { "path": "./data/lines.txt", "mode": "read", "chunk_size": 65536 }
pub fn open_file_stream(
    input: FileStreamRequest,
    evaluator: &dyn PolicyEvaluator,
) -> RivetResult<FileStreamPlan> {
    // vhco:step validate mode -- `mode` must be read, write or append (validation.file_mode, exit 2); `chunk_size` defaults to 65536 and must be 1..=8 MiB (limit.chunk_size)
    let span = input.span.clone();
    let mode = FileStreamMode::parse(&input.mode).ok_or_else(|| {
        RivetError::validation(
            "validation.file_mode",
            format!(
                "`with file open` mode must be read, write or append, got `{}`",
                input.mode
            ),
        )
        .with_span(span.clone())
    })?;
    let chunk_size = match input.chunk_size {
        None => DEFAULT_CHUNK_SIZE,
        Some(n) if n >= 1 && (n as u64) <= MAX_CHUNK_SIZE => n as u64,
        Some(n) => {
            return Err(RivetError::new(
                ErrorKind::Limit,
                "limit.chunk_size",
                format!("chunk_size {n} is outside 1..={MAX_CHUNK_SIZE} bytes"),
            )
            .with_span(span));
        }
    };
    // vhco:step authorize evaluator.evaluate -- one permit per intent of the mode; the first denial stops before any handle opens
    // vhco:error permission_denied -- any intent denied => permission.denied (403, exit 3) with capability/access/target details and zero effects
    for (capability, verb) in stream_intents(mode) {
        let permit = evaluator.evaluate(&EffectIntent {
            capability,
            verb,
            target: EffectTarget::Path(input.path.clone()),
            operation_id: input.operation_id.clone(),
            effect_id: None,
            span: span.clone(),
        });
        if permit.decision == Decision::Denied {
            return Err(RivetError::permission(format!(
                "{} {} on {} denied: {}",
                capability.as_str(),
                verb.as_str(),
                input.path,
                permit.rule
            ))
            .with_span(span)
            .with_details(Value::object([
                ("capability", Value::text(capability.as_str())),
                ("access", Value::text(verb.as_str())),
                ("target", Value::text(&input.path)),
            ])));
        }
    }
    Ok(FileStreamPlan {
        path: input.path,
        mode,
        chunk_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::{Permit, Policy};

    struct Only(Vec<(Capability, AccessVerb)>, Policy);

    impl PolicyEvaluator for Only {
        fn evaluate(&self, intent: &EffectIntent) -> Permit {
            let ok = self.0.contains(&(intent.capability, intent.verb));
            Permit {
                intent: intent.clone(),
                decision: if ok {
                    Decision::Allowed
                } else {
                    Decision::Denied
                },
                rule: "test".into(),
            }
        }
        fn policy(&self) -> &Policy {
            &self.1
        }
    }

    fn req(mode: &str, chunk: Option<i64>) -> FileStreamRequest {
        FileStreamRequest {
            path: "./data/a.txt".into(),
            mode: mode.into(),
            chunk_size: chunk,
            operation_id: "t.op".into(),
            span: None,
        }
    }

    // vhco:test files.open_file_stream -- modes map to read / create+update / append intents and chunk_size is bounded
    #[test]
    fn modes_authorize_their_verbs_and_bound_chunks() {
        let read = Only(
            vec![(Capability::Read, AccessVerb::Read)],
            Policy::default(),
        );
        let plan = open_file_stream(req("read", None), &read).unwrap();
        assert_eq!(plan.chunk_size, DEFAULT_CHUNK_SIZE);
        assert_eq!(plan.mode, FileStreamMode::Read);
        let e = open_file_stream(req("write", None), &read).unwrap_err();
        assert_eq!(e.code, "permission.denied");
        let create_only = Only(
            vec![(Capability::Write, AccessVerb::Create)],
            Policy::default(),
        );
        assert_eq!(
            open_file_stream(req("write", None), &create_only)
                .unwrap_err()
                .code,
            "permission.denied"
        );
        let append = Only(
            vec![(Capability::Write, AccessVerb::Append)],
            Policy::default(),
        );
        assert!(open_file_stream(req("append", Some(16)), &append).is_ok());
        assert_eq!(
            open_file_stream(req("read", Some(0)), &read)
                .unwrap_err()
                .code,
            "limit.chunk_size"
        );
        assert_eq!(
            open_file_stream(req("rw", None), &read).unwrap_err().code,
            "validation.file_mode"
        );
    }
}
