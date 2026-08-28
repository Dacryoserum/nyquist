//! Measurement bench for real music, run against whatever is in `corpus/local/`.
//!
//! ## Why this exists
//!
//! The committed corpus is built entirely from noise — pink and white, EQ'd and gated (see
//! `fixtures/generate_corpus.sh`). Noise is reproducible and licence-free, which is why it
//! was chosen, and for the rolloff measurement it works: a codec's lowpass is a lowpass
//! whatever goes through it.
//!
//! It is useless for the one case that remains open. A perceptual encoder betrays itself by
//! *discarding* what a listener would not miss, and noise is the material where it discards
//! least: incompressible, unmaskable, so at V0 the encoder spends bits everywhere and leaves
//! no trace. Measured on this corpus, `transcoded_mp3_v0_44k` and `authentic_44k_noise` have
//! the same per-subband statistics to the decimal — so any V0 detector, correct or not,
//! would score identically on both and could be neither validated nor refuted here.
//!
//! Real music has quiet passages, tonal masking and decays, which is where an encoder at V0
//! actually starves. This bench reports, for any file dropped into `corpus/local/` (which is
//! gitignored, so nothing copyrighted is committed), the statistics a detector would have to
//! separate on. Point it at a lossless original and its own LAME V0 transcode and the answer
//! is readable directly: if the columns differ, a detector is worth building; if they do not,
//! the spectral route is closed for V0 and only quantization-error analysis remains — see
//! `corpus/README.md`, "Approaches prototyped and rejected".
//!
//! Passes trivially when the directory is empty, so it costs a committed repository nothing.

use std::path::PathBuf;

use nyquist_lib::decode::decode_file;
use nyquist_lib::mdct_grid::analyze_mdct_grid;
use nyquist_lib::spectral::analyze_spectrum;
use nyquist_lib::transcode_detect::assess_transcode_risk;

const GRANULE: usize = 576;
const SUBBANDS: usize = 32;
const BINS_PER_SUBBAND: usize = GRANULE / 2 / SUBBANDS;

fn local_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus/local")
}

/// Per-subband level relative to the granule's own mean, in dB, for every granule sampled.
///
/// The quantity a subband-null detector would key on: an encoder that discards a subband for
/// a granule leaves a hole one subband wide (`rate / 64`) lasting exactly 576 samples.
/// Hann-windowed, because a rectangular window's 13 dB sidelobes would fill any such hole
/// with its neighbours' leakage long before it could be measured.
fn subband_null_depths(channel: &[f32]) -> Vec<f32> {
    use rustfft::num_complex::Complex32;
    use rustfft::FftPlanner;

    let window: Vec<f32> = (0..GRANULE)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / GRANULE as f32).cos())
        .collect();
    let fft = FftPlanner::new().plan_fft_forward(GRANULE);
    let mut buf = vec![Complex32::new(0.0, 0.0); GRANULE];
    let mut depths = Vec::new();

    let granules = channel.len() / GRANULE;
    for g in 0..granules {
        let frame = &channel[g * GRANULE..(g + 1) * GRANULE];
        let rms = (frame.iter().map(|&s| s * s).sum::<f32>() / GRANULE as f32).sqrt();
        // Silence is null in every subband and would swamp the percentiles with material
        // that says nothing about the encoder.
        if rms < 1e-4 {
            continue;
        }
        for (slot, (&s, &w)) in buf.iter_mut().zip(frame.iter().zip(window.iter())) {
            *slot = Complex32::new(s * w, 0.0);
        }
        fft.process(&mut buf);

        let levels: Vec<f32> = (0..SUBBANDS)
            .map(|b| {
                buf[b * BINS_PER_SUBBAND..(b + 1) * BINS_PER_SUBBAND]
                    .iter()
                    .map(|c| c.norm())
                    .sum::<f32>()
                    / BINS_PER_SUBBAND as f32
            })
            .collect();
        let mean: f32 = levels.iter().sum::<f32>() / SUBBANDS as f32;
        if mean <= 0.0 {
            continue;
        }
        depths.extend(levels.iter().map(|&l| {
            if l > 0.0 {
                20.0 * (l / mean).log10()
            } else {
                -200.0
            }
        }));
    }
    depths
}

#[test]
fn report_statistics_for_local_files() {
    let dir = local_dir();
    let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| matches!(x, "flac" | "wav" | "mp3" | "m4a" | "aac" | "ogg"))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();

    if files.is_empty() {
        eprintln!(
            "LOCAL: {} is empty — nothing to measure.\n\
             \n\
             Drop a lossless original and its own LAME V0 transcode in there to find out\n\
             whether V0 leaves anything measurable on real music. Produce the transcode with:\n\
             \n\
             \x20   ffmpeg -i original.flac -c:a libmp3lame -V 0 v0.mp3\n\
             \x20   ffmpeg -i v0.mp3 -c:a flac v0-transcode.flac\n\
             \n\
             Nothing in that directory is committed (see .gitignore).",
            dir.display()
        );
        return;
    }

    eprintln!(
        "\n{:<38} {:>7} {:>9} {:>9} {:>20} {:>8} {:>8} {:>8} {:>8}",
        "file", "rate", "edge_hz", "steep", "verdict", "grid_z", "null_p1", "null_p5", "null_p25"
    );

    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let decoded = match decode_file(&path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{name:<38} decode failed: {e}");
                continue;
            }
        };
        let Ok(spectral) = analyze_spectrum(&decoded) else {
            eprintln!("{name:<38} spectral analysis failed");
            continue;
        };
        let grid = analyze_mdct_grid(&decoded);
        let assessment = assess_transcode_risk(
            &spectral,
            decoded.sample_rate as f64 / 2.0,
            &decoded.encoder_tag_matches,
            &grid,
            &decoded.codec_short_name,
            &decoded.decode_status,
        );

        // The loudest channel, not the first: joint stereo means the signature can sit in
        // one channel, and a silent first channel would report nothing at all.
        let channel = decoded
            .channel_samples
            .iter()
            .max_by(|a, b| {
                let e = |c: &[f32]| c.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
                e(a).total_cmp(&e(b))
            })
            .expect("decoded audio has at least one channel");

        let mut depths = subband_null_depths(channel);
        depths.sort_by(f32::total_cmp);
        let pct = |p: f32| -> f32 {
            if depths.is_empty() {
                f32::NAN
            } else {
                depths[((depths.len() as f32 - 1.0) * p) as usize]
            }
        };

        eprintln!(
            "{:<38} {:>7} {:>9} {:>9.0} {:>20} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            name,
            decoded.sample_rate,
            spectral
                .encoder_edge_hz
                .map(|hz| format!("{hz:.0}"))
                .unwrap_or_else(|| "none".to_string()),
            spectral.rolloff_steepness_db_per_khz,
            format!("{:?}", assessment.verdict),
            grid.z_score,
            pct(0.01),
            pct(0.05),
            pct(0.25),
        );
    }

    eprintln!(
        "\nHow to read it: `grid_z` catches AAC (fires above 20). `edge_hz`/`steep` catch any\n\
         lossy encode that lowpasses. The three `null_*` columns are the open question — if a\n\
         real V0 transcode sits materially below its own lossless original there, a detector\n\
         has something to key on. On the synthetic corpus they match to the decimal."
    );
}
