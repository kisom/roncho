use serde_json::json;

use crate::client::Honcho;
use crate::error::Error;
use crate::models::page::{ListOptions, Page};
use crate::models::scope::{check_scope_id, Scope, ScopeBackfill, ScopeCreate, ScopeStatusBody};
use crate::models::session::Session;

pub async fn get_or_create_scope(client: &Honcho, create: &ScopeCreate) -> Result<Scope, Error> {
    check_scope_id(&create.id).map_err(Error::Configuration)?;
    let body = serde_json::to_value(create).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json("scopes", &body).await
}

pub async fn get_scope(client: &Honcho, scope_id: &str) -> Result<Scope, Error> {
    check_scope_id(scope_id).map_err(Error::Configuration)?;
    let path = format!("scopes/{scope_id}");
    client.get_json(&path, &[]).await
}

pub async fn list_scopes(client: &Honcho, opts: &ListOptions) -> Result<Page<Scope>, Error> {
    let query = client.list_query_params(opts);
    client
        .post_json_query("scopes/list", &query, &json!({}))
        .await
}

pub async fn add_scope_sessions(
    client: &Honcho,
    scope_id: &str,
    session_ids: &[String],
) -> Result<(), Error> {
    check_scope_id(scope_id).map_err(Error::Configuration)?;
    let count = session_ids.len();
    if !(1..=100).contains(&count) {
        return Err(Error::Configuration(format!(
            "a scope membership call takes 1 to 100 session ids, got {count}"
        )));
    }
    let path = format!("scopes/{scope_id}/sessions");
    let body = json!({ "session_ids": session_ids });
    client.post_json_empty(&path, &body).await
}

pub async fn list_scope_sessions(
    client: &Honcho,
    scope_id: &str,
    opts: &ListOptions,
) -> Result<Page<Session>, Error> {
    check_scope_id(scope_id).map_err(Error::Configuration)?;
    let query = client.list_query_params(opts);
    let path = format!("scopes/{scope_id}/sessions/list");
    client.post_json_query(&path, &query, &json!({})).await
}

pub async fn remove_scope_session(
    client: &Honcho,
    scope_id: &str,
    session_id: &str,
) -> Result<(), Error> {
    check_scope_id(scope_id).map_err(Error::Configuration)?;
    let path = format!("scopes/{scope_id}/sessions/{session_id}");
    client.delete_json::<serde_json::Value>(&path).await?;
    Ok(())
}

pub async fn scope_status(
    client: &Honcho,
    scope_id: &str,
) -> Result<std::collections::HashMap<String, ScopeBackfill>, Error> {
    check_scope_id(scope_id).map_err(Error::Configuration)?;
    let path = format!("scopes/{scope_id}/status");
    let body: ScopeStatusBody = client.get_json(&path, &[]).await?;
    Ok(body.into_map())
}
