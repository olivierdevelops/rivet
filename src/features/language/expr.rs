//! Rivet expression and argument parser (RES-2026-0001 finding 3).
//!
//! Capy decides where an expression starts and ends; this module parses the
//! exact source text into `Expr`. The grammar mirrors Capy's value grammar:
//! literals, strings with `${dotted.path}`, lists, objects, dotted paths,
//! prefix calls `(f a b)`, grouping `(a + b)`, `not`, unary `-`, and infix
//! `* / %` > `+ -` > comparisons > `and` > `or` (all left-associative).

use crate::domain::ir::{Arg, BinOp, Expr, TemplatePart};
use crate::domain::source::SourceSpan;
use crate::domain::{RivetError, RivetResult, Value};

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(Vec<TemplatePart>),
    Punct(String),
    Open(char),
    Close(char),
    Comma,
    Colon,
    Dot,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

/// Maps byte offsets inside a capture's text back to source positions.
#[derive(Clone, Debug)]
pub struct SpanMap {
    base: SourceSpan,
    text: String,
}

impl SpanMap {
    pub fn new(base: SourceSpan, text: &str) -> SpanMap {
        SpanMap {
            base,
            text: text.to_string(),
        }
    }

    fn pos(&self, offset: usize) -> (u32, u32) {
        let before = &self.text[..offset.min(self.text.len())];
        let newlines = before.matches('\n').count() as u32;
        if newlines == 0 {
            (
                self.base.start_line,
                self.base.start_col + before.len() as u32,
            )
        } else {
            let last = before.rsplit('\n').next().unwrap_or("");
            (self.base.start_line + newlines, last.len() as u32 + 1)
        }
    }

    pub fn span(&self, start: usize, end: usize) -> SourceSpan {
        let (sl, sc) = self.pos(start);
        let (el, ec) = self.pos(end);
        SourceSpan::new(&self.base.file, sl, sc, el, ec)
    }
}

fn err(map: &SpanMap, code: &str, msg: impl Into<String>, start: usize, end: usize) -> RivetError {
    RivetError::syntax(code, msg, Some(map.span(start, end.max(start + 1))))
}

fn lex(text: &str, map: &SpanMap) -> RivetResult<Vec<Token>> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        if c.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
            let mut is_float = false;
            if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
                is_float = true;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < bytes.len() && (bytes[i] as char).is_alphabetic() {
                let mut j = i;
                while j < bytes.len() && (bytes[j] as char).is_alphanumeric() {
                    j += 1;
                }
                return Err(err(
                    map,
                    "syntax.duration_unquoted",
                    format!(
                        "`{}` is not a value; durations are quoted strings",
                        &text[start..j]
                    ),
                    start,
                    j,
                )
                .with_hint(format!("write \"{}\"", &text[start..j])));
            }
            let lit = text[start..i].replace('_', "");
            let tok = if is_float {
                Tok::Float(
                    lit.parse()
                        .map_err(|_| err(map, "syntax.number", "invalid number", start, i))?,
                )
            } else {
                Tok::Int(lit.parse().map_err(|_| {
                    err(
                        map,
                        "syntax.number",
                        "integer out of 64-bit range",
                        start,
                        i,
                    )
                })?)
            };
            out.push(Token { tok, start, end: i });
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            let (parts, end) = lex_string(text, i, c, map)?;
            out.push(Token {
                tok: Tok::Str(parts),
                start,
                end,
            });
            i = end;
            continue;
        }
        if c.is_alphabetic() || c == '_' || !c.is_ascii() {
            let rest = &text[i..];
            let len: usize = rest
                .char_indices()
                .find(|(_, ch)| {
                    // identifier characters: letters, digits, `_` and any non-ASCII, non-space rune
                    (ch.is_whitespace() || ch.is_ascii()) && *ch != '_' && !ch.is_alphanumeric()
                })
                .map(|(k, _)| k)
                .unwrap_or(rest.len());
            out.push(Token {
                tok: Tok::Ident(rest[..len].to_string()),
                start,
                end: i + len,
            });
            i += len;
            continue;
        }
        match c {
            '(' | '[' | '{' => {
                out.push(Token {
                    tok: Tok::Open(c),
                    start,
                    end: i + 1,
                });
                i += 1;
            }
            ')' | ']' | '}' => {
                out.push(Token {
                    tok: Tok::Close(c),
                    start,
                    end: i + 1,
                });
                i += 1;
            }
            ',' => {
                out.push(Token {
                    tok: Tok::Comma,
                    start,
                    end: i + 1,
                });
                i += 1;
            }
            _ => {
                // Greedy punctuation run, as in Capy's lexer.
                let mut j = i;
                while j < bytes.len() && "=<>!+-*/%&|^~?:.;@$\\".contains(bytes[j] as char) {
                    j += 1;
                }
                if j == i {
                    return Err(err(
                        map,
                        "syntax.character",
                        format!("unexpected character `{c}`"),
                        i,
                        i + 1,
                    ));
                }
                let p = &text[i..j];
                let tok = match p {
                    ":" => Tok::Colon,
                    "." => Tok::Dot,
                    _ => Tok::Punct(p.to_string()),
                };
                out.push(Token { tok, start, end: j });
                i = j;
            }
        }
    }
    Ok(out)
}

fn lex_string(
    text: &str,
    start: usize,
    quote: char,
    map: &SpanMap,
) -> RivetResult<(Vec<TemplatePart>, usize)> {
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut chars = text[start + 1..].char_indices().peekable();
    while let Some((k, ch)) = chars.next() {
        let abs = start + 1 + k;
        if ch == quote {
            if !lit.is_empty() || parts.is_empty() {
                parts.push(TemplatePart::Lit(lit));
            }
            return Ok((parts, abs + 1));
        }
        if ch == '\n' && quote != '`' {
            return Err(err(
                map,
                "syntax.string",
                "unterminated string; use backticks for multi-line text",
                start,
                abs,
            ));
        }
        if ch == '\\' {
            let Some((_, esc)) = chars.next() else { break };
            match esc {
                'n' => lit.push('\n'),
                't' => lit.push('\t'),
                'r' => lit.push('\r'),
                '"' => lit.push('"'),
                '\'' => lit.push('\''),
                '`' => lit.push('`'),
                '\\' => lit.push('\\'),
                '$' => lit.push('$'),
                'x' => {
                    let hex: String = (0..2)
                        .filter_map(|_| chars.next().map(|(_, c)| c))
                        .collect();
                    let v = u8::from_str_radix(&hex, 16).map_err(|_| {
                        err(
                            map,
                            "syntax.escape",
                            "\\x needs two hex digits",
                            abs,
                            abs + 4,
                        )
                    })?;
                    lit.push(v as char);
                }
                'u' => {
                    let hex: String = (0..4)
                        .filter_map(|_| chars.next().map(|(_, c)| c))
                        .collect();
                    let v = u32::from_str_radix(&hex, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or_else(|| {
                            err(
                                map,
                                "syntax.escape",
                                "\\u needs four hex digits",
                                abs,
                                abs + 6,
                            )
                        })?;
                    lit.push(v);
                }
                other => {
                    return Err(err(
                        map,
                        "syntax.escape",
                        format!(
                            "unknown escape `\\{other}`; allowed: \\n \\t \\\" \\\\ \\xNN \\uNNNN"
                        ),
                        abs,
                        abs + 2,
                    )
                    .with_hint(if other == '0' {
                        "write \\x00".to_string()
                    } else {
                        "remove the backslash".to_string()
                    }));
                }
            }
            continue;
        }
        if ch == '$' && chars.peek().map(|(_, c)| *c) == Some('{') {
            chars.next();
            let mut inner = String::new();
            let mut closed = false;
            for (_, c) in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                inner.push(c);
            }
            if !closed {
                return Err(err(
                    map,
                    "syntax.interpolation",
                    "unterminated ${…}",
                    abs,
                    abs + 2,
                ));
            }
            let path: Vec<String> = inner.trim().split('.').map(str::to_string).collect();
            let valid = !path.is_empty()
                && path.iter().all(|seg| {
                    !seg.is_empty()
                        && seg
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_alphabetic() || c == '_')
                        && seg.chars().all(|c| c.is_alphanumeric() || c == '_')
                });
            if !valid {
                return Err(err(
                    map,
                    "syntax.interpolation",
                    format!(
                        "`${{{inner}}}` is not a dotted path; compute it in an assignment first"
                    ),
                    abs,
                    abs + inner.len() + 3,
                ));
            }
            if !lit.is_empty() {
                parts.push(TemplatePart::Lit(std::mem::take(&mut lit)));
            }
            parts.push(TemplatePart::Path(path));
            continue;
        }
        lit.push(ch);
    }
    Err(err(
        map,
        "syntax.string",
        "unterminated string",
        start,
        text.len(),
    ))
}

struct P<'a> {
    toks: Vec<Token>,
    pos: usize,
    map: &'a SpanMap,
    len: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n).map(|t| &t.tok)
    }

    fn start(&self) -> usize {
        self.toks.get(self.pos).map(|t| t.start).unwrap_or(self.len)
    }

    fn prev_end(&self) -> usize {
        if self.pos == 0 {
            0
        } else {
            self.toks[self.pos - 1].end
        }
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn fail<T>(&self, msg: impl Into<String>) -> RivetResult<T> {
        let s = self.start();
        Err(err(self.map, "syntax.expression", msg, s, s + 1))
    }

    fn expect_close(&mut self, c: char) -> RivetResult<()> {
        match self.peek() {
            Some(Tok::Close(x)) if *x == c => {
                self.pos += 1;
                Ok(())
            }
            _ => self.fail(format!("expected `{c}`")),
        }
    }

    fn binop(&self) -> Option<(BinOp, u8)> {
        Some(match self.peek()? {
            Tok::Ident(w) if w == "or" => (BinOp::Or, 1),
            Tok::Ident(w) if w == "and" => (BinOp::And, 2),
            Tok::Punct(p) => match p.as_str() {
                "==" => (BinOp::Eq, 3),
                "!=" => (BinOp::Ne, 3),
                "<" => (BinOp::Lt, 3),
                "<=" => (BinOp::Le, 3),
                ">" => (BinOp::Gt, 3),
                ">=" => (BinOp::Ge, 3),
                "+" => (BinOp::Add, 4),
                "-" => (BinOp::Sub, 4),
                "*" => (BinOp::Mul, 5),
                "/" => (BinOp::Div, 5),
                "%" => (BinOp::Rem, 5),
                _ => return None,
            },
            _ => return None,
        })
    }

    fn expr(&mut self, min: u8) -> RivetResult<Expr> {
        let mut lhs = self.unary()?;
        while let Some((op, prec)) = self.binop() {
            if prec < min {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(prec + 1)?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> RivetResult<Expr> {
        match self.peek() {
            Some(Tok::Ident(w)) if w == "not" => {
                self.pos += 1;
                Ok(Expr::Not(Box::new(self.unary()?)))
            }
            Some(Tok::Punct(p)) if p == "-" => {
                self.pos += 1;
                match self.unary()? {
                    Expr::Lit(Value::Int(i)) => Ok(Expr::Lit(Value::Int(-i))),
                    Expr::Lit(Value::Float(f)) => Ok(Expr::Lit(Value::Float(-f))),
                    e => Ok(Expr::Neg(Box::new(e))),
                }
            }
            _ => self.primary(),
        }
    }

    fn path(&mut self) -> RivetResult<(Vec<String>, usize, usize)> {
        let start = self.start();
        let mut segs = Vec::new();
        match self.bump() {
            Some(Token {
                tok: Tok::Ident(w), ..
            }) => segs.push(w),
            _ => return self.fail("expected a name"),
        }
        while matches!(self.peek(), Some(Tok::Dot))
            && matches!(self.peek_at(1), Some(Tok::Ident(_)))
        {
            self.pos += 1;
            if let Some(Token {
                tok: Tok::Ident(w), ..
            }) = self.bump()
            {
                segs.push(w);
            }
        }
        Ok((segs, start, self.prev_end()))
    }

    fn primary(&mut self) -> RivetResult<Expr> {
        let start = self.start();
        match self.peek().cloned() {
            None => self.fail("expected a value"),
            Some(Tok::Int(i)) => {
                self.pos += 1;
                Ok(Expr::Lit(Value::Int(i)))
            }
            Some(Tok::Float(f)) => {
                self.pos += 1;
                Ok(Expr::Lit(Value::Float(f)))
            }
            Some(Tok::Str(parts)) => {
                self.pos += 1;
                Ok(match parts.as_slice() {
                    [TemplatePart::Lit(s)] => Expr::Lit(Value::Text(s.clone())),
                    _ => Expr::Template(parts),
                })
            }
            Some(Tok::Ident(w)) if w == "true" || w == "false" || w == "null" => {
                self.pos += 1;
                Ok(Expr::Lit(match w.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    _ => Value::Null,
                }))
            }
            Some(Tok::Ident(_)) => {
                let (segs, s, e) = self.path()?;
                Ok(Expr::Path(segs, self.map.span(s, e)))
            }
            Some(Tok::Open('[')) => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    if matches!(self.peek(), Some(Tok::Close(']'))) {
                        self.pos += 1;
                        break;
                    }
                    items.push(self.expr(0)?);
                    match self.peek() {
                        Some(Tok::Comma) => self.pos += 1,
                        Some(Tok::Close(']')) => {}
                        _ => return self.fail("expected `,` or `]` in list"),
                    }
                }
                Ok(Expr::List(items))
            }
            Some(Tok::Open('{')) => {
                self.pos += 1;
                let mut pairs = Vec::new();
                loop {
                    if matches!(self.peek(), Some(Tok::Close('}'))) {
                        self.pos += 1;
                        break;
                    }
                    let key = match self.bump().map(|t| t.tok) {
                        Some(Tok::Ident(k)) => k,
                        Some(Tok::Str(parts)) => match parts.as_slice() {
                            [TemplatePart::Lit(s)] => s.clone(),
                            _ => return self.fail("object keys cannot interpolate"),
                        },
                        _ => return self.fail("expected an object key"),
                    };
                    if !matches!(self.bump().map(|t| t.tok), Some(Tok::Colon)) {
                        return self.fail(format!("expected `:` after key `{key}`"));
                    }
                    let value = self.expr(0)?;
                    pairs.push((key, value));
                    match self.peek() {
                        Some(Tok::Comma) => self.pos += 1,
                        Some(Tok::Close('}')) => {}
                        _ => return self.fail("expected `,` or `}` in object"),
                    }
                }
                Ok(Expr::Object(pairs))
            }
            Some(Tok::Open('(')) => {
                self.pos += 1;
                // `(f a b)` is a prefix call; `(a + b)` groups (Capy grouping rule).
                if matches!(self.peek(), Some(Tok::Ident(w)) if w != "not" && w != "true" && w != "false" && w != "null")
                {
                    let save = self.pos;
                    let (segs, s, e) = self.path()?;
                    if self.binop().is_some() {
                        self.pos = save;
                        let inner = self.expr(0)?;
                        self.expect_close(')')?;
                        return Ok(inner);
                    }
                    let mut args = Vec::new();
                    while !matches!(self.peek(), Some(Tok::Close(')')) | None) {
                        args.push(self.unary()?);
                    }
                    self.expect_close(')')?;
                    return Ok(Expr::Call {
                        func: segs.join("."),
                        args,
                        span: self.map.span(s, e),
                    });
                }
                let inner = self.expr(0)?;
                self.expect_close(')')?;
                Ok(inner)
            }
            Some(_) => {
                let s = start;
                Err(err(
                    self.map,
                    "syntax.expression",
                    "unexpected token in expression",
                    s,
                    s + 1,
                ))
            }
        }
    }
}

/// Parse one complete expression; trailing tokens are an error.
pub fn parse_expr(text: &str, map: &SpanMap) -> RivetResult<Expr> {
    let toks = lex(text, map)?;
    let len = text.len();
    let mut p = P {
        toks,
        pos: 0,
        map,
        len,
    };
    let e = p.expr(0)?;
    if p.pos < p.toks.len() {
        return p.fail("unexpected trailing tokens after the expression");
    }
    Ok(e)
}

/// Parse an option/effect argument list: a sequence of words and values.
/// A single bare identifier is a `Word`; dotted paths, literals, lists,
/// objects and calls are expressions.
pub fn parse_args(text: &str, map: &SpanMap) -> RivetResult<Vec<Arg>> {
    let toks = lex(text, map)?;
    let len = text.len();
    let mut p = P {
        toks,
        pos: 0,
        map,
        len,
    };
    let mut out = Vec::new();
    while p.pos < p.toks.len() {
        let start = p.start();
        if let Some(Tok::Ident(w)) = p.peek().cloned() {
            let is_dotted = matches!(p.peek_at(1), Some(Tok::Dot))
                && matches!(p.peek_at(2), Some(Tok::Ident(_)));
            if !is_dotted && !matches!(w.as_str(), "true" | "false" | "null" | "not") {
                p.pos += 1;
                out.push(Arg::Word(w, map.span(start, p.prev_end())));
                continue;
            }
        }
        let e = p.unary()?;
        out.push(Arg::Expr(e, map.span(start, p.prev_end())));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(text: &str) -> SpanMap {
        SpanMap::new(SourceSpan::new("t", 1, 1, 1, 1), text)
    }

    fn e(text: &str) -> Expr {
        parse_expr(text, &m(text)).unwrap()
    }

    #[test]
    fn precedence_and_associativity() {
        let Expr::Binary {
            op: BinOp::Add,
            rhs,
            ..
        } = e("a + b * c")
        else {
            panic!()
        };
        assert!(matches!(*rhs, Expr::Binary { op: BinOp::Mul, .. }));
        let Expr::Binary {
            op: BinOp::Sub,
            lhs,
            ..
        } = e("a - b - c")
        else {
            panic!()
        };
        assert!(matches!(*lhs, Expr::Binary { op: BinOp::Sub, .. }));
    }

    #[test]
    fn prefix_call_vs_grouping() {
        assert!(
            matches!(e("(request \"users.get\" {id: id})"), Expr::Call { func, args, .. } if func == "request" && args.len() == 2)
        );
        assert!(matches!(e("(foo)"), Expr::Call { args, .. } if args.is_empty()));
        assert!(matches!(
            e("(a + b) * c"),
            Expr::Binary { op: BinOp::Mul, .. }
        ));
        assert!(
            matches!(e("(base64.decode \"AAEC\")"), Expr::Call { func, .. } if func == "base64.decode")
        );
    }

    #[test]
    fn templates_and_escapes() {
        assert_eq!(
            e("\"Hello, ${person}!\""),
            Expr::Template(vec![
                TemplatePart::Lit("Hello, ".into()),
                TemplatePart::Path(vec!["person".into()]),
                TemplatePart::Lit("!".into())
            ])
        );
        assert_eq!(e("\"\\x00\""), Expr::Lit(Value::Text("\0".into())));
        assert!(parse_expr("\"\\0\"", &m("\"\\0\"")).is_err());
        assert!(parse_expr("\"${a + b}\"", &m("\"${a + b}\"")).is_err());
    }

    #[test]
    fn objects_lists_and_multiline() {
        assert!(matches!(e("{ready: true, \"k\": [1, 2]}"), Expr::Object(p) if p.len() == 2));
        assert!(
            matches!(e("{\n  user: user.result,\n  orders: orders.result\n}"), Expr::Object(p) if p.len() == 2)
        );
    }

    #[test]
    fn unquoted_duration_is_a_clear_error() {
        let err = parse_expr("5s", &m("5s")).unwrap_err();
        assert_eq!(err.code, "syntax.duration_unquoted");
    }

    #[test]
    fn args_mix_words_and_values() {
        let text = "3 on status [429, 503] backoff exponential base \"100ms\" jitter true";
        let args = parse_args(text, &m(text)).unwrap();
        let words: Vec<_> = args.iter().filter_map(Arg::word).collect();
        assert_eq!(
            words,
            vec!["on", "status", "backoff", "exponential", "base", "jitter"]
        );
        assert_eq!(args.len(), 10);
        let text = "message.peer json {received: true}";
        let args = parse_args(text, &m(text)).unwrap();
        assert!(matches!(&args[0], Arg::Expr(Expr::Path(p, _), _) if p.len() == 2));
    }
}
