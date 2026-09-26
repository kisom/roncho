use crate::api::peers as peers_api;
use crate::api::scopes as scopes_api;
use crate::api::sessions as sessions_api;
use crate::api::workspace;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::message::MessageCreate;
use crate::models::page::{ListOptions, Page};
use crate::models::peer::PeerCreate;
use crate::models::session::SessionCreate;
use crate::resources::conclusions::Conclusions;
use crate::resources::peer::Peer;
use crate::resources::session::Session;
use crate::resources::workspace::Workspaces;

impl Honcho {
    /// Get or create a peer by ID. If the peer doesn't exist, it's created.
    pub async fn peer(&self, id: impl Into<String>) -> Result<Peer, Error> {
        let create = PeerCreate::new(id.into());
        let model = peers_api::get_or_create_peer(self, &create).await?;
        Ok(Peer::from_model(self.clone(), model))
    }

    /// Get or create a peer with configuration and metadata.
    pub async fn peer_with_config(
        &self,
        id: impl Into<String>,
        metadata: Option<serde_json::Map<String, serde_json::Value>>,
        configuration: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<Peer, Error> {
        let create = PeerCreate::new(id.into())
            .with_metadata(metadata.unwrap_or_default())
            .with_configuration(configuration.unwrap_or_default());
        let model = peers_api::get_or_create_peer(self, &create).await?;
        Ok(Peer::from_model(self.clone(), model))
    }

    /// List all peers in this workspace.
    pub async fn peers(
        &self,
        opts: &ListOptions,
    ) -> Result<Page<crate::models::peer::Peer>, Error> {
        peers_api::list_peers(self, opts).await
    }

    /// Get or create a session by ID. If the session doesn't exist, it's created.
    pub async fn session(&self, id: impl Into<String>) -> Result<Session, Error> {
        let create = SessionCreate::new(id.into());
        let model = sessions_api::get_or_create_session(self, &create).await?;
        Ok(Session::from_model(self.clone(), model))
    }

    /// Get or create a session with configuration and metadata.
    pub async fn session_with_config(
        &self,
        id: impl Into<String>,
        metadata: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<Session, Error> {
        let create = SessionCreate::new(id.into()).with_metadata(metadata.unwrap_or_default());
        let model = sessions_api::get_or_create_session(self, &create).await?;
        Ok(Session::from_model(self.clone(), model))
    }

    /// List all sessions in this workspace.
    pub async fn sessions(&self, opts: &ListOptions) -> Result<Page<Session>, Error> {
        let page: Page<crate::models::session::Session> =
            sessions_api::list_sessions(self, opts).await?;
        Ok(Page {
            items: page
                .items
                .into_iter()
                .map(|m| Session::from_model(self.clone(), m))
                .collect(),
            total: page.total,
            page: page.page,
            size: page.size,
            pages: page.pages,
        })
    }

    /// Access the workspace-level Conclusions API.
    pub fn conclusions(&self) -> Conclusions {
        Conclusions::new(self.clone())
    }

    /// Access the Workspaces API for listing, updating, and deleting workspaces.
    pub fn workspaces(&self) -> Workspaces {
        Workspaces::new(self.clone())
    }

    /// Search across all content in the workspace.
    pub async fn search(
        &self,
        query: impl Into<String>,
    ) -> Result<Vec<crate::models::message::Message>, Error> {
        self.search_with(crate::models::message::MessageSearch::new(query))
            .await
    }

    pub async fn search_with(
        &self,
        search: crate::models::message::MessageSearch,
    ) -> Result<Vec<crate::models::message::Message>, Error> {
        workspace::search_workspace(self, &search).await
    }

    /// Ask a question across every peer in the workspace.
    pub async fn chat(
        &self,
        query: impl Into<String>,
        opts: Option<crate::models::chat::DialecticOptions>,
    ) -> Result<crate::models::chat::ChatResponse, Error> {
        let mut opts = opts.unwrap_or_default();
        opts.query = query.into();
        workspace::chat_workspace(self, &opts).await
    }

    /// Stream workspace-level chat.
    pub fn chat_stream(
        &self,
        query: impl Into<String>,
        opts: Option<crate::models::chat::DialecticOptions>,
    ) -> std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<crate::models::chat::StreamChunk, Error>> + Send>,
    > {
        let mut opts = opts.unwrap_or_default();
        opts.query = query.into();
        workspace::chat_workspace_stream(self.clone(), opts)
    }

    /// Create a message attributed to a peer ID.
    pub fn message(&self, content: impl Into<String>, peer_id: impl Into<String>) -> MessageCreate {
        MessageCreate::new(content, peer_id.into())
    }

    /// `POST /v3/workspaces/{workspace}/scopes`.
    pub async fn scope(
        &self,
        create: &crate::models::scope::ScopeCreate,
    ) -> Result<crate::models::scope::Scope, Error> {
        scopes_api::get_or_create_scope(self, create).await
    }

    /// `GET /v3/workspaces/{workspace}/scopes/{id}`.
    pub async fn get_scope(&self, scope_id: &str) -> Result<crate::models::scope::Scope, Error> {
        scopes_api::get_scope(self, scope_id).await
    }

    /// `POST /v3/workspaces/{workspace}/scopes/list`.
    pub async fn list_scopes(
        &self,
        opts: &ListOptions,
    ) -> Result<Page<crate::models::scope::Scope>, Error> {
        scopes_api::list_scopes(self, opts).await
    }

    /// `POST /v3/workspaces/{workspace}/scopes/{id}/sessions`.
    pub async fn add_scope_sessions(
        &self,
        scope_id: &str,
        session_ids: &[String],
    ) -> Result<(), Error> {
        scopes_api::add_scope_sessions(self, scope_id, session_ids).await
    }

    /// `POST /v3/workspaces/{workspace}/scopes/{id}/sessions/list`.
    pub async fn list_scope_sessions(
        &self,
        scope_id: &str,
        opts: &ListOptions,
    ) -> Result<Page<Session>, Error> {
        let page = scopes_api::list_scope_sessions(self, scope_id, opts).await?;
        Ok(Page {
            items: page
                .items
                .into_iter()
                .map(|m| Session::from_model(self.clone(), m))
                .collect(),
            total: page.total,
            page: page.page,
            size: page.size,
            pages: page.pages,
        })
    }

    /// `DELETE /v3/workspaces/{workspace}/scopes/{id}/sessions/{session}`.
    pub async fn remove_scope_session(
        &self,
        scope_id: &str,
        session_id: &str,
    ) -> Result<(), Error> {
        scopes_api::remove_scope_session(self, scope_id, session_id).await
    }

    /// `GET /v3/workspaces/{workspace}/scopes/{id}/status`.
    pub async fn scope_status(
        &self,
        scope_id: &str,
    ) -> Result<std::collections::HashMap<String, crate::models::scope::ScopeBackfill>, Error> {
        scopes_api::scope_status(self, scope_id).await
    }

    /// `GET /v3/workspaces/{workspace}/queue/status`.
    pub async fn queue_status(
        &self,
        query: &crate::models::workspace::QueueStatusQuery,
    ) -> Result<crate::models::workspace::QueueStatus, Error> {
        workspace::queue_status(self, query).await
    }

    /// `POST /v3/workspaces/{workspace}/schedule_dream`.
    pub async fn schedule_dream(
        &self,
        dream: &crate::models::scope::ScheduleDream,
    ) -> Result<(), Error> {
        workspace::schedule_dream(self, dream).await
    }
}
