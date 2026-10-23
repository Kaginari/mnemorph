//! Server-sent events, read incrementally: bytes in, whole events out.

/// One server-sent event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` name; empty when the server sent none.
    pub event: String,
    /// The `data:` lines, joined by newlines.
    pub data: String,
}

/// An incremental reader: feed it chunks as they arrive, take the events they complete.
#[derive(Debug, Default)]
pub struct SseReader {
    pending: Vec<u8>,
    event: String,
    data: Vec<String>,
}

impl SseReader {
    /// Feeds a chunk; returns every event it completed. A chunk may end mid-line or mid-UTF-8
    /// character; the remainder waits for the next chunk.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.pending.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let mut line: Vec<u8> = self.pending.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line);
            if line.is_empty() {
                if !self.data.is_empty() || !self.event.is_empty() {
                    out.push(SseEvent {
                        event: std::mem::take(&mut self.event),
                        data: self.data.join("\n"),
                    });
                    self.data.clear();
                }
            } else if let Some(rest) = line.strip_prefix(':') {
                let _ = rest; // a comment: keep-alive
            } else {
                let (field, value) = line.split_once(':').unwrap_or((&line, ""));
                let value = value.strip_prefix(' ').unwrap_or(value);
                match field {
                    "event" => value.clone_into(&mut self.event),
                    "data" => self.data.push(value.to_owned()),
                    _ => {}
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_across_chunk_boundaries() {
        let stream = "event: message_start\ndata: {\"a\":1}\n\n: ping\n\nevent: content_block_delta\r\ndata: {\"t\":\"é\"}\r\n\r\n";
        let bytes = stream.as_bytes();
        for split in 0..bytes.len() {
            let mut r = SseReader::default();
            let mut events = r.feed(&bytes[..split]);
            events.extend(r.feed(&bytes[split..]));
            assert_eq!(
                events,
                [
                    SseEvent {
                        event: "message_start".into(),
                        data: "{\"a\":1}".into()
                    },
                    SseEvent {
                        event: "content_block_delta".into(),
                        data: "{\"t\":\"é\"}".into()
                    },
                ],
                "split at {split}"
            );
        }
    }

    #[test]
    fn multi_line_data_and_unknown_fields() {
        let mut r = SseReader::default();
        let e = r.feed(b"id: 7\ndata: one\ndata: two\nretry: 10\n\n");
        assert_eq!(
            e,
            [SseEvent {
                event: String::new(),
                data: "one\ntwo".into()
            }]
        );
        assert_eq!(r.feed(b"data"), []);
        assert_eq!(
            r.feed(b"\n\n"),
            [SseEvent {
                event: String::new(),
                data: String::new()
            }]
        );
    }
}
