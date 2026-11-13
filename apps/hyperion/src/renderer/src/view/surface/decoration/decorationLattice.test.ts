import { describe, expect, it } from "vitest";

import type { PatchKey } from "../../terrain/patchKey";
import { bandLimitM } from "../../terrain/planet";
import {
  DECORATION_CLASSES,
  DECORATION_OCTAVES,
  type DecorationMix,
  type DecorationTexelInput,
  decorateTexel,
  decorationLevel,
  decorationPxPerRad,
  decorationSeed,
  FACE_ARC_PER_S,
  gradientNoise,
  largestCellM,
  latticeCellM,
  latticeHash,
  latticePlace,
  latticePrefix,
  lowbias32,
  MAX_LATTICE_LEVEL,
  NO_DECORATION_MIX,
  octaveFade,
  perturbNormal,
  RIPPLE_LEVEL_OFFSET,
  RIPPLE_WAVELENGTH_M,
  mixOf,
  rippleOf,
  SCAR_LEVEL_OFFSET,
  SCARS_PER_CELL,
  scarShares,
} from "./decorationLattice";

/** R05's band limit, m, read as every caller reads it. */
const BAND_LIMIT_M = bandLimitM();

/** 1080p's pixel scale at a 60° field. */
const PIXEL_SCALE = decorationPxPerRad({ viewport: { widthPx: 1920 }, fovXRad: Math.PI / 3 });

const MOON_RADIUS_M = 1_737_400;
const EARTH_RADIUS_M = 6_378_137;

/** A finest-level patch of a Moon-like body, 24 m across, away from the face's centre. */
const MOON_PATCH: PatchKey = { face: 2, level: 17, i: 101_234, j: 77_001 };

function mixOfClass(code: number): DecorationMix {
  return DECORATION_CLASSES[code] ?? NO_DECORATION_MIX;
}

/** A texel input on `key` at (u, v), on a body of radius `radiusM`. */
function texel(
  key: PatchKey,
  u: number,
  v: number,
  radiusM: number,
  overrides: Partial<DecorationTexelInput> = {},
): DecorationTexelInput {
  const level = decorationLevel(radiusM, BAND_LIMIT_M);
  const side = 2 ** key.level;
  return {
    seed: decorationSeed("0123456789abcdef"),
    level,
    key,
    u,
    v,
    cellM: latticeCellM((key.i + u) / side, (key.j + v) / side, radiusM, level),
    distanceM: 0,
    pxPerRad: PIXEL_SCALE,
    mix: mixOfClass(0),
    ripple: null,
    scarShares: [1, 1, 1, 1],
    ...overrides,
  };
}

/** The means of the albedo factor less 1 and of the two slopes over a patch, on an n × n grid. */
function patchMeans(
  n: number,
  at: (u: number, v: number) => DecorationTexelInput,
): readonly [number, number, number] {
  let albedo = 0;
  let x = 0;
  let y = 0;
  for (let a = 0; a < n; a += 1) {
    for (let b = 0; b < n; b += 1) {
      const d = decorateTexel(at((a + 0.5) / n, (b + 0.5) / n));
      albedo += d.albedo - 1;
      x += d.slope[0];
      y += d.slope[1];
    }
  }
  const count = n * n;
  return [albedo / count, x / count, y / count];
}

describe("the lattice hash", () => {
  it("is lowbias32 in u32 arithmetic, 0 at 0 and in [0, 2³²) everywhere", () => {
    expect(lowbias32(0)).toBe(0);
    for (const x of [1, 2, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff]) {
      const h = lowbias32(x);
      expect(Number.isInteger(h) && h >= 0 && h < 2 ** 32).toBe(true);
    }
  });

  it("keys each point by seed, salt, face, level and place apart", () => {
    const base = latticeHash(1, 1, 0, 20, 5, 7);
    expect(
      [
        latticeHash(2, 1, 0, 20, 5, 7),
        latticeHash(1, 2, 0, 20, 5, 7),
        latticeHash(1, 1, 1, 20, 5, 7),
        latticeHash(1, 1, 0, 21, 5, 7),
        latticeHash(1, 1, 0, 20, 6, 7),
        latticeHash(1, 1, 0, 20, 5, 8),
        latticeHash(1, 1, 0, 20, 7, 5),
      ].every((h) => h !== base),
    ).toBe(true);
  });

  it("is the prefix's point hash, so that an octave forms its prefix once", () => {
    expect(latticeHash(9, 3, 4, 25, 1_000_001, 3)).toBe(latticeHash(9, 3, 4, 25, 1_000_001, 3));
    expect(latticeHash(9, 3, 4, 25, 1_000_001, 3)).toBe(
      lowbias32((lowbias32((latticePrefix(9, 3, 4, 25) ^ 1_000_001) >>> 0) ^ 3) >>> 0),
    );
  });

  it("draws a body's seed from its detail seed, and refuses one that is not 16 hex digits", () => {
    expect(decorationSeed("0123456789abcdef")).toBe(decorationSeed("0123456789abcdef"));
    expect(decorationSeed("0123456789abcdef")).not.toBe(decorationSeed("0123456789abcdee"));
    expect(decorationSeed("0123456789abcdef")).not.toBe(decorationSeed("1123456789abcdef"));
    expect(() => decorationSeed("0123456789ABCDEF")).toThrow(RangeError);
    expect(() => decorationSeed("0123")).toThrow(RangeError);
  });
});

describe("a texel's place on a lattice", () => {
  it("is the same point of the face from two patches that meet at an edge", () => {
    const left: PatchKey = { face: 1, level: 17, i: 400, j: 900 };
    const right: PatchKey = { face: 1, level: 17, i: 401, j: 900 };
    for (const level of [15, 17, 22, 27]) {
      const a = latticePlace(left, 1, 0.25, level);
      const b = latticePlace(right, 0, 0.25, level);
      expect([a.x + a.fx, a.y + a.fy]).toEqual([b.x + b.fx, b.y + b.fy]);
    }
  });

  it("is the same from a patch and from its child, at any level", () => {
    const parent: PatchKey = { face: 3, level: 16, i: 1_000, j: 2_001 };
    const child: PatchKey = { face: 3, level: 17, i: 2_001, j: 4_002 };
    for (const level of [14, 16, 20, 26]) {
      const a = latticePlace(parent, 0.5 + 0.25 / 2, 0.125 / 2, level);
      const b = latticePlace(child, 0.25, 0.125, level);
      expect([a.x, a.y, a.fx, a.fy]).toEqual([b.x, b.y, b.fx, b.fy]);
    }
  });

  it("forms its cell in integers, its fraction in [0, 1)", () => {
    const place = latticePlace({ face: 0, level: 19, i: 524_287, j: 3 }, 0.999, 0.001, 30);
    expect(place.x).toBe(524_287 * 2 ** 11 + Math.floor(0.999 * 2 ** 11));
    expect(place.fx >= 0 && place.fx < 1 && place.fy >= 0 && place.fy < 1).toBe(true);
  });

  it("refuses a level past 31", () => {
    expect(() => latticePlace(MOON_PATCH, 0, 0, MAX_LATTICE_LEVEL + 1)).toThrow(RangeError);
  });
});

describe("the octaves and the band limit", () => {
  it.each([2e5, MOON_RADIUS_M, 3.39e6, EARTH_RADIUS_M, 1.6e7])(
    "sums no octave above the band limit on a body of radius %f m",
    (radiusM) => {
      const level = decorationLevel(radiusM, BAND_LIMIT_M);
      // The coarsest noise octave's cells, about half its wavelength, are at most half the limit.
      expect(largestCellM(radiusM, level)).toBeLessThanOrEqual(BAND_LIMIT_M / 2);
      // And the next coarser lattice's would not be: the level is the least that keeps it.
      expect(largestCellM(radiusM, level - 1)).toBeGreaterThan(BAND_LIMIT_M / 2);
      // The largest scar is half its lattice's cell across, the coarsest scar lattice two levels up.
      const scarCellM = largestCellM(radiusM, level - SCAR_LEVEL_OFFSET);
      expect(scarCellM / 2).toBeLessThanOrEqual(BAND_LIMIT_M);
      expect(RIPPLE_WAVELENGTH_M).toBeLessThan(BAND_LIMIT_M);
      expect(level + DECORATION_OCTAVES - 1).toBeLessThanOrEqual(MAX_LATTICE_LEVEL);
    },
  );

  it("bounds a cell's width anywhere on a face by FACE_ARC_PER_S", () => {
    let largest = 0;
    for (let a = 0; a <= 200; a += 1) {
      for (let b = 0; b <= 200; b += 1) {
        const [x, y] = latticeCellM(a / 200, b / 200, 1, 0);
        largest = Math.max(largest, x, y);
      }
    }
    expect(largest).toBeLessThanOrEqual(FACE_ARC_PER_S);
    expect(largest).toBeGreaterThan(FACE_ARC_PER_S - 1e-3);
  });

  it("refuses a body whose finest lattice would pass level 31", () => {
    expect(() => decorationLevel(1e9, BAND_LIMIT_M)).toThrow(RangeError);
  });
});

describe("the fade", () => {
  it("keeps an octave whole at 2 px, drops it at 1 px and fades it by log₂ between", () => {
    const at = (spanPx: number): number => octaveFade(spanPx, PIXEL_SCALE, PIXEL_SCALE);
    expect([at(4), at(2), at(1), at(0.5)]).toEqual([1, 1, 0, 0]);
    expect(at(Math.SQRT2)).toBeCloseTo(0.5, 12);
    expect(octaveFade(1, 0, PIXEL_SCALE)).toBe(1);
  });

  it("drops every octave of a lunar plain seen from 10 km, and keeps the coarsest from 100 m", () => {
    const far = decorateTexel(texel(MOON_PATCH, 0.3, 0.6, MOON_RADIUS_M, { distanceM: 10_000 }));
    expect(far).toEqual({ albedo: 1, slope: [0, 0] });
    const near = decorateTexel(texel(MOON_PATCH, 0.3, 0.6, MOON_RADIUS_M, { distanceM: 100 }));
    expect(near.albedo).not.toBe(1);
  });
});

describe("gradient noise", () => {
  it("is 0 at every lattice point and its rates are its own finite differences", () => {
    const prefix = latticePrefix(77, 1, 0, 20);
    expect(gradientNoise(prefix, { x: 5, y: 9, fx: 0, fy: 0 }, 0).value).toBe(0);
    const h = 1e-6;
    for (const [fx, fy] of [
      [0.2, 0.7],
      [0.5, 0.5],
      [0.9, 0.1],
    ] as const) {
      const at = gradientNoise(prefix, { x: 5, y: 9, fx, fy }, 3);
      const dx =
        (gradientNoise(prefix, { x: 5, y: 9, fx: fx + h, fy }, 3).value -
          gradientNoise(prefix, { x: 5, y: 9, fx: fx - h, fy }, 3).value) /
        (2 * h);
      const dy =
        (gradientNoise(prefix, { x: 5, y: 9, fx, fy: fy + h }, 3).value -
          gradientNoise(prefix, { x: 5, y: 9, fx, fy: fy - h }, 3).value) /
        (2 * h);
      expect(Math.abs(at.dx - dx)).toBeLessThan(1e-6);
      expect(Math.abs(at.dy - dy)).toBeLessThan(1e-6);
    }
  });

  it("joins its neighbouring cells with no step", () => {
    const prefix = latticePrefix(77, 1, 0, 20);
    const left = gradientNoise(prefix, { x: 5, y: 9, fx: 1, fy: 0.3 }, 0);
    const right = gradientNoise(prefix, { x: 6, y: 9, fx: 0, fy: 0.3 }, 0);
    expect(Math.abs(left.value - right.value)).toBeLessThan(1e-12);
    expect(Math.abs(left.dx - right.dx)).toBeLessThan(1e-12);
  });
});

describe("zero mean over a patch (Design note 3)", () => {
  // From 150 m the octaves of cells up to the third are summed and the finer ones dropped, so the
  // grid's four samples a cell of the finest summed octave average every field over the patch.
  it.each([
    ["Bedrock", 0],
    ["Regolith", 1],
    ["Dust", 14],
  ])("gives %s on a lunar patch a mean of each perturbation within 10⁻³", (_name, code) => {
    const level = decorationLevel(MOON_RADIUS_M, BAND_LIMIT_M);
    const cells = 2 ** (level - MOON_PATCH.level + 2);
    const means = patchMeans(4 * cells, (u, v) =>
      texel(MOON_PATCH, u, v, MOON_RADIUS_M, { distanceM: 150, mix: mixOfClass(code) }),
    );
    for (const mean of means) {
      expect(Math.abs(mean)).toBeLessThan(1e-3);
    }
  });

  it("gives a rippled sand patch a mean slope within 10⁻³", () => {
    const key: PatchKey = { face: 4, level: 19, i: 200_222, j: 311_111 };
    const level = decorationLevel(EARTH_RADIUS_M, BAND_LIMIT_M);
    const side = 2 ** key.level;
    const cellM = latticeCellM((key.i + 0.5) / side, (key.j + 0.5) / side, EARTH_RADIUS_M, level);
    const rippleCells = RIPPLE_WAVELENGTH_M / (cellM[0] / 2 ** RIPPLE_LEVEL_OFFSET);
    const ripple = rippleOf(Math.cos(0.4), Math.sin(0.4), rippleCells);
    const sandRipplesOnly: DecorationMix = { roughness: 0, albedo: 0, ripples: 1, scars: 0 };
    const means = patchMeans(1_024, (u, v) =>
      texel(key, u, v, EARTH_RADIUS_M, { distanceM: 2, mix: sandRipplesOnly, ripple }),
    );
    expect(Math.abs(means[1])).toBeLessThan(1e-3);
    expect(Math.abs(means[2])).toBeLessThan(1e-3);
  });

  it("draws ripples a tenth of a metre apart, about 10° at their steepest", () => {
    const key: PatchKey = { face: 4, level: 19, i: 200_222, j: 311_111 };
    const level = decorationLevel(EARTH_RADIUS_M, BAND_LIMIT_M);
    const side = 2 ** key.level;
    const cellM = latticeCellM((key.i + 0.5) / side, (key.j + 0.5) / side, EARTH_RADIUS_M, level);
    const ripple = rippleOf(1, 0, RIPPLE_WAVELENGTH_M / (cellM[0] / 2 ** RIPPLE_LEVEL_OFFSET));
    const sandRipplesOnly: DecorationMix = { roughness: 0, albedo: 0, ripples: 1, scars: 0 };
    let steepest = 0;
    for (let a = 0; a < 2_000; a += 1) {
      const d = decorateTexel(
        texel(key, a / 2_000, 0.5, EARTH_RADIUS_M, { mix: sandRipplesOnly, ripple }),
      );
      steepest = Math.max(steepest, Math.hypot(d.slope[0], d.slope[1]));
    }
    // π ÷ 18 at the crests, the wobble's quarter turn adding at most its share.
    expect(steepest).toBeGreaterThan(0.12);
    expect(steepest).toBeLessThan(0.3);
  });
});

/** Trask's equilibrium N(>D), km⁻², at a diameter in metres. */
function saturation(dM: number): number {
  return 0.079 / (dM / 1_000) ** 2;
}

describe("a patch's ripples", () => {
  it("are none in a calm or at no wavelength", () => {
    expect(rippleOf(0, 0, 1.5)).toBeNull();
    expect(rippleOf(1, 0, 0)).toBeNull();
  });

  it("lie along the wind at a length of 8, their wavelength a whole count along it", () => {
    expect(rippleOf(1, 0, 1.5)).toEqual({ p: 8, q: 0, n: 12 });
    expect(rippleOf(0, -2, 1.5)).toEqual({ p: 0, q: -8, n: 12 });
  });
});

describe("the crater scars (Design note 20)", () => {
  it("hold Trask's equilibrium, 0.948 scars a saturated cell", () => {
    expect(SCARS_PER_CELL).toBeCloseTo(0.948, 12);
  });

  it("take each octave's share of saturation from the body's density, capped at 1", () => {
    expect(scarShares(saturation, 4)).toEqual([1, 1, 1, 1]);
    expect(scarShares((dM) => 2 * saturation(dM), 4)).toEqual([1, 1, 1, 1]);
    expect(scarShares((dM) => 0.25 * saturation(dM), 4).map((s) => s.toFixed(6))).toEqual([
      "0.250000",
      "0.250000",
      "0.250000",
      "0.250000",
    ]);
    expect(scarShares(() => 0, 4)).toEqual([0, 0, 0, 0]);
  });

  it("are none where the air screens every small crater", () => {
    const d = decorateTexel(
      texel(MOON_PATCH, 0.4, 0.4, MOON_RADIUS_M, {
        mix: { roughness: 0, albedo: 0, ripples: 0, scars: 1 },
        scarShares: [0, 0, 0, 0],
      }),
    );
    expect(d).toEqual({ albedo: 1, slope: [0, 0] });
  });

  it("change the slope alone, never the albedo: scars are normal detail only", () => {
    const scarsOnly: DecorationMix = { roughness: 0, albedo: 0, ripples: 0, scars: 1 };
    for (let a = 0; a < 100; a += 1) {
      const d = decorateTexel(
        texel(MOON_PATCH, (a + 0.5) / 100, 0.37, MOON_RADIUS_M, { mix: scarsOnly }),
      );
      expect(d.albedo).toBe(1);
    }
  });

  it("scar a share of a saturated patch", () => {
    const scarsOnly: DecorationMix = { roughness: 0, albedo: 0, ripples: 0, scars: 1 };
    const n = 400;
    let scarred = 0;
    for (let a = 0; a < n; a += 1) {
      for (let b = 0; b < n; b += 1) {
        const d = decorateTexel(
          texel(MOON_PATCH, (b + 0.5) / n, (a + 0.5) / n, MOON_RADIUS_M, { mix: scarsOnly }),
        );
        scarred += d.slope[0] !== 0 || d.slope[1] !== 0 ? 1 : 0;
      }
    }
    expect(scarred / (n * n)).toBeGreaterThan(0.05);
    expect(scarred / (n * n)).toBeLessThan(0.9);
  });

  it("end at their rims with no step", () => {
    // From 520 m only the coarsest scars, 0.6–1.2 m across, are drawn; 4,000 samples a row, 6 mm
    // apart, see a bowl's slope change by at most 8δ(1 − 3ρ²) ÷ R a metre, about 0.016 a sample.
    const scarsOnly: DecorationMix = { roughness: 0, albedo: 0, ripples: 0, scars: 1 };
    const n = 4_000;
    let largestStep = 0;
    let rims = 0;
    for (let row = 0; row < 10; row += 1) {
      let previous = 0;
      for (let b = 0; b < n; b += 1) {
        const d = decorateTexel(
          texel(MOON_PATCH, (b + 0.5) / n, (row + 0.5) / 10, MOON_RADIUS_M, {
            mix: scarsOnly,
            distanceM: 520,
          }),
        );
        const slope = Math.hypot(d.slope[0], d.slope[1]);
        if (b > 0) {
          largestStep = Math.max(largestStep, Math.abs(slope - previous));
          rims += (slope === 0) !== (previous === 0) ? 1 : 0;
        }
        previous = slope;
      }
    }
    expect(rims).toBeGreaterThan(10);
    expect(largestStep).toBeLessThan(0.03);
  });
});

describe("the perturbed normal", () => {
  it("tilts away from a rising slope and stays unit length", () => {
    const n = perturbNormal([0, 0, 1], [1, 0, 0], [0, 1, 0], [0.1, -0.2]);
    expect(Math.hypot(...n)).toBeCloseTo(1, 12);
    expect(n[0]).toBeLessThan(0);
    expect(n[1]).toBeGreaterThan(0);
    expect(perturbNormal([0, 0, 1], [1, 0, 0], [0, 1, 0], [0, 0])).toEqual([0, 0, 1]);
  });
});

describe("the classes' mixes", () => {
  it("give every one of MaterialClass's fifteen codes a mix, Seabed none", () => {
    expect(DECORATION_CLASSES).toHaveLength(15);
    expect(DECORATION_CLASSES[12]).toEqual(NO_DECORATION_MIX);
  });

  it("blend by shares, the rest the reference surface's, an unknown code taking none", () => {
    const bedrock = mixOfClass(0);
    const sand = mixOfClass(3);
    const blend = mixOf([0, 3], [0.25, 0.5]);
    expect(blend.roughness).toBeCloseTo(0.25 * bedrock.roughness + 0.5 * sand.roughness, 15);
    expect(blend.ripples).toBeCloseTo(0.5, 15);
    expect(mixOf([99], [1])).toEqual(NO_DECORATION_MIX);
    expect(mixOf([0], [1])).toEqual(bedrock);
  });

  it("ripple only sand", () => {
    expect(DECORATION_CLASSES.flatMap((mix, code) => (mix.ripples > 0 ? [code] : []))).toEqual([3]);
  });
});
