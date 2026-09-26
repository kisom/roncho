# roncho

A native Rust SDK for [Honcho](https://honcho.dev) — the persistent, reasoning-based memory platform for agents.

`roncho` gives your agent a memory of the people and projects it works with: it remembers facts about peers across sessions, recalls past conversations, and answers questions about what it already knows. This crate is an idiomatic Rust binding to Honcho's HTTP API (v3).

> **Status:** 1.0.0. `roncho::blocking::Client` is frozen. The async `Honcho` client is not.

## Features

- **Peers** — get-or-create, list, chat, search, and build the working representation of a peer.
- **Sessions** — get-or-create, list, add/remove peers, add and list messages, clone, and delete.
- **Chat** — ask a peer (or the whole workspace) a question, with optional evidence and reasoning.
- **Streaming** — stream chat responses and paginated lists as async streams.
- **Context** — fetch formatted conversation context and convert it to OpenAI or Anthropic message formats.
- **Pagination** — iterate over results page-by-page with `Page<T>` / `PaginatedIter`, or access individual pages manually.
- **Metadata & filtering** — attach metadata to messages and filter on them.
- **Workspaces** — manage workspaces (get-or-create, list, get, update, delete) via `honcho.workspaces()`.

## Installation

```toml
[dependencies]
roncho = "1.0.0"
```

A blocking-only build has no async runtime and no default base URL. Timestamps are RFC 3339 strings so that graph does not pull `chrono`, `uuid`, or `url`.

```toml
[dependencies]
roncho = { version = "1.0.0", default-features = false, features = ["blocking"] }
```

```rust
use roncho::blocking::Client;

let client = Client::builder()
    .base_url("http://127.0.0.1:8000")
    .workspace_id("my-workspace")
    .read_timeout(std::time::Duration::from_secs(120))
    .build()?;
```

The API key is optional. When it is set it is sent as `Authorization: Bearer`, it is omitted from `Debug`, and it is wiped with `zeroize` when the client is dropped. An empty string is a configuration error. Leave the key unset to send no `Authorization` header. Keys are per workspace, so build one client per workspace, each with that workspace's key.

The blocking client reads no environment variables and does not follow redirects. `Client::builder().build()` fills a `base_url`, `workspace_id`, or `api_key` you left unset from `~/.config/roncho/roncho.toml` when that file exists. `Client::builder().from_file(path)` reads a chosen file instead. A value set on the builder wins. `without_config_file()` skips the file. Connecting tries every address a name resolves to, with the connect timeout applied per address. Resolving the name itself is not limited by that timeout. `https://` needs the `tls` feature (`rustls` 0.23 with the `ring` provider and the platform certificate store). See `CHANGELOG.md` and `docs/compatibility-2026-09-25.md`.

## Configuration

Configure the client with the builder, or fall back to environment variables:

| Env var             | Purpose                     | Default                 |
|---------------------|-----------------------------|-------------------------|
| `HONCHO_API_KEY`    | API key (optional; empty is an error) | none (no `Authorization` header) |
| `HONCHO_BASE_URL`   | API base URL                | `https://api.honcho.dev`|
| `HONCHO_WORKSPACE_ID` | default workspace         | —                       |

```rust
use roncho::Honcho;

let honcho = Honcho::builder()
    .workspace_id("ws_123")
    .api_key("sk_...")
    .build()?;
```

The async builder falls back to those environment variables for any field you don't set. A missing key sends no `Authorization` header. An empty key, set on the builder or in the environment, is a configuration error.

Retries on the async client: `max_retries` defaults to 3 extra attempts. `GET`, `PUT`, `DELETE`, and `HEAD` are retried on HTTP 429 and 5xx. A connect failure is retried for every method, including `POST`. A POST response status is not retried. The delay starts at 200ms and doubles each attempt, and stops doubling after the fifth.

## Quickstart

```rust
use roncho::{Honcho, MessageCreate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let honcho = Honcho::builder()
        .workspace_id(std::env::var("HONCHO_WORKSPACE_ID")?)
        .api_key(std::env::var("HONCHO_API_KEY")?)
        .build()?;

    // Get or create a peer.
    let peer = honcho.peer("alice").await?;

    // Get or create a session, and record a message.
    let session = honcho.session("quickstart").await?;
    let message = peer.message("Hi Alice, remind me to ship the SDK today.");
    session.add_messages(&[message]).await?;

    // Ask the peer something.
    let response = peer.chat("What did we just talk about?", None).await?;
    if let Some(text) = response.content {
        println!("{}", text);
    }

    Ok(())
}
```

Run the bundled example (needs real credentials):

```bash
HONCHO_API_KEY=sk_... HONCHO_WORKSPACE_ID=ws_... cargo run --example quickstart
```

## Usage

### Peers

```rust
let peer = honcho.peer("alice").await?;

// Ask a question.
let answer = peer.chat("Summarize what we worked on last week.", None).await?;

// Search this peer's memories.
let results = peer.search("SDK", &ListOptions::default()).await?;

// Iterate this peer's sessions.
let sessions = peer.sessions(&ListOptions::default()).await?;
```

### Sessions

```rust
let session = honcho.session("project-x").await?;

// Add messages attributed to specific peers.
let alice = honcho.peer("alice").await?;
let bob = honcho.peer("bob").await?;
session
    .add_messages(&[
        alice.message("Here's the plan."),
        bob.message("Sounds good."),
    ])
    .await?;

// List and search within the session.
let messages = session.messages(&ListOptions::default(), None).await?;
```

### Streaming

Stream a chat response or an entire paginated list:

```rust
use futures::StreamExt;

let mut stream = peer.chat_stream("Keep going.", None);
while let Some(chunk) = std::pin::pin!(stream).next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);
}
```

### Context for LLMs

Fetch a session's context and convert it to the format your model expects
(OpenAI or Anthropic):

```rust
let opts = roncho::resources::session::SessionContextRequest::new()
    .tokens(2000)
    .summary(true);
let context = session.context(&opts).await?;

// `context` can be rendered as OpenAI or Anthropic messages for your model.
```

### Search

```rust
// Across the whole workspace.
let results = honcho.search("onboarding").await?;
```

### Pagination

```rust
let page = peer.sessions(&ListOptions::default().page(1).size(50)).await?;
println!("{} of {} total", page.items.len(), page.total);
```

## Development

```bash
cargo build          # compile the library
cargo test           # run the test suite (mockito-based integration tests)
cargo clippy         # lint
cargo fmt            # format
cargo run --example quickstart   # run the quickstart (needs real credentials)
cargo test --test live -- --ignored --nocapture --test-threads=1
# live suite: RONCHO_LIVE_URL or base_url in ~/.config/roncho/roncho.toml
```

See `AGENTS.md` for project conventions. `docs/compatibility-2026-09-25.md` records where one self-hosted server differed from OpenAPI 3.2.1.

## Roadmap

`roncho::blocking::Client` is the 1.0.0 public API. It includes scopes, file upload, and dreaming. Webhooks stay out. The async `Honcho` client follows those same operations and keeps the environment fallback and the `https://api.honcho.dev` default.

The CLI and the blocking client read `~/.config/roncho/roncho.toml` (`api_key`, `base_url`, `workspace_id`). A sample with authentication left off is `docs/roncho.toml.example`. The blocking builder uses that file only for fields you did not set.
