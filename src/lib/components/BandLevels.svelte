<script lang="ts">
  /** Band levels as a column profile rather than a list of rows.
   *
   * These eight numbers are a *shape* — where the file's energy sits and where it stops —
   * and a stack of eight labelled bars asks the reader to reassemble that shape from a list.
   * Drawn as adjacent columns it is read in one glance, which is the whole point: an encoder
   * wall appears as a cliff between neighbours, a dark master as a slope.
   *
   * It also costs about half the vertical space of the row form, which is what lets this
   * card sit level with the stereo one beside it instead of towering over it.
   *
   * Plain elements rather than canvas: eight bars is nothing for the DOM, and it keeps the
   * values available to assistive tech and to hover without a second code path. */
  import { fmtNumber, t } from "$lib/i18n.svelte";
  import type { BandLevel } from "$lib/api";

  const T = $derived(t());

  let { bands }: { bands: BandLevel[] } = $props();

  /** dB floor the columns are drawn against. Matches the spectrogram's own display floor, so
   * a band reading "empty" here means the same thing as black there. */
  const FLOOR_DB = -90;

  const shortHz = (hz: number) =>
    hz === 0 ? "0" : hz >= 1000 ? `${fmtNumber(hz / 1000, hz % 1000 === 0 ? 0 : 1)}k` : `${hz}`;

  /** Above which fill fraction the value no longer fits above its bar and is drawn inside
   * the bar's head instead. */
  const INSIDE_ABOVE = 0.86;

  const columns = $derived(
    bands.map((band) => {
      const fraction = Math.min(1, Math.max(0, (band.level_db - FLOOR_DB) / -FLOOR_DB));
      return {
        band,
        fraction,
        inside: fraction > INSIDE_ABOVE,
        value: fmtNumber(band.level_db, 0),
        label: shortHz(band.low_hz),
        range: `${shortHz(band.low_hz)}–${band.high_hz !== null ? shortHz(band.high_hz) : T.spectralDetail.toNyquist}`
      };
    })
  );
</script>

<div class="profile">
  <div class="plot">
    {#each columns as column (column.band.low_hz)}
      <div
        class="column"
        title="{column.range} · {fmtNumber(column.band.level_db, 0)} dB"
        style:--fill="{(column.fraction * 100).toFixed(1)}%"
      >
        <!-- The bar sits inside a full-height track so every column shares one baseline and
             one ceiling; without the track the bars would only align at the bottom. -->
        <span class="bar"></span>
        <!-- Sits just above the bar's head, or inside it when the bar is tall enough that
             "above" would be off the top of the chart. Anchored to the bar rather than to a
             fixed row so the number stays tied to the level it describes. -->
        <span class="value" class:inside={column.inside}>{column.value}</span>
      </div>
    {/each}
  </div>
  <div class="axis">
    {#each columns as column (column.band.low_hz)}
      <span>{column.label}</span>
    {/each}
  </div>
  <!-- The chart above carries its numbers in `title` attributes, which a screen reader does
       not reliably announce and a keyboard user cannot reach at all. The same readings as a
       plain list, visually hidden: the picture is for the eye, this is the data. -->
  <ul class="sr-only">
    {#each columns as column (column.band.low_hz)}
      <li>{column.range}: {fmtNumber(column.band.level_db, 0)} dB</li>
    {/each}
  </ul>
</div>

<style>
  /* Available to assistive technology, absent from the layout. */
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

  .profile {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-top: 0.7rem;
  }

  .plot {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 78px;
    border-bottom: 1px solid var(--ink-hair);
  }

  .column {
    position: relative;
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
    min-width: 0;
  }

  /* Both placements anchor to `bottom: var(--fill)` — the bar's head — and differ only in
     which side of it the text sits on. Anchoring the inside case to the top of the *column*
     instead put it above the head of any bar shorter than full height, floating in the gap
     and clipped by the top of the plot. */
  .value {
    position: absolute;
    left: 0;
    right: 0;
    bottom: var(--fill);
    /* Lifted clear of the bar's head. */
    margin-bottom: 2px;
    font-family: var(--mono);
    font-size: 0.56rem;
    line-height: 1;
    text-align: center;
    color: var(--ink-low);
    pointer-events: none;
  }

  .value.inside {
    margin-bottom: 0;
    /* Dropped by its own height plus a hair, so it sits just under the head, inside the bar. */
    transform: translateY(calc(100% + 3px));
    /* Reads against the filled bar rather than the card, so it needs the inverse ink. */
    color: var(--bg);
  }

  .column:hover .value {
    color: var(--ink-hi);
  }

  .column:hover .value.inside {
    color: var(--bg);
  }

  .bar {
    width: 100%;
    height: var(--fill);
    /* A hair of height even at the floor, so an empty band reads as measured-and-empty
       rather than as a gap in the chart. */
    min-height: 1px;
    background: var(--ink-low);
    border-radius: 1px 1px 0 0;
    transition: height 0.35s ease;
  }

  .column:hover .bar {
    background: var(--ink-hi);
  }

  .axis {
    display: flex;
    gap: 2px;
    font-family: var(--mono);
    font-size: 0.58rem;
    letter-spacing: 0.04em;
    color: var(--ink-low);
  }

  .axis span {
    flex: 1;
    min-width: 0;
    text-align: center;
  }

  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }
  }
</style>
