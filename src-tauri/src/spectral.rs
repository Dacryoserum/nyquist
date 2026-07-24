//! FFT-based spectrogram and raw spectral cutoff detection. See
//! `.claude/skills/dsp-correctness/SKILL.md` and
//! `.claude/skills/tauri-ipc-contract/SKILL.md` before touching this file.
//!
//! The spectral cutoff computed here is a **raw measurement**, not a transcode verdict —
//! that scoring is V0.3 (`transcode_detect.rs`, not yet written). Never phrase this value
//! in the UI as "authentic"/"transcoded".

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;
use serde::Serialize;

use crate::decode::DecodedAudio;

const FFT_SIZE: usize = 4096;
const HOP_SIZE: usize = FFT_SIZE / 2; // 50% overlap — standard STFT tradeoff.
const TARGET_TIME_BINS: usize = 600;
const TARGET_FREQUENCY_BINS: usize = 300;
/// Display floor: anything this far below the loudest frame is rendered as silence.
/// Not a measurement threshold — purely a visualization contrast choice.
const DB_FLOOR: f32 = -90.0;
const DB_CEIL: f32 = 0.0;
/// A frame's energy must be within this many dB of the spectrum's overall peak to count
/// toward the cutoff — i.e. "the highest frequency that still carries meaningful energy",
/// not "the highest frequency with any energy at all" (FFT leakage/dither means there's
/// always *some* energy in every bin).
const CUTOFF_THRESHOLD_DB: f32 = -40.0;
/// Reference dB drops (relative to the mean-spectrum peak) used to measure rolloff
/// steepness — see [`rolloff_steepness_db_per_khz`]. Chosen to bracket a typical lossy
/// encoder lowpass transition without reaching all the way down to the noise floor.
const STEEPNESS_UPPER_DB: f32 = -20.0;
const STEEPNESS_LOWER_DB: f32 = -55.0;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SpectrogramData {
    pub time_bin_count: usize,
    pub frequency_bin_count: usize,
    pub max_frequency_hz: f64,
    pub duration_seconds: f64,
    /// Row-major `[time][frequency]`, dB values quantized to u8 (DB_FLOOR..=DB_CEIL
    /// mapped to 0..=255) and base64-encoded — never a raw JSON float matrix, see
    /// tauri-ipc-contract skill. Decode client-side into a `Uint8Array`.
    pub intensity_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SpectralAnalysis {
    /// Highest frequency still carrying energy within `CUTOFF_THRESHOLD_DB` of the
    /// track's peak. A raw indicator only — see module docs.
    pub spectral_cutoff_hz: f64,
    /// How abruptly energy drops around the cutoff, in dB per kHz (higher = steeper).
    /// A hard lossy-encoder lowpass produces a steep, narrow-band transition; a natural
    /// mix/mastering rolloff spreads the same dB drop over many kHz. Position alone
    /// (`spectral_cutoff_hz`) cannot tell these apart — real acoustic recordings can
    /// legitimately have a *low* cutoff position (see `.claude/CONTEXT.md`, ~8kHz
    /// measured on a real orchestral track) without being a transcode. A raw indicator
    /// only — see module docs.
    pub rolloff_steepness_db_per_khz: f64,
    /// `spectral_cutoff_hz` computed independently within each of the spectrogram's time
    /// bins (same `global_peak_db` reference throughout, so values are directly
    /// comparable across the track) rather than once over the whole file. Catches a
    /// transcode that only patches in real high-frequency content for part of the track
    /// (e.g. just a loud finale) — the whole-file cutoff alone would average that out and
    /// miss it. Same length/time alignment as `spectrogram.time_bin_count`. Still a raw
    /// measurement, not a verdict.
    pub cutoff_over_time_hz: Vec<f64>,
    pub spectrogram: SpectrogramData,
}

pub fn analyze_spectrum(decoded: &DecodedAudio) -> Result<SpectralAnalysis, String> {
    if decoded.sample_rate == 0 || decoded.channels == 0 {
        return Err("cannot compute spectrum: no decoded audio".to_string());
    }

    let mono = downmix_to_mono(decoded);
    if mono.len() < FFT_SIZE {
        return Err("file too short for spectral analysis".to_string());
    }

    let nyquist_hz = decoded.sample_rate as f64 / 2.0;
    let raw_bin_count = FFT_SIZE / 2; // Nyquist bin excluded (real-input FFT symmetry).

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let window = hann_window(FFT_SIZE);

    let frame_count = (mono.len() - FFT_SIZE) / HOP_SIZE + 1;
    let mut frames_db: Vec<Vec<f32>> = Vec::with_capacity(frame_count);

    let mut scratch = vec![Complex32::new(0.0, 0.0); FFT_SIZE];
    for frame_idx in 0..frame_count {
        let start = frame_idx * HOP_SIZE;
        for i in 0..FFT_SIZE {
            scratch[i] = Complex32::new(mono[start + i] * window[i], 0.0);
        }
        fft.process(&mut scratch);

        let mut bins_db = Vec::with_capacity(raw_bin_count);
        for bin in scratch.iter().take(raw_bin_count) {
            let magnitude = bin.norm() / (FFT_SIZE as f32 / 2.0);
            bins_db.push(linear_to_db(magnitude));
        }
        frames_db.push(bins_db);
    }

    let global_peak_db = frames_db.iter().flatten().copied().fold(f32::MIN, f32::max);
    let spectral_cutoff_hz = detect_cutoff_in_frames(&frames_db, global_peak_db, raw_bin_count, nyquist_hz);
    let rolloff_steepness_db_per_khz = measure_rolloff_steepness(&frames_db, raw_bin_count, nyquist_hz);

    let time_bin_count = TARGET_TIME_BINS.min(frame_count);
    let frequency_bin_count = TARGET_FREQUENCY_BINS.min(raw_bin_count);
    let intensity = downsample_and_quantize(&frames_db, raw_bin_count, time_bin_count, frequency_bin_count);
    let cutoff_over_time_hz =
        cutoff_over_time(&frames_db, raw_bin_count, nyquist_hz, time_bin_count, global_peak_db);

    Ok(SpectralAnalysis {
        spectral_cutoff_hz,
        rolloff_steepness_db_per_khz,
        cutoff_over_time_hz,
        spectrogram: SpectrogramData {
            time_bin_count,
            frequency_bin_count,
            max_frequency_hz: nyquist_hz,
            duration_seconds: mono.len() as f64 / decoded.sample_rate as f64,
            intensity_base64: base64_encode(&intensity),
        },
    })
}

fn downmix_to_mono(decoded: &DecodedAudio) -> Vec<f32> {
    if decoded.channels == 1 {
        return decoded.channel_samples[0].clone();
    }
    let len = decoded.channel_samples[0].len();
    let mut mono = vec![0.0f32; len];
    for channel in &decoded.channel_samples {
        for (m, s) in mono.iter_mut().zip(channel.iter()) {
            *m += s;
        }
    }
    let inv_channels = 1.0 / decoded.channels as f32;
    for m in &mut mono {
        *m *= inv_channels;
    }
    mono
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos())
        .collect()
}

fn linear_to_db(magnitude: f32) -> f32 {
    if magnitude <= 1e-10 {
        DB_FLOOR
    } else {
        20.0 * magnitude.log10()
    }
}

/// Highest frequency where any frame in `frames` comes within `CUTOFF_THRESHOLD_DB` of
/// `peak_db` — scans from Nyquist downward so the result is "highest surviving
/// frequency", not "first frequency that happens to be loud". `peak_db` is passed in
/// (rather than computed from `frames`) so callers can share one whole-file reference
/// across many windows — see [`cutoff_over_time`].
fn detect_cutoff_in_frames(frames: &[Vec<f32>], peak_db: f32, raw_bin_count: usize, nyquist_hz: f64) -> f64 {
    if peak_db <= DB_FLOOR || frames.is_empty() {
        return 0.0;
    }
    let threshold = peak_db + CUTOFF_THRESHOLD_DB;

    for bin in (0..raw_bin_count).rev() {
        let bin_peak_db = frames.iter().map(|frame| frame[bin]).fold(f32::MIN, f32::max);
        if bin_peak_db >= threshold {
            return bin as f64 / raw_bin_count as f64 * nyquist_hz;
        }
    }
    0.0
}

/// `detect_cutoff_in_frames`, computed independently within each of `time_bin_count`
/// windows over `frames_db` — same window boundaries as `downsample_and_quantize`, so the
/// result lines up with the spectrogram's time axis. See `SpectralAnalysis::cutoff_over_time_hz`.
fn cutoff_over_time(
    frames_db: &[Vec<f32>],
    raw_bin_count: usize,
    nyquist_hz: f64,
    time_bin_count: usize,
    global_peak_db: f32,
) -> Vec<f64> {
    let frame_count = frames_db.len();
    (0..time_bin_count)
        .map(|t| {
            let frame_start = t * frame_count / time_bin_count;
            let frame_end = ((t + 1) * frame_count / time_bin_count).max(frame_start + 1).min(frame_count);
            detect_cutoff_in_frames(&frames_db[frame_start..frame_end], global_peak_db, raw_bin_count, nyquist_hz)
        })
        .collect()
}

/// Steady-state spectral envelope (mean over time, not max) — max-over-time is right for
/// the visualization (preserves transients) but would overstate how much energy survives
/// at a given frequency for the purpose of measuring a *sustained* rolloff shape.
fn mean_spectrum(frames_db: &[Vec<f32>], raw_bin_count: usize) -> Vec<f32> {
    let mut mean = vec![0.0f32; raw_bin_count];
    for frame in frames_db {
        for (m, &v) in mean.iter_mut().zip(frame.iter()) {
            *m += v;
        }
    }
    let inv_frames = 1.0 / frames_db.len() as f32;
    for m in &mut mean {
        *m *= inv_frames;
    }
    mean
}

/// Highest frequency where the steady-state spectrum crosses `threshold_db` below the
/// spectrum's own peak. Used twice (at two different thresholds) to measure rolloff
/// steepness — see [`measure_rolloff_steepness`].
fn highest_crossing_hz(mean_db: &[f32], peak_db: f32, threshold_db: f32, raw_bin_count: usize, nyquist_hz: f64) -> f64 {
    let target = peak_db + threshold_db;
    for bin in (0..raw_bin_count).rev() {
        if mean_db[bin] >= target {
            return bin as f64 / raw_bin_count as f64 * nyquist_hz;
        }
    }
    0.0
}

/// dB drop per kHz between two reference points on the steady-state spectral envelope
/// (`STEEPNESS_UPPER_DB` and `STEEPNESS_LOWER_DB` below peak). A narrow frequency span
/// for that dB drop means a steep/abrupt transition (encoder-lowpass-like); a wide span
/// means a gradual one (natural mix/mastering rolloff-like). See `SpectralAnalysis` docs
/// for why this matters — cutoff *position* alone cannot make that distinction.
fn measure_rolloff_steepness(frames_db: &[Vec<f32>], raw_bin_count: usize, nyquist_hz: f64) -> f64 {
    let mean_db = mean_spectrum(frames_db, raw_bin_count);
    let peak_db = mean_db.iter().copied().fold(f32::MIN, f32::max);
    if peak_db <= DB_FLOOR {
        return 0.0;
    }

    let f_upper = highest_crossing_hz(&mean_db, peak_db, STEEPNESS_UPPER_DB, raw_bin_count, nyquist_hz);
    let f_lower = highest_crossing_hz(&mean_db, peak_db, STEEPNESS_LOWER_DB, raw_bin_count, nyquist_hz);

    // If the spectrum never meaningfully drops toward the lower reference level before
    // reaching Nyquist, there is nothing to measure at all — this must return "no rolloff
    // found" (0.0), not "infinitely steep". A near-zero span here means the scan found no
    // real transition (flat spectrum all the way up), which is the opposite situation
    // from an actual near-instant transition; conflating the two via a naive division
    // previously misclassified every full-bandwidth file (noise, V0, AAC256) as having an
    // extremely steep — i.e. artificial-looking — cutoff. See corpus_smoke.rs.
    let near_nyquist_margin_hz = nyquist_hz * 0.02;
    if f_lower >= nyquist_hz - near_nyquist_margin_hz {
        return 0.0;
    }

    let span_khz = (f_lower - f_upper).abs() / 1000.0;
    if span_khz < 0.1 {
        // A genuine transition was found (f_lower is well below Nyquist) but compressed
        // into a near-zero span: as steep as this measurement can resolve.
        return (STEEPNESS_LOWER_DB - STEEPNESS_UPPER_DB).abs() as f64 / 0.1;
    }
    (STEEPNESS_LOWER_DB - STEEPNESS_UPPER_DB).abs() as f64 / span_khz
}

/// Max-pools over time (preserves transient peaks for the visualization) and mean-pools
/// over frequency (avoids single-bin noise causing visual banding), then quantizes to u8
/// against a fixed dB range for a consistent look across files.
fn downsample_and_quantize(
    frames_db: &[Vec<f32>],
    raw_bin_count: usize,
    time_bin_count: usize,
    frequency_bin_count: usize,
) -> Vec<u8> {
    let frame_count = frames_db.len();
    let mut out = vec![0u8; time_bin_count * frequency_bin_count];

    for t in 0..time_bin_count {
        let frame_start = t * frame_count / time_bin_count;
        let frame_end = ((t + 1) * frame_count / time_bin_count).max(frame_start + 1).min(frame_count);

        for f in 0..frequency_bin_count {
            let bin_start = f * raw_bin_count / frequency_bin_count;
            let bin_end = ((f + 1) * raw_bin_count / frequency_bin_count).max(bin_start + 1).min(raw_bin_count);

            let mut max_over_time = DB_FLOOR;
            for frame in &frames_db[frame_start..frame_end] {
                let mean_over_freq: f32 =
                    frame[bin_start..bin_end].iter().sum::<f32>() / (bin_end - bin_start) as f32;
                max_over_time = max_over_time.max(mean_over_freq);
            }

            let normalized = ((max_over_time - DB_FLOOR) / (DB_CEIL - DB_FLOOR)).clamp(0.0, 1.0);
            out[t * frequency_bin_count + f] = (normalized * 255.0).round() as u8;
        }
    }

    out
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}
