# AGENTS.md — roncho

Rust SDK for [Honcho](https://honcho.dev), the persistent, reasoning-based memory platform for agents. Native bindings to Honcho's HTTP API (v3).

## Commands

```bash
cargo build          # compile the library
cargo test           # run the test suite (integration tests under tests/, mockito-based)
cargo fmt            # format with rustfmt
cargo clippy         # lint
cargo run --example quickstart   # run the quickstart example (needs real Honcho credentials)
```

`cargo test` is the gate for SDK changes. Integration tests live under `tests/` and mock Honcho with `mockito`. Keep those mocks on the live v3 contract in `SPEC.md`, not on an earlier guess.

## Tech Stack

- Rust 2021 edition, `async` throughout (no blocking wrappers in this milestone).
- `reqwest` (rustls) for HTTP, `serde`/`serde_json` for payloads, `chrono` for timestamps, `uuid` for IDs, `thiserror` for errors.
- `tokio` async runtime (dev-dependency) and `mockito` for HTTP mocking in tests.
- Async iterators via `futures` + `async-stream` for pagination and streaming.

## Architecture

Three-layer separation — keep it clean when adding code:

- `src/models/` — plain `serde` structs matching API JSON shapes (snake_case, derive `Serialize`/`Deserialize`). No client reference.
- `src/resources/` — runtime objects (`Peer`, `Session`) that hold a clone of the client + their own identity, exposing ergonomic methods (`chat`, `message`, `add_messages`, ...). They delegate to `api/`.
- `src/api/` — raw HTTP call functions (`peers.rs`, `sessions.rs`, `workspace.rs`).
- `src/client.rs` — `Honcho` client + `HonchoBuilder`, shared HTTP logic (`get_json`/`post_json`/`delete_json`, headers, URL construction).
- `src/error.rs` — `Error` enum via `thiserror`.

Design philosophy: models are data-only; resources are client-bound runtime objects mirroring the Python/TS SDK pattern where `honcho.peer("alice")` returns an object you call `.chat()` on directly.

## Conventions

- New public API surface is re-exported from `src/lib.rs`.
- Use the builder pattern for client config (`Honcho::builder()...build()`).
- Config errors are returned from methods, not from constructors (no fallible constructors).
- Tests live in `tests/*.rs`, registered as `mod` in `tests/smoke_test.rs`; shared helpers go in `tests/common/mod.rs`.
- Follow Honcho API v3 endpoint contracts in `SPEC.md` — do not drift from them.

## Environment Variables

- `HONCHO_API_KEY` — authentication (required).
- `HONCHO_BASE_URL` — base URL (default `https://api.honcho.dev`).
- `HONCHO_WORKSPACE_ID` — default workspace.

## Security

- API keys come from env vars or explicit config. **Never** hardcode, log, or commit secrets/keys. The repo `.gitignore` excludes `.opencode`.

## Scope

In scope: peers (get-or-create, list, chat, search, messages, sessions), sessions (get-or-create, list, messages, context, chat, search), pagination, streaming, metadata/filtering. Out of scope (future): Conclusions API, peer cards, scopes, workspaces API, file uploads, webhooks, sync wrappers. See `SPEC.md` for full detail.
