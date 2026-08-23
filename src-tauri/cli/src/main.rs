//! Headless companion to the desktop app — runs the same analysis pipeline
//! (`nyquist_lib::analysis::analyze`) from the command line for scripting/batch use
//! (`find ~/Music -name '*.flac' -print0 | xargs -0 nyquist-cli --json`), the workflow
//! this project's target audience actually lives in day to day. No GUI dependencies.

use std::path::PathBuf;
use std::process::ExitCode;

use nyquist_lib::analysis::{self, AnalysisResult};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return if args.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS };
    }

    let json_output = args.iter().any(|a| a == "--json");
    let show_timing = args.iter().any(|a| a == "--timing");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

    if paths.is_empty() {
        eprintln!("error: no input file given");
        print_usage();
        return ExitCode::FAILURE;
    }

    let mut any_failed = false;
    for path in paths {
        match analysis::analyze_with_timings(&PathBuf::from(path)) {
            Ok((result, timings)) => {
                if show_timing {
                    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
                    eprintln!(
                        "{path}: decode {:.0}ms | signal {:.0}ms | DR14 {:.0}ms | spectral \
                         {:.0}ms | bit-depth {:.0}ms | mdct {:.0}ms | total {:.0}ms",
                        ms(timings.decode),
                        ms(timings.signal),
                        ms(timings.dynamic_range),
                        ms(timings.spectral),
                        ms(timings.bit_depth),
                        ms(timings.mdct_grid),
                        ms(timings.total)
                    );
                }
                if json_output {
                    match serde_json::to_string_pretty(&result) {
                        Ok(json) => println!("{json}"),
                        Err(e) => {
                            eprintln!("{path}: error: could not serialize result: {e}");
                            any_failed = true;
                        }
                    }
                } else {
                    print_human_readable(&result);
                }
            }
            Err(e) => {
                eprintln!("{path}: error: {e}");
                any_failed = true;
            }
        }
    }

    if any_failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn print_usage() {
    eprintln!(
        "usage: nyquist-cli [--json] <file>...\n\n\
         Analyzes one or more audio files and prints the results.\n\
         --json     print each result as a JSON object (one per line group), for scripting.\n\
         --timing   print per-stage wall-clock cost to stderr, for profiling.\n\
         With no --json, prints a human-readable summary per file."
    );
}

fn fmt_duration(seconds: f64) -> String {
    let m = (seconds / 60.0).floor() as u64;
    let s = (seconds % 60.0).round() as u64;
    format!("{m}:{s:02}")
}

fn print_human_readable(r: &AnalysisResult) {
    let fi = &r.file_info;
    let sa = &r.signal_analysis;
    let spa = &r.spectral_analysis;
    let ta = &r.transcode_assessment;

    println!("{}", fi.filename);
    println!("  Container/codec:   {} / {}", fi.container, fi.codec);
    println!("  Sample rate:       {} Hz (Nyquist {} Hz)", fi.sample_rate_hz, fi.nyquist_hz);
    println!(
        "  Bit depth/chans:   {}, {}ch",
        fi.bit_depth.map(|b| format!("{b}-bit")).unwrap_or_else(|| "unknown".to_string()),
        fi.channels
    );
    println!("  Duration:          {}", fmt_duration(fi.duration_seconds));
    println!(
        "  Integrity:         {}{}",
        match fi.integrity_verified {
            Some(true) => "verified",
            Some(false) => "FAILED (checksum mismatch)",
            None => "n/a",
        },
        if fi.decode_errors > 0 {
            format!(
                " ⚠ {} packet(s) failed to decode and were skipped — measurements below \
                 describe incomplete audio",
                fi.decode_errors
            )
        } else {
            String::new()
        }
    );
    println!("  Peak / True peak:  {:.1} dBFS / {:.1} dBTP", sa.peak_dbfs, sa.true_peak_dbtp);
    println!("  RMS:               {:.1} dBFS", sa.rms_dbfs);
    println!(
        "  LUFS / LRA:        {} / {}",
        sa.lufs_integrated.map(|v| format!("{v:.1} LUFS")).unwrap_or_else(|| "n/a".to_string()),
        sa.loudness_range_lu.map(|v| format!("{v:.1} LU")).unwrap_or_else(|| "n/a".to_string())
    );
    println!("  Clipped samples:   {}", sa.clipping_count_total);
    println!(
        "  Dynamic range:     {}",
        r.dynamic_range.dr14.map(|v| format!("DR{v}")).unwrap_or_else(|| "n/a".to_string())
    );
    if let Some(declared) = r.bit_depth_analysis.declared_bit_depth {
        let effective = r.bit_depth_analysis.effective_bit_depth;
        let note = match effective {
            Some(eff) if eff < declared => {
                format!(" ⚠ only {eff}-bit of real information detected, likely padded")
            }
            _ => String::new(),
        };
        println!("  Bit depth check:   declared {declared}-bit{note}");
    }
    let sr = &r.sample_rate_analysis;
    if sr.likely_upsampled {
        println!(
            "  Sample rate check: ⚠ declared {} Hz but content stops at {:.1} kHz ({:.0}% of \
             available bandwidth){}",
            sr.declared_sample_rate_hz,
            sr.content_bandwidth_hz / 1000.0,
            sr.bandwidth_ratio * 100.0,
            sr.sufficient_sample_rate_hz
                .map(|rate| format!(" — {rate} Hz would carry this losslessly"))
                .unwrap_or_default()
        );
    }
    println!(
        "  Spectral cutoff:   {:.1} kHz (rolloff {:.0} dB/kHz{})",
        spa.spectral_cutoff_hz / 1000.0,
        spa.rolloff_steepness_db_per_khz,
        spa.stopband_depth_db.map(|db| format!(", stopband {db:.0} dB down")).unwrap_or_default()
    );
    // Reported, not scored — see spectral.rs on why cutoff stability does not separate a
    // codec's lowpass from a mastering one.
    println!("  Cutoff stability:  ±{:.0} Hz over the track", spa.cutoff_stability_hz);
    if let Some(st) = &r.stereo_analysis {
        let flag = if st.dual_mono {
            " ⚠ dual mono (channels are bit-identical)"
        } else if st.mono_compatibility_risk {
            " ⚠ negative correlation — will cancel when summed to mono"
        } else if st.effectively_mono {
            " (negligible stereo width)"
        } else {
            ""
        };
        println!(
            "  Stereo image:      correlation {:.2}, side/mid {:.1} dB{}",
            st.correlation, st.side_to_mid_db, flag
        );
    }
    let grid = &r.mdct_grid;
    if grid.analyzed {
        println!(
            "  MDCT grid:         z={:.1} at offset {} (zeroed {:.1}% vs {:.1}% baseline){}",
            grid.z_score,
            grid.frame_offset,
            grid.zero_fraction_at_offset * 100.0,
            grid.zero_fraction_baseline * 100.0,
            if grid.grid_detected { " ⚠ AAC encoder grid" } else { "" }
        );
    }
    println!("  Verdict:           {:?} (confidence {:.0}%)", ta.verdict, ta.confidence_score * 100.0);
    for indicator in &ta.indicators {
        // `message` rather than the structured detail beside it: the CLI is English-only by
        // design, and this is the same prose the JSON report carries.
        println!("    - {}", indicator.message);
    }
    println!();
}
