//! Known-value correctness check for `signal_analysis.rs`, per the dsp-correctness skill:
//! verify against a reference signal with a hand-computable expected result, not just "it
//! runs without crashing."

use std::path::PathBuf;

use nyquist_lib::decode::decode_file;
use nyquist_lib::dynamic_range::compute_dr14;
use nyquist_lib::signal_analysis::analyze_signal;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus/calibration").join(name)
}

/// For any sine wave, RMS = peak / sqrt(2), i.e. RMS sits ~3.0103 dB below peak
/// regardless of frequency or amplitude. A 1kHz sine at -3dBFS peak must therefore
/// measure RMS ≈ -6.01 dBFS — see corpus/README.md for how the fixture was generated.
#[test]
fn sine_1khz_minus3dbfs_matches_known_rms() {
    let path = fixture("sine_1khz_minus3dbfs.flac");
    let decoded = decode_file(&path).expect("fixture should decode");
    let signal = analyze_signal(&decoded).expect("analysis should succeed");

    assert!(
        (signal.peak_dbfs - -3.0).abs() < 0.1,
        "expected peak ~= -3.0 dBFS, got {}",
        signal.peak_dbfs
    );
    assert!(
        (signal.rms_dbfs - -6.0103).abs() < 0.15,
        "expected RMS ~= -6.01 dBFS (peak - 3.0103dB for a sine wave), got {}",
        signal.rms_dbfs
    );
    assert_eq!(signal.clipping_count_total, 0, "a -3dBFS sine should never clip");
}

/// DR14's per-block RMS formula (sqrt(2 * mean(x^2)), see dynamic_range.rs module docs)
/// deliberately equals the true peak for a perfectly stationary sine wave — unlike plain
/// RMS, which sits 3.01dB below peak. A signal that never varies in level has zero
/// *dynamic range* by definition, and DR14 is built to reflect that: expect DR ≈ 0, not
/// the ~3dB one would get from a naive peak-to-RMS crest factor. Cross-validated against
/// a line-by-line Python port of the reference `dr14_t.meter` implementation (see
/// `.claude/CONTEXT.md`) — both agree to within 0.0005 dB on this fixture.
#[test]
fn sine_1khz_has_near_zero_dr14() {
    let path = fixture("sine_1khz_minus3dbfs.flac");
    let decoded = decode_file(&path).expect("fixture should decode");
    let dr = compute_dr14(&decoded);

    assert_eq!(dr.dr14, Some(0), "a stationary sine should round to DR0");
    for (ch, value) in dr.per_channel_db.iter().enumerate() {
        let v = value.unwrap_or_else(|| panic!("channel {ch}: expected a measurable DR value"));
        assert!(v.abs() < 0.01, "channel {ch}: expected DR ~= 0.0 dB, got {v}");
    }
}
