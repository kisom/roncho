use crate::api::peers as peers_api;
use crate::api::sessions as sessions_api;
use crate::api::workspace;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::message::MessageCreate;
use crate::models::page::{ListOptions, Page};
use crate::models::peer::{PeerCreate};
use crate::models::session::{SessionCreate};
use crate::resources::peer::Peer;
use crate::resources::session::Session;

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
    pub async fn peers(&self, opts: &ListOptions) -> Result<Page<crate::models::peer::Peer>, Error> {
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
        let page: Page<crate::models::session::Session> = sessions_api::list_sessions(self, opts).await?;
        Ok(Page {
            items: page.items.into_iter().map(|m| Session::from_model(self.clone(), m)).collect(),
            total: page.total,
            page: page.page,
            size: page.size,
            pages: page.pages,
        })
    }

    /// Search across all content in the workspace.
    pub async fn search(
        &self,
        query: impl Into<String>,
    ) -> Result<Page<crate::models::message::Message>, Error> {
        workspace::search_workspace(self, &query.into()).await
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
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<crate::models::chat::StreamChunk, Error>> + Send>> {
        let mut opts = opts.unwrap_or_default();
        opts.query = query.into();
        workspace::chat_workspace_stream(self.clone(), opts)
    }

    /// Create a message attributed to a peer ID.
    pub fn message(
        &self,
        content: impl Into<String>,
        peer_id: impl Into<String>,
    ) -> MessageCreate {
        MessageCreate::new(content, peer_id.into())
    }
}
