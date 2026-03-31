use pour::data::cache::Cache;
use tempfile::tempdir;

#[test]
fn round_trip_save_and_load() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    // Create, populate, and save.
    let mut cache = Cache::load_from(path.clone());
    cache.set("Beans/Origins", vec!["Ethiopia".into(), "Colombia".into()]);
    cache.set("Recipes/Methods", vec!["V60".into(), "Aeropress".into()]);
    cache.save().unwrap();

    // Load from the same path and verify.
    let loaded = Cache::load_from(path);
    let origins = loaded.get("Beans/Origins").expect("origins should exist");
    assert_eq!(origins, vec!["Ethiopia", "Colombia"]);

    let methods = loaded.get("Recipes/Methods").expect("methods should exist");
    assert_eq!(methods, vec!["V60", "Aeropress"]);
}

#[test]
fn load_missing_file_returns_empty_cache() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nonexistent").join("state.json");

    let cache = Cache::load_from(path);
    assert!(cache.get("anything").is_none());
}

#[test]
fn load_corrupt_file_returns_empty_cache() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    std::fs::write(&path, "NOT VALID JSON {{{").unwrap();

    let cache = Cache::load_from(path);
    assert!(cache.get("anything").is_none());
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("deep").join("nested").join("state.json");

    let mut cache = Cache::load_from(path.clone());
    cache.set("test/source", vec!["item".into()]);
    cache.save().unwrap();

    assert!(path.exists());

    // Verify we can load it back.
    let loaded = Cache::load_from(path);
    let items = loaded.get("test/source").expect("entry should exist");
    assert_eq!(items, vec!["item"]);
}

#[test]
fn get_returns_none_for_unknown_source() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let cache = Cache::load_from(path);
    assert!(cache.get("nonexistent/source").is_none());
}

#[test]
fn set_overwrites_existing_entry() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let mut cache = Cache::load_from(path);
    cache.set("source", vec!["old".into()]);
    cache.set("source", vec!["new1".into(), "new2".into()]);

    let items = cache.get("source").unwrap();
    assert_eq!(items, vec!["new1", "new2"]);
}
