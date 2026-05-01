use crate::transport::TransportReadError;
use anyhow::{Context, Result};
use std::path::PathBuf;

use super::VaultEntry;

/// Filesystem-based writer for direct vault access.
///
/// Used as a fallback when the Obsidian Local REST API is unavailable.
/// All paths are resolved relative to the vault `base_path`.
pub struct FsWriter {
    base_path: PathBuf,
}

impl FsWriter {
    /// Create a new filesystem writer rooted at `base_path`.
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Return a reference to the vault base path.
    ///
    /// Test-only accessor — no production caller. Kept `pub` (rather than
    /// gated behind a `test-utils` feature) for v1.0.0 to avoid a Cargo
    /// surface change. Revisit at v1.1 if the surface curation pass tightens
    /// further.
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Resolve a vault-relative path string against the base path.
    ///
    /// Normalizes mixed separators: converts all `\` and `/` components
    /// through `PathBuf` so that a forward-slash relative path joins
    /// correctly against a backslash-style Windows base path.
    fn resolve_path(&self, relative_path: &str) -> PathBuf {
        // Replace backslashes with forward slashes so Path::join treats the
        // entire string as a sequence of components rather than a single
        // opaque segment. PathBuf handles the rest on every platform.
        let normalized = relative_path.replace('\\', "/");
        self.base_path.join(normalized)
    }

    /// Resolve a vault-relative path with traversal-escape validation.
    ///
    /// Rejects any path that could escape the vault root:
    /// - Any path component equal to `..`
    /// - Paths that start with `/` or `\` (Unix/Windows absolute)
    /// - Paths that start with a Windows drive letter (`C:`, `D:`, …)
    /// - Paths that start with `~` (home-dir expansion)
    ///
    /// Returns the resolved `PathBuf` on success, or an `anyhow::Error`
    /// describing why the path was rejected.
    fn resolve_path_validated(&self, relative_path: &str) -> anyhow::Result<PathBuf> {
        // Reject home-dir shortcuts.
        if relative_path.starts_with('~') {
            anyhow::bail!("FS: path must not start with '~': {relative_path:?}");
        }

        // Reject Unix/Windows absolute paths.
        if relative_path.starts_with('/') || relative_path.starts_with('\\') {
            anyhow::bail!("FS: path must be relative, not absolute: {relative_path:?}");
        }

        // Reject Windows drive-letter paths (e.g. `C:`, `C:\`, `C:/`).
        // A drive letter is a single ASCII letter followed by `:`.
        if relative_path.len() >= 2
            && relative_path.as_bytes()[0].is_ascii_alphabetic()
            && relative_path.as_bytes()[1] == b':'
        {
            anyhow::bail!("FS: path must be relative, not absolute: {relative_path:?}");
        }

        // Normalise separators then check each component for `..`.
        let normalized = relative_path.replace('\\', "/");
        for component in normalized.split('/') {
            if component == ".." {
                anyhow::bail!("FS: path must not contain '..': {relative_path:?}");
            }
        }

        Ok(self.base_path.join(normalized))
    }

    /// Create a new file at `relative_path` with the given content.
    ///
    /// Parent directories are created automatically.
    /// Returns an error if the file already exists.
    pub fn create_file(&self, relative_path: &str, content: &str) -> Result<()> {
        let full_path = self.resolve_path_validated(relative_path)?;

        if full_path.exists() {
            anyhow::bail!("FS: file already exists: {}", full_path.display());
        }

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("FS: failed to create directories for {}", parent.display())
            })?;
        }

        std::fs::write(&full_path, content)
            .with_context(|| format!("FS: failed to write file {}", full_path.display()))?;

        Ok(())
    }

    /// Append content to an existing file at `relative_path`.
    ///
    /// Returns an error if the file does not exist.
    pub fn append_to_file(&self, relative_path: &str, content: &str) -> Result<()> {
        use std::io::Write;

        let full_path = self.resolve_path_validated(relative_path)?;

        if !full_path.exists() {
            anyhow::bail!("FS: file not found: {}", full_path.display());
        }

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&full_path)
            .with_context(|| {
                format!(
                    "FS: failed to open file for appending: {}",
                    full_path.display()
                )
            })?;

        file.write_all(content.as_bytes())
            .with_context(|| format!("FS: failed to append to file {}", full_path.display()))?;

        Ok(())
    }

    /// Append `content` under a specific heading in an existing markdown file.
    ///
    /// Finds `heading` (e.g. `"## Log"`) in the file at `relative_path` and
    /// inserts `content` after all existing content in that section, but before
    /// the next heading of equal or higher level (i.e. same or fewer `#` symbols).
    /// If `shallow` is `true`, *any* subsequent heading is treated as the section
    /// boundary regardless of level. If the heading is the last section in the
    /// file, content is appended at EOF.
    ///
    /// A blank line is inserted before `content` to preserve clean markdown spacing.
    ///
    /// Returns an error if:
    /// - the file does not exist
    /// - `heading` is not found in the file
    /// - `heading` has no `#` prefix (not a valid markdown heading)
    pub fn append_under_heading(
        &self,
        relative_path: &str,
        heading: &str,
        content: &str,
        shallow: bool,
    ) -> Result<()> {
        let full_path = self.resolve_path_validated(relative_path)?;

        if !full_path.exists() {
            anyhow::bail!("FS: file not found: {}", full_path.display());
        }

        // Determine the level of the target heading (number of leading `#` chars).
        let heading_level = heading.chars().take_while(|&c| c == '#').count();

        if heading_level == 0 {
            anyhow::bail!("FS: not a valid markdown heading: {:?}", heading);
        }

        let raw = std::fs::read_to_string(&full_path)
            .with_context(|| format!("FS: failed to read file {}", full_path.display()))?;

        let lines: Vec<&str> = raw.lines().collect();

        // Find the target heading line index.
        let heading_idx = lines
            .iter()
            .position(|l| l.trim_end() == heading)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "FS: heading {:?} not found in {}",
                    heading,
                    full_path.display()
                )
            })?;

        // Find insertion point: first line after `heading_idx` that is a heading
        // of equal or higher level (level <= heading_level), or any heading when
        // `shallow` is true.
        let insert_before = lines[heading_idx + 1..]
            .iter()
            .position(|l| {
                let hashes = l.chars().take_while(|&c| c == '#').count();
                // Must be a real heading: starts with at least one `#` followed by a space.
                hashes > 0
                    && l.chars().nth(hashes) == Some(' ')
                    && (shallow || hashes <= heading_level)
            })
            .map(|rel| heading_idx + 1 + rel); // absolute index

        // Build the new file content by splicing in a blank line + content.
        let mut result = String::with_capacity(raw.len() + content.len() + 2);

        match insert_before {
            Some(next_heading_idx) => {
                // Everything up to (but not including) the next heading.
                // Strip trailing blank lines from that block, then re-add one blank
                // line as separator before our content, then another before the heading.
                let before_lines = &lines[..next_heading_idx];

                // Trim trailing empty lines from the section.
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
                // Heading is the last section — append at EOF.
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

        // Atomic write: write to a sibling temp file, then rename.
        let tmp_path = full_path.with_extension("tmp");
        std::fs::write(&tmp_path, &result)
            .with_context(|| format!("FS: failed to write temp file {}", tmp_path.display()))?;
        crate::util::atomic_replace(&tmp_path, &full_path).with_context(|| {
            format!(
                "FS: failed to replace {} with {}",
                tmp_path.display(),
                full_path.display()
            )
        })?;

        Ok(())
    }

    /// List all entries in a directory with type information.
    ///
    /// Returns `.md` files (as stems) and subdirectories, sorted
    /// directories-first then alphabetically within each group.
    /// Non-`.md` files are excluded.
    pub fn list_directory_all(&self, relative_dir_path: &str) -> Result<Vec<VaultEntry>> {
        // Reject paths that attempt to escape the vault root
        if relative_dir_path.contains("..") {
            anyhow::bail!("FS: path must not contain '..'");
        }

        let full_path = self.resolve_path(relative_dir_path);

        if !full_path.is_dir() {
            anyhow::bail!("FS: directory not found: {}", full_path.display());
        }

        let mut entries: Vec<VaultEntry> = Vec::new();

        let dir_entries = std::fs::read_dir(&full_path)
            .with_context(|| format!("FS: failed to read directory {}", full_path.display()))?;

        for entry in dir_entries {
            let entry = entry
                .with_context(|| format!("FS: failed to read entry in {}", full_path.display()))?;

            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            // Skip hidden entries (dotfiles/dotdirs like .obsidian, .git, .trash)
            if name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                entries.push(VaultEntry {
                    name: name.to_string(),
                    is_dir: true,
                });
            } else if path.is_file()
                && let Some(ext) = path.extension()
                && ext == "md"
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                entries.push(VaultEntry {
                    name: stem.to_string(),
                    is_dir: false,
                });
            }
        }

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Ok(entries)
    }

    /// List `.md` files in a directory, returning their stem names.
    ///
    /// For example, a directory containing `latte.md` and `espresso.md`
    /// would return `["espresso", "latte"]` (sorted alphabetically).
    /// Non-`.md` files and subdirectories are excluded.
    pub fn list_directory(&self, relative_dir_path: &str) -> Result<Vec<String>> {
        if relative_dir_path.contains("..") {
            anyhow::bail!("FS: path must not contain '..'");
        }

        let full_path = self.resolve_path(relative_dir_path);

        if !full_path.is_dir() {
            anyhow::bail!("FS: directory not found: {}", full_path.display());
        }

        let mut names: Vec<String> = Vec::new();

        let entries = std::fs::read_dir(&full_path)
            .with_context(|| format!("FS: failed to read directory {}", full_path.display()))?;

        for entry in entries {
            let entry = entry
                .with_context(|| format!("FS: failed to read entry in {}", full_path.display()))?;

            let path = entry.path();

            if path.is_file()
                && let Some(ext) = path.extension()
                && ext == "md"
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }

        names.sort();
        Ok(names)
    }

    /// Read a single file at `relative_path` and return its UTF-8 content.
    ///
    /// Returns a typed `TransportReadError` so callers can distinguish
    /// "not found" from other I/O errors without platform-specific string
    /// matching (Windows and Unix error messages differ).
    ///
    /// Error mapping:
    /// - File does not exist (`ErrorKind::NotFound`) → `TransportReadError::NotFound`
    /// - Any other I/O error → `TransportReadError::Other`
    pub fn read_file(
        &self,
        relative_path: &str,
    ) -> std::result::Result<String, TransportReadError> {
        let full_path = self
            .resolve_path_validated(relative_path)
            .map_err(|e| TransportReadError::Other(e.to_string()))?;
        std::fs::read_to_string(&full_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TransportReadError::NotFound
            } else {
                TransportReadError::Other(format!(
                    "FS: failed to read file {}: {e}",
                    full_path.display()
                ))
            }
        })
    }
}
