use serde::Deserialize;

use crate::models::chat::{Evidence, StreamChunk};

#[derive(Debug, Deserialize)]
struct SsePayload {
    #[serde(default)]
    delta: Option<Delta>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    evidence: Option<Evidence>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

/// Incremental SSE decoder. Socket reads are not event-aligned.
pub(crate) struct SseParser {
    pending: String,
    data: String,
}

impl SseParser {
    pub(crate) fn new() -> Self {
        Self {
            pending: String::new(),
            data: String::new(),
        }
    }

    pub(crate) fn push(&mut self, chunk: &str) -> Result<Vec<StreamChunk>, String> {
        self.pending.push_str(chunk);
        let mut out = Vec::new();
        while let Some(idx) = self.pending.find('\n') {
            let mut line = self.pending.drain(..=idx).collect::<String>();
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }
            if line.is_empty() {
                if let Some(chunk) = self.finish_event()? {
                    out.push(chunk);
                }
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.split_once(':') {
                Some((field, rest)) => (field, rest.strip_prefix(' ').unwrap_or(rest)),
                None => (line.as_str(), ""),
            };
            match field {
                "data" => {
                    if !self.data.is_empty() {
                        self.data.push('\n');
                    }
                    self.data.push_str(value);
                }
                "error" => return Err(value.to_string()),
                _ => {}
            }
        }
        Ok(out)
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<StreamChunk>, String> {
        if !self.pending.is_empty() || !self.data.is_empty() {
            self.pending.push('\n');
            return self.push("");
        }
        Ok(Vec::new())
    }

    fn finish_event(&mut self) -> Result<Option<StreamChunk>, String> {
        if self.data.is_empty() {
            return Ok(None);
        }
        let raw = std::mem::take(&mut self.data);
        let payload: SsePayload = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        if payload.done {
            return Ok(Some(StreamChunk {
                content: String::new(),
                done: true,
                evidence: payload.evidence,
            }));
        }
        let content = payload.delta.and_then(|d| d.content).unwrap_or_default();
        if content.is_empty() {
            return Ok(None);
        }
        Ok(Some(StreamChunk {
            content,
            done: false,
            evidence: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::SseParser;

    #[test]
    fn splits_events_across_chunks() {
        let mut parser = SseParser::new();
        let first = parser.push("data: {\"delta\":{\"content\":\"Hel").unwrap();
        assert!(first.is_empty());
        let second = parser
            .push("lo\"},\"done\":false}\n\ndata: {\"done\":true}\n\n")
            .unwrap();
        assert_eq!(second.len(), 2);
        assert_eq!(second[0].content, "Hello");
        assert!(!second[0].done);
        assert!(second[1].done);
        assert!(second[1].content.is_empty());
    }
}
