<script lang="ts">
  /** Two files, read side by side.
   *
   * The question this answers is the one people actually have when they hold two copies of
   * the same album: which of these is the better master, and is either of them lying? So the
   * verdicts lead, the measurements follow as a single aligned table, and the spectrograms
   * sit underneath where the difference is usually visible at a glance.
   *
   * Rows are marked, never scored. Where "more is better" is genuinely unambiguous —
   * bandwidth, dynamic range, fewer clipped samples — the leading side is flagged; for
   * everything else the two values are simply put next to each other and left to the reader.
   * Declaring an overall winner would mean weighing incommensurable things (a wider spectrum
   * against a louder master) and this tool has no basis for that weighting. */
  import Icon from "$lib/components/Icon.svelte";
  import MdctGrid from "$lib/components/MdctGrid.svelte";
  import Spectrogram from "$lib/components/Spectrogram.svelte";
  import { fmtNumber, t } from "$lib/i18n.svelte";
  import type { AnalysisResult, Verdict } from "$lib/api";
  import type { IconName } from "$lib/icons";

  const T = $derived(t());

  let {
    a,
    b,
    onClear
  }: { a: AnalysisResult; b: AnalysisResult; onClear: () => void } = $props();

  const fmt = (v: number, d = 1) => fmtNumber(v, d);
  const fmtHz = (hz: number) => (hz >= 1000 ? `${fmtNumber(hz / 1000, 1)} kHz` : `${fmtNumber(hz, 0)} Hz`);
  const fmtDuration = (s: number) =>
    `${Math.floor(s / 60)}:${Math.round(s % 60).toString().padStart(2, "0")}`;
  const na = () => T.loudness.na;

  const verdictMeta: Record<Verdict, { label: string; icon: IconName; tone: string }> = $derived({
    probably_authentic: { label: T.verdict.probablyAuthentic.label, icon: "checkCircle", tone: "authentic" },
    probably_transcoded: { label: T.verdict.probablyTranscoded.label, icon: "alertCircle", tone: "transcoded" },
    indeterminate: { label: T.verdict.indeterminate.label, icon: "helpCircle", tone: "indeterminate" },
    declared_lossy: { label: T.verdict.declaredLossy.label, icon: "waveform", tone: "declared" }
  });

  /** `better` says which side leads when that is a meaningful thing to say at all. */
  type Row = { label: string; a: string; b: string; better?: "a" | "b" | null };

  /** Compares two numbers, returning the winning side or null when they are close enough
   * that calling one better would be noise. `epsilon` is per-metric because a 0.1 dB
   * loudness difference is nothing while a 0.1 kHz bandwidth difference can matter. */
  function lead(
    va: number | null,
    vb: number | null,
    epsilon: number,
    higherIsBetter = true
  ): "a" | "b" | null {
    if (va === null || vb === null || Math.abs(va - vb) <= epsilon) return null;
    const aWins = higherIsBetter ? va > vb : va < vb;
    return aWins ? "a" : "b";
  }

  const rows = $derived.by<Row[]>(() => {
    const [fa, fb] = [a.file_info, b.file_info];
    const [sa, sb] = [a.signal_analysis, b.signal_analysis];
    const [spa, spb] = [a.spectral_analysis, b.spectral_analysis];
    return [
      { label: T.file2.codec, a: fa.codec, b: fb.codec },
      { label: T.file2.sampleRate, a: fmtHz(fa.sample_rate_hz), b: fmtHz(fb.sample_rate_hz) },
      {
        label: T.file2.bitDepth,
        a: fa.bit_depth ? T.file2.bits(fa.bit_depth) : "—",
        b: fb.bit_depth ? T.file2.bits(fb.bit_depth) : "—"
      },
      { label: T.file2.duration, a: fmtDuration(fa.duration_seconds), b: fmtDuration(fb.duration_seconds) },
      {
        label: T.spectrum.bandwidth,
        a: fmtHz(spa.spectral_cutoff_hz),
        b: fmtHz(spb.spectral_cutoff_hz),
        better: lead(spa.spectral_cutoff_hz, spb.spectral_cutoff_hz, 300)
      },
      {
        label: T.spectrum.rolloffSteepness,
        a: spa.encoder_edge_hz !== null ? `${fmt(spa.rolloff_steepness_db_per_khz, 0)} dB/kHz` : T.spectrum.noEdgeFound,
        b: spb.encoder_edge_hz !== null ? `${fmt(spb.rolloff_steepness_db_per_khz, 0)} dB/kHz` : T.spectrum.noEdgeFound,
        // A shallower rolloff means less evidence of an encoder lowpass.
        better: lead(spa.rolloff_steepness_db_per_khz, spb.rolloff_steepness_db_per_khz, 5, false)
      },
      {
        label: T.dynamics.dynamicRange,
        a: a.dynamic_range.dr14 !== null ? `DR${a.dynamic_range.dr14}` : na(),
        b: b.dynamic_range.dr14 !== null ? `DR${b.dynamic_range.dr14}` : na(),
        better: lead(a.dynamic_range.dr14, b.dynamic_range.dr14, 0.5)
      },
      {
        label: T.loudness.integratedLoudness,
        a: sa.lufs_integrated !== null ? `${fmt(sa.lufs_integrated)} LUFS` : na(),
        b: sb.lufs_integrated !== null ? `${fmt(sb.lufs_integrated)} LUFS` : na()
      },
      {
        label: T.loudness.truePeak,
        a: `${fmt(sa.true_peak_dbtp)} dBTP`,
        b: `${fmt(sb.true_peak_dbtp)} dBTP`
      },
      {
        label: T.dynamics.clippedSamples,
        a: `${sa.clipping_count_total}`,
        b: `${sb.clipping_count_total}`,
        better: lead(sa.clipping_count_total, sb.clipping_count_total, 0, false)
      },
      {
        label: T.mdct.title,
        a: a.mdct_grid.analyzed ? `${fmt(a.mdct_grid.z_score, 1)} σ` : na(),
        b: b.mdct_grid.analyzed ? `${fmt(b.mdct_grid.z_score, 1)} σ` : na(),
        // Lower is better here: a high score is an encoder grid.
        better: a.mdct_grid.analyzed && b.mdct_grid.analyzed
          ? lead(a.mdct_grid.z_score, b.mdct_grid.z_score, 10, false)
          : null
      }
    ];
  });
</script>

<section class="compare">
  <header class="compare-head">
    <h2 class="section-title">{T.compare.title}</h2>
    <button class="ghost" onclick={onClear}>
      <Icon name="close" size={13} /> {T.compare.exit}
    </button>
  </header>

  <div class="verdicts">
    {#each [{ r: a, side: "A" }, { r: b, side: "B" }] as entry (entry.side)}
      {@const vm = verdictMeta[entry.r.transcode_assessment.verdict]}
      <article class="verdict-card {vm.tone}">
        <span class="side">{entry.side}</span>
        <h3 class="name" title={entry.r.file_info.filename}>{entry.r.file_info.filename}</h3>
        <div class="verdict-line">
          <Icon name={vm.icon} size={20} />
          <span class="label">{vm.label}</span>
          {#if entry.r.transcode_assessment.verdict !== "declared_lossy"}
            <span class="confidence">{fmt(entry.r.transcode_assessment.confidence_score * 100, 0)}%</span>
          {/if}
        </div>
      </article>
    {/each}
  </div>

  <table class="delta">
    <thead>
      <tr>
        <th scope="col">{T.compare.metric}</th>
        <th scope="col">A</th>
        <th scope="col">B</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as row (row.label)}
        <tr class:differs={row.a !== row.b}>
          <th scope="row">{row.label}</th>
          <td class:leads={row.better === "a"}>{row.a}</td>
          <td class:leads={row.better === "b"}>{row.b}</td>
        </tr>
      {/each}
    </tbody>
  </table>
  <p class="note">{T.compare.note}</p>

  <div class="stacked">
    {#each [{ r: a, side: "A" }, { r: b, side: "B" }] as entry (entry.side)}
      <div class="panel">
        <span class="panel-label">{entry.side} — {entry.r.file_info.filename}</span>
        <Spectrogram
          data={entry.r.spectral_analysis.spectrogram}
          spectralCutoffHz={entry.r.spectral_analysis.spectral_cutoff_hz}
          cutoffOverTimeHz={entry.r.spectral_analysis.cutoff_over_time_hz}
          showPalette={false}
        />
        <MdctGrid grid={entry.r.mdct_grid} />
      </div>
    {/each}
  </div>
</section>

<style>
  .compare {
    display: flex;
    flex-direction: column;
    gap: 1.4rem;
  }

  .compare-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .verdicts,
  .stacked {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .verdict-card {
    position: relative;
    padding: 1rem 1.1rem;
    border: 1px solid var(--ink-hair);
    border-radius: 4px;
    background: var(--bg-raised);
  }

  /* Same three severity tints the main verdict uses, so a card means the same thing here
     as it does on the single-file dashboard. */
  .verdict-card.transcoded {
    border-color: color-mix(in srgb, var(--bad) 45%, var(--ink-hair));
  }
  .verdict-card.authentic {
    border-color: color-mix(in srgb, var(--ok) 40%, var(--ink-hair));
  }
  .verdict-card.indeterminate {
    border-color: color-mix(in srgb, var(--warn) 40%, var(--ink-hair));
  }
  /* Neutral: this state is not a finding, so it gets no severity tint. */
  .verdict-card.declared {
    border-color: var(--ink-hair);
  }

  .side,
  .panel-label {
    font-family: var(--mono);
    font-size: 0.62rem;
    letter-spacing: 0.14em;
    color: var(--ink-low);
  }

  .name {
    margin: 0.3rem 0 0.7rem;
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--ink-hi);
    /* Filenames are long and the column is narrow; truncating keeps the two cards the same
       height so their verdict lines stay on one baseline. */
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .verdict-line {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .verdict-line .label {
    font-size: 0.85rem;
    color: var(--ink-hi);
  }

  .confidence {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 0.78rem;
    color: var(--ink-mid);
  }

  .delta {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.78rem;
  }

  .delta th,
  .delta td {
    padding: 0.42rem 0.6rem;
    text-align: left;
    border-bottom: 1px solid var(--ink-grid);
  }

  .delta thead th {
    font-family: var(--mono);
    font-size: 0.62rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-low);
    font-weight: 400;
  }

  .delta tbody th {
    font-weight: 400;
    color: var(--ink-mid);
  }

  .delta td {
    font-family: var(--mono);
    color: var(--ink-low);
    width: 30%;
  }

  /* Rows where the two files disagree are the only ones worth reading closely. */
  .delta tr.differs td {
    color: var(--ink-hi);
  }

  .delta td.leads {
    color: var(--ok);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-width: 0;
  }

  .note {
    margin: 0;
    font-size: 0.73rem;
    color: var(--ink-low);
    line-height: 1.6;
  }

  /* Below this the two columns stop being comparable at a glance and stacking reads better
     than shrinking. */
  @media (max-width: 860px) {
    .verdicts,
    .stacked {
      grid-template-columns: 1fr;
    }
  }
</style>
