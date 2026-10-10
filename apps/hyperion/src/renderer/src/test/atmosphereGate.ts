/**
 * R05.T12.e's quadrature gate on the per-frame marches (plan R05, `decision-r05-high-atmosphere.md`
 * and its addenda A and B), shared by `view/atmosphere/marchSteps.test.ts`, which holds R05's
 * kernels to it, and `view/atmosphere/tablesCpu.test.ts`, which holds R08.T6.a's CPU twin to it
 * with the multiple-scattering term included.
 *
 * @remarks
 * It holds the gate's 1,046 rays on Earth's medium, its metric and its schemes, the reference's
 * convergence rule, and R05's own `f64` twin of the two kernels' quadrature: single scattering per
 * unit sun illuminance with the sun's transmittance from a fine grid, and a Lambertian ground. Moved
 * here unchanged from `marchSteps.test.ts` (R08.T6.a), so that the twin is held to the same rays
 * and R05's pixels.
 */

import { add, cross, dot, normalise, scale, type Vec3 } from "../geometry/vec3";
import { EARTH_REFERENCE, RAYLEIGH_SCALE_HEIGHT_M } from "../view/atmosphere/earth";
import { TABLE_SIZES } from "../view/atmosphere/hillaire";
import {
  grazingTwilight,
  LOW_TWIN_WORST,
  MARCH_TOLERANCE,
  marchSplit,
  marchStep,
  type MarchStep,
  TWILIGHT_TOLERANCE,
} from "../view/atmosphere/marchSteps";
import { densityAt, extinction, type MediumTerm } from "../view/atmosphere/medium";
import {
  intersectsGround,
  opticalDepth,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
} from "../view/atmosphere/opticalDepth";

/** WGS 84's equatorial radius a, m (NIMA TR8350.2): the twin's sphere. */
export const BOTTOM_M = 6_378_137;
const TOP_M = BOTTOM_M + EARTH_REFERENCE.topHeightM;
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
 * The reference's placed steps, confirmed against twice as many to 0.05%. A ray whose reference has
 * not converged at 4,096 takes 8,192, confirmed against 16,384: a twilight ray that crosses the
 * ground's hard shadow edge converges only as 1 ÷ n there.
 */
export const REFERENCE_STEPS = 4_096;
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

const TERMS = EARTH_REFERENCE.terms.map((term) => ({ term, sigma: extinction(term) }));

/** The sun's optical depth per channel at the grid's nodes, (u, v) = (i, j) ÷ (size − 1). */
function sunOpticalDepthGrid(): Float64Array {
  const grid = new Float64Array(SUN_GRID_U * SUN_GRID_V * 3);
  for (let j = 0; j < SUN_GRID_V; j += 1) {
    for (let i = 0; i < SUN_GRID_U; i += 1) {
      const { rM, mu } = transmittanceUvToRMu(SHELL, i / (SUN_GRID_U - 1), j / (SUN_GRID_V - 1));
      grid.set(
        opticalDepth(EARTH_REFERENCE, BOTTOM_M, rM, mu, SUN_STEPS),
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

/** How a scheme places its steps: the kernels' placement, or the even steps they had. */
export type Placement = "placed" | "even";

/** The steps of a march over [tStartM, tEndM]: the kernels' placement, or the even steps they had. */
export function stepsFor(
  placement: Placement,
  o: Vec3,
  d: Vec3,
  tStartM: number,
  tEndM: number,
  n: number,
): MarchStep[] {
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
function sphereHits(o: Vec3, d: Vec3, radiusM: number): readonly [number, number] | null {
  const b = dot(o, d);
  const disc = b * b - (dot(o, o) - radiusM * radiusM);
  if (disc < 0) {
    return null;
  }
  const root = Math.sqrt(disc);
  return [-b - root, -b + root];
}

type MarchFamily = "disc" | "limb" | "inside" | "near-level";
type SkyFamily = "sky" | "band" | "past" | "high limb" | "high band";
/** A family of the gate's rays: the march's four, then the sky view's five. */
export type Family = MarchFamily | SkyFamily;
const MARCH_FAMILIES: readonly Family[] = ["disc", "limb", "inside", "near-level"];
/** The families in the order the gate counts them. */
export const FAMILIES: readonly Family[] = [
  ...MARCH_FAMILIES,
  "sky",
  "band",
  "past",
  "high limb",
  "high band",
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
  readonly ground: boolean;
  /** The point the sun's zenith angle is given at, and that angle. */
  readonly sunPoint: Vec3;
  readonly sunZenithDeg: number;
}

/** The kernel a family's rays are the twin of. */
export const kernelOf = (ray: GateRay): "rayMarch" | "skyView" =>
  MARCH_FAMILIES.includes(ray.family) ? "rayMarch" : "skyView";

/**
 * A sun at a zenith angle and an azimuth from the ray's own, at a point `q`; a vertical ray takes
 * its azimuths from any horizontal axis.
 */
function sunAt(q: Vec3, d: Vec3, zenithDeg: number, azimuthDeg: number): Vec3 {
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

/**
 * The march's rays: from the decision's model, the disc from 400 km (ground zenith 0/60/80/87° ×
 * sun 30/80/90/95° at the ground × azimuth 0/90/180°), the limb at R08's tangent heights 0.1,
 * 0.3, 1, 3 and 10 Rayleigh scale heights (sun 30/90° at the tangent point × three azimuths), and
 * terrain 40–300 km away from 20 and 60 km (sun 30/85° × azimuth 0/180°), beyond the
 * aerial-perspective volume's 32 km; from addendum A, the near-level march from 0.5–5 km to terrain
 * at 40, 80 and 150 km before the visible horizon (the ruling's suns).
 */
export function marchRays(): GateRay[] {
  const rays: GateRay[] = [];
  const orbit = { x: 0, y: 0, z: BOTTOM_M + 400_000 };
  const fromOrbit = (
    family: "disc" | "limb",
    name: string,
    alphaRad: number,
    sunZenithDeg: number,
    sunAzimuthDeg: number,
  ): void => {
    const d = { x: Math.sin(alphaRad), y: 0, z: -Math.cos(alphaRad) };
    const shell = sphereHits(orbit, d, TOP_M);
    if (shell === null) {
      throw new Error(`${name} misses the atmosphere`);
    }
    const ground = sphereHits(orbit, d, BOTTOM_M);
    const hits = ground !== null && ground[0] > 0;
    const q = along(orbit, d, hits ? ground[0] : -dot(orbit, d));
    rays.push({
      family,
      name,
      o: orbit,
      d,
      s: sunAt(q, d, sunZenithDeg, sunAzimuthDeg),
      tStartM: shell[0],
      tEndM: hits ? ground[0] : shell[1],
      ground: hits,
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
          sunPoint: q,
          sunZenithDeg: zen,
        });
      }
    }
  };
  const insideSuns = [30, 85].flatMap((zen) => [0, 180].map((az) => [zen, az] as const));
  for (const cameraM of [20_000, 60_000]) {
    toTerrain("inside", cameraM, [40_000, 100_000, 300_000], insideSuns);
  }
  for (const cameraM of [500, 1_000, 2_000, 5_000]) {
    toTerrain("near-level", cameraM, [40_000, 80_000, 150_000], SUNS_FULL);
  }
  return rays;
}

/** The sky view's suns: 30/80/90° from the camera's zenith, 0/180° in azimuth from the view's. */
const SKY_SUNS = [30, 80, 90].flatMap((zen) => [0, 180].map((az) => [zen, az] as const));

/** The addenda's suns for the band and past it: 30/80/90/95° at the camera, 0/180° in azimuth. */
const BAND_SUNS = [30, 80, 90, 95].flatMap((zen) => [0, 180].map((az) => [zen, az] as const));

/** Addendum B's high cameras, m: the sky view serves cameras up to 1 m below the top. */
const HIGH_CAMERAS_M = [60_000, 80_000, EARTH_REFERENCE.topHeightM - 1];

/**
 * The sky view's rays: from the decision's model, cameras at 2 m, 1 km, 10 km and 50 km at view
 * zenith 0/45/80/88/95°; from addendum A, cameras at 0.5–50 km in the band between the local and
 * the visible horizon (0.2, 0.5, 0.8 and 0.98 of the dip, and 0.01° either side of the horizontal)
 * and 0.01°, 0.1°, 0.5° and 2° past the visible horizon, the sun at 30/80/90/95° × 0/180° at the
 * camera; from addendum B, cameras at 60, 80 and 99.999 km, their band (0.2, 0.5, 0.8 and 0.98 of
 * the dip, the same suns) and their limb (tangent at 0.84–25 km). No ground bounce, as the kernel
 * has none; a ray that meets the ground ends there.
 */
export function skyRays(): GateRay[] {
  const rays: GateRay[] = [];
  const add1 = (
    family: SkyFamily,
    cameraM: number,
    viewZenithDeg: number,
    suns: ReadonlyArray<readonly [number, number]> = SKY_SUNS,
  ): void => {
    const o = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    const d = { x: Math.sin(rad(viewZenithDeg)), y: 0, z: Math.cos(rad(viewZenithDeg)) };
    const shell = sphereHits(o, d, TOP_M);
    const ground = sphereHits(o, d, BOTTOM_M);
    if (shell === null) {
      throw new Error("a sky ray misses the top");
    }
    const tEndM = ground !== null && ground[0] > 0 ? ground[0] : shell[1];
    for (const [zen, az] of suns) {
      const s = {
        x: Math.sin(rad(zen)) * Math.cos(rad(az)),
        y: Math.sin(rad(zen)) * Math.sin(rad(az)),
        z: Math.cos(rad(zen)),
      };
      rays.push({
        family,
        name: `${family}, ${cameraM} m, view zenith ${viewZenithDeg.toFixed(3)}°, sun ${zen}°/${az}°`,
        o,
        d,
        s,
        tStartM: 0,
        tEndM,
        ground: false,
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
            sunPoint: q,
            sunZenithDeg: zen,
          });
        }
      }
    }
  }
  return rays;
}

/** The gate's 1,046 rays: the march's, then the sky view's. */
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
export function singleScatteringPixel(ray: GateRay, placement: Placement, n: number): Rgb3 {
  const { l, t } = march(
    ray.o,
    ray.d,
    ray.s,
    stepsFor(placement, ray.o, ray.d, ray.tStartM, ray.tEndM, n),
  );
  if (!ray.ground) {
    return l;
  }
  const mu = dot(normalise(along(ray.o, ray.d, ray.tEndM)), ray.s);
  const toSun: Rgb3 = [0, 0, 0];
  sunTransmittance(BOTTOM_M, mu, toSun);
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

/** A scheme: a placement at a count for the march and one for the sky view. */
export interface Scheme {
  readonly placement: Placement;
  readonly marchSteps: number;
  readonly skySteps: number;
}

/** One ray's reference, its confirmation and its pixel under each scheme. */
export interface RayResult {
  readonly ray: GateRay;
  /** The reference's steps, 4,096 or, where that has not converged, 8,192. */
  readonly referenceSteps: number;
  readonly reference: Rgb3;
  /** The same placement at twice the reference's steps. */
  readonly confirm: Rgb3;
  readonly pixels: ReadonlyMap<string, Rgb3>;
}

const keyOf = (s: Scheme): string => `${s.placement} ${s.marchSteps}/${s.skySteps}`;
/** The high setting's counts, placed. */
export const HIGH: Scheme = {
  placement: "placed",
  marchSteps: TABLE_SIZES.high.rayMarchSamples,
  skySteps: TABLE_SIZES.high.skyViewSamples,
};
/** The low setting's counts, placed. */
export const LOW: Scheme = {
  placement: "placed",
  marchSteps: TABLE_SIZES.low.rayMarchSamples,
  skySteps: TABLE_SIZES.low.skyViewSamples,
};
/** A scheme's counts with even steps. */
export const evenAt = (s: Scheme): Scheme => ({ ...s, placement: "even" });

/** A ray's pixel under a placement at a count of steps. */
export type PixelOf = (ray: GateRay, placement: Placement, n: number) => Rgb3;

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
 * Runs the gate: each ray's reference at 4,096 placed steps, confirmed against 8,192, or at 8,192
 * confirmed against 16,384 where 4,096 has not converged, and its pixel under each scheme.
 */
export function runGate(
  rays: readonly GateRay[],
  pixelOf: PixelOf,
  schemes: readonly Scheme[],
): Gate {
  const first = rays.map((ray) => {
    const steps = (s: Scheme): number => (kernelOf(ray) === "skyView" ? s.skySteps : s.marchSteps);
    return {
      ray,
      referenceSteps: REFERENCE_STEPS,
      reference: pixelOf(ray, "placed", REFERENCE_STEPS),
      confirm: pixelOf(ray, "placed", 2 * REFERENCE_STEPS),
      pixels: new Map(schemes.map((s) => [keyOf(s), pixelOf(ray, s.placement, steps(s))])),
    };
  });
  const firstMax = brightest(first);
  const results = first.map((result) => {
    const max = firstMax.get(kernelOf(result.ray)) ?? [0, 0, 0];
    if (errorOf(result.reference, result.confirm, max) < CONVERGED) {
      return result;
    }
    return {
      ray: result.ray,
      referenceSteps: 2 * REFERENCE_STEPS,
      reference: result.confirm,
      confirm: pixelOf(result.ray, "placed", 4 * REFERENCE_STEPS),
      pixels: result.pixels,
    };
  });
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
