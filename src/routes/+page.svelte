<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    analyzeFile,
    authorizePlayback,
    exportReport,
    type AnalysisResult,
    type Verdict
  } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import Meter from "$lib/components/Meter.svelte";
  import Spectrogram from "$lib/components/Spectrogram.svelte";
  import ThinkingOrb from "$lib/components/ThinkingOrb.svelte";
  import type { IconName } from "$lib/icons";

  let result = $state<AnalysisResult | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let lastPath = $state<string | null>(null);
  let dragging = $state(false);
  let theme = $state<"light" | "dark">("dark");

  let audioEl = $state<HTMLAudioElement | undefined>();
  let audioSrc = $state<string | null>(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let scrubbing = $state(false);

  onMount(() => {
    const saved = localStorage.getItem("nyquist-theme");
    theme = saved === "light" || saved === "dark" ? saved : matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
    applyTheme();

    // Tauri delivers file drops through the webview, not through HTML5 drag events.
    // Guarded so the page still mounts under a plain `vite dev` browser session, where
    // the Tauri internals this reaches for don't exist — drag and drop is a convenience,
    // and the file picker covers the same ground without it.
    let unlisten: (() => void) | undefined;
    try {
      getCurrentWebview()
        .onDragDropEvent((event) => {
          if (event.payload.type === "over") {
            dragging = true;
          } else if (event.payload.type === "drop") {
            dragging = false;
            const dropped = event.payload.paths[0];
            if (dropped) analyze(dropped);
          } else {
            dragging = false;
          }
        })
        .then((fn) => (unlisten = fn))
        .catch(() => {});
    } catch {
      /* Not running inside Tauri. */
    }
    return () => unlisten?.();
  });

  function applyTheme() {
    document.documentElement.setAttribute("data-theme", theme);
  }

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    localStorage.setItem("nyquist-theme", theme);
    applyTheme();
  }

  async function analyze(path: string) {
    loading = true;
    error = null;
    result = null;
    isPlaying = false;
    currentTime = 0;
    audioSrc = null;
    lastPath = path;
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

  async function pickAndAnalyze() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Audio", extensions: ["flac", "mp3", "m4a", "aac", "alac", "wav", "ogg"] }]
    });
    if (!path || Array.isArray(path)) return;
    analyze(path);
  }

  function togglePlay() {
    if (!audioEl) return;
    isPlaying ? audioEl.pause() : audioEl.play();
  }

  function seekTo(seconds: number) {
    if (!audioEl) return;
    audioEl.currentTime = seconds;
    currentTime = seconds;
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

  const fmt = (v: number, d = 1) => v.toFixed(d);
  const fmtDuration = (s: number) => `${Math.floor(s / 60)}:${Math.round(s % 60).toString().padStart(2, "0")}`;
  const fmtHz = (hz: number) => (hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${hz.toFixed(0)} Hz`);
  const fmtCount = (n: number) =>
    n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1_000 ? `${(n / 1_000).toFixed(1)}K` : `${n}`;

  type Tone = "good" | "warn" | "bad" | "neutral";

  /** DR bands follow the Pleasurize Music Foundation / DR-database convention the
   * audiophile community actually publishes against. A convention, not a standard. */
  const drTone = (dr: number): Tone => (dr >= 12 ? "good" : dr >= 8 ? "warn" : "bad");
  const drLabel = (dr: number) =>
    dr >= 14 ? "wide" : dr >= 12 ? "good" : dr >= 8 ? "moderate" : "heavily compressed";

  /** EBU R128 and every major streaming platform ask for -1 dBTP of headroom; above 0 the
   * file will clip on any resampling or lossy re-encode downstream. */
  const truePeakTone = (dbtp: number): Tone => (dbtp > 0 ? "bad" : dbtp > -1 ? "warn" : "good");

  const verdictMeta: Record<Verdict, { label: string; icon: IconName; tone: string; blurb: string }> = {
    probably_authentic: {
      label: "Probably authentic",
      icon: "checkCircle",
      tone: "authentic",
      blurb: "No encoder fingerprint found in the spectrum."
    },
    probably_transcoded: {
      label: "Probably transcoded",
      icon: "alertCircle",
      tone: "transcoded",
      blurb: "This looks like lossy audio wrapped in a lossless container."
    },
    indeterminate: {
      label: "Inconclusive",
      icon: "helpCircle",
      tone: "indeterminate",
      blurb: "Not enough evidence either way. That is a real answer, not a failure."
    }
  };

  type Finding = { icon: IconName; tone: "warn" | "bad"; title: string; detail: string };

  /** Only genuine problems appear here. A clean file shows nothing, so anything in this
   * list is worth the user's attention by construction. */
  const findings = $derived.by<Finding[]>(() => {
    if (!result) return [];
    const f: Finding[] = [];
    const { file_info: fi, bit_depth_analysis: bd, sample_rate_analysis: sr, signal_analysis: sa } = result;

    if (fi.integrity_verified === false) {
      f.push({
        icon: "shield",
        tone: "bad",
        title: "Checksum mismatch",
        detail:
          "The audio does not match the checksum stored inside the file. It has been truncated, edited, or corrupted since it was created."
      });
    }
    if (fi.decode_errors > 0) {
      f.push({
        icon: "alertTriangle",
        tone: "bad",
        title: `${fi.decode_errors} damaged packet${fi.decode_errors > 1 ? "s" : ""} skipped`,
        detail:
          "Part of the file could not be decoded. Every measurement below describes the audio that survived, not the whole track."
      });
    }
    if (bd.declared_bit_depth !== null && bd.effective_bit_depth !== null && bd.effective_bit_depth < bd.declared_bit_depth) {
      f.push({
        icon: "layers",
        tone: "warn",
        title: `${bd.declared_bit_depth}-bit container holding ${bd.effective_bit_depth}-bit audio`,
        detail: `Every sample lands exactly on the ${bd.effective_bit_depth}-bit quantization grid, so the extra depth carries no information. The file was padded, not remastered.`
      });
    }
    if (sr.likely_upsampled) {
      f.push({
        icon: "ruler",
        tone: "warn",
        title: `${(sr.declared_sample_rate_hz / 1000).toFixed(1)} kHz declared, ${(sr.content_bandwidth_hz / 1000).toFixed(1)} kHz used`,
        detail: `Content stops at ${(sr.bandwidth_ratio * 100).toFixed(0)}% of the bandwidth this sample rate exists to carry${
          sr.sufficient_sample_rate_hz ? `. A ${(sr.sufficient_sample_rate_hz / 1000).toFixed(1)} kHz file would hold all of it losslessly` : ""
        }. The audio is intact — the sample rate on the label is inflated.`
      });
    }
    if (sa.clipping_count_total > 0) {
      f.push({
        icon: "clip",
        tone: sa.clipping_count_total > 1000 ? "bad" : "warn",
        title: `${fmtCount(sa.clipping_count_total)} clipped samples`,
        detail: "Samples pinned at full scale, where the waveform was flattened rather than reproduced."
      });
    }
    return f;
  });
</script>

<svelte:head><title>Nyquist</title></svelte:head>

<div class="shell" class:dragging>
  <header class="topbar">
    <!-- Wordmark only. The orb was tried here, held still, and dropped: at 30px its dots
         collapse into a smudge that reads as neither the orb nor a logo. The name set in
         spaced monospace carries the instrument feel on its own. -->
    <div class="brand">
      <h1>Nyquist</h1>
      <p class="tagline">Is this file what it says it is?</p>
    </div>
    <div class="topbar-actions">
      {#if result}
        <button class="ghost" onclick={pickAndAnalyze} disabled={loading}>
          <Icon name="upload" size={14} /> Open another
        </button>
        <button class="ghost" onclick={handleExport}>
          <Icon name="download" size={14} /> Export JSON
        </button>
      {/if}
      <button class="ghost icon-only" onclick={toggleTheme} aria-label="Switch theme">
        <Icon name={theme === "dark" ? "sun" : "moon"} size={15} />
      </button>
    </div>
  </header>

  <main class="page">
    {#if !result && !loading}
      <section class="dropzone">
        <Icon name="upload" size={26} />
        <h2>Drop an audio file here</h2>
        <p>FLAC, ALAC, WAV, MP3, AAC or OGG. Nothing leaves your machine.</p>
        <button class="primary" onclick={pickAndAnalyze}>Choose a file</button>
        {#if error}
          <p class="error" role="alert">{error}</p>
        {/if}
      </section>
    {/if}

    {#if loading}
      <section class="loading" aria-live="polite">
        <ThinkingOrb state="composing" size={64} dark={theme === "dark"} />
        <p>Decoding and analyzing…</p>
        <span class="hint">Full-length FFT and loudness passes over every sample.</span>
      </section>
    {/if}

    {#if error && result === null && !loading && lastPath}
      <p class="error standalone" role="alert">{error}</p>
    {/if}

    {#if result}
      {@const fi = result.file_info}
      {@const sa = result.signal_analysis}
      {@const dr = result.dynamic_range}
      {@const spa = result.spectral_analysis}
      {@const ta = result.transcode_assessment}
      {@const sr = result.sample_rate_analysis}
      {@const vm = verdictMeta[ta.verdict]}

      <!-- The verdict is the reason the app exists, so it opens the page and everything
           else is evidence arranged underneath it. -->
      <section class="verdict {vm.tone}">
        <div class="verdict-head">
          <Icon name={vm.icon} size={30} />
          <div class="verdict-copy">
            <h2>{vm.label}</h2>
            <p>{vm.blurb}</p>
          </div>
          <div class="confidence">
            <span class="confidence-value">{(ta.confidence_score * 100).toFixed(0)}<i>%</i></span>
            <span class="confidence-label">confidence</span>
          </div>
        </div>
        <ul class="evidence">
          {#each ta.indicators as indicator (indicator)}
            <li>{indicator}</li>
          {/each}
        </ul>
      </section>

      {#if findings.length}
        <section class="findings">
          {#each findings as finding (finding.title)}
            <article class="finding {finding.tone}">
              <Icon name={finding.icon} size={17} />
              <div>
                <h3>{finding.title}</h3>
                <p>{finding.detail}</p>
              </div>
            </article>
          {/each}
        </section>
      {/if}

      <section class="card file-card">
        <div class="file-head">
          <h2 class="filename">{fi.filename}</h2>
          <span class="chips">
            <span class="chip">{fi.codec.toUpperCase()}</span>
            <span class="chip">{fmtHz(fi.sample_rate_hz)}</span>
            {#if fi.bit_depth}<span class="chip">{fi.bit_depth}-bit</span>{/if}
            <span class="chip">{fi.channels === 2 ? "stereo" : `${fi.channels}ch`}</span>
            <span class="chip">{fmtDuration(fi.duration_seconds)}</span>
          </span>
        </div>

        <!-- Bandwidth against Nyquist: the one picture that makes an upsampled file
             obvious at a glance, and the reason sample_rate_analysis exists. -->
        <div class="bandwidth">
          <div class="bandwidth-head">
            <span class="label">Bandwidth used</span>
            <span class="value" class:value-warn={sr.likely_upsampled}>
              {fmtHz(sr.content_bandwidth_hz)} of {fmtHz(fi.nyquist_hz)}
              <em>({(sr.bandwidth_ratio * 100).toFixed(0)}%)</em>
            </span>
          </div>
          <Meter value={sr.bandwidth_ratio} min={0} max={1} tone={sr.likely_upsampled ? "warn" : "good"} />
        </div>
      </section>

      <section class="card">
        <h2 class="section-title">Spectrum</h2>
        <Spectrogram
          data={spa.spectrogram}
          spectralCutoffHz={spa.spectral_cutoff_hz}
          cutoffOverTimeHz={spa.cutoff_over_time_hz}
          currentTimeSeconds={currentTime}
          onSeek={audioSrc ? seekTo : undefined}
        />
        <div class="spectral-stats">
          <div class="stat-block">
            <span class="label">Bandwidth</span>
            <span class="value">{fmtHz(spa.spectral_cutoff_hz)}</span>
          </div>
          <div class="stat-block">
            <span class="label">Rolloff steepness</span>
            <span class="value">
              {spa.encoder_edge_hz !== null
                ? `${fmt(spa.rolloff_steepness_db_per_khz, 0)} dB/kHz @ ${fmtHz(spa.encoder_edge_hz)}`
                : "no edge found"}
            </span>
          </div>
        </div>
        <p class="note">
          Bandwidth is where content stops — the lowpass edge if there is one, otherwise
          Nyquist. Steepness is what separates an encoder from a dark mix: a codec's lowpass
          falls off a cliff, a mastering choice slopes away. Click the spectrogram to jump
          playback there.
        </p>
      </section>

      <div class="metric-columns">
        <section class="card">
          <h2 class="section-title">Loudness</h2>

          <div class="metric">
            <div class="metric-head">
              <span class="label">Integrated loudness</span>
              <span class="value">{sa.lufs_integrated !== null ? `${fmt(sa.lufs_integrated)} LUFS` : "n/a"}</span>
            </div>
            {#if sa.lufs_integrated !== null}
              <Meter value={sa.lufs_integrated} min={-30} max={0} reference={-14} referenceLabel="-14 LUFS streaming target" />
              <span class="scale-note">tick marks the -14 LUFS streaming target</span>
            {/if}
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">True peak</span>
              <span class="value {truePeakTone(sa.true_peak_dbtp)}">{fmt(sa.true_peak_dbtp)} dBTP</span>
            </div>
            <Meter value={sa.true_peak_dbtp} min={-12} max={3} tone={truePeakTone(sa.true_peak_dbtp)} reference={-1} referenceLabel="-1 dBTP" />
            <span class="scale-note">
              {sa.true_peak_dbtp > 0
                ? "above full scale — will clip when resampled or re-encoded"
                : "tick marks the -1 dBTP headroom EBU R128 asks for"}
            </span>
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">Loudness range</span>
              <span class="value">{sa.loudness_range_lu !== null ? `${fmt(sa.loudness_range_lu)} LU` : "n/a"}</span>
            </div>
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">Peak / RMS</span>
              <span class="value">{fmt(sa.peak_dbfs)} / {fmt(sa.rms_dbfs)} dBFS</span>
            </div>
          </div>
        </section>

        <section class="card">
          <h2 class="section-title">Dynamics</h2>

          <div class="metric">
            <div class="metric-head">
              <span class="label">Dynamic range</span>
              <span class="value {dr.dr14 !== null ? drTone(dr.dr14) : ''}">
                {dr.dr14 !== null ? `DR${dr.dr14}` : "n/a"}
              </span>
            </div>
            {#if dr.dr14 !== null}
              <Meter value={dr.dr14} min={0} max={20} tone={drTone(dr.dr14)} />
              <span class="scale-note">{drLabel(dr.dr14)} — Pleasurize DR scale, the one the loudness-war database uses</span>
            {/if}
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">Clipped samples</span>
              <span class="value {sa.clipping_count_total > 0 ? 'warn' : ''}">{fmtCount(sa.clipping_count_total)}</span>
            </div>
          </div>

          <table class="channels">
            <thead>
              <tr><th>Ch</th><th>Peak</th><th>RMS</th><th>Crest</th><th>DR</th><th>Clipped</th></tr>
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
            Crest factor is a plain peak-to-RMS ratio. DR is the block-based Pleasurize
            algorithm. They measure different things and are not meant to match.
          </p>
        </section>
      </div>

      <section class="card">
        <h2 class="section-title">File</h2>
        <dl class="facts">
          <div><dt>Container</dt><dd>{fi.container}</dd></div>
          <div><dt>Codec</dt><dd>{fi.codec}</dd></div>
          <div><dt>Sample rate</dt><dd>{fmtHz(fi.sample_rate_hz)}</dd></div>
          <div><dt>Nyquist</dt><dd>{fmtHz(fi.nyquist_hz)}</dd></div>
          <div><dt>Bit depth</dt><dd>{fi.bit_depth ? `${fi.bit_depth}-bit` : "—"}</dd></div>
          <div><dt>Channels</dt><dd>{fi.channels}</dd></div>
          <div><dt>Duration</dt><dd>{fmtDuration(fi.duration_seconds)}</dd></div>
          <div><dt>Size</dt><dd>{(fi.file_size_bytes / 1_000_000).toFixed(1)} MB</dd></div>
          <div><dt>Avg. bitrate</dt><dd>{fi.bitrate_kbps ? `${fmt(fi.bitrate_kbps, 0)} kbps` : "—"}</dd></div>
          <div><dt>Samples</dt><dd>{fmtCount(fi.sample_count)}</dd></div>
          <div>
            <dt>Integrity</dt>
            <dd class:bad={fi.integrity_verified === false} class:good={fi.integrity_verified === true}>
              {fi.integrity_verified === true
                ? "Checksum verified"
                : fi.integrity_verified === false
                  ? "Checksum mismatch"
                  : fi.codec === "flac"
                    ? "No checksum stored"
                    : "Not available for this codec"}
            </dd>
          </div>
        </dl>
      </section>

      <p class="disclaimer">
        Nyquist reports what it can measure and says so when that is not enough. The transcode
        verdict rests mainly on the shape of the spectral rolloff, which cannot see a
        transparent encode such as LAME V0 or AAC 256 — a clean result is not proof of
        provenance.
      </p>
    {/if}
  </main>

  {#if result && audioSrc}
    <div class="player">
      <button class="play" onclick={togglePlay} aria-label={isPlaying ? "Pause" : "Play"}>
        <Icon name={isPlaying ? "pause" : "play"} size={15} />
      </button>
      <span class="time">{fmtDuration(currentTime)}</span>
      <input
        class="scrubber"
        type="range"
        min="0"
        max={result.file_info.duration_seconds}
        step="0.1"
        value={currentTime}
        aria-label="Playback position"
        oninput={(e) => {
          scrubbing = true;
          seekTo(Number(e.currentTarget.value));
        }}
        onchange={() => (scrubbing = false)}
      />
      <span class="time muted-time">{fmtDuration(result.file_info.duration_seconds)}</span>
    </div>
  {/if}

  {#if audioSrc}
    <audio
      bind:this={audioEl}
      src={audioSrc}
      ontimeupdate={(e) => {
        if (!scrubbing) currentTime = e.currentTarget.currentTime;
      }}
      onplay={() => (isPlaying = true)}
      onpause={() => (isPlaying = false)}
      onended={() => (isPlaying = false)}
    ></audio>
  {/if}
</div>

<style>
  /* ── Instrument theme ───────────────────────────────────────────────────────────────
     Built from the thinking orb's own visual model: monochrome ink painted on
     transparency, form made of dots, hierarchy carried by alpha rather than by colour or
     by boxes.

     Colour survives in exactly two places, both load-bearing. The severity tints below are
     pulled far down in saturation so they read as tinted greys inside the monochrome world
     — this app's entire purpose is to state a verdict, and encoding that in weight alone
     would be unreadable. The spectrogram keeps its inferno colormap: it is the one true
     data surface, and perceptually uniform beats on-palette for something users read
     values off. Greyscale chrome around one vivid readout is how instruments actually
     look, so the contrast is the point.
     ─────────────────────────────────────────────────────────────────────────────────── */

  :global(:root) {
    color-scheme: dark;
    --bg: #0b0b0c;
    --bg-raised: rgba(255, 255, 255, 0.022);
    --ink-hi: rgba(255, 255, 255, 0.93);
    --ink-mid: rgba(255, 255, 255, 0.6);
    --ink-low: rgba(255, 255, 255, 0.38);
    --ink-faint: rgba(255, 255, 255, 0.13);
    --ink-hair: rgba(255, 255, 255, 0.09);
    --ink-grid: rgba(255, 255, 255, 0.045);
    --ok: #92b39b;
    --warn: #cfa96f;
    --bad: #d0918a;
    --mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    --sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, sans-serif;
  }

  :global(:root[data-theme="light"]) {
    color-scheme: light;
    --bg: #f5f4f2;
    --bg-raised: rgba(17, 17, 18, 0.018);
    --ink-hi: rgba(17, 17, 18, 0.92);
    --ink-mid: rgba(17, 17, 18, 0.62);
    --ink-low: rgba(17, 17, 18, 0.42);
    --ink-faint: rgba(17, 17, 18, 0.16);
    --ink-hair: rgba(17, 17, 18, 0.12);
    --ink-grid: rgba(17, 17, 18, 0.06);
    --ok: #4b7256;
    --warn: #8a6520;
    --bad: #a2483f;
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--ink-hi);
    font-family: var(--sans);
    -webkit-font-smoothing: antialiased;
  }

  .shell {
    min-height: 100vh;
    padding-bottom: 5.5rem;
    /* The faint dot lattice the whole theme rests on — the orb's material, spread thin
       enough to register as texture rather than pattern.
       Deliberately NOT `background-attachment: fixed`: that pins the gradient to the
       viewport, so the compositor has to re-rasterize it across the full window on every
       scroll frame instead of scrolling a painted layer. It was the main source of scroll
       stutter here. Letting the lattice scroll with the content is both cheaper and, on a
       page this long, indistinguishable. */
    background-image: radial-gradient(var(--ink-grid) 1px, transparent 1px);
    background-size: 18px 18px;
  }

  .shell.dragging::after {
    content: "Drop to analyze";
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    border: 1px dashed var(--ink-faint);
    color: var(--ink-hi);
    font-family: var(--mono);
    font-size: 0.82rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    pointer-events: none;
  }

  /* ── chrome ── */

  .topbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.8rem 1.6rem;
    /* Opaque rather than a translucent blur. A `backdrop-filter` on a sticky bar makes the
       compositor re-blur whatever scrolls beneath it every frame, which is the second half
       of the stutter. Solid also suits the flat instrument look better than frosted glass. */
    background: var(--bg);
    border-bottom: 1px solid var(--ink-hair);
  }

  .brand {
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: 0.94rem;
    font-weight: 500;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    font-family: var(--mono);
  }

  .tagline {
    margin: 0.1rem 0 0;
    font-size: 0.74rem;
    color: var(--ink-low);
  }

  .topbar-actions {
    display: flex;
    gap: 0.4rem;
  }

  button {
    font: inherit;
    cursor: pointer;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 0.42rem;
    padding: 0.44em 0.8em;
    font-family: var(--mono);
    font-size: 0.7rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    border-radius: 2px;
    border: 1px solid var(--ink-hair);
    background: transparent;
    color: var(--ink-mid);
    transition: color 0.18s ease, border-color 0.18s ease;
  }

  .ghost:hover:not(:disabled) {
    color: var(--ink-hi);
    border-color: var(--ink-faint);
  }

  .ghost:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .icon-only {
    padding: 0.44em 0.55em;
  }

  /* Inverted rather than accented: in a monochrome system the strongest thing available
     is ink itself, so the primary action is a solid block of it. */
  .primary {
    padding: 0.6em 1.4em;
    font-family: var(--mono);
    font-size: 0.74rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    border-radius: 2px;
    border: none;
    background: var(--ink-hi);
    color: var(--bg);
  }

  .primary:hover {
    opacity: 0.86;
  }

  :global(button:focus-visible),
  :global(input:focus-visible),
  :global([tabindex]:focus-visible) {
    outline: 1px solid var(--ink-mid);
    outline-offset: 2px;
  }

  .page {
    max-width: 900px;
    margin: 0 auto;
    padding: 1.9rem 1.6rem 0;
    display: flex;
    flex-direction: column;
    gap: 1.15rem;
  }

  /* ── empty + loading ── */

  .dropzone,
  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    padding: 5rem 2rem;
    text-align: center;
    color: var(--ink-low);
  }

  .dropzone {
    border: 1px dashed var(--ink-faint);
    border-radius: 3px;
  }

  .dropzone :global(svg) {
    color: var(--ink-low);
  }

  .dropzone h2 {
    margin: 0.5rem 0 0;
    font-size: 1.05rem;
    font-weight: 450;
    letter-spacing: -0.01em;
    color: var(--ink-hi);
  }

  .dropzone p {
    margin: 0 0 1rem;
    font-size: 0.82rem;
  }

  .loading p {
    margin: 1rem 0 0;
    font-family: var(--mono);
    font-size: 0.76rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-mid);
  }

  .hint {
    font-size: 0.74rem;
    color: var(--ink-low);
  }

  .error {
    color: var(--bad);
    font-size: 0.8rem;
    margin: 0.7rem 0 0;
    font-family: var(--mono);
  }

  .error.standalone {
    padding: 1rem 1.2rem;
    border: 1px solid var(--ink-hair);
    border-left: 2px solid var(--bad);
    border-radius: 2px;
  }

  /* ── surfaces ──
     No filled cards and no rounded boxes. Sections are separated by a hairline and by
     space, which keeps the dot lattice continuous behind everything. */

  .card,
  .verdict,
  .finding {
    padding: 1.4rem 1.5rem;
    border: 1px solid var(--ink-hair);
    border-radius: 3px;
    background: var(--bg-raised);
  }

  .section-title {
    margin: 0 0 1.15rem;
    font-family: var(--mono);
    font-size: 0.66rem;
    font-weight: 500;
    color: var(--ink-low);
    text-transform: uppercase;
    letter-spacing: 0.18em;
  }

  /* ── verdict ── */

  .verdict {
    border-left: 2px solid var(--verdict-ink, var(--ink-faint));
  }

  .verdict.authentic {
    --verdict-ink: var(--ok);
  }
  .verdict.transcoded {
    --verdict-ink: var(--warn);
  }
  .verdict.indeterminate {
    --verdict-ink: var(--ink-low);
  }

  .verdict-head {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
  }

  .verdict-head :global(svg) {
    color: var(--verdict-ink);
    flex-shrink: 0;
    margin-top: 0.15rem;
  }

  .verdict-copy {
    flex: 1;
    min-width: 0;
  }

  /* Light and wide rather than heavy: the verdict carries enough weight from its position
     on the page, and shouting it would undercut a measurement that is explicitly hedged. */
  .verdict-copy h2 {
    margin: 0;
    font-size: 1.42rem;
    font-weight: 300;
    letter-spacing: -0.015em;
    color: var(--verdict-ink);
  }

  .verdict-copy p {
    margin: 0.28rem 0 0;
    font-size: 0.83rem;
    line-height: 1.5;
    color: var(--ink-mid);
  }

  .confidence {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    flex-shrink: 0;
  }

  .confidence-value {
    font-family: var(--mono);
    font-size: 1.5rem;
    font-weight: 400;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    color: var(--verdict-ink);
  }

  .confidence-value i {
    font-style: normal;
    font-size: 0.8rem;
    color: var(--ink-low);
  }

  .confidence-label {
    margin-top: 0.35rem;
    font-family: var(--mono);
    font-size: 0.6rem;
    color: var(--ink-low);
    text-transform: uppercase;
    letter-spacing: 0.16em;
  }

  .evidence {
    margin: 1.25rem 0 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    font-size: 0.84rem;
    line-height: 1.6;
  }

  /* Dotted rule instead of a solid one — the orb's material used as punctuation. */
  .evidence li {
    padding-left: 0.95rem;
    color: var(--ink-mid);
    background-image: radial-gradient(var(--ink-faint) 1px, transparent 1px);
    background-size: 3px 4px;
    background-repeat: repeat-y;
    background-position: left top;
  }

  /* ── findings ── */

  .findings {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .finding {
    display: flex;
    gap: 0.85rem;
    padding: 1rem 1.2rem;
    border-left: 2px solid var(--finding-ink);
  }

  .finding.warn {
    --finding-ink: var(--warn);
  }
  .finding.bad {
    --finding-ink: var(--bad);
  }

  .finding :global(svg) {
    color: var(--finding-ink);
    flex-shrink: 0;
    margin-top: 0.15rem;
  }

  .finding h3 {
    margin: 0;
    font-size: 0.86rem;
    font-weight: 500;
    color: var(--ink-hi);
  }

  .finding p {
    margin: 0.3rem 0 0;
    font-size: 0.8rem;
    color: var(--ink-mid);
    line-height: 1.55;
  }

  /* ── file header ── */

  .file-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.7rem;
  }

  .filename {
    margin: 0;
    font-family: var(--mono);
    font-size: 0.9rem;
    font-weight: 400;
    color: var(--ink-hi);
    word-break: break-all;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .chip {
    padding: 0.24em 0.55em;
    font-family: var(--mono);
    font-size: 0.64rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    border: 1px solid var(--ink-hair);
    border-radius: 2px;
    color: var(--ink-low);
    white-space: nowrap;
  }

  .bandwidth {
    margin-top: 1.4rem;
  }

  /* ── readings ── */

  .bandwidth-head,
  .metric-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.5rem;
  }

  .label {
    font-size: 0.79rem;
    color: var(--ink-mid);
  }

  /* Every number in the interface is monospaced and tabular. This app is a row of
     readings; proportional digits would make them wander. */
  .value {
    font-family: var(--mono);
    font-size: 0.83rem;
    font-variant-numeric: tabular-nums;
    color: var(--ink-hi);
    text-align: right;
  }

  .value em {
    font-style: normal;
    color: var(--ink-low);
  }

  .value.good {
    color: var(--ok);
  }
  .value.warn,
  .value-warn {
    color: var(--warn);
  }
  .value.bad {
    color: var(--bad);
  }

  .metric + .metric {
    margin-top: 1.25rem;
  }

  .scale-note {
    display: block;
    margin-top: 0.5rem;
    font-size: 0.7rem;
    color: var(--ink-low);
    line-height: 1.5;
  }

  .metric-columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.15rem;
    align-items: start;
  }

  .spectral-stats {
    display: flex;
    gap: 3rem;
    margin-top: 1.2rem;
  }

  .stat-block {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .stat-block .label {
    font-family: var(--mono);
    font-size: 0.63rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--ink-low);
  }

  .stat-block .value {
    text-align: left;
    font-size: 1.05rem;
    font-weight: 400;
  }

  .note {
    margin: 1rem 0 0;
    font-size: 0.73rem;
    color: var(--ink-low);
    line-height: 1.6;
  }

  /* ── channel table ── */

  .channels {
    width: 100%;
    margin-top: 1.4rem;
    border-collapse: collapse;
    font-family: var(--mono);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }

  .channels th {
    color: var(--ink-low);
    font-weight: 400;
    text-align: right;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-size: 0.62rem;
    padding-bottom: 0.45em;
    border-bottom: 1px solid var(--ink-hair);
  }

  .channels td {
    text-align: right;
    padding: 0.45em 0.4em;
    color: var(--ink-mid);
  }

  .channels tbody tr + tr td {
    border-top: 1px solid var(--ink-hair);
  }

  .channels th:first-child,
  .channels td:first-child {
    text-align: left;
    padding-left: 0;
    color: var(--ink-low);
  }

  /* ── file facts ── */

  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 0.7rem 1.6rem;
    margin: 0;
  }

  /* Dotted leader between name and value, the way a spec sheet sets them. */
  .facts div {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    font-size: 0.78rem;
  }

  .facts dt {
    color: var(--ink-low);
    white-space: nowrap;
  }

  .facts div::after {
    content: "";
    flex: 1;
    height: 1px;
    margin-bottom: 0.22em;
    background-image: radial-gradient(var(--ink-faint) 1px, transparent 1px);
    background-size: 4px 2px;
  }

  .facts dd {
    margin: 0;
    order: 3;
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    color: var(--ink-hi);
    white-space: nowrap;
  }

  .facts dd.good {
    color: var(--ok);
  }
  .facts dd.bad {
    color: var(--bad);
  }

  .disclaimer {
    margin: 0.4rem 0 1.2rem;
    font-size: 0.73rem;
    line-height: 1.7;
    color: var(--ink-low);
  }

  /* ── player ── */

  .player {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    bottom: 1.2rem;
    z-index: 15;
    display: flex;
    align-items: center;
    gap: 0.8rem;
    width: min(660px, calc(100vw - 3rem));
    padding: 0.6rem 1rem;
    border-radius: 3px;
    border: 1px solid var(--ink-hair);
    /* Opaque for the same reason as the topbar — this sits over the scrolling page. */
    background: var(--bg);
    box-shadow: 0 2px 18px rgba(0, 0, 0, 0.35);
  }

  .play {
    display: grid;
    place-items: center;
    width: 1.9rem;
    height: 1.9rem;
    border-radius: 2px;
    border: 1px solid var(--ink-faint);
    background: transparent;
    color: var(--ink-hi);
    flex-shrink: 0;
    transition: border-color 0.18s ease;
  }

  .play:hover {
    border-color: var(--ink-mid);
  }

  .scrubber {
    flex: 1;
    min-width: 0;
    appearance: none;
    height: 3px;
    border-radius: 2px;
    background: var(--ink-faint);
  }

  .scrubber::-webkit-slider-thumb {
    appearance: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--ink-hi);
    cursor: pointer;
  }

  .time {
    font-family: var(--mono);
    font-size: 0.7rem;
    color: var(--ink-mid);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .muted-time {
    color: var(--ink-low);
  }

  @media (max-width: 720px) {
    .metric-columns {
      grid-template-columns: 1fr;
    }
    .page {
      padding: 1.3rem 1rem 0;
    }
    .card,
    .verdict {
      padding: 1.15rem 1.2rem;
    }
    .spectral-stats {
      gap: 1.8rem;
    }
  }
</style>
