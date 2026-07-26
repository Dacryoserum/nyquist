# Nyquist

> Status: **early, functional prototype.** Core analysis, spectrogram, a first-pass
> transcode-likelihood verdict, playback, and a CLI are all working end to end — see
> [Roadmap](#roadmap) for what's still ahead of a public release. No packaged builds yet.

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
- Renders an interactive spectrogram (FFT) with playback and click-to-seek, and measures
  the spectral cutoff position, its rolloff steepness, and how the cutoff evolves over
  the track.
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
- Reports damaged packets that had to be skipped, so a corrupt file is never silently
  analyzed as if it were whole.
- Scores the likelihood of a lossy-to-lossless transcode from the above, always reported
  as a **3-state, explainable verdict** (probably authentic / probably transcoded /
  indeterminate) with bounded confidence and a human-readable list of what produced it —
  never a flat yes/no, because natural treble-poor masters exist and false positives
  matter more than missed detections.
- Exports the full analysis as a JSON report.
- Ships a headless CLI (`nyquist-cli`) for scripting/batch use, sharing the exact same
  analysis pipeline as the desktop app.

## Tech stack

Tauri (Rust backend + Svelte frontend, native webview). Rust core: `symphonia` for
decoding, `rustfft` for spectral analysis, `ebur128` for LUFS/LRA/true peak, `rayon` for
running the independent analysis stages concurrently. Playback uses the webview's native
`<audio>` element via Tauri's `asset://` protocol, not a Rust audio crate. macOS first,
Windows and Linux planned after V1.0.

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
cargo run --release --bin nyquist-cli -- --json path/to/file.flac
cargo run --release --bin nyquist-cli -- --timing path/to/file.flac   # per-stage profile
```

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
- Catching transparent lossy encodes (LAME V0, AAC 256kbps), which do not lowpass at all
  and are therefore invisible to the spectral method — the main known gap, and one that
  needs a different indicator rather than a threshold change.
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
