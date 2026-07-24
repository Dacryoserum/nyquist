//! Runs the full analysis pipeline (decode → signal → dynamic range → spectral →
//! transcode assessment) and assembles the result. Shared by the Tauri command
//! (`commands::analyze_file`) and the CLI binary (`bin/nyquist-cli.rs`) so the two never
//! drift apart — see `.claude/skills/tauri-ipc-contract/SKILL.md` before changing the
//! shape of [`AnalysisResult`].

use std::path::Path;

use serde::Serialize;

use crate::bit_depth::{self, BitDepthAnalysis};
use crate::decode;
use crate::dynamic_range::{self, DynamicRangeResult};
use crate::metadata::{self, FileInfo};
use crate::signal_analysis::{self, SignalAnalysis};
use crate::spectral::{self, SpectralAnalysis};
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
}

pub fn analyze(path: &Path) -> Result<AnalysisResult, String> {
    let decoded = decode::decode_file(path)?;
    let file_info = metadata::build_file_info(path, &decoded)?;
    let signal_analysis = signal_analysis::analyze_signal(&decoded)?;
    let dynamic_range = dynamic_range::compute_dr14(&decoded);
    let spectral_analysis = spectral::analyze_spectrum(&decoded)?;
    let transcode_assessment = transcode_detect::assess_transcode_risk(
        &spectral_analysis,
        file_info.nyquist_hz as f64,
        &decoded.encoder_tag_matches,
    );
    let bit_depth_analysis = bit_depth::analyze_bit_depth(&decoded);
    let encoder_tag_matches = decoded.encoder_tag_matches.clone();

    Ok(AnalysisResult {
        file_info,
        signal_analysis,
        dynamic_range,
        spectral_analysis,
        transcode_assessment,
        encoder_tag_matches,
        bit_depth_analysis,
    })
}
