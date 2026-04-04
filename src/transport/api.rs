use anyhow::{Context, Result};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::Client;
use serde::Deserialize;

/// Characters that must be percent-encoded inside a URL path segment.
///
/// This is the "path segment" set from RFC 3986: everything except unreserved
/// characters and the sub-delimiters that are safe inside a segment.
/// Forward slash is intentionally excluded so that the helper below can split
/// on it and encode each component individually.
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'%')
    .add(b'?')
    .add(b'{')
    .add(b'}');

use super::VaultEntry;

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
        let url = format!("{}/vault/{}", self.base_url, encode_vault_path(vault_path));

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

    /// Append content under a heading in an existing note.
    ///
    /// Reads the file via API, splices the content under the target heading
    /// (using the same logic as the FS transport), then writes the modified
    /// file back. This avoids the `***` thematic break that the Obsidian REST
    /// API inserts when using heading-targeted PATCH appends.
    pub async fn append_under_heading(
        &self,
        vault_path: &str,
        heading: &str,
        content: &str,
    ) -> Result<()> {
        let url = format!("{}/vault/{}", self.base_url, encode_vault_path(vault_path));

        // 1. Read current file content.
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .header("Accept", "text/markdown")
            .send()
            .await
            .context("API: failed to read file for append")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API: failed to read file for append ({}): {}", status, body);
        }

        let raw = resp
            .text()
            .await
            .context("API: failed to read response body")?;

        // 2. Splice content under the heading.
        let modified = splice_under_heading(&raw, heading, content)?;

        // 3. Write modified content back.
        let resp = self
            .client
            .put(&url)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "text/markdown")
            .body(modified)
            .send()
            .await
            .context("API: failed to write file after append")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "API: failed to write file after append ({}): {}",
                status,
                body
            );
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
        let url = format!("{}/vault/{}/", self.base_url, encode_vault_path(vault_dir_path));

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

    /// Execute an Obsidian command by its ID.
    ///
    /// Sends a POST to `/commands/{command_id}/`. This can trigger plugin
    /// commands like Templater's template processing.
    pub async fn execute_command(&self, command_id: &str) -> Result<()> {
        let url = format!("{}/commands/{}/", self.base_url, command_id);

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("API: failed to send execute_command request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API execute_command failed ({}): {}", status, body);
        }

        Ok(())
    }
}

/// Percent-encode a vault path for use in a URL.
///
/// Each `/`-separated path segment is encoded individually so that directory
/// separators are preserved in the final URL while spaces and other special
/// characters within segment names are safely encoded.
///
/// Example: `"Coffee Notes/2024-01-01 latte.md"` →
///          `"Coffee%20Notes/2024-01-01%20latte.md"`
fn encode_vault_path(vault_path: &str) -> String {
    vault_path
        .split('/')
        .map(|segment| utf8_percent_encode(segment, PATH_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Splice `content` under `heading` in a markdown document, returning the
/// modified document. Mirrors the logic in `FsClient::append_under_heading`.
fn splice_under_heading(raw: &str, heading: &str, content: &str) -> Result<String> {
    let heading_level = heading.chars().take_while(|&c| c == '#').count();
    if heading_level == 0 {
        anyhow::bail!("not a valid markdown heading: {:?}", heading);
    }

    let lines: Vec<&str> = raw.lines().collect();

    let heading_idx = lines
        .iter()
        .position(|l| l.trim_end() == heading)
        .ok_or_else(|| anyhow::anyhow!("heading {:?} not found in file", heading))?;

    // Find the next heading of equal or higher level.
    let insert_before = lines[heading_idx + 1..]
        .iter()
        .position(|l| {
            let hashes = l.chars().take_while(|&c| c == '#').count();
            hashes > 0 && l.chars().nth(hashes) == Some(' ') && hashes <= heading_level
        })
        .map(|rel| heading_idx + 1 + rel);

    let mut result = String::with_capacity(raw.len() + content.len() + 2);

    match insert_before {
        Some(next_heading_idx) => {
            let before_lines = &lines[..next_heading_idx];
            let section_end = before_lines
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map(|i| i + 1)
                .unwrap_or(before_lines.len());

            for line in &before_lines[..section_end] {
                result.push_str(line);
                result.push('\n');
            }
            result.push('\n');
            result.push_str(content.trim_end_matches('\n'));
            result.push('\n');
            result.push('\n');

            for line in &lines[next_heading_idx..] {
                result.push_str(line);
                result.push('\n');
            }
        }
        None => {
            let trimmed_end = lines
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map(|i| i + 1)
                .unwrap_or(lines.len());

            for line in &lines[..trimmed_end] {
                result.push_str(line);
                result.push('\n');
            }
            result.push('\n');
            result.push_str(content.trim_end_matches('\n'));
            result.push('\n');
        }
    }

    Ok(result)
}
