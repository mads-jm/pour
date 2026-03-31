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
        .append_under_heading("daily.md", "## Log", "- new entry")
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
        .append_under_heading("note.md", "## Log", "- second")
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

    let result = writer.append_under_heading("note.md", "## Missing", "data");
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

    let result = writer.append_under_heading("ghost.md", "## Log", "data");
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
        .append_under_heading("note.md", "## Log", "- appended")
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
fn list_directory_returns_empty_vec_for_empty_dir() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();

    let writer = FsWriter::new(dir.path().to_path_buf());
    let names = writer.list_directory("empty").expect("should succeed");

    assert!(names.is_empty());
}
