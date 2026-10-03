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
//! - **Never blocks the TUI.** [`DevicePlayer`] hands each tone to one
//!   long-lived audio thread, which opens the device, plays, and waits out the
//!   tone. The summary screen renders and takes keys while the tone is still
//!   sounding. Quitting mid-tone cuts it off, which is accepted.
//! - **One audio thread per process.** Started on the first tone, then kept
//!   until the process exits. On Windows, cpal caches its device enumerator
//!   process-wide inside the COM apartment of whichever thread asked first;
//!   if that thread exits, the next tone reads a dead pointer and the process
//!   dies with an access violation (cpal#1302). The same thread also
//!   serializes tones: a save during a tone queues behind it instead of
//!   opening a second stream.
//! - **Never writes to the terminal.** The TUI owns a raw-mode terminal (see
//!   `hooks::run` for the same rule applied to child processes). A failure is
//!   a one-line message on the [`Chime`] channel, which the event loop turns
//!   into a status toast. On Linux, alsa-lib's own stderr diagnostics go to a
//!   per-thread buffer instead. That covers alsa-lib's messages on pour's
//!   audio thread, where cpal opens the PCM. On cpal's `cpal_alsa_out` thread
//!   it covers only what happens from the first data callback onward. Plugins
//!   that log through their own logger (PipeWire's `pw_log`) bypass the
//!   handler entirely and can reach stderr from either thread.
//! - **Never changes the capture.** The note is already written when the tone
//!   starts, and nothing here reports back into the summary.

use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
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
    /// Whether [`Chime::take_failure`] has already handed out a failure.
    reported: bool,
}

impl Chime {
    /// A chime that plays through `player`.
    pub fn new(player: Arc<dyn Player>) -> Self {
        let (failures_tx, failures_rx) = mpsc::channel();
        Chime {
            player,
            failures_tx,
            failures_rx,
            reported: false,
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

    /// The first playback failure this chime has seen, handed out once.
    ///
    /// After that, every later failure is drained and dropped, so a session
    /// raises at most one sound toast: a machine with no device (an SSH
    /// session, say) fails the same way on every save, and the repeat says
    /// nothing new.
    pub fn take_failure(&mut self) -> Option<String> {
        if self.reported {
            while self.failures_rx.try_recv().is_ok() {}
            return None;
        }
        let failure = self.failures_rx.try_recv().ok();
        self.reported = failure.is_some();
        failure
    }
}

/// Plays the tone on the system's default output device through `cpal`.
///
/// Every `DevicePlayer` shares one process-wide [`SoundThread`], because the
/// state that thread protects (cpal's cached WASAPI enumerator) is
/// process-wide too. A second `DevicePlayer` must not get a second thread.
pub struct DevicePlayer;

/// The audio thread behind every [`DevicePlayer`]. Never dropped, so once its
/// thread starts it runs until the process exits.
static DEVICE_THREAD: LazyLock<SoundThread> =
    LazyLock::new(|| SoundThread::new(play_on_default_device));

impl Player for DevicePlayer {
    fn start(&self, failures: Sender<String>) {
        DEVICE_THREAD.start(failures);
    }
}

/// A [`Player`] that runs every tone on one thread of its own, one at a time.
///
/// The thread is spawned by the first [`Player::start`], not by
/// [`SoundThread::new`], and then waits for requests until the `SoundThread`
/// is dropped. Each `start` queues one call to `play`; calls run in order,
/// each after the previous one returns, and none is skipped.
///
/// If the thread dies (a panic inside `play`), later starts report
/// `sound thread stopped` rather than spawning a replacement. On Windows a
/// replacement is exactly what cpal#1302 crashes on.
pub struct SoundThread {
    play: Arc<dyn Fn() -> Result<(), String> + Send + Sync>,
    /// Request queue into the thread; `None` until the first `start`. Each
    /// request is the channel that request's failure goes back on.
    queue: Mutex<Option<Sender<Sender<String>>>>,
}

impl SoundThread {
    /// A player that runs `play` once per tone on its own thread. Spawns
    /// nothing until the first `start`.
    pub fn new(play: impl Fn() -> Result<(), String> + Send + Sync + 'static) -> Self {
        SoundThread {
            play: Arc::new(play),
            queue: Mutex::new(None),
        }
    }

    fn spawn(&self) -> std::io::Result<Sender<Sender<String>>> {
        let (tx, rx) = mpsc::channel::<Sender<String>>();
        let play = Arc::clone(&self.play);
        // `Builder::spawn`, not `thread::spawn`: the latter panics when the OS
        // refuses a thread, and a panic here would tear down the TUI.
        std::thread::Builder::new()
            .name("pour-sound".to_string())
            .spawn(move || {
                for failures in rx {
                    if let Err(e) = play() {
                        let _ = failures.send(e);
                    }
                }
            })?;
        Ok(tx)
    }
}

impl Player for SoundThread {
    fn start(&self, failures: Sender<String>) {
        // Nothing panics while holding this lock, but if something ever did,
        // the `Option` inside is still coherent; don't turn that into a
        // second panic on the TUI thread.
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if queue.is_none() {
            match self.spawn() {
                Ok(tx) => *queue = Some(tx),
                Err(e) => {
                    // Left `None`, so the next tone tries again.
                    let _ = failures.send(format!("could not start sound thread: {e}"));
                    return;
                }
            }
        }
        if let Some(tx) = queue.as_ref()
            && let Err(SendError(failures)) = tx.send(failures)
        {
            let _ = failures.send("sound thread stopped".to_string());
        }
    }
}

/// Open the default output device, play the tone, and hold the stream open
/// until it has drained. Blocks the calling thread for the length of the tone.
/// Runs only on [`DEVICE_THREAD`].
fn play_on_default_device() -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    // Once per tone, not once per thread: a fresh install also drops the
    // previous buffer, so diagnostics don't pile up over a long session.
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
        // cpal's ALSA host ranks these four above I16, so a device that
        // offers any of them gets it as its default.
        cpal::SampleFormat::I24 => build_stream::<cpal::I24>(&device, config, samples, channels),
        cpal::SampleFormat::U24 => build_stream::<cpal::U24>(&device, config, samples, channels),
        cpal::SampleFormat::U32 => build_stream::<u32>(&device, config, samples, channels),
        cpal::SampleFormat::F64 => build_stream::<f64>(&device, config, samples, channels),
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
            // first use. Anything alsa-lib prints on this thread before the
            // first callback is not covered.
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
/// terminal the TUI is drawing on. The handler is thread-local. It is installed
/// on pour's audio thread before each tone, which covers every alsa-lib call
/// made on that thread. It is also installed inside the stream callback, so on
/// cpal's `cpal_alsa_out` thread it covers only the first callback onward,
/// not the alsa-lib calls cpal makes on that thread before it. It never covers
/// a plugin that logs through its own logger, such as PipeWire's `pw_log`.
/// Failing to install it is not worth reporting; the tone still plays.
#[cfg(target_os = "linux")]
fn quiet_alsa_on_this_thread() {
    let _ = alsa::Output::local_error_handler();
}

#[cfg(not(target_os = "linux"))]
fn quiet_alsa_on_this_thread() {}
