/**
 * The "composing" thinking-orb: a dotted sphere with an undulating multi-band sash.
 *
 * Ported from `thinking-orbs` v0.1.1 by Jakub Antalik (MIT), https://orbs.jakubantalik.com
 * — specifically its `ribbon` mode, which is what the `composing` state resolves to, at the
 * 64px size preset.
 *
 * ## Why ported rather than installed
 *
 * The package is a React component: it declares `react` and `react-dom` as peer
 * dependencies and its module does a top-level `import from "react"`, so Vite cannot even
 * resolve it without React present. Adding React and ReactDOM to a Svelte app to draw one
 * 64px loading indicator would roughly double the frontend bundle for a Tauri desktop app
 * whose entire `.dmg` is 4.2 MB.
 *
 * The drawing code itself has no framework in it — the library's own architecture separates
 * frame painters (`MODE_DRAWS`, typed `(ctx, size, t, dark, opts) => void`) from the React
 * wrapper that owns the canvas. Only that painter is reproduced here, with the animation
 * maths unchanged and variables given readable names; the canvas, timing and theme plumbing
 * live in `ThinkingOrb.svelte`. Verified frame-for-frame against the upstream painter — see
 * that component's docs.
 *
 * The preset numbers below are not hand-derived: they are the exact output of the library's
 * own `resolvePreset('composing', 64)`, which scales the base options by the size preset's
 * count and dot-size factors.
 */

interface Dot {
  x: number;
  y: number;
  z: number;
  r: number;
  /** 0..1 ink lightness before the dark/light flip. */
  white: number;
  /** Alpha. */
  a: number;
}

/** Fully resolved draw options for `composing` at 64px. */
export interface OrbOptions {
  lanes: number;
  segs: number;
  ghostN: number;
  rBase: number;
  rDepth: number;
  rsPow: number;
  rMin: number;
  spin: number;
  bandMul: number;
  wobMul: number;
  /**
   * Set by the upstream size-preset scaling for every mode, but never read by the ribbon
   * painter — its dot radii come from `rBase`/`rDepth`, which the same scaling already
   * adjusted. Carried here so this preset stays a byte-for-byte copy of
   * `resolvePreset('composing', 64)`; applying it would make the dots wrong.
   */
  rSizeMul: number;
}

/**
 * `resolvePreset('composing', 64)` from thinking-orbs v0.1.1, evaluated once and inlined.
 *
 * `spin: 0` is intentional upstream and is what makes this state read as "composing" rather
 * than "working": it zeroes the yaw and the pitch oscillation, so the sphere holds still and
 * only the sash undulates through it.
 */
export const COMPOSING_64: OrbOptions = {
  lanes: 3,
  segs: 44,
  ghostN: 38,
  rBase: 0.935,
  rDepth: 1.4449999999999998,
  rsPow: 0.6,
  rMin: 0.3,
  rSizeMul: 0.85,
  spin: 0,
  bandMul: 3.9,
  wobMul: 1
};

/** Baked speed multiplier for this preset; `t` is seconds scaled by it. */
export const COMPOSING_SPEED = 2.34;

/** Evenly distributed point on the unit sphere (Fibonacci lattice). */
function fibonacciSpherePoint(index: number, total: number): [number, number, number] {
  const goldenAngle = Math.PI * (3 - Math.sqrt(5));
  const y = 1 - (2 * (index + 0.5)) / total;
  const ringRadius = Math.sqrt(1 - y * y);
  const theta = index * goldenAngle;
  return [ringRadius * Math.cos(theta), y, ringRadius * Math.sin(theta)];
}

/**
 * Builds a yaw-then-pitch rotation followed by an orthographic projection to canvas
 * coordinates. Returns screen x/y plus the rotated z, which the painter keeps for depth
 * shading and back-to-front sorting.
 */
function makeProjector(
  yaw: number,
  pitch: number,
  centerX: number,
  centerY: number,
  scale: number
): (x: number, y: number, z: number) => [number, number, number] {
  const sinPitch = Math.sin(pitch);
  const cosPitch = Math.cos(pitch);
  const sinYaw = Math.sin(yaw);
  const cosYaw = Math.cos(yaw);
  return (x, y, z) => {
    const rotatedX = x * cosYaw + z * sinYaw;
    const depthAxis = -x * sinYaw + z * cosYaw;
    const rotatedY = y * cosPitch - depthAxis * sinPitch;
    const rotatedZ = y * sinPitch + depthAxis * cosPitch;
    return [centerX + rotatedX * scale, centerY - rotatedY * scale, rotatedZ];
  };
}

/** Dot radii scale sub-linearly with canvas size, so 20px and 64px both stay legible. */
function dotScaleFor(size: number, power: number): number {
  return (size / 300) ** power;
}

/** Painters back-to-front so nearer dots overlap farther ones. */
function paintDots(ctx: CanvasRenderingContext2D, dots: Dot[], dark: boolean, minRadius: number) {
  dots.sort((a, b) => a.z - b.z);
  for (const dot of dots) {
    const alpha = dot.a;
    if (alpha < 0.02) continue;
    const lightness = Math.min(1, Math.max(0, dot.white));
    // Dark theme paints light ink, light theme paints dark ink, both on transparency.
    const channel = Math.round((dark ? 1 - lightness : lightness) * 255);
    ctx.fillStyle = `rgba(${channel},${channel},${channel},${alpha})`;
    ctx.beginPath();
    ctx.arc(dot.x, dot.y, Math.max(minRadius, dot.r), 0, Math.PI * 2);
    ctx.fill();
  }
}

/**
 * Draws one frame into `ctx`, which must already be scaled so that `size` is in CSS pixels.
 * `t` is elapsed seconds multiplied by [`COMPOSING_SPEED`]. Does not clear the canvas.
 */
export function drawComposingOrb(
  ctx: CanvasRenderingContext2D,
  size: number,
  t: number,
  dark: boolean,
  opts: OrbOptions = COMPOSING_64
) {
  const centerX = size / 2;
  const centerY = size / 2;
  const radius = (size / 2) * 0.78;
  const spin = opts.spin;
  const project = makeProjector(t * 0.1 * spin, 0.3, centerX, centerY, 1);
  const dotScale = dotScaleFor(size, opts.rsPow);
  const dots: Dot[] = [];

  // Faint static shell the sash travels through, giving the sphere its volume.
  for (let i = 0; i < opts.ghostN; i++) {
    const point = fibonacciSpherePoint(i, opts.ghostN);
    const [x, y, z] = project(point[0] * radius, point[1] * radius, point[2] * radius);
    const depth = (z / radius + 1) / 2;
    dots.push({ x, y, z, r: 0.8 * dotScale, white: 0.78, a: 0.1 + 0.22 * depth });
  }

  // Orientation of the sash's plane. With spin at 0 both of these are constant, so the
  // sash keeps a fixed attitude and all the motion comes from the wobble term below.
  const yaw = t * 0.24 * spin;
  const pitch = 0.55 + 0.3 * Math.sin(t * 0.18) * spin;
  const cosYaw = Math.cos(yaw);
  const sinYaw = Math.sin(yaw);
  const cosPitch = Math.cos(pitch);
  // In-plane basis vectors, plus the plane normal the band offsets are pushed along.
  const upY = -sinYaw * Math.sin(pitch);
  const upZ = cosYaw * Math.sin(pitch);
  const normalX = -sinYaw * cosPitch;
  const normalY = sinYaw * upY - cosYaw * upZ;
  const normalZ = cosYaw * cosPitch;

  const bands = Math.max(1, Math.round(opts.lanes * opts.bandMul));
  const halfSpan = Math.max(1, (bands - 1) / 2);

  for (let band = 0; band < bands; band++) {
    const bandOffset = (band - (bands - 1) / 2) * 0.075;
    // 0 at the sash's centre, 1 at its edges — thins and lightens the outer bands.
    const edge = Math.abs(band - (bands - 1) / 2) / halfSpan;

    for (let seg = 0; seg < opts.segs; seg++) {
      const angle = (seg / opts.segs) * 2 * Math.PI;
      // Two travelling sine components at different rates: the undulation.
      const wobble =
        (0.16 * Math.sin(angle * 3 - t * 1.7 + band * 0.22) + 0.07 * Math.sin(angle * 5 + t * 1.1)) *
        opts.wobMul;
      const offset = bandOffset + wobble;

      const vx = cosYaw * Math.cos(angle) + upY * Math.sin(angle) + normalX * offset;
      const vy = cosPitch * Math.sin(angle) + normalY * offset;
      const vz = sinYaw * Math.cos(angle) + upZ * Math.sin(angle) + normalZ * offset;
      const length = Math.sqrt(vx * vx + vy * vy + vz * vz);

      const [x, y, z] = project((vx / length) * radius, (vy / length) * radius, (vz / length) * radius);
      const depth = (z / radius + 1) / 2;

      dots.push({
        x,
        y,
        z,
        r: (opts.rBase + opts.rDepth * depth) * (1 - 0.25 * edge) * dotScale,
        white: 0.52 - 0.44 * depth + 0.18 * edge,
        a: 0.4 + 0.6 * depth
      });
    }
  }

  paintDots(ctx, dots, dark, opts.rMin);
}
