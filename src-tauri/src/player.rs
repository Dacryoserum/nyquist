//! Native audio playback, fed from the samples the analysis already decoded.
//!
//! ## Why not the webview's `<audio>` element
//!
//! Because it kept a second, disagreeing copy of the truth. The element parses the file
//! itself and forms its own opinion of how long it is, while every other part of this app —
//! the scrubber's range, the spectrogram's time axis, the readout — works from the length the
//! decoder measured. Three separate symptoms came out of that one split:
//!
//! - **Seeking landed in the wrong place.** A click on the spectrogram is a fraction of *our*
//!   duration, handed to the element as `currentTime`, which the element interprets against
//!   *its* duration and clamps to *its* idea of what is seekable.
//! - **The counter drifted.** The scrubber's maximum came from the decoder, its position from
//!   the element.
//! - **Long tracks stopped early.** WebKit decided the file ended where the audio it had
//!   happened to buffer stopped, and revised its own duration down to match — which is also
//!   what made the other two get worse as a track went on.
//!
//! Serving the file over loopback HTTP fixed the delivery ceiling that started this, and the
//! module that did it is gone now: it could not fix the disagreement, because the
//! disagreement was never about transport. Two clocks cannot be synchronised by improving the
//! courier between them. There is now one clock — a sample index into the decoded track —
//! and every displayed number is derived from it.
//!
//! ## What this costs
//!
//! The decoded track stays in memory for as long as it is loaded, where before it was dropped
//! once the analysis finished. That is roughly 10 MB per stereo minute at 44.1 kHz, and four
//! times that at 96 kHz. It buys sample-exact seeking, a position that cannot drift, and the
//! removal of an entire hand-written HTTP server from the attack surface.
//!
//! Deliberately *not* re-decoding on demand: the file is already decoded, and decoding it a
//! second time to save memory would reintroduce the two-sources-of-truth problem in a new
//! place.

use std::num::NonZero;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::Source;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, SampleRate};
use serde::Serialize;

use crate::decode::DecodedAudio;

/// What the frontend needs to draw the transport, in one round trip.
///
/// Polled rather than pushed: at the rate a playhead needs (10-20 Hz) an event stream would
/// cost more than it saves, and a poll cannot fall behind the way a dropped event can.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PlaybackState {
    /// Seconds from the start of the track. Derived from the sample index actually handed to
    /// the audio device, so it cannot disagree with what is being heard.
    pub position_seconds: f64,
    /// Length of the loaded track, from the decoder. The same number the scrubber, the
    /// spectrogram and the report all use.
    pub duration_seconds: f64,
    pub playing: bool,
    /// The playhead has reached the end of the track.
    pub ended: bool,
    /// No track is loaded, so the other fields describe nothing.
    pub loaded: bool,
    /// Why nothing is loaded, when something was tried and failed — no audio output device,
    /// a file with no playable frames. `None` when playback simply has not been asked for.
    ///
    /// Carried so the UI can say what went wrong instead of showing a bare "unavailable":
    /// the analysis is a separate outcome and stays on screen either way, but a reason the
    /// user can act on is the difference between a limitation and a mystery.
    pub unavailable_reason: Option<String>,
}

impl PlaybackState {
    fn idle(unavailable_reason: Option<String>) -> Self {
        Self {
            position_seconds: 0.0,
            duration_seconds: 0.0,
            playing: false,
            ended: false,
            loaded: false,
            unavailable_reason,
        }
    }
}

/// One decoded track, and the audio device playing it.
struct Loaded {
    /// Holds the output device open. Dropping it silences everything, so it is kept here
    /// rather than being allowed to fall out of scope after setup.
    _device: MixerDeviceSink,
    player: rodio::Player,
    /// Interleaved sample index, shared with the source feeding the device. The single
    /// source of truth for "where are we".
    cursor: Arc<AtomicUsize>,
    /// Frames x channels. The end of the track in the units `cursor` counts.
    total_samples: usize,
    channels: usize,
    sample_rate: u32,
}

impl Loaded {
    fn position_seconds(&self) -> f64 {
        let frames = self.cursor.load(Ordering::Relaxed) / self.channels;
        frames as f64 / self.sample_rate as f64
    }

    fn duration_seconds(&self) -> f64 {
        (self.total_samples / self.channels) as f64 / self.sample_rate as f64
    }

    fn ended(&self) -> bool {
        self.cursor.load(Ordering::Relaxed) >= self.total_samples
    }
}

/// Playback state held for the lifetime of the app.
pub struct Player {
    loaded: Mutex<Option<Loaded>>,
    /// Kept across loads, because a user who turned the volume down means it.
    volume: Mutex<f32>,
    /// Why the last load failed, if it did. Cleared by a load that succeeds.
    last_error: Mutex<Option<String>>,
}

/// Written out rather than derived: a derived `Default` would start the volume at 0.0, and a
/// player that is silent until someone touches a slider is a bug that looks like a broken
/// audio device.
impl Default for Player {
    fn default() -> Self {
        Self {
            loaded: Mutex::new(None),
            volume: Mutex::new(1.0),
            last_error: Mutex::new(None),
        }
    }
}

impl Player {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes ownership of a decoded track and opens the audio device for it.
    ///
    /// Starts paused: analysing a file is not a request to hear it.
    pub fn load(&self, decoded: DecodedAudio) -> Result<(), String> {
        let outcome = self.try_load(decoded);
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = outcome.as_ref().err().cloned();
        }
        outcome
    }

    fn try_load(&self, decoded: DecodedAudio) -> Result<(), String> {
        let channels = decoded.channels;
        let sample_rate = decoded.sample_rate;
        if channels == 0 || sample_rate == 0 {
            return Err("this file has no playable audio".to_string());
        }
        // The shortest channel bounds the track. Channels of unequal length mean a damaged
        // file; playing past the short one would read silence from it while the others keep
        // going, which sounds like a fault in the player rather than in the file.
        let frames = decoded
            .channel_samples
            .iter()
            .map(Vec::len)
            .min()
            .unwrap_or(0);
        if frames == 0 {
            return Err("this file decoded to no audio frames".to_string());
        }

        let channel_count: ChannelCount =
            NonZero::new(u16::try_from(channels).map_err(|_| "too many channels to play")?)
                .ok_or("this file has no playable audio")?;
        let rate: SampleRate = NonZero::new(sample_rate).ok_or("invalid sample rate")?;

        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("no audio output device available: {e}"))?;
        let player = rodio::Player::connect_new(device.mixer());
        player.pause();
        player.set_volume(*self.volume.lock().map_err(|_| "player state is poisoned")?);

        let cursor = Arc::new(AtomicUsize::new(0));
        let total_samples = frames * channels;
        player.append(TrackSource {
            channel_samples: Arc::new(decoded.channel_samples),
            channels: channel_count,
            sample_rate: rate,
            cursor: Arc::clone(&cursor),
            total_samples,
        });

        // Replaces whatever was loaded; dropping the old `Loaded` stops it and closes its
        // device. Assigned in one step so there is never a window with no player at all.
        *self.loaded.lock().map_err(|_| "player state is poisoned")? = Some(Loaded {
            _device: device,
            player,
            cursor,
            total_samples,
            channels,
            sample_rate,
        });
        Ok(())
    }

    /// Drops the loaded track and releases the audio device.
    pub fn unload(&self) {
        if let Ok(mut loaded) = self.loaded.lock() {
            *loaded = None;
        }
        if let Ok(mut error) = self.last_error.lock() {
            *error = None;
        }
    }

    pub fn play(&self) -> Result<PlaybackState, String> {
        self.with(|loaded| {
            // Pressing play at the end restarts rather than doing nothing, which is what the
            // button appears to promise.
            if loaded.ended() {
                loaded.cursor.store(0, Ordering::Relaxed);
            }
            loaded.player.play();
        })
    }

    pub fn pause(&self) -> Result<PlaybackState, String> {
        self.with(|loaded| loaded.player.pause())
    }

    /// Moves the playhead. Sample-exact, and expressed in the same seconds as everything
    /// else on screen.
    pub fn seek(&self, seconds: f64) -> Result<PlaybackState, String> {
        self.with(|loaded| {
            let frame = (seconds.max(0.0) * loaded.sample_rate as f64).round() as usize;
            let target = (frame * loaded.channels).min(loaded.total_samples);
            loaded.cursor.store(target, Ordering::Relaxed);
        })
    }

    pub fn set_volume(&self, volume: f32) -> Result<PlaybackState, String> {
        let volume = volume.clamp(0.0, 1.0);
        *self.volume.lock().map_err(|_| "player state is poisoned")? = volume;
        self.with(|loaded| loaded.player.set_volume(volume))
    }

    pub fn state(&self) -> Result<PlaybackState, String> {
        self.with(|_| {})
    }

    /// Runs an operation against the loaded track and reports the resulting state.
    ///
    /// The end-of-track pause lives here rather than in the source: the source runs on the
    /// audio thread, where taking a lock to stop a player would be exactly the wrong thing to
    /// do. Since the source feeds silence past the end (see [`TrackSource::next`]), noticing
    /// late costs nothing audible.
    fn with(&self, op: impl FnOnce(&Loaded)) -> Result<PlaybackState, String> {
        let guard = self.loaded.lock().map_err(|_| "player state is poisoned")?;
        let Some(loaded) = guard.as_ref() else {
            let reason = self.last_error.lock().ok().and_then(|e| e.clone());
            return Ok(PlaybackState::idle(reason));
        };
        op(loaded);
        let ended = loaded.ended();
        if ended {
            loaded.player.pause();
        }
        Ok(PlaybackState {
            position_seconds: loaded.position_seconds(),
            duration_seconds: loaded.duration_seconds(),
            playing: !loaded.player.is_paused(),
            ended,
            loaded: true,
            unavailable_reason: None,
        })
    }
}

/// Feeds the decoded track to the audio device, interleaving on the fly.
///
/// Reads straight out of the per-channel buffers the decoder produced. Interleaving into a
/// second contiguous buffer first would have doubled the memory the track costs, for a copy
/// whose only purpose is to save an integer division per sample.
struct TrackSource {
    channel_samples: Arc<Vec<Vec<f32>>>,
    channels: ChannelCount,
    sample_rate: SampleRate,
    cursor: Arc<AtomicUsize>,
    total_samples: usize,
}

impl Iterator for TrackSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let index = self.cursor.fetch_add(1, Ordering::Relaxed);
        if index >= self.total_samples {
            // Silence rather than `None`, and the difference matters: a source that ends is
            // one rodio drops, and a dropped source cannot be seeked back into. Staying alive
            // and quiet makes "seek backwards after the track finished" an ordinary cursor
            // move instead of a rebuild of the whole playback chain.
            //
            // Pinned rather than left to run away, so the reported position stops at the end
            // of the track instead of counting past it for as long as nobody presses stop.
            self.cursor.store(self.total_samples, Ordering::Relaxed);
            return Some(0.0);
        }
        let channels = self.channels.get() as usize;
        let (frame, channel) = (index / channels, index % channels);
        Some(self.channel_samples[channel][frame])
    }
}

impl Source for TrackSource {
    fn current_span_len(&self) -> Option<usize> {
        // One span, uniform throughout: the format never changes mid-track.
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        // Endless by construction — see `next`. The *track's* duration is reported through
        // `PlaybackState`, which is the number the UI actually uses.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(channels: u16, frames: usize) -> (TrackSource, Arc<AtomicUsize>) {
        let channel_samples: Vec<Vec<f32>> = (0..channels)
            .map(|c| {
                (0..frames)
                    .map(|f| (c as f32) * 1000.0 + f as f32)
                    .collect()
            })
            .collect();
        let cursor = Arc::new(AtomicUsize::new(0));
        let src = TrackSource {
            channel_samples: Arc::new(channel_samples),
            channels: NonZero::new(channels).unwrap(),
            sample_rate: NonZero::new(44_100).unwrap(),
            cursor: Arc::clone(&cursor),
            total_samples: frames * channels as usize,
        };
        (src, cursor)
    }

    /// The planar buffers have to come out interleaved in the order the device expects, or
    /// the channels swap and the stereo image inverts.
    #[test]
    fn planar_channels_are_interleaved_in_order() {
        let (mut src, _) = source(2, 3);
        let got: Vec<f32> = (&mut src).take(6).collect();
        assert_eq!(got, vec![0.0, 1000.0, 1.0, 1001.0, 2.0, 1002.0]);
    }

    /// Past the end the source must stay alive and quiet — a source that returns `None` is
    /// dropped by rodio, and a dropped source cannot be seeked back into.
    #[test]
    fn the_source_never_ends_and_the_cursor_stops_at_the_end() {
        let (mut src, cursor) = source(1, 4);
        let got: Vec<f32> = (&mut src).take(8).collect();
        assert_eq!(got, vec![0.0, 1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(
            cursor.load(Ordering::Relaxed),
            4,
            "the cursor must not run past the end"
        );
    }

    /// Seeking is a cursor move, and it is exact: the next sample out is the one at the
    /// requested frame, not one the device happened to have buffered.
    #[test]
    fn moving_the_cursor_moves_the_next_sample_exactly() {
        let (mut src, cursor) = source(2, 100);
        cursor.store(50 * 2, Ordering::Relaxed);
        assert_eq!(src.next(), Some(50.0), "left channel of frame 50");
        assert_eq!(src.next(), Some(1050.0), "right channel of frame 50");

        // And backwards, including from past the end.
        cursor.store(usize::MAX, Ordering::Relaxed);
        assert_eq!(src.next(), Some(0.0), "silence past the end");
        cursor.store(0, Ordering::Relaxed);
        assert_eq!(src.next(), Some(0.0), "frame 0, left");
        assert_eq!(src.next(), Some(1000.0), "frame 0, right");
    }
}
