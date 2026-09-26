//! Shared `roncho.toml` used by the CLI and, when asked, by the blocking client.
//!
//! The file lives at `$XDG_CONFIG_HOME/roncho/roncho.toml`, or
//! `~/.config/roncho/roncho.toml` when `XDG_CONFIG_HOME` is unset.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FileConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub workspace_id: Option<String>,
}

pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{home}/.config"));
    Some(Path::new(&config_dir).join("roncho").join("roncho.toml"))
}

/// `Ok(None)` when the default path cannot be built or the file is not there.
pub fn load_default() -> Result<Option<FileConfig>, String> {
    let Some(path) = default_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    load_path(&path).map(Some)
}

pub fn load_path(path: &Path) -> Result<FileConfig, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|err| format!("reading {}: {err}", path.display()))?;
    toml::from_str(&contents).map_err(|err| format!("parsing {}: {err}", path.display()))
}
