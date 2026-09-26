use mockito::Server;

mod common;
use common::make_client;

#[test]
fn create_peer_succeeds() {
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

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");

        assert_eq!(peer.id, "peer-1");
    });
}

#[test]
fn list_peers_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/peers/list")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"peer-1","display_name":"Peer One","workspace_id":"test-workspace","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}],"total":1,"page":1,"size":50,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let page = client
            .peers(&roncho::models::page::ListOptions::default())
            .await
            .expect("failed to list peers");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "peer-1");
    });
}

#[test]
fn peer_chat_returns_response() {
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
            .mock("POST", "/v3/workspaces/test-workspace/peers/peer-1/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"content":"Hello world"}"#)
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");

        let resp = peer
            .chat("Hi", None)
            .await
            .expect("failed to chat");

        assert_eq!(resp.content(), "Hello world");
    });
}

#[test]
fn peer_chat_stream_succeeds() {
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
            .mock("POST", "/v3/workspaces/test-workspace/peers/peer-1/chat")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(
                "data: {\"delta\":{\"content\":\"Hello\"},\"done\":false}\n\ndata: {\"delta\":{\"content\":\" world\"},\"done\":false}\n\ndata: {\"done\":true}\n\n",
            )
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");

        let stream = peer.chat_stream("Hi", None);
        use futures::StreamExt;
        let mut collected: Vec<String> = Vec::new();
        let mut stream = std::pin::pin!(stream);

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("stream error");
            collected.push(chunk.content);
        }

        let combined = collected.join("");
        assert!(combined.contains("Hello"));
        assert!(combined.contains("world"));
    });
}

#[test]
fn peer_search_messages_succeeds() {
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
            .mock("POST", "/v3/workspaces/test-workspace/peers/peer-1/search")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"id":"msg-1","content":"Hello","peer_id":"peer-1","session_id":"sess-1","workspace_id":"ws-1","created_at":"2024-01-01T00:00:00Z","metadata":{},"token_count":1}]"#,
            )
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");

        let page = peer
            .search("Hello")
            .await
            .expect("failed to search");

        assert_eq!(page.len(), 1);
        assert_eq!(page[0].content, "Hello");
    });
}

#[test]
fn peer_context_succeeds() {
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
            .mock("GET", "/v3/workspaces/test-workspace/peers/peer-1/context")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"peer_id":"peer-1","target_id":"peer-1","representation":"peer-1","peer_card":[]}"#,
            )
            .create();

        let client = make_client(&server.url());
        let peer = client.peer("peer-1").await.expect("failed to create peer");

        let ctx = peer
            .context(None, None)
            .await
            .expect("failed to get context");

        assert_eq!(ctx.representation, Some("peer-1".to_string()));
    });
}
