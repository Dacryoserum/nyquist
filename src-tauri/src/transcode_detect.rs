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

/// Below this, a rolloff reads as natural (mix/mastering/instrument decay) regardless of
/// where it sits — see module docs. Natural rolloff measured so far: ~5-10 dB/kHz.
const STEEPNESS_TRANSCODE_THRESHOLD: f64 = 60.0;
/// At or above this, confidence in a "transcoded" reading saturates. Real LAME/AAC
/// cutoffs measured in this project's corpus: ~190-270 dB/kHz.
const STEEPNESS_CONFIDENT_THRESHOLD: f64 = 150.0;
/// Cutoff ratio (cutoff_hz / nyquist_hz) above which a *low-steepness* file counts as
/// "full bandwidth" evidence for authenticity, rather than merely inconclusive.
const HIGH_RATIO_THRESHOLD: f64 = 0.92;

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
fn apply_tag_evidence(assessment: &mut TranscodeAssessment, matches: &[EncoderTagMatch]) {
    let Some(first) = matches.first() else { return };

    let indicator = format!(
        "Tag \"{}\" contains \"{}\", matching the known lossy encoder \"{}\" — {}.",
        first.tag_key,
        first.tag_value,
        first.matched_pattern,
        if matches.len() > 1 {
            format!("plus {} more matching tag(s)", matches.len() - 1)
        } else {
            "a strong, though not infallible, sign this file was lossy-encoded at some point"
                .to_string()
        }
    );

    if assessment.verdict == Verdict::ProbablyTranscoded {
        assessment.confidence_score = assessment.confidence_score.max(TAG_MATCH_CORROBORATING_CONFIDENCE);
    } else {
        assessment.verdict = Verdict::ProbablyTranscoded;
        assessment.confidence_score = TAG_MATCH_ONLY_CONFIDENCE;
    }
    assessment.indicators.push(indicator);
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
    let ratio = spectral.spectral_cutoff_hz / nyquist_hz;

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
                steepness,
                spectral.spectral_cutoff_hz / 1000.0
            )],
        };
    }

    if ratio >= HIGH_RATIO_THRESHOLD {
        let strength = ((ratio - HIGH_RATIO_THRESHOLD) / (1.0 - HIGH_RATIO_THRESHOLD)).clamp(0.0, 1.0);
        return TranscodeAssessment {
            verdict: Verdict::ProbablyAuthentic,
            confidence_score: 0.4 + 0.2 * strength,
            indicators: vec![
                format!(
                    "Spectral content extends to {:.1} kHz, close to the {:.1} kHz Nyquist \
                     frequency, with no sharp rolloff detected.",
                    spectral.spectral_cutoff_hz / 1000.0,
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

    TranscodeAssessment {
        verdict: Verdict::Indeterminate,
        confidence_score: 0.3,
        indicators: vec![format!(
            "Spectral cutoff at {:.1} kHz with a gradual rolloff (~{:.0} dB/kHz) — too low \
             to confirm full bandwidth, but not steep enough to indicate an artificial \
             encoder cutoff either. Quiet or naturally bandlimited recordings (orchestral, \
             ambient, ...) commonly show exactly this pattern whether or not they were ever \
             lossy-compressed, since a lossy encoder has nothing audible to remove above \
             content that was already this quiet.",
            spectral.spectral_cutoff_hz / 1000.0,
            steepness
        )],
    }
}
