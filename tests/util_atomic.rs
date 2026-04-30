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
// Case 7 — Windows non-atomicity documentation
//
// On Windows, atomic_replace calls remove_file(dst) then rename(src, dst).
// A process crash between those two calls leaves src (.tmp) on disk and
// dst (config.toml) absent — the user is left without a config.
//
// This test documents the window by verifying that remove_file succeeds
// independently of rename, i.e. there truly are two observable OS calls.
// Un-ignore once Slice 1 replaces this with a truly atomic implementation
// (MoveFileExW with MOVEFILE_REPLACE_EXISTING on Windows).
// ---------------------------------------------------------------------------
#[cfg(windows)]
#[test]
#[ignore = "documents non-atomicity bug on Windows; un-ignore after Slice 1 fix"]
fn windows_non_atomicity_window_exists() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let dir = tempdir().unwrap();
    let src = dir.path().join("source.tmp");
    let dst = dir.path().join("destination.toml");

    fs::write(&src, b"new").unwrap();
    fs::write(&dst, b"old").unwrap();

    // Simulate what atomic_replace does on Windows manually, inserting an
    // observation point between remove_file and rename to confirm the gap.
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = Arc::clone(&barrier);

    let dst_path = dst.clone();
    let observer = thread::spawn(move || {
        // Wait until remove_file has been called.
        barrier_clone.wait();
        // At this exact moment, dst should not exist on disk.
        !dst_path.exists()
    });

    // Step 1: remove dst
    fs::remove_file(&dst).unwrap();
    // Signal observer thread — dst is now absent.
    barrier.wait();

    // Step 2: rename src -> dst
    fs::rename(&src, &dst).unwrap();

    let gap_was_observed = observer.join().unwrap();
    assert!(
        gap_was_observed,
        "expected dst to be absent between remove_file and rename (atomicity window)"
    );
}
