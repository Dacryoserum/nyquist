<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import {
    analyzeFile,
    authorizePlayback,
    exportReport,
    type AnalysisResult,
    type BitDepthAnalysis,
    type FileInfo,
    type Verdict
  } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import Spectrogram from "$lib/components/Spectrogram.svelte";
  import type { IconName } from "$lib/icons";

  let result = $state<AnalysisResult | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  let audioEl = $state<HTMLAudioElement | undefined>();
  let audioSrc = $state<string | null>(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);

  async function pickAndAnalyze() {
    const path = await open({
      multiple: false,
      filters: [
        { name: "Audio", extensions: ["flac", "mp3", "m4a", "aac", "alac", "wav", "ogg"] }
      ]
    });
    if (!path || Array.isArray(path)) return;

    loading = true;
    error = null;
    result = null;
    isPlaying = false;
    currentTime = 0;
    audioSrc = null;
    try {
      const [analysis] = await Promise.all([analyzeFile(path), authorizePlayback(path)]);
      result = analysis;
      audioSrc = convertFileSrc(path);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function togglePlay() {
    if (!audioEl) return;
    if (isPlaying) {
      audioEl.pause();
    } else {
      audioEl.play();
    }
  }

  function seekTo(seconds: number) {
    if (!audioEl) return;
    audioEl.currentTime = seconds;
    currentTime = seconds;
  }

  function fmt(value: number, digits = 1): string {
    return value.toFixed(digits);
  }

  function fmtDuration(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = Math.round(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function fmtHz(hz: number): string {
    return hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${hz.toFixed(0)} Hz`;
  }

  function fmtCount(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return n.toString();
  }

  function integrityLabel(fi: FileInfo): string {
    if (fi.integrity_verified === true) return "Verified";
    if (fi.integrity_verified === false) return "Failed!";
    return fi.codec === "flac" ? "No checksum in file" : "N/A for this codec";
  }

  function isBitDepthPadded(bd: BitDepthAnalysis): boolean {
    return (
      bd.effective_bit_depth !== null &&
      bd.declared_bit_depth !== null &&
      bd.effective_bit_depth < bd.declared_bit_depth
    );
  }

  function bitDepthLabel(fi: FileInfo, bd: BitDepthAnalysis): string {
    if (!fi.bit_depth) return "—";
    if (isBitDepthPadded(bd)) return `${fi.bit_depth}-bit (only ~${bd.effective_bit_depth}-bit real)`;
    return `${fi.bit_depth}-bit`;
  }

  async function handleExport() {
    if (!result) return;
    const path = await save({
      defaultPath: `${result.file_info.filename}.report.json`,
      filters: [{ name: "JSON", extensions: ["json"] }]
    });
    if (!path) return;
    await exportReport(path, JSON.stringify(result, null, 2));
  }

  const verdictMeta: Record<Verdict, { label: string; icon: IconName; className: string }> = {
    probably_authentic: { label: "Probably authentic", icon: "checkCircle", className: "verdict-authentic" },
    probably_transcoded: { label: "Probably transcoded", icon: "alertCircle", className: "verdict-transcoded" },
    indeterminate: { label: "Indeterminate", icon: "helpCircle", className: "verdict-indeterminate" }
  };
</script>

<main class="page">
  <header>
    <div class="title-row">
      <div class="mark">N</div>
      <div>
        <h1>Nyquist</h1>
        <p class="subtitle">Audio quality analyzer</p>
      </div>
    </div>
  </header>

  <button class="pick-button" onclick={pickAndAnalyze} disabled={loading}>
    <Icon name="upload" size={16} />
    {loading ? "Analyzing…" : "Choose an audio file"}
  </button>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if result}
    {@const fi = result.file_info}
    {@const sa = result.signal_analysis}
    {@const dr = result.dynamic_range}
    {@const spa = result.spectral_analysis}
    {@const ta = result.transcode_assessment}
    {@const bd = result.bit_depth_analysis}
    {@const vm = verdictMeta[ta.verdict]}

    <section class="card">
      <div class="filename-row">
        <h2 class="filename">{fi.filename}</h2>
        <button class="export-button" onclick={handleExport}>
          <Icon name="download" size={14} />
          Export report
        </button>
      </div>

      {#if audioSrc}
        <div class="player">
          <button class="play-button" onclick={togglePlay} aria-label={isPlaying ? "Pause" : "Play"}>
            <Icon name={isPlaying ? "pause" : "play"} size={16} />
          </button>
          <input
            class="scrubber"
            type="range"
            min="0"
            max={fi.duration_seconds}
            step="0.1"
            value={currentTime}
            oninput={(e) => seekTo(Number(e.currentTarget.value))}
          />
          <span class="player-time">{fmtDuration(currentTime)} / {fmtDuration(fi.duration_seconds)}</span>
        </div>
      {/if}

      <div class="stats-grid">
        <div class="stat"><Icon name="disc" /><span class="label">Container</span><span class="value">{fi.container}</span></div>
        <div class="stat"><Icon name="disc" /><span class="label">Codec</span><span class="value">{fi.codec}</span></div>
        <div class="stat"><Icon name="activity" /><span class="label">Sample rate</span><span class="value">{fmtHz(fi.sample_rate_hz)}</span></div>
        <div class="stat"><Icon name="activity" /><span class="label">Nyquist</span><span class="value">{fmtHz(fi.nyquist_hz)}</span></div>
        <div class="stat">
          <Icon name="layers" />
          <span class="label">Bit depth</span>
          <span class="value" class:value-bad={isBitDepthPadded(bd)}>{bitDepthLabel(fi, bd)}</span>
        </div>
        <div class="stat"><Icon name="stereo" /><span class="label">Channels</span><span class="value">{fi.channels}</span></div>
        <div class="stat"><Icon name="clock" /><span class="label">Duration</span><span class="value">{fmtDuration(fi.duration_seconds)}</span></div>
        <div class="stat"><Icon name="file" /><span class="label">File size</span><span class="value">{(fi.file_size_bytes / 1_000_000).toFixed(1)} MB</span></div>
        <div class="stat"><Icon name="gauge" /><span class="label">Avg. bitrate</span><span class="value">{fi.bitrate_kbps ? `${fmt(fi.bitrate_kbps, 0)} kbps` : "—"}</span></div>
        <div class="stat"><Icon name="hash" /><span class="label">Samples</span><span class="value">{fmtCount(fi.sample_count)}</span></div>
        <div class="stat">
          <Icon name="shield" />
          <span class="label">Integrity</span>
          <span class="value" class:value-bad={fi.integrity_verified === false}>{integrityLabel(fi)}</span>
        </div>
      </div>
    </section>

    <section class="card">
      <h2>Transcode likelihood</h2>
      <div class="verdict {vm.className}">
        <Icon name={vm.icon} size={22} />
        <div class="verdict-text">
          <span class="verdict-label">{vm.label}</span>
          <span class="verdict-confidence">confidence {(ta.confidence_score * 100).toFixed(0)}%</span>
        </div>
      </div>
      <ul class="indicators">
        {#each ta.indicators as indicator (indicator)}
          <li>{indicator}</li>
        {/each}
      </ul>
      <p class="note">
        Early, single-method heuristic (spectral rolloff shape) validated against a small
        synthetic corpus — not a certainty. See indicators above for exactly what produced
        this verdict.
      </p>
    </section>

    <section class="card">
      <h2>Signal</h2>
      <div class="stats-grid">
        <div class="stat"><Icon name="arrowsVertical" /><span class="label">Peak</span><span class="value">{fmt(sa.peak_dbfs)} dBFS</span></div>
        <div class="stat"><Icon name="triangle" /><span class="label">True peak</span><span class="value">{fmt(sa.true_peak_dbtp)} dBTP</span></div>
        <div class="stat"><Icon name="peak" /><span class="label">RMS</span><span class="value">{fmt(sa.rms_dbfs)} dBFS</span></div>
        <div class="stat"><Icon name="speaker" /><span class="label">Loudness</span><span class="value">{sa.lufs_integrated !== null ? `${fmt(sa.lufs_integrated)} LUFS` : "n/a (near-silent)"}</span></div>
        <div class="stat"><Icon name="speaker" /><span class="label">Loudness range</span><span class="value">{sa.loudness_range_lu !== null ? `${fmt(sa.loudness_range_lu)} LU` : "n/a"}</span></div>
        <div class="stat"><Icon name="clip" /><span class="label">Clipped samples</span><span class="value">{fmtCount(sa.clipping_count_total)}</span></div>
        <div class="stat"><Icon name="gauge" /><span class="label">Dynamic range (DR)</span><span class="value">{dr.dr14 !== null ? `DR${dr.dr14}` : "n/a"}</span></div>
        <div class="stat"><Icon name="funnel" /><span class="label">Spectral cutoff</span><span class="value">{fmtHz(spa.spectral_cutoff_hz)}</span></div>
        <div class="stat"><Icon name="arrowsVertical" /><span class="label">Rolloff steepness</span><span class="value">{fmt(spa.rolloff_steepness_db_per_khz, 0)} dB/kHz</span></div>
      </div>

      <table class="channels">
        <thead>
          <tr>
            <th>Ch</th>
            <th>Peak</th>
            <th>RMS</th>
            <th>Crest</th>
            <th>DR</th>
            <th>Clipped</th>
          </tr>
        </thead>
        <tbody>
          {#each sa.per_channel as ch (ch.channel)}
            <tr>
              <td>{ch.channel}</td>
              <td>{fmt(ch.peak_dbfs)}</td>
              <td>{fmt(ch.rms_dbfs)}</td>
              <td>{fmt(ch.crest_factor_db)}</td>
              <td>{dr.per_channel_db[ch.channel - 1] != null ? fmt(dr.per_channel_db[ch.channel - 1] ?? 0) : "—"}</td>
              <td>{fmtCount(ch.clipping_count)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="note">
        Crest factor is a simple peak-to-RMS ratio. DR is the Pleasurize Music Foundation
        DR14 algorithm (3-second block RMS/peak analysis) — the number this community
        compares against the public loudness-war database. They measure different things
        and won't match.
      </p>
    </section>

    <section class="card">
      <h2>Spectrogram</h2>
      <Spectrogram
        data={spa.spectrogram}
        spectralCutoffHz={spa.spectral_cutoff_hz}
        cutoffOverTimeHz={spa.cutoff_over_time_hz}
        currentTimeSeconds={currentTime}
        onSeek={audioSrc ? seekTo : undefined}
      />
      <p class="note">
        Spectral cutoff and rolloff shown here are raw measurements — see "Transcode
        likelihood" above for how they're interpreted. Click anywhere on the spectrogram
        to jump playback there.
      </p>
    </section>
  {/if}

  {#if audioSrc}
    <audio
      bind:this={audioEl}
      src={audioSrc}
      ontimeupdate={(e) => (currentTime = e.currentTarget.currentTime)}
      onplay={() => (isPlaying = true)}
      onpause={() => (isPlaying = false)}
      onended={() => (isPlaying = false)}
    ></audio>
  {/if}
</main>

<style>
  :root {
    color-scheme: light dark;
    --bg: #f7f3f0;
    --card-bg: #ffffff;
    --border: #e6ddd6;
    --fg: #211a15;
    --muted: #7d6f64;
    --accent: #c97a52;
    --accent-fg: #ffffff;
  }

  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #15110f;
      --card-bg: #201a17;
      --border: #33281f;
      --fg: #f3ece6;
      --muted: #a89a8f;
      --accent: #e2a385;
      --accent-fg: #1a120d;
    }
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--fg);
    font-family:
      -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, sans-serif;
  }

  .page {
    max-width: 760px;
    margin: 0 auto;
    padding: 3rem 1.5rem 4rem;
  }

  header {
    margin-bottom: 1.75rem;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 0.85rem;
  }

  .mark {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-fg);
    font-weight: 700;
    font-size: 1.15rem;
    flex-shrink: 0;
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
    letter-spacing: -0.02em;
  }

  .subtitle {
    margin: 0.1rem 0 0;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .pick-button {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.65em 1.3em;
    font-size: 0.95rem;
    font-weight: 600;
    border-radius: 8px;
    border: none;
    background: var(--accent);
    color: var(--accent-fg);
    cursor: pointer;
  }

  .pick-button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .error {
    color: #d64545;
    margin-top: 1rem;
  }

  .card {
    margin-top: 1.5rem;
    padding: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--card-bg);
  }

  .card h2 {
    margin: 0 0 1.1rem;
    font-size: 1rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .filename-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .filename {
    color: var(--fg) !important;
    text-transform: none !important;
    font-size: 1.05rem !important;
    letter-spacing: normal !important;
    word-break: break-all;
    margin: 0 !important;
  }

  .export-button {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4em 0.75em;
    font-size: 0.78rem;
    font-weight: 600;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .export-button:hover {
    color: var(--fg);
    border-color: var(--accent);
  }

  .value-bad {
    color: #d64545 !important;
  }

  .player {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 0.9rem 0 1.3rem;
  }

  .play-button {
    display: grid;
    place-items: center;
    width: 2.1rem;
    height: 2.1rem;
    border-radius: 999px;
    border: none;
    background: var(--accent);
    color: var(--accent-fg);
    cursor: pointer;
    flex-shrink: 0;
  }

  .scrubber {
    flex: 1;
    accent-color: var(--accent);
  }

  .player-time {
    font-size: 0.78rem;
    color: var(--muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.7rem 1.5rem;
  }

  .stat {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    font-size: 0.88rem;
    color: var(--muted);
  }

  .stat :global(svg) {
    flex-shrink: 0;
    color: var(--accent);
  }

  .stat .label {
    white-space: nowrap;
  }

  .stat .value {
    margin-left: auto;
    color: var(--fg);
    font-weight: 600;
    text-align: right;
  }

  .channels {
    width: 100%;
    margin-top: 1.5rem;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .channels th {
    color: var(--muted);
    font-weight: 500;
    text-align: right;
  }

  .channels td {
    text-align: right;
    padding: 0.4em 0.5em;
    border-bottom: 1px solid var(--border);
  }

  .channels th:first-child,
  .channels td:first-child {
    text-align: left;
    padding-left: 0;
  }

  .note {
    margin: 0.85rem 0 0;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .verdict {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
  }

  .verdict-text {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .verdict-label {
    font-weight: 700;
    font-size: 1rem;
  }

  .verdict-confidence {
    font-size: 0.78rem;
    opacity: 0.75;
  }

  .verdict-authentic {
    background: rgba(93, 156, 115, 0.14);
    color: #4d8a63;
  }

  .verdict-transcoded {
    background: rgba(200, 140, 50, 0.16);
    color: #b3771f;
  }

  .verdict-indeterminate {
    background: rgba(140, 130, 120, 0.14);
    color: var(--muted);
  }

  @media (prefers-color-scheme: dark) {
    .verdict-authentic {
      color: #93d1a8;
    }
    .verdict-transcoded {
      color: #eab871;
    }
  }

  .indicators {
    margin: 0.9rem 0 0;
    padding-left: 1.1rem;
    font-size: 0.85rem;
    color: var(--fg);
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .indicators li {
    line-height: 1.4;
  }
</style>
