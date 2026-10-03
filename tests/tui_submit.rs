//! `handle_submit` and the completion sound.
//!
//! Drives a real submit over the filesystem transport into a temp vault, with
//! the app's `Chime` swapped for a fake `Player`, so the tests can see whether
//! playback was attempted without anything audible playing.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use pour::app::{App, Screen};
use pour::config::Config;
use pour::data::cache::Cache;
use pour::data::field_presets::FieldPresets;
use pour::data::history::History;
use pour::data::presets::Presets;
use pour::sound::{Chime, Player};
use pour::transport::Transport;
use pour::transport::fs::FsWriter;

/// One module per write mode, plus a create module whose hook always fails.
/// `{sound}` is replaced with the `[sound]` table under test (or nothing).
const TOML: &str = r####"{sound}
[vault]
base_path = "{base}"

[modules.note]
mode = "create"
path = "notes/%Y%m%d-%H%M%S.md"

[[modules.note.fields]]
name = "title"
field_type = "text"
prompt = "Title"

[modules.log]
mode = "append"
path = "log/%Y%m%d.md"
append_under_header = "## Log"
append_template = "- {{body}}"

[[modules.log.fields]]
name = "body"
field_type = "text"
prompt = "Body"

[modules.habit]
mode = "update"
path = "daily/%Y%m%d.md"

[[modules.habit.fields]]
name = "meditated"
field_type = "toggle"
prompt = "Meditated?"

[modules.hooked]
mode = "create"
path = "hooked/%Y%m%d-%H%M%S.md"
post_write_shell = "exit 3"

[[modules.hooked.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####;

/// Counts `start` calls and plays nothing.
#[derive(Default)]
struct CountingPlayer {
    starts: AtomicUsize,
}

impl CountingPlayer {
    fn starts(&self) -> usize {
        self.starts.load(Ordering::SeqCst)
    }
}

impl Player for CountingPlayer {
    fn start(&self, _failures: Sender<String>) {
        self.starts.fetch_add(1, Ordering::SeqCst);
    }
}

/// Fails every time, the way a machine with no output device does.
struct FailingPlayer;

impl Player for FailingPlayer {
    fn start(&self, failures: Sender<String>) {
        let _ = failures.send("no audio output device".to_string());
    }
}

struct Fixture {
    dir: tempfile::TempDir,
    app: App,
    cache: Cache,
}

/// A temp vault and an `App` over it. `sound` is spliced in ahead of
/// `[vault]`; pass `""` for a config with no `[sound]` table at all.
fn fixture(sound: &str, player: Arc<dyn Player>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_string_lossy().replace('\\', "\\\\");
    let toml = TOML.replace("{sound}", sound).replace("{base}", &base);
    let config = Config::from_toml(&toml).expect("fixture config must validate");

    let mut app = App::new(
        config,
        Transport::Fs(FsWriter::new(dir.path().to_path_buf())),
        History::load_from(dir.path().join("history.json")),
        Presets::empty(),
        FieldPresets::empty(),
    );
    app.chime = Chime::new(player);
    let cache = Cache::load_from(dir.path().join("cache.json"));
    Fixture { dir, app, cache }
}

/// Seed today's note for the `log` and `habit` modules, which write into a
/// note that already exists.
fn seed_today(dir: &tempfile::TempDir) {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    std::fs::create_dir_all(dir.path().join("log")).unwrap();
    std::fs::write(
        dir.path().join(format!("log/{today}.md")),
        "# Log\n\n## Log\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("daily")).unwrap();
    std::fs::write(
        dir.path().join(format!("daily/{today}.md")),
        "---\nmeditated: false\n---\n\nbody\n",
    )
    .unwrap();
}

/// Open `module`'s form, fill `values`, and submit it.
async fn submit(f: &mut Fixture, module: &str, values: &[(&str, &str)]) {
    f.app.selected_module = f
        .app
        .module_keys
        .iter()
        .position(|k| k == module)
        .expect("module in fixture");
    let mut form = f.app.init_form(module).expect("form opens");
    for (k, v) in values {
        form.field_values.insert(k.to_string(), v.to_string());
    }
    f.app.form_state = Some(form);
    pour::tui::handle_submit(&mut f.app, &mut f.cache).await;
}

fn saved(app: &App) -> bool {
    app.screen == Screen::Summary
        && app
            .summary_state
            .as_ref()
            .is_some_and(|s| s.file_path.is_some())
}

// ── Key absent or off: no playback attempted ────────────────────────────────

#[tokio::test]
async fn no_sound_table_never_starts_the_player() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("", player.clone());
    seed_today(&f.dir);

    submit(&mut f, "note", &[("title", "a")]).await;
    assert!(saved(&f.app), "the create capture must succeed");
    submit(&mut f, "log", &[("body", "b")]).await;
    assert!(saved(&f.app), "the append capture must succeed");
    submit(&mut f, "habit", &[("meditated", "true")]).await;
    assert!(saved(&f.app), "the update capture must succeed");

    assert_eq!(player.starts(), 0);
}

#[tokio::test]
async fn on_save_false_never_starts_the_player() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("[sound]\non_save = false", player.clone());

    submit(&mut f, "note", &[("title", "a")]).await;

    assert!(saved(&f.app));
    assert_eq!(player.starts(), 0);
}

// ── Key on: one tone per saved capture, every write mode ────────────────────

#[tokio::test]
async fn create_save_starts_the_player_once() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("[sound]\non_save = true", player.clone());

    submit(&mut f, "note", &[("title", "a")]).await;

    assert!(saved(&f.app));
    assert_eq!(player.starts(), 1);
}

#[tokio::test]
async fn append_save_starts_the_player_once() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("[sound]\non_save = true", player.clone());
    seed_today(&f.dir);

    submit(&mut f, "log", &[("body", "b")]).await;

    assert!(saved(&f.app));
    assert_eq!(player.starts(), 1);
}

#[tokio::test]
async fn update_save_starts_the_player_once() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("[sound]\non_save = true", player.clone());
    seed_today(&f.dir);

    submit(&mut f, "habit", &[("meditated", "true")]).await;

    assert!(saved(&f.app));
    assert_eq!(player.starts(), 1);
}

#[tokio::test]
async fn failed_write_plays_nothing() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("[sound]\non_save = true", player.clone());
    // No seed: an `update` module never creates its note, so this write fails.

    submit(&mut f, "habit", &[("meditated", "true")]).await;

    let summary = f.app.summary_state.as_ref().expect("summary shown");
    assert!(summary.file_path.is_none(), "write should have failed");
    assert!(summary.message.starts_with("Write failed"));
    assert_eq!(player.starts(), 0);
}

#[tokio::test]
async fn failing_hook_does_not_suppress_the_tone() {
    let player = Arc::new(CountingPlayer::default());
    let mut f = fixture("[sound]\non_save = true", player.clone());

    submit(&mut f, "hooked", &[("title", "a")]).await;

    assert!(saved(&f.app));
    let message = &f.app.summary_state.as_ref().unwrap().message;
    assert!(message.contains("Warning"), "hook should warn: {message}");
    assert_eq!(player.starts(), 1);
}

// ── Playback failure does not change the submit outcome ─────────────────────

#[tokio::test]
async fn playback_failure_leaves_the_summary_unchanged() {
    let mut ok = fixture(
        "[sound]\non_save = true",
        Arc::new(CountingPlayer::default()),
    );
    let mut failing = fixture("[sound]\non_save = true", Arc::new(FailingPlayer));

    submit(&mut ok, "note", &[("title", "a")]).await;
    submit(&mut failing, "note", &[("title", "a")]).await;

    assert!(
        saved(&failing.app),
        "a failed tone must not fail the capture"
    );
    let ok_summary = ok.app.summary_state.as_ref().unwrap();
    let failing_summary = failing.app.summary_state.as_ref().unwrap();
    assert_eq!(failing_summary.message, ok_summary.message);
    assert_eq!(failing_summary.message, "Entry saved successfully.");
    assert_eq!(failing_summary.transport_mode, ok_summary.transport_mode);

    let written = failing_summary.file_path.as_ref().unwrap();
    assert!(
        failing.dir.path().join(written).exists(),
        "the note must be on disk"
    );
}

#[tokio::test]
async fn playback_failure_surfaces_as_a_status_toast() {
    let mut f = fixture("[sound]\non_save = true", Arc::new(FailingPlayer));

    submit(&mut f, "note", &[("title", "a")]).await;
    assert!(
        f.app.status_message.is_none(),
        "submit itself raises no toast"
    );

    // The event loop calls this every tick.
    f.app.tick_status();
    let toast = f.app.status_message.as_ref().expect("toast raised");
    assert_eq!(toast.text, "sound: no audio output device");
    assert!(!toast.text.contains('\n'), "toast is one line");
}

/// Expire whatever toast is up, as if its display time had run out.
fn expire_toast(app: &mut App) {
    let toast = app.status_message.as_mut().expect("a toast to expire");
    toast.expires_at = Instant::now();
}

#[tokio::test]
async fn sound_failure_waits_for_a_live_toast() {
    let mut f = fixture("[sound]\non_save = true", Arc::new(FailingPlayer));
    // Point the cache under a regular file, so the submit's own
    // `cache.save()` fails and raises its warning in the same submit that
    // plays the tone.
    let blocker = f.dir.path().join("blocker");
    std::fs::write(&blocker, "").unwrap();
    f.cache = Cache::load_from(blocker.join("cache.json"));

    submit(&mut f, "note", &[("title", "a")]).await;
    assert!(saved(&f.app), "the capture itself must succeed");
    let first = f.app.status_message.as_ref().expect("cache toast raised");
    assert!(
        first.text.starts_with("cache.save failed"),
        "{}",
        first.text
    );

    // The sound failure is already queued. The cache toast must survive it.
    f.app.tick_status();
    let still = f.app.status_message.as_ref().expect("cache toast kept");
    assert!(
        still.text.starts_with("cache.save failed"),
        "{}",
        still.text
    );

    // Once the cache toast clears, the sound toast takes the slot.
    expire_toast(&mut f.app);
    f.app.tick_status();
    let sound = f.app.status_message.as_ref().expect("sound toast raised");
    assert_eq!(sound.text, "sound: no audio output device");
}

#[tokio::test]
async fn sound_failure_toasts_once_per_session() {
    let mut f = fixture("[sound]\non_save = true", Arc::new(FailingPlayer));
    seed_today(&f.dir);

    submit(&mut f, "note", &[("title", "a")]).await;
    f.app.tick_status();
    assert_eq!(
        f.app.status_message.as_ref().map(|t| t.text.as_str()),
        Some("sound: no audio output device")
    );
    expire_toast(&mut f.app);

    // Two more saves, two more failures: neither raises a toast.
    submit(&mut f, "log", &[("body", "b")]).await;
    assert!(saved(&f.app));
    f.app.tick_status();
    submit(&mut f, "habit", &[("meditated", "true")]).await;
    assert!(saved(&f.app));
    f.app.tick_status();

    assert!(
        f.app.status_message.is_none(),
        "a later failure raised a toast: {:?}",
        f.app.status_message
    );
}
