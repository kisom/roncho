use mockito::Server;

mod common;
use common::make_client;

fn query_params(path_and_query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(q) = path_and_query.split('?').nth(1) {
        for pair in q.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

fn workspace_body(id: &str) -> &'static str {
    format!(
        r#"{{"id":"{id}","metadata":{{}},"configuration":{{}},"created_at":"2024-01-01T00:00:00Z"}}"#
    )
    .leak()
}

#[test]
fn get_or_create_workspace_hits_root_url() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(workspace_body("ws-1"))
            .create();

        let client = make_client(&server.url());
        let ws = client
            .workspaces()
            .get_or_create(roncho::WorkspaceCreate::new("ws-1"))
            .await
            .expect("failed to get or create workspace");

        assert_eq!(ws.id, "ws-1");
        assert_eq!(ws.metadata.len(), 0);
    });
}

#[test]
fn get_workspace_hits_root_url() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("GET", "/v3/workspaces/ws-1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(workspace_body("ws-1"))
            .create();

        let client = make_client(&server.url());
        let ws = client
            .workspaces()
            .get("ws-1")
            .await
            .expect("failed to get workspace");

        assert_eq!(ws.id, "ws-1");
    });
}

#[test]
fn list_workspaces_hits_root_url_and_sends_query() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", mockito::Matcher::Regex("^/v3/workspaces/list".to_string()))
            .match_request(|req| {
                let q = query_params(req.path_and_query());
                q.get("page") == Some(&"2".to_string())
                    && q.get("size") == Some(&"25".to_string())
                    && q.get("reverse") == Some(&"true".to_string())
            })
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"items":[{"id":"ws-1","metadata":{},"configuration":{},"created_at":"2024-01-01T00:00:00Z"}],"total":1,"page":2,"size":25,"pages":4}"#,
            )
            .create();

        let client = make_client(&server.url());
        let opts = roncho::WorkspaceListOptions::new()
            .page(2)
            .size(25)
            .reverse(true);
        let page = client
            .workspaces()
            .list(&opts)
            .await
            .expect("failed to list workspaces");

        assert_eq!(page.total, 1);
        assert_eq!(page.page, 2);
        assert_eq!(page.items[0].id, "ws-1");
    });
}

#[test]
fn update_workspace_hits_root_url() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("PUT", "/v3/workspaces/ws-1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(workspace_body("ws-1"))
            .create();

        let client = make_client(&server.url());
        let update = roncho::WorkspaceUpdate::new().with_metadata({
            let mut m = serde_json::Map::new();
            m.insert("note".to_string(), serde_json::json!("hello"));
            m
        });
        let ws = client
            .workspaces()
            .update("ws-1", update)
            .await
            .expect("failed to update workspace");

        assert_eq!(ws.id, "ws-1");
    });
}

#[test]
fn delete_workspace_hits_root_url() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("DELETE", "/v3/workspaces/ws-1")
            .with_status(204)
            .create();

        let client = make_client(&server.url());
        client
            .workspaces()
            .delete("ws-1")
            .await
            .expect("failed to delete workspace");
    });
}

#[test]
fn delete_workspace_conflict_returns_api_error() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("DELETE", "/v3/workspaces/ws-1")
            .with_status(409)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"conflict","message":"workspace has active sessions"}"#)
            .create();

        let client = make_client(&server.url());
        let err = client
            .workspaces()
            .delete("ws-1")
            .await
            .expect_err("expected failure");
        assert!(matches!(err, roncho::Error::Api { .. }));
    });
}

#[test]
fn get_or_create_workspace_sends_configuration() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces")
            .match_request(|req| {
                let body = req.body().ok().and_then(|b| std::str::from_utf8(b).ok());
                matches!(body, Some(b) if b.contains("\"id\":\"ws-1\"") && b.contains("\"configuration\""))
            })
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(workspace_body("ws-1"))
            .create();

        let client = make_client(&server.url());
        let mut config = serde_json::Map::new();
        config.insert("reasoning".to_string(), serde_json::json!({"enabled": true}));
        let create = roncho::WorkspaceCreate::new("ws-1").with_configuration(config);
        let ws = client
            .workspaces()
            .get_or_create(create)
            .await
            .expect("failed to get or create workspace");

        assert_eq!(ws.id, "ws-1");
    });
}
