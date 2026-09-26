use crate::client::Honcho;
use crate::error::Error;
use crate::models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionListOptions, ConclusionQuery,
};
use crate::models::page::{ListOptions, Page};

pub async fn create_conclusions(
    client: &Honcho,
    batch: &ConclusionBatchCreate,
) -> Result<Vec<Conclusion>, Error> {
    let body = serde_json::to_value(batch).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json("conclusions", &body).await
}

pub async fn list_conclusions(
    client: &Honcho,
    opts: &ConclusionListOptions,
) -> Result<Page<Conclusion>, Error> {
    let list_opts = ListOptions {
        page: opts.page,
        size: opts.size,
        reverse: opts.reverse,
    };
    let query = client.list_query_params(&list_opts);
    let filters = opts.filters.clone().unwrap_or_default();
    let body = serde_json::json!({ "filters": filters });
    client
        .post_json_query("conclusions/list", &query, &body)
        .await
}

pub async fn query_conclusions(
    client: &Honcho,
    opts: &ConclusionQuery,
) -> Result<Vec<Conclusion>, Error> {
    let body = serde_json::to_value(opts).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json("conclusions/query", &body).await
}

pub async fn get_conclusion(client: &Honcho, conclusion_id: &str) -> Result<Conclusion, Error> {
    let path = format!("conclusions/{}", conclusion_id);
    client.get_json(&path, &[]).await
}

pub async fn delete_conclusion(client: &Honcho, conclusion_id: &str) -> Result<(), Error> {
    let path = format!("conclusions/{}", conclusion_id);
    client.delete_json::<serde_json::Value>(&path).await?;
    Ok(())
}
