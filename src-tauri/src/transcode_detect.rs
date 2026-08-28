//! Lossy-to-lossless transcode likelihood scoring. See
//! `.claude/skills/transcode-heuristic-validation/SKILL.md` before touching this file —
//! this is the highest-stakes code in the project: a wrong verdict either discredits a
//! legitimate file or vouches for a fake one.
//!
//! **Never a binary yes/no.** Always a 4-state verdict — `ProbablyAuthentic`,
//! `ProbablyTranscoded`, `Indeterminate`, `DeclaredLossy` — with a bounded, honest
//! confidence weight and a human-readable list of what evidence produced it.
//!
//! ## An absence of evidence is not evidence
//!
//! The distinction the rest of this module is organized around: finding no encoder
//! fingerprint says the spectrum is clean, not that the file is lossless. `ProbablyAuthentic`
//! therefore requires *positive* evidence — currently one thing, real content in the top of a
//! hi-res band, which no CD-rate lossy encode could have put there. Everything else that
//! comes back clean produces `Indeterminate`.
//!
//! That is a stronger constraint than it sounds, and it was added because the previous
//! version returned `ProbablyAuthentic` at 0.65 for the two LAME V0 transcodes in this
//! project's own corpus. Missing a transcode is a limit of the method; *vouching* for one is
//! the tool telling the user a lie it was built to prevent.
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
//! ## Known blind spot, and what closed half of it
//!
//! A transparent lossy encode does not reliably lowpass at all, so the rolloff measurement
//! above cannot see one. That used to mean LAME V0 *and* AAC 256 came out
//! `ProbablyAuthentic` — the tool vouching for a transcode rather than merely missing it,
//! which is the worst thing it can do. Both halves of that are now fixed: AAC by the grid
//! sweep below, LAME V0 by the rule above that a clean sweep alone reaches no verdict.
//!
//! **The AAC half is now covered**, by `mdct_grid.rs`, which reads a different property
//! entirely: an AAC encoder quantizes on a 1024-point MDCT, and re-analysing the decoded
//! signal at the encoder's own frame alignment recovers the coefficients it zeroed. That is
//! structural, not statistical, so it survives a transparent encode. Across the corpus it
//! separates by a factor of eleven — 12 authentic fixtures at z ≤ 4.7, three AAC transcodes
//! at 79, 132 and 215, all three agreeing on frame offset 960.
//!
//! **The MP3 half is still open, and cannot be closed the same way.** MP3 uses a hybrid
//! filterbank — a 32-band polyphase stage feeding an 18-point MDCT per band — which a plain
//! MDCT does not invert at any size. LAME V0 remains the one case in this corpus that is
//! transcoded and reads authentic. Since MP3 is at least as common a source of fake-lossless
//! files as AAC, most of the practical risk survives, and [`GRID_CLEAR_BONUS`] is sized to
//! say so rather than to celebrate.
//!
//! The blind spot is also wider than bitrate alone suggests. On non-stationary material AAC
//! at *128* kbps escapes the rolloff test too — 18.3 kHz at only 27 dB/kHz, under
//! [`STEEPNESS_TRANSCODE_THRESHOLD`] — where the same encoder on flat noise reads
//! ~106 dB/kHz. What decides is the program material, not the setting. The grid sweep
//! catches that case regardless, which is exactly the point of having an indicator that does
//! not read the envelope.
//!
//! Three other candidates were prototyped against the corpus and rejected: spectral holes
//! (flags a legitimate in-the-box piano harder than a real LAME transcode), the codec frame
//! grid measured in the time domain (no signal — TDAC overlap smooths it away), and
//! joint-stereo collapse (no separation at all). Their measurements are recorded in
//! `corpus/README.md` so the next attempt does not repeat them.

use serde::Serialize;

use crate::decode::DecodeStatus;
use crate::mdct_grid::MdctGridAnalysis;
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
/// Confidence in "probably authentic" once positive evidence has been found on top of a
/// clean sweep. Flat rather than scaled by bandwidth: the evidence is categorical, and the
/// ceiling is set by the known blind spot in the module docs, not by how much of Nyquist the
/// content happens to occupy.
///
/// An earlier version scaled this by cutoff/Nyquist and required that ratio to reach 0.92.
/// That silently made the branch unreachable for real music, whose peak-relative cutoff sits
/// around 5 kHz, so every genuine lossless file fell through to `Indeterminate` at 30%.
const AUTHENTIC_BASE_CONFIDENCE: f64 = 0.6;
/// Confidence attached to the `Indeterminate` verdict returned when the sweep found no
/// encoder lowpass and nothing else positively vouched for the file.
///
/// Low on purpose, and the same number as the gradual-rolloff case, because it means the
/// same thing: not enough was established to say. A clean sweep used to return
/// `ProbablyAuthentic` at 0.65 here, which had the tool actively vouching for the two LAME
/// V0 transcodes in this project's own corpus — the worst failure mode it has, since a
/// transparent MP3 encode is precisely what this method cannot see. See "Known blind spot".
const NO_EVIDENCE_CONFIDENCE: f64 = 0.3;
/// Confidence when the MDCT grid fires on top of an already-"transcoded" spectral verdict.
/// The two are structurally independent — one reads the envelope, the other the coefficient
/// alignment — so agreement between them is worth more than either alone.
const GRID_CORROBORATED_CONFIDENCE: f64 = 0.95;
/// Confidence when the grid is the only evidence. Still high: unlike a tag, this is a
/// measurement of the audio, and a lossless file landing on a 1024-sample MDCT grid by
/// chance is not something the corpus shows any sign of (12 authentic fixtures peak at
/// z = 4.7 against a threshold of 20). Held under 1.0 because the corpus is small and only
/// covers one AAC encoder.
const GRID_ONLY_CONFIDENCE: f64 = 0.9;
/// Added to an "authentic" verdict when the grid sweep ran and found nothing.
///
/// Deliberately small. A clean sweep rules out AAC, which is the most common source of
/// fake-lossless files bought from a store — but it says nothing at all about MP3, and
/// `mdct_grid.rs` cannot be made to. Since LAME V0 is at least as common in the wild as
/// AAC 256, most of the blind spot survives, and the number should reflect that rather than
/// reward the half of the problem that was solved.
const GRID_CLEAR_BONUS: f64 = 0.05;
/// Ceiling on a "probably authentic" verdict however much corroboration accumulates. The
/// verdict still rests in part on an absence of evidence, and the MP3 blind spot is still
/// open.
const AUTHENTIC_MAX_CONFIDENCE: f64 = 0.7;
/// Added when content runs above the 22.05 kHz ceiling a CD-sourced lossy encode could
/// carry. Independent of the grid sweep: no MP3 exists at a sample rate high enough to
/// reach there, so that whole transcode path is ruled out by measurement rather than by
/// absence. Small for the same reason as [`GRID_CLEAR_BONUS`] — it narrows the space of
/// possible lies, it does not prove the file honest.
const ABOVE_CD_BANDWIDTH_BONUS: f64 = 0.05;
/// Highest frequency any 44.1 kHz source can carry. Content above this cannot have come
/// through a CD-rate lossy encode.
const CD_NYQUIST_HZ: f64 = 22_050.0;
/// How loud the top of the declared band must be, relative to the 1 kHz-22.05 kHz reference
/// band, before it counts as evidence of a genuine hi-res source. See
/// `spectral::SpectralAnalysis::above_cd_ceiling_db` for exactly which band is measured.
///
/// A ceiling test that only asked "does anything reach above 22.05 kHz" backfires on
/// upsampled files: resampling 44.1 kHz material to 96 kHz leaves anti-imaging ringing up
/// around 25 kHz — above the ceiling, yet produced by a CD-rate source, which is exactly the
/// inference this is meant to license.
///
/// Measured on this project's corpus: genuine hi-res at -0.03 dB (full-band noise) and
/// -18.6 dB (music-like), against -47.7 dB for the upsampled fixture and -64.2 dB for the
/// upsampled transcode. Set to leave 11 dB of margin under the weaker genuine case and 18 dB
/// over the stronger fake — and the gap is structural, not lucky: an upsampler's anti-imaging
/// filter leaves the top of the new band empty by construction.
const ABOVE_CD_CEILING_MIN_DB: f64 = -30.0;
/// Lower bound of the scanned range, in kHz, quoted to the user so the claim states its own
/// scope. Mirrors `spectral::MIN_PLAUSIBLE_ENCODER_CUTOFF_HZ`.
const MIN_SCANNED_KHZ: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
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
    /// Caveat attached when the spectrum was inconclusive and the tag carries the verdict.
    TagIsOnlyEvidence,
    /// Caveat attached when a lossy encoder tag contradicts a full-bandwidth spectrum.
    TagContradictsSpectrum,
    /// Degenerate input: no usable sample rate, so no spectral claim can be made.
    InvalidSampleRate,
    /// A rolloff steep enough to read as a codec's lowpass rather than mix/mastering.
    SharpRolloff {
        steepness_db_per_khz: f64,
        edge_khz: f64,
    },
    /// The scan ran the whole plausible encoder range and found no lowpass anywhere.
    NoEncoderLowpass {
        scanned_from_khz: f64,
        nyquist_khz: f64,
    },
    /// The blind spot from the module docs, stated alongside every "authentic" verdict.
    TransparentEncodeUnseen,
    /// An edge exists but is too gradual to attribute to a codec either way.
    GradualRolloff {
        cutoff_khz: f64,
        steepness_db_per_khz: f64,
    },
    /// The file's own MDCT coefficients collapse at one specific frame alignment — an AAC
    /// encoder's grid. Structural evidence, independent of the spectral envelope.
    MdctGridAligned {
        z_score: f64,
        frame_offset: usize,
        zero_percent: f64,
        baseline_percent: f64,
    },
    /// The grid sweep ran and found no alignment: AAC is ruled out, MP3 is not.
    MdctGridClear,
    /// The band above the CD ceiling carries real content, which no CD-rate lossy encode
    /// could have put there.
    ContentAboveCdCeiling { level_db: f64, ceiling_khz: f64 },
    /// The container declares a lossy codec, so there is no disguise to see through.
    DeclaredLossyCodec { codec: String },
    /// Part of the audio never reached the analysis, so no verdict can describe the file.
    DecodeIncomplete {
        skipped_packets: usize,
        stopped_early: bool,
    },
}

impl IndicatorDetail {
    /// The reference wording for this observation. Single source of the English prose:
    /// the CLI, the JSON export and the English UI all end up rendering this exact string.
    fn english(&self) -> String {
        match self {
            Self::EncoderTagMatched {
                tag_key,
                tag_value,
                matched_pattern,
                additional_tags,
            } => {
                format!(
                    "Encoder tag \"{}\" reads \"{}\", matching the lossy-only encoder \"{}\"{}. \
                     Note that tags stored at the end of a file (ID3v1, APEv2) are not read, \
                     so an absent tag is not evidence either way.",
                    tag_key,
                    tag_value,
                    matched_pattern,
                    if *additional_tags > 0 {
                        format!(", plus {additional_tags} more matching tag(s)")
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
            Self::SharpRolloff {
                steepness_db_per_khz,
                edge_khz,
            } => format!(
                "Sharp spectral rolloff (~{steepness_db_per_khz:.0} dB/kHz) around \
                 {edge_khz:.1} kHz — steep enough to match a lossy encoder's lowpass filter \
                 rather than natural mix/mastering content (natural rolloff measured well \
                 under 20 dB/kHz across this project's test corpus, real and synthetic)."
            ),
            Self::NoEncoderLowpass {
                scanned_from_khz,
                nyquist_khz,
            } => format!(
                "No encoder lowpass found: the spectrum was scanned from \
                 {scanned_from_khz:.0} kHz to the {nyquist_khz:.1} kHz Nyquist frequency and \
                 no point showed the sharp drop into a sustained empty band that a lossy \
                 codec leaves behind."
            ),
            Self::TransparentEncodeUnseen => "That is not evidence of authenticity. A \
                 transparent lossy encode (e.g. LAME V0) does not lowpass at all, and this \
                 project's own corpus shows those measuring indistinguishable from lossless \
                 by this method — so an absent cutoff is equally consistent with a careful \
                 MP3 transcode. No indicator of transcoding was detected; that is a \
                 different statement from the file being lossless."
                .to_string(),
            Self::MdctGridAligned {
                z_score,
                frame_offset,
                zero_percent,
                baseline_percent,
            } => format!(
                "The file's own MDCT coefficients collapse at one specific frame alignment \
                 (offset {frame_offset}, {z_score:.0} standard deviations above this file's \
                 own behaviour at every other offset): {zero_percent:.1}% of coefficients \
                 read as zeroed there against {baseline_percent:.1}% elsewhere. That is an \
                 AAC encoder's quantization grid. Lossless audio has no such alignment."
            ),
            Self::MdctGridClear => "The MDCT grid sweep found no encoder alignment, which \
                 rules out an AAC source — including the transparent settings a spectral \
                 measurement cannot see. It says nothing about MP3, whose hybrid filterbank \
                 this test cannot invert, so the blind spot narrows rather than closes."
                .to_string(),
            Self::DeclaredLossyCodec { codec } => format!(
                "This file is {}, which is a lossy format. It is not pretending to be \
                 anything else, so there is no transcode to detect: the question this \
                 verdict answers — whether a lossless container is hiding lossy audio — does \
                 not apply. Every measurement below still describes the file accurately, \
                 including the encoder's own lowpass and frame grid.",
                codec.to_uppercase()
            ),
            Self::ContentAboveCdCeiling {
                level_db,
                ceiling_khz,
            } => format!(
                "The band above {ceiling_khz:.2} kHz carries real content — {level_db:.0} dB \
                 relative to the band below it. No MP3 or other CD-rate lossy encode exists \
                 at a sample rate high enough to put it there, so that whole transcode path \
                 is ruled out by measurement rather than by absence of evidence. This is the \
                 only positive evidence of authenticity in this report."
            ),
            Self::DecodeIncomplete {
                skipped_packets,
                stopped_early,
            } => {
                let what = match (*skipped_packets, *stopped_early) {
                    (0, _) => "the stream asked to be restarted part-way through (chained \
                               segments or a format change) and decoding stopped there"
                        .to_string(),
                    (n, false) => format!("{n} packet(s) could not be decoded and were skipped"),
                    (n, true) => format!(
                        "{n} packet(s) could not be decoded and were skipped, and the stream \
                         then asked to be restarted part-way through and decoding stopped there"
                    ),
                };
                format!(
                    "Part of the audio never reached the analysis: {what}. Every measurement \
                     below describes only the portion that decoded, so no verdict about the \
                     file as a whole can be given. Repair or re-rip the file and analyse it \
                     again."
                )
            }
            Self::GradualRolloff {
                cutoff_khz,
                steepness_db_per_khz,
            } => format!(
                "Content stops around {cutoff_khz:.1} kHz, but the transition there is \
                 gradual (~{steepness_db_per_khz:.0} dB/kHz) rather than the near-vertical \
                 wall a codec produces. That is consistent with a deliberately dark master, \
                 a vinyl or tape transfer, or a lossy encode whose filter this method cannot \
                 separate from those — not enough to call it either way."
            ),
        }
    }
}

/// What the spectral pass established, carried alongside the verdict so later evidence can
/// tell "the sweep ran and found no lowpass" from "nothing could be established".
///
/// The two both produce `Indeterminate` now, and they must not be treated alike: a residual
/// encoder tag on a file whose spectrum positively shows no lowpass is a *conflict*, while
/// the same tag on a file the spectrum could say nothing about is the only evidence there is.
#[derive(PartialEq, Clone, Copy)]
enum SpectralOutcome {
    /// The sweep ran the whole plausible range and found no encoder lowpass.
    NoLowpass,
    /// An edge was found, steep enough to be a codec's.
    CodecLowpass,
    /// A gradual edge, or a sample rate that makes the question meaningless.
    Inconclusive,
}

pub fn assess_transcode_risk(
    spectral: &SpectralAnalysis,
    nyquist_hz: f64,
    encoder_tag_matches: &[EncoderTagMatch],
    mdct_grid: &MdctGridAnalysis,
    codec: &str,
    decode_status: &DecodeStatus,
) -> TranscodeAssessment {
    // Short-circuit before any of the evidence below is weighed. None of it is *wrong* on a
    // lossy file — a lowpass and an encoder grid really are there — but all of it would be
    // answering a question nobody asked.
    if is_declared_lossy(codec) {
        return TranscodeAssessment {
            verdict: Verdict::DeclaredLossy,
            // Not an inference, so not a probability: the container states this outright.
            // `None` rather than a confident 1.0, which read as the same kind of claim the
            // other three verdicts make and would have to be explained away in every
            // surface that renders it.
            confidence_score: None,
            indicators: vec![Indicator::new(IndicatorDetail::DeclaredLossyCodec {
                codec: codec.to_string(),
            })],
        };
    }

    let (mut assessment, outcome) = assess_from_spectrum(spectral, nyquist_hz);
    apply_mdct_grid_evidence(&mut assessment, outcome, mdct_grid);
    apply_tag_evidence(&mut assessment, outcome, encoder_tag_matches);
    withhold_on_incomplete_decode(&mut assessment, decode_status);
    assessment
}

/// A verdict describes a file. When part of the file never reached the decoder, the
/// measurements describe a fragment, and a verdict drawn from them would be a claim about
/// the whole made from a part.
///
/// Applied last, over every other line of evidence, and it only ever *removes* a claim: the
/// measurements stay, stated as measurements, and the accusation or the endorsement is
/// withdrawn. A truncated FLAC used to come out with an ordinary verdict at an ordinary
/// confidence, with the damage visible only as a packet count in another section.
fn withhold_on_incomplete_decode(assessment: &mut TranscodeAssessment, status: &DecodeStatus) {
    if status.complete {
        return;
    }
    assessment.verdict = Verdict::Indeterminate;
    assessment.confidence_score = Some(NO_EVIDENCE_CONFIDENCE);
    assessment.indicators.insert(
        0,
        Indicator::new(IndicatorDetail::DecodeIncomplete {
            skipped_packets: status.skipped_packets,
            stopped_early: status.stopped_early,
        }),
    );
}

/// The MDCT grid is the only indicator here allowed to overturn a spectral "authentic", and
/// the asymmetry is deliberate.
///
/// `apply_tag_evidence` below refuses to do that, because a tag is a string some unrelated
/// program wrote once and no tool has checked since. The grid is not a claim about the file,
/// it is a measurement *of* the file: an alignment at which its own coefficients collapse,
/// scored against its own behaviour at the other 1023 offsets. When that fires against a
/// spectrum that found no lowpass, the two are not in conflict — the spectrum found nothing
/// because a transparent encode leaves nothing there to find, which is exactly the blind
/// spot this indicator exists to cover.
///
/// A clean grid result is *weak* evidence the other way, and is scored as such: it rules out
/// AAC, which is a real narrowing, but says nothing about LAME. See [`GRID_CLEAR_BONUS`].
fn apply_mdct_grid_evidence(
    assessment: &mut TranscodeAssessment,
    outcome: SpectralOutcome,
    grid: &MdctGridAnalysis,
) {
    if !grid.analyzed {
        return;
    }

    if grid.grid_detected {
        assessment
            .indicators
            .push(Indicator::new(IndicatorDetail::MdctGridAligned {
                z_score: grid.z_score,
                frame_offset: grid.frame_offset,
                zero_percent: grid.zero_fraction_at_offset * 100.0,
                baseline_percent: grid.zero_fraction_baseline * 100.0,
            }));
        assessment.confidence_score = Some(match assessment.verdict {
            // Two structurally independent signals agreeing: a lowpass in the envelope and a
            // frame grid in the coefficients.
            Verdict::ProbablyTranscoded => assessment
                .confidence_score
                .unwrap_or(0.0)
                .max(GRID_CORROBORATED_CONFIDENCE),
            _ => {
                // The spectral branch may have attached the transparent-encode caveat, whose
                // whole point is that an absent cutoff proves nothing because this case
                // cannot be seen. It just was seen, so leaving the sentence in would have the
                // verdict argue against itself. The lowpass measurement itself stays: it is
                // still true, and it is what explains why the envelope alone missed this file.
                assessment
                    .indicators
                    .retain(|i| !matches!(i.detail, IndicatorDetail::TransparentEncodeUnseen));
                assessment.verdict = Verdict::ProbablyTranscoded;
                GRID_ONLY_CONFIDENCE
            }
        });
        return;
    }

    // Only worth stating where the user is being asked to weigh an absence: a clean sweep
    // rules out AAC, which narrows the known blind spot to MP3. It is not enough on its own
    // to move a verdict — LAME is at least as common a source of fake-lossless files — so on
    // an `Indeterminate` it is stated and the verdict stands.
    if assessment.verdict == Verdict::ProbablyAuthentic {
        assessment
            .indicators
            .push(Indicator::new(IndicatorDetail::MdctGridClear));
        assessment.confidence_score = Some(
            (assessment.confidence_score.unwrap_or(0.0) + GRID_CLEAR_BONUS)
                .min(AUTHENTIC_MAX_CONFIDENCE),
        );
    } else if outcome == SpectralOutcome::NoLowpass {
        assessment
            .indicators
            .push(Indicator::new(IndicatorDetail::MdctGridClear));
    }
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
fn apply_tag_evidence(
    assessment: &mut TranscodeAssessment,
    outcome: SpectralOutcome,
    matches: &[EncoderTagMatch],
) {
    let Some(first) = matches.first() else { return };

    // Counted as *tags*, not as pattern hits. The scan tries every known encoder name
    // against every tag, so one `ENCODER=LAME 3.100 (lame)` string used to produce two
    // matches and report "plus 1 more matching tag" for a file that carried exactly one.
    let distinct_tags = matches
        .iter()
        .map(|m| (m.tag_key.as_str(), m.tag_value.as_str()))
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    assessment
        .indicators
        .push(Indicator::new(IndicatorDetail::EncoderTagMatched {
            tag_key: first.tag_key.clone(),
            tag_value: first.tag_value.clone(),
            matched_pattern: first.matched_pattern.clone(),
            additional_tags: distinct_tags.saturating_sub(1),
        }));

    match assessment.verdict {
        // Two independent signals agreeing is stronger than either alone.
        Verdict::ProbablyTranscoded => {
            assessment.confidence_score = Some(
                assessment
                    .confidence_score
                    .unwrap_or(0.0)
                    .max(TAG_MATCH_CORROBORATING_CONFIDENCE),
            );
        }
        // Direct conflict: the sweep positively established there is no encoder lowpass,
        // and the metadata names a lossy tool. Whether the spectrum or the tag is stale, the
        // honest answer is that they disagree — letting the tag win outright meant a
        // leftover encoder string turned a clean measurement into a 75% accusation.
        //
        // Reached from `Indeterminate` as well as `ProbablyAuthentic` since a clean sweep on
        // its own no longer vouches for a file; the conflict is with the *measurement*, not
        // with the verdict it produced.
        _ if outcome == SpectralOutcome::NoLowpass => {
            assessment.verdict = Verdict::Indeterminate;
            assessment.confidence_score = Some(TAG_CONFLICT_CONFIDENCE);
            assessment
                .indicators
                .push(Indicator::new(IndicatorDetail::TagContradictsSpectrum));
        }
        // The spectrum could not settle it either way; the tag is then the only evidence
        // there is, and it only ever points one direction.
        Verdict::Indeterminate => {
            assessment.verdict = Verdict::ProbablyTranscoded;
            assessment.confidence_score = Some(TAG_MATCH_ONLY_CONFIDENCE);
            assessment
                .indicators
                .push(Indicator::new(IndicatorDetail::TagIsOnlyEvidence));
        }
        // Unreachable: `assess_transcode_risk` returns before any evidence is applied when
        // the codec is lossy. Spelled out rather than folded into a catch-all so that adding
        // a fifth verdict is a compile error here instead of a silent fall-through.
        Verdict::DeclaredLossy => {}
        // `ProbablyAuthentic` is only reachable with `SpectralOutcome::NoLowpass`, which the
        // conflict arm above already took.
        Verdict::ProbablyAuthentic => {}
    }
}

fn assess_from_spectrum(
    spectral: &SpectralAnalysis,
    nyquist_hz: f64,
) -> (TranscodeAssessment, SpectralOutcome) {
    if nyquist_hz <= 0.0 {
        return (
            TranscodeAssessment {
                verdict: Verdict::Indeterminate,
                confidence_score: Some(0.0),
                indicators: vec![Indicator::new(IndicatorDetail::InvalidSampleRate)],
            },
            SpectralOutcome::Inconclusive,
        );
    }

    let steepness = spectral.rolloff_steepness_db_per_khz;
    // Where the wall is, when there is one. Falls back to the measured bandwidth so the
    // wording still names a frequency in the degenerate case.
    let measured_cutoff_hz = spectral.spectral_cutoff_hz.unwrap_or(nyquist_hz);
    let edge_khz = spectral.encoder_edge_hz.unwrap_or(measured_cutoff_hz) / 1000.0;

    if steepness >= STEEPNESS_TRANSCODE_THRESHOLD {
        let strength = ((steepness - STEEPNESS_TRANSCODE_THRESHOLD)
            / (STEEPNESS_CONFIDENT_THRESHOLD - STEEPNESS_TRANSCODE_THRESHOLD))
            .clamp(0.0, 1.0);
        return (
            TranscodeAssessment {
                verdict: Verdict::ProbablyTranscoded,
                confidence_score: Some(0.5 + 0.3 * strength),
                indicators: vec![Indicator::new(IndicatorDetail::SharpRolloff {
                    steepness_db_per_khz: steepness,
                    edge_khz,
                })],
            },
            SpectralOutcome::CodecLowpass,
        );
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
        let mut indicators = vec![
            Indicator::new(IndicatorDetail::NoEncoderLowpass {
                scanned_from_khz: MIN_SCANNED_KHZ,
                nyquist_khz: nyquist_hz / 1000.0,
            }),
            Indicator::new(IndicatorDetail::TransparentEncodeUnseen),
        ];

        // Content above the CD ceiling cannot have come through a 44.1 kHz lossy encode — no
        // MP3 exists at a sample rate high enough to reach there. This is the whole reason
        // the branch can reach a verdict at all: it is positive evidence, measured, and it
        // closes the MP3 blind spot for this one class of file rather than asking the user to
        // trust an absence.
        //
        // Everything else here — no lowpass found, no AAC grid — rules things *out*. Ruling
        // out is not vouching, and treating it as such is what had this branch returning
        // `ProbablyAuthentic` at 0.65 for two real LAME V0 transcodes in this project's own
        // corpus. A file with no positive evidence now comes back `Indeterminate`, which is
        // the honest reading and, per the transcode-heuristic-validation skill, a legitimate
        // result rather than a failure to fix.
        let above_ceiling_db = spectral.above_cd_ceiling_db;

        if above_ceiling_db.is_some_and(|db| db >= ABOVE_CD_CEILING_MIN_DB) {
            indicators.push(Indicator::new(IndicatorDetail::ContentAboveCdCeiling {
                level_db: above_ceiling_db.unwrap_or_default(),
                ceiling_khz: CD_NYQUIST_HZ / 1000.0,
            }));
            return (
                TranscodeAssessment {
                    verdict: Verdict::ProbablyAuthentic,
                    confidence_score: Some(
                        (AUTHENTIC_BASE_CONFIDENCE + ABOVE_CD_BANDWIDTH_BONUS)
                            .min(AUTHENTIC_MAX_CONFIDENCE),
                    ),
                    indicators,
                },
                SpectralOutcome::NoLowpass,
            );
        }

        return (
            TranscodeAssessment {
                verdict: Verdict::Indeterminate,
                confidence_score: Some(NO_EVIDENCE_CONFIDENCE),
                indicators,
            },
            SpectralOutcome::NoLowpass,
        );
    }

    // An edge exists but is too gradual to attribute to a codec: a deliberately dark
    // master, a vinyl transfer, or a tape source can all end this way.
    (
        TranscodeAssessment {
            verdict: Verdict::Indeterminate,
            confidence_score: Some(NO_EVIDENCE_CONFIDENCE),
            indicators: vec![Indicator::new(IndicatorDetail::GradualRolloff {
                cutoff_khz: measured_cutoff_hz / 1000.0,
                steepness_db_per_khz: steepness,
            })],
        },
        SpectralOutcome::Inconclusive,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spectral::{BandLevel, SpectrogramData};

    fn spectral(
        encoder_edge_hz: Option<f64>,
        steepness: f64,
        cutoff: Option<f64>,
    ) -> SpectralAnalysis {
        SpectralAnalysis {
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
            analyzed: true,
            grid_detected: false,
            z_score: 1.0,
            frame_offset: 0,
            zero_fraction_at_offset: 0.1,
            zero_fraction_baseline: 0.1,
            sweep_profile_base64: String::new(),
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

    /// The failure this module's most important fix addresses: a clean spectral sweep is an
    /// absence of evidence, and absence used to be returned as `ProbablyAuthentic` at 0.65 —
    /// the tool vouching for the two real LAME V0 transcodes in its own corpus.
    #[test]
    fn a_clean_sweep_alone_no_longer_vouches_for_a_file() {
        let assessment = assess_transcode_risk(
            &spectral(None, 0.0, None),
            22_050.0,
            &[],
            &clear_grid(),
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert!(assessment
            .indicators
            .iter()
            .any(|i| matches!(i.detail, IndicatorDetail::NoEncoderLowpass { .. })));
    }

    /// Positive evidence still reaches a verdict: content in the top of a hi-res band cannot
    /// have come through a CD-rate lossy encode.
    #[test]
    fn measured_hi_res_content_supports_an_authentic_verdict() {
        let mut sp = spectral(None, 0.0, None);
        sp.above_cd_ceiling_db = Some(-5.0);
        let assessment =
            assess_transcode_risk(&sp, 48_000.0, &[], &clear_grid(), "flac", &complete());
        assert_eq!(assessment.verdict, Verdict::ProbablyAuthentic);
        assert!(assessment
            .indicators
            .iter()
            .any(|i| matches!(i.detail, IndicatorDetail::ContentAboveCdCeiling { .. })));
    }

    /// An upsampled file has content above the ceiling too, but only ringing. It must not
    /// clear the bar the measurement above sets.
    #[test]
    fn upsampled_ringing_does_not_support_an_authentic_verdict() {
        let mut sp = spectral(None, 0.0, Some(25_000.0));
        sp.above_cd_ceiling_db = Some(-47.7); // the corpus's upsampled fixture
        let assessment =
            assess_transcode_risk(&sp, 48_000.0, &[], &clear_grid(), "flac", &complete());
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
    }

    /// A tag naming a lossy encoder on a file the sweep found no lowpass in is a conflict,
    /// not a confession. Letting the tag carry the verdict would turn a leftover encoder
    /// string into an accusation — the reason `NoLowpass` is tracked separately from the
    /// `Indeterminate` it now produces.
    #[test]
    fn a_stale_encoder_tag_over_a_clean_sweep_is_a_conflict() {
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
            "flac",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert_eq!(assessment.confidence_score, Some(TAG_CONFLICT_CONFIDENCE));
        assert!(assessment
            .indicators
            .iter()
            .any(|i| matches!(i.detail, IndicatorDetail::TagContradictsSpectrum)));
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
            "flac",
            &complete(),
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
    /// describe — the measurements stand, the claim does not.
    #[test]
    fn an_incomplete_decode_withholds_the_verdict() {
        let damaged = DecodeStatus {
            complete: false,
            skipped_packets: 12,
            stopped_early: false,
            channels_unequal: false,
        };
        let assessment = assess_transcode_risk(
            // A spectrum that would otherwise read as an obvious transcode.
            &spectral(Some(16_000.0), 95.0, Some(16_000.0)),
            22_050.0,
            &[],
            &clear_grid(),
            "flac",
            &damaged,
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert!(matches!(
            assessment.indicators.first().map(|i| &i.detail),
            Some(IndicatorDetail::DecodeIncomplete {
                skipped_packets: 12,
                ..
            })
        ));
        // The measurement itself is still reported — only the accusation is withdrawn.
        assert!(assessment
            .indicators
            .iter()
            .any(|i| matches!(i.detail, IndicatorDetail::SharpRolloff { .. })));
    }

    /// Channels of different lengths mean different sections of the report describe
    /// different amounts of audio. That is a damaged file, and it withholds the verdict for
    /// the same reason a skipped packet does.
    #[test]
    fn unequal_channel_lengths_withhold_the_verdict() {
        let ragged = DecodeStatus {
            complete: false,
            skipped_packets: 0,
            stopped_early: false,
            channels_unequal: true,
        };
        let assessment = assess_transcode_risk(
            &spectral(Some(16_000.0), 95.0, Some(16_000.0)),
            22_050.0,
            &[],
            &clear_grid(),
            "flac",
            &ragged,
        );
        assert_eq!(assessment.verdict, Verdict::Indeterminate);
        assert!(assessment
            .indicators
            .iter()
            .any(|i| matches!(i.detail, IndicatorDetail::DecodeIncomplete { .. })));
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
            "mp3",
            &complete(),
        );
        assert_eq!(assessment.verdict, Verdict::DeclaredLossy);
        assert_eq!(assessment.confidence_score, None);
    }
}
