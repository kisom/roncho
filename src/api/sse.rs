use std::pin::Pin;

use async_stream::stream;
use futures::{Stream, StreamExt};
use serde::Deserialize;

use crate::client::Honcho;
use crate::error::Error;
use crate::models::chat::{Evidence, StreamChunk};

/// One `data:` payload from a chat `text/event-stream`.
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

/// Incremental SSE decoder. Chunks from the socket are not event-aligned.
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

    pub(crate) fn push(&mut self, chunk: &str) -> Result<Vec<StreamChunk>, Error> {
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
                "error" => return Err(Error::Stream(value.to_string())),
                _ => {}
            }
        }
        Ok(out)
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<StreamChunk>, Error> {
        if !self.pending.is_empty() || !self.data.is_empty() {
            self.pending.push('\n');
            return self.push("");
        }
        Ok(Vec::new())
    }

    fn finish_event(&mut self) -> Result<Option<StreamChunk>, Error> {
        if self.data.is_empty() {
            return Ok(None);
        }
        let raw = std::mem::take(&mut self.data);
        let payload: SsePayload =
            serde_json::from_str(&raw).map_err(|e| Error::Stream(e.to_string()))?;
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

pub(crate) fn failed(
    err: Error,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<StreamChunk, Error>> + Send>> {
    Box::pin(stream! {
        yield Err(err);
    })
}

pub(crate) fn open_chat_stream(
    client: Honcho,
    url: url::Url,
    body: serde_json::Value,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, Error>> + Send>> {
    let headers = match client.headers() {
        Ok(headers) => headers,
        Err(err) => {
            return Box::pin(stream! {
                yield Err(err);
            });
        }
    };
    let send = client
        .http
        .post(url)
        .headers(headers)
        .json(&body)
        .timeout(client.stream_timeout())
        .send();

    Box::pin(stream!({
        let resp = match send.await {
            Ok(resp) => resp,
            Err(err) => {
                yield Err(Error::Http(err));
                return;
            }
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            yield Err(crate::error::parse_api_error(status, &text));
            return;
        }

        let mut bytes = std::pin::pin!(resp.bytes_stream());
        let mut parser = SseParser::new();
        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    yield Err(Error::Http(err));
                    return;
                }
            };
            let text = String::from_utf8_lossy(&chunk);
            match parser.push(&text) {
                Ok(events) => {
                    for event in events {
                        yield Ok(event);
                    }
                }
                Err(err) => {
                    yield Err(err);
                    return;
                }
            }
        }
        match parser.finish() {
            Ok(events) => {
                for event in events {
                    yield Ok(event);
                }
            }
            Err(err) => yield Err(err),
        }
    }))
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
