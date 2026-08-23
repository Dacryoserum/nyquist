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
  import MdctGrid from "$lib/components/MdctGrid.svelte";
import Meter from "$lib/components/Meter.svelte";
  import Spectrogram from "$lib/components/Spectrogram.svelte";
  import ThinkingOrb from "$lib/components/ThinkingOrb.svelte";
  import type { IconName } from "$lib/icons";
  import { fmtNumber, initLang, langState, t, toggleLang } from "$lib/i18n.svelte";

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
  let volume = $state(1);
  let muted = $state(false);

  const T = $derived(t());

  // Keeps the document's own `lang` attribute (screen readers, browser spellcheck/translate
  // prompts) in sync with the in-app toggle — `app.html` only sets the pre-hydration default.
  $effect(() => {
    document.documentElement.lang = langState.current;
  });

  onMount(() => {
    const saved = localStorage.getItem("nyquist-theme");
    theme = saved === "light" || saved === "dark" ? saved : matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
    applyTheme();
    initLang();

    try {
      const savedVolume = localStorage.getItem("nyquist-volume");
      if (savedVolume !== null) volume = Math.min(1, Math.max(0, Number(savedVolume)));
    } catch {
      /* Best-effort only. */
    }

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
      filters: [
        { name: t().dialogs.audioFiles, extensions: ["flac", "mp3", "m4a", "aac", "alac", "wav", "ogg"] }
      ]
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

  function toggleMute() {
    muted = !muted;
  }

  function setVolume(v: number) {
    volume = Math.min(1, Math.max(0, v));
    if (volume > 0) muted = false;
    try {
      localStorage.setItem("nyquist-volume", String(volume));
    } catch {
      /* Best-effort only. */
    }
  }

  async function handleExport() {
    if (!result) return;
    const path = await save({
      defaultPath: `${result.file_info.filename}.report.json`,
      filters: [{ name: t().dialogs.jsonFiles, extensions: ["json"] }]
    });
    if (!path) return;
    await exportReport(path, JSON.stringify(result, null, 2));
  }

  const fmt = (v: number, d = 1) => fmtNumber(v, d);
  const fmtDuration = (s: number) => `${Math.floor(s / 60)}:${Math.round(s % 60).toString().padStart(2, "0")}`;
  const fmtHz = (hz: number) => (hz >= 1000 ? `${fmtNumber(hz / 1000, 1)} kHz` : `${fmtNumber(hz, 0)} Hz`);
  const fmtCount = (n: number) =>
    n >= 1_000_000 ? `${fmtNumber(n / 1_000_000, 1)}M` : n >= 1_000 ? `${fmtNumber(n / 1_000, 1)}K` : `${n}`;

  type Tone = "good" | "warn" | "bad" | "neutral";

  /** DR bands follow the Pleasurize Music Foundation / DR-database convention the
   * audiophile community actually publishes against. A convention, not a standard. */
  const drTone = (dr: number): Tone => (dr >= 12 ? "good" : dr >= 8 ? "warn" : "bad");

  /** EBU R128 and every major streaming platform ask for -1 dBTP of headroom; above 0 the
   * file will clip on any resampling or lossy re-encode downstream. */
  const truePeakTone = (dbtp: number): Tone => (dbtp > 0 ? "bad" : dbtp > -1 ? "warn" : "good");

  const verdictMeta = $derived<Record<Verdict, { label: string; icon: IconName; tone: string; blurb: string }>>({
    probably_authentic: {
      label: T.verdict.probablyAuthentic.label,
      icon: "checkCircle",
      tone: "authentic",
      blurb: T.verdict.probablyAuthentic.blurb
    },
    probably_transcoded: {
      label: T.verdict.probablyTranscoded.label,
      icon: "alertCircle",
      tone: "transcoded",
      blurb: T.verdict.probablyTranscoded.blurb
    },
    indeterminate: {
      label: T.verdict.indeterminate.label,
      icon: "helpCircle",
      tone: "indeterminate",
      blurb: T.verdict.indeterminate.blurb
    }
  });

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
        title: T.findings.checksumMismatchTitle,
        detail: T.findings.checksumMismatchDetail
      });
    }
    if (fi.decode_errors > 0) {
      f.push({
        icon: "alertTriangle",
        tone: "bad",
        title: T.findings.damagedPacketsTitle(fi.decode_errors),
        detail: T.findings.damagedPacketsDetail
      });
    }
    if (bd.declared_bit_depth !== null && bd.effective_bit_depth !== null && bd.effective_bit_depth < bd.declared_bit_depth) {
      f.push({
        icon: "layers",
        tone: "warn",
        title: T.findings.bitDepthPaddingTitle(bd.declared_bit_depth, bd.effective_bit_depth),
        detail: T.findings.bitDepthPaddingDetail(bd.effective_bit_depth)
      });
    }
    if (sr.likely_upsampled) {
      f.push({
        icon: "ruler",
        tone: "warn",
        title: T.findings.upsampledTitle(fmt(sr.declared_sample_rate_hz / 1000, 1), fmt(sr.content_bandwidth_hz / 1000, 1)),
        detail: T.findings.upsampledDetail(
          fmt(sr.bandwidth_ratio * 100, 0),
          sr.sufficient_sample_rate_hz ? fmt(sr.sufficient_sample_rate_hz / 1000, 1) : null
        )
      });
    }
    if (sa.clipping_count_total > 0) {
      f.push({
        icon: "clip",
        tone: sa.clipping_count_total > 1000 ? "bad" : "warn",
        title: T.findings.clippedSamplesTitle(fmtCount(sa.clipping_count_total)),
        detail: T.findings.clippedSamplesDetail
      });
    }
    return f;
  });
</script>

<svelte:head><title>Nyquist</title></svelte:head>

<div class="shell" class:dragging data-drop-label={T.dragOverlay}>
  <header class="topbar">
    <!-- Wordmark only. The orb was tried here, held still, and dropped: at 30px its dots
         collapse into a smudge that reads as neither the orb nor a logo. The name set in
         spaced monospace carries the instrument feel on its own. -->
    <div class="brand">
      <h1>Nyquist</h1>
      <p class="tagline">{T.brand.tagline}</p>
    </div>
    <div class="topbar-actions">
      {#if result}
        <button class="ghost" onclick={pickAndAnalyze} disabled={loading}>
          <Icon name="upload" size={14} /> {T.actions.openAnother}
        </button>
        <button class="ghost" onclick={handleExport}>
          <Icon name="download" size={14} /> {T.actions.exportJson}
        </button>
      {/if}
      <!-- Discreet by design: two-letter code naming the *other* language, matching the
           same small mono ghost-button chrome as the theme toggle rather than a flag or a
           globe icon that would imply a menu of more than one alternative. -->
      <button class="ghost icon-only lang-toggle" onclick={toggleLang} aria-label={T.actions.switchLang}>
        {langState.current === "fr" ? "EN" : "FR"}
      </button>
      <button class="ghost icon-only" onclick={toggleTheme} aria-label={T.actions.switchTheme}>
        <Icon name={theme === "dark" ? "sun" : "moon"} size={15} />
      </button>
    </div>
  </header>

  <main class="page">
    {#if !result && !loading}
      <section class="dropzone">
        <Icon name="upload" size={26} />
        <h2>{T.dropzone.title}</h2>
        <p>{T.dropzone.subtitle}</p>
        <button class="primary" onclick={pickAndAnalyze}>{T.dropzone.chooseFile}</button>
        {#if error}
          <p class="error" role="alert">{error}</p>
        {/if}
      </section>
    {/if}

    {#if loading}
      <section class="loading" aria-live="polite">
        <ThinkingOrb state="composing" size={64} dark={theme === "dark"} />
        <p>{T.loading.text}</p>
        <span class="hint">{T.loading.hint}</span>
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
            <span class="confidence-value">{fmt(ta.confidence_score * 100, 0)}<i>%</i></span>
            <span class="confidence-label">{T.verdict.confidence}</span>
          </div>
        </div>
        <ul class="evidence">
          {#each ta.indicators as indicator (indicator.code)}
            <li>{T.indicator(indicator)}</li>
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
            {#if fi.bit_depth}<span class="chip">{T.file2.bits(fi.bit_depth)}</span>{/if}
            <span class="chip">{fi.channels === 2 ? T.file2.stereo : T.file2.channelCount(fi.channels)}</span>
            <span class="chip">{fmtDuration(fi.duration_seconds)}</span>
          </span>
        </div>

        <!-- Bandwidth against Nyquist: the one picture that makes an upsampled file
             obvious at a glance, and the reason sample_rate_analysis exists. -->
        <div class="bandwidth">
          <div class="bandwidth-head">
            <span class="label">{T.file.bandwidthUsed}</span>
            <span class="value" class:value-warn={sr.likely_upsampled}>
              {T.file.bandwidthPhrase(fmtHz(sr.content_bandwidth_hz), fmtHz(fi.nyquist_hz))}
              <em>({fmt(sr.bandwidth_ratio * 100, 0)}%)</em>
            </span>
          </div>
          <Meter value={sr.bandwidth_ratio} min={0} max={1} tone={sr.likely_upsampled ? "warn" : "good"} />
        </div>
      </section>

      <section class="card">
        <h2 class="section-title">{T.spectrum.title}</h2>
        <Spectrogram
          data={spa.spectrogram}
          spectralCutoffHz={spa.spectral_cutoff_hz}
          cutoffOverTimeHz={spa.cutoff_over_time_hz}
          currentTimeSeconds={currentTime}
          onSeek={audioSrc ? seekTo : undefined}
        />
        <div class="spectral-stats">
          <div class="stat-block">
            <span class="label">{T.spectrum.bandwidth}</span>
            <span class="value">{fmtHz(spa.spectral_cutoff_hz)}</span>
          </div>
          <div class="stat-block">
            <span class="label">{T.spectrum.rolloffSteepness}</span>
            <span class="value">
              {spa.encoder_edge_hz !== null
                ? T.spectrum.steepnessValue(fmt(spa.rolloff_steepness_db_per_khz, 0), fmtHz(spa.encoder_edge_hz))
                : T.spectrum.noEdgeFound}
            </span>
          </div>
        </div>
        <p class="note">{T.spectrum.note}</p>
      </section>

      <section class="card">
        <h2 class="section-title">{T.mdct.title}</h2>
        <MdctGrid grid={result.mdct_grid} />
      </section>

      <div class="metric-columns">
        <section class="card">
          <h2 class="section-title">{T.loudness.title}</h2>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.loudness.integratedLoudness}</span>
              <span class="value">{sa.lufs_integrated !== null ? `${fmt(sa.lufs_integrated)} LUFS` : T.loudness.na}</span>
            </div>
            {#if sa.lufs_integrated !== null}
              <Meter value={sa.lufs_integrated} min={-30} max={0} reference={-14} referenceLabel={T.loudness.lufsTargetNote} />
              <span class="scale-note">{T.loudness.lufsTargetNote}</span>
            {/if}
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.loudness.truePeak}</span>
              <span class="value {truePeakTone(sa.true_peak_dbtp)}">{fmt(sa.true_peak_dbtp)} dBTP</span>
            </div>
            <Meter value={sa.true_peak_dbtp} min={-12} max={3} tone={truePeakTone(sa.true_peak_dbtp)} reference={-1} referenceLabel="-1 dBTP" />
            <span class="scale-note">
              {sa.true_peak_dbtp > 0 ? T.loudness.clipWarnNote : T.loudness.headroomNote}
            </span>
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.loudness.loudnessRange}</span>
              <span class="value">{sa.loudness_range_lu !== null ? `${fmt(sa.loudness_range_lu)} LU` : T.loudness.na}</span>
            </div>
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.loudness.peakRms}</span>
              <span class="value">{fmt(sa.peak_dbfs)} / {fmt(sa.rms_dbfs)} dBFS</span>
            </div>
          </div>
        </section>

        <section class="card">
          <h2 class="section-title">{T.dynamics.title}</h2>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.dynamics.dynamicRange}</span>
              <span class="value {dr.dr14 !== null ? drTone(dr.dr14) : ''}">
                {dr.dr14 !== null ? `DR${dr.dr14}` : T.loudness.na}
              </span>
            </div>
            {#if dr.dr14 !== null}
              <Meter value={dr.dr14} min={0} max={20} tone={drTone(dr.dr14)} />
              <span class="scale-note">{T.dynamics.drNote(T.dynamics.drLabel(dr.dr14))}</span>
            {/if}
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.dynamics.clippedSamples}</span>
              <span class="value {sa.clipping_count_total > 0 ? 'warn' : ''}">{fmtCount(sa.clipping_count_total)}</span>
            </div>
          </div>

          <table class="channels">
            <thead>
              <tr>
                <th>{T.dynamics.table.ch}</th>
                <th>{T.dynamics.table.peak}</th>
                <th>{T.dynamics.table.rms}</th>
                <th>{T.dynamics.table.crest}</th>
                <th>{T.dynamics.table.dr}</th>
                <th>{T.dynamics.table.clipped}</th>
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
          <p class="note">{T.dynamics.channelsNote}</p>
        </section>
      </div>

      <section class="card">
        <h2 class="section-title">{T.file2.title}</h2>
        <dl class="facts">
          <div><dt>{T.file2.container}</dt><dd>{fi.container}</dd></div>
          <div><dt>{T.file2.codec}</dt><dd>{fi.codec}</dd></div>
          <div><dt>{T.file2.sampleRate}</dt><dd>{fmtHz(fi.sample_rate_hz)}</dd></div>
          <div><dt>{T.file2.nyquist}</dt><dd>{fmtHz(fi.nyquist_hz)}</dd></div>
          <div><dt>{T.file2.bitDepth}</dt><dd>{fi.bit_depth ? T.file2.bits(fi.bit_depth) : "—"}</dd></div>
          <div><dt>{T.file2.channels}</dt><dd>{fi.channels}</dd></div>
          <div><dt>{T.file2.duration}</dt><dd>{fmtDuration(fi.duration_seconds)}</dd></div>
          <div><dt>{T.file2.size}</dt><dd>{T.file2.megabytes(fmt(fi.file_size_bytes / 1_000_000, 1))}</dd></div>
          <div><dt>{T.file2.avgBitrate}</dt><dd>{fi.bitrate_kbps ? `${fmt(fi.bitrate_kbps, 0)} kbps` : "—"}</dd></div>
          <div><dt>{T.file2.samples}</dt><dd>{fmtCount(fi.sample_count)}</dd></div>
          <div>
            <dt>{T.file2.integrity}</dt>
            <dd class:bad={fi.integrity_verified === false} class:good={fi.integrity_verified === true}>
              {fi.integrity_verified === true
                ? T.file2.integrityVerified
                : fi.integrity_verified === false
                  ? T.file2.integrityMismatch
                  : fi.codec === "flac"
                    ? T.file2.integrityNoChecksum
                    : T.file2.integrityUnavailable}
            </dd>
          </div>
        </dl>
      </section>

      <div class="metric-columns">
        <section class="card">
          <h2 class="section-title">{T.spectralDetail.title}</h2>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.spectralDetail.stability}</span>
              <span class="value">± {fmtHz(spa.cutoff_stability_hz)}</span>
            </div>
            <span class="scale-note">{T.spectralDetail.stabilityNote}</span>
          </div>

          <div class="metric">
            <div class="metric-head">
              <span class="label">{T.spectralDetail.stopbandDepth}</span>
              <span class="value">
                {spa.stopband_depth_db !== null
                  ? `${fmt(spa.stopband_depth_db, 0)} dB`
                  : T.spectralDetail.noStopband}
              </span>
            </div>
            {#if spa.stopband_depth_db !== null}
              <span class="scale-note">{T.spectralDetail.stopbandNote}</span>
            {/if}
          </div>

          <!-- The spectral shape the verdict is drawn from, as a table rather than a claim:
               an encoder wall reads as an abrupt fall between neighbours, a dark master as a
               steady slope. -->
          <span class="label band-label">{T.spectralDetail.bandLevels}</span>
          <div class="bands">
            {#each spa.band_levels_db as band (band.low_hz)}
              <div class="band-row">
                <span class="band-range">
                  {fmtHz(band.low_hz)}–{band.high_hz !== null ? fmtHz(band.high_hz) : T.spectralDetail.toNyquist}
                </span>
                <Meter value={band.level_db} min={-90} max={0} />
                <span class="band-value">{fmt(band.level_db, 0)}</span>
              </div>
            {/each}
          </div>
          <p class="note">{T.spectralDetail.bandLevelsNote}</p>
        </section>

        {#if result.stereo_analysis}
          {@const st = result.stereo_analysis}
          <section class="card">
            <h2 class="section-title">{T.stereo.title}</h2>

            <div class="metric">
              <div class="metric-head">
                <span class="label">{T.stereo.correlation}</span>
                <span class="value {st.mono_compatibility_risk ? 'warn' : ''}">{fmt(st.correlation, 2)}</span>
              </div>
              <Meter value={st.correlation} min={-1} max={1} reference={0} referenceLabel="0" />
              <span class="scale-note">{T.stereo.correlationNote}</span>
            </div>

            <div class="metric">
              <div class="metric-head">
                <span class="label">{T.stereo.width}</span>
                <span class="value">{fmt(st.side_to_mid_db, 1)} dB</span>
              </div>
            </div>

            {#if st.dual_mono}
              <p class="note flag">{T.stereo.dualMonoNote}</p>
            {:else if st.effectively_mono}
              <p class="note flag">{T.stereo.effectivelyMono}</p>
            {/if}
            {#if st.mono_compatibility_risk}
              <p class="note flag">{T.stereo.phaseRiskNote}</p>
            {/if}

            <span class="label band-label">{T.stereo.perBand}</span>
            <div class="bands">
              {#each st.per_band as band (band.name)}
                <div class="band-row">
                  <span class="band-range">{T.stereo.bandName(band.name)}</span>
                  <Meter value={band.side_to_mid_db} min={-60} max={0} />
                  <span class="band-value">{fmt(band.side_to_mid_db, 0)}</span>
                </div>
              {/each}
            </div>
            <p class="note">{T.stereo.note}</p>
          </section>
        {/if}
      </div>

      <p class="disclaimer">{T.disclaimer}</p>
    {/if}
  </main>

  {#if result && audioSrc}
    <div class="player">
      <button class="play" onclick={togglePlay} aria-label={isPlaying ? T.player.pause : T.player.play}>
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
        aria-label={T.player.playbackPosition}
        oninput={(e) => {
          scrubbing = true;
          seekTo(Number(e.currentTarget.value));
        }}
        onchange={() => (scrubbing = false)}
      />
      <span class="time muted-time">{fmtDuration(result.file_info.duration_seconds)}</span>
      <button class="mute" onclick={toggleMute} aria-label={muted || volume === 0 ? T.player.unmute : T.player.mute}>
        <Icon name={muted || volume === 0 ? "volumeMute" : "speaker"} size={15} />
      </button>
      <input
        class="volume"
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={muted ? 0 : volume}
        aria-label={T.player.volume}
        oninput={(e) => setVolume(Number(e.currentTarget.value))}
      />
    </div>
  {/if}

  {#if audioSrc}
    <audio
      bind:this={audioEl}
      src={audioSrc}
      bind:volume
      bind:muted
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
    /* Read from the `data-drop-label` attribute (set from `T.dragOverlay` in the markup)
       rather than a literal string, so this CSS-only overlay — kept off the JS/reactive
       path on purpose, see the comment on `.shell` above — still follows the language
       toggle. */
    content: attr(data-drop-label);
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

  /* A note that states a property of *this* file rather than explaining the panel — brought
     up to body-text ink so it doesn't read as boilerplate the way the panel notes do. */
  .note.flag {
    margin-top: 0.7rem;
    color: var(--ink-mid);
  }

  /* ── band breakdown (spectral levels, stereo width) ── */

  .band-label {
    display: block;
    margin-top: 1.6rem;
  }

  .bands {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    margin-top: 0.7rem;
  }

  .band-row {
    display: grid;
    /* Fixed label and value columns so every meter starts and ends on the same x, making
       the column of bars readable as a spectral shape rather than as separate readings. */
    grid-template-columns: 8.5rem 1fr 2.5rem;
    align-items: center;
    gap: 0.7rem;
  }

  .band-range {
    font-family: var(--mono);
    font-size: 0.68rem;
    color: var(--ink-low);
    white-space: nowrap;
  }

  .band-value {
    font-family: var(--mono);
    font-size: 0.68rem;
    color: var(--ink-mid);
    text-align: right;
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

  /* ── volume ── */

  .mute {
    display: grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    flex-shrink: 0;
    border: none;
    background: transparent;
    color: var(--ink-mid);
    transition: color 0.18s ease;
  }

  .mute:hover {
    color: var(--ink-hi);
  }

  .volume {
    flex-shrink: 0;
    width: 64px;
    appearance: none;
    height: 3px;
    border-radius: 2px;
    background: var(--ink-faint);
  }

  .volume::-webkit-slider-thumb {
    appearance: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ink-hi);
    cursor: pointer;
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
    /* The player is already tight against a narrow scrubber; the volume slider is the
       first thing to go, not the mute button that still communicates state. */
    .volume {
      display: none;
    }
  }
</style>
