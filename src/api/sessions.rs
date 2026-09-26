use serde_json::{json, Map, Value};

use crate::client::Honcho;
use crate::error::Error;
use crate::models::context::SessionContext;
use crate::models::message::{Message, MessageCreate, MessageSearch};
use crate::models::page::{ListOptions, Page};
use crate::models::peer::Peer;
use crate::models::session::{Session, SessionCreate, SessionPeerConfig};

pub async fn get_or_create_session(
    client: &Honcho,
    create: &SessionCreate,
) -> Result<Session, Error> {
    let body = serde_json::to_value(create).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json("sessions", &body).await
}

pub async fn list_sessions(client: &Honcho, opts: &ListOptions) -> Result<Page<Session>, Error> {
    let query = client.list_query_params(opts);
    client
        .post_json_query("sessions/list", &query, &json!({}))
        .await
}

fn peer_map(peers: &[(String, Option<SessionPeerConfig>)]) -> Result<Value, Error> {
    let mut map = Map::new();
    for (id, config) in peers {
        let value = match config {
            Some(cfg) => serde_json::to_value(cfg).map_err(|e| Error::Encode(e.to_string()))?,
            None => json!({}),
        };
        map.insert(id.clone(), value);
    }
    Ok(Value::Object(map))
}

pub async fn add_peers_to_session(
    client: &Honcho,
    session_id: &str,
    peers: &[(String, Option<SessionPeerConfig>)],
) -> Result<Session, Error> {
    let path = format!("sessions/{}/peers", session_id);
    let body = peer_map(peers)?;
    client.post_json(&path, &body).await
}

pub async fn set_peers_for_session(
    client: &Honcho,
    session_id: &str,
    peers: &[(String, Option<SessionPeerConfig>)],
) -> Result<Session, Error> {
    let path = format!("sessions/{}/peers", session_id);
    let body = peer_map(peers)?;
    client.put_json_query(&path, &[], &body).await
}

pub async fn remove_peers_from_session(
    client: &Honcho,
    session_id: &str,
    peer_ids: &[String],
) -> Result<Session, Error> {
    let path = format!("sessions/{}/peers", session_id);
    client.delete_json_body(&path, Some(&json!(peer_ids))).await
}

pub async fn get_session_peers(
    client: &Honcho,
    session_id: &str,
    opts: &ListOptions,
) -> Result<Page<Peer>, Error> {
    let mut query = Vec::new();
    if let Some(page) = opts.page {
        query.push(("page", page.to_string()));
    }
    if let Some(size) = opts.size {
        query.push(("size", size.to_string()));
    }
    let path = format!("sessions/{}/peers", session_id);
    client.get_json(&path, &query).await
}

pub async fn create_messages(
    client: &Honcho,
    session_id: &str,
    messages: &[MessageCreate],
) -> Result<Vec<Message>, Error> {
    let msgs = serde_json::to_value(messages).map_err(|e| Error::Encode(e.to_string()))?;
    let body = json!({ "messages": msgs });
    let path = format!("sessions/{}/messages", session_id);
    client.post_json(&path, &body).await
}

pub async fn list_messages(
    client: &Honcho,
    session_id: &str,
    opts: &ListOptions,
    filters: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<Page<Message>, Error> {
    let query = client.list_query_params(opts);
    let body = match filters {
        Some(filters) => json!({ "filters": filters }),
        None => json!({}),
    };
    let path = format!("sessions/{}/messages/list", session_id);
    client.post_json_query(&path, &query, &body).await
}

pub async fn get_session_context(
    client: &Honcho,
    session_id: &str,
    opts: &SessionContextOptions,
) -> Result<SessionContext, Error> {
    let mut query: Vec<(&str, String)> = Vec::new();

    if let Some(tokens) = opts.tokens {
        query.push(("tokens", tokens.to_string()));
    }
    if let Some(search_query) = &opts.search_query {
        query.push(("search_query", search_query.clone()));
    }
    if let Some(summary) = opts.summary {
        query.push(("summary", summary.to_string()));
    }
    if let Some(peer_target) = &opts.peer_target {
        query.push(("peer_target", peer_target.clone()));
    }
    if let Some(peer_perspective) = &opts.peer_perspective {
        query.push(("peer_perspective", peer_perspective.clone()));
    }
    if let Some(scope) = &opts.scope {
        query.push(("scope", scope.clone()));
    }
    if let Some(sessions) = &opts.sessions {
        for session in sessions {
            query.push(("sessions", session.clone()));
        }
    }
    if let Some(limit_to_session) = opts.limit_to_session {
        query.push(("limit_to_session", limit_to_session.to_string()));
    }
    if let Some(search_top_k) = opts.search_top_k {
        query.push(("search_top_k", search_top_k.to_string()));
    }
    if let Some(search_max_distance) = opts.search_max_distance {
        query.push(("search_max_distance", search_max_distance.to_string()));
    }
    if let Some(include_most_frequent) = opts.include_most_frequent {
        query.push(("include_most_frequent", include_most_frequent.to_string()));
    }
    if let Some(max_conclusions) = opts.max_conclusions {
        query.push(("max_conclusions", max_conclusions.to_string()));
    }

    let path = format!("sessions/{}/context", session_id);
    client.get_json(&path, &query).await
}

#[derive(Debug, Clone, Default)]
pub struct SessionContextOptions {
    pub tokens: Option<u32>,
    pub search_query: Option<String>,
    pub summary: Option<bool>,
    pub peer_target: Option<String>,
    pub peer_perspective: Option<String>,
    pub scope: Option<String>,
    pub sessions: Option<Vec<String>>,
    pub limit_to_session: Option<bool>,
    pub search_top_k: Option<u32>,
    pub search_max_distance: Option<f64>,
    pub include_most_frequent: Option<bool>,
    pub max_conclusions: Option<u32>,
}

pub async fn search_session(
    client: &Honcho,
    session_id: &str,
    search: &MessageSearch,
) -> Result<Vec<Message>, Error> {
    let path = format!("sessions/{}/search", session_id);
    client
        .post_json(&path, &crate::api::peers::search_body(search, false))
        .await
}

pub async fn clone_session(
    client: &Honcho,
    session_id: &str,
    message_id: Option<&str>,
) -> Result<Session, Error> {
    let mut query = Vec::new();
    if let Some(message_id) = message_id {
        query.push(("message_id", message_id.to_string()));
    }
    let path = format!("sessions/{}/clone", session_id);
    client.post_query(&path, &query).await
}

pub async fn delete_session(client: &Honcho, session_id: &str) -> Result<(), Error> {
    let path = format!("sessions/{}", session_id);
    client.delete_json::<Value>(&path).await?;
    Ok(())
}
