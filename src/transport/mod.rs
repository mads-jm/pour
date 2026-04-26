pub mod api;
pub mod fs;

use crate::config::Config;
use anyhow::Result;

use api::ApiClient;
use fs::FsWriter;

/// Typed error for `Transport::read_file`.
///
/// Using a typed enum instead of substring-matching on error messages ensures
/// correct classification on all platforms (Windows error strings differ from
/// Unix) and avoids false positives when vault paths happen to contain
/// substrings like "not found".
#[derive(Debug)]
pub enum TransportReadError {
    /// The requested file does not exist in the vault.
    NotFound,
    /// The transport backend (API) is unreachable — connect/timeout error.
    Unreachable(String),
    /// Any other error (permission denied, I/O error, parse failure, etc.).
    Other(String),
}

impl std::fmt::Display for TransportReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportReadError::NotFound => write!(f, "file not found"),
            TransportReadError::Unreachable(msg) => write!(f, "transport unreachable: {msg}"),
            TransportReadError::Other(msg) => write!(f, "read error: {msg}"),
        }
    }
}

impl std::error::Error for TransportReadError {}

/// A single entry returned by directory listing — either a file or a subdirectory.
#[derive(Debug, Clone)]
pub struct VaultEntry {
    /// Display name: stem for `.md` files, bare name for directories.
    pub name: String,
    /// `true` if this entry is a subdirectory.
    pub is_dir: bool,
}

/// Which transport backend is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    Api,
    FileSystem,
}

impl std::fmt::Display for TransportMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportMode::Api => write!(f, "API"),
            TransportMode::FileSystem => write!(f, "File System"),
        }
    }
}

/// Unified transport layer that delegates to either the API client or
/// the filesystem writer.
pub enum Transport {
    Api(ApiClient),
    Fs(FsWriter),
}

impl Transport {
    /// Attempt to connect via the API; fall back to filesystem if the
    /// API is unreachable or not configured.
    ///
    /// The API path is tried when both `api_port` and `api_key` are
    /// present in the config AND `check_connection()` succeeds.
    pub async fn connect(config: &Config) -> Self {
        if let (Some(port), Some(api_key)) = (config.vault.api_port, config.vault.api_key.as_ref())
        {
            let client = ApiClient::new(port, api_key.clone());
            if client.check_connection().await {
                return Transport::Api(client);
            }
        }

        let base_path = std::path::PathBuf::from(&config.vault.base_path);
        Transport::Fs(FsWriter::new(base_path))
    }

    /// Return which transport mode is currently active.
    pub fn mode(&self) -> TransportMode {
        match self {
            Transport::Api(_) => TransportMode::Api,
            Transport::Fs(_) => TransportMode::FileSystem,
        }
    }

    /// Create (or overwrite) a file at the given vault-relative path.
    pub async fn create_file(&self, vault_path: &str, content: &str) -> Result<()> {
        match self {
            Transport::Api(client) => client.create_file(vault_path, content).await,
            Transport::Fs(writer) => writer.create_file(vault_path, content),
        }
    }

    /// Append content under a heading in an existing note.
    ///
    /// Both backends are heading-aware: the API uses its native heading
    /// targeting; the filesystem backend parses the markdown to find the
    /// heading and inserts content before the next same-or-higher-level heading.
    /// When `shallow` is `true`, any subsequent heading is treated as the
    /// section boundary regardless of level.
    pub async fn append_under_heading(
        &self,
        vault_path: &str,
        heading: &str,
        content: &str,
        shallow: bool,
    ) -> Result<()> {
        match self {
            Transport::Api(client) => {
                client
                    .append_under_heading(vault_path, heading, content, shallow)
                    .await
            }
            Transport::Fs(writer) => {
                writer.append_under_heading(vault_path, heading, content, shallow)
            }
        }
    }

    /// List files in a vault directory.
    ///
    /// The API returns raw filenames (including `.md` extensions and
    /// trailing `/` for directories). The filesystem backend returns
    /// `.md` file stems only. Callers should handle both shapes.
    pub async fn list_directory(&self, vault_dir_path: &str) -> Result<Vec<String>> {
        match self {
            Transport::Api(client) => client.list_directory(vault_dir_path).await,
            Transport::Fs(writer) => writer.list_directory(vault_dir_path),
        }
    }

    /// List directory entries with type information (file vs directory).
    ///
    /// Returns entries sorted directories-first, then alphabetically within each group.
    /// For files, only `.md` files are included and names are returned without extension.
    pub async fn list_directory_entries(&self, vault_dir_path: &str) -> Result<Vec<VaultEntry>> {
        match self {
            Transport::Api(client) => client.list_directory_entries(vault_dir_path).await,
            Transport::Fs(writer) => writer.list_directory_all(vault_dir_path),
        }
    }

    /// Read a single file at `vault_path` and return its UTF-8 content.
    ///
    /// Returns a typed `TransportReadError` so callers can distinguish "not
    /// found" from "transport unreachable" from other errors without string
    /// matching on platform-specific error messages.
    ///
    /// - API backend: `NOT_FOUND` status → `NotFound`; connect/timeout → `Unreachable`.
    /// - FS backend: `io::ErrorKind::NotFound` → `NotFound`; other I/O → `Other`.
    pub async fn read_file(&self, vault_path: &str) -> std::result::Result<String, TransportReadError> {
        match self {
            Transport::Api(client) => client.read_file(vault_path).await,
            Transport::Fs(writer) => writer.read_file(vault_path),
        }
    }

    /// Execute an Obsidian command by its ID.
    ///
    /// Only available via the API transport. On filesystem transport, this
    /// is a no-op (returns `Ok(())`), since command execution requires the
    /// Obsidian REST API.
    pub async fn execute_command(&self, command_id: &str) -> Result<()> {
        match self {
            Transport::Api(client) => client.execute_command(command_id).await,
            Transport::Fs(_) => Ok(()),
        }
    }
}
