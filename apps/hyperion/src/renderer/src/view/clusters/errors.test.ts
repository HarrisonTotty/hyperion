/**
 * The cluster path's measured errors (plan R14, R14.T3.a): e_s bounds every stride's distance from
 * the stride-1 mesh at k = 0, ½ and 1, blocks and strips alike, on both vertex paths; it is
 * monotone; and a flat patch on the curved datum strides with an error growing as s².
 */

import { describe, expect, it } from "vitest";

import {
  gpuPlacement,
  type OracleBake,
  slotTermsF32,
  strideDistanceM,
} from "../../test/strideOracle";
import { seededRandom } from "../../test/seededRandom";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import { faceDifferenceMorphF32, faceDifferencePositionF32 } from "../terrain/gpu/vertexEmulation";
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry } from "../terrain/planet";
import { chordSagittaM } from "../terrain/select";
import { clusterErrorM, clusterErrors, ERRORS_PER_CLUSTER } from "./errors";
import {
  CLUSTER_PAD_FRACTION,
  CLUSTER_PAD_M,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
} from "./terrain";
import { STRIDES } from "./templates";

/** A Moon-sized spheroid of a slight flattening, so that the datum's normal varies with latitude. */
const MOON_LIKE_FIGURE: BodyFigure = {
  equatorialRadiusM: 1_737_400,
  polarRadiusM: 1_736_000,
  pole: null,
};

/**
 * A bake of `key` with smooth random relief of `reliefM` about a mean near the skirt depth (so that
 * feet bend), its morph targets the parent's mesh, on `face-differences` or, with `offsets`, on
 * `baked-offsets` with offsets the shader's own `f32` placement makes.
 */
function randomBake(
  figure: BodyFigure,
  key: PatchKey,
  seed: number,
  reliefM: number,
  offsets: boolean,
): OracleBake {
  const next = seededRandom(seed);
  const skirtDepthM = 5 + 50 * next();
  const waves = Array.from({ length: 6 }, () => ({
    a: reliefM * next(),
    fx: 1 + 12 * next(),
    fy: 1 + 12 * next(),
    phase: 2 * Math.PI * next(),
  }));
  const relief = (x: number, y: number): number =>
    skirtDepthM +
    waves.reduce((sum, w) => sum + w.a * Math.sin((w.fx * x + w.fy * y) / 64 + w.phase), 0) +
    0.1 * reliefM * (next() - 0.5);
  const heights = new Float32Array(2 * 65 * 65);
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      heights[2 * (y * 65 + x)] = relief(x, y);
    }
  }
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      const v = y * 65 + x;
      // The parent's own height at an even vertex; a chord's at an odd one.
      const even = x % 2 === 0 && y % 2 === 0;
      heights[2 * v + 1] =
        even || key.level === 0 ? (heights[2 * v] ?? 0) + reliefM * 0.2 * (next() - 0.5) : 0;
    }
  }
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      if (key.level === 0 || (x % 2 === 0 && y % 2 === 0)) {
        continue;
      }
      const ax = x % 2 === 1 ? x - 1 : x;
      const ay = y % 2 === 1 ? y - 1 : y;
      const bx = x % 2 === 1 ? x + 1 : x;
      const by = y % 2 === 1 ? y + 1 : y;
      heights[2 * (y * 65 + x) + 1] =
        0.5 * ((heights[2 * (ay * 65 + ax) + 1] ?? 0) + (heights[2 * (by * 65 + bx) + 1] ?? 0));
    }
  }
  const originHeightM = heights[2 * (32 * 65 + 32)] ?? 0;
  let packed: Float32Array | null = null;
  if (offsets) {
    const rec = slotTermsF32(key, figure, originHeightM);
    packed = new Float32Array(6 * 65 * 65);
    for (let y = 0; y <= 64; y += 1) {
      for (let x = 0; x <= 64; x += 1) {
        const v = y * 65 + x;
        const q0 = faceDifferencePositionF32(rec, x, y, heights[2 * v] ?? 0);
        const q1 = faceDifferenceMorphF32(
          rec,
          x,
          y,
          (px, py) => heights[2 * (py * 65 + px) + 1] ?? 0,
        );
        packed.set([q0.x, q0.y, q0.z, q1.x, q1.y, q1.z], 6 * v);
      }
    }
  }
  return { key, figure, heights, offsets: packed, originHeightM, skirtDepthM };
}

/** The largest amount by which the oracle's distance passes e_s over `bake`'s clusters. */
function worstExcessM(bake: OracleBake, clusters: ReadonlyArray<number>): number {
  const errors = clusterErrors(bake, bake.key, planetGeometry(bake.figure, null));
  const place = gpuPlacement(bake);
  let worst = -Infinity;
  for (const cluster of clusters) {
    for (const stride of [2, 4, 8] as const) {
      for (const k of [0, 0.5, 1]) {
        const lattice = cluster < TERRAIN_BLOCKS ? 1 : 8;
        worst = Math.max(
          worst,
          strideDistanceM(place, cluster, stride, k, lattice) -
            clusterErrorM(errors, cluster, stride),
        );
      }
    }
  }
  return worst;
}

/** Some blocks and every strip of an edge, and a strip of each other edge. */
const SOME_CLUSTERS = [
  0,
  9,
  27,
  36,
  63,
  ...Array.from({ length: 8 }, (_, m) => 64 + m),
  75,
  83,
  95,
];

describe("the clusters' measured errors (plan R14, R14.T3.a)", () => {
  it.each([
    [
      "an Earth's level 3, face differences",
      WGS84_FIGURE,
      { face: 4, level: 3, i: 5, j: 2 },
      false,
    ],
    [
      "an Earth's level 12, baked offsets",
      WGS84_FIGURE,
      { face: 1, level: 12, i: 2_000, j: 3_100 },
      true,
    ],
    ["a Moon's root, face differences", MOON_LIKE_FIGURE, { face: 2, level: 0, i: 0, j: 0 }, false],
    [
      "a Moon's level 9, baked offsets",
      MOON_LIKE_FIGURE,
      { face: 5, level: 9, i: 300, j: 7 },
      true,
    ],
  ] as const)(
    "bound the stride-s mesh's distance from the stride-1 mesh at k = 0, ½ and 1 on %s",
    (_name, figure, key, offsets) => {
      for (const seed of [1, 2]) {
        const bake = randomBake(figure, key, seed, 400, offsets);
        expect(worstExcessM(bake, SOME_CLUSTERS)).toBeLessThanOrEqual(0);
      }
    },
  );

  it("are 0 at stride 1 and never fall as the stride grows", () => {
    const bake = randomBake(WGS84_FIGURE, { face: 0, level: 7, i: 40, j: 99 }, 3, 900, true);
    const errors = clusterErrors(bake, bake.key, planetGeometry(WGS84_FIGURE, null));
    expect(errors).toHaveLength(ERRORS_PER_CLUSTER * TERRAIN_CLUSTERS_PER_PAGE);
    for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
      const row = STRIDES.map((s) => clusterErrorM(errors, cluster, s));
      expect(row[0]).toBe(0);
      expect(row.every((e, n) => Number.isFinite(e) && e >= (row[n - 1] ?? 0))).toBe(true);
    }
  });

  it("take a block's largest at a stride-1 vertex, which the oracle finds, and the placement's allowance", () => {
    const bake = randomBake(MOON_LIKE_FIGURE, { face: 3, level: 6, i: 20, j: 41 }, 4, 300, true);
    const errors = clusterErrors(bake, bake.key, planetGeometry(MOON_LIKE_FIGURE, null));
    const place = gpuPlacement(bake);
    let reachM = 0;
    const offsets = bake.offsets ?? new Float32Array();
    for (let n = 0; n < offsets.length; n += 3) {
      reachM = Math.max(
        reachM,
        Math.hypot(offsets[n] ?? 0, offsets[n + 1] ?? 0, offsets[n + 2] ?? 0),
      );
    }
    const allowanceM = CLUSTER_PAD_M + CLUSTER_PAD_FRACTION * reachM;
    for (const cluster of [0, 18, 45, 63]) {
      let finer = 0;
      for (const stride of [2, 4, 8] as const) {
        finer = Math.max(
          finer,
          strideDistanceM(place, cluster, stride, 0),
          strideDistanceM(place, cluster, stride, 1),
        );
        const e = clusterErrorM(errors, cluster, stride);
        expect(e).toBeGreaterThanOrEqual(finer + CLUSTER_PAD_M);
        expect(e).toBeLessThanOrEqual(finer + allowanceM + 1e-9);
      }
    }
  });

  it("add a foot's bend where its height crosses the skirt depth between k = 0 and 1", () => {
    // Along edge 0, h₀ − d runs −200, −100, 0, −100 m and repeats, a chord of its even vertices,
    // and h₁ − d is 100 m throughout: the stride-2 feet and those between them meet at k = 0 and
    // k = 1, where a foot's height min(h − d, 0) is a chord of its neighbours', and at k = ½ a foot
    // between them stands at the datum while its neighbours' mean is 25 m below it.
    const key: PatchKey = { face: 2, level: 10, i: 300, j: 511 };
    const skirtDepthM = 30;
    const heights = new Float32Array(2 * 65 * 65);
    for (let x = 0; x <= 64; x += 1) {
      heights[2 * x] = skirtDepthM + ([-200, -100, 0, -100][x % 4] ?? 0);
      heights[2 * x + 1] = skirtDepthM + 100;
    }
    const bake: OracleBake = {
      key,
      figure: WGS84_FIGURE,
      heights,
      offsets: null,
      originHeightM: 0,
      skirtDepthM,
    };
    const errors = clusterErrors(bake, key, planetGeometry(WGS84_FIGURE, null));
    const place = gpuPlacement(bake);
    const strip = TERRAIN_BLOCKS;
    expect(strideDistanceM(place, strip, 2, 0, 8)).toBeLessThan(1);
    expect(strideDistanceM(place, strip, 2, 1, 8)).toBeLessThan(1);
    expect(strideDistanceM(place, strip, 2, 0.5, 8)).toBeGreaterThan(24);
    expect(clusterErrorM(errors, strip, 2)).toBeGreaterThanOrEqual(
      strideDistanceM(place, strip, 2, 0.5, 8),
    );
  });

  it("refuse a bake whose heights or offsets are not a patch's", () => {
    const planet = planetGeometry(WGS84_FIGURE, null);
    const key: PatchKey = { face: 0, level: 4, i: 3, j: 9 };
    const heights = new Float32Array(2 * 65 * 65);
    expect(() =>
      clusterErrors({ heights: heights.subarray(1), offsets: null, skirtDepthM: 1 }, key, planet),
    ).toThrow(RangeError);
    expect(() =>
      clusterErrors(
        { heights, offsets: new Float32Array(3 * 65 * 65), skirtDepthM: 1 },
        key,
        planet,
      ),
    ).toThrow(RangeError);
  });

  it("stride a flat patch on the curved datum with an error growing as s², as its chords sag", () => {
    const key: PatchKey = { face: 1, level: 6, i: 17, j: 30 };
    const heights = new Float32Array(2 * 65 * 65);
    const planet = planetGeometry(WGS84_FIGURE, null);
    const errors = clusterErrors({ heights, offsets: null, skirtDepthM: 10 }, key, planet);
    const sagM = chordSagittaM(planet, key.level);
    for (const cluster of [0, 27, 63]) {
      const [e1, e2, e4, e8] = STRIDES.map((s) => clusterErrorM(errors, cluster, s));
      expect(e1).toBe(0);
      expect(e2).toBeGreaterThan(0);
      expect((e4 ?? 0) / (e2 ?? 1)).toBeGreaterThan(3.5);
      expect((e4 ?? 0) / (e2 ?? 1)).toBeLessThan(4.5);
      expect((e8 ?? 0) / (e4 ?? 1)).toBeGreaterThan(3.5);
      expect((e8 ?? 0) / (e4 ?? 1)).toBeLessThan(4.5);
      // A stride-8 chord is eight of the level's spacings: its sag is under 64 times the level's.
      expect(e8).toBeLessThan(64 * sagM);
      expect(e8).toBeGreaterThan(16 * sagM);
    }
  });
});
