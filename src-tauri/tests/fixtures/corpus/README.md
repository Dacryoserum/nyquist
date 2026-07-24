# Test corpus

Synthetic audio fixtures with **known ground truth**, generated locally by
`../generate_corpus.sh` (requires `ffmpeg` with `libmp3lame` and `aac_at`). Nothing here is
sourced from real commercial music — every file is either synthesized (`anoisesrc`, `sine`
lavfi filters) or a real lossy-encode/decode round trip applied to that synthesized source.
This is deliberate: ground truth needs to be exactly known (not guessed at from files of
uncertain provenance), and it keeps the corpus copyright-clean to commit to a public repo.

Regenerate with:

```bash
./generate_corpus.sh
```

Deterministic (`seed=42`) — re-running should produce equivalent files.

## Calibration (`calibration/`)

Used to sanity-check `signal_analysis.rs` against a value computable by hand, per the
`dsp-correctness` skill's "test against a known-value reference signal" rule — not for
transcode detection.

| File | What it is | Expected result |
|---|---|---|
| `sine_1khz_minus3dbfs.flac` | 1kHz sine, -3dBFS peak, 44.1kHz/16-bit | peak ≈ -3.0 dBFS, RMS ≈ -6.0 dBFS (RMS of a sine is always ~3.01dB below its peak) |

## Authentic (genuinely lossless — no transcode)

| File | What it is |
|---|---|
| `authentic_44k_noise.flac` | Full-bandwidth white noise, 44.1kHz/16-bit. Flat spectrum to Nyquist, no encoder ever touched it. |
| `authentic_96k_noise.flac` | Same, but genuinely hi-res (96kHz/24-bit) — real energy well past 20kHz, unlike an upsampled fake. |
| `authentic_44k_lowpass_naturally.flac` | **False-positive trap.** White noise lowpassed at 12kHz *before* lossless encoding — genuinely lossless, but treble-poor by nature (simulates a legitimately dark master/mix). A naive "cutoff below X kHz ⇒ transcoded" heuristic must NOT flag this file. See `transcode-heuristic-validation` skill. |

## Transcoded (lossy source, re-encoded losslessly to hide it)

All derived from the *same* 44.1kHz/16-bit white-noise source WAV — only the lossy step
differs, so any spectral difference between them is attributable to the encoder, not the
source material.

Measured with `ffmpeg -af "highpass=f=<X>,astats"` (RMS energy above frequency X) during
corpus generation — see git history / regenerate and re-measure if you doubt these numbers:

| File | Encoder | Measured spectral cutoff | Difficulty |
|---|---|---|---|
| `transcoded_mp3_128_44k.flac` | LAME MP3 128kbps CBR | **~16kHz** — sharp, obvious roll-off | Easy — textbook LAME 128 lowpass |
| `transcoded_mp3_320_44k.flac` | LAME MP3 320kbps CBR | **~20.5kHz** — clear but subtler roll-off | Moderate |
| `transcoded_mp3_v0_44k.flac` | LAME MP3 VBR V0 (~245kbps) | **No detectable cutoff** — spectrum indistinguishable from `authentic_44k_noise` by this method | Hard — LAME's "transparent" preset doesn't lowpass |
| `transcoded_aac_256_44k.flac` | AAC 256kbps (Apple AudioToolbox) | **No detectable cutoff** — same as above | Hard — matches a real iTunes/Apple Music-sourced fake-lossless file |

**This split is intentional, not a fixture bug.** `mp3_v0` and `aac_256` exist specifically
to prove that spectral-cutoff detection alone cannot catch every transcode — see the spec's
original warning about "un encodeur MP3 bien réglé (LAME V0) peut monter à 19-20kHz." V0.3's
scoring needs indicators beyond raw cutoff frequency (e.g. quantization noise floor) to have
any chance on these two; a detector that only checks `cutoff_hz` should score them
"indéterminé," never a confident "authentic."

## Bit-depth padding ("fake hi-res")

A different quality issue from lossy transcoding — a file can be zero-padded to a wider
container bit depth without ever touching a lossy codec. See `bit_depth.rs` module docs.

| File | What it is |
|---|---|
| `bitdepth_fake24_from16.flac` | Genuinely 16-bit white noise, re-containered as 24-bit with no dithering — the common careless "fake hi-res" pattern. Must be detected as effectively 16-bit. |
| `bitdepth_genuine24.flac` | Same noise seed, genuinely generated at 24-bit resolution. Must NOT be flagged. |

## Adding your own local fixtures

Real (possibly copyrighted) files you want to test against locally — not for commit — go in
`corpus/local/`, which is gitignored.
