//! The completion sound: one short synthesized tone when a TUI capture saves.
//!
//! Off unless `[sound] on_save = true`. The gate lives at the call site in
//! `tui::loop_::handle_submit`; nothing in this module touches an audio device
//! until [`Chime::play`] is called, so a config without the key never opens one.
//!
//! Every audio call in pour goes through this file. Backing the `cpal`
//! dependency out, or replacing the tone with a richer voice, is a change here
//! and nowhere else.
//!
//! # Rules this module keeps
//!
//! - **Never blocks the TUI.** [`DevicePlayer`] opens the device, plays, and
//!   waits out the tone on its own thread. The summary screen renders and takes
//!   keys while the tone is still sounding. Quitting mid-tone cuts it off,
//!   which is accepted.
//! - **Never writes to the terminal.** The TUI owns a raw-mode terminal (see
//!   `hooks::run` for the same rule applied to child processes). A failure is
//!   a one-line message on the [`Chime`] channel, which the event loop turns
//!   into a status toast. On ALSA, the library's own stderr diagnostics are
//!   routed into a buffer on both threads that call into it.
//! - **Never changes the capture.** The note is already written when the tone
//!   starts, and nothing here reports back into the summary.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

/// How long the synthesized tone lasts, attack to silence.
pub const TONE_DURATION: Duration = Duration::from_millis(500);

/// Pitch of the tone: E5. High enough to read as a struck bar, low enough not
/// to be shrill on laptop speakers.
const TONE_HZ: f32 = 659.25;

/// Peak amplitude, as a fraction of full scale. Quiet on purpose: the tone
/// confirms a capture, it should not startle.
const PEAK: f32 = 0.25;

/// Linear fade-in. Starting a sine at full amplitude clicks.
const ATTACK: Duration = Duration::from_millis(5);

/// Time constant of the exponential decay. After 500 ms the envelope is down
/// to about e^-4, roughly -36 dB, before the release brings it to zero.
const DECAY_TAU_SECS: f32 = 0.12;

/// Linear fade-out over the last stretch, so the final sample is exactly zero
/// and the tone ends instead of cutting off.
const RELEASE: Duration = Duration::from_millis(30);

/// Extra time the playback thread keeps the stream open after the last sample
/// is handed to the device, so its buffer can drain before the stream drops.
const DRAIN_MARGIN: Duration = Duration::from_millis(300);

/// Synthesize the tone as mono `f32` samples at `sample_rate`.
///
/// A sine at E5 (659.25 Hz) under a struck-note envelope: a 5 ms linear attack, an
/// exponential decay, and a 30 ms linear release that lands on exactly zero.
/// Pure and deterministic, so it is tested directly.
pub fn synthesize(sample_rate: u32) -> Vec<f32> {
    let rate = sample_rate as f32;
    let total = (TONE_DURATION.as_secs_f32() * rate).round() as usize;
    let attack = (ATTACK.as_secs_f32() * rate).max(1.0);
    let release = (RELEASE.as_secs_f32() * rate).max(1.0);

    (0..total)
        .map(|i| {
            let t = i as f32 / rate;
            let remaining = (total - 1 - i) as f32;
            let envelope = (i as f32 / attack).min(1.0)
                * (-t / DECAY_TAU_SECS).exp()
                * (remaining / release).min(1.0);
            PEAK * envelope * (std::f32::consts::TAU * TONE_HZ * t).sin()
        })
        .collect()
}

/// Something that can play the tone.
///
/// [`DevicePlayer`] is the real one. Tests substitute fakes so that nothing
/// audible plays under `cargo test` and so the call can be observed.
pub trait Player: Send + Sync {
    /// Start the tone and return without waiting for it to finish.
    ///
    /// A failure, whether it happens before this returns or later on another
    /// thread, is sent on `failures` as one line of text. Implementations must
    /// not print and must not panic.
    fn start(&self, failures: Sender<String>);
}

/// The TUI's handle on the completion sound: a [`Player`] plus the channel its
/// failures come back on.
pub struct Chime {
    player: Arc<dyn Player>,
    failures_tx: Sender<String>,
    failures_rx: Receiver<String>,
}

impl Chime {
    /// A chime that plays through `player`.
    pub fn new(player: Arc<dyn Player>) -> Self {
        let (failures_tx, failures_rx) = mpsc::channel();
        Chime {
            player,
            failures_tx,
            failures_rx,
        }
    }

    /// A chime that plays on the default audio output device. Constructing it
    /// touches no device; only [`Chime::play`] does.
    pub fn device() -> Self {
        Self::new(Arc::new(DevicePlayer))
    }

    /// Start the tone. Returns immediately.
    pub fn play(&self) {
        self.player.start(self.failures_tx.clone());
    }

    /// The oldest playback failure not yet collected, if any.
    pub fn take_failure(&self) -> Option<String> {
        self.failures_rx.try_recv().ok()
    }
}

/// Plays the tone on the system's default output device through `cpal`.
pub struct DevicePlayer;

impl Player for DevicePlayer {
    fn start(&self, failures: Sender<String>) {
        let from_thread = failures.clone();
        // `Builder::spawn`, not `thread::spawn`: the latter panics when the OS
        // refuses a thread, and a panic here would tear down the TUI.
        let spawned = std::thread::Builder::new()
            .name("pour-sound".to_string())
            .spawn(move || {
                if let Err(e) = play_on_default_device() {
                    let _ = from_thread.send(e);
                }
            });
        if let Err(e) = spawned {
            let _ = failures.send(format!("could not start sound thread: {e}"));
        }
    }
}

/// Open the default output device, play the tone, and hold the stream open
/// until it has drained. Blocks the calling thread for the length of the tone.
fn play_on_default_device() -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    quiet_alsa_on_this_thread();

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("audio device unavailable: {e}"))?;
    let config = supported.config();
    let samples = Arc::new(synthesize(config.sample_rate));
    let channels = usize::from(config.channels).max(1);

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, config, samples, channels),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, config, samples, channels),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, config, samples, channels),
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, config, samples, channels),
        other => return Err(format!("unsupported audio sample format {other}")),
    }
    .map_err(|e| format!("could not open audio stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("could not start audio: {e}"))?;
    std::thread::sleep(TONE_DURATION + DRAIN_MARGIN);
    Ok(())
}

/// Build an output stream that writes `samples` to every channel of each
/// frame, then silence once they run out.
fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    samples: Arc<Vec<f32>>,
    channels: usize,
) -> Result<cpal::Stream, cpal::Error>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;

    let mut next = 0usize;
    let mut quieted = false;
    device.build_output_stream::<T, _, _>(
        config,
        move |out: &mut [T], _| {
            // The callback runs on cpal's own audio thread, which alsa-lib
            // will print from if it fails there. Install the quiet handler on
            // first use.
            if !quieted {
                quiet_alsa_on_this_thread();
                quieted = true;
            }
            for frame in out.chunks_mut(channels) {
                let value = T::from_sample(samples.get(next).copied().unwrap_or(0.0));
                frame.fill(value);
                next = next.saturating_add(1);
            }
        },
        // A mid-tone stream error has no one to report to that would act on
        // it: the tone is cosmetic and already under way.
        |_err| {},
        Some(Duration::from_secs(2)),
    )
}

/// Route alsa-lib's diagnostics on the current thread into an in-memory
/// buffer instead of stderr.
///
/// alsa-lib prints to stderr by default (`ALSA lib confmisc.c: cannot find
/// card '0'` and the like on a machine with no sound card), and stderr is the
/// terminal the TUI is drawing on. The handler is thread-local, which is why it
/// is installed both on the playback thread (device open and close) and inside
/// the stream callback (cpal's audio thread). Failing to install it is not
/// worth reporting; the tone still plays.
#[cfg(target_os = "linux")]
fn quiet_alsa_on_this_thread() {
    let _ = alsa::Output::local_error_handler();
}

#[cfg(not(target_os = "linux"))]
fn quiet_alsa_on_this_thread() {}
