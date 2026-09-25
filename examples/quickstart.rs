use roncho::models::chat::DialecticOptions;
use roncho::models::page::ListOptions;
use roncho::{Honcho, MessageCreate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let honcho = Honcho::builder().build()?;

    let peer = honcho
        .peer("Alice")
        .await
        .expect("failed to get or create peer");

    println!("Peer: {} ({})", peer.id, peer.display_name);

    let session = honcho
        .session("quickstart-session")
        .await
        .expect("failed to get or create session");

    println!("Session: {}", session.id);

    let _msg = session
        .add_messages(&[MessageCreate { content: "Hello, Honcho!".to_string(), peer_id: peer.id.clone(), metadata: None, created_at: None }])
        .await
        .expect("failed to add message");

    let opts = DialecticOptions {
        query: "What did I just say?".to_string(),
        session_id: Some(session.id.to_string()),
        filters: None,
        target: Some(peer.id.clone()),
        scope: None,
        stream: Some(true),
        reasoning_level: None,
        response_format: None,
        include_evidence: None,
    };

    let stream = peer.chat_stream(peer.id.clone(), Some(opts));
    let mut pinned_stream = std::pin::pin!(stream);
    print!("Assistant: ");
    use futures::StreamExt;
    while let Some(chunk) = pinned_stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.content);
    }
    println!();

    let messages = session
        .messages(&ListOptions::default(), None)
        .await
        .expect("failed to list messages");

    println!("\nMessages in session:");
    for msg in &messages.items {
        println!("  [{}] {}", msg.role, msg.content);
    }

    Ok(())
}
