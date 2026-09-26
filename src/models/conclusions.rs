use serde::{Deserialize, Serialize};

/// Reasoning level at which a conclusion was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    #[default]
    Explicit,
    Deductive,
    Inductive,
    Contradiction,
}

fn default_times_derived() -> u64 {
    1
}

fn null_times_derived<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u64>::deserialize(deserializer)?.unwrap_or(1))
}

pub(crate) fn null_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// A conclusion: a logical certainty derived from interactions between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conclusion {
    pub id: String,
    pub content: String,
    #[serde(rename = "observer_id")]
    pub observer_id: String,
    #[serde(rename = "observed_id")]
    pub observed_id: String,
    #[serde(rename = "session_id")]
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub level: Level,
    #[serde(rename = "source_ids", default, deserialize_with = "null_vec")]
    pub source_ids: Vec<String>,
    #[serde(
        rename = "times_derived",
        default = "default_times_derived",
        deserialize_with = "null_times_derived"
    )]
    pub times_derived: u64,
    #[serde(rename = "created_at")]
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::Conclusion;

    #[test]
    fn null_source_ids_and_times_derived_decode() {
        let raw = r#"{"id":"c","content":"fact","observer_id":"a","observed_id":"b","session_id":"s","level":"explicit","source_ids":null,"times_derived":null,"created_at":"2024-01-01T00:00:00Z"}"#;
        let conclusion: Conclusion = serde_json::from_str(raw).unwrap();
        assert!(conclusion.source_ids.is_empty());
        assert_eq!(conclusion.times_derived, 1);
    }
}

/// Create a single conclusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConclusionCreate {
    pub content: String,
    pub observer_id: String,
    pub observed_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl ConclusionCreate {
    pub fn new(
        content: impl Into<String>,
        observer_id: impl Into<String>,
        observed_id: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            observer_id: observer_id.into(),
            observed_id: observed_id.into(),
            session_id: None,
        }
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Batch of conclusions to create in a single request (max 100).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConclusionBatchCreate {
    pub conclusions: Vec<ConclusionCreate>,
}

impl ConclusionBatchCreate {
    pub fn new(conclusions: Vec<ConclusionCreate>) -> Self {
        Self { conclusions }
    }

    pub fn push(mut self, conclusion: ConclusionCreate) -> Self {
        self.conclusions.push(conclusion);
        self
    }
}

/// Options for a conclusions list request.
#[derive(Debug, Clone, Default)]
pub struct ConclusionListOptions {
    pub page: Option<usize>,
    pub size: Option<usize>,
    pub reverse: Option<bool>,
    pub filters: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ConclusionListOptions {
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

    pub fn filters(mut self, filters: serde_json::Map<String, serde_json::Value>) -> Self {
        self.filters = Some(filters);
        self
    }
}

/// Semantic search parameters for querying conclusions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConclusionQuery {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ConclusionQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    pub fn top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn distance(mut self, distance: f64) -> Self {
        self.distance = Some(distance);
        self
    }

    pub fn filters(mut self, filters: serde_json::Map<String, serde_json::Value>) -> Self {
        self.filters = Some(filters);
        self
    }
}
