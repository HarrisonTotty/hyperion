import { beforeAll, describe, expect, it } from "vitest";

import {
  bakePatch,
  initSync,
  NormalScale,
  Ridges,
  VertexPath,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { type PatchKey, parentKey } from "./patchKey";
import { planetGeometry } from "./planet";
import { inheritedHeightRangeM } from "./select";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

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
