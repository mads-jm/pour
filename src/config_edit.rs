use std::path::Path;

use toml_edit::DocumentMut;

use crate::config::{Config, ConfigError};

/// In-memory mutable view of the config file passed to `Config::edit` closures.
///
/// `doc` is the TOML representation that the closure mutates. `parsed` is a
/// fresh-from-disk parsed Config snapshot for validation lookups (e.g. "does
/// this module exist?"). The closure must not assume `parsed` reflects edits
/// it has made to `doc` — `parsed` is a snapshot taken at the start of the
/// edit transaction.
pub struct ConfigDraft<'a> {
    pub doc: &'a mut DocumentMut,
    pub parsed: &'a Config,
}

impl Config {
    /// Transactionally edit the config file at `path`.
    ///
    /// Loads the TOML doc and parses the Config snapshot, hands both to `f`,
    /// then atomically writes `draft.doc` back to disk. If `f` returns an
    /// error, the file on disk is unchanged.
    pub fn edit<F>(path: &Path, f: F) -> Result<(), ConfigError>
    where
        F: FnOnce(&mut ConfigDraft<'_>) -> Result<(), ConfigError>,
    {
        let content = std::fs::read_to_string(path).map_err(ConfigError::ReadError)?;
        let mut doc: DocumentMut = content
            .parse()
            .map_err(|e: toml_edit::TomlError| ConfigError::EditParseError(e.to_string()))?;
        let parsed = Config::from_toml(&content)?;

        let mut draft = ConfigDraft {
            doc: &mut doc,
            parsed: &parsed,
        };
        f(&mut draft)?;

        Config::write_atomic(path, &draft.doc.to_string())
    }
}
