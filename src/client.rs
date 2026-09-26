use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use url::Url;

use crate::error::{parse_api_error, Error};
use crate::models::page::ListOptions;

const DEFAULT_BASE_URL: &str = "https://api.honcho.dev";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_RETRIES: usize = 3;
const USER_AGENT_VALUE: &str = concat!("roncho/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct Honcho {
    pub(crate) workspace_id: String,
    pub(crate) api_key: Option<zeroize::Zeroizing<String>>,
    pub(crate) base_url: Url,
    pub(crate) http: reqwest::Client,
    pub(crate) max_retries: usize,
    pub(crate) timeout: Duration,
}

impl std::fmt::Debug for Honcho {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Honcho")
            .field("workspace_id", &self.workspace_id)
            .field("base_url", &self.base_url)
            .field("api_key_present", &self.api_key.is_some())
            .field("max_retries", &self.max_retries)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Honcho {
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    pub fn builder() -> HonchoBuilder {
        HonchoBuilder::default()
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn base_url(&self) -> &url::Url {
        &self.base_url
    }

    pub fn url(&self, path: &str) -> Result<Url, Error> {
        let full_path = format!("/v3/workspaces/{}/{}", self.workspace_id, path);
        self.base_url
            .join(&full_path)
            .map_err(|e| Error::InvalidUrl(e.to_string()))
    }

    pub(crate) fn headers(&self) -> Result<HeaderMap, Error> {
        let mut headers = HeaderMap::new();
        if let Some(key) = &self.api_key {
            let auth_value = format!("Bearer {}", key.as_str());
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value).map_err(|_| {
                    Error::Configuration("api key is not a valid header value".into())
                })?,
            );
        }
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        Ok(headers)
    }

    pub(crate) fn list_query_params(&self, opts: &ListOptions) -> Vec<(&str, String)> {
        let mut params = Vec::new();
        if let Some(page) = opts.page {
            params.push(("page", page.to_string()));
        }
        if let Some(size) = opts.size {
            params.push(("size", size.to_string()));
        }
        if let Some(reverse) = opts.reverse {
            params.push(("reverse", reverse.to_string()));
        }
        params
    }

    pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;
        let resp = self
            .send(
                self.http
                    .get(url)
                    .headers(headers)
                    .query(query)
                    .timeout(self.timeout),
            )
            .await?;
        decode_json(resp).await
    }

    pub(crate) async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        self.post_json_query(path, &[], body).await
    }

    pub(crate) async fn post_json_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;
        let resp = self
            .send(
                self.http
                    .post(url)
                    .headers(headers)
                    .query(query)
                    .json(body)
                    .timeout(self.timeout),
            )
            .await?;
        decode_json(resp).await
    }

    pub(crate) async fn post_json_empty(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(), Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;
        let resp = self
            .send(
                self.http
                    .post(url)
                    .headers(headers)
                    .json(body)
                    .timeout(self.timeout),
            )
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(parse_api_error(status.as_u16(), &text));
        }
        Ok(())
    }

    pub(crate) async fn post_bytes<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let mut headers = self.headers()?;
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(content_type)
                .map_err(|_| Error::Configuration("content type is not a valid header".into()))?,
        );
        let resp = self
            .send(
                self.http
                    .post(url)
                    .headers(headers)
                    .body(body)
                    .timeout(self.timeout),
            )
            .await?;
        decode_json(resp).await
    }

    /// POST with a query string and no body. Clone uses this.
    pub(crate) async fn post_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;
        let resp = self
            .send(
                self.http
                    .post(url)
                    .headers(headers)
                    .query(query)
                    .timeout(self.timeout),
            )
            .await?;
        decode_json(resp).await
    }

    pub(crate) async fn put_json_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;
        let resp = self
            .send(
                self.http
                    .put(url)
                    .headers(headers)
                    .query(query)
                    .json(body)
                    .timeout(self.timeout),
            )
            .await?;
        decode_json(resp).await
    }

    pub(crate) async fn delete_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, Error> {
        self.delete_json_body(path, None).await
    }

    pub(crate) async fn delete_json_body<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;
        let mut req = self.http.delete(url).headers(headers).timeout(self.timeout);
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = self.send(req).await?;
        decode_json(resp).await
    }

    /// Streaming chat can outlive a normal JSON call. The configured timeout
    /// still applies, but never shorter than five minutes.
    pub(crate) fn stream_timeout(&self) -> Duration {
        self.timeout.max(Duration::from_secs(300))
    }

    pub(crate) async fn send(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, Error> {
        let req = builder.build()?;
        self.send_retry(req).await
    }

    /// Retry policy: `max_retries` (default 3) extra attempts.
    /// GET, PUT, DELETE, and HEAD retry on HTTP 429 and 5xx.
    /// A connect failure retries for every method, including POST.
    /// Other methods do not retry an HTTP status. Delay is 200ms, then
    /// doubled each attempt, and stops doubling after the fifth.
    async fn send_retry(&self, req: reqwest::Request) -> Result<reqwest::Response, Error> {
        let idempotent = matches!(
            *req.method(),
            reqwest::Method::GET
                | reqwest::Method::PUT
                | reqwest::Method::DELETE
                | reqwest::Method::HEAD
        );
        let mut attempt = 0usize;
        let mut current = req;
        loop {
            let next = current.try_clone();
            match self.http.execute(current).await {
                Ok(resp)
                    if idempotent
                        && attempt < self.max_retries
                        && (resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                            || resp.status().is_server_error()) =>
                {
                    let Some(cloned) = next else {
                        return Ok(resp);
                    };
                    let _ = resp.bytes().await;
                    attempt += 1;
                    tokio::time::sleep(retry_delay(attempt)).await;
                    current = cloned;
                }
                Ok(resp) => return Ok(resp),
                Err(err) if err.is_connect() && attempt < self.max_retries => {
                    let Some(cloned) = next else {
                        return Err(err.into());
                    };
                    attempt += 1;
                    tokio::time::sleep(retry_delay(attempt)).await;
                    current = cloned;
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

async fn decode_json<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Result<T, Error> {
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(parse_api_error(status.as_u16(), &text));
    }
    let bytes = resp.bytes().await.map_err(Error::Http)?;
    if bytes.is_empty() {
        return serde_json::from_slice(b"{}").map_err(|e| Error::Decode(e.to_string()));
    }
    serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(200u64.saturating_mul(1u64 << (attempt.min(5) - 1)))
}

pub struct HonchoBuilder {
    workspace_id: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    max_retries: Option<usize>,
    timeout: Option<Duration>,
    read_config: bool,
    config_path: Option<std::path::PathBuf>,
}

impl Default for HonchoBuilder {
    fn default() -> Self {
        Self {
            workspace_id: None,
            api_key: None,
            base_url: None,
            max_retries: None,
            timeout: None,
            read_config: true,
            config_path: None,
        }
    }
}

impl HonchoBuilder {
    pub fn workspace_id(mut self, id: impl Into<String>) -> Self {
        self.workspace_id = Some(id.into());
        self
    }

    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn max_retries(mut self, retries: usize) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Deadline for one JSON call, including the response body.
    ///
    /// Streaming chat uses this value, or five minutes, whichever is longer,
    /// because a high reasoning level often outlasts a typical request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn from_file(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self.read_config = true;
        self
    }

    pub fn without_config_file(mut self) -> Self {
        self.read_config = false;
        self.config_path = None;
        self
    }

    pub fn build(self) -> Result<Honcho, Error> {
        let mut api_key = self.api_key.clone();
        let mut workspace_id = self.workspace_id.clone();
        let mut base_url = self.base_url.clone();
        if self.read_config && (api_key.is_none() || workspace_id.is_none() || base_url.is_none()) {
            let file = if let Some(path) = &self.config_path {
                Some(crate::config::load_path(path).map_err(Error::Configuration)?)
            } else {
                crate::config::load_default().map_err(Error::Configuration)?
            };
            if let Some(file) = file {
                if api_key.is_none() {
                    api_key = file.api_key;
                }
                if workspace_id.is_none() {
                    workspace_id = file.workspace_id.filter(|id| !id.is_empty());
                }
                if base_url.is_none() {
                    base_url = file.base_url.filter(|url| !url.is_empty());
                }
            }
        }
        let api_key = match api_key.or_else(|| std::env::var("HONCHO_API_KEY").ok()) {
            Some(key) if key.is_empty() => {
                return Err(Error::Configuration(
                    "api key is empty; omit it to send no Authorization header".into(),
                ));
            }
            Some(key) => Some(zeroize::Zeroizing::new(key)),
            None => None,
        };

        let workspace_id = workspace_id
            .or_else(|| std::env::var("HONCHO_WORKSPACE_ID").ok())
            .ok_or(Error::MissingWorkspaceId)?;

        let base_url_str = base_url
            .or_else(|| std::env::var("HONCHO_BASE_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let base_url = Url::parse(&base_url_str).map_err(|e| Error::InvalidUrl(e.to_string()))?;

        let timeout = self
            .timeout
            .unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        let max_retries = self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(10))
            // Per-request timeouts override this. The floor keeps a stream
            // alive when the JSON deadline is shorter than a long chat.
            .timeout(timeout.max(Duration::from_secs(300)))
            .build()
            .map_err(Error::Http)?;

        Ok(Honcho {
            workspace_id,
            api_key,
            base_url,
            http,
            max_retries,
            timeout,
        })
    }
}
