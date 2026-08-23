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
use nyquist_lib::mdct_grid::analyze_mdct_grid;
use nyquist_lib::metadata::build_file_info;
use nyquist_lib::sample_rate::analyze_sample_rate;
use nyquist_lib::signal_analysis::analyze_signal;
use nyquist_lib::spectral::analyze_spectrum;
use nyquist_lib::stereo::analyze_stereo;
use nyquist_lib::transcode_detect::{assess_transcode_risk, Verdict};

struct Fixture {
    filename: &'static str,
    expected_sample_rate: u32,
    expected_channels: usize,
    /// (min, max) Hz the measured content bandwidth should land in — the edge where energy
    /// stops, or Nyquist when nothing bounds it. Cross-checks the independently
    /// ffmpeg-measured ground truth in corpus/README.md rather than duplicating it.
    ///
    /// Note this is no longer the peak-relative "-40 dB below the spectral peak" point: that
    /// reference only behaves on flat material, and reading it on music-shaped input is what
    /// made every real file inconclusive. See `spectral::find_spectral_edge`.
    expected_cutoff_range_hz: (f64, f64),
    /// Nominal length in seconds. Per-fixture rather than a shared constant: the
    /// silence-padded transcode is deliberately longer than the 5s the rest share.
    expected_duration_seconds: f64,
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
        expected_duration_seconds: 5.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        filename: "authentic_96k_noise.flac",
        expected_sample_rate: 96_000,
        expected_channels: 2,
        expected_cutoff_range_hz: (46_000.0, 48_000.0),
        expected_duration_seconds: 5.0,
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
        expected_duration_seconds: 5.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        filename: "transcoded_mp3_320_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        // LAME 320's wall is narrow enough that the wide bandwidth probe steps over it;
        // it is caught by steepness, which is what the verdict assertion below covers.
        expected_cutoff_range_hz: (19_000.0, 22_050.0),
        expected_duration_seconds: 5.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        filename: "transcoded_mp3_128_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (15_500.0, 18_000.0),
        expected_duration_seconds: 5.0,
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
        expected_duration_seconds: 5.0,
        is_actually_transcoded: true,
        known_undetectable: true,
    },
    Fixture {
        filename: "transcoded_aac_256_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 5.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        // False-positive trap: genuinely lossless *tonal* content. A sustained chord's
        // spectrum falls from peak to noise floor within a couple of FFT bins, which a
        // steepness measurement based on the frequency span between two dB levels reads
        // as an infinitely steep brick wall. This used to be reported as "probably
        // transcoded" at 80% confidence — as was the project's own 1 kHz calibration sine.
        filename: "authentic_44k_tonal.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        // Tonal content has no lowpass, so its bandwidth reads as Nyquist.
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 5.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // Music-shaped rather than flat: the case that exposed a peak-relative bandwidth
        // measurement as unusable. Genuinely lossless and full-bandwidth, so it must read
        // authentic — it used to come out "indeterminate" at 30%, as every real FLAC did.
        filename: "authentic_musiclike_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // Same, at a genuine hi-res rate: additionally guards against the sample-rate check
        // calling real 96 kHz material upsampled, which the same bug caused.
        filename: "authentic_musiclike_96k.flac",
        expected_sample_rate: 96_000,
        expected_channels: 2,
        expected_cutoff_range_hz: (46_000.0, 48_000.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // False-negative trap: the same LAME 128 transcode as above, padded with digital
        // silence the way essentially every real track is (lead-in, fade-out, gaps). The
        // silence used to raise the averaged noise floor above the encoder's stopband and
        // switch detection off entirely — 191 dB/kHz became 0, and a caught transcode
        // became "indeterminate".
        filename: "transcoded_mp3_128_padded_silence.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (15_500.0, 18_000.0),
        expected_duration_seconds: 11.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        // Lossless throughout, so NOT a transcode — the deception here is the sample rate,
        // which `sample_rate.rs` covers and which is asserted separately below. Listed
        // here to prove the transcode verdict doesn't misattribute a resampler's brick
        // wall to a lossy encoder.
        filename: "upsampled_44k_to_96k.flac",
        expected_sample_rate: 96_000,
        expected_channels: 2,
        expected_cutoff_range_hz: (22_000.0, 26_000.0),
        expected_duration_seconds: 5.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // Both defects at once: lossy source, then upsampled so the encoder cutoff no
        // longer sits anywhere near the declared Nyquist.
        filename: "transcoded_mp3_128_upsampled_96k.flac",
        expected_sample_rate: 96_000,
        expected_channels: 2,
        expected_cutoff_range_hz: (15_500.0, 18_000.0),
        expected_duration_seconds: 5.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    // ── Non-stationary, true-stereo material ────────────────────────────────────────
    // Everything above is stationary noise in dual-mono. These five share one source with
    // decorrelated channels, quiet passages, transients and sustained tones — the closest
    // this corpus gets to the material the tool is actually pointed at. They are also what
    // makes the size of the blind spot visible: on flat noise it costs two fixtures, here
    // it costs three, one of which (AAC 128) is caught on stationary material and escapes
    // on this one.
    Fixture {
        filename: "authentic_dynamic_stereo_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // Still caught, and the control that proves the new material did not simply break
        // detection across the board: 72 dB/kHz at 16.8 kHz.
        filename: "transcoded_dynamic_mp3_128_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (15_500.0, 18_000.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        // The documented blind spot, reproduced on realistic material: no lowpass at all,
        // so this reads "probably authentic" at 60% — the tool actively vouching for a
        // transcode rather than merely failing to catch it.
        filename: "transcoded_dynamic_mp3_v0_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: true,
        known_undetectable: true,
    },
    Fixture {
        // Same, for Apple's AAC at 256 kbps.
        filename: "transcoded_dynamic_aac_256_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    Fixture {
        // New information from this material: AAC 128 lowpasses at 18.3 kHz but only at
        // 27 dB/kHz, under the 40 dB/kHz gate, so it lands on "indeterminate" instead of
        // being caught. On stationary noise the same encoder reads ~106 dB/kHz and is
        // caught comfortably — the gap is a property of the *material*, not the bitrate,
        // which is exactly what a stationary-only corpus cannot show.
        filename: "transcoded_dynamic_aac_128_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (18_000.0, 19_500.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: true,
        known_undetectable: false,
    },
    // ── False-positive traps on non-stationary material ─────────────────────────────
    Fixture {
        // Lossless tonal content decaying to digital silence between notes, with partials
        // at decreasing amplitudes so the high band empties before the mids do. Reads as a
        // codec zeroing high bands without any encoder involved; see corpus/README.md.
        filename: "authentic_decay_to_silence_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        // Content genuinely stops where the top partial sits — no lowpass, so no edge.
        expected_cutoff_range_hz: (10_000.0, 22_050.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: false,
        known_undetectable: false,
    },
    Fixture {
        // Loud, and empty above the bass. Guards any rule of the form "high band is silent
        // while the file is loud, therefore lossy".
        filename: "authentic_bass_only_44k.flac",
        expected_sample_rate: 44_100,
        expected_channels: 2,
        expected_cutoff_range_hz: (21_000.0, 22_050.0),
        expected_duration_seconds: 10.0,
        is_actually_transcoded: false,
        known_undetectable: false,
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
            (file_info.duration_seconds - fx.expected_duration_seconds).abs() < 1.0,
            "{}: expected ~{}s, got {}",
            fx.filename,
            fx.expected_duration_seconds,
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

        let grid = analyze_mdct_grid(&decoded);
        let assessment =
            assess_transcode_risk(&spectral, file_info.nyquist_hz as f64, &decoded.encoder_tag_matches, &grid);
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
        "expected exactly the 2 remaining undetectable cases to miss — LAME V0 on both the \
         stationary and the non-stationary material. Every AAC case is now caught by the MDCT \
         grid sweep (mdct_grid.rs), which cannot invert MP3's hybrid filterbank. Got \
         {known_misses:?} — if this shrinks further the MP3 side has been solved: update \
         `known_undetectable` and this assertion; if it grows, something regressed"
    );
}

/// `dual_mono` is an exact claim — the two channels are bit-identical — so it is asserted
/// against fixtures whose construction is known rather than against a threshold.
///
/// This also pins down a property of the corpus itself that went unnoticed until it was
/// measured: every fixture built with ffmpeg's `-ac 2` is a mono source upmixed, so its
/// channels really are identical. That is why the non-stationary fixtures were built by
/// joining two independently seeded noise sources instead, and why any future detector that
/// reads the stereo image can only be developed against those.
#[test]
fn dual_mono_is_detected_exactly() {
    let cases = [
        ("authentic_44k_noise.flac", true),
        ("authentic_dynamic_stereo_44k.flac", false),
        ("transcoded_dynamic_mp3_v0_44k.flac", false),
    ];

    for (filename, expect_dual_mono) in cases {
        let decoded = decode_file(&corpus_dir().join(filename)).expect("fixture should decode");
        let stereo = analyze_stereo(&decoded).expect("two-channel fixture should yield stereo analysis");

        assert_eq!(
            stereo.dual_mono, expect_dual_mono,
            "{filename}: dual_mono should be {expect_dual_mono} (correlation {:.3}, side/mid {:.1} dB)",
            stereo.correlation, stereo.side_to_mid_db
        );
        assert!(
            (-1.0..=1.0).contains(&stereo.correlation),
            "{filename}: correlation {} outside -1..=1",
            stereo.correlation
        );
        assert_eq!(stereo.per_band.len(), 3, "{filename}: expected low/mid/high bands");

        if expect_dual_mono {
            // A duplicated channel has no side content at all, so width must read at the
            // floor rather than merely "small".
            assert!(
                stereo.side_to_mid_db <= -60.0,
                "{filename}: bit-identical channels must report no width; got {} dB",
                stereo.side_to_mid_db
            );
        } else {
            assert!(
                stereo.side_to_mid_db > -60.0,
                "{filename}: decorrelated channels should carry measurable width; got {} dB",
                stereo.side_to_mid_db
            );
        }
    }
}

/// A pure sine is the least ambiguous authentic signal there is, and it lives in this
/// project's own calibration corpus. It was nevertheless reported as "probably transcoded"
/// at 80% confidence, because its spectrum drops from peak to noise floor inside a couple
/// of FFT bins and the old steepness measurement divided a fixed dB drop by that
/// near-zero frequency span. Guarding the case explicitly: if a sine ever reads as
/// transcoded again, the rolloff measurement has regressed to dividing by a vanishing
/// span, whatever else changed around it.
#[test]
fn a_pure_sine_is_never_reported_as_transcoded() {
    let path = corpus_dir().join("calibration/sine_1khz_minus3dbfs.flac");
    let decoded = decode_file(&path).expect("calibration sine should decode");
    let file_info = build_file_info(&path, &decoded).expect("metadata should build");
    let spectral = analyze_spectrum(&decoded).expect("spectral analysis should succeed");
    let grid = analyze_mdct_grid(&decoded);
    let assessment =
        assess_transcode_risk(&spectral, file_info.nyquist_hz as f64, &decoded.encoder_tag_matches, &grid);

    assert_eq!(
        spectral.rolloff_steepness_db_per_khz, 0.0,
        "a sine has no encoder cutoff to measure; got {} dB/kHz",
        spectral.rolloff_steepness_db_per_khz
    );
    assert_ne!(
        assessment.verdict,
        Verdict::ProbablyTranscoded,
        "pure sine flagged as transcoded — false positive, got {:?}",
        assessment.verdict
    );
}

/// `sample_rate::analyze_sample_rate` — the sample-rate counterpart to the bit-depth
/// padding check below. Genuine hi-res must stay silent; a file resampled up from CD rate
/// must be caught even though it is lossless end to end and therefore invisible to the
/// transcode verdict.
#[test]
fn upsampled_hi_res_is_detected_without_false_positives() {
    let cases: &[(&str, bool)] = &[
        ("upsampled_44k_to_96k.flac", true),
        ("transcoded_mp3_128_upsampled_96k.flac", true),
        ("authentic_96k_noise.flac", false),
        // Music-shaped genuine hi-res: the false positive the peak-relative bandwidth
        // measurement produced, reporting real 96 kHz content as upsampled from 44.1 kHz.
        ("authentic_musiclike_96k.flac", false),
        // A 44.1 kHz file makes no hi-res claim, however treble-poor it is.
        ("authentic_44k_lowpass_naturally.flac", false),
    ];

    for (filename, expected_upsampled) in cases {
        let path = corpus_dir().join(filename);
        let decoded = decode_file(&path).unwrap_or_else(|e| panic!("{filename}: decode failed: {e}"));
        let spectral =
            analyze_spectrum(&decoded).unwrap_or_else(|e| panic!("{filename}: spectral failed: {e}"));
        let analysis =
            analyze_sample_rate(decoded.sample_rate, spectral.spectral_cutoff_hz);

        assert_eq!(
            analysis.likely_upsampled, *expected_upsampled,
            "{filename}: expected likely_upsampled={expected_upsampled}, got {} \
             (declared {} Hz, bandwidth {:.0} Hz, ratio {:.2})",
            analysis.likely_upsampled,
            analysis.declared_sample_rate_hz,
            analysis.content_bandwidth_hz,
            analysis.bandwidth_ratio
        );

        if *expected_upsampled {
            let sufficient = analysis
                .sufficient_sample_rate_hz
                .unwrap_or_else(|| panic!("{filename}: flagged as upsampled but named no sufficient rate"));
            assert!(
                sufficient < analysis.declared_sample_rate_hz,
                "{filename}: sufficient rate {sufficient} must be below the declared rate"
            );
        }
    }
}

/// Clipping is counted on both halves of the waveform. Signed PCM is asymmetric — 16-bit
/// positive full scale normalizes to 0.99997, not 1.0 — so a naive `abs() >= 1.0` test
/// silently counted only negative-side clipping and halved every reported figure.
#[test]
fn clipping_is_counted_symmetrically() {
    let path = corpus_dir().join("calibration/fullscale_both_polarities.flac");
    let decoded = decode_file(&path).expect("full-scale fixture should decode");
    let signal = analyze_signal(&decoded).expect("signal analysis should succeed");

    // 1000 samples at +32767 and 1000 at -32768.
    assert_eq!(
        signal.clipping_count_total, 2000,
        "expected both polarities counted (2000); got {} — a value near 1000 means only \
         the negative half is being detected",
        signal.clipping_count_total
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
