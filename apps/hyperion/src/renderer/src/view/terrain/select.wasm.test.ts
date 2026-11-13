import { beforeAll, describe, expect, it } from "vitest";

import {
  bakePatch,
  initSync,
  NormalScale,
  Ridges,
  SurfaceFields,
  testPlanetVersion,
  VertexPath,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { moonLikeFixture } from "../../test/moonLikeFixture";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { MAX_LEVEL, type PatchKey, parentKey } from "./patchKey";
import {
  F32_RELATIVE_STEP,
  levelBoundM,
  levelHeightRangeM,
  type PlanetGeometry,
  planetGeometry,
  subtreeHeightRangeM,
} from "./planet";
import { inheritedHeightRangeM } from "./select";
import { answerRequest, type HeightModule, WorkerFields } from "./workers/heightBake";
import type { BakedPatch, BakeSettings, HeightWorkerRequest } from "./workers/messages";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/**
 * A figure for the Moon-like fixture's table: its mean radius, 1,737.4 km, as a sphere. The subtree
 * ranges read the table alone.
 */
const MOON_FIGURE = { equatorialRadiusM: 1_737_400, polarRadiusM: 1_737_400, pole: null };

/** The baked heights of `key` (own and morph target, interleaved) and its baked range, metres. */
function bake(
  key: PatchKey,
  skirtM = 0,
): { heights: Float32Array; range: readonly [number, number]; skirtDepthM: number } {
  const patch = bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    VertexPath.FaceDifferences,
    NormalScale.Mesh,
    Ridges.Off,
    skirtM,
  );
  try {
    const [low, high] = patch.heightRangeM();
    return {
      heights: patch.heights(),
      range: [low ?? NaN, high ?? NaN],
      skirtDepthM: patch.skirtDepthM,
    };
  } finally {
    patch.free();
  }
}

/** The heights of the skirts' bottoms: each edge vertex's own height and morph target, less the skirt. */
function skirtBottoms(heights: Float32Array, skirtDepthM: number): number[] {
  const bottoms: number[] = [];
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      if (x === 0 || y === 0 || x === 64 || y === 64) {
        const at = 2 * (65 * y + x);
        bottoms.push((heights[at] ?? NaN) - skirtDepthM, (heights[at + 1] ?? NaN) - skirtDepthM);
      }
    }
  }
  return bottoms;
}

/** The ancestor of `key` at `level`. */
function ancestorAt(key: PatchKey, level: number): PatchKey {
  let k = key;
  while (k.level > level) {
    const up = parentKey(k);
    if (up === null) {
      break;
    }
    k = up;
  }
  return k;
}

describe("height ranges inherited from a baked ancestor", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
  const leaves: PatchKey[] = [
    { face: 0, level: 12, i: 1_234, j: 2_345 },
    { face: 2, level: 14, i: 9_001, j: 3_210 },
    { face: 4, level: 10, i: 511, j: 512 },
  ];

  it.each(leaves.flatMap((leaf) => [2, 4].map((up) => [leaf, leaf.level - up] as const)))(
    "holds every baked height of %o under its ancestor at level %i",
    (leaf, level) => {
      const ancestor = ancestorAt(leaf, level);
      const { range } = bake(ancestor);
      const inherited = inheritedHeightRangeM(planet, leaf, {
        level: ancestor.level,
        lowM: range[0],
        highM: range[1],
      });
      const { heights } = bake(leaf);
      const escapes = [...heights].filter((h) => h < inherited[0] || h > inherited[1]);
      expect(escapes).toEqual([]);
    },
  );

  it.each([0, 5])("reaches down to the skirts' bottoms with a skirt margin of %i m", (skirtM) => {
    for (const leaf of leaves) {
      const ancestor = ancestorAt(leaf, leaf.level - 2);
      const { range } = bake(ancestor, skirtM);
      const inherited = inheritedHeightRangeM(
        planet,
        leaf,
        { level: ancestor.level, lowM: range[0], highM: range[1] },
        skirtM,
      );
      const { heights, skirtDepthM } = bake(leaf, skirtM);
      expect(skirtBottoms(heights, skirtDepthM).filter((h) => h < inherited[0])).toEqual([]);
    }
  });

  it("tightens a patch's range against its level's", () => {
    const leaf = leaves[0];
    if (leaf === undefined) {
      throw new Error("no leaf");
    }
    const ancestor = ancestorAt(leaf, leaf.level - 2);
    const { range } = bake(ancestor);
    const inherited = inheritedHeightRangeM(planet, leaf, {
      level: ancestor.level,
      lowM: range[0],
      highM: range[1],
    });
    const level = inheritedHeightRangeM(planet, leaf, null);
    expect(inherited[1] - inherited[0]).toBeLessThan((level[1] - level[0]) / 4);
  });
});

/** The module as a height worker loads it. */
const MODULE: HeightModule = {
  bakePatch,
  testPlanetVersion,
  createFields: () => new SurfaceFields(),
};

/** The Moon-like fixture, both payloads, the whole field a session holds. */
const MOON = moonLikeFixture("all");

/** The Moon-like fixture's fields, held through the worker's own answers, and its last table. */
function moonFields(): { fields: WorkerFields; levels: Float64Array } {
  const fields = new WorkerFields(MODULE);
  let levels: Float64Array | null = null;
  for (const [n, block] of MOON.blocks.entries()) {
    const request: HeightWorkerRequest =
      n === 0
        ? { kind: "field-init", id: n, body: MOON.body, block, form: "payload", sigma: "skipped" }
        : { kind: "field-chunk", id: n, body: MOON.body, block };
    const { reply } = answerRequest(MODULE, request, fields);
    if (reply.kind !== "field-held" || reply.block === null) {
      throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
    }
    levels = reply.block.levels;
  }
  if (levels === null) {
    throw new Error("the fixture has no block");
  }
  return { fields, levels };
}

/**
 * The levels whose subtree range misses a row, each as `<level> over <row>`: every level's should
 * hold every row from its parent's to the finest level drawn and every skirt the bake hangs there,
 * with a margin of `skirtM`. A level-m patch hangs its skirts from its own heights and its morph
 * targets, its parent level's, by ε_m, an `f32` step of the largest |h| over both rows and the
 * margin (`hyperion_surface::patch`).
 */
function subtreeEscapes(planet: PlanetGeometry, skirtM: number): string[] {
  const escapes: string[] = [];
  // Every level drawn, the planet's finest the deepest.
  for (let n = 0; n <= planet.finestLevel; n += 1) {
    const [low, high] = subtreeHeightRangeM(planet, n, skirtM);
    for (let m = Math.max(0, n - 1); m <= planet.finestLevel; m += 1) {
      const [rowLow, rowHigh] = levelHeightRangeM(planet, m);
      const [parentLow, parentHigh] = levelHeightRangeM(planet, Math.max(0, m - 1));
      const largest = Math.max(...[rowLow, rowHigh, parentLow, parentHigh].map(Math.abs));
      const bottom =
        Math.min(rowLow, parentLow) - levelBoundM(planet, m) - largest * F32_RELATIVE_STEP - skirtM;
      if (low > bottom || high < rowHigh) {
        escapes.push(`${String(n)} over ${String(m)}`);
      }
    }
  }
  return escapes;
}

/**
 * A table of flat rows of ±1 km and ε of 10 m but where level 5's row falls to −5 km and level 6's
 * ε rises to 3 km: a parent row deeper than its child's under an ε that rises with the level, so
 * that only the bake's pairing, the child's ε under the parent's row, holds level 6's skirts.
 */
function risingBoundTable(): Float64Array {
  const table = new Float64Array(4 * (MAX_LEVEL + 1));
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    table.set([level === 6 ? 3_000 : 10, level === 5 ? -5_000 : -1_000, 1_000, 1], 4 * level);
  }
  return table;
}

describe("a level's subtree range", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it.each((["off", "on"] as const).flatMap((ridges) => [0, 5].map((m) => [ridges, m] as const)))(
    "holds every finer row and skirt bottom of the test planet, ridges %s, margin %i m",
    (ridges, m) => {
      expect(subtreeEscapes(planetGeometry(WGS84_FIGURE, goldenLevelTable(ridges)), m)).toEqual([]);
    },
  );

  it("holds the skirts of a level whose ε rises under a deeper parent row", () => {
    expect(subtreeEscapes(planetGeometry(MOON_FIGURE, risingBoundTable()), 0)).toEqual([]);
  });

  it("holds every finer row and skirt bottom of the Moon-like fixture's table", () => {
    const { levels } = moonFields();
    const planet = planetGeometry(MOON_FIGURE, levels);
    expect(subtreeEscapes(planet, 0)).toEqual([]);
  });

  it("holds every vertex and skirt bottom of level-0 to level-5 bakes over the Moon-like fixture", () => {
    // Each baked height, morph target and skirt bottom lies inside the box its own level's range
    // makes, and inside every coarser level's, which prunes it with its subtree (R09.T8.d).
    const { planet, bakes } = moonBakes();
    const escapes: string[] = [];
    for (const patch of bakes) {
      const level = patch.key.level;
      for (let coarser = 0; coarser <= level; coarser += 1) {
        const [low, high] = subtreeHeightRangeM(planet, coarser);
        const out = reachedHeights(patch).filter((h) => h < low || h > high);
        if (out.length > 0) {
          escapes.push(`level ${String(level)} under ${String(coarser)}: ${String(out.length)}`);
        }
      }
    }
    expect(escapes).toEqual([]);
  });

  it("holds every vertex and skirt bottom of a bake in the range inherited from it or its parent", () => {
    // A baked node's range is its baked range widened, clamped to its level's subtree range: at
    // level 0 the clamp once sat at row 0's low while the skirts hung ε₀ below it
    // (decision-r09-t8d-reach §7).
    const { planet, bakes } = moonBakes();
    const escapes: string[] = [];
    for (const patch of bakes.filter((b) => b.key.level <= 3)) {
      const level = patch.key.level;
      const parent = bakes.find((b) => b.key.level === level - 1);
      for (const ancestor of parent === undefined ? [patch] : [patch, parent]) {
        const [lowM, highM] = ancestor.heightRangeM;
        const [low, high] = inheritedHeightRangeM(planet, patch.key, {
          level: ancestor.key.level,
          lowM,
          highM,
        });
        const out = reachedHeights(patch).filter((h) => h < low || h > high);
        if (out.length > 0) {
          const from = String(ancestor.key.level);
          escapes.push(`level ${String(level)} from ${from}: ${String(out.length)}`);
        }
      }
    }
    expect(escapes).toEqual([]);
  });
});

/** Every height a bake reaches: its vertices' own heights, morph targets and skirt bottoms. */
function reachedHeights(patch: BakedPatch): number[] {
  return [...patch.heights, ...skirtBottoms(patch.heights, patch.skirtDepthM)];
}

/**
 * The Moon-like fixture's planet and its bakes at levels 0 to 5 over the first survey, four cells
 * of level 6 at (22, 22) to (23, 23) of face +y, each the next's parent.
 */
function moonBakes(): { planet: PlanetGeometry; bakes: BakedPatch[] } {
  const { fields, levels } = moonFields();
  const planet = planetGeometry(MOON_FIGURE, levels);
  const settings: BakeSettings = {
    vertexPath: "face-differences",
    normals: "mesh",
    source: { kind: "body", body: MOON.body, detailSeed: MOON.seed, weights: "baked" },
  };
  const bakes: BakedPatch[] = [];
  for (let level = 0; level <= 5; level += 1) {
    const key: PatchKey = { face: 1, level, i: 22 >> (6 - level), j: 22 >> (6 - level) };
    const { reply } = answerRequest(
      MODULE,
      { kind: "bake", id: level, key, generation: 1, settings },
      fields,
    );
    if (reply.kind !== "baked") {
      throw new Error(`${JSON.stringify(key)} was not baked: ${JSON.stringify(reply)}`);
    }
    bakes.push(reply.bake);
  }
  return { planet, bakes };
}
