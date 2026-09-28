//! Pure value semantics shared by the interpreter and load-time constant
//! evaluation (`global NAME = EXPR`, PROP-2026-0002 R7) and the static effect
//! analysis (globals substituted into targets, R9).
//!
//! ```text
//!   binary / values_equal / pure_call ◀── infra::execution_driver (requests)
//!             │
//!             └──────────────────────◀── fold(expr, lookup)  (compile_globals, effect sites)
//! ```

use super::errors::{RivetError, RivetResult};
use super::ir::{BinOp, Expr, TemplatePart};
use super::value::Value;
use base64::Engine;

/// Built-in functions without effects: a global may call these (R7).
pub const PURE_FUNCTIONS: &[&str] = &[
    "length",
    "base64.encode",
    "base64.decode",
    "text",
    "keys",
    "xml.element",
];

pub fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// One binary operator over two evaluated operands (the language's arithmetic,
/// comparison, text/list concatenation and boolean rules).
pub fn binary(op: BinOp, l: Value, r: Value) -> RivetResult<Value> {
    use BinOp::*;
    let overflow = || RivetError::validation("value.overflow", "integer overflow");
    Ok(match (op, l, r) {
        (Eq, a, b) => Value::Bool(values_equal(&a, &b)),
        (Ne, a, b) => Value::Bool(!values_equal(&a, &b)),
        (And, a, b) => Value::Bool(a.truthy() && b.truthy()),
        (Or, a, b) => {
            if a.truthy() {
                a
            } else {
                b
            }
        }
        (Add, Value::Text(a), b) => Value::Text(a + &b.to_display()),
        (Add, Value::List(mut a), Value::List(b)) => {
            a.extend(b);
            Value::List(a)
        }
        (Add, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_add(b).ok_or_else(overflow)?),
        (Sub, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_sub(b).ok_or_else(overflow)?),
        (Mul, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_mul(b).ok_or_else(overflow)?),
        (Div, Value::Int(_), Value::Int(0)) | (Rem, Value::Int(_), Value::Int(0)) => {
            return Err(RivetError::validation(
                "value.division_by_zero",
                "division by zero",
            ));
        }
        (Div, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_div(b).ok_or_else(overflow)?),
        (Rem, Value::Int(a), Value::Int(b)) => Value::Int(a.checked_rem(b).ok_or_else(overflow)?),
        (op @ (Lt | Le | Gt | Ge), Value::Text(a), Value::Text(b)) => Value::Bool(match op {
            Lt => a < b,
            Le => a <= b,
            Gt => a > b,
            _ => a >= b,
        }),
        (op, a, b) => match (num(&a), num(&b)) {
            (Some(x), Some(y)) => match op {
                Add => Value::Float(x + y),
                Sub => Value::Float(x - y),
                Mul => Value::Float(x * y),
                Div if y == 0.0 => {
                    return Err(RivetError::validation(
                        "value.division_by_zero",
                        "division by zero",
                    ));
                }
                Div => Value::Float(x / y),
                Rem => Value::Float(x % y),
                Lt => Value::Bool(x < y),
                Le => Value::Bool(x <= y),
                Gt => Value::Bool(x > y),
                Ge => Value::Bool(x >= y),
                _ => unreachable!(),
            },
            _ => {
                return Err(RivetError::validation(
                    "value.type",
                    format!(
                        "operator {op:?} does not apply to {} and {}",
                        a.type_name(),
                        b.type_name()
                    ),
                ));
            }
        },
    })
}

/// A pure built-in over evaluated arguments: `None` when `func` is not one of
/// [`PURE_FUNCTIONS`]; otherwise its value or `(code, message)`.
pub fn pure_call(func: &str, vals: &[Value]) -> Option<Result<Value, (&'static str, String)>> {
    let arity = |n: usize| -> Result<(), (&'static str, String)> {
        if vals.len() != n {
            return Err((
                "call.arity",
                format!("({func} …) takes {n} argument(s), got {}", vals.len()),
            ));
        }
        Ok(())
    };
    let out = match func {
        "length" => arity(1).and_then(|_| {
            Ok(Value::Int(match &vals[0] {
                Value::List(l) => l.len(),
                Value::Text(s) => s.chars().count(),
                Value::Object(o) => o.len(),
                Value::Bytes(b) => b.len(),
                other => {
                    return Err((
                        "call.length",
                        format!("(length …) of {}", other.type_name()),
                    ));
                }
            } as i64))
        }),
        "base64.encode" => arity(1).and_then(|_| {
            let bytes = match &vals[0] {
                Value::Bytes(b) => b.clone(),
                Value::Text(s) => s.as_bytes().to_vec(),
                other => {
                    return Err((
                        "call.base64",
                        format!("(base64.encode …) of {}", other.type_name()),
                    ));
                }
            };
            Ok(Value::Text(
                base64::engine::general_purpose::STANDARD.encode(bytes),
            ))
        }),
        "xml.element" => arity(3).and_then(|_| {
            super::transports::xml_element(&vals[0], &vals[1], &vals[2])
                .map_err(|m| ("call.xml", m))
        }),
        "base64.decode" => arity(1).and_then(|_| {
            let text = vals[0]
                .as_str()
                .ok_or(("call.base64", "(base64.decode …) needs text".to_string()))?;
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .map(Value::Bytes)
                .map_err(|e| ("call.base64", format!("invalid base64: {e}")))
        }),
        "text" => arity(1).map(|_| Value::Text(vals[0].to_display())),
        "keys" => arity(1).and_then(|_| match &vals[0] {
            Value::Object(o) => Ok(Value::List(o.iter().map(|(k, _)| Value::text(k)).collect())),
            other => Err(("call.keys", format!("(keys …) of {}", other.type_name()))),
        }),
        _ => return None,
    };
    Some(out)
}

/// Read `path[1..]` below `root` (object keys, list indices, `list.length`).
pub fn read_path(root: &Value, path: &[String]) -> Option<Value> {
    let mut cur = root.clone();
    for seg in path {
        cur = match &cur {
            Value::Object(_) => cur.get(seg).cloned()?,
            Value::List(items) => match seg.parse::<usize>() {
                Ok(n) => items.get(n).cloned()?,
                Err(_) if seg == "length" => Value::Int(items.len() as i64),
                Err(_) => return None,
            },
            _ => return None,
        };
    }
    Some(cur)
}

/// Evaluate an expression that reads only constants: literals, lists,
/// objects, arithmetic, comparisons, `${…}` templates, pure built-ins and the
/// dotted paths `lookup` resolves (globals). `Ok(None)` when some part is not
/// constant (a param, local, effect or `request`); `Err` when a constant part
/// fails to evaluate (division by zero, a type error).
pub fn fold(e: &Expr, lookup: &dyn Fn(&[String]) -> Option<Value>) -> RivetResult<Option<Value>> {
    Ok(Some(match e {
        Expr::Lit(v) => v.clone(),
        Expr::Template(parts) => {
            let mut s = String::new();
            for p in parts {
                match p {
                    TemplatePart::Lit(l) => s.push_str(l),
                    TemplatePart::Path(path) => match lookup(path) {
                        Some(v) => s.push_str(&v.to_display()),
                        None => return Ok(None),
                    },
                }
            }
            Value::Text(s)
        }
        Expr::Path(path, _) => match lookup(path) {
            Some(v) => v,
            None => return Ok(None),
        },
        Expr::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                match fold(i, lookup)? {
                    Some(v) => out.push(v),
                    None => return Ok(None),
                }
            }
            Value::List(out)
        }
        Expr::Object(pairs) => {
            let mut out = Vec::with_capacity(pairs.len());
            for (k, v) in pairs {
                match fold(v, lookup)? {
                    Some(v) => out.push((k.clone(), v)),
                    None => return Ok(None),
                }
            }
            Value::Object(out)
        }
        Expr::Not(inner) => match fold(inner, lookup)? {
            Some(v) => Value::Bool(!v.truthy()),
            None => return Ok(None),
        },
        Expr::Neg(inner) => match fold(inner, lookup)? {
            Some(Value::Int(i)) => Value::Int(
                i.checked_neg()
                    .ok_or_else(|| RivetError::validation("value.overflow", "integer overflow"))?,
            ),
            Some(Value::Float(f)) => Value::Float(-f),
            Some(other) => {
                return Err(RivetError::validation(
                    "value.type",
                    format!("cannot negate {}", other.type_name()),
                ));
            }
            None => return Ok(None),
        },
        Expr::Binary { op, lhs, rhs } => {
            let (Some(l), Some(r)) = (fold(lhs, lookup)?, fold(rhs, lookup)?) else {
                return Ok(None);
            };
            binary(*op, l, r)?
        }
        Expr::Call { func, args, .. } => {
            if !PURE_FUNCTIONS.contains(&func.as_str()) {
                return Ok(None);
            }
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                match fold(a, lookup)? {
                    Some(v) => vals.push(v),
                    None => return Ok(None),
                }
            }
            match pure_call(func, &vals) {
                Some(Ok(v)) => v,
                Some(Err((code, msg))) => return Err(RivetError::validation(code, msg)),
                None => return Ok(None),
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(p: &[String]) -> Option<Value> {
        let root = Value::object([
            ("api", Value::text("https://api.example.com")),
            ("n", Value::Int(50)),
        ]);
        read_path(&root.get(&p[0])?.clone(), &p[1..])
    }

    #[test]
    fn folds_templates_arithmetic_and_pure_calls() {
        let t = Expr::Template(vec![
            TemplatePart::Path(vec!["api".into()]),
            TemplatePart::Lit("/users".into()),
        ]);
        assert_eq!(
            fold(&t, &lookup).unwrap(),
            Some(Value::text("https://api.example.com/users"))
        );
        let sum = Expr::Binary {
            op: BinOp::Mul,
            lhs: Box::new(Expr::Path(vec!["n".into()], Default::default())),
            rhs: Box::new(Expr::Lit(Value::Int(2))),
        };
        assert_eq!(fold(&sum, &lookup).unwrap(), Some(Value::Int(100)));
        let unknown = Expr::Path(vec!["id".into()], Default::default());
        assert_eq!(fold(&unknown, &lookup).unwrap(), None);
        let len = Expr::Call {
            func: "length".into(),
            args: vec![Expr::Lit(Value::text("abc"))],
            span: Default::default(),
        };
        assert_eq!(fold(&len, &lookup).unwrap(), Some(Value::Int(3)));
        let req = Expr::Call {
            func: "request".into(),
            args: vec![],
            span: Default::default(),
        };
        assert_eq!(fold(&req, &lookup).unwrap(), None);
    }
}
