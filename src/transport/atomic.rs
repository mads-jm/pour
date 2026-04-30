use std::path::Path;

/// Atomically replace `dst` with `src` by writing to a temp file first.
///
/// On Unix, `std::fs::rename` overwrites the target atomically.
/// On Windows, `rename` fails if the target exists, so we must remove it first.
/// This leaves a small window where `dst` doesn't exist — acceptable for a
/// user-local config file (the temp file is the recovery copy).
///
/// # Errors
///
/// Returns an `io::Error` if:
/// - `src` does not exist (`ErrorKind::NotFound`)
/// - `dst` is a directory
/// - On Windows: removing the existing `dst` fails for any reason other than
///   `NotFound`
/// - The underlying rename fails
///
/// # Windows non-atomicity
///
/// On Windows the implementation calls `remove_file(dst)` then `rename(src,
/// dst)`. A crash between those two calls leaves `src` (.tmp) on disk and `dst`
/// absent. This is documented by the `#[ignore]`d test
/// `windows_non_atomicity_window_exists` in `tests/util_atomic.rs`.
/// A truly atomic replacement (`MoveFileExW MOVEFILE_REPLACE_EXISTING`) is
/// deferred to a later slice.
pub fn atomic_replace(src: &Path, dst: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // Remove old file first; ignore "not found" errors.
        match std::fs::remove_file(dst) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    std::fs::rename(src, dst)
}
