use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use roncho::blocking::Client;
use roncho::models::chat::DialecticOptions;
use roncho::models::conclusions::{ConclusionBatchCreate, ConclusionCreate, ConclusionQuery};
use roncho::models::message::{MessageCreate, MessageSearch};
use roncho::models::page::ListOptions;
use roncho::models::peer::PeerCreate;
use roncho::models::session::SessionCreate;
use roncho::models::workspace::WorkspaceCreate;

fn client_at(port: u16) -> Client {
    Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .connect_timeout(Duration::from_secs(2))
        .read_timeout(Duration::from_secs(2))
        .build()
        .expect("client")
}

fn accept_limited(listener: TcpListener) -> Option<std::net::TcpStream> {
    listener.set_nonblocking(true).unwrap();
    let started = std::time::Instant::now();
    loop {
        match listener.accept() {
            Ok((sock, _)) => {
                sock.set_nonblocking(false).unwrap();
                return Some(sock);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() > Duration::from_secs(2) {
                    return None;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("accept: {err}"),
        }
    }
}

fn serve(response: Vec<u8>) -> (u16, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let Some(mut sock) = accept_limited(listener) else {
            return;
        };
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            match sock.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(buf);
        let _ = sock.write_all(&response);
    });
    (port, rx)
}

fn request_text(rx: mpsc::Receiver<Vec<u8>>) -> String {
    String::from_utf8(rx.recv_timeout(Duration::from_secs(3)).unwrap()).unwrap()
}

fn json_ok(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[test]
fn builder_requires_base_url_and_workspace_and_rejects_https() {
    let missing = Client::builder().workspace_id("box").build();
    assert!(missing.is_err());
    let https = Client::builder()
        .base_url("https://127.0.0.1:9")
        .workspace_id("box")
        .build();
    #[cfg(not(feature = "tls"))]
    assert!(https.is_err());
    #[cfg(feature = "tls")]
    assert!(https.is_ok());
    let ok = Client::builder()
        .base_url("http://127.0.0.1:9")
        .workspace_id("box")
        .build();
    assert!(ok.is_ok());
}

#[test]
fn key_is_optional_and_absent_from_debug() {
    let (port, rx) = serve(json_ok(r#"{"status":"ok"}"#));
    let client = Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .api_key("super-secret")
        .read_timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let builder = Client::builder().api_key("super-secret");
    assert!(
        !format!("{builder:?}").contains("super-secret"),
        "builder debug included the key: {builder:?}"
    );
    let rendered = format!("{client:?}");
    assert!(!rendered.contains("super-secret"));
    assert!(rendered.contains("api_key_present"));
    client.probe().unwrap();
    let raw = request_text(rx);
    assert!(raw.contains("Authorization: Bearer super-secret\r\n"));

    let (port, rx) = serve(json_ok(r#"{"status":"ok"}"#));
    let client = client_at(port);
    client.probe().unwrap();
    let raw = request_text(rx);
    assert!(!raw.contains("Authorization:"));
}

#[test]
fn get_or_create_workspace_round_trip() {
    let body =
        r#"{"id":"box","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#;
    let (port, rx) = serve(json_ok(body));
    let ws = client_at(port)
        .get_or_create_workspace(&WorkspaceCreate::new("box"))
        .unwrap();
    assert_eq!(ws.id, "box");
    let raw = request_text(rx);
    assert!(raw.starts_with("POST /v3/workspaces HTTP/1.1\r\n"));
    assert!(raw.contains(r#""id":"box""#));
}

#[test]
fn peer_session_messages_and_search() {
    let peer = r#"{"id":"p","workspace_id":"box","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#;
    let (port, rx) = serve(json_ok(peer));
    client_at(port).peer(&PeerCreate::new("p")).unwrap();
    let raw = request_text(rx);
    assert!(raw.starts_with("POST /v3/workspaces/box/peers HTTP/1.1"));
    assert!(raw.contains(r#""id":"p""#));

    let session = r#"{"id":"s","is_active":true,"workspace_id":"box","created_at":"2024-01-01T00:00:00Z","metadata":{},"configuration":{}}"#;
    let (port, rx) = serve(json_ok(session));
    client_at(port).session(&SessionCreate::new("s")).unwrap();
    let raw = request_text(rx);
    assert!(raw.contains("POST /v3/workspaces/box/sessions "));
    assert!(raw.contains(r#""id":"s""#));

    let msg = r#"[{"id":"m","content":"hi","peer_id":"p","session_id":"s","workspace_id":"box","created_at":"2024-01-01T00:00:00Z","token_count":1,"metadata":{}}]"#;
    let (port, rx) = serve(json_ok(msg));
    let created = client_at(port)
        .add_messages("s", &[MessageCreate::new("hi", "p")])
        .unwrap();
    assert_eq!(created[0].content, "hi");
    let raw = request_text(rx);
    assert!(raw.contains("/sessions/s/messages "));
    assert!(raw.contains(r#""content":"hi""#));
    assert!(raw.contains(r#""peer_id":"p""#));
    assert!(raw.contains(r#""messages""#));

    let page = r#"{"items":[],"total":0,"page":2,"size":10,"pages":0}"#;
    let (port, rx) = serve(json_ok(page));
    let opts = ListOptions::default().page(2).size(10);
    client_at(port).list_messages("s", &opts, None).unwrap();
    let raw = request_text(rx);
    assert!(raw.contains("POST /v3/workspaces/box/sessions/s/messages/list?"));
    assert!(raw.contains("page=2"));
    assert!(raw.contains("size=10"));

    let found = r#"[{"id":"m","content":"hi","peer_id":"p","session_id":"s","workspace_id":"box","created_at":"2024-01-01T00:00:00Z","token_count":1,"metadata":{}}]"#;
    let (port, rx) = serve(json_ok(found));
    let mut search = MessageSearch::new("hi");
    search.limit = Some(5);
    let hits = client_at(port).search_workspace(&search).unwrap();
    assert_eq!(hits.len(), 1);
    let raw = request_text(rx);
    assert!(raw.contains("POST /v3/workspaces/box/search "));
    assert!(raw.contains(r#""query":"hi""#));
    assert!(raw.contains(r#""limit":5"#));

    let (port, rx) = serve(json_ok(found));
    client_at(port).search_peer("p", &search).unwrap();
    let raw = request_text(rx);
    assert!(raw.contains("/peers/p/search "));
    assert!(raw.contains(r#""query":"hi""#));
    assert!(raw.contains(r#""limit":5"#));

    let (port, rx) = serve(json_ok(found));
    client_at(port).search_session("s", &search).unwrap();
    let raw = request_text(rx);
    assert!(raw.contains("/sessions/s/search "));
    assert!(raw.contains(r#""query":"hi""#));
}

#[test]
fn conclusions_chat_and_deletes() {
    let concl = r#"[{"id":"c","content":"fact","observer_id":"p","observed_id":"q","created_at":"2024-01-01T00:00:00Z"}]"#;
    let (port, rx) = serve(json_ok(concl));
    let batch = ConclusionBatchCreate {
        conclusions: vec![ConclusionCreate::new("fact", "p", "q")],
    };
    let made = client_at(port).create_conclusions(&batch).unwrap();
    assert_eq!(made[0].id, "c");
    let raw = request_text(rx);
    assert!(raw.contains(r#""content":"fact""#));
    assert!(raw.contains(r#""observer_id":"p""#));
    assert!(raw.contains(r#""observed_id":"q""#));

    let page = r#"{"items":[],"total":0,"page":1,"size":50,"pages":0}"#;
    let (port, _) = serve(json_ok(page));
    client_at(port)
        .list_conclusions(&roncho::ConclusionListOptions::new())
        .unwrap();

    let (port, rx) = serve(json_ok(concl));
    let mut query = ConclusionQuery::new("fact");
    query.filters = Some(
        serde_json::json!({"observer_id": "p", "observed_id": "q"})
            .as_object()
            .cloned()
            .unwrap(),
    );
    client_at(port).query_conclusions(&query).unwrap();
    let raw = request_text(rx);
    assert!(raw.contains(r#""query":"fact""#));
    assert!(raw.contains(r#""observer_id":"p""#));
    assert!(raw.contains(r#""observed_id":"q""#));

    let (port, rx) = serve(
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    );
    client_at(port).delete_conclusion("c").unwrap();
    assert!(request_text(rx).starts_with("DELETE /v3/workspaces/box/conclusions/c "));

    let (port, _) =
        serve(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
    client_at(port).delete_session("s").unwrap();

    let chat = r#"{"content":"yes"}"#;
    let (port, rx) = serve(json_ok(chat));
    let opts = DialecticOptions {
        reasoning_level: Some(roncho::ReasoningLevel::Low),
        include_evidence: Some(false),
        ..DialecticOptions::default()
    };
    let answer = client_at(port).peer_chat("p", &opts).unwrap();
    assert_eq!(answer.content(), "yes");
    let raw = request_text(rx);
    assert!(raw.contains("POST /v3/workspaces/box/peers/p/chat "));
    assert!(raw.contains(r#""reasoning_level":"low""#));
    assert!(raw.contains(r#""query":""#));
    assert!(!raw.contains(r#""stream":true"#));
    assert!(!raw.contains("\"reasoning_level\":null"));

    let (port, rx) = serve(json_ok(chat));
    client_at(port)
        .workspace_chat(&DialecticOptions {
            query: "q".into(),
            ..DialecticOptions::default()
        })
        .unwrap();
    let raw = request_text(rx);
    assert!(raw.contains("POST /v3/workspaces/box/chat "));
    assert!(raw.contains(r#""query":"q""#));
    assert!(!raw.contains("target"));
    assert!(!raw.contains("filters"));

    let (port, rx) = serve(json_ok(chat));
    let mut format = serde_json::Map::new();
    format.insert("type".into(), serde_json::json!("json_object"));
    client_at(port)
        .peer_chat(
            "p",
            &DialecticOptions {
                query: "q".into(),
                response_format: Some(format),
                ..DialecticOptions::default()
            },
        )
        .unwrap();
    assert!(request_text(rx).contains(r#""response_format":{"type":"json_object"}"#));
}

#[test]
fn workspace_delete_409_is_its_own_error() {
    let (port, _) = serve(
        b"HTTP/1.1 409 Conflict\r\nContent-Length: 11\r\nConnection: close\r\n\r\nactive ones"
            .to_vec(),
    );
    let err = client_at(port).delete_workspace("box").unwrap_err();
    assert!(matches!(
        err,
        roncho::blocking::Error::ActiveSessions { .. }
    ));
}

#[test]
fn refused_timeout_partial_redirect_oversize_and_garbage() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let err = client_at(port).probe().unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Connect(_)));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let Some(sock) = accept_limited(listener) else {
            return;
        };
        thread::sleep(Duration::from_millis(500));
        drop(sock);
    });
    let err = Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .connect_timeout(Duration::from_secs(1))
        .read_timeout(Duration::from_millis(100))
        .build()
        .unwrap()
        .probe()
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Timeout));

    let (port, _) =
        serve(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort".to_vec());
    let err = client_at(port).probe().unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Closed));

    let (port, _) = serve(b"HTTP/1.1 302 Found\r\nLocation: http://elsewhere/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
    let err = client_at(port).probe().unwrap_err();
    assert!(matches!(
        err,
        roncho::blocking::Error::Redirect { status: 302 }
    ));

    let (port, _) =
        serve(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n".to_vec());
    let err = Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .max_body(16)
        .read_timeout(Duration::from_secs(1))
        .build()
        .unwrap()
        .probe()
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::TooLarge));

    let (port, _) = serve(json_ok("not-json"));
    let err = client_at(port)
        .get_or_create_workspace(&WorkspaceCreate::new("box"))
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Decode(_)));

    for status in [401u16, 403] {
        let raw = format!(
            "HTTP/1.1 {status} Forbidden\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno"
        );
        let (port, _) = serve(raw.into_bytes());
        let err = client_at(port).probe().unwrap_err();
        assert!(
            matches!(err, roncho::blocking::Error::Unauthorized { status: got } if got == status),
            "{err}"
        );
    }
}

#[test]
fn conclusion_query_and_batch_are_refused_before_send() {
    let err = client_at(1)
        .query_conclusions(&ConclusionQuery::new("fact"))
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Config(_)), "{err}");

    let empty = ConclusionBatchCreate {
        conclusions: vec![],
    };
    let err = client_at(1).create_conclusions(&empty).unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Config(_)), "{err}");

    let too_many = ConclusionBatchCreate {
        conclusions: (0..101)
            .map(|i| ConclusionCreate::new(format!("c{i}"), "p", "q"))
            .collect(),
    };
    let err = client_at(1).create_conclusions(&too_many).unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Config(_)), "{err}");

    let empty_key = Client::builder()
        .base_url("http://127.0.0.1:1")
        .workspace_id("box")
        .api_key("")
        .build();
    assert!(matches!(empty_key, Err(roncho::blocking::Error::Config(_))));
}

#[test]
fn probe_request_and_body_caps_on_chunked_and_close() {
    let (port, rx) = serve(json_ok(r#"{"status":"ok"}"#));
    let probe = client_at(port).probe().unwrap();
    assert_eq!(probe.status, 200);
    let raw = request_text(rx);
    assert!(raw.starts_with("GET /health "), "{raw}");

    let (port, _) = serve(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n20\r\n0123456789abcdef0123456789abcdef\r\n0\r\n\r\n"
            .to_vec(),
    );
    let err = Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .max_body(16)
        .read_timeout(Duration::from_secs(1))
        .build()
        .unwrap()
        .probe()
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::TooLarge), "{err}");

    let mut close_body = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    close_body.extend(std::iter::repeat(b'x').take(64));
    let (port, _) = serve(close_body);
    let err = Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .max_body(16)
        .read_timeout(Duration::from_secs(1))
        .build()
        .unwrap()
        .probe()
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::TooLarge), "{err}");
}

#[test]
fn queue_status_sends_filters() {
    let body = r#"{"completed_work_units":1,"in_progress_work_units":0,"pending_work_units":2,"total_work_units":3}"#;
    let (port, rx) = serve(json_ok(body));
    let status = client_at(port)
        .queue_status(
            &roncho::QueueStatusQuery::new()
                .observer_id("owner")
                .session_id("s"),
        )
        .unwrap();
    assert_eq!(status.pending_work_units, 2);
    let raw = request_text(rx);
    assert!(raw.starts_with("GET /v3/workspaces/box/queue/status?"));
    assert!(raw.contains("observer_id=owner"));
    assert!(raw.contains("session_id=s"));
}

fn chunk(data: &str) -> Vec<u8> {
    format!("{:x}\r\n{data}\r\n", data.len()).into_bytes()
}

#[test]
fn chat_stream_reassembles_events_and_times_out_between_them() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let Some(mut sock) = accept_limited(listener) else {
            return;
        };
        sock.set_nodelay(true).unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            let n = sock.read(&mut tmp).unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let _ = tx.send(buf);
        let head = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
        sock.write_all(head).unwrap();
        sock.write_all(&chunk("data: {\"delta\":{\"content\":\"Hel"))
            .unwrap();
        sock.flush().unwrap();
        thread::sleep(Duration::from_millis(40));
        sock.write_all(&chunk("lo\"},\"done\":false}\n\n")).unwrap();
        sock.write_all(&chunk("data: {\"done\":true}\n\n")).unwrap();
        sock.write_all(b"0\r\n\r\n").unwrap();
    });
    let client = client_at(port);
    let stream = client
        .peer_chat_stream(
            "p",
            &DialecticOptions {
                query: "q".into(),
                ..DialecticOptions::default()
            },
        )
        .unwrap();
    let request = String::from_utf8(rx.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
    assert!(
        request.contains(r#""stream":true"#),
        "stream request omitted stream:true: {request}"
    );
    let mut events: Vec<_> = stream.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(events[0].content, "Hello");
    assert!(events.last().unwrap().done);
    let _ = events.pop();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let Some(mut sock) = accept_limited(listener) else {
            return;
        };
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf);
        let head = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
        sock.write_all(head).unwrap();
        sock.write_all(&chunk(
            "data: {\"delta\":{\"content\":\"Hi\"},\"done\":false}\n\n",
        ))
        .unwrap();
        sock.flush().unwrap();
        thread::sleep(Duration::from_millis(400));
    });
    let client = Client::builder()
        .base_url(format!("http://127.0.0.1:{port}"))
        .workspace_id("box")
        .read_timeout(Duration::from_millis(100))
        .connect_timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let mut stream = client
        .peer_chat_stream(
            "p",
            &DialecticOptions {
                query: "q".into(),
                ..DialecticOptions::default()
            },
        )
        .unwrap();
    let first = stream.next().unwrap().unwrap();
    assert_eq!(first.content, "Hi");
    let err = stream.next().unwrap().unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Timeout));
}

#[test]
fn chat_stream_rejects_plain_json_and_rejoins_utf8() {
    let (port, _) = serve(json_ok(r#"{"content":"yes"}"#));
    let err = client_at(port)
        .peer_chat_stream(
            "p",
            &DialecticOptions {
                query: "q".into(),
                ..DialecticOptions::default()
            },
        )
        .unwrap()
        .next()
        .unwrap()
        .unwrap_err();
    assert!(matches!(err, roncho::blocking::Error::Stream(_)), "{err}");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let Some(mut sock) = accept_limited(listener) else {
            return;
        };
        sock.set_nodelay(true).unwrap();
        let mut tmp = [0u8; 2048];
        let mut buf = Vec::new();
        loop {
            let n = sock.read(&mut tmp).unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let head = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
        sock.write_all(head).unwrap();
        let mut first = b"data: {\"delta\":{\"content\":\"caf".to_vec();
        first.push(0xC3);
        sock.write_all(&chunk_bytes(&first)).unwrap();
        sock.flush().unwrap();
        thread::sleep(Duration::from_millis(40));
        let mut second = vec![0xA9];
        second.extend_from_slice(b"\"},\"done\":false}\n\n");
        sock.write_all(&chunk_bytes(&second)).unwrap();
        sock.write_all(&chunk_bytes(b"data: {\"done\":true}\n\n"))
            .unwrap();
        sock.write_all(b"0\r\n\r\n").unwrap();
    });
    let text: String = client_at(port)
        .peer_chat_stream(
            "p",
            &DialecticOptions {
                query: "q".into(),
                ..DialecticOptions::default()
            },
        )
        .unwrap()
        .map(|item| item.unwrap())
        .map(|chunk| chunk.content)
        .collect();
    assert_eq!(text, "caf\u{e9}");
}

fn chunk_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = format!("{:x}\r\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\r\n");
    out
}

/// Live checks against `RONCHO_LIVE_URL`. Ignored unless run by name.
/// Writes only to a throwaway workspace and deletes it.
#[test]
#[ignore]
fn live_server_throwaway_workspace() {
    let base = std::env::var("RONCHO_LIVE_URL")
        .expect("set RONCHO_LIVE_URL to the server this suite should call");
    let workspace = format!("roncho-test-{}", std::process::id());
    let mut builder = Client::builder()
        .base_url(&base)
        .workspace_id(&workspace)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(90));
    if let Ok(key) = std::env::var("RONCHO_LIVE_KEY") {
        if !key.is_empty() {
            builder = builder.api_key(key);
        }
    }
    let client = builder.build().unwrap();
    match Client::builder()
        .base_url(&base)
        .workspace_id(&workspace)
        .api_key("roncho-refused-key")
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(15))
        .build()
        .unwrap()
        .probe()
    {
        Err(roncho::blocking::Error::Unauthorized { status }) => {
            println!("REFUSED_KEY {status}");
        }
        Ok(probe) => println!(
            "REFUSED_KEY accepted status={} (authentication still off)",
            probe.status
        ),
        Err(err) => panic!("refused key returned {err}"),
    }
    struct Cleanup(Client);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let id = self.0.workspace_id().to_string();
            let _ = self.0.delete_session("quiet-s");
            let _ = self.0.delete_session("s");
            let _ = self.0.delete_workspace(&id);
        }
    }
    let _cleanup = Cleanup(client.clone());

    let probe = client.probe().expect("health");
    println!("PROBE {} {}", probe.status, probe.body);

    let ws = client
        .get_or_create_workspace(&WorkspaceCreate::new(&workspace))
        .expect("workspace");
    println!("WORKSPACE {}", ws.id);

    let peer = client.peer(&PeerCreate::new("owner")).expect("peer");
    println!("PEER {}", peer.id);

    let session = client
        .session(
            &SessionCreate::new("s").with_peers(
                [(
                    "owner".into(),
                    roncho::SessionPeerConfig {
                        observe_me: Some(true),
                        observe_others: Some(false),
                    },
                )]
                .into_iter()
                .collect(),
            ),
        )
        .expect("session");
    println!("SESSION {} active={}", session.id, session.is_active);

    let conflict = client.delete_workspace(&workspace).expect_err("409");
    println!("DELETE_WS_WHILE_ACTIVE {conflict:?}");

    let messages = client
        .add_messages(
            "s",
            &[MessageCreate::new("hello ping", "owner").with_created_at("2024-01-01T00:00:00Z")],
        )
        .expect("messages");
    println!("MESSAGE {}", messages[0].id);

    let page = client
        .list_messages("s", &ListOptions::default().page(1).size(10), None)
        .expect("list");
    println!("LIST {} items", page.items.len());

    let mut search = MessageSearch::new("hello");
    search.limit = Some(5);
    let found = client.search_workspace(&search).expect("search");
    println!("SEARCH {}", found.len());
    let peer_hits = client.search_peer("owner", &search).expect("peer search");
    println!("PEER_SEARCH {}", peer_hits.len());
    let session_hits = client.search_session("s", &search).expect("session search");
    println!("SESSION_SEARCH {}", session_hits.len());

    let batch = ConclusionBatchCreate {
        conclusions: vec![ConclusionCreate::new("noted fact", "owner", "owner")],
    };
    let made = client.create_conclusions(&batch).expect("conclusion");
    println!("CONCLUSION {}", made[0].id);
    let mut query = ConclusionQuery::new("noted fact");
    query.filters = Some(
        serde_json::json!({"observer_id": "owner", "observed_id": "owner"})
            .as_object()
            .cloned()
            .unwrap(),
    );
    let queried = client.query_conclusions(&query).expect("query");
    println!("QUERY {}", queried.len());
    client
        .delete_conclusion(&made[0].id)
        .expect("delete conclusion");

    let answer = client
        .peer_chat(
            "owner",
            &DialecticOptions {
                query: "What did the owner just say?".into(),
                session_id: Some("s".into()),
                reasoning_level: Some(roncho::ReasoningLevel::Minimal),
                ..DialecticOptions::default()
            },
        )
        .expect("chat");
    println!("CHAT {}", answer.content());

    let workspace_answer = client
        .workspace_chat(&DialecticOptions {
            query: "What did the owner just say?".into(),
            session_id: Some("s".into()),
            reasoning_level: Some(roncho::ReasoningLevel::Minimal),
            ..DialecticOptions::default()
        })
        .expect("workspace chat");
    println!("WORKSPACE_CHAT {}", workspace_answer.content());

    let evidenced = client
        .peer_chat(
            "owner",
            &DialecticOptions {
                query: "What did the owner just say?".into(),
                session_id: Some("s".into()),
                reasoning_level: Some(roncho::ReasoningLevel::Minimal),
                include_evidence: Some(true),
                ..DialecticOptions::default()
            },
        )
        .expect("evidence");
    println!(
        "EVIDENCE present={} messages={}",
        evidenced.evidence.is_some(),
        evidenced
            .evidence
            .as_ref()
            .map(|item| item.messages.len())
            .unwrap_or(0)
    );

    let streamed: String = client
        .peer_chat_stream(
            "owner",
            &DialecticOptions {
                query: "Reply with the word ping.".into(),
                session_id: Some("s".into()),
                reasoning_level: Some(roncho::ReasoningLevel::Minimal),
                ..DialecticOptions::default()
            },
        )
        .expect("stream")
        .map(|item| item.expect("stream item"))
        .map(|chunk| chunk.content)
        .collect();
    println!("STREAM {streamed}");

    let queue = client
        .queue_status(&roncho::QueueStatusQuery::new().session_id("s"))
        .expect("queue");
    println!(
        "QUEUE pending={} in_progress={} completed={} total={}",
        queue.pending_work_units,
        queue.in_progress_work_units,
        queue.completed_work_units,
        queue.total_work_units
    );

    for sample in 0..3 {
        let token = format!("roncho-fact-{sample}-{}", std::process::id());
        let started = std::time::Instant::now();
        client
            .add_messages(
                "s",
                &[MessageCreate::new(
                    format!("Remember this exact token {token}"),
                    "owner",
                )],
            )
            .expect("timed message");
        let mut elapsed = None;
        while started.elapsed() < Duration::from_secs(75) {
            let mut timed = ConclusionQuery::new(&token);
            timed.filters = Some(
                serde_json::json!({"observer_id": "owner", "observed_id": "owner"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            );
            let rows = client.query_conclusions(&timed).unwrap_or_default();
            if rows.iter().any(|row| row.content.contains(&token)) {
                elapsed = Some(started.elapsed());
                break;
            }
            thread::sleep(Duration::from_secs(5));
        }
        let status = client
            .queue_status(&roncho::QueueStatusQuery::new().session_id("s"))
            .expect("queue during timing");
        println!(
            "TIMING sample={sample} elapsed={elapsed:?} pending={} completed={}",
            status.pending_work_units, status.completed_work_units
        );
    }

    client
        .peer(
            &PeerCreate::new("quiet").with_configuration(
                serde_json::json!({"observe_me": false})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .expect("quiet peer");
    client
        .session(
            &SessionCreate::new("quiet-s").with_peers(
                [(
                    "quiet".into(),
                    roncho::SessionPeerConfig {
                        observe_me: Some(false),
                        observe_others: Some(false),
                    },
                )]
                .into_iter()
                .collect(),
            ),
        )
        .expect("quiet session");
    let quiet_token = format!("roncho-quiet-{}", std::process::id());
    let before = client
        .queue_status(&roncho::QueueStatusQuery::new().session_id("quiet-s"))
        .expect("quiet queue before");
    client
        .add_messages(
            "quiet-s",
            &[MessageCreate::new(
                format!("Do not derive {quiet_token}"),
                "quiet",
            )],
        )
        .expect("quiet message");
    thread::sleep(Duration::from_secs(20));
    let after = client
        .queue_status(&roncho::QueueStatusQuery::new().session_id("quiet-s"))
        .expect("quiet queue after");
    let mut quiet_query = ConclusionQuery::new(&quiet_token);
    quiet_query.filters = Some(
        serde_json::json!({"observer": "quiet", "observed": "quiet"})
            .as_object()
            .cloned()
            .unwrap(),
    );
    let quiet_rows = client.query_conclusions(&quiet_query).unwrap_or_default();
    println!(
        "OBSERVE_ME_FALSE before_pending={} after_pending={} conclusions={}",
        before.pending_work_units,
        after.pending_work_units,
        quiet_rows.len()
    );

    let noreason_id = format!("{workspace}-noreason");
    let mut noreason_builder = Client::builder()
        .base_url(&base)
        .workspace_id(&noreason_id)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(30));
    if let Ok(key) = std::env::var("RONCHO_LIVE_KEY") {
        if !key.is_empty() {
            noreason_builder = noreason_builder.api_key(key);
        }
    }
    let noreason = noreason_builder.build().unwrap();
    noreason
        .get_or_create_workspace(
            &WorkspaceCreate::new(&noreason_id).with_configuration(
                serde_json::json!({"reasoning": {"enabled": false}})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .expect("noreason workspace");
    noreason
        .peer(&PeerCreate::new("owner"))
        .expect("noreason peer");
    noreason
        .session(
            &SessionCreate::new("s").with_peers(
                [(
                    "owner".into(),
                    roncho::SessionPeerConfig {
                        observe_me: Some(true),
                        observe_others: Some(false),
                    },
                )]
                .into_iter()
                .collect(),
            ),
        )
        .expect("noreason session");
    let off_token = format!("roncho-off-{}", std::process::id());
    noreason
        .add_messages(
            "s",
            &[MessageCreate::new(
                format!("Reasoning is off {off_token}"),
                "owner",
            )],
        )
        .expect("noreason message");
    thread::sleep(Duration::from_secs(20));
    let off_queue = noreason
        .queue_status(&roncho::QueueStatusQuery::new().session_id("s"))
        .expect("noreason queue");
    println!(
        "REASONING_OFF pending={} completed={} total={}",
        off_queue.pending_work_units, off_queue.completed_work_units, off_queue.total_work_units
    );
    let _ = noreason.delete_session("s");
    noreason
        .delete_workspace(&noreason_id)
        .expect("delete noreason workspace");

    client
        .delete_session("quiet-s")
        .expect("delete quiet session");
    client.delete_session("s").expect("delete session");
    client
        .delete_workspace(&workspace)
        .expect("delete workspace");
    println!("CLEANED {workspace}");
}
