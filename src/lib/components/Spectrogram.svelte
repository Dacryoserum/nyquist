<script lang="ts">
  import { onMount } from "svelte";
  import type { SpectrogramData } from "$lib/api";
  import { inferno } from "$lib/colormap";

  let {
    data,
    spectralCutoffHz,
    cutoffOverTimeHz,
    currentTimeSeconds = 0,
    onSeek
  }: {
    data: SpectrogramData;
    spectralCutoffHz: number;
    cutoffOverTimeHz?: number[];
    currentTimeSeconds?: number;
    onSeek?: (seconds: number) => void;
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
        const [r, g, b] = inferno(value);
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
      aria-label="Click to seek playback position"
      tabindex={0}
    >
      <canvas bind:this={canvas} aria-label="Spectrogram"></canvas>
      <div class="cutoff-line" style:top="{cutoffLinePercent}%">
        <span class="cutoff-label">raw cutoff ~{fmtFreq(spectralCutoffHz)}Hz</span>
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
    <span class="legend-label">Quiet</span>
    <div class="legend-bar"></div>
    <span class="legend-label">Loud</span>
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
    font-size: 0.7rem;
    color: var(--muted);
    padding: 0.25rem 0;
    text-align: right;
    min-width: 2.5rem;
  }

  .canvas-wrap {
    position: relative;
    flex: 1;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--border);
  }

  .canvas-wrap.seekable {
    cursor: pointer;
  }

  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent, #e2a385);
    pointer-events: none;
    box-shadow: 0 0 4px rgba(0, 0, 0, 0.5);
  }

  canvas {
    display: block;
    width: 100%;
    height: 220px;
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
    font-size: 0.65rem;
    color: rgba(255, 255, 255, 0.75);
    background: rgba(0, 0, 0, 0.35);
    padding: 0.05rem 0.35rem;
    border-radius: 4px;
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
    font-size: 0.7rem;
    color: var(--muted);
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
    height: 8px;
    border-radius: 4px;
    background: linear-gradient(
      90deg,
      rgb(0, 0, 4) 0%,
      rgb(87, 16, 110) 25%,
      rgb(188, 55, 84) 50%,
      rgb(249, 142, 9) 75%,
      rgb(252, 255, 164) 100%
    );
  }

  .legend-label {
    font-size: 0.72rem;
    color: var(--muted);
  }
</style>
