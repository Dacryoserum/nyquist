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
| `fullscale_both_polarities.flac` | Exactly 1000 samples at +32767, then 1000 at -32768, then a quiet tone | clipping count = **2000**. Signed PCM is asymmetric and decoders normalize by the negative bound, so positive full scale arrives as 0.99997 and negative as exactly -1.0 — a naive `abs() >= 1.0` test sees only half of these and reports 1000. |

## Authentic (genuinely lossless — no transcode)

| File | What it is |
|---|---|
| `authentic_44k_noise.flac` | Full-bandwidth white noise, 44.1kHz/16-bit. Flat spectrum to Nyquist, no encoder ever touched it. |
| `authentic_96k_noise.flac` | Same, but genuinely hi-res (96kHz/24-bit) — real energy well past 20kHz, unlike an upsampled fake. |
| `authentic_44k_lowpass_naturally.flac` | **False-positive trap.** White noise lowpassed at 12kHz *before* lossless encoding — genuinely lossless, but treble-poor by nature (simulates a legitimately dark master/mix). A naive "cutoff below X kHz ⇒ transcoded" heuristic must NOT flag this file. See `transcode-heuristic-validation` skill. |
| `authentic_44k_tonal.flac` | **False-positive trap #2.** A sustained three-note chord, genuinely lossless. Tonal content has almost no energy between its partials, so its spectrum falls from peak to noise floor inside a couple of FFT bins. A steepness measurement built on "how many kHz between the -20dB and -55dB crossings" divides by a near-zero span and reads that as an infinitely steep encoder brick wall. This file, and the calibration sine above, were both reported as *probably transcoded at 80% confidence* before `measure_rolloff_steepness` was rewritten. Real-world equivalents: solo piano, organ, synth pads, sparse electronic. |

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
| `transcoded_mp3_v0_44k.flac` | LAME MP3 VBR V0 (~245kbps) | **No detectable cutoff** — spectrum indistinguishable from `authentic_44k_noise` by this method | **Still uncaught.** LAME's "transparent" preset doesn't lowpass, and its hybrid filterbank defeats the MDCT grid test |
| `transcoded_aac_256_44k.flac` | AAC 256kbps (Apple AudioToolbox) | **No detectable cutoff** spectrally — **caught by the MDCT grid** at z = 215 | Was the hardest case; the grid sweep closes it |

**This split is intentional, not a fixture bug.** `mp3_v0` and `aac_256` exist specifically
to prove that spectral-cutoff detection alone cannot catch every transcode — see the spec's
original warning about "un encodeur MP3 bien réglé (LAME V0) peut monter à 19-20kHz." V0.3's
scoring needs indicators beyond raw cutoff frequency (e.g. quantization noise floor) to have
any chance on these two; a detector that only checks `cutoff_hz` should score them
"indéterminé," never a confident "authentic."

> **Half of this is now covered.** `mdct_grid.rs` catches every AAC case here by reading the
> encoder's MDCT quantization grid rather than the spectral envelope — see the table at the
> end of this file. `transcoded_mp3_v0_44k.flac` still comes out `ProbablyAuthentic`, which
> is the tool vouching for a transcode rather than merely missing it, and MP3's hybrid
> filterbank cannot be inverted the same way.

## Non-stationary, true-stereo material

Everything above is **stationary noise in dual-mono**, and both of those limit what the
corpus can validate:

- `-ac 2` upmixes a mono source, so L and R come out bit-identical — `side = L-R` is digital
  silence. Anything that reads the stereo image is untestable on those files. (`stereo.rs`
  reports this: `authentic_44k_noise.flac` correctly flags `dual_mono`.)
- Stationary noise is the most favourable material a perceptual encoder ever sees. Codecs
  betray themselves on *change*, and 5 seconds of steady noise exercises none of it.

These share one source built from two independently seeded pink-noise channels, with quiet
passages every 2s, transients every 0.5s, and sustained tones — the closest this corpus gets
to real music. Measured with the current detector:

| File | Encoder | Measured | Verdict today |
|---|---|---|---|
| `authentic_dynamic_stereo_44k.flac` | none — lossless | no edge | `Indeterminate` ✅ (no evidence either way; see below) |
| `transcoded_dynamic_mp3_128_44k.flac` | LAME MP3 128k | 16.8kHz @ 72 dB/kHz | `ProbablyTranscoded` 72% ✅ |
| `transcoded_dynamic_mp3_v0_44k.flac` | LAME MP3 V0 | no edge | `Indeterminate` ⚠ (missed, not vouched for) |
| `transcoded_dynamic_aac_256_44k.flac` | AAC 256k (Apple) | no edge | `ProbablyTranscoded` ✅ (caught by the MDCT grid sweep) |
| `transcoded_dynamic_aac_128_44k.flac` | AAC 128k (Apple) | 18.3kHz @ 27 dB/kHz | `ProbablyTranscoded` ✅ (caught by the MDCT grid sweep) |

**A note on `Indeterminate` rows.** A clean spectral sweep is an absence of evidence, and
since the verdict rework it produces `Indeterminate` rather than `ProbablyAuthentic` —
including on genuinely authentic 44.1 kHz files. That is the honest reading: on a CD-rate
file with no detectable cutoff, a lossless master and a LAME V0 transcode are not
distinguishable by any method in this project. `ProbablyAuthentic` now requires positive
evidence (real content in the top of a hi-res band), which only the 96 kHz fixtures can
supply. The table above reads worse than the previous one and is more truthful: no file that
is actually a transcode is vouched for any more.

The AAC 128 row is the one the stationary corpus could not have shown: on flat noise the
same encoder measures ~106 dB/kHz and is caught comfortably, but on this material its
transition is gradual enough to fall under the 40 dB/kHz gate. **The gap is a property of
the material, not of the bitrate.**

## False-positive traps on non-stationary material

| File | What it is |
|---|---|
| `authentic_decay_to_silence_44k.flac` | Lossless tonal content produced "in the box": partials at decreasing amplitudes (1/h, like a real instrument) decaying toward digital silence between notes. The quiet partials cross the 16-bit floor well before the loud ones, so the high band empties while the mids still ring — the same shape a codec produces when it zeroes high bands, with no encoder anywhere. Real-world equivalents: solo piano VST, sparse electronic music, anything mixed without a noise floor. |
| `authentic_bass_only_44k.flac` | Loud low-frequency content with essentially no treble: the high band sits at the floor while the file as a whole is loud. Guards any rule of the form "high band is empty while the signal is strong, therefore lossy". |

## What was tried against the blind spot, and why nothing shipped

Prototyped against this corpus plus a larger throwaway set. Recorded here so the next
attempt starts from the results rather than from the same five ideas.

**What the failures have in common**, and where the next attempt should therefore start:
every one of them reads the decoded *spectrum* — where energy is, or is not. On a transparent
encode there is nothing missing to find, which is what "transparent" means. The literature's
answer is not a better spectral measurement but a different question: Derrien (JAES 67(3),
2019, *Detection of Genuine Lossless Audio Files*) looks for **quantization error** — decoded
coefficients clustering on a quantizer's reconstruction levels — which is present in every
coefficient the encoder touched, including the ones it kept. It reports null false positives
over 1576 files for AAC and states the approach carries over to MP3. Doing that for MP3 needs
the hybrid filterbank actually inverted: the 32-band polyphase analysis stage (a normative
512-coefficient prototype window from ISO/IEC 11172-3, to be sourced rather than recalled)
followed by the 18-point MDCT per subband. That is a piece of work, not a threshold.

| Approach | Result |
|---|---|
| **Spectral holes** — high bands zeroed during quiet passages | Catches LAME V0 with a wide margin (31.6% of frames vs 0.0% for lossless). **But** an in-the-box piano decaying to digital silence scores 22.5-23.3%, *above* a real LAME V2 transcode. Tightening the loudness gate zeroes every codec (0.00) while the piano stays at 22.55 — the codec's holes live only in quiet passages, exactly where the piano's do. No threshold keeps one without the other. |
| **Codec frame grid** — periodicity at 1152 (MP3) / 1024 (AAC) samples in the HF envelope | No signal: all fixtures score under 3 σ above background. MDCT's 50%-overlapped TDAC windowing cross-fades quantization noise across frame boundaries, so there is no abrupt periodicity to find. |
| **Cutoff stability over time** — "a codec's lowpass is a fixed filter" | Comes out **backwards**. A mastering lowpass is also a fixed filter: the authentic naturally-dark fixtures measure 0 Hz of drift while real LAME/AAC transcodes wander 33-252 Hz. Reported by `spectral.rs` as information; must never be scored. |
| **Joint-stereo / intensity-stereo collapse** — side channel vanishing in the high bands | No separation. High-band side/mid sits within a decibel of the lossless source for V0 and AAC 256 alike (14.5-15.3 dB across the set). Measured and reported by `stereo.rs`, not scored. |
| **Subband/granule grid** — nulls aligned to MP3's own lattice (32 subbands of `rate/64`, 576-sample granules), swept over all 576 granule offsets the way `mdct_grid` sweeps MDCT offsets | The idea was that even where a plain MDCT cannot invert MP3's hybrid filterbank, a *discarded subband* still leaves a 689 Hz-wide hole lasting exactly one granule, and that holes landing on one grid would separate from holes music makes anywhere. **The phenomenon is not there at V0.** Per-subband level relative to the granule mean, deepest and 1st-percentile values: `transcoded_mp3_v0_44k` −8.5 / −5.4 dB against `authentic_44k_noise` −8.4 / −5.4; `transcoded_dynamic_mp3_v0_44k` −31.6 / −23.9 against `authentic_dynamic_stereo_44k` −28.7 / −23.6. Identical to the decimal. LAME V0 on this material discards nothing, so there is no null to align. The deep nulls that *do* appear (`mp3_128` at −94 dB, `mp3_320` at −73) sit in the lowpass region the rolloff test already catches, and the tonal authentic fixture reaches −63.9 dB at its 25th percentile — so a null count accuses it, reproducing the spectral-holes trap one row up. |

## The corpus's own blind spot: it is made of noise

Every fixture here comes from `anoisesrc` — pink or white, EQ'd, gated, decorrelated. That
was the right trade for reproducibility and licensing, and for the rolloff measurement it is
sound: a lowpass is a lowpass whatever passes through it.

It cannot validate a detector for **transparent MP3**. A perceptual encoder betrays itself by
discarding what a listener would not miss, and noise is the material where it discards least
— incompressible and unmaskable, so at V0 the encoder spends bits everywhere. Measured:
`transcoded_mp3_v0_44k` and `authentic_44k_noise` have identical per-subband statistics to
the decimal (min −8.5 / −8.4 dB, p1 −5.4 / −5.4). Any V0 detector, correct or broken, scores
the same on both. Nothing here can confirm or refute one.

Real music has quiet passages, tonal masking and decays — where an encoder at V0 actually
starves. `tests/local_probe.rs` reports the statistics a detector would have to separate on,
for any file dropped into `corpus/local/` (gitignored). Point it at a lossless original and
its own LAME V0 transcode:

```sh
ffmpeg -i original.flac -c:a libmp3lame -V 0 v0.mp3
ffmpeg -i v0.mp3 -c:a flac corpus/local/v0-transcode.flac
cd src-tauri && cargo test --release --test local_probe -- --nocapture
```

If the `null_*` columns differ between the pair, a subband detector is worth building. If
they do not, the spectral route is closed for V0 and only quantization-error analysis
remains. **That measurement has to come before the implementation**, not after — the
`transcode-heuristic-validation` skill is explicit that an unvalidated scoring change is not
to be shipped, and this corpus cannot provide the validation.

## Silence padding (false-negative trap)

| File | What it is |
|---|---|
| `transcoded_mp3_128_padded_silence.flac` | The same LAME 128 transcode as above with 3s of digital silence before and after — i.e. what nearly every real track looks like (lead-in, fade-out, gaps between movements). **Ground truth is unchanged: this IS a transcode.** Digital silence decodes to exact zeros, which land on the dB floor; when the spectral averaging let those frames into the steady-state envelope they raised the apparent noise floor above the encoder's real stopband and buried the cutoff. Measured steepness went 191 → 0 dB/kHz and the verdict flipped from "probably transcoded" to "indeterminate". |

## Sample-rate padding ("fake hi-res" by upsampling)

The sample-rate counterpart to the bit-depth section below, and the case the
`transcode-heuristic-validation` skill asks for by name ("fichier réellement upsamplé sans
mensonge de source"). Handled by `sample_rate.rs`, deliberately **not** by the transcode
verdict: these files are lossless end to end, so calling them "transcoded" would name the
wrong defect.

| File | What it is |
|---|---|
| `upsampled_44k_to_96k.flac` | Genuinely lossless 44.1kHz noise resampled to 96kHz. No lossy encoder ever touched it, but content stops at ~24.8kHz — barely half of the 48kHz Nyquist it claims. Must be flagged as upsampled and must NOT be called a transcode. |
| `transcoded_mp3_128_upsampled_96k.flac` | The sneaky combination the skill warns about: lossy source, then upsampled so the encoder cutoff no longer sits near the declared Nyquist. Both defects must be reported — transcode *and* inflated sample rate. |

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
