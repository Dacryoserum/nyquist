//! Searches for MP3 (MPEG-1/2 Layer III) quantization-grid evidence in decoded PCM.
//!
//! An MP3 encoder does not quantize a plain MDCT. It runs a 32-band polyphase filterbank,
//! then an 18-coefficient MDCT inside each subband, then alias-reduction butterflies across
//! subband edges (ISO/IEC 11172-3, §2.4.3.4 and Annex C). A plain MDCT of the decoded signal
//! does not land back on those coefficients, which is why `mdct_grid.rs` is blind to MP3.
//! This module rebuilds the encoder's own analysis chain and re-analyzes the file at each of
//! the 576 possible granule alignments.
//!
//! At the encoder's alignment the quantizer shows through: within a small neighbourhood of
//! the spectrum, coefficients are either gone (quantized to zero) or at least one step
//! large. One sample off that alignment the structure smears and magnitudes are spread
//! smoothly again. The statistic is the share of coefficients sitting far below the RMS of
//! their own 36-coefficient neighbourhood. It is *local* on purpose: a threshold relative
//! to the whole frame mostly counts "the treble is quiet", which music does by itself, and
//! it measured as a much weaker separator (see docs/detection-research.md).
//!
//! Unlike a spectral lowpass, this survives "transparent" presets. LAME V0 keeps the full
//! bandwidth but still quantizes every coefficient it keeps.
//!
//! This is a hypothesis test, not a proof of origin. Scope: long blocks, one energetic
//! channel, the file's own sample grid. Resampling, pitch/time edits, heavy processing or
//! added noise after decoding erase the alignment, so a negative search does not rule MP3
//! out by itself. Scores are robust deviations, not tail probabilities.

use serde::Serialize;

use crate::decode::DecodedAudio;
use crate::mdct_grid::{
    encode_profile, median_absolute_deviation, median_of, most_energetic_channel,
};
use crate::mp3_window::SYNTHESIS_WINDOW;

const SUBBANDS: usize = 32;
/// Subband samples per granule, and MDCT coefficients per subband.
const SLOTS: usize = 18;
/// One granule: 576 coefficients, advancing 576 samples.
const GRANULE: usize = SUBBANDS * SLOTS;
/// Polyphase prototype length.
const TAPS: usize = 512;
/// Coefficients per neighbourhood the sparsity test is local to: two subbands.
const BLOCK: usize = 36;
/// A coefficient this far below its neighbourhood's RMS counts as quantized away. For
/// unquantized material magnitudes are spread smoothly and under 1% fall this low.
const SPARSE_THRESHOLD_DB: f32 = -40.0;
/// Granules measured per candidate offset. The sweep only has to rank offsets.
const SWEEP_FRAMES: usize = 24;
/// Granules measured again at the winning offset, on audio the sweep never saw.
const CONFIRM_FRAMES: usize = 96;
/// Fewest confirmation granules that still count as a second measurement.
const MIN_CONFIRM_FRAMES: usize = 16;
/// Granules this far below the loudest one are left out: near-silence quantizes to the
/// output bit depth, not to a codec.
const QUIET_FRAME_DB: f32 = -40.0;
/// Required of the sweep and of the confirmation pass alike.
///
/// Set from measurement on 14 independent commercial recordings and their 70 LAME round
/// trips (V0, V2, 128/192/320 CBR): no file without an MP3 history exceeded 4 on either
/// pass, no MP3 derivative fell under 16. See docs/detection-research.md.
pub const GRID_DETECTION_Z: f64 = 12.0;
/// Samples either side of a sweep granule that a confirmation granule must stay clear of,
/// so the second pass never re-reads audio the first pass ranked offsets on.
const FRAME_GUARD: usize = 5 * GRANULE;
/// Samples a granule pair needs before its nominal start (the polyphase history).
const HISTORY: usize = TAPS - SUBBANDS;
// Candidate starts begin one granule in, which must cover the filterbank history.
const _: () = assert!(GRANULE >= HISTORY);
/// Samples needed after a candidate start to analyze it at every offset.
const LOOKAHEAD: usize = GRANULE + 3 * GRANULE;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Mp3GridAnalysis {
    /// False when the file was too short or too quiet to sweep. Every other field is
    /// meaningless in that case.
    pub analyzed: bool,
    /// True when one granule offset stands out on the sweep *and* on separate confirmation
    /// audio — see [`GRID_DETECTION_Z`].
    pub grid_detected: bool,
    /// How far the best offset stands above the file's own median offset, in robust
    /// standard deviations.
    pub z_score: f64,
    /// The same question asked again on granules the sweep did not use, against control
    /// offsets measured on those same granules.
    pub confirmed_z_score: f64,
    /// The winning offset in samples, 0..576. Checkable: files decoded by the same decoder
    /// land on the same value, which a spurious peak does not.
    pub frame_offset: usize,
    /// Share of coefficients quantized away at that offset, 0.0-1.0.
    pub sparse_fraction_at_offset: f64,
    /// The same share at control offsets — what this file looks like unaligned.
    pub sparse_fraction_baseline: f64,
    /// The whole sweep, one byte per offset scaled against the strongest, base64-encoded.
    /// Same convention as `MdctGridAnalysis::sweep_profile_base64`.
    pub sweep_profile_base64: String,
}

impl Mp3GridAnalysis {
    fn not_analyzed() -> Self {
        Self {
            analyzed: false,
            grid_detected: false,
            z_score: 0.0,
            confirmed_z_score: 0.0,
            frame_offset: 0,
            sparse_fraction_at_offset: 0.0,
            sparse_fraction_baseline: 0.0,
            sweep_profile_base64: String::new(),
        }
    }
}

/// Sweeps all 576 granule alignments for the one at which the MP3 quantizer shows through.
///
/// One channel rather than a downmix, for the reason `mdct_grid` gives: joint stereo codes
/// mid/side, and summing channels refills what the encoder removed.
pub fn analyze_mp3_grid(decoded: &DecodedAudio) -> Mp3GridAnalysis {
    let Some(channel) = most_energetic_channel(decoded) else {
        return Mp3GridAnalysis::not_analyzed();
    };
    let sweep_starts = pick_loud_starts(channel, SWEEP_FRAMES, &[]);
    if sweep_starts.len() < SWEEP_FRAMES / 2 {
        return Mp3GridAnalysis::not_analyzed();
    }
    let confirm_starts = pick_loud_starts(channel, CONFIRM_FRAMES, &sweep_starts);
    if confirm_starts.len() < MIN_CONFIRM_FRAMES {
        return Mp3GridAnalysis::not_analyzed();
    }

    let bank = HybridFilterbank::new();
    let mut slots = vec![[0.0f32; SUBBANDS]; 2 * SLOTS + SLOTS];
    let mut coefficients = [0.0f32; GRANULE];

    // Offsets sharing a polyphase phase (offset mod 32) share their subband samples, so the
    // filterbank runs 32 times per granule rather than 576.
    let mut profile = vec![0.0f32; GRANULE];
    for phase in 0..SUBBANDS {
        for &start in &sweep_starts {
            bank.subband_slots(channel, start + phase, &mut slots);
            for shift in 0..SLOTS {
                bank.granule(&slots[shift..shift + 2 * SLOTS], &mut coefficients);
                profile[shift * SUBBANDS + phase] += sparse_fraction(&coefficients);
            }
        }
    }
    for value in &mut profile {
        *value /= sweep_starts.len() as f32;
    }

    let (best_offset, &best) = profile
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .expect("sweep is non-empty");
    let median = median_of(&profile);
    let sigma = 1.4826 * median_absolute_deviation(&profile, median);
    if sigma <= 0.0 {
        return Mp3GridAnalysis::not_analyzed();
    }
    let z_score = (best - median) as f64 / sigma as f64;

    let mut measure = |offset: usize| -> f32 {
        let mut total = 0.0f32;
        for &start in &confirm_starts {
            bank.subband_slots(channel, start + offset, &mut slots[..2 * SLOTS]);
            bank.granule(&slots[..2 * SLOTS], &mut coefficients);
            total += sparse_fraction(&coefficients);
        }
        total / confirm_starts.len() as f32
    };
    let confirmed = measure(best_offset);
    // 19 is coprime with both 32 and 18, so the controls visit every polyphase phase and
    // every granule shift instead of re-measuring the candidate's own phase.
    let controls: Vec<f32> = (0..GRANULE)
        .step_by(19)
        .filter(|&offset| offset.abs_diff(best_offset) > 2)
        .map(&mut measure)
        .collect();
    let baseline = median_of(&controls);
    let confirmed_sigma = 1.4826 * median_absolute_deviation(&controls, baseline);
    let confirmed_z_score = if confirmed_sigma > 0.0 {
        (confirmed - baseline) as f64 / confirmed_sigma as f64
    } else {
        0.0
    };

    Mp3GridAnalysis {
        analyzed: true,
        grid_detected: z_score >= GRID_DETECTION_Z && confirmed_z_score >= GRID_DETECTION_Z,
        z_score,
        confirmed_z_score,
        frame_offset: best_offset,
        sparse_fraction_at_offset: confirmed as f64,
        sparse_fraction_baseline: baseline as f64,
        sweep_profile_base64: encode_profile(&profile, best),
    }
}

/// Granule starts drawn from the loud part of the file and spread across it, each with
/// room for the polyphase history before it and every candidate offset after it.
fn pick_loud_starts(channel: &[f32], wanted: usize, excluded: &[usize]) -> Vec<usize> {
    let Some(last) = channel.len().checked_sub(LOOKAHEAD) else {
        return Vec::new();
    };
    let levels: Vec<(usize, f64)> = (GRANULE..last)
        .step_by(GRANULE)
        .map(|start| {
            let frame = &channel[start..start + 2 * GRANULE];
            let power = frame.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
            (start, power / frame.len() as f64)
        })
        .collect();
    let loudest = levels.iter().map(|&(_, p)| p).fold(0.0f64, f64::max);
    if loudest <= 0.0 {
        return Vec::new();
    }
    let floor = loudest * 10f64.powf(QUIET_FRAME_DB as f64 / 10.0);
    let loud: Vec<usize> = levels
        .iter()
        .filter(|&&(start, power)| {
            power > floor
                && excluded
                    .iter()
                    .all(|&other| start.abs_diff(other) >= FRAME_GUARD)
        })
        .map(|&(start, _)| start)
        .collect();
    let step = (loud.len() / wanted).max(1);
    loud.into_iter().step_by(step).take(wanted).collect()
}

/// Share of coefficients more than [`SPARSE_THRESHOLD_DB`] below the RMS of their own
/// [`BLOCK`]-coefficient neighbourhood. A neighbourhood with no energy at all contributes
/// nothing: exact zeros are digital silence, not a quantizer.
fn sparse_fraction(coefficients: &[f32; GRANULE]) -> f32 {
    let ratio_squared = 10f32.powf(SPARSE_THRESHOLD_DB / 10.0);
    let mut sparse = 0usize;
    for block in coefficients.as_chunks::<BLOCK>().0 {
        let mean_square = block.iter().map(|&c| c * c).sum::<f32>() / BLOCK as f32;
        let threshold = mean_square * ratio_squared;
        sparse += block.iter().filter(|&&c| c * c < threshold).count();
    }
    sparse as f32 / GRANULE as f32
}

/// The Layer III encoder's analysis chain: polyphase filterbank, long-block MDCT per
/// subband, alias-reduction butterflies. Tables are built once per analysis.
struct HybridFilterbank {
    /// Analysis window C[i] (ISO/IEC 11172-3 Table C.1).
    window: [f32; TAPS],
    /// `matrix[k][i] = cos((2k + 1)(i - 16)π / 64)`, 32 × 64.
    matrix: Vec<[f32; 64]>,
    /// Sine-windowed 36 → 18 MDCT basis, `mdct[k][n]`.
    mdct: [[f32; 2 * SLOTS]; SLOTS],
    /// Butterfly coefficients from Table B.9.
    cs: [f32; 8],
    ca: [f32; 8],
}

impl HybridFilterbank {
    fn new() -> Self {
        use std::f64::consts::PI;
        let mut window = [0.0f32; TAPS];
        for (w, &d) in window.iter_mut().zip(SYNTHESIS_WINDOW.iter()) {
            *w = d / 32.0;
        }
        let matrix = (0..SUBBANDS)
            .map(|k| {
                let mut row = [0.0f32; 64];
                for (i, value) in row.iter_mut().enumerate() {
                    *value = ((2 * k + 1) as f64 * (i as f64 - 16.0) * PI / 64.0).cos() as f32;
                }
                row
            })
            .collect();
        let mut mdct = [[0.0f32; 2 * SLOTS]; SLOTS];
        for (k, row) in mdct.iter_mut().enumerate() {
            for (n, value) in row.iter_mut().enumerate() {
                let window = (PI / 36.0 * (n as f64 + 0.5)).sin();
                let basis = (PI / 72.0 * (2 * n + 1 + SLOTS) as f64 * (2 * k + 1) as f64).cos();
                *value = (window * basis) as f32;
            }
        }
        const C: [f64; 8] = [
            -0.6, -0.535, -0.33, -0.185, -0.095, -0.041, -0.0142, -0.0037,
        ];
        let mut cs = [0.0f32; 8];
        let mut ca = [0.0f32; 8];
        for i in 0..8 {
            let norm = (1.0 + C[i] * C[i]).sqrt();
            cs[i] = (1.0 / norm) as f32;
            ca[i] = (C[i] / norm) as f32;
        }
        Self {
            window,
            matrix,
            mdct,
            cs,
            ca,
        }
    }

    /// Fills `slots` with consecutive 32-band subband samples; slot `t` is the filterbank
    /// output once the 32 input samples starting at `start + 32 t` have been shifted in.
    /// `start` must be at least [`HISTORY`].
    fn subband_slots(&self, channel: &[f32], start: usize, slots: &mut [[f32; SUBBANDS]]) {
        for (t, slot) in slots.iter_mut().enumerate() {
            let newest = start + SUBBANDS * (t + 1) - 1;
            let history = &channel[newest + 1 - TAPS..=newest];
            // Window, then fold the 512 products into 64 partial sums. The standard indexes
            // from the newest sample backwards.
            let mut folded = [0.0f32; 64];
            for (i, (&w, &x)) in self.window.iter().zip(history.iter().rev()).enumerate() {
                folded[i % 64] += w * x;
            }
            for (out, row) in slot.iter_mut().zip(self.matrix.iter()) {
                *out = row.iter().zip(folded.iter()).map(|(&m, &y)| m * y).sum();
            }
        }
    }

    /// Turns 36 consecutive subband slots into one granule's 576 long-block coefficients,
    /// ordered subband by subband.
    fn granule(&self, slots: &[[f32; SUBBANDS]], out: &mut [f32; GRANULE]) {
        debug_assert_eq!(slots.len(), 2 * SLOTS);
        for band in 0..SUBBANDS {
            // Frequency inversion: odd time samples of odd subbands are negated before the
            // MDCT, which undoes the spectral mirroring of odd polyphase bands.
            let mut input = [0.0f32; 2 * SLOTS];
            for (n, value) in input.iter_mut().enumerate() {
                let sample = slots[n][band];
                *value = if band % 2 == 1 && n % 2 == 1 {
                    -sample
                } else {
                    sample
                };
            }
            for (k, row) in self.mdct.iter().enumerate() {
                out[band * SLOTS + k] = row.iter().zip(input.iter()).map(|(&m, &x)| m * x).sum();
            }
        }
        // Encoder-side alias reduction: the inverse rotation of the decoder's butterflies.
        for edge in (SLOTS..GRANULE).step_by(SLOTS) {
            for i in 0..8 {
                let lower = out[edge - 1 - i];
                let upper = out[edge + i];
                out[edge - 1 - i] = lower * self.cs[i] + upper * self.ca[i];
                out[edge + i] = upper * self.cs[i] - lower * self.ca[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::DecodeStatus;

    fn decoded(samples: Vec<f32>) -> DecodedAudio {
        DecodedAudio {
            sample_rate: 44_100,
            channels: 1,
            channel_layout: symphonia::core::audio::Channels::Discrete(1),
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
        }
    }

    fn noise(len: usize) -> Vec<f32> {
        let mut state = 0x2468_ace1u32;
        (0..len)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 8) as f32 / 8_388_608.0 - 1.0
            })
            .collect()
    }

    /// The polyphase stage must put a tone in the subband that owns its frequency, or the
    /// whole chain is analyzing something other than what an encoder analyzed. Subband k
    /// spans `k·π/32 .. (k+1)·π/32`; the tone sits mid-band in subband 5.
    #[test]
    fn a_tone_lands_in_its_own_subband() {
        let omega = std::f32::consts::PI * 5.5 / 32.0;
        let samples: Vec<f32> = (0..4096).map(|n| (omega * n as f32).sin()).collect();
        let bank = HybridFilterbank::new();
        let mut slots = vec![[0.0f32; SUBBANDS]; 36];
        bank.subband_slots(&samples, HISTORY + 64, &mut slots);
        let mut energy = [0.0f32; SUBBANDS];
        for slot in &slots {
            for (e, &s) in energy.iter_mut().zip(slot.iter()) {
                *e += s * s;
            }
        }
        let total: f32 = energy.iter().sum();
        assert!(
            energy[5] / total > 0.999,
            "subband 5 holds {:.4} of the energy",
            energy[5] / total
        );
    }

    /// A stationary tone must also stay compact after the MDCT and butterflies: the
    /// hybrid's 576 coefficients are ordered by frequency, so the energy belongs around
    /// coefficient 5.5 × 18 = 99.
    #[test]
    fn the_hybrid_orders_coefficients_by_frequency() {
        let omega = std::f32::consts::PI * 5.5 / 32.0;
        let samples: Vec<f32> = (0..4096).map(|n| (omega * n as f32).sin()).collect();
        let bank = HybridFilterbank::new();
        let mut slots = vec![[0.0f32; SUBBANDS]; 36];
        let mut coefficients = [0.0f32; GRANULE];
        bank.subband_slots(&samples, HISTORY + 64, &mut slots);
        bank.granule(&slots, &mut coefficients);
        let total: f32 = coefficients.iter().map(|c| c * c).sum();
        let near: f32 = coefficients[94..104].iter().map(|c| c * c).sum();
        assert!(
            near / total > 0.98,
            "{:.4} near coefficient 99",
            near / total
        );
    }

    /// Guards the direction that matters most: accusing a lossless file.
    #[test]
    fn unaligned_noise_shows_no_grid() {
        let result = analyze_mp3_grid(&decoded(noise(GRANULE * 600)));
        assert!(result.analyzed);
        assert!(
            !result.grid_detected,
            "white noise read as an MP3 grid: z {} / {}",
            result.z_score, result.confirmed_z_score
        );
    }

    /// The second pass is only independent if it reads audio the first pass never ranked
    /// offsets on, with room for the filterbank history.
    #[test]
    fn confirmation_never_reuses_sweep_audio() {
        let samples = noise(GRANULE * 600);
        let sweep = pick_loud_starts(&samples, SWEEP_FRAMES, &[]);
        let confirm = pick_loud_starts(&samples, CONFIRM_FRAMES, &sweep);
        assert!(confirm.len() >= MIN_CONFIRM_FRAMES);
        for &start in &confirm {
            assert!(start >= HISTORY);
            assert!(sweep
                .iter()
                .all(|&other| start.abs_diff(other) >= FRAME_GUARD));
        }
    }

    #[test]
    fn silence_and_short_files_are_not_analyzed() {
        assert!(!analyze_mp3_grid(&decoded(vec![0.0; GRANULE * 600])).analyzed);
        assert!(!analyze_mp3_grid(&decoded(noise(GRANULE * 10))).analyzed);
    }
}
