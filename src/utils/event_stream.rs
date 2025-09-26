use bytes::BytesMut;
use serde_json::Value;
use std::io;
use tokio_util::codec::Decoder;

#[derive(Debug, Clone, Default)]
pub struct EventStream {
    pub event: Option<String>,
    pub id: Option<String>,
    pub data: Value,
    pub retry: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct EventStreamData {
    current: EventStream,
    has_any: bool,
    buffer: String,
}

impl EventStreamData {
    pub fn new() -> Self {
        Self {
            current: EventStream {
                event: None,
                id: None,
                data: Value::Null,
                retry: None,
            },
            has_any: false,
            buffer: String::new(),
        }
    }

    fn process_line(&mut self, line: &str) -> Option<EventStream> {
        if line.is_empty() {
            if self.has_any {
                // 尝试将累计的 data 文本解析为 JSON
                let payload = self.buffer.trim();
                let data_val = if payload.is_empty() || payload == "[DONE]" {
                    Value::Null
                } else if let Some(pos) = payload.find('{') {
                    serde_json::from_str::<Value>(&payload[pos..]).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };
                self.current.data = data_val;
                let ev = std::mem::take(&mut self.current);
                self.has_any = false;
                self.buffer.clear();
                return Some(ev);
            } else {
                return None;
            }
        }
        if line.starts_with(':') {
            return None;
        }

        let (field, value) = match line.find(':') {
            Some(idx) => {
                let f = &line[..idx];
                let mut v = &line[idx + 1..];
                if v.starts_with(' ') {
                    v = &v[1..];
                }
                (f, v)
            }
            None => (line, ""),
        };

        match field {
            "data" => {
                if !self.buffer.is_empty() {
                    self.buffer.push('\n');
                }
                self.buffer.push_str(value);
                self.has_any = true;
            }
            "event" => {
                self.current.event = Some(value.to_string());
                self.has_any = true;
            }
            "id" => {
                self.current.id = Some(value.to_string());
                self.has_any = true;
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.current.retry = Some(ms);
                }
                self.has_any = true;
            }
            _ => {}
        }
        None
    }

    fn take_one_line(src: &mut BytesMut) -> io::Result<Option<String>> {
        if let Some(pos) = src.iter().position(|b| *b == b'\n') {
            let mut line_bytes = src.split_to(pos + 1);
            if line_bytes.ends_with(b"\n") {
                line_bytes.truncate(line_bytes.len() - 1);
            }
            if line_bytes.ends_with(b"\r") {
                line_bytes.truncate(line_bytes.len() - 1);
            }
            let line = String::from_utf8(line_bytes.to_vec())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(line))
        } else {
            Ok(None)
        }
    }
}

impl Decoder for EventStreamData {
    type Item = EventStream;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        loop {
            match Self::take_one_line(src)? {
                Some(line) => {
                    if let Some(ev) = self.process_line(&line) {
                        return Ok(Some(ev));
                    }
                }
                None => return Ok(None),
            }
        }
    }

    fn decode_eof(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        if !src.is_empty() {
            let mut last = String::from_utf8(src.split_to(src.len()).to_vec())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if last.ends_with('\n') {
                last.pop();
            }
            if last.ends_with('\r') {
                last.pop();
            }
            if let Some(ev) = self.process_line(&last) {
                return Ok(Some(ev));
            }
        }
        if self.has_any {
            let payload = self.buffer.trim();
            let data_val = if payload.is_empty() || payload == "[DONE]" {
                Value::Null
            } else if let Some(pos) = payload.find('{') {
                serde_json::from_str::<Value>(&payload[pos..]).unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            self.current.data = data_val;
            let ev = std::mem::take(&mut self.current);
            self.has_any = false;
            self.buffer.clear();
            return Ok(Some(ev));
        }
        Ok(None)
    }
}
