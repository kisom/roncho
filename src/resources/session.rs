use std::pin::Pin;

use async_stream::stream;
use futures::Stream;

use crate::api::sessions as sessions_api;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::context::SessionContext;
use crate::models::message::{Message, MessageCreate, MessageSearch};
use crate::models::page::{ListOptions, Page};
use crate::models::session::SessionPeerConfig;

/// A session — a multi-party interaction thread between peers.
#[derive(Debug, Clone)]
pub struct Session {
    pub(crate) client: Honcho,
    pub id: String,
    pub(crate) created_at: String,
    pub is_active: bool,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub configuration: serde_json::Map<String, serde_json::Value>,
}

impl Session {
    pub fn from_model(client: Honcho, model: crate::models::session::Session) -> Self {
        Self {
            client,
            id: model.id,
            created_at: model.created_at,
            is_active: model.is_active,
            metadata: model.metadata,
            configuration: model.configuration,
        }
    }

    pub fn workspace_id(&self) -> &str {
        self.client.workspace_id()
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn to_model(&self) -> crate::models::session::Session {
        crate::models::session::Session {
            id: self.id.clone(),
            created_at: self.created_at.clone(),
            is_active: self.is_active,
            metadata: self.metadata.clone(),
            configuration: self.configuration.clone(),
            workspace_id: self.client.workspace_id().to_string(),
        }
    }

    /// Add peers to this session. Peers that don't exist are auto-created.
    pub async fn add_peers(
        &self,
        peers: &[(&crate::resources::peer::Peer, Option<SessionPeerConfig>)],
    ) -> Result<(), Error> {
        let peer_list: Vec<(String, Option<SessionPeerConfig>)> = peers
            .iter()
            .map(|(p, cfg)| (p.id.clone(), cfg.clone()))
            .collect();
        sessions_api::add_peers_to_session(&self.client, &self.id, &peer_list).await?;
        Ok(())
    }

    /// Replace all peers in this session.
    pub async fn set_peers(
        &self,
        peers: &[(&crate::resources::peer::Peer, Option<SessionPeerConfig>)],
    ) -> Result<(), Error> {
        let peer_list: Vec<(String, Option<SessionPeerConfig>)> = peers
            .iter()
            .map(|(p, cfg)| (p.id.clone(), cfg.clone()))
            .collect();
        sessions_api::set_peers_for_session(&self.client, &self.id, &peer_list).await?;
        Ok(())
    }

    /// Remove peers from this session.
    pub async fn remove_peers(&self, peer_ids: &[impl AsRef<str>]) -> Result<(), Error> {
        let ids: Vec<String> = peer_ids.iter().map(|s| s.as_ref().to_string()).collect();
        sessions_api::remove_peers_from_session(&self.client, &self.id, &ids).await?;
        Ok(())
    }

    /// Add messages to this session.
    pub async fn add_messages(&self, messages: &[MessageCreate]) -> Result<Vec<Message>, Error> {
        sessions_api::create_messages(&self.client, &self.id, messages).await
    }

    /// Get messages from this session.
    pub async fn messages(
        &self,
        opts: &ListOptions,
        filters: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<Page<Message>, Error> {
        sessions_api::list_messages(&self.client, &self.id, opts, filters).await
    }

    /// Stream all messages from this session, auto-paginating.
    pub fn messages_stream(
        &self,
        page_size: usize,
    ) -> Pin<Box<dyn Stream<Item = Result<Message, Error>> + Send>> {
        let client = self.client.clone();
        let session_id = self.id.clone();
        Box::pin(stream!({
            let mut page = 1usize;
            loop {
                let opts = ListOptions::default().page(page).size(page_size);
                let result = sessions_api::list_messages(&client, &session_id, &opts, None).await;
                match result {
                    Ok(page_data) => {
                        let has_next = page_data.has_next();
                        let empty = page_data.items.is_empty();
                        for msg in page_data.items {
                            yield Ok(msg);
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

    /// Get formatted conversation context for LLM integration.
    pub async fn context(&self, opts: &SessionContextRequest) -> Result<SessionContext, Error> {
        let api_opts = sessions_api::SessionContextOptions {
            tokens: opts.tokens,
            search_query: opts.search_query.clone(),
            summary: opts.summary,
            peer_target: opts.peer_target.clone(),
            peer_perspective: opts.peer_perspective.clone(),
            scope: opts.scope.clone(),
            sessions: opts.sessions.clone(),
            limit_to_session: opts.limit_to_session,
            search_top_k: opts.search_top_k,
            search_max_distance: opts.search_max_distance,
            include_most_frequent: opts.include_most_frequent,
            max_conclusions: opts.max_conclusions,
        };
        sessions_api::get_session_context(&self.client, &self.id, &api_opts).await
    }

    /// Search content within this session.
    pub async fn search(&self, query: impl Into<String>) -> Result<Vec<Message>, Error> {
        self.search_with(MessageSearch::new(query)).await
    }

    pub async fn search_with(&self, search: MessageSearch) -> Result<Vec<Message>, Error> {
        sessions_api::search_session(&self.client, &self.id, &search).await
    }

    /// Clone this session, optionally cutting off at `message_id`.
    pub async fn clone(&self, message_id: Option<&str>) -> Result<Session, Error> {
        let model = sessions_api::clone_session(&self.client, &self.id, message_id).await?;
        Ok(Session::from_model(self.client.clone(), model))
    }

    /// Delete this session and all associated messages.
    pub async fn delete(&self) -> Result<(), Error> {
        sessions_api::delete_session(&self.client, &self.id).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionContextRequest {
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

impl SessionContextRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tokens(mut self, tokens: u32) -> Self {
        self.tokens = Some(tokens);
        self
    }

    pub fn summary(mut self, summary: bool) -> Self {
        self.summary = Some(summary);
        self
    }

    pub fn peer_target(mut self, target: impl Into<String>) -> Self {
        self.peer_target = Some(target.into());
        self
    }

    pub fn peer_perspective(mut self, perspective: impl Into<String>) -> Self {
        self.peer_perspective = Some(perspective.into());
        self
    }

    pub fn limit_to_session(mut self, limit: bool) -> Self {
        self.limit_to_session = Some(limit);
        self
    }

    pub fn search_query(mut self, query: impl Into<String>) -> Self {
        self.search_query = Some(query.into());
        self
    }

    pub fn search_top_k(mut self, k: u32) -> Self {
        self.search_top_k = Some(k);
        self
    }

    pub fn search_max_distance(mut self, distance: f64) -> Self {
        self.search_max_distance = Some(distance);
        self
    }

    pub fn include_most_frequent(mut self, include: bool) -> Self {
        self.include_most_frequent = Some(include);
        self
    }

    pub fn max_conclusions(mut self, max: u32) -> Self {
        self.max_conclusions = Some(max);
        self
    }
}
