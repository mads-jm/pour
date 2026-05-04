use pour::data::json_store::JsonStore;
use serde::{Deserialize, Serialize};
use tempfile::tempdir;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Sample {
    name: String,
    count: u32,
    items: Vec<String>,
}

#[test]
fn load_missing_file_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.json");
    let store: JsonStore<Sample> = JsonStore::new(path);

    let value = store.load();
    assert_eq!(value, Sample::default());
}

#[test]
fn save_then_load_round_trips() {
    let dir = tempdir().unwrap();
    let store: JsonStore<Sample> = JsonStore::new(dir.path().join("sample.json"));

    let original = Sample {
        name: "pour".into(),
        count: 42,
        items: vec!["a".into(), "b".into()],
    };
    store.save(&original).unwrap();

    let recovered = store.load();
    assert_eq!(original, recovered);
}

#[test]
fn save_creates_missing_parent_directories() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a").join("b").join("c").join("file.json");
    let store: JsonStore<Sample> = JsonStore::new(nested.clone());

    let value = Sample {
        name: "nested".into(),
        ..Default::default()
    };
    store.save(&value).unwrap();

    assert!(nested.exists(), "expected nested file to be created");
}

#[test]
fn load_with_migration_returns_normal_value_on_valid_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sample.json");
    let store: JsonStore<Sample> = JsonStore::new(path);
    store
        .save(&Sample {
            name: "ok".into(),
            count: 1,
            items: vec![],
        })
        .unwrap();

    let value = store.load_with_migration(|_raw| {
        panic!("migrate must not be called when straight deserialize succeeds")
    });
    assert_eq!(value.name, "ok");
}

#[test]
fn load_with_migration_recovers_from_legacy_schema() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("legacy.json");
    std::fs::write(&path, r#"{"legacy_name":"old","n":7}"#).unwrap();

    let store: JsonStore<Sample> = JsonStore::new(path);
    let value = store.load_with_migration(|raw| {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Some(Sample {
            name: v.get("legacy_name")?.as_str()?.to_owned(),
            count: v.get("n")?.as_u64()? as u32,
            items: vec![],
        })
    });

    assert_eq!(value.name, "old");
    assert_eq!(value.count, 7);
}

#[test]
fn load_with_migration_returns_default_when_migrate_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bogus.json");
    std::fs::write(&path, "not even close to json").unwrap();

    let store: JsonStore<Sample> = JsonStore::new(path);
    let value = store.load_with_migration(|_raw| None);
    assert_eq!(value, Sample::default());
}

#[test]
fn save_does_not_leave_tmp_file_after_success() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("clean.json");
    let store: JsonStore<Sample> = JsonStore::new(path.clone());

    store.save(&Sample::default()).unwrap();

    let tmp = path.with_extension("tmp");
    assert!(
        !tmp.exists(),
        "expected .tmp to be cleaned up after success"
    );
}

#[test]
fn path_accessor_returns_constructed_path() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("x.json");
    let store: JsonStore<Sample> = JsonStore::new(p.clone());
    assert_eq!(store.path(), p);
}
