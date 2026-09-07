# Detecting a lossy history: evidence and limits

Reviewed 2026-09-07. This note separates published research, Nyquist's implementation,
and measurements on its own synthetic corpus. None is a guarantee of authenticity.

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

## Implemented: a broader, confirmed AAC hypothesis

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
produce a provenance verdict. `probably_authentic` is reserved and currently never emitted.
Only confirmed codec-grid evidence can produce `probably_transcoded`. Damaged/incomplete
decoding or a failed embedded checksum withholds that inference. Declared MP3/AAC files
remain `declared_lossy`, not accused of disguising anything.

Measured bandwidth is not the original sampling rate. Filtering a native 96 kHz signal
cannot be distinguished from upsampling by the measured edge alone. A suggested lower
rate therefore cannot truthfully be described as a guaranteed lossless conversion.

## Regression accounting

Initial corpus: **20 files, 10 lossy derivatives and 10 lossless controls**.

| Metric | Before (`0.5.0`) | Evidence revision |
| --- | ---: | ---: |
| Detected lossy derivatives | 8/10 | 3/10 |
| Missed/abstained lossy derivatives | 2/10 | 7/10 |
| Lossless files accused | 0/10 | 0/10 |
| Lossy derivatives called authentic | 0/10 | 0/10 |

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
