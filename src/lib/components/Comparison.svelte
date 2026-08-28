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
  import { evidenceStrength, type AnalysisResult, type Verdict } from "$lib/api";
  import type { IconName } from "$lib/icons";

  const T = $derived(t());

  let {
    a,
    b,
    onClear
  }: { a: AnalysisResult; b: AnalysisResult; onClear: () => void } = $props();

  const fmt = (v: number, d = 1) => fmtNumber(v, d);
  const fmtHz = (hz: number) => (hz >= 1000 ? `${fmtNumber(hz / 1000, 1)} kHz` : `${fmtNumber(hz, 0)} Hz`);
  // `Math.floor`, not `Math.round`: rounding 59.6 s produced "0:60".
  const fmtDuration = (s: number) =>
    `${Math.floor(s / 60)}:${Math.floor(s % 60).toString().padStart(2, "0")}`;
  const na = () => T.loudness.na;

  /** Built per side, because the lossy label names the file's own codec. */
  const verdictMetaFor = (r: AnalysisResult): Record<Verdict, { label: string; icon: IconName; tone: string }> => ({
    probably_authentic: { label: T.verdict.probablyAuthentic.label, icon: "checkCircle", tone: "authentic" },
    probably_transcoded: { label: T.verdict.probablyTranscoded.label, icon: "alertCircle", tone: "transcoded" },
    indeterminate: { label: T.verdict.indeterminate.label, icon: "helpCircle", tone: "indeterminate" },
    declared_lossy: {
      label: T.verdict.declaredLossy.label(r.file_info.codec),
      icon: "waveform",
      tone: "declared"
    }
  });

  /** `better` says which side leads when that is a meaningful thing to say at all. */
  type Row = { label: string; a: string; b: string; better?: "a" | "b" | null };
  type Group = { heading: string; rows: Row[] };

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

  /** Grouped rather than one flat list of eleven: the three groups answer different
   * questions (what the file claims to be, what its spectrum shows, how it was mastered) and
   * a reader is usually only chasing one of them at a time. */
  const groups = $derived.by<Group[]>(() => {
    const [fa, fb] = [a.file_info, b.file_info];
    const [sa, sb] = [a.signal_analysis, b.signal_analysis];
    const [spa, spb] = [a.spectral_analysis, b.spectral_analysis];
    const [sta, stb] = [a.stereo_analysis, b.stereo_analysis];

    const integrity = (v: boolean | null, codec: string) =>
      v === true
        ? T.file2.integrityVerified
        : v === false
          ? T.file2.integrityMismatch
          : codec === "flac"
            ? T.file2.integrityNoChecksum
            : T.file2.integrityUnavailable;

    const declared: Row[] = [
      { label: T.file2.codec, a: fa.codec, b: fb.codec },
      { label: T.file2.sampleRate, a: fmtHz(fa.sample_rate_hz), b: fmtHz(fb.sample_rate_hz) },
      {
        label: T.file2.bitDepth,
        a: fa.bit_depth ? T.file2.bits(fa.bit_depth) : "—",
        b: fb.bit_depth ? T.file2.bits(fb.bit_depth) : "—"
      },
      { label: T.file2.duration, a: fmtDuration(fa.duration_seconds), b: fmtDuration(fb.duration_seconds) },
      {
        label: T.file2.size,
        a: T.file2.megabytes(fmt(fa.file_size_bytes / 1_000_000, 1)),
        b: T.file2.megabytes(fmt(fb.file_size_bytes / 1_000_000, 1))
      },
      {
        label: T.file2.avgBitrate,
        a: fa.bitrate_kbps ? `${fmt(fa.bitrate_kbps, 0)} kbps` : "—",
        b: fb.bitrate_kbps ? `${fmt(fb.bitrate_kbps, 0)} kbps` : "—"
      },
      {
        label: T.file2.integrity,
        a: integrity(fa.integrity_verified, fa.codec),
        b: integrity(fb.integrity_verified, fb.codec)
      }
    ];

    const spectrum: Row[] = [
      {
        label: T.spectrum.bandwidth,
        // `null` means nothing bounded the content, which is not a bandwidth and cannot be
        // ranked against one — `lead` already returns nothing when either side is null.
        a: spa.spectral_cutoff_hz !== null ? fmtHz(spa.spectral_cutoff_hz) : T.spectrum.noLimitMeasured,
        b: spb.spectral_cutoff_hz !== null ? fmtHz(spb.spectral_cutoff_hz) : T.spectrum.noLimitMeasured,
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
        label: T.spectralDetail.stopbandDepth,
        a: spa.stopband_depth_db !== null ? `${fmt(spa.stopband_depth_db, 0)} dB` : T.spectralDetail.noStopband,
        b: spb.stopband_depth_db !== null ? `${fmt(spb.stopband_depth_db, 0)} dB` : T.spectralDetail.noStopband
      },
      {
        label: T.spectralDetail.stability,
        a: `± ${fmtHz(spa.cutoff_stability_hz)}`,
        b: `± ${fmtHz(spb.cutoff_stability_hz)}`
      },
      {
        label: T.mdct.title,
        a: a.mdct_grid.analyzed ? `${fmt(a.mdct_grid.z_score, 1)} σ` : na(),
        b: b.mdct_grid.analyzed ? `${fmt(b.mdct_grid.z_score, 1)} σ` : na(),
        // Lower is better here: a high score is an encoder grid.
        better:
          a.mdct_grid.analyzed && b.mdct_grid.analyzed
            ? lead(a.mdct_grid.z_score, b.mdct_grid.z_score, 10, false)
            : null
      }
    ];

    const loudness: Row[] = [
      {
        label: T.loudness.integratedLoudness,
        a: sa.lufs_integrated !== null ? `${fmt(sa.lufs_integrated)} LUFS` : na(),
        b: sb.lufs_integrated !== null ? `${fmt(sb.lufs_integrated)} LUFS` : na()
      },
      {
        label: T.loudness.loudnessRange,
        a: sa.loudness_range_lu !== null ? `${fmt(sa.loudness_range_lu)} LU` : na(),
        b: sb.loudness_range_lu !== null ? `${fmt(sb.loudness_range_lu)} LU` : na(),
        better: lead(sa.loudness_range_lu, sb.loudness_range_lu, 0.5)
      },
      {
        label: T.loudness.truePeak,
        a: `${fmt(sa.true_peak_dbtp)} dBTP`,
        b: `${fmt(sb.true_peak_dbtp)} dBTP`,
        // Lower true peak means more headroom before a downstream encode clips.
        better: lead(sa.true_peak_dbtp, sb.true_peak_dbtp, 0.3, false)
      },
      {
        label: T.loudness.peakRms,
        a: `${fmt(sa.peak_dbfs)} / ${fmt(sa.rms_dbfs)} dBFS`,
        b: `${fmt(sb.peak_dbfs)} / ${fmt(sb.rms_dbfs)} dBFS`
      }
    ];

    const dynamics: Row[] = [
      {
        label: T.dynamics.dynamicRange,
        a: a.dynamic_range.dr14 !== null ? `DR${a.dynamic_range.dr14}` : na(),
        b: b.dynamic_range.dr14 !== null ? `DR${b.dynamic_range.dr14}` : na(),
        better: lead(a.dynamic_range.dr14, b.dynamic_range.dr14, 0.5)
      },
      {
        label: T.dynamics.clippedRuns,
        a: `${sa.clipped_run_count_total}`,
        b: `${sb.clipped_run_count_total}`,
        better: lead(sa.clipped_run_count_total, sb.clipped_run_count_total, 0, false)
      }
    ];

    const stereoRows: Row[] = [
      {
        label: T.stereo.correlation,
        a: sta ? fmt(sta.correlation, 2) : na(),
        b: stb ? fmt(stb.correlation, 2) : na()
      },
      {
        label: T.stereo.width,
        a: sta ? `${fmt(sta.side_to_mid_db, 1)} dB` : na(),
        b: stb ? `${fmt(stb.side_to_mid_db, 1)} dB` : na()
      },
      // Only worth a row when at least one side is affected; otherwise it is a line of "no"
      // in a table that is already long.
      ...(sta?.dual_mono || stb?.dual_mono
        ? [
            {
              label: T.stereo.dualMono,
              a: sta?.dual_mono ? T.compare.yes : T.compare.no,
              b: stb?.dual_mono ? T.compare.yes : T.compare.no
            }
          ]
        : []),
      ...(sta && stb
        ? sta.per_band.map((band, i) => ({
            label: `${T.stereo.perBand} · ${T.stereo.bandName(band.name)}`,
            a: `${fmt(band.side_to_mid_db, 0)} dB`,
            b: `${fmt(stb.per_band[i]?.side_to_mid_db ?? 0, 0)} dB`
          }))
        : [])
    ];

    return [
      { heading: T.compare.groupDeclared, rows: declared },
      { heading: T.compare.groupSpectrum, rows: spectrum },
      { heading: T.loudness.title, rows: loudness },
      { heading: T.dynamics.title, rows: dynamics },
      { heading: T.stereo.title, rows: stereoRows }
    ];
  });
</script>

<section class="compare">
  <header class="compare-head">
    <h2 class="section-title">{T.compare.title}</h2>
    <button class="ghost icon-only" onclick={onClear} aria-label={T.compare.exit} title={T.compare.exit}>
      <Icon name="close" size={14} />
    </button>
  </header>

  <!-- One player for two files is a deliberate choice (two unsynchronised transports is a
       worse experience than one), but which file it holds was left implicit. -->
  <p class="now-playing">{T.compare.nowPlaying(a.file_info.filename)}</p>

  <div class="verdicts">
    {#each [{ r: a, side: "A" }, { r: b, side: "B" }] as entry (entry.side)}
      {@const vm = verdictMetaFor(entry.r)[entry.r.transcode_assessment.verdict]}
      <article class="verdict-card {vm.tone}">
        <span class="side">{entry.side}</span>
        <h3 class="name" title={entry.r.file_info.filename}>{entry.r.file_info.filename}</h3>
        <div class="verdict-line">
          <Icon name={vm.icon} size={20} />
          <span class="label">{vm.label}</span>
          {#if entry.r.transcode_assessment.confidence_score !== null}
            <span class="confidence">
              {T.verdict.strength[evidenceStrength(entry.r.transcode_assessment.confidence_score)]}
            </span>
          {/if}
        </div>
        <!-- A verdict with no stated evidence is not acceptable anywhere else in this app,
             and this view was the one place it appeared without any. -->
        <ul class="evidence">
          {#each entry.r.transcode_assessment.indicators.slice(0, 3) as indicator (indicator.code)}
            <li>{T.indicator(indicator)}</li>
          {/each}
        </ul>
      </article>
    {/each}
  </div>

  <div class="table-scroll">
  <table class="delta">
    <thead>
      <tr>
        <th scope="col">{T.compare.metric}</th>
        <th scope="col">A</th>
        <th scope="col">B</th>
      </tr>
    </thead>
    {#each groups as group (group.heading)}
      <tbody>
        <tr class="group">
          <th scope="colgroup" colspan="3">{group.heading}</th>
        </tr>
        {#each group.rows as row (row.label)}
          <tr class:differs={row.a !== row.b}>
            <th scope="row">{row.label}</th>
            <!-- The marker and its label, not colour alone: the lead was previously encoded
                 only as a text colour, which no screen reader announces and which a
                 red-green colour deficiency does not separate from the other side. -->
            <td class:leads={row.better === "a"}>
              {row.a}{#if row.better === "a"}<span class="lead-mark" title={T.compare.leads}>▲<span
                    class="sr-only">{T.compare.leads}</span></span>{/if}
            </td>
            <td class:leads={row.better === "b"}>
              {row.b}{#if row.better === "b"}<span class="lead-mark" title={T.compare.leads}>▲<span
                    class="sr-only">{T.compare.leads}</span></span>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    {/each}
  </table>
  </div>
  <p class="note">{T.compare.note}</p>

  <!-- Side by side, because comparing two pictures means seeing both at once. They fall to
       one column only when the window gets far narrower than its 1000px default, at which
       point each half is too cramped to read and stacking is the lesser loss. -->
  <div class="spectra">
    {#each [{ r: a, side: "A" }, { r: b, side: "B" }] as entry (entry.side)}
      <div class="panel">
        <span class="panel-label">{entry.side} — {entry.r.file_info.filename}</span>
        <Spectrogram
          data={entry.r.spectral_analysis.spectrogram}
          spectralCutoffHz={entry.r.spectral_analysis.spectral_cutoff_hz ?? undefined}
          cutoffOverTimeHz={entry.r.spectral_analysis.cutoff_over_time_hz}
          showPalette={false}
        />
      </div>
    {/each}
  </div>

  <!-- These stay side by side: they are small, and the whole reading is "one has a spike,
       the other does not", which is easiest to see with both in one glance. -->
  <div class="grids">
    {#each [{ r: a, side: "A" }, { r: b, side: "B" }] as entry (entry.side)}
      <div class="panel">
        <span class="panel-label">{entry.side} — {T.mdct.title}</span>
        <MdctGrid grid={entry.r.mdct_grid} />
      </div>
    {/each}
  </div>
</section>

<style>
  /* The delta table has three columns of prose and no room to shed one; it scrolls inside
     its own box rather than making the whole page scroll sideways. */
  .table-scroll {
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }

  .now-playing {
    margin: 0 0 0.75rem;
    font-size: 0.72rem;
    letter-spacing: 0.02em;
    color: var(--ink-low);
  }

  .evidence {
    margin: 0.6rem 0 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 0.35rem;
    font-size: 0.72rem;
    line-height: 1.45;
    color: var(--ink-low);
  }

  .lead-mark {
    margin-left: 0.3em;
    font-size: 0.65em;
    vertical-align: 0.15em;
  }

  /* Visible to assistive technology, not on screen: the marker above needs a name, and the
     `title` attribute alone is not reliably announced. */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

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
  .grids {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .spectra {
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
     as it does on the single-file dashboard.

     Each one is declared twice: a plain colour first, then the mixed one. `color-mix` is
     not in WKWebView before macOS 12, and this app declares support down to 10.15 — a
     browser that cannot parse the second declaration keeps the first rather than dropping
     the property and losing the card's border entirely. */
  .verdict-card.transcoded {
    border-color: var(--bad);
    border-color: color-mix(in srgb, var(--bad) 45%, var(--ink-hair));
  }
  .verdict-card.authentic {
    border-color: var(--ok);
    border-color: color-mix(in srgb, var(--ok) 40%, var(--ink-hair));
  }
  .verdict-card.indeterminate {
    border-color: var(--warn);
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

  /* Section headers inside the table. Sit on their own row rather than as separate tables
     so all three groups keep one set of column widths and the A/B values stay aligned down
     the whole thing. */
  .delta tr.group th {
    padding-top: 1.1rem;
    font-family: var(--mono);
    font-size: 0.6rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-low);
    border-bottom-color: var(--ink-hair);
  }

  .delta tbody:first-of-type tr.group th {
    padding-top: 0.42rem;
  }

  /* Rows where the two files disagree are the only ones worth reading closely. */
  .delta tr.differs td {
    color: var(--ink-hi);
  }

  .delta td.leads {
    color: var(--ok);
  }

  /* `min-width: 0` matters: without it a grid item refuses to shrink below its content's
     intrinsic width, and the spectrogram canvas would push the two columns wider than the
     window instead of narrowing with it. */
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

  /* Below this the verdict cards and grid charts stop being comparable at a glance and
     stacking reads better than shrinking. */
  @media (max-width: 860px) {
    .verdicts,
    .grids {
      grid-template-columns: 1fr;
    }
  }

  /* The spectrograms hold out longer: side by side is the whole point of them here, and a
     narrow pair still compares better than a stacked one. This is well under the window's
     1000px default, so it only triggers on a deliberately shrunken window. */
  @media (max-width: 700px) {
    .spectra {
      grid-template-columns: 1fr;
    }
  }
</style>
