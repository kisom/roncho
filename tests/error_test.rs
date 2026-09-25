use std::sync::Mutex;

use mockito::Server;

mod common;
use common::make_client;

/// Serializes tests that mutate process-global environment variables.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard that restores an environment variable to its previous value on drop.
pub struct EnvGuard {
    key: String,
    previous: Option<String>,
}

impl EnvGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            previous,
        }
    }

    pub fn remove(key: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key: key.to_string(),
            previous,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(&self.key, value),
            None => std::env::remove_var(&self.key),
        }
    }
}

#[test]
fn session_not_found_returns_error() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"detail":"Session not found"}"#)
            .create();

        let client = make_client(&server.url());
        let result = client.session("missing").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Session not found"),
            "Error was: {}",
            err
        );
    });
}

#[test]
fn server_error_returns_error() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"detail":"Internal server error"}"#)
            .create();

        let client = make_client(&server.url());
        let result = client.session("test").await;

        assert!(result.is_err());
    });
}

#[test]
fn peer_chat_error_returns_error() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/peers")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"bad","display_name":"Bad Peer","workspace_id":"test-workspace","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock("POST", "/v3/workspaces/test-workspace/peers/bad/chat")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"detail":"Bad request"}"#)
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("bad").await.expect("failed to create peer");

        let result = peer.chat("test", None).await;
        assert!(result.is_err());
    });
}

#[test]
fn missing_api_key_returns_error() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let _removed = EnvGuard::remove("HONCHO_API_KEY");
    let result = roncho::Honcho::builder()
        .base_url("http://localhost:9999")
        .build();

    assert!(result.is_err());
}

#[test]
fn missing_workspace_id_uses_default() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let _ws = EnvGuard::set("HONCHO_WORKSPACE_ID", "default-ws");
    let _key = EnvGuard::set("HONCHO_API_KEY", "test-key");
    let client = roncho::Honcho::builder().build().unwrap();
    assert_eq!(client.workspace_id(), "default-ws");
}
