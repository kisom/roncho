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

- Rust 2021 edition. The async client is the default feature. `roncho::blocking::Client` is HTTP/1.1 on `std::net` and is selected with `default-features = false, features = ["blocking"]`.
- Async HTTP uses `reqwest` (rustls). Payloads use `serde`/`serde_json`. Timestamps and ids are strings (`chrono` and `uuid` are not dependencies). Errors use `thiserror`. The blocking client wipes its key with `zeroize`.
- `tokio` is a dev-dependency for the async tests, and `mockito` mocks HTTP there.
- Async iterators use `futures` + `async-stream` for pagination and streaming. Those crates are not in a blocking-only build.

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

- `HONCHO_API_KEY` — optional on the async client. Unset sends no `Authorization` header. An empty value is a configuration error. The blocking client does not read this variable.
- `HONCHO_BASE_URL` — base URL (default `https://api.honcho.dev`).
- `HONCHO_WORKSPACE_ID` — default workspace.

## Security

- API keys come from env vars or explicit config. **Never** hardcode, log, or commit secrets/keys. The repo `.gitignore` excludes `.opencode`.

## Scope

In scope: the async client and the blocking client (peers, sessions, messages, conclusions, search, chat, queue status, workspaces). Out of scope: peer cards, scopes, file uploads, webhooks. See `SPEC.md`.
