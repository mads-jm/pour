use std::path::Path;

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
