import { describe, expect, it } from "vitest";

import { keptBody } from "../../scenes/kept";
import {
  answerRequest,
  type HeightModule,
  StandInFields,
  type WasmFieldBlock,
  type WasmSurfaceFields,
  WorkerFields,
} from "./heightBake";

const MOON = keptBody(1);
const PLANET = keptBody(2);

/** A block answer of the fake module: no cell surveyed anew, a table of zeros. */
function emptyBlock(): WasmFieldBlock {
  return {
    newlySurveyed: () => new Uint32Array(),
    newlyHeld: () => new Uint32Array(),
    levelTable: () => new Float64Array(100),
    fieldLevel: 6,
    free: () => {},
  };
}

/** The fake module's key of a body. */
function bodyKey(system: bigint, body: number): string {
  return `${system.toString(16)}.${String(body)}`;
}

/**
 * A module whose fields take every block of 8 bytes or more, refuse a shorter one as the real
 * module refuses a block it cannot decode, and trap on a block whose first byte is 255; it bakes
 * nothing, the bake tests being `heightWasm.test.ts`'s and `fieldsWasm.test.ts`'s.
 */
function fakeModule(): { module: HeightModule; held: Map<string, number> } {
  const held = new Map<string, number>();
  const key = bodyKey;
  const take = (
    system: bigint,
    body: number,
    block: Uint8Array,
    start: boolean,
  ): WasmFieldBlock => {
    if (block[0] === 255) {
      throw new WebAssembly.RuntimeError("unreachable");
    }
    if (block.length < 8) {
      throw new Error("the block does not decode");
    }
    const at = key(system, body);
    if (!start && !held.has(at)) {
      throw new Error("not started");
    }
    held.set(at, (start ? 0 : (held.get(at) ?? 0)) + block.length);
    return emptyBlock();
  };
  const fields: WasmSurfaceFields = {
    fieldInit: (system, body, block) => take(system, body, block, true),
    fieldChunk: (system, body, block) => take(system, body, block, false),
    fieldRelease: (system, body) => held.delete(key(system, body)),
    get heldBytes() {
      return [...held.values()].reduce((a, b) => a + b, 0);
    },
    bake: () => undefined,
    reading: () => undefined,
    slantRange: () => {
      throw new Error("a field test drew a line of sight");
    },
    material: () => undefined,
    photometry: () => undefined,
    free: () => {},
  };
  return {
    module: {
      bakePatch: () => {
        throw new Error("a field message baked");
      },
      testPlanetVersion: () => 2,
      createFields: () => fields,
    },
    held,
  };
}

describe("a height worker's stand-in fields", () => {
  it("holds each body's blocks from its field-init and drops them at its release", () => {
    const fields = new StandInFields();
    fields.init(MOON, new ArrayBuffer(8));
    expect(fields.chunk(MOON, new ArrayBuffer(4))).toBe(true);
    fields.init(PLANET, new ArrayBuffer(2));
    expect(fields.heldBytes).toBe(14);
    expect(fields.bodies).toEqual([MOON, PLANET]);
    fields.init(MOON, new ArrayBuffer(3));
    expect(fields.heldBytes).toBe(5);
    fields.release(MOON);
    expect(fields.bodies).toEqual([PLANET]);
    expect(fields.chunk(MOON, new ArrayBuffer(1))).toBe(false);
    expect(fields.heldBytes).toBe(2);
  });
});

describe("a height worker's fields", () => {
  it("decodes a payload's blocks in the module and answers with what it holds", () => {
    const { module, held } = fakeModule();
    const fields = new WorkerFields(module);
    const init = answerRequest(
      module,
      { kind: "field-init", id: 1, body: MOON, block: new ArrayBuffer(8), form: "payload" },
      fields,
    );
    expect(init.reply).toMatchObject({ kind: "field-held", id: 1, body: MOON, heldBytes: 8 });
    const chunk = answerRequest(
      module,
      { kind: "field-chunk", id: 2, body: MOON, block: new ArrayBuffer(16) },
      fields,
    );
    expect(chunk.reply).toMatchObject({ kind: "field-held", id: 2, heldBytes: 24 });
    if (chunk.reply.kind !== "field-held" || chunk.reply.block === null) {
      throw new Error("a payload's block has its change");
    }
    expect(chunk.reply.block.fieldLevel).toBe(6);
    expect(chunk.transfer).toEqual([
      chunk.reply.block.newlySurveyed.buffer,
      chunk.reply.block.newlyHeld.buffer,
      chunk.reply.block.levels.buffer,
    ]);
    expect(held.size).toBe(1);
    expect(fields.bodies).toEqual([MOON]);
  });

  it("holds a stand-in's blocks undecoded, with no change to answer", () => {
    const { module, held } = fakeModule();
    const fields = new WorkerFields(module);
    answerRequest(
      module,
      { kind: "field-init", id: 1, body: MOON, block: new ArrayBuffer(3), form: "stand-in" },
      fields,
    );
    const chunk = answerRequest(
      module,
      { kind: "field-chunk", id: 2, body: MOON, block: new ArrayBuffer(2) },
      fields,
    );
    expect(chunk.reply).toEqual({
      kind: "field-held",
      id: 2,
      body: MOON,
      heldBytes: 5,
      block: null,
    });
    expect(held.size).toBe(0);
    // A payload's start replaces the stand-in.
    answerRequest(
      module,
      { kind: "field-init", id: 3, body: MOON, block: new ArrayBuffer(8), form: "payload" },
      fields,
    );
    expect(fields.heldBytes).toBe(8);
  });

  it("refuses a block of a body with no field-init, holding nothing more", () => {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    const answer = answerRequest(
      module,
      { kind: "field-chunk", id: 4, body: MOON, block: new ArrayBuffer(8) },
      fields,
    );
    expect(answer.reply).toMatchObject({
      kind: "field-refused",
      id: 4,
      body: MOON,
      heldBytes: 0,
    });
    expect(fields.bodies).toEqual([]);
  });

  it("answers a block the module refuses as refused, the field as it was", () => {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    fields.init(MOON, new ArrayBuffer(8), "payload");
    const answer = answerRequest(
      module,
      { kind: "field-chunk", id: 5, body: MOON, block: new ArrayBuffer(2) },
      fields,
    );
    expect(answer.reply).toMatchObject({
      kind: "field-refused",
      body: MOON,
      message: "the block does not decode",
      heldBytes: 8,
    });
    expect(fields.init(PLANET, new ArrayBuffer(1), "payload")).toEqual({
      kind: "refused",
      message: "the block does not decode",
    });
    expect(fields.bodies).toEqual([MOON]);
  });

  it("rethrows a trap of the module, so that the worker is replaced", () => {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    expect(() =>
      answerRequest(
        module,
        {
          kind: "field-init",
          id: 6,
          body: MOON,
          block: new Uint8Array(8).fill(255).buffer,
          form: "payload",
        },
        fields,
      ),
    ).toThrow(WebAssembly.RuntimeError);
  });

  it("leaves no field held after a release", () => {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    fields.init(MOON, new ArrayBuffer(8), "payload");
    fields.init(PLANET, new ArrayBuffer(4), "stand-in");
    answerRequest(module, { kind: "field-release", id: 7, body: MOON }, fields);
    const answer = answerRequest(module, { kind: "field-release", id: 8, body: PLANET }, fields);
    expect(answer.reply).toEqual({
      kind: "field-held",
      id: 8,
      body: PLANET,
      heldBytes: 0,
      block: null,
    });
    expect(fields.bodies).toEqual([]);
  });

  it("answers a body's bake the field cannot make yet as bake-unsurveyed", () => {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    const answer = answerRequest(
      module,
      {
        kind: "bake",
        id: 9,
        key: { face: 2, level: 8, i: 1, j: 2 },
        generation: 1,
        settings: {
          vertexPath: "face-differences",
          normals: "mesh",
          source: {
            kind: "body",
            body: MOON,
            detailSeed: "6173736d626c7900",
            weights: "baked",
          },
        },
      },
      fields,
    );
    expect(answer.reply).toMatchObject({ kind: "bake-unsurveyed", id: 9 });
    expect(answer.transfer).toEqual([]);
  });
});
