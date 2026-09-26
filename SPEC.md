# roncho — Rust Port of the Honcho SDK

## Overview

**roncho** is a native Rust SDK for the [Honcho](https://honcho.dev) memory platform (v3 API). Honcho gives agents persistent, reasoning-based memory about peers (users, agents, entities) across sessions. This crate provides idiomatic Rust bindings to Honcho's HTTP API.

## Project Name

The crate is named `roncho` — a portmanteau of "Rust" + "Honcho."

## Scope (Initial Milestone: Agent Basics)

This milestone covers the core primitives an agent needs to:

1. **Write memories** — Create/retrieve peers and sessions, ingest messages with optional timestamps and metadata.
2. **Query the system** — Query peer representations via natural-language chat, retrieve formatted session context for LLM integration, list peers and sessions, search content.

### In Scope

- Honcho client initialization (API key, workspace ID, base URL, environment)
- Peer resource: get-or-create, list, chat, search, message helper, sessions
- Session resource: get-or-create, list, add/remove peers, add messages, list messages, get context, search
- Message creation helper (`peer.message(content)` → typed `MessageCreate` builder)
- SessionContext: fetch and convert to OpenAI/Anthropic message formats
- Pagination support (iterators + manual page access)
- Streaming support for chat endpoints (via `futures` + `async-stream`)
- Metadata and filtering
- Workspaces API: get-or-create (`POST /v3/workspaces`; there is no GET-by-id), list, update, delete — root-scoped `/v3/workspaces`
- Conclusions API: create, list, query, get, delete
- Peer cards and peer context
- Streaming chat (`text/event-stream` of `delta` / `done` events)

### Out of Scope (Future Milestones)

- Scopes as a first-class resource (chat and search can still pass a scope name)
- File uploads
- Webhooks
- Dreaming / queue status
- Sync convenience layer (async-only for this milestone)

## Dependencies

| Crate             | Purpose                                              |
|-------------------|------------------------------------------------------|
| `serde`           | Serialize/deserialize API payloads                   |
| `serde_json`      | JSON value handling for metadata                     |
| `reqwest`         | HTTP client (`rustls-tls`, default features off)     |
| `url`             | Base URL construction                                |
| `uuid`            | UUID generation for client-side IDs                  |
| `chrono`          | Timestamp parsing (DateTime<Utc>)                    |
| `thiserror`       | Ergonomic error types                                |
| `async-stream`    | Async iterators for pagination and streaming         |
| `futures`         | Async iterator traits                                |
| `tokio`           | Async runtime (dev-dependency for tests)             |
| `mockito`         | HTTP mocking for tests                               |

## Architecture

```
src/
├── lib.rs              # Crate root, re-exports
├── client.rs           # Honcho client, shared HTTP logic
├── error.rs            # Error types
├── models/             # Data models (API shapes)
│   ├── mod.rs
│   ├── peer.rs         # Peer, PeerCreate
│   ├── session.rs      # Session, SessionCreate, SessionPeerConfig
│   ├── message.rs      # Message, MessageCreate
│   ├── context.rs      # SessionContext, PeerContext, Summary
│   ├── page.rs         # Paginated response wrapper
│   └── chat.rs         # Chat response, evidence types
├── api/                # HTTP resource builders
│   ├── mod.rs
│   ├── peers.rs        # Peer API: create, list, chat, search
│   └── sessions.rs     # Session API: create, list, messages, context, chat
└── resources/          # Runtime objects with methods (client-bound)
    ├── mod.rs
    ├── peer.rs         # Peer object with .chat(), .message(), etc.
    ├── session.rs      # Session object with .add_messages(), .context(), etc.
    └── message.rs      # Message builder returned by peer.message()
```

### Design Philosophy

- **Models** are plain `serde` structs matching the API JSON shapes (snake_case, derived `Deserialize`/`Serialize`).
- **Resources** (Peer, Session) are runtime objects that hold a reference to the client + their own identity, and expose ergonomic methods. They mirror the Python/TypeScript SDK's pattern where `honcho.peer("alice")` returns an object you can call `.chat()` on directly.
- **API modules** contain the raw HTTP call functions. Resources delegate to these.
- **Async throughout.** All I/O methods are `async`. No blocking wrappers in this milestone.

## Data Model

### Core Primitives

```
Workspace
  ├── has many Peers
  └── has many Sessions

Peer (observer) ←→→ (observed) Peer  (many-to-many via sessions)

Session
  └── has many Messages (each attributed to one Peer)
```

### Key Types

#### `Honcho` (Client)
```rust
pub struct Honcho {
    workspace_id: String,
    api_key: Secret<String>,
    base_url: Url,
    http: reqwest::Client,
    max_retries: usize,
    timeout: Duration,
}
```
Constructor: `Honcho::builder()` or `Honcho::new(workspace_id, api_key)` with defaults from env vars.

#### `Peer` (Resource)
```rust
pub struct Peer {
    client: Honcho,   // clone of client (Arc-free, cheap clone)
    id: String,
    created_at: DateTime<Utc>,
    metadata: serde_json::Map,
    configuration: serde_json::Map,
}
```
Methods:
- `peer.chat(query, opts) -> ChatResponse`
- `peer.chat_stream(query, opts) -> ChatStream`
- `peer.message(content, meta, created_at) -> MessageCreate`  (builder)
- `peer.sessions() -> PaginatedIter<Session>`
- `peer.search(query) -> Vec<Message>`
- `peer.conclusions()` / `peer.conclusions_of(observed)`
- `peer.get_card(target) -> Vec<String>`
- `peer.set_card(card, target) -> Vec<String>`
- `peer.context(target, opts) -> PeerContext`

#### `Session` (Resource)
```rust
pub struct Session {
    client: Honcho,
    id: String,
    created_at: DateTime<Utc>,
    is_active: bool,
    metadata: serde_json::Map,
    configuration: serde_json::Map,
}
```
Methods:
- `session.add_peers(peers) -> Result<()>`
- `session.remove_peers(peer_ids) -> Result<()>`
- `session.set_peers(peers) -> Result<()>`
- `session.add_messages(messages) -> Result<Vec<Message>>`
- `session.messages(opts) -> PaginatedIter<Message>`
- `session.context(tokens, opts) -> SessionContext`
- `session.search(query, opts) -> PaginatedIter<Message>`
- `session.clone(message_id) -> Session`
- `session.delete() -> Result<()>`

#### `Message` (Data + Builder)
```rust
pub struct Message {
    pub id: String,
    pub content: String,
    pub peer_id: String,
    pub session_id: String,
    pub workspace_id: String,
    pub metadata: serde_json::Map,
    pub created_at: DateTime<Utc>,
    pub token_count: u32,
}

pub struct MessageCreate {
    pub content: String,
    pub peer_id: String,
    pub metadata: Option<serde_json::Map>,
    pub created_at: Option<DateTime<Utc>>,
}
```
`MessageCreate` is created via `peer.message(content)` and can be converted into a `MessageCreate` payload.

#### `SessionContext` (Formatted Context)
```rust
pub struct SessionContext {
    pub id: String,
    pub messages: Vec<Message>,
    pub summary: Option<Summary>,
    pub peer_representation: Option<String>,
    pub peer_card: Option<Vec<String>>,
}
```
Methods:
- `to_openai(&self, assistant: &Peer) -> Vec<OpenAIMessage>`
- `to_anthropic(&self, assistant: &Peer) -> Vec<AnthropicMessage>`

#### `Page<T>` (Pagination)
```rust
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub size: usize,
    pub pages: usize,
}
```
Supports iteration via `PaginatedIter` which auto-fetches subsequent pages.

## API Endpoints (v3)

| Method | Path                                              | Purpose             |
|--------|---------------------------------------------------|---------------------|
| POST   | `/v3/workspaces/{ws}/peers`                        | Get or create peer  |
| POST   | `/v3/workspaces/{ws}/peers/list`                   | List peers          |
| POST   | `/v3/workspaces/{ws}/peers/{peer}/chat`            | Peer chat           |
| POST   | `/v3/workspaces/{ws}/peers/{peer}/search`          | Search peer messages (returns `Message[]`) |
| GET    | `/v3/workspaces/{ws}/peers/{peer}/context`         | Get peer context    |
| GET    | `/v3/workspaces/{ws}/peers/{peer}/card`            | Get peer card       |
| PUT    | `/v3/workspaces/{ws}/peers/{peer}/card`            | Set peer card       |
| POST   | `/v3/workspaces/{ws}/peers/{peer}/sessions`        | List a peer's sessions |
| POST   | `/v3/workspaces/{ws}/sessions`                     | Get or create session |
| POST   | `/v3/workspaces/{ws}/sessions/list`                | List sessions       |
| POST   | `/v3/workspaces/{ws}/sessions/{session}/messages`  | Create messages (201) |
| POST   | `/v3/workspaces/{ws}/sessions/{session}/messages/list` | List messages (`page`/`size`/`reverse` are query params) |
| GET    | `/v3/workspaces/{ws}/sessions/{session}/context`   | Get session context |
| POST   | `/v3/workspaces/{ws}/sessions/{session}/peers`     | Add peers (body is the peer map) |
| PUT    | `/v3/workspaces/{ws}/sessions/{session}/peers`     | Replace peers       |
| DELETE | `/v3/workspaces/{ws}/sessions/{session}/peers`     | Remove peers (JSON array body) |
| GET    | `/v3/workspaces/{ws}/sessions/{session}/peers`     | List session peers (`Page[Peer]`) |
| POST   | `/v3/workspaces/{ws}/sessions/{session}/clone`     | Clone (`message_id` query) |
| POST   | `/v3/workspaces/{ws}/chat`                         | Workspace chat      |
| POST   | `/v3/workspaces/{ws}/search`                       | Search workspace (`Message[]`) |
| POST   | `/v3/workspaces/{ws}/conclusions`                  | Create conclusions  |
| POST   | `/v3/workspaces/{ws}/conclusions/list`             | List conclusions    |
| POST   | `/v3/workspaces/{ws}/conclusions/query`            | Query conclusions   |
| POST   | `/v3/workspaces`                                   | Get or create workspace |
| POST   | `/v3/workspaces/list`                              | List workspaces     |
| PUT    | `/v3/workspaces/{workspace}`                       | Update workspace    |
| DELETE | `/v3/workspaces/{workspace}`                       | Delete workspace (202) |

> Note: The Workspaces API is root-scoped — the workspace is resolved from the JWT, not a `{ws}` path segment, so requests target `/v3/workspaces`, `/v3/workspaces/list`, and `/v3/workspaces/{id}`.

> List routes are POST `.../list` (or POST `.../peers/{id}/sessions`) with `page`, `size`, and `reverse` on the query string. Search routes return a bare array and take `limit`, not a page.

## Environment Variables

- `HONCHO_API_KEY` — API key for authentication
- `HONCHO_BASE_URL` — Base URL (default: `https://api.honcho.dev`)
- `HONCHO_WORKSPACE_ID` — Default workspace ID

## Error Handling

```rust
pub enum Error {
    Api { status: u16, message: String },
    Http(reqwest::Error),
    Decode(String),      // JSON deserialization error
    InvalidUrl(String),
    MissingApiKey,
    MissingWorkspaceId,
    Configuration(String),
}
```

Uses `thiserror` for `Display` + `std::error::Error`. No fallible constructors; config errors are returned from methods that need the key.

## Testing Strategy

- **Unit tests**: Mock serde serialization round-trips.
- **Integration tests**: Use `mockito` to mock HTTP, verify request/response shapes and retry behavior.
- Test the `to_openai` / `to_anthropic` conversion functions.
- Test pagination iteration.

## Future Milestones

1. **Scopes** — first-class scope resources
2. **Sync wrappers** — blocking convenience layer
3. **File uploads** — PDF/text ingestion
4. **Webhooks, dreaming, and queue status**

## Compatibility

Shaped against Honcho OpenAPI 3.2.1 (`https://honcho.dev/docs/v3/openapi.json`). The client does not send or check an API version header.
