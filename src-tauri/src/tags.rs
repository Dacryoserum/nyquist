//! Scans container tags (Vorbis comments, ID3, ...) for leftover traces of a lossy
//! encoder — e.g. a FLAC whose `ENCODER` comment still reads "LAME3.100" because the
//! MP3→FLAC conversion tool didn't strip it. A weaker, more circumstantial signal than
//! the spectral analysis in `spectral.rs`/`transcode_detect.rs`: many legitimate lossless
//! conversion pipelines strip tags, and plenty of genuine transcodes carry no residual
//! metadata at all. Treated asymmetrically in `transcode_detect.rs`: a match is real
//! evidence, but the *absence* of a match is not evidence of authenticity.

use serde::Serialize;
use symphonia::core::formats::FormatReader;
use symphonia::core::meta::RawValue;

/// Substrings (case-insensitive) of known lossy encoders/tools, checked against every tag
/// value in the file. Not exhaustive — a miss here means "no match found", not "no lossy
/// encoder was ever involved".
const KNOWN_LOSSY_ENCODER_PATTERNS: &[&str] =
    &["lame3", "lame v3", "itunes", "qaac", "nero aac", "faac", "fdk-aac", "fhg", "fraunhofer", "libmp3lame"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EncoderTagMatch {
    pub tag_key: String,
    pub tag_value: String,
    pub matched_pattern: String,
}

/// Reads whatever metadata is available immediately after probing (works for header-based
/// tags: FLAC/OGG Vorbis comments, ID3v2 — not APEv2/ID3v1 trailers, which would require
/// scanning the whole file first; out of scope for this first pass).
pub fn scan_for_lossy_encoder_traces(format: &mut Box<dyn FormatReader>) -> Vec<EncoderTagMatch> {
    let Some(revision) = format.metadata().current().cloned() else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    for tag in &revision.media.tags {
        let Some(value_str) = tag_value_as_string(&tag.raw.value) else { continue };
        let value_lower = value_str.to_lowercase();
        for pattern in KNOWN_LOSSY_ENCODER_PATTERNS {
            if value_lower.contains(pattern) {
                matches.push(EncoderTagMatch {
                    tag_key: tag.raw.key.clone(),
                    tag_value: value_str.clone(),
                    matched_pattern: (*pattern).to_string(),
                });
            }
        }
    }
    matches
}

fn tag_value_as_string(value: &RawValue) -> Option<String> {
    match value {
        RawValue::String(s) => Some(s.to_string()),
        RawValue::StringList(list) => Some(list.join(" ")),
        _ => None,
    }
}
