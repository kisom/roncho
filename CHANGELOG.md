# Changelog

## Unreleased

Blocking client (`features = ["blocking"]`, and `default-features = false` to drop the async stack):

- `roncho::blocking::Client` performs workspace, peer, session, message, conclusion, search, chat, and `GET /health` calls on `std::net`. One request per connection, `Connection: close`.
- No default base URL, no environment variables, no redirects, no retries. Connect timeout, idle read timeout, and an 8 MiB response cap are caller-set (those three have defaults of 10s, 120s, and 8 MiB).
- Optional API key. A 409 on workspace delete is `Error::ActiveSessions`.
- Wire timestamps are strings. `source_ids` on a conclusion treats JSON `null` as an empty list, which rift 3.2.0 sends.

- `peer_chat_stream` and `workspace_chat_stream` parse SSE across socket reads. The read timeout is the gap between bytes, so a pause between events times out and a slow answer does not.

- Feature `tls` (implies `blocking`) speaks `https://` with rustls 0.23.45, the ring provider, and `rustls-native-certs` 0.8.4. A plain `blocking` build still has no TLS crates. Certificates that are not in the platform store are refused before the HTTP request is sent.

Not in this cut: a git tag. The async client is unchanged in role and still defaults to `https://api.honcho.dev`.
