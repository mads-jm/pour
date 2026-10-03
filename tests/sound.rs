//! The completion tone's synthesis and the `Chime` plumbing.
//!
//! Nothing here opens an audio device: synthesis is pure, every `Chime` is
//! built over a fake `Player`, and every `SoundThread` runs a fake tone.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

use pour::sound::{Chime, Player, SoundThread, TONE_DURATION, synthesize};

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

#[test]
fn tone_lasts_under_a_second_at_common_rates() {
    for rate in [22_050u32, 44_100, 48_000, 96_000] {
        let samples = synthesize(rate);
        let seconds = samples.len() as f32 / rate as f32;
        assert!(seconds > 0.1, "{rate} Hz: tone too short ({seconds}s)");
        assert!(
            seconds < 1.0,
            "{rate} Hz: tone must stay under 1s ({seconds}s)"
        );
        assert!(
            (seconds - TONE_DURATION.as_secs_f32()).abs() < 0.001,
            "{rate} Hz: length should match TONE_DURATION"
        );
    }
}

#[test]
fn tone_is_finite_and_within_full_scale() {
    let samples = synthesize(48_000);
    assert!(samples.iter().all(|s| s.is_finite()));
    let p = peak(&samples);
    assert!(p > 0.05, "tone should be audible, peak {p}");
    assert!(p <= 1.0, "tone must not clip, peak {p}");
}

#[test]
fn tone_ends_on_exact_silence() {
    let samples = synthesize(48_000);
    assert_eq!(*samples.last().unwrap(), 0.0, "last sample must be silent");
}

#[test]
fn tone_starts_quietly_instead_of_clicking() {
    let samples = synthesize(48_000);
    // First millisecond is inside the 5 ms attack ramp.
    let first_ms = &samples[..48];
    assert!(
        peak(first_ms) < peak(&samples) / 2.0,
        "attack should ramp in, first-ms peak {}",
        peak(first_ms)
    );
}

#[test]
fn tone_decays_like_a_struck_note() {
    let samples = synthesize(48_000);
    let window = 48_000 / 20; // 50 ms
    let head = peak(&samples[..window]);
    let tail = peak(&samples[samples.len() - window..]);
    assert!(
        tail < head * 0.05,
        "tail ({tail}) should be well under 5% of the head ({head})"
    );
}

#[test]
fn tone_is_deterministic() {
    assert_eq!(synthesize(44_100), synthesize(44_100));
}

/// Counts `start` calls and plays nothing.
#[derive(Default)]
struct CountingPlayer {
    starts: AtomicUsize,
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

#[test]
fn chime_starts_its_player_once_per_play() {
    let player = Arc::new(CountingPlayer::default());
    let chime = Chime::new(player.clone());
    assert_eq!(player.starts.load(Ordering::SeqCst), 0);
    chime.play();
    assert_eq!(player.starts.load(Ordering::SeqCst), 1);
    chime.play();
    assert_eq!(player.starts.load(Ordering::SeqCst), 2);
}

#[test]
fn chime_without_failures_has_nothing_to_report() {
    let mut chime = Chime::new(Arc::new(CountingPlayer::default()));
    chime.play();
    assert_eq!(chime.take_failure(), None);
}

#[test]
fn chime_hands_back_a_failure_once() {
    let mut chime = Chime::new(Arc::new(FailingPlayer));
    chime.play();
    assert_eq!(
        chime.take_failure().as_deref(),
        Some("no audio output device")
    );
    assert_eq!(chime.take_failure(), None);
}

#[test]
fn chime_reports_only_its_first_failure() {
    let mut chime = Chime::new(Arc::new(FailingPlayer));
    chime.play();
    chime.play();
    assert!(chime.take_failure().is_some(), "first failure is reported");
    assert_eq!(chime.take_failure(), None, "queued repeat is dropped");

    chime.play();
    assert_eq!(chime.take_failure(), None, "later failure is dropped");
}

#[test]
fn chime_holds_a_failure_until_it_is_taken() {
    // The first failure must not be spent by plays that happen before
    // anyone asks for it: `tick_status` leaves it queued while another toast
    // is up.
    let mut chime = Chime::new(Arc::new(FailingPlayer));
    chime.play();
    chime.play();
    chime.play();
    assert_eq!(
        chime.take_failure().as_deref(),
        Some("no audio output device")
    );
}

// ── SoundThread: one long-lived thread, tones in order ─────────────────────

/// How long a test waits for the sound thread before calling it hung.
const WAIT: Duration = Duration::from_secs(5);

#[test]
fn sound_thread_plays_every_tone_on_one_thread() {
    let (done_tx, done_rx) = mpsc::channel::<ThreadId>();
    let player = SoundThread::new(move || {
        let _ = done_tx.send(thread::current().id());
        Ok(())
    });
    let (failures, _failures_rx) = mpsc::channel();

    for _ in 0..3 {
        player.start(failures.clone());
    }

    let ids: Vec<ThreadId> = (0..3)
        .map(|_| done_rx.recv_timeout(WAIT).unwrap())
        .collect();
    assert_ne!(ids[0], thread::current().id(), "plays off the caller");
    assert!(
        ids.iter().all(|id| *id == ids[0]),
        "every tone on the same thread: {ids:?}"
    );

    // A tone requested after the queue drained still lands on that thread.
    player.start(failures.clone());
    assert_eq!(done_rx.recv_timeout(WAIT).unwrap(), ids[0]);
}

#[test]
fn sound_thread_queues_tones_without_overlap_or_skips() {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let most_at_once = Arc::new(AtomicUsize::new(0));
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let player = {
        let in_flight = Arc::clone(&in_flight);
        let most_at_once = Arc::clone(&most_at_once);
        SoundThread::new(move || {
            let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            most_at_once.fetch_max(now, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(30));
            in_flight.fetch_sub(1, Ordering::SeqCst);
            let _ = done_tx.send(());
            Ok(())
        })
    };
    let (failures, _failures_rx) = mpsc::channel();

    // Five saves in quick succession, each landing while a tone is sounding.
    for _ in 0..5 {
        player.start(failures.clone());
    }

    for n in 1..=5 {
        done_rx
            .recv_timeout(WAIT)
            .unwrap_or_else(|_| panic!("tone {n} of 5 never played"));
    }
    assert_eq!(most_at_once.load(Ordering::SeqCst), 1, "tones overlapped");
}

#[test]
fn sound_thread_start_returns_while_the_tone_is_still_playing() {
    // The tone blocks until the test releases it. If `start` waited for the
    // tone, the release would never be sent and `recv_timeout` would fail.
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let player = SoundThread::new(move || {
        let _ = release_rx.lock().unwrap().recv_timeout(WAIT);
        let _ = done_tx.send(());
        Ok(())
    });
    let (failures, _failures_rx) = mpsc::channel();

    player.start(failures);
    assert!(
        done_rx.try_recv().is_err(),
        "tone still sounding after start returned"
    );
    release_tx.send(()).unwrap();
    done_rx
        .recv_timeout(WAIT)
        .expect("tone finishes once released");
}

#[test]
fn sound_thread_sends_a_failed_tone_back_as_one_line() {
    let player = SoundThread::new(|| Err("no audio output device".to_string()));
    let (failures, failures_rx) = mpsc::channel();

    player.start(failures);

    assert_eq!(
        failures_rx.recv_timeout(WAIT).as_deref(),
        Ok("no audio output device")
    );
}
