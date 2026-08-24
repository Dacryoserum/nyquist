<script lang="ts">
  /** A measurement placed on its own scale, drawn as a row of dots rather than a bar —
   * the same vocabulary the thinking orb is built from, so a reading and the loading
   * indicator belong to one visual system.
   *
   * `reference` marks the conventional target where one exists. It is a tick, not a
   * pass/fail line: most of these are community conventions rather than standards. */
  let {
    value,
    min,
    max,
    tone = "neutral",
    reference,
    referenceLabel,
    dots = 32,
    gradient = false
  }: {
    value: number;
    min: number;
    max: number;
    tone?: "good" | "warn" | "bad" | "neutral";
    reference?: number;
    referenceLabel?: string;
    dots?: number;
    /** Ramp the lit dots from plain ink at the low end of the *scale* to the tone colour at
     * the high end, instead of painting them all one colour.
     *
     * The ramp is positional, not a second severity axis: it says how far along its own
     * scale the reading sits, which is the thing a row of identical dots hides. Reserved for
     * meters where that distance is the reading — a correlation of 0.9 and one of 0.1 are
     * different in kind, not just in length. */
    gradient?: boolean;
  } = $props();

  const fraction = $derived(Math.min(1, Math.max(0, (value - min) / (max - min))));
  const filled = $derived(Math.round(fraction * dots));
  /** Share of the tone colour this dot carries, as a ready-made percentage string.
   *
   * Computed here rather than as `calc(var(--tint) * 100%)` inside the `color-mix()`: a
   * calc() in that position is not reliably parsed across engines, and when it fails the
   * whole declaration is dropped, which silently leaves the dots unpainted rather than
   * merely uncoloured.
   *
   * Eased rather than linear. This project's palette is deliberately desaturated — `--ok` is
   * a sage grey-green, not a signal green — so a straight ramp mixed against white stays
   * invisible over most of its length. The exponent brings the colour in early enough to
   * actually read while leaving both endpoints exactly where they were. */
  function tintAt(index: number): string {
    if (!gradient) return "100%";
    const position = Math.min(1, index / Math.max(1, dots - 1));
    return `${(Math.pow(position, 0.6) * 100).toFixed(1)}%`;
  }

  const referenceIndex = $derived(
    reference === undefined
      ? -1
      : Math.round(Math.min(1, Math.max(0, (reference - min) / (max - min))) * (dots - 1))
  );
</script>

<div
  class="meter"
  class:good={tone === "good"}
  class:warn={tone === "warn"}
  class:bad={tone === "bad"}
  role="meter"
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
>
  {#each Array(dots) as _, i (i)}
    <!-- Lit dots fade in toward the reading's head, echoing the orb's depth shading
         instead of presenting a flat filled bar. -->
    <span
      class="dot"
      class:lit={i < filled}
      class:ref={i === referenceIndex}
      title={i === referenceIndex ? referenceLabel : undefined}
      style:--lit-alpha={i < filled ? String(0.45 + 0.55 * ((i + 1) / Math.max(1, filled))) : "1"}
      style:--tint={tintAt(i)}
    ></span>
  {/each}
</div>

<style>
  .meter {
    --meter-ink: var(--ink-hi);
    display: flex;
    align-items: center;
    gap: 2px;
    height: 10px;
  }
  .meter.good {
    --meter-ink: var(--ok);
  }
  .meter.warn {
    --meter-ink: var(--warn);
  }
  .meter.bad {
    --meter-ink: var(--bad);
  }

  .dot {
    flex: 1;
    height: 3px;
    min-width: 2px;
    border-radius: 50%;
    background: var(--ink-faint);
    transition: background 0.4s ease, opacity 0.4s ease;
  }

  .dot.lit {
    /* `color-mix` rather than two stacked layers: the dot has to stay a single painted
       element for the alpha ramp above to keep working on top of it. */
    background: color-mix(in srgb, var(--meter-ink) var(--tint), var(--ink-hi));
    opacity: var(--lit-alpha);
  }

  /* The conventional target. Taller and unfilled so it reads as a scale marking rather
     than part of the reading, and stays visible once the reading passes it. */
  .dot.ref {
    height: 10px;
    border-radius: 1px;
    background: var(--ink-low);
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .dot {
      transition: none;
    }
  }
</style>
