use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A single cached entry for a dynamic select source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub items: Vec<String>,
    pub updated_at: String,
}

/// On-disk schema for the cache file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheData {
    #[serde(default)]
    pub dynamic_selects: HashMap<String, CacheEntry>,
}

/// Manages reading and writing the `state.json` cache file that backs
/// dynamic select dropdowns when the API and filesystem are unavailable.
#[derive(Debug)]
pub struct Cache {
    data: CacheData,
    path: PathBuf,
}

impl Cache {
    /// Load the cache from the default platform cache directory
    /// (`~/.cache/pour/state.json` on Linux, equivalent on other OSes).
    ///
    /// Returns an empty cache if the file is missing or corrupt.
    pub fn load() -> Self {
        let path = default_cache_path();
        Self::load_from(path)
    }

    /// Load the cache from a specific file path.
    ///
    /// Returns an empty cache if the file is missing or corrupt.
    pub fn load_from(path: PathBuf) -> Self {
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str::<CacheData>(&contents).ok())
            .unwrap_or_default();

        Cache { data, path }
    }

    /// Return the cached items for `source`, or `None` if no entry exists.
    pub fn get(&self, source: &str) -> Option<Vec<String>> {
        self.data
            .dynamic_selects
            .get(source)
            .map(|entry| entry.items.clone())
    }

    /// Insert or update the cached items for `source`, stamping the
    /// current UTC time as `updated_at`.
    pub fn set(&mut self, source: &str, items: Vec<String>) {
        let entry = CacheEntry {
            items,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        self.data.dynamic_selects.insert(source.to_owned(), entry);
    }

    /// Persist the cache to disk, creating parent directories if needed.
    ///
    /// Uses atomic write (temp file + rename) to avoid corruption if the
    /// process is interrupted mid-write.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.data)?;
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, json)?;
        crate::util::atomic_replace(&tmp_path, &self.path)?;
        Ok(())
    }
}

/// Resolve the default cache file path using the platform cache directory.
fn default_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("pour")
        .join("state.json")
}
