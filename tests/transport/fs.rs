use pour::transport::fs::FsWriter;
use std::path::PathBuf;

#[test]
fn new_stores_base_path() {
    let writer = FsWriter::new(PathBuf::from("/tmp/vault"));
    assert_eq!(writer.base_path(), &PathBuf::from("/tmp/vault"));
}

#[test]
fn create_file_writes_content() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    writer
        .create_file("note.md", "# Hello\n")
        .expect("create_file should succeed");

    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert_eq!(content, "# Hello\n");
}

#[test]
fn create_file_creates_parent_directories() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    writer
        .create_file("sub/dir/note.md", "nested")
        .expect("create_file should create parent dirs");

    assert!(dir.path().join("sub/dir/note.md").exists());
    let content = std::fs::read_to_string(dir.path().join("sub/dir/note.md")).unwrap();
    assert_eq!(content, "nested");
}

#[test]
fn create_file_errors_if_file_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    std::fs::write(dir.path().join("existing.md"), "old").unwrap();

    let result = writer.create_file("existing.md", "new");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("already exists"),
        "expected 'already exists' error, got: {msg}"
    );
}

#[test]
fn append_to_file_appends_content() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    std::fs::write(dir.path().join("note.md"), "line1\n").unwrap();

    writer
        .append_to_file("note.md", "line2\n")
        .expect("append_to_file should succeed");

    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert_eq!(content, "line1\nline2\n");
}

#[test]
fn append_to_file_errors_if_not_found() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.append_to_file("missing.md", "data");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "expected 'not found' error, got: {msg}"
    );
}

#[test]
fn list_directory_returns_md_stems_sorted() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let beans = dir.path().join("Beans");
    std::fs::create_dir_all(&beans).unwrap();

    std::fs::write(beans.join("latte.md"), "").unwrap();
    std::fs::write(beans.join("espresso.md"), "").unwrap();
    std::fs::write(beans.join("cappuccino.md"), "").unwrap();
    // Non-md file should be excluded
    std::fs::write(beans.join("notes.txt"), "").unwrap();

    let writer = FsWriter::new(dir.path().to_path_buf());
    let names = writer
        .list_directory("Beans")
        .expect("list_directory should succeed");

    assert_eq!(names, vec!["cappuccino", "espresso", "latte"]);
}

#[test]
fn list_directory_excludes_subdirectories() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = dir.path().join("Notes");
    std::fs::create_dir_all(base.join("subdir")).unwrap();
    std::fs::write(base.join("file.md"), "").unwrap();

    let writer = FsWriter::new(dir.path().to_path_buf());
    let names = writer.list_directory("Notes").expect("should succeed");

    assert_eq!(names, vec!["file"]);
}

#[test]
fn list_directory_errors_if_not_a_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.list_directory("nonexistent");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "expected 'not found' error, got: {msg}"
    );
}

// ── append_under_heading ─────────────────────────────────────────────────────

#[test]
fn append_under_heading_inserts_before_next_same_level_heading() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let initial = "# Title\n\n## Log\n\n- existing entry\n\n## Other\n\nsome text\n";
    std::fs::write(dir.path().join("daily.md"), initial).unwrap();

    writer
        .append_under_heading("daily.md", "## Log", "- new entry", false)
        .expect("should succeed");

    let content = std::fs::read_to_string(dir.path().join("daily.md")).unwrap();

    // New entry must appear after existing content, before ## Other.
    let log_pos = content.find("- existing entry").unwrap();
    let new_pos = content.find("- new entry").unwrap();
    let other_pos = content.find("## Other").unwrap();
    assert!(log_pos < new_pos, "new entry should follow existing entry");
    assert!(new_pos < other_pos, "new entry should precede ## Other");
}

#[test]
fn append_under_heading_last_section_appends_at_eof() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let initial = "# Title\n\n## Log\n\n- first\n";
    std::fs::write(dir.path().join("note.md"), initial).unwrap();

    writer
        .append_under_heading("note.md", "## Log", "- second", false)
        .expect("should succeed");

    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    let first_pos = content.find("- first").unwrap();
    let second_pos = content.find("- second").unwrap();
    assert!(first_pos < second_pos, "- second should follow - first");
    assert!(
        content.ends_with("- second\n"),
        "file should end with the new entry"
    );
}

#[test]
fn append_under_heading_errors_when_heading_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    std::fs::write(dir.path().join("note.md"), "## Present\n\ncontent\n").unwrap();

    let result = writer.append_under_heading("note.md", "## Missing", "data", false);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "expected 'not found' error, got: {msg}"
    );
}

#[test]
fn append_under_heading_errors_when_file_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.append_under_heading("ghost.md", "## Log", "data", false);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "expected 'not found' error, got: {msg}"
    );
}

#[test]
fn append_under_heading_does_not_stop_at_deeper_subheading() {
    // ## Log should NOT treat ### Sub as a section boundary.
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let initial = "## Log\n\n- existing\n\n### Sub\n\nsubcontent\n\n## Next\n\nnext content\n";
    std::fs::write(dir.path().join("note.md"), initial).unwrap();

    writer
        .append_under_heading("note.md", "## Log", "- appended", false)
        .expect("should succeed");

    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();

    // All three markers must be present in order.
    let sub_pos = content.find("### Sub").unwrap();
    let appended_pos = content.find("- appended").unwrap();
    let next_pos = content.find("## Next").unwrap();

    // Appended content must come after the subheading's subcontent and before ## Next.
    assert!(
        sub_pos < appended_pos,
        "appended content should follow ### Sub block"
    );
    assert!(
        appended_pos < next_pos,
        "appended content should precede ## Next"
    );
}

#[test]
fn append_under_heading_shallow_stops_at_deeper_subheading() {
    // With shallow=true, ### Tasks should treat #### Completed as a boundary.
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let initial = "### Tasks\n\n- existing\n\n#### Completed\n\n- [x] done\n\n## Next\n";
    std::fs::write(dir.path().join("note.md"), initial).unwrap();

    writer
        .append_under_heading("note.md", "### Tasks", "- new task", true)
        .expect("should succeed");

    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();

    let new_pos = content.find("- new task").unwrap();
    let completed_pos = content.find("#### Completed").unwrap();
    assert!(
        new_pos < completed_pos,
        "with shallow=true, new task should appear before #### Completed, got:\n{content}"
    );
}

#[test]
fn list_directory_returns_empty_vec_for_empty_dir() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();

    let writer = FsWriter::new(dir.path().to_path_buf());
    let names = writer.list_directory("empty").expect("should succeed");

    assert!(names.is_empty());
}

// ── path traversal rejection ─────────────────────────────────────────────────

#[test]
fn create_file_rejects_dotdot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.create_file("../escape.md", "evil");
    assert!(result.is_err(), "create_file should reject '..'");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains(".."), "error should mention '..', got: {msg}");
}

#[test]
fn append_to_file_rejects_absolute_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    // Use platform-appropriate absolute path.
    #[cfg(windows)]
    let abs = "C:\\Windows\\system32\\evil.md";
    #[cfg(not(windows))]
    let abs = "/etc/passwd";

    let result = writer.append_to_file(abs, "evil");
    assert!(
        result.is_err(),
        "append_to_file should reject absolute paths"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("absolute") || msg.contains("..") || msg.contains("relative"),
        "error should describe the rejection, got: {msg}"
    );
}

#[test]
fn append_under_heading_rejects_dotdot_in_middle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.append_under_heading("notes/../../escape.md", "## Log", "evil", false);
    assert!(
        result.is_err(),
        "append_under_heading should reject '..' in middle of path"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains(".."), "error should mention '..', got: {msg}");
}

#[test]
fn read_file_rejects_dotdot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.read_file("../secret.md");
    assert!(result.is_err(), "read_file should reject '..'");
    // read_file returns TransportReadError — check the Display output.
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("..") || msg.contains("read error"),
        "error should indicate traversal rejection, got: {msg}"
    );
}

#[test]
fn create_file_rejects_tilde_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.create_file("~/.ssh/authorized_keys", "evil");
    assert!(result.is_err(), "create_file should reject '~' paths");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains('~') || msg.contains("relative"),
        "error should describe '~' rejection, got: {msg}"
    );
}

#[test]
fn create_file_accepts_safe_relative_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = FsWriter::new(dir.path().to_path_buf());

    let result = writer.create_file("notes/coffee.md", "# Coffee\n");
    assert!(
        result.is_ok(),
        "create_file should accept safe relative paths, got: {:?}",
        result.unwrap_err()
    );
    assert!(dir.path().join("notes/coffee.md").exists());
}

// ─── Frontmatter patch: the guarded surgical edit (spec §2.2) ───────────────

use pour::output::frontmatter::FrontmatterValue;
use pour::transport::{TransportPatchError, classify_patch_status};

const NOTE: &str = "---\ncannabis: false\nwater: null\nmood: \"ok\"\n---\n\n# 20260805\n\nbody\n";

fn seeded_note(dir: &tempfile::TempDir, rel: &str) -> FsWriter {
    let writer = FsWriter::new(dir.path().to_path_buf());
    writer.create_file(rel, NOTE).expect("seed note");
    writer
}

#[test]
fn patch_frontmatter_replaces_an_existing_key() {
    let dir = tempfile::tempdir().unwrap();
    let writer = seeded_note(&dir, "daily/20260805.md");

    writer
        .patch_frontmatter(
            "daily/20260805.md",
            "water",
            &FrontmatterValue::Number(16.0),
        )
        .expect("patch should succeed");

    let after = std::fs::read_to_string(dir.path().join("daily/20260805.md")).unwrap();
    assert_eq!(after, NOTE.replace("water: null", "water: 16"));
}

#[test]
fn patch_frontmatter_inserts_a_missing_key() {
    let dir = tempfile::tempdir().unwrap();
    let writer = seeded_note(&dir, "note.md");

    writer
        .patch_frontmatter("note.md", "steps", &FrontmatterValue::Number(8000.0))
        .expect("a stale template must not block the capture");

    let after = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert_eq!(
        after,
        NOTE.replace("mood: \"ok\"\n---", "mood: \"ok\"\nsteps: 8000\n---")
    );
}

#[test]
fn patch_frontmatter_leaves_the_body_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let writer = seeded_note(&dir, "note.md");

    writer
        .patch_frontmatter("note.md", "cannabis", &FrontmatterValue::Bool(true))
        .unwrap();

    let after = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert!(
        after.ends_with("---\n\n# 20260805\n\nbody\n"),
        "got: {after:?}"
    );
}

#[test]
fn patch_frontmatter_on_a_missing_note_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let writer = FsWriter::new(dir.path().to_path_buf());

    let err = writer
        .patch_frontmatter("nope.md", "water", &FrontmatterValue::Number(1.0))
        .expect_err("pour never fabricates the note");
    assert!(matches!(err, TransportPatchError::NotFound), "got {err:?}");
}

#[test]
fn patch_frontmatter_on_a_note_without_frontmatter_errors_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let writer = FsWriter::new(dir.path().to_path_buf());
    writer
        .create_file("plain.md", "# No frontmatter\n")
        .unwrap();

    let err = writer
        .patch_frontmatter("plain.md", "water", &FrontmatterValue::Number(1.0))
        .expect_err("no block to patch");
    assert!(matches!(err, TransportPatchError::Other(_)), "got {err:?}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("plain.md")).unwrap(),
        "# No frontmatter\n"
    );
}

/// Simulate a concurrent writer: replace the file's content and push its mtime
/// forward explicitly, so the test does not depend on filesystem timestamp
/// resolution.
fn external_write(path: &std::path::Path, content: &str, mtime: std::time::SystemTime) {
    std::fs::write(path, content).unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(mtime).unwrap();
}

#[test]
fn a_write_that_lands_between_the_baseline_stat_and_the_read_aborts() {
    // The race the guard exists for, reproduced exactly: pour stats the file,
    // Obsidian saves it, *then* pour reads. The bytes the edit is computed from
    // are already the concurrent writer's, so the re-stat before the write must
    // catch the difference against the pre-read baseline and abort.
    let dir = tempfile::tempdir().unwrap();
    let writer = seeded_note(&dir, "note.md");
    let path = dir.path().join("note.md");

    let baseline = std::fs::metadata(&path).and_then(|m| m.modified()).unwrap();

    const OBSIDIAN_WROTE: &str =
        "---\ncannabis: true\nwater: 40\nmood: \"ok\"\n---\n\n# 20260805\n\nbody\n";
    external_write(
        &path,
        OBSIDIAN_WROTE,
        baseline + std::time::Duration::from_secs(5),
    );

    let err = writer
        .patch_frontmatter_since(
            "note.md",
            baseline,
            "water",
            &FrontmatterValue::Number(16.0),
        )
        .expect_err("a file that moved after the baseline must abort the patch");

    assert!(
        matches!(err, TransportPatchError::Conflict(_)),
        "got {err:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        OBSIDIAN_WROTE,
        "the concurrent writer's bytes must survive untouched"
    );
    assert!(
        !dir.path().join("note.tmp").exists(),
        "no orphan temp file may survive an aborted write"
    );
}

#[test]
fn guarded_write_aborts_without_writing_when_the_mtime_moved() {
    let dir = tempfile::tempdir().unwrap();
    let writer = seeded_note(&dir, "note.md");

    // A deliberately stale baseline stands in for "another process wrote this
    // file between our stat and our write".
    let stale = std::time::SystemTime::UNIX_EPOCH;
    let err = writer
        .patch_frontmatter_since("note.md", stale, "water", &FrontmatterValue::Number(1.0))
        .expect_err("mtime mismatch must abort");

    assert!(
        matches!(err, TransportPatchError::Conflict(_)),
        "got {err:?}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.md")).unwrap(),
        NOTE,
        "an aborted guarded write must leave the file byte-identical"
    );
    assert!(
        !dir.path().join("note.tmp").exists(),
        "no orphan temp file may survive an aborted write"
    );
}

#[test]
fn guarded_write_succeeds_when_the_mtime_matches() {
    let dir = tempfile::tempdir().unwrap();
    let writer = seeded_note(&dir, "note.md");

    let mtime = std::fs::metadata(dir.path().join("note.md"))
        .and_then(|m| m.modified())
        .unwrap();

    writer
        .patch_frontmatter_since("note.md", mtime, "water", &FrontmatterValue::Number(16.0))
        .expect("an unchanged file must accept the write");

    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.md")).unwrap(),
        NOTE.replace("water: null", "water: 16")
    );
}

#[test]
fn classify_patch_status_degrades_old_plugins_but_not_missing_notes() {
    // A Local REST API older than v3.0 rejects `Target-Type: frontmatter`
    // outright — that is a degrade signal, not a capture failure (§2.1).
    for status in [400u16, 405, 415, 501] {
        assert!(
            matches!(
                classify_patch_status(status, "unsupported"),
                TransportPatchError::Unsupported(_)
            ),
            "HTTP {status} should degrade to the filesystem"
        );
    }
    // A missing note is §2.3's problem and must stay distinguishable.
    assert!(matches!(
        classify_patch_status(404, ""),
        TransportPatchError::NotFound
    ));
    assert!(matches!(
        classify_patch_status(500, "boom"),
        TransportPatchError::Other(_)
    ));
}
