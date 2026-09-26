//! Blocking Honcho client. HTTP/1.1 on `std::net`, one request per connection.
//!
//! There is no default base URL and no environment fallback. Build it with
//! [`Client::builder`].

mod client;
mod error;
mod http;
mod stream;
#[cfg(feature = "tls")]
mod tls;

pub use client::{Client, ClientBuilder, Probe};
pub use error::Error;
pub use stream::ChatStream;
