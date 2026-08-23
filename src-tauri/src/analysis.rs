//! Runs the full analysis pipeline (decode → signal → dynamic range → spectral →
//! transcode assessment) and assembles the result. Shared by the Tauri command
//! (`commands::analyze_file`) and the CLI binary (`bin/nyquist-cli.rs`) so the two never
//! drift apart — see `.claude/skills/tauri-ipc-contract/SKILL.md` before changing the
//! shape of [`AnalysisResult`].

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::bit_depth::{self, BitDepthAnalysis};
use crate::decode;
use crate::dynamic_range::{self, DynamicRangeResult};
use crate::mdct_grid::{self, MdctGridAnalysis};
use crate::metadata::{self, FileInfo};
use crate::sample_rate::{self, SampleRateAnalysis};
use crate::signal_analysis::{self, SignalAnalysis};
use crate::spectral::{self, SpectralAnalysis};
use crate::stereo::{self, StereoAnalysis};
use crate::tags::EncoderTagMatch;
use crate::transcode_detect::{self, TranscodeAssessment};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AnalysisResult {
    pub file_info: FileInfo,
    pub signal_analysis: SignalAnalysis,
    pub dynamic_range: DynamicRangeResult,
    pub spectral_analysis: SpectralAnalysis,
    pub transcode_assessment: TranscodeAssessment,
    pub encoder_tag_matches: Vec<EncoderTagMatch>,
    /// A separate quality issue from `transcode_assessment` — a file can be padded to a
    /// wider bit depth without ever having been lossy-compressed. See bit_depth.rs.
    pub bit_depth_analysis: BitDepthAnalysis,
    /// The sample-rate counterpart to `bit_depth_analysis`: a file can be resampled up to
    /// a hi-res rate it never earns, again without any lossy step. See sample_rate.rs.
    pub sample_rate_analysis: SampleRateAnalysis,
    /// `None` for anything that is not exactly two channels. Reported information only —
    /// see stereo.rs on why the stereo image does *not* feed the transcode verdict.
    pub stereo_analysis: Option<StereoAnalysis>,
    /// AAC encoder frame-grid alignment. Unlike the stereo image, this one *does* feed the
    /// verdict — see mdct_grid.rs.
    pub mdct_grid: MdctGridAnalysis,
}

/// Wall-clock cost of each pipeline stage. Deliberately **not** part of
/// [`AnalysisResult`]: it is a profiling aid for the CLI (`nyquist-cli --timing`) and for
/// deciding whether a stage has grown slow enough to need progress reporting, not part of
/// the IPC contract the frontend depends on.
#[derive(Debug, Clone, Copy, Default)]
pub struct StageTimings {
    pub decode: Duration,
    pub signal: Duration,
    pub dynamic_range: Duration,
    pub spectral: Duration,
    pub bit_depth: Duration,
    pub stereo: Duration,
    pub mdct_grid: Duration,
    pub total: Duration,
}

pub fn analyze(path: &Path) -> Result<AnalysisResult, String> {
    analyze_with_timings(path).map(|(result, _)| result)
}

/// [`analyze`], plus how long each stage took. Both go through this one body so the timed
/// and untimed paths can never diverge.
pub fn analyze_with_timings(path: &Path) -> Result<(AnalysisResult, StageTimings), String> {
    let mut timings = StageTimings::default();
    let started = Instant::now();

    let stage = Instant::now();
    let decoded = decode::decode_file(path)?;
    timings.decode = stage.elapsed();

    let file_info = metadata::build_file_info(path, &decoded)?;

    // Everything past the decode reads `decoded` and nothing else, so these four stages are
    // independent of one another and only ran in sequence out of habit. Wall time for the
    // group is now the slowest member rather than their sum.
    //
    // Each is still timed individually, so `--timing` reports the cost of the work rather
    // than the elapsed span, and a stage that grows expensive is still visible even when it
    // no longer sits on the critical path.
    let ((signal_analysis, dynamic_range), (spectral_analysis, bit_depth_analysis)) = rayon::join(
        || {
            rayon::join(
                || {
                    let stage = Instant::now();
                    let out = signal_analysis::analyze_signal(&decoded);
                    (out, stage.elapsed())
                },
                || {
                    let stage = Instant::now();
                    let out = dynamic_range::compute_dr14(&decoded);
                    (out, stage.elapsed())
                },
            )
        },
        || {
            rayon::join(
                || {
                    let stage = Instant::now();
                    let out = spectral::analyze_spectrum(&decoded);
                    (out, stage.elapsed())
                },
                || {
                    rayon::join(
                        || {
                            let stage = Instant::now();
                            let out = bit_depth::analyze_bit_depth(&decoded);
                            (out, stage.elapsed())
                        },
                        || {
                            rayon::join(
                                || {
                                    let stage = Instant::now();
                                    let out = stereo::analyze_stereo(&decoded);
                                    (out, stage.elapsed())
                                },
                                || {
                                    let stage = Instant::now();
                                    let out = mdct_grid::analyze_mdct_grid(&decoded);
                                    (out, stage.elapsed())
                                },
                            )
                        },
                    )
                },
            )
        },
    );

    let (signal_analysis, signal_elapsed) = signal_analysis;
    let (dynamic_range, dr_elapsed) = dynamic_range;
    let (spectral_analysis, spectral_elapsed) = spectral_analysis;
    let ((bit_depth_analysis, bit_depth_elapsed), ((stereo_analysis, stereo_elapsed), (mdct_grid, mdct_elapsed))) =
        bit_depth_analysis;

    timings.signal = signal_elapsed;
    timings.dynamic_range = dr_elapsed;
    timings.spectral = spectral_elapsed;
    timings.bit_depth = bit_depth_elapsed;
    timings.stereo = stereo_elapsed;
    timings.mdct_grid = mdct_elapsed;

    let signal_analysis = signal_analysis?;
    let spectral_analysis = spectral_analysis?;
    let transcode_assessment = transcode_detect::assess_transcode_risk(
        &spectral_analysis,
        file_info.nyquist_hz as f64,
        &decoded.encoder_tag_matches,
        &mdct_grid,
    );
    let sample_rate_analysis = sample_rate::analyze_sample_rate(
        file_info.sample_rate_hz,
        spectral_analysis.spectral_cutoff_hz,
    );
    let encoder_tag_matches = decoded.encoder_tag_matches.clone();
    timings.total = started.elapsed();

    Ok((
        AnalysisResult {
            file_info,
            signal_analysis,
            dynamic_range,
            spectral_analysis,
            transcode_assessment,
            encoder_tag_matches,
            bit_depth_analysis,
            sample_rate_analysis,
            stereo_analysis,
            mdct_grid,
        },
        timings,
    ))
}
