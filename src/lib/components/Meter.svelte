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
    dots = 32
  }: {
    value: number;
    min: number;
    max: number;
    tone?: "good" | "warn" | "bad" | "neutral";
    reference?: number;
    referenceLabel?: string;
    dots?: number;
  } = $props();

  const fraction = $derived(Math.min(1, Math.max(0, (value - min) / (max - min))));
  const filled = $derived(Math.round(fraction * dots));
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
    background: var(--meter-ink);
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
