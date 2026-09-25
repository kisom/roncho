use std::pin::Pin;

use async_stream::stream;
use futures::Stream;
use serde_json::json;

use crate::error::Error;
use crate::models::chat::{ChatResponse, DialecticOptions, ReasoningLevel, StreamChunk};
use crate::models::message::Message;
use crate::models::page::{ListOptions, Page};
use crate::models::peer::{Peer, PeerCreate};
use crate::models::session::Session;
use crate::client::Honcho;

pub async fn get_or_create_peer(
    client: &Honcho,
    create: &PeerCreate,
) -> Result<Peer, Error> {
    let body = serde_json::to_value(create).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json("peers", &body).await
}

pub async fn list_peers(
    client: &Honcho,
    opts: &ListOptions,
) -> Result<Page<Peer>, Error> {
    let query = client.list_query_params(opts);
    client.get_json("peers", &query).await
}

pub async fn peer_chat(
    client: &Honcho,
    peer_id: &str,
    opts: &DialecticOptions,
) -> Result<ChatResponse, Error> {
    let body = serde_json::json!({
        "query": opts.query,
        "session_id": opts.session_id,
        "filters": opts.filters,
        "target": opts.target,
        "scope": opts.scope,
        "stream": opts.stream.unwrap_or(false),
        "reasoning_level": opts.reasoning_level.map(|l| match l {
            ReasoningLevel::Minimal => "minimal",
            ReasoningLevel::Low => "low",
            ReasoningLevel::Medium => "medium",
            ReasoningLevel::High => "high",
            ReasoningLevel::Max => "max",
        }),
        "response_format": opts.response_format,
        "include_evidence": opts.include_evidence,
    });

    let path = format!("peers/{}/chat", peer_id);
    client.post_json(&path, &body).await
}

pub fn peer_chat_stream(
    client: Honcho,
    peer_id: String,
    opts: DialecticOptions,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, Error>> + Send>> {
    let path = format!("peers/{}/chat", peer_id);

    let url = match client.url(&path) {
        Ok(u) => u,
        Err(e) => {
            return Box::pin(stream! {
                yield Err(e);
            });
        }
    };

    let headers = match client.headers() {
        Ok(h) => h,
        Err(e) => {
            return Box::pin(stream! {
                yield Err(e);
            });
        }
    };

    let body = serde_json::json!({
        "query": opts.query,
        "session_id": opts.session_id,
        "filters": opts.filters,
        "target": opts.target,
        "scope": opts.scope,
        "stream": true,
        "reasoning_level": opts.reasoning_level.map(|l| match l {
            ReasoningLevel::Minimal => "minimal",
            ReasoningLevel::Low => "low",
            ReasoningLevel::Medium => "medium",
            ReasoningLevel::High => "high",
            ReasoningLevel::Max => "max",
        }),
        "response_format": opts.response_format,
        "include_evidence": opts.include_evidence,
    });

    let send_future = client.http.post(url).headers(headers).json(&body).send();

    Box::pin(stream!({
        let resp = match send_future.await {
            Ok(r) => r,
            Err(e) => {
                yield Err(Error::Http(e));
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
        use futures::StreamExt;

        let mut event_data: Option<String> = None;

        while let Some(chunk_result) = bytes.next().await {
            let chunk = match chunk_result {
                Ok(b) => b,
                Err(e) => {
                    yield Err(Error::Http(e));
                    return;
                }
            };

            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.is_empty() {
                    if let Some(data) = event_data.take() {
                        if !data.is_empty() {
                            yield Ok(StreamChunk { content: data });
                        }
                    }
                    continue;
                }

                if line.starts_with(":") {
                    continue;
                }

                let parts: Vec<&str> = line.splitn(2, ": ").collect();
                let field = parts.get(0).copied().unwrap_or("");
                let value = parts.get(1).copied().unwrap_or("").trim_start();

                match field {
                    "data" => {
                        event_data = Some(value.to_string());
                    }
                    "error" => {
                        yield Err(Error::Stream(value.to_string()));
                        return;
                    }
                    _ => {}
                }
            }
        }

        if let Some(data) = event_data {
            if !data.is_empty() {
                yield Ok(StreamChunk { content: data });
            }
        }
    }))
}

pub async fn search_peer_messages(
    client: &Honcho,
    peer_id: &str,
    query: &str,
) -> Result<Page<Message>, Error> {
    let body = json!({
        "filters": { "query": query }
    });
    let path = format!("peers/{}/search", peer_id);
    client.post_json(&path, &body).await
}

pub async fn get_peer_sessions(
    client: &Honcho,
    peer_id: &str,
    opts: &ListOptions,
) -> Result<Page<Session>, Error> {
    let query = client.list_query_params(opts);
    let path = format!("peers/{}/sessions", peer_id);
    client.get_json(&path, &query).await
}
