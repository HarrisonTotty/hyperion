/**
 * The wireframe's terrain rules (plan R10, R10.T11; Design note 14): `terrainWire.ts` and the
 * arithmetic of `shaders/terrainWire.wgsl` that its twins mirror, against the task's pure tests:
 * "`contourInterval` walks the 1-2-5 sequence from the nadir's height above ground with ±20%
 * hysteresis and never falls below 1 m; `contourPhase` with the patch-local height equals the `f64`
 * contour levels to 1 mm at ±20 km; each family is drawn in full at 6 px of spacing or more and
 * fades to nothing at 3 px, in CSS pixels times the stroke scale, minor before index; the index
 * contour is 1.5 px". Also the graticule drawn on the terrain's height (Design note 14), the nadir's
 * height above the ground, the label block's `CONTOUR INTERVAL 200 m · INDEX 1 km` reading, the
 * material and its phase records. The selection's tests, the ratio to the lit style and the
 * grounded-body rule, are `terrainWireSelection.test.ts`.
 *
 * @remarks
 * The shader's arithmetic is emulated in `f32`, every operation correctly rounded (`Math.fround`)
 * in the WGSL's order, with the records' values narrowed as `WirePhaseRecords` and the slot record
 * narrow them; the graticule's also at the corners of WGSL's bounds for its arithmetic
 * (`test/preciseTrigF32.ts`' `wgslArithmetics`). The shader's arc tangents are `preciseTrig.wgsl`'s
 * `preciseAtan2`, emulated by its own `f32` twin under the same arithmetic, rather than WGSL's
 * built-in `atan2`, which WGSL bounds only to 4,096 ULP.
 */

import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { type Arithmetic, preciseAtan2, wgslArithmetics } from "../../test/preciseTrigF32";
import { seededRandom } from "../../test/seededRandom";
import { bindingStages } from "../../test/wgslStages";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import {
  declaredStructLayout,
  OFFSET_MEMBER,
  type StructLayout,
  uniformLayout,
} from "../engine/webgpu/uniforms";
import { WIRE_PHASE_RECORD_BYTES, WirePhaseRecords } from "./gpu/uniforms";
import terrainWireWgsl from "./shaders/terrainWire.wgsl?raw";
import {
  CONTOUR_FADE_PX,
  CONTOUR_FIELD_CH,
  CONTOUR_INDEX_EVERY,
  contourFade,
  contourInterval,
  contourIntervalReading,
  contourLengthReading,
  contourPhase,
  contourShade,
  contourStepM,
  type GraticulePatch,
  graticuleOffsetsRad,
  graticulePatch,
  heightAboveGroundM,
  lineCoverage,
  TERRAIN_WIRE_MATERIALS,
  terrainWireMaterialSpec,
} from "./terrainWire";

const f = Math.fround;
const RAD = Math.PI / 180;

/** `x` modulo `m`, in [0, m). */
function mod(x: number, m: number): number {
  return x - m * Math.floor(x / m);
}

/** `x` reduced to (−m ÷ 2, m ÷ 2] modulo `m`. */
function wrapped(x: number, m: number): number {
  return x - m * Math.round(x / m);
}

/** ∫ g(x) dx over [−halfWidthPx, halfWidthPx], device px, by the midpoint rule at 10⁻³ px. */
function integratePx(halfWidthPx: number, g: (xPx: number) => number): number {
  const dx = 1e-3;
  const n = Math.round((2 * halfWidthPx) / dx);
  let sum = 0;
  for (let i = 0; i < n; i += 1) {
    sum += g(-halfWidthPx + (i + 0.5) * dx);
  }
  return sum * dx;
}

describe("the contour interval's 1-2-5 sequence (Design note 14)", () => {
  it("runs 1, 2, 5, 10, 20, 50, 100, 200, 500 and 1,000 m for steps 0 to 9", () => {
    expect(Array.from({ length: 10 }, (_, k) => contourStepM(k))).toEqual([
      1, 2, 5, 10, 20, 50, 100, 200, 500, 1000,
    ]);
  });

  it.each([0.5, -1, Number.NaN, Number.POSITIVE_INFINITY])("refuses step %s", (k) => {
    expect(() => contourStepM(k)).toThrow(RangeError);
  });
});

/** Every step of the sequence, m. */
const STEPS = Array.from({ length: 46 }, (_, k) => contourStepM(k));

/** The step index of `intervalM`, or −1 where it is none. */
function stepIndex(intervalM: number): number {
  return STEPS.indexOf(intervalM);
}

describe("the contour interval with none stated before (Design note 14)", () => {
  it.each([
    [4_000, 200],
    [20, 1],
    [3e6, 200_000],
    [2e9, 1e8],
  ])("takes, at %s m above the ground, the step nearest a twentieth of it: %s m", (aglM, step) => {
    expect(contourInterval(aglM, null)).toBe(step);
  });

  it.each([0, 15, -350])("never falls below 1 m, at %s m above the ground", (aglM) => {
    expect(contourInterval(aglM, null)).toBe(1);
  });

  it("takes the step nearest a twentieth of the height in log terms from 20 m to 20,000 km", () => {
    const misses: string[] = [];
    for (let i = 0; i <= 1_000; i += 1) {
      const aglM = 20 * 10 ** (0.006_123 * i);
      const target = aglM / 20;
      const chosen = contourInterval(aglM, null);
      const best = Math.min(...STEPS.map((step) => Math.abs(Math.log(target / step))));
      if (Math.abs(Math.log(target / chosen)) > best + 1e-12) {
        misses.push(`${String(aglM)} m: ${String(chosen)} m`);
      }
    }
    expect(misses).toEqual([]);
  });
});

describe("the contour interval's ±20% hysteresis (Design note 14)", () => {
  // From 200 m: up at 1.2 √(200 × 500) m, about 379.5 m, and down at 0.8 √(100 × 200) m, about
  // 113.1 m, for a twentieth of the height.
  const upM = 1.2 * Math.sqrt(200 * 500);
  const downM = 0.8 * Math.sqrt(100 * 200);

  it("holds 200 m while a twentieth of the height is just under 1.2 √(200 × 500) m", () => {
    expect(contourInterval(20 * upM * (1 - 1e-9), 200)).toBe(200);
  });

  it("steps up from 200 m to 500 m once a twentieth of the height passes 1.2 √(200 × 500) m", () => {
    expect(contourInterval(20 * upM * (1 + 1e-9), 200)).toBe(500);
  });

  it("holds 200 m while a twentieth of the height is just over 0.8 √(100 × 200) m", () => {
    expect(contourInterval(20 * downM * (1 + 1e-9), 200)).toBe(200);
  });

  it("steps down from 200 m to 100 m once a twentieth of the height falls below 0.8 √(100 × 200) m", () => {
    expect(contourInterval(20 * downM * (1 - 1e-9), 200)).toBe(100);
  });

  it("keeps the interval it holds inside the band, where none held would give the nearer step", () => {
    expect([
      contourInterval(7_000, null),
      contourInterval(7_000, 200),
      contourInterval(5_500, null),
      contourInterval(5_500, 500),
    ]).toEqual([500, 200, 200, 500]);
  });

  it("resolves a jump of many steps in one call, up and down", () => {
    expect([contourInterval(20e6, 1), contourInterval(20, 1e6)]).toEqual([1e6, 1]);
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])(
    "refuses a height of %s m",
    (aglM) => {
      expect(() => contourInterval(aglM, 200)).toThrow(RangeError);
    },
  );

  it.each([300, 0, -200, 3])("takes a previous interval of %s m, no step, as none", (previous) => {
    // 7,000 m: none held gives 500 m, 200 m held keeps 200 m.
    expect(contourInterval(7_000, previous)).toBe(500);
  });
});

/** The intervals stated along `path` of heights above the ground, each from the one before. */
function walk(path: ReadonlyArray<number>): number[] {
  let previous: number | null = null;
  const intervals: number[] = [];
  for (const aglM of path) {
    previous = contourInterval(aglM, previous);
    intervals.push(previous);
  }
  return intervals;
}

/** The distinct changes of step between successive intervals, sorted. */
function stepChanges(intervals: ReadonlyArray<number>): number[] {
  const changes = new Set<number>();
  for (let i = 1; i < intervals.length; i += 1) {
    changes.add(stepIndex(intervals[i] ?? Number.NaN) - stepIndex(intervals[i - 1] ?? Number.NaN));
  }
  return [...changes].toSorted((a, b) => a - b);
}

describe("the contour interval along a climb and a descent from 1 m to 1,000 km", () => {
  const heightsM = Array.from({ length: 2_001 }, (_, i) => 10 ** ((6 * i) / 2_000));
  const climb = walk(heightsM);
  const descent = walk(heightsM.toReversed());

  it("states a 1-2-5 step at every height", () => {
    expect([...climb, ...descent].filter((interval) => stepIndex(interval) < 0)).toEqual([]);
  });

  it("steps only up, one step at a time, as the height rises", () => {
    expect(stepChanges(climb)).toEqual([0, 1]);
  });

  it("steps only down, one step at a time, as the height falls", () => {
    expect(stepChanges(descent)).toEqual([-1, 0]);
  });

  it("runs from 1 m at 1 m to 50 km at 1,000 km and back", () => {
    expect([climb[0], climb.at(-1), descent[0], descent.at(-1)]).toEqual([1, 50_000, 50_000, 1]);
  });

  it("lags the height each way: never finer descending than climbing at the same height", () => {
    const descentAt = descent.toReversed();
    expect(climb.filter((interval, i) => interval > (descentAt[i] ?? Number.NaN))).toEqual([]);
    expect(climb.some((interval, i) => interval < (descentAt[i] ?? Number.NaN))).toBe(true);
  });
});

/** The intervals and patch origins the phase is checked at, m. */
const INTERVALS_M = [1, 2, 5, 10, 100, 1_000] as const;
const ORIGINS_M = [
  -20_000, -19_876.543, -10_000, -9_876.5, 0, 9_876.5, 10_000, 19_876.543, 20_000,
] as const;

/** Heights within ±1 km of `originM` as the bake holds them, `f32`: the ends, near it, and a spread. */
function heightsNear(originM: number, seed: number): number[] {
  const random = seededRandom(seed);
  const offsetsM = [-1_000, -999.9993, -517.3, -0.0004, 0, 0.0007, 3.21, 250.0001, 999.9997, 1_000];
  for (let k = 0; k < 200; k += 1) {
    offsetsM.push(2_000 * random() - 1_000);
  }
  return offsetsM.map((d) => f(originM + d));
}

/**
 * The shader's contour coordinate at a vertex (`vertexMain`'s `contourU`), every operation in
 * `f32`: local = h − h₀ with h₀ the slot record's `f32`, then (local + φ) × (1 ÷ CI) with φ as the
 * phase record narrows it and 1 ÷ CI formed in `f64` and narrowed once (the `Draw`'s `contour.w`),
 * a product WGSL rounds correctly where it allows a quotient 2.5 ULP.
 */
function shaderContourU(heightM: number, originHeightM: number, intervalM: number): number {
  const local = f(f(heightM) - f(originHeightM));
  return f(f(local + f(contourPhase(originHeightM, intervalM))) * f(1 / intervalM));
}

describe("the contour phase (Design note 14)", () => {
  it("lies in [0, 5 CI) and is the origin's f32 height modulo 5 CI", () => {
    const misses: string[] = [];
    for (const originM of [...ORIGINS_M, -3.25, 7.5, 123_456.789]) {
      for (const intervalM of INTERVALS_M) {
        const phase = contourPhase(originM, intervalM);
        const periods = (f(originM) - phase) / (5 * intervalM);
        if (
          !(phase >= 0 && phase < 5 * intervalM) ||
          Math.abs(periods - Math.round(periods)) > 1e-9
        ) {
          misses.push(`${String(originM)} m at ${String(intervalM)} m: ${String(phase)} m`);
        }
      }
    }
    expect(misses).toEqual([]);
  });

  it.each([
    [Number.NaN, 10],
    [Number.POSITIVE_INFINITY, 10],
    [100, 0],
    [100, -5],
    [100, Number.NaN],
  ])("refuses an origin at %s m with an interval of %s m", (originM, intervalM) => {
    expect(() => contourPhase(originM, intervalM)).toThrow(RangeError);
  });

  it("crosses each contour level where the f64 height does, to 1 mm within 1 km of origins at up to ±20 km", () => {
    const misses: string[] = [];
    ORIGINS_M.forEach((originM, o) => {
      for (const intervalM of INTERVALS_M) {
        for (const heightM of heightsNear(originM, 17 + o)) {
          const u = shaderContourU(heightM, originM, intervalM);
          const errorM = wrapped(
            (u - Math.floor(u)) * intervalM - mod(heightM, intervalM),
            intervalM,
          );
          if (Math.abs(errorM) > 1e-3) {
            misses.push(
              `${String(heightM)} m by ${String(originM)} m at ${String(intervalM)} m: ${String(errorM)} m`,
            );
          }
        }
      }
    });
    expect(misses).toEqual([]);
  });

  it("equals the f64 contour coordinate (h − 5 CI ⌊h₀ ÷ 5 CI⌋) ÷ CI to 1 mm of height", () => {
    const misses: string[] = [];
    ORIGINS_M.forEach((originM, o) => {
      for (const intervalM of INTERVALS_M) {
        const period = 5 * intervalM;
        const base = period * Math.floor(f(originM) / period);
        for (const heightM of heightsNear(originM, 41 + o)) {
          const errorM = shaderContourU(heightM, originM, intervalM) * intervalM - (heightM - base);
          if (Math.abs(errorM) > 1e-3) {
            misses.push(
              `${String(heightM)} m by ${String(originM)} m at ${String(intervalM)} m: ${String(errorM)} m`,
            );
          }
        }
      }
    });
    expect(misses).toEqual([]);
  });

  it("tells the index contours: round(u) mod 5 is 0 at each multiple of 5 CI and at no contour between", () => {
    const misses: string[] = [];
    for (const originM of ORIGINS_M) {
      for (const intervalM of INTERVALS_M) {
        const first = Math.ceil((originM - 1_000) / intervalM);
        const last = Math.floor((originM + 1_000) / intervalM);
        for (let m = first; m <= last; m += 1) {
          const levelM = m * intervalM;
          const onIndex = mod(Math.round(shaderContourU(levelM, originM, intervalM)), 5) === 0;
          if (onIndex !== (mod(levelM, 5 * intervalM) === 0)) {
            misses.push(`${String(levelM)} m by ${String(originM)} m at ${String(intervalM)} m`);
          }
        }
      }
    }
    expect(misses).toEqual([]);
  });

  it("is needed: an f32 height at ±20 km above the datum steps 2⁻⁹ m, about 2 mm", () => {
    expect([f(20_000.001) - 20_000, f(-20_000.001) + 20_000]).toEqual([2 ** -9, -(2 ** -9)]);
  });

  it("is needed: a 2 mm ramp at 20 km holds two f32 heights above the datum, 0.1 mm apart a fragment", () => {
    const rampM = Array.from({ length: 21 }, (_, i) => 20_000 + 1e-4 * i);
    expect(new Set(rampM.map(f)).size).toBe(2);
  });

  it("resolves that ramp 0.1 mm a fragment patch-locally, near a patch origin 100 m below it", () => {
    const local = Array.from({ length: 21 }, (_, i) => f(20_000 + 1e-4 * i - 19_900));
    const steps = local.slice(1).map((value, i) => value - (local[i] ?? Number.NaN));
    expect(steps.filter((stepM) => Math.abs(stepM - 1e-4) > 2e-5)).toEqual([]);
  });
});

describe.each([1, 2, 3])("a contour family's fade at a stroke scale of %s", (scale) => {
  it("is 1 from 6 px of spacing times the scale", () => {
    expect([6, 6.5, 100].map((px) => contourFade(px * scale, scale))).toEqual([1, 1, 1]);
  });

  it("is 0 at 3 px of spacing times the scale and below", () => {
    expect([3, 2, 0].map((px) => contourFade(px * scale, scale))).toEqual([0, 0, 0]);
  });

  it("falls linearly between them", () => {
    const fades = [3.75, 4.5, 5.25].map((px) => contourFade(px * scale, scale));
    expect(fades.map((fade, i) => Math.abs(fade - 0.25 * (i + 1)))).toEqual([
      expect.closeTo(0, 12),
      expect.closeTo(0, 12),
      expect.closeTo(0, 12),
    ]);
  });
});

describe("a line's coverage (R02's lines.wgsl)", () => {
  it.each([1, 1.5, 2, 3, 4.5, 6])("sums to its width across a line of %s px", (widthPx) => {
    const across = integratePx(widthPx / 2 + 2, (x) => lineCoverage(Math.abs(x), widthPx));
    expect(across).toBeCloseTo(widthPx, 5);
  });

  it.each([1, 1.5, 2, 3])("covers the centre of a line of %s px in full", (widthPx) => {
    expect(lineCoverage(0, widthPx)).toBe(1);
  });
});

describe.each([1, 2])(
  "a fragment's contours at a stroke scale of %s (terrainWire.wgsl's wire_contours)",
  (scale) => {
    /** The coordinate's change a device px at a minor spacing of `spacingPx` CSS px. */
    const perPxAt = (spacingPx: number): number => 1 / (spacingPx * scale);

    it("draws a minor contour in full at 6 px of spacing", () => {
      expect(contourShade(1, perPxAt(6), null, scale).minor).toBeCloseTo(1, 12);
    });

    it("draws a minor contour at half strength at 4.5 px of spacing", () => {
      expect(contourShade(1, perPxAt(4.5), null, scale).minor).toBeCloseTo(0.5, 12);
    });

    it("draws no minor contour at 3 px of spacing", () => {
      expect(contourShade(1, perPxAt(3), null, scale).minor).toBe(0);
    });

    it("still draws the index contour in full where the minor ones are gone: minor before index", () => {
      expect([
        contourShade(1, perPxAt(3), null, scale).minor,
        contourShade(5, perPxAt(3), null, scale).index,
      ]).toEqual([0, 1]);
    });

    it("fades the index contour from 6 px of its own spacing, five minor ones, to nothing at 3 px", () => {
      const fades = [1.2, 0.9, 0.6].map((px) => contourShade(5, perPxAt(px), null, scale).index);
      expect(fades.map((fade, i) => Math.abs(fade - (1 - 0.5 * i)))).toEqual([
        expect.closeTo(0, 12),
        expect.closeTo(0, 12),
        expect.closeTo(0, 12),
      ]);
    });

    it("covers 1.5 px times the scale across an index contour", () => {
      const perPx = perPxAt(10);
      const across = integratePx(
        4 * scale,
        (x) => contourShade(5 + x * perPx, perPx, null, scale).index,
      );
      expect(across).toBeCloseTo(1.5 * scale, 5);
    });

    it("draws no minor contour on an index level, which is the index contour's", () => {
      const perPx = perPxAt(10);
      const minors = Array.from({ length: 801 }, (_, i) => {
        const x = (i - 400) * 0.01 * scale;
        return contourShade(5 + x * perPx, perPx, null, scale).minor;
      });
      expect(Math.max(...minors)).toBe(0);
    });

    it("covers 1 px times the scale across a minor contour, with no index coverage there", () => {
      const perPx = perPxAt(10);
      const minor = integratePx(
        4 * scale,
        (x) => contourShade(2 + x * perPx, perPx, null, scale).minor,
      );
      const index = integratePx(
        4 * scale,
        (x) => contourShade(2 + x * perPx, perPx, null, scale).index,
      );
      expect([minor, index]).toEqual([expect.closeTo(scale, 5), 0]);
    });

    it.each([0.001, 1, 1_000])(
      "draws the coastline in full at sea level, its height changing %s m a px",
      (perPxM) => {
        expect(contourShade(0.3, 0.01, { aboveSeaM: 0, perPxM }, scale).coastline).toBe(1);
      },
    );

    it("covers 2 px times the scale across the coastline", () => {
      const perPxM = 0.37;
      const across = integratePx(
        scale + 2,
        (x) => contourShade(0.3, 0.01, { aboveSeaM: x * perPxM, perPxM }, scale).coastline,
      );
      expect(across).toBeCloseTo(2 * scale, 5);
    });

    it("draws no coastline on a world with no sea", () => {
      expect(contourShade(0.3, 0.01, null, scale).coastline).toBe(0);
    });
  },
);

describe("the shader's contour constants", () => {
  it.each([
    ["CONTOUR_FULL_PX", CONTOUR_FADE_PX.full],
    ["CONTOUR_GONE_PX", CONTOUR_FADE_PX.gone],
    ["CONTOUR_INDEX_EVERY", CONTOUR_INDEX_EVERY],
  ] as const)("declares %s as terrainWire.ts does", (name, value) => {
    const declared = new RegExp(`const ${name} : f32 = ([0-9.]+);`, "u").exec(terrainWireWgsl)?.[1];
    expect(Number(declared)).toBe(value);
  });
});

/** The radii and origins the graticule is checked at. */
const RADII_M = [
  ["the Moon's", 1_737_400],
  ["the Earth's", 6_378_137],
] as const;
const LATS_DEG = [0, 28, 60, 89, -45] as const;
const LONS_DEG = [0, 47, 179.9, -120] as const;
/** R02's fine graticule step, 15°. */
const STEP_RAD = 15 * RAD;

/** The point at `radiusM` at geocentric latitude and longitude `latDeg`, `lonDeg`, body-fixed. */
function pointAt(radiusM: number, latDeg: number, lonDeg: number): Vec3 {
  const lat = latDeg * RAD;
  const lon = lonDeg * RAD;
  return vec3(
    radiusM * Math.cos(lat) * Math.cos(lon),
    radiusM * Math.cos(lat) * Math.sin(lon),
    radiusM * Math.sin(lat),
  );
}

/** A point's geocentric latitude and longitude, rad, in `f64`. */
function geocentric(p: Vec3): { readonly latRad: number; readonly lonRad: number } {
  return { latRad: Math.atan2(p.z, Math.hypot(p.x, p.y)), lonRad: Math.atan2(p.y, p.x) };
}

/** `count` offsets of seeded directions and lengths up to `maxM`, a tenth of it at least. */
function offsetsWithin(seed: number, count: number, maxM: number): Vec3[] {
  const random = seededRandom(seed);
  const out: Vec3[] = [];
  while (out.length < count) {
    const d = vec3(2 * random() - 1, 2 * random() - 1, 2 * random() - 1);
    const length = Math.hypot(d.x, d.y, d.z);
    if (length > 0.1 && length <= 1) {
      const u = normalise(d);
      const m = maxM * (0.1 + 0.9 * random());
      out.push(vec3(u.x * m, u.y * m, u.z * m));
    }
  }
  return out;
}

/** Every origin of the checks: each radius, latitude and longitude. */
const ORIGINS = RADII_M.flatMap(([body, radiusM]) =>
  LATS_DEG.flatMap((latDeg) =>
    LONS_DEG.map((lonDeg) => ({
      body,
      radiusM,
      latDeg,
      lonDeg,
      originM: pointAt(radiusM, latDeg, lonDeg),
    })),
  ),
);

/** The origin's latitude and longitude as the patch's lines and phases give them, rad. */
function patchLatLon(patch: GraticulePatch, stepRad: number): { latRad: number; lonRad: number } {
  return {
    latRad: patch.latIndex * stepRad + patch.latPhaseRad,
    lonRad: patch.lonIndex * stepRad + patch.lonPhaseRad,
  };
}

/** The misses of the f64 offsets over the origins and `offsetsFor`'s offsets, past `toleranceRad`. */
function f64Misses(
  offsetsFor: (radiusM: number, seed: number) => Vec3[],
  toleranceRad: number,
): string[] {
  const misses: string[] = [];
  ORIGINS.forEach(({ body, radiusM, latDeg, lonDeg, originM }, o) => {
    const patch = graticulePatch(originM, STEP_RAD);
    const origin = patchLatLon(patch, STEP_RAD);
    for (const q of offsetsFor(radiusM, 100 + o)) {
      const truth = geocentric(vec3(originM.x + q.x, originM.y + q.y, originM.z + q.z));
      const { dLatRad, dLonRad } = graticuleOffsetsRad(patch, q);
      const latError = origin.latRad + dLatRad - truth.latRad;
      const lonError = wrapped(origin.lonRad + dLonRad - truth.lonRad, 2 * Math.PI);
      if (Math.abs(latError) > toleranceRad || Math.abs(lonError) > toleranceRad) {
        misses.push(
          `${body} ${String(latDeg)}° ${String(lonDeg)}°: ${String(latError)}, ${String(lonError)} rad`,
        );
      }
    }
  });
  return misses;
}

/**
 * `wire_graticule_offsets` in `f32`, each operation rounded by `ops` in the WGSL's order, its
 * `wire_atan2` through `preciseTrig.wgsl`'s `preciseAtan2` (its `f32` twin, `preciseTrigF32.ts`),
 * the record's radius and frame and the vertex's offset narrowed to `f32` as the records hold them.
 */
function shaderOffsets(
  patch: GraticulePatch,
  offsetM: Vec3,
  ops: Arithmetic,
): { dLatRad: number; dLonRad: number } {
  const { add, sub, mul, div, sqrt } = ops;
  const atan2 = (y: number, x: number): number =>
    f((y < 0 ? -1 : 1) * preciseAtan2(Math.abs(y), x, ops));
  const sinLat = f(patch.sinLat);
  const cosLat = f(patch.cosLat);
  const sinLon = f(patch.sinLon);
  const cosLon = f(patch.cosLon);
  const radius = f(patch.radiusM);
  const qx = f(offsetM.x);
  const qy = f(offsetM.y);
  const qz = f(offsetM.z);
  const e = add(mul(-sinLon, qx), mul(cosLon, qy));
  const n = add(sub(mul(mul(-sinLat, cosLon), qx), mul(mul(sinLat, sinLon), qy)), mul(cosLat, qz));
  const w = add(
    add(add(radius, mul(mul(cosLat, cosLon), qx)), mul(mul(cosLat, sinLon), qy)),
    mul(sinLat, qz),
  );
  const c = sub(mul(w, cosLat), mul(n, sinLat));
  const b = sqrt(add(mul(c, c), mul(e, e)));
  const dLonRad = atan2(e, c);
  if (!(c > 0)) {
    return {
      dLatRad: sub(atan2(add(mul(w, sinLat), mul(n, cosLat)), b), atan2(sinLat, cosLat)),
      dLonRad,
    };
  }
  const delta = div(mul(e, e), add(b, c));
  return { dLatRad: atan2(sub(n, mul(delta, sinLat)), add(w, mul(delta, cosLat))), dLonRad };
}

/** The fine patches' offsets of the f32 checks, up to 2 km. */
const FINE_OFFSETS = (seed: number): Vec3[] => offsetsWithin(seed, 20, 2_000);

describe("the graticule's place on a patch (Design note 14)", () => {
  it("gives a fine patch's points their geocentric latitude and longitude to 10⁻¹² rad in f64, offsets up to 1 km", () => {
    expect(f64Misses((_, seed) => offsetsWithin(seed, 20, 1_000), 1e-12)).toEqual([]);
  });

  it("gives a coarse patch's points their geocentric latitude and longitude to 10⁻¹² rad in f64, offsets up to 0.3 R", () => {
    expect(f64Misses((radiusM, seed) => offsetsWithin(seed, 20, 0.3 * radiusM), 1e-12)).toEqual([]);
  });

  it("places coarse offsets past the axis from a patch at 89°, its other branch, to 10⁻¹² rad in f64", () => {
    const originM = pointAt(6_378_137, 89, 47);
    const radial = vec3(Math.cos(47 * RAD), Math.sin(47 * RAD), 0);
    const past = offsetsWithin(7, 200, 0.3 * 6_378_137).filter(
      (q) => (originM.x + q.x) * radial.x + (originM.y + q.y) * radial.y <= 0,
    );
    // The seed's offsets reach past the axis, or the branch goes untested.
    expect(past.length).toBeGreaterThan(0);
    const patch = graticulePatch(originM, STEP_RAD);
    const origin = patchLatLon(patch, STEP_RAD);
    const misses = past.flatMap((q) => {
      const truth = geocentric(vec3(originM.x + q.x, originM.y + q.y, originM.z + q.z));
      const { dLatRad, dLonRad } = graticuleOffsetsRad(patch, q);
      const latError = origin.latRad + dLatRad - truth.latRad;
      const lonError = wrapped(origin.lonRad + dLonRad - truth.lonRad, 2 * Math.PI);
      return Math.abs(latError) > 1e-12 || Math.abs(lonError) > 1e-12
        ? [`${String(latError)}, ${String(lonError)} rad`]
        : [];
    });
    expect(misses).toEqual([]);
  });

  // Correctly rounded first, then each operation at a corner of WGSL's bounds (2.5 ULP a division,
  // `sqrt` as 1 ÷ `inverseSqrt`), `preciseAtan2`'s own operations at the same corners.
  it.each(wgslArithmetics().map(([label, ops]) => ({ label, ops })))(
    "places a fine patch's lines in f32 within 2 × 10⁻⁹ rad of arc of the f64 truth, offsets up to 2 km, arithmetic $label",
    ({ ops }) => {
      const misses: string[] = [];
      ORIGINS.forEach(({ body, latDeg, lonDeg, originM }, o) => {
        const patch = graticulePatch(originM, STEP_RAD);
        const origin = geocentric(originM);
        for (const q of FINE_OFFSETS(300 + o)) {
          const truth = geocentric(vec3(originM.x + q.x, originM.y + q.y, originM.z + q.z));
          const ours = shaderOffsets(patch, q, ops);
          const latError = ours.dLatRad - (truth.latRad - origin.latRad);
          const lonArcError =
            wrapped(ours.dLonRad - (truth.lonRad - origin.lonRad), 2 * Math.PI) *
            Math.cos(truth.latRad);
          if (Math.abs(latError) > 2e-9 || Math.abs(lonArcError) > 2e-9) {
            misses.push(
              `${body} ${String(latDeg)}° ${String(lonDeg)}°: ${String(latError)}, ${String(lonArcError)} rad`,
            );
          }
        }
      });
      expect(misses).toEqual([]);
    },
  );

  it("needs the patch's place: the f32 latitude of the absolute position errs past 2 × 10⁻⁸ rad over the same points", () => {
    let worstRad = 0;
    ORIGINS.forEach(({ originM }, o) => {
      for (const q of FINE_OFFSETS(300 + o)) {
        const truth = geocentric(vec3(originM.x + q.x, originM.y + q.y, originM.z + q.z));
        const px = f(f(originM.x) + f(q.x));
        const py = f(f(originM.y) + f(q.y));
        const pz = f(f(originM.z) + f(q.z));
        const naive = f(Math.atan2(pz, f(Math.sqrt(f(f(px * px) + f(py * py))))));
        worstRad = Math.max(worstRad, Math.abs(naive - truth.latRad));
      }
    });
    expect(worstRad).toBeGreaterThan(2e-8);
  });

  it.each([STEP_RAD, 30 * RAD])(
    "names each origin's nearest lines and its phase from them, at a step of %s rad",
    (stepRad) => {
      const misses: string[] = [];
      for (const { body, latDeg, lonDeg, originM } of ORIGINS) {
        const patch = graticulePatch(originM, stepRad);
        const { latRad, lonRad } = geocentric(originM);
        const back = patchLatLon(patch, stepRad);
        if (
          patch.latIndex !== Math.round(latRad / stepRad) ||
          patch.lonIndex !== Math.round(lonRad / stepRad) ||
          // −45° at a 30° step is a tie, whose phase is half a step to the last bit.
          Math.abs(patch.latPhaseRad) > (stepRad / 2) * (1 + 1e-15) ||
          Math.abs(patch.lonPhaseRad) > (stepRad / 2) * (1 + 1e-15) ||
          Math.abs(back.latRad - latRad) > 1e-15 ||
          Math.abs(back.lonRad - lonRad) > 1e-15
        ) {
          misses.push(`${body} ${String(latDeg)}° ${String(lonDeg)}°`);
        }
      }
      expect(misses).toEqual([]);
    },
  );

  it("carries the origin's frame: the sines and cosines of its latitude and longitude, and its radius", () => {
    const misses: string[] = [];
    for (const { body, radiusM, latDeg, lonDeg, originM } of ORIGINS) {
      const patch = graticulePatch(originM, STEP_RAD);
      const { latRad, lonRad } = geocentric(originM);
      const errors = [
        patch.sinLat - Math.sin(latRad),
        patch.cosLat - Math.cos(latRad),
        patch.sinLon - Math.sin(lonRad),
        patch.cosLon - Math.cos(lonRad),
        (patch.radiusM - radiusM) / radiusM,
      ];
      if (errors.some((error) => Math.abs(error) > 1e-14)) {
        misses.push(`${body} ${String(latDeg)}° ${String(lonDeg)}°`);
      }
    }
    expect(misses).toEqual([]);
  });

  it("names no line and no phase where no graticule is drawn", () => {
    const patch = graticulePatch(pointAt(6_378_137, 28, 47), 0);
    expect([patch.latIndex, patch.lonIndex, patch.latPhaseRad, patch.lonPhaseRad]).toEqual([
      0, 0, 0, 0,
    ]);
  });
});

describe("the camera's height above the ground at its nadir (Design note 14)", () => {
  const SPHERE = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000 };
  const OBLATE = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752 };
  const up = normalise(vec3(1, 2, 3));
  const cameraOverSphere = vec3(
    up.x * (6_371_000 + 4_000),
    up.y * (6_371_000 + 4_000),
    up.z * (6_371_000 + 4_000),
  );

  it("is the height above the datum less the ground's height there, 4,000 m over ground at 1,500 m", () => {
    expect(heightAboveGroundM(cameraOverSphere, SPHERE, () => 1_500)).toBeCloseTo(2_500, 6);
  });

  it("adds the depth of ground below the datum, 4,000 m over ground at −1,500 m", () => {
    expect(heightAboveGroundM(cameraOverSphere, SPHERE, () => -1_500)).toBeCloseTo(5_500, 6);
  });

  it("is the height above the datum where no ground is drawn", () => {
    expect(heightAboveGroundM(cameraOverSphere, SPHERE, () => null)).toBeCloseTo(4_000, 6);
  });

  it("asks the ground's height at the unit direction straight down on a sphere", () => {
    const asked: Vec3[] = [];
    heightAboveGroundM(cameraOverSphere, SPHERE, (direction) => {
      asked.push(direction);
      return null;
    });
    expect(asked.map((d) => [d.x - up.x, d.y - up.y, d.z - up.z])).toEqual([
      [expect.closeTo(0, 14), expect.closeTo(0, 14), expect.closeTo(0, 14)],
    ]);
  });

  it.each([
    ["the pole", vec3(0, 0, 6_356_752 + 4_000), vec3(0, 0, 1)],
    ["the equator", vec3(6_378_137 + 4_000, 0, 0), vec3(1, 0, 0)],
  ] as const)(
    "measures along the spheroid's normal over %s, asking at its direction",
    (_, cameraM, direction) => {
      const asked: Vec3[] = [];
      const heightM = heightAboveGroundM(cameraM, OBLATE, (d) => {
        asked.push(d);
        return null;
      });
      expect([heightM, asked]).toEqual([expect.closeTo(4_000, 9), [direction]]);
    },
  );

  it("measures along the spheroid's normal at 40° to 1 mm", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = OBLATE;
    const beta = 40 * RAD;
    const lon = 30 * RAD;
    const ground = vec3(
      a * Math.cos(beta) * Math.cos(lon),
      a * Math.cos(beta) * Math.sin(lon),
      c * Math.sin(beta),
    );
    const normal = normalise(vec3(ground.x / (a * a), ground.y / (a * a), ground.z / (c * c)));
    const cameraM = vec3(
      ground.x + 4_000 * normal.x,
      ground.y + 4_000 * normal.y,
      ground.z + 4_000 * normal.z,
    );
    expect(heightAboveGroundM(cameraM, OBLATE, () => null)).toBeCloseTo(4_000, 3);
  });

  it("asks at the ground point's parametric direction (x ÷ a, y ÷ a, z ÷ c), a unit vector towards the camera's", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = OBLATE;
    const cameraM = vec3(3.1e6, 2.2e6, 4.9e6);
    const asked: Vec3[] = [];
    heightAboveGroundM(cameraM, OBLATE, (d) => {
      asked.push(d);
      return 0;
    });
    const expected = normalise(vec3(cameraM.x / a, cameraM.y / a, cameraM.z / c));
    const d = asked[0] ?? vec3(Number.NaN, Number.NaN, Number.NaN);
    expect([
      Math.hypot(d.x, d.y, d.z),
      d.x - expected.x,
      d.y - expected.y,
      d.z - expected.z,
    ]).toEqual([
      expect.closeTo(1, 14),
      expect.closeTo(0, 14),
      expect.closeTo(0, 14),
      expect.closeTo(0, 14),
    ]);
  });
});

describe("the label block's CONTOUR INTERVAL reading (the guide's Views)", () => {
  it.each([
    [1, "1 m · INDEX 5 m"],
    [2, "2 m · INDEX 10 m"],
    [5, "5 m · INDEX 25 m"],
    [100, "100 m · INDEX 500 m"],
    [200, "200 m · INDEX 1 km"],
    [500, "500 m · INDEX 2.5 km"],
    [1_000, "1 km · INDEX 5 km"],
    [2_000, "2 km · INDEX 10 km"],
  ])("reads an interval of %s m as %s", (intervalM, reading) => {
    expect(contourIntervalReading(intervalM)).toBe(reading);
  });

  it.each([
    [999, "999 m"],
    [1_000, "1 km"],
    [2_500, "2.5 km"],
  ])("states a length of %s m as %s", (lengthM, reading) => {
    expect(contourLengthReading(lengthM)).toBe(reading);
  });

  it("sizes the interval's field for its longest reading, 10000 km", () => {
    expect([CONTOUR_FIELD_CH, contourLengthReading(10_000_000)]).toEqual([8, "10000 km"]);
  });
});

/** Each storage buffer's name in the WGSL. */
const WGSL_NAMES = new Map([
  ["heights", "heights"],
  ["slots", "slots"],
  ["instances", "instances"],
  ["contacts", "contacts"],
  ["offsets", "offsets"],
  ["surveyMask", "survey_mask"],
  ["wirePhases", "wire_phases"],
]);

/** A struct layout's members as `name : type @ offset`, or nothing where no struct is declared. */
function membersOf(layout: StructLayout | null): string[] | undefined {
  return layout?.members.map((m) => `${m.name} : ${m.type} @ ${String(m.offsetBytes)}`);
}

describe.each(["baked-offsets", "face-differences"] as const)(
  "the wireframe terrain's material on the %s path",
  (path) => {
    const spec = terrainWireMaterialSpec(path);
    const storage = spec.storageBuffers ?? [];

    it("is named for its path", () => {
      expect([spec.name, spec.displayName]).toEqual([
        `terrain-wire-${path}`,
        path === "baked-offsets" ? "WIREFRAME TERRAIN OFFSETS" : "WIREFRAME TERRAIN",
      ]);
    });

    it("draws opaque into the canvas with depth, back faces culled", () => {
      expect([spec.depthWrite, spec.colourWrites, spec.blend, spec.cullMode]).toEqual([
        true,
        true,
        "none",
        "back",
      ]);
    });

    it("binds no texture and no sampler", () => {
      expect([spec.textures, spec.samplers]).toEqual([[], []]);
    });

    it("binds R05's vertex storage, the survey mask at 17 and the phases at 18, each to the stages that read it", () => {
      const read = bindingStages(spec.vertexWgsl);
      const offsets = path === "baked-offsets" ? [["offsets", 5, "vertex", "vertex"]] : [];
      expect(
        storage.map((buffer) => [
          buffer.name,
          buffer.binding,
          [...(buffer.stages ?? [])].toSorted().join(" "),
          [...(read.get(buffer.binding) ?? [])].toSorted().join(" "),
        ]),
      ).toEqual([
        ["heights", 0, "vertex", "vertex"],
        ["slots", 1, "vertex", "vertex"],
        ["instances", 2, "vertex", "vertex"],
        ["contacts", 3, "vertex", "vertex"],
        ...offsets,
        ["surveyMask", 17, "fragment vertex", "fragment vertex"],
        ["wirePhases", 18, "vertex", "vertex"],
      ]);
    });

    it("leaves no binding its entry points read out of its layout", () => {
      const declared = new Set(storage.map((buffer) => buffer.binding));
      expect([...bindingStages(spec.vertexWgsl).keys()].filter((b) => !declared.has(b))).toEqual(
        [],
      );
    });

    it("declares each storage buffer at its binding in the WGSL", () => {
      const undeclared = storage.filter(
        (buffer) =>
          !new RegExp(
            `@binding\\(${String(buffer.binding)}\\) var[^\\n]* ${WGSL_NAMES.get(buffer.name) ?? "?"}\\b`,
            "u",
          ).test(spec.vertexWgsl),
      );
      expect(undeclared.map((buffer) => buffer.name)).toEqual([]);
    });

    it("takes the lit material's draw and atlas, then the contours', graticule's and colours' uniforms", () => {
      expect(spec.uniforms.map((u) => `${u.name} : ${u.type}`)).toEqual([
        "bodyRotation : mat4x4f",
        "atlas : vec4f",
        "contour : vec4f",
        "contourWidthsPx : vec4f",
        "graticule : vec4f",
        "muted : vec4f",
        "text : vec4f",
        "casing : vec4f",
        "occluderSlopePx : f32",
      ]);
    });

    it("declares the Draw struct the engine packs from its uniforms", () => {
      expect(membersOf(declaredStructLayout(spec.vertexWgsl, "Draw"))).toEqual(
        membersOf(uniformLayout([OFFSET_MEMBER, ...spec.uniforms])),
      );
    });
  },
);

describe("the wireframe terrain's materials in the catalogue", () => {
  it("registers both paths' materials", () => {
    expect(
      TERRAIN_WIRE_MATERIALS.map((spec) =>
        WGSL_CATALOGUE.some((entry) => entry.kind === "material" && entry.spec === spec),
      ),
    ).toEqual([true, true]);
  });
});

/** A patch's place with distinct values in every word. */
const PLACE_A: GraticulePatch = {
  radiusM: 6_378_137.3,
  latPhaseRad: 0.01,
  lonPhaseRad: -0.02,
  latIndex: 3,
  lonIndex: -7,
  sinLat: 0.5,
  cosLat: Math.sqrt(3) / 2,
  sinLon: -0.6,
  cosLon: 0.8,
};
const PLACE_B: GraticulePatch = {
  radiusM: 1_737_400.1,
  latPhaseRad: -0.1,
  lonPhaseRad: 0.2,
  latIndex: -5,
  lonIndex: 11,
  sinLat: -0.28,
  cosLat: 0.96,
  sinLon: 0.6,
  cosLon: -0.8,
};

describe("the wireframe's phase records (uniforms.ts' WirePhaseRecords)", () => {
  it("are 48 bytes, the WGSL's WirePhase of three vec4f", () => {
    const layout = declaredStructLayout(terrainWireWgsl, "WirePhase");
    expect([
      WIRE_PHASE_RECORD_BYTES,
      layout?.sizeBytes,
      layout?.members.map((m) => m.type),
    ]).toEqual([48, 48, ["vec4f", "vec4f", "vec4f"]]);
  });

  it("hold each record's words in the WGSL's order, narrowed to f32", () => {
    const records = new WirePhaseRecords(2);
    records.push(12.5, -340.25, PLACE_A);
    records.push(7.3, 1_234.567, PLACE_B);
    const words = (phase: number, aboveSea: number, p: GraticulePatch): number[] =>
      [
        phase,
        aboveSea,
        p.radiusM,
        0,
        p.latPhaseRad,
        p.lonPhaseRad,
        p.latIndex,
        p.lonIndex,
        p.sinLat,
        p.cosLat,
        p.sinLon,
        p.cosLon,
      ].map(f);
    expect([...new Float32Array(records.buffer, 0, 24)]).toEqual([
      ...words(12.5, -340.25, PLACE_A),
      ...words(7.3, 1_234.567, PLACE_B),
    ]);
  });

  it("give the bytes of the records this frame, and none once cleared", () => {
    const records = new WirePhaseRecords(3);
    records.push(1, 2, PLACE_A);
    records.push(3, 4, PLACE_B);
    const before = [records.count, records.bytes().byteLength];
    records.clear();
    expect([...before, records.count, records.bytes().byteLength]).toEqual([2, 96, 0, 0]);
  });

  it("refuse a record past their capacity", () => {
    const records = new WirePhaseRecords(1);
    records.push(1, 2, PLACE_A);
    expect(() => {
      records.push(3, 4, PLACE_B);
    }).toThrow(RangeError);
  });

  it.each([0, 1.5, -2])("refuse a capacity of %s", (capacity) => {
    expect(() => new WirePhaseRecords(capacity)).toThrow(RangeError);
  });
});
