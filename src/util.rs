use std::path::Path;

/// Atomically replace `dst` with `src` by writing to a temp file first.
///
/// On Unix, `std::fs::rename` overwrites the target atomically.
/// On Windows, `rename` fails if the target exists, so we must remove it first.
/// This leaves a small window where `dst` doesn't exist — acceptable for a
/// user-local config file (the temp file is the recovery copy).
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
