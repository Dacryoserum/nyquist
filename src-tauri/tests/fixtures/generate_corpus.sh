#!/usr/bin/env bash
# Regenerates the synthetic test corpus in tests/fixtures/corpus/ from scratch.
#
# Every fixture is synthesized locally (noise/sine via ffmpeg lavfi, real lossy
# encode/decode passes via ffmpeg) rather than sourced from real commercial music, so:
#   - ground truth (authentic vs. transcoded) is exactly known, not guessed
#   - the corpus can be committed to a public repo with no copyright ambiguity
#   - it is fully reproducible: same seed in, same files out
#
# Requires: ffmpeg with libmp3lame and aac_at (AudioToolbox) encoders — both present in
# the Homebrew ffmpeg build. See corpus/README.md for what each fixture is testing.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORPUS_DIR="$SCRIPT_DIR/corpus"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

SEED=42
DURATION=5

echo "== Calibration fixture (known-value sanity check for signal_analysis.rs) =="

# 1kHz sine at -3 dBFS peak. For a sine wave, RMS = peak / sqrt(2), i.e. RMS is always
# ~3.01 dB below peak. So this fixture has a known expected result: peak ~= -3.0 dBFS,
# RMS ~= -6.0 dBFS. See dsp-correctness skill and tests/calibration.rs.
#
# Two ffmpeg quirks to compensate for here, confirmed empirically (see git history /
# corpus/README.md if this ever needs re-deriving):
#   - the `sine` source filter's native peak amplitude is 0.125 (-18.06dBFS), NOT 1.0 as
#     its docs might suggest (unlike `anoisesrc`, `sine` has no `amplitude` option) — so
#     "volume=-3dB" alone lands at ~-21dB, not -3dB. +15.0637dB compensates exactly.
#   - `-ac 2` mono->stereo upmix applies its own ~-3dB/channel normalization; `pan=stereo|
#     c0=c0|c1=c0` duplicates the channel at unity gain instead, avoiding that surprise.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "sine=frequency=1000:sample_rate=44100:duration=${DURATION}" \
  -af "volume=15.0637dB,pan=stereo|c0=c0|c1=c0" -sample_fmt s16 -c:a flac \
  "$CORPUS_DIR/calibration/sine_1khz_minus3dbfs.flac"

echo "== Authentic (genuinely lossless, no transcode) fixtures =="

# Full-bandwidth white noise at standard CD-quality sample rate. A real lossless file:
# flat energy spectrum all the way to Nyquist (22.05 kHz), no encoder cutoff artifact.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "anoisesrc=color=white:sample_rate=44100:duration=${DURATION}:seed=${SEED}" \
  -ac 2 -sample_fmt s16 -c:a flac \
  "$CORPUS_DIR/authentic_44k_noise.flac"

# Same, but genuinely hi-res (96kHz/24-bit) — energy should extend well past 20kHz,
# unlike an upsampled-from-lossy fake.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "anoisesrc=color=white:sample_rate=96000:duration=${DURATION}:seed=${SEED}" \
  -ac 2 -sample_fmt s32 -c:a flac -sample_fmt s32 \
  "$CORPUS_DIR/authentic_96k_noise.flac"

# Deliberate false-positive trap: genuinely lossless but naturally treble-poor (lowpass
# applied before lossless encoding, not after a lossy round-trip). A naive "cutoff below
# 16kHz => transcoded" heuristic must NOT flag this file. See transcode-heuristic-
# validation skill.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "anoisesrc=color=white:sample_rate=44100:duration=${DURATION}:seed=${SEED}" \
  -af "lowpass=f=12000" -ac 2 -sample_fmt s16 -c:a flac \
  "$CORPUS_DIR/authentic_44k_lowpass_naturally.flac"

echo "== Transcoded (lossy source re-encoded as lossless) fixtures =="

# Single lossless intermediate WAV shared by every transcode below, so all of them are
# genuinely derived from the same source signal — only the lossy step differs.
SRC_WAV="$WORK_DIR/src_44k_noise.wav"
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "anoisesrc=color=white:sample_rate=44100:duration=${DURATION}:seed=${SEED}" \
  -ac 2 -sample_fmt s16 \
  "$SRC_WAV"

transcode_via_mp3() {
  local bitrate_args="$1" out_name="$2"
  local tmp_mp3="$WORK_DIR/tmp_$out_name.mp3"
  ffmpeg -hide_banner -loglevel error -y -i "$SRC_WAV" -c:a libmp3lame $bitrate_args "$tmp_mp3"
  ffmpeg -hide_banner -loglevel error -y -i "$tmp_mp3" -c:a flac "$CORPUS_DIR/$out_name.flac"
}

transcode_via_aac() {
  local bitrate_args="$1" out_name="$2"
  local tmp_m4a="$WORK_DIR/tmp_$out_name.m4a"
  ffmpeg -hide_banner -loglevel error -y -i "$SRC_WAV" -c:a aac_at $bitrate_args "$tmp_m4a"
  ffmpeg -hide_banner -loglevel error -y -i "$tmp_m4a" -c:a flac "$CORPUS_DIR/$out_name.flac"
}

# CBR 320kbps MP3 — the easiest transcode case, but still shows LAME's ~20.5kHz lowpass.
transcode_via_mp3 "-b:a 320k" "transcoded_mp3_320_44k"
# CBR 128kbps MP3 — aggressive, unambiguous cutoff well below 20kHz.
transcode_via_mp3 "-b:a 128k" "transcoded_mp3_128_44k"
# VBR V0 (~245kbps, LAME's "transparent" preset) — the hard, audiophile-fooling case:
# high perceptual quality, but still has a lossy-encoder cutoff a spectral analysis
# should catch even though the file "sounds" transparent.
transcode_via_mp3 "-q:a 0" "transcoded_mp3_v0_44k"
# AAC 256kbps via Apple's encoder — matches the common real-world case of a fake-lossless
# file that actually traces back to an iTunes/Apple Music purchase.
transcode_via_aac "-b:a 256k" "transcoded_aac_256_44k"

echo "== Bit-depth padding ('fake hi-res') fixtures =="

# Genuine 16-bit source, then simply re-containered as 24-bit with no new information
# (no dithering) — the common real-world "fake hi-res" pattern this project's
# bit_depth.rs is meant to catch. Same noise seed as the 16-bit corpus fixtures above so
# it's directly comparable.
SRC_16BIT_WAV="$WORK_DIR/src_16bit.wav"
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "anoisesrc=color=white:sample_rate=44100:duration=${DURATION}:seed=${SEED}" \
  -ac 2 -sample_fmt s16 \
  "$SRC_16BIT_WAV"
ffmpeg -hide_banner -loglevel error -y -i "$SRC_16BIT_WAV" -sample_fmt s32 -c:a flac \
  "$CORPUS_DIR/bitdepth_fake24_from16.flac"

# Same seed, but genuinely generated at 24-bit resolution — must NOT be flagged.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "anoisesrc=color=white:sample_rate=44100:duration=${DURATION}:seed=${SEED}" \
  -ac 2 -sample_fmt s32 -c:a flac \
  "$CORPUS_DIR/bitdepth_genuine24.flac"

echo "== Done. Fixtures written to $CORPUS_DIR =="
