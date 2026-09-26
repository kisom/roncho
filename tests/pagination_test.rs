use mockito::Server;

mod common;
use common::make_client;

#[test]
fn pagination_first_page_has_next() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions/list")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"items":[],"total":100,"page":1,"size":10,"pages":10}"#)
            .create();

        let client = make_client(&server.url());
        let opts = roncho::models::page::ListOptions::new().page(1).size(10);
        let page = client.sessions(&opts).await.expect("failed to get page 1");

        assert!(page.has_next());
        assert!(!page.has_prev());
        assert_eq!(page.total, 100);
        assert_eq!(page.pages, 10);
    });
}

#[test]
fn pagination_last_page() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/peers/list")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"items":[],"total":10,"page":2,"size":10,"pages":2}"#)
            .create();

        let client = make_client(&server.url());
        let opts = roncho::models::page::ListOptions::new().page(2).size(10);
        let page = client.peers(&opts).await.expect("failed to get last page");

        assert!(!page.has_next());
        assert!(page.has_prev());
    });
}

#[test]
fn pagination_single_page() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions/list")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"items":[],"total":5,"page":1,"size":50,"pages":1}"#)
            .create();

        let client = make_client(&server.url());
        let page = client
            .sessions(&roncho::models::page::ListOptions::default())
            .await
            .expect("failed to list sessions");

        assert!(!page.has_next());
        assert!(!page.has_prev());
    });
}
