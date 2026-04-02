use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use super::VaultEntry;

/// JSON shape returned by `GET /vault/{path}` with
/// `Accept: application/vnd.olrapi.document-map+json`.
#[derive(Deserialize)]
struct DocumentMap {
    headings: Vec<String>,
}

/// HTTP client for the Obsidian Local REST API.
///
/// Communicates over HTTPS with a self-signed certificate.
/// All methods require a valid API key for Bearer authentication.
pub struct ApiClient {
    client: Client,
    pub base_url: String,
    api_key: String,
}

/// JSON shape returned by `GET /vault/{dir}/`.
///
/// The Obsidian REST API may return either an array of plain strings
/// or an array of objects with a `path` field. Both are normalised
/// into `Vec<String>` via `DirectoryEntry`.
#[derive(Deserialize)]
struct DirectoryListing {
    files: Vec<DirectoryEntry>,
}

/// A single entry in a directory listing.
///
/// Handles both `"filename.md"` (plain string) and `{"path": "filename.md"}`
/// (object with path field) response shapes.
#[derive(Deserialize)]
#[serde(untagged)]
enum DirectoryEntry {
    Plain(String),
    Object { path: String },
}

impl DirectoryEntry {
    fn into_path(self) -> String {
        match self {
            DirectoryEntry::Plain(s) => s,
            DirectoryEntry::Object { path } => path,
        }
    }
}

impl ApiClient {
    /// Create a new API client targeting the given port with the given key.
    ///
    /// The underlying `reqwest::Client` accepts self-signed certificates
    /// and enforces a 5-second timeout on every request.
    pub fn new(port: u16, api_key: String) -> Self {
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");

        Self {
            client,
            base_url: format!("https://127.0.0.1:{port}"),
            api_key,
        }
    }

    /// Check whether the Obsidian REST API is reachable.
    ///
    /// Returns `true` if a GET to `/` succeeds with a 2xx status.
    /// Returns `false` on any network or HTTP error (never propagates).
    pub async fn check_connection(&self) -> bool {
        let result = self
            .client
            .get(format!("{}/", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await;

        match result {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Create (or overwrite) a file at `vault_path` with the given content.
    ///
    /// Sends a PUT to `/vault/{vault_path}` with `Content-Type: text/markdown`.
    pub async fn create_file(&self, vault_path: &str, content: &str) -> Result<()> {
        let url = format!("{}/vault/{}", self.base_url, vault_path);

        let resp = self
            .client
            .put(&url)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "text/markdown")
            .body(content.to_owned())
            .send()
            .await
            .context("API: failed to send create_file request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API create_file failed ({}): {}", status, body);
        }

        Ok(())
    }

    /// Fetch the document map for a vault file and resolve the full
    /// `::` delimited heading path that ends with the given heading text.
    ///
    /// The Obsidian REST API requires heading targets to include their
    /// full ancestor path (e.g. `Parent::Child`) rather than just the
    /// leaf heading text. This method fetches the document map via
    /// `Accept: application/vnd.olrapi.document-map+json` and finds
    /// the first heading path whose final segment matches `heading_text`.
    async fn resolve_heading_target(
        &self,
        vault_path: &str,
        heading_text: &str,
    ) -> Result<String> {
        let url = format!("{}/vault/{}", self.base_url, vault_path);

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .header("Accept", "application/vnd.olrapi.document-map+json")
            .send()
            .await
            .context("API: failed to fetch document map")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API: failed to fetch document map ({}): {}", status, body);
        }

        let doc_map: DocumentMap = resp
            .json()
            .await
            .context("API: failed to parse document map JSON")?;

        // Find the first heading path whose leaf segment matches.
        doc_map
            .headings
            .into_iter()
            .find(|h| {
                let leaf = h.rsplit("::").next().unwrap_or(h);
                leaf == heading_text
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "API: heading {:?} not found in document map for {}",
                    heading_text,
                    vault_path
                )
            })
    }

    /// Append content under a heading in an existing note.
    ///
    /// The `heading` parameter is the raw config value (e.g. `"## Journal"`).
    /// The `##` prefix is stripped to obtain the heading text, then the full
    /// `::` delimited heading path is resolved via the document map before
    /// sending the PATCH request.
    pub async fn append_under_heading(
        &self,
        vault_path: &str,
        heading: &str,
        content: &str,
    ) -> Result<()> {
        // Strip the markdown heading prefix ("## " → heading text).
        let heading_text = heading.trim_start_matches('#').trim();

        let target = self
            .resolve_heading_target(vault_path, heading_text)
            .await?;

        let url = format!("{}/vault/{}", self.base_url, vault_path);

        let resp = self
            .client
            .patch(&url)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "text/markdown")
            .header("Operation", "append")
            .header("Target-Type", "heading")
            .header("Target", &target)
            .body(format!("\n{content}\n"))
            .send()
            .await
            .context("API: failed to send append_under_heading request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API append_under_heading failed ({}): {}", status, body);
        }

        Ok(())
    }

    /// List files in a vault directory.
    ///
    /// Sends a GET to `/vault/{vault_dir_path}/` (trailing slash required).
    /// Returns the raw file list from the API (includes `.md` files and
    /// subdirectory names ending in `/`).
    pub async fn list_directory(&self, vault_dir_path: &str) -> Result<Vec<String>> {
        let listing = self.fetch_directory_listing(vault_dir_path).await?;
        Ok(listing
            .files
            .into_iter()
            .map(DirectoryEntry::into_path)
            .collect())
    }

    /// List directory entries with type information.
    ///
    /// Sends a GET to `/vault/{vault_dir_path}/`. Entries ending in `/` are
    /// treated as directories. Returns entries sorted directories-first, then
    /// alphabetically within each group. File names are returned without their
    /// `.md` extension; non-`.md` files are excluded.
    pub async fn list_directory_entries(&self, vault_dir_path: &str) -> Result<Vec<VaultEntry>> {
        let listing = self.fetch_directory_listing(vault_dir_path).await?;

        let mut entries: Vec<VaultEntry> = listing
            .files
            .into_iter()
            .filter_map(|entry| {
                let raw = entry.into_path();
                if raw.ends_with('/') {
                    // Directory: strip trailing slash, use bare name component
                    let name = raw
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(&raw)
                        .to_string();
                    if name.is_empty() {
                        return None;
                    }
                    Some(VaultEntry { name, is_dir: true })
                } else {
                    // File: only include .md files, strip extension
                    let stem = if let Some(s) = raw.rsplit('/').next() {
                        s
                    } else {
                        &raw
                    };
                    if !stem.ends_with(".md") {
                        return None;
                    }
                    let name = stem.strip_suffix(".md").unwrap_or(stem).to_string();
                    if name.is_empty() {
                        return None;
                    }
                    Some(VaultEntry {
                        name,
                        is_dir: false,
                    })
                }
            })
            .collect();

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Ok(entries)
    }

    /// Shared helper: fetch and deserialise a directory listing from the API.
    async fn fetch_directory_listing(&self, vault_dir_path: &str) -> Result<DirectoryListing> {
        let url = format!("{}/vault/{}/", self.base_url, vault_dir_path);

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("API: failed to send list_directory request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API list_directory failed ({}): {}", status, body);
        }

        resp.json()
            .await
            .context("API: failed to parse directory listing JSON")
    }
}
