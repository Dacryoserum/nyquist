<script lang="ts">
  import { onMount } from "svelte";
  import { COMPOSING_64, COMPOSING_SPEED, drawComposingOrb } from "$lib/thinkingOrb";

  /**
   * The "composing" thinking orb, ported from `thinking-orbs` by Jakub Antalik (MIT) —
   * see `$lib/thinkingOrb.ts` for what was taken and why the package isn't a dependency.
   *
   * This component is the part the upstream React wrapper owned: canvas sizing for the
   * device pixel ratio, the animation clock, theme, and pausing. The drawing itself is
   * upstream's, verified frame-for-frame — 36 224 canvas draw calls identical across eight
   * timestamps and both themes.
   */
  // `state` is renamed on destructure: a local binding called `state` would make every
  // `$state(...)` rune in this file parse as a store subscription on it. The prop keeps its
  // name for callers.
  let {
    state: orbState = "composing",
    size = 64,
    dark = true,
    speed = 1,
    paused = false
  }: {
    /** Only `composing` is ported; the prop exists so adding states later isn't breaking. */
    state?: "composing";
    size?: number;
    /** Dark paints light ink on transparency; light paints dark ink. */
    dark?: boolean;
    /** Multiplier on top of the preset's own baked speed. */
    speed?: number;
    paused?: boolean;
  } = $props();

  let canvas = $state<HTMLCanvasElement | undefined>();
  let reduceMotion = $state(false);

  onMount(() => {
    if (typeof matchMedia === "undefined") return;
    const query = matchMedia("(prefers-reduced-motion: reduce)");
    reduceMotion = query.matches;
    const onChange = (e: MediaQueryListEvent) => (reduceMotion = e.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  });

  $effect(() => {
    if (!canvas) return;

    // Read reactively so the effect re-runs (and the canvas is resized/repainted) when any
    // of these change.
    const currentSize = size;
    const currentDark = dark;
    const currentSpeed = COMPOSING_SPEED * speed;
    const isPaused = paused;
    const isStatic = reduceMotion;

    // Capped at 2: beyond that the extra pixels cost real work on a 3x display and buy
    // nothing visible on dots this small.
    const dpr = Math.min(2, (typeof devicePixelRatio !== "undefined" && devicePixelRatio) || 1);
    canvas.width = Math.round(currentSize * dpr);
    canvas.height = Math.round(currentSize * dpr);

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const paint = (t: number) => {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, currentSize, currentSize);
      drawComposingOrb(ctx, currentSize, t, currentDark, COMPOSING_64);
    };

    // Reduced motion or explicitly paused: one still frame. 0.6 is the timestamp upstream
    // picked for this — the sash is mid-undulation there, so the shape still reads.
    if (isStatic || isPaused) {
      paint(0.6);
      return;
    }

    let frame = 0;
    let running = true;

    const tick = () => {
      paint((performance.now() / 1000) * currentSpeed);
      if (running) frame = requestAnimationFrame(tick);
    };

    // Don't burn frames animating a hidden window — this runs during a multi-second
    // analysis, which is exactly when someone might switch away.
    const onVisibility = () => {
      if (document.visibilityState === "hidden") {
        running = false;
        cancelAnimationFrame(frame);
      } else if (!running) {
        running = true;
        frame = requestAnimationFrame(tick);
      }
    };

    document.addEventListener("visibilitychange", onVisibility);
    frame = requestAnimationFrame(tick);

    return () => {
      running = false;
      cancelAnimationFrame(frame);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  });
</script>

<!-- Hidden from assistive tech on purpose: this is a decorative indicator, and the status
     text beside it already sits in an aria-live region. Announcing it too would be noise. -->
<canvas
  bind:this={canvas}
  style:width="{size}px"
  style:height="{size}px"
  aria-hidden="true"
  data-state={orbState}
></canvas>

<style>
  canvas {
    display: block;
    /* The orb is drawn on transparency, so it inherits whatever sits behind it. */
    background: none;
  }
</style>
