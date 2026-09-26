//! Live checks against a Honcho server.
//!
//! ```text
//! cargo test --test live -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Needs `RONCHO_LIVE_URL`, or `base_url` in `~/.config/roncho/roncho.toml`.
//! Optional `RONCHO_LIVE_KEY`. Writes only to throwaway workspaces named
//! `roncho-live-<pid>-<test>` and deletes them.

#![cfg(feature = "blocking")]

use std::sync::Mutex;
use std::time::Duration;

use roncho::blocking::{Client, FileUpload};
use roncho::models::chat::DialecticOptions;
use roncho::models::conclusions::{ConclusionBatchCreate, ConclusionCreate, ConclusionQuery};
use roncho::models::message::{MessageCreate, MessageSearch};
use roncho::models::page::ListOptions;
use roncho::models::peer::PeerCreate;
use roncho::models::session::SessionCreate;
use roncho::models::workspace::WorkspaceCreate;
use roncho::{DreamType, QueueStatusQuery, ScheduleDream, ScopeCreate};

static LIVE: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LIVE.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn origin() -> (String, Option<String>) {
    let env_key = std::env::var("RONCHO_LIVE_KEY")
        .ok()
        .filter(|key| !key.is_empty());
    if let Ok(url) = std::env::var("RONCHO_LIVE_URL") {
        assert!(
            !url.is_empty(),
            "RONCHO_LIVE_URL is empty; set it to the Honcho origin"
        );
        return (url, env_key);
    }
    let file = roncho::config::load_default()
        .expect("reading roncho.toml")
        .expect("set RONCHO_LIVE_URL or put base_url in ~/.config/roncho/roncho.toml");
    let url = file
        .base_url
        .filter(|value| !value.is_empty())
        .expect("set RONCHO_LIVE_URL or base_url in ~/.config/roncho/roncho.toml");
    let key = env_key.or(file.api_key.filter(|value| !value.is_empty()));
    (url, key)
}

fn client_for(suffix: &str) -> (Client, Cleanup) {
    let (base, key) = origin();
    let workspace = format!("roncho-live-{}-{suffix}", std::process::id());
    let mut builder = Client::builder()
        .without_config_file()
        .base_url(&base)
        .workspace_id(&workspace)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(90));
    if let Some(key) = key {
        builder = builder.api_key(key);
    }
    let client = builder.build().expect("live client");
    client
        .get_or_create_workspace(&WorkspaceCreate::new(&workspace))
        .expect("create workspace");
    let cleanup = Cleanup {
        client: client.clone(),
    };
    (client, cleanup)
}

struct Cleanup {
    client: Client,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let id = self.client.workspace_id().to_string();
        let _ = self.client.delete_session("s");
        let _ = self.client.delete_workspace(&id);
    }
}

fn owner_session(client: &Client) {
    client.peer(&PeerCreate::new("owner")).expect("peer");
    client
        .session(
            &SessionCreate::new("s").with_peers(
                [(
                    "owner".into(),
                    roncho::SessionPeerConfig {
                        observe_me: Some(true),
                        observe_others: Some(false),
                    },
                )]
                .into_iter()
                .collect(),
            ),
        )
        .expect("session");
}

#[test]
#[ignore]
fn probe_and_refused_key() {
    let _lock = lock();
    let (base, _) = origin();
    let (client, _cleanup) = client_for("probe");
    let probe = client.probe().expect("health");
    assert!(
        (200..300).contains(&probe.status),
        "health status {}",
        probe.status
    );

    match Client::builder()
        .without_config_file()
        .base_url(&base)
        .workspace_id(client.workspace_id())
        .api_key("roncho-refused-key")
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(15))
        .build()
        .unwrap()
        .probe()
    {
        Err(roncho::blocking::Error::Unauthorized { status }) => {
            assert!(status == 401 || status == 403);
        }
        Ok(_) => {}
        Err(err) => panic!("refused key returned {err}"),
    }
}

#[test]
#[ignore]
fn workspace_peer_session_messages() {
    let _lock = lock();
    let (client, _cleanup) = client_for("msgs");
    owner_session(&client);

    let conflict = client
        .delete_workspace(client.workspace_id())
        .expect_err("active session should 409");
    assert!(matches!(
        conflict,
        roncho::blocking::Error::ActiveSessions { .. }
    ));

    let messages = client
        .add_messages("s", &[MessageCreate::new("live ping", "owner")])
        .expect("messages");
    assert_eq!(messages[0].content, "live ping");
    assert_eq!(messages[0].peer_id, "owner");

    let page = client
        .list_messages("s", &ListOptions::default().page(1).size(10), None)
        .expect("list");
    assert!(
        page.items
            .iter()
            .any(|item| item.content.contains("live ping")),
        "listed messages did not include the one just written"
    );
}

#[test]
#[ignore]
fn search() {
    let _lock = lock();
    let (client, _cleanup) = client_for("search");
    owner_session(&client);
    client
        .add_messages("s", &[MessageCreate::new("searchable live ping", "owner")])
        .expect("messages");
    let mut search = MessageSearch::new("searchable");
    search.limit = Some(5);
    client.search_workspace(&search).expect("workspace search");
    client.search_peer("owner", &search).expect("peer search");
    client.search_session("s", &search).expect("session search");
}

#[test]
#[ignore]
fn conclusions() {
    let _lock = lock();
    let (client, _cleanup) = client_for("concl");
    owner_session(&client);
    let batch = ConclusionBatchCreate {
        conclusions: vec![ConclusionCreate::new("live fact", "owner", "owner")],
    };
    let made = client.create_conclusions(&batch).expect("create");
    assert_eq!(made[0].content, "live fact");
    let mut query = ConclusionQuery::new("live fact");
    query.filters = Some(
        serde_json::json!({"observer_id": "owner", "observed_id": "owner"})
            .as_object()
            .cloned()
            .unwrap(),
    );
    client.query_conclusions(&query).expect("query");
    client
        .delete_conclusion(&made[0].id)
        .expect("delete conclusion");
}

#[test]
#[ignore]
fn chat_and_stream() {
    let _lock = lock();
    let (client, _cleanup) = client_for("chat");
    owner_session(&client);
    client
        .add_messages("s", &[MessageCreate::new("the token is maple", "owner")])
        .expect("messages");
    let opts = DialecticOptions {
        query: "What token did the owner just say?".into(),
        session_id: Some("s".into()),
        reasoning_level: Some(roncho::ReasoningLevel::Minimal),
        include_evidence: Some(true),
        ..DialecticOptions::default()
    };
    let answer = client.peer_chat("owner", &opts).expect("peer chat");
    assert!(!answer.content().is_empty());
    let workspace = client.workspace_chat(&opts).expect("workspace chat");
    assert!(!workspace.content().is_empty());
    let streamed: String = client
        .peer_chat_stream("owner", &opts)
        .expect("stream")
        .map(|item| item.expect("chunk"))
        .map(|chunk| chunk.content)
        .collect();
    assert!(!streamed.is_empty(), "stream produced no content");
}

#[test]
#[ignore]
fn scopes_upload_dream_queue() {
    let _lock = lock();
    let (client, _cleanup) = client_for("scope");
    owner_session(&client);
    client
        .add_messages("s", &[MessageCreate::new("scope note", "owner")])
        .expect("messages");

    let scope = client
        .scope(&ScopeCreate::new("live-therapy"))
        .expect("scope");
    assert_eq!(scope.id, "live-therapy");
    client.get_scope("live-therapy").expect("get scope");
    client
        .add_scope_sessions("live-therapy", &["s".into()])
        .expect("add session to scope");
    client
        .list_scope_sessions("live-therapy", &ListOptions::default())
        .expect("list scope sessions");
    client.scope_status("live-therapy").expect("scope status");

    let uploaded = client
        .upload_file(
            "s",
            &FileUpload {
                peer_id: "owner",
                filename: "note.txt",
                bytes: b"uploaded live note",
                content_type: None,
                metadata: None,
                configuration: None,
                created_at: None,
            },
        )
        .expect("upload");
    assert!(
        uploaded
            .iter()
            .any(|item| item.content.contains("uploaded live note")),
        "upload created no messages with the file text"
    );

    client
        .schedule_dream(&ScheduleDream::new("owner", DreamType::Omni))
        .expect("dream");
    client
        .queue_status(&QueueStatusQuery::new().session_id("s"))
        .expect("queue");
}
