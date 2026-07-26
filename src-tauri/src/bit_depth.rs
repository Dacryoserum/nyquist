//! Detects a "fake hi-res" file: content whose container declares a bit depth (e.g.
//! 24-bit) that its actual samples never use, because the real information behind it
//! never exceeded a narrower depth (e.g. 16-bit) — the file was zero-padded/upsampled,
//! not genuinely re-mastered. A different problem from the lossy-transcode detection in
//! `transcode_detect.rs`: this file may never have touched a lossy codec at all.
//!
//! ## Method: exact quantization-grid alignment, not a noise-floor/SNR estimate
//!
//! A tempting approach is comparing the measured noise floor to the theoretical
//! quantization SNR for N-bit PCM (~6.02*N dB) — deliberately not used here: it needs a
//! genuinely quiet passage to measure against (unreliable on brickwalled masters, exactly
//! the kind of file this feature also needs to work on) and depends on a formula this
//! project can't cite from an ITU/EBU standard the way LUFS can.
//!
//! Instead: if a file's real information never exceeded N bits, every decoded sample,
//! quantized back to the *container's declared* bit depth, is an exact multiple of the
//! N-bit quantization step — not approximately, exactly (up to float round-trip error).
//! This is checked directly rather than estimated, and reports nothing (`None`) rather
//! than guessing when the answer isn't clear-cut.
//!
//! ## Known false-negative (by design, not a bug)
//!
//! A file that was properly dithered before being padded to a wider container will *not*
//! align exactly to the narrower grid — dither intentionally adds sub-LSB noise, which is
//! exactly what defeats this check. That's an acceptable, honest gap: it means this
//! detector only ever fires on genuine, exact evidence (a lazy padding job with no
//! dithering — the common real-world case for careless fake-hi-res files), never on a
//! guess. See `.claude/skills/transcode-heuristic-validation/SKILL.md`.

use serde::Serialize;

use crate::decode::DecodedAudio;

/// Alignment must clear this fraction of samples to count — not 100%, to tolerate a
/// handful of float round-trip edge cases without demanding literal bit-exactness.
const ALIGNMENT_THRESHOLD: f64 = 0.999;
/// Below this, "effective bit depth" stops being a meaningful concept to report (8-bit
/// audio is not a realistic candidate to distinguish from padding artifacts).
const MIN_CANDIDATE_BITS: u32 = 8;
const MAX_DECLARED_BITS: u32 = 32;
/// Samples reach this module as `f32`, whose mantissa carries 24 significant bits. Beyond
/// that the decode itself has already discarded the low-order bits, so every sample would
/// appear to sit on a coarser grid than declared and a 32-bit file would be confidently
/// mislabelled as padded. Files declaring more than this are reported as unverifiable
/// (`None`) rather than analyzed against evidence that no longer exists.
const MAX_VERIFIABLE_BITS: u32 = 24;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BitDepthAnalysis {
    /// From the container (e.g. FLAC STREAMINFO) — `None` for codecs where this isn't a
    /// meaningful concept (MP3, AAC: no fixed PCM bit depth to declare).
    pub declared_bit_depth: Option<u32>,
    /// Smallest bit depth whose quantization grid explains ~all samples. `None` if there
    /// was nothing to check (no declared depth) or no narrower depth fit — which includes
    /// the common, unremarkable case of a file that's exactly as deep as it claims.
    pub effective_bit_depth: Option<u32>,
}

pub fn analyze_bit_depth(decoded: &DecodedAudio) -> BitDepthAnalysis {
    let declared_bit_depth = decoded.bits_per_sample;

    let Some(declared_bits) =
        declared_bit_depth.filter(|&b| b > MIN_CANDIDATE_BITS && b <= MAX_DECLARED_BITS)
    else {
        return BitDepthAnalysis { declared_bit_depth, effective_bit_depth: None };
    };

    // Beyond f32's 24-bit mantissa there is nothing left to measure — see
    // MAX_VERIFIABLE_BITS. Say so instead of reporting a confident wrong answer.
    if declared_bits > MAX_VERIFIABLE_BITS {
        return BitDepthAnalysis { declared_bit_depth, effective_bit_depth: None };
    }

    let scale = 2f64.powi((declared_bits - 1) as i32);

    // A sample sits on the grid for candidate depth `c` exactly when its integer value is
    // divisible by `2^(declared - c)`, i.e. when it has at least `declared - c` trailing
    // zero bits. So one pass tallying trailing-zero counts answers the question for every
    // candidate at once, instead of materializing all samples and rescanning per candidate.
    //
    // That materialization was the single largest allocation in the whole pipeline: a
    // `Vec<i64>` over every sample of every channel, ~737 MB on an 8-minute 96 kHz stereo
    // file, walked up to 16 times. This version allocates nothing and reads each sample once.
    let mut trailing_zero_histogram = [0u64; 64];
    let mut total_samples: u64 = 0;
    for channel in &decoded.channel_samples {
        for &sample in channel {
            let value = (sample as f64 * scale).round() as i64;
            // Zero is on every grid; `trailing_zeros()` would report 64 for it anyway, but
            // bucketing it explicitly keeps the intent legible.
            let tz = if value == 0 { 63 } else { value.trailing_zeros().min(63) as usize };
            trailing_zero_histogram[tz] += 1;
            total_samples += 1;
        }
    }

    if total_samples == 0 {
        return BitDepthAnalysis { declared_bit_depth, effective_bit_depth: None };
    }

    // Coarsest grid first: alignment to a coarse grid implies alignment to every finer
    // one, so the first candidate that fits is the smallest depth explaining the data.
    for candidate in MIN_CANDIDATE_BITS..declared_bits {
        let required_trailing_zeros = (declared_bits - candidate) as usize;
        let aligned: u64 = trailing_zero_histogram[required_trailing_zeros..].iter().sum();
        if aligned as f64 / total_samples as f64 >= ALIGNMENT_THRESHOLD {
            return BitDepthAnalysis { declared_bit_depth, effective_bit_depth: Some(candidate) };
        }
    }

    BitDepthAnalysis { declared_bit_depth, effective_bit_depth: Some(declared_bits) }
}
