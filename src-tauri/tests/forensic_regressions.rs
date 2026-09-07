//! Counterexamples with known processing histories. These are regression checks, not a
//! held-out accuracy estimate: derivatives of one source must never be counted as an
//! independent validation dataset. See fixtures/corpus/forensic/README.md.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use nyquist_lib::analysis::{analyze, analyze_with_progress, PROGRESS_STAGE_COUNT};
use nyquist_lib::decode::{decode_file, DecodedAudio};
use nyquist_lib::mdct_grid::{analyze_mdct_grid, MdctWindow};
use nyquist_lib::signal_analysis::analyze_signal;
use nyquist_lib::spectral::analyze_spectrum;
use nyquist_lib::transcode_detect::Verdict;
use symphonia::core::audio::Channels;

fn fixture(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/corpus")
        .join(filename)
}

fn decoded(filename: &str) -> DecodedAudio {
    decode_file(&fixture(filename)).unwrap_or_else(|e| panic!("{filename}: {e}"))
}

#[test]
fn silence_is_not_ultrasonic_evidence_or_a_zero_loudness_range() {
    let result = analyze(&fixture("forensic/silence_96k.flac")).unwrap();
    assert!(!result.spectral_analysis.has_signal);
    assert!(result.spectral_analysis.above_cd_ceiling_db.is_none());
    assert!(result.signal_analysis.lufs_integrated.is_none());
    assert!(result.signal_analysis.loudness_range_lu.is_none());
    assert_eq!(result.transcode_assessment.verdict, Verdict::Indeterminate);
    assert!(!result.mdct_grid.analyzed);
}

#[test]
fn legitimate_sharp_filters_do_not_accuse_lossless_audio() {
    for filename in [
        "forensic/native_96k_fir.flac",
        "forensic/lossless_resampled_96k.flac",
    ] {
        let result = analyze(&fixture(filename)).unwrap();
        assert!(result.spectral_analysis.rolloff_steepness_db_per_khz > 40.0);
        assert!(result.sample_rate_analysis.bandwidth_limited);
        assert!(
            !result.mdct_grid.grid_detected,
            "{filename}: spurious codec grid"
        );
        assert_eq!(
            result.transcode_assessment.verdict,
            Verdict::Indeterminate,
            "{filename}"
        );
    }
}

#[test]
fn added_ultrasonic_noise_never_vouches_for_a_known_mp3() {
    let result = analyze(&fixture("forensic/mp3_with_added_noise_96k.flac")).unwrap();
    assert!(result.spectral_analysis.above_cd_ceiling_db.unwrap() > -30.0);
    assert_eq!(result.transcode_assessment.verdict, Verdict::Indeterminate);
}

#[test]
fn reversing_one_channel_cannot_erase_the_spectrum() {
    for filename in [
        "transcoded_mp3_128_44k.flac",
        "transcoded_mp3_128_upsampled_96k.flac",
    ] {
        let mut audio = decoded(filename);
        let original = analyze_spectrum(&audio).unwrap();
        audio.channel_samples[1].iter_mut().for_each(|s| *s = -*s);
        let inverted = analyze_spectrum(&audio).unwrap();
        assert!(inverted.has_signal);
        assert_eq!(original.spectral_cutoff_hz, inverted.spectral_cutoff_hz);
        assert_eq!(original.encoder_edge_hz, inverted.encoder_edge_hz);
        assert!(
            (original.rolloff_steepness_db_per_khz - inverted.rolloff_steepness_db_per_khz).abs()
                < 1e-6
        );
        assert_eq!(
            original.spectrogram.intensity_base64,
            inverted.spectrogram.intensity_base64
        );
    }
}

#[test]
fn kbd_aac_is_confirmed_even_after_gain_polarity_and_a_sample_crop() {
    let mut audio = decoded("forensic/native_aac_256.flac");
    let original = analyze_mdct_grid(&audio);
    assert!(
        original.grid_detected,
        "z={}, confirmation={}",
        original.z_score, original.confirmed_z_score
    );
    assert!(matches!(original.window, Some(MdctWindow::KaiserBessel)));
    assert!(original.z_score >= 20.0 && original.confirmed_z_score >= 20.0);
    for channel in &mut audio.channel_samples {
        channel.drain(..137);
        channel.iter_mut().for_each(|s| *s *= -0.5);
    }
    let transformed = analyze_mdct_grid(&audio);
    assert!(
        transformed.grid_detected,
        "z={}, confirmation={}",
        transformed.z_score, transformed.confirmed_z_score
    );
    assert_eq!(
        transformed.frame_offset,
        (original.frame_offset + 1024 - 137) % 1024
    );
}

#[test]
fn native_aac_receives_a_structural_verdict_in_the_shared_pipeline() {
    let result = analyze(&fixture("forensic/native_aac_256.flac")).unwrap();
    assert_eq!(
        result.transcode_assessment.verdict,
        Verdict::ProbablyTranscoded
    );
    assert!(result.transcode_assessment.confidence_score.unwrap() < 1.0);
    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["mdct_grid"]["window"], "kaiser_bessel");
    assert!(json["mdct_grid"]["confirmed_z_score"]
        .as_f64()
        .unwrap()
        .is_finite());
    assert!(json["sample_rate_analysis"]
        .get("likely_upsampled")
        .is_none());
    assert!(json["sample_rate_analysis"]
        .get("sufficient_sample_rate_hz")
        .is_none());
}

#[test]
fn side_channels_in_71_match_the_independent_loudness_reference() {
    let audio = decoded("forensic/side_only_71.wav");
    assert_eq!(audio.channels, 8);
    let signal = analyze_signal(&audio).unwrap();
    assert!(signal.loudness_layout_supported);
    // FFmpeg's independent ebur128 measurement on the committed PCM: -18.5 LUFS.
    assert!((signal.lufs_integrated.unwrap() - -18.5).abs() < 0.1);
    assert!((signal.peak_dbfs - -20.0).abs() < 0.01);
}

#[test]
fn unknown_multichannel_layout_withholds_loudness_but_not_peaks() {
    let mut audio = decoded("forensic/side_only_71.wav");
    audio.channel_layout = Channels::Discrete(8);
    let signal = analyze_signal(&audio).unwrap();
    assert!(!signal.loudness_layout_supported);
    assert!(signal.lufs_integrated.is_none());
    assert!(signal.loudness_range_lu.is_none());
    assert!((signal.true_peak_dbtp - -20.0).abs() < 0.1);
}

#[test]
fn lfe_energy_is_excluded_from_loudness_but_retained_in_true_peak() {
    let mut audio = decoded("forensic/side_only_71.wav");
    let tone = audio.channel_samples[6].clone();
    for channel in &mut audio.channel_samples {
        channel.fill(0.0);
    }
    audio.channel_samples[3] = tone;
    let signal = analyze_signal(&audio).unwrap();
    assert!(signal.loudness_layout_supported);
    assert!(signal.lufs_integrated.is_none());
    assert!((signal.true_peak_dbtp - -20.0).abs() < 0.1);
}

#[test]
fn every_completed_stage_is_reported_without_changing_the_analysis() {
    let completed = AtomicUsize::new(0);
    let path = fixture("calibration/sine_1khz_minus3dbfs.flac");
    let (result, audio) = analyze_with_progress(&path, &|| {
        completed.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    assert_eq!(completed.load(Ordering::Relaxed), PROGRESS_STAGE_COUNT);
    assert_eq!(
        result.file_info.sample_count,
        audio.channel_samples[0].len()
    );
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::to_value(analyze(&path).unwrap()).unwrap()
    );
}
