/**
 * The survey lookup on a generated world (plan R10, R10.T16.b; decision-r10-t16-contracts §3), on
 * the real module: the Moon-like fixture's two payloads (`hyperion_surface`'s
 * `tests/golden/worker/moon_like.golden`, a survey of face +y at the field's level 6 and the
 * delta that extends it) taken by a worker's fields through its own answers, the lookup fed by the
 * blocks' answers, and patches about the survey's edge baked over them at levels from three to two
 * finer than the field. No patch the lookup calls bare bakes a surveyed vertex.
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
import type { PatchKey } from "./patchKey";
import { SurveyedCells } from "./surveyLookup";
import { answerRequest, type HeightModule, WorkerFields } from "./workers/heightBake";
import type { BakeSettings, HeightWorkerRequest } from "./workers/messages";

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

/** The fixture's body, seed and both payloads' blocks. */
const WORLD = moonLikeFixture("all");

/** The field's level: the first survey's four cells are (22, 22) to (23, 23) of face +y. */
const FIELD_LEVEL = 6;

/**
 * Rows of patches across the survey and its edges on face +y: at each level from 3 to 8, along j
 * at the survey's middle, from two patches before the survey's cells to two after.
 */
function patchesAbout(): PatchKey[] {
  const keys: PatchKey[] = [];
  for (let level = 3; level <= FIELD_LEVEL + 2; level += 1) {
    const scale = 2 ** (level - FIELD_LEVEL);
    const from = Math.floor(22 * scale) - 2;
    const to = Math.ceil(24 * scale) + 1;
    const j = Math.floor(23 * scale);
    for (let i = Math.max(0, from); i <= to; i += 1) {
      keys.push({ face: 1, level, i, j });
    }
    // A column too, across the survey's other edges.
    for (let row = Math.max(0, from); row <= to; row += 1) {
      keys.push({ face: 1, level, i: Math.floor(23 * scale), j: row });
    }
  }
  return keys;
}

describe("the survey lookup on a generated world (R10.T16.b)", () => {
  const SETTINGS: BakeSettings = {
    vertexPath: "face-differences",
    normals: "mesh",
    source: { kind: "body", body: WORLD.body, detailSeed: WORLD.seed, weights: "skipped" },
  };
  let fields: WorkerFields;
  let survey: SurveyedCells;

  beforeAll(() => {
    initSync({ module: wasmBytes() });
    fields = new WorkerFields(module);
    survey = new SurveyedCells();
    WORLD.blocks.forEach((block, n) => {
      const request: HeightWorkerRequest =
        n === 0
          ? {
              kind: "field-init",
              id: n,
              body: WORLD.body,
              block,
              form: "payload",
              sigma: "skipped",
            }
          : { kind: "field-chunk", id: n, body: WORLD.body, block };
      const { reply } = answerRequest(module, request, fields);
      if (reply.kind !== "field-held" || reply.block === null) {
        throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
      }
      survey.add(reply.block);
    });
  });

  it("calls bare only patches whose every vertex bakes unsurveyed", () => {
    const keys = patchesAbout();
    const bare = keys.filter((key) => !survey.surveyedUnder(key));
    const falseBare = bare.filter((key, id) => {
      const { reply } = answerRequest(
        module,
        { kind: "bake", id, key, generation: 1, settings: SETTINGS },
        fields,
      );
      if (reply.kind !== "baked") {
        throw new Error(`${JSON.stringify(key)} was not baked: ${JSON.stringify(reply)}`);
      }
      return reply.bake.layers?.surveyMask.some((byte) => byte !== 0) ?? true;
    });
    expect(falseBare).toEqual([]);
    // Both kinds are about the edge, at every level the field's cells are finer than a patch.
    expect(survey.level).toBe(FIELD_LEVEL);
    expect(bare.length).toBeGreaterThan(10);
    expect(keys.length - bare.length).toBeGreaterThan(10);
  });
});
