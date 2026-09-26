use std::pin::Pin;

use futures::Stream;

use crate::api::peers::{self, workspace_chat_body};
use crate::api::sse;
use crate::client::Honcho;
use crate::error::{parse_api_error, Error};
use crate::models::chat::{ChatResponse, DialecticOptions, StreamChunk};
use crate::models::message::{Message, MessageSearch};
use crate::models::page::{ListOptions, Page};
use crate::models::scope::ScheduleDream;
use crate::models::workspace::{
    QueueStatus, QueueStatusQuery, Workspace, WorkspaceCreate, WorkspaceListOptions,
    WorkspaceUpdate,
};

/// Build a URL rooted at `/v3/workspaces` (not scoped to a workspace).
///
/// The Workspaces API endpoints are not workspace-scoped: the workspace is
/// resolved from the JWT rather than a `{ws}` path segment, so requests target
/// `/v3/workspaces`, `/v3/workspaces/list`, and `/v3/workspaces/{id}`.
fn root_url(client: &Honcho, path: &str) -> Result<url::Url, Error> {
    let full_path = if path.is_empty() {
        "/v3/workspaces".to_string()
    } else {
        format!("/v3/workspaces/{}", path)
    };
    client
        .base_url()
        .join(&full_path)
        .map_err(|e| Error::InvalidUrl(e.to_string()))
}

/// Send a JSON request against a root-scoped Workspaces API path.
async fn request_json<T: serde::de::DeserializeOwned>(
    client: &Honcho,
    path: &str,
    method: reqwest::Method,
    query: Option<&[(&str, String)]>,
    body: Option<&serde_json::Value>,
) -> Result<T, Error> {
    let url = root_url(client, path)?;
    let headers = client.headers()?;

    let mut req = client.http.request(method, url).headers(headers);
    if let Some(body) = body {
        req = req.json(body);
    }
    if let Some(query) = query {
        req = req.query(query);
    }

    let resp = client.send(req.timeout(client.timeout)).await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(parse_api_error(status.as_u16(), &text));
    }

    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        return serde_json::from_slice::<T>(b"{}").map_err(|e| Error::Decode(e.to_string()));
    }

    serde_json::from_slice::<T>(&bytes).map_err(|e| Error::Decode(e.to_string()))
}

pub async fn get_or_create_workspace(
    client: &Honcho,
    create: &WorkspaceCreate,
) -> Result<Workspace, Error> {
    let body = serde_json::to_value(create).map_err(|e| Error::Encode(e.to_string()))?;
    request_json(client, "", reqwest::Method::POST, None, Some(&body)).await
}

pub async fn list_workspaces(
    client: &Honcho,
    opts: &WorkspaceListOptions,
) -> Result<Page<Workspace>, Error> {
    let list_opts = ListOptions {
        page: opts.page,
        size: opts.size,
        reverse: opts.reverse,
    };
    let query = client.list_query_params(&list_opts);
    let body = serde_json::json!({});
    request_json(
        client,
        "list",
        reqwest::Method::POST,
        Some(&query),
        Some(&body),
    )
    .await
}

/// The API has no GET for a workspace. Reading is get-or-create on
/// `POST /v3/workspaces`, which creates the workspace when it is missing.
pub async fn get_workspace(client: &Honcho, workspace_id: &str) -> Result<Workspace, Error> {
    let body = serde_json::json!({ "id": workspace_id });
    request_json(client, "", reqwest::Method::POST, None, Some(&body)).await
}

pub async fn update_workspace(
    client: &Honcho,
    workspace_id: &str,
    update: &WorkspaceUpdate,
) -> Result<Workspace, Error> {
    let body = serde_json::to_value(update).map_err(|e| Error::Encode(e.to_string()))?;
    request_json(
        client,
        workspace_id,
        reqwest::Method::PUT,
        None,
        Some(&body),
    )
    .await
}

pub async fn delete_workspace(client: &Honcho, workspace_id: &str) -> Result<(), Error> {
    let url = root_url(client, workspace_id)?;
    let headers = client.headers()?;
    let resp = client
        .send(
            client
                .http
                .delete(url)
                .headers(headers)
                .timeout(client.timeout),
        )
        .await?;
    let status = resp.status();
    if status.as_u16() == 409 {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::ActiveSessions { body });
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(parse_api_error(status.as_u16(), &text));
    }
    Ok(())
}

pub async fn queue_status(client: &Honcho, query: &QueueStatusQuery) -> Result<QueueStatus, Error> {
    let mut pairs = Vec::new();
    if let Some(id) = &query.observer_id {
        pairs.push(("observer_id", id.clone()));
    }
    if let Some(id) = &query.sender_id {
        pairs.push(("sender_id", id.clone()));
    }
    if let Some(id) = &query.session_id {
        pairs.push(("session_id", id.clone()));
    }
    client.get_json("queue/status", &pairs).await
}

pub async fn schedule_dream(client: &Honcho, dream: &ScheduleDream) -> Result<(), Error> {
    if dream.observer.is_empty() {
        return Err(Error::Configuration("dream observer is required".into()));
    }
    let body = serde_json::to_value(dream).map_err(|e| Error::Encode(e.to_string()))?;
    client.post_json_empty("schedule_dream", &body).await
}

pub async fn search_workspace(
    client: &Honcho,
    search: &MessageSearch,
) -> Result<Vec<Message>, Error> {
    client
        .post_json("search", &peers::search_body(search, true))
        .await
}

pub async fn chat_workspace(
    client: &Honcho,
    opts: &DialecticOptions,
) -> Result<ChatResponse, Error> {
    client.post_json("chat", &workspace_chat_body(opts)?).await
}

pub fn chat_workspace_stream(
    client: Honcho,
    opts: DialecticOptions,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, Error>> + Send>> {
    let body = match workspace_chat_body(&opts) {
        Ok(body) => body,
        Err(err) => return sse::failed(err),
    };
    let url = match client.url("chat") {
        Ok(url) => url,
        Err(err) => return sse::failed(err),
    };
    sse::open_chat_stream(client, url, body)
}
