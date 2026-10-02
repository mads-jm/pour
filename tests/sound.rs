//! The completion tone's synthesis and the `Chime` plumbing.
//!
//! Nothing here opens an audio device: synthesis is pure, and every `Chime`
//! is built over a fake `Player`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

use pour::sound::{Chime, Player, TONE_DURATION, synthesize};

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
    let chime = Chime::new(Arc::new(CountingPlayer::default()));
    chime.play();
    assert_eq!(chime.take_failure(), None);
}

#[test]
fn chime_hands_back_each_failure_once() {
    let chime = Chime::new(Arc::new(FailingPlayer));
    chime.play();
    assert_eq!(
        chime.take_failure().as_deref(),
        Some("no audio output device")
    );
    assert_eq!(chime.take_failure(), None);
}
