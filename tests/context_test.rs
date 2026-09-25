mod common;
use common::make_client;
use mockito::Server;

#[test]
fn session_context_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server.mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server.mock("GET", "/v3/workspaces/test-workspace/sessions/sess-1/context")
            .match_query(mockito::Matcher::Regex("tokens=10".to_string()))
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
            .context(&roncho::SessionContextRequest {
                tokens: Some(10),
                ..roncho::SessionContextRequest::new()
            })
            .await
            .expect("failed to get context");

        assert_eq!(ctx.id, "sess-1");
    });
}

#[test]
fn peer_context_with_target_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server.mock("POST", "/v3/workspaces/test-workspace/peers")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"peer-1","display_name":"test-peer","workspace_id":"test-workspace","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server.mock("GET", "/v3/workspaces/test-workspace/peers/peer-1/context")
            .match_query(mockito::Matcher::Regex("target=peer-2".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"representation":"peer-1","peer_card":[]}"#)
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");
        
        let ctx = peer
            .context(Some("peer-2"), None)
            .await
            .expect("failed to get context");

        assert_eq!(ctx.representation, Some("peer-1".to_string()));
    });
}