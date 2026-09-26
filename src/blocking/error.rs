use std::fmt;

/// Failure from the blocking client. The API key is never included.
///
/// `non_exhaustive` so later variants (a refused key, for example) are not a breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// TCP connect failed.
    Connect(std::io::Error),
    /// No bytes arrived before the read timeout.
    Timeout,
    /// The server closed the socket before a complete response.
    Closed,
    /// A 3xx response. Redirects are not followed.
    Redirect { status: u16 },
    /// An HTTP status outside 2xx, with at most the first 4 KiB of the body.
    Status { status: u16, body: String },
    /// The server refused the key. 401 and 403 only; the body is not kept.
    Unauthorized { status: u16 },
    /// `DELETE` workspace returned 409 because sessions are still active.
    ActiveSessions { body: String },
    /// The response exceeded the configured body cap. Reading stopped there.
    TooLarge,
    /// The body was not the JSON the operation expected.
    Decode(String),
    /// A chat stream event was not the documented SSE payload.
    Stream(String),
    /// Builder inputs were missing or not an `http://` origin.
    Config(String),
    /// A socket error that is neither a connect failure nor a timeout.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "could not connect: {err}"),
            Error::Timeout => write!(f, "timed out waiting for bytes"),
            Error::Closed => write!(f, "the server closed the connection"),
            Error::Redirect { status } => write!(f, "refusing redirect (HTTP {status})"),
            Error::Status { status, body } => write!(f, "HTTP {status}: {body}"),
            Error::Unauthorized { status } => {
                write!(f, "the API key was refused (HTTP {status})")
            }
            Error::ActiveSessions { body } => {
                write!(f, "workspace still has active sessions: {body}")
            }
            Error::TooLarge => write!(f, "response body exceeded the configured cap"),
            Error::Decode(err) => write!(f, "response was not usable JSON: {err}"),
            Error::Stream(err) => write!(f, "chat stream error: {err}"),
            Error::Config(err) => write!(f, "invalid client configuration: {err}"),
            Error::Io(err) => write!(f, "socket error: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Connect(err) | Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

pub(crate) fn is_timeout(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}
