use anyhow::{Context, Result};
use roncho::Honcho;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::format::{self, Mode};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub workspace_id: Option<String>,
}

pub fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| format!("{home}/.config"));
    Some(Path::new(&config_dir).join("roncho").join("roncho.toml"))
}

pub fn load_file() -> Result<FileConfig> {
    let path = match config_path() {
        Some(p) => p,
        None => return Ok(FileConfig::default()),
    };
    if !path.exists() {
        return Ok(FileConfig::default());
    }
    let contents = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "reading {} (keep your API key out of version control)",
            path.display()
        )
    })?;
    let cfg: FileConfig =
        toml::from_str(&contents).with_context(|| format!("parsing {}", path.display()))?;
    Ok(cfg)
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
