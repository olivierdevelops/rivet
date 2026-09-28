//! Syntax-highlight tokens and their renderers (PROP-2026-0002 R17, UC-08).
//!
//! `language.highlight_source` produces the tokens from real parser spans;
//! these pure functions turn them back into coloured text:
//!
//! ```text
//!  source + [HighlightToken{line, col, len, class, text}]
//!     │
//!     ├─ ansi ─▶ \x1b[1;35mglobal\x1b[0m \x1b[1;33mapi\x1b[0m = \x1b[32m"https://…"\x1b[0m
//!     ├─ html ─▶ <pre class="rv-source"><code><span class="rv-keyword">global</span> …</code></pre>
//!     └─ json ─▶ {"line":1,"col":1,"len":6,"class":"keyword","text":"global"}   (one per line)
//! ```
//!
//! Every token lies on one line; `col` and `len` count characters (Unicode
//! scalar values), 1-based, so an editor can place them directly.

use crate::domain::RivetError;

/// The token classes, in the order PROP-2026-0002 R17 lists them.
pub const TOKEN_CLASSES: &[&str] = &[
    "keyword",
    "option",
    "type",
    "effect",
    "string",
    "interpolation",
    "number",
    "comment",
    "operation_id",
    "global",
    "variable",
    "operator",
];

// vhco:domain HighlightToken { line: int; col: int; len: int; class: string; text: string }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightToken {
    /// 1-based line.
    pub line: u32,
    /// 1-based column, in characters.
    pub col: u32,
    /// Length in characters (never crosses a line end).
    pub len: u32,
    /// One of [`TOKEN_CLASSES`].
    pub class: String,
    /// The exact source text of the token.
    pub text: String,
}

impl HighlightToken {
    /// One JSON object, keys in the documented order `line, col, len, class, text`.
    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"line\":{},\"col\":{},\"len\":{},\"class\":{},\"text\":{}}}",
            self.line,
            self.col,
            self.len,
            serde_json::Value::String(self.class.clone()),
            serde_json::Value::String(self.text.clone())
        )
    }
}

// vhco:domain HighlightFormat { ansi | html | json }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightFormat {
    /// Terminal colours (SGR escape codes); the source text is unchanged otherwise.
    Ansi,
    /// `<pre class="rv-source"><code>` with `<span class="rv-CLASS">`, HTML-escaped.
    Html,
    /// JSON lines: one `{line, col, len, class, text}` object per token.
    Json,
}

impl HighlightFormat {
    pub fn parse(name: &str) -> Result<HighlightFormat, RivetError> {
        match name {
            "ansi" => Ok(HighlightFormat::Ansi),
            "html" => Ok(HighlightFormat::Html),
            "json" => Ok(HighlightFormat::Json),
            other => Err(RivetError::validation(
                "validation.usage",
                format!("unknown highlight format `{other}`; use ansi, html or json"),
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            HighlightFormat::Ansi => "ansi",
            HighlightFormat::Html => "html",
            HighlightFormat::Json => "json",
        }
    }
}

/// The SGR colour of a class (`None`: printed plain).
pub fn ansi_color(class: &str) -> Option<&'static str> {
    Some(match class {
        "keyword" => "1;35",
        "option" => "36",
        "type" => "33",
        "effect" => "1;34",
        "string" => "32",
        "interpolation" => "1;32",
        "number" => "91",
        "comment" => "90",
        "operation_id" => "1;94",
        "global" => "1;33",
        "operator" => "37",
        _ => return None,
    })
}

fn html_escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}

/// Render `source` with `tokens` in `format`. Text outside tokens (spaces,
/// brackets, commas, anything after a syntax error) is copied unchanged, so
/// stripping the colours gives back the source.
pub fn render(source: &str, tokens: &[HighlightToken], format: HighlightFormat) -> String {
    if format == HighlightFormat::Json {
        let mut out = String::new();
        for t in tokens {
            out.push_str(&t.to_json_line());
            out.push('\n');
        }
        return out;
    }
    let mut sorted: Vec<&HighlightToken> = tokens.iter().collect();
    sorted.sort_by_key(|t| (t.line, t.col));
    let mut next = sorted.into_iter().peekable();
    let mut out = String::new();
    if format == HighlightFormat::Html {
        out.push_str("<pre class=\"rv-source\"><code>");
    }
    let plain = |s: &str, out: &mut String| match format {
        HighlightFormat::Html => html_escape(s, out),
        _ => out.push_str(s),
    };
    for (i, line) in source.split_inclusive('\n').enumerate() {
        let line_no = i as u32 + 1;
        let chars: Vec<char> = line.chars().collect();
        let mut col = 0usize; // 0-based char index into `chars`
        while let Some(t) = next.peek() {
            if t.line != line_no {
                if t.line < line_no {
                    next.next(); // out of order or overlapping: skip
                    continue;
                }
                break;
            }
            let start = (t.col.max(1) - 1) as usize;
            let end = (start + t.len as usize).min(chars.len());
            if start < col || start >= chars.len() {
                next.next();
                continue;
            }
            let before: String = chars[col..start].iter().collect();
            plain(&before, &mut out);
            let text: String = chars[start..end].iter().collect();
            match format {
                HighlightFormat::Html => {
                    out.push_str("<span class=\"rv-");
                    out.push_str(&t.class);
                    out.push_str("\">");
                    html_escape(&text, &mut out);
                    out.push_str("</span>");
                }
                _ => match ansi_color(&t.class) {
                    Some(code) => {
                        out.push_str("\x1b[");
                        out.push_str(code);
                        out.push('m');
                        out.push_str(&text);
                        out.push_str("\x1b[0m");
                    }
                    None => out.push_str(&text),
                },
            }
            col = end;
            next.next();
        }
        let rest: String = chars[col.min(chars.len())..].iter().collect();
        plain(&rest, &mut out);
    }
    if format == HighlightFormat::Html {
        out.push_str("</code></pre>\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(line: u32, col: u32, class: &str, text: &str) -> HighlightToken {
        HighlightToken {
            line,
            col,
            len: text.chars().count() as u32,
            class: class.into(),
            text: text.into(),
        }
    }

    // vhco:test language.highlight_source -- the three renderers keep the source text: ANSI wraps tokens in SGR codes, HTML escapes and wraps spans, JSON prints one ordered object per token
    #[test]
    fn renderers_keep_the_source_text() {
        let src = "x = \"<a>\" # é\n";
        let toks = [
            tok(1, 1, "variable", "x"),
            tok(1, 3, "operator", "="),
            tok(1, 5, "string", "\"<a>\""),
            tok(1, 11, "comment", "# é"),
        ];
        assert_eq!(
            render(src, &toks, HighlightFormat::Ansi),
            "x \x1b[37m=\x1b[0m \x1b[32m\"<a>\"\x1b[0m \x1b[90m# é\x1b[0m\n"
        );
        assert_eq!(
            render(src, &toks, HighlightFormat::Html),
            "<pre class=\"rv-source\"><code><span class=\"rv-variable\">x</span> <span class=\"rv-operator\">=</span> <span class=\"rv-string\">&quot;&lt;a&gt;&quot;</span> <span class=\"rv-comment\"># é</span>\n</code></pre>\n"
        );
        assert_eq!(
            render(src, &toks[..1], HighlightFormat::Json),
            "{\"line\":1,\"col\":1,\"len\":1,\"class\":\"variable\",\"text\":\"x\"}\n"
        );
        assert!(HighlightFormat::parse("svg").is_err());
        assert_eq!(HighlightFormat::parse("html").unwrap().name(), "html");
    }
}
