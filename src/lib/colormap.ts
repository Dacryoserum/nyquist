/** Inferno-like colormap: dark purple (quiet) through red/orange to pale yellow (loud). */
const STOPS: readonly [number, number, number, number][] = [
  [0.0, 0, 0, 4],
  [0.25, 87, 16, 110],
  [0.5, 188, 55, 84],
  [0.75, 249, 142, 9],
  [1.0, 252, 255, 164]
];

/** t in [0, 1] -> [r, g, b] in [0, 255]. */
export function inferno(t: number): [number, number, number] {
  const clamped = Math.min(1, Math.max(0, t));
  for (let i = 0; i < STOPS.length - 1; i++) {
    const [t0, r0, g0, b0] = STOPS[i];
    const [t1, r1, g1, b1] = STOPS[i + 1];
    if (clamped >= t0 && clamped <= t1) {
      const f = (clamped - t0) / (t1 - t0);
      return [Math.round(r0 + (r1 - r0) * f), Math.round(g0 + (g1 - g0) * f), Math.round(b0 + (b1 - b0) * f)];
    }
  }
  const [, r, g, b] = STOPS[STOPS.length - 1];
  return [r, g, b];
}

/** CSS linear-gradient stops string, for the "Quiet -> Loud" legend bar. */
export function infernoGradientCss(): string {
  const steps = 12;
  const stops: string[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const [r, g, b] = inferno(t);
    stops.push(`rgb(${r},${g},${b}) ${(t * 100).toFixed(0)}%`);
  }
  return `linear-gradient(90deg, ${stops.join(", ")})`;
}
