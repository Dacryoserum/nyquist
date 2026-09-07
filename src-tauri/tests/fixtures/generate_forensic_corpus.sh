#!/usr/bin/env bash
# Reproducible counterexamples, separate from the original corpus. No source audio is
# downloaded. Requires FFmpeg's native AAC encoder and the existing synthetic corpus.
# Existing outputs are not overwritten; regenerate into a new directory to compare builds.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORPUS_DIR="$SCRIPT_DIR/corpus"
OUT_DIR="${1:-$CORPUS_DIR/forensic}"
WORK_DIR="$(mktemp -d)"
mkdir -p "$OUT_DIR"

ffmpeg -hide_banner -loglevel error -n -f lavfi \
  -i "anullsrc=r=96000:cl=stereo:d=12" -c:a flac "$OUT_DIR/silence_96k.flac"

# A native 96 kHz source with a steep mastering filter; never encoded lossy or resampled.
ffmpeg -hide_banner -loglevel error -n \
  -f lavfi -i "anoisesrc=r=96000:d=5:seed=123:a=0.2" \
  -f lavfi -i "sinc=r=96000:lp=18000:lptaps=4095" \
  -filter_complex "[0:a][1:a]afir=dry=1:wet=1" -t 5 -sample_fmt s32 -c:a flac \
  "$OUT_DIR/native_96k_fir.flac"

# A sharp resampling filter is not evidence of a lossy history either.
ffmpeg -hide_banner -loglevel error -n -i "$CORPUS_DIR/authentic_44k_noise.flac" \
  -af "aresample=96000:filter_size=512:cutoff=0.98" -sample_fmt s32 -c:a flac \
  "$OUT_DIR/lossless_resampled_96k.flac"

# KBD long windows differ from the AudioToolbox sine-window fixtures in the old corpus.
ffmpeg -hide_banner -loglevel error -n -i "$CORPUS_DIR/authentic_dynamic_stereo_44k.flac" \
  -c:a aac -b:a 256k "$WORK_DIR/native_aac.m4a"
ffmpeg -hide_banner -loglevel error -n -i "$WORK_DIR/native_aac.m4a" \
  -c:a flac "$OUT_DIR/native_aac_256.flac"

# Ultrasonic energy added *after* MP3 compression defeats an authenticity claim based on
# high-frequency energy alone. The genuine reference and seed are fixed by the old corpus.
ffmpeg -hide_banner -loglevel error -n -i "$CORPUS_DIR/transcoded_dynamic_mp3_v0_44k.flac" \
  -f lavfi -i "anoisesrc=r=96000:d=10:seed=124:a=0.01" \
  -filter_complex "[0:a]aresample=96000[m];[m][1:a]amix=inputs=2:normalize=0" \
  -t 10 -c:a flac "$OUT_DIR/mp3_with_added_noise_96k.flac"

# Only the final two (side) channels of a declared 7.1 layout carry signal. Ebur128's
# default map ignores them. Independent reference: FFmpeg ebur128 reads about -18.5 LUFS.
ffmpeg -hide_banner -loglevel error -n -f lavfi \
  -i "aevalsrc=0|0|0|0|0|0|0.1*sin(2*PI*1000*t)|0.1*sin(2*PI*1000*t):s=48000:d=4:c=7.1" \
  -c:a pcm_s16le "$OUT_DIR/side_only_71.wav"

echo "Forensic fixtures generated in $OUT_DIR. AAC intermediate kept in $WORK_DIR."
