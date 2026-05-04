//! Generic JSON-backed file store.
//!
//! Consolidates the load/save pattern that was duplicated across
//! [`crate::data::cache::Cache`], [`crate::data::presets::Presets`], and
//! [`crate::data::field_presets::FieldPresets`]. Each of those owned its own
//! near-identical implementation of `read_to_string -> from_str -> default`
//! load and `to_string_pretty -> tmp -> atomic_replace` save.
//!
//! The contract:
//!
//! - **`load`** — reads the JSON file at `path` and deserializes to `T`. On
//!   missing file, parse failure, or any I/O error, returns `T::default()`.
//!   This matches the pre-existing behavior of all three call sites.
//! - **`load_with_migration`** — same as `load`, but if straight deserialize
//!   fails the supplied `migrate` callback gets a chance to recover from a
//!   legacy on-disk format. Used by `presets.rs` for version-aware migration.
//! - **`save`** — writes via the single sanctioned atomic-write primitive
//!   ([`crate::transport::atomic::atomic_replace`]). Creates parent
//!   directories if needed. On rename failure the orphan `.tmp` is removed,
//!   closing a leak that previously existed at most call sites.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// A JSON file at a fixed path that round-trips to a `Default + Serialize +
/// DeserializeOwned` type.
///
/// The store does not cache the deserialized value — `load` reads from disk
/// each call, and `save` takes the value to persist. Holding the path and
/// nothing else keeps the store cheap to clone and trivial to reason about.
#[derive(Debug, Clone)]
pub struct JsonStore<T> {
    path: PathBuf,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> JsonStore<T>
where
    T: Serialize + DeserializeOwned + Default,
{
    /// Build a store rooted at `path`. The file does not need to exist yet.
    pub fn new(path: PathBuf) -> Self {
        JsonStore {
            path,
            _marker: std::marker::PhantomData,
        }
    }

    /// The on-disk path this store reads from and writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read and deserialize, returning `T::default()` on missing file or
    /// parse failure.
    pub fn load(&self) -> T {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str::<T>(&raw).ok())
            .unwrap_or_default()
    }

    /// Like [`load`](Self::load), but when straight deserialize fails the
    /// `migrate` callback gets a chance to interpret the raw bytes as a legacy
    /// schema and produce a recovered value.
    ///
    /// `migrate` is called only after a parse failure; if the file is missing
    /// or empty the default is returned without calling the migration. If
    /// `migrate` returns `None`, the default is also returned.
    pub fn load_with_migration<F>(&self, migrate: F) -> T
    where
        F: FnOnce(&str) -> Option<T>,
    {
        let Ok(raw) = std::fs::read_to_string(&self.path) else {
            return T::default();
        };

        if let Ok(value) = serde_json::from_str::<T>(&raw) {
            return value;
        }

        migrate(&raw).unwrap_or_default()
    }

    /// Serialize `value` and write it to `path` atomically.
    ///
    /// Behavior:
    /// 1. If a parent directory exists in the path, it is `create_dir_all`'d.
    /// 2. The serialized JSON is written to `<path>.tmp` first.
    /// 3. The tmp file is atomically renamed onto `path` via
    ///    [`crate::transport::atomic::atomic_replace`].
    /// 4. On any failure during the rename, the tmp file is best-effort
    ///    removed so successive failures don't accumulate orphans.
    pub fn save(&self, value: &T) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serde_json::to_string_pretty(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, serialized)?;

        if let Err(e) = crate::transport::atomic::atomic_replace(&tmp_path, &self.path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }

        Ok(())
    }
}
