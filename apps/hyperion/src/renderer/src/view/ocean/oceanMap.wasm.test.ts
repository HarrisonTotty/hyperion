/**
 * The ocean's data on the real module (plan R11, R11.T8.b): the month blend's twin against the
 * module's `monthBlend` export, to the bit, and the worker's ocean map over the Moon-like worker
 * fixture (`hyperion_surface`'s `tests/golden/worker/moon_like.golden`), taken through the worker's
 * own answers. The crate's own tests hold the map over an Earth-like coast on all three targets.
 */

import { beforeAll, describe, expect, it } from "vitest";

import {
  bakePatch,
  initSync,
  monthBlend as moduleMonthBlend,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { moonLikeFixture, type WorkerFixture } from "../../test/moonLikeFixture";
import { fieldCellAt, fieldCellCount } from "../terrain/coverage";
import { answerRequest, type HeightModule, WorkerFields } from "../terrain/workers/heightBake";
import type { OceanMapAnswer } from "../terrain/workers/messages";
import { monthBlend } from "./monthBlend";
import { oceanMaskOf } from "./oceanMask";

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

/** A fixture's ocean map and normal-map cells, its blocks posted through the worker's answers. */
function oceanOf(fixture: WorkerFixture): {
  readonly map: OceanMapAnswer;
  readonly surveyed: ReadonlyArray<number>;
} {
  const fields = new WorkerFields(module);
  fixture.blocks.forEach((block, n) => {
    answerRequest(
      module,
      n === 0
        ? {
            kind: "field-init",
            id: n,
            body: fixture.body,
            block,
            form: "payload",
            sigma: "skipped",
          }
        : { kind: "field-chunk", id: n, body: fixture.body, block },
      fields,
    );
  });
  const answer = answerRequest(module, { kind: "ocean-map", id: 99, body: fixture.body }, fields);
  if (answer.reply.kind !== "ocean-map" || answer.reply.sea === null) {
    throw new Error(`no ocean map: ${JSON.stringify(answer.reply)}`);
  }
  const { map, mask } = answer.reply.sea;
  // The worker's mask, decided beside its answer (perf-long-tasks-5), lays each cell out where its
  // cell's key put it as the page did before, `(face · n + j) · n + i`.
  const n = 2 ** map.level;
  const places = new Int32Array(fieldCellCount(map.level)).fill(-1);
  map.cells.forEach((index, k) => {
    const cell = fieldCellAt(map.level, index);
    places[(cell.face * n + cell.j) * n + cell.i] = k;
  });
  expect(mask.places).toEqual(places);
  expect(mask).toEqual(oceanMaskOf(map));
  const normals = answerRequest(
    module,
    { kind: "class-map", id: 100, body: fixture.body, detailSeed: "617373656d626c79" },
    fields,
  );
  if (normals.reply.kind !== "class-map" || normals.reply.map === null) {
    throw new Error("no class map");
  }
  return { map, surveyed: [...normals.reply.map.normalCells] };
}

describe("the ocean's data on the real module (R11.T8.b)", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it("blends the months as the module's export does, to the bit, and refuses what it refuses", () => {
    for (const e of [0, 0.0167, 0.6, 0.95]) {
      for (let m = -20; m < 20; m += 0.0137) {
        const twin = monthBlend(12, e, m);
        const blend = moduleMonthBlend(12, e, m);
        expect([twin?.from, twin?.to, twin?.weight], `e ${e}, m ${m}`).toEqual([
          blend.from,
          blend.to,
          blend.weight,
        ]);
        expect(Object.is(twin?.weight, blend.weight), `e ${e}, m ${m}`).toBe(true);
        blend.free();
      }
    }
    for (const [months, e, m] of [
      [6, 0, 1],
      [12, 1, 1],
      [12, 0.1, Number.NaN],
    ] as const) {
      expect(monthBlend(months, e, m)).toBeNull();
      expect(() => moduleMonthBlend(months, e, m)).toThrow(/no blend of a year/u);
    }
  });

  it("answers each surveyed cell of the Moon-like field, all of it dry, with twelve winds a cell", () => {
    const { map, surveyed } = oceanOf(moonLikeFixture("all"));
    expect(map.complete).toBe(true);
    expect([...map.cells]).toEqual(surveyed);
    expect(map.depthM.every((d) => d === 0)).toBe(true);
    expect(map.winds.length).toBe(24 * map.cells.length);
    expect([map.months, map.windSpeedsMps.length]).toEqual([map.months === 1 ? 1 : 12, 256]);
    // R09's wind speed codes: 0.01 m/s × 2^((c − 1) ÷ 16), calm at 0.
    expect(map.windSpeedsMps[0]).toBe(0);
    for (const c of [1, 17, 100, 255]) {
      expect(map.windSpeedsMps[c]).toBeCloseTo(0.01 * 2 ** ((c - 1) / 16), 12);
    }
    expect(oceanMaskOf(map).openCells).toBe(0);
  });
});
