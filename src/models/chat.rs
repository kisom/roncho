use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::models::conclusions::Level;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: Option<String>,
    #[serde(default)]
    pub evidence: Option<Evidence>,
}

impl ChatResponse {
    pub fn content(&self) -> &str {
        self.content.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(default)]
    pub conclusions: Vec<EvidenceObservation>,
    #[serde(default)]
    pub messages: Vec<EvidenceMessageRef>,
    #[serde(default)]
    pub tool_calls: Vec<EvidenceToolCall>,
    #[serde(rename = "reasoning_trace_id")]
    #[serde(default)]
    pub reasoning_trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceMessageRef {
    pub id: String,
    #[serde(rename = "session_id")]
    pub session_id: String,
    #[serde(rename = "peer_id")]
    pub peer_id: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceObservation {
    pub id: String,
    pub level: Level,
    pub content: String,
    pub created_at: String,
    pub observer_id: String,
    pub observed_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default, deserialize_with = "crate::models::conclusions::null_vec")]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceToolCall {
    #[serde(rename = "tool_name")]
    pub tool_name: String,
    #[serde(default)]
    #[serde(rename = "tool_input")]
    pub tool_input: serde_json::Map<String, serde_json::Value>,
}

/// One scope name, or a list of them. Chat accepts either.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScopeNames {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningLevel {
    Minimal,
    Low,
    Medium,
    High,
    Max,
}

/// Options for a dialectic/chat request.
#[derive(Debug, Clone, Default)]
pub struct DialecticOptions {
    pub session_id: Option<String>,
    pub filters: Option<serde_json::Map<String, serde_json::Value>>,
    pub scope: Option<ScopeNames>,
    pub target: Option<String>,
    pub query: String,
    pub stream: Option<bool>,
    pub reasoning_level: Option<ReasoningLevel>,
    pub response_format: Option<serde_json::Map<String, serde_json::Value>>,
    pub include_evidence: Option<bool>,
}

/// A single chunk from a streaming chat response.
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub done: bool,
    pub evidence: Option<Evidence>,
}

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

#[cfg(test)]
mod evidence_null_tests {
    use super::EvidenceObservation;

    #[test]
    fn null_source_ids_decode() {
        let raw = r#"{"id":"c","level":"explicit","content":"fact","created_at":"2024-01-01T00:00:00Z","observer_id":"a","observed_id":"b","source_ids":null}"#;
        let row: EvidenceObservation = serde_json::from_str(raw).unwrap();
        assert!(row.source_ids.is_empty());
    }
}
