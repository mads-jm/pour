use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A single saved preset for a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetEntry {
    pub name: String,
    /// Maps field_name -> field_value for all fields captured in this preset.
    pub values: HashMap<String, String>,
}

/// On-disk schema for the presets file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetsData {
    /// Maps module_key -> ordered list of presets.
    #[serde(default)]
    pub modules: HashMap<String, Vec<PresetEntry>>,
}

/// Manages reading and writing the `presets.json` file that stores
/// per-module named presets at `~/.cache/pour/presets.json`.
#[derive(Debug)]
pub struct Presets {
    data: PresetsData,
    path: PathBuf,
}

impl Presets {
    /// Load presets from the default platform cache directory
    /// (`~/.cache/pour/presets.json` on Linux, equivalent on other OSes).
    ///
    /// Returns empty presets if the file is missing or corrupt.
    pub fn load() -> Self {
        let path = default_presets_path();
        Self::load_from(path)
    }

    /// Create an empty `Presets` instance not backed by any file.
    ///
    /// Useful in tests or contexts where preset persistence is not needed.
    /// Calling `save()` on an instance created this way will attempt to write
    /// to a non-existent path — callers that do not intend to persist should
    /// simply avoid calling `save()`.
    pub fn empty() -> Self {
        Presets {
            data: PresetsData::default(),
            path: PathBuf::new(),
        }
    }

    /// Load presets from a specific file path.
    ///
    /// Returns empty presets if the file is missing or corrupt.
    pub fn load_from(path: PathBuf) -> Self {
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str::<PresetsData>(&contents).ok())
            .unwrap_or_default();

        Presets { data, path }
    }

    /// Persist presets to disk, creating parent directories if needed.
    ///
    /// Uses atomic write (temp file + rename) to avoid corruption if the
    /// process is interrupted mid-write.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.data)?;
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, &json)?;
        crate::util::atomic_replace(&tmp_path, &self.path)?;
        Ok(())
    }

    /// Return the ordered list of presets for `module_key`.
    ///
    /// Returns an empty vec if the module has no saved presets.
    pub fn get(&self, module_key: &str) -> Vec<PresetEntry> {
        self.data
            .modules
            .get(module_key)
            .cloned()
            .unwrap_or_default()
    }

    /// Upsert a preset for `module_key` by name.
    ///
    /// If a preset with `entry.name` already exists it is overwritten in place.
    /// Otherwise the entry is appended to the end of the list.
    pub fn set(&mut self, module_key: &str, entry: PresetEntry) {
        let list = self
            .data
            .modules
            .entry(module_key.to_owned())
            .or_default();

        if let Some(existing) = list.iter_mut().find(|p| p.name == entry.name) {
            *existing = entry;
        } else {
            list.push(entry);
        }
    }

    /// Remove the preset named `preset_name` from `module_key`.
    ///
    /// Returns `true` if the preset existed and was removed, `false` if it
    /// was not found.
    pub fn delete(&mut self, module_key: &str, preset_name: &str) -> bool {
        let Some(list) = self.data.modules.get_mut(module_key) else {
            return false;
        };

        let Some(idx) = list.iter().position(|p| p.name == preset_name) else {
            return false;
        };
        list.remove(idx);
        true
    }

    /// Swap the preset named `preset_name` with its neighbor in `module_key`.
    ///
    /// `direction` is `+1` to move forward (toward the end) or `-1` to move
    /// backward (toward the start). No-op when the preset is at a boundary or
    /// not found.
    pub fn reorder(&mut self, module_key: &str, preset_name: &str, direction: i32) {
        let Some(list) = self.data.modules.get_mut(module_key) else {
            return;
        };

        let Some(idx) = list.iter().position(|p| p.name == preset_name) else {
            return;
        };

        let new_idx = match direction {
            d if d > 0 => {
                if idx + 1 >= list.len() {
                    return; // already at end
                }
                idx + 1
            }
            d if d < 0 => {
                if idx == 0 {
                    return; // already at start
                }
                idx - 1
            }
            _ => return, // direction == 0, no-op
        };

        list.swap(idx, new_idx);
    }
}

/// Resolve the default presets file path using the platform cache directory.
fn default_presets_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("pour")
        .join("presets.json")
}
