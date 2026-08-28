# Nyquist

> Status: **early, functional prototype.** Core analysis, spectrogram, a first-pass
> transcode-likelihood verdict, playback, and a CLI are all working end to end — see
> [Roadmap](#roadmap) for what's still ahead. Unsigned macOS/Windows builds are published
> on [GitHub Releases](https://github.com/Dacryoserum/nyquist/releases) on every tag — see
> [Installing a release build](#installing-a-release-build).

Nyquist is a cross-platform desktop app for inspecting the real technical quality of an
audio file — sample rate, bit depth, dynamic range, loudness (LUFS), true peak — and, most
importantly, detecting whether a file labeled as lossless (FLAC, ALAC, WAV) is actually a
**transcode from a lossy source** (MP3, AAC, ...) or padded to look like a bit depth it
never really had. Mislabeled lossy-to-lossless transcodes and fake-hi-res files are a
widespread problem among digital music collectors; Nyquist aims to make both detectable
without specialized audio-engineering knowledge.

It's also meant to work as a general-purpose audio file inspector beyond that one use
case: metadata, integrity, and signal characteristics for any FLAC/MP3/ALAC/WAV/AAC/OGG
file.

## Why "Nyquist"

Named after the Nyquist frequency — the theoretical ceiling of a sampled signal's usable
bandwidth, and the direct reason a fake-lossless file leaves a fingerprint: a real
192kHz/24-bit master has meaningful content well past 16-20kHz, while a lossy transcode
often shows an abrupt spectral cutoff right around where the original lossy encoder cut
it off.

## What it does today

- Decodes FLAC/MP3/AAC/ALAC/WAV/OGG (`symphonia`) and extracts technical metadata:
  container, codec, sample rate, bit depth, duration, average bitrate.
- Verifies file integrity where the container supports it (FLAC's embedded MD5 checksum).
- Signal analysis: peak, true peak (oversampled, ITU-R BS.1770), RMS, integrated loudness
  (LUFS, EBU R128), loudness range (LRA), per-channel clipping, and DR14 (the Pleasurize
  Music Foundation Dynamic Range algorithm this community compares publicly).
- Renders an interactive spectrogram (FFT) with playback, click-to-seek, and volume
  control, and measures the spectral cutoff position, its rolloff steepness, and how the
  cutoff evolves over the track.
- Scans encoder tags for leftover lossy-encoder signatures (LAME, qaac, FhG, ...). Only
  the tags naming the encoding tool are read, and only encoders that exclusively produce
  lossy output count — iTunes, for instance, is one of the most common *lossless* CD
  rippers, so its name says nothing either way.
- Detects bit-depth padding — a file zero-padded into a wider container without ever
  containing more real information (a "fake hi-res" pattern distinct from lossy
  transcoding).
- Detects sample-rate padding — a file resampled up to 96/192 kHz whose content stops well
  below the bandwidth that rate exists to carry. The counterpart to bit-depth padding, and
  likewise reported separately from the transcode verdict: such a file is lossless end to
  end, so calling it "transcoded" would name the wrong defect.
- Reports an incomplete decode — skipped packets, or a stream that stopped early — and
  **withholds the transcode verdict** when one happens, so a corrupt file is never quietly
  judged as if it were whole.
- Scores the likelihood of a lossy-to-lossless transcode from the above, always reported as
  a **4-state, explainable verdict** with a human-readable list of what produced it — never
  a flat yes/no, because natural treble-poor masters exist and false positives matter more
  than missed detections. The states are:
  - **probably authentic** — positive evidence was measured that rules out a lossy source;
  - **probably transcoded** — an encoder fingerprint was found;
  - **inconclusive** — no sign of transcoding was found, which is *not* evidence of
    authenticity. On a 44.1 kHz file with no detectable cutoff this is the expected and
    honest answer, not a failure;
  - **lossy format** — the file is an MP3/AAC/Opus and says so, so the question of a
    disguise does not arise.

  Evidence is reported as a weak/moderate/strong reading rather than a percentage: the
  underlying weights are tuned on a twenty-fixture corpus, not calibrated against a held-out
  validation set, and a percentage would claim a precision they do not have. The raw number
  is still in the exported JSON.
- Exports the full analysis as a JSON report.
- Ships a headless CLI (`nyquist-cli`) for scripting/batch use, sharing the exact same
  analysis pipeline as the desktop app.
- UI in French by default, with a discreet toggle to switch to English.

## Tech stack

Tauri (Rust backend + Svelte frontend, native webview). Rust core: `symphonia` for
decoding, `rustfft` for spectral analysis, `ebur128` for LUFS/LRA/true peak, `rayon` for
running the independent analysis stages concurrently. Playback uses the webview's native
`rodio` (over `cpal`) for playback, fed directly from the samples the analysis already
decoded — the webview plays nothing at all. Two earlier attempts went through the webview's
`<audio>` element, over Tauri's `asset://` protocol and then over a loopback HTTP server;
both kept the element's own idea of how long the file was, which disagreed with the
decoder's and produced wrong seeks, a drifting counter and long tracks stopping early.
There is one clock now: a sample index into the decoded track. macOS first, Windows and
Linux planned after V1.0.

## Building from source

Requires Rust (stable) and Node 18+. `ffmpeg` is only needed to *regenerate* the test
corpus — the fixtures are committed, so tests run without it.

```bash
npm install
npm run tauri dev      # run the app
npm run tauri build    # produce a .dmg / .app in src-tauri/target/release/bundle
```

Checks, all of which CI enforces on every PR:

```bash
npm run check && npm run build              # frontend
cd src-tauri
cargo test --release -- --nocapture         # includes the corpus false-positive report
cargo clippy --all-targets -- -D warnings
```

The headless CLI shares the exact analysis pipeline as the app:

```bash
cargo run --release -p nyquist-cli -- --json path/to/file.flac
cargo run --release -p nyquist-cli -- --timing path/to/file.flac   # per-stage profile
```

(`-p`, not `--bin`: the CLI is its own workspace member, and `--bin nyquist-cli` fails from
the workspace root.)

## Installing a release build

Builds are **not signed or notarized**, so macOS Gatekeeper refuses them on first launch
with an "unidentified developer" warning. Right-click (or Control-click) Nyquist.app and
choose **Open**, then confirm — once per install. Signing requires a paid Apple Developer
account and that decision is still open; the release notes say the same thing rather than
leaving people to work it out.

## Roadmap

Done: core analysis, test corpus, spectrogram, a first-pass transcode-detection verdict,
audio playback, and a batch of audiophile-focused features (integrity check, DR14, LRA,
tag fingerprinting, bit-depth padding detection, JSON export, CLI). See `CHANGELOG.md`
for the detailed list.

Ahead:
- Remaining UI polish (session history).
- Catching transparent **MP3** encodes (LAME V0), which do not lowpass at all and are
  therefore invisible to the spectral method — the main known gap. Such a file comes out
  *inconclusive*, never "probably authentic": the tool declines to vouch for what it cannot
  see. The AAC half of this gap is closed, by the MDCT grid sweep in
  `src-tauri/src/mdct_grid.rs`, which catches AAC 256 and AAC 128 across the corpus
  including settings no spectral measurement can see. It cannot be extended to MP3, whose
  hybrid filterbank a plain MDCT does not invert.

  Two approaches have been implemented, measured and rejected against the corpus; both are
  recorded with their numbers in `src-tauri/tests/fixtures/corpus/README.md` so the next
  attempt starts from the results. The blocker turned out not to be the algorithm but the
  corpus: it is built from noise, which is the material a perceptual encoder discards least,
  so LAME V0 leaves no trace in it to detect. `tests/local_probe.rs` measures the statistics
  a detector would need, against real music placed in the gitignored `corpus/local/`. That
  measurement comes before any further implementation.
- **V1.0** — Public macOS release (`.dmg`), notarization.
- **V1.1+** — Windows. **V2.0+** — folder/library batch scanning (CLI already covers part
  of this), side-by-side file comparison, local history (SQLite).

Explicitly out of scope for now: MQA detection (different risk profile — a contested,
technical topic without enough validated research behind it yet to implement responsibly).

## Contributing

Not yet open for external contributions — still stabilizing the core detection logic.
`CONTRIBUTING.md` documents the workflow for when that changes.

## License

[MIT](LICENSE). Vendored third-party code and its licenses are listed in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
