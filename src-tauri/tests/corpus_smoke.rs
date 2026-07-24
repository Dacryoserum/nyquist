//! Regression coverage for decode.rs/metadata.rs/signal_analysis.rs/spectral.rs against
//! real encoder output (FLAC, and FLAC re-encoded from MP3/AAC) rather than synthetic
//! in-memory buffers. Catches things a unit test can't, e.g. a symphonia upgrade silently
//! breaking AAC decoding. Ground truth for each fixture: tests/fixtures/corpus/README.md.
//!
//! Also validates `transcode_detect::assess_transcode_risk` per the
//! transcode-heuristic-validation skill: run against the whole corpus, report false
//! positive/negative counts explicitly rather than only asserting the cases that pass.

use std::path::PathBuf;

use nyquist_lib::bit_depth::analyze_bit_depth;
use nyquist_lib::decode::decode_file;
use nyquist_lib::metadata::build_file_info;
use nyquist_lib::signal_analysis::analyze_signal;
use nyquist_lib::spectral::analyze_spectrum;
use nyquist_lib::transcode_detect::{assess_transcode_risk, Verdict};

struct Fixture {
    filename: &'static str,
    expected_sample_rate: u32,
    expected_channels: usize,
    /// (min, max) Hz the raw spectral cutoff detector should land in — a cross-check
    /// against the independently ffmpeg-measured ground truth in corpus/README.md, not a
    /// duplicate of it (different method: our own FFT vs. ffmpeg's highpass+astats sweep).
    expected_cutoff_range_hz: (f64, f64),
    /// Ground truth per corpus/README.md — is this file actually a lossy transcode?
    is_actually_transcoded: bool,
    /// True for the two fixtures documented in corpus/README.md as undetectable by
    /// spectral-cutoff methods (LAME V0, AAC 256 — neither lowpasses). A wrong verdict on
    /// these is an expected, already-understood limitation, not a regression — see
    /// transcode_detect.rs module docs "Known blind spot". Still counted in the report,
    /// just not asserted as a hard failure.
    known_undetectable: bool,
}

const FIXTURES: &[Fixture] = &[
    // Expected ranges below are cross-checked against the independent ffmpeg
    // highpass+astats measurements documented in corpus/README.md — same conclusions,
    // different method, which is the point (they must agree, not just both exist).
    Fixture {
        filename: "authentic_44k_noise.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        filename: "authentic_96k_noise.flac",
        expected_sample_rate: 96_000,
        expected_channels: 2,
        expected_cutoff_range_hz: (46_000.0, 48_000.0),
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // Not a tight bound on purpose: the point of this fixture is that a naturally
        // treble-poor (but genuinely lossless) file must NOT read like the sharp-cutoff
        // mp3_128 case below, not that it lands at any specific frequency.
        filename: "authentic_44k_lowpass_naturally.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (19_000.0, 22_050.0),
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        filename: "transcoded_mp3_320_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (19_000.0, 21_500.0),
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        filename: "transcoded_mp3_128_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (15_500.0, 18_000.0),
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        // LAME V0 doesn't lowpass — indistinguishable from authentic by cutoff frequency
        // alone. Asserting that on purpose: proves this detector can't catch this case,
        // consistent with corpus/README.md's documented "hard" classification.
        filename: "transcoded_mp3_v0_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        is_actually_transcoded: true,
        known_undetectable: true,
    },
    Fixture {
        filename: "transcoded_aac_256_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        is_actually_transcoded: true,
        known_undetectable: true,
    },
];

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus")
}

#[test]
fn every_corpus_fixture_decodes_and_analyzes_cleanly() {
    let mut false_positives: Vec<&str> = Vec::new();
    let mut false_negatives: Vec<&str> = Vec::new();
    let mut known_misses: Vec<&str> = Vec::new();

    for fx in FIXTURES {
        let path = corpus_dir().join(fx.filename);
        let decoded = decode_file(&path).unwrap_or_else(|e| panic!("{}: decode failed: {e}", fx.filename));

        assert_eq!(decoded.sample_rate, fx.expected_sample_rate, "{}: unexpected sample rate", fx.filename);
        assert_eq!(decoded.channels, fx.expected_channels, "{}: unexpected channel count", fx.filename);
        assert!(
            decoded.channel_samples.iter().all(|c| !c.is_empty()),
            "{}: decoded to empty channel buffer",
            fx.filename
        );

        let file_info = build_file_info(&path, &decoded)
            .unwrap_or_else(|e| panic!("{}: metadata build failed: {e}", fx.filename));
        assert!(
            (file_info.duration_seconds - 5.0).abs() < 1.0,
            "{}: unexpected duration {}",
            fx.filename,
            file_info.duration_seconds
        );

        let signal = analyze_signal(&decoded).unwrap_or_else(|e| panic!("{}: signal analysis failed: {e}", fx.filename));
        assert!(signal.peak_dbfs.is_finite(), "{}: non-finite peak", fx.filename);
        assert!(signal.rms_dbfs.is_finite(), "{}: non-finite RMS", fx.filename);
        assert!(
            signal.lufs_integrated.is_some_and(f64::is_finite),
            "{}: expected a measurable LUFS value for non-silent noise/sine content",
            fx.filename
        );
        assert_eq!(signal.per_channel.len(), fx.expected_channels, "{}: per-channel stats count mismatch", fx.filename);

        let spectral =
            analyze_spectrum(&decoded).unwrap_or_else(|e| panic!("{}: spectral analysis failed: {e}", fx.filename));
        eprintln!(
            "{}: spectral_cutoff_hz = {}, rolloff_steepness_db_per_khz = {}",
            fx.filename, spectral.spectral_cutoff_hz, spectral.rolloff_steepness_db_per_khz
        );
        assert!(
            spectral.spectral_cutoff_hz >= fx.expected_cutoff_range_hz.0
                && spectral.spectral_cutoff_hz <= fx.expected_cutoff_range_hz.1,
            "{}: spectral cutoff {} Hz outside expected range {:?}",
            fx.filename,
            spectral.spectral_cutoff_hz,
            fx.expected_cutoff_range_hz
        );
        assert_eq!(
            spectral.cutoff_over_time_hz.len(),
            spectral.spectrogram.time_bin_count,
            "{}: cutoff_over_time_hz length must match the spectrogram's time axis",
            fx.filename
        );
        assert!(
            spectral.cutoff_over_time_hz.iter().all(|&hz| hz.is_finite() && hz >= 0.0),
            "{}: cutoff_over_time_hz contains a non-finite or negative value",
            fx.filename
        );

        let expected_bytes = spectral.spectrogram.time_bin_count * spectral.spectrogram.frequency_bin_count;
        let expected_base64_len = expected_bytes.div_ceil(3) * 4;
        assert_eq!(
            spectral.spectrogram.intensity_base64.len(),
            expected_base64_len,
            "{}: spectrogram payload size mismatch",
            fx.filename
        );

        let assessment = assess_transcode_risk(&spectral, file_info.nyquist_hz as f64, &decoded.encoder_tag_matches);
        eprintln!(
            "{}: verdict = {:?}, confidence = {:.2}, actually_transcoded = {}",
            fx.filename, assessment.verdict, assessment.confidence_score, fx.is_actually_transcoded
        );
        assert!(!assessment.indicators.is_empty(), "{}: verdict has no stated indicators", fx.filename);

        let flagged_transcoded = matches!(assessment.verdict, Verdict::ProbablyTranscoded);
        match (fx.is_actually_transcoded, flagged_transcoded) {
            (false, true) => false_positives.push(fx.filename),
            (true, false) if fx.known_undetectable => known_misses.push(fx.filename),
            (true, false) => false_negatives.push(fx.filename),
            _ => {}
        }
    }

    eprintln!(
        "\n=== transcode_detect corpus report: {} fixtures, {} false positives, {} \
         unexpected false negatives, {} known/documented misses ===",
        FIXTURES.len(),
        false_positives.len(),
        false_negatives.len(),
        known_misses.len()
    );

    // A false positive here means flagging a genuinely authentic (or, for the natural-
    // lowpass trap fixture, authentic-but-treble-poor) file as transcoded — the single
    // worst outcome for this tool, per AGENTS.md and the transcode-heuristic-validation
    // skill. Zero tolerance on the current corpus.
    assert!(false_positives.is_empty(), "false positives on authentic fixtures: {false_positives:?}");
    // A false negative on a case NOT already documented as undetectable would mean the
    // heuristic regressed on a case it used to catch (e.g. mp3_128/mp3_320). The two
    // known/documented misses (V0, AAC256) are asserted separately below so a fix to that
    // blind spot is visible instead of silently swallowed by a loose assertion.
    assert!(false_negatives.is_empty(), "unexpected false negatives: {false_negatives:?}");
    assert_eq!(
        known_misses.len(),
        2,
        "expected exactly the 2 documented undetectable cases (V0, AAC256) to miss; got {known_misses:?} — \
         if this is now 0, the blind spot may be fixed: update `known_undetectable` and this assertion; \
         if this is >2, something else regressed"
    );
}

/// Cross-check for `bit_depth::analyze_bit_depth` — separate from the transcode-detection
/// fixtures above, since bit-depth padding is a different quality issue (see bit_depth.rs
/// module docs): a file can be padded to a wider container without ever touching a lossy
/// codec.
#[test]
fn bit_depth_padding_is_detected_without_false_positives() {
    let path = corpus_dir().join("bitdepth_fake24_from16.flac");
    let decoded = decode_file(&path).expect("bitdepth_fake24_from16.flac should decode");
    let analysis = analyze_bit_depth(&decoded);
    assert_eq!(analysis.declared_bit_depth, Some(24));
    assert_eq!(
        analysis.effective_bit_depth,
        Some(16),
        "16-bit content zero-padded into a 24-bit container should be detected as effectively 16-bit"
    );

    let path = corpus_dir().join("bitdepth_genuine24.flac");
    let decoded = decode_file(&path).expect("bitdepth_genuine24.flac should decode");
    let analysis = analyze_bit_depth(&decoded);
    assert_eq!(analysis.declared_bit_depth, Some(24));
    assert_eq!(
        analysis.effective_bit_depth,
        Some(24),
        "genuinely 24-bit content must not be flagged as padded — false positive"
    );
}
