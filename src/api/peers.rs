use std::pin::Pin;

use futures::Stream;
use serde_json::{json, Value};

use crate::api::sse;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::chat::{ChatResponse, DialecticOptions, ScopeNames, StreamChunk};
use crate::models::message::{Message, MessageSearch};
use crate::models::page::{ListOptions, Page};
use crate::models::peer::{Peer, PeerCreate};
use crate::models::session::Session;

pub async fn get_or_create_peer(client: &Honcho, create: &PeerCreate) -> Result<Peer, Error> {
    let body = serde_json::to_value(create).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json("peers", &body).await
}

pub async fn list_peers(client: &Honcho, opts: &ListOptions) -> Result<Page<Peer>, Error> {
    let query = client.list_query_params(opts);
    client
        .post_json_query("peers/list", &query, &json!({}))
        .await
}

pub async fn peer_chat(
    client: &Honcho,
    peer_id: &str,
    opts: &DialecticOptions,
) -> Result<ChatResponse, Error> {
    let path = format!("peers/{}/chat", peer_id);
    client.post_json(&path, &peer_chat_body(opts)).await
}

pub fn peer_chat_stream(
    client: Honcho,
    peer_id: String,
    opts: DialecticOptions,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, Error>> + Send>> {
    let path = format!("peers/{}/chat", peer_id);
    let url = match client.url(&path) {
        Ok(url) => url,
        Err(err) => return sse::failed(err),
    };
    sse::open_chat_stream(client, url, peer_chat_body(&opts))
}

pub async fn search_peer_messages(
    client: &Honcho,
    peer_id: &str,
    search: &MessageSearch,
) -> Result<Vec<Message>, Error> {
    let path = format!("peers/{}/search", peer_id);
    client.post_json(&path, &search_body(search, false)).await
}

pub async fn get_peer_sessions(
    client: &Honcho,
    peer_id: &str,
    opts: &ListOptions,
) -> Result<Page<Session>, Error> {
    let query = client.list_query_params(opts);
    let path = format!("peers/{}/sessions", peer_id);
    client.post_json_query(&path, &query, &json!({})).await
}

pub(crate) fn peer_chat_body(opts: &DialecticOptions) -> Value {
    chat_body(opts, true)
}

pub(crate) fn workspace_chat_body(opts: &DialecticOptions) -> Result<Value, Error> {
    if opts.target.is_some() || opts.filters.is_some() {
        return Err(Error::Configuration(
            "workspace chat has no target or filters; use session_id or scope".into(),
        ));
    }
    Ok(chat_body(opts, false))
}

fn chat_body(opts: &DialecticOptions, include_peer_fields: bool) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("query".into(), json!(opts.query));
    insert_some(
        &mut body,
        "session_id",
        opts.session_id.as_ref().map(json_str),
    );
    if let Some(stream) = opts.stream {
        body.insert("stream".into(), json!(stream));
    }
    if let Some(level) = opts.reasoning_level {
        body.insert(
            "reasoning_level".into(),
            json!(match level {
                crate::models::chat::ReasoningLevel::Minimal => "minimal",
                crate::models::chat::ReasoningLevel::Low => "low",
                crate::models::chat::ReasoningLevel::Medium => "medium",
                crate::models::chat::ReasoningLevel::High => "high",
                crate::models::chat::ReasoningLevel::Max => "max",
            }),
        );
    }
    insert_some(
        &mut body,
        "response_format",
        opts.response_format.clone().map(Value::Object),
    );
    if let Some(include) = opts.include_evidence {
        body.insert("include_evidence".into(), json!(include));
    }
    if let Some(scope) = &opts.scope {
        body.insert("scope".into(), scope_value(scope));
    }
    if include_peer_fields {
        insert_some(
            &mut body,
            "filters",
            opts.filters.clone().map(Value::Object),
        );
        insert_some(&mut body, "target", opts.target.as_ref().map(json_str));
    }
    Value::Object(body)
}

fn scope_value(scope: &ScopeNames) -> Value {
    match scope {
        ScopeNames::One(name) => json!(name),
        ScopeNames::Many(names) => json!(names),
    }
}

fn json_str(value: &String) -> Value {
    json!(value)
}

fn insert_some(body: &mut serde_json::Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        body.insert(key.to_string(), value);
    }
}

pub(crate) fn search_body(search: &MessageSearch, include_scope: bool) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("query".into(), json!(search.query));
    if let Some(filters) = &search.filters {
        body.insert("filters".into(), Value::Object(filters.clone()));
    }
    if let Some(limit) = search.limit {
        body.insert("limit".into(), json!(limit));
    }
    if include_scope {
        if let Some(scope) = &search.scope {
            body.insert("scope".into(), json!(scope));
        }
    }
    Value::Object(body)
}
