//! Value ⇄ bytes codecs (satisfies Codec) plus the incremental decoders the
//! transport adapters share: stream items (sse / jsonl / lines / bytes) and
//! socket frames (newline / length32 / delimiter / raw).
//!
//! ```text
//!  chunks ──▶ StreamDecoder::push ──▶ pop() ──▶ Value items   (HTTP bodies, child stdout)
//!  bytes  ──▶ take_frame(buf, framing, max) ──▶ one frame      (TCP / Unix sockets)
//! ```

use crate::domain::transports::{Codec, CodecInput, CodecKind, Framing, StreamMode};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};

// vhco:infra codec satisfies Codec
pub struct StdCodec;

fn parse_err(code: &str, msg: impl Into<String>) -> RivetError {
    RivetError::new(ErrorKind::Parse, code, msg)
}

impl Codec for StdCodec {
    fn decode(&self, input: &CodecInput) -> RivetResult<Value> {
        let bytes = input.bytes.as_deref().unwrap_or(&[]);
        match input.kind {
            CodecKind::Json => decode_json(bytes),
            CodecKind::Text | CodecKind::Xml => String::from_utf8(bytes.to_vec())
                .map(Value::Text)
                .map_err(|e| parse_err("parse.utf8", format!("body is not valid UTF-8: {e}"))),
            CodecKind::Bytes => Ok(Value::Bytes(bytes.to_vec())),
            CodecKind::Form => Ok(Value::Object(
                url::form_urlencoded::parse(bytes)
                    .map(|(k, v)| (k.into_owned(), Value::Text(v.into_owned())))
                    .collect(),
            )),
        }
    }

    fn encode(&self, input: &CodecInput) -> RivetResult<Vec<u8>> {
        let v = input.value.clone().unwrap_or(Value::Null);
        Ok(match input.kind {
            CodecKind::Json => v.to_json().to_string().into_bytes(),
            CodecKind::Text | CodecKind::Xml => match v {
                Value::Text(s) => s.into_bytes(),
                other => other.to_display().into_bytes(),
            },
            CodecKind::Bytes => match v {
                Value::Bytes(b) => b,
                Value::Text(s) => s.into_bytes(),
                other => {
                    return Err(RivetError::validation(
                        "validation.codec",
                        format!("`bytes` needs bytes or text, got {}", other.type_name()),
                    ));
                }
            },
            CodecKind::Form => match v {
                Value::Object(pairs) => {
                    let mut s = url::form_urlencoded::Serializer::new(String::new());
                    for (k, val) in pairs {
                        s.append_pair(&k, &val.to_display());
                    }
                    s.finish().into_bytes()
                }
                other => {
                    return Err(RivetError::validation(
                        "validation.codec",
                        format!("`form` needs an object, got {}", other.type_name()),
                    ));
                }
            },
        })
    }
}

pub fn decode_json(bytes: &[u8]) -> RivetResult<Value> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Null);
    }
    serde_json::from_slice::<serde_json::Value>(bytes)
        .map(|j| Value::from_json(&j))
        .map_err(|e| {
            parse_err(
                "parse.json",
                format!("invalid JSON at line {} column {}", e.line(), e.column()),
            )
        })
}

/// Incremental splitter for streamed bodies and child stdout.
pub struct StreamDecoder {
    mode: StreamMode,
    decode_json: bool,
    max_item: usize,
    buf: Vec<u8>,
    // SSE event under construction
    event: Option<String>,
    id: Option<String>,
    retry: Option<i64>,
    data: Vec<String>,
    has_data: bool,
}

impl StreamDecoder {
    pub fn new(mode: StreamMode, decode_json: bool, max_item: usize) -> StreamDecoder {
        StreamDecoder {
            mode,
            decode_json,
            max_item,
            buf: Vec::new(),
            event: None,
            id: None,
            retry: None,
            data: Vec::new(),
            has_data: false,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    fn take_line(&mut self) -> RivetResult<Option<String>> {
        match self.buf.iter().position(|b| *b == b'\n') {
            Some(i) => {
                let mut line: Vec<u8> = self.buf.drain(..=i).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                String::from_utf8(line)
                    .map(Some)
                    .map_err(|_| parse_err("parse.utf8", "stream line is not valid UTF-8"))
            }
            None if self.buf.len() > self.max_item => Err(RivetError::new(
                ErrorKind::Limit,
                "limit.stream_item",
                format!("a stream line exceeded {} bytes", self.max_item),
            )),
            None => Ok(None),
        }
    }

    /// Next complete item, if the buffer holds one.
    pub fn pop(&mut self) -> RivetResult<Option<Value>> {
        match self.mode {
            StreamMode::Bytes => {
                if self.buf.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(Value::Bytes(std::mem::take(&mut self.buf))))
                }
            }
            StreamMode::Lines => Ok(self.take_line()?.map(Value::Text)),
            StreamMode::Jsonl => loop {
                match self.take_line()? {
                    None => return Ok(None),
                    Some(l) if l.trim().is_empty() => continue,
                    Some(l) => return decode_json(l.as_bytes()).map(Some),
                }
            },
            StreamMode::Sse => loop {
                let Some(line) = self.take_line()? else {
                    return Ok(None);
                };
                if line.is_empty() {
                    if let Some(ev) = self.dispatch() {
                        return Ok(Some(ev));
                    }
                    continue;
                }
                self.sse_field(&line);
            },
        }
    }

    fn sse_field(&mut self, line: &str) {
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => {
                self.data.push(value.to_string());
                self.has_data = true;
            }
            "id" if !value.contains('\0') => self.id = Some(value.to_string()),
            "retry" => self.retry = value.parse().ok(),
            _ => {}
        }
    }

    fn dispatch(&mut self) -> Option<Value> {
        if !self.has_data {
            self.event = None;
            return None;
        }
        let text = std::mem::take(&mut self.data).join("\n");
        self.has_data = false;
        let data = if self.decode_json {
            decode_json(text.as_bytes()).unwrap_or(Value::Text(text))
        } else {
            Value::Text(text)
        };
        let mut ev = Value::object([
            (
                "event",
                Value::text(self.event.take().unwrap_or_else(|| "message".into())),
            ),
            (
                "id",
                self.id.clone().map(Value::Text).unwrap_or(Value::Null),
            ),
            ("data", data),
        ]);
        if let Some(r) = self.retry {
            ev.set("retry", Value::Int(r));
        }
        Some(ev)
    }

    /// End of input: the final unterminated line / pending event, if any.
    pub fn finish(&mut self) -> RivetResult<Option<Value>> {
        match self.mode {
            StreamMode::Bytes => self.pop(),
            StreamMode::Sse => {
                if !self.buf.is_empty() {
                    self.buf.push(b'\n');
                    if let Some(line) = self.take_line()?
                        && !line.is_empty()
                    {
                        self.sse_field(&line);
                    }
                }
                Ok(self.dispatch())
            }
            StreamMode::Lines | StreamMode::Jsonl => {
                if self.buf.is_empty() {
                    return Ok(None);
                }
                self.buf.push(b'\n');
                self.pop()
            }
        }
    }
}

fn limit_frame(max: u64) -> RivetError {
    RivetError::new(
        ErrorKind::Limit,
        "limit.frame",
        format!("a frame exceeded max_frame ({max} bytes)"),
    )
}

/// Wrap one outgoing payload in its framing.
pub fn encode_frame(framing: &Framing, payload: Vec<u8>, max: u64) -> RivetResult<Vec<u8>> {
    if payload.len() as u64 > max {
        return Err(limit_frame(max));
    }
    Ok(match framing {
        Framing::Raw => payload,
        Framing::Newline => {
            if payload.contains(&b'\n') {
                return Err(RivetError::validation(
                    "validation.frame_delimiter",
                    "a newline-framed message cannot contain a newline",
                ));
            }
            let mut p = payload;
            p.push(b'\n');
            p
        }
        Framing::Delimiter(d) => {
            if !d.is_empty() && payload.windows(d.len()).any(|w| w == d.as_slice()) {
                return Err(RivetError::validation(
                    "validation.frame_delimiter",
                    "the payload contains the frame delimiter",
                ));
            }
            let mut p = payload;
            p.extend_from_slice(d);
            p
        }
        Framing::Length32 { big_endian } => {
            let n = payload.len() as u32;
            let mut p = if *big_endian {
                n.to_be_bytes().to_vec()
            } else {
                n.to_le_bytes().to_vec()
            };
            p.extend(payload);
            p
        }
    })
}

/// Take one complete incoming frame from `buf`, if present. Oversized
/// frames are rejected before their payload is buffered.
pub fn take_frame(buf: &mut Vec<u8>, framing: &Framing, max: u64) -> RivetResult<Option<Vec<u8>>> {
    match framing {
        Framing::Raw => {
            if buf.is_empty() {
                Ok(None)
            } else {
                let n = buf.len().min(max.max(1) as usize);
                Ok(Some(buf.drain(..n).collect()))
            }
        }
        Framing::Newline | Framing::Delimiter(_) => {
            let delim: &[u8] = match framing {
                Framing::Delimiter(d) => d,
                _ => b"\n",
            };
            if delim.is_empty() {
                return Err(RivetError::validation(
                    "validation.frame_delimiter",
                    "the delimiter is empty",
                ));
            }
            match buf.windows(delim.len()).position(|w| w == delim) {
                Some(i) => {
                    if i as u64 > max {
                        return Err(limit_frame(max));
                    }
                    let mut frame: Vec<u8> = buf.drain(..i + delim.len()).collect();
                    frame.truncate(i);
                    if matches!(framing, Framing::Newline) && frame.last() == Some(&b'\r') {
                        frame.pop();
                    }
                    Ok(Some(frame))
                }
                None if buf.len() as u64 > max => Err(limit_frame(max)),
                None => Ok(None),
            }
        }
        Framing::Length32 { big_endian } => {
            if buf.len() < 4 {
                return Ok(None);
            }
            let head = [buf[0], buf[1], buf[2], buf[3]];
            let n = if *big_endian {
                u32::from_be_bytes(head)
            } else {
                u32::from_le_bytes(head)
            } as u64;
            if n > max {
                return Err(limit_frame(max));
            }
            if (buf.len() as u64) < 4 + n {
                return Ok(None);
            }
            let frame: Vec<u8> = buf.drain(..4 + n as usize).skip(4).collect();
            Ok(Some(frame))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // vhco:test transports.exchange_http -- SSE events carry event/id and JSON-decoded data; lines deliver the final unterminated line
    #[test]
    fn sse_and_lines() {
        let mut d = StreamDecoder::new(StreamMode::Sse, true, 1 << 20);
        d.push(b": comment\nevent: delta\nid: 7\ndata: {\"delta\":\"he\"}\n\ndata: plain\n");
        let ev = d.pop().unwrap().unwrap();
        assert_eq!(ev.get("event"), Some(&Value::text("delta")));
        assert_eq!(ev.get("id"), Some(&Value::text("7")));
        assert_eq!(
            ev.get("data").unwrap().get("delta"),
            Some(&Value::text("he"))
        );
        assert_eq!(d.pop().unwrap(), None);
        d.push(b"\n");
        assert_eq!(
            d.pop().unwrap().unwrap().get("data"),
            Some(&Value::text("plain"))
        );
        let mut l = StreamDecoder::new(StreamMode::Lines, false, 1 << 20);
        l.push(b"a\r\nb");
        assert_eq!(l.pop().unwrap(), Some(Value::text("a")));
        assert_eq!(l.pop().unwrap(), None);
        assert_eq!(l.finish().unwrap(), Some(Value::text("b")));
        let mut j = StreamDecoder::new(StreamMode::Jsonl, true, 1 << 20);
        j.push(b"{\"x\":1}\nnot json\n");
        assert!(j.pop().unwrap().is_some());
        assert_eq!(j.pop().unwrap_err().code, "parse.json");
    }

    // vhco:test transports.exchange_socket -- length32 frames reassemble partial reads and reject oversized lengths before buffering
    #[test]
    fn frames() {
        let f = Framing::Length32 { big_endian: true };
        let wire = encode_frame(&f, vec![0, 1, 2], 16).unwrap();
        assert_eq!(wire, vec![0, 0, 0, 3, 0, 1, 2]);
        let mut buf = wire[..5].to_vec();
        assert_eq!(take_frame(&mut buf, &f, 16).unwrap(), None);
        buf.extend_from_slice(&wire[5..]);
        assert_eq!(take_frame(&mut buf, &f, 16).unwrap(), Some(vec![0, 1, 2]));
        let mut big = vec![0xff, 0xff, 0xff, 0xff];
        assert_eq!(
            take_frame(&mut big, &f, 16).unwrap_err().code,
            "limit.frame"
        );
        let mut nl = b"STATUS ok\r\nrest".to_vec();
        assert_eq!(
            take_frame(&mut nl, &Framing::Newline, 64).unwrap(),
            Some(b"STATUS ok".to_vec())
        );
        assert!(encode_frame(&Framing::Newline, b"a\nb".to_vec(), 64).is_err());
        let form = StdCodec
            .encode(&CodecInput {
                kind: CodecKind::Form,
                bytes: None,
                value: Some(Value::object([("q", Value::text("a & b"))])),
            })
            .unwrap();
        assert_eq!(form, b"q=a+%26+b");
    }
}
