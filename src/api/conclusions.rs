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
    let count = batch.conclusions.len();
    if !(1..=100).contains(&count) {
        return Err(Error::Configuration(format!(
            "conclusion batch must contain 1 to 100 items, got {count}"
        )));
    }
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
    require_conclusion_parties(opts)?;
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

fn require_conclusion_parties(query: &ConclusionQuery) -> Result<(), Error> {
    let Some(filters) = &query.filters else {
        return Err(Error::Configuration(
            "conclusion query requires observer and observed filters".into(),
        ));
    };
    let observer = filters.contains_key("observer_id") || filters.contains_key("observer");
    let observed = filters.contains_key("observed_id") || filters.contains_key("observed");
    if observer && observed {
        Ok(())
    } else {
        Err(Error::Configuration(
            "conclusion query requires observer and observed filters (observer_id/observed_id or observer/observed)".into(),
        ))
    }
}
