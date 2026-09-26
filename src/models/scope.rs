use serde::{Deserialize, Serialize};

/// A named set of sessions. The `id` is the unprefixed scope name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    pub id: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub created_at: String,
}

/// Body for `POST /scopes`. `metadata` is written onto an existing scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCreate {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ScopeCreate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            metadata: None,
        }
    }
}

pub(crate) fn check_scope_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 506 {
        return Err("scope id must be 1 to 506 characters".into());
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("scope id may contain only letters, digits, '_' and '-'".into());
    }
    Ok(())
}

/// One session's backfill into a scope. `docs_copied` appears once it completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeBackfill {
    pub state: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub docs_copied: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ScopeStatusBody {
    Wrapped {
        backfill_status: std::collections::HashMap<String, ScopeBackfill>,
    },
    Flat(std::collections::HashMap<String, ScopeBackfill>),
}

impl ScopeStatusBody {
    pub(crate) fn into_map(self) -> std::collections::HashMap<String, ScopeBackfill> {
        match self {
            ScopeStatusBody::Wrapped { backfill_status } => backfill_status,
            ScopeStatusBody::Flat(map) => map,
        }
    }
}

/// `POST /schedule_dream`. `observed` defaults to `observer` on the server.
#[derive(Debug, Clone, Serialize)]
pub struct ScheduleDream {
    pub observer: String,
    pub dream_type: DreamType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<bool>,
}

impl ScheduleDream {
    pub fn new(observer: impl Into<String>, dream_type: DreamType) -> Self {
        Self {
            observer: observer.into(),
            dream_type,
            observed: None,
            session_id: None,
            rebuild: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DreamType {
    Omni,
    CardRefresh,
}
