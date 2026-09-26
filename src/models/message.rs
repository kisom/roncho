use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub content: String,
    pub peer_id: String,
    pub session_id: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub created_at: String,
    #[serde(default)]
    pub token_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::Message;

    #[test]
    fn search_shape_omits_workspace_and_token_count() {
        let raw = r#"{"id":"m","content":"hi","peer_id":"p","session_id":"s","created_at":"2024-01-01T00:00:00Z","metadata":{"label":"keep"}}"#;
        let msg: Message = serde_json::from_str(raw).unwrap();
        assert!(msg.workspace_id.is_none());
        assert!(msg.token_count.is_none());
        assert_eq!(msg.metadata["label"], "keep");
    }
}

/// Builder for creating a new message, returned by `Peer::message()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCreate {
    pub content: String,
    pub peer_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl MessageCreate {
    pub fn new(content: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            peer_id: peer_id.into(),
            metadata: None,
            created_at: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Map<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_metadata_entry(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let map = self.metadata.get_or_insert_with(serde_json::Map::new);
        map.insert(key.into(), value);
        self
    }

    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = Some(created_at.into());
        self
    }
}

/// Body for message search. `scope` is only sent on workspace search.
#[derive(Debug, Clone)]
pub struct MessageSearch {
    pub query: String,
    pub filters: Option<serde_json::Map<String, serde_json::Value>>,
    pub limit: Option<u32>,
    pub scope: Option<String>,
}

impl MessageSearch {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            filters: None,
            limit: None,
            scope: None,
        }
    }
}
