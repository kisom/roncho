#![cfg(feature = "async")]
use mockito::Server;

mod common;
use common::make_client;

fn make_message_json(id: &str, content: &str) -> String {
    format!(
        r#"{{"id":"{id}","session_id":"sess-1","content":"{content}","role":"assistant","created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z","metadata":{{}},"configuration":{{}},"peer_id":"peer-1","workspace_id":"ws-1","token_count":1}}"#,
        id = id,
        content = content
    )
}

#[test]
fn create_message_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/sessions/sess-1/messages",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!("[{}]", make_message_json("msg-1", "Hello")))
            .create();

        let client = make_client(&server.url());
        let session = roncho::Session::from_model(
            client.clone(),
            roncho::models::session::Session {
                id: "sess-1".to_string(),
                is_active: true,
                workspace_id: "ws-1".to_string(),
                metadata: Default::default(),
                configuration: Default::default(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            },
        );

        let msg_create = roncho::MessageCreate::new("Hello", "assistant");
        let msg = session
            .add_messages(&[msg_create])
            .await
            .expect("failed to create message")
            .remove(0);

        assert_eq!(msg.id, "msg-1");
        assert_eq!(msg.content, "Hello");
    });
}

#[test]
fn list_messages_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/sessions/sess-1/messages/list",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{"items":[{}],"total":1,"page":1,"size":50,"pages":1}}"#,
                make_message_json("msg-1", "Hello")
            ))
            .create();

        let client = make_client(&server.url());
        let session = roncho::Session::from_model(
            client.clone(),
            roncho::models::session::Session {
                id: "sess-1".to_string(),
                is_active: true,
                workspace_id: "ws-1".to_string(),
                metadata: Default::default(),
                configuration: Default::default(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            },
        );

        let page = session
            .messages(&roncho::models::page::ListOptions::default(), None)
            .await
            .expect("failed to list messages");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].content, "Hello");
    });
}
