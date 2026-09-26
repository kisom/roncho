# roncho

A native Rust SDK for [Honcho](https://honcho.dev) — the persistent, reasoning-based memory platform for agents.

`roncho` gives your agent a memory of the people and projects it works with: it remembers facts about peers across sessions, recalls past conversations, and answers questions about what it already knows. This crate is an idiomatic Rust binding to Honcho's HTTP API (v3).

> **Status:** pre-1.0, actively under development. The public surface is still settling and the API may change between versions.

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

The crate is not yet published to crates.io. For now, depend on it via git:

```toml
[dependencies]
roncho = { git = "https://git.wntrmute.dev/kyle/roncho" }
```

## Configuration

Configure the client with the builder, or fall back to environment variables:

| Env var             | Purpose                     | Default                 |
|---------------------|-----------------------------|-------------------------|
| `HONCHO_API_KEY`    | API key (required)          | —                       |
| `HONCHO_BASE_URL`   | API base URL                | `https://api.honcho.dev`|
| `HONCHO_WORKSPACE_ID` | default workspace         | —                       |

```rust
use roncho::Honcho;

let honcho = Honcho::builder()
    .workspace_id("ws_123")
    .api_key("sk_...")
    .build()?;
```

The builder falls back to the environment variables above for any field you don't set explicitly.

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
```

See `AGENTS.md` for project conventions and `SPEC.md` for the full API contract.

## Roadmap

Near-term goals for `roncho`:

- Complete support for the **Conclusions API**.
- Build a **CLI tool** on top of the SDK.

See `SPEC.md` for the full scope and future milestones.
