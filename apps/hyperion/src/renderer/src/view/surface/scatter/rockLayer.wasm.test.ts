/**
 * A finest patch's rock layer on the real module (plan R11, T4.a): the worker's bake of each patch,
 * through the worker's own answers, against the same module's `rocksNear` on the main thread,
 * over the fixture fields of `hyperion_surface`'s `tests/golden/worker/`.
 */

import { beforeAll, describe, expect, it } from "vitest";

import earthFixture from "../../../../../../../../crates/hyperion-surface/tests/golden/worker/earth_like.golden?raw";
import moonFixture from "../../../../../../../../crates/hyperion-surface/tests/golden/worker/moon_like.golden?raw";
import {
  bakePatch,
  initSync,
  rockWords,
  SurfaceFields,
  testPlanetVersion,
} from "../../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../../generated/surface/hyperion_surface_bg.wasm?inline";
import {
  directFieldsOf,
  finestAbout,
  parseWorkerFixture,
  type WorkerFixture,
  workerFieldsOf,
} from "../../../test/workerFieldFixtures";
import { finestLevel, uvToSt, vertexDir, xyzToFaceUv } from "../../terrain/cube";
import type { PatchKey } from "../../terrain/patchKey";
import { type BodyFigure, spheroidPoint } from "../../terrain/planet";
import { answerRequest, type HeightModule } from "../../terrain/workers/heightBake";
import type { BakeSettings } from "../../terrain/workers/messages";
import { ROCK_WORDS, rockAt, rockLayerOf } from "./rockLayer";

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

/** The settings of the wireframe's bake: no weights, horizon or normals. */
function settingsOf(fixture: WorkerFixture): BakeSettings {
  return {
    vertexPath: "face-differences",
    normals: "mesh",
    source: {
      kind: "body",
      body: fixture.body,
      detailSeed: fixture.seed,
      weights: "skipped",
      horizon: "skipped",
      normals: "skipped",
    },
  };
}

/** The patch of `level` holding the direction `dir`, as the surface crate's `PatchKey::containing`. */
function patchContaining(level: number, dir: readonly [number, number, number]): PatchKey {
  const { face, u, v } = xyzToFaceUv(dir);
  const side = 2 ** level;
  const cell = (st: number): number => Math.min(side - 1, Math.max(0, Math.floor(st * side)));
  return { face, level, i: cell(uvToSt(u)), j: cell(uvToSt(v)) };
}

/** The IEEE 754 bits of each of `words`. */
function bitsOf(words: Float64Array): BigUint64Array {
  return new BigUint64Array(Float64Array.from(words).buffer);
}

beforeAll(() => {
  initSync({ module: wasmBytes() });
});

describe("a finest patch's rocks on the real module (R11.T4.a)", () => {
  it("hands each rock over in the crate's words", () => {
    expect(rockWords()).toBe(ROCK_WORDS);
  });

  it.each([
    ["the Moon-like world", moonFixture],
    ["the Earth-like world", earthFixture],
  ])(
    "on %s, bakes each finest patch's list in the worker as rocksNear about it finds them on the main thread",
    { timeout: 60_000 },
    (_, text) => {
      const fixture = parseWorkerFixture(text);
      const fields = workerFieldsOf(module, fixture);
      const { direct, args, seed } = directFieldsOf(() => new SurfaceFields(), fixture);
      const [a, c] = direct.figureM(...args) ?? [];
      if (a === undefined || c === undefined) {
        throw new Error("the fixture's field holds a figure");
      }
      const figure: BodyFigure = { equatorialRadiusM: a, polarRadiusM: c, pole: null };
      const finest = finestLevel(a);
      let compared = 0;
      let baked = 0;
      for (let step = 0; step < 24; step += 1) {
        const key = finestAbout(fixture, finest, step);
        const { reply } = answerRequest(
          module,
          { kind: "bake", id: 100 + step, key, generation: 1, settings: settingsOf(fixture) },
          fields,
        );
        if (reply.kind !== "baked") {
          throw new Error(`patch ${JSON.stringify(key)} was not baked: ${reply.kind}`);
        }
        const rocks = reply.bake.layers?.rocks;
        if (rocks === undefined) {
          throw new Error("a finest patch's bake carries its rocks");
        }
        baked += 1;
        const centre = spheroidPoint(figure, vertexDir(key, 32, 32));
        const radiusM = Math.max(
          ...[
            [0, 0],
            [64, 0],
            [0, 64],
            [64, 64],
          ].map(([x = 0, y = 0]) => {
            const corner = spheroidPoint(figure, vertexDir(key, x, y));
            return Math.hypot(corner[0] - centre[0], corner[1] - centre[1], corner[2] - centre[2]);
          }),
        );
        const near = direct.rocksNear(...args, seed, ...centre, 1.01 * radiusM);
        if (near === undefined) {
          throw new Error("the surveyed ground's rocks are known");
        }
        const nearLayer = rockLayerOf(near);
        const held: number[] = [];
        for (let n = 0; n < nearLayer.count; n += 1) {
          const { dir } = rockAt(nearLayer, n);
          const holder = patchContaining(finest, [dir.x, dir.y, dir.z]);
          if (holder.face === key.face && holder.i === key.i && holder.j === key.j) {
            held.push(...near.subarray(ROCK_WORDS * n, ROCK_WORDS * (n + 1)));
          }
        }
        expect(bitsOf(rocks.words)).toEqual(bitsOf(Float64Array.from(held)));
        compared += rocks.count;
      }
      direct.free();
      fields.free();
      expect(baked).toBe(24);
      expect(compared).toBeGreaterThan(0);
    },
  );
});
