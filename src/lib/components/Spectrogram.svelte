<script lang="ts">
  import { onMount } from "svelte";
  import type { SpectrogramData } from "$lib/api";
  import { COLORMAP_NAMES, colormap, colormapState, gradientCss, setColormap } from "$lib/colormap.svelte";
  import { t } from "$lib/i18n.svelte";

  const T = $derived(t());

  let {
    data,
    spectralCutoffHz,
    cutoffOverTimeHz,
    currentTimeSeconds = 0,
    onSeek,
    showPalette = true
  }: {
    data: SpectrogramData;
    spectralCutoffHz: number;
    cutoffOverTimeHz?: number[];
    currentTimeSeconds?: number;
    onSeek?: (seconds: number) => void;
    /** The palette is a single global setting, so the picker is shown once. Comparison view
     * renders two spectrograms and turns it off on both. */
    showPalette?: boolean;
  } = $props();

  let canvas: HTMLCanvasElement;
  let canvasWrap: HTMLDivElement;

  function handleSeekClick(event: MouseEvent) {
    if (!onSeek || !canvasWrap) return;
    const rect = canvasWrap.getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    onSeek(fraction * data.duration_seconds);
  }

  /** Keyboard equivalent of click-to-seek: a click position doesn't map to a key, so this
   * nudges/jumps instead (arrows: 5s, Home/End: start/end) rather than being a no-op. */
  function handleSeekKeydown(event: KeyboardEvent) {
    if (!onSeek) return;
    const clamp = (t: number) => Math.min(data.duration_seconds, Math.max(0, t));
    switch (event.key) {
      case "ArrowLeft":
        onSeek(clamp(currentTimeSeconds - 5));
        break;
      case "ArrowRight":
        onSeek(clamp(currentTimeSeconds + 5));
        break;
      case "Home":
        onSeek(0);
        break;
      case "End":
        onSeek(data.duration_seconds);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function decodeBase64(b64: string): Uint8Array {
    const binary = atob(b64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
  }

  function draw() {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const { time_bin_count: tCount, frequency_bin_count: fCount } = data;
    canvas.width = tCount;
    canvas.height = fCount;

    const intensity = decodeBase64(data.intensity_base64);
    const image = ctx.createImageData(tCount, fCount);

    for (let t = 0; t < tCount; t++) {
      for (let f = 0; f < fCount; f++) {
        const value = intensity[t * fCount + f] / 255;
        const [r, g, b] = colormap(value);
        // Canvas y=0 is the top; frequency should increase upward, so flip the row.
        const y = fCount - 1 - f;
        const idx = (y * tCount + t) * 4;
        image.data[idx] = r;
        image.data[idx + 1] = g;
        image.data[idx + 2] = b;
        image.data[idx + 3] = 255;
      }
    }

    ctx.putImageData(image, 0, 0);
  }

  onMount(draw);
  $effect(() => {
    // `colormapState.current` is read here on purpose, not used: reading a rune inside an
    // effect is what subscribes the effect to it, so switching palette repaints the canvas.
    colormapState.current;
    if (canvas && data) draw();
  });

  function fmtTime(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = Math.round(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function fmtFreq(hz: number): string {
    return `${(hz / 1000).toFixed(0)}k`;
  }

  const timeLabels = $derived(
    [0, 0.25, 0.5, 0.75, 1].map((f) => fmtTime(data.duration_seconds * f))
  );
  const freqLabels = $derived(
    [1, 0.75, 0.5, 0.25, 0].map((f) => fmtFreq(data.max_frequency_hz * f))
  );
  const cutoffLinePercent = $derived(
    100 - (spectralCutoffHz / data.max_frequency_hz) * 100
  );
  const playheadPercent = $derived(
    data.duration_seconds > 0 ? (currentTimeSeconds / data.duration_seconds) * 100 : 0
  );
  /** SVG polyline points (percent-space, 0-100) tracing the cutoff over time. */
  const cutoffOverTimePoints = $derived.by(() => {
    if (!cutoffOverTimeHz || cutoffOverTimeHz.length === 0) return "";
    const n = cutoffOverTimeHz.length;
    return cutoffOverTimeHz
      .map((hz, i) => {
        const x = (i / (n - 1 || 1)) * 100;
        const y = 100 - (hz / data.max_frequency_hz) * 100;
        return `${x},${y}`;
      })
      .join(" ");
  });
</script>

<div class="spectrogram">
  <div class="plot">
    <div class="freq-axis">
      {#each freqLabels as label (label)}
        <span>{label}</span>
      {/each}
    </div>
    <div
      class="canvas-wrap"
      class:seekable={!!onSeek}
      bind:this={canvasWrap}
      onclick={handleSeekClick}
      onkeydown={handleSeekKeydown}
      role="button"
      aria-label={T.spectrogram.seekAria}
      tabindex={0}
    >
      <canvas bind:this={canvas} aria-label={T.spectrogram.canvasAria}></canvas>
      <div class="cutoff-line" style:top="{cutoffLinePercent}%">
        <span class="cutoff-label">{T.spectrogram.rawCutoff(fmtFreq(spectralCutoffHz))}</span>
      </div>
      {#if cutoffOverTimePoints}
        <svg class="cutoff-trace" viewBox="0 0 100 100" preserveAspectRatio="none">
          <polyline points={cutoffOverTimePoints} />
        </svg>
      {/if}
      {#if onSeek}
        <div class="playhead" style:left="{playheadPercent}%"></div>
      {/if}
    </div>
  </div>
  <div class="time-axis">
    {#each timeLabels as label (label)}
      <span>{label}</span>
    {/each}
  </div>
  <div class="legend">
    <span class="legend-label">{T.spectrogram.quiet}</span>
    <div class="legend-bar" style:background={gradientCss()}></div>
    <span class="legend-label">{T.spectrogram.loud}</span>
    <!-- Palette picker. Four swatches rather than a dropdown: the choice is entirely
         visual, so showing the options themselves is both smaller and more direct than
         naming them. Sits at the end of the legend because that is already the row about
         how the picture is coloured. -->
    {#if showPalette}
      <div class="palette" role="radiogroup" aria-label={T.spectrogram.palette}>
        {#each COLORMAP_NAMES as name (name)}
          <button
            type="button"
            class="swatch"
            class:active={colormapState.current === name}
            style:background={gradientCss(name)}
            role="radio"
            aria-checked={colormapState.current === name}
            aria-label={T.spectrogram.paletteName(name)}
            title={T.spectrogram.paletteName(name)}
            onclick={() => setColormap(name)}
          ></button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .spectrogram {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .plot {
    display: flex;
    gap: 0.5rem;
  }

  .freq-axis {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    font-family: var(--mono);
    font-size: 0.62rem;
    letter-spacing: 0.08em;
    color: var(--ink-low);
    padding: 0.25rem 0;
    text-align: right;
    min-width: 2.5rem;
  }

  .canvas-wrap {
    position: relative;
    flex: 1;
    border-radius: 3px;
    overflow: hidden;
    border: 1px solid var(--ink-hair);
  }

  .canvas-wrap.seekable {
    cursor: pointer;
  }

  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--ink-hi);
    pointer-events: none;
    box-shadow: 0 0 4px rgba(0, 0, 0, 0.5);
  }

  canvas {
    display: block;
    width: 100%;
    /* The spectrogram is the one view that *shows* the verdict rather than asserting it,
       so it gets the vertical space to be read rather than glanced at. */
    height: 300px;
    image-rendering: pixelated;
  }

  .cutoff-line {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px dashed rgba(255, 255, 255, 0.55);
    pointer-events: none;
  }

  .cutoff-label {
    position: absolute;
    right: 0.4rem;
    top: -1.1rem;
    font-family: var(--mono);
    font-size: 0.6rem;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.8);
    background: rgba(0, 0, 0, 0.45);
    padding: 0.08rem 0.35rem;
    border-radius: 2px;
    white-space: nowrap;
  }

  .cutoff-trace {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .cutoff-trace polyline {
    fill: none;
    stroke: rgba(255, 255, 255, 0.85);
    stroke-width: 0.6;
    vector-effect: non-scaling-stroke;
  }

  .time-axis {
    display: flex;
    justify-content: space-between;
    font-family: var(--mono);
    font-size: 0.62rem;
    letter-spacing: 0.08em;
    color: var(--ink-low);
    padding-left: 3rem;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-top: 0.3rem;
    padding-left: 3rem;
  }

  .legend-bar {
    flex: 1;
    max-width: 260px;
    height: 6px;
    border-radius: 2px;
    /* Gradient comes from the selected colormap, applied inline. */
  }

  .palette {
    display: flex;
    gap: 4px;
    margin-left: auto;
  }

  /* Deliberately small and unlabelled: this is a preference, not a reading, and it should
     not compete with the spectrogram it sits under. Each swatch *is* its own preview. */
  .swatch {
    width: 22px;
    height: 10px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 2px;
    cursor: pointer;
    opacity: 0.5;
    transition: opacity 0.15s ease, border-color 0.15s ease;
  }

  .swatch:hover {
    opacity: 0.85;
  }

  .swatch.active {
    opacity: 1;
    border-color: var(--ink-low);
  }

  .swatch:focus-visible {
    outline: 1px solid var(--ink-mid);
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .swatch {
      transition: none;
    }
  }

  .legend-label {
    font-family: var(--mono);
    font-size: 0.6rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-low);
  }
</style>
