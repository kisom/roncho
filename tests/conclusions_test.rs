use mockito::Server;

mod common;
use common::make_client;

fn conclusion_body(id: &str) -> &'static str {
    format!(
        r#"{{"id":"{id}","content":"hello","observer_id":"peer-1","observed_id":"peer-2","session_id":null,"level":"explicit","source_ids":[],"times_derived":1,"created_at":"2024-01-01T00:00:00Z"}}"#
    )
    .leak()
}

#[test]
fn create_conclusions_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/conclusions")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"id":"concl-1","content":"hello","observer_id":"peer-1","observed_id":"peer-2","session_id":null,"level":"explicit","source_ids":[],"times_derived":1,"created_at":"2024-01-01T00:00:00Z"}]"#,
            )
            .create();

        let client = make_client(&server.url());
        let create = roncho::ConclusionCreate::new("hello", "peer-1", "peer-2");
        let batch = roncho::ConclusionBatchCreate::new(vec![create]);
        let results = client
            .conclusions()
            .create(batch)
            .await
            .expect("failed to create conclusions");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "concl-1");
        assert_eq!(results[0].content, "hello");
        assert_eq!(results[0].level, roncho::Level::Explicit);
    });
}

#[test]
fn list_conclusions_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/conclusions/list",
            )
            .match_request(|req| req.body().ok().and_then(|b| std::str::from_utf8(b).ok()) == Some(r#"{"filters":{}}"#))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"concl-1","content":"hello","observer_id":"peer-1","observed_id":"peer-2","session_id":null,"level":"explicit","source_ids":[],"times_derived":1,"created_at":"2024-01-01T00:00:00Z"}],"total":1,"page":1,"size":50,"pages":1}"#,
            )
            .create();

        let client = make_client(&server.url());
        let page = client
            .conclusions()
            .list(roncho::ConclusionListOptions::new())
            .await
            .expect("failed to list conclusions");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "concl-1");
    });
}

#[test]
fn list_conclusions_applies_filters() {
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
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/conclusions/list",
            )
            .match_request(|req| req.body().ok().and_then(|b| std::str::from_utf8(b).ok()) == Some(r#"{"filters":{"observed_id":"peer-1"}}"#))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[],"total":0,"page":1,"size":50,"pages":0}"#,
            )
            .create();

        let client = make_client(&server.url());
        let page = client
            .peer("peer-1")
            .await
            .expect("failed to get peer")
            .conclusions()
            .list(roncho::ConclusionListOptions::new())
            .await
            .expect("failed to list peer conclusions");

        assert_eq!(page.total, 0);
    });
}

#[test]
fn query_conclusions_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/conclusions/query")
            .match_request(|req| req.body().ok().and_then(|b| std::str::from_utf8(b).ok()) == Some(r#"{"query":"search","top_k":5}"#))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"id":"concl-1","content":"hello","observer_id":"peer-1","observed_id":"peer-2","session_id":null,"level":"explicit","source_ids":[],"times_derived":1,"created_at":"2024-01-01T00:00:00Z"}]"#,
            )
            .create();

        let client = make_client(&server.url());
        let results = client
            .conclusions()
            .query("search", Some(5))
            .await
            .expect("failed to query conclusions");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "concl-1");
    });
}

#[test]
fn get_conclusion_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "GET",
                "/v3/workspaces/test-workspace/conclusions/concl-1",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(conclusion_body("concl-1"))
            .create();

        let client = make_client(&server.url());
        let conclusion = client
            .conclusions()
            .get("concl-1")
            .await
            .expect("failed to get conclusion");

        assert_eq!(conclusion.id, "concl-1");
        assert_eq!(conclusion.level, roncho::Level::Explicit);
    });
}

#[test]
fn delete_conclusion_succeeds() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "DELETE",
                "/v3/workspaces/test-workspace/conclusions/concl-1",
            )
            .with_status(204)
            .create();

        let client = make_client(&server.url());
        client
            .conclusions()
            .delete("concl-1")
            .await
            .expect("failed to delete conclusion");
    });
}

#[test]
fn create_conclusions_returns_error_on_failure() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/conclusions")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"invalid","message":"bad request"}"#)
            .create();

        let client = make_client(&server.url());
        let create = roncho::ConclusionCreate::new("hello", "peer-1", "peer-2");
        let batch = roncho::ConclusionBatchCreate::new(vec![create]);
        let err = client
            .conclusions()
            .create(batch)
            .await
            .expect_err("expected failure");
        assert!(matches!(err, roncho::Error::Api { .. }));
    });
}
