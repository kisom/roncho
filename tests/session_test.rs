use mockito::Server;

mod common;
use common::make_client;

#[test]
fn create_session_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        assert_eq!(session.id, "sess-1");
    });
}

#[test]
fn list_sessions_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("GET", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}],"total":1,"page":1,"size":50,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let page = client
            .sessions(&roncho::models::page::ListOptions::default())
            .await
            .expect("failed to list sessions");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.total, 1);
    });
}

#[test]
fn session_delete_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock("DELETE", "/v3/workspaces/test-workspace/sessions/sess-1")
            .with_status(204)
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        session.delete().await.expect("failed to delete session");
    });
}

#[test]
fn session_clone_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-orig","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions/sess-orig/clone")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-clone","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-orig")
            .await
            .expect("failed to create session");

        let cloned = session
            .clone(None)
            .await
            .expect("failed to clone session");

        assert_eq!(cloned.id, "sess-clone");
    });
}

#[test]
fn session_add_messages_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions/sess-1/messages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"id":"msg-1","content":"Hello","role":"assistant","peer_id":"peer-1","session_id":"sess-1","workspace_id":"ws-1","created_at":"2024-01-01T00:00:00Z","metadata":{},"token_count":1}]"#,
            )
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        let msg = roncho::MessageCreate::new("Hello", "peer-1");
        let messages = session
            .add_messages(&[msg])
            .await
            .expect("failed to add messages");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");
    });
}

#[test]
fn session_messages_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/sessions/sess-1/messages/list",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"msg-1","content":"Hello","role":"assistant","peer_id":"peer-1","session_id":"sess-1","workspace_id":"ws-1","created_at":"2024-01-01T00:00:00Z","metadata":{},"token_count":1}],"total":1,"page":1,"size":50,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        let page = session
            .messages(&roncho::models::page::ListOptions::default(), None)
            .await
            .expect("failed to list messages");

        assert_eq!(page.items.len(), 1);
    });
}

#[test]
fn session_context_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock("GET", "/v3/workspaces/test-workspace/sessions/sess-1/context")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"sess-1","messages":[]}"#)
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        let ctx = session
            .context(&roncho::SessionContextRequest::default())
            .await
            .expect("failed to get context");

        assert_eq!(ctx.id, "sess-1");
    });
}
