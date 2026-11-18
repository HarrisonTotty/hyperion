import { describe, expect, it } from "vitest";

import { vec3 } from "../../../geometry/vec3";
import type { PatchKey } from "../../terrain/patchKey";
import type { BakedPatch, TerrainLayers } from "../../terrain/workers/messages";
import { ResidentRocks } from "./residentRocks";
import { ROCK_WORDS, rockLayerOf } from "./rockLayer";

const KEY: PatchKey = { face: 2, level: 17, i: 5, j: 9 };

/** A bake of `KEY` at `generation`, with `rocks` rocks in its layers, or none where `null`. */
function bake(generation: number, rocks: number | null): BakedPatch {
  const layers: TerrainLayers = {
    palette: new Uint8Array(16),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(65 * 65),
    fieldLevel: 6,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
    photometry: new Float32Array(40),
    ...(rocks === null ? {} : { rocks: rockLayerOf(new Float64Array(ROCK_WORDS * rocks)) }),
  };
  return {
    key: KEY,
    generation,
    originM: vec3(1, 2, 3),
    heights: new Float32Array(),
    offsets: null,
    normals: new Float16Array(),
    heightRangeM: [0, 0],
    boundingRadiusM: 1,
    originHeightM: 0,
    skirtDepthM: 0,
    layers,
  };
}

describe("the resident rocks (R11.T4.a)", () => {
  it("holds a finest bake's rocks while it is resident, and forgets them with it", () => {
    const rocks = new ResidentRocks();
    rocks.take("k", bake(1, 3));
    expect(rocks.get("k")?.layer.count).toBe(3);
    expect(rocks.get("k")?.originM).toEqual(vec3(1, 2, 3));
    expect([rocks.size, rocks.bytes]).toEqual([1, 3 * ROCK_WORDS * 8]);
    const before = rocks.revision;
    rocks.forget("k");
    expect([rocks.get("k"), rocks.size, rocks.bytes]).toEqual([undefined, 0, 0]);
    expect(rocks.revision).toBeGreaterThan(before);
  });

  it("takes a rebake's rocks in place of the old, and a bake without them as none", () => {
    const rocks = new ResidentRocks();
    rocks.take("k", bake(1, 3));
    rocks.take("k", bake(2, 1));
    expect([rocks.get("k")?.generation, rocks.get("k")?.layer.count]).toEqual([2, 1]);
    expect(rocks.bytes).toBe(ROCK_WORDS * 8);
    rocks.take("k", bake(3, null));
    expect([rocks.get("k"), rocks.bytes]).toEqual([undefined, 0]);
  });

  it("changes its revision only when a layer is taken or forgotten", () => {
    const rocks = new ResidentRocks();
    const start = rocks.revision;
    rocks.forget("absent");
    rocks.clear();
    expect(rocks.revision).toBe(start);
    rocks.take("a", bake(1, 0));
    rocks.take("b", bake(1, 2));
    rocks.clear();
    expect(rocks.revision).toBe(start + 3);
    expect(rocks.size).toBe(0);
  });
});
