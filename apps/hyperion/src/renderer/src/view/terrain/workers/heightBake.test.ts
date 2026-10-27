import { describe, expect, it } from "vitest";

import { keptBody } from "../../scenes/kept";
import {
  answerRequest,
  type HeightModule,
  StandInFields,
  type WasmBakedTerrain,
  type WasmClassMap,
  type WasmFieldBlock,
  type WasmGroundReadout,
  type WasmNormalMap,
  type WasmSurfaceFields,
  WorkerFields,
} from "./heightBake";
import type { ClassMapAnswer, GroundReadoutRequest } from "./messages";

const MOON = keptBody(1);
const PLANET = keptBody(2);

/** A block answer of the fake module: no cell surveyed anew, a table and σ of zeros. */
function emptyBlock(): WasmFieldBlock {
  return {
    newlySurveyed: () => new Uint32Array(),
    newlyHeld: () => new Uint32Array(),
    levelTable: () => new Float64Array(100),
    levelSigmaM: () => new Float64Array(25),
    fieldLevel: 6,
    free: () => {},
  };
}

/**
 * The fake module's class map: one texel of one class, wholly surveyed, with `codes` its maturity
 * (none by default) and then K × 3 nodes rising from 0.01; `free` names it in `freed`.
 */
function oneTexelMap(freed: string[], codes: Uint16Array = new Uint16Array()): WasmClassMap {
  return {
    level: 6,
    fieldLevel: 6,
    classes: () => Uint8Array.from([1, 0]),
    photometry: () => Float32Array.from([0.1, 0.1, 0.1, 1, 1]),
    texels: () => Uint32Array.from([7]),
    fractions: () => Float32Array.from([1]),
    weights: () => Uint8Array.from([255]),
    maturity: () => codes.slice(),
    nodes: () => Float32Array.from({ length: 16 * 3 }, (_, k) => 0.01 * (k + 1)),
    complete: true,
    free: () => {
      freed.push("class map");
    },
  };
}

/**
 * The fake module's normal map: the same cell, its pair (0.25, −0.5) with a tail a half float
 * rounds off, every surveyed cell answered; `free` names it in `freed`.
 */
function oneCellNormals(freed: string[]): WasmNormalMap {
  return {
    level: 6,
    cells: () => Uint32Array.from([7]),
    pairs: () => Float32Array.from([0.25 + 2 ** -20, -0.5]),
    complete: true,
    free: () => {
      freed.push("normal map");
    },
  };
}

/**
 * The fake module's readout: on the ground, its sight's, reading's and material's getters, a
 * capacity and a friction that depends on the temperature, at `temperatureK`; or, off the ground
 * (`kind` 1 or 2), every getter `undefined` or `false`, as the module answers.
 */
function readoutOf(kind: number, temperatureK: number | undefined): WasmGroundReadout {
  const on = kind === 0;
  const value = (x: number): number | undefined => (on ? x : undefined);
  return {
    kind,
    rangeM: value(41_200),
    rangeSigmaM: value(300),
    grazing: false,
    rangeResolved: false,
    hitM: () => (on ? Float64Array.of(1, 2, 3) : undefined),
    atFace: value(2),
    atU: value(0.25),
    atV: value(-0.5),
    elevationM: value(2_140),
    elevationSigmaM: value(180),
    slopeRmsRad: value(0.2),
    slopeSigmaRad: value(0.07),
    slopeBaselineM: value(2),
    resolutionM: value(410),
    readingResolved: false,
    classCode: value(7),
    entry: value(2),
    bearingCapacityPa: undefined,
    capacitySlopeRad: value(0.2),
    padFriction: value(0.08),
    frictionOnTemperature: on,
    temperatureK: on ? temperatureK : undefined,
    free: () => {},
  };
}

/** The fake module's key of a body. */
function bodyKey(system: bigint, body: number): string {
  return `${system.toString(16)}.${String(body)}`;
}

/** What {@link fakeModule}'s bake gives (R10.T1.c), and what its fields were asked of it. */
interface FakeBake {
  /** The bake's maturity codes: two a vertex, or none. */
  readonly codes: Uint16Array;
  /** The levels and palettes `maturityNodes` was asked at, in order. */
  readonly nodesAsked: Array<{ readonly level: number; readonly palette: Uint8Array }>;
}

/** A body's bake of the fake module: zeros of a patch's sizes, a palette of one slot, `codes`. */
function fakeBaked(codes: Uint16Array): WasmBakedTerrain {
  return {
    heights: () => new Float32Array(65 * 65 * 2),
    offsets: () => undefined,
    normals: () => new Float32Array(67 * 67 * 2),
    origin: () => Float64Array.of(1, 2, 3),
    heightRangeM: () => Float32Array.of(-10, 10),
    boundingRadiusM: 100,
    originHeightM: 0,
    skirtDepthM: 1,
    palette: () =>
      Uint8Array.of(1, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255),
    weights: () => new Uint8Array(65 * 65 * 8),
    surveyMask: () => new Uint8Array(65 * 65),
    unsurveyedCells: () => new Uint32Array(),
    horizonNear: () => new Uint32Array(),
    horizonFar: () => new Uint32Array(),
    horizonRefusedCells: () => new Uint32Array(),
    maturity: () => codes,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    horizonComplete: true,
    free: () => {},
  };
}

/**
 * A module whose fields take every block of 8 bytes or more, refuse a shorter one as the real
 * module refuses a block it cannot decode, and trap on a block whose first byte is 255; it bakes
 * nothing unless given `bake`, whose bake a held body's fields answer with its photometry and
 * nodes, the bake tests being otherwise `heightWasm.test.ts`'s and `fieldsWasm.test.ts`'s.
 */
function fakeModule(
  bake?: FakeBake,
  classMapCodes: Uint16Array = new Uint16Array(),
): {
  module: HeightModule;
  held: Map<string, number>;
  /** The module's class and normal maps freed, in order. */
  freed: string[];
} {
  const held = new Map<string, number>();
  const freed: string[] = [];
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
    bake: (system, body) =>
      bake !== undefined && held.has(key(system, body)) ? fakeBaked(bake.codes) : undefined,
    reading: () => undefined,
    slantRange: () => {
      throw new Error("a field test drew a line of sight");
    },
    material: () => undefined,
    photometry: (system, body) =>
      bake !== undefined && held.has(key(system, body)) ? new Float32Array(40) : undefined,
    maturityNodes: (system, body, palette, level) => {
      bake?.nodesAsked.push({ level, palette });
      return held.has(key(system, body)) ? new Float32Array(8 * 2 * 16 * 3) : undefined;
    },
    // A held body's map of one texel; seed 0xdead the module's refusal, 0xffff a trap.
    classMap: (system, body, seed) => {
      if (seed === 0xdeadn) {
        throw new Error("a body index above 65,535");
      }
      if (seed === 0xffffn) {
        throw new WebAssembly.RuntimeError("unreachable");
      }
      return held.has(key(system, body)) ? oneTexelMap(freed, classMapCodes) : undefined;
    },
    // A held body's normal map of the class map's cell; seed 0xbeef, a broken module's, none.
    normalMap: (system, body, seed) =>
      held.has(key(system, body)) && seed !== 0xbeefn ? oneCellNormals(freed) : undefined,
    // A held body's readout on the ground at the season's anomaly as its temperature, kelvin, and
    // `not-surveyed` with none held; seed 0xdead the module's refusal, 0xffff a trap.
    groundReadout: (system, body, seed, ...rest) => {
      if (seed === 0xdeadn) {
        throw new Error("a slope baseline of 3 m is none of the readout's 2 m, 5 m and 100 m");
      }
      if (seed === 0xffffn) {
        throw new WebAssembly.RuntimeError("unreachable");
      }
      return held.has(key(system, body)) ? readoutOf(0, rest[9]) : readoutOf(2, undefined);
    },
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
    freed,
  };
}

/**
 * A class-map query's answer from {@link fakeModule} holding a field, its map's maturity `codes`
 * (none by default), and what it freed.
 */
function classMapAnswered(codes: Uint16Array = new Uint16Array()): {
  readonly map: ClassMapAnswer;
  readonly transfer: ReadonlyArray<Transferable>;
  readonly freed: ReadonlyArray<string>;
} {
  const { module, freed } = fakeModule(undefined, codes);
  const fields = new WorkerFields(module);
  answerRequest(
    module,
    { kind: "field-init", id: 1, body: MOON, block: new ArrayBuffer(8), form: "payload" },
    fields,
  );
  const answer = answerRequest(
    module,
    { kind: "class-map", id: 2, body: MOON, detailSeed: "00000000000000a5" },
    fields,
  );
  if (answer.reply.kind !== "class-map" || answer.reply.map === null) {
    throw new Error(`no map: ${JSON.stringify(answer.reply)}`);
  }
  return { map: answer.reply.map, transfer: answer.transfer, freed };
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
      chunk.reply.block.sigmaM.buffer,
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

  describe("a class-map query (R10.T10.d, R10.T10.g)", () => {
    it("answers with the module's class map and normal map, each at its level and complete", () => {
      const { map } = classMapAnswered();
      expect(map).toMatchObject({
        level: 6,
        fieldLevel: 6,
        complete: true,
        normalLevel: 6,
        normalsComplete: true,
      });
    });

    it("answers the class map's texels and weights as the module's", () => {
      const { map } = classMapAnswered();
      expect([...map.texels, ...map.weights]).toEqual([7, 255]);
    });

    it("answers the normal map's cells and pairs, the pairs as half floats, rounded as a bake's", () => {
      const { map } = classMapAnswered();
      expect([
        map.normalPairs instanceof Float16Array,
        ...map.normalCells,
        ...map.normalPairs,
      ]).toEqual([true, 7, 0.25, -0.5]);
    });

    it("transfers every array of both maps", () => {
      const { map, transfer } = classMapAnswered();
      expect(transfer).toEqual([
        map.classes.buffer,
        map.photometry.buffer,
        map.texels.buffer,
        map.fractions.buffer,
        map.weights.buffer,
        map.normalCells.buffer,
        map.normalPairs.buffer,
      ]);
    });

    it("frees both of the module's answers", () => {
      expect(classMapAnswered().freed).toEqual(["class map", "normal map"]);
    });

    it("answers no maturity where the module's map carries none (R10.T1.c)", () => {
      const { map } = classMapAnswered();
      expect(Object.hasOwn(map, "maturity")).toBe(false);
    });

    it("answers each texel's maturity code and each class's nodes where the module's map carries them, both transferred (R10.T1.c)", () => {
      const { map, transfer } = classMapAnswered(Uint16Array.from([40_000]));
      const { maturity } = map;
      if (maturity === undefined) {
        throw new Error("the map was answered without its maturity");
      }
      expect([
        [...maturity.codes],
        maturity.nodes.length,
        maturity.nodes[0],
        maturity.nodes[47],
      ]).toEqual([[40_000], 48, Math.fround(0.01), Math.fround(0.01 * 48)]);
      expect(transfer.slice(-2)).toEqual([maturity.codes.buffer, maturity.nodes.buffer]);
    });
  });

  it("answers a module's class map without its normal map as class-map-failed, the class map freed", () => {
    const { module, freed } = fakeModule();
    const fields = new WorkerFields(module);
    answerRequest(
      module,
      { kind: "field-init", id: 1, body: MOON, block: new ArrayBuffer(8), form: "payload" },
      fields,
    );
    const answer = answerRequest(
      module,
      { kind: "class-map", id: 2, body: MOON, detailSeed: "000000000000beef" },
      fields,
    );
    expect(answer.reply).toEqual({
      kind: "class-map-failed",
      id: 2,
      body: MOON,
      message: `the module answered ${MOON}'s class map but no normal map`,
    });
    expect(freed).toEqual(["class map"]);
  });

  /** A class-map query of `body` by `detailSeed`, the worker holding `held`'s fields. */
  function askClassMap(
    held: ReadonlyArray<readonly [string, "stand-in" | "payload"]>,
    body: string,
    detailSeed: string,
  ): ReturnType<typeof answerRequest> {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    held.forEach(([each, form], n) => {
      answerRequest(
        module,
        { kind: "field-init", id: n + 1, body: each, block: new ArrayBuffer(8), form },
        fields,
      );
    });
    return answerRequest(module, { kind: "class-map", id: 3, body, detailSeed }, fields);
  }

  it("answers no class map of a body with no field", () => {
    expect(askClassMap([], MOON, "00000000000000a5").reply).toEqual({
      kind: "class-map",
      id: 3,
      body: MOON,
      map: null,
    });
  });

  it("answers no class map of a body whose field is a stand-in's", () => {
    expect(askClassMap([[PLANET, "stand-in"]], PLANET, "00000000000000a5").reply).toMatchObject({
      kind: "class-map",
      map: null,
    });
  });

  it("answers the module's refusal of a class map as class-map-failed", () => {
    expect(askClassMap([[MOON, "payload"]], MOON, "000000000000dead").reply).toEqual({
      kind: "class-map-failed",
      id: 3,
      body: MOON,
      message: "a body index above 65,535",
    });
  });

  it("rethrows a trap in the module's class map", () => {
    expect(() => askClassMap([[MOON, "payload"]], MOON, "000000000000ffff")).toThrow(
      WebAssembly.RuntimeError,
    );
  });

  /** A ground-readout query of `body` by `detailSeed`, the worker holding `held`'s fields. */
  function askReadout(
    held: ReadonlyArray<readonly [string, "stand-in" | "payload"]>,
    body: string,
    detailSeed: string,
    seasonMeanAnomalyRad: number | undefined,
  ): ReturnType<typeof answerRequest> {
    const { module } = fakeModule();
    const fields = new WorkerFields(module);
    held.forEach(([each, form], n) => {
      answerRequest(
        module,
        { kind: "field-init", id: n + 1, body: each, block: new ArrayBuffer(8), form },
        fields,
      );
    });
    const request: GroundReadoutRequest = {
      kind: "ground-readout",
      id: 4,
      body,
      detailSeed,
      originM: { x: 0, y: 2e6, z: 0 },
      direction: { x: 0, y: -1, z: 0 },
      maxM: 1e7,
      slopeBaselineM: 2,
      padWidthM: 1,
      seasonMeanAnomalyRad,
    };
    return answerRequest(module, request, fields);
  }

  it("answers a ground-readout query with the module's readout, copied out (R10.T12)", () => {
    const answer = askReadout([[MOON, "payload"]], MOON, "00000000000000a5", 243);
    expect(answer.reply).toEqual({
      kind: "ground-readout",
      id: 4,
      body: MOON,
      readout: {
        kind: "ground",
        sight: {
          rangeM: 41_200,
          sigmaM: 300,
          resolved: false,
          hitM: [1, 2, 3],
          at: { face: 2, u: 0.25, v: -0.5 },
        },
        reading: {
          elevationM: 2_140,
          elevationSigmaM: 180,
          slopeRmsRad: 0.2,
          slopeSigmaRad: 0.07,
          slopeBaselineM: 2,
          resolutionM: 410,
          resolved: false,
        },
        material: { classCode: 7, entry: 2 },
        properties: {
          bearingCapacityPa: null,
          capacitySlopeRad: 0.2,
          padFriction: 0.08,
          frictionOnTemperature: true,
          temperatureK: 243,
        },
      },
    });
    expect(answer.transfer).toEqual([]);
  });

  it("hands the module no season where none is known, and reads no temperature back", () => {
    const answer = askReadout([[MOON, "payload"]], MOON, "00000000000000a5", undefined);
    expect(answer.reply).toMatchObject({
      kind: "ground-readout",
      readout: { properties: { temperatureK: null, frictionOnTemperature: true } },
    });
  });

  it("answers not-surveyed where the module holds no field of the body", () => {
    expect(askReadout([[PLANET, "stand-in"]], PLANET, "00000000000000a5", 1).reply).toEqual({
      kind: "ground-readout",
      id: 4,
      body: PLANET,
      readout: { kind: "not-surveyed" },
    });
  });

  it("answers the module's refusal of a readout as ground-readout-failed", () => {
    expect(askReadout([[MOON, "payload"]], MOON, "000000000000dead", 1).reply).toEqual({
      kind: "ground-readout-failed",
      id: 4,
      body: MOON,
      message: "a slope baseline of 3 m is none of the readout's 2 m, 5 m and 100 m",
    });
  });

  it("rethrows a trap in the module's readout", () => {
    expect(() => askReadout([[MOON, "payload"]], MOON, "000000000000ffff", 1)).toThrow(
      WebAssembly.RuntimeError,
    );
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

  describe("a bake's maturity layer (R10.T1.c's client half)", () => {
    /** A held body's bake at level 13 by {@link fakeModule} baking `codes`, and what it asked. */
    function bakedWith(codes: Uint16Array): {
      readonly answer: ReturnType<typeof answerRequest>;
      readonly asked: FakeBake["nodesAsked"];
    } {
      const bake: FakeBake = { codes, nodesAsked: [] };
      const { module } = fakeModule(bake);
      const fields = new WorkerFields(module);
      answerRequest(
        module,
        { kind: "field-init", id: 1, body: MOON, block: new ArrayBuffer(8), form: "payload" },
        fields,
      );
      const answer = answerRequest(
        module,
        {
          kind: "bake",
          id: 2,
          key: { face: 1, level: 13, i: 8_161, j: 6_926 },
          generation: 4,
          settings: {
            vertexPath: "face-differences",
            normals: "mesh",
            source: { kind: "body", body: MOON, detailSeed: "617373656d626c79", weights: "baked" },
          },
        },
        fields,
      );
      return { answer, asked: bake.nodesAsked };
    }

    it("posts the bake's codes with its palette's nodes at the patch's level, both transferred", () => {
      const codes = Uint16Array.from({ length: 2 * 65 * 65 }, (_, n) => n % 65_536);
      const { answer, asked } = bakedWith(codes);
      if (answer.reply.kind !== "baked") {
        throw new Error(`not baked: ${JSON.stringify(answer.reply)}`);
      }
      const maturity = answer.reply.bake.layers?.maturity;
      expect(maturity?.codes).toBe(codes);
      expect(maturity?.nodes).toHaveLength(8 * 2 * 16 * 3);
      expect(asked.map(({ level, palette }) => [level, palette])).toEqual([
        [13, answer.reply.bake.layers?.palette],
      ]);
      expect(answer.transfer).toEqual(
        expect.arrayContaining([maturity?.codes.buffer, maturity?.nodes.buffer]),
      );
    });

    it("posts no maturity, and asks for no nodes, where the bake made no layer", () => {
      const { answer, asked } = bakedWith(new Uint16Array());
      if (answer.reply.kind !== "baked") {
        throw new Error(`not baked: ${JSON.stringify(answer.reply)}`);
      }
      expect(Object.hasOwn(answer.reply.bake.layers ?? {}, "maturity")).toBe(false);
      expect(asked).toEqual([]);
    });
  });
});
