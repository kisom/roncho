use std::pin::Pin;

use async_stream::stream;
use futures::Stream;
use serde_json::json;

use crate::client::Honcho;
use crate::error::{parse_api_error, Error};
use crate::models::chat::{ChatResponse, DialecticOptions, ReasoningLevel, StreamChunk};
use crate::models::message::Message;
use crate::models::page::{ListOptions, Page};
use crate::models::workspace::{Workspace, WorkspaceCreate, WorkspaceListOptions, WorkspaceUpdate};

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

    let resp = req.timeout(client.timeout).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(parse_api_error(status, &text));
    }

    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        // 204 No Content (e.g. delete) — decode as an empty object so callers
        // can ignore the response body.
        return serde_json::from_str::<T>("{}").map_err(|e| Error::Decode(e.to_string()));
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

pub async fn get_workspace(client: &Honcho, workspace_id: &str) -> Result<Workspace, Error> {
    request_json(client, workspace_id, reqwest::Method::GET, None, None).await
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
    request_json::<serde_json::Value>(client, workspace_id, reqwest::Method::DELETE, None, None)
        .await?;
    Ok(())
}

pub async fn search_workspace(client: &Honcho, query: &str) -> Result<Page<Message>, Error> {
    let body = json!({ "query": query });
    client.post_json("search", &body).await
}

pub async fn chat_workspace(
    client: &Honcho,
    opts: &DialecticOptions,
) -> Result<ChatResponse, Error> {
    let body = serde_json::json!({
        "query": opts.query,
        "session_id": opts.session_id,
        "filters": opts.filters,
        "target": opts.target,
        "scope": opts.scope,
        "stream": opts.stream.unwrap_or(false),
        "reasoning_level": opts.reasoning_level.map(|l| match l {
            ReasoningLevel::Minimal => "minimal",
            ReasoningLevel::Low => "low",
            ReasoningLevel::Medium => "medium",
            ReasoningLevel::High => "high",
            ReasoningLevel::Max => "max",
        }),
        "response_format": opts.response_format,
        "include_evidence": opts.include_evidence,
    });

    client.post_json("chat", &body).await
}

pub fn chat_workspace_stream(
    client: Honcho,
    opts: DialecticOptions,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, Error>> + Send>> {
    let url = match client.url("chat") {
        Ok(u) => u,
        Err(e) => {
            return Box::pin(stream! {
                yield Err(e);
            });
        }
    };

    let headers = match client.headers() {
        Ok(h) => h,
        Err(e) => {
            return Box::pin(stream! {
                yield Err(e);
            });
        }
    };

    let body = serde_json::json!({
        "query": opts.query,
        "session_id": opts.session_id,
        "filters": opts.filters,
        "target": opts.target,
        "scope": opts.scope,
        "stream": true,
        "reasoning_level": opts.reasoning_level.map(|l| match l {
            ReasoningLevel::Minimal => "minimal",
            ReasoningLevel::Low => "low",
            ReasoningLevel::Medium => "medium",
            ReasoningLevel::High => "high",
            ReasoningLevel::Max => "max",
        }),
        "response_format": opts.response_format,
        "include_evidence": opts.include_evidence,
    });

    let send_future = client.http.post(url).headers(headers).json(&body).send();

    Box::pin(stream!({
        let resp = match send_future.await {
            Ok(r) => r,
            Err(e) => {
                yield Err(Error::Http(e));
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            yield Err(crate::error::parse_api_error(status, &text));
            return;
        }

        let mut bytes = std::pin::pin!(resp.bytes_stream());
        use futures::StreamExt;

        let mut event_data: Option<String> = None;

        while let Some(chunk_result) = bytes.next().await {
            let chunk = match chunk_result {
                Ok(b) => b,
                Err(e) => {
                    yield Err(Error::Http(e));
                    return;
                }
            };

            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.is_empty() {
                    if let Some(data) = event_data.take() {
                        if !data.is_empty() {
                            yield Ok(StreamChunk { content: data });
                        }
                    }
                    continue;
                }

                if line.starts_with(":") {
                    continue;
                }

                let parts: Vec<&str> = line.splitn(2, ": ").collect();
                let field = parts.first().copied().unwrap_or("");
                let value = parts.get(1).copied().unwrap_or("").trim_start();

                match field {
                    "data" => {
                        event_data = Some(value.to_string());
                    }
                    "error" => {
                        yield Err(Error::Stream(value.to_string()));
                        return;
                    }
                    _ => {}
                }
            }
        }

        if let Some(data) = event_data {
            if !data.is_empty() {
                yield Ok(StreamChunk { content: data });
            }
        }
    }))
}
