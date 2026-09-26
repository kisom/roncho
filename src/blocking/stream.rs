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
    finished: bool,
    saw_done: bool,
}

impl ChatStream {
    pub(crate) fn from_body(body: LiveBody) -> Self {
        Self {
            body: Some(body),
            parser: SseParser::new(),
            pending: VecDeque::new(),
            finished: false,
            saw_done: false,
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
                Ok(Some(bytes)) => {
                    let text = String::from_utf8_lossy(&bytes);
                    match self.parser.push(&text) {
                        Ok(chunks) => self.pending.extend(chunks),
                        Err(err) => {
                            self.finished = true;
                            return Some(Err(Error::Stream(err)));
                        }
                    }
                }
                Ok(None) => {
                    self.finished = true;
                    match self.parser.finish() {
                        Ok(chunks) => self.pending.extend(chunks),
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

pub(crate) fn streaming_opts(opts: &DialecticOptions) -> DialecticOptions {
    let mut opts = opts.clone();
    opts.stream = Some(true);
    opts
}
