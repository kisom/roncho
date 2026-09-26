#![cfg(feature = "async")]
use mockito::Server;

mod common;
use common::make_client;
use roncho::{DreamType, FileUpload, QueueStatusQuery, ScheduleDream, ScopeCreate};

#[test]
fn scope_get_or_create_and_lookup() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock("POST", "/v3/workspaces/test-workspace/scopes")
            .match_body(r#"{"id":"therapy"}"#)
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"therapy","created_at":"2024-01-01T00:00:00Z","metadata":{}}"#)
            .create();
        server
            .mock("GET", "/v3/workspaces/test-workspace/scopes/therapy")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"therapy","created_at":"2024-01-01T00:00:00Z","metadata":{}}"#)
            .create();

        let client = make_client(&server.url());
        let created = client
            .scope(&ScopeCreate::new("therapy"))
            .await
            .expect("scope");
        assert_eq!(created.id, "therapy");
        let got = client.get_scope("therapy").await.expect("get");
        assert_eq!(got.id, "therapy");
    });
}

#[test]
fn add_scope_sessions_and_dream() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/scopes/therapy/sessions",
            )
            .match_body(r#"{"session_ids":["s"]}"#)
            .with_status(204)
            .create();
        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/schedule_dream",
            )
            .match_request(|req| {
                req.body()
                    .ok()
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .is_some_and(|body| {
                        body.contains(r#""observer":"owner""#)
                            && body.contains(r#""dream_type":"omni""#)
                    })
            })
            .with_status(204)
            .create();
        server
            .mock("GET", "/v3/workspaces/test-workspace/queue/status")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"completed_work_units":0,"in_progress_work_units":0,"pending_work_units":0,"total_work_units":0}"#,
            )
            .create();

        let client = make_client(&server.url());
        client
            .add_scope_sessions("therapy", &["s".into()])
            .await
            .expect("add sessions");
        client
            .schedule_dream(&ScheduleDream::new("owner", DreamType::Omni))
            .await
            .expect("dream");
        let status = client
            .queue_status(&QueueStatusQuery::new())
            .await
            .expect("queue");
        assert_eq!(status.pending_work_units, 0);
        assert!(client
            .add_scope_sessions("therapy", &[])
            .await
            .is_err());
    });
}

#[test]
fn session_upload_file() {
    let mut server = Server::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        server
            .mock(
                "POST",
                "/v3/workspaces/test-workspace/sessions/s/messages/upload",
            )
            .match_body(mockito::Matcher::Regex("filename=\"note.txt\"".into()))
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"id":"m","content":"hello","peer_id":"p","session_id":"s","created_at":"2024-01-01T00:00:00Z","metadata":{}}]"#,
            )
            .create();
        server
            .mock("POST", "/v3/workspaces/test-workspace/sessions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"s","is_active":true,"workspace_id":"test-workspace","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#,
            )
            .create();

        let client = make_client(&server.url());
        let session = client.session("s").await.expect("session");
        let messages = session
            .upload_file(&FileUpload {
                peer_id: "p",
                filename: "note.txt",
                bytes: b"hello",
                content_type: None,
                metadata: None,
                configuration: None,
                created_at: None,
            })
            .await
            .expect("upload");
        assert_eq!(messages[0].content, "hello");
    });
}
