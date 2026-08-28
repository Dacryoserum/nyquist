//! Detects the sample-rate half of "fake hi-res": a file declaring 96 or 192 kHz whose
//! content stops dead well below the bandwidth that rate exists to carry, because it was
//! resampled up from 44.1/48 kHz rather than mastered at the higher rate.
//!
//! The direct counterpart to `bit_depth.rs`, which covers the *depth* half of the same
//! deception, and deliberately kept separate from `transcode_detect.rs` for the same
//! reason bit depth is: an upsampled file may never have touched a lossy codec. Reporting
//! "probably transcoded" for one would be a category error — the audio is bit-for-bit
//! lossless, it is the *sample rate* on the label that is inflated.
//!
//! ## Why this needs its own module rather than a tweak to the transcode verdict
//!
//! A resampler's anti-imaging filter is a brick wall, so an upsampled file shows exactly
//! the steep cutoff that `transcode_detect.rs` treats as an encoder signature. Left to
//! that module the file would be reported as a lossy transcode, naming the wrong defect
//! and pointing the user at the wrong remedy. Splitting them keeps each verdict about one
//! thing.
//!
//! ## What it deliberately does not claim
//!
//! Not the exact source rate. A resampler leaves transition-band ringing above its true
//! cutoff, so the measured bandwidth overshoots the original Nyquist by a variable margin
//! (~24.8 kHz measured on a 44.1→96 kHz fixture whose real edge was 22.05 kHz). Naming
//! "this came from 44.1 kHz" on that basis would be a guess dressed as a measurement, so
//! this reports the bandwidth actually observed and the smallest standard rate that would
//! carry it — a claim the measurement supports.

use serde::Serialize;

/// Standard rates a file could plausibly have been produced at, ascending.
const STANDARD_SAMPLE_RATES_HZ: &[u32] = &[44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// Only files claiming to be hi-res are assessed. At 44.1/48 kHz a low bandwidth is
/// ordinary content (a dark mix, an acoustic recording), not a claim about resolution.
const HI_RES_THRESHOLD_HZ: u32 = 48_000;

/// Content must occupy at least this fraction of the declared Nyquist for the declared
/// rate to be carrying its own weight. Genuine 96 kHz material measured on this project's
/// corpus fills essentially all of it; the upsampled fixture reaches 0.52.
const MIN_BANDWIDTH_RATIO: f64 = 0.65;

/// Slack applied to the measured bandwidth before picking a sufficient rate, absorbing the
/// resampler ringing described in the module docs so it doesn't push the answer a tier up.
const BANDWIDTH_TOLERANCE: f64 = 0.9;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SampleRateAnalysis {
    pub declared_sample_rate_hz: u32,
    /// Highest frequency still carrying meaningful content — the spectral cutoff. `None`
    /// when the sweep found no point where the content stops, which is not the same as the
    /// content reaching Nyquist and must not be displayed as a bandwidth figure.
    pub content_bandwidth_hz: Option<f64>,
    /// `content_bandwidth_hz` as a fraction of the declared Nyquist. Near 1.0 means the
    /// declared rate is being used; near 0.5 means roughly half of it is empty. `None`
    /// whenever the bandwidth itself was not measured — a ratio of 1.0 fabricated from a
    /// Nyquist fallback is exactly the number that made a bass-only file read as
    /// full-bandwidth hi-res.
    pub bandwidth_ratio: Option<f64>,
    /// True only for files that claim hi-res, were actually measured, and fail to use it.
    /// Never set on an unmeasured bandwidth.
    pub likely_upsampled: bool,
    /// Smallest standard sample rate whose Nyquist covers the measured bandwidth — i.e.
    /// what this content would fit in losslessly. `None` when the file isn't claiming
    /// hi-res, or when the measurement doesn't clearly point below the declared rate.
    pub sufficient_sample_rate_hz: Option<u32>,
}

/// `spectral_cutoff_hz` comes from `spectral.rs` and is a raw measurement; this module only
/// interprets it against the declared rate.
///
/// `None` means the sweep found no point where the content stops, and nothing here can be
/// concluded from that: it is consistent with a file that fills its bandwidth and with one
/// whose energy fades too gradually to bound. Reported as unmeasured rather than assumed to
/// reach Nyquist — the fallback used to hand this function a ratio of 1.0 for a file it had
/// measured nothing about.
pub fn analyze_sample_rate(
    declared_sample_rate_hz: u32,
    spectral_cutoff_hz: Option<f64>,
) -> SampleRateAnalysis {
    let nyquist_hz = declared_sample_rate_hz as f64 / 2.0;
    let bandwidth_ratio = spectral_cutoff_hz
        .filter(|_| nyquist_hz > 0.0)
        .map(|hz| (hz / nyquist_hz).clamp(0.0, 1.0));

    let claims_hi_res = declared_sample_rate_hz > HI_RES_THRESHOLD_HZ;
    let likely_upsampled =
        claims_hi_res && bandwidth_ratio.is_some_and(|ratio| ratio < MIN_BANDWIDTH_RATIO);

    let sufficient_sample_rate_hz = if likely_upsampled {
        let needed_nyquist = spectral_cutoff_hz.unwrap_or(nyquist_hz) * BANDWIDTH_TOLERANCE;
        STANDARD_SAMPLE_RATES_HZ
            .iter()
            .copied()
            .find(|&rate| rate as f64 / 2.0 >= needed_nyquist && rate < declared_sample_rate_hz)
    } else {
        None
    };

    SampleRateAnalysis {
        declared_sample_rate_hz,
        content_bandwidth_hz: spectral_cutoff_hz,
        bandwidth_ratio,
        likely_upsampled,
        sufficient_sample_rate_hz,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genuine_hi_res_using_its_bandwidth_is_not_flagged() {
        let analysis = analyze_sample_rate(96_000, Some(47_900.0));
        assert!(!analysis.likely_upsampled);
        assert_eq!(analysis.sufficient_sample_rate_hz, None);
    }

    #[test]
    fn hi_res_declaration_with_cd_bandwidth_is_flagged() {
        // The 44.1 -> 96 kHz corpus fixture measures ~24.8 kHz of content.
        let analysis = analyze_sample_rate(96_000, Some(24_800.0));
        assert!(analysis.likely_upsampled);
        assert!(analysis.bandwidth_ratio.unwrap() < 0.55);
        assert_eq!(analysis.sufficient_sample_rate_hz, Some(48_000));
    }

    #[test]
    fn cd_rate_files_are_never_flagged_however_dark() {
        // A genuinely treble-poor 44.1 kHz master is ordinary content, not a resolution
        // claim — this module must stay silent about it.
        let analysis = analyze_sample_rate(44_100, Some(6_000.0));
        assert!(!analysis.likely_upsampled);
        assert_eq!(analysis.sufficient_sample_rate_hz, None);
    }

    /// An unmeasured bandwidth is not a wide one. Nothing may be concluded, and nothing may
    /// be displayed as though it had been measured.
    #[test]
    fn an_unmeasured_bandwidth_produces_no_ratio_and_no_verdict() {
        let analysis = analyze_sample_rate(96_000, None);
        assert_eq!(analysis.content_bandwidth_hz, None);
        assert_eq!(
            analysis.bandwidth_ratio, None,
            "no ratio may be fabricated from a fallback"
        );
        assert!(!analysis.likely_upsampled);
        assert_eq!(analysis.sufficient_sample_rate_hz, None);
    }
}
