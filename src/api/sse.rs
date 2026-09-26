use std::pin::Pin;

use async_stream::stream;
use futures::{Stream, StreamExt};

use crate::client::Honcho;
use crate::error::Error;
use crate::models::chat::StreamChunk;
use crate::sse::SseParser;

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
            match parser.push(&text).map_err(Error::Stream) {
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
        match parser.finish().map_err(Error::Stream) {
            Ok(events) => {
                for event in events {
                    yield Ok(event);
                }
            }
            Err(err) => yield Err(err),
        }
    }))
}
