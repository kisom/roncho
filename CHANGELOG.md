# Changelog

## Unreleased

Blocking client (`features = ["blocking"]`, and `default-features = false` to drop the async stack):

- `roncho::blocking::Client` performs workspace, peer, session, message, conclusion, search, chat, and `GET /health` calls on `std::net`. One request per connection, `Connection: close`.
- No default base URL, no environment variables, no redirects, no retries. Connect timeout, idle read timeout, and an 8 MiB response cap are caller-set (those three have defaults of 10s, 120s, and 8 MiB).
- Optional API key. A 409 on workspace delete is `Error::ActiveSessions`.
- Wire timestamps are strings. `source_ids` on a conclusion treats JSON `null` as an empty list, which rift 3.2.0 sends.

Not in this cut: blocking SSE (R17), `https://` (R4), a git tag. The async client is unchanged in role and still defaults to `https://api.honcho.dev`.
