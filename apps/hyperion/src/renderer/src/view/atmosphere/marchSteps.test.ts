import { beforeAll, describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3 } from "../../geometry/vec3";
import { EARTH_REFERENCE, RAYLEIGH_SCALE_HEIGHT_M } from "./earth";
import { RAY_MARCH_KERNEL, SKY_VIEW_KERNEL, TABLE_SIZES } from "./hillaire";
import {
  grazingTwilight,
  LOW_TWIN_WORST,
  MARCH_TOLERANCE,
  marchSplit,
  marchStep,
  type MarchStep,
  TWILIGHT_TOLERANCE,
} from "./marchSteps";
import { densityAt, extinction, type MediumTerm } from "./medium";
import {
  intersectsGround,
  opticalDepth,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
} from "./opticalDepth";
import aerialPerspectiveWgsl from "./shaders/aerialPerspective.wgsl?raw";
import commonWgsl from "./shaders/common.wgsl?raw";
import rayMarchWgsl from "./shaders/rayMarch.wgsl?raw";
import skyViewWgsl from "./shaders/skyView.wgsl?raw";
import sourceWgsl from "./shaders/source.wgsl?raw";

/** The steps of a placed march of `n` steps over [tStartM, tEndM]. */
function stepsOf(
  tStartM: number,
  tEndM: number,
  nearestM: number,
  n: number,
  fromCamera: boolean,
): MarchStep[] {
  const split = marchSplit(tStartM, tEndM, nearestM, n, fromCamera);
  return Array.from({ length: n }, (_, i) => marchStep(split, i));
}

describe("marchSplit and marchStep", () => {
  it("tile the segment front to back, each step's midpoint inside it", () => {
    for (const [tStart, tEnd, nearest, n, fromCamera] of [
      [0, 100_000, -5, 30, true],
      [300_000, 2_600_000, 1_450_000, 32, false],
      [12, 4_000, 9_000, 16, false],
      [0, 80_000, 20_000, 2, true],
      [0, 1_200_000, 79_000, 30, true],
    ] as const) {
      const steps = stepsOf(tStart, tEnd, nearest, n, fromCamera);
      let edge = tStart;
      for (const { tM, dtM } of steps) {
        expect(tM - dtM / 2).toBeCloseTo(edge, 6);
        edge = tM + dtM / 2;
        expect(dtM).toBeGreaterThan(0);
      }
      expect(edge).toBeCloseTo(tEnd, 6);
    }
  });

  it("give a one-sided segment all its steps: upward from the camera, or down to the ground", () => {
    expect(marchSplit(0, 100_000, -40, 30, true)).toMatchObject({ stepsBefore: 0, stepsAfter: 30 });
    expect(marchSplit(300_000, 800_000, 900_000, 32, false)).toMatchObject({
      lowestM: 800_000,
      stepsBefore: 32,
      stepsAfter: 0,
      beforeTowardStart: false,
    });
  });

  it("share a two-sided segment's steps by the square roots of its sides' lengths", () => {
    // √1,000 ÷ (√1,000 + √3,000) of 32 is 11.7.
    expect(marchSplit(0, 4_000, 1_000, 32, false)).toMatchObject({
      stepsBefore: 12,
      stepsAfter: 20,
    });
  });

  it("give a side that is not empty at least one step", () => {
    expect(marchSplit(0, 1e12, 1, 32, false)).toMatchObject({ stepsBefore: 1, stepsAfter: 31 });
    expect(marchSplit(0, 1e12, 1e12 - 1, 32, false)).toMatchObject({
      stepsBefore: 31,
      stepsAfter: 1,
    });
  });

  it("round half a step up, as the WGSL's floor(x + 0.5) does and WGSL's round() would not", () => {
    // √1 ÷ (√1 + √9) of 10 is 2.5.
    expect(marchSplit(0, 10, 1, 10, false).stepsBefore).toBe(3);
  });

  it("place the camera side of a two-sided segment from a camera in the air toward the camera", () => {
    const toward = marchSplit(0, 1_200_000, 79_000, 30, true);
    expect(toward.beforeTowardStart).toBe(true);
    const steps = stepsOf(0, 1_200_000, 79_000, 30, true);
    const n = toward.stepsBefore;
    expect(steps[0]?.dtM).toBeCloseTo(79_000 / n ** 2, 6);
    expect(steps[n - 1]?.dtM).toBeCloseTo(((2 * n - 1) * 79_000) / n ** 2, 6);
  });

  it("place every other side toward the lowest point: from orbit, or a one-sided segment", () => {
    expect(marchSplit(0, 1_200_000, 79_000, 30, false).beforeTowardStart).toBe(false);
    expect(marchSplit(0, 100_000, 200_000, 30, true).beforeTowardStart).toBe(false);
  });

  it("shorten every other side's steps quadratically toward the lowest point", () => {
    const up = stepsOf(0, 100_000, -1, 32, true);
    expect(up[0]?.dtM).toBeCloseTo(100_000 / 32 ** 2, 6);
    expect(up[31]?.dtM).toBeCloseTo((63 * 100_000) / 32 ** 2, 6);
    const toGround = stepsOf(0, 100_000, 200_000, 32, true);
    expect(toGround[31]?.dtM).toBeCloseTo(100_000 / 32 ** 2, 6);
  });

  it("refuse fewer than 2 steps", () => {
    expect(() => marchSplit(0, 1, 0, 1, false)).toThrow(RangeError);
    expect(() => marchSplit(0, 1, 0, 2.5, false)).toThrow(RangeError);
  });
});

/** A unit vector `deg` from +z toward +x. */
function tilted(deg: number): Vec3 {
  return { x: Math.sin((deg * Math.PI) / 180), y: 0, z: Math.cos((deg * Math.PI) / 180) };
}

describe("grazingTwilight", () => {
  const camera = { x: 0, y: 0, z: 6_378_137 + 1_000 };

  it("needs the sun more than 80° from the zenith, by more than the margin", () => {
    const level = { x: 0, y: 1, z: 0 };
    expect(grazingTwilight(camera, level, tilted(80), 0, 100_000)).toBe(false);
    expect(grazingTwilight(camera, level, tilted(80.01), 0, 100_000)).toBe(true);
  });

  it("needs the ray within 10° of the horizon, by more than the margin", () => {
    const sun = tilted(90);
    expect(grazingTwilight(camera, tilted(80), sun, 0, 100_000)).toBe(false);
    expect(grazingTwilight(camera, tilted(80.01), sun, 0, 100_000)).toBe(true);
  });

  it("judges a ray at its lowest point: the camera, the ground or the tangent point", () => {
    // Straight down from 1 km: the ground below, where the ray is vertical, is not twilight.
    expect(grazingTwilight(camera, tilted(180), tilted(85), 0, 1_000)).toBe(false);
    // A limb ray from orbit with the sun on its tangent point's horizon is twilight there.
    const orbit = { x: 0, y: 0, z: 6_378_137 + 400_000 };
    const alpha = Math.asin((6_378_137 + 1_000) / orbit.z);
    const limb = { x: Math.sin(alpha), y: 0, z: -Math.cos(alpha) };
    const t = -(orbit.z * limb.z);
    const tangent = { x: t * limb.x, y: 0, z: orbit.z + t * limb.z };
    const up = normalise(tangent);
    expect(grazingTwilight(orbit, limb, { x: -up.z, y: 0, z: up.x }, 300_000, 4_000_000)).toBe(
      true,
    );
  });
});

describe("the kernels' WGSL", () => {
  it("places the sky view's and the ray march's steps with marchSplit and marchStep", () => {
    expect(SKY_VIEW_KERNEL.reference).toMatch(/\bmarchSplit\([^;]*, samples, true\);/);
    expect(RAY_MARCH_KERNEL.reference).toMatch(/\bmarchSplit\([^;]*, samples, shell\.x <= 0\.0\);/);
    for (const kernel of [SKY_VIEW_KERNEL.reference, RAY_MARCH_KERNEL.reference]) {
      expect(kernel).toMatch(/\bmarchStep\(split, i\)/);
    }
    for (const wgsl of [skyViewWgsl, rayMarchWgsl]) {
      expect(wgsl).not.toMatch(/\(f32\(i\) \+ 0\.5\) \* dt/);
    }
  });

  it("evaluates each term's density once a sample in the per-frame kernels", () => {
    for (const wgsl of [skyViewWgsl, aerialPerspectiveWgsl, rayMarchWgsl, sourceWgsl]) {
      expect(wgsl).not.toMatch(/\bmediumAt\(/);
      expect(wgsl).not.toMatch(/\bphasedScatteringAt\(/);
    }
    expect(sourceWgsl).toMatch(/fn sourceAt\([^)]*local : SampleMedium/);
  });

  it("takes each step's in-scattering through common.wgsl's stepFactor", () => {
    expect(commonWgsl).toMatch(/fn stepFactor\(x : vec3f\) -> vec3f/);
    for (const wgsl of [skyViewWgsl, aerialPerspectiveWgsl, rayMarchWgsl]) {
      expect(wgsl).toMatch(/throughput \* source \* dt \* stepFactor\(stepDepth\)/);
      expect(wgsl).not.toMatch(/source - source \* stepTransmittance/);
    }
  });
});

/** `common.wgsl`'s stepFactor in emulated f32, each operation rounded, with a correctly rounded exp. */
function stepFactorF32(x: number): number {
  const f = Math.fround;
  const v = f(x);
  if (v < f(0.01)) {
    return f(1 - f(v * f(0.5 - f(v / 6))));
  }
  return f(f(1 - f(Math.exp(-v))) / v);
}

/** The form it replaces, (1 − exp(−x)) ÷ x as (S − S·T) ÷ σ_t computes it, in emulated f32. */
function oldFactorF32(x: number): number {
  const f = Math.fround;
  const v = f(x);
  return f(f(1 - f(Math.exp(-v))) / v);
}

/** g(x) = (1 − e^(−x)) ÷ x in f64, to its last bit at small x. */
function exactFactor(x: number): number {
  return -Math.expm1(-x) / x;
}

describe("the step factor", () => {
  const xs = [1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 0.009_999, 0.01, 0.010_001, 0.1, 1, 10];

  it("keeps a thin step's in-scattering within 2 × 10⁻⁵ in f32, across the switch at 0.01", () => {
    const worst = Math.max(...xs.map((x) => Math.abs(stepFactorF32(x) / exactFactor(x) - 1)));
    expect(worst).toBeLessThan(2e-5);
  });

  it("is needed: the old form loses more than 1% of a step of optical depth 10⁻⁶ in f32", () => {
    expect(Math.abs(oldFactorF32(1e-6) / exactFactor(1e-6) - 1)).toBeGreaterThan(0.01);
  });
});

// --- The quadrature gate (R05.T12.e, with addendum A) --------------------------------------------

/** WGS 84's equatorial radius a, m (NIMA TR8350.2): the twin's sphere. */
const BOTTOM_M = 6_378_137;
const TOP_M = BOTTOM_M + EARTH_REFERENCE.topHeightM;
const SHELL = { bottomRadiusM: BOTTOM_M, topRadiusM: TOP_M };

/** The spike's terrain albedo (Design note 17), the twin's Lambertian ground. */
const GROUND_ALBEDO = 0.15;

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
const REFERENCE_STEPS = 4_096;
const CONVERGED = 5e-4;

/** The floor of e's denominator, as a fraction of the kernel's brightest reference pixel. */
const FLOOR = 1e-3;

type Rgb3 = [number, number, number];
const CHANNELS = [0, 1, 2] as const;
const along = (o: Vec3, d: Vec3, tM: number): Vec3 => add(o, scale(d, tM));
const rad = (deg: number): number => (deg * Math.PI) / 180;

/** The phase a term scatters with, as `phaseOf` in `source.wgsl`. */
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
  }
  return phase;
}

const TERMS = EARTH_REFERENCE.terms.map((term) => ({ term, sigma: extinction(term) }));

let sunGrid: Float64Array | undefined;

/** The sun's optical depth per channel at the grid's nodes, (u, v) = (i, j) ÷ (size − 1). */
function sunOpticalDepthGrid(): Float64Array {
  if (sunGrid !== undefined) {
    return sunGrid;
  }
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
  sunGrid = grid;
  return grid;
}

/**
 * The sun's transmittance from radius `rM` at zenith cosine `muSun` into `out`, or none in the
 * ground's shadow.
 */
function sunTransmittance(rM: number, muSun: number, out: Rgb3): void {
  if (intersectsGround(SHELL, rM, muSun)) {
    out.fill(0);
    return;
  }
  const grid = sunOpticalDepthGrid();
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

type Placement = "placed" | "even";

/** The steps of a march over [tStartM, tEndM]: the kernels' placement, or the even steps they had. */
function stepsFor(
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
type Family = MarchFamily | SkyFamily;
const MARCH_FAMILIES: readonly Family[] = ["disc", "limb", "inside", "near-level"];
const FAMILIES: readonly Family[] = [
  ...MARCH_FAMILIES,
  "sky",
  "band",
  "past",
  "high limb",
  "high band",
];

/** One ray of the gate, the segment its kernel marches and whether it ends on the ground. */
interface GateRay {
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
const kernelOf = (ray: GateRay): "rayMarch" | "skyView" =>
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
function marchRays(): GateRay[] {
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
function skyRays(): GateRay[] {
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

/** The sun's angle from the zenith at the point a gate ray names it at, degrees. */
function sunZenithAtDeg(ray: GateRay): number {
  return (Math.acos(dot(normalise(ray.sunPoint), ray.s)) * 180) / Math.PI;
}

/** The bound of `LOW_TWIN_WORST` a ray's kernel and camera fall under. */
function lowBoundOf(ray: GateRay): keyof typeof LOW_TWIN_WORST {
  if (kernelOf(ray) === "rayMarch") {
    return "rayMarch";
  }
  return ray.family === "high limb" || ray.family === "high band" ? "skyViewHigh" : "skyView";
}

/** Whether a gate ray is a twilight ray, held to 5% at high's counts. */
const twilight = (ray: GateRay): boolean =>
  grazingTwilight(ray.o, ray.d, ray.s, ray.tStartM, ray.tEndM);

/** A ray's pixel: the march's in-scattering, plus the lit ground through its transmittance. */
function pixel(ray: GateRay, placement: Placement, n: number): Rgb3 {
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
function errorOf(l: Rgb3, ref: Rgb3, lMax: Rgb3): number {
  let e = 0;
  for (const c of CHANNELS) {
    e = Math.max(e, Math.abs(l[c] - ref[c]) / Math.max(ref[c], FLOOR * lMax[c]));
  }
  return e;
}

/** A scheme: a placement at a count for the march and one for the sky view. */
interface Scheme {
  readonly placement: Placement;
  readonly marchSteps: number;
  readonly skySteps: number;
}

interface RayResult {
  readonly ray: GateRay;
  /** The reference's steps, 4,096 or, where that has not converged, 8,192. */
  readonly referenceSteps: number;
  readonly reference: Rgb3;
  /** The same placement at twice the reference's steps. */
  readonly confirm: Rgb3;
  readonly pixels: ReadonlyMap<string, Rgb3>;
}

const keyOf = (s: Scheme): string => `${s.placement} ${s.marchSteps}/${s.skySteps}`;
const HIGH: Scheme = {
  placement: "placed",
  marchSteps: TABLE_SIZES.high.rayMarchSamples,
  skySteps: TABLE_SIZES.high.skyViewSamples,
};
const LOW: Scheme = {
  placement: "placed",
  marchSteps: TABLE_SIZES.low.rayMarchSamples,
  skySteps: TABLE_SIZES.low.skyViewSamples,
};
const evenAt = (s: Scheme): Scheme => ({ ...s, placement: "even" });
const SCHEMES: readonly Scheme[] = [HIGH, evenAt(HIGH), LOW, evenAt(LOW)];

let results: RayResult[] = [];
const lMax = new Map<"rayMarch" | "skyView", Rgb3>();

/** One ray's e under a scheme, against the reference. */
interface RayError {
  readonly ray: GateRay;
  readonly e: number;
}

/** Each ray's e under a scheme. */
function errors(scheme: Scheme): RayError[] {
  return results.map((result) => {
    const l = result.pixels.get(keyOf(scheme));
    const max = lMax.get(kernelOf(result.ray));
    if (l === undefined || max === undefined) {
      throw new Error(`no ${keyOf(scheme)} pixel for ${result.ray.name}`);
    }
    return { ray: result.ray, e: errorOf(l, result.reference, max) };
  });
}

/** The rays a scheme fails at high's tolerances: e over 2%, or 5% for twilight rays. */
function failures(scheme: Scheme): RayError[] {
  return errors(scheme).filter(
    ({ ray, e }) => !(e <= (twilight(ray) ? TWILIGHT_TOLERANCE : MARCH_TOLERANCE)),
  );
}

const describeFailures = (failed: readonly RayError[]): string[] =>
  failed.map(({ ray, e }) => `${ray.name}: ${(e * 100).toFixed(2)}%`);

describe("the per-frame marches' quadrature (R05.T12.e's gate)", () => {
  beforeAll(() => {
    const rays = [...marchRays(), ...skyRays()];
    const brightest = (): void => {
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
    };
    results = rays.map((ray) => {
      const steps = (s: Scheme): number =>
        kernelOf(ray) === "skyView" ? s.skySteps : s.marchSteps;
      return {
        ray,
        referenceSteps: REFERENCE_STEPS,
        reference: pixel(ray, "placed", REFERENCE_STEPS),
        confirm: pixel(ray, "placed", 2 * REFERENCE_STEPS),
        pixels: new Map(SCHEMES.map((s) => [keyOf(s), pixel(ray, s.placement, steps(s))])),
      };
    });
    brightest();
    results = results.map((result) => {
      const max = lMax.get(kernelOf(result.ray)) ?? [0, 0, 0];
      if (errorOf(result.reference, result.confirm, max) < CONVERGED) {
        return result;
      }
      return {
        ...result,
        referenceSteps: 2 * REFERENCE_STEPS,
        reference: result.confirm,
        confirm: pixel(result.ray, "placed", 4 * REFERENCE_STEPS),
      };
    });
    brightest();
  }, 600_000);

  it("covers the decision's rays and those of addenda A and B", () => {
    const count = (family: Family): number => results.filter((r) => r.ray.family === family).length;
    expect(FAMILIES.map(count)).toEqual([40, 30, 20, 108, 120, 336, 224, 72, 96]);
  });

  it("puts each ray's sun at its zenith angle where the ray names it", () => {
    const misplaced = results.filter(
      ({ ray }) => !(Math.abs(sunZenithAtDeg(ray) - ray.sunZenithDeg) < 1e-9),
    );
    expect(misplaced.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("gives every ray a finite reference, lit wherever the sun is up", () => {
    const dark = results.filter(
      ({ ray, reference }) =>
        !reference.every(Number.isFinite) || (ray.sunZenithDeg < 90 && !(reference[1] > 0)),
    );
    expect(dark.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("has a reference of 4,096 placed steps within 0.05% of 8,192, or of 8,192 within 16,384", () => {
    const unconverged = results
      .map(({ ray, reference, confirm }) => ({
        ray,
        e: errorOf(reference, confirm, lMax.get(kernelOf(ray)) ?? [0, 0, 0]),
      }))
      .filter(({ e }) => !(e < CONVERGED));
    expect(describeFailures(unconverged)).toEqual([]);
  });

  it("takes the longer reference only for twilight rays", () => {
    const longer = results.filter(({ referenceSteps }) => referenceSteps > REFERENCE_STEPS);
    expect(longer.filter(({ ray }) => !twilight(ray)).map(({ ray }) => ray.name)).toEqual([]);
  });

  it("holds every ray within 2%, and twilight rays within 5%, at high's counts", () => {
    expect(describeFailures(failures(HIGH))).toEqual([]);
  });

  it("fails the even steps the kernels had, on the disc, at the limb and in the sky", () => {
    const failed = failures(evenAt(HIGH));
    const passed = (["disc", "limb", "sky"] as const).filter(
      (family) => !failed.some(({ ray }) => ray.family === family),
    );
    expect(passed).toEqual([]);
  });

  it("leaves low's worst ray over the whole set no worse than even steps' at low's counts", () => {
    const worst = (scheme: Scheme): number => Math.max(...errors(scheme).map(({ e }) => e));
    expect(worst(LOW)).toBeLessThanOrEqual(worst(evenAt(LOW)));
  });

  it("keeps the twin within the bounds low's GPU agreement check holds low to", () => {
    const over = errors(LOW).filter(({ ray, e }) => {
      const bound = LOW_TWIN_WORST[lowBoundOf(ray)];
      return !(e <= (twilight(ray) ? bound.twilight : bound.ordinary));
    });
    expect(describeFailures(over)).toEqual([]);
  });
});
