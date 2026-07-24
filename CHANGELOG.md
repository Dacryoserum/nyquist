# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer once
releases start shipping.

## [Unreleased]

### Added

- Project scaffolding: agent workflow (`AGENTS.md`, `.claude/`), license (MIT), changelog.
- Tauri + SvelteKit application shell (V0.1): file picker, raw results display.
- Audio decoding via `symphonia` (FLAC, MP3, AAC, ALAC, WAV, OGG).
- Technical file metadata: container, codec, sample rate, bit depth, duration, average
  bitrate, Nyquist frequency.
- Signal analysis: peak, RMS, crest factor, and per-channel clipping count; integrated
  loudness (LUFS) and true peak via `ebur128` (ITU-R BS.1770 / EBU R128).
- Synthetic, reproducible test corpus (`src-tauri/tests/fixtures/corpus/`) with known
  ground truth for future transcode-detection validation: authentic and lossy-transcoded
  (MP3 128/320/V0, AAC 256) fixtures, plus a deliberate false-positive trap (naturally
  treble-poor but genuinely lossless audio).
- Integration tests validating signal analysis against a known-value reference signal and
  against every corpus fixture.
- Spectrogram computation (FFT via `rustfft`, Hann-windowed STFT) and a raw spectral
  cutoff measurement, downsampled and quantized in Rust before transmission (never a dense
  JSON matrix on IPC). Cross-validated against independent ffmpeg measurements on the test
  corpus.
- Redesigned UI: dark theme, card-based dashboard, icon set, canvas-rendered spectrogram
  with an inferno colormap and a "Quiet → Loud" legend.
- Rolloff steepness measurement (dB/kHz) alongside spectral cutoff position — needed
  because cutoff position alone is unreliable on real, dynamic-range-heavy music (see
  `.claude/CONTEXT.md`).
- Transcode likelihood scoring (`transcode_detect.rs`): 3-state verdict (probably
  authentic / probably transcoded / indeterminate) with a bounded confidence score and
  human-readable indicators, never a binary certainty. Validated against the test corpus
  with an explicit false-positive/negative report in `tests/corpus_smoke.rs` (0 false
  positives, 2 documented undetectable cases: LAME V0, AAC 256kbps).
- Audio playback: native `<audio>` element streamed via Tauri's `asset://` protocol
  (seekable, no whole-file JS memory load), with a play/pause/scrub bar and click-to-seek
  directly on the spectrogram.
- FLAC integrity verification: checks the file's own embedded checksum (STREAMINFO MD5)
  against what was actually decoded, via symphonia's built-in decoder verification.
- DR14 (Pleasurize Music Foundation Dynamic Range) — the metric this community compares
  against the public loudness-war database, distinct from the existing crest factor.
  Algorithm verified against the open-source `dr14_t.meter` reference implementation.
- Loudness Range (LRA, EBU Tech 3342) alongside integrated LUFS.
- Encoder tag fingerprint scan (`tags.rs`): flags known lossy-encoder signatures (LAME,
  iTunes, FhG, ...) left over in container tags — a corroborating signal for
  `transcode_assessment`, never used on its own to claim authenticity.
- Bit-depth padding ("fake hi-res") detection (`bit_depth.rs`): flags a file whose real
  content never used the bit depth its container declares (e.g. 16-bit content
  zero-padded into a 24-bit FLAC) — a distinct quality issue from lossy transcoding.
- Spectral cutoff over time: the same cutoff measurement computed per spectrogram time
  window instead of once for the whole file, to catch a transcode that only patches in
  real high-frequency content for part of a track.
- JSON report export from the UI.
- `nyquist-cli`: a headless companion binary for scripting/batch analysis, sharing the
  exact same analysis pipeline as the desktop app.

### Changed

- `AnalysisResult` (backend↔frontend contract) now includes `spectral_analysis`,
  `transcode_assessment`, `dynamic_range`, `encoder_tag_matches`, and `bit_depth_analysis`.
- Removed the unused `rodio` dependency (added speculatively in V0.1, never wired up) in
  favor of the browser-native audio element for playback.
- Extracted the analysis pipeline into a shared `analysis.rs` module, used by both the
  Tauri command and the new CLI binary.
