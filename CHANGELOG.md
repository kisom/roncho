# Changelog

## 0.1.0 — 2026-09-25

First release. The blocking client is what Boxmaker's `memoryd` should pin.

Blocking client (`features = ["blocking"]`, and `default-features = false` to drop the async stack):

- `roncho::blocking::Client` performs workspace, peer, session, message, conclusion, search, chat, and `GET /health` calls on `std::net`. One request per connection, `Connection: close`.
- No default base URL, no environment variables, no redirects, no retries. Connect timeout, idle read timeout, and an 8 MiB response cap are caller-set (those three have defaults of 10s, 120s, and 8 MiB).
- Optional API key. A 409 on workspace delete is `Error::ActiveSessions`.
- Wire timestamps are strings. `source_ids` and chat-evidence `source_ids` treat JSON `null` as an empty list. `times_derived` treats JSON `null` as `1`. Both show up on rift.

- `peer_chat_stream` and `workspace_chat_stream` parse SSE across socket reads. The read timeout is the gap between bytes, so a pause between events times out and a slow answer does not.

- Feature `tls` (implies `blocking`) speaks `https://` with rustls 0.23.45, the ring provider, and `rustls-native-certs` 0.8.4. A plain `blocking` build still has no TLS crates. Certificates that are not in the platform store are refused before the HTTP request is sent.

The async client still defaults to `https://api.honcho.dev` and reads `HONCHO_API_KEY`, `HONCHO_BASE_URL`, and `HONCHO_WORKSPACE_ID`.

Not in this release: queue status, message get/update, scopes, webhooks, and file upload. Rift did not report its image string. See `docs/rift-2026-09-25.md`.
