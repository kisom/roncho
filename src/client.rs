use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use url::Url;

use crate::error::{Error, parse_api_error};
use crate::models::page::ListOptions;

const DEFAULT_BASE_URL: &str = "https://api.honcho.dev";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_RETRIES: usize = 3;
const USER_AGENT_VALUE: &str = concat!("roncho/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct Honcho {
    pub(crate) workspace_id: String,
    pub(crate) api_key: String,
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
            .field("max_retries", &self.max_retries)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Default for Honcho {
    fn default() -> Self {
        Self::new().expect("valid default config")
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
        let auth_value = format!("Bearer {}", self.api_key);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value)
                .map_err(|e| Error::Configuration(e.to_string()))?,
        );
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
            .http
            .get(url)
            .headers(headers)
            .query(query)
            .timeout(self.timeout)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(parse_api_error(status, &text));
        }

        resp.json::<T>().await.map_err(|e| Error::Decode(e.to_string()))
    }

    pub(crate) async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;

        let resp = self
            .http
            .post(url)
            .headers(headers)
            .json(body)
            .timeout(self.timeout)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(parse_api_error(status, &text));
        }

        resp.json::<T>().await.map_err(|e| Error::Decode(e.to_string()))
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
            .http
            .post(url)
            .headers(headers)
            .query(query)
            .json(body)
            .timeout(self.timeout)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(parse_api_error(status, &text));
        }

        resp.json::<T>().await.map_err(|e| Error::Decode(e.to_string()))
    }

    pub(crate) async fn delete_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, Error> {
        let url = self.url(path)?;
        let headers = self.headers()?;

        let resp = self
            .http
            .delete(url)
            .headers(headers)
            .timeout(self.timeout)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(parse_api_error(status, &text));
        }

        let len = resp.content_length();
        if len == Some(0) || len.is_none() {
            return Ok(serde_json::from_str("{}").unwrap());
        }

        resp.json::<T>().await.map_err(|e| Error::Decode(e.to_string()))
    }
}

pub struct HonchoBuilder {
    workspace_id: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    max_retries: Option<usize>,
    timeout: Option<Duration>,
}

impl Default for HonchoBuilder {
    fn default() -> Self {
        Self {
            workspace_id: None,
            api_key: None,
            base_url: None,
            max_retries: None,
            timeout: None,
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

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<Honcho, Error> {
        let workspace_id = self
            .workspace_id
            .or_else(|| std::env::var("HONCHO_WORKSPACE_ID").ok())
            .ok_or(Error::MissingApiKey)?;

        let api_key = self
            .api_key
            .or_else(|| std::env::var("HONCHO_API_KEY").ok())
            .ok_or(Error::MissingApiKey)?;

        let base_url_str = self
            .base_url
            .or_else(|| std::env::var("HONCHO_BASE_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let base_url = Url::parse(&base_url_str)
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;

        let timeout = self.timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        let max_retries = self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
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
