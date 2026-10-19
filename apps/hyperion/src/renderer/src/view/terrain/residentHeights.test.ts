/**
 * The heights a terrain pass keeps of its resident bakes (plan R10, R10.T16.c): the survey edge's
 * rim from the bakes that hold the edge, the drawn ground beneath a line of sight, and what is
 * kept and forgotten.
 */

import { describe, expect, it } from "vitest";

import { type Vec3, vec3 } from "../../geometry/vec3";
import { faceUvToDir, stToUv } from "./cube";
import { type PatchKey, patchKeyString } from "./patchKey";
import { ResidentHeights } from "./residentHeights";
import { type BakedPatch, PALETTE_NONE, type TerrainLayers } from "./workers/messages";

/** A body's layers over `surveyed`'s vertices, on a field of `fieldLevel`. */
function layersOf(surveyed: (x: number, y: number) => boolean, fieldLevel: number): TerrainLayers {
  const mask = new Uint8Array(65 * 65);
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      mask[65 * y + x] = surveyed(x, y) ? 255 : 0;
    }
  }
  return {
    palette: new Uint8Array(16).fill(PALETTE_NONE),
    weights: new Uint8Array(),
    surveyMask: mask,
    fieldLevel,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/** A bake of `key` whose own height at vertex (x, y) is `height(x, y)`, its morph target 0. */
function bakeOf(
  key: PatchKey,
  height: (x: number, y: number) => number,
  surveyed: (x: number, y: number) => boolean,
  fieldLevel = 8,
): BakedPatch {
  const heights = new Float32Array(65 * 65 * 2);
  let top = Number.NEGATIVE_INFINITY;
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      const h = height(x, y);
      heights[2 * (65 * y + x)] = h;
      top = Math.max(top, h);
    }
  }
  return {
    key,
    generation: 1,
    originM: vec3(0, 0, 0),
    heights,
    offsets: null,
    normals: new Float16Array(65 * 65 * 2),
    heightRangeM: [0, top],
    boundingRadiusM: 1,
    originHeightM: 0,
    skirtDepthM: 1,
    layers: layersOf(surveyed, fieldLevel),
  };
}

/** The direction of the point (x, y), in quads, of `key`'s patch. */
function at(key: PatchKey, x: number, y: number): Vec3 {
  const n = 2 ** key.level;
  return faceUvToDir(key.face, stToUv((key.i + x / 64) / n), stToUv((key.j + y / 64) / n));
}

const KEY: PatchKey = { face: 1, level: 5, i: 10, j: 12 };
const WEST_HALF = (x: number): boolean => x < 32;
const EVERY = (): boolean => true;
const NONE = (): boolean => false;

describe("the survey edge's rim from the resident bakes (R10.T16.c)", () => {
  it("stands at a bake's height where its mask holds the edge, bilinear between its vertices", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(KEY, (x, y) => 100 + 2 * x + y, WEST_HALF));
    expect(heights.rimHeightAtM(at(KEY, 10.5, 20.25))).toBeCloseTo(100 + 21 + 20.25, 3);
  });

  it("stands on the datum (none) where no bake holding the edge is resident", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(KEY, () => 300, EVERY));
    heights.take(bakeOf({ ...KEY, i: 11 }, () => 300, NONE));
    expect([
      heights.rimHeightAtM(at(KEY, 20, 20)),
      heights.rimHeightAtM(at({ ...KEY, i: 11 }, 20, 20)),
      heights.rimHeightAtM(at({ ...KEY, i: 20 }, 20, 20)),
    ]).toEqual([null, null, null]);
  });

  it("takes the finest bake holding the edge over a point", () => {
    const heights = new ResidentHeights();
    const parent: PatchKey = { face: 1, level: 4, i: 5, j: 6 };
    heights.take(bakeOf(parent, () => 50, WEST_HALF));
    heights.take(bakeOf(KEY, () => 100, WEST_HALF));
    expect([
      heights.rimHeightAtM(at(KEY, 32, 32)),
      heights.rimHeightAtM(at({ ...KEY, i: 11, j: 13 }, 32, 32)),
    ]).toEqual([100, 50]);
  });

  it("reads the bake across a side two patches share, on one face and across a face's edge", () => {
    const heights = new ResidentHeights();
    // Only the patch east of KEY is held: KEY's own east side reads it.
    heights.take(bakeOf({ ...KEY, i: 11 }, () => 70, WEST_HALF));
    // Face +y's u = −1 edge is face +x's u = +1 edge (`faceUvToXyz`): a patch there on face +y.
    const across: PatchKey = { face: 1, level: 5, i: 0, j: 12 };
    heights.take(bakeOf(across, () => 90, WEST_HALF));
    const onFaceX: PatchKey = { face: 0, level: 5, i: 31, j: 12 };
    expect([
      heights.rimHeightAtM(at(KEY, 64, 30)),
      heights.rimHeightAtM(at(onFaceX, 64, 30)),
    ]).toEqual([70, 90]);
  });
});

describe("the drawn ground under a line of sight (R10.T16.c)", () => {
  it("is a wholly surveyed bake's height on its 17 × 17 grid, and its rim none", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(KEY, (x) => 10 * x, EVERY));
    expect(heights.groundHeightAtM(at(KEY, 6, 30))).toBeCloseTo(60, 3);
    expect(heights.rimHeightAtM(at(KEY, 6, 30))).toBeNull();
  });

  it("is an edge bake's height on its surveyed cells, and the datum on the others", () => {
    const heights = new ResidentHeights();
    // A field of level 8 under a level-5 patch: coarse cells of 8 quads, read at their centres.
    heights.take(bakeOf(KEY, () => 400, WEST_HALF, 8));
    expect([
      heights.groundHeightAtM(at(KEY, 10, 30)),
      heights.groundHeightAtM(at(KEY, 50, 30)),
    ]).toEqual([400, 0]);
  });

  it("is the datum over a bake surveying nothing, and none where no bake is resident", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(KEY, () => 400, NONE));
    expect([
      heights.groundHeightAtM(at(KEY, 10, 30)),
      heights.groundHeightAtM(at({ ...KEY, j: 20 }, 10, 30)),
    ]).toEqual([0, null]);
  });
});

describe("what the resident heights keep (R10.T16.c)", () => {
  it("keeps an edge bake whole, a surveyed one on a coarse grid and a bare one not at all", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(KEY, () => 1, WEST_HALF));
    heights.take(bakeOf({ ...KEY, i: 11 }, () => 1, EVERY));
    heights.take(bakeOf({ ...KEY, i: 12 }, () => 1, NONE));
    // 65 × 65 f32 and the 4,228 B packed mask; 17 × 17 f32; nothing.
    expect([heights.bytes, heights.edgeBakes]).toEqual([65 * 65 * 4 + 4_228 + 17 * 17 * 4, 1]);
  });

  it("forgets a bake as it leaves, every bake at a clear, and stands its ceiling at the highest held", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(KEY, () => 250, WEST_HALF));
    heights.take(bakeOf({ ...KEY, i: 11 }, () => 900, EVERY));
    heights.take(bakeOf({ ...KEY, i: 12 }, () => 5_000, NONE));
    const ceiling = heights.ceilingM;
    heights.forget(patchKeyString({ ...KEY, i: 11 }));
    const afterForget = [heights.ceilingM, heights.groundHeightAtM(at({ ...KEY, i: 11 }, 30, 30))];
    heights.clear();
    expect([ceiling, ...afterForget, heights.rimHeightAtM(at(KEY, 10, 10)), heights.bytes]).toEqual(
      [900, 250, null, null, 0],
    );
  });
});
