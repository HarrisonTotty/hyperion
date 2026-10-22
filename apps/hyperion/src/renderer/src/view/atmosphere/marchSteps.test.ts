import { beforeAll, describe, expect, it } from "vitest";

import { dot, normalise, type Vec3 } from "../../geometry/vec3";
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
  lowBoundOf,
  oldFactorF32,
  overLowBounds,
  REFERENCE_STEPS,
  runGate,
  type Scheme,
  singleScatteringPixel,
  skySegmentOf,
  stepFactorF32,
  sunZenithAtDeg,
  twilight,
} from "../../test/atmosphereGate";
import { RAY_MARCH_KERNEL, SKY_VIEW_KERNEL } from "./hillaire";
import {
  grazingTwilight,
  marchEdgesM,
  marchEvenStep,
  marchEvenSteps,
  marchPieces,
  marchPiecesSteps,
  marchSplit,
  marchStep,
  type MarchStep,
  SKY_WARP_COSINE,
  skyWarpOf,
} from "./marchSteps";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import aerialPerspectiveWgsl from "./shaders/aerialPerspective.wgsl?raw";
import commonWgsl from "./shaders/common.wgsl?raw";
import rayMarchWgsl from "./shaders/rayMarch.wgsl?raw";
import skyViewWgsl from "./shaders/skyView.wgsl?raw";
import irradianceWgsl from "./shaders/irradiance.wgsl?raw";
import scatteringWgsl from "./shaders/scattering.wgsl?raw";
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

/** The boundaries the steps of a march put between them, from its start: each step's two ends. */
function boundariesOf(steps: readonly MarchStep[]): number[] {
  return [
    (steps[0]?.tM ?? Number.NaN) - (steps[0]?.dtM ?? Number.NaN) / 2,
    ...steps.map(({ tM, dtM }) => tM + dtM / 2),
  ];
}

/** Whether `t` stands at one of `boundaries` to 10⁻⁶ m. */
const isBoundary = (boundaries: readonly number[], t: number): boolean =>
  boundaries.some((b) => Math.abs(b - t) < 1e-6);

describe("marchPieces and marchPiecesSteps: the marches cut at layer edges (R08.T6.i)", () => {
  /** A two-sided segment from orbit, 12 steps before t* and 20 after. */
  const orbit = marchSplit(300_000, 2_600_000, 1_450_000, 32, false);
  /** A two-sided segment from a camera in the air, its camera side placed toward the camera. */
  const camera = marchSplit(0, 1_200_000, 79_000, 30, true);
  /** A one-sided segment, upward from the camera: 20 steps after t*, none before. */
  const upward = marchSplit(0, 5_000, -1, 20, true);

  it("take each ramp of a term's layer edges as one edge, at its middle height", () => {
    expect(marchEdgesM([18_560, 18_640, 38_490, 38_500], 100_000)).toEqual([18_600, 38_495]);
    expect(marchEdgesM([0, 120, 5_000, 5_100], 100_000)).toEqual([60, 5_050]);
    expect(marchEdgesM([], 100_000)).toEqual([]);
    // Unused places, and a middle at or above the top, are left out.
    expect(marchEdgesM([-1, -1, -1, -1], 100_000)).toEqual([]);
    expect(marchEdgesM([99_990, 100_020], 100_000)).toEqual([]);
  });

  it("keep R05.T12.e's steps to the bit on a segment that crosses no edge", () => {
    for (const split of [orbit, camera, upward]) {
      const placed = Array.from({ length: split.stepsBefore + split.stepsAfter }, (_, i) =>
        marchStep(split, i),
      );
      expect(marchPiecesSteps(marchPieces(split, []))).toEqual(placed);
      // Crossings outside the segment, at its ends or at t* are no crossings.
      const outside = [-split.beforeM - 5, -split.beforeM, 0, split.afterM, split.afterM + 5];
      const pieces = marchPieces(split, outside);
      expect(pieces.fromLowestM).toEqual([]);
      expect(marchPiecesSteps(pieces)).toEqual(placed);
    }
  });

  it("keep R05.T12.e's steps on a side that crosses no edge, cutting the other", () => {
    const pieces = marchPieces(orbit, [400_000]);
    expect(pieces.before).toBeNull();
    expect(pieces.after).not.toBeNull();
    const steps = marchPiecesSteps(pieces);
    for (let i = 0; i < orbit.stepsBefore; i += 1) {
      expect(steps[i]).toEqual(marchStep(orbit, i));
    }
  });

  it("take each crossing once, in order, whatever order it is given in", () => {
    const pieces = marchPieces(orbit, [700_000, -200_000, 100_000, -600_000, 100_000]);
    expect(pieces.fromLowestM).toEqual([-600_000, -200_000, 100_000, 700_000]);
    expect(pieces.crossingsBefore).toBe(2);
  });

  it("tile the segment front to back, every crossing a boundary between steps", () => {
    for (const [split, crossings] of [
      [orbit, [-600_000, -200_000, 100_000, 700_000]],
      [camera, [-40_000, 300_000]],
      [upward, [1_000, 3_500]],
    ] as const) {
      const steps = marchPiecesSteps(marchPieces(split, crossings));
      expect(steps).toHaveLength(split.stepsBefore + split.stepsAfter);
      const boundaries = boundariesOf(steps);
      for (const [k, { dtM }] of steps.entries()) {
        expect(dtM).toBeGreaterThan(0);
        expect(boundaries[k + 1] ?? Number.NaN).toBeCloseTo((boundaries[k] ?? Number.NaN) + dtM, 6);
      }
      expect(boundaries[0]).toBeCloseTo(split.startM, 6);
      expect(boundaries.at(-1)).toBeCloseTo(split.startM + split.beforeM + split.afterM, 6);
      for (const t of crossings) {
        expect(isBoundary(boundaries, split.lowestM + t)).toBe(true);
      }
      expect(isBoundary(boundaries, split.lowestM)).toBe(true);
    }
  });

  it("share a side's steps by the square roots of its pieces' lengths, at least one a piece", () => {
    // Pieces of 1,000 and 4,000 m: √1,000 ÷ (√1,000 + √4,000) of the 18 left over is 6.
    expect(marchPieces(upward, [1_000]).after).toEqual([7, 13]);
    // A piece far shorter than the side still takes one.
    const cut = marchPieces(upward, [0.01, 2_500]);
    expect(cut.after?.[0]).toBe(1);
    expect(cut.after?.reduce((a, b) => a + b, 0)).toBe(20);
  });

  it("place each piece toward its end nearer t*", () => {
    const steps = marchPiecesSteps(marchPieces(upward, [1_000]));
    // [0, 1,000] in 7 from t*, then [1,000, 5,000] in 13 from its crossing.
    expect(steps[0]?.dtM).toBeCloseTo(1_000 / 7 ** 2, 9);
    expect(steps[6]?.dtM).toBeCloseTo((13 * 1_000) / 7 ** 2, 9);
    expect(steps[7]?.dtM).toBeCloseTo(4_000 / 13 ** 2, 9);
    // Before t*, from orbit: the step before each crossing and before t* is its piece's smallest.
    const before = marchPieces(orbit, [-600_000]);
    const fromOrbit = marchPiecesSteps(before);
    const [first, second] = before.before ?? [];
    if (first === undefined || second === undefined) {
      throw new Error("the side before t* is not cut");
    }
    expect(fromOrbit[first - 1]?.dtM).toBeCloseTo((orbit.beforeM - 600_000) / first ** 2, 6);
    expect(fromOrbit[first + second - 1]?.dtM).toBeCloseTo(600_000 / second ** 2, 6);
  });

  it("place a camera side's pieces toward their ends nearer the camera", () => {
    const pieces = marchPieces(camera, [-40_000]);
    const [first, second] = pieces.before ?? [];
    if (first === undefined || second === undefined) {
      throw new Error("the camera side is not cut");
    }
    const steps = marchPiecesSteps(pieces);
    expect(steps[0]?.dtM).toBeCloseTo((camera.beforeM - 40_000) / first ** 2, 6);
    expect(steps[first]?.dtM).toBeCloseTo(40_000 / second ** 2, 6);
  });

  it("keep R05.T12.e's placement on a side with more pieces than steps, and count it", () => {
    // One step before t*, 1 m long, which a crossing would cut in two.
    const short = marchSplit(0, 1e12, 1, 32, false);
    expect(short.stepsBefore).toBe(1);
    const pieces = marchPieces(short, [-0.5, 1e9]);
    expect(pieces.before).toBeNull();
    expect(pieces.after).not.toBeNull();
    expect(pieces.crowded).toBe(1);
    expect(marchPiecesSteps(pieces)[0]).toEqual(marchStep(short, 0));
  });
});

/**
 * Where a point `t` along a segment from `startM`, `lengthM` long, stands in u under the warp a; a
 * point a rounding before the start, at it.
 */
function warpedUAt(startM: number, lengthM: number, warp: number, t: number): number {
  const x = (t - startM) / lengthM;
  if (!(x > 0)) {
    return 0;
  }
  return (2 * x) / (1 - warp + Math.sqrt((1 - warp) ** 2 + 4 * warp * x));
}

/** A unit vector `deg` from +y toward +x. */
function fromY(deg: number): Vec3 {
  return { x: Math.sin((deg * Math.PI) / 180), y: Math.cos((deg * Math.PI) / 180), z: 0 };
}

describe("the sun-aware warp of the ray march's two-sided sky segments (R08.T8)", () => {
  /** Two-sided segments: from orbit, and from a camera in the band, as the ray march splits them. */
  const orbit = marchSplit(300_000, 2_600_000, 1_450_000, 36, false);
  const band = marchSplit(0, 1_100_000, 640_000, 16, false);
  const WARPS = [0, 0.3, 0.7, 1] as const;

  it("is 0 with the Sun ahead of the ray or across it, 1 beyond about 101.5°, and linear in d · s between", () => {
    const d = { x: 0, y: 1, z: 0 };
    const at = fromY;
    expect(skyWarpOf(d, at(0))).toBe(0);
    expect(skyWarpOf(d, at(90))).toBe(0);
    expect(skyWarpOf(d, at(180))).toBe(1);
    expect(skyWarpOf(d, at(102))).toBe(1);
    expect(skyWarpOf(d, at(95))).toBeCloseTo(Math.sin((5 * Math.PI) / 180) / SKY_WARP_COSINE, 12);
    expect(skyWarpOf(d, null)).toBe(0);
    // No step in it as the Sun passes the ends of its ramp.
    expect(Math.abs(skyWarpOf(d, at(90.0001)))).toBeLessThan(1e-5);
    expect(skyWarpOf(d, at(Math.acos(-SKY_WARP_COSINE) / (Math.PI / 180) - 1e-4))).toBeCloseTo(
      1,
      4,
    );
  });

  it("keeps the even steps to the bit at a = 0 on a segment that crosses no edge", () => {
    for (const split of [orbit, band]) {
      const n = split.stepsBefore + split.stepsAfter;
      const unitM = (split.beforeM + split.afterM) / n;
      const even = Array.from({ length: n }, (_, i) => ({
        tM: split.lowestM + (unitM * (i + 0.5) - split.beforeM),
        dtM: unitM,
      }));
      expect(Array.from({ length: n }, (_, i) => marchEvenStep(split, i, 0))).toEqual(even);
      expect(marchEvenSteps(split, [], 0)).toEqual(even);
    }
  });

  it("is R05.T12.e's one-sided placement from the segment's start at a = 1", () => {
    for (const split of [orbit, band]) {
      const n = split.stepsBefore + split.stepsAfter;
      const fromStart = marchSplit(
        split.startM,
        split.startM + split.beforeM + split.afterM,
        0,
        n,
        false,
      );
      for (let i = 0; i < n; i += 1) {
        const warped = marchEvenStep(split, i, 1);
        const placed = marchStep(fromStart, i);
        expect(warped.tM).toBeCloseTo(placed.tM, 6);
        expect(warped.dtM / placed.dtM).toBeCloseTo(1, 12);
      }
    }
  });

  it("tiles the segment front to back at every warp, whole and cut, every crossing a boundary", () => {
    for (const [split, crossings] of [
      [orbit, []],
      [orbit, [-600_000, -200_000, 100_000, 700_000]],
      [band, []],
      [band, [-500_000, -20_000, 300_000]],
    ] as const) {
      for (const warp of WARPS) {
        const steps = marchEvenSteps(split, crossings, warp);
        expect(steps).toHaveLength(split.stepsBefore + split.stepsAfter);
        const boundaries = boundariesOf(steps);
        for (const [k, { dtM }] of steps.entries()) {
          expect(dtM).toBeGreaterThan(0);
          expect(boundaries[k + 1] ?? Number.NaN).toBeCloseTo(
            (boundaries[k] ?? Number.NaN) + dtM,
            6,
          );
        }
        expect(boundaries[0]).toBeCloseTo(split.startM, 6);
        expect(boundaries.at(-1)).toBeCloseTo(split.startM + split.beforeM + split.afterM, 6);
        for (const t of crossings) {
          expect(isBoundary(boundaries, split.lowestM + t)).toBe(true);
        }
      }
    }
  });

  it("spaces each piece's steps evenly in u, sharing the segment's by the pieces' lengths in u", () => {
    const crossings = [-500_000, -20_000, 300_000];
    const lengthM = band.beforeM + band.afterM;
    for (const warp of WARPS) {
      const steps = marchEvenSteps(band, crossings, warp);
      const us = boundariesOf(steps).map((t) => warpedUAt(band.startM, lengthM, warp, t));
      const cutsU = crossings.map((c) => warpedUAt(band.startM, lengthM, warp, band.lowestM + c));
      const ends = [0, ...cutsU, 1];
      let at = 0;
      for (let p = 0; p + 1 < ends.length; p += 1) {
        const highU = ends[p + 1] ?? Number.NaN;
        const widths: number[] = [];
        while (at + 1 < us.length && (us[at] ?? Number.NaN) < highU - 1e-12) {
          widths.push((us[at + 1] ?? Number.NaN) - (us[at] ?? Number.NaN));
          at += 1;
        }
        const width = (highU - (ends[p] ?? Number.NaN)) / widths.length;
        for (const w of widths) {
          expect(w).toBeCloseTo(width, 9);
        }
        // One a piece, and the 12 left over shared by the pieces' lengths in u, each piece's count
        // rounding its end's share: within a step of 12 times its length.
        expect(
          Math.abs(widths.length - 1 - 12 * (highU - (ends[p] ?? Number.NaN))),
        ).toBeLessThanOrEqual(1);
      }
    }
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
  it("places the sky view's and the ray march's steps with marchSplit and marchStep, every two-sided sky segment of the ray march under the sun-aware warp (R08.T8)", () => {
    expect(SKY_VIEW_KERNEL.reference).toMatch(/\bmarchSplit\([^;]*, samples, true\);/);
    expect(RAY_MARCH_KERNEL.reference).toMatch(
      /\bmarchSplit\([^;]*, samples, shell\.x <= 0\.0 && depth > 0\.0\);/,
    );
    for (const kernel of [SKY_VIEW_KERNEL.reference, RAY_MARCH_KERNEL.reference]) {
      expect(kernel).toMatch(/\bmarchStep\(split, i\)/);
    }
    for (const wgsl of [skyViewWgsl, rayMarchWgsl]) {
      expect(wgsl).not.toMatch(/\(f32\(i\) \+ 0\.5\) \* dt/);
    }
    // Nothing drawn along it and both sides non-empty, from inside the air or outside it
    // (`decision-r08-t8-leans.md` §4 and §5.4): the sun-aware warp by the first source, once a
    // pixel, 0 where none is drawn, whole or cut at the layer edges.
    expect(rayMarchWgsl).toMatch(
      /let evenSky = depth == 0\.0 && split\.beforeM > 0\.0 && split\.afterM > 0\.0;/,
    );
    expect(rayMarchWgsl).toMatch(
      /var warp = 0\.0;\s*if \(evenSky && count > 0u\) \{\s*warp = marchSkyWarp\(dir, view\.sources\[0\]\.direction\.xyz\);\s*\}/,
    );
    // One loop a path, its step chosen in it (`rayMarchStepOf`, `rayMarchWalkStepOf`).
    expect(rayMarchWgsl).toMatch(
      /fn rayMarchStepOf\([^)]*\) -> MarchStep \{\s*if \(evenSky\) \{\s*return marchEvenStep\(split, i, warp\);\s*\}\s*return marchStep\(split, i\);/,
    );
    expect(rayMarchWgsl).toMatch(
      /fn rayMarchWalkStepOf\([^)]*\) -> MarchStep \{\s*if \(evenSky\) \{\s*return marchWalkEvenStep\(walk, i\);\s*\}\s*return marchWalkStep\(walk, i\);/,
    );
    expect(rayMarchWgsl).toMatch(
      /for \(var i = 0u; i < samples; i\+\+\) \{\s*rayMarchStep\(&march, ray, body, rayMarchStepOf\(split, i, evenSky, warp\)(, &\w+)*\);/,
    );
    expect(rayMarchWgsl).toMatch(
      /if \(evenSky\) \{\s*marchWalkEvenReady\(&walk, warp\);\s*\} else \{\s*marchWalkReady\(&walk\);\s*\}\s*for \(var i = 0u; i < samples; i\+\+\) \{\s*rayMarchStep\(&march, ray, body, rayMarchWalkStepOf\(&walk, i, evenSky\)(, &\w+)*\);/,
    );
    // The sky view and the multiple-scattering kernel keep R05.T12.e's placement.
    for (const wgsl of [skyViewWgsl, multiScatteringWgsl]) {
      expect(wgsl).not.toMatch(/marchEvenStep|marchWalkEven|marchSkyWarp/);
    }
  });

  it("takes the warp's constant and form in common.wgsl as marchSteps.ts takes them (R08.T8)", () => {
    expect(commonWgsl).toMatch(
      new RegExp(
        `const MARCH_SKY_WARP_COSINE : f32 = ${String(SKY_WARP_COSINE).replace(".", "\\.")};`,
      ),
    );
    expect(commonWgsl).toMatch(
      /fn marchSkyWarp\(dir : vec3f, sun : vec3f\) -> f32 \{\s*return clamp\(-dot\(dir, sun\) \/ MARCH_SKY_WARP_COSINE, 0\.0, 1\.0\);/,
    );
    expect(commonWgsl).toMatch(
      /let mid = \(1\.0 - warp\) \* \(k \+ 0\.5\) \+ warp \* \(k \* k \+ k \+ 0\.5\) \/ n;\s*let width = \(1\.0 - warp\) \+ warp \* \(2\.0 \* k \+ 1\.0\) \/ n;/,
    );
    expect(commonWgsl).toMatch(/return 2\.0 \* x \/ \(b \+ sqrt\(b \* b \+ 4\.0 \* warp \* x\)\);/);
  });

  it("cuts the sky view's, the ray march's and the multiple-scattering kernel's steps at the medium's layer edges, and keeps R05.T12.e's where it has none (R08.T6.i)", () => {
    for (const wgsl of [skyViewWgsl, rayMarchWgsl, multiScatteringWgsl]) {
      expect(wgsl).toMatch(/var<uniform> layerEdges : LayerEdges;/);
      expect(wgsl).toMatch(/let edgeCount = min\(u32\(layerEdges\.count\.x\), MARCH_EDGES_MAX\);/);
      // No edge: R05.T12.e's steps, and no walk declared; edges: the walk, made ready first. The
      // ray march chooses each step in its loop, its two-sided sky segments' warped (R08.T8). The
      // sources' directions and angles follow by pointer (R08.T8.d).
      expect(wgsl).toMatch(
        /if \(edgeCount == 0u\) \{\s*for \(var i = 0u; i < samples; i\+\+\) \{\s*\w+\(&march, ray, (body, (frame, )?)?(marchStep\(split, i\)|rayMarchStepOf\(split, i, evenSky, warp\))(, &\w+)*\);/,
      );
      expect(wgsl).toMatch(/var walk : MarchWalk;\s*marchWalkStart\(&walk, split\);/);
      expect(wgsl).toMatch(
        /marchWalkReady\(&walk\);\s*(\}\s*)?for \(var i = 0u; i < samples; i\+\+\) \{\s*\w+\(&march, ray, (body, (frame, )?)?(marchWalkStep\(&walk, i\)|rayMarchWalkStepOf\(&walk, i, evenSky\))(, &\w+)*\);/,
      );
    }
    expect(skyViewWgsl).toMatch(/marchWalkCrossSphere\(/);
    expect(multiScatteringWgsl).toMatch(/marchWalkCrossSphere\(/);
    expect(rayMarchWgsl).toMatch(/marchWalkCrossShell\(/);
    // The aerial perspective keeps its even steps (decision-r08-t6d-findings.md §1.3).
    expect(aerialPerspectiveWgsl).not.toMatch(/layerEdges|marchWalk/);
    expect(commonWgsl).toMatch(/const MARCH_EDGES_MAX : u32 = 2u \* MAX_TERMS;/);
  });

  it("evaluates each term's density once a sample in the per-frame kernels", () => {
    for (const wgsl of [
      skyViewWgsl,
      aerialPerspectiveWgsl,
      rayMarchWgsl,
      sourceWgsl,
      scatteringWgsl,
      irradianceWgsl,
    ]) {
      expect(wgsl).not.toMatch(/\bmediumAt\(/);
      expect(wgsl).not.toMatch(/\bphasedScatteringAt\(/);
    }
    expect(sourceWgsl).toMatch(/fn sourceAt\([^)]*local : SampleMedium/);
    // Since R08.T9.b the sample's medium and its phases are scattering.wgsl's, which the sky's
    // irradiance takes too.
    expect(scatteringWgsl).toMatch(/fn sampleMediumAt\(heightM : f32\) -> SampleMedium/);
    expect(irradianceWgsl).toMatch(/let local = sampleMediumAt\(sampleHeightM\);/);
  });

  it("takes each step's in-scattering through common.wgsl's stepFactor", () => {
    expect(commonWgsl).toMatch(/fn stepFactor\(x : vec3f\) -> vec3f/);
    // Since R08.T7 every per-frame kernel steps through source.wgsl's `viewMarchStep`, each light
    // slot's share through its own throughput.
    expect(sourceWgsl).toMatch(
      /\(\*march\)\.throughput\[p\] \* shares\[p\] \* dtM \* stepFactor\(stepDepth\)/,
    );
    for (const wgsl of [skyViewWgsl, aerialPerspectiveWgsl, rayMarchWgsl]) {
      expect(wgsl).toMatch(/viewMarchStep\(/);
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

/**
 * The rays R05's twin misses at high's counts under the kernels' rule since R08.T8, the ray march's
 * two-sided sky segments under the sun-aware warp (`decision-r08-t8-leans.md` §4.3 and §5.4;
 * measured 2026-10-21 and recorded in plan R08's Risks, "Deviations in T8.a, as built"): the limb
 * tangent at 0.1 H with the sun on its tangent point's horizon beyond it, from 400 km and from
 * 101 km (congruent chords), over its 5%. Its sun is ahead, so the warp leaves it in even steps
 * (a = 0), and the last light holds. R05.T12.e's placement held it at 3.22% in this twin, which has
 * single scattering alone; R08.T6.a's twin, with multiple scattering, holds it in even steps at
 * 4.35% on R05's Earth and 3.24% on the measured Earth (`tablesCpu.test.ts`). Pinned to the 0.01
 * point both ways, so that a change shows.
 */
const R05_TWIN_HIGH_MISS = [
  "limb, 0.1 H, sun 90°/0°: 13.25%",
  "night limb, 101 km, 0.1 H, sun 90°/0°: 13.25%",
] as const;

/**
 * The same rays over `LOW_TWIN_WORST`'s ray-march twilight bound at low's counts, which is R08.T6.a's
 * twin's (23.94% on R05's Earth): R05's twin's single scattering puts them at 78.90%, the even steps'
 * own figure (measured 2026-10-21 under the sun-aware warp, recorded as {@link R05_TWIN_HIGH_MISS}
 * is; the last light holds).
 */
const R05_TWIN_LOW_MISS = [
  "limb, 0.1 H, sun 90°/0°: 78.90%",
  "night limb, 101 km, 0.1 H, sun 90°/0°: 78.90%",
] as const;

let gate: Gate = { results: [], lMax: new Map() };

describe("the per-frame marches' quadrature (R05.T12.e's gate)", () => {
  beforeAll(() => {
    gate = runGate(gateRays(), singleScatteringPixel, SCHEMES);
  }, 600_000);

  it("covers the decision's rays, those of addenda A and B, and R08.T8's night limb and last light", () => {
    const count = (family: Family): number =>
      gate.results.filter((r) => r.ray.family === family).length;
    expect(FAMILIES.map(count)).toEqual([40, 30, 70, 20, 108, 120, 336, 224, 72, 96, 525]);
    expect(gate.results).toHaveLength(1_641);
  });

  it("makes every last-light ray a two-sided twilight sky ray of the ray march, held to its 5% (R08.T8)", () => {
    const last = gate.results.filter(({ ray }) => ray.family === "last light");
    expect(last.filter(({ ray }) => !twilight(ray)).map(({ ray }) => ray.name)).toEqual([]);
    expect(new Set(last.map(({ ray }) => lowBoundOf(ray)))).toEqual(new Set(["rayMarch"]));
    expect(last.filter(({ ray }) => !skySegmentOf(ray)).map(({ ray }) => ray.name)).toEqual([]);
    // Each falls to a lowest point beyond the camera and climbs out, above the ground.
    const oneSided = last.filter(({ ray }) => {
      const split = marchSplit(ray.tStartM, ray.tEndM, -dot(ray.o, ray.d), 2, false);
      return !(split.beforeM > 0 && split.afterM > 0);
    });
    expect(oneSided.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("makes every night-limb ray a twilight ray of the ray march, held to its 5% (R08.T8)", () => {
    const night = gate.results.filter(({ ray }) => ray.family === "night limb");
    expect(night.filter(({ ray }) => !twilight(ray)).map(({ ray }) => ray.name)).toEqual([]);
    expect(new Set(night.map(({ ray }) => lowBoundOf(ray)))).toEqual(new Set(["rayMarch"]));
    // None repeats the limb's own rays.
    const limb = new Set(
      gate.results
        .filter(({ ray }) => ray.family === "limb")
        .map(({ ray }) => `${ray.d.x} ${ray.s.x} ${ray.s.y} ${ray.s.z}`),
    );
    expect(
      night.filter(({ ray }) => limb.has(`${ray.d.x} ${ray.s.x} ${ray.s.y} ${ray.s.z}`)),
    ).toEqual([]);
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

  it("has a reference of 2,048, 4,096 or 8,192 placed steps within 0.05% of twice its steps", () => {
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

  it("holds every ray within 2%, and twilight rays within 5%, at high's counts, but the limb's one named miss (R08.T8)", () => {
    expect(describeFailures(failures(gate, HIGH))).toEqual([...R05_TWIN_HIGH_MISS]);
  });

  // Where the kernels keep R05.T12.e's placement. Since R08.T8 the ray march steps the limb under
  // the sun-aware warp itself, evenly where its sun is ahead or across it, so the limb left this
  // list: at high's counts R05's even steps miss it only on the kernels' own miss
  // (`R05_TWIN_HIGH_MISS`, 13.25%), its ordinary rays at 0.52%, where R05.T12.e's placement had
  // 1.07% and 3.22% (measured 2026-10-21).
  it("fails the even steps the kernels had, on the disc and in the sky", () => {
    const failed = failures(gate, evenAt(HIGH));
    const passed = (["disc", "sky"] as const).filter(
      (family) => !failed.some(({ ray }) => ray.family === family),
    );
    expect(passed).toEqual([]);
  });

  it("leaves low's worst ray over the whole set no worse than even steps' at low's counts", () => {
    const worst = (scheme: Scheme): number => Math.max(...errors(gate, scheme).map(({ e }) => e));
    expect(worst(LOW)).toBeLessThanOrEqual(worst(evenAt(LOW)));
  });

  it("keeps the twin within the bounds low's GPU agreement check holds low to, but the limb's one named miss (R08.T8)", () => {
    expect(describeFailures(overLowBounds(gate, LOW))).toEqual([...R05_TWIN_LOW_MISS]);
  });
});
