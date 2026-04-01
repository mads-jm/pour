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

    /// Create a new file at `relative_path` with the given content.
    ///
    /// Parent directories are created automatically.
    /// Returns an error if the file already exists.
    pub fn create_file(&self, relative_path: &str, content: &str) -> Result<()> {
        let full_path = self.resolve_path(relative_path);

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

        let full_path = self.resolve_path(relative_path);

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
    /// If the heading is the last section in the file, content is appended at EOF.
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
    ) -> Result<()> {
        let full_path = self.resolve_path(relative_path);

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
        // of equal or higher level (level <= heading_level).
        let insert_before = lines[heading_idx + 1..]
            .iter()
            .position(|l| {
                let hashes = l.chars().take_while(|&c| c == '#').count();
                // Must be a real heading: starts with at least one `#` followed by a space.
                hashes > 0 && l.chars().nth(hashes) == Some(' ') && hashes <= heading_level
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
        std::fs::rename(&tmp_path, &full_path).with_context(|| {
            format!(
                "FS: failed to rename {} to {}",
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
            let entry = entry.with_context(|| {
                format!("FS: failed to read entry in {}", full_path.display())
            })?;

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
}
