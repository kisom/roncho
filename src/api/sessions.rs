use serde_json::json;

use crate::client::Honcho;
use crate::error::Error;
use crate::models::context::SessionContext;
use crate::models::message::{Message, MessageCreate};
use crate::models::page::{ListOptions, Page};
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
    client.get_json("sessions", &query).await
}

pub async fn add_peers_to_session(
    client: &Honcho,
    session_id: &str,
    peers: &[(String, Option<SessionPeerConfig>)],
) -> Result<serde_json::Value, Error> {
    let mut peer_map = serde_json::Map::new();
    for (id, config) in peers {
        if let Some(cfg) = config {
            peer_map.insert(id.clone(), serde_json::to_value(cfg).unwrap_or_default());
        } else {
            peer_map.insert(id.clone(), serde_json::Value::Null);
        }
    }

    let body = json!({ "peers": peer_map });
    let path = format!("sessions/{}/peers", session_id);
    client.post_json(&path, &body).await
}

pub async fn set_peers_for_session(
    client: &Honcho,
    session_id: &str,
    peers: &[(String, Option<SessionPeerConfig>)],
) -> Result<serde_json::Value, Error> {
    let mut peer_map = serde_json::Map::new();
    for (id, config) in peers {
        if let Some(cfg) = config {
            peer_map.insert(id.clone(), serde_json::to_value(cfg).unwrap_or_default());
        } else {
            peer_map.insert(id.clone(), serde_json::Value::Null);
        }
    }

    let body = serde_json::json!({ "peers": peer_map });
    let path = format!("sessions/{}/peers/set", session_id);
    client.post_json(&path, &body).await
}

pub async fn remove_peers_from_session(
    client: &Honcho,
    session_id: &str,
    peer_ids: &[String],
) -> Result<serde_json::Value, Error> {
    let body = json!({ "peers": peer_ids });
    let path = format!("sessions/{}/peers/remove", session_id);
    client.post_json(&path, &body).await
}

pub async fn get_session_peers(
    client: &Honcho,
    session_id: &str,
    opts: &ListOptions,
) -> Result<Page<SessionPeerInfo>, Error> {
    let _query = client.list_query_params(opts);
    let path = format!("sessions/{}/peers", session_id);
    client.get_json(&path, &Vec::new()).await
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionPeerInfo {
    pub id: String,
    pub peer_id: String,
    pub session_id: String,
    #[serde(default)]
    pub observe_me: Option<bool>,
    #[serde(default)]
    pub observe_others: Option<bool>,
}

pub async fn create_messages(
    client: &Honcho,
    session_id: &str,
    messages: &[MessageCreate],
) -> Result<Vec<Message>, Error> {
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();

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
    let _query = client.list_query_params(opts);
    let body = json!({ "filters": filters });
    let path = format!("sessions/{}/messages/list", session_id);
    client.post_json(&path, &body).await
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
    pub limit_to_session: Option<bool>,
    pub search_top_k: Option<u32>,
    pub search_max_distance: Option<f64>,
    pub include_most_frequent: Option<bool>,
    pub max_conclusions: Option<u32>,
}

pub async fn search_session(
    client: &Honcho,
    session_id: &str,
    query: &str,
) -> Result<Page<Message>, Error> {
    let body = json!({ "filters": { "query": query } });
    let path = format!("sessions/{}/search", session_id);
    client.post_json(&path, &body).await
}

pub async fn clone_session(
    client: &Honcho,
    session_id: &str,
    up_to_message_id: Option<&str>,
) -> Result<Session, Error> {
    let body = json!({
        "up_to_message_id": up_to_message_id
    });
    let path = format!("sessions/{}/clone", session_id);
    client.post_json(&path, &body).await
}

pub async fn delete_session(client: &Honcho, session_id: &str) -> Result<serde_json::Value, Error> {
    let path = format!("sessions/{}", session_id);
    client.delete_json(&path).await
}

pub async fn get_session(client: &Honcho, session_id: &str) -> Result<Session, Error> {
    let path = format!("sessions/{}", session_id);
    client.get_json::<Session>(&path, &[]).await
}
