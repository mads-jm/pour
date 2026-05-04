use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::data::json_store::JsonStore;

/// A single saved preset for a `composite_array` field.
///
/// Stores the full row-set (each row is a `Vec<String>` whose length matches
/// the field's `sub_fields` at save time). On apply, rows are reconciled
/// against the current sub_field shape so config edits don't break old presets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldPresetEntry {
    pub name: String,
    /// Optional human-readable description for disambiguating similar presets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Saved rows. `rows[i][j]` is the value of sub-field `j` in row `i`.
    pub rows: Vec<Vec<String>>,
}

/// On-disk schema for the field-presets file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldPresetsData {
    /// Maps `"<module_key>.<field_name>"` -> ordered list of presets.
    #[serde(default)]
    pub fields: HashMap<String, Vec<FieldPresetEntry>>,
}

/// Manages reading and writing the `field_presets.json` file that stores
/// per-field named presets at `~/.pour/field_presets.json`.
///
/// Companion to [`crate::data::presets::Presets`]. The two systems are kept
/// separate because their value shapes differ (scalar vs. nested rows) and
/// their scoping is different (per-module vs. per-`module.field`).
#[derive(Debug)]
pub struct FieldPresets {
    data: FieldPresetsData,
    path: PathBuf,
}

impl FieldPresets {
    /// Load field presets from the default location
    /// (`~/.pour/field_presets.json`, or `$POUR_HOME/field_presets.json`).
    ///
    /// Returns empty presets if the file is missing or corrupt.
    pub fn load() -> Self {
        Self::load_from(crate::paths::field_presets_path())
    }

    /// Create an empty `FieldPresets` instance not backed by any file.
    pub fn empty() -> Self {
        FieldPresets {
            data: FieldPresetsData::default(),
            path: PathBuf::new(),
        }
    }

    /// Load field presets from a specific file path.
    ///
    /// Returns empty presets if the file is missing or corrupt.
    pub fn load_from(path: PathBuf) -> Self {
        let data = JsonStore::<FieldPresetsData>::new(path.clone()).load();
        FieldPresets { data, path }
    }

    /// Persist field presets to disk, creating parent directories if needed.
    pub fn save(&self) -> Result<()> {
        JsonStore::<FieldPresetsData>::new(self.path.clone()).save(&self.data)?;
        Ok(())
    }

    /// Return the ordered list of presets for `key` (typically `"module.field"`).
    pub fn get(&self, key: &str) -> Vec<FieldPresetEntry> {
        self.data.fields.get(key).cloned().unwrap_or_default()
    }

    /// Upsert a preset for `key` by name.
    pub fn set(&mut self, key: &str, entry: FieldPresetEntry) {
        let list = self.data.fields.entry(key.to_owned()).or_default();
        if let Some(existing) = list.iter_mut().find(|p| p.name == entry.name) {
            *existing = entry;
        } else {
            list.push(entry);
        }
    }

    /// Remove the preset named `preset_name` from `key`.
    /// Returns `true` if removed, `false` if not found.
    pub fn delete(&mut self, key: &str, preset_name: &str) -> bool {
        let Some(list) = self.data.fields.get_mut(key) else {
            return false;
        };
        let Some(idx) = list.iter().position(|p| p.name == preset_name) else {
            return false;
        };
        list.remove(idx);
        true
    }

    /// Swap the preset named `preset_name` with its neighbor in `key`.
    pub fn reorder(&mut self, key: &str, preset_name: &str, direction: i32) {
        let Some(list) = self.data.fields.get_mut(key) else {
            return;
        };
        let Some(idx) = list.iter().position(|p| p.name == preset_name) else {
            return;
        };

        let new_idx = match direction {
            d if d > 0 => {
                if idx + 1 >= list.len() {
                    return;
                }
                idx + 1
            }
            d if d < 0 => {
                if idx == 0 {
                    return;
                }
                idx - 1
            }
            _ => return,
        };

        list.swap(idx, new_idx);
    }
}

/// Build the storage key for a composite field preset.
pub fn preset_key(module_key: &str, field_name: &str) -> String {
    format!("{module_key}.{field_name}")
}

/// Reconcile saved rows against the current sub_field count.
///
/// If the config schema for the composite field changed since the preset was
/// saved (sub-fields added or removed), rows are right-padded with empty
/// strings or truncated so the shape matches `sub_field_count`.
///
/// Returns `(adjusted_rows, was_adjusted)` so callers can surface a status
/// message when reconciliation occurred.
pub fn reconcile_rows(rows: Vec<Vec<String>>, sub_field_count: usize) -> (Vec<Vec<String>>, bool) {
    let mut adjusted = false;
    let reconciled = rows
        .into_iter()
        .map(|row| {
            if row.len() == sub_field_count {
                row
            } else {
                adjusted = true;
                let mut new_row = row;
                new_row.resize(sub_field_count, String::new());
                new_row
            }
        })
        .collect();
    (reconciled, adjusted)
}
