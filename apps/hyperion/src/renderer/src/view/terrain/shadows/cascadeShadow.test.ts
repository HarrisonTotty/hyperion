import { describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, sub, type Vec3 } from "../../../geometry/vec3";
import { lookAlong, quaternionFromAxisAngle, rotationRows } from "../../camera/quaternion";
import { rotateToBody, type Rotation3, rotation3FromRows } from "../../coords/rotation";
import {
  CASCADE_FILTER_TAPS,
  CASCADE_FOOTPRINT_TEXELS,
  CASCADE_TAP_RADIUS_TEXELS,
  type CascadeIndex,
  type CascadeUniforms,
  CASCADE_UNIFORM_SPECS,
  cascadeAt,
  cascadeBlend,
  cascadeLitFraction,
  cascadeMapPoint,
  cascadeReceiverLit,
  cascadeTerm,
  cascadeUniforms,
  coversWithFootprint,
  directLitShare,
  packCascadeUniforms,
  receiverOffsetM,
  weightedCascadeLit,
} from "./cascadeShadow";
import cascadeShadowWgsl from "./cascadeShadow.wgsl?raw";
import {
  CASCADE_COUNT,
  CASCADE_FADE_FRACTION,
  CASCADE_SIZE_PX,
  type CascadeInput,
  type CascadeSet,
  fitCascades,
} from "./cascades";

const RAD = Math.PI / 180;

/** `TERRAIN TEST`'s `TEST MOON`: its mean radius and relief range, m (as `cascades.test.ts`). */
const MOON_RADIUS_M = 1_737_400;
const MOON_RELIEF_M = 14_900;

/** 28° N, 47° E, `TERRAIN TEST`'s survey. */
const UP: Vec3 = normalise({
  x: Math.cos(28 * RAD) * Math.cos(47 * RAD),
  y: Math.cos(28 * RAD) * Math.sin(47 * RAD),
  z: Math.sin(28 * RAD),
});

/** The body's rotation of the cases: a turn about a tilted axis, so that no axes coincide. */
const ROTATION: Rotation3 = rotation3FromRows(
  rotationRows(quaternionFromAxisAngle({ x: 0.2, y: -0.3, z: 1 }, 0.7)),
);

/** The direction 20° below the horizon towards the north, body-fixed. */
const FORWARD_BF: Vec3 = (() => {
  const north = normalise(sub({ x: 0, y: 0, z: 1 }, scale(UP, UP.z)));
  return add(scale(north, Math.cos(20 * RAD)), scale(UP, -Math.sin(20 * RAD)));
})();

/** The sun 10° up towards the west of north, body-fixed. */
const SUN_BF: Vec3 = (() => {
  const east = normalise(cross({ x: 0, y: 0, z: 1 }, UP));
  const north = cross(UP, east);
  const towards = normalise(add(scale(east, -0.8), scale(north, 0.6)));
  return add(scale(towards, Math.cos(10 * RAD)), scale(UP, Math.sin(10 * RAD)));
})();

/** A camera 2 m over `TEST MOON` looking 20° down, the sun 10° up, the map's texel 1 m. */
const INPUT: CascadeInput = {
  cameraBodyFixedM: scale(UP, MOON_RADIUS_M + 2),
  orientationBodyFixed: lookAlong(FORWARD_BF, UP),
  datumNormalBodyFixed: UP,
  rotation: ROTATION,
  fovXRad: 60 * RAD,
  aspect: 16 / 9,
  heightAboveGroundM: 2,
  sunDirectionBodyFixed: SUN_BF,
  bodyRadiusM: MOON_RADIUS_M,
  reliefRangeM: MOON_RELIEF_M,
  horizonTexelM: 1,
  depthClipControl: true,
};

/** The fitted set of {@link INPUT}, with the cascades fully on. */
const SET: CascadeSet = fitCascades(INPUT);

/** The view axis in the body's non-rotating axes, as the session hands it to the packing. */
const VIEW_AXIS_NR: Vec3 = rotateToBody(ROTATION, FORWARD_BF);

/** A body-fixed vector in the non-rotating axes. */
function nonRotating(v: Vec3): Vec3 {
  return rotateToBody(ROTATION, v);
}

/** {@link SET} packed. */
function packed(): CascadeUniforms {
  const out = cascadeUniforms();
  packCascadeUniforms(SET, VIEW_AXIS_NR, out);
  return out;
}

/** Cascade i's snapped centre less the camera, in the body's non-rotating axes, m. */
function centreFromCamera(i: CascadeIndex): Vec3 {
  return nonRotating(sub(SET.cascades[i].centreBodyFixedM, INPUT.cameraBodyFixedM));
}

/** The unit view axis, non-rotating. */
const AXIS_NR: Vec3 = normalise(VIEW_AXIS_NR);

/**
 * A receiver on the view axis halfway between cascade i's near end and its fade start, camera-
 * relative and non-rotating: inside its slice and its map, before its blend. (Its sphere's centre
 * will not do: cascades 2 and 3 take the far cap's sphere, centred at their far ends.)
 */
function midSlice(i: CascadeIndex): Vec3 {
  const cascade = SET.cascades[i];
  return scale(AXIS_NR, 0.5 * (cascade.nearM + cascade.fadeStartM));
}

/** C₁ … C₄ and the fade starts of {@link SET}. */
const FAR_M = SET.cascades.map((c) => c.farM);
const FADE_M = SET.cascades.map((c) => c.fadeStartM);

/** Coverings for the fallback: every map, all but the first's, the last's or the first's alone. */
const coversAll = (): boolean => true;
const coversNotFirst = (j: CascadeIndex): boolean => j !== 0;
const coversOnlyLast = (j: CascadeIndex): boolean => j === 3;
const coversOnlyFirst = (j: CascadeIndex): boolean => j === 0;

/** A compare that fails the test if it is called. */
function refuseSample(): number {
  throw new Error("sampled at w = 0");
}

/** Lit fractions told apart by cascade: 0.1, 0.3, 0.6 and 0.8. */
const BY_CASCADE: readonly [number, number, number, number] = [0.1, 0.3, 0.6, 0.8];

/** A compare that gives each cascade its {@link BY_CASCADE} value everywhere. */
function constantCompare(cascade: CascadeIndex): number {
  return BY_CASCADE[cascade];
}

describe("the cascade a receiver falls in", () => {
  it("is the first whose far end its distance along the view axis does not pass, none beyond C₄", () => {
    const [c1 = 0, c2 = 0, c3 = 0, c4 = 0] = FAR_M;
    expect(
      [0.1, c1 / 2, c1, c1 + 1e-6, c2, (c2 + c3) / 2, c4, c4 * (1 + 1e-9)].map((d) =>
        cascadeAt(d, FAR_M),
      ),
    ).toEqual([0, 0, 0, 1, 1, 2, 3, null]);
  });

  it("blends into the next linearly over the last tenth of its range", () => {
    const cascade = SET.cascades[1];
    const span = cascade.farM - cascade.nearM;
    expect(cascade.fadeStartM).toBeCloseTo(cascade.farM - CASCADE_FADE_FRACTION * span, 9);
    const at = (share: number): number =>
      cascadeBlend(cascade.nearM + share * span, cascade.fadeStartM, cascade.farM);
    expect([0, 0.5, 0.9, 0.925, 0.95, 1].map((share) => at(share))).toEqual(
      [0, 0, 0, 0.25, 0.5, 1].map((t) => expect.closeTo(t, 9)),
    );
  });
});

describe("the cascades' lit fraction c", () => {
  const litAt = (d: number): number =>
    cascadeLitFraction(d, FAR_M, FADE_M, coversAll, constantCompare);

  it("is each cascade's own before its fade starts", () => {
    expect(SET.cascades.map((c) => litAt((c.nearM + c.fadeStartM) / 2))).toEqual(BY_CASCADE);
  });

  it.each([1, 2, 3])(
    "is continuous through split C%i, taking the next cascade's value there",
    (n) => {
      const i = n - 1;
      const split = FAR_M[i] ?? Number.NaN;
      const next = BY_CASCADE[n] ?? Number.NaN;
      // The band's slope bounds the step across the split: |Δc| ÷ (band width) a metre.
      const slope = Math.abs(next - (BY_CASCADE[i] ?? Number.NaN)) / (split - (FADE_M[i] ?? 0));
      for (const deltaM of [1e-2, 1e-4, 1e-6]) {
        expect(Math.abs(litAt(split - deltaM) - litAt(split + deltaM))).toBeLessThanOrEqual(
          slope * deltaM + 1e-12,
        );
      }
      expect(litAt(split)).toBeCloseTo(next, 12);
      expect(litAt(split * (1 + 1e-9))).toBe(next);
    },
  );

  it("fades the last cascade to lit at C₄, and is lit beyond it", () => {
    const c4 = FAR_M[3] ?? Number.NaN;
    const slope = (1 - BY_CASCADE[3]) / (c4 - (FADE_M[3] ?? 0));
    expect(litAt(c4)).toBeCloseTo(1, 12);
    for (const deltaM of [1e-2, 1e-4, 1e-6]) {
      expect(1 - litAt(c4 - deltaM)).toBeLessThanOrEqual(slope * deltaM + 1e-12);
      expect(litAt(c4 + deltaM)).toBe(1);
    }
    expect(litAt(1e12)).toBe(1);
  });

  it("mixes the two cascades by the blend inside the band", () => {
    const cascade = SET.cascades[0];
    const d = (cascade.fadeStartM + cascade.farM) / 2;
    expect(litAt(d)).toBeCloseTo(0.5 * (BY_CASCADE[0] + BY_CASCADE[1]), 12);
  });

  it("falls to the next covering cascade where its own map does not cover the receiver, without the blend", () => {
    const cascade = SET.cascades[0];
    const inBand = (cascade.fadeStartM + cascade.farM) / 2;
    const near = cascade.nearM + 1;
    expect(cascadeLitFraction(near, FAR_M, FADE_M, coversNotFirst, constantCompare)).toBe(
      BY_CASCADE[1],
    );
    expect(cascadeLitFraction(inBand, FAR_M, FADE_M, coversNotFirst, constantCompare)).toBe(
      BY_CASCADE[1],
    );
    expect(cascadeLitFraction(inBand, FAR_M, FADE_M, coversOnlyLast, constantCompare)).toBe(
      BY_CASCADE[3],
    );
    expect(cascadeLitFraction(inBand, FAR_M, FADE_M, () => false, constantCompare)).toBe(1);
  });

  it("blends into lit where no later cascade covers the receiver", () => {
    const cascade = SET.cascades[0];
    const inBand = (cascade.fadeStartM + cascade.farM) / 2;
    expect(cascadeLitFraction(inBand, FAR_M, FADE_M, coversOnlyFirst, constantCompare)).toBeCloseTo(
      0.5 * (BY_CASCADE[0] + 1),
      12,
    );
  });
});

describe("c′ and the minimum with the horizon map", () => {
  it("weighs c by w: lit at w = 0, c at w = 1", () => {
    expect([
      weightedCascadeLit(0, 0),
      weightedCascadeLit(1, 0.2),
      weightedCascadeLit(0.5, 0.2),
    ]).toEqual([1, expect.closeTo(0.2, 15), expect.closeTo(0.6, 15)]);
  });

  it("never brightens: the term is at most 1 and the direct sun's lit share is min(m, c′)", () => {
    const steps = Array.from({ length: 21 }, (_, k) => k / 20);
    for (const m of steps) {
      for (const c of steps) {
        const term = cascadeTerm(m, c);
        expect(term).toBeLessThanOrEqual(1);
        expect(term).toBeGreaterThanOrEqual(0);
        expect(directLitShare(m, c)).toBeCloseTo(Math.min(m, c), 15);
      }
    }
  });

  it("is 1 where the map hides the whole disc, whose term is already 0", () => {
    expect([cascadeTerm(0, 0), cascadeTerm(0, 1), cascadeTerm(-1e-9, 0.5)]).toEqual([1, 1, 1]);
    expect(directLitShare(0, 0.3)).toBe(0);
  });

  it("is formed in the WGSL as here: 1 at w = 0 before any sample, then 1 − w (1 − c)", () => {
    const code = cascadeShadowWgsl.replaceAll(/\/\/[^\n]*/gu, "");
    const start = code.indexOf("fn cascade_lit_fraction(");
    const body = code.slice(start);
    expect(start).toBeGreaterThan(0);
    expect(body.indexOf("if (!(weight > 0.0)) {\n    return 1.0;")).toBeLessThan(
      body.indexOf("cascade_sample("),
    );
    expect(body).toContain("return 1.0 - weight * (1.0 - c);");
  });
});

describe("the receiver's normal offset", () => {
  it("is the cascade's offset × saturate(1 − N·L): none under an overhead sun, all at grazing and behind", () => {
    const n: Vec3 = { x: 0, y: 0, z: 1 };
    const tilt = (deg: number): Vec3 => ({
      x: Math.sin(deg * RAD),
      y: 0,
      z: Math.cos(deg * RAD),
    });
    expect([0, 60, 90, 120].map((deg) => receiverOffsetM(0.1, n, tilt(deg)))).toEqual(
      [0, 0.05, 0.1, 0.1].map((m) => expect.closeTo(m, 12)),
    );
  });
});

describe("the filter's taps", () => {
  it("lie on the disc of one texel, a square √2 apart, of mean 0, their offsets distinct along each axis", () => {
    expect(CASCADE_FILTER_TAPS).toHaveLength(4);
    for (const [x, y] of CASCADE_FILTER_TAPS) {
      expect(Math.hypot(x, y)).toBeCloseTo(CASCADE_TAP_RADIUS_TEXELS, 12);
    }
    const sum = CASCADE_FILTER_TAPS.reduce(([sx, sy], [x, y]) => [sx + x, sy + y], [0, 0]);
    expect(sum.map((s) => Math.abs(s))).toEqual([expect.closeTo(0, 12), expect.closeTo(0, 12)]);
    const apart = CASCADE_FILTER_TAPS.flatMap(([ax, ay], i) =>
      CASCADE_FILTER_TAPS.slice(i + 1).map(([bx, by]) => Math.hypot(ax - bx, ay - by)),
    );
    expect(Math.min(...apart)).toBeCloseTo(Math.SQRT2, 12);
    const columns = new Set(CASCADE_FILTER_TAPS.map(([x]) => x.toFixed(6)));
    const rows = new Set(CASCADE_FILTER_TAPS.map(([, y]) => y.toFixed(6)));
    expect([columns.size, rows.size]).toEqual([4, 4]);
    // Along each axis, neighbouring offsets stand 2 ÷ √10 ≈ 0.63 texel apart.
    for (const axis of [0, 1] as const) {
      const offsets = CASCADE_FILTER_TAPS.map((tap) => tap[axis]).toSorted((a, b) => a - b);
      const steps = offsets.slice(1).map((offset, i) => offset - (offsets[i] ?? Number.NaN));
      expect(steps).toEqual(steps.map(() => expect.closeTo(2 / Math.sqrt(10), 12)));
    }
  });

  it("are the WGSL's own, as are the map's size, the cascades and the footprint", () => {
    const taps = [
      ...cascadeShadowWgsl.matchAll(
        /const CASCADE_TAP_(\d) : vec2f = vec2f\((-?[0-9.]+), (-?[0-9.]+)\);/gu,
      ),
    ].map(([, , x = "", y = ""]) => [Number(x), Number(y)]);
    expect(taps).toHaveLength(4);
    taps.forEach(([x = Number.NaN, y = Number.NaN], i) => {
      const [tx = Number.NaN, ty = Number.NaN] = CASCADE_FILTER_TAPS[i] ?? [];
      expect([x, y]).toEqual([expect.closeTo(tx, 9), expect.closeTo(ty, 9)]);
    });
    expect(cascadeShadowWgsl).toContain(`const CASCADE_COUNT : u32 = ${String(CASCADE_COUNT)}u;`);
    expect(cascadeShadowWgsl).toContain(
      `const CASCADE_SIZE_PX : f32 = ${CASCADE_SIZE_PX.toFixed(1)};`,
    );
    expect(cascadeShadowWgsl).toContain(
      `const CASCADE_FOOTPRINT_TEXELS : f32 = ${CASCADE_FOOTPRINT_TEXELS.toFixed(1)};`,
    );
  });
});

describe("packing a view's cascades into the lit Draw", () => {
  it("names its fields as the Draw's cascade members, in order", () => {
    const fresh = cascadeUniforms();
    expect(CASCADE_UNIFORM_SPECS.map((u) => u.name)).toEqual(Object.keys(fresh));
    const fields = [
      fresh.cascadeLight0,
      fresh.cascadeLight1,
      fresh.cascadeLight2,
      fresh.cascadeLight3,
      fresh.cascadeFarM,
      fresh.cascadeFadeStartM,
      fresh.cascadeNormalOffsetM,
      fresh.cascadeViewAxis,
    ];
    expect(CASCADE_UNIFORM_SPECS.map((u, i) => [u.type, fields[i]?.length])).toEqual([
      ["mat4x4f", 16],
      ["mat4x4f", 16],
      ["mat4x4f", 16],
      ["mat4x4f", 16],
      ["vec4f", 4],
      ["vec4f", 4],
      ["vec4f", 4],
      ["vec4f", 4],
    ]);
  });

  it("carries each cascade's matrix, far end, fade start and offset, and the unit view axis with w", () => {
    const out = packed();
    const lights = [out.cascadeLight0, out.cascadeLight1, out.cascadeLight2, out.cascadeLight3];
    SET.cascades.forEach((cascade, i) => {
      expect([...(lights[i] ?? [])]).toEqual([...cascade.lightFromCamera]);
      expect(out.cascadeFarM[i]).toBe(Math.fround(cascade.farM));
      expect(out.cascadeFadeStartM[i]).toBe(Math.fround(cascade.fadeStartM));
      expect(out.cascadeNormalOffsetM[i]).toBe(Math.fround(cascade.normalOffsetM));
    });
    expect([...out.cascadeViewAxis]).toEqual(
      [AXIS_NR.x, AXIS_NR.y, AXIS_NR.z, SET.weight].map((v) => Math.fround(v)),
    );
    expect(SET.weight).toBe(1);
    // Any length of the axis packs as its unit.
    const scaled = cascadeUniforms();
    packCascadeUniforms(SET, scale(VIEW_AXIS_NR, 7.5), scaled);
    expect([...scaled.cascadeViewAxis]).toEqual(
      [...out.cascadeViewAxis].map((v) => expect.closeTo(v, 7)),
    );
  });

  it("puts each snapped centre at the map's middle and its sphere's x side at u = 1, its y side at v = 0", () => {
    const out = packed();
    const lights = [out.cascadeLight0, out.cascadeLight1, out.cascadeLight2, out.cascadeLight3];
    const xL = nonRotating(SET.basis.x);
    const yL = nonRotating(SET.basis.y);
    const zL = nonRotating(SET.basis.z);
    for (const i of [0, 1, 2, 3] as const) {
      const cascade = SET.cascades[i];
      const light = lights[i] ?? new Float32Array(16);
      const centre = centreFromCamera(i);
      const middle = cascadeMapPoint(light, centre);
      const span = cascade.depthNearM + cascade.depthFarM;
      // To f32's rounding of the matrix and the centre's metre-scale offset.
      expect([middle.u, middle.v, middle.depth]).toEqual([
        expect.closeTo(0.5, 5),
        expect.closeTo(0.5, 5),
        expect.closeTo(cascade.depthFarM / span, 5),
      ]);
      const side = cascadeMapPoint(light, add(centre, scale(xL, cascade.radiusM)));
      const top = cascadeMapPoint(light, add(centre, scale(yL, cascade.radiusM)));
      const sunward = cascadeMapPoint(light, add(centre, scale(zL, cascade.depthNearM)));
      expect([side.u, top.v, sunward.depth]).toEqual([
        expect.closeTo(1, 5),
        expect.closeTo(0, 5),
        expect.closeTo(1, 5),
      ]);
    }
  });

  it("writes identity matrices, zeros and w = 0 for no cascades, which the receiver reads as lit without a sample", () => {
    const out = packed();
    packCascadeUniforms(null, VIEW_AXIS_NR, out);
    expect(out).toEqual(cascadeUniforms());
    expect(cascadeReceiverLit(out, centreFromCamera(0), UP, SUN_BF, refuseSample)).toBe(1);
  });

  it("refuses a view axis of zero length", () => {
    expect(() => packCascadeUniforms(SET, { x: 0, y: 0, z: 0 }, cascadeUniforms())).toThrow(
      /view axis/u,
    );
  });
});

describe("the receiver's twin on a fitted set", () => {
  const normal = nonRotating(UP);
  const sun = nonRotating(SUN_BF);
  const byCascade = (cascade: CascadeIndex): number => BY_CASCADE[cascade];

  it("takes each cascade's value in its own slice before its fade, and the blend in its band", () => {
    const out = packed();
    for (const i of [0, 1, 2, 3] as const) {
      expect(cascadeReceiverLit(out, midSlice(i), normal, sun, byCascade)).toBeCloseTo(
        BY_CASCADE[i],
        6,
      );
    }
    const first = SET.cascades[0];
    const inBand = scale(AXIS_NR, 0.5 * (first.fadeStartM + first.farM));
    expect(cascadeReceiverLit(out, inBand, normal, sun, byCascade)).toBeCloseTo(
      0.5 * (BY_CASCADE[0] + BY_CASCADE[1]),
      5,
    );
  });

  it("compares at the clamped depth through four taps a texel apart about the receiver's map point", () => {
    const out = packed();
    const seen: Array<readonly [number, number, number, number]> = [];
    cascadeReceiverLit(out, midSlice(1), normal, sun, (cascade, u, v, depth) => {
      seen.push([cascade, u, v, depth]);
      return 1;
    });
    expect(seen).toHaveLength(4);
    const lean = 1 - dot(normal, sun);
    expect(lean).toBeGreaterThan(0);
    const moved = add(midSlice(1), scale(normal, lean * (out.cascadeNormalOffsetM[1] ?? 0)));
    const point = cascadeMapPoint(out.cascadeLight1, moved);
    seen.forEach(([cascade, u, v, depth], k) => {
      const [tx = 0, ty = 0] = CASCADE_FILTER_TAPS[k] ?? [];
      expect([cascade, u, v, depth]).toEqual([
        1,
        expect.closeTo(point.u + tx / CASCADE_SIZE_PX, 12),
        expect.closeTo(point.v + ty / CASCADE_SIZE_PX, 12),
        expect.closeTo(Math.min(Math.max(point.depth, 0), 1), 12),
      ]);
    });
  });

  it("falls to the next cascade where the receiver with the footprint overhangs its own map", () => {
    const out = packed();
    const base = midSlice(0);
    // Across the view axis, so that the receiver stays in cascade 1's slice: u rises linearly
    // along the light's x turned square to the axis, to the map's edge.
    const xL = nonRotating(SET.basis.x);
    const across = normalise(sub(xL, scale(AXIS_NR, dot(xL, AXIS_NR))));
    const uAt = (sM: number): number =>
      cascadeMapPoint(out.cascadeLight0, add(base, scale(across, sM))).u;
    const perMetre = uAt(1) - uAt(0);
    const margin = CASCADE_FOOTPRINT_TEXELS / CASCADE_SIZE_PX;
    const atU = (u: number): Vec3 => add(base, scale(across, (u - uAt(0)) / perMetre));
    // A texel inside the footprint's margin, and a texel beyond it.
    const inside = atU(1 - margin - 1 / CASCADE_SIZE_PX);
    const outside = atU(1 - margin + 1 / CASCADE_SIZE_PX);
    for (const p of [inside, outside]) {
      expect(cascadeAt(dot(p, AXIS_NR), FAR_M)).toBe(0);
    }
    const lights = [
      out.cascadeLight0,
      out.cascadeLight1,
      out.cascadeLight2,
      out.cascadeLight3,
    ] as const;
    const covering = (p: Vec3): CascadeIndex[] =>
      ([0, 1, 2, 3] as const).filter((j) => coversWithFootprint(cascadeMapPoint(lights[j], p)));
    expect(covering(inside)[0]).toBe(0);
    const fallback = covering(outside)[0];
    if (fallback === undefined) {
      throw new Error("no cascade covers the receiver past cascade 1's edge");
    }
    expect(fallback).toBeGreaterThan(0);
    // With the sun along the normal there is no offset, so the edge is where the geometry puts it.
    expect(cascadeReceiverLit(out, inside, sun, sun, byCascade)).toBeCloseTo(BY_CASCADE[0], 9);
    expect(cascadeReceiverLit(out, outside, sun, sun, byCascade)).toBeCloseTo(
      BY_CASCADE[fallback],
      9,
    );
  });

  it("is lit beyond C₄, and c′ = 1 − w (1 − c) at a partial weight", () => {
    const out = packed();
    const beyond = scale(AXIS_NR, (FAR_M[3] ?? 0) * 1.01);
    expect(cascadeReceiverLit(out, beyond, normal, sun, () => 0)).toBe(1);
    out.cascadeViewAxis[3] = 0.25;
    expect(cascadeReceiverLit(out, midSlice(2), normal, sun, byCascade)).toBeCloseTo(
      1 - 0.25 * (1 - BY_CASCADE[2]),
      6,
    );
  });
});
