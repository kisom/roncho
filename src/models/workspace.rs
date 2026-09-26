use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A workspace returned by the Honcho API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub configuration: serde_json::Map<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Parameters for getting or creating a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCreate {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<serde_json::Map<String, serde_json::Value>>,
}

impl WorkspaceCreate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            metadata: None,
            configuration: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Map<String, serde_json::Value>) -> Self {
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

/// Parameters for updating a workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<serde_json::Map<String, serde_json::Value>>,
}

impl WorkspaceUpdate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_metadata(mut self, metadata: serde_json::Map<String, serde_json::Value>) -> Self {
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

/// Options for paginating the list workspaces endpoint.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceListOptions {
    pub page: Option<usize>,
    pub size: Option<usize>,
    pub reverse: Option<bool>,
}

impl WorkspaceListOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn page(mut self, page: usize) -> Self {
        self.page = Some(page);
        self
    }

    pub fn size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = Some(reverse);
        self
    }
}
