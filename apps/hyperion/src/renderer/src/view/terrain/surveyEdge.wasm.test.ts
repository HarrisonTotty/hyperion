/**
 * The survey edge on a generated world (plan R10, R10.T10.c; Design note 12), on the real module:
 * the Moon-like fixture's first survey (`hyperion_surface`'s `tests/golden/worker/moon_like.golden`,
 * four cells of level 6 on face +y, with their margins) taken by a worker's fields through its own
 * answers, and patches across the survey's edge baked over it at levels from six coarser than the
 * field to one finer. The terrain's discard (`terrainLit.wgsl`'s `survey_surveyed`, its twin
 * `surveyedAt`) is the coverage's cells exactly, as the reference surface's is (`smoothMesh.wgsl`),
 * and the skirts the edge joins the two surfaces with stand within the terrain's own relief there,
 * on a flattened world whose heights are measured along its spheroid's normal (the task's check by
 * hand, run here).
 */

import { beforeAll, describe, expect, it } from "vitest";

import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { moonLikeFixture } from "../../test/moonLikeFixture";
import { fieldCellIndex } from "./coverage";
import { packSurveyMask, SURVEY_FIN_LINES, surveyCellCode, surveyedAt } from "./gpu/resources";
import type { PatchKey } from "./patchKey";
import { answerRequest, type HeightModule, WorkerFields } from "./workers/heightBake";
import type { BakedPatch, BakeSettings, HeightWorkerRequest } from "./workers/messages";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const module: HeightModule = {
  bakePatch,
  testPlanetVersion,
  createFields: () => new SurfaceFields(),
};

/** The fixture's body, seed and first payload's blocks, the first survey's. */
const SURVEY = moonLikeFixture("first");

/** The field's level, 6: the survey's four cells are (22, 22) to (23, 23) of face +y there. */
const FIELD_LEVEL = 6;

/**
 * Patches across the survey's edge: one level finer than the field (two patches a cell), the
 * field's own, and coarser ones whose cells span 32, 16, 8, 4 and 1 quads (codes 5 to 0), each
 * holding a piece of the edge.
 */
const PATCHES: ReadonlyArray<PatchKey> = [
  { face: 1, level: 7, i: 43, j: 45 },
  { face: 1, level: 7, i: 44, j: 45 },
  { face: 1, level: 7, i: 47, j: 46 },
  { face: 1, level: 7, i: 48, j: 46 },
  { face: 1, level: 7, i: 46, j: 47 },
  { face: 1, level: 7, i: 46, j: 48 },
  { face: 1, level: 6, i: 23, j: 23 },
  { face: 1, level: 6, i: 24, j: 23 },
  { face: 1, level: 6, i: 22, j: 21 },
  { face: 1, level: 5, i: 11, j: 11 },
  { face: 1, level: 5, i: 12, j: 11 },
  { face: 1, level: 4, i: 5, j: 5 },
  { face: 1, level: 3, i: 2, j: 2 },
  { face: 1, level: 2, i: 1, j: 1 },
  { face: 1, level: 0, i: 0, j: 0 },
];

/** The fields holding the first survey, and the cells its blocks surveyed. */
function heldSurvey(): { fields: WorkerFields; surveyed: Set<number> } {
  const fields = new WorkerFields(module);
  const surveyed = new Set<number>();
  SURVEY.blocks.forEach((block, n) => {
    const request: HeightWorkerRequest =
      n === 0
        ? { kind: "field-init", id: n, body: SURVEY.body, block, form: "payload" }
        : { kind: "field-chunk", id: n, body: SURVEY.body, block };
    const { reply } = answerRequest(module, request, fields);
    if (reply.kind !== "field-held" || reply.block === null) {
      throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
    }
    const runs = reply.block.newlySurveyed;
    for (let r = 0; r + 1 < runs.length; r += 2) {
      for (let cell = runs[r] ?? 0; cell < (runs[r + 1] ?? 0); cell += 1) {
        surveyed.add(cell);
      }
    }
  });
  return { fields, surveyed };
}

const SETTINGS: BakeSettings = {
  vertexPath: "face-differences",
  normals: "mesh",
  source: { kind: "body", body: SURVEY.body, detailSeed: SURVEY.seed, weights: "baked" },
};

/** Each of {@link PATCHES} baked over the first survey, and the cells it surveyed. */
function bakedSurvey(): { bakes: BakedPatch[]; surveyed: Set<number> } {
  const { fields, surveyed } = heldSurvey();
  const bakes = PATCHES.map((key, id) => {
    const { reply } = answerRequest(
      module,
      { kind: "bake", id, key, generation: 1, settings: SETTINGS },
      fields,
    );
    if (reply.kind !== "baked") {
      throw new Error(`${JSON.stringify(key)} was not baked: ${JSON.stringify(reply)}`);
    }
    return reply.bake;
  });
  return { bakes, surveyed };
}

/** The coarse cell holding the point (x, y), in quads, of `key`'s patch. */
function cellOf(key: PatchKey, x: number, y: number): PatchKey {
  const quadsPerCell = 64 * 2 ** (key.level - FIELD_LEVEL);
  const along = (index: number, f: number): number => Math.floor((index * 64 + f) / quadsPerCell);
  return { face: key.face, level: FIELD_LEVEL, i: along(key.i, x), j: along(key.j, y) };
}

describe("the survey edge on a generated world (R10.T10.c)", () => {
  let baked: ReturnType<typeof bakedSurvey>;

  beforeAll(() => {
    initSync({ module: wasmBytes() });
    baked = bakedSurvey();
  });

  it("bakes each patch with its field's level, whose cells its mask holds", () => {
    expect(baked.surveyed.size).toBe(4);
    for (const i of [22, 23]) {
      for (const j of [22, 23]) {
        expect(baked.surveyed.has(fieldCellIndex({ face: 1, level: FIELD_LEVEL, i, j }))).toBe(
          true,
        );
      }
    }
    expect(baked.bakes.map((bake) => bake.layers?.fieldLevel)).toEqual(
      PATCHES.map(() => FIELD_LEVEL),
    );
  });

  it("discards the terrain exactly outside the coverage's surveyed cells, a coarse cell of two quads or more", () => {
    const wrong: string[] = [];
    let surveyedQuads = 0;
    let unsurveyedQuads = 0;
    for (const bake of baked.bakes) {
      const { key, layers } = bake;
      if (layers === null) {
        throw new Error("a body's bake carries its layers");
      }
      const code = surveyCellCode(key.level, layers.fieldLevel);
      if (code === 0) {
        continue;
      }
      const packed = packSurveyMask(layers.surveyMask, code);
      for (let y = 0; y < 64; y += 1) {
        for (let x = 0; x < 64; x += 1) {
          // Each quad's centre, and a point by each of its corners.
          for (const [fx, fy] of [
            [x + 0.5, y + 0.5],
            [x + 0.01, y + 0.01],
            [x + 0.99, y + 0.99],
          ] as const) {
            const expected = baked.surveyed.has(fieldCellIndex(cellOf(key, fx, fy)));
            if (surveyedAt(packed, fx, fy) !== expected) {
              wrong.push(`${JSON.stringify(key)} (${String(fx)}, ${String(fy)})`);
            }
          }
          if (baked.surveyed.has(fieldCellIndex(cellOf(key, x + 0.5, y + 0.5)))) {
            surveyedQuads += 1;
          } else {
            unsurveyedQuads += 1;
          }
        }
      }
    }
    expect(wrong.slice(0, 5)).toEqual([]);
    expect(wrong).toHaveLength(0);
    // The patches hold both sides of the edge.
    expect(surveyedQuads).toBeGreaterThan(10_000);
    expect(unsurveyedQuads).toBeGreaterThan(10_000);
  });

  it("gives the codes of cells of 64 quads or more down to one, and fins on every cell's edge to 16 quads", () => {
    expect(PATCHES.map((key) => surveyCellCode(key.level, FIELD_LEVEL))).toEqual([
      6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 5, 4, 3, 2, 0,
    ]);
    // A cell of 2^c quads has its edges on the fins' lines inside a patch for c from 1 to 4.
    for (const code of [1, 2, 3, 4]) {
      for (const line of SURVEY_FIN_LINES) {
        expect(line % 2 ** code).toBe(0);
      }
    }
  });

  it("joins the terrain to the datum by skirts within the terrain's own relief at the edge", () => {
    // The skirts span the terrain's height above the datum along every cell's side that parts a
    // surveyed cell from one not surveyed, which the spheroid's normal measures (Design note 12).
    let largestSpanM = 0;
    let largestReliefM = 0;
    let sides = 0;
    const over: string[] = [];
    for (const bake of baked.bakes) {
      const { key, heights } = bake;
      if (key.level < FIELD_LEVEL) {
        continue;
      }
      const own = (x: number, y: number): number => heights[2 * (65 * y + x)] ?? Number.NaN;
      const surveyed = (x: number, y: number): boolean =>
        baked.surveyed.has(fieldCellIndex(cellOf(key, x, y)));
      // The patch is one cell or inside one: its own state, against its neighbours' across each
      // edge, the cells outside it.
      if (!surveyed(32, 32)) {
        continue;
      }
      let low = Number.POSITIVE_INFINITY;
      let high = Number.NEGATIVE_INFINITY;
      for (let y = 0; y <= 64; y += 1) {
        for (let x = 0; x <= 64; x += 1) {
          low = Math.min(low, own(x, y));
          high = Math.max(high, own(x, y));
        }
      }
      const reliefM = high - low;
      // Each edge's k-th vertex, and a point just outside the edge, in the neighbouring cell.
      const edges: ReadonlyArray<{
        readonly vertex: (k: number) => readonly [number, number];
        readonly outside: readonly [number, number];
      }> = [
        { vertex: (k) => [0, k], outside: [-0.5, 32] },
        { vertex: (k) => [64, k], outside: [64.5, 32] },
        { vertex: (k) => [k, 0], outside: [32, -0.5] },
        { vertex: (k) => [k, 64], outside: [32, 64.5] },
      ];
      for (const { vertex, outside } of edges) {
        if (surveyed(...outside)) {
          continue;
        }
        sides += 1;
        for (let k = 0; k <= 64; k += 1) {
          const [x, y] = vertex(k);
          const spanM = Math.abs(own(x, y));
          largestSpanM = Math.max(largestSpanM, spanM);
          if (spanM > reliefM) {
            over.push(
              `${JSON.stringify(key)} (${String(x)}, ${String(y)}): ${spanM.toFixed(0)} m over ${reliefM.toFixed(0)} m`,
            );
          }
        }
      }
      largestReliefM = Math.max(largestReliefM, reliefM);
    }
    expect(sides).toBeGreaterThanOrEqual(4);
    expect(over).toEqual([]);
    expect(largestSpanM).toBeLessThanOrEqual(largestReliefM);
  });
});
