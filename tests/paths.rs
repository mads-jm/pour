//! Tests for `POUR_HOME` / `POUR_CONFIG` path resolution.
//!
//! These tests mutate process-global env vars and therefore must run
//! serially. They restore prior state on exit.

use std::sync::Mutex;

use pour::paths;

/// Serialize env-var access across tests in this file.
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    key: &'static str,
    prior: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var(key).ok();
        // SAFETY: tests in this file are serialized via ENV_LOCK.
        unsafe {
            std::env::set_var(key, value);
        }
        EnvGuard { key, prior }
    }

    fn unset(key: &'static str) -> Self {
        let prior = std::env::var(key).ok();
        // SAFETY: tests in this file are serialized via ENV_LOCK.
        unsafe {
            std::env::remove_var(key);
        }
        EnvGuard { key, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests in this file are serialized via ENV_LOCK.
        unsafe {
            match &self.prior {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
fn pour_home_env_var_overrides_default() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let _g1 = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());
    let _g2 = EnvGuard::unset("POUR_CONFIG");

    let home = paths::pour_home();
    assert_eq!(home, tmp.path());

    assert_eq!(paths::config_path(), tmp.path().join("config.toml"));
    assert_eq!(paths::secrets_path(), tmp.path().join("secrets.toml"));
    assert_eq!(paths::presets_path(), tmp.path().join("presets.json"));
}

#[test]
fn cache_files_live_under_cache_subdir() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let _g1 = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());
    let _g2 = EnvGuard::unset("POUR_CONFIG");

    let cache = tmp.path().join("cache");
    assert_eq!(paths::cache_dir(), cache);
    assert_eq!(paths::state_path(), cache.join("state.json"));
    assert_eq!(paths::history_path(), cache.join("history.jsonl"));
    assert_eq!(
        paths::history_summary_path(),
        cache.join("history-summary.json"),
    );
}

#[test]
fn pour_config_overrides_pour_home_for_config_and_secrets() {
    let _lock = ENV_LOCK.lock().unwrap();
    let home_tmp = tempfile::tempdir().unwrap();
    let cfg_tmp = tempfile::tempdir().unwrap();
    let cfg_path = cfg_tmp.path().join("custom.toml");

    let _g1 = EnvGuard::set("POUR_HOME", home_tmp.path().to_str().unwrap());
    let _g2 = EnvGuard::set("POUR_CONFIG", cfg_path.to_str().unwrap());

    // POUR_CONFIG wins for config + secrets…
    assert_eq!(paths::config_path(), cfg_path);
    assert_eq!(paths::secrets_path(), cfg_tmp.path().join("secrets.toml"));

    // …but presets/cache still track POUR_HOME.
    assert_eq!(paths::presets_path(), home_tmp.path().join("presets.json"));
    assert_eq!(
        paths::state_path(),
        home_tmp.path().join("cache").join("state.json"),
    );
}
