//! Blocking Honcho client. HTTP/1.1 on `std::net`, one request per connection.
//!
//! There is no default base URL and no environment fallback. Build it with
//! [`Client::builder`]. A field you leave unset is filled from
//! `roncho.toml` when that file exists. A value you set on the builder wins.

mod client;
mod error;
mod http;
mod stream;
#[cfg(feature = "tls")]
mod tls;

pub use crate::upload::FileUpload;
pub use client::{Client, ClientBuilder, Probe};
pub use error::Error;
pub use stream::ChatStream;
