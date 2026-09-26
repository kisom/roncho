#[cfg(feature = "async")]
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[cfg(feature = "async")]
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// 401 or 403. The body is not kept, and the key is not included.
    #[error("the API key was refused (HTTP {status})")]
    Unauthorized { status: u16 },

    /// `DELETE` workspace returned 409 because sessions are still active.
    #[error("workspace still has active sessions: {body}")]
    ActiveSessions { body: String },

    #[error("failed to decode JSON response: {0}")]
    Decode(String),

    #[error("failed to encode JSON request: {0}")]
    Encode(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("API key is required — set HONCHO_API_KEY or pass it explicitly")]
    MissingApiKey,

    #[error("workspace id is required — set HONCHO_WORKSPACE_ID or pass it explicitly")]
    MissingWorkspaceId,

    #[error("configuration error: {0}")]
    Configuration(String),

    #[error("streaming response error: {0}")]
    Stream(String),

    #[error("no more pages")]
    NoMorePages,
}

impl Error {
    pub fn is_api_error(&self) -> bool {
        matches!(self, Error::Api { .. })
    }

    pub fn status_code(&self) -> Option<u16> {
        match self {
            Error::Api { status, .. } | Error::Unauthorized { status } => Some(*status),
            _ => None,
        }
    }
}

#[cfg(feature = "async")]
#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    detail: Option<serde_json::Value>,
}

#[cfg(feature = "async")]
pub(crate) fn parse_api_error(status: u16, body: &str) -> Error {
    if status == 401 || status == 403 {
        return Error::Unauthorized { status };
    }
    if let Ok(err_body) = serde_json::from_str::<ApiErrorBody>(body) {
        if let Some(detail) = err_body.detail {
            let message = match detail {
                serde_json::Value::String(s) => s,
                serde_json::Value::Array(items) => items
                    .iter()
                    .filter_map(|item| {
                        item.get("msg")
                            .and_then(|m| m.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
                other => other.to_string(),
            };
            if !message.is_empty() {
                return Error::Api { status, message };
            }
        }
    }
    Error::Api {
        status,
        message: if body.is_empty() {
            status.to_string()
        } else {
            body.to_string()
        },
    }
}
