pub mod api;
pub mod fs;

use crate::config::Config;
use anyhow::Result;

use api::ApiClient;
use fs::FsWriter;

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
    pub async fn append_under_heading(
        &self,
        vault_path: &str,
        heading: &str,
        content: &str,
    ) -> Result<()> {
        match self {
            Transport::Api(client) => {
                client
                    .append_under_heading(vault_path, heading, content)
                    .await
            }
            Transport::Fs(writer) => writer.append_under_heading(vault_path, heading, content),
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
}
