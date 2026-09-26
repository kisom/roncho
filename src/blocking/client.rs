use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::error::Error;
use super::http::{self, RawRequest};
use crate::models::chat::{ChatResponse, DialecticOptions, ReasoningLevel, ScopeNames};
use crate::models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionListOptions, ConclusionQuery,
};
use crate::models::message::{Message, MessageCreate, MessageSearch};
use crate::models::page::{ListOptions, Page};
use crate::models::peer::{Peer, PeerCreate};
use crate::models::session::{Session, SessionCreate};
use crate::models::workspace::{Workspace, WorkspaceCreate};

const DEFAULT_CONNECT: Duration = Duration::from_secs(10);
const DEFAULT_READ: Duration = Duration::from_secs(120);
const DEFAULT_MAX_BODY: usize = 8 * 1024 * 1024;

/// One workspace on one Honcho origin. Cheap to clone. `Send + Sync`.
#[derive(Clone)]
pub struct Client {
    host: String,
    port: u16,
    host_header: String,
    workspace_id: String,
    api_key: Option<String>,
    connect_timeout: Duration,
    read_timeout: Duration,
    max_body: usize,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("workspace_id", &self.workspace_id)
            .field("api_key_present", &self.api_key.is_some())
            .field("connect_timeout", &self.connect_timeout)
            .field("read_timeout", &self.read_timeout)
            .field("max_body", &self.max_body)
            .finish()
    }
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    /// `POST /v3/workspaces`. Creates the workspace when the id is new.
    pub fn get_or_create_workspace(&self, create: &WorkspaceCreate) -> Result<Workspace, Error> {
        self.call("POST", "/v3/workspaces", None, Some(to_json(create)?))
    }

    /// `POST /v3/workspaces/{workspace}/peers`.
    pub fn peer(&self, create: &PeerCreate) -> Result<Peer, Error> {
        self.call("POST", &self.scoped("peers"), None, Some(to_json(create)?))
    }

    /// `POST /v3/workspaces/{workspace}/sessions`.
    pub fn session(&self, create: &SessionCreate) -> Result<Session, Error> {
        self.call(
            "POST",
            &self.scoped("sessions"),
            None,
            Some(to_json(create)?),
        )
    }

    /// `POST /v3/workspaces/{workspace}/sessions/{session}/messages`.
    pub fn add_messages(
        &self,
        session_id: &str,
        messages: &[MessageCreate],
    ) -> Result<Vec<Message>, Error> {
        let path = self.scoped(&format!("sessions/{}/messages", enc(session_id)?));
        self.call("POST", &path, None, Some(json!({ "messages": messages })))
    }

    /// `POST /v3/workspaces/{workspace}/sessions/{session}/messages/list`.
    pub fn list_messages(
        &self,
        session_id: &str,
        opts: &ListOptions,
        filters: Option<&serde_json::Map<String, Value>>,
    ) -> Result<Page<Message>, Error> {
        let path = self.scoped(&format!("sessions/{}/messages/list", enc(session_id)?));
        let body = match filters {
            Some(filters) => json!({ "filters": filters }),
            None => json!({}),
        };
        self.call("POST", &path, Some(&list_query(opts)), Some(body))
    }

    /// `DELETE /v3/workspaces/{workspace}/sessions/{session}`.
    pub fn delete_session(&self, session_id: &str) -> Result<(), Error> {
        let path = self.scoped(&format!("sessions/{}", enc(session_id)?));
        self.call_ignore("DELETE", &path, None, None)
    }

    /// `DELETE /v3/workspaces/{id}`. A 409 is [`Error::ActiveSessions`].
    pub fn delete_workspace(&self, workspace_id: &str) -> Result<(), Error> {
        let path = format!("/v3/workspaces/{}", enc(workspace_id)?);
        let response = self.raw("DELETE", &path, None, None)?;
        if response.status == 409 {
            return Err(Error::ActiveSessions {
                body: http::body_prefix(&response.body),
            });
        }
        expect_success(response.status, &response.body)?;
        Ok(())
    }

    /// `POST /v3/workspaces/{workspace}/conclusions`.
    pub fn create_conclusions(
        &self,
        batch: &ConclusionBatchCreate,
    ) -> Result<Vec<Conclusion>, Error> {
        self.call(
            "POST",
            &self.scoped("conclusions"),
            None,
            Some(to_json(batch)?),
        )
    }

    /// `POST /v3/workspaces/{workspace}/conclusions/list`.
    pub fn list_conclusions(
        &self,
        opts: &ConclusionListOptions,
    ) -> Result<Page<Conclusion>, Error> {
        let list = ListOptions {
            page: opts.page,
            size: opts.size,
            reverse: opts.reverse,
        };
        let body = match &opts.filters {
            Some(filters) => json!({ "filters": filters }),
            None => json!({}),
        };
        self.call(
            "POST",
            &self.scoped("conclusions/list"),
            Some(&list_query(&list)),
            Some(body),
        )
    }

    /// `POST /v3/workspaces/{workspace}/conclusions/query`.
    pub fn query_conclusions(&self, query: &ConclusionQuery) -> Result<Vec<Conclusion>, Error> {
        self.call(
            "POST",
            &self.scoped("conclusions/query"),
            None,
            Some(to_json(query)?),
        )
    }

    /// `DELETE /v3/workspaces/{workspace}/conclusions/{id}`.
    pub fn delete_conclusion(&self, conclusion_id: &str) -> Result<(), Error> {
        let path = self.scoped(&format!("conclusions/{}", enc(conclusion_id)?));
        self.call_ignore("DELETE", &path, None, None)
    }

    /// `POST /v3/workspaces/{workspace}/search`. Body is `{"query","limit"?}`.
    pub fn search_workspace(&self, search: &MessageSearch) -> Result<Vec<Message>, Error> {
        self.call(
            "POST",
            &self.scoped("search"),
            None,
            Some(search_body(search)),
        )
    }

    /// `POST /v3/workspaces/{workspace}/peers/{peer}/search`.
    pub fn search_peer(
        &self,
        peer_id: &str,
        search: &MessageSearch,
    ) -> Result<Vec<Message>, Error> {
        let path = self.scoped(&format!("peers/{}/search", enc(peer_id)?));
        self.call("POST", &path, None, Some(search_body(search)))
    }

    /// `POST /v3/workspaces/{workspace}/sessions/{session}/search`.
    pub fn search_session(
        &self,
        session_id: &str,
        search: &MessageSearch,
    ) -> Result<Vec<Message>, Error> {
        let path = self.scoped(&format!("sessions/{}/search", enc(session_id)?));
        self.call("POST", &path, None, Some(search_body(search)))
    }

    /// `POST /v3/workspaces/{workspace}/peers/{peer}/chat`. Not streamed.
    pub fn peer_chat(&self, peer_id: &str, opts: &DialecticOptions) -> Result<ChatResponse, Error> {
        let path = self.scoped(&format!("peers/{}/chat", enc(peer_id)?));
        self.call("POST", &path, None, Some(chat_body(opts, true)))
    }

    /// `POST /v3/workspaces/{workspace}/chat`. No `target` or `filters`.
    pub fn workspace_chat(&self, opts: &DialecticOptions) -> Result<ChatResponse, Error> {
        if opts.target.is_some() || opts.filters.is_some() {
            return Err(Error::Config(
                "workspace chat has no target or filters".into(),
            ));
        }
        self.call(
            "POST",
            &self.scoped("chat"),
            None,
            Some(chat_body(opts, false)),
        )
    }

    /// `GET /health` on the configured origin. Honcho 3.2.1 exposes this and no version field.
    pub fn probe(&self) -> Result<Probe, Error> {
        let response = self.raw("GET", "/health", None, None)?;
        expect_success(response.status, &response.body)?;
        let body = if response.body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&response.body).unwrap_or(Value::Null)
        };
        Ok(Probe {
            status: response.status,
            body,
        })
    }

    fn scoped(&self, suffix: &str) -> String {
        format!("/v3/workspaces/{}/{}", enc_ok(&self.workspace_id), suffix)
    }

    fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        query: Option<&[(&str, String)]>,
        body: Option<Value>,
    ) -> Result<T, Error> {
        let response = self.raw(method, path, query, body)?;
        expect_success(response.status, &response.body)?;
        if response.body.is_empty() {
            return serde_json::from_str("{}").map_err(|err| Error::Decode(err.to_string()));
        }
        serde_json::from_slice(&response.body).map_err(|err| {
            let preview = http::body_prefix(&response.body);
            Error::Decode(format!("{err}; body: {preview}"))
        })
    }

    fn call_ignore(
        &self,
        method: &str,
        path: &str,
        query: Option<&[(&str, String)]>,
        body: Option<Value>,
    ) -> Result<(), Error> {
        let response = self.raw(method, path, query, body)?;
        expect_success(response.status, &response.body)?;
        Ok(())
    }

    fn raw(
        &self,
        method: &str,
        path: &str,
        query: Option<&[(&str, String)]>,
        body: Option<Value>,
    ) -> Result<http::RawResponse, Error> {
        let path = match query {
            Some(query) if !query.is_empty() => format!("{path}?{}", encode_query(query)),
            _ => path.to_string(),
        };
        let owned = body.map(|value| value.to_string().into_bytes());
        http::exchange(
            &self.host,
            self.port,
            self.connect_timeout,
            self.read_timeout,
            self.max_body,
            &RawRequest {
                method,
                path: &path,
                host_header: &self.host_header,
                authorization: self.api_key.as_deref(),
                body: owned.as_deref(),
            },
        )
    }
}

/// Result of [`Client::probe`].
#[derive(Debug, Clone)]
pub struct Probe {
    pub status: u16,
    pub body: Value,
}

#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: Option<String>,
    workspace_id: Option<String>,
    api_key: Option<String>,
    connect_timeout: Duration,
    read_timeout: Duration,
    max_body: usize,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: None,
            workspace_id: None,
            api_key: None,
            connect_timeout: DEFAULT_CONNECT,
            read_timeout: DEFAULT_READ,
            max_body: DEFAULT_MAX_BODY,
        }
    }
}

impl ClientBuilder {
    /// `http://host` or `http://host:port`. Required. `https://` is rejected.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn workspace_id(mut self, id: impl Into<String>) -> Self {
        self.workspace_id = Some(id.into());
        self
    }

    /// Sent as `Authorization: Bearer` when set. Omitted entirely when unset.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// How long a read may go with no bytes. Not a deadline for the whole response.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    pub fn max_body(mut self, bytes: usize) -> Self {
        self.max_body = bytes;
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        let base = self
            .base_url
            .ok_or_else(|| Error::Config("base URL is required".into()))?;
        let workspace_id = self
            .workspace_id
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Error::Config("workspace id is required".into()))?;
        let origin = parse_origin(&base)?;
        if self.max_body == 0 {
            return Err(Error::Config("max body must be greater than zero".into()));
        }
        Ok(Client {
            host: origin.host,
            port: origin.port,
            host_header: origin.host_header,
            workspace_id,
            api_key: self.api_key.filter(|key| !key.is_empty()),
            connect_timeout: self.connect_timeout,
            read_timeout: self.read_timeout,
            max_body: self.max_body,
        })
    }
}

struct Origin {
    host: String,
    port: u16,
    host_header: String,
}

fn parse_origin(raw: &str) -> Result<Origin, Error> {
    let rest = raw
        .strip_prefix("http://")
        .ok_or_else(|| Error::Config("base URL must be http:// (no TLS in this build)".into()))?;
    if rest.is_empty() || rest.contains('@') || rest.contains('?') || rest.contains('#') {
        return Err(Error::Config("base URL must be an http origin".into()));
    }
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (rest, ""),
    };
    if !path.is_empty() {
        return Err(Error::Config("base URL must not include a path".into()));
    }
    let (host, port) = if let Some(host) = authority.strip_prefix('[') {
        let (host, port) = host
            .split_once(']')
            .ok_or_else(|| Error::Config("bad IPv6 host".into()))?;
        let port = match port {
            "" => 80,
            rest => rest
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .ok_or_else(|| Error::Config("bad port".into()))?,
        };
        (host.to_string(), port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.is_empty() {
            return Err(Error::Config("base URL is missing a host".into()));
        }
        let port = port.parse().map_err(|_| Error::Config("bad port".into()))?;
        (host.to_string(), port)
    } else {
        (authority.to_string(), 80)
    };
    if host.is_empty() {
        return Err(Error::Config("base URL is missing a host".into()));
    }
    let host_header = if host.contains(':') {
        format!("[{host}]:{port}")
    } else if port == 80 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    Ok(Origin {
        host,
        port,
        host_header,
    })
}

fn expect_success(status: u16, body: &[u8]) -> Result<(), Error> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Status {
            status,
            body: http::body_prefix(body),
        })
    }
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(|err| Error::Decode(err.to_string()))
}

fn list_query(opts: &ListOptions) -> Vec<(&str, String)> {
    let mut query = Vec::new();
    if let Some(page) = opts.page {
        query.push(("page", page.to_string()));
    }
    if let Some(size) = opts.size {
        query.push(("size", size.to_string()));
    }
    if let Some(reverse) = opts.reverse {
        query.push(("reverse", reverse.to_string()));
    }
    query
}

fn search_body(search: &MessageSearch) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("query".into(), json!(search.query));
    if let Some(limit) = search.limit {
        body.insert("limit".into(), json!(limit));
    }
    if let Some(filters) = &search.filters {
        body.insert("filters".into(), Value::Object(filters.clone()));
    }
    Value::Object(body)
}

fn chat_body(opts: &DialecticOptions, peer: bool) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("query".into(), json!(opts.query));
    if let Some(session) = &opts.session_id {
        body.insert("session_id".into(), json!(session));
    }
    if let Some(level) = opts.reasoning_level {
        body.insert("reasoning_level".into(), json!(level_name(level)));
    }
    if let Some(include) = opts.include_evidence {
        body.insert("include_evidence".into(), json!(include));
    }
    if let Some(scope) = &opts.scope {
        let value = match scope {
            ScopeNames::One(name) => json!(name),
            ScopeNames::Many(names) => json!(names),
        };
        body.insert("scope".into(), value);
    }
    if peer {
        if let Some(target) = &opts.target {
            body.insert("target".into(), json!(target));
        }
        if let Some(filters) = &opts.filters {
            body.insert("filters".into(), Value::Object(filters.clone()));
        }
    }
    Value::Object(body)
}

fn level_name(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Minimal => "minimal",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
        ReasoningLevel::Max => "max",
    }
}

fn enc(segment: &str) -> Result<String, Error> {
    if segment.is_empty() {
        return Err(Error::Config("empty path segment".into()));
    }
    Ok(encode_path(segment))
}

fn enc_ok(segment: &str) -> String {
    encode_path(segment)
}

fn encode_path(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn encode_query(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode_path(key), encode_path(value)))
        .collect::<Vec<_>>()
        .join("&")
}
