use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub is_active: bool,
    pub workspace_id: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub configuration: serde_json::Map<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreate {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers: Option<std::collections::HashMap<String, SessionPeerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
}

impl SessionCreate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            metadata: None,
            peers: None,
            configuration: None,
            scopes: None,
        }
    }

    pub fn with_peers(mut self, peers: std::collections::HashMap<String, SessionPeerConfig>) -> Self {
        self.peers = Some(peers);
        self
    }

    pub fn with_metadata(
        mut self,
        metadata: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_configuration(
        mut self,
        configuration: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.configuration = Some(configuration);
        self
    }
}

/// Per-peer configuration within a session. Controls theory-of-mind behavior.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionPeerConfig {
    /// Whether Honcho will use reasoning to form a representation of this peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observe_me: Option<bool>,
    /// Whether this peer should form a theory-of-mind representation of other peers in the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observe_others: Option<bool>,
}

impl SessionPeerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe_me(mut self, observe: bool) -> Self {
        self.observe_me = Some(observe);
        self
    }

    pub fn observe_others(mut self, observe: bool) -> Self {
        self.observe_others = Some(observe);
        self
    }
}
