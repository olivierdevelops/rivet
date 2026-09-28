//! Small readers for evaluated effect-form arguments, shared by the
//! transport adapters (keywords stay `EvalArg::Word`, values are evaluated).

use super::execution_driver::{EffectCtx, EvalArg, EvaluatedForm};
use crate::domain::files::{Codec as FileCodec, FileOperation, FileVerb};
use crate::domain::ir::{EffectForm, parse_duration_ms};
use crate::domain::ports::FileAccess;
use crate::domain::transports::EffectOrigin;
use crate::domain::{RivetError, RivetResult, Value};

pub fn bad(code: &str, msg: impl Into<String>) -> RivetError {
    RivetError::validation(code, msg)
}

pub fn text(a: Option<&EvalArg>) -> Option<String> {
    match a? {
        EvalArg::Value(Value::Text(s)) => Some(s.clone()),
        EvalArg::Value(v) => Some(v.to_display()),
        EvalArg::Word(_) => None,
    }
}

pub fn word(a: Option<&EvalArg>) -> Option<&str> {
    a.and_then(EvalArg::word)
}

pub fn int(a: Option<&EvalArg>) -> Option<i64> {
    match a? {
        EvalArg::Value(Value::Int(i)) => Some(*i),
        EvalArg::Value(Value::Float(f)) if f.fract() == 0.0 => Some(*f as i64),
        _ => None,
    }
}

pub fn boolean(a: Option<&EvalArg>) -> Option<bool> {
    match a? {
        EvalArg::Value(Value::Bool(b)) => Some(*b),
        EvalArg::Word(w) if w == "true" => Some(true),
        EvalArg::Word(w) if w == "false" => Some(false),
        _ => None,
    }
}

pub fn duration(a: Option<&EvalArg>) -> Option<u64> {
    text(a).and_then(|t| parse_duration_ms(&t))
}

pub fn int_list(a: Option<&EvalArg>) -> Option<Vec<i64>> {
    match a? {
        EvalArg::Value(Value::List(items)) => items.iter().map(Value::as_i64).collect(),
        _ => None,
    }
}

/// A key position (`query KEY V`, `header KEY V`): the raw word as written,
/// even when a variable of that name exists, else the evaluated text.
pub fn key(form: &EffectForm, index: usize, pos: usize, args: &[EvalArg]) -> Option<String> {
    form.options
        .get(index)
        .and_then(|o| o.args.get(pos))
        .and_then(|a| a.word())
        .map(str::to_string)
        .or_else(|| text(args.get(pos)))
}

/// Option list with each entry's raw index, for `key` lookups.
pub fn options(f: &EvaluatedForm) -> impl Iterator<Item = (usize, &str, &[EvalArg])> {
    f.options
        .iter()
        .enumerate()
        .map(|(i, (k, a))| (i, k.as_str(), a.as_slice()))
}

pub fn origin(ctx: &EffectCtx) -> EffectOrigin {
    EffectOrigin {
        operation_id: ctx.operation_id.clone(),
        span: Some(ctx.span.clone()),
    }
}

/// Effective time budget: the option (if any) capped by the request deadline.
pub fn budget_ms(ctx: &EffectCtx, option: Option<u64>) -> u64 {
    let remaining = ctx.remaining().as_millis() as u64;
    option.map(|t| t.min(remaining)).unwrap_or(remaining).max(1)
}

/// One `tls …` line: `tls true`, `tls server_name V`, `tls ca_file|cert_file|key_file PATH`
/// (file-valued lines are read through the policed file port right here).
/// Returns true when the line requests TLS on a transport (`tls true` or any line).
pub async fn apply_tls(
    files: &dyn FileAccess,
    args: &[EvalArg],
    tls: &mut crate::domain::transports::TlsMaterial,
) -> RivetResult<()> {
    let what = word(args.first()).unwrap_or("");
    match what {
        "server_name" => {
            tls.server_name = Some(
                text(args.get(1))
                    .ok_or_else(|| bad("validation.tls", "`tls server_name` needs a string"))?,
            )
        }
        "ca_file" | "cert_file" | "key_file" => {
            let path = text(args.get(1)).ok_or_else(|| {
                bad(
                    "validation.tls",
                    format!("`tls {what}` needs a path string"),
                )
            })?;
            let bytes = read_file(files, &path).await?;
            match what {
                "ca_file" => tls.ca_pem = Some(bytes),
                "cert_file" => tls.cert_pem = Some(bytes),
                _ => tls.key_pem = Some(bytes),
            }
        }
        _ if boolean(args.first()).is_some() => {}
        other => {
            return Err(bad(
                "validation.tls",
                format!(
                    "unknown tls option `{other}` (true, server_name, ca_file, cert_file, key_file)"
                ),
            ));
        }
    }
    Ok(())
}

/// Read a file named by an option (`tls ca_file`, `body file`) through the
/// policed file port: authorized as allow_read before any connection.
pub async fn read_file(files: &dyn FileAccess, path: &str) -> RivetResult<Vec<u8>> {
    let mut op = FileOperation::new(FileVerb::Read, path);
    op.codec = Some(FileCodec::Bytes);
    match files.apply(op).await? {
        Value::Bytes(b) => Ok(b),
        Value::Text(s) => Ok(s.into_bytes()),
        other => Ok(other.to_display().into_bytes()),
    }
}
