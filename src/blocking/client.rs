use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::error::Error;
use super::http::{self, LiveBody, RawRequest};
use super::stream::{self, ChatStream};
use crate::models::chat::{ChatResponse, DialecticOptions, ReasoningLevel, ScopeNames};
use crate::models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionListOptions, ConclusionQuery,
};
use crate::models::message::{Message, MessageCreate, MessageSearch};
use crate::models::page::{ListOptions, Page};
use crate::models::peer::{Peer, PeerCreate};
use crate::models::session::{Session, SessionCreate};
use crate::models::workspace::{QueueStatus, QueueStatusQuery, Workspace, WorkspaceCreate};

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
    api_key: Option<zeroize::Zeroizing<String>>,
    connect_timeout: Duration,
    read_timeout: Duration,
    max_body: usize,
    https: bool,
    #[cfg(feature = "tls")]
    tls: Option<std::sync::Arc<rustls::ClientConfig>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("https", &self.https)
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
    /// A batch of 0 or more than 100 is refused before the request is sent.
    pub fn create_conclusions(
        &self,
        batch: &ConclusionBatchCreate,
    ) -> Result<Vec<Conclusion>, Error> {
        let count = batch.conclusions.len();
        if !(1..=100).contains(&count) {
            return Err(Error::Config(format!(
                "conclusion batch must contain 1 to 100 items, got {count}"
            )));
        }
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
    ///
    /// On the self-hosted server checked for 0.1.0, `filters` must name the observer and the observed peer
    /// (`observer_id`/`observed_id`, or `observer`/`observed`). Omitting them is a 422.
    pub fn query_conclusions(&self, query: &ConclusionQuery) -> Result<Vec<Conclusion>, Error> {
        require_conclusion_parties(query)?;
        self.call(
            "POST",
            &self.scoped("conclusions/query"),
            None,
            Some(to_json(query)?),
        )
    }

    /// `GET /v3/workspaces/{workspace}/queue/status`.
    pub fn queue_status(&self, query: &QueueStatusQuery) -> Result<QueueStatus, Error> {
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
        self.call("GET", &self.scoped("queue/status"), Some(&pairs), None)
    }

    /// `DELETE /v3/workspaces/{workspace}/conclusions/{id}`.
    pub fn delete_conclusion(&self, conclusion_id: &str) -> Result<(), Error> {
        let path = self.scoped(&format!("conclusions/{}", enc(conclusion_id)?));
        self.call_ignore("DELETE", &path, None, None)
    }

    /// `POST /v3/workspaces/{workspace}/search`. Body is `{"query","limit"?,"scope"?}`.
    pub fn search_workspace(&self, search: &MessageSearch) -> Result<Vec<Message>, Error> {
        self.call(
            "POST",
            &self.scoped("search"),
            None,
            Some(search_body(search, true)),
        )
    }

    /// `POST /v3/workspaces/{workspace}/peers/{peer}/search`.
    pub fn search_peer(
        &self,
        peer_id: &str,
        search: &MessageSearch,
    ) -> Result<Vec<Message>, Error> {
        let path = self.scoped(&format!("peers/{}/search", enc(peer_id)?));
        self.call("POST", &path, None, Some(search_body(search, false)))
    }

    /// `POST /v3/workspaces/{workspace}/sessions/{session}/search`.
    pub fn search_session(
        &self,
        session_id: &str,
        search: &MessageSearch,
    ) -> Result<Vec<Message>, Error> {
        let path = self.scoped(&format!("sessions/{}/search", enc(session_id)?));
        self.call("POST", &path, None, Some(search_body(search, false)))
    }

    /// `POST /v3/workspaces/{workspace}/peers/{peer}/chat` with `stream: true`.
    pub fn peer_chat_stream(
        &self,
        peer_id: &str,
        opts: &DialecticOptions,
    ) -> Result<ChatStream, Error> {
        let path = self.scoped(&format!("peers/{}/chat", enc(peer_id)?));
        self.open_chat(&path, chat_body(&stream::streaming_opts(opts), true))
    }

    /// `POST /v3/workspaces/{workspace}/chat` with `stream: true`.
    pub fn workspace_chat_stream(&self, opts: &DialecticOptions) -> Result<ChatStream, Error> {
        if opts.target.is_some() || opts.filters.is_some() {
            return Err(Error::Config(
                "workspace chat has no target or filters".into(),
            ));
        }
        self.open_chat(
            &self.scoped("chat"),
            chat_body(&stream::streaming_opts(opts), false),
        )
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

    fn open_chat(&self, path: &str, body: Value) -> Result<ChatStream, Error> {
        let owned = body.to_string().into_bytes();
        let live = self.open_live(&RawRequest {
            method: "POST",
            path,
            host_header: &self.host_header,
            authorization: self.authorization(),
            body: Some(&owned),
        })?;
        Ok(ChatStream::from_body(live))
    }

    fn open_live(&self, request: &RawRequest<'_>) -> Result<LiveBody, Error> {
        http::open(&self.endpoint(), request)
    }

    fn endpoint(&self) -> http::Endpoint<'_> {
        http::Endpoint {
            host: &self.host,
            port: self.port,
            connect_timeout: self.connect_timeout,
            read_timeout: self.read_timeout,
            max_body: self.max_body,
            #[cfg(feature = "tls")]
            tls: self.tls.as_ref(),
        }
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
            &self.endpoint(),
            &RawRequest {
                method,
                path: &path,
                host_header: &self.host_header,
                authorization: self.authorization(),
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

#[derive(Clone)]
pub struct ClientBuilder {
    base_url: Option<String>,
    workspace_id: Option<String>,
    api_key: Option<String>,
    connect_timeout: Duration,
    read_timeout: Duration,
    max_body: usize,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field("workspace_id", &self.workspace_id)
            .field("api_key_present", &self.api_key.is_some())
            .field("connect_timeout", &self.connect_timeout)
            .field("read_timeout", &self.read_timeout)
            .field("max_body", &self.max_body)
            .finish()
    }
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
    /// `http://host[:port]`. With the `tls` feature, `https://host[:port]` is also accepted.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn workspace_id(mut self, id: impl Into<String>) -> Self {
        self.workspace_id = Some(id.into());
        self
    }

    /// Sent as `Authorization: Bearer` when set. Omitted entirely when unset.
    /// An empty string is a configuration error, not "no key".
    /// One client holds one key: build a client per workspace.
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
        #[cfg(feature = "tls")]
        let tls = if origin.https {
            Some(super::tls::config()?)
        } else {
            None
        };
        Ok(Client {
            host: origin.host,
            port: origin.port,
            host_header: origin.host_header,
            workspace_id,
            api_key: match self.api_key {
                Some(key) if key.is_empty() => {
                    return Err(Error::Config(
                        "api key is empty; omit it to send no Authorization header".into(),
                    ));
                }
                Some(key) => Some(zeroize::Zeroizing::new(key)),
                None => None,
            },
            connect_timeout: self.connect_timeout,
            read_timeout: self.read_timeout,
            max_body: self.max_body,
            https: origin.https,
            #[cfg(feature = "tls")]
            tls,
        })
    }
}

struct Origin {
    host: String,
    port: u16,
    host_header: String,
    https: bool,
}

fn parse_origin(raw: &str) -> Result<Origin, Error> {
    let (rest, https) = if let Some(rest) = raw.strip_prefix("https://") {
        #[cfg(not(feature = "tls"))]
        {
            let _ = rest;
            return Err(Error::Config(
                "https requires the tls feature; this build speaks http only".into(),
            ));
        }
        #[cfg(feature = "tls")]
        (rest, true)
    } else if let Some(rest) = raw.strip_prefix("http://") {
        (rest, false)
    } else {
        return Err(Error::Config(
            "base URL must start with http:// or https://".into(),
        ));
    };
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
            "" => {
                if https {
                    443
                } else {
                    80
                }
            }
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
        (authority.to_string(), if https { 443 } else { 80 })
    };
    if host.is_empty() {
        return Err(Error::Config("base URL is missing a host".into()));
    }
    let host_header = if host.contains(':') {
        format!("[{host}]:{port}")
    } else if (!https && port == 80) || (https && port == 443) {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    Ok(Origin {
        host,
        port,
        host_header,
        https,
    })
}

fn authorization_of(key: &Option<zeroize::Zeroizing<String>>) -> Option<&str> {
    key.as_ref().map(|value| value.as_str())
}

impl Client {
    fn authorization(&self) -> Option<&str> {
        authorization_of(&self.api_key)
    }
}

fn expect_success(status: u16, body: &[u8]) -> Result<(), Error> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(status_error(status, body))
    }
}

fn status_error(status: u16, body: &[u8]) -> Error {
    match status {
        401 | 403 => Error::Unauthorized { status },
        _ => Error::Status {
            status,
            body: http::body_prefix(body),
        },
    }
}

fn require_conclusion_parties(query: &ConclusionQuery) -> Result<(), Error> {
    let Some(filters) = &query.filters else {
        return Err(Error::Config(
            "conclusion query requires observer and observed filters".into(),
        ));
    };
    let observer = filters.contains_key("observer_id") || filters.contains_key("observer");
    let observed = filters.contains_key("observed_id") || filters.contains_key("observed");
    if observer && observed {
        Ok(())
    } else {
        Err(Error::Config(
            "conclusion query requires observer and observed filters (observer_id/observed_id or observer/observed)".into(),
        ))
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

fn search_body(search: &MessageSearch, include_scope: bool) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("query".into(), json!(search.query));
    if let Some(limit) = search.limit {
        body.insert("limit".into(), json!(limit));
    }
    if let Some(filters) = &search.filters {
        body.insert("filters".into(), Value::Object(filters.clone()));
    }
    if include_scope {
        if let Some(scope) = &search.scope {
            body.insert("scope".into(), json!(scope));
        }
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
    if let Some(format) = &opts.response_format {
        body.insert("response_format".into(), Value::Object(format.clone()));
    }
    if let Some(stream) = opts.stream {
        body.insert("stream".into(), json!(stream));
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
