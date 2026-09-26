use crate::api::conclusions as conclusions_api;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionListOptions, ConclusionQuery,
};
use crate::models::page::Page;

/// Handle for the workspace-level Conclusions API.
///
/// Obtain one from `honcho.conclusions()`, or a peer-scoped one from
/// `peer.conclusions()` (which filters by `observed_id`).
#[derive(Debug, Clone)]
pub struct Conclusions {
    client: Honcho,
    filters: Option<serde_json::Map<String, serde_json::Value>>,
}

impl Conclusions {
    pub(crate) fn new(client: Honcho) -> Self {
        Self {
            client,
            filters: None,
        }
    }

    pub(crate) fn with_filters(
        client: Honcho,
        filters: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        Self {
            client,
            filters: Some(filters),
        }
    }

    /// Create one or more conclusions (batch of 1–100).
    pub async fn create(&self, batch: ConclusionBatchCreate) -> Result<Vec<Conclusion>, Error> {
        conclusions_api::create_conclusions(&self.client, &batch).await
    }

    /// List conclusions, ordered by recency unless `reverse` is set.
    pub async fn list(&self, opts: ConclusionListOptions) -> Result<Page<Conclusion>, Error> {
        let mut opts = opts;
        if self.filters.is_some() && opts.filters.is_none() {
            opts.filters = self.filters.clone();
        }
        conclusions_api::list_conclusions(&self.client, &opts).await
    }

    /// Semantic search over conclusions.
    pub async fn query(
        &self,
        query: impl Into<String>,
        top_k: Option<u32>,
    ) -> Result<Vec<Conclusion>, Error> {
        let mut opts = ConclusionQuery::new(query);
        opts.top_k = top_k;
        if self.filters.is_some() && opts.filters.is_none() {
            opts.filters = self.filters.clone();
        }
        conclusions_api::query_conclusions(&self.client, &opts).await
    }

    /// Fetch a single conclusion by ID.
    pub async fn get(&self, conclusion_id: &str) -> Result<Conclusion, Error> {
        conclusions_api::get_conclusion(&self.client, conclusion_id).await
    }

    /// Delete a single conclusion by ID.
    pub async fn delete(&self, conclusion_id: &str) -> Result<(), Error> {
        conclusions_api::delete_conclusion(&self.client, conclusion_id).await
    }
}
