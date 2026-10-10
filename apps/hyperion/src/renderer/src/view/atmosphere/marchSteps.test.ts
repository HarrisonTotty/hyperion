import { beforeAll, describe, expect, it } from "vitest";

import { normalise, type Vec3 } from "../../geometry/vec3";
import {
  CONVERGED,
  describeFailures,
  errorOf,
  errors,
  evenAt,
  failures,
  FAMILIES,
  type Family,
  type Gate,
  gateRays,
  HIGH,
  kernelOf,
  LOW,
  oldFactorF32,
  overLowBounds,
  REFERENCE_STEPS,
  runGate,
  type Scheme,
  singleScatteringPixel,
  stepFactorF32,
  sunZenithAtDeg,
  twilight,
} from "../../test/atmosphereGate";
import { RAY_MARCH_KERNEL, SKY_VIEW_KERNEL } from "./hillaire";
import { grazingTwilight, marchSplit, marchStep, type MarchStep } from "./marchSteps";
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
// The rays, the metric and R05's single-scattering twin are `test/atmosphereGate.ts`'s, which
// R08.T6.a's CPU twin is held to as well.

const SCHEMES: readonly Scheme[] = [HIGH, evenAt(HIGH), LOW, evenAt(LOW)];

let gate: Gate = { results: [], lMax: new Map() };

describe("the per-frame marches' quadrature (R05.T12.e's gate)", () => {
  beforeAll(() => {
    gate = runGate(gateRays(), singleScatteringPixel, SCHEMES);
  }, 600_000);

  it("covers the decision's rays and those of addenda A and B", () => {
    const count = (family: Family): number =>
      gate.results.filter((r) => r.ray.family === family).length;
    expect(FAMILIES.map(count)).toEqual([40, 30, 20, 108, 120, 336, 224, 72, 96]);
  });

  it("puts each ray's sun at its zenith angle where the ray names it", () => {
    const misplaced = gate.results.filter(
      ({ ray }) => !(Math.abs(sunZenithAtDeg(ray) - ray.sunZenithDeg) < 1e-9),
    );
    expect(misplaced.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("gives every ray a finite reference, lit wherever the sun is up", () => {
    const dark = gate.results.filter(
      ({ ray, reference }) =>
        !reference.every(Number.isFinite) || (ray.sunZenithDeg < 90 && !(reference[1] > 0)),
    );
    expect(dark.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("has a reference of 4,096 placed steps within 0.05% of 8,192, or of 8,192 within 16,384", () => {
    const unconverged = gate.results
      .map(({ ray, reference, confirm }) => ({
        ray,
        e: errorOf(reference, confirm, gate.lMax.get(kernelOf(ray)) ?? [0, 0, 0]),
      }))
      .filter(({ e }) => !(e < CONVERGED));
    expect(describeFailures(unconverged)).toEqual([]);
  });

  it("takes the longer reference only for twilight rays", () => {
    const longer = gate.results.filter(({ referenceSteps }) => referenceSteps > REFERENCE_STEPS);
    expect(longer.filter(({ ray }) => !twilight(ray)).map(({ ray }) => ray.name)).toEqual([]);
  });

  it("holds every ray within 2%, and twilight rays within 5%, at high's counts", () => {
    expect(describeFailures(failures(gate, HIGH))).toEqual([]);
  });

  it("fails the even steps the kernels had, on the disc, at the limb and in the sky", () => {
    const failed = failures(gate, evenAt(HIGH));
    const passed = (["disc", "limb", "sky"] as const).filter(
      (family) => !failed.some(({ ray }) => ray.family === family),
    );
    expect(passed).toEqual([]);
  });

  it("leaves low's worst ray over the whole set no worse than even steps' at low's counts", () => {
    const worst = (scheme: Scheme): number => Math.max(...errors(gate, scheme).map(({ e }) => e));
    expect(worst(LOW)).toBeLessThanOrEqual(worst(evenAt(LOW)));
  });

  it("keeps the twin within the bounds low's GPU agreement check holds low to", () => {
    expect(describeFailures(overLowBounds(gate, LOW))).toEqual([]);
  });
});
