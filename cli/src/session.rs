use anyhow::Result;
use clap::Subcommand;
use roncho::models::context::SessionContext;
use roncho::models::message::Message;
use roncho::models::page::{ListOptions, Page};
use roncho::models::session::{Session as SessionModel, SessionPeerConfig};
use roncho::resources::session::{Session, SessionContextRequest};
use roncho::Honcho;

use crate::config;
use crate::format::{self, Mode};
use crate::peer::PageArgs;

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Get or create a session
    Get { id: String },
    /// List sessions
    List(PageArgs),
    /// Add a message to a session
    Add {
        id: String,
        content: String,
        #[arg(long)]
        peer: String,
    },
    /// List messages in a session
    Messages {
        id: String,
        #[command(flatten)]
        page: PageArgs,
        /// JSON filter object sent to the API, e.g. '{"filters":{"query":"x"}}'
        #[arg(long)]
        filters: Option<String>,
    },
    /// Search within a session
    Search { id: String, query: String },
    /// Get formatted session context for an LLM
    Context {
        id: String,
        #[arg(long)]
        tokens: Option<u32>,
        #[arg(long)]
        summary: bool,
        #[arg(long)]
        peer_target: Option<String>,
        #[arg(long)]
        query: Option<String>,
    },
    /// Add a peer to a session
    AddPeer {
        id: String,
        peer: String,
        #[arg(long)]
        observe_me: bool,
        #[arg(long)]
        observe_others: bool,
    },
    /// Remove a peer from a session
    RemovePeer { id: String, peer: String },
    /// Clone a session up to a message
    Clone {
        id: String,
        #[arg(long = "up-to")]
        up_to: Option<String>,
    },
    /// Delete a session
    Delete { id: String },
}

pub async fn run(mode: &Mode, cmd: SessionCmd) -> Result<()> {
    let honcho = config::build()?;
    match cmd {
        SessionCmd::Get { id } => cmd_get(&honcho, &id, *mode).await,
        SessionCmd::List(args) => cmd_list(&honcho, &args, *mode).await,
        SessionCmd::Add { id, content, peer } => {
            cmd_add(&honcho, &id, &content, &peer, *mode).await
        }
        SessionCmd::Messages { id, page, filters } => {
            cmd_messages(&honcho, &id, &page, filters.as_deref(), *mode).await
        }
        SessionCmd::Search { id, query } => cmd_search(&honcho, &id, &query, *mode).await,
        SessionCmd::Context {
            id,
            tokens,
            summary,
            peer_target,
            query,
        } => cmd_context(&honcho, &id, tokens, summary, peer_target, query, *mode).await,
        SessionCmd::AddPeer {
            id,
            peer,
            observe_me,
            observe_others,
        } => cmd_add_peer(&honcho, &id, &peer, observe_me, observe_others, *mode).await,
        SessionCmd::RemovePeer { id, peer } => cmd_remove_peer(&honcho, &id, &peer, *mode).await,
        SessionCmd::Clone { id, up_to } => cmd_clone(&honcho, &id, up_to.as_deref(), *mode).await,
        SessionCmd::Delete { id } => cmd_delete(&honcho, &id, *mode).await,
    }
}

async fn cmd_get(honcho: &Honcho, id: &str, mode: Mode) -> Result<()> {
    let session = honcho.session(id).await?;
    format::emit(mode, &format_session(&session), &session.to_model());
    Ok(())
}

async fn cmd_list(honcho: &Honcho, args: &PageArgs, mode: Mode) -> Result<()> {
    let page = honcho.sessions(&args.to_list_options()).await?;
    let page: Page<SessionModel> = Page {
        items: page.items.into_iter().map(|s| s.to_model()).collect(),
        total: page.total,
        page: page.page,
        size: page.size,
        pages: page.pages,
    };
    format::page(mode, &page, |s| {
        let state = if s.is_active { "active" } else { "archived" };
        format!("{}  {}  {}", s.id, state, s.workspace_id)
    });
    Ok(())
}

async fn cmd_add(
    honcho: &Honcho,
    id: &str,
    content: &str,
    peer_id: &str,
    mode: Mode,
) -> Result<()> {
    let session = honcho.session(id).await?;
    let peer = honcho.peer(peer_id).await?;
    let message = peer.message(content);
    let created: Vec<Message> = session.add_messages(&[message]).await?;
    format::page(mode, &page_of(created), |m| {
        format!(
            "{}  [{}]  {}",
            m.id,
            m.role,
            format::truncate(&m.content, 100)
        )
    });
    Ok(())
}

async fn cmd_messages(
    honcho: &Honcho,
    id: &str,
    args: &PageArgs,
    filters: Option<&str>,
    mode: Mode,
) -> Result<()> {
    let session = honcho.session(id).await?;
    let parsed = match filters {
        Some(f) => Some(
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(f)
                .map_err(|e| anyhow::anyhow!("invalid --filters JSON: {e}"))?,
        ),
        None => None,
    };
    let results: Page<Message> = session.messages(&args.to_list_options(), parsed).await?;
    format::page(mode, &results, |m| {
        format!(
            "{}  [{}]  {}",
            m.id,
            m.role,
            format::truncate(&m.content, 100)
        )
    });
    Ok(())
}

async fn cmd_search(honcho: &Honcho, id: &str, query: &str, mode: Mode) -> Result<()> {
    let session = honcho.session(id).await?;
    let results: Page<Message> = session.search(query, &ListOptions::default()).await?;
    format::page(mode, &results, |m| {
        format!(
            "{}  [{}]  {}",
            m.id,
            m.role,
            format::truncate(&m.content, 100)
        )
    });
    Ok(())
}

async fn cmd_context(
    honcho: &Honcho,
    id: &str,
    tokens: Option<u32>,
    summary: bool,
    peer_target: Option<String>,
    query: Option<String>,
    mode: Mode,
) -> Result<()> {
    let session = honcho.session(id).await?;
    let mut req = SessionContextRequest::new();
    if let Some(t) = tokens {
        req = req.tokens(t);
    }
    if summary {
        req = req.summary(true);
    }
    if let Some(t) = peer_target {
        req = req.peer_target(t);
    }
    if let Some(q) = query {
        req = req.search_query(q);
    }
    let context: SessionContext = session.context(&req).await?;
    format::emit(mode, &format_context(&context), &context);
    Ok(())
}

async fn cmd_add_peer(
    honcho: &Honcho,
    id: &str,
    peer_id: &str,
    observe_me: bool,
    observe_others: bool,
    mode: Mode,
) -> Result<()> {
    let session = honcho.session(id).await?;
    let peer = honcho.peer(peer_id).await?;
    let mut cfg = SessionPeerConfig::default();
    if observe_me {
        cfg.observe_me = Some(true);
    }
    if observe_others {
        cfg.observe_others = Some(true);
    }
    session.add_peers(&[(&peer, Some(cfg))]).await?;
    format::emit(
        mode,
        &format!("added peer {} to session {}", peer_id, id),
        &serde_json::json!({ "session_id": id, "peer_id": peer_id }),
    );
    Ok(())
}

async fn cmd_remove_peer(honcho: &Honcho, id: &str, peer_id: &str, mode: Mode) -> Result<()> {
    let session = honcho.session(id).await?;
    session.remove_peers(&[peer_id]).await?;
    format::emit(
        mode,
        &format!("removed peer {} from session {}", peer_id, id),
        &serde_json::json!({ "session_id": id, "peer_id": peer_id }),
    );
    Ok(())
}

async fn cmd_clone(honcho: &Honcho, id: &str, up_to: Option<&str>, mode: Mode) -> Result<()> {
    let session = honcho.session(id).await?;
    let cloned = session.clone(up_to).await?;
    let label = match up_to {
        Some(u) => format!("cloned {id}..{u}"),
        None => format!("cloned {id}"),
    };
    format::emit(mode, &label, &cloned.to_model());
    Ok(())
}

async fn cmd_delete(honcho: &Honcho, id: &str, mode: Mode) -> Result<()> {
    let session = honcho.session(id).await?;
    session.delete().await?;
    format::emit(
        mode,
        &format!("deleted session {id}"),
        &serde_json::json!({ "deleted": id }),
    );
    Ok(())
}

fn page_of(messages: Vec<Message>) -> Page<Message> {
    let total = messages.len();
    Page {
        items: messages,
        total,
        page: 1,
        size: total,
        pages: 1,
    }
}

fn format_session(s: &Session) -> String {
    let mut out = format!(
        "id:        {}\nstate:     {}\nworkspace: {}\ncreated:   {}",
        s.id,
        if s.is_active { "active" } else { "archived" },
        s.workspace_id(),
        s.created_at().format("%Y-%m-%dT%H:%M:%SZ")
    );
    if !s.metadata.is_empty() {
        out.push_str(&format!("\nmetadata:  {}", format::json_map(&s.metadata)));
    }
    if !s.configuration.is_empty() {
        out.push_str(&format!(
            "\nconfig:    {}",
            format::json_map(&s.configuration)
        ));
    }
    out
}

fn format_context(c: &SessionContext) -> String {
    let mut out = format!("id: {}\n", c.id);
    match &c.summary {
        Some(s) => out.push_str(&format!(
            "summary:   {}\n",
            format::truncate(&s.content, 200)
        )),
        None => out.push_str("summary:   -\n"),
    }
    match &c.peer_representation {
        Some(r) => out.push_str(&format!("representation: {}\n", format::truncate(r, 200))),
        None => out.push_str("representation: -\n"),
    }
    match &c.peer_card {
        Some(card) if !card.is_empty() => out.push_str(&format!(
            "peer card: {}\n",
            format::truncate(&card.join(", "), 200)
        )),
        _ => out.push_str("peer card: -\n"),
    }
    out.push_str("messages:\n");
    for m in &c.messages {
        out.push_str(&format!(
            "  [{}] {}\n",
            m.role,
            format::truncate(&m.content, 120)
        ));
    }
    out
}
