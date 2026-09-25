use roncho::Honcho;

#[test]
fn client_builder_requires_api_key() {
    let result = Honcho::builder()
        .base_url("http://localhost")
        .build();
    assert!(result.is_err());
}

#[test]
fn client_builder_with_api_key_succeeds() {
    let result = Honcho::builder()
        .api_key("test-key")
        .base_url("http://localhost")
        .workspace_id("ws-1")
        .build();
    assert!(result.is_ok());

    let client = result.unwrap();
    assert_eq!(client.workspace_id(), "ws-1");
}

#[test]
fn client_url_construction() {
    let client = Honcho::builder()
        .api_key("test-key")
        .base_url("https://api.example.com")
        .workspace_id("ws-1")
        .build()
        .expect("failed to build client");

    let url = client.url("peers").expect("failed to construct URL");
    assert_eq!(url.as_str(), "https://api.example.com/v3/workspaces/ws-1/peers");
}

#[test]
fn client_url_with_id() {
    let client = Honcho::builder()
        .api_key("test-key")
        .base_url("https://api.example.com")
        .workspace_id("ws-1")
        .build()
        .expect("failed to build client");

    let url = client
        .url("peers/peer-1/chat")
        .expect("failed to construct URL");
    assert_eq!(url.as_str(), "https://api.example.com/v3/workspaces/ws-1/peers/peer-1/chat");
}

#[test]
fn list_options_defaults() {
    let opts = roncho::models::page::ListOptions::default();
    assert!(opts.page.is_none());
    assert!(opts.size.is_none());
    assert!(opts.reverse.is_none());
}

#[test]
fn list_options_builder() {
    let opts = roncho::models::page::ListOptions::new()
        .page(2)
        .size(20)
        .reverse(true);

    assert_eq!(opts.page, Some(2));
    assert_eq!(opts.size, Some(20));
    assert_eq!(opts.reverse, Some(true));
}

#[test]
fn page_has_next_logic() {
    let page: roncho::models::page::Page<String> = roncho::models::page::Page {
        items: vec![],
        total: 10,
        page: 1,
        size: 5,
        pages: 2,
    };
    assert!(page.has_next());
    assert!(!page.has_prev());
}

#[test]
fn page_last_page_logic() {
    let page: roncho::models::page::Page<String> = roncho::models::page::Page {
        items: vec![],
        total: 10,
        page: 2,
        size: 5,
        pages: 2,
    };
    assert!(!page.has_next());
    assert!(page.has_prev());
}

#[test]
fn message_create_builder() {
    let msg = roncho::MessageCreate::new("Hello", "peer-1");
    assert_eq!(msg.content, "Hello");
    assert_eq!(msg.peer_id, "peer-1");
}

#[test]
fn session_context_request_defaults() {
    let req = roncho::SessionContextRequest::default();
    assert!(req.tokens.is_none());
    assert!(req.summary.is_none());
}

#[test]
fn chat_response_content() {
    let resp: roncho::models::chat::ChatResponse = serde_json::from_str(r#"{"content":"Hello"}"#).unwrap();
    assert_eq!(resp.content(), "Hello");
    assert!(resp.content.is_some());
}

#[test]
fn chat_response_no_content() {
    let resp: roncho::models::chat::ChatResponse = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(resp.content(), "");
    assert!(resp.content.is_none());
}
