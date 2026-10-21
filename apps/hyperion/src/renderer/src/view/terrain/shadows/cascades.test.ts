import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../../geometry/vec3";
import type { Quaternion } from "../../camera/pose";
import {
  lookAlong,
  quaternion,
  quaternionFromAxisAngle,
  rotationRows,
} from "../../camera/quaternion";
import {
  IDENTITY_ROTATION,
  rotateToBody,
  type Rotation3,
  rotation3FromRows,
} from "../../coords/rotation";
import { bytesPerTexel, extentOf } from "../../engine/memory";
import { TEXTURE_USAGE } from "../../engine/gpuFlags";
import {
  CASCADE_BYTES,
  CASCADE_COUNT,
  CASCADE_DEPTH_BIAS,
  CASCADE_FADE_FRACTION,
  CASCADE_MEMORY_CATEGORY,
  CASCADE_SIZE_PX,
  CASCADE_TEXTURE,
  type Cascade,
  type CascadeInput,
  cascadeMatricesF64,
  type CascadeSet,
  cascadeSplitsM,
  cascadeWeight,
  casterReachM,
  discTopDirection,
  fitCascades,
  gridLatticeSpacingM,
  halfDiagonalTanSquared,
  lightBasis,
  NORMAL_OFFSET_TEXELS,
  POLE_FALLBACK_RAD,
  shadowedRangeM,
  sliceSphere,
  sunElevationRad,
} from "./cascades";

const RAD = Math.PI / 180;

/**
 * `TERRAIN TEST`'s `TEST MOON`: its mean radius and relief range, m, the split ruling's S6 figures
 * (the worker golden's realised relief, 14,921.5 m, rounded).
 */
const MOON_RADIUS_M = 1_737_400;
const MOON_RELIEF_M = 14_900;

/** An Earth's mean radius, m: 6,371 km, GRS80's R₁ = 6,371.0088 km (Moritz 2000), rounded. */
const EARTH_RADIUS_M = 6_371_000;

/** The hand values' view: 60° across at 16:9, the near plane 0.1 m. */
const FOV_X_RAD = 60 * RAD;
const ASPECT = 16 / 9;

/** mulberry32: a small seeded generator of uniform numbers in [0, 1), for repeatable cases. */
function generator(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** A unit vector uniform on the sphere, from two draws. */
function randomUnit(next: () => number): Vec3 {
  const z = 2 * next() - 1;
  const azimuth = 2 * Math.PI * next();
  const s = Math.sqrt(1 - z * z);
  return { x: s * Math.cos(azimuth), y: s * Math.sin(azimuth), z };
}

/** A uniformly random rotation, as a unit quaternion. */
function randomQuaternion(next: () => number): Quaternion {
  const [u1, u2, u3] = [next(), next(), next()];
  const a = Math.sqrt(1 - u1);
  const b = Math.sqrt(u1);
  return quaternion(
    a * Math.sin(2 * Math.PI * u2),
    a * Math.cos(2 * Math.PI * u2),
    b * Math.sin(2 * Math.PI * u3),
    b * Math.cos(2 * Math.PI * u3),
  );
}

/** A body rotation from a quaternion, through its rows. */
function rotationOf(q: Quaternion): Rotation3 {
  return rotation3FromRows(rotationRows(q));
}

/** A column-major 4 × 4 applied to (p, 1), in `f64`: clip x, y and depth (w stays 1). */
function apply(m: ArrayLike<number>, p: Vec3): Vec3 {
  const at = (i: number): number => m[i] ?? Number.NaN;
  return {
    x: at(0) * p.x + at(4) * p.y + at(8) * p.z + at(12),
    y: at(1) * p.x + at(5) * p.y + at(9) * p.z + at(13),
    z: at(2) * p.x + at(6) * p.y + at(10) * p.z + at(14),
  };
}

/** A vector's components each rounded to `f32`. */
function fround3(v: Vec3): Vec3 {
  return { x: Math.fround(v.x), y: Math.fround(v.y), z: Math.fround(v.z) };
}

/** A body-fixed vector in the light's axes. */
function inLight(set: CascadeSet, v: Vec3): Vec3 {
  return { x: dot(v, set.basis.x), y: dot(v, set.basis.y), z: dot(v, set.basis.z) };
}

/** A body-fixed light-axes vector back in the body-fixed axes. */
function fromLight(set: CascadeSet, v: Vec3): Vec3 {
  return add(add(scale(set.basis.x, v.x), scale(set.basis.y, v.y)), scale(set.basis.z, v.z));
}

/** The zero vector. */
const ZERO_VEC: Vec3 = { x: 0, y: 0, z: 0 };

/** The direction 20° below the horizon over `up` towards `along`. */
function lookDown(up: Vec3, along: Vec3): Vec3 {
  const u = normalise(up);
  const horizontal = normalise(sub(along, scale(u, dot(along, u))));
  return add(scale(horizontal, Math.cos(20 * RAD)), scale(u, -Math.sin(20 * RAD)));
}

/** A camera `heightM` over a sphere of `radiusM` above `up`, looking 20° down towards `along`. */
function groundCamera(
  radiusM: number,
  heightM: number,
  up: Vec3,
  along: Vec3,
): Pick<CascadeInput, "cameraBodyFixedM" | "orientationBodyFixed" | "datumNormalBodyFixed"> {
  const u = normalise(up);
  const forward = lookDown(u, along);
  return {
    cameraBodyFixedM: scale(u, radiusM + heightM),
    orientationBodyFixed: lookAlong(forward, u),
    datumNormalBodyFixed: u,
  };
}

/** The up direction of the base cases: 28° N, 47° E, `TERRAIN TEST`'s survey. */
const BASE_UP: Vec3 = {
  x: Math.cos(28 * RAD) * Math.cos(47 * RAD),
  y: Math.cos(28 * RAD) * Math.sin(47 * RAD),
  z: Math.sin(28 * RAD),
};

/** The direction to a sun `elevationRad` up over `up`, towards the west of north. */
function sunOver(up: Vec3, elevationRad: number): Vec3 {
  const u = normalise(up);
  const east = normalise(cross({ x: 0, y: 0, z: 1 }, u));
  const north = cross(u, east);
  const towards = normalise(add(scale(east, -0.8), scale(north, 0.6)));
  return add(scale(towards, Math.cos(elevationRad)), scale(u, Math.sin(elevationRad)));
}

/** A base input on `TEST MOON`: h = 2 m, the sun 10° up, the horizon map's texel 1 m. */
function baseInput(overrides: Partial<CascadeInput> = {}): CascadeInput {
  return {
    ...groundCamera(MOON_RADIUS_M, 2, BASE_UP, { x: 0, y: 0, z: 1 }),
    rotation: rotationOf(quaternionFromAxisAngle({ x: 0.2, y: -0.3, z: 1 }, 0.7)),
    fovXRad: FOV_X_RAD,
    aspect: ASPECT,
    heightAboveGroundM: 2,
    sunDirectionBodyFixed: sunOver(BASE_UP, 10 * RAD),
    bodyRadiusM: MOON_RADIUS_M,
    reliefRangeM: MOON_RELIEF_M,
    horizonTexelM: 1,
    depthClipControl: true,
    ...overrides,
  };
}

/** Each split C₀ … C₄ at a height above the ground, the view's far infinite. */
function splitsAt(heightM: number): number[] {
  return [...cascadeSplitsM(0.1, shadowedRangeM(heightM))];
}

describe("the splits", () => {
  it("rise strictly from the near plane to the shadowed range", () => {
    for (const heightM of [0, 2, 100, 1_000, 4_000, 1e5, 1e8]) {
      const rangeM = shadowedRangeM(heightM);
      const splits = cascadeSplitsM(0.1, rangeM);
      expect(splits).toHaveLength(CASCADE_COUNT + 1);
      expect(splits[0]).toBe(0.1);
      expect(splits[CASCADE_COUNT]).toBe(rangeM);
      for (let i = 1; i <= CASCADE_COUNT; i += 1) {
        expect(splits[i]).toBeGreaterThan(splits[i - 1] ?? Number.NaN);
      }
    }
  });

  it("are non-decreasing in the height above the ground from 0 to 10⁸ m", () => {
    // h = 0, then 241 heights log-spaced over 10⁻³ … 10⁸ m.
    const heights = [0, ...Array.from({ length: 241 }, (_, k) => 10 ** (-3 + (11 * k) / 240))];
    const decreases: string[] = [];
    for (let k = 1; k < heights.length; k += 1) {
      const [before, after] = [splitsAt(heights[k - 1] ?? 0), splitsAt(heights[k] ?? 0)];
      for (let i = 0; i <= CASCADE_COUNT; i += 1) {
        if ((after[i] ?? Number.NaN) < (before[i] ?? Number.NaN)) {
          decreases.push(`C${i} at ${String(heights[k])} m`);
        }
      }
    }
    expect(decreases).toEqual([]);
  });

  // Below the kink f holds at 2 km, above it rises at 20 m a metre of height, and ∂Cᵢ/∂f is at
  // most i ÷ N, so the step across 100 ± δ is at most 20 δ i ÷ N: no jump, a slope. At the brief's
  // δ = 10⁻⁶ m that is 2 × 10⁻⁵ m for C₄ = f, so its bound of 10⁻⁶ m holds from δ = 10⁻⁸ m.
  it.each([1e-6, 1e-8])("are continuous across the kink at h = 100 m, δ = %s m", (deltaM) => {
    const [below, above] = [splitsAt(100 - deltaM), splitsAt(100 + deltaM)];
    for (let i = 0; i <= CASCADE_COUNT; i += 1) {
      const stepM = Math.abs((above[i] ?? Number.NaN) - (below[i] ?? Number.NaN));
      expect(stepM).toBeLessThanOrEqual((20 * deltaM * i) / CASCADE_COUNT + 1e-12);
    }
  });

  it("step by under 10⁻⁶ m across h = 100 ± 10⁻⁸ m", () => {
    const [below, above] = [splitsAt(100 - 1e-8), splitsAt(100 + 1e-8)];
    const steps = above.map((c, i) => Math.abs(c - (below[i] ?? Number.NaN)));
    expect(Math.max(...steps)).toBeLessThan(1e-6);
  });
});

/** The slice's eight corners at distances `nearM` and `farM` along −z, the camera's axes. */
function corners(nearM: number, farM: number, fovXRad: number, aspect: number): Vec3[] {
  const tanX = Math.tan(fovXRad / 2);
  const tanY = tanX / aspect;
  return [nearM, farM].flatMap((d) =>
    [-1, 1].flatMap((sx) => [-1, 1].map((sy) => ({ x: sx * d * tanX, y: sy * d * tanY, z: -d }))),
  );
}

/** The largest distance of the slice's corners from the sphere's centre, m. */
function farthestCornerM(nearM: number, farM: number, fovXRad: number, aspect: number): number {
  const t2 = halfDiagonalTanSquared(fovXRad, aspect);
  const centre = { x: 0, y: 0, z: -sliceSphere(nearM, farM, t2).distanceM };
  return Math.max(...corners(nearM, farM, fovXRad, aspect).map((c) => norm(sub(c, centre))));
}

describe("the spheres", () => {
  it("hold their slice's eight corners, touching the farthest", () => {
    const next = generator(0x5eed_0c5d);
    const cases: Array<readonly [number, number, number, number]> = [];
    for (let k = 0; k < 200; k += 1) {
      const nearM = 10 ** (-1 + 6 * next());
      const farM = nearM * (1 + 10 ** (-3 + 5 * next()));
      cases.push([nearM, farM, (5 + 170 * next()) * RAD, 0.25 + 3.75 * next()]);
    }
    cases.push([0.1, 125.91, FOV_X_RAD, ASPECT], [501.14, 2000, FOV_X_RAD, ASPECT]);
    for (const [nearM, farM, fovXRad, aspect] of cases) {
      const t2 = halfDiagonalTanSquared(fovXRad, aspect);
      const { radiusM } = sliceSphere(nearM, farM, t2);
      const farthestM = farthestCornerM(nearM, farM, fovXRad, aspect);
      expect(farthestM).toBeLessThanOrEqual(radiusM * (1 + 1e-9));
      expect(farthestM).toBeGreaterThanOrEqual(radiusM * (1 - 1e-9));
    }
  });

  it("have the closed form's radius, equidistant from all eight corners off the far cap", () => {
    const t2 = halfDiagonalTanSquared(FOV_X_RAD, ASPECT);
    const [nearM, farM] = [0.1, 125.91];
    const sphere = sliceSphere(nearM, farM, t2);
    const z = ((nearM + farM) * (1 + t2)) / 2;
    expect(sphere.distanceM).toBe(z);
    expect(sphere.radiusM).toBe(Math.sqrt((farM - z) ** 2 + farM ** 2 * t2));
    const centre = { x: 0, y: 0, z: -z };
    for (const corner of corners(nearM, farM, FOV_X_RAD, ASPECT)) {
      expect(norm(sub(corner, centre)) / sphere.radiusM).toBeCloseTo(1, 12);
    }
  });

  it("take the far cap's sphere where the centre would stand beyond the slice", () => {
    // Cascade 2 at h = 2 m: z = (125.91 + 260.62)(1 + t²) ÷ 2 = 278.1 m, beyond f′.
    const t2 = halfDiagonalTanSquared(FOV_X_RAD, ASPECT);
    const [nearM, farM] = [125.91, 260.62];
    expect(((nearM + farM) * (1 + t2)) / 2).toBeGreaterThan(farM);
    const sphere = sliceSphere(nearM, farM, t2);
    expect(sphere.distanceM).toBe(farM);
    expect(sphere.radiusM).toBe(farM * Math.sqrt(t2));
    // The far corners lie on it, and the near ones inside.
    const centre = { x: 0, y: 0, z: -farM };
    const distances = corners(nearM, farM, FOV_X_RAD, ASPECT).map((c) => norm(sub(c, centre)));
    for (const d of distances.slice(4)) {
      expect(d / sphere.radiusM).toBeCloseTo(1, 12);
    }
    for (const d of distances.slice(0, 4)) {
      expect(d).toBeLessThan(sphere.radiusM);
    }
  });
});

describe("the hand values", () => {
  // Figures worked by hand for φx 60°, aspect 16 ÷ 9 and n 0.1 m (plan R10's "Figures
  // (R10.T9.a)"), given to 10⁻² m and so held to half of it; texel₁ and texel₄ each to half a unit
  // of its last digit.
  const rows = [
    {
      heightM: 2,
      rangeM: 2_000,
      splitsM: [125.91, 260.62, 501.14, 2_000],
      radiiM: [90.55, 172.64, 331.97, 1_339.96],
      texelsM: { first: 0.0884, firstHalfUnit: 5e-5, last: 1.31, lastHalfUnit: 5e-3 },
    },
    {
      heightM: 100,
      rangeM: 2_000,
      splitsM: [125.91, 260.62, 501.14, 2_000],
      radiiM: [90.55, 172.64, 331.97, 1_339.96],
      texelsM: { first: 0.0884, firstHalfUnit: 5e-5, last: 1.31, lastHalfUnit: 5e-3 },
    },
    {
      heightM: 1_000,
      rangeM: 20_000,
      splitsM: [1_251.6, 2_533.55, 4_459.31, 20_000],
      radiiM: [900.38, 1_678.28, 2_953.94, 13_464.76],
      texelsM: { first: 0.88, firstHalfUnit: 5e-3, last: 13.15, lastHalfUnit: 5e-3 },
    },
    {
      heightM: 4_000,
      rangeM: 80_000,
      splitsM: [5_002.26, 10_067.09, 17_006.23, 80_000],
      radiiM: [3_598.6, 6_668.66, 11_265.29, 53_968.99],
      texelsM: { first: 3.51, firstHalfUnit: 5e-3, last: 52.7, lastHalfUnit: 5e-3 },
    },
  ];

  it.each(rows)("give the hand-worked splits, radii and texels at h = $heightM m", (row) => {
    const set = fitCascades(baseInput({ heightAboveGroundM: row.heightM }));
    expect(set.rangeM).toBe(row.rangeM);
    set.cascades.forEach((cascade, i) => {
      expect(Math.abs(cascade.farM - (row.splitsM[i] ?? Number.NaN))).toBeLessThanOrEqual(5e-3);
      expect(Math.abs(cascade.radiusM - (row.radiiM[i] ?? Number.NaN))).toBeLessThanOrEqual(5e-3);
    });
    const { first, firstHalfUnit, last, lastHalfUnit } = row.texelsM;
    expect(Math.abs(set.cascades[0].texelM - first)).toBeLessThanOrEqual(firstHalfUnit);
    expect(Math.abs(set.cascades[3].texelM - last)).toBeLessThanOrEqual(lastHalfUnit);
  });
});

/** A set's four snapped centres, to hand to the next frame. */
function centresOf(set: CascadeSet): readonly [Vec3, Vec3, Vec3, Vec3] {
  const [a, b, c, d] = set.cascades;
  return [a.centreBodyFixedM, b.centreBodyFixedM, c.centreBodyFixedM, d.centreBodyFixedM];
}

/** `x` less its nearest integer: a phase in texels wrapped to [−½, ½]. */
function wrapHalf(x: number): number {
  return x - Math.round(x);
}

/** The lattice point nearest `v` with spacing 2^⌈log₂(2 r₁)⌉, worked here from the ruling. */
function latticePointOf(v: Vec3, cascade1RadiusM: number): Vec3 {
  const spacingM = 2 ** Math.ceil(Math.log2(2 * cascade1RadiusM));
  const round = (x: number): number => spacingM * Math.floor(x / spacingM + 0.5);
  return { x: round(v.x), y: round(v.y), z: round(v.z) };
}

/** Each cascade's unsnapped centre, the camera plus its sphere's distance along the view axis. */
function unsnappedCentres(input: CascadeInput, set: CascadeSet): Vec3[] {
  const [r0, r1, r2] = rotationRows(input.orientationBodyFixed);
  const forward = { x: -r0.z, y: -r1.z, z: -r2.z };
  const t2 = halfDiagonalTanSquared(input.fovXRad, input.aspect);
  return set.cascades.map((cascade) =>
    add(
      input.cameraBodyFixedM,
      scale(forward, sliceSphere(cascade.nearM, cascade.farM, t2).distanceM),
    ),
  );
}

/** How far from whole texels of its own `(centre − origin)` lies in x_L and y_L, per cascade. */
function offGrid(set: CascadeSet, origins: ReadonlyArray<Vec3>): string[] {
  const misses: string[] = [];
  for (const [i, cascade] of set.cascades.entries()) {
    const origin = origins[i];
    if (origin === undefined) {
      throw new Error(`no origin for cascade ${String(i + 1)}`);
    }
    const step = inLight(set, sub(cascade.centreBodyFixedM, origin));
    for (const [axis, value] of [
      ["x", step.x],
      ["y", step.y],
    ] as const) {
      const texels = value / cascade.texelM;
      if (!(Math.abs(wrapHalf(texels)) <= 1e-6)) {
        misses.push(`cascade ${String(i + 1)} ${axis}: ${String(texels)} texels`);
      }
    }
  }
  return misses;
}

/** The base input with the camera `moveM` east along the ground on `radiusM`, turned `turnRad`. */
function movedInput(radiusM: number, moveM: number, turnRad: number): CascadeInput {
  const east = normalise(cross({ x: 0, y: 0, z: 1 }, BASE_UP));
  const up = rotateToBody(rotationOf(quaternionFromAxisAngle(east, moveM / radiusM)), BASE_UP);
  const look = rotateToBody(rotationOf(quaternionFromAxisAngle(up, turnRad)), {
    x: 0,
    y: 0,
    z: 1,
  });
  return baseInput({
    ...groundCamera(radiusM, 2, up, look),
    sunDirectionBodyFixed: sunOver(BASE_UP, 10 * RAD),
    bodyRadiusM: radiusM,
  });
}

describe("a carried grid", () => {
  const moves = [
    { move: "1 m along the ground", moveM: 1, turnRad: 0 },
    { move: "1 km along the ground", moveM: 1_000, turnRad: 0 },
    { move: "100 km along the ground", moveM: 100_000, turnRad: 0 },
    { move: "a 90° turn", moveM: 0, turnRad: Math.PI / 2 },
  ];
  const cases = [
    { body: "TEST MOON", radiusM: MOON_RADIUS_M },
    { body: "an Earth", radiusM: EARTH_RADIUS_M },
  ].flatMap(({ body, radiusM }) =>
    moves.map(({ move, moveM, turnRad }) => ({ body, radiusM, move, moveM, turnRad })),
  );

  it.each(cases)("never re-grids on $body after $move", ({ radiusM, moveM, turnRad }) => {
    // The same sun and height, so the same basis and texels.
    const before = fitCascades(movedInput(radiusM, 0, 0));
    const after = fitCascades({
      ...movedInput(radiusM, moveM, turnRad),
      previousCentresBodyFixedM: centresOf(before),
    });
    expect(after.cascades.map((c) => c.texelM)).toEqual(before.cascades.map((c) => c.texelM));
    expect(offGrid(after, centresOf(before))).toEqual([]);
  });

  it("slides a receiver by at most 0.075 texel a frame as TEST MOON's sun turns", () => {
    // TERRAIN TEST: R 1,737.4 km, the camera at 28° N and h = 2 m, the sun 30° up in the east;
    // between two frames it turns by δ = 2π ÷ 86,400 rad about the pole (one turn in 86,400 s at
    // 60 s/s and 60 Hz), the camera still and the centres carried. The bound is δ r ÷ texel,
    // 1,024 δ = 0.0745 texel.
    const east = normalise(cross({ x: 0, y: 0, z: 1 }, BASE_UP));
    const sun = add(scale(east, Math.cos(30 * RAD)), scale(BASE_UP, Math.sin(30 * RAD)));
    const input = baseInput({ sunDirectionBodyFixed: sun });
    const before = fitCascades(input);
    const turned = rotateToBody(
      rotationOf(quaternionFromAxisAngle({ x: 0, y: 0, z: 1 }, (-2 * Math.PI) / 86_400)),
      sun,
    );
    const after = fitCascades({
      ...input,
      sunDirectionBodyFixed: turned,
      previousCentresBodyFixedM: centresOf(before),
    });
    const [first, next] = [before.cascades[0], after.cascades[0]];
    const texelM = first.texelM;
    const random = generator(0x0001_e5e7);
    let worstCarriedTexels = 0;
    for (let k = 0; k < 500; k += 1) {
      const receiver = add(
        first.centreBodyFixedM,
        scale(randomUnit(random), first.radiusM * Math.cbrt(random())),
      );
      const was = inLight(before, sub(receiver, first.centreBodyFixedM));
      const is = inLight(after, sub(receiver, next.centreBodyFixedM));
      worstCarriedTexels = Math.max(
        worstCarriedTexels,
        Math.abs(wrapHalf((is.x - was.x) / texelM)),
        Math.abs(wrapHalf((is.y - was.y) / texelM)),
      );
    }
    // Measured at this seed: 0.073 texel.
    expect(worstCarriedTexels).toBeLessThanOrEqual(0.075);
  });

  it("differs from a grid from the body's centre, which slides by over 100 texels a frame", () => {
    // The brief's first S3 in the same frames and receivers: the scenario tells the grids apart.
    const east = normalise(cross({ x: 0, y: 0, z: 1 }, BASE_UP));
    const sun = add(scale(east, Math.cos(30 * RAD)), scale(BASE_UP, Math.sin(30 * RAD)));
    const turned = rotateToBody(
      rotationOf(quaternionFromAxisAngle({ x: 0, y: 0, z: 1 }, (-2 * Math.PI) / 86_400)),
      sun,
    );
    const first = fitCascades(baseInput({ sunDirectionBodyFixed: sun })).cascades[0];
    const [was, is] = [lightBasis(sun), lightBasis(turned)];
    const random = generator(0x0001_e5e7);
    let leastFromCentreTexels = Infinity;
    for (let k = 0; k < 500; k += 1) {
      const receiver = add(
        first.centreBodyFixedM,
        scale(randomUnit(random), first.radiusM * Math.cbrt(random())),
      );
      const slideXTexels = (dot(receiver, is.x) - dot(receiver, was.x)) / first.texelM;
      const slideYTexels = (dot(receiver, is.y) - dot(receiver, was.y)) / first.texelM;
      leastFromCentreTexels = Math.min(
        leastFromCentreTexels,
        Math.max(Math.abs(slideXTexels), Math.abs(slideYTexels)),
      );
    }
    // Measured at this seed: 573 texels at the least.
    expect(leastFromCentreTexels).toBeGreaterThan(100);
  });

  it("scales about its centre when the texel changes", () => {
    // h 1,000 → 1,000.1 m, the camera and sun still: each texel grows by ε ≈ 10⁻⁴, and a receiver
    // s from the carried centre along x_L or y_L moves against the grid by s ε, to rounding.
    const before = fitCascades(baseInput({ heightAboveGroundM: 1_000 }));
    const after = fitCascades(
      baseInput({ heightAboveGroundM: 1_000.1, previousCentresBodyFixedM: centresOf(before) }),
    );
    const random = generator(0x0000_91f0);
    let worstM = 0;
    for (const [i, was] of before.cascades.entries()) {
      const is = after.cascades[i];
      if (is === undefined) {
        throw new Error(`no cascade ${String(i + 1)} after`);
      }
      const epsilon = is.texelM / was.texelM - 1;
      expect(Math.abs(epsilon)).toBeGreaterThan(1e-6);
      for (let k = 0; k < 100; k += 1) {
        const receiver = add(
          was.centreBodyFixedM,
          scale(randomUnit(random), was.radiusM * Math.cbrt(random())),
        );
        const fromWas = inLight(before, sub(receiver, was.centreBodyFixedM));
        const fromIs = inLight(after, sub(receiver, is.centreBodyFixedM));
        for (const [s, sNow] of [
          [fromWas.x, fromIs.x],
          [fromWas.y, fromIs.y],
        ] as const) {
          const movedM = wrapHalf(s / was.texelM - sNow / is.texelM) * is.texelM;
          worstM = Math.max(worstM, Math.abs(movedM - s * epsilon));
        }
      }
    }
    // Measured at this seed: 8.8 × 10⁻¹¹ m.
    expect(worstM).toBeLessThan(1e-6);
  });
});

describe("snapping", () => {
  it.each([
    { path: "the lattice", carried: false },
    { path: "a carry", carried: true },
  ])(
    "puts each centre within half a texel of its unsnapped centre, at its depth: $path",
    ({ carried }) => {
      const elsewhere = fitCascades(movedInput(MOON_RADIUS_M, 1_000, 0.3));
      const input = carried
        ? { ...baseInput(), previousCentresBodyFixedM: centresOf(elsewhere) }
        : baseInput();
      const set = fitCascades(input);
      const unsnapped = unsnappedCentres(input, set);
      for (const [i, cascade] of set.cascades.entries()) {
        const off = inLight(set, sub(cascade.centreBodyFixedM, unsnapped[i] ?? ZERO_VEC));
        expect(Math.abs(off.x)).toBeLessThanOrEqual(cascade.texelM / 2 + 1e-8);
        expect(Math.abs(off.y)).toBeLessThanOrEqual(cascade.texelM / 2 + 1e-8);
        expect(Math.abs(off.z)).toBeLessThan(1e-8);
      }
    },
  );

  it("centres each sphere ahead of the camera along its look", () => {
    // The look as the camera was built, not as the module reads it from the orientation.
    const forward = lookDown(BASE_UP, { x: 0, y: 0, z: 1 });
    const input = baseInput();
    const set = fitCascades(input);
    const t2 = halfDiagonalTanSquared(FOV_X_RAD, ASPECT);
    for (const cascade of set.cascades) {
      const { distanceM } = sliceSphere(cascade.nearM, cascade.farM, t2);
      const ahead = add(input.cameraBodyFixedM, scale(forward, distanceM));
      expect(norm(sub(cascade.centreBodyFixedM, ahead))).toBeLessThanOrEqual(cascade.texelM);
    }
  });

  it("snaps a first frame by whole texels from the lattice point", () => {
    const input = baseInput();
    const set = fitCascades(input);
    const origin = latticePointOf(input.cameraBodyFixedM, set.cascades[0].radiusM);
    expect(offGrid(set, [origin, origin, origin, origin])).toEqual([]);
  });
});

describe("the lattice", () => {
  it.each([
    { heightM: 0, spacingM: 256 },
    { heightM: 2, spacingM: 256 },
    { heightM: 50, spacingM: 256 },
    { heightM: 100, spacingM: 256 },
    { heightM: 1_000, spacingM: 2_048 },
    { heightM: 4_000, spacingM: 8_192 },
  ])("is $spacingM m apart at h = $heightM m for a 60°, 16:9 view", ({ heightM, spacingM }) => {
    const radiusM = fitCascades(baseInput({ heightAboveGroundM: heightM })).cascades[0].radiusM;
    expect(gridLatticeSpacingM(radiusM)).toBe(spacingM);
  });

  it("is the power of two in [D₁, 2D₁)", () => {
    const random = generator(0x0000_1a77);
    const radii = [
      0.5,
      1,
      64,
      128,
      1_024,
      ...Array.from({ length: 200 }, () => 10 ** (6 * random())),
    ];
    const misses = radii.filter((radiusM) => {
      const spacingM = gridLatticeSpacingM(radiusM);
      return !(
        Number.isInteger(Math.log2(spacingM)) &&
        spacingM >= 2 * radiusM &&
        spacingM < 4 * radiusM
      );
    });
    expect(misses).toEqual([]);
  });

  it("serves all four cascades from the lattice point nearest the camera, across faces", () => {
    const misses = [0, 37, 129, 5_000].flatMap((moveM) => {
      const input = movedInput(MOON_RADIUS_M, moveM, 0);
      const set = fitCascades(input);
      const origin = latticePointOf(input.cameraBodyFixedM, set.cascades[0].radiusM);
      return offGrid(set, [origin, origin, origin, origin]);
    });
    expect(misses).toEqual([]);
  });

  it("puts the lattice point within L√3 ÷ 2 of the camera, by its formula", () => {
    const random = generator(0x0000_0b0c);
    const radiusM = fitCascades(baseInput()).cascades[0].radiusM;
    const boundM = (gridLatticeSpacingM(radiusM) * Math.sqrt(3)) / 2;
    const worstM = Math.max(
      ...Array.from({ length: 200 }, () => {
        const cameraM = scale(randomUnit(random), MOON_RADIUS_M + 1e4 * random());
        return norm(sub(latticePointOf(cameraM, radiusM), cameraM));
      }),
    );
    expect(worstM).toBeLessThanOrEqual(boundM);
  });

  it("gives the same outputs bit for bit whatever the order of calls", () => {
    const a = baseInput();
    const b = movedInput(EARTH_RADIUS_M, 2_500, 1);
    const firstA = fitCascades(a);
    const firstB = fitCascades(b);
    const carriedA = { ...a, previousCentresBodyFixedM: centresOf(firstB) };
    const firstCarried = fitCascades(carriedA);
    // Again, in another order.
    const againCarried = fitCascades(carriedA);
    const againB = fitCascades(b);
    const againA = fitCascades(a);
    expect([againA, againB, againCarried]).toEqual([firstA, firstB, firstCarried]);
  });
});

describe("the fitted radius", () => {
  it("does not change as the camera turns", () => {
    const next = generator(0x00c0_ffee);
    const reference = fitCascades(baseInput());
    for (let k = 0; k < 100; k += 1) {
      const set = fitCascades(baseInput({ orientationBodyFixed: randomQuaternion(next) }));
      set.cascades.forEach((cascade, i) => {
        expect(cascade.radiusM).toBe(reference.cascades[i]?.radiusM);
        expect(cascade.texelM).toBe(reference.cascades[i]?.texelM);
      });
    }
  });
});

describe("the cascades' weight", () => {
  /** The weight at h with the horizon map's texel at cascade 1's ÷ `ratio`. */
  function weightAt(heightM: number, ratio: number): number {
    const texelM = fitCascades(baseInput({ heightAboveGroundM: heightM })).cascades[0].texelM;
    return fitCascades(baseInput({ heightAboveGroundM: heightM, horizonTexelM: texelM / ratio }))
      .weight;
  }

  it("is 0 wherever cascade 1's texel is coarser than the horizon map's", () => {
    for (const heightM of [0, 2, 100, 1_000, 4_000, 1e6]) {
      for (const ratio of [1, 1 + 1e-9, 1.01, 1.5, 4, 1e3, 1e9]) {
        expect(weightAt(heightM, ratio)).toBe(0);
      }
    }
  });

  it("is 1 at half the horizon map's texel and below", () => {
    for (const heightM of [0, 2, 1_000]) {
      for (const ratio of [0.5, 0.4, 0.1, 1e-6]) {
        expect(weightAt(heightM, ratio)).toBe(1);
      }
    }
  });

  it("is linear and continuous between", () => {
    for (const ratio of [0.5, 0.55, 0.6, 0.75, 0.9, 0.99, 1]) {
      expect(cascadeWeight(ratio, 1)).toBeCloseTo(2 * (1 - ratio), 12);
    }
    for (const edge of [0.5, 1]) {
      expect(Math.abs(cascadeWeight(edge + 1e-9, 1) - cascadeWeight(edge - 1e-9, 1))).toBeLessThan(
        1e-8,
      );
    }
  });
});

/** Everything the set holds in the body-fixed or light axes, which no rotation should move. */
function bodyFixed(set: CascadeSet): unknown {
  return {
    rangeM: set.rangeM,
    splitsM: set.splitsM,
    weight: set.weight,
    unclippedDepth: set.unclippedDepth,
    casterReachM: set.casterReachM,
    sunElevationRad: set.sunElevationRad,
    basis: set.basis,
    // The projection is in the light's axes alone; the view rotation and the product hold R.
    cascades: set.cascades.map((c) => ({
      nearM: c.nearM,
      farM: c.farM,
      fadeStartM: c.fadeStartM,
      centreBodyFixedM: c.centreBodyFixedM,
      centreFromCameraLightM: c.centreFromCameraLightM,
      radiusM: c.radiusM,
      texelM: c.texelM,
      normalOffsetM: c.normalOffsetM,
      depthNearM: c.depthNearM,
      depthFarM: c.depthFarM,
      projection: c.projection,
    })),
  };
}

describe("the body's rotation", () => {
  it("moves nothing body-fixed", () => {
    const next = generator(0x0b0d_7f1e);
    const reference = fitCascades(baseInput({ rotation: IDENTITY_ROTATION }));
    for (let k = 0; k < 20; k += 1) {
      const set = fitCascades(baseInput({ rotation: rotationOf(randomQuaternion(next)) }));
      expect(bodyFixed(set)).toEqual(bodyFixed(reference));
    }
  });

  it("leaves a body-fixed point's light coordinates where they were, to f32 rounding", () => {
    const next = generator(0x0001_2345);
    const input = baseInput();
    const reference = fitCascades(baseInput({ rotation: IDENTITY_ROTATION }));
    const points = reference.cascades.flatMap((cascade) =>
      Array.from({ length: 8 }, () =>
        add(cascade.centreBodyFixedM, scale(randomUnit(next), cascade.radiusM * next())),
      ),
    );
    for (let k = 0; k < 10; k += 1) {
      const rotation = rotationOf(randomQuaternion(next));
      const set = fitCascades({ ...input, rotation });
      set.cascades.forEach((cascade, i) => {
        const before = reference.cascades[i];
        if (before === undefined) {
          throw new Error(`no cascade ${i + 1} in the reference`);
        }
        for (const point of points) {
          const relative = sub(point, input.cameraBodyFixedM);
          const was = apply(before.lightFromCamera, relative);
          const is = apply(cascade.lightFromCamera, rotateToBody(rotation, relative));
          // f32 entries: about 2⁻²⁴ of the largest term, |p| ÷ r ≲ 25 in clip units.
          expect(Math.abs(is.x - was.x)).toBeLessThan(5e-6);
          expect(Math.abs(is.y - was.y)).toBeLessThan(5e-6);
          expect(Math.abs(is.z - was.z)).toBeLessThan(5e-6);
        }
      });
    }
  });
});

describe("the matrices", () => {
  const input = baseInput();
  // Both capability paths: the near plane at r with depth-clip-control, r plus the reach without.
  const sets = [true, false].map((depthClipControl) => fitCascades({ ...input, depthClipControl }));
  /** The camera-relative, non-rotating position of an offset from a centre in the light's axes. */
  const fromCentre = (set: CascadeSet, cascade: Cascade, offset: Vec3): Vec3 =>
    rotateToBody(input.rotation, fromLight(set, add(cascade.centreFromCameraLightM, offset)));
  const variants = sets.flatMap((set) =>
    set.cascades.flatMap((cascade, i) => {
      const f64 = cascadeMatricesF64(
        set.basis,
        input.rotation,
        cascade.centreFromCameraLightM,
        cascade.radiusM,
        cascade.depthNearM,
        cascade.depthFarM,
      );
      const path = set.unclippedDepth ? "unclipped" : "pulled back";
      const f32 = {
        viewRotation: cascade.viewRotation,
        projection: cascade.projection,
        lightFromCamera: cascade.lightFromCamera,
      };
      return [
        { name: `cascade ${i + 1} in f64, ${path}`, set, cascade, m: f64, tolerance: 1e-12 },
        { name: `cascade ${i + 1} in f32, ${path}`, set, cascade, m: f32, tolerance: 2e-6 },
      ];
    }),
  );

  it.each(variants)(
    "map the sphere and depth range as stated: $name",
    ({ set, cascade, m, tolerance }) => {
      const r = cascade.radiusM;
      const centre = apply(m.lightFromCamera, fromCentre(set, cascade, { x: 0, y: 0, z: 0 }));
      expect(Math.abs(centre.x)).toBeLessThan(tolerance);
      expect(Math.abs(centre.y)).toBeLessThan(tolerance);
      for (const [offset, x, y] of [
        [{ x: r, y: 0, z: 0 }, 1, 0],
        [{ x: -r, y: 0, z: 0 }, -1, 0],
        [{ x: 0, y: r, z: 0 }, 0, 1],
        [{ x: 0, y: -r, z: 0 }, 0, -1],
      ] as const) {
        const clip = apply(m.lightFromCamera, fromCentre(set, cascade, offset));
        expect(Math.abs(clip.x - x)).toBeLessThan(tolerance);
        expect(Math.abs(clip.y - y)).toBeLessThan(tolerance);
      }
      const near = apply(
        m.lightFromCamera,
        fromCentre(set, cascade, { x: 0, y: 0, z: cascade.depthNearM }),
      );
      const far = apply(m.lightFromCamera, fromCentre(set, cascade, { x: 0, y: 0, z: -r }));
      expect(Math.abs(near.z - 1)).toBeLessThan(tolerance);
      expect(Math.abs(far.z)).toBeLessThan(tolerance);
    },
  );

  it.each(variants)(
    "form lightFromCamera as the projection after the view rotation: $name",
    ({ set, cascade, m, tolerance }) => {
      const r = cascade.radiusM;
      const p = fromCentre(set, cascade, { x: 0.3 * r, y: -0.2 * r, z: 0.1 * r });
      const viaBoth = apply(m.projection, apply(m.viewRotation, p));
      const viaProduct = apply(m.lightFromCamera, p);
      expect(norm(sub(viaBoth, viaProduct))).toBeLessThan(tolerance * 10);
    },
  );

  it.each(variants)("are right-handed, the light looking down −z_L: $name", ({ m, tolerance }) => {
    const v = m.viewRotation;
    const column = (c: number): Vec3 => ({
      x: v[c * 4] ?? Number.NaN,
      y: v[c * 4 + 1] ?? Number.NaN,
      z: v[c * 4 + 2] ?? Number.NaN,
    });
    expect(dot(column(0), cross(column(1), column(2)))).toBeCloseTo(1, 6);
    // The sun's direction, in the non-rotating axes, is the light's +z; moving towards it nears
    // the near plane, so depth rises towards the sun.
    const sun = rotateToBody(input.rotation, normalise(input.sunDirectionBodyFixed));
    const sunInLight = apply(v, sun);
    expect(Math.abs(sunInLight.x)).toBeLessThan(tolerance);
    expect(Math.abs(sunInLight.y)).toBeLessThan(tolerance);
    expect(Math.abs(sunInLight.z - 1)).toBeLessThan(tolerance);
    const towards = apply(m.lightFromCamera, sun);
    const origin = apply(m.lightFromCamera, { x: 0, y: 0, z: 0 });
    expect(towards.z).toBeGreaterThan(origin.z);
  });

  it.each(variants)("keep the view rotation's translation zero and its w 1: $name", ({ m }) => {
    for (const index of [3, 7, 11, 12, 13, 14]) {
      expect(m.viewRotation[index]).toBe(0);
    }
    expect(m.viewRotation[15]).toBe(1);
    for (const matrix of [m.projection, m.lightFromCamera]) {
      expect([matrix[3], matrix[7], matrix[11], matrix[15]]).toEqual([0, 0, 0, 1]);
    }
  });

  it.each(sets)(
    "narrow each entry once, from the f64 matrices: unclipped $unclippedDepth",
    (set) => {
      for (const cascade of set.cascades) {
        const f64 = cascadeMatricesF64(
          set.basis,
          input.rotation,
          cascade.centreFromCameraLightM,
          cascade.radiusM,
          cascade.depthNearM,
          cascade.depthFarM,
        );
        for (const [narrowed, wide] of [
          [cascade.viewRotation, f64.viewRotation],
          [cascade.projection, f64.projection],
          [cascade.lightFromCamera, f64.lightFromCamera],
        ] as const) {
          expect(narrowed).toBeInstanceOf(Float32Array);
          expect(narrowed).toHaveLength(16);
          expect([...narrowed]).toEqual([...wide].map(Math.fround));
        }
      }
    },
  );
});

describe("camera-relative precision", () => {
  it("holds clip x and y within 10⁻² texel at an Earth's radius, h = 2 m", () => {
    const next = generator(0x0000_6371);
    const input = baseInput({
      ...groundCamera(EARTH_RADIUS_M, 2, BASE_UP, { x: 0, y: 0, z: 1 }),
      bodyRadiusM: EARTH_RADIUS_M,
    });
    const set = fitCascades(input);
    const cascade = set.cascades[0];
    const f64 = cascadeMatricesF64(
      set.basis,
      input.rotation,
      cascade.centreFromCameraLightM,
      cascade.radiusM,
      cascade.depthNearM,
      cascade.depthFarM,
    );
    // The same transform formed from body-centred positions: the snapped centre measured from the
    // body's centre in its translation, applied to body-centred non-rotating positions.
    const bodyCentred = cascadeMatricesF64(
      set.basis,
      input.rotation,
      inLight(set, cascade.centreBodyFixedM),
      cascade.radiusM,
      cascade.depthNearM,
      cascade.depthFarM,
    ).lightFromCamera;
    const bodyCentredF32 = Float32Array.from(bodyCentred);
    const texelClip = 2 / CASCADE_SIZE_PX;
    let worstRelativeTexels = 0;
    let worstBodyCentredTexels = 0;
    for (let k = 0; k < 500; k += 1) {
      const point = add(
        cascade.centreBodyFixedM,
        scale(randomUnit(next), cascade.radiusM * Math.cbrt(next())),
      );
      const relative = rotateToBody(input.rotation, sub(point, input.cameraBodyFixedM));
      const exact = apply(f64.lightFromCamera, relative);
      const narrowed = apply(cascade.lightFromCamera, fround3(relative));
      worstRelativeTexels = Math.max(
        worstRelativeTexels,
        Math.abs(narrowed.x - exact.x) / texelClip,
        Math.abs(narrowed.y - exact.y) / texelClip,
      );
      const centred = apply(bodyCentredF32, fround3(rotateToBody(input.rotation, point)));
      worstBodyCentredTexels = Math.max(
        worstBodyCentredTexels,
        Math.abs(centred.x - exact.x) / texelClip,
        Math.abs(centred.y - exact.y) / texelClip,
      );
    }
    // Measured at this seed: 1.4 × 10⁻⁴ texel.
    expect(worstRelativeTexels).toBeLessThan(1e-2);
    // Body-centred, an f32 position at 6.371 × 10⁶ m lies on a 0.5 m grid, and the translation's
    // x and y entries here, about 3.2 × 10⁴ and 6.1 × 10⁴, on grids of 2⁻⁹ and 2⁻⁸ (clip units).
    // Measured at this seed: 4.6 of cascade 1's 8.84 cm texels, 0.40 m, the order of the split
    // ruling's 0.38 m.
    expect(worstBodyCentredTexels).toBeGreaterThan(4);
  });
});

/** How the basis for `sun` fails to be orthonormal and right-handed to 10⁻¹², z along it. */
function basisDefects(sun: Vec3): string[] {
  const { x, y, z } = lightBasis(sun);
  const defects: string[] = [];
  for (const [name, v] of [
    ["x", x],
    ["y", y],
    ["z", z],
  ] as const) {
    if (!(Math.abs(norm(v) - 1) < 1e-12)) {
      defects.push(`|${name}| = ${norm(v)}`);
    }
  }
  for (const [name, a, b] of [
    ["x · y", x, y],
    ["y · z", y, z],
    ["z · x", z, x],
  ] as const) {
    if (!(Math.abs(dot(a, b)) < 1e-12)) {
      defects.push(`${name} = ${dot(a, b)}`);
    }
  }
  if (!(norm(sub(cross(x, y), z)) < 1e-12)) {
    defects.push("x × y is not z");
  }
  if (!(norm(sub(z, normalise(sun))) < 1e-12)) {
    defects.push("z is not along the sun");
  }
  return defects;
}

describe("the light's axes", () => {
  it("are orthonormal and right-handed, z along the sun", () => {
    const next = generator(0x0000_5a17);
    const defects = Array.from({ length: 200 }, () =>
      basisDefects(scale(randomUnit(next), 10 ** (-3 + 6 * next()))),
    ).flat();
    expect(defects).toEqual([]);
  });

  it("put x along the pole's projection normal to the sun", () => {
    const sun = sunOver(BASE_UP, 10 * RAD);
    const { x, z } = lightBasis(sun);
    const projected = normalise(sub({ x: 0, y: 0, z: 1 }, scale(z, z.z)));
    expect(norm(sub(x, projected))).toBeLessThan(1e-15);
  });

  it("are continuous in the sun away from the poles", () => {
    const next = generator(0x0000_c0de);
    let checked = 0;
    while (checked < 200) {
      const sun = randomUnit(next);
      if (Math.acos(Math.abs(sun.z)) < 2 * RAD) {
        continue;
      }
      const axis = normalise(cross(sun, randomUnit(next)));
      const moved = rotateToBody(rotationOf(quaternionFromAxisAngle(axis, 1e-6)), sun);
      expect(norm(sub(lightBasis(moved).x, lightBasis(sun).x))).toBeLessThan(1e-5);
      checked += 1;
    }
  });

  it.each([
    { pole: "north", sign: 1 },
    { pole: "south", sign: -1 },
  ])("switch to the body's +x within 1° of the $pole pole, valid on both sides", ({ sign }) => {
    for (const azimuthDeg of [0, 37, 90, 200]) {
      const at = (fromPoleRad: number): Vec3 => ({
        x: Math.sin(fromPoleRad) * Math.cos(azimuthDeg * RAD),
        y: Math.sin(fromPoleRad) * Math.sin(azimuthDeg * RAD),
        z: sign * Math.cos(fromPoleRad),
      });
      const inside = at(POLE_FALLBACK_RAD * (1 - 1e-6));
      const outside = at(POLE_FALLBACK_RAD * (1 + 1e-6));
      expect([...basisDefects(inside), ...basisDefects(outside)]).toEqual([]);
      const xInside = lightBasis(inside).x;
      const xOutside = lightBasis(outside).x;
      const zInside = normalise(inside);
      const zOutside = normalise(outside);
      const plusX = normalise(sub({ x: 1, y: 0, z: 0 }, scale(zInside, zInside.x)));
      const pole = normalise(sub({ x: 0, y: 0, z: 1 }, scale(zOutside, zOutside.z)));
      expect(norm(sub(xInside, plusX))).toBeLessThan(1e-12);
      expect(norm(sub(xOutside, pole))).toBeLessThan(1e-12);
    }
  });
});

describe("the caster reach and the capability", () => {
  it("gives the split ruling's TERRAIN TEST figures", () => {
    const capM = Math.sqrt(2 * MOON_RADIUS_M * MOON_RELIEF_M);
    expect(Math.abs(capM - 227_500)).toBeLessThan(50);
    // Δh ÷ sin e★: 853.8 km at 1° (beyond the cap), 171.0, 85.8 and 29.8 km at 5°, 10° and 30°.
    expect(Math.abs(MOON_RELIEF_M / Math.sin(RAD) - 853_800)).toBeLessThan(50);
    for (const [elevationDeg, reachM] of [
      [1, 227_500],
      [5, 171_000],
      [10, 85_800],
      [30, 29_800],
    ] as const) {
      expect(
        Math.abs(casterReachM(elevationDeg * RAD, MOON_RELIEF_M, MOON_RADIUS_M) - reachM),
      ).toBeLessThan(50);
    }
  });

  it("is the cap with the sun on or below the horizon", () => {
    const capM = Math.sqrt(2 * MOON_RADIUS_M * MOON_RELIEF_M);
    for (const elevationRad of [0, -0, -1e-9, -0.1, -Math.PI / 2]) {
      expect(casterReachM(elevationRad, MOON_RELIEF_M, MOON_RADIUS_M)).toBe(capM);
    }
  });

  it("takes a caster's height over flat ground", () => {
    const casterHeightM = 40;
    const set = fitCascades(baseInput({ reliefRangeM: 0, casterHeightM }));
    expect(set.casterReachM).toBe(
      Math.min(
        casterHeightM / Math.sin(set.sunElevationRad),
        Math.sqrt(2 * MOON_RADIUS_M * casterHeightM),
      ),
    );
    expect(set.casterReachM).toBeCloseTo(casterHeightM / Math.sin(10 * RAD), 6);
    expect(fitCascades(baseInput({ reliefRangeM: 0 })).casterReachM).toBe(0);
  });

  it("pancakes with depth-clip-control and pulls the near plane back without it", () => {
    const withClip = fitCascades(baseInput({ depthClipControl: true }));
    expect(withClip.unclippedDepth).toBe(true);
    for (const cascade of withClip.cascades) {
      expect(cascade.depthNearM).toBe(cascade.radiusM);
      expect(cascade.depthFarM).toBe(cascade.radiusM);
    }
    const without = fitCascades(baseInput({ depthClipControl: false }));
    expect(without.unclippedDepth).toBe(false);
    expect(without.casterReachM).toBeGreaterThan(0);
    for (const cascade of without.cascades) {
      expect(cascade.depthNearM).toBe(cascade.radiusM + without.casterReachM);
      expect(cascade.depthFarM).toBe(cascade.radiusM);
    }
  });

  it("measures the sun's elevation from the datum's normal", () => {
    expect(sunElevationRad(sunOver(BASE_UP, 10 * RAD), BASE_UP)).toBeCloseTo(10 * RAD, 14);
    expect(sunElevationRad(sunOver(BASE_UP, -3 * RAD), scale(BASE_UP, 7))).toBeCloseTo(
      -3 * RAD,
      14,
    );
  });
});

describe("the normal offset", () => {
  it("is one and a half texels in each cascade, R10.T9.b's tuning", () => {
    expect(NORMAL_OFFSET_TEXELS).toBe(1.5);
    for (const heightM of [2, 1_000]) {
      for (const cascade of fitCascades(baseInput({ heightAboveGroundM: heightM })).cascades) {
        expect(cascade.normalOffsetM).toBe(1.5 * cascade.texelM);
      }
    }
  });
});

/** The angle between two directions, rad, by atan2 so that it keeps its precision when small. */
function angleRad(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

/** A matcher of `v`, each component to 10⁻¹⁵. */
function nearVec(v: Vec3): unknown {
  return {
    x: expect.closeTo(v.x, 15),
    y: expect.closeTo(v.y, 15),
    z: expect.closeTo(v.z, 15),
  };
}

describe("the disc's top edge (decision-r10-t9b-penumbra)", () => {
  /** The Sun's angular radius from 1 au, rad: asin(R☉ ÷ au) (IAU 2015 B3, IAU 2012 B2). */
  const RHO = Math.asin(6.957e8 / 1.495978707e11);

  it.each([-0.2, 2, 45])(
    "stands ρ from the sun, of unit length, its elevation e★ + ρ in the sun's azimuth, at e★ = %f°",
    (elevationDeg) => {
      const sun = sunOver(BASE_UP, elevationDeg * RAD);
      const top = discTopDirection(scale(sun, 3), scale(BASE_UP, 0.5), RHO);
      // The azimuth: the horizontal parts of the two point the same way.
      const level = (v: Vec3): Vec3 => normalise(sub(v, scale(BASE_UP, dot(v, BASE_UP))));
      expect([
        angleRad(top, sun),
        norm(top),
        sunElevationRad(top, BASE_UP),
        dot(level(top), level(sun)),
      ]).toEqual([
        expect.closeTo(RHO, 12),
        expect.closeTo(1, 14),
        expect.closeTo(elevationDeg * RAD + RHO, 12),
        expect.closeTo(1, 12),
      ]);
    },
  );

  it("lies in the plane of the sun and the datum's normal, on the normal's side of the sun", () => {
    const sun = sunOver(BASE_UP, 10 * RAD);
    const top = discTopDirection(sun, BASE_UP, RHO);
    const planeNormal = normalise(cross(sun, BASE_UP));
    // On the normal's side: nearer the normal than the sun, by ρ.
    expect([dot(top, planeNormal), angleRad(top, BASE_UP)]).toEqual([
      expect.closeTo(0, 14),
      expect.closeTo(angleRad(sun, BASE_UP) - RHO, 12),
    ]);
  });

  it("is the sun where the sun stands along the datum's normal, at the zenith or the nadir", () => {
    expect([
      discTopDirection(scale(BASE_UP, 2), BASE_UP, RHO),
      discTopDirection(scale(BASE_UP, -1), BASE_UP, RHO),
    ]).toEqual([nearVec(BASE_UP), nearVec(scale(BASE_UP, -1))]);
  });

  it("is the sun at ρ = 0, and finite for a grazing sun and a disc of a quarter turn", () => {
    const sun = sunOver(BASE_UP, 10 * RAD);
    const grazing = sunOver(BASE_UP, 0);
    const numbers = [
      discTopDirection(grazing, BASE_UP, RHO),
      discTopDirection(grazing, BASE_UP, Math.PI / 2),
      discTopDirection(sunOver(BASE_UP, -89.999 * RAD), BASE_UP, RHO),
    ].flatMap(({ x, y, z }) => [x, y, z]);
    expect(discTopDirection(sun, BASE_UP, 0)).toEqual(sun);
    expect(numbers.filter((n) => !Number.isFinite(n))).toEqual([]);
    expect(sunElevationRad(discTopDirection(grazing, BASE_UP, RHO), BASE_UP)).toBeCloseTo(RHO, 12);
  });

  it("refuses a sun or a normal of zero length, and a ρ outside [0, π ÷ 2]", () => {
    const sun = sunOver(BASE_UP, 10 * RAD);
    const zero = { x: 0, y: 0, z: 0 };
    for (const call of [
      () => discTopDirection(zero, BASE_UP, RHO),
      () => discTopDirection(sun, zero, RHO),
      () => discTopDirection(sun, BASE_UP, -1e-3),
      () => discTopDirection(sun, BASE_UP, Number.NaN),
      () => discTopDirection(sun, BASE_UP, 2),
    ]) {
      expect(call).toThrow(RangeError);
    }
  });
});

describe("the layers", () => {
  it("take 67,108,864 B by R01's texel arithmetic", () => {
    const { width, height, depthOrArrayLayers } = extentOf(CASCADE_TEXTURE.size);
    const bytes = width * height * depthOrArrayLayers * bytesPerTexel(CASCADE_TEXTURE.format);
    expect(bytes).toBe(67_108_864);
    expect(CASCADE_BYTES).toBe(bytes);
  });

  it("are four 2,048² depth32float layers of one mip", () => {
    expect(extentOf(CASCADE_TEXTURE.size)).toEqual({
      width: 2048,
      height: 2048,
      depthOrArrayLayers: 4,
    });
    expect(CASCADE_TEXTURE).toMatchObject({ dimension: "2d", format: "depth32float", mips: 1 });
  });

  it("are drawn as attachments and sampled", () => {
    expect(CASCADE_TEXTURE.usage).toBe(
      TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
    );
  });

  it("are filed under R12's shadows category", () => {
    expect(CASCADE_MEMORY_CATEGORY).toBe("shadows");
  });
});

describe("the fade", () => {
  it("starts each cascade's blend 90% of the way through its range", () => {
    expect(CASCADE_FADE_FRACTION).toBe(0.1);
    for (const cascade of fitCascades(baseInput()).cascades) {
      expect(cascade.fadeStartM).toBeCloseTo(
        cascade.nearM + 0.9 * (cascade.farM - cascade.nearM),
        9,
      );
    }
  });
});

describe("the bias", () => {
  it("is away from the light, its constant a WebGPU integer depthBias", () => {
    expect(Number.isInteger(CASCADE_DEPTH_BIAS.constant)).toBe(true);
    expect(CASCADE_DEPTH_BIAS.constant).toBeGreaterThan(0);
    expect(CASCADE_DEPTH_BIAS.slopeScale).toBeGreaterThan(0);
    expect(CASCADE_DEPTH_BIAS.clamp).toBeGreaterThanOrEqual(0);
  });
});

describe("refusals", () => {
  const zero = ZERO_VEC;
  const finite: Vec3 = { x: 1, y: 2, z: 3 };
  it.each<{ readonly name: string; readonly overrides: Partial<CascadeInput> }>([
    {
      name: "a previous centre that is not finite",
      overrides: {
        previousCentresBodyFixedM: [finite, finite, { x: 0, y: Number.NaN, z: 0 }, finite],
      },
    },
    {
      name: "a camera that is not finite",
      overrides: { cameraBodyFixedM: { x: Number.NaN, y: 0, z: 1 } },
    },
    {
      name: "an orientation that is not finite",
      overrides: { orientationBodyFixed: { w: Number.NaN, x: 0, y: 0, z: 0 } },
    },
    { name: "a height that is not finite", overrides: { heightAboveGroundM: Infinity } },
    { name: "a far plane that is not a number", overrides: { farM: Number.NaN } },
    { name: "a far plane at −∞", overrides: { farM: -Infinity } },
    {
      name: "a sun that is not finite",
      overrides: { sunDirectionBodyFixed: { x: Infinity, y: 0, z: 0 } },
    },
    { name: "a sun of zero length", overrides: { sunDirectionBodyFixed: zero } },
    { name: "a datum normal of zero length", overrides: { datumNormalBodyFixed: zero } },
    { name: "a field of view of 0", overrides: { fovXRad: 0 } },
    { name: "a field of view of π", overrides: { fovXRad: Math.PI } },
    { name: "an aspect of 0", overrides: { aspect: 0 } },
    { name: "a near plane of 0", overrides: { nearM: 0 } },
    { name: "a body radius of 0", overrides: { bodyRadiusM: 0 } },
    { name: "a horizon texel of 0", overrides: { horizonTexelM: 0 } },
    { name: "a horizon texel that is not finite", overrides: { horizonTexelM: Infinity } },
    { name: "a negative relief range", overrides: { reliefRangeM: -1 } },
    { name: "a negative caster height", overrides: { casterHeightM: -1 } },
    { name: "a far plane not beyond the near", overrides: { nearM: 0.1, farM: 0.1 } },
  ])("refuse $name", ({ overrides }) => {
    expect(() => fitCascades(baseInput(overrides))).toThrow(RangeError);
  });

  it("refuse previous centres that are not four", () => {
    const three: ReadonlyArray<Vec3> = [finite, finite, finite];
    const input = { ...baseInput(), previousCentresBodyFixedM: three };
    expect(() =>
      fitCascades(
        // @ts-expect-error -- three centres, which the type forbids, are refused at run time too.
        input,
      ),
    ).toThrow(RangeError);
  });

  const t2 = halfDiagonalTanSquared(FOV_X_RAD, ASPECT);
  const basis = lightBasis({ x: 0, y: 0.6, z: 0.8 });
  it.each<readonly [string, () => unknown]>([
    ["a shadowed range with a far plane of 0", () => shadowedRangeM(2, 0)],
    ["a shadowed range at a height that is not a number", () => shadowedRangeM(Number.NaN)],
    ["splits whose range is not beyond the near plane", () => cascadeSplitsM(0.1, 0.1)],
    ["splits from a near plane of 0", () => cascadeSplitsM(0, 10)],
    ["t² for a field of view of π", () => halfDiagonalTanSquared(Math.PI, ASPECT)],
    ["t² for a negative aspect", () => halfDiagonalTanSquared(FOV_X_RAD, -1)],
    ["a sphere whose slice ends are out of order", () => sliceSphere(5, 3, t2)],
    ["a sphere for a t² of 0", () => sliceSphere(1, 2, 0)],
    ["a basis for a sun of zero length", () => lightBasis(zero)],
    ["an elevation over a normal of zero length", () => sunElevationRad(finite, zero)],
    ["a reach at an elevation beyond π ÷ 2", () => casterReachM(2, 1, 1)],
    ["a reach over a negative relief", () => casterReachM(0.1, -1, 1)],
    ["a reach on a body of radius 0", () => casterReachM(0.1, 1, 0)],
    ["a weight for a horizon texel of 0", () => cascadeWeight(1, 0)],
    ["a weight for a cascade texel that is not finite", () => cascadeWeight(Infinity, 1)],
    [
      "matrices for a radius of 0",
      () => cascadeMatricesF64(basis, IDENTITY_ROTATION, zero, 0, 1, 1),
    ],
    [
      "matrices for a far depth of 0",
      () => cascadeMatricesF64(basis, IDENTITY_ROTATION, zero, 1, 1, 0),
    ],
    ["a lattice for a radius of 0", () => gridLatticeSpacingM(0)],
  ])("refuse %s", (_name, call) => {
    expect(call).toThrow(RangeError);
  });
});

/** A vector's three components. */
function components(v: Vec3): number[] {
  return [v.x, v.y, v.z];
}

/** Every number a set holds, flattened. */
function numbersOf(set: CascadeSet): number[] {
  return [
    set.rangeM,
    ...set.splitsM,
    set.weight,
    set.casterReachM,
    set.sunElevationRad,
    ...components(set.basis.x),
    ...components(set.basis.y),
    ...components(set.basis.z),
    ...set.cascades.flatMap((c) => [
      c.nearM,
      c.farM,
      c.fadeStartM,
      ...components(c.centreBodyFixedM),
      ...components(c.centreFromCameraLightM),
      c.radiusM,
      c.texelM,
      c.normalOffsetM,
      c.depthNearM,
      c.depthFarM,
      ...c.viewRotation,
      ...c.projection,
      ...c.lightFromCamera,
    ]),
  ];
}

describe("degenerate ground", () => {
  it.each([
    { name: "h = 0", overrides: { heightAboveGroundM: 0 } },
    { name: "h below the ground", overrides: { heightAboveGroundM: -3 } },
    { name: "h = 10⁸ m", overrides: { heightAboveGroundM: 1e8 } },
    { name: "an infinite far", overrides: { farM: Infinity } },
    { name: "no relief", overrides: { reliefRangeM: 0, depthClipControl: false } },
    { name: "the sun at the zenith", overrides: { sunDirectionBodyFixed: BASE_UP } },
  ])("give finite outputs: $name", ({ overrides }) => {
    const numbers = numbersOf(fitCascades(baseInput(overrides)));
    expect(numbers.filter((n) => !Number.isFinite(n))).toEqual([]);
  });

  it("takes a finite far below 2 km as the shadowed range", () => {
    const set = fitCascades(baseInput({ farM: 500 }));
    expect(set.rangeM).toBe(500);
    expect(set.splitsM[CASCADE_COUNT]).toBe(500);
    expect(set.cascades[3].farM).toBe(500);
    expect(shadowedRangeM(2, 500)).toBe(500);
  });

  it("takes a height below the ground as 0", () => {
    expect(shadowedRangeM(-5)).toBe(shadowedRangeM(0));
    expect(fitCascades(baseInput({ heightAboveGroundM: -5 }))).toEqual(
      fitCascades(baseInput({ heightAboveGroundM: 0 })),
    );
  });
});
