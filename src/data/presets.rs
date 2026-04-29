use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Outcome of `Presets::api_set` — distinguishes HTTP 201 (Created) vs 200
/// (Updated) per §6.8.
#[derive(Debug, PartialEq)]
pub enum SetResult {
    Created,
    Updated,
}

/// Error returned by `Presets::api_reorder` when the supplied name list is not
/// an exact permutation of the current preset names, or when the underlying
/// save fails.
#[derive(Debug)]
pub enum ReorderError {
    /// The request list contained duplicate names. Checked before missing/extra.
    DuplicateNames(Vec<String>),
    MissingNames(Vec<String>),
    ExtraNames(Vec<String>),
    SaveFailed(anyhow::Error),
}

/// A single saved preset for a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetEntry {
    pub name: String,
    /// Optional human-readable description for disambiguating similar presets.
    /// Rendered as a dim subtitle under the preset row in the form view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
/// per-module named presets at `~/.pour/presets.json`.
#[derive(Debug)]
pub struct Presets {
    data: PresetsData,
    path: PathBuf,
}

impl Presets {
    /// Load presets from the default location (`~/.pour/presets.json`,
    /// or `$POUR_HOME/presets.json` if overridden).
    ///
    /// Returns empty presets if the file is missing or corrupt.
    pub fn load() -> Self {
        Self::load_from(crate::paths::presets_path())
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
        let list = self.data.modules.entry(module_key.to_owned()).or_default();

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

    // ---------------------------------------------------------------------------
    // API-surface methods (Step D — HTTP CRUD)
    // ---------------------------------------------------------------------------

    /// Return the preset list for `module_key` as a slice, or `None` if no
    /// presets have been stored for that module yet.
    ///
    /// Returns `None` (not an empty slice) only when the module key has never
    /// appeared in the file. An empty vec is returned when the key exists but
    /// all presets have been deleted.
    pub fn module_presets(&self, module_key: &str) -> Option<&[PresetEntry]> {
        self.data.modules.get(module_key).map(|v| v.as_slice())
    }

    /// Upsert a preset by name for `module_key`.
    ///
    /// - If a preset with the same name already exists, it is updated in-place
    ///   (position preserved) and `SetResult::Updated` is returned.
    /// - If no preset with that name exists, it is appended and
    ///   `SetResult::Created` is returned.
    ///
    /// Persists immediately via atomic write.
    pub fn api_set(
        &mut self,
        module_key: &str,
        name: &str,
        description: Option<String>,
        values: HashMap<String, String>,
    ) -> Result<SetResult> {
        let list = self.data.modules.entry(module_key.to_owned()).or_default();

        let result = if let Some(existing) = list.iter_mut().find(|p| p.name == name) {
            existing.description = description;
            existing.values = values;
            SetResult::Updated
        } else {
            list.push(PresetEntry {
                name: name.to_owned(),
                description,
                values,
            });
            SetResult::Created
        };

        self.save()?;
        Ok(result)
    }

    /// Remove the preset named `preset_name` from `module_key`.
    ///
    /// Returns `true` if found and removed, `false` if not found.
    /// Persists immediately via atomic write when `true`.
    pub fn api_remove(&mut self, module_key: &str, preset_name: &str) -> Result<bool> {
        let Some(list) = self.data.modules.get_mut(module_key) else {
            return Ok(false);
        };

        let Some(idx) = list.iter().position(|p| p.name == preset_name) else {
            return Ok(false);
        };

        list.remove(idx);
        self.save()?;
        Ok(true)
    }

    /// Reorder presets for `module_key` to match `new_order`.
    ///
    /// `new_order` must be an exact permutation of the current preset names —
    /// no missing names, no extra names. Returns a `ReorderError` variant if
    /// the constraint is violated. On success, persists immediately.
    pub fn api_reorder(
        &mut self,
        module_key: &str,
        new_order: Vec<String>,
    ) -> std::result::Result<(), ReorderError> {
        let list = self.data.modules.entry(module_key.to_owned()).or_default();

        // Reject duplicates first — before the missing/extra set-diff check.
        // Without this, ["Alpha","Beta","Beta"] against ["Alpha","Beta"] would
        // pass set-diff (sets are equal) and then silently drop the second Beta.
        let mut seen = std::collections::HashSet::new();
        let duplicates: Vec<String> = new_order
            .iter()
            .filter(|name| !seen.insert(name.as_str()))
            .cloned()
            .collect();
        if !duplicates.is_empty() {
            return Err(ReorderError::DuplicateNames(duplicates));
        }

        let current_names: std::collections::HashSet<&str> =
            list.iter().map(|p| p.name.as_str()).collect();
        let new_names: std::collections::HashSet<&str> =
            new_order.iter().map(|s| s.as_str()).collect();

        let missing: Vec<String> = current_names
            .difference(&new_names)
            .map(|s| s.to_string())
            .collect();
        let extra: Vec<String> = new_names
            .difference(&current_names)
            .map(|s| s.to_string())
            .collect();

        if !missing.is_empty() {
            return Err(ReorderError::MissingNames(missing));
        }
        if !extra.is_empty() {
            return Err(ReorderError::ExtraNames(extra));
        }

        // Build a map from name → owned PresetEntry for O(n) reorder.
        let mut by_name: HashMap<String, PresetEntry> =
            list.drain(..).map(|p| (p.name.clone(), p)).collect();

        for name in &new_order {
            // Guaranteed to exist — we validated above.
            if let Some(entry) = by_name.remove(name) {
                list.push(entry);
            }
        }

        self.save().map_err(ReorderError::SaveFailed)?;

        Ok(())
    }
}
