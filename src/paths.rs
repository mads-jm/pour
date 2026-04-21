//! Central resolver for every user-data path Pour writes.
//!
//! All files live under a single root (`POUR_HOME`, defaulting to `~/.pour/`):
//!
//! ```text
//! ~/.pour/
//!   config.toml
//!   secrets.toml
//!   presets.json
//!   cache/
//!     state.json
//!     history.jsonl
//!     history-summary.json
//! ```
//!
//! `POUR_CONFIG` still overrides the `config.toml` location (tests rely on
//! this), in which case `secrets.toml` sits beside the overridden config.

use std::path::PathBuf;

/// Root directory for all Pour state. Respects `POUR_HOME`, otherwise
/// `~/.pour/`. Falls back to `./.pour` if the home directory cannot be
/// resolved.
pub fn pour_home() -> PathBuf {
    if let Ok(env_path) = std::env::var("POUR_HOME")
        && !env_path.is_empty()
    {
        return PathBuf::from(env_path);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pour")
}

/// Path to `config.toml`. `POUR_CONFIG` takes precedence over `POUR_HOME`.
pub fn config_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("POUR_CONFIG")
        && !env_path.is_empty()
    {
        return PathBuf::from(env_path);
    }
    pour_home().join("config.toml")
}

/// Path to `secrets.toml` (always sibling of `config.toml`).
pub fn secrets_path() -> PathBuf {
    config_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secrets.toml")
}

/// Path to `presets.json`. Lives at the root of `pour_home()` — user-curated,
/// not ephemeral.
pub fn presets_path() -> PathBuf {
    pour_home().join("presets.json")
}

/// Directory for ephemeral/regenerable cache files.
pub fn cache_dir() -> PathBuf {
    pour_home().join("cache")
}

/// Path to `state.json` (dynamic-select cache).
pub fn state_path() -> PathBuf {
    cache_dir().join("state.json")
}

/// Path to `history.jsonl` (capture log).
pub fn history_path() -> PathBuf {
    cache_dir().join("history.jsonl")
}

/// Path to `history-summary.json` (precomputed dashboard stats).
pub fn history_summary_path() -> PathBuf {
    cache_dir().join("history-summary.json")
}
