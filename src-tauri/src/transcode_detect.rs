//! Lossy-to-lossless transcode likelihood scoring. See
//! `.claude/skills/transcode-heuristic-validation/SKILL.md` before touching this file —
//! this is the highest-stakes code in the project: a wrong verdict either discredits a
//! legitimate file or vouches for a fake one.
//!
//! **Never a binary yes/no.** Always a 3-state verdict with a bounded, honest confidence
//! score and a human-readable list of what evidence produced it.
//!
//! ## Why rolloff *steepness* is the primary signal, not cutoff *position*
//!
//! Empirically checked against `tests/fixtures/corpus/` and, ad hoc, against real
//! commercial FLACs re-encoded through MP3 (not committed — copyrighted, see
//! `.claude/CONTEXT.md` for the numbers). Cutoff **position** alone is unreliable: real
//! quiet/orchestral recordings can have a natural cutoff as low as ~6-8kHz — well below
//! where any lossy encoder would even engage its lowpass — while being entirely authentic.
//! The *same* material transcoded through MP3 128kbps can show a near-identical position,
//! because the encoder had no audible content up there to remove in the first place.
//!
//! Rolloff **steepness** (dB/kHz, see `spectral.rs`) does not have this problem: an
//! encoder's lowpass filter produces a steep, narrow-band transition (~190-270 dB/kHz
//! measured on real LAME MP3 output in this project's corpus); natural rolloff — whether a
//! deliberate mix EQ choice or an instrument's own decay — is gradual (~5-10 dB/kHz in
//! every authentic case measured so far, synthetic or real). This module therefore uses
//! steepness as the primary gate for a "transcoded" verdict, and cutoff position only to
//! describe *where* an already-confirmed cutoff sits, never as independent evidence.
//!
//! ## Known blind spot
//!
//! A transparent lossy encode (LAME VBR V0, AAC ≥256kbps) does not reliably lowpass at
//! all — `transcoded_mp3_v0_44k.flac` and `transcoded_aac_256_44k.flac` in the corpus
//! measure indistinguishable from genuinely lossless noise by this method. This module
//! cannot catch that case; confidence in any "probably authentic" verdict is capped
//! accordingly, and detecting it is left to a future indicator (e.g. quantization noise
//! floor analysis, not implemented — see roadmap).

use serde::Serialize;

use crate::spectral::SpectralAnalysis;
use crate::tags::EncoderTagMatch;

/// Confidence assigned when a tag match is the *only* evidence (spectral verdict wasn't
/// already "transcoded") — high but deliberately not near-certain, since tags can be
/// stale, manually edited, or simply wrong. See tags.rs module docs.
const TAG_MATCH_ONLY_CONFIDENCE: f64 = 0.75;
/// Confidence when a tag match *corroborates* an already-"transcoded" spectral verdict —
/// two independent signals agreeing is stronger than either alone.
const TAG_MATCH_CORROBORATING_CONFIDENCE: f64 = 0.9;
/// Confidence attached to the `Indeterminate` verdict produced when a lossy encoder tag
/// directly contradicts a full-bandwidth spectrum. Low by construction: the point of this
/// state is that the evidence genuinely conflicts, not that a middle answer is well
/// supported.
const TAG_CONFLICT_CONFIDENCE: f64 = 0.25;

/// Below this, a rolloff reads as natural (mix/mastering/instrument decay) regardless of
/// where it sits — see module docs.
///
/// Recalibrated when `spectral.rs` switched from a span-between-two-dB-levels measurement
/// to a bounded drop across a fixed window; the two are on different scales and these
/// numbers are not comparable to the previous ones. Measured on the current corpus:
/// genuinely authentic content tops out at 12 dB/kHz (the naturally-treble-poor trap
/// fixture), a resampled file reads 18, and real LAME transcodes land at 90-94. The
/// physics backs the gap up — a mastering lowpass steep enough to be exotic (96 dB/octave,
/// linear phase) still only works out to ~6 dB/kHz up at 16 kHz, so nothing short of a
/// codec or resampler brick wall reaches 40.
const STEEPNESS_TRANSCODE_THRESHOLD: f64 = 40.0;
/// At or above this, confidence in a "transcoded" reading saturates. Real LAME cutoffs
/// measured in this project's corpus after the recalibration above: ~90-94 dB/kHz.
const STEEPNESS_CONFIDENT_THRESHOLD: f64 = 85.0;
/// Confidence in "probably authentic" when the scan found no lowpass at all. Flat rather
/// than scaled by bandwidth: the evidence is categorical (an edge was searched for and not
/// found), and the ceiling is set by the known blind spot in the module docs, not by how
/// much of Nyquist the content happens to occupy.
///
/// The previous version scaled this by cutoff/Nyquist and required that ratio to reach 0.92.
/// That silently made the branch unreachable for real music, whose peak-relative cutoff sits
/// around 5 kHz, so every genuine lossless file fell through to `Indeterminate` at 30%.
const NO_EDGE_CONFIDENCE: f64 = 0.6;
/// Lower bound of the scanned range, in kHz, quoted to the user so the claim states its own
/// scope. Mirrors `spectral::MIN_PLAUSIBLE_ENCODER_CUTOFF_HZ`.
const MIN_SCANNED_KHZ: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    ProbablyAuthentic,
    ProbablyTranscoded,
    Indeterminate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TranscodeAssessment {
    pub verdict: Verdict,
    /// Confidence in the *stated* verdict, 0.0-1.0. Deliberately capped well below 1.0 in
    /// every branch: this is a single-indicator, first-pass heuristic on a small corpus,
    /// not a validated statistical classifier — see module docs.
    pub confidence_score: f64,
    /// Human-readable evidence, one entry per contributing observation. Always non-empty
    /// — a verdict with no stated indicators is not acceptable in this codebase.
    pub indicators: Vec<String>,
}

pub fn assess_transcode_risk(
    spectral: &SpectralAnalysis,
    nyquist_hz: f64,
    encoder_tag_matches: &[EncoderTagMatch],
) -> TranscodeAssessment {
    let mut assessment = assess_from_spectrum(spectral, nyquist_hz);
    apply_tag_evidence(&mut assessment, encoder_tag_matches);
    assessment
}

/// Tag evidence is asymmetric on purpose: a match is real evidence of a transcode, but no
/// match is not evidence of authenticity (plenty of legitimate lossless pipelines strip
/// tags, and plenty of genuine transcodes never carried revealing ones to begin with) — so
/// this only ever pushes *toward* "transcoded", never *away* from it.
///
/// It does not, however, get to *override* the spectrum. A tag is a string an unrelated
/// program wrote once and no tool has validated since; the spectrum is a measurement of
/// the actual audio. When the two disagree — a residual lossy encoder name on a file whose
/// content runs to full bandwidth — the honest answer is that they disagree, which is what
/// `Indeterminate` is for. Letting the tag win outright meant a stale encoder string
/// silently converted a well-supported "probably authentic" into a 75%-confidence
/// accusation.
fn apply_tag_evidence(assessment: &mut TranscodeAssessment, matches: &[EncoderTagMatch]) {
    let Some(first) = matches.first() else { return };

    let indicator = format!(
        "Encoder tag \"{}\" reads \"{}\", matching the lossy-only encoder \"{}\"{}.",
        first.tag_key,
        first.tag_value,
        first.matched_pattern,
        if matches.len() > 1 {
            format!(", plus {} more matching tag(s)", matches.len() - 1)
        } else {
            String::new()
        }
    );
    assessment.indicators.push(indicator);

    match assessment.verdict {
        // Two independent signals agreeing is stronger than either alone.
        Verdict::ProbablyTranscoded => {
            assessment.confidence_score =
                assessment.confidence_score.max(TAG_MATCH_CORROBORATING_CONFIDENCE);
        }
        // Spectrum had nothing to say; the tag is then the only evidence there is.
        Verdict::Indeterminate => {
            assessment.verdict = Verdict::ProbablyTranscoded;
            assessment.confidence_score = TAG_MATCH_ONLY_CONFIDENCE;
            assessment.indicators.push(
                "The spectrum alone was inconclusive, so this verdict rests on the tag — which \
                 can be stale, copied from a source file, or simply wrong."
                    .to_string(),
            );
        }
        // Direct conflict: measurement says full bandwidth, metadata says lossy tool.
        Verdict::ProbablyAuthentic => {
            assessment.verdict = Verdict::Indeterminate;
            assessment.confidence_score = TAG_CONFLICT_CONFIDENCE;
            assessment.indicators.push(
                "This contradicts the spectral measurement above, which found no encoder \
                 cutoff. Either the tag is left over from an earlier step in the file's \
                 history and the audio really is lossless, or it was a transparent lossy \
                 encode that this method cannot see. Reported as inconclusive rather than \
                 letting either signal overrule the other."
                    .to_string(),
            );
        }
    }
}

fn assess_from_spectrum(spectral: &SpectralAnalysis, nyquist_hz: f64) -> TranscodeAssessment {
    if nyquist_hz <= 0.0 {
        return TranscodeAssessment {
            verdict: Verdict::Indeterminate,
            confidence_score: 0.0,
            indicators: vec!["Invalid sample rate; cannot evaluate spectral content.".to_string()],
        };
    }

    let steepness = spectral.rolloff_steepness_db_per_khz;
    // Where the wall is, when there is one. Falls back to the measured bandwidth so the
    // wording still names a frequency in the degenerate case.
    let edge_khz = spectral.encoder_edge_hz.unwrap_or(spectral.spectral_cutoff_hz) / 1000.0;

    if steepness >= STEEPNESS_TRANSCODE_THRESHOLD {
        let strength = ((steepness - STEEPNESS_TRANSCODE_THRESHOLD)
            / (STEEPNESS_CONFIDENT_THRESHOLD - STEEPNESS_TRANSCODE_THRESHOLD))
            .clamp(0.0, 1.0);
        return TranscodeAssessment {
            verdict: Verdict::ProbablyTranscoded,
            confidence_score: 0.5 + 0.3 * strength,
            indicators: vec![format!(
                "Sharp spectral rolloff (~{:.0} dB/kHz) around {:.1} kHz — steep enough to \
                 match a lossy encoder's lowpass filter rather than natural mix/mastering \
                 content (natural rolloff measured well under 20 dB/kHz across this \
                 project's test corpus, real and synthetic).",
                steepness, edge_khz
            )],
        };
    }

    // No edge survived the scan. `spectral.rs` sweeps the whole plausible encoder range and
    // gates each candidate on being broadband below and staying down to Nyquist above, so
    // "nothing found" is a positive result: there is no lowpass in this file.
    //
    // This replaces an earlier test that asked whether the peak-relative cutoff reached 92%
    // of Nyquist. That was unreachable for real music — which sits 40 dB below its own peak
    // by ~5 kHz while still carrying content to the top — so genuine lossless files all fell
    // through to `Indeterminate`. See `find_spectral_edge`.
    if spectral.encoder_edge_hz.is_none() {
        return TranscodeAssessment {
            verdict: Verdict::ProbablyAuthentic,
            confidence_score: NO_EDGE_CONFIDENCE,
            indicators: vec![
                format!(
                    "No encoder lowpass found: the spectrum was scanned from {:.0} kHz to the \
                     {:.1} kHz Nyquist frequency and no point showed the sharp drop into a \
                     sustained empty band that a lossy codec leaves behind.",
                    MIN_SCANNED_KHZ,
                    nyquist_hz / 1000.0
                ),
                "This does not rule out a transparent lossy encode (e.g. LAME V0, AAC \
                 256kbps) — this project's own corpus shows those measuring \
                 indistinguishable from lossless by this method. Confidence is capped \
                 accordingly."
                    .to_string(),
            ],
        };
    }

    // An edge exists but is too gradual to attribute to a codec: a deliberately dark
    // master, a vinyl transfer, or a tape source can all end this way.
    TranscodeAssessment {
        verdict: Verdict::Indeterminate,
        confidence_score: 0.3,
        indicators: vec![format!(
            "Content stops around {:.1} kHz, but the transition there is gradual \
             (~{:.0} dB/kHz) rather than the near-vertical wall a codec produces. That is \
             consistent with a deliberately dark master, a vinyl or tape transfer, or a \
             lossy encode whose filter this method cannot separate from those — not enough \
             to call it either way.",
            spectral.spectral_cutoff_hz / 1000.0,
            steepness
        )],
    }
}
