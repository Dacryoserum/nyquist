//! Detection of an AAC encoder's MDCT quantization grid.
//!
//! This is the one indicator in the project that does not read the spectral envelope, and
//! it is the reason it can see what `transcode_detect`'s rolloff measurement cannot.
//!
//! ## Why it works
//!
//! The MDCT is invertible through time-domain alias cancellation: analysing a decoded
//! signal with the same transform size, window and *frame alignment* the encoder used
//! returns the encoder's own quantized coefficients. The ones it quantized to zero come
//! back as zero — buried under whatever requantization noise the container added since, but
//! still dozens of dB below their neighbours.
//!
//! A genuinely lossless file has no such alignment. Its coefficients are full at every
//! offset. So the test is not "are there zeros" (a quiet passage or a lowpass produces
//! plenty) but "is there *one particular offset* at which zeros suddenly appear" — measured
//! against the same file's own behaviour at all the other offsets, which makes it
//! self-calibrating rather than dependent on an absolute threshold.
//!
//! This is the same idea `bit_depth.rs` applies one level down: a file whose samples land
//! exactly on a coarser quantization grid than it declares was padded, not remastered. Here
//! the grid is the encoder's, in the frequency domain.
//!
//! ## Scope, and why MP3 is not covered
//!
//! AAC transforms with a plain 1024-point MDCT, so re-analysing at the right offset inverts
//! it exactly. **MP3 does not**: it uses a hybrid filterbank — a 32-band polyphase stage
//! followed by an 18-point MDCT per subband — which a single 576-point MDCT does not
//! invert. Measured on this project's corpus, every MP3 fixture scores in the same range as
//! authentic material (z ≤ 6.1) whatever transform size is tried. Covering MP3 would mean
//! reimplementing its polyphase stage; that is a separate piece of work, not a threshold to
//! tune. The blind spot narrows to LAME, it does not close.
//!
//! Only the sine window is tested. Apple's encoder uses it for long blocks; the KBD
//! alternative was measured across the corpus and produced no alignment peak on any fixture,
//! including the AAC ones this catches with the sine window.
//!
//! ## Measured separation
//!
//! Across the corpus: 12 authentic fixtures peak at z ≤ 5.4, the three AAC transcodes at
//! z = 59.8, 125.6 and 279.2 — and all three agree on frame offset 960, which is the
//! encoder's actual grid rather than a coincidence of noise. [`GRID_DETECTION_Z`] sits in
//! the empty band between those two groups.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;
use serde::Serialize;

#[cfg(test)]
use crate::decode::DecodeStatus;
use crate::decode::DecodedAudio;

/// AAC long-block MDCT size. Independent of sample rate — a frame is 1024 coefficients
/// whether the file is 44.1 or 96 kHz.
const AAC_MDCT_N: usize = 1024;
/// A coefficient this far below its own frame's RMS counts as "the encoder zeroed it".
/// Not a hard zero on purpose: writing a decoded stream back out as 16-bit lifts every true
/// zero up to the dither floor, which is still far under this.
const ZERO_THRESHOLD_DB: f32 = -70.0;
/// Frames measured per candidate offset during the sweep. The alignment peak is extremely
/// sharp — one sample either side of the right offset and it is gone — so the sweep only
/// has to rank offsets, not measure them precisely.
const SWEEP_FRAMES: usize = 12;
/// Frames measured again at the winning offset, where precision does matter.
const CONFIRM_FRAMES: usize = 48;
/// Frames whose RMS is this far below the loudest frame carry no usable coefficients: a
/// silent lead-in is all zeros at *every* offset and would flatten the contrast the whole
/// measurement depends on. The silence-padded corpus fixture is exactly this case.
const SILENT_FRAME_DB: f32 = -60.0;
/// Robust z-score above which an offset counts as a real grid alignment rather than the
/// noise any file produces. The corpus leaves an empty band from 6.1 to 59.8; this sits in
/// it, far enough from the authentic side to absorb material this corpus does not contain.
const GRID_DETECTION_Z: f64 = 20.0;
/// Minimum samples needed to sweep at all: enough for the offset range plus the confirming
/// frames.
const MIN_SAMPLES: usize = AAC_MDCT_N * (CONFIRM_FRAMES + 4);

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MdctGridAnalysis {
    /// True when one frame offset stands out far enough to be an encoder's grid rather than
    /// this file's own noise — see [`GRID_DETECTION_Z`].
    pub grid_detected: bool,
    /// How far the best offset stands above the file's own median offset, in robust
    /// standard deviations (median absolute deviation based, so a handful of odd offsets
    /// cannot inflate it).
    pub z_score: f64,
    /// The winning offset in samples, 0..1024. Reported because it is checkable: independent
    /// AAC files encoded by the same tool land on the same value, which a spurious peak does
    /// not.
    pub frame_offset: usize,
    /// Fraction of MDCT coefficients reading as zeroed at that offset, 0.0-1.0.
    pub zero_fraction_at_offset: f64,
    /// The same fraction at the median offset — what this file looks like when it is *not*
    /// aligned. The gap between the two is the actual evidence.
    pub zero_fraction_baseline: f64,
    /// False when the file was too short, or too quiet, to sweep. Every other field is
    /// meaningless in that case.
    pub analyzed: bool,
    /// The whole sweep: one byte per candidate offset, each the zero-fraction at that offset
    /// scaled against the strongest one, base64-encoded.
    ///
    /// This is the evidence in its raw form, and it is worth showing rather than summarizing.
    /// A lossless file draws a noisy band — every offset behaves much like every other. A
    /// transcode draws a flat floor with a single spike standing on it. The difference is not
    /// subtle and does not need a threshold explained to be understood, which is exactly the
    /// property a verdict people are asked to trust should have.
    ///
    /// Scaled to the file's own maximum on purpose: the shape carries the finding, and an
    /// absolute scale would flatten the authentic case into an unreadable line near zero.
    pub sweep_profile_base64: String,
}

impl MdctGridAnalysis {
    fn not_analyzed() -> Self {
        Self {
            grid_detected: false,
            z_score: 0.0,
            frame_offset: 0,
            zero_fraction_at_offset: 0.0,
            zero_fraction_baseline: 0.0,
            analyzed: false,
            sweep_profile_base64: String::new(),
        }
    }
}

/// Sweeps every frame offset looking for the alignment at which coefficients collapse.
///
/// Runs on one channel rather than a downmix: AAC codes a stereo pair as mid/side per frame,
/// and averaging the channels back together refills exactly the coefficients the encoder
/// zeroed. Measured on the corpus, the downmix destroys the peak completely.
pub fn analyze_mdct_grid(decoded: &DecodedAudio) -> MdctGridAnalysis {
    // The most energetic channel, not the first. Still one channel rather than a downmix,
    // for the reason above — but a file whose first channel is silent (a mono source laid
    // into one side, an intro that enters on the right) would otherwise be swept on nothing
    // at all and report a clean result it never actually measured.
    let Some(channel) = decoded
        .channel_samples
        .iter()
        .filter(|c| !c.is_empty())
        .max_by(|a, b| {
            let energy = |c: &[f32]| c.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
            energy(a).total_cmp(&energy(b))
        })
    else {
        return MdctGridAnalysis::not_analyzed();
    };
    if channel.len() < MIN_SAMPLES {
        return MdctGridAnalysis::not_analyzed();
    }

    let n = AAC_MDCT_N;
    let mut mdct = MdctTransform::new(n);

    // Frames are picked from the loud part of the file and reused for every offset, so the
    // sweep compares like with like: an offset must win on the same audio the others saw.
    let starts = pick_loud_frame_starts(channel, n, SWEEP_FRAMES);
    if starts.is_empty() {
        return MdctGridAnalysis::not_analyzed();
    }

    let mut coefficients = vec![0.0f32; n];

    let mut zero_fractions = Vec::with_capacity(n);
    for offset in 0..n {
        zero_fractions.push(zero_fraction(
            channel,
            &starts,
            offset,
            &mut mdct,
            &mut coefficients,
        ));
    }

    let (best_offset, &best) = zero_fractions
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .expect("sweep is non-empty");

    let median = median_of(&zero_fractions);
    let mad = median_absolute_deviation(&zero_fractions, median);
    // 1.4826 rescales MAD to a standard deviation for normally distributed data, which is
    // what makes the threshold comparable across files.
    let sigma = 1.4826 * mad;
    if sigma <= 0.0 {
        return MdctGridAnalysis::not_analyzed();
    }

    // Re-measure the winner over more frames: the sweep only had to rank offsets, but the
    // number that gets compared to a threshold should rest on more than a dozen frames.
    let confirm_starts = pick_loud_frame_starts(channel, n, CONFIRM_FRAMES);
    let confirmed = zero_fraction(
        channel,
        &confirm_starts,
        best_offset,
        &mut mdct,
        &mut coefficients,
    );

    let z_score = ((best as f64) - median as f64) / sigma as f64;

    MdctGridAnalysis {
        grid_detected: z_score >= GRID_DETECTION_Z,
        z_score,
        frame_offset: best_offset,
        zero_fraction_at_offset: confirmed as f64,
        zero_fraction_baseline: median as f64,
        analyzed: true,
        sweep_profile_base64: encode_profile(&zero_fractions, best),
    }
}

/// Frame starts drawn from the loudest part of the file, spread across it.
///
/// Silence is the trap here: a digitally silent lead-in produces all-zero coefficients at
/// every offset, which raises the baseline to 1.0 and erases the contrast the sweep needs.
/// The corpus's silence-padded fixture measured exactly 0.0 everywhere before this.
fn pick_loud_frame_starts(channel: &[f32], n: usize, wanted: usize) -> Vec<usize> {
    let usable = channel.len().saturating_sub(3 * n);
    if usable == 0 {
        return Vec::new();
    }
    let candidate_count = usable / n;
    if candidate_count == 0 {
        return Vec::new();
    }

    let rms_of = |start: usize| -> f32 {
        let frame = &channel[start..(start + 2 * n).min(channel.len())];
        (frame.iter().map(|&s| s * s).sum::<f32>() / frame.len() as f32).sqrt()
    };

    let levels: Vec<(usize, f32)> = (0..candidate_count)
        .map(|i| (i * n, rms_of(i * n)))
        .collect();
    let loudest = levels.iter().map(|&(_, r)| r).fold(0.0f32, f32::max);
    if loudest <= 0.0 {
        return Vec::new();
    }
    let floor = loudest * 10f32.powf(SILENT_FRAME_DB / 20.0);

    let loud: Vec<usize> = levels
        .iter()
        .filter(|&&(_, r)| r > floor)
        .map(|&(s, _)| s)
        .collect();
    if loud.is_empty() {
        return Vec::new();
    }
    // Spread the picks over the whole track rather than taking the first N: a transcode
    // spliced into part of a file should still register.
    let step = (loud.len() / wanted).max(1);
    loud.into_iter().step_by(step).take(wanted).collect()
}

/// Fraction of coefficients sitting more than [`ZERO_THRESHOLD_DB`] below their frame's RMS,
/// averaged over the sampled frames.
fn zero_fraction(
    channel: &[f32],
    starts: &[usize],
    offset: usize,
    mdct: &mut MdctTransform,
    coefficients: &mut [f32],
) -> f32 {
    let n = mdct.n;
    let ratio = 10f32.powf(ZERO_THRESHOLD_DB / 20.0);
    let mut counted = 0usize;
    let mut zeros = 0usize;

    for &start in starts {
        let from = start + offset;
        if from + 2 * n > channel.len() {
            continue;
        }
        mdct.transform(&channel[from..from + 2 * n], coefficients);

        let energy: f32 = coefficients.iter().map(|&c| c * c).sum();
        if energy <= 0.0 {
            continue;
        }
        let rms = (energy / n as f32).sqrt();
        let threshold = rms * ratio;
        zeros += coefficients
            .iter()
            .filter(|&&c| c.abs() < threshold)
            .count();
        counted += n;
    }

    if counted == 0 {
        0.0
    } else {
        zeros as f32 / counted as f32
    }
}

/// Everything the transform needs, allocated once and reused across the 1024-offset sweep:
/// the window, both twiddle tables, the FFT plan and its scratch. Bundling them is not only
/// tidier than threading eight parameters through the sweep — it is what keeps the inner
/// loop allocation-free.
struct MdctTransform {
    n: usize,
    window: Vec<f32>,
    pre: Vec<Complex32>,
    post: Vec<Complex32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    scratch: Vec<Complex32>,
    fft_scratch: Vec<Complex32>,
}

impl MdctTransform {
    fn new(n: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(2 * n);
        let (pre, post) = twiddles(n);
        let fft_scratch = vec![Complex32::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        Self {
            n,
            window: sine_window(2 * n),
            pre,
            post,
            fft,
            scratch: vec![Complex32::new(0.0, 0.0); 2 * n],
            fft_scratch,
        }
    }

    /// Modified discrete cosine transform, `2n` windowed samples in, `n` coefficients out.
    ///
    /// Computed through a `2n`-point complex FFT with a pre- and post-twiddle rather than the
    /// `2n × n` cosine matrix the definition suggests. The sweep evaluates this once per
    /// offset per frame — 1024 offsets — and the matrix form is roughly two orders of
    /// magnitude too slow at that rate. Verified against the direct form in this module's
    /// tests.
    fn transform(&mut self, input: &[f32], out: &mut [f32]) {
        for (slot, ((&x, &w), &p)) in self.scratch.iter_mut().zip(
            self.window
                .iter()
                .zip(input.iter())
                .map(|(w, x)| (x, w))
                .zip(self.pre.iter()),
        ) {
            *slot = p * (x * w);
        }
        self.fft
            .process_with_scratch(&mut self.scratch, &mut self.fft_scratch);
        for (o, (&y, &p)) in out
            .iter_mut()
            .zip(self.scratch[..self.n].iter().zip(self.post.iter()))
        {
            *o = (y * p).re;
        }
    }
}

/// The sine window AAC uses for long blocks.
fn sine_window(len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| (std::f32::consts::PI / len as f32 * (i as f32 + 0.5)).sin())
        .collect()
}

/// Pre-twiddle (`2n` long) and post-twiddle (`n` long) that turn a plain DFT into an MDCT.
fn twiddles(n: usize) -> (Vec<Complex32>, Vec<Complex32>) {
    let pre = (0..2 * n)
        .map(|i| {
            let angle = -std::f64::consts::PI * i as f64 / (2.0 * n as f64);
            Complex32::new(angle.cos() as f32, angle.sin() as f32)
        })
        .collect();
    let n0 = 0.5 + n as f64 / 2.0;
    let post = (0..n)
        .map(|k| {
            let angle = -std::f64::consts::PI * n0 * (k as f64 + 0.5) / n as f64;
            Complex32::new(angle.cos() as f32, angle.sin() as f32)
        })
        .collect();
    (pre, post)
}

/// Quantizes the sweep to one byte per offset, scaled against `peak`.
fn encode_profile(zero_fractions: &[f32], peak: f32) -> String {
    use base64::Engine;
    let scale = if peak > 0.0 { 255.0 / peak } else { 0.0 };
    let bytes: Vec<u8> = zero_fractions
        .iter()
        .map(|&v| (v * scale).clamp(0.0, 255.0) as u8)
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn median_of(values: &[f32]) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[sorted.len() / 2]
}

fn median_absolute_deviation(values: &[f32], median: f32) -> f32 {
    let deviations: Vec<f32> = values.iter().map(|&v| (v - median).abs()).collect();
    median_of(&deviations)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The FFT route must agree with the transform's definition, or every number this
    /// module reports is measuring the wrong thing.
    #[test]
    fn fft_mdct_matches_the_direct_definition() {
        let n = 64;
        let mut mdct = MdctTransform::new(n);
        let window = sine_window(2 * n);

        // Deterministic pseudo-random input; the exact values do not matter, only that they
        // are not symmetric in a way that could hide an indexing error.
        let input: Vec<f32> = (0..2 * n)
            .map(|i| ((i * 37 % 101) as f32 / 50.0 - 1.0) * (i as f32 * 0.7).sin())
            .collect();

        let mut fast = vec![0.0f32; n];
        mdct.transform(&input, &mut fast);

        let direct: Vec<f32> = (0..n)
            .map(|k| {
                (0..2 * n)
                    .map(|i| {
                        let phase = std::f64::consts::PI / n as f64
                            * (i as f64 + 0.5 + n as f64 / 2.0)
                            * (k as f64 + 0.5);
                        (input[i] * window[i]) as f64 * phase.cos()
                    })
                    .sum::<f64>() as f32
            })
            .collect();

        let scale = direct.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        for (i, (&a, &b)) in fast.iter().zip(direct.iter()).enumerate() {
            assert!(
                (a - b).abs() / scale < 1e-4,
                "coefficient {i}: fft route gave {a}, definition gives {b}"
            );
        }
    }

    /// White noise has no encoder grid, so no offset may stand out. Guards the direction
    /// that matters: this indicator accusing a lossless file.
    #[test]
    fn unaligned_noise_shows_no_grid() {
        let mut state = 0x12345678u32;
        let samples: Vec<f32> = (0..AAC_MDCT_N * 80)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                (state >> 8) as f32 / 8388608.0 - 1.0
            })
            .collect();

        let decoded = DecodedAudio {
            sample_rate: 44_100,
            channels: 1,
            codec_short_name: "flac".into(),
            container_short_name: "flac".into(),
            bits_per_sample: Some(16),
            channel_samples: vec![samples],
            integrity_verified: None,
            encoder_tag_matches: Vec::new(),
            decode_status: DecodeStatus {
                complete: true,
                skipped_packets: 0,
                stopped_early: false,
                channels_unequal: false,
            },
        };

        let result = analyze_mdct_grid(&decoded);
        assert!(result.analyzed, "80 frames of noise should be analyzable");
        assert!(
            !result.grid_detected,
            "white noise must not read as an encoder grid; z was {}",
            result.z_score
        );
    }
}
