//! RMS, peak, true peak, LUFS, and clipping — see
//! `.claude/skills/dsp-correctness/SKILL.md` before touching this file.

use ebur128::{EbuR128, Mode};
use serde::Serialize;

use crate::decode::DecodedAudio;

/// Floor used instead of -inf/NaN for digital silence, since serde_json cannot serialize
/// non-finite floats.
const SILENCE_FLOOR_DB: f64 = -120.0;

fn linear_to_db(value: f64) -> f64 {
    if value <= 1e-10 {
        SILENCE_FLOOR_DB
    } else {
        20.0 * value.log10()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChannelStats {
    pub channel: usize,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    /// Simple peak-to-RMS crest factor — NOT the DR14 (Pleasurize Music Foundation)
    /// algorithm, which is block-based. Labelled explicitly per dsp-correctness skill.
    pub crest_factor_db: f64,
    pub clipping_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SignalAnalysis {
    pub peak_dbfs: f64,
    pub true_peak_dbtp: f64,
    pub rms_dbfs: f64,
    /// `None` for content with no gated block above the absolute/relative threshold
    /// (e.g. near-silent files) — a legitimate "not measurable", not a bug.
    pub lufs_integrated: Option<f64>,
    /// Loudness Range (EBU Tech 3342), in LU — how much the perceived loudness varies
    /// over the track. Companion metric to integrated LUFS, not a replacement.
    pub loudness_range_lu: Option<f64>,
    pub clipping_count_total: usize,
    pub per_channel: Vec<ChannelStats>,
}

pub fn analyze_signal(decoded: &DecodedAudio) -> Result<SignalAnalysis, String> {
    let mut per_channel = Vec::with_capacity(decoded.channels);
    let mut overall_peak_linear: f64 = 0.0;
    let mut clipping_count_total = 0usize;
    let mut total_sum_sq: f64 = 0.0;
    let mut total_samples: usize = 0;

    for (idx, samples) in decoded.channel_samples.iter().enumerate() {
        let mut peak_linear: f64 = 0.0;
        let mut sum_squares: f64 = 0.0;
        let mut clipping_count = 0usize;

        for &s in samples {
            let abs = s.abs() as f64;
            peak_linear = peak_linear.max(abs);
            sum_squares += (s as f64) * (s as f64);
            // Full-scale clip, not an arbitrary epsilon — see dsp-correctness skill.
            if abs >= 1.0 {
                clipping_count += 1;
            }
        }

        let rms_linear =
            if samples.is_empty() { 0.0 } else { (sum_squares / samples.len() as f64).sqrt() };

        let peak_dbfs = linear_to_db(peak_linear);
        let rms_dbfs = linear_to_db(rms_linear);

        per_channel.push(ChannelStats {
            channel: idx + 1,
            peak_dbfs,
            rms_dbfs,
            crest_factor_db: peak_dbfs - rms_dbfs,
            clipping_count,
        });

        overall_peak_linear = overall_peak_linear.max(peak_linear);
        clipping_count_total += clipping_count;
        total_sum_sq += sum_squares;
        total_samples += samples.len();
    }

    let overall_rms_linear =
        if total_samples == 0 { 0.0 } else { (total_sum_sq / total_samples as f64).sqrt() };

    let (lufs_integrated, loudness_range_lu, true_peak_dbtp) = measure_loudness(decoded)?;

    Ok(SignalAnalysis {
        peak_dbfs: linear_to_db(overall_peak_linear),
        true_peak_dbtp,
        rms_dbfs: linear_to_db(overall_rms_linear),
        lufs_integrated,
        loudness_range_lu,
        clipping_count_total,
        per_channel,
    })
}

/// LUFS, LRA (EBU R128 / EBU Tech 3342 / ITU-R BS.1770), and True Peak via `ebur128` — see
/// AGENTS.md "Décisions actées". Do not hand-roll K-weighting, gating, or oversampling here.
fn measure_loudness(decoded: &DecodedAudio) -> Result<(Option<f64>, Option<f64>, f64), String> {
    if decoded.channels == 0 || decoded.sample_rate == 0 {
        return Ok((None, None, SILENCE_FLOOR_DB));
    }

    let mut meter =
        EbuR128::new(decoded.channels as u32, decoded.sample_rate, Mode::I | Mode::LRA | Mode::TRUE_PEAK)
            .map_err(|e| format!("could not initialize loudness meter: {e}"))?;

    let channel_refs: Vec<&[f32]> = decoded.channel_samples.iter().map(|c| c.as_slice()).collect();
    meter
        .add_frames_planar_f32(&channel_refs)
        .map_err(|e| format!("loudness analysis failed: {e}"))?;

    let lufs_integrated = match meter.loudness_global() {
        Ok(lufs) if lufs.is_finite() => Some(lufs),
        _ => None,
    };
    let loudness_range_lu = match meter.loudness_range() {
        Ok(lra) if lra.is_finite() => Some(lra),
        _ => None,
    };

    let true_peak_linear = (0..decoded.channels)
        .map(|ch| meter.true_peak(ch as u32).unwrap_or(0.0))
        .fold(0.0_f64, f64::max);

    Ok((lufs_integrated, loudness_range_lu, linear_to_db(true_peak_linear)))
}
