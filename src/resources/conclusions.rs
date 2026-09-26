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
/// `peer.conclusions()` / `peer.conclusions_of()`. Scoped filters are merged
/// into list and query calls and are not replaced by caller filters.
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
        opts.filters = merge_scope(self.filters.as_ref(), opts.filters.as_ref());
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
        opts.filters = merge_scope(self.filters.as_ref(), opts.filters.as_ref());
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

/// Scope keys win. A caller filter cannot widen the view by replacing them.
fn merge_scope(
    scope: Option<&serde_json::Map<String, serde_json::Value>>,
    caller: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match (scope, caller) {
        (None, None) => None,
        (Some(scope), None) => Some(scope.clone()),
        (None, Some(caller)) => Some(caller.clone()),
        (Some(scope), Some(caller)) => {
            let mut merged = caller.clone();
            for (key, value) in scope {
                merged.insert(key.clone(), value.clone());
            }
            Some(merged)
        }
    }
}
