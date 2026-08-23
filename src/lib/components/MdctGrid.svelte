<script lang="ts">
  /** The MDCT grid sweep, drawn as itself.
   *
   * There is one column per candidate frame offset — 1024 of them — and its height is the
   * share of MDCT coefficients that read as zeroed at that offset. The finding does not need
   * a threshold explained to be legible: a lossless file draws a low, uneven ridge because
   * every offset behaves like every other, and an AAC encoder's grid draws a flat floor with
   * a single spike standing on it.
   *
   * Canvas rather than SVG: 1024 columns is more DOM nodes than a chart this small should
   * cost, and none of them need to be individually interactive. */
  import { onMount } from "svelte";
  import { t } from "$lib/i18n.svelte";
  import type { MdctGridAnalysis } from "$lib/api";

  const T = $derived(t());

  let { grid }: { grid: MdctGridAnalysis } = $props();

  let canvas = $state<HTMLCanvasElement | undefined>();
  let theme = $state<"light" | "dark">("dark");

  const profile = $derived.by(() => {
    if (!grid.analyzed || !grid.sweep_profile_base64) return new Uint8Array(0);
    const binary = atob(grid.sweep_profile_base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
  });

  onMount(() => {
    // The chart is drawn with explicit colours rather than CSS variables (canvas cannot read
    // them), so it has to follow the theme toggle itself.
    const read = () =>
      (theme = document.documentElement.getAttribute("data-theme") === "light" ? "light" : "dark");
    read();
    const observer = new MutationObserver(read);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  });

  $effect(() => {
    const bytes = profile;
    const dark = theme === "dark";
    if (!canvas || bytes.length === 0) return;

    const dpr = Math.min(2, (typeof devicePixelRatio !== "undefined" && devicePixelRatio) || 1);
    const cssWidth = canvas.clientWidth || 600;
    const cssHeight = 96;
    canvas.width = Math.round(cssWidth * dpr);
    canvas.height = Math.round(cssHeight * dpr);

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssWidth, cssHeight);

    const ink = dark ? "255, 255, 255" : "17, 17, 18";
    const spike = grid.grid_detected ? (dark ? "#d0918a" : "#a2483f") : `rgba(${ink}, 0.55)`;

    // More offsets than pixels: each column takes the tallest value in its slice, so the one
    // spike that carries the finding can never be averaged away by its flat neighbours.
    const columns = Math.min(bytes.length, Math.floor(cssWidth));
    const barWidth = cssWidth / columns;
    for (let c = 0; c < columns; c++) {
      const from = Math.floor((c * bytes.length) / columns);
      const to = Math.max(from + 1, Math.floor(((c + 1) * bytes.length) / columns));
      let peak = 0;
      let peakIndex = from;
      for (let i = from; i < to; i++) {
        if (bytes[i] > peak) {
          peak = bytes[i];
          peakIndex = i;
        }
      }
      const height = (peak / 255) * (cssHeight - 2);
      const isDetectedOffset = grid.grid_detected && peakIndex === grid.frame_offset;
      ctx.fillStyle = isDetectedOffset ? spike : `rgba(${ink}, 0.28)`;
      // Minimum 1px so a floor of near-zero values still reads as a baseline rather than
      // as empty space.
      ctx.fillRect(c * barWidth, cssHeight - Math.max(1, height), Math.max(1, barWidth - 0.5), Math.max(1, height));
    }
  });
</script>

<div class="mdct">
  {#if !grid.analyzed}
    <p class="unavailable">{T.mdct.notAnalyzed}</p>
  {:else}
    <div class="head">
      <span class="status" class:detected={grid.grid_detected}>
        {grid.grid_detected ? T.mdct.detected : T.mdct.clear}
      </span>
      {#if grid.grid_detected}
        <span class="detail">{T.mdct.offset} {grid.frame_offset}</span>
      {/if}
    </div>

    <!-- No `role="img"`: a canvas already carries an image role implicitly, and Svelte
         rightly flags the explicit one. The figures below state the same result in text, so
         a screen reader gets the finding without the picture. -->
    <canvas bind:this={canvas} aria-label={T.mdct.chartAria}></canvas>
    <div class="axis">
      <span>0</span>
      <span>{T.mdct.axisOffset}</span>
      <span>1024</span>
    </div>

    <dl class="figures">
      <div>
        <dt>{T.mdct.zeroed}</dt>
        <dd>
          {(grid.zero_fraction_at_offset * 100).toFixed(1)}%
          <em>({(grid.zero_fraction_baseline * 100).toFixed(1)}% {T.mdct.baseline})</em>
        </dd>
      </div>
      <div>
        <dt>{T.mdct.strength}</dt>
        <dd>{grid.z_score.toFixed(1)} σ</dd>
      </div>
    </dl>

    <p class="note">{T.mdct.note}</p>
  {/if}
</div>

<style>
  .mdct {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  .status {
    font-size: 0.78rem;
    color: var(--ink-mid);
  }

  .status.detected {
    color: var(--bad);
  }

  .detail,
  .axis {
    font-family: var(--mono);
    font-size: 0.62rem;
    letter-spacing: 0.08em;
    color: var(--ink-low);
  }

  canvas {
    display: block;
    width: 100%;
    height: 96px;
    /* The faint rule the columns stand on, so a flat profile still reads as a measurement
       rather than as a failure to draw anything. */
    border-bottom: 1px solid var(--ink-hair);
  }

  .axis {
    display: flex;
    justify-content: space-between;
  }

  .figures {
    display: flex;
    gap: 2rem;
    margin: 0.4rem 0 0;
  }

  .figures dt {
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ink-low);
  }

  .figures dd {
    margin: 0.15rem 0 0;
    font-family: var(--mono);
    font-size: 0.9rem;
    color: var(--ink-hi);
  }

  .figures em {
    font-style: normal;
    font-size: 0.72rem;
    color: var(--ink-low);
  }

  .note {
    margin: 0.6rem 0 0;
    font-size: 0.73rem;
    color: var(--ink-low);
    line-height: 1.6;
  }

  .unavailable {
    margin: 0;
    font-size: 0.78rem;
    color: var(--ink-low);
  }
</style>
