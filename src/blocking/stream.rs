use std::collections::VecDeque;

use super::error::Error;
use super::http::LiveBody;
use crate::models::chat::{DialecticOptions, StreamChunk};
use crate::sse::SseParser;

/// Chat events from one blocking HTTP response.
///
/// [`Iterator::next`] waits on the socket. The client's read timeout is "no
/// bytes for this long", so a pause between events is a timeout and a slow
/// trickle is not.
pub struct ChatStream {
    body: Option<LiveBody>,
    parser: SseParser,
    pending: VecDeque<StreamChunk>,
    /// Bytes of a UTF-8 character that arrived split across reads.
    utf8: Vec<u8>,
    finished: bool,
    saw_done: bool,
    produced: bool,
}

impl ChatStream {
    pub(crate) fn from_body(body: LiveBody) -> Self {
        Self {
            body: Some(body),
            parser: SseParser::new(),
            pending: VecDeque::new(),
            utf8: Vec::new(),
            finished: false,
            saw_done: false,
            produced: false,
        }
    }
}

impl Iterator for ChatStream {
    type Item = Result<StreamChunk, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(chunk) = self.pending.pop_front() {
                if chunk.done {
                    self.saw_done = true;
                }
                self.produced = true;
                return Some(Ok(chunk));
            }
            if self.finished || self.saw_done {
                return None;
            }
            let Some(body) = self.body.as_mut() else {
                self.finished = true;
                return None;
            };
            match body.read_some() {
                Ok(Some(bytes)) => match take_utf8(&mut self.utf8, &bytes) {
                    Ok(text) => match self.parser.push(&text) {
                        Ok(chunks) => self.pending.extend(chunks),
                        Err(err) => {
                            self.finished = true;
                            return Some(Err(Error::Stream(err)));
                        }
                    },
                    Err(err) => {
                        self.finished = true;
                        return Some(Err(err));
                    }
                },
                Ok(None) => {
                    self.finished = true;
                    if !self.utf8.is_empty() {
                        return Some(Err(Error::Stream(
                            "response ended inside a UTF-8 character".into(),
                        )));
                    }
                    match self.parser.finish() {
                        Ok(chunks) => {
                            if chunks.is_empty() && !self.produced {
                                return Some(Err(Error::Stream(
                                    "chat response was not a text/event-stream".into(),
                                )));
                            }
                            self.pending.extend(chunks);
                        }
                        Err(err) => return Some(Err(Error::Stream(err))),
                    }
                }
                Err(Error::Closed) if self.pending.is_empty() && !self.saw_done => {
                    self.finished = true;
                    match self.parser.finish() {
                        Ok(chunks) => {
                            if chunks.is_empty() {
                                return Some(Err(Error::Closed));
                            }
                            self.pending.extend(chunks);
                        }
                        Err(err) => return Some(Err(Error::Stream(err))),
                    }
                }
                Err(err) => {
                    self.finished = true;
                    return Some(Err(err));
                }
            }
        }
    }
}

/// Append `bytes`, returning the complete UTF-8 and keeping a trailing
/// partial character for the next read.
fn take_utf8(carry: &mut Vec<u8>, bytes: &[u8]) -> Result<String, Error> {
    carry.extend_from_slice(bytes);
    match std::str::from_utf8(carry) {
        Ok(text) => {
            let owned = text.to_string();
            carry.clear();
            Ok(owned)
        }
        Err(err) => {
            let valid = err.valid_up_to();
            let text = std::str::from_utf8(&carry[..valid])
                .expect("valid_up_to is a char boundary")
                .to_string();
            carry.drain(..valid);
            if err.error_len().is_some() || carry.len() > 3 {
                return Err(Error::Stream("response was not valid UTF-8".into()));
            }
            Ok(text)
        }
    }
}

pub(crate) fn streaming_opts(opts: &DialecticOptions) -> DialecticOptions {
    let mut opts = opts.clone();
    opts.stream = Some(true);
    opts
}
