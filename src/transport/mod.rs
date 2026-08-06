pub mod api;
pub(crate) mod atomic; // implementation detail; re-exported via pour::util::atomic_replace
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

/// Typed error for [`Transport::patch_frontmatter`].
///
/// The variant that matters most is `Unsupported`: it is the signal that the
/// *capture* is still fine and should be retried over the filesystem, rather
/// than an error to show the user (spec §2.1).
#[derive(Debug)]
pub enum TransportPatchError {
    /// The target note does not exist. Pour never fabricates it (spec §2.3).
    NotFound,
    /// This backend cannot serve a frontmatter patch — a Local REST API older
    /// than v3.0 that does not understand `Target-Type: frontmatter`, or one
    /// that has gone away mid-capture. Callers degrade to the filesystem path.
    Unsupported(String),
    /// The file changed on disk between the read and the write. **Nothing was
    /// written.**
    Conflict(String),
    /// Any other failure (permissions, I/O, malformed frontmatter).
    Other(String),
}

impl std::fmt::Display for TransportPatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportPatchError::NotFound => write!(f, "file not found"),
            TransportPatchError::Unsupported(msg) => {
                write!(f, "frontmatter patch unsupported: {msg}")
            }
            TransportPatchError::Conflict(msg) => write!(f, "aborted, nothing written: {msg}"),
            TransportPatchError::Other(msg) => write!(f, "patch error: {msg}"),
        }
    }
}

impl std::error::Error for TransportPatchError {}

/// Classify a non-2xx response to the frontmatter PATCH.
///
/// `400`/`405`/`415`/`501` all mean "this plugin does not understand the
/// request shape" — which is exactly how a pre-v3.0 Local REST API answers a
/// `Target-Type: frontmatter` PATCH — so they degrade rather than fail. `404`
/// is the note itself missing, which §2.3 handles separately and must not be
/// confused with an old plugin.
pub fn classify_patch_status(status: u16, body: &str) -> TransportPatchError {
    match status {
        404 => TransportPatchError::NotFound,
        400 | 405 | 415 | 501 => TransportPatchError::Unsupported(format!("HTTP {status}: {body}")),
        _ => TransportPatchError::Other(format!("API patch_frontmatter failed ({status}): {body}")),
    }
}

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
            && let Ok(client) = ApiClient::new(port, api_key.clone())
            && client.check_connection().await
        {
            return Transport::Api(client);
        }

        let base_path = std::path::PathBuf::from(config.vault.effective_base_path());
        Transport::Fs(FsWriter::new(base_path))
    }

    /// A transport scoped to a module that overrides the vault root, or `None`
    /// when the module writes to the vault like everything else.
    ///
    /// Always filesystem, never API, and that is not a fallback: the Obsidian
    /// Local REST API can only address notes inside the vault it serves, so a
    /// module rooted elsewhere has no API to fall back *from*. A caller that
    /// reports the transport must therefore report this one's mode, not the
    /// app-level transport's, or it will claim "API" for a write the API never
    /// saw.
    ///
    /// Chosen at write time rather than at [`Transport::connect`] so that the
    /// app keeps exactly one connected transport: this is a per-write
    /// redirection of a path, not a second connection.
    pub fn for_module(module: &crate::config::ModuleConfig) -> Option<Self> {
        module
            .root_override()
            .map(|root| Transport::Fs(FsWriter::new(std::path::PathBuf::from(root))))
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
    pub async fn read_file(
        &self,
        vault_path: &str,
    ) -> std::result::Result<String, TransportReadError> {
        match self {
            Transport::Api(client) => client.read_file(vault_path).await,
            Transport::Fs(writer) => writer.read_file(vault_path),
        }
    }

    /// Replace (or insert) a single frontmatter key on an existing note.
    ///
    /// The one mutation primitive behind `update` mode. Both backends edit
    /// exactly one key and leave the rest of the file — including the body —
    /// alone:
    ///
    /// - **API**: `PATCH /vault/{path}` with `Operation: replace`,
    ///   `Target-Type: frontmatter`, `Target: <key>`; Obsidian's own metadata
    ///   layer does the mutation, so pour never re-emits YAML.
    /// - **Filesystem**: read → capture mtime → line-level edit → re-verify
    ///   mtime → [`crate::util::atomic_replace`]. A mismatch aborts *without
    ///   writing* (spec §2.2).
    ///
    /// Returns [`TransportPatchError::Unsupported`] when the API cannot serve
    /// the operation; the caller is expected to retry over the filesystem
    /// rather than surface it (spec §2.1).
    pub async fn patch_frontmatter(
        &self,
        vault_path: &str,
        key: &str,
        value: &crate::output::frontmatter::FrontmatterValue,
    ) -> std::result::Result<(), TransportPatchError> {
        match self {
            Transport::Api(client) => client.patch_frontmatter(vault_path, key, value).await,
            Transport::Fs(writer) => writer.patch_frontmatter(vault_path, key, value),
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
