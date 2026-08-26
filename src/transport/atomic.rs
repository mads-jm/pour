use std::path::{Path, PathBuf};

/// Atomically replace `dst` with `src`.
///
/// On both Unix and Windows, `std::fs::rename` performs an atomic replace:
/// - **Unix**: `rename(2)` is atomic by POSIX — the directory entry for `dst`
///   is swapped in a single operation; readers always see either the old or
///   new file, never an absent one.
/// - **Windows**: the Rust stdlib calls `MoveFileExW(src, dst,
///   MOVEFILE_REPLACE_EXISTING)`, which replaces `dst` atomically when
///   supported by the filesystem. A fallback via `SetFileInformationByHandle`
///   with `FILE_RENAME_FLAG_REPLACE_IF_EXISTS | FILE_RENAME_FLAG_POSIX_SEMANTICS`
///   covers edge cases (e.g., read-only attributes).
///
/// The previous Windows implementation called `remove_file(dst)` then
/// `rename(src, dst)`, which introduced a window where `dst` did not exist.
/// That workaround is unnecessary — `std::fs::rename` handles the replace
/// atomically itself.
///
/// # Errors
///
/// Returns an `io::Error` if:
/// - `src` does not exist (`ErrorKind::NotFound`)
/// - `dst` is a directory
/// - The underlying rename fails (permissions, cross-device move, etc.)
pub fn atomic_replace(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::rename(src, dst)
}

/// Resolve the real file a write should land on, following a symlink at `path`.
///
/// [`atomic_replace`] finishes with `rename(2)`, which swaps a *directory
/// entry*. Renaming onto a symlink therefore replaces the link itself with a
/// regular file, and the file the link pointed at keeps its old contents. Any
/// dotfiles setup that links a config into a tracked directory (stow, chezmoi,
/// a bare repo) loses the link on the first write, and the edit lands somewhere
/// version control cannot see.
///
/// When `path` is a symlink this returns the file it resolves to, so callers
/// place their temp file beside the *real* target and rename onto that. Doing
/// so also keeps the rename on one filesystem: a link pointing across a mount
/// point would otherwise fail with `EXDEV`, since the temp file would be
/// created next to the link rather than next to the target.
///
/// Everything else is returned unchanged — a regular file, a path that does not
/// exist yet, or a dangling symlink (there is no target worth preserving). A
/// symlinked *parent* directory needs no handling either: the temp file and the
/// rename target both resolve through it already, so the link survives.
///
/// On Windows, [`std::fs::canonicalize`] returns an extended-length `\\?\`
/// path. That is confined to the symlink branch, so ordinary files keep the
/// caller's path verbatim in any error message they build.
pub fn resolve_write_target(path: &Path) -> PathBuf {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}
