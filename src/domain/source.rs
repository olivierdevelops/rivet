//! Source bundles and source positions.

use super::value::Value;

// vhco:domain SourceSpan { file: string; start_line: int; start_col: int; end_line: int; end_col: int }
/// 1-indexed, source-absolute; `end_col` is exclusive (Capy AST JSON convention).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl SourceSpan {
    pub fn new(
        file: &str,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> SourceSpan {
        SourceSpan {
            file: file.to_string(),
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Inverse of [`SourceSpan::to_value`] (remote error envelopes).
    pub fn from_value(v: &Value) -> Option<SourceSpan> {
        let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0).max(0) as u32;
        Some(SourceSpan {
            file: v.get("file")?.as_str()?.to_string(),
            start_line: n("line"),
            start_col: n("column"),
            end_line: n("end_line"),
            end_col: n("end_column"),
        })
    }

    pub fn to_value(&self) -> Value {
        Value::object([
            ("file", Value::text(&self.file)),
            ("line", Value::Int(self.start_line as i64)),
            ("column", Value::Int(self.start_col as i64)),
            ("end_line", Value::Int(self.end_line as i64)),
            ("end_column", Value::Int(self.end_col as i64)),
        ])
    }

    /// Short `file:line` form used in manifests.
    pub fn short(&self) -> String {
        format!("{}:{}", self.file, self.start_line)
    }

    /// Slice the exact source text covered by this span.
    pub fn slice<'a>(&self, text: &'a str) -> &'a str {
        let start = offset_of(text, self.start_line, self.start_col);
        let end = offset_of(text, self.end_line, self.end_col);
        match (start, end) {
            (Some(s), Some(e)) if e >= s => &text[s..e],
            _ => "",
        }
    }
}

/// Byte offset of a 1-indexed (line, column) position; column counts bytes.
pub fn offset_of(text: &str, line: u32, col: u32) -> Option<usize> {
    let mut offset = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i as u32 + 1 == line {
            let c = (col.saturating_sub(1)) as usize;
            return if c <= l.len() {
                Some(offset + c)
            } else {
                Some(offset + l.len())
            };
        }
        offset += l.len();
    }
    if line as usize == text.split_inclusive('\n').count() + 1 && col <= 1 {
        return Some(text.len());
    }
    None
}

// vhco:domain SourceFile { path: string; text: string }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

// vhco:domain SourceBundle { entry: string; root: string; files: SourceFile[] }
/// The in-memory source handed to the compiler. Loading the bytes is host
/// bootstrap I/O; compilation itself never reads files.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SourceBundle {
    pub entry: String,
    pub root: String,
    pub files: Vec<SourceFile>,
}

impl SourceBundle {
    pub fn single(path: &str, text: &str) -> SourceBundle {
        SourceBundle {
            entry: path.to_string(),
            root: ".".to_string(),
            files: vec![SourceFile {
                path: path.to_string(),
                text: text.to_string(),
            }],
        }
    }

    pub fn text_of(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_single_and_multi_line_spans() {
        let text = "abc\n  def ghi\nxyz";
        assert_eq!(SourceSpan::new("f", 2, 3, 2, 6).slice(text), "def");
        assert_eq!(SourceSpan::new("f", 1, 2, 2, 6).slice(text), "bc\n  def");
    }
}
