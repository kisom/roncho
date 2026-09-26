# Changelog

## 0.1.1 — 2026-09-26

- Blocking tests compile with `cargo test --no-default-features -F blocking` and `-F tls`. The async integration tests are gated on `feature = "async"`.
- `query_conclusions` requires observer and observed filters and refuses the call before sending when they are missing. A conclusion batch of 0 or more than 100 is refused the same way.
- Blocking chat sends `response_format` when the caller set one.
- `Message.workspace_id` and `Message.token_count` are optional. `id`, `content`, `peer_id`, `session_id`, `created_at`, and `metadata` stay.
- Blocking `Error` is `#[non_exhaustive]`. HTTP 401 and 403 are `Error::Unauthorized` on both clients. An empty API key is a configuration error. Each client holds one key, wiped with `zeroize` 1.9 when the client is dropped.
- `queue_status` is `GET /v3/workspaces/{id}/queue/status`.
- Dial tries every resolved address. The connect timeout applies per address and does not bound name resolution.
- `rustls` and `rustls-native-certs` use caret requirements.
- Async workspace delete reports 409 as `Error::ActiveSessions`. The async API key is optional. Its retry policy is documented in the README.
- Checked against the compatibility notes in `docs/compatibility-2026-09-25.md`.

## 0.1.0 — 2026-09-25

First release of the blocking client.

Blocking client (`features = ["blocking"]`, and `default-features = false` to drop the async stack):

- `roncho::blocking::Client` performs workspace, peer, session, message, conclusion, search, chat, and `GET /health` calls on `std::net`. One request per connection, `Connection: close`.
- No default base URL, no environment variables, no redirects, no retries. Connect timeout, idle read timeout, and an 8 MiB response cap are caller-set (those three have defaults of 10s, 120s, and 8 MiB).
- Optional API key. A 409 on workspace delete is `Error::ActiveSessions`.
- Wire timestamps are strings. `source_ids` and chat-evidence `source_ids` treat JSON `null` as an empty list. `times_derived` treats JSON `null` as `1`. A self-hosted server sent both nulls.

- `peer_chat_stream` and `workspace_chat_stream` parse SSE across socket reads. The read timeout is the gap between bytes, so a pause between events times out and a slow answer does not.

- Feature `tls` (implies `blocking`) speaks `https://` with rustls 0.23.45, the ring provider, and `rustls-native-certs` 0.8.4. A plain `blocking` build still has no TLS crates. Certificates that are not in the platform store are refused before the HTTP request is sent.

The async client still defaults to `https://api.honcho.dev` and reads `HONCHO_API_KEY`, `HONCHO_BASE_URL`, and `HONCHO_WORKSPACE_ID`.

Not in this release: queue status, message get/update, scopes, webhooks, and file upload. The checked server did not report an image string. See `docs/compatibility-2026-09-25.md`.
