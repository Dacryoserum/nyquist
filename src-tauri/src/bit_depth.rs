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

    let scale = 2f64.powi((declared_bits - 1) as i32);
    let grid_values: Vec<i64> = decoded
        .channel_samples
        .iter()
        .flat_map(|channel| channel.iter())
        .map(|&s| (s as f64 * scale).round() as i64)
        .collect();

    if grid_values.is_empty() {
        return BitDepthAnalysis { declared_bit_depth, effective_bit_depth: None };
    }

    for candidate in MIN_CANDIDATE_BITS..declared_bits {
        let step = 1i64 << (declared_bits - candidate);
        let aligned = grid_values.iter().filter(|&&v| v % step == 0).count();
        let fraction = aligned as f64 / grid_values.len() as f64;
        if fraction >= ALIGNMENT_THRESHOLD {
            return BitDepthAnalysis { declared_bit_depth, effective_bit_depth: Some(candidate) };
        }
    }

    BitDepthAnalysis { declared_bit_depth, effective_bit_depth: Some(declared_bits) }
}
