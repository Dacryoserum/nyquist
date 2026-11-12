# Detecting a lossy history: evidence and limits

Reviewed 2026-11-12. This note separates published research, Nyquist's implementation,
and measurements on its own corpora. None is a guarantee of authenticity.

## Current state (2026-11-12): two structural searches, and a tested negative

The verdict rests on two searches of the same kind, one per codec family:

- `mdct_grid.rs` re-analyzes the signal on AAC's long-block MDCT at all 1024 frame offsets.
- `mp3_grid.rs` re-analyzes it through MP3's hybrid filterbank — the 32-band polyphase
  stage with the normative 512-tap window (ISO/IEC 11172-3 Table B.3), the 36-point
  sine-windowed MDCT per subband, frequency inversion and the alias-reduction butterflies —
  at all 576 granule offsets.

Both use one statistic: the share of coefficients more than 40 dB below the RMS of their
own small neighbourhood (32 coefficients for AAC, 36 for MP3). A quantizer leaves exactly
that inside a band — coefficients are either gone or at least one step large — and only at
the encoder's own alignment. Both require the winning offset to exceed a robust deviation
of 12 on the sweep and again on audio the sweep did not use.

The earlier statistic (coefficients 70 dB below the *frame's* RMS) mostly counts "the treble
is quiet", which raises every offset alike. On real music it missed 3 of 28 AAC round trips
and gave MP3 scores as low as 3.9 on LAME V0; the local form misses none of either.

### Validation on real recordings

14 excerpts of 45 s, one per album, from 14 different commercial releases (orchestral
scores, electronic, rap; 16/44.1, 24/44.1 and 24/48 sources, all brought to 16-bit 44.1 kHz).
Each was encoded and decoded back to PCM seven ways. One source turned out to carry an AAC
grid already (see below) and is counted with the lossy files for the AAC search.

| Material | Files | AAC search, min(sweep, confirm) | MP3 search, min(sweep, confirm) |
| --- | ---: | --- | --- |
| Lossless sources | 13 | highest 2.1 (sweep ≤ 4.1) | highest 1.6 (sweep ≤ 3.7) |
| LAME V0 | 14 | — | lowest 27.6 |
| LAME V2 | 14 | — | lowest 41.9 |
| LAME 320 CBR | 14 | — | lowest 16.6 |
| LAME 192 CBR | 14 | — | lowest 53.2 |
| LAME 128 CBR | 14 | — | lowest 59.5 |
| AAC 256 (AudioToolbox) | 14 | lowest 14.9 | highest sweep 3.9 |
| AAC 192 (FFmpeg native) | 14 | lowest 42.1 | highest sweep 3.6 |

At a threshold of 12: 70/70 MP3 and 28/28 AAC derivatives detected, 0/13 lossless sources
accused. Every MP3 derivative lands on the same granule offset (528, the decoder's delay
modulo 576), which a spurious peak would not do. The MP3 scores of AAC files and the AAC
scores of MP3 files stay at the lossless level (sweep ≤ 4.2): the searches do not
cross-trigger. The one exception is informative: the 320 kbps MP3 made from the source that
already carried an AAC grid still scores 15 on the AAC search, through a second encode.

These are **development figures**: the thresholds were chosen after looking at them, the
sources are few, one decoder (FFmpeg) produced every derivative, and no file was processed
after decoding. They are not a held-out accuracy estimate.

### What `probably_authentic` now means

Both searches run, both come back under 6 (the highest any unencoded file reached was 5.6,
over 400+ full tracks), and none of the following holds:

- the sample rate is above 48 kHz — MP3 and AAC do not operate there, so a lossy source
  must have been resampled, which moves it off the encoder's sample grid and erases what
  both searches read (`mp3_with_added_noise_96k` and `transcoded_mp3_128_upsampled_96k`
  both pass the grid searches clean, which is why no hi-res file is cleared);
- a brick-wall edge (≥ 40 dB/kHz) with no grid behind it — the footprint a codec outside
  these two families would leave (Opus, Vorbis, WMA), and also what a mastering filter
  leaves, so it withholds the verdict without accusing;
- a lossy-encoder tag, a failed checksum, a partial decode, or a file too short or too
  quiet to search.

This is a tested negative, which the previous `probably_authentic` (a clean spectrum) was
not: the same searches catch the LAME V0 fixtures that the old rule vouched for. It is
still not proof. A lossy file resampled (44.1 → 48 kHz), time-stretched, re-equalized or
dithered heavily after decoding can pass, as can a codec these searches do not model, and
short-block-only passages are not read. The evidence strength is capped at "moderate".

### Known misses

- `transcoded_mp3_320_44k` (white noise at 320 kbps CBR): the encoder has bits to spare on
  stationary noise and removes no coefficient, so there is no sparsity to find (MP3 score
  2.2). Left `indeterminate` by its 20.4 kHz brick wall. Real music at 320 kbps is caught
  14/14, but this shows the search reads *removed* coefficients, not the quantizer's step
  itself — a very high bitrate on dense material is its weak side.
- Anything resampled after the lossy step.

## Published work

- Kim & Rafii, **Lossy Audio Compression Identification**, EUSIPCO 2018
  ([full paper](https://eurasip.org/Proceedings/Eusipco/Eusipco2018/papers/1570436395.pdf)).
  Searches transform/window/alignment hypotheses using adjacent log-energy differences
  and combines frame positions across blocks. It evaluates identification *among lossy
  formats*, not false accusations of lossless masters. Nyquist borrows the window-search
  direction, not its statistic or reported accuracy.
- Hennequin, Royo-Letelier & Moussallam, **Codec independent lossy audio compression
  detection**, ICASSP 2017
  ([author-hosted paper](https://romain-hennequin.fr/doc/ICASSP2017_Deezer_quality_estimation.pdf),
  [DOI](https://doi.org/10.1109/ICASSP.2017.7952251)). A CNN on spectrograms is trained across
  codecs and bitrates; the authors report robustness to resampling. This is a promising
  direction for MP3 and processed material, but an architecture is not a validated model
  for this app. The accessible abstract and indexed paper text informed this review;
  no model weights or training data were imported.
- García-Hernández & Gómez-Flores, **Detection of AAC compression using MDCT-based features
  and supervised learning**, published online 2021, journal volume 2022
  ([publisher abstract](https://www.tandfonline.com/doi/abs/10.1080/0952813X.2021.1882003)).
  Uses MDCT variance, dimensionality reduction and supervised classifiers. The abstract
  reports 97% compressed/uncompressed accuracy in its experiment. Only the abstract was
  available for this review; that figure is not a Nyquist performance estimate.

## 2026-09 revision: a broader, confirmed AAC hypothesis

Historical: the statistic and threshold described here were replaced on 2026-11-12 (see
the current state above); the window search and the confirmation pass are unchanged.

`mdct_grid.rs` now tests sine and Kaiser–Bessel-derived long windows. It retains the existing
near-zero coefficient statistic and the robust deviation threshold of 20. Both the initial
1024-offset sweep and a second pass must exceed that threshold. Confirmation excludes all
samples used in the first pass and measures its own background at 32 spaced control offsets
(omitting the candidate's immediate neighbourhood). At least eight confirmation frames
must be available. The selected window and both scores are exposed in JSON and the UI.

This has concrete value: the new native FFmpeg AAC 256 fixture fails the old sine-only
search but passes the KBD search. The existing three AudioToolbox AAC fixtures remain
detected. Gain, polarity inversion and a 137-sample crop of the new fixture are tested too.
These transformations are robustness checks on **one source**, not extra independent
successes. The mathematical window overlap identity and direct MDCT definition are tested.

The search still covers one energetic channel and long AAC blocks only. Short blocks,
window transitions, resampling, noise, editing and codec tools can obscure the grid. A
negative result therefore cannot rule out AAC. MP3 is not covered by this grid statistic.
The two score thresholds and evidence weights are heuristics, not calibrated probabilities.

## Critical inference corrections

A steep lowpass is compatible with both a codec and a native mastering/resampling filter.
The forensic fixtures reproduce both lossless false positives from the old spectral rule.
Conversely, ultrasonic noise added after an MP3 round-trip can pass the old positive
authenticity test. Silence also reached that branch through equal numerical floors.

Consequently, spectrum and tags remain visible observations but cannot independently
produce a provenance verdict. Only confirmed codec-grid evidence can produce
`probably_transcoded`, and only two clean codec-grid searches can produce
`probably_authentic` (see the current state above). Damaged/incomplete
decoding or a failed embedded checksum withholds that inference. Declared MP3/AAC files
remain `declared_lossy`, not accused of disguising anything.

Measured bandwidth is not the original sampling rate. Filtering a native 96 kHz signal
cannot be distinguished from upsampling by the measured edge alone. A suggested lower
rate therefore cannot truthfully be described as a guaranteed lossless conversion.

## Regression accounting

Initial corpus: **20 files, 10 lossy derivatives and 10 lossless controls**.

| Metric | Before (`0.5.0`) | Evidence revision (`0.6.0`) | Two grid searches |
| --- | ---: | ---: | ---: |
| Detected lossy derivatives | 8/10 | 3/10 | 8/10 |
| Missed/abstained lossy derivatives | 2/10 | 7/10 | 2/10 |
| Lossless files accused | 0/10 | 0/10 | 0/10 |
| Lossy derivatives called authentic | 0/10 | 0/10 | 0/10 |
| Lossless files called authentic | — | 0/10 | 7/10 (the 3 others are 96 kHz) |

Five MP3 verdicts based solely on spectral edges are deliberately withdrawn. This is a
precision-oriented policy change, **not improved recall**. The three AAC detections remain
mandatory assertions; all seven MP3 abstentions are explicitly counted. The old corpus's
zero false positives did not survive the new sharply filtered lossless counterexamples.

The separate forensic corpus adds six files: three lossless provenance controls (silence,
native FIR, lossless resampling), two lossy derivatives (KBD AAC, noise-added MP3), and one
7.1 loudness calibration. Its tests also cover phase invariance, LFE exclusion, unknown
layouts and shared-pipeline progress. See its [generation recipe and truth labels](../src-tauri/tests/fixtures/corpus/forensic/README.md).

This remains a small, related, synthetic development corpus. Zero observed false positives
does not establish a population false-positive rate. It lacks independently sourced real
masters across genres, hardware, dithering, rates and production chains.

## Next ambitious step, not silently shipped

A codec-independent learned detector may add more value than additional cutoff thresholds.
Before integrating one: obtain licensed real lossless sources, create multiple codec and
post-processing paths, and split train/validation/test **by original source** before making
derivatives. Keep unseen encoders, genuine sharp filters, vinyl/tape, sparse instruments,
resampling and added noise in the held-out set. Measure false accusations, missed cases,
abstention/coverage and calibration separately, with uncertainty intervals. Require an
improvement over this baseline at a maintainer-approved false-positive budget.

No neural model is shipped here: the repository has neither that dataset nor independently
validated weights. No new dependency, network inference or audio upload was introduced.

## Loudness references

Channel positions are passed to the existing `ebur128` implementation rather than guessing
from channel count. The 7.1 side-only fixture is independently measured by FFmpeg at
approximately -18.5 LUFS and -20 dBTP. LFE channels contribute to peaks but not loudness;
unknown multichannel layouts withhold LUFS/LRA. References:
[ITU-R BS.1770](https://www.itu.int/rec/R-REC-BS.1770/en),
[EBU R128](https://tech.ebu.ch/publications/r128),
[EBU loudness test set](https://tech.ebu.ch/publications/ebu_loudness_test_set).
