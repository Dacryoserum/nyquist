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

# Exactly 1000 samples pinned at positive full scale followed by 1000 at negative full
# scale, then a quiet tone. Signed PCM is asymmetric — s16 runs -32768..=+32767 — and
# decoders normalize by the negative bound, so positive full scale arrives as 0.99997 and
# negative as exactly -1.0. A clipping test written as `abs() >= 1.0` therefore sees only
# half the clipped samples. `aevalsrc` is used rather than a loud sine because `n` lets the
# sample counts be exact, which makes the expected result an exact number (2000) instead of
# an approximation. See tests/corpus_smoke.rs::clipping_is_counted_symmetrically.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "aevalsrc=exprs='if(lt(n\,1000)\,1\,if(lt(n\,2000)\,-1\,0.1*sin(2*PI*440*n/44100)))':s=44100:d=1" \
  -sample_fmt s16 -c:a flac \
  "$CORPUS_DIR/calibration/fullscale_both_polarities.flac"

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

echo "== Narrowband / tonal fixtures (false-positive traps) =="

# Deliberate false-positive trap #2: genuinely lossless, but *tonal* rather than
# broadband. A sustained chord (three partials plus harmonics) has almost no energy
# between its peaks, so the spectrum falls from peak to noise floor within a couple of FFT
# bins. A rolloff-steepness measurement that only looks at "how many kHz between -20dB and
# -55dB" reads that near-vertical drop as an encoder brick wall and confidently accuses a
# perfectly authentic file. Real-world equivalents: solo piano/organ/synth pads, sparse
# electronic music, test tones. Must NOT be flagged.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "sine=frequency=220:sample_rate=44100:duration=${DURATION}" \
  -f lavfi -i "sine=frequency=330:sample_rate=44100:duration=${DURATION}" \
  -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=${DURATION}" \
  -filter_complex "amix=inputs=3:normalize=0,volume=6dB,pan=stereo|c0=c0|c1=c0" \
  -sample_fmt s16 -c:a flac \
  "$CORPUS_DIR/authentic_44k_tonal.flac"

echo "== Music-shaped spectrum (false-'inconclusive' trap) =="

# Genuinely lossless, full-bandwidth, but with the spectral *shape* of real music rather
# than the flat shape of white noise: energy concentrated in the low mids, high frequencies
# present all the way to Nyquist but far below the peak.
#
# Every other fixture here is white noise, which is flat — so a "cutoff" measured relative
# to the spectral peak lands at Nyquist for all of them and looks fine. Real music is 40 dB
# below its own peak by ~5 kHz while still carrying content to the top, which made that
# measurement read ~5 kHz and dragged every genuine FLAC to "indeterminate", while also
# making real hi-res look upsampled. These two fixtures are the regression guard for that:
# they must read as authentic, full-bandwidth, and NOT upsampled.
for RATE_FMT in "44100 s16" "96000 s32"; do
  set -- $RATE_FMT
  RATE=$1; FMT=$2
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "anoisesrc=color=pink:sample_rate=${RATE}:duration=10:seed=3" \
    -af "bass=g=10:f=120,treble=g=-14:f=6000,volume=-6dB,pan=stereo|c0=c0|c1=c0" \
    -sample_fmt "$FMT" -c:a flac \
    "$CORPUS_DIR/authentic_musiclike_$((RATE / 1000))k.flac"
done

echo "== Silence-padded transcode (false-negative trap) =="

# Same LAME 128 transcode as above, but with 3s of digital silence in front and behind —
# i.e. what virtually every real track looks like (lead-in, fade-out, gaps between
# movements). Digital silence decodes to exact zeros, which land on the dB floor; if the
# spectral averaging lets those frames raise the apparent noise floor, they bury the very
# encoder cutoff being measured and the detection silently switches off. Ground truth is
# unchanged from transcoded_mp3_128_44k: this IS a transcode and must still be caught.
ffmpeg -hide_banner -loglevel error -y \
  -i "$CORPUS_DIR/transcoded_mp3_128_44k.flac" \
  -af "adelay=3000|3000,apad=pad_dur=3" -sample_fmt s16 -c:a flac \
  "$CORPUS_DIR/transcoded_mp3_128_padded_silence.flac"

echo "== Sample-rate padding ('fake hi-res' by upsampling) fixtures =="

# Genuinely lossless 44.1kHz content resampled to 96kHz. No lossy encoder was ever
# involved, so this is NOT a transcode — but it is not real hi-res either: all content
# stops dead at the old 22.05kHz Nyquist, less than half the new one. The sample-rate twin
# of bit_depth.rs's padding detection. Explicitly requested by the
# transcode-heuristic-validation skill ("fichier réellement upsamplé sans mensonge de
# source").
ffmpeg -hide_banner -loglevel error -y \
  -i "$CORPUS_DIR/authentic_44k_noise.flac" -ar 96000 -sample_fmt s32 -c:a flac \
  "$CORPUS_DIR/upsampled_44k_to_96k.flac"

# The deliberately sneaky case the skill warns about: a lossy transcode that was then
# upsampled, so the encoder cutoff no longer sits anywhere near the file's stated Nyquist.
# Both problems at once — lossy source AND fake sample rate.
ffmpeg -hide_banner -loglevel error -y \
  -i "$CORPUS_DIR/transcoded_mp3_128_44k.flac" -ar 96000 -sample_fmt s32 -c:a flac \
  "$CORPUS_DIR/transcoded_mp3_128_upsampled_96k.flac"

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
