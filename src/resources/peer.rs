use std::pin::Pin;

use async_stream::stream;
use chrono::{DateTime, Utc};
use futures::Stream;

use crate::api::peers as peers_api;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::chat::{ChatResponse, DialecticOptions, StreamChunk};
use crate::models::context::PeerContext;
use crate::models::message::{Message, MessageCreate, MessageSearch};
use crate::models::page::{ListOptions, Page};

/// A peer entity in Honcho — a user, agent, or any persistent identity.
#[derive(Debug, Clone)]
pub struct Peer {
    pub(crate) client: Honcho,
    pub id: String,
    pub(crate) created_at: DateTime<Utc>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub configuration: serde_json::Map<String, serde_json::Value>,
}

impl Peer {
    pub(crate) fn from_model(client: Honcho, model: crate::models::peer::Peer) -> Self {
        Self {
            client,
            id: model.id,
            created_at: model.created_at,
            metadata: model.metadata,
            configuration: model.configuration,
        }
    }

    pub fn workspace_id(&self) -> &str {
        self.client.workspace_id()
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn to_model(&self) -> crate::models::peer::Peer {
        crate::models::peer::Peer {
            id: self.id.clone(),
            workspace_id: self.client.workspace_id().to_string(),
            created_at: self.created_at,
            metadata: self.metadata.clone(),
            configuration: self.configuration.clone(),
        }
    }

    /// Create a message builder attributed to this peer.
    pub fn message(&self, content: impl Into<String>) -> MessageCreate {
        MessageCreate::new(content, &self.id)
    }

    /// Query this peer's representation using natural language.
    pub async fn chat(
        &self,
        query: impl Into<String>,
        opts: Option<DialecticOptions>,
    ) -> Result<ChatResponse, Error> {
        let mut opts = opts.unwrap_or_default();
        opts.query = query.into();
        peers_api::peer_chat(&self.client, &self.id, &opts).await
    }

    /// Stream chat responses for this peer.
    pub fn chat_stream(
        &self,
        query: impl Into<String>,
        opts: Option<DialecticOptions>,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, Error>> + Send>> {
        let mut opts = opts.unwrap_or_default();
        opts.query = query.into();
        opts.stream = Some(true);
        peers_api::peer_chat_stream(self.client.clone(), self.id.clone(), opts)
    }

    /// Get this peer's sessions.
    pub async fn sessions(&self, opts: &ListOptions) -> Result<Page<Session>, Error> {
        let page: Page<crate::models::session::Session> =
            peers_api::get_peer_sessions(&self.client, &self.id, opts).await?;
        Ok(Page {
            items: page
                .items
                .into_iter()
                .map(|m| Session::from_model(self.client.clone(), m))
                .collect(),
            total: page.total,
            page: page.page,
            size: page.size,
            pages: page.pages,
        })
    }

    /// Stream all sessions for this peer, auto-paginating.
    pub fn sessions_stream(
        &self,
        page_size: usize,
    ) -> Pin<Box<dyn Stream<Item = Result<Session, Error>> + Send>> {
        let client = self.client.clone();
        let peer_id = self.id.clone();
        Box::pin(stream!({
            let mut page = 1usize;
            loop {
                let opts = ListOptions::default().page(page).size(page_size);
                let result = peers_api::get_peer_sessions(&client, &peer_id, &opts).await;
                match result {
                    Ok(page_data) => {
                        let has_next = page_data.has_next();
                        let empty = page_data.items.is_empty();
                        for session in page_data.items {
                            yield Ok(Session::from_model(client.clone(), session));
                        }
                        if !has_next || empty {
                            break;
                        }
                        page += 1;
                    }
                    Err(e) => {
                        yield Err(e);
                        break;
                    }
                }
            }
        }))
    }

    /// Search messages attributed to this peer.
    pub async fn search(&self, query: impl Into<String>) -> Result<Vec<Message>, Error> {
        self.search_with(MessageSearch::new(query)).await
    }

    pub async fn search_with(&self, search: MessageSearch) -> Result<Vec<Message>, Error> {
        peers_api::search_peer_messages(&self.client, &self.id, &search).await
    }

    /// Get the working representation of this peer, optionally about another peer.
    pub async fn context(
        &self,
        target: Option<&str>,
        opts: Option<PeerContextOptions>,
    ) -> Result<PeerContext, Error> {
        let opts = opts.unwrap_or_default();
        let mut query = Vec::new();
        if let Some(target) = target {
            query.push(("target", target.to_string()));
        }
        if let Some(search_query) = opts.search_query {
            query.push(("search_query", search_query));
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
        let path = format!("peers/{}/context", self.id);
        self.client.get_json(&path, &query).await
    }

    /// Get the peer card for this peer (or about another peer).
    pub async fn get_card(&self, target: Option<&str>) -> Result<Vec<String>, Error> {
        let mut query = Vec::new();
        if let Some(target) = target {
            query.push(("target", target.to_string()));
        }
        let path = format!("peers/{}/card", self.id);
        let card: PeerCardResponse = self.client.get_json(&path, &query).await?;
        Ok(card.peer_card.unwrap_or_default())
    }

    /// Set the peer card for this peer (or about another peer).
    pub async fn set_card(
        &self,
        card: &[impl AsRef<str>],
        target: Option<&str>,
    ) -> Result<Vec<String>, Error> {
        let card_values: Vec<String> = card.iter().map(|s| s.as_ref().to_string()).collect();
        let mut query = Vec::new();
        if let Some(target) = target {
            query.push(("target", target.to_string()));
        }
        let body = serde_json::json!({ "peer_card": card_values });
        let path = format!("peers/{}/card", self.id);
        let card: PeerCardResponse = self.client.put_json_query(&path, &query, &body).await?;
        Ok(card.peer_card.unwrap_or_default())
    }

    /// Conclusions this peer holds about itself (`observer` and `observed` are this peer).
    pub fn conclusions(&self) -> crate::resources::conclusions::Conclusions {
        self.conclusion_view(&self.id)
    }

    /// Conclusions this peer holds about `observed_id`.
    pub fn conclusions_of(
        &self,
        observed_id: impl AsRef<str>,
    ) -> crate::resources::conclusions::Conclusions {
        self.conclusion_view(observed_id.as_ref())
    }

    fn conclusion_view(&self, observed_id: &str) -> crate::resources::conclusions::Conclusions {
        let mut filters = serde_json::Map::new();
        filters.insert(
            "observer_id".into(),
            serde_json::Value::String(self.id.clone()),
        );
        filters.insert(
            "observed_id".into(),
            serde_json::Value::String(observed_id.to_string()),
        );
        crate::resources::conclusions::Conclusions::with_filters(self.client.clone(), filters)
    }
}

#[derive(Debug, serde::Deserialize)]
struct PeerCardResponse {
    peer_card: Option<Vec<String>>,
}

#[derive(Default)]
pub struct PeerContextOptions {
    pub search_query: Option<String>,
    pub search_top_k: Option<u32>,
    pub search_max_distance: Option<f64>,
    pub include_most_frequent: Option<bool>,
    pub max_conclusions: Option<u32>,
}

// Re-export Session for use in return types
pub use crate::resources::session::Session;
