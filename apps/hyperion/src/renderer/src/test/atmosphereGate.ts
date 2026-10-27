/**
 * R05.T12.e's quadrature gate on the per-frame marches (plan R05, `decision-r05-high-atmosphere.md`
 * and its addenda A and B), shared by `view/atmosphere/marchSteps.test.ts`, which holds R05's
 * kernels to it, and `view/atmosphere/tablesCpu.test.ts`, which holds R08.T6.a's CPU twin to it
 * with the multiple-scattering term included.
 *
 * @remarks
 * It holds the gate's 1,641 rays on Earth's medium, its metric and its schemes, the reference's
 * convergence rule, and R05's own `f64` twin of the two kernels' quadrature: single scattering per
 * unit sun illuminance with the sun's transmittance from a fine grid, and a Lambertian ground. Moved
 * here unchanged from `marchSteps.test.ts` (R08.T6.a), so that the twin is held to the same rays
 * and R05's pixels.
 *
 * Since R08.T8 (`decision-r08-t8-leans.md` §4 and §5.4) the schemes follow the kernels' rule, the
 * ray march's two-sided sky segments under the sun-aware warp ({@link Placement}'s `kernel`: even
 * where the Sun is ahead or across the ray, R05's one-sided placement from the segment's start where
 * it is behind), while the references keep R05.T12.e's placed steps; the march's rays gain the
 * night-side and terminator limb (`night limb`, {@link marchRays}), and the sky's the camera's last
 * light (`last light`, {@link skyRays}), which the ray march draws from the band's cameras and the
 * gate holds as the ray march's ({@link kernelOf}).
 *
 * Since R08.T8.e (`decision-r08-t8-leans.md` §8, lever 3) a scheme carries the ray march's count by
 * the bare ground's and the thin sky's optical depth ({@link Scheme}'s `marchDepthSteps`, high's
 * `TableSizes` rule),
 * and its ray-march families' pixels under the kernel's rule take the count the kernel takes
 * ({@link gateMarchSamples}); the sky view's families keep the sky view's own count, and the
 * references and even steps take theirs throughout.
 */

import { add, cross, dot, normalise, scale, type Vec3 } from "../geometry/vec3";
import { R05_EARTH_REFERENCE, RAYLEIGH_SCALE_HEIGHT_M } from "../view/atmosphere/earth";
import { TABLE_SIZES } from "../view/atmosphere/hillaire";
import {
  drawnSurfaceKind,
  grazingTwilight,
  LOW_TWIN_WORST,
  MARCH_TOLERANCE,
  type MarchDepthSteps,
  marchEvenStep,
  marchSamplesOf,
  type MarchSegmentKind,
  marchSplit,
  marchStep,
  type MarchStep,
  skyWarpOf,
  TWILIGHT_TOLERANCE,
} from "../view/atmosphere/marchSteps";
import { densityAt, extinction, type MediumTerm } from "../view/atmosphere/medium";
import {
  intersectsGround,
  opticalDepth,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
} from "../view/atmosphere/opticalDepth";
import {
  sphereBareGroundDepthG,
  sphereSkyDepthG,
  type TwinSources,
} from "../view/atmosphere/tablesCpu";

/** WGS 84's equatorial radius a, m (NIMA TR8350.2): the twin's sphere. */
export const BOTTOM_M = 6_378_137;
const TOP_M = BOTTOM_M + R05_EARTH_REFERENCE.topHeightM;
const SHELL = { bottomRadiusM: BOTTOM_M, topRadiusM: TOP_M };

/** The spike's terrain albedo (Design note 17), the twin's Lambertian ground. */
export const GROUND_ALBEDO = 0.15;

/**
 * The sun's optical depth: `opticalDepth.ts`'s integrator at 64 Simpson intervals, on a 512 × 128
 * grid of the transmittance table's (u, v) (Bruneton 2017's mapping), read bilinearly. Every scheme
 * and the reference read the same function, so the gate's differences are the view ray's
 * quadrature alone. Integrating each sample's sun ray afresh (lane C, 2026-10-06, over these rays)
 * passes every test here as well: it moves no ray's e at high's counts by more than 0.23 percentage
 * points and at low's by more than 3.9, both twilight rays whose sun is on the horizon, where the
 * grid's bilinear read is coarsest; and it takes seven times as long.
 */
const SUN_STEPS = 64;
const SUN_GRID_U = 512;
const SUN_GRID_V = 128;

/**
 * The reference's placed steps at first, confirmed against twice as many to 0.05%. A ray whose
 * reference has not converged at 2,048 takes 4,096, confirmed against 8,192, and one that has not
 * converged at 4,096 takes 8,192, confirmed against 16,384 (decision-test-coverage R3(b), which
 * lowered the first from 4,096): a twilight ray that crosses the ground's hard shadow edge
 * converges only as 1 ÷ n there.
 */
export const REFERENCE_STEPS = 2_048;
/** The most placed steps a reference takes, confirmed against twice as many. */
const MAX_REFERENCE_STEPS = 8_192;
/** The reference's convergence bound: its e against twice its steps under 0.05%. */
export const CONVERGED = 5e-4;

/** The floor of e's denominator, as a fraction of the kernel's brightest reference pixel. */
const FLOOR = 1e-3;

/** A radiance or a transmittance per channel, mutable for the marches' sums. */
export type Rgb3 = [number, number, number];
const CHANNELS = [0, 1, 2] as const;
const along = (o: Vec3, d: Vec3, tM: number): Vec3 => add(o, scale(d, tM));
const rad = (deg: number): number => (deg * Math.PI) / 180;

/**
 * The phase a term scatters with, as `phaseOf` in `source.wgsl`.
 *
 * @throws Error for a `tabulated` phase, which Earth's medium does not have.
 */
function phaseOf(term: MediumTerm, cosTheta: number): number {
  let phase: number;
  switch (term.phase.kind) {
    case "none":
      phase = 0;
      break;
    case "rayleigh":
      phase = (3 / (16 * Math.PI)) * (1 + cosTheta * cosTheta);
      break;
    case "cornette-shanks": {
      const g = term.phase.asymmetry;
      const k = ((3 / (8 * Math.PI)) * (1 - g * g)) / (2 + g * g);
      phase = (k * (1 + cosTheta * cosTheta)) / Math.max(1 + g * g - 2 * g * cosTheta, 1e-6) ** 1.5;
      break;
    }
    case "tabulated":
      throw new Error(`R05's gate twin reads no tabulated phase (term ${term.name})`);
  }
  return phase;
}

const TERMS = R05_EARTH_REFERENCE.terms.map((term) => ({ term, sigma: extinction(term) }));

/** The sun's optical depth per channel at the grid's nodes, (u, v) = (i, j) ÷ (size − 1). */
function sunOpticalDepthGrid(): Float64Array {
  const grid = new Float64Array(SUN_GRID_U * SUN_GRID_V * 3);
  for (let j = 0; j < SUN_GRID_V; j += 1) {
    for (let i = 0; i < SUN_GRID_U; i += 1) {
      const { rM, mu } = transmittanceUvToRMu(SHELL, i / (SUN_GRID_U - 1), j / (SUN_GRID_V - 1));
      grid.set(
        opticalDepth(R05_EARTH_REFERENCE, BOTTOM_M, rM, mu, SUN_STEPS),
        (j * SUN_GRID_U + i) * 3,
      );
    }
  }
  return grid;
}

/** The grid, computed once when the gate first reads it. */
let sunGrid: Float64Array | undefined;

/**
 * The sun's transmittance from radius `rM` at zenith cosine `muSun` into `out`, or none in the
 * ground's shadow: R05's twin's, from the 512 × 128 grid.
 */
export function sunTransmittance(rM: number, muSun: number, out: Rgb3): void {
  if (intersectsGround(SHELL, rM, muSun)) {
    out.fill(0);
    return;
  }
  sunGrid ??= sunOpticalDepthGrid();
  const grid = sunGrid;
  const [u, v] = transmittanceRMuToUv(SHELL, rM, muSun);
  const x = Math.min(Math.max(u, 0), 1) * (SUN_GRID_U - 1);
  const y = Math.min(Math.max(v, 0), 1) * (SUN_GRID_V - 1);
  const i = Math.min(Math.floor(x), SUN_GRID_U - 2);
  const j = Math.min(Math.floor(y), SUN_GRID_V - 2);
  const fx = x - i;
  const fy = y - j;
  const a = (j * SUN_GRID_U + i) * 3;
  const b = a + SUN_GRID_U * 3;
  for (const c of CHANNELS) {
    const tau =
      (1 - fy) * ((1 - fx) * (grid[a + c] ?? Number.NaN) + fx * (grid[a + 3 + c] ?? Number.NaN)) +
      fy * ((1 - fx) * (grid[b + c] ?? Number.NaN) + fx * (grid[b + 3 + c] ?? Number.NaN));
    out[c] = Math.exp(-tau);
  }
}

/** `common.wgsl`'s stepFactor in emulated f32, each operation rounded, with a correctly rounded exp. */
export function stepFactorF32(x: number): number {
  const f = Math.fround;
  const v = f(x);
  if (v < f(0.01)) {
    return f(1 - f(v * f(0.5 - f(v / 6))));
  }
  return f(f(1 - f(Math.exp(-v))) / v);
}

/** The form it replaces, (1 − exp(−x)) ÷ x as (S − S·T) ÷ σ_t computes it, in emulated f32. */
export function oldFactorF32(x: number): number {
  const f = Math.fround;
  const v = f(x);
  return f(f(1 - f(Math.exp(-v))) / v);
}

/**
 * How a scheme places its steps: R05.T12.e's placement (`placed`, which every reference keeps); the
 * kernels' own rule (`kernel`: R05.T12.e's, but the ray march's two-sided sky segments under the
 * sun-aware warp since R08.T8, `decision-r08-t8-leans.md` §4.2 and §5.4); or the even steps the
 * kernels had before R05.T12.e (`even`).
 */
export type Placement = "placed" | "kernel" | "even";

/**
 * The steps of a gate ray's march over its segment under a placement. Under `kernel` a segment the
 * ray march marches as its sky, nothing drawn along it ({@link skySegmentOf}), takes the sun-aware
 * warp by the ray's sun (`marchEvenStep`, `skyWarpOf`) where it is two-sided, and its sides about
 * the lowest point where it is one-sided.
 */
export function stepsFor(placement: Placement, ray: GateRay, n: number): MarchStep[] {
  const { o, d, tStartM, tEndM } = ray;
  let steps: MarchStep[];
  switch (placement) {
    case "even": {
      const dtM = (tEndM - tStartM) / n;
      steps = Array.from({ length: n }, (_, i) => ({ tM: tStartM + (i + 0.5) * dtM, dtM }));
      break;
    }
    case "placed": {
      // A segment starts at the camera exactly when the camera is inside the atmosphere.
      const split = marchSplit(tStartM, tEndM, -dot(o, d), n, tStartM <= 0);
      steps = Array.from({ length: n }, (_, i) => marchStep(split, i));
      break;
    }
    case "kernel": {
      // `rayMarch.wgsl`: the camera side toward the camera only for a drawn surface, and every
      // two-sided sky segment under the warp by the march's first source, the ray's sun.
      const sky = skySegmentOf(ray);
      const split = marchSplit(tStartM, tEndM, -dot(o, d), n, tStartM <= 0 && !sky);
      const warped = sky && split.beforeM > 0 && split.afterM > 0;
      const warp = skyWarpOf(d, ray.s);
      steps = Array.from({ length: n }, (_, i) =>
        warped ? marchEvenStep(split, i, warp) : marchStep(split, i),
      );
      break;
    }
  }
  return steps;
}

/**
 * The kernels' quadrature in `f64`: single scattering per unit sun illuminance, each step adding
 * throughput · S · Δt · g(σ_t Δt) at its midpoint's medium, g(x) = (1 − e^(−x)) ÷ x, and the
 * ray's transmittance.
 */
function march(o: Vec3, d: Vec3, s: Vec3, steps: readonly MarchStep[]): { l: Rgb3; t: Rgb3 } {
  const cosTheta = dot(d, s);
  const phases = TERMS.map(({ term }) => phaseOf(term, cosTheta));
  const l: Rgb3 = [0, 0, 0];
  const t: Rgb3 = [1, 1, 1];
  const phased: Rgb3 = [0, 0, 0];
  const sigma: Rgb3 = [0, 0, 0];
  const toSun: Rgb3 = [0, 0, 0];
  for (const { tM, dtM } of steps) {
    const px = o.x + tM * d.x;
    const py = o.y + tM * d.y;
    const pz = o.z + tM * d.z;
    const r = Math.sqrt(px * px + py * py + pz * pz);
    const h = r - BOTTOM_M;
    phased.fill(0);
    sigma.fill(0);
    for (let k = 0; k < TERMS.length; k += 1) {
      const entry = TERMS[k];
      if (entry === undefined) {
        continue;
      }
      const density = densityAt(entry.term.density, h);
      const phase = phases[k] ?? 0;
      for (const c of CHANNELS) {
        phased[c] += entry.term.scattering[c] * density * phase;
        sigma[c] += entry.sigma[c] * density;
      }
    }
    sunTransmittance(r, (px * s.x + py * s.y + pz * s.z) / r, toSun);
    for (const c of CHANNELS) {
      const x = sigma[c] * dtM;
      const factor = x > 0 ? -Math.expm1(-x) / x : 1;
      l[c] += t[c] * toSun[c] * phased[c] * dtM * factor;
      t[c] *= Math.exp(-x);
    }
  }
  return { l, t };
}

/** The near and far distances along a ray to a sphere about the centre, or null if missed. */
export function sphereHits(o: Vec3, d: Vec3, radiusM: number): readonly [number, number] | null {
  const b = dot(o, d);
  const disc = b * b - (dot(o, o) - radiusM * radiusM);
  if (disc < 0) {
    return null;
  }
  const root = Math.sqrt(disc);
  return [-b - root, -b + root];
}

type MarchFamily = "disc" | "limb" | "night limb" | "inside" | "near-level" | "drawn";
type SkyFamily = "sky" | "band" | "past" | "high limb" | "high band" | "last light";
/**
 * A family of the gate's rays: the march's five and, since R08.T11.b, its drawn surfaces
 * ({@link drawnSurfaceRays}), then the sky's six.
 */
export type Family = MarchFamily | SkyFamily;
const MARCH_FAMILIES: readonly Family[] = [
  "disc",
  "limb",
  "night limb",
  "inside",
  "near-level",
  "drawn",
];
/**
 * The families whose rays are the ray march's: the march's, and since R08.T8 the sky's last light
 * (`decision-r08-t8-leans.md` §5.5), which the ray march draws from the band's cameras and the gate
 * holds it to wherever it holds the march, marched as a sky pixel at the march's count.
 */
const RAY_MARCH_FAMILIES: ReadonlySet<Family> = new Set([...MARCH_FAMILIES, "last light"]);
/** The families in the order the gate counts them. */
export const FAMILIES: readonly Family[] = [
  ...MARCH_FAMILIES,
  "sky",
  "band",
  "past",
  "high limb",
  "high band",
  "last light",
];

/** One ray of the gate, the segment its kernel marches and whether it ends on the ground. */
export interface GateRay {
  readonly family: Family;
  readonly name: string;
  readonly o: Vec3;
  readonly d: Vec3;
  readonly s: Vec3;
  readonly tStartM: number;
  readonly tEndM: number;
  /**
   * Whether it ends on a lit surface of the gate's albedo: the ground, or since R08.T11.b a surface
   * drawn in the air ({@link GateRay.drawnHeightM}).
   */
  readonly ground: boolean;
  /**
   * Where a ray ends on a surface the scene draws, its depth in the scene's depth (R08.T11.b), the
   * surface's height above the sphere, m, 0 on the ground; `null` where nothing is drawn, the bare
   * ground or the sky.
   */
  readonly drawnHeightM: number | null;
  /** The point the sun's zenith angle is given at, and that angle. */
  readonly sunPoint: Vec3;
  readonly sunZenithDeg: number;
}

/** The kernel a family's rays are the twin of. */
export const kernelOf = (ray: GateRay): "rayMarch" | "skyView" =>
  RAY_MARCH_FAMILIES.has(ray.family) ? "rayMarch" : "skyView";

/**
 * Whether the ray march marches a gate ray as its sky, nothing drawn along it (R08.T8;
 * `rayMarch.wgsl`'s `depth == 0.0`): a ray march family's ray that meets no ground, the limb's from
 * orbit and the last light's from inside the air. A disc ray ends on the bare ground before its
 * lowest point, one-sided, and the inside and near-level rays end on drawn terrain.
 */
export const skySegmentOf = (ray: GateRay): boolean => kernelOf(ray) === "rayMarch" && !ray.ground;

/** The distance along a gate ray to the sphere's first hit ahead, m, or `null` where it meets none. */
function groundHitOf(ray: GateRay): number | null {
  const ground = sphereHits(ray.o, ray.d, BOTTOM_M);
  return ground !== null && ground[0] > 0 ? ground[0] : null;
}

/**
 * What a gate ray's segment ends on, as the ray march tells it under `rule` (plan R08, R08.T8.e;
 * `marchSteps.ts`'s `MarchSegmentKind`): a drawn surface, the inside, near-level and drawn
 * families' (`surface`), counted since R08.T11.b as the bare ground where it stands on the datum,
 * at or beyond its hit or within the rule's band above it (`marchSteps.ts`'s `drawnSurfaceKind`);
 * the bare ground, nothing drawn along the ray (`ground`: the disc from orbit, and the sky families'
 * rays that meet the ground and end there); otherwise the sky.
 */
export function marchKindOf(ray: GateRay, rule: MarchDepthSteps | null): MarchSegmentKind {
  const hit = groundHitOf(ray);
  if (ray.drawnHeightM !== null) {
    return drawnSurfaceKind(hit === null ? null : ray.drawnHeightM, rule);
  }
  return hit !== null && hit <= ray.tEndM ? "ground" : "sky";
}

/**
 * A gate ray's count as the ray march takes it under `rule` (plan R08, R08.T8.e;
 * `marchSteps.ts`'s `marchSamplesOf`): `n` with no rule, on a drawn surface, or on the sky under a
 * rule with no thin sky; otherwise scaled by the segment's green optical depth on the gate's sphere
 * through `transmittanceAt` (light slot 0, the twin's own transmittance read), the bare ground's from
 * its ground point along −d, the sky's from its lowest point along each side (`tablesCpu.ts`'s
 * `sphereBareGroundDepthG` and `sphereSkyDepthG`), on a medium with `edges` march edges.
 */
export function gateMarchSamples(
  ray: GateRay,
  n: number,
  rule: MarchDepthSteps | null,
  edges: number,
  transmittanceAt: TwinSources["sunTransmittance"],
): number {
  const kind = marchKindOf(ray, rule);
  if (rule === null || kind === "surface" || (kind === "sky" && rule.thinSky === null)) {
    return n;
  }
  const segment = {
    originM: ray.o,
    direction: ray.d,
    tStartM: ray.tStartM,
    tEndM: ray.tEndM,
    fromCamera: ray.tStartM <= 0,
  };
  const sphere = { kind: "sphere", bottomRadiusM: BOTTOM_M } as const;
  // The bare ground's depth from the sphere's hit: its end, or a drawn surface's beyond it
  // (R08.T11.b).
  const depthG =
    kind === "ground"
      ? sphereBareGroundDepthG(transmittanceAt, sphere, segment, groundHitOf(ray) ?? ray.tEndM)
      : sphereSkyDepthG(transmittanceAt, sphere, segment);
  return marchSamplesOf(kind, depthG, n, edges, rule);
}

/**
 * A sun at a zenith angle and an azimuth from the ray's own, at a point `q`, azimuth 0 ahead along
 * the ray; a vertical ray takes its azimuths from any horizontal axis.
 */
export function sunAt(q: Vec3, d: Vec3, zenithDeg: number, azimuthDeg: number): Vec3 {
  const n = normalise(q);
  const horizontalOf = (v: Vec3): Vec3 => along(v, n, -dot(v, n));
  const towards = horizontalOf(d);
  const f = normalise(dot(towards, towards) > 1e-12 ? towards : horizontalOf({ x: 1, y: 0, z: 0 }));
  const y = cross(n, f);
  const z = rad(zenithDeg);
  const a = rad(azimuthDeg);
  const horizontal = add(scale(f, Math.cos(a)), scale(y, Math.sin(a)));
  return normalise(add(scale(n, Math.cos(z)), scale(horizontal, Math.sin(z))));
}

/** The ruling's suns for the march's new family: 30/80/90/95° from the zenith, 0/90/180° in azimuth. */
const SUNS_FULL = [30, 80, 90, 95].flatMap((zen) => [0, 90, 180].map((az) => [zen, az] as const));

/** A march ray from `o` that meets the ground `distanceM` away, or null if it cannot. */
function towardTerrain(o: Vec3, distanceM: number): { d: Vec3; tM: number } | null {
  const rc = Math.hypot(o.x, o.y, o.z);
  const cosG = (rc * rc + distanceM * distanceM - BOTTOM_M * BOTTOM_M) / (2 * rc * distanceM);
  if (Math.abs(cosG) > 1) {
    return null;
  }
  const g = Math.acos(cosG);
  const d = { x: Math.sin(g), y: 0, z: -Math.cos(g) };
  const ground = sphereHits(o, d, BOTTOM_M);
  // The terrain must be the ray's first meeting with the ground, before the visible horizon.
  return ground !== null && Math.abs(ground[0] - distanceM) < 1 ? { d, tM: ground[0] } : null;
}

/** The night limb's cameras, m above the ground: R05.T12.e's orbit, and 1 km over the top. */
const NIGHT_LIMB_CAMERAS_M = [400_000, R05_EARTH_REFERENCE.topHeightM + 1_000] as const;

/** The night limb's suns, degrees from the zenith at the tangent point: the terminator and beyond. */
const NIGHT_LIMB_SUN_ZENITHS_DEG = [90, 95, 100, 105] as const;

/**
 * The march's rays: from the decision's model, the disc from 400 km (ground zenith 0/60/80/87° ×
 * sun 30/80/90/95° at the ground × azimuth 0/90/180°), the limb at R08's tangent heights 0.1,
 * 0.3, 1, 3 and 10 Rayleigh scale heights (sun 30/90° at the tangent point × three azimuths), and
 * terrain 40–300 km away from 20 and 60 km (sun 30/85° × azimuth 0/180°), beyond the
 * aerial-perspective volume's 32 km; from addendum A, the near-level march from 0.5–5 km to terrain
 * at 40, 80 and 150 km before the visible horizon (the ruling's suns).
 *
 * Since R08.T8 (`decision-r08-t8-leans.md` §4.3), the night limb: the night-side and terminator
 * limb from 400 km and from 1 km over the top, at the limb's tangent heights, the sun 90, 95, 100
 * and 105° from the zenith at the tangent point × azimuth 0/180°, less the limb's own rays (400 km,
 * 90°), 70 rays, every one a twilight ray. R05.T12.e's suns at 30 and 90° never sampled it, where
 * the placement about the lowest point from outside failed (12.19% at high's 36 steps).
 */
export function marchRays(): GateRay[] {
  const rays: GateRay[] = [];
  const orbit = { x: 0, y: 0, z: BOTTOM_M + 400_000 };
  const fromOrbit = (
    family: "disc" | "limb" | "night limb",
    name: string,
    alphaRad: number,
    sunZenithDeg: number,
    sunAzimuthDeg: number,
    camera: Vec3 = orbit,
  ): void => {
    const d = { x: Math.sin(alphaRad), y: 0, z: -Math.cos(alphaRad) };
    const shell = sphereHits(camera, d, TOP_M);
    if (shell === null) {
      throw new Error(`${name} misses the atmosphere`);
    }
    const ground = sphereHits(camera, d, BOTTOM_M);
    const hits = ground !== null && ground[0] > 0;
    const q = along(camera, d, hits ? ground[0] : -dot(camera, d));
    rays.push({
      family,
      name,
      o: camera,
      d,
      s: sunAt(q, d, sunZenithDeg, sunAzimuthDeg),
      tStartM: shell[0],
      tEndM: hits ? ground[0] : shell[1],
      ground: hits,
      drawnHeightM: null,
      sunPoint: q,
      sunZenithDeg,
    });
  };
  for (const groundZenithDeg of [0, 60, 80, 87]) {
    for (const zen of [30, 80, 90, 95]) {
      for (const az of groundZenithDeg === 0 ? [0] : [0, 90, 180]) {
        const alpha = Math.asin((BOTTOM_M / orbit.z) * Math.sin(rad(groundZenithDeg)));
        fromOrbit(
          "disc",
          `disc, ground zenith ${groundZenithDeg}°, sun ${zen}°/${az}°`,
          alpha,
          zen,
          az,
        );
      }
    }
  }
  for (const tangentScaleHeights of [0.1, 0.3, 1, 3, 10]) {
    for (const zen of [30, 90]) {
      for (const az of [0, 90, 180]) {
        const tangentM = tangentScaleHeights * RAYLEIGH_SCALE_HEIGHT_M;
        const alpha = Math.asin((BOTTOM_M + tangentM) / orbit.z);
        fromOrbit("limb", `limb, ${tangentScaleHeights} H, sun ${zen}°/${az}°`, alpha, zen, az);
      }
    }
  }
  for (const cameraM of NIGHT_LIMB_CAMERAS_M) {
    const camera = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    for (const tangentScaleHeights of [0.1, 0.3, 1, 3, 10]) {
      for (const zen of NIGHT_LIMB_SUN_ZENITHS_DEG) {
        for (const az of [0, 180]) {
          // The limb's own rays, which it does not repeat.
          if (cameraM === 400_000 && zen === 90) {
            continue;
          }
          const alpha = Math.asin(
            (BOTTOM_M + tangentScaleHeights * RAYLEIGH_SCALE_HEIGHT_M) / camera.z,
          );
          fromOrbit(
            "night limb",
            `night limb, ${cameraM / 1_000} km, ${tangentScaleHeights} H, sun ${zen}°/${az}°`,
            alpha,
            zen,
            az,
            camera,
          );
        }
      }
    }
  }
  const toTerrain = (
    family: "inside" | "near-level",
    cameraM: number,
    distances: readonly number[],
    suns: ReadonlyArray<readonly [number, number]>,
  ): void => {
    const o = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    for (const distanceM of distances) {
      const hit = towardTerrain(o, distanceM);
      if (hit === null) {
        continue;
      }
      const q = along(o, hit.d, hit.tM);
      for (const [zen, az] of suns) {
        rays.push({
          family,
          name: `${family}, ${cameraM / 1_000} km, terrain ${distanceM / 1_000} km, sun ${zen}°/${az}°`,
          o,
          d: hit.d,
          s: sunAt(q, hit.d, zen, az),
          tStartM: 0,
          tEndM: hit.tM,
          ground: true,
          drawnHeightM: 0,
          sunPoint: q,
          sunZenithDeg: zen,
        });
      }
    }
  };
  for (const cameraM of INSIDE_CAMERAS_M) {
    toTerrain("inside", cameraM, INSIDE_TERRAIN_M, INSIDE_SUNS);
  }
  for (const cameraM of [500, 1_000, 2_000, 5_000]) {
    toTerrain("near-level", cameraM, [40_000, 80_000, 150_000], SUNS_FULL);
  }
  return rays;
}

/** The inside family's cameras, m above the ground: from inside the air, under the march band. */
const INSIDE_CAMERAS_M = [20_000, 60_000] as const;

/** The inside family's terrain, m from the camera: every one beyond the aerial perspective's reach. */
const INSIDE_TERRAIN_M = [40_000, 100_000, 300_000] as const;

/** The inside family's suns: 30/85° from the zenith at the terrain, 0/180° in azimuth. */
const INSIDE_SUNS = [30, 85].flatMap((zen) => [0, 180].map((az) => [zen, az] as const));

/** The drawn surfaces' cameras from orbit, m from the centre: R05.T12.e's orbit and 10⁷ m. */
const DRAWN_ORBIT_CAMERAS_M = [BOTTOM_M + 400_000, 1e7] as const;

/**
 * The drawn surfaces' ground zenith angles from orbit, degrees: the disc family's face, and 89°,
 * the limb's last ground ray, a pixel inside the limb from 400 km at 1080p across 60°.
 */
const DRAWN_GROUND_ZENITHS_DEG = [0, 60, 80, 87, 89] as const;

/**
 * The band's edge the drawn surfaces stand at, m over the ground: high's band
 * (`TableSizes`' `marchDepthSteps.drawnBandM`, R08.T11.b).
 */
function drawnBandEdgeM(): number {
  const rule = TABLE_SIZES.high.marchDepthSteps;
  if (rule === null) {
    throw new Error("high's march takes a rule by optical depth");
  }
  return rule.drawnBandM;
}

/**
 * The march's drawn surfaces (plan R08, R08.T11.b; `decision-r08-t11-frame.md` §2, lever A's
 * family): surfaces the scene draws, each lit at the gate's albedo, on the datum and at the
 * band's edge, {@link drawnBandEdgeM} over it, where high's march takes the bare ground's count by
 * the optical depth from the datum's hit. From orbit, 400 km over the ground and 10⁷ m from the
 * centre (the face, ground zenith 0/60/80/87°, and the limb's last ground ray, 89°) under the disc
 * family's suns (30/80/90/95° at the surface × 0/90/180°, one azimuth at the nadir), on the ground
 * and at the band's edge, but the disc's own rays (on the ground from 400 km at 0–87°), 168 rays;
 * from 20 and 60 km, the inside family's terrain under its suns, at the band's edge or, where it
 * is nearer or behind the camera, just past the aerial perspective's 32 km reach, where the march
 * first draws a surface from there, 20 rays (no ground lies 40 km from the 60 km camera, as in the
 * inside family). On the gate's sphere a ray's chord through the shells is set by its ground zenith
 * alone, so the 10⁷ m rays repeat the 400 km ones' segments and figures in this `f64` twin; the
 * `f32` kernel's far form, which only 10⁷ m takes, is the smoke's (`R08.T11.b the face's count on a
 * drawn surface`). A surface beyond the datum's hit, R07's mesh
 * on its chords, ends the march at the hit, so its ray is the one on the ground. The inside and
 * near-level families are drawn surfaces on the ground too.
 */
export function drawnSurfaceRays(): GateRay[] {
  const rays: GateRay[] = [];
  const bandM = drawnBandEdgeM();
  // Each surface at its height: 0 on the ground, the band's on its edge's shell, by construction.
  const push = (
    name: string,
    o: Vec3,
    d: Vec3,
    span: { readonly tStartM: number; readonly tEndM: number; readonly heightM: number },
    zen: number,
    az: number,
  ): void => {
    const q = along(o, d, span.tEndM);
    rays.push({
      family: "drawn",
      name: `drawn, ${name}, sun ${zen}°/${az}°`,
      o,
      d,
      s: sunAt(q, d, zen, az),
      tStartM: span.tStartM,
      tEndM: span.tEndM,
      ground: true,
      drawnHeightM: span.heightM,
      sunPoint: q,
      sunZenithDeg: zen,
    });
  };
  for (const cameraM of DRAWN_ORBIT_CAMERAS_M) {
    const o = { x: 0, y: 0, z: cameraM };
    const place = cameraM === DRAWN_ORBIT_CAMERAS_M[0] ? "400 km" : "1e7 m";
    for (const groundZenithDeg of DRAWN_GROUND_ZENITHS_DEG) {
      const alpha = Math.asin((BOTTOM_M / cameraM) * Math.sin(rad(groundZenithDeg)));
      const d = { x: Math.sin(alpha), y: 0, z: -Math.cos(alpha) };
      const shell = sphereHits(o, d, TOP_M);
      const ground = sphereHits(o, d, BOTTOM_M);
      const edge = sphereHits(o, d, BOTTOM_M + bandM);
      if (shell === null || ground === null || edge === null) {
        throw new Error(`a drawn ray from ${place} at ${groundZenithDeg}° misses the ground`);
      }
      // On the ground but where the disc family holds the same ray, and at the band's edge.
      const ends = [
        { at: "on the ground", tEndM: ground[0], heightM: 0 },
        { at: `${bandM / 1_000} km over the ground`, tEndM: edge[0], heightM: bandM },
      ].filter(({ heightM }) => !(heightM === 0 && place === "400 km" && groundZenithDeg < 89));
      for (const end of ends) {
        for (const zen of [30, 80, 90, 95]) {
          for (const az of groundZenithDeg === 0 ? [0] : [0, 90, 180]) {
            push(
              `${place}, ground zenith ${groundZenithDeg}°, ${end.at}`,
              o,
              d,
              { tStartM: shell[0], tEndM: end.tEndM, heightM: end.heightM },
              zen,
              az,
            );
          }
        }
      }
    }
  }
  const reachM = TABLE_SIZES.high.aerialPerspectiveReachM;
  for (const cameraM of INSIDE_CAMERAS_M) {
    const o = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    for (const distanceM of INSIDE_TERRAIN_M) {
      const hit = towardTerrain(o, distanceM);
      if (hit === null) {
        continue;
      }
      // The band's edge, where the camera stands over it, or the first surface past the reach.
      const edge = cameraM > bandM ? sphereHits(o, hit.d, BOTTOM_M + bandM) : null;
      const tM = Math.max(edge?.[0] ?? 0, reachM + 1_000);
      if (!(tM < hit.tM)) {
        continue;
      }
      const q = along(o, hit.d, tM);
      const heightM =
        edge !== null && tM === edge[0] ? bandM : Math.hypot(q.x, q.y, q.z) - BOTTOM_M;
      for (const [zen, az] of INSIDE_SUNS) {
        push(
          `${cameraM / 1_000} km, terrain ${distanceM / 1_000} km, ${(tM / 1_000).toFixed(1)} km ahead at ${(heightM / 1_000).toFixed(1)} km`,
          o,
          hit.d,
          { tStartM: 0, tEndM: tM, heightM },
          zen,
          az,
        );
      }
    }
  }
  return rays;
}

/** The sky view's suns: 30/80/90° from the camera's zenith, 0/180° in azimuth from the view's. */
const SKY_SUNS = [30, 80, 90].flatMap((zen) => [0, 180].map((az) => [zen, az] as const));

/** The addenda's suns for the band and past it: 30/80/90/95° at the camera, 0/180° in azimuth. */
const BAND_SUNS = [30, 80, 90, 95].flatMap((zen) => [0, 180].map((az) => [zen, az] as const));

/** Addendum B's high cameras, m: the sky view serves cameras up to 1 m below the top. */
const HIGH_CAMERAS_M = [60_000, 80_000, R05_EARTH_REFERENCE.topHeightM - 1];

/** The last light's cameras, m: every band's, 30–99.999 km (`decision-r08-t8-leans.md` §5.5). */
const LAST_LIGHT_CAMERAS_M = [
  30_000,
  40_000,
  50_000,
  60_000,
  80_000,
  95_000,
  R05_EARTH_REFERENCE.topHeightM - 1,
];

/**
 * The last light's rays, as shares of the way from 0.06° under the horizontal to 0.05° over the
 * visible horizon, evenly.
 */
const LAST_LIGHT_RAYS = [0, 0.25, 0.5, 0.75, 1] as const;

/** The last light's suns, degrees over the camera's own visible horizon: 0.5° under to 1.5° over. */
const LAST_LIGHT_SUNS_OVER_DEG = [-0.5, 0, 0.5, 1, 1.5] as const;

/** The last light's suns' azimuths from the ray's, degrees: behind the view and well off its side. */
const LAST_LIGHT_AZIMUTHS_DEG = [120, 150, 180] as const;

/**
 * The sky's rays: from the decision's model, cameras at 2 m, 1 km, 10 km and 50 km at view
 * zenith 0/45/80/88/95°; from addendum A, cameras at 0.5–50 km in the band between the local and
 * the visible horizon (0.2, 0.5, 0.8 and 0.98 of the dip, and 0.01° either side of the horizontal)
 * and 0.01°, 0.1°, 0.5° and 2° past the visible horizon, the sun at 30/80/90/95° × 0/180° at the
 * camera; from addendum B, cameras at 60, 80 and 99.999 km, their band (0.2, 0.5, 0.8 and 0.98 of
 * the dip, the same suns) and their limb (tangent at 0.84–25 km). No ground bounce, as the kernel
 * has none; a ray that meets the ground ends there.
 *
 * Since R08.T8 (`decision-r08-t8-leans.md` §5.5), the last light, the ray march's: from cameras at
 * 30, 40, 50, 60, 80, 95 and 99.999 km, five rays from 0.06° under the horizontal to 0.05° over the
 * visible horizon, the sun 0.5° under to 1.5° over the camera's own visible horizon in 0.5° steps
 * at 120°, 150° and 180° from the ray's azimuth, 525 rays, every one a two-sided twilight ray. The
 * gate's band suns stopped at 95° from the zenith and its cameras skipped 30–45 km, which hid the
 * window where even steps drew the sky under the horizontal 45–124 codes off (§5.1).
 */
export function skyRays(): GateRay[] {
  const rays: GateRay[] = [];
  const add1 = (
    family: SkyFamily,
    cameraM: number,
    viewZenithDeg: number,
    suns: ReadonlyArray<readonly [zenithDeg: number, azimuthDeg: number, name?: string]> = SKY_SUNS,
  ): void => {
    const o = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    const d = { x: Math.sin(rad(viewZenithDeg)), y: 0, z: Math.cos(rad(viewZenithDeg)) };
    const shell = sphereHits(o, d, TOP_M);
    const ground = sphereHits(o, d, BOTTOM_M);
    if (shell === null) {
      throw new Error("a sky ray misses the top");
    }
    const tEndM = ground !== null && ground[0] > 0 ? ground[0] : shell[1];
    for (const [zen, az, sunName] of suns) {
      const s = {
        x: Math.sin(rad(zen)) * Math.cos(rad(az)),
        y: Math.sin(rad(zen)) * Math.sin(rad(az)),
        z: Math.cos(rad(zen)),
      };
      rays.push({
        family,
        name: `${family}, ${cameraM} m, view zenith ${viewZenithDeg.toFixed(3)}°, sun ${sunName ?? `${zen}°/${az}°`}`,
        o,
        d,
        s,
        tStartM: 0,
        tEndM,
        ground: false,
        drawnHeightM: null,
        sunPoint: o,
        sunZenithDeg: zen,
      });
    }
  };
  for (const cameraM of [2, 1_000, 10_000, 50_000]) {
    for (const vz of [0, 45, 80, 88, 95]) {
      add1("sky", cameraM, vz);
    }
  }
  for (const cameraM of [500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000]) {
    const dipDeg = (Math.acos(BOTTOM_M / (BOTTOM_M + cameraM)) * 180) / Math.PI;
    for (const vz of [89.99, 90.01, ...[0.2, 0.5, 0.8, 0.98].map((f) => 90 + f * dipDeg)]) {
      add1("band", cameraM, vz, BAND_SUNS);
    }
    for (const past of [0.01, 0.1, 0.5, 2]) {
      add1("past", cameraM, 90 + dipDeg + past, BAND_SUNS);
    }
  }
  // From addendum B: cameras high in the atmosphere (the sky view's highest is 1 m below the top),
  // the band seen from them and their limb, at tangent heights of 0.84–25 km with the sun at 30° and
  // 90° at the tangent point, as the march's limb family sets it.
  for (const cameraM of HIGH_CAMERAS_M) {
    const dipDeg = (Math.acos(BOTTOM_M / (BOTTOM_M + cameraM)) * 180) / Math.PI;
    for (const f of [0.2, 0.5, 0.8, 0.98]) {
      add1("high band", cameraM, 90 + f * dipDeg, BAND_SUNS);
    }
  }
  for (const cameraM of HIGH_CAMERAS_M) {
    const o = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    for (const tangentM of [840, 2_500, 8_400, 25_000]) {
      const alpha = Math.asin((BOTTOM_M + tangentM) / o.z);
      const d = { x: Math.sin(alpha), y: 0, z: -Math.cos(alpha) };
      const shell = sphereHits(o, d, TOP_M);
      if (shell === null) {
        throw new Error("a high camera's limb ray misses the top");
      }
      const q = along(o, d, -dot(o, d));
      for (const zen of [30, 90]) {
        for (const az of [0, 90, 180]) {
          rays.push({
            family: "high limb",
            name: `high limb, ${cameraM / 1_000} km, tangent ${tangentM / 1_000} km, sun ${zen}°/${az}°`,
            o,
            d,
            s: sunAt(q, d, zen, az),
            tStartM: 0,
            tEndM: shell[1],
            ground: false,
            drawnHeightM: null,
            sunPoint: q,
            sunZenithDeg: zen,
          });
        }
      }
    }
  }
  // Since R08.T8 (`decision-r08-t8-leans.md` §5.5): the camera's last light, the sky between the
  // horizontal and the visible horizon with the Sun just over or under the camera's own visible
  // horizon, behind the view or well off to its side.
  for (const cameraM of LAST_LIGHT_CAMERAS_M) {
    const dipDeg = (Math.acos(BOTTOM_M / (BOTTOM_M + cameraM)) * 180) / Math.PI;
    const suns = LAST_LIGHT_SUNS_OVER_DEG.flatMap((overDeg) => {
      const where =
        overDeg === 0 ? "on" : `${Math.abs(overDeg)}° ${overDeg > 0 ? "over" : "under"}`;
      return LAST_LIGHT_AZIMUTHS_DEG.map(
        (az) => [90 + dipDeg - overDeg, az, `${where} the visible horizon/${az}°`] as const,
      );
    });
    for (const f of LAST_LIGHT_RAYS) {
      // From 0.06° under the horizontal to 0.05° over the visible horizon.
      add1("last light", cameraM, 90.06 + f * (dipDeg - 0.11), suns);
    }
  }
  return rays;
}

/** The gate's 1,641 rays: the march's, then the sky's. */
export function gateRays(): GateRay[] {
  return [...marchRays(), ...skyRays()];
}

/** The sun's angle from the zenith at the point a gate ray names it at, degrees. */
export function sunZenithAtDeg(ray: GateRay): number {
  return (Math.acos(dot(normalise(ray.sunPoint), ray.s)) * 180) / Math.PI;
}

/** The bound of `LOW_TWIN_WORST` a ray's kernel and camera fall under. */
export function lowBoundOf(ray: GateRay): keyof typeof LOW_TWIN_WORST {
  if (kernelOf(ray) === "rayMarch") {
    return "rayMarch";
  }
  return ray.family === "high limb" || ray.family === "high band" ? "skyViewHigh" : "skyView";
}

/** Whether a gate ray is a twilight ray, held to 5% at high's counts. */
export const twilight = (ray: GateRay): boolean =>
  grazingTwilight(ray.o, ray.d, ray.s, ray.tStartM, ray.tEndM);

/**
 * A ray's pixel in R05's twin: the march's in-scattering, plus the lit ground through its
 * transmittance.
 */
export function singleScatteringPixel(
  ray: GateRay,
  placement: Placement,
  n: number,
  rule: MarchDepthSteps | null,
): Rgb3 {
  // R05's Earth has no march edge; its count by optical depth reads the twin's own sun grid.
  const samples = gateMarchSamples(ray, n, rule, 0, (heightM, mu, out) => {
    sunTransmittance(BOTTOM_M + heightM, mu, out);
  });
  const { l, t } = march(ray.o, ray.d, ray.s, stepsFor(placement, ray, samples));
  if (!ray.ground) {
    return l;
  }
  // A surface drawn in the air is lit at its own height (R08.T11.b).
  const mu = dot(normalise(along(ray.o, ray.d, ray.tEndM)), ray.s);
  const toSun: Rgb3 = [0, 0, 0];
  sunTransmittance(BOTTOM_M + (ray.drawnHeightM ?? 0), mu, toSun);
  const lit = (c: 0 | 1 | 2): number => (mu > 0 ? (GROUND_ALBEDO / Math.PI) * mu * toSun[c] : 0);
  return [l[0] + t[0] * lit(0), l[1] + t[1] * lit(1), l[2] + t[2] * lit(2)];
}

/** e = max over channels of |L − L_ref| ÷ max(L_ref, 10⁻³ L_max) (R08 Design note 10's floor). */
export function errorOf(l: Rgb3, ref: Rgb3, lMax: Rgb3): number {
  let e = 0;
  for (const c of CHANNELS) {
    e = Math.max(e, Math.abs(l[c] - ref[c]) / Math.max(ref[c], FLOOR * lMax[c]));
  }
  return e;
}

/**
 * A scheme: a placement at a count for the march and one for the sky view, and each count's scaling
 * by the segment's optical depth (plan R08, R08.T8.e; `marchSteps.ts`'s `MarchDepthSteps`), `null`
 * where every ray takes the count.
 */
export interface Scheme {
  readonly placement: Placement;
  readonly marchSteps: number;
  readonly skySteps: number;
  /** The ray march's rule for its families' rays: `TableSizes`' `marchDepthSteps`. */
  readonly marchDepthSteps: MarchDepthSteps | null;
  /** The rule for the sky view's families' rays: `null` but where the ray march draws them. */
  readonly skyDepthSteps: MarchDepthSteps | null;
}

/** One ray's reference, its confirmation and its pixel under each scheme. */
export interface RayResult {
  readonly ray: GateRay;
  /** The reference's steps: 2,048, or 4,096 or 8,192 where fewer have not converged. */
  readonly referenceSteps: number;
  readonly reference: Rgb3;
  /** The same placement at twice the reference's steps. */
  readonly confirm: Rgb3;
  readonly pixels: ReadonlyMap<string, Rgb3>;
}

const byDepth = (rule: MarchDepthSteps | null): string => (rule === null ? "" : " by depth");
const keyOf = (s: Scheme): string =>
  `${s.placement} ${s.marchSteps}${byDepth(s.marchDepthSteps)}/${s.skySteps}${byDepth(s.skyDepthSteps)}`;
/**
 * The high setting's counts under the kernels' rule (since R08.T8 the ray march's two-sided sky
 * segments under the sun-aware warp; the references keep R05.T12.e's placed steps).
 */
export const HIGH: Scheme = {
  placement: "kernel",
  marchSteps: TABLE_SIZES.high.rayMarchSamples,
  skySteps: TABLE_SIZES.high.skyViewSamples,
  marchDepthSteps: TABLE_SIZES.high.marchDepthSteps,
  skyDepthSteps: null,
};
/** The low setting's counts under the kernels' rule, as {@link HIGH}'s. */
export const LOW: Scheme = {
  placement: "kernel",
  marchSteps: TABLE_SIZES.low.rayMarchSamples,
  skySteps: TABLE_SIZES.low.skyViewSamples,
  marchDepthSteps: TABLE_SIZES.low.marchDepthSteps,
  skyDepthSteps: null,
};
/** A scheme's counts with even steps, every ray at its count. */
export const evenAt = (s: Scheme): Scheme => ({
  ...s,
  placement: "even",
  marchDepthSteps: null,
  skyDepthSteps: null,
});

/**
 * A ray's pixel under a placement at a count of steps, scaled under placement `kernel` by the
 * segment's optical depth where `rule` is given (plan R08, R08.T8.e; {@link gateMarchSamples}).
 */
export type PixelOf = (
  ray: GateRay,
  placement: Placement,
  n: number,
  rule: MarchDepthSteps | null,
) => Rgb3;

/** The gate's results: each ray's, and each kernel's brightest reference pixel per channel. */
export interface Gate {
  readonly results: readonly RayResult[];
  readonly lMax: ReadonlyMap<"rayMarch" | "skyView", Rgb3>;
}

/** Each kernel's brightest reference pixel per channel. */
function brightest(results: readonly RayResult[]): Map<"rayMarch" | "skyView", Rgb3> {
  const lMax = new Map<"rayMarch" | "skyView", Rgb3>();
  for (const kernel of ["rayMarch", "skyView"] as const) {
    const max: Rgb3 = [0, 0, 0];
    for (const { ray, reference } of results) {
      if (kernelOf(ray) === kernel) {
        for (const c of CHANNELS) {
          max[c] = Math.max(max[c], reference[c]);
        }
      }
    }
    lMax.set(kernel, max);
  }
  return lMax;
}

/**
 * Runs the gate: each ray's reference, the coarsest of 2,048, 4,096 or 8,192 placed steps within
 * 0.05% of twice its steps, and its pixel under each scheme.
 *
 * @remarks
 * Every ray starts at 2,048 steps, confirmed against 4,096. Each of two levels then judges every
 * ray against the brightest references of the level before, and doubles the steps of each one whose
 * reference has not converged: the reference takes its confirmation's pixel, and the confirmation
 * twice as many steps again. A ray at 8,192, confirmed against 16,384, goes no further; whether it
 * has converged is the tests'.
 */
export function runGate(
  rays: readonly GateRay[],
  pixelOf: PixelOf,
  schemes: readonly Scheme[],
): Gate {
  let results: readonly RayResult[] = rays.map((ray) => {
    const sky = kernelOf(ray) === "skyView";
    const steps = (s: Scheme): number => (sky ? s.skySteps : s.marchSteps);
    const rule = (s: Scheme): MarchDepthSteps | null => (sky ? s.skyDepthSteps : s.marchDepthSteps);
    return {
      ray,
      referenceSteps: REFERENCE_STEPS,
      reference: pixelOf(ray, "placed", REFERENCE_STEPS, null),
      confirm: pixelOf(ray, "placed", 2 * REFERENCE_STEPS, null),
      pixels: new Map(schemes.map((s) => [keyOf(s), pixelOf(ray, s.placement, steps(s), rule(s))])),
    };
  });
  // One level a doubling up to the most: 2,048 to 4,096, then 4,096 to 8,192.
  for (let from = REFERENCE_STEPS; from < MAX_REFERENCE_STEPS; from *= 2) {
    const lMax = brightest(results);
    results = results.map((result) => {
      const max = lMax.get(kernelOf(result.ray)) ?? [0, 0, 0];
      if (
        result.referenceSteps >= MAX_REFERENCE_STEPS ||
        errorOf(result.reference, result.confirm, max) < CONVERGED
      ) {
        return result;
      }
      const doubled = 2 * result.referenceSteps;
      return {
        ray: result.ray,
        referenceSteps: doubled,
        reference: result.confirm,
        confirm: pixelOf(result.ray, "placed", 2 * doubled, null),
        pixels: result.pixels,
      };
    });
  }
  return { results, lMax: brightest(results) };
}

/** One ray's e under a scheme, against the reference. */
export interface RayError {
  readonly ray: GateRay;
  readonly e: number;
}

/** Each ray's e under a scheme. */
export function errors(gate: Gate, scheme: Scheme): RayError[] {
  return gate.results.map((result) => {
    const l = result.pixels.get(keyOf(scheme));
    const max = gate.lMax.get(kernelOf(result.ray));
    if (l === undefined || max === undefined) {
      throw new Error(`no ${keyOf(scheme)} pixel for ${result.ray.name}`);
    }
    return { ray: result.ray, e: errorOf(l, result.reference, max) };
  });
}

/** The rays a scheme fails at high's tolerances: e over 2%, or 5% for twilight rays. */
export function failures(gate: Gate, scheme: Scheme): RayError[] {
  return errors(gate, scheme).filter(
    ({ ray, e }) => !(e <= (twilight(ray) ? TWILIGHT_TOLERANCE : MARCH_TOLERANCE)),
  );
}

/** The rays a scheme puts over `LOW_TWIN_WORST`, by their kernel, camera and class. */
export function overLowBounds(gate: Gate, scheme: Scheme): RayError[] {
  return errors(gate, scheme).filter(({ ray, e }) => {
    const bound = LOW_TWIN_WORST[lowBoundOf(ray)];
    return !(e <= (twilight(ray) ? bound.twilight : bound.ordinary));
  });
}

/** Each failure as its ray's name and its e in percent. */
export const describeFailures = (failed: readonly RayError[]): string[] =>
  failed.map(({ ray, e }) => `${ray.name}: ${(e * 100).toFixed(2)}%`);
