/** Colormaps for the spectrogram, and the persisted choice between them.
 *
 * All four are perceptually ordered — lightness rises monotonically with intensity — because
 * the spectrogram is the one surface in this app where a user reads values off a picture,
 * and a palette that dips in lightness invents contours the data does not have. That rules
 * out rainbow/jet-style ramps entirely; the choice offered here is one of temperament, not
 * of whether the picture stays honest.
 */

export type ColormapName = "inferno" | "viridis" | "ice" | "mono";

type Stop = readonly [number, number, number, number];

/** Dark purple (quiet) through red/orange to pale yellow (loud). The default: it is what an
 * audio spectrogram conventionally looks like, and it has the widest lightness range of the
 * four, so faint high-frequency content stays visible. */
const INFERNO: readonly Stop[] = [
  [0.0, 0, 0, 4],
  [0.25, 87, 16, 110],
  [0.5, 188, 55, 84],
  [0.75, 249, 142, 9],
  [1.0, 252, 255, 164]
];

/** Deep blue through green to yellow. Colour-vision-deficiency safe, which inferno's
 * purple-to-red span is not. */
const VIRIDIS: readonly Stop[] = [
  [0.0, 68, 1, 84],
  [0.25, 59, 82, 139],
  [0.5, 33, 145, 140],
  [0.75, 94, 201, 98],
  [1.0, 253, 231, 37]
];

/** Near-black through blue to white. The coolest of the four, and the one that sits most
 * quietly next to the monochrome chrome the rest of the app is built from. */
const ICE: readonly Stop[] = [
  [0.0, 3, 5, 18],
  [0.3, 21, 52, 105],
  [0.6, 46, 130, 176],
  [0.85, 148, 200, 220],
  [1.0, 240, 249, 255]
];

/** Straight luminance. Strips the reading down to the one dimension that carries it, and
 * makes the spectrogram belong to the instrument theme completely — at the cost of the
 * colour separation that makes a faint cutoff edge easy to spot. */
const MONO: readonly Stop[] = [
  [0.0, 6, 6, 7],
  [0.5, 120, 120, 124],
  [1.0, 245, 245, 247]
];

const RAMPS: Record<ColormapName, readonly Stop[]> = {
  inferno: INFERNO,
  viridis: VIRIDIS,
  ice: ICE,
  mono: MONO
};

/** Order shown in the picker. */
export const COLORMAP_NAMES: readonly ColormapName[] = ["inferno", "viridis", "ice", "mono"];

const STORAGE_KEY = "nyquist-colormap";

/** Module-scope rune, same pattern as the language state: the spectrogram and its legend
 * both read it and both re-render when it changes, with no prop threading. */
export const colormapState = $state<{ current: ColormapName }>({ current: "inferno" });

let initialized = false;

/** Called once from the page's `onMount`, for the same reason `initLang` is. */
export function initColormap() {
  if (initialized) return;
  initialized = true;
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && saved in RAMPS) colormapState.current = saved as ColormapName;
  } catch {
    /* localStorage unavailable — keep the default. */
  }
}

export function setColormap(name: ColormapName) {
  colormapState.current = name;
  try {
    localStorage.setItem(STORAGE_KEY, name);
  } catch {
    /* Best-effort persistence only. */
  }
}

function sample(ramp: readonly Stop[], t: number): [number, number, number] {
  const clamped = Math.min(1, Math.max(0, t));
  for (let i = 0; i < ramp.length - 1; i++) {
    const [t0, r0, g0, b0] = ramp[i];
    const [t1, r1, g1, b1] = ramp[i + 1];
    if (clamped >= t0 && clamped <= t1) {
      const f = (clamped - t0) / (t1 - t0);
      return [
        Math.round(r0 + (r1 - r0) * f),
        Math.round(g0 + (g1 - g0) * f),
        Math.round(b0 + (b1 - b0) * f)
      ];
    }
  }
  const [, r, g, b] = ramp[ramp.length - 1];
  return [r, g, b];
}

/** t in [0, 1] -> [r, g, b] in [0, 255], using the currently selected map. */
export function colormap(t: number): [number, number, number] {
  return sample(RAMPS[colormapState.current], t);
}

/** CSS `linear-gradient` stops for a named map — used by the "quiet → loud" legend and by
 * the swatches in the picker. */
export function gradientCss(name: ColormapName = colormapState.current): string {
  const steps = 12;
  const stops: string[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const [r, g, b] = sample(RAMPS[name], t);
    stops.push(`rgb(${r},${g},${b}) ${((t * 100) | 0).toFixed(0)}%`);
  }
  return `linear-gradient(90deg, ${stops.join(", ")})`;
}
