use mockito::Server;

mod common;
use common::make_client;

#[test]
fn workspace_search_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/search")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"msg-1","content":"Hello","role":"assistant","peer_id":"peer-1","session_id":"sess-1","workspace_id":"ws-1","created_at":"2024-01-01T00:00:00Z","metadata":{},"token_count":1}],"total":1,"page":1,"size":50,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let page = client
            .search("Hello world")
            .await
            .expect("failed to search");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].content, "Hello");
    });
}

#[test]
fn workspace_chat_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"content":"Hi there!"}"#)
            .create();

        let client = make_client(&server.url());
        let resp = client
            .chat("Hello", None)
            .await
            .expect("failed to chat");

        assert_eq!(resp.content(), "Hi there!");
    });
}

#[test]
fn workspace_chat_stream_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/chat")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body("data: Hi there\n\ndata: !\n\n")
            .create();

        let client = make_client(&server.url());
        let stream = client.chat_stream("Hello", None);
        use futures::StreamExt;
        let mut collected: Vec<String> = Vec::new();
        let mut stream = std::pin::pin!(stream);

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("stream error");
            collected.push(chunk.content);
        }

        let combined = collected.join("");
        assert!(combined.contains("Hi there"));
    });
}
