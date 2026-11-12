//! Evidence-based assessment of a possible lossy history in decoded audio.
//!
//! Two structural tests carry the verdict, one per codec family that accounts for nearly
//! all fake lossless files: `mdct_grid` re-analyzes the signal on AAC's transform, `mp3_grid`
//! on MP3's hybrid filterbank. Each looks for the quantizer's footprint at one exact frame
//! alignment, which a spectral envelope cannot fake and a "transparent" preset cannot avoid.
//!
//! - A confirmed grid is positive evidence of a lossy history: `ProbablyTranscoded`.
//! - Both searches run with full power and find nothing, at a sample rate those codecs
//!   operate at, with no unexplained brick-wall filter and no contradicting metadata:
//!   `ProbablyAuthentic`. This is a tested negative, not an absence of evidence — on the
//!   validation set every MP3 and AAC derivative of real recordings was caught by the same
//!   searches (docs/detection-research.md). It is still not proof: audio resampled or
//!   processed after decoding, or encoded by a codec outside these two families, can pass.
//! - Anything else is `Indeterminate`, with the specific reason stated.
//!
//! A filter edge alone never accuses a file (mastering and resampling filters look the
//! same) and ultrasonic energy alone never clears one (it can be added afterwards). Tags
//! are reported and can only withhold a clean verdict. The weights are evidence strengths,
//! never calibrated probabilities. See the transcode-heuristic-validation skill.

use serde::Serialize;

use crate::decode::DecodeStatus;
use crate::mdct_grid::MdctGridAnalysis;
use crate::mp3_grid::Mp3GridAnalysis;
use crate::spectral::SpectralAnalysis;
use crate::tags::EncoderTagMatch;

const NO_EVIDENCE_CONFIDENCE: f64 = 0.3;
const GRID_ONLY_CONFIDENCE: f64 = 0.85;
const GRID_CORROBORATED_CONFIDENCE: f64 = 0.9;
/// Both codec searches clean. Capped: the residual risk (post-decode processing, other
/// codecs) is real and not measurable from the file.
const AUTHENTIC_CONFIDENCE: f64 = 0.7;
/// A sweep score under this reads as "no alignment stands out". Between this and the
/// detection threshold a search is neither a detection nor a clean result. On the
/// validation set, files without the codec in their history peaked at 4.5.
const GRID_CLEAR_Z: f64 = 6.0;
/// Highest Nyquist frequency at which the two codecs operate (48 kHz sampling). Above it a
/// lossy source must have been resampled, which erases the alignment both searches need,
/// so a clean result there says nothing.
const CODEC_MAX_NYQUIST_HZ: f64 = 24_000.0;
const STEEPNESS_TRANSCODE_THRESHOLD: f64 = 40.0;
const ABOVE_CD_CEILING_MIN_DB: f64 = -30.0;
const CD_NYQUIST_HZ: f64 = 22_050.0;
const MIN_SCANNED_KHZ: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Both codec-grid searches ran with full power and found nothing — a tested negative.
    /// Never inferred from a spectrum alone. See the module docs for what it cannot rule out.
    ProbablyAuthentic,
    ProbablyTranscoded,
    Indeterminate,
    /// The file is in a lossy format and says so. Not a verdict about deception — the
    /// question this module asks does not arise.
    ///
    /// The other three states answer "is this *lossless* file secretly lossy?". Running that
    /// question on an MP3 is a category error, and it produced a genuinely absurd answer: an
    /// ordinary MP3 came out "probably transcoded" at 80%, and an AAC file at 95% because
    /// `mdct_grid` correctly found the encoder grid that is *supposed* to be there. Nothing
    /// is hidden in either case. Every measurement is still reported; only the accusation is
    /// withdrawn.
    DeclaredLossy,
}

/// Codecs that are lossy by definition, so a file in one of them is not hiding anything.
///
/// An explicit list rather than "not a known lossless codec": the failure directions are not
/// symmetric. Wrongly calling a lossy codec lossless leaves the status quo — a nonsensical
/// verdict. Wrongly calling a *lossless* codec lossy would silently switch off the check on
/// a file that needs it, which is far worse. An unrecognized codec therefore keeps the full
/// assessment.
fn is_declared_lossy(codec: &str) -> bool {
    matches!(codec, "mp3" | "mp2" | "mp1" | "aac" | "vorbis" | "opus")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TranscodeAssessment {
    pub verdict: Verdict,
    /// Strength of the evidence behind the *stated* verdict, 0.0-1.0, or `None` where the
    /// verdict is not an inference at all ([`Verdict::DeclaredLossy`] — the container says
    /// so outright, and there is nothing to be more or less sure of).
    ///
    /// Deliberately capped well below 1.0 in every branch: this is a small set of
    /// indicators tuned on a small corpus, not a validated statistical classifier — see
    /// module docs. It is **not** a probability, and no surface should render it as one; the
    /// UI shows it as weak/moderate/strong evidence and keeps the number for the JSON
    /// report, where a reader can see what it is.
    pub confidence_score: Option<f64>,
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
        Self {
            message: detail.english(),
            detail,
        }
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
        /// Further *tags* beyond the one quoted, 0 when it was the only one. Counted by
        /// distinct `(key, value)` pair rather than by pattern hit: one tag naming an
        /// encoder twice ("LAME 3.100 (lame)") is one piece of evidence, not two.
        additional_tags: usize,
    },
    /// A lossy-encoder tag with no grid behind it: it neither accuses nor lets the file be
    /// cleared.
    TagIsOnlyEvidence,
    /// Silence or an analysis-floor spectrum supplies no spectral evidence.
    InsufficientSignal,
    IntegrityMismatch,
    /// Degenerate input: no usable sample rate, so no spectral claim can be made.
    InvalidSampleRate,
    /// A sharp edge, compatible with both native filtering and codec lowpasses.
    SharpRolloff {
        steepness_db_per_khz: f64,
        edge_khz: f64,
    },
    /// No sharp sustained edge found in the tested frequency range.
    NoEncoderLowpass {
        scanned_from_khz: f64,
        nyquist_khz: f64,
    },
    /// An edge exists but is too gradual to attribute to a codec either way.
    GradualRolloff {
        cutoff_khz: f64,
        steepness_db_per_khz: f64,
    },
    /// The file's own MDCT coefficients collapse at one specific frame alignment — an AAC
    /// encoder's grid. Structural evidence, independent of the spectral envelope.
    MdctGridAligned {
        z_score: f64,
        confirmed_z_score: f64,
        window: crate::mdct_grid::MdctWindow,
        frame_offset: usize,
        zero_percent: f64,
        baseline_percent: f64,
    },
    /// No alignment stands out under the tested AAC hypotheses.
    MdctGridClear,
    /// The file's coefficients, re-derived through MP3's own hybrid filterbank, are
    /// quantized at one specific granule alignment. Covers presets with no lowpass.
    Mp3GridAligned {
        z_score: f64,
        confirmed_z_score: f64,
        frame_offset: usize,
        sparse_percent: f64,
        baseline_percent: f64,
    },
    /// No granule alignment stands out under the MP3 long-block hypothesis.
    Mp3GridClear,
    /// A search scored between "nothing" and "confirmed": neither verdict is supported.
    GridInconclusive {
        codec: GridCodec,
        z_score: f64,
        confirmed_z_score: f64,
    },
    /// Too short or too quiet for a grid search, so there is no tested negative.
    GridSearchUnavailable,
    /// Both searches came back clean with full power. The positive statement behind
    /// [`Verdict::ProbablyAuthentic`], with its limits spelled out.
    NoCodecGridFound,
    /// Above the rates MP3 and AAC operate at, a clean search cannot clear a file.
    SampleRateOutsideCodecRange {
        sample_rate_khz: f64,
    },
    /// A brick-wall edge with no codec grid behind it: a mastering filter, or a codec the
    /// searches do not cover. Withholds the authentic verdict.
    UnexplainedSharpEdge {
        edge_khz: f64,
    },
    /// Energy above the CD ceiling, which could also have been added after transcoding.
    ContentAboveCdCeiling {
        level_db: f64,
        ceiling_khz: f64,
    },
    /// The container declares a lossy codec, so there is no disguise to see through.
    DeclaredLossyCodec {
        codec: String,
    },
    /// Part of the audio never reached the analysis, so no verdict can describe the file.
    DecodeIncomplete {
        skipped_packets: usize,
        stopped_early: bool,
        channels_unequal: bool,
    },
}

/// Which codec family a grid search was looking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GridCodec {
    Aac,
    Mp3,
}

impl IndicatorDetail {
    /// English report wording, shared by the CLI, exported JSON and English UI.
    fn english(&self) -> String {
        match self {
            Self::EncoderTagMatched { tag_key, tag_value, matched_pattern, additional_tags } =>
                format!("Encoder tag {tag_key}={tag_value:?} matches {matched_pattern:?} ({additional_tags} additional distinct tags). Metadata can be stale or edited."),
            Self::TagIsOnlyEvidence =>
                "The encoder tag is not backed by a codec grid in the audio. It does not establish a lossy history, and the file is not cleared while it stands.".into(),
            Self::InsufficientSignal =>
                "No usable spectral signal was measured. Silence cannot establish bandwidth or provenance.".into(),
            Self::IntegrityMismatch =>
                "The embedded audio checksum does not match the decoded samples. The provenance verdict is withheld.".into(),
            Self::InvalidSampleRate =>
                "Invalid sample rate; spectral provenance cannot be assessed.".into(),
            Self::SharpRolloff { steepness_db_per_khz, edge_khz } =>
                format!("A sharp spectral edge (~{steepness_db_per_khz:.0} dB/kHz) was measured near {edge_khz:.1} kHz. A lossy codec, a mastering filter and a resampler can all produce it; this observation alone does not identify the cause."),
            Self::NoEncoderLowpass { scanned_from_khz, nyquist_khz } =>
                format!("No qualifying sharp edge was detected in the tested range ({scanned_from_khz:.0}–{nyquist_khz:.1} kHz). This does not rule out a filter or a lossy source."),
            Self::GradualRolloff { cutoff_khz, steepness_db_per_khz } =>
                format!("A gradual edge was measured near {cutoff_khz:.1} kHz (~{steepness_db_per_khz:.0} dB/kHz). Its cause is not established."),
            Self::MdctGridAligned { z_score, confirmed_z_score, window, frame_offset, zero_percent, baseline_percent } =>
                format!("A 1024-coefficient MDCT grid ({window:?} window, offset {frame_offset}) was found and confirmed on separate audio frames: robust scores {z_score:.1} and {confirmed_z_score:.1}. Confirmation measured {zero_percent:.1}% near-zero coefficients against {baseline_percent:.1}% at control offsets. This is evidence consistent with AAC long-block quantization; the scores are not probabilities."),
            Self::MdctGridClear =>
                "No AAC grid: re-analyzed on AAC's long-block transform (sine and KBD windows), no frame alignment stands out.".into(),
            Self::Mp3GridAligned { z_score, confirmed_z_score, frame_offset, sparse_percent, baseline_percent } =>
                format!("An MP3 quantization grid was found at granule offset {frame_offset} and confirmed on separate audio: robust scores {z_score:.1} and {confirmed_z_score:.1}. Re-analyzed through MP3's own filterbank, {sparse_percent:.1}% of coefficients are quantized away at that alignment against {baseline_percent:.1}% at control offsets. This is evidence of an MP3 encode, including presets that keep the full bandwidth; the scores are not probabilities."),
            Self::Mp3GridClear =>
                "No MP3 grid: re-analyzed through MP3's hybrid filterbank at all 576 granule alignments, none stands out.".into(),
            Self::GridInconclusive { codec, z_score, confirmed_z_score } =>
                format!("The {} grid search is inconclusive (scores {z_score:.1} and {confirmed_z_score:.1}): above what unencoded audio produces, below a confirmed detection. Neither verdict is supported.", match codec { GridCodec::Aac => "AAC", GridCodec::Mp3 => "MP3" }),
            Self::GridSearchUnavailable =>
                "The file is too short or too quiet for the codec-grid searches, so no tested result is available.".into(),
            Self::NoCodecGridFound =>
                "Neither an MP3 nor an AAC quantization grid is present. These searches caught every MP3 and AAC round trip of real recordings in validation, including LAME V0. Not proof: audio resampled or processed after decoding, or a codec outside these two families, can pass.".into(),
            Self::SampleRateOutsideCodecRange { sample_rate_khz } =>
                format!("At {sample_rate_khz:.1} kHz the codec-grid searches cannot clear a file: MP3 and AAC sources stop at 48 kHz, so a lossy source would have been resampled, which erases the grid."),
            Self::UnexplainedSharpEdge { edge_khz } =>
                format!("The brick-wall edge near {edge_khz:.1} kHz has no MP3 or AAC grid behind it. A mastering or resampling filter explains it, and so does a codec these searches do not cover (Opus, Vorbis, WMA). The authentic verdict is withheld."),
            Self::ContentAboveCdCeiling { level_db, ceiling_khz } =>
                format!("The tested upper band above {ceiling_khz:.2} kHz measures {level_db:.0} dB relative to the reference band. Noise or processing after a lossy encode can create this content, so it does not establish authenticity."),
            Self::DeclaredLossyCodec { codec } =>
                format!("This file declares the lossy codec {}. The question of a lossy source hidden in a lossless file does not apply.", codec.to_uppercase()),
            Self::DecodeIncomplete { skipped_packets, stopped_early, channels_unequal } =>
                format!("The decode is incomplete: {skipped_packets} skipped packet(s), early stop: {stopped_early}, unequal channel lengths: {channels_unequal}. Measurements describe only the decoded portion; the provenance verdict is withheld."),
        }
    }
}

/// What one grid search contributes to the verdict.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GridOutcome {
    /// Too short or too quiet to search.
    Unavailable,
    /// Searched, nothing stands out.
    Clear,
    /// Above the noise of unencoded audio, below a confirmed detection.
    Inconclusive,
    /// Confirmed on separate audio.
    Detected,
}

fn grid_outcome(analyzed: bool, detected: bool, z: f64, confirmed_z: f64) -> GridOutcome {
    if !analyzed {
        GridOutcome::Unavailable
    } else if detected {
        GridOutcome::Detected
    } else if z < GRID_CLEAR_Z && confirmed_z < GRID_CLEAR_Z {
        GridOutcome::Clear
    } else {
        GridOutcome::Inconclusive
    }
}

/// Weighs the two codec-grid searches, with the spectrum and tags as context.
///
/// See the module docs for the policy. Every reason a verdict is reached or withheld is
/// pushed as an indicator; nothing is folded into the score silently.
pub fn assess_transcode_risk(
    spectral: &SpectralAnalysis,
    nyquist_hz: f64,
    encoder_tag_matches: &[EncoderTagMatch],
    mdct_grid: &MdctGridAnalysis,
    mp3_grid: &Mp3GridAnalysis,
    codec: &str,
    decode_status: &DecodeStatus,
) -> TranscodeAssessment {
    if is_declared_lossy(codec) {
        return TranscodeAssessment {
            verdict: Verdict::DeclaredLossy,
            confidence_score: None,
            indicators: vec![Indicator::new(IndicatorDetail::DeclaredLossyCodec {
                codec: codec.into(),
            })],
        };
    }

    let mut indicators = Vec::new();
    let mut push = |detail: IndicatorDetail| indicators.push(Indicator::new(detail));
    // Set by every reason the file cannot be cleared, each stated as its own indicator. A
    // partial decode is one: a clean search of a fragment does not clear the file.
    let mut withheld = !decode_status.complete;

    let sharp = spectral.rolloff_steepness_db_per_khz >= STEEPNESS_TRANSCODE_THRESHOLD;
    let sharp_edge_khz = spectral
        .encoder_edge_hz
        .filter(|_| sharp)
        .map(|hz| hz / 1000.0);
    let rate_valid = nyquist_hz.is_finite() && nyquist_hz > 0.0;
    if !rate_valid {
        push(IndicatorDetail::InvalidSampleRate);
        withheld = true;
    } else if !spectral.has_signal {
        push(IndicatorDetail::InsufficientSignal);
        withheld = true;
    } else {
        push(match (spectral.encoder_edge_hz, sharp_edge_khz) {
            (_, Some(edge_khz)) => IndicatorDetail::SharpRolloff {
                steepness_db_per_khz: spectral.rolloff_steepness_db_per_khz,
                edge_khz,
            },
            (Some(hz), None) => IndicatorDetail::GradualRolloff {
                cutoff_khz: hz / 1000.0,
                steepness_db_per_khz: spectral.rolloff_steepness_db_per_khz,
            },
            (None, None) => IndicatorDetail::NoEncoderLowpass {
                scanned_from_khz: MIN_SCANNED_KHZ.min(nyquist_hz / 1000.0),
                nyquist_khz: nyquist_hz / 1000.0,
            },
        });
        if let Some(level_db) = spectral
            .above_cd_ceiling_db
            .filter(|db| *db >= ABOVE_CD_CEILING_MIN_DB)
        {
            push(IndicatorDetail::ContentAboveCdCeiling {
                level_db,
                ceiling_khz: CD_NYQUIST_HZ / 1000.0,
            });
        }
    }

    let aac = grid_outcome(
        mdct_grid.analyzed && mdct_grid.window.is_some(),
        mdct_grid.grid_detected,
        mdct_grid.z_score,
        mdct_grid.confirmed_z_score,
    );
    let mp3 = grid_outcome(
        mp3_grid.analyzed,
        mp3_grid.grid_detected,
        mp3_grid.z_score,
        mp3_grid.confirmed_z_score,
    );

    match (aac, mdct_grid.window) {
        (GridOutcome::Detected, Some(window)) => push(IndicatorDetail::MdctGridAligned {
            z_score: mdct_grid.z_score,
            confirmed_z_score: mdct_grid.confirmed_z_score,
            window,
            frame_offset: mdct_grid.frame_offset,
            zero_percent: mdct_grid.zero_fraction_at_offset * 100.0,
            baseline_percent: mdct_grid.zero_fraction_baseline * 100.0,
        }),
        (GridOutcome::Clear, _) => push(IndicatorDetail::MdctGridClear),
        (GridOutcome::Inconclusive, _) => push(IndicatorDetail::GridInconclusive {
            codec: GridCodec::Aac,
            z_score: mdct_grid.z_score,
            confirmed_z_score: mdct_grid.confirmed_z_score,
        }),
        _ => {}
    }
    match mp3 {
        GridOutcome::Detected => push(IndicatorDetail::Mp3GridAligned {
            z_score: mp3_grid.z_score,
            confirmed_z_score: mp3_grid.confirmed_z_score,
            frame_offset: mp3_grid.frame_offset,
            sparse_percent: mp3_grid.sparse_fraction_at_offset * 100.0,
            baseline_percent: mp3_grid.sparse_fraction_baseline * 100.0,
        }),
        GridOutcome::Clear => push(IndicatorDetail::Mp3GridClear),
        GridOutcome::Inconclusive => push(IndicatorDetail::GridInconclusive {
            codec: GridCodec::Mp3,
            z_score: mp3_grid.z_score,
            confirmed_z_score: mp3_grid.confirmed_z_score,
        }),
        GridOutcome::Unavailable => {}
    }
    let detected = aac == GridOutcome::Detected || mp3 == GridOutcome::Detected;
    if aac == GridOutcome::Unavailable || mp3 == GridOutcome::Unavailable {
        push(IndicatorDetail::GridSearchUnavailable);
        withheld = true;
    }
    withheld |= aac == GridOutcome::Inconclusive || mp3 == GridOutcome::Inconclusive;

    if !detected && rate_valid && spectral.has_signal {
        if nyquist_hz > CODEC_MAX_NYQUIST_HZ {
            push(IndicatorDetail::SampleRateOutsideCodecRange {
                sample_rate_khz: 2.0 * nyquist_hz / 1000.0,
            });
            withheld = true;
        }
        if let Some(edge_khz) = sharp_edge_khz {
            push(IndicatorDetail::UnexplainedSharpEdge { edge_khz });
            withheld = true;
        }
    }

    if let Some(first) = encoder_tag_matches.first() {
        let distinct_tags = encoder_tag_matches
            .iter()
            .map(|tag| (tag.tag_key.as_str(), tag.tag_value.as_str()))
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        push(IndicatorDetail::EncoderTagMatched {
            tag_key: first.tag_key.clone(),
            tag_value: first.tag_value.clone(),
            matched_pattern: first.matched_pattern.clone(),
            additional_tags: distinct_tags.saturating_sub(1),
        });
        if !detected {
            // A tag naming a lossy encoder on audio with no grid is a conflict. It cannot
            // accuse the file, and the file cannot be cleared over it either.
            push(IndicatorDetail::TagIsOnlyEvidence);
            withheld = true;
        }
    }

    let (verdict, confidence) = if detected {
        let corroborated = sharp_edge_khz.is_some() || !encoder_tag_matches.is_empty();
        (
            Verdict::ProbablyTranscoded,
            if corroborated {
                GRID_CORROBORATED_CONFIDENCE
            } else {
                GRID_ONLY_CONFIDENCE
            },
        )
    } else if withheld {
        (Verdict::Indeterminate, NO_EVIDENCE_CONFIDENCE)
    } else {
        push(IndicatorDetail::NoCodecGridFound);
        (Verdict::ProbablyAuthentic, AUTHENTIC_CONFIDENCE)
    };

    let mut assessment = TranscodeAssessment {
        verdict,
        confidence_score: Some(confidence),
        indicators,
    };
    if !decode_status.complete {
        assessment.verdict = Verdict::Indeterminate;
        assessment.confidence_score = Some(NO_EVIDENCE_CONFIDENCE);
        assessment.indicators.insert(
            0,
            Indicator::new(IndicatorDetail::DecodeIncomplete {
                skipped_packets: decode_status.skipped_packets,
                stopped_early: decode_status.stopped_early,
                channels_unequal: decode_status.channels_unequal,
            }),
        );
    }
    assessment
}

impl TranscodeAssessment {
    /// Withdraws the provenance inference when the file's embedded checksum failed.
    pub fn withhold_on_integrity_failure(&mut self) {
        if self.verdict != Verdict::DeclaredLossy {
            self.verdict = Verdict::Indeterminate;
            self.confidence_score = Some(NO_EVIDENCE_CONFIDENCE);
            self.indicators
                .insert(0, Indicator::new(IndicatorDetail::IntegrityMismatch));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spectral::{BandLevel, SpectrogramData};

    #[test]
    fn a_checksum_failure_withdraws_even_a_structural_verdict() {
        let mut assessment = TranscodeAssessment {
            verdict: Verdict::ProbablyTranscoded,
            confidence_score: Some(GRID_ONLY_CONFIDENCE),
            indicators: Vec::new(),
        };
        assessment.withhold_on_integrity_failure();
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert_eq!(assessment.confidence_score, Some(NO_EVIDENCE_CONFIDENCE));
        assert!(matches!(
            assessment.indicators[0].detail,
            IndicatorDetail::IntegrityMismatch
        ));
    }

    fn spectral(
        encoder_edge_hz: Option<f64>,
        steepness: f64,
        cutoff: Option<f64>,
    ) -> SpectralAnalysis {
        SpectralAnalysis {
            has_signal: true,
            spectral_cutoff_hz: cutoff,
            rolloff_steepness_db_per_khz: steepness,
            encoder_edge_hz,
            cutoff_over_time_hz: Vec::new(),
            cutoff_stability_hz: 0.0,
            band_levels_db: Vec::<BandLevel>::new(),
            stopband_depth_db: None,
            above_cd_ceiling_db: None,
            spectrogram: SpectrogramData {
                time_bin_count: 0,
                frequency_bin_count: 0,
                max_frequency_hz: 22_050.0,
                duration_seconds: 0.0,
                intensity_base64: String::new(),
            },
        }
    }

    fn clear_grid() -> MdctGridAnalysis {
        MdctGridAnalysis {
            window: Some(crate::mdct_grid::MdctWindow::Sine),
            confirmed_z_score: 1.0,
            analyzed: true,
            grid_detected: false,
            z_score: 3.0,
            frame_offset: 0,
            zero_fraction_at_offset: 0.01,
            zero_fraction_baseline: 0.01,
            sweep_profile_base64: String::new(),
        }
    }

    fn clear_mp3() -> Mp3GridAnalysis {
        Mp3GridAnalysis {
            analyzed: true,
            grid_detected: false,
            z_score: 3.0,
            confirmed_z_score: 0.5,
            frame_offset: 0,
            sparse_fraction_at_offset: 0.01,
            sparse_fraction_baseline: 0.01,
            sweep_profile_base64: String::new(),
        }
    }

    fn mp3_at(z_score: f64, confirmed_z_score: f64) -> Mp3GridAnalysis {
        Mp3GridAnalysis {
            grid_detected: z_score >= crate::mp3_grid::GRID_DETECTION_Z
                && confirmed_z_score >= crate::mp3_grid::GRID_DETECTION_Z,
            z_score,
            confirmed_z_score,
            frame_offset: 528,
            sparse_fraction_at_offset: 0.12,
            ..clear_mp3()
        }
    }

    fn complete() -> DecodeStatus {
        DecodeStatus {
            complete: true,
            skipped_packets: 0,
            stopped_early: false,
            channels_unequal: false,
        }
    }

    fn has(assessment: &TranscodeAssessment, wanted: fn(&IndicatorDetail) -> bool) -> bool {
        assessment.indicators.iter().any(|i| wanted(&i.detail))
    }

    /// The verdict this module exists to be able to give: both codec searches ran and found
    /// nothing, at a rate those codecs work at, with nothing contradicting it.
    #[test]
    fn two_clean_grid_searches_clear_a_cd_rate_file() {
        let assessment = assess_transcode_risk(
            &spectral(None, 0.0, None),
            22_050.0,
            &[],
            &clear_grid(),
            &clear_mp3(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::ProbablyAuthentic);
        assert_eq!(assessment.confidence_score, Some(AUTHENTIC_CONFIDENCE));
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::NoCodecGridFound
        )));
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::Mp3GridClear
        )));
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::MdctGridClear
        )));
    }

    /// The failure the previous policy was built to prevent must stay prevented: a clean
    /// *spectrum* is not a tested negative. Without the grid searches there is no verdict.
    #[test]
    fn a_clean_spectrum_without_grid_searches_clears_nothing() {
        let unavailable = Mp3GridAnalysis {
            analyzed: false,
            ..clear_mp3()
        };
        let assessment = assess_transcode_risk(
            &spectral(None, 0.0, None),
            22_050.0,
            &[],
            &clear_grid(),
            &unavailable,
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::GridSearchUnavailable
        )));
    }

    /// LAME V0 keeps the full bandwidth, so the spectrum is spotless. The grid still accuses.
    #[test]
    fn an_mp3_grid_accuses_a_file_with_a_spotless_spectrum() {
        let assessment = assess_transcode_risk(
            &spectral(None, 0.0, None),
            22_050.0,
            &[],
            &clear_grid(),
            &mp3_at(31.8, 57.3),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::ProbablyTranscoded);
        assert_eq!(assessment.confidence_score, Some(GRID_ONLY_CONFIDENCE));
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::Mp3GridAligned { .. }
        )));
        assert!(!has(&assessment, |d| matches!(
            d,
            IndicatorDetail::NoCodecGridFound
        )));
    }

    /// Between "nothing" and "confirmed" neither verdict is supported, in either direction.
    #[test]
    fn a_borderline_grid_score_neither_accuses_nor_clears() {
        for (z, confirmed) in [(9.0, 9.0), (30.0, 8.0), (4.0, 9.0)] {
            let assessment = assess_transcode_risk(
                &spectral(None, 0.0, None),
                22_050.0,
                &[],
                &clear_grid(),
                &mp3_at(z, confirmed),
                "flac",
                &complete(),
            );
            assert_eq!(
                assessment.verdict,
                Verdict::Indeterminate,
                "z {z}/{confirmed}"
            );
            assert!(has(&assessment, |d| matches!(
                d,
                IndicatorDetail::GridInconclusive {
                    codec: GridCodec::Mp3,
                    ..
                }
            )));
        }
    }

    /// Above 48 kHz a lossy source was necessarily resampled, which erases the grid. A clean
    /// search there is not a tested negative — and ultrasonic content can be added after an
    /// encode, so it does not rescue the verdict either.
    #[test]
    fn a_hi_res_rate_is_never_cleared_by_clean_grids() {
        let mut sp = spectral(None, 0.0, None);
        sp.above_cd_ceiling_db = Some(-5.0);
        let assessment = assess_transcode_risk(
            &sp,
            48_000.0,
            &[],
            &clear_grid(),
            &clear_mp3(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::ContentAboveCdCeiling { .. }
        )));
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::SampleRateOutsideCodecRange { .. }
        )));
    }

    /// A brick wall with no grid behind it is what Opus or Vorbis would leave, and also what
    /// a mastering filter leaves. It accuses nothing and it blocks the clean verdict.
    #[test]
    fn an_unexplained_brick_wall_withholds_the_authentic_verdict() {
        let assessment = assess_transcode_risk(
            &spectral(Some(16_000.0), 90.0, Some(16_000.0)),
            22_050.0,
            &[],
            &clear_grid(),
            &clear_mp3(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::UnexplainedSharpEdge { .. }
        )));
    }

    /// A gradual rolloff is how dark masters look. It must not cost a file its verdict.
    #[test]
    fn a_gradual_rolloff_does_not_block_the_authentic_verdict() {
        let assessment = assess_transcode_risk(
            &spectral(Some(17_000.0), 12.0, Some(17_000.0)),
            22_050.0,
            &[],
            &clear_grid(),
            &clear_mp3(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::ProbablyAuthentic);
    }

    /// A tag naming a lossy encoder on audio with no grid is a conflict, not a confession:
    /// it must not accuse the file, and the file must not be cleared over it.
    #[test]
    fn encoder_metadata_alone_never_accuses_a_file() {
        let tags = [EncoderTagMatch {
            tag_key: "ENCODER".into(),
            tag_value: "LAME 3.100".into(),
            matched_pattern: "lame".into(),
        }];
        let assessment = assess_transcode_risk(
            &spectral(None, 0.0, None),
            22_050.0,
            &tags,
            &clear_grid(),
            &clear_mp3(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert_eq!(assessment.confidence_score, Some(NO_EVIDENCE_CONFIDENCE));
        assert!(has(&assessment, |d| matches!(
            d,
            IndicatorDetail::TagIsOnlyEvidence
        )));
    }

    /// One tag matching two patterns is one piece of evidence. It used to be reported as
    /// "plus 1 more matching tag" on a file carrying exactly one.
    #[test]
    fn repeated_patterns_in_one_tag_count_once() {
        let tags = [
            EncoderTagMatch {
                tag_key: "ENCODER".into(),
                tag_value: "LAME 3.100 (lame)".into(),
                matched_pattern: "lame".into(),
            },
            EncoderTagMatch {
                tag_key: "ENCODER".into(),
                tag_value: "LAME 3.100 (lame)".into(),
                matched_pattern: "lame3".into(),
            },
        ];
        let assessment = assess_transcode_risk(
            &spectral(Some(16_000.0), 90.0, Some(16_000.0)),
            22_050.0,
            &tags,
            &clear_grid(),
            &mp3_at(60.0, 80.0),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::ProbablyTranscoded);
        assert_eq!(
            assessment.confidence_score,
            Some(GRID_CORROBORATED_CONFIDENCE)
        );
        let additional = assessment.indicators.iter().find_map(|i| match i.detail {
            IndicatorDetail::EncoderTagMatched {
                additional_tags, ..
            } => Some(additional_tags),
            _ => None,
        });
        assert_eq!(
            additional,
            Some(0),
            "two patterns in one tag is still one tag"
        );
    }

    /// A verdict describes a file. When part of the file never decoded, there is no file to
    /// describe — the measurements stand, the claim does not. In either direction.
    #[test]
    fn an_incomplete_decode_withholds_every_verdict() {
        let damaged = DecodeStatus {
            complete: false,
            skipped_packets: 12,
            stopped_early: false,
            channels_unequal: true,
        };
        for mp3 in [mp3_at(60.0, 80.0), clear_mp3()] {
            let assessment = assess_transcode_risk(
                &spectral(None, 0.0, None),
                22_050.0,
                &[],
                &clear_grid(),
                &mp3,
                "flac",
                &damaged,
            );
            assert_eq!(assessment.verdict, Verdict::Indeterminate);
            assert_eq!(assessment.confidence_score, Some(NO_EVIDENCE_CONFIDENCE));
            assert!(matches!(
                assessment.indicators.first().map(|i| &i.detail),
                Some(IndicatorDetail::DecodeIncomplete {
                    skipped_packets: 12,
                    channels_unequal: true,
                    ..
                })
            ));
        }
    }

    /// Silence has nothing to search, so it has nothing to clear.
    #[test]
    fn silence_is_never_cleared() {
        let mut sp = spectral(None, 0.0, None);
        sp.has_signal = false;
        let assessment = assess_transcode_risk(
            &sp,
            22_050.0,
            &[],
            &clear_grid(),
            &clear_mp3(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
    }

    /// A lossy container is not accused of hiding anything, and its verdict carries no
    /// confidence figure — there is nothing to be more or less sure of.
    #[test]
    fn a_declared_lossy_file_carries_no_confidence_score() {
        let assessment = assess_transcode_risk(
            &spectral(Some(16_000.0), 95.0, Some(16_000.0)),
            22_050.0,
            &[],
            &clear_grid(),
            &mp3_at(60.0, 80.0),
            "mp3",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::DeclaredLossy);
        assert_eq!(assessment.confidence_score, None);
    }
}
