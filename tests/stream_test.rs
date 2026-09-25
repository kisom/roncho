use mockito::Server;

mod common;
use common::make_client;

#[test]
fn session_messages_stream_succeeds() {
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
                r#"{"items":[{"id":"msg-1","content":"Hi","role":"assistant","peer_id":"peer-1","session_id":"sess-1","workspace_id":"ws-1","created_at":"2024-01-01T00:00:00Z","metadata":{},"token_count":1}],"total":1,"page":1,"size":1,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        let stream = session.messages_stream(1);
        use futures::StreamExt;
        let mut count = 0;
        let mut stream = std::pin::pin!(stream);

        while let Some(result) = stream.next().await {
            result.expect("stream error");
            count += 1;
        }

        assert_eq!(count, 1);
    });
}

#[test]
fn peer_sessions_stream_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/peers")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"peer-1","display_name":"Peer One","workspace_id":"test-workspace","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        server
            .mock("GET", "/v3/workspaces/test-workspace/peers/peer-1/sessions")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"sess-1","workspace_id":"test-workspace","is_active":true,"created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}],"total":1,"page":1,"size":50,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");

        let stream = peer.sessions_stream(50);
        use futures::StreamExt;
        let mut count = 0;
        let mut stream = std::pin::pin!(stream);

        while let Some(result) = stream.next().await {
            result.expect("stream error");
            count += 1;
        }

        assert_eq!(count, 1);
    });
}

#[test]
fn stream_with_error_returns_error() {
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
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"detail":"Internal error"}"#)
            .create();

        let client = make_client(&server.url());
        let session = client
            .session("sess-1")
            .await
            .expect("failed to create session");

        let stream = session.messages_stream(50);
        use futures::StreamExt;
        let mut stream = std::pin::pin!(stream);

        let result = stream.next().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    });
}
