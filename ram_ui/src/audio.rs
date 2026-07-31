//! Rain ambience, synthesised rather than sampled.
//!
//! There is no audio asset to ship: the sound is filtered noise generated in
//! real time. That keeps the binary small, sidesteps licensing entirely, and
//! means the loop can never audibly repeat.
//!
//! ## Why a thread
//!
//! `rodio::OutputStream` owns a `cpal` stream and is neither `Send` nor safe to
//! drop casually, and opening an audio device can block for a noticeable
//! moment. Both problems disappear if the stream lives on its own thread and
//! the UI only holds a channel sender: the UI thread never blocks, and
//! `AppState` stays free of `!Send` fields.
//!
//! Failure is always silent-but-logged. A machine with no sound device, or one
//! where exclusive-mode audio is held by something else, must not stop the
//! account manager from working.

use rodio::Source;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

enum Cmd {
    Enabled(bool),
    Volume(f32),
    /// Swap the playing source: `None` = built-in synth, `Some` = a file.
    Source(Option<PathBuf>),
}

/// Map the 0..=1 slider to an amplitude multiplier.
///
/// Loudness is perceived roughly logarithmically, so a linear slider spends
/// almost its whole travel in "far too loud" and gives no usable control at
/// the quiet end — which is exactly the range ambience lives in. Cubing it
/// puts the useful adjustment where the hand actually is.
fn perceptual_gain(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v * v
}

/// Handle to the rain audio thread. Cheap to construct; the thread and the
/// audio device are only touched the first time rain is actually switched on.
#[derive(Default)]
pub struct RainAudio {
    tx: Option<Sender<Cmd>>,
    /// Last values we sent, so we only message the thread on real changes.
    sent_enabled: bool,
    /// `None` = nothing sent yet. Deliberately not a sentinel float: `NaN`
    /// compares false against everything, so a `NaN` sentinel silently
    /// suppresses the first volume message and the sink stays muted forever.
    sent_volume: Option<f32>,
    sent_source: Option<Option<PathBuf>>,
    /// Set when the device could not be opened, to avoid retry storms.
    dead: bool,
}

impl RainAudio {
    /// Reconcile the audio thread with the current settings. Call every frame;
    /// it is a no-op unless something changed.
    pub fn update(&mut self, enabled: bool, volume: f32, source: Option<&std::path::Path>) {
        if self.dead {
            return;
        }
        // Don't open an audio device until the user actually wants sound.
        if self.tx.is_none() {
            if !enabled {
                return;
            }
            match spawn() {
                Some(tx) => self.tx = Some(tx),
                None => {
                    self.dead = true;
                    return;
                }
            }
            // Force both settings through on the first connection.
            self.sent_enabled = !enabled;
            self.sent_volume = None;
            self.sent_source = None;
        }

        let tx = self.tx.as_ref().expect("just ensured Some");

        let want_source = source.map(|p| p.to_path_buf());
        if self.sent_source.as_ref() != Some(&want_source) {
            if tx.send(Cmd::Source(want_source.clone())).is_err() {
                self.dead = true;
                return;
            }
            self.sent_source = Some(want_source);
            // A new sink starts silent; re-send the level.
            self.sent_volume = None;
        }

        let volume_changed = match self.sent_volume {
            Some(sent) => (volume - sent).abs() > f32::EPSILON,
            None => true,
        };
        if volume_changed {
            if tx.send(Cmd::Volume(volume)).is_err() {
                self.dead = true;
                return;
            }
            self.sent_volume = Some(volume);
        }
        if enabled != self.sent_enabled {
            if tx.send(Cmd::Enabled(enabled)).is_err() {
                self.dead = true;
                return;
            }
            self.sent_enabled = enabled;
        }
    }
}

fn spawn() -> Option<Sender<Cmd>> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    let spawned = std::thread::Builder::new()
        .name("rm-rain-audio".to_string())
        .spawn(move || {
            // `_stream` must outlive the sink — dropping it kills playback.
            let (_stream, handle) = match rodio::OutputStream::try_default() {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("Rain audio disabled — no output device: {e}");
                    return;
                }
            };
            let sink = match rodio::Sink::try_new(&handle) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Rain audio disabled — could not create sink: {e}");
                    return;
                }
            };

            let mut sink = sink;
            let mut gain = 0.0f32;
            let mut playing = false;

            sink.set_volume(0.0);
            sink.append(RainNoise::new(48_000));
            sink.pause();

            // Ends when the sender is dropped, i.e. when the app exits.
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Cmd::Enabled(v) => {
                        playing = v;
                        if v {
                            sink.play();
                        } else {
                            sink.pause();
                        }
                    }
                    Cmd::Volume(v) => {
                        gain = perceptual_gain(v);
                        sink.set_volume(gain);
                    }
                    Cmd::Source(path) => {
                        // Rebuilding the sink is the simplest way to guarantee
                        // the old source is gone rather than queued behind.
                        sink.stop();
                        sink = match rodio::Sink::try_new(&handle) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("Could not rebuild audio sink: {e}");
                                return;
                            }
                        };
                        sink.set_volume(gain);
                        match path {
                            Some(p) => match load_looped(&p) {
                                Ok(src) => sink.append(src),
                                Err(e) => {
                                    tracing::warn!(
                                        "Rain sound file {} unusable ({e}); using the built-in synth",
                                        p.display()
                                    );
                                    sink.append(RainNoise::new(48_000));
                                }
                            },
                            None => sink.append(RainNoise::new(48_000)),
                        }
                        if playing {
                            sink.play();
                        } else {
                            sink.pause();
                        }
                    }
                }
            }
        });

    match spawned {
        Ok(_) => Some(tx),
        Err(e) => {
            tracing::warn!("Could not start rain audio thread: {e}");
            None
        }
    }
}

/// Decode a user-supplied audio file into an endlessly looping source.
fn load_looped(
    path: &std::path::Path,
) -> Result<rodio::decoder::LoopedDecoder<std::io::BufReader<std::fs::File>>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    rodio::Decoder::new_looped(std::io::BufReader::new(file)).map_err(|e| e.to_string())
}

/// Infinite rain generator.
///
/// Three layers, because plain white noise sounds like a broken radio rather
/// than weather:
/// 1. a two-pole lowpass over white noise — the low roar of heavy rainfall
/// 2. a highpassed copy — the hiss of individual droplets
/// 3. a very slow sine gust envelope so the intensity breathes
struct RainNoise {
    rng: u32,
    lp1: f32,
    lp2: f32,
    hp: f32,
    phase: f32,
    sample_rate: u32,
}

impl RainNoise {
    fn new(sample_rate: u32) -> Self {
        Self {
            rng: 0x1234_5678,
            lp1: 0.0,
            lp2: 0.0,
            hp: 0.0,
            phase: 0.0,
            sample_rate,
        }
    }

    /// xorshift32 mapped to -1.0..=1.0.
    fn white(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Iterator for RainNoise {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let w = self.white();

        // Body: cascaded one-pole lowpass. A lower corner than before — the
        // old setting left too much upper-mid energy, which is what made this
        // read as harsh static rather than distant rainfall.
        self.lp1 += 0.08 * (w - self.lp1);
        self.lp2 += 0.08 * (self.lp1 - self.lp2);
        let body = self.lp2 * 2.4;

        // Droplets: whatever the lowpass rejected, well under the body. This
        // is the harsh component, so it stays quiet.
        self.hp += 0.50 * (w - self.hp);
        let sparkle = (w - self.hp) * 0.10;

        // Gusts: ~14 second period.
        self.phase += std::f32::consts::TAU * 0.07 / self.sample_rate as f32;
        if self.phase > std::f32::consts::TAU {
            self.phase -= std::f32::consts::TAU;
        }
        let gust = 0.80 + 0.20 * self.phase.sin();

        // Peak lands near 0.3 rather than clipping against full scale. Room
        // ambience wants roughly -25 dBFS, and the sink's own gain scales this
        // down further, so generating anything near 1.0 here was the bug.
        Some(((body + sparkle) * gust).clamp(-1.0, 1.0) * 0.30)
    }
}

impl Source for RainNoise {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
