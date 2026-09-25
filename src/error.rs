use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error (status {status}): {message}")]
    Api {
        status: StatusCode,
        message: String,
    },

    #[error("failed to decode JSON response: {0}")]
    Decode(String),

    #[error("failed to encode JSON request: {0}")]
    Encode(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("API key is required — set HONCHO_API_KEY or pass it explicitly")]
    MissingApiKey,

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

    pub fn status_code(&self) -> Option<StatusCode> {
        match self {
            Error::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    detail: Option<serde_json::Value>,
}

pub(crate) fn parse_api_error(status: StatusCode, body: &str) -> Error {
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
