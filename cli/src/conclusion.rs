use anyhow::Result;
use clap::Subcommand;
use roncho::models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionCreate, ConclusionListOptions,
};
use roncho::models::page::Page;
use roncho::resources::conclusions::Conclusions;

use crate::config;
use crate::format::{self, Mode};

#[derive(Subcommand)]
pub enum ConclusionCmd {
    /// Create one conclusion
    Create {
        content: String,
        #[arg(long)]
        observer: String,
        #[arg(long)]
        observed: String,
        #[arg(long)]
        session: Option<String>,
    },
    /// List conclusions
    List {
        /// JSON filter object, e.g. '{"observed_id":"alice"}'
        #[arg(long)]
        filters: Option<String>,
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        size: Option<usize>,
        #[arg(long)]
        reverse: Option<bool>,
    },
    /// Semantic search over conclusions
    Query {
        query: String,
        #[arg(long)]
        top_k: Option<u32>,
    },
    /// Fetch a single conclusion by ID
    Get { id: String },
    /// Delete a conclusion by ID
    Delete { id: String },
}

pub async fn run(mode: &Mode, cmd: ConclusionCmd) -> Result<()> {
    let honcho = config::build()?;
    let conclusions: Conclusions = honcho.conclusions();
    match cmd {
        ConclusionCmd::Create {
            content,
            observer,
            observed,
            session,
        } => cmd_create(&conclusions, content, observer, observed, session, *mode).await,
        ConclusionCmd::List {
            filters,
            page,
            size,
            reverse,
        } => cmd_list(&conclusions, filters.as_deref(), page, size, reverse, *mode).await,
        ConclusionCmd::Query { query, top_k } => cmd_query(&conclusions, query, top_k, *mode).await,
        ConclusionCmd::Get { id } => cmd_get(&conclusions, &id, *mode).await,
        ConclusionCmd::Delete { id } => cmd_delete(&conclusions, &id, *mode).await,
    }
}

async fn cmd_create(
    conclusions: &Conclusions,
    content: String,
    observer: String,
    observed: String,
    session: Option<String>,
    mode: Mode,
) -> Result<()> {
    let mut create = ConclusionCreate::new(content, observer, observed);
    if let Some(session) = session {
        create = create.with_session_id(session);
    }
    let batch = ConclusionBatchCreate::new(vec![create]);
    let created = conclusions.create(batch).await?;
    format::page(mode, &page_of(&created), |c| {
        format!(
            "{}  {} -> {}  {}",
            c.id,
            c.observer_id,
            c.observed_id,
            format::truncate(&c.content, 100)
        )
    });
    Ok(())
}

async fn cmd_list(
    conclusions: &Conclusions,
    filters: Option<&str>,
    page: Option<usize>,
    size: Option<usize>,
    reverse: Option<bool>,
    mode: Mode,
) -> Result<()> {
    let mut opts = ConclusionListOptions::new()
        .page(page.unwrap_or(1))
        .size(size.unwrap_or(50))
        .reverse(reverse.unwrap_or(false));
    if let Some(f) = filters {
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(f).map_err(|e| anyhow::anyhow!("invalid --filters JSON: {e}"))?;
        opts = opts.filters(map);
    }
    let result: Page<Conclusion> = conclusions.list(opts).await?;
    format::page(mode, &result, |c| {
        format!(
            "{}  {} -> {}  {}",
            c.id,
            c.observer_id,
            c.observed_id,
            format::truncate(&c.content, 100)
        )
    });
    Ok(())
}

async fn cmd_query(
    conclusions: &Conclusions,
    query: String,
    top_k: Option<u32>,
    mode: Mode,
) -> Result<()> {
    let results = conclusions.query(query, top_k).await?;
    format::page(mode, &page_of(&results), |c| {
        format!(
            "{}  {} -> {}  {}",
            c.id,
            c.observer_id,
            c.observed_id,
            format::truncate(&c.content, 100)
        )
    });
    Ok(())
}

async fn cmd_get(conclusions: &Conclusions, id: &str, mode: Mode) -> Result<()> {
    let conclusion: Conclusion = conclusions.get(id).await?;
    format::emit(mode, &format_conclusion(&conclusion), &conclusion);
    Ok(())
}

async fn cmd_delete(conclusions: &Conclusions, id: &str, mode: Mode) -> Result<()> {
    conclusions.delete(id).await?;
    format::emit(
        mode,
        &format!("deleted {id}"),
        &serde_json::json!({ "deleted": id }),
    );
    Ok(())
}

fn page_of(conclusions: &[Conclusion]) -> Page<Conclusion> {
    let total = conclusions.len();
    Page {
        items: conclusions.to_vec(),
        total,
        page: 1,
        size: total,
        pages: 1,
    }
}

fn format_conclusion(c: &Conclusion) -> String {
    let level = c.level;
    format!(
        "id:          {}\ncontent:     {}\nobserver:    {}\nobserved:    {}\nlevel:       {level:?}\ntimes:       {}\nsession:     {}\ncreated:     {}",
        c.id,
        format::truncate(&c.content, 200),
        c.observer_id,
        c.observed_id,
        c.times_derived,
        c.session_id.as_deref().unwrap_or("-"),
        c.created_at.format("%Y-%m-%dT%H:%M:%SZ")
    )
}
