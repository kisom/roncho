mod conclusion;
mod config;
mod format;
mod peer;
mod session;

use anyhow::Result;
use clap::{Parser, Subcommand};
use format::Mode;

#[derive(Parser)]
#[command(name = "roncho", version, about = "CLI for the Honcho memory platform")]
struct Cli {
    /// Emit JSON instead of human-readable output
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

impl Cli {
    fn mode(&self) -> Mode {
        if self.json {
            Mode::Json
        } else {
            Mode::Human
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Show the effective configuration
    Config,
    /// Work with peers
    #[command(subcommand)]
    Peer(peer::PeerCmd),
    /// Work with sessions
    #[command(subcommand)]
    Session(session::SessionCmd),
    /// Ask the whole workspace a question
    Chat(#[command(flatten)] peer::ChatArgs),
    /// Search across all content in the workspace
    Search(#[command(flatten)] peer::SearchArgs),
    /// Stream a chat response
    Stream(#[command(flatten)] peer::StreamArgs),
    /// Work with conclusions
    #[command(subcommand)]
    Conclusion(conclusion::ConclusionCmd),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mode = cli.mode();
    match cli.command {
        Command::Config => config::cmd_config(mode).await,
        Command::Peer(cmd) => peer::run(&mode, cmd).await,
        Command::Session(cmd) => session::run(&mode, cmd).await,
        Command::Chat(args) => peer::run_workspace_chat(&mode, args).await,
        Command::Search(args) => peer::run_workspace_search(&mode, args).await,
        Command::Stream(args) => peer::run_stream(&mode, args).await,
        Command::Conclusion(cmd) => conclusion::run(&mode, cmd).await,
    }
}
