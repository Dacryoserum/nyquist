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
//! accordingly.
//!
//! Two things about that blind spot are worth stating precisely, because both were
//! understated until the corpus grew material that could show them.
//!
//! **It is wider than the bitrate suggests.** On non-stationary material, AAC at *128*
//! kbps also escapes: `transcoded_dynamic_aac_128_44k.flac` lowpasses at 18.3 kHz but only
//! at 27 dB/kHz, under [`STEEPNESS_TRANSCODE_THRESHOLD`], and lands on `Indeterminate`. The
//! same encoder on flat noise measures ~106 dB/kHz and is caught easily. What decides it is
//! the program material, not the bitrate.
//!
//! **In this branch the failure is an assertion, not a shrug.** A file with no detectable
//! edge is reported `ProbablyAuthentic` at [`NO_EDGE_CONFIDENCE`], so a transparent
//! transcode is not merely missed — it is actively vouched for. `corpus/README.md` has
//! called for these to score "indéterminé, never a confident authentic" since the corpus was
//! written, and this module does not yet meet that bar. It is left as-is deliberately rather
//! than papered over with a lower constant: routing every no-edge file to `Indeterminate`
//! would make the verdict useless for the genuinely lossless majority, and any number
//! between the two would be calibrated against nothing but the corpus's own arbitrary
//! composition.
//!
//! Four candidate indicators were prototyped against the corpus to close the gap —
//! spectral holes, the codec frame grid, cutoff stability, and joint-stereo collapse. None
//! separates a transparent encode from lossless; one of them actively flags legitimate
//! in-the-box piano harder than it flags a real LAME transcode. The measurements are
//! recorded in `corpus/README.md` so the next attempt does not repeat them.

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
    pub indicators: Vec<Indicator>,
}

/// One piece of stated evidence behind a verdict.
///
/// Carries the same claim twice, on purpose. `message` is the English prose this module
/// authors: it is what `nyquist-cli` prints and what an exported JSON report preserves, so
/// a report stays readable and diffable no matter which language the UI was in when it was
/// produced. `detail` is the same observation as a code plus its raw measurements, which
/// lets the UI re-render the sentence in the user's language (see `src/lib/i18n.svelte.ts`)
/// instead of showing backend English inside a translated interface.
///
/// The prose is derived from the detail by [`IndicatorDetail::english`], never written at
/// the call site, so the two cannot drift apart.
#[derive(Debug, Serialize)]
pub struct Indicator {
    pub message: String,
    #[serde(flatten)]
    pub detail: IndicatorDetail,
}

impl Indicator {
    fn new(detail: IndicatorDetail) -> Self {
        Self { message: detail.english(), detail }
    }
}

/// The closed set of observations this module can make, with the numbers each one quotes.
///
/// Serialized internally tagged on `code` and flattened into [`Indicator`], so an entry
/// reads `{"message": "...", "code": "sharp_rolloff", "steepness_db_per_khz": 92.4, ...}`.
/// Adding a variant is a frontend-visible change: `src/lib/api.ts` and the translation
/// switch in `src/lib/i18n.svelte.ts` must gain it in the same PR, and `npm run check`
/// fails until they do.
///
/// Frequencies are in kHz rather than Hz because that is the unit every message quotes;
/// keeping the conversion here means the UI formats a number instead of re-deriving one.
#[derive(Debug, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum IndicatorDetail {
    /// A container tag names an encoder that only ever produces lossy output.
    EncoderTagMatched {
        tag_key: String,
        tag_value: String,
        matched_pattern: String,
        /// Further matching tags beyond the one quoted, 0 when it was the only one.
        additional_matches: usize,
    },
    /// Caveat attached when the spectrum was inconclusive and the tag carries the verdict.
    TagIsOnlyEvidence,
    /// Caveat attached when a lossy encoder tag contradicts a full-bandwidth spectrum.
    TagContradictsSpectrum,
    /// Degenerate input: no usable sample rate, so no spectral claim can be made.
    InvalidSampleRate,
    /// A rolloff steep enough to read as a codec's lowpass rather than mix/mastering.
    SharpRolloff { steepness_db_per_khz: f64, edge_khz: f64 },
    /// The scan ran the whole plausible encoder range and found no lowpass anywhere.
    NoEncoderLowpass { scanned_from_khz: f64, nyquist_khz: f64 },
    /// The blind spot from the module docs, stated alongside every "authentic" verdict.
    TransparentEncodeUnseen,
    /// An edge exists but is too gradual to attribute to a codec either way.
    GradualRolloff { cutoff_khz: f64, steepness_db_per_khz: f64 },
}

impl IndicatorDetail {
    /// The reference wording for this observation. Single source of the English prose:
    /// the CLI, the JSON export and the English UI all end up rendering this exact string.
    fn english(&self) -> String {
        match self {
            Self::EncoderTagMatched { tag_key, tag_value, matched_pattern, additional_matches } => {
                format!(
                    "Encoder tag \"{}\" reads \"{}\", matching the lossy-only encoder \"{}\"{}.",
                    tag_key,
                    tag_value,
                    matched_pattern,
                    if *additional_matches > 0 {
                        format!(", plus {additional_matches} more matching tag(s)")
                    } else {
                        String::new()
                    }
                )
            }
            Self::TagIsOnlyEvidence => "The spectrum alone was inconclusive, so this verdict \
                 rests on the tag — which can be stale, copied from a source file, or simply \
                 wrong."
                .to_string(),
            Self::TagContradictsSpectrum => "This contradicts the spectral measurement above, \
                 which found no encoder cutoff. Either the tag is left over from an earlier \
                 step in the file's history and the audio really is lossless, or it was a \
                 transparent lossy encode that this method cannot see. Reported as \
                 inconclusive rather than letting either signal overrule the other."
                .to_string(),
            Self::InvalidSampleRate => {
                "Invalid sample rate; cannot evaluate spectral content.".to_string()
            }
            Self::SharpRolloff { steepness_db_per_khz, edge_khz } => format!(
                "Sharp spectral rolloff (~{steepness_db_per_khz:.0} dB/kHz) around \
                 {edge_khz:.1} kHz — steep enough to match a lossy encoder's lowpass filter \
                 rather than natural mix/mastering content (natural rolloff measured well \
                 under 20 dB/kHz across this project's test corpus, real and synthetic)."
            ),
            Self::NoEncoderLowpass { scanned_from_khz, nyquist_khz } => format!(
                "No encoder lowpass found: the spectrum was scanned from \
                 {scanned_from_khz:.0} kHz to the {nyquist_khz:.1} kHz Nyquist frequency and \
                 no point showed the sharp drop into a sustained empty band that a lossy \
                 codec leaves behind."
            ),
            Self::TransparentEncodeUnseen => "This does not rule out a transparent lossy \
                 encode (e.g. LAME V0, AAC 256kbps) — this project's own corpus shows those \
                 measuring indistinguishable from lossless by this method. Confidence is \
                 capped accordingly."
                .to_string(),
            Self::GradualRolloff { cutoff_khz, steepness_db_per_khz } => format!(
                "Content stops around {cutoff_khz:.1} kHz, but the transition there is \
                 gradual (~{steepness_db_per_khz:.0} dB/kHz) rather than the near-vertical \
                 wall a codec produces. That is consistent with a deliberately dark master, \
                 a vinyl or tape transfer, or a lossy encode whose filter this method cannot \
                 separate from those — not enough to call it either way."
            ),
        }
    }
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

    assessment.indicators.push(Indicator::new(IndicatorDetail::EncoderTagMatched {
        tag_key: first.tag_key.clone(),
        tag_value: first.tag_value.clone(),
        matched_pattern: first.matched_pattern.clone(),
        additional_matches: matches.len() - 1,
    }));

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
            assessment.indicators.push(Indicator::new(IndicatorDetail::TagIsOnlyEvidence));
        }
        // Direct conflict: measurement says full bandwidth, metadata says lossy tool.
        Verdict::ProbablyAuthentic => {
            assessment.verdict = Verdict::Indeterminate;
            assessment.confidence_score = TAG_CONFLICT_CONFIDENCE;
            assessment.indicators.push(Indicator::new(IndicatorDetail::TagContradictsSpectrum));
        }
    }
}

fn assess_from_spectrum(spectral: &SpectralAnalysis, nyquist_hz: f64) -> TranscodeAssessment {
    if nyquist_hz <= 0.0 {
        return TranscodeAssessment {
            verdict: Verdict::Indeterminate,
            confidence_score: 0.0,
            indicators: vec![Indicator::new(IndicatorDetail::InvalidSampleRate)],
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
            indicators: vec![Indicator::new(IndicatorDetail::SharpRolloff {
                steepness_db_per_khz: steepness,
                edge_khz,
            })],
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
                Indicator::new(IndicatorDetail::NoEncoderLowpass {
                    scanned_from_khz: MIN_SCANNED_KHZ,
                    nyquist_khz: nyquist_hz / 1000.0,
                }),
                Indicator::new(IndicatorDetail::TransparentEncodeUnseen),
            ],
        };
    }

    // An edge exists but is too gradual to attribute to a codec: a deliberately dark
    // master, a vinyl transfer, or a tape source can all end this way.
    TranscodeAssessment {
        verdict: Verdict::Indeterminate,
        confidence_score: 0.3,
        indicators: vec![Indicator::new(IndicatorDetail::GradualRolloff {
            cutoff_khz: spectral.spectral_cutoff_hz / 1000.0,
            steepness_db_per_khz: steepness,
        })],
    }
}
