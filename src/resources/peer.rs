use std::pin::Pin;

use async_stream::stream;
use chrono::{DateTime, Utc};
use futures::Stream;

use crate::api::peers as peers_api;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::chat::{ChatResponse, DialecticOptions, StreamChunk};
use crate::models::context::PeerContext;
use crate::models::message::MessageCreate;
use crate::models::page::{ListOptions, Page};

/// A peer entity in Honcho — a user, agent, or any persistent identity.
#[derive(Debug, Clone)]
pub struct Peer {
    pub(crate) client: Honcho,
    pub id: String,
    pub display_name: String,
    pub(crate) created_at: DateTime<Utc>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub configuration: serde_json::Map<String, serde_json::Value>,
}

impl Peer {
    pub(crate) fn from_model(client: Honcho, model: crate::models::peer::Peer) -> Self {
        Self {
            client,
            id: model.id,
            display_name: model.display_name,
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
            display_name: self.display_name.clone(),
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
    pub async fn sessions(
        &self,
        opts: &ListOptions,
    ) -> Result<Page<Session>, Error> {
        let page: Page<crate::models::session::Session> =
            peers_api::get_peer_sessions(&self.client, &self.id, opts).await?;
        Ok(Page {
            items: page.items.into_iter().map(|m| Session::from_model(self.client.clone(), m)).collect(),
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
                        for session in page_data.items {
                            yield Ok(Session::from_model(client.clone(), session));
                        }
                        if !has_next {
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
    pub async fn search(
        &self,
        query: impl Into<String>,
        _opts: &ListOptions,
    ) -> Result<Page<crate::models::message::Message>, Error> {
        peers_api::search_peer_messages(&self.client, &self.id, &query.into()).await
    }

    /// Get the working representation of this peer, optionally about another peer.
    pub async fn context(
        &self,
        target: Option<&str>,
        _opts: Option<PeerContextOptions>,
    ) -> Result<PeerContext, Error> {
        let path = if let Some(target) = target {
            format!("peers/{}/context?target={}", self.id, target)
        } else {
            format!("peers/{}/context", self.id)
        };
        self.client.get_json(&path, &[]).await
    }

    /// Get the peer card for this peer (or about another peer).
    pub async fn get_card(&self, target: Option<&str>) -> Result<Vec<String>, Error> {
        let path = if let Some(target) = target {
            format!("peers/{}/card?target={}", self.id, target)
        } else {
            format!("peers/{}/card", self.id)
        };
        self.client.get_json::<Vec<String>>(&path, &[]).await
    }

    /// Set the peer card for this peer (or about another peer).
    pub async fn set_card(
        &self,
        card: &[impl AsRef<str>],
        target: Option<&str>,
    ) -> Result<Vec<String>, Error> {
        let card_values: Vec<String> = card.iter().map(|s| s.as_ref().to_string()).collect();

        let body = if let Some(target) = target {
            serde_json::json!({ "target": target, "card": card_values })
        } else {
            serde_json::json!({ "card": card_values })
        };

        let path = format!("peers/{}/card", self.id);
        self.client.post_json(&path, &body).await
    }

    /// Access conclusions about this peer. List and query results are filtered
    /// to conclusions where this peer is the observed peer.
    pub fn conclusions(&self) -> crate::resources::conclusions::Conclusions {
        let filters = serde_json::json!({ "observed_id": self.id })
            .as_object()
            .cloned()
            .unwrap_or_default();
        crate::resources::conclusions::Conclusions::with_filters(
            self.client.clone(),
            filters,
        )
    }
}

pub struct PeerContextOptions {
    pub search_query: Option<String>,
    pub search_top_k: Option<u32>,
    pub search_max_distance: Option<f64>,
    pub include_most_frequent: Option<bool>,
    pub max_conclusions: Option<u32>,
}

impl Default for PeerContextOptions {
    fn default() -> Self {
        Self {
            search_query: None,
            search_top_k: None,
            search_max_distance: None,
            include_most_frequent: None,
            max_conclusions: None,
        }
    }
}

// Re-export Session for use in return types
pub use crate::resources::session::Session;
