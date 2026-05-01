use pour::util::atomic_replace;
use std::fs;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Case 1 — Happy path: src exists, dst absent
// ---------------------------------------------------------------------------
#[test]
fn happy_path_src_exists_dst_absent() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("source.tmp");
    let dst = dir.path().join("destination.toml");

    fs::write(&src, b"hello world").unwrap();
    assert!(!dst.exists());

    atomic_replace(&src, &dst).unwrap();

    assert!(!src.exists(), "src should be gone after rename");
    assert_eq!(fs::read(&dst).unwrap(), b"hello world");
}

// ---------------------------------------------------------------------------
// Case 2 — Replace existing: src and dst both exist
// ---------------------------------------------------------------------------
#[test]
fn replace_existing_dst() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("source.tmp");
    let dst = dir.path().join("destination.toml");

    fs::write(&src, b"new content").unwrap();
    fs::write(&dst, b"old content").unwrap();

    atomic_replace(&src, &dst).unwrap();

    assert!(!src.exists(), "src should be gone after rename");
    assert_eq!(fs::read(&dst).unwrap(), b"new content");
}

// ---------------------------------------------------------------------------
// Case 3 — Source missing: pin current behavior (Err, NotFound)
// ---------------------------------------------------------------------------
#[test]
fn source_missing_returns_err() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("nonexistent.tmp");
    let dst = dir.path().join("destination.toml");

    let result = atomic_replace(&src, &dst);

    assert!(
        result.is_err(),
        "expected Err when src does not exist, got Ok"
    );
    assert_eq!(
        result.unwrap_err().kind(),
        std::io::ErrorKind::NotFound,
        "expected NotFound error kind"
    );
}

// ---------------------------------------------------------------------------
// Case 4 — Destination is a directory: pin current behavior
// ---------------------------------------------------------------------------
// On both Unix and Windows, renaming a file onto an existing directory fails.
// Pin whatever the OS returns so we know when behavior changes.
#[test]
fn dst_is_a_directory_returns_err() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("source.tmp");
    let dst = dir.path().join("existing_dir");

    fs::write(&src, b"data").unwrap();
    fs::create_dir(&dst).unwrap();

    let result = atomic_replace(&src, &dst);

    assert!(
        result.is_err(),
        "expected Err when dst is a directory, got Ok"
    );
}

// ---------------------------------------------------------------------------
// Case 5 — Unix-only: permission failure on dst directory
// ---------------------------------------------------------------------------
// Skip on Windows — removing write permission on a directory is not portable
// and behaves differently under different Windows ACL configurations.
#[cfg(unix)]
#[test]
fn unix_read_only_dst_dir_returns_err() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let ro_dir = dir.path().join("readonly");
    fs::create_dir(&ro_dir).unwrap();

    let src = dir.path().join("source.tmp");
    fs::write(&src, b"data").unwrap();

    let dst = ro_dir.join("output.toml");

    // Remove write+exec from the directory so rename/create inside it fails.
    fs::set_permissions(&ro_dir, fs::Permissions::from_mode(0o444)).unwrap();

    let result = atomic_replace(&src, &dst);

    // Restore permissions so tempdir cleanup can succeed.
    fs::set_permissions(&ro_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        result.is_err(),
        "expected Err when dst directory is read-only"
    );
}

// ---------------------------------------------------------------------------
// Case 6 — Round-trip: write content, atomic_replace, read back, content matches
// ---------------------------------------------------------------------------
#[test]
fn round_trip_content_survives() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("draft.tmp");
    let dst = dir.path().join("config.toml");

    let payload = b"[vault]\nbase_path = \"/some/path\"\n";
    fs::write(&src, payload).unwrap();

    atomic_replace(&src, &dst).unwrap();

    let read_back = fs::read(&dst).unwrap();
    assert_eq!(read_back, payload);
}

// ---------------------------------------------------------------------------
// Case 7 — Windows atomicity regression test
//
// atomic_replace now delegates entirely to std::fs::rename, which on Windows
// calls MoveFileExW(src, dst, MOVEFILE_REPLACE_EXISTING). That is a single
// atomic OS operation — there is no window between remove and rename.
//
// This test verifies that dst remains continuously present throughout the
// operation when replacing an existing file. We do this by calling
// atomic_replace on the main thread while a background thread polls for
// dst's existence. The background thread must never observe dst absent.
//
// Previously ignored because the old implementation had an explicit
// remove_file/rename split that made the gap real and observable. Now that
// the gap is gone, this serves as a regression guard.
// ---------------------------------------------------------------------------
#[cfg(windows)]
#[test]
fn windows_atomic_replace_no_gap() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration;

    let dir = tempdir().unwrap();
    let src = dir.path().join("source.tmp");
    let dst = dir.path().join("destination.toml");

    fs::write(&src, b"new").unwrap();
    fs::write(&dst, b"old").unwrap();

    let dst_path = dst.clone();
    let gap_observed = Arc::new(AtomicBool::new(false));
    let gap_observed_clone = Arc::clone(&gap_observed);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);

    // Poller: spin-checks dst existence until told to stop.
    let poller = thread::spawn(move || {
        while !stop_clone.load(Ordering::Relaxed) {
            if !dst_path.exists() {
                gap_observed_clone.store(true, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_micros(10));
        }
    });

    // Give the poller time to start before we call atomic_replace.
    thread::sleep(Duration::from_millis(5));

    atomic_replace(&src, &dst).unwrap();

    stop.store(true, Ordering::Relaxed);
    poller.join().unwrap();

    assert!(
        !gap_observed.load(Ordering::Relaxed),
        "dst was absent during atomic_replace — atomicity gap detected"
    );
}
