use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

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
#[derive(Deserialize)]
struct DirectoryListing {
    files: Vec<String>,
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

    /// Append content under a heading in an existing note.
    ///
    /// Sends a PATCH to `/vault/{vault_path}` using the v3 header-based API:
    /// - `Operation: append`
    /// - `Target-Type: heading`
    /// - `Target: {heading}`
    pub async fn append_under_heading(
        &self,
        vault_path: &str,
        heading: &str,
        content: &str,
    ) -> Result<()> {
        let url = format!("{}/vault/{}", self.base_url, vault_path);

        let resp = self
            .client
            .patch(&url)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "text/markdown")
            .header("Operation", "append")
            .header("Target-Type", "heading")
            .header("Target", heading)
            .body(content.to_owned())
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

        let listing: DirectoryListing = resp
            .json()
            .await
            .context("API: failed to parse directory listing JSON")?;

        Ok(listing.files)
    }
}
