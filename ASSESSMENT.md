# Assessment of the generated SDK

This note is about the crate as the local model left it (`a9708f1`), before the Honcho OpenAPI 3.2.1 alignment in the working tree. The model was asked for a Rust SDK for Honcho v3. It produced a crate that builds, a CLI, examples, and a green mock test suite. It did not produce a client that can talk to the API.

Checked against `https://honcho.dev/docs/v3/openapi.json` (Honcho 3.2.1).

## What it did well

The shape of the crate is the right shape. Models stay free of the HTTP client, `src/api` owns the calls, and `Peer` / `Session` are the objects you call methods on. That matches the Python and TypeScript SDKs and it is easy to extend.

Auth is `Authorization: Bearer`. The default host is `https://api.honcho.dev`. Workspace routes sit under `/v3/workspaces/{id}/...`, and the workspace-admin routes correctly stay on `/v3/workspaces` because the id comes from the token. Config comes from the builder or `HONCHO_API_KEY` / `HONCHO_BASE_URL` / `HONCHO_WORKSPACE_ID`. Nothing secrets-shaped is hardcoded.

Conclusions, added in a later milestone, are the best of the HTTP layer. Create, list, query, get, and delete use the right paths, and list actually sends `page` / `size` / `reverse` on the query string. Workspace list, update, and delete are in the same class. The model can follow a contract when the contract is narrow and recent.

Errors are a `thiserror` enum. JSON bodies that fail to decode become `Error::Decode` on the happy path. The CLI is a thin wrapper with a config file and a human/JSON switch, not a second HTTP stack.

## Where it failed

The dominant failure is invented protocol that is internally consistent. The suite could not catch it, because the same pass wrote the mocks.

Response models required fields the API does not send. `Message.role` and `Peer.display_name` are not in OpenAPI. Every live message and peer payload would fail to deserialize. Tests and the quickstart stuffed those fields into fixtures, so decode tests passed.

Several calls would 404 or 422 before decode:

- List peers, list sessions, and list a peer's sessions were `GET` on the collection. The API only has `POST .../peers/list`, `POST .../sessions/list`, and `POST .../peers/{id}/sessions`.
- Message list built `page`, `size`, and `reverse`, then called a helper that drops the query string. Page 2 never happened, and the message stream could spin.
- Search put `query` inside `filters` and decoded `Page<Message>`. The body field is top-level `query`, and the response is a bare array. There is no page.
- Adding session peers wrapped the map in `{"peers": ...}` and used JSON `null` for a missing config. Set and remove posted to `/peers/set` and `/peers/remove`, which are not routes. Set is `PUT` of the map. Remove is `DELETE` of an id array. Listing session peers decoded a type the API does not return.
- Session clone posted `up_to_message_id`. The cutoff is the query parameter `message_id`, and there is no body.
- Peer card and peer context glued `target` into the path. Card set was `POST {"card": ...}` instead of `PUT {"peer_card": ...}`. Context options were accepted and ignored.
- Default chat sent `"reasoning_level": null` and `"include_evidence": null`. Both are non-null in the schema, so a default call is a 422. Workspace chat also sent `target` and `filters`, which that operation does not have.
- The SSE parser treated each `data:` line as raw text, did not stop on `{"done": true}`, and split lines only inside one TCP chunk. The API sends `{"delta": {"content": "..."}, "done": false}`. The test fixture was `data: Hi there`.
- `GET /v3/workspaces/{id}` does not exist. Read is get-or-create via `POST /v3/workspaces`.
- `peer.conclusions()` filtered only on `observed_id`, so it returned every observer's conclusions about that peer. A caller-supplied filter replaced the scope instead of narrowing it.
- Chat evidence reused the conclusions type and defaulted `times_derived` to 1 when the payload has no such field. A tool call without `tool_input` failed the whole response.

A few knobs were decorative. `max_retries` defaulted to 3 and was never read. `Honcho::default` panicked if env vars were missing, against the crate's own rule that config errors are returned. `delete_json` treated a missing `Content-Length` as an empty body and `unwrap`ed the decode. `roncho config` ignored `~/.config/roncho/roncho.toml`, which every other command reads. A missing workspace id was reported as a missing API key.

`SPEC.md` drifted with the code. It still listed conclusions and streaming as future work after both existed, and its endpoint table documented the wrong `GET` list routes. The last commit before this fix only rewrapped lines.

## How to use this model on an SDK

Give it the OpenAPI document, or a saved response for each endpoint, and tell it that prose and other SDKs lose when they disagree. The mistakes here are analogies: chat messages have a `role`, collections are `GET`, optional fields are JSON `null`, streaming is `data:` plus text. Those are plausible. They are not this API.

Make the contract check a separate step from the implementation, done against the spec, not against the code. This model will write a mock that repeats whatever path it just invented, then treat a green `cargo test` as proof.

Ask for a table of method, path, query, and body before it writes call sites. Reject the table if a row is not in the spec. Do that after the first milestone. Conclusions were closer to the spec than peers and sessions, but the early mistakes were already locked in by tests and then polished.

Ban `serde_json::json!` for optional request fields. Omit absent fields. For SSE, paste one real event trace, including a chunk split across reads, and make the test use that trace.

Do not spend a follow-up turn on formatting while decode of a real payload is still unproven. A clean diff is not a working client.

## After the alignment

The working tree now follows those routes and payload shapes. `cargo test --workspace` passes against mocks of the 3.2.1 contract. That is still not a call to `api.honcho.dev`. Scopes, uploads, webhooks, and queue status remain unwired on purpose.
