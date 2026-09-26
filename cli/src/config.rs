use anyhow::{Context, Result};
use roncho::config::FileConfig;
use roncho::Honcho;

use crate::format::{self, Mode};

pub fn config_path() -> Option<std::path::PathBuf> {
    roncho::config::default_path()
}

pub fn load_file() -> Result<FileConfig> {
    roncho::config::load_default()
        .map(|cfg| cfg.unwrap_or_default())
        .map_err(|err| anyhow::anyhow!(err))
        .context("config file")
}

pub fn build() -> Result<Honcho> {
    let cfg = load_file()?;
    let builder = Honcho::builder();
    let builder = match cfg.workspace_id {
        Some(w) => builder.workspace_id(w),
        None => builder,
    };
    let builder = match cfg.api_key {
        Some(k) => builder.api_key(k),
        None => builder,
    };
    let builder = match cfg.base_url {
        Some(u) => builder.base_url(u),
        None => builder,
    };
    let honcho = builder.build().context("failed to build Honcho client")?;
    Ok(honcho)
}

pub async fn cmd_config(mode: Mode) -> Result<()> {
    let file = load_file()?;
    let resolved = build();
    let (workspace_id, base_url, api_key_present) = match &resolved {
        Ok(h) => (
            Some(h.workspace_id().to_string()),
            Some(h.base_url().to_string()),
            true,
        ),
        Err(_) => (
            file.workspace_id
                .clone()
                .or_else(|| std::env::var("HONCHO_WORKSPACE_ID").ok()),
            file.base_url
                .clone()
                .or_else(|| std::env::var("HONCHO_BASE_URL").ok()),
            file.api_key.is_some() || std::env::var_os("HONCHO_API_KEY").is_some(),
        ),
    };
    let api_key_source = if file.api_key.is_some() {
        "config file"
    } else if std::env::var_os("HONCHO_API_KEY").is_some() {
        "environment"
    } else {
        "none"
    };
    let value = serde_json::json!({
        "config_file": config_path().map(|p| p.display().to_string()),
        "workspace_id": workspace_id,
        "base_url": base_url,
        "api_key_present": api_key_present,
        "api_key_source": api_key_source,
    });
    let human = format!(
        "config file: {}\nworkspace_id: {}\nbase_url: {}\napi_key: {} ({})",
        config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(none; using environment)".to_string()),
        workspace_id.unwrap_or_else(|| "(unset)".to_string()),
        base_url.unwrap_or_else(|| "(unset)".to_string()),
        if api_key_present {
            "present"
        } else {
            "MISSING"
        },
        api_key_source
    );
    format::emit(mode, &human, &value);
    Ok(())
}
