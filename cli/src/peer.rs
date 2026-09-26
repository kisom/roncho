use anyhow::Result;
use clap::{Args, Subcommand};
use futures::StreamExt;
use roncho::models::chat::{DialecticOptions, ReasoningLevel};
use roncho::models::message::Message;
use roncho::models::page::{ListOptions, Page};
use roncho::models::peer::Peer as PeerModel;
use roncho::models::session::Session as SessionModel;
use roncho::resources::peer::Peer;
use roncho::Honcho;

use crate::config;
use crate::format::{self, Mode};

#[derive(Copy, Clone, clap::ValueEnum)]
pub enum ReasoningArg {
    #[value(name = "minimal")]
    Minimal,
    #[value(name = "low")]
    Low,
    #[value(name = "medium")]
    Medium,
    #[value(name = "high")]
    High,
    Max,
}

impl From<ReasoningArg> for ReasoningLevel {
    fn from(r: ReasoningArg) -> Self {
        match r {
            ReasoningArg::Minimal => ReasoningLevel::Minimal,
            ReasoningArg::Low => ReasoningLevel::Low,
            ReasoningArg::Medium => ReasoningLevel::Medium,
            ReasoningArg::High => ReasoningLevel::High,
            ReasoningArg::Max => ReasoningLevel::Max,
        }
    }
}

#[derive(Args, Default, Clone)]
pub struct PageArgs {
    #[arg(long)]
    pub page: Option<usize>,
    #[arg(long)]
    pub size: Option<usize>,
    #[arg(long)]
    pub reverse: Option<bool>,
}

impl PageArgs {
    pub fn to_list_options(&self) -> ListOptions {
        let mut opts = ListOptions::default()
            .page(self.page.unwrap_or(1))
            .size(self.size.unwrap_or(50));
        if let Some(reverse) = self.reverse {
            opts = opts.reverse(reverse);
        }
        opts
    }
}

#[derive(Args, Clone)]
pub struct ChatArgs {
    pub query: String,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long, value_enum)]
    pub reasoning: Option<ReasoningArg>,
    #[arg(long)]
    pub evidence: bool,
}

impl ChatArgs {
    pub fn dialectic(&self) -> DialecticOptions {
        DialecticOptions {
            session_id: self.session.clone(),
            filters: None,
            target: self.target.clone(),
            scope: None,
            query: self.query.clone(),
            stream: None,
            reasoning_level: self.reasoning.map(|r| r.into()),
            response_format: None,
            include_evidence: if self.evidence { Some(true) } else { None },
        }
    }
}

#[derive(Args, Clone)]
pub struct StreamArgs {
    pub peer: String,
    pub query: String,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long, value_enum)]
    pub reasoning: Option<ReasoningArg>,
}

impl StreamArgs {
    pub fn dialectic(&self) -> DialecticOptions {
        DialecticOptions {
            session_id: self.session.clone(),
            filters: None,
            target: self.target.clone(),
            scope: None,
            query: self.query.clone(),
            stream: Some(true),
            reasoning_level: self.reasoning.map(|r| r.into()),
            response_format: None,
            include_evidence: None,
        }
    }
}

#[derive(Args, Clone)]
pub struct SearchArgs {
    pub query: String,
}

#[derive(Subcommand)]
pub enum PeerCmd {
    /// Get or create a peer
    Get { id: String },
    /// List peers
    List(PageArgs),
    /// Ask a peer a question
    Chat {
        id: String,
        #[command(flatten)]
        chat: ChatArgs,
    },
    /// Search this peer's messages
    Search { id: String, query: String },
    /// List this peer's sessions
    Sessions {
        id: String,
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        size: Option<usize>,
    },
    /// Get the peer card (working biography)
    Card {
        id: String,
        #[arg(long)]
        target: Option<String>,
    },
}

pub async fn run(mode: &Mode, cmd: PeerCmd) -> Result<()> {
    let honcho = config::build()?;
    match cmd {
        PeerCmd::Get { id } => cmd_get(&honcho, &id, *mode).await,
        PeerCmd::List(args) => cmd_list(&honcho, &args, *mode).await,
        PeerCmd::Chat { id, chat } => cmd_chat(&honcho, &id, &chat, *mode).await,
        PeerCmd::Search { id, query } => cmd_search(&honcho, &id, &query, *mode).await,
        PeerCmd::Sessions { id, page, size } => cmd_sessions(&honcho, &id, page, size, *mode).await,
        PeerCmd::Card { id, target } => cmd_card(&honcho, &id, target.as_deref(), *mode).await,
    }
}

pub async fn run_workspace_chat(mode: &Mode, args: ChatArgs) -> Result<()> {
    let honcho = config::build()?;
    let dialectic = args.dialectic();
    let resp = honcho.chat(args.query, Some(dialectic)).await?;
    format::emit(*mode, resp.content(), &resp);
    Ok(())
}

pub async fn run_workspace_search(mode: &Mode, args: SearchArgs) -> Result<()> {
    let honcho = config::build()?;
    let results: Page<Message> = honcho.search(args.query).await?;
    format::page(*mode, &results, |m| {
        format!(
            "{}  [{}]  {}",
            m.id,
            m.role,
            format::truncate(&m.content, 100)
        )
    });
    Ok(())
}

pub async fn run_stream(mode: &Mode, args: StreamArgs) -> Result<()> {
    let honcho = config::build()?;
    let dialectic = args.dialectic();
    let peer = honcho.peer(args.peer).await?;
    let mut stream = peer.chat_stream(args.query, Some(dialectic));
    let mut out = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        out.push_str(&chunk.content);
    }
    format::emit(*mode, &out, &serde_json::json!({ "streamed": true }));
    Ok(())
}

async fn cmd_get(honcho: &Honcho, id: &str, mode: Mode) -> Result<()> {
    let peer = honcho.peer(id).await?;
    format::emit(mode, &format_peer(&peer), &peer.to_model());
    Ok(())
}

async fn cmd_list(honcho: &Honcho, args: &PageArgs, mode: Mode) -> Result<()> {
    let page: Page<PeerModel> = honcho.peers(&args.to_list_options()).await?;
    format::page(mode, &page, |p| {
        format!("{}  {}  {}", p.id, p.display_name, p.workspace_id)
    });
    Ok(())
}

async fn cmd_chat(honcho: &Honcho, id: &str, args: &ChatArgs, mode: Mode) -> Result<()> {
    let peer = honcho.peer(id).await?;
    let resp = peer.chat(&args.query, Some(args.dialectic())).await?;
    format::emit(mode, resp.content(), &resp);
    Ok(())
}

async fn cmd_search(honcho: &Honcho, id: &str, query: &str, mode: Mode) -> Result<()> {
    let peer = honcho.peer(id).await?;
    let results: Page<Message> = peer.search(query, &ListOptions::default()).await?;
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

async fn cmd_sessions(
    honcho: &Honcho,
    id: &str,
    page: Option<usize>,
    size: Option<usize>,
    mode: Mode,
) -> Result<()> {
    let peer = honcho.peer(id).await?;
    let opts = ListOptions::default()
        .page(page.unwrap_or(1))
        .size(size.unwrap_or(50));
    let page = peer.sessions(&opts).await?;
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

async fn cmd_card(honcho: &Honcho, id: &str, target: Option<&str>, mode: Mode) -> Result<()> {
    let peer = honcho.peer(id).await?;
    let card = peer.get_card(target).await?;
    let human = card.join("\n");
    format::emit(mode, &human, &card);
    Ok(())
}

fn format_peer(p: &Peer) -> String {
    let mut s = format!(
        "id:        {}\ndisplay:   {}\nworkspace: {}\ncreated:   {}",
        p.id,
        p.display_name,
        p.workspace_id(),
        p.created_at().format("%Y-%m-%dT%H:%M:%SZ")
    );
    if !p.metadata.is_empty() {
        s.push_str(&format!("\nmetadata:    {}", format::json_map(&p.metadata)));
    }
    if !p.configuration.is_empty() {
        s.push_str(&format!(
            "\nconfig:      {}",
            format::json_map(&p.configuration)
        ));
    }
    s
}
