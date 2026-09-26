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

`cargo test` is the gate for SDK changes. Integration tests live under `tests/` and mock Honcho with `mockito`. Keep those mocks on the Honcho v3 HTTP contract, not on an earlier guess. A blocking-only run is `cargo test --no-default-features -F blocking -p roncho`, and the TLS run is the same command with `-F tls`. Live checks are `cargo test --test live -- --ignored --nocapture --test-threads=1` and need `RONCHO_LIVE_URL` or `base_url` in `~/.config/roncho/roncho.toml`. They write only to throwaway `roncho-live-*` workspaces.

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
- Follow the Honcho v3 HTTP contract. `docs/compatibility-2026-09-25.md` records where one self-hosted server differed from OpenAPI 3.2.1.

## Environment Variables

- `HONCHO_API_KEY` — optional on the async client. Unset sends no `Authorization` header. An empty value is a configuration error. The blocking client does not read this variable. It does read `~/.config/roncho/roncho.toml` for builder fields that were left unset.
- `HONCHO_BASE_URL` — base URL (default `https://api.honcho.dev`).
- `HONCHO_WORKSPACE_ID` — default workspace.

## Security

- API keys come from env vars or explicit config. **Never** hardcode, log, or commit secrets/keys. The repo `.gitignore` excludes `.opencode`.

## Scope

`roncho::blocking::Client` is frozen as of 1.0.0. New work lands there first. The async `Honcho` client follows those shapes and may keep its environment fallback and `https://api.honcho.dev` default.

In scope now, on the blocking client: peers, sessions, messages, conclusions, search, chat (including streaming), queue status, workspaces, scopes, file upload, and dreaming. The async client also has peer cards, peer context, and session context.

Out of scope: webhooks.
