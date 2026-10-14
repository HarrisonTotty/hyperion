/**
 * The height worker's fields on the real module (plan R10, T6.b): a fixture field's payload blocks,
 * from `hyperion_surface`'s `tests/golden/worker/moon_like.golden`, posted through the worker's own
 * answers, and every entry point's answer over them held to what the native build pinned there
 * (`src/worker/golden.rs`), bit for bit.
 */

import { beforeAll, describe, expect, it } from "vitest";

import fixture from "../../../../../../../../crates/hyperion-surface/tests/golden/worker/moon_like.golden?raw";
import {
  bakePatch,
  initSync,
  LineOfSightKind,
  SurfaceFields,
  testPlanetVersion,
  WeightsLayer,
} from "../../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../../generated/surface/hyperion_surface_bg.wasm?inline";
import type { Face } from "../patchKey";
import { f32Digest, f64Digest } from "./f32Digest";
import {
  answerRequest,
  type FacePoint,
  type HeightModule,
  packNormals,
  WASM_LINE_OF_SIGHT,
  WASM_NORMAL_SCALE,
  WASM_VERTEX_PATH,
  WASM_WEIGHTS_LAYER,
  WorkerFields,
} from "./heightBake";
import {
  type BakedPatch,
  type BakeSettings,
  type HeightSourceSetting,
  type HeightWorkerRequest,
  PALETTE_NONE,
  type WeightsLayerSetting,
} from "./messages";
import { needsRebake } from "./rebake";

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

/** The IEEE 754 bits of `value` as 16 hexadecimal digits, as the testkit prints them. */
function bitsHex(value: number): string {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, value);
  return view.getBigUint64(0).toString(16).padStart(16, "0");
}

/** The `f64` of 16 hexadecimal digits of bits. */
function fromBits(hex: string | undefined): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(`0x${hex ?? "nan"}`));
  return view.getFloat64(0);
}

/** The bytes of a hexadecimal string, in a buffer of their own. */
function bytesOf(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);
  for (let n = 0; n < bytes.length; n += 1) {
    bytes[n] = Number.parseInt(hex.slice(2 * n, 2 * n + 2), 16);
  }
  return bytes.buffer;
}

/** The golden's digest of bytes: each as an `f64`, as `byte_digest` takes them. */
function byteDigest(bytes: Uint8Array): bigint {
  return f64Digest(Float64Array.from(bytes));
}

/** A digest as the golden prints it, 16 hexadecimal digits. */
function digestHex(digest: bigint): string {
  return digest.toString(16).padStart(16, "0");
}

function isFace(n: number): n is Face {
  return Number.isInteger(n) && n >= 0 && n <= 5;
}

/** A face index of the golden, checked. */
function faceOf(word: string | undefined): Face {
  const n = Number(word);
  if (!isFace(n)) {
    throw new Error(`the fixture names no face, ${String(word)}`);
  }
  return n;
}

/** One record of the fixture: its head line's words and the lines under it, by label. */
interface FixtureRecord {
  readonly head: ReadonlyArray<string>;
  /** The `label = 0x…` lines' bits, by label. */
  readonly bits: ReadonlyMap<string, string>;
  /** The other lines' words, by their first. */
  readonly lines: ReadonlyMap<string, ReadonlyArray<string>>;
}

/** The fixture's lines, parsed. */
interface Fixture {
  readonly body: string;
  readonly seed: string;
  readonly blocks: ReadonlyArray<ArrayBuffer>;
  /** Each block's payload, 1 or 2. */
  readonly payloads: ReadonlyArray<number>;
  /** Each block's change, as its runs' words, `-` for none. */
  readonly changes: ReadonlyArray<ReadonlyArray<string>>;
  /** The level table's rows after the first block and after the last, as bits. */
  readonly levels: Readonly<Record<"first" | "last", ReadonlyArray<string>>>;
  readonly records: ReadonlyArray<FixtureRecord>;
}

/** The record heads: each starts a record of the lines under it. */
const HEADS = new Set(["patch", "reading", "slant", "material"]);

function parseFixture(): Fixture {
  let body = "";
  let seed = "";
  const blocks: ArrayBuffer[] = [];
  const payloads: number[] = [];
  const changes: string[][] = [];
  const levels: Record<"first" | "last", string[]> = { first: [], last: [] };
  const records: { head: string[]; bits: Map<string, string>; lines: Map<string, string[]> }[] = [];
  for (const line of fixture.split("\n").slice(1)) {
    const words = line.split(" ");
    const [head, ...rest] = words;
    if (head === undefined || head === "") {
      continue;
    }
    const current = records.at(-1);
    if (head === "body") {
      body = rest[0] ?? "";
    } else if (head === "seed") {
      seed = rest[0] ?? "";
    } else if (head === "block") {
      blocks.push(bytesOf(rest[2] ?? ""));
      payloads.push(Number(rest[0]));
    } else if (head === "change") {
      changes.push(rest.slice(1));
    } else if (head === "level") {
      const when = rest[0];
      if (when === "first" || when === "last") {
        levels[when].push(...rest.slice(2));
      }
    } else if (HEADS.has(head)) {
      records.push({ head: words, bits: new Map(), lines: new Map() });
    } else if (current !== undefined && words[1] === "=") {
      current.bits.set(head, (words[2] ?? "").slice(2));
    } else if (current !== undefined) {
      current.lines.set(head, rest);
    }
  }
  return { body, seed, blocks, payloads, changes, levels, records };
}

const FIXTURE = parseFixture();

/** The fixture's records of `head`. */
function recordsOf(head: string): FixtureRecord[] {
  return FIXTURE.records.filter((r) => r.head[0] === head);
}

/** A record's bits of `label`. */
function bitsOf(record: FixtureRecord, label: string): string {
  const bits = record.bits.get(label);
  if (bits === undefined) {
    throw new Error(`the fixture's ${record.head.join(" ")} has no ${label}`);
  }
  return bits;
}

/** A worker's fields holding every block of the fixture, through the worker's answers. */
function heldFields(): WorkerFields {
  const fields = new WorkerFields(module);
  FIXTURE.blocks.forEach((block, n) => {
    const request: HeightWorkerRequest =
      n === 0
        ? { kind: "field-init", id: n, body: FIXTURE.body, block, form: "payload" }
        : { kind: "field-chunk", id: n, body: FIXTURE.body, block };
    const { reply } = answerRequest(module, request, fields);
    if (reply.kind !== "field-held") {
      throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
    }
  });
  return fields;
}

/** The runs of a block's answer, in the fixture's words. */
function runWords(runs: Uint32Array): string[] {
  const words: string[] = [];
  for (let n = 0; n + 1 < runs.length; n += 2) {
    words.push(`${String(runs[n])}-${String(runs[n + 1])}`);
  }
  return words.length === 0 ? ["-"] : words;
}

/** The palette's bytes in the fixture's words. */
function paletteWords(palette: Uint8Array): string[] {
  const words: string[] = [];
  for (let n = 0; n + 1 < palette.length; n += 2) {
    const [code, entry] = [palette[n], palette[n + 1]];
    words.push(
      code === PALETTE_NONE
        ? "none"
        : `${String(code)}:${entry === PALETTE_NONE ? "-" : String(entry)}`,
    );
  }
  return words;
}

const VERTEX_PATHS = {
  "baked-offsets": "baked-offsets",
  "face-differences": "face-differences",
} as const;
const NORMALS = { mesh: "mesh", double: "double" } as const;
const WEIGHTS = { baked: "baked", skipped: "skipped" } as const;

function isKeyOf<T extends object>(table: T, name: string): name is Extract<keyof T, string> {
  return Object.hasOwn(table, name);
}

/** A word of the fixture as one of `table`'s values. */
function lookup<T extends Record<string, string>>(table: T, word: string | undefined): T[keyof T] {
  if (word === undefined || !isKeyOf(table, word)) {
    throw new Error(`the fixture names an unknown setting ${String(word)}`);
  }
  return table[word];
}

describe("the height worker's fields on the real module", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it("maps the bake's weights and the line of sight to the module's enum values", () => {
    expect(WASM_WEIGHTS_LAYER.baked).toBe(WeightsLayer.Baked);
    expect(WASM_WEIGHTS_LAYER.skipped).toBe(WeightsLayer.Skipped);
    expect(WASM_LINE_OF_SIGHT.ground).toBe(LineOfSightKind.Ground);
    expect(WASM_LINE_OF_SIGHT["no-ground"]).toBe(LineOfSightKind.NoGround);
    expect(WASM_LINE_OF_SIGHT["not-surveyed"]).toBe(LineOfSightKind.NotSurveyed);
  });

  it("takes every block of the fixture, answering each with the native change and table", () => {
    expect(FIXTURE.blocks.length).toBeGreaterThanOrEqual(4);
    const fields = new WorkerFields(module);
    const answers = FIXTURE.blocks.map((block, n) => {
      const { reply, transfer } = answerRequest(
        module,
        n === 0
          ? { kind: "field-init", id: n, body: FIXTURE.body, block, form: "payload" }
          : { kind: "field-chunk", id: n, body: FIXTURE.body, block },
        fields,
      );
      if (reply.kind !== "field-held" || reply.block === null) {
        throw new Error(`block ${String(n)} was not taken: ${JSON.stringify(reply)}`);
      }
      return { body: reply.body, block: reply.block, transfer };
    });
    expect(answers.map((a) => a.body)).toEqual(FIXTURE.blocks.map(() => FIXTURE.body));
    expect(answers.map((a) => runWords(a.block.newlySurveyed))).toEqual(FIXTURE.changes);
    expect(answers.map((a) => a.transfer)).toEqual(
      answers.map((a) => [a.block.newlySurveyed.buffer, a.block.levels.buffer]),
    );
    expect(answers.map((a) => a.block.fieldLevel)).toEqual(FIXTURE.blocks.map(() => 6));
    const tables = answers.map((a) => [...a.block.levels].map(bitsHex));
    expect(tables[0]).toEqual(FIXTURE.levels.first);
    expect(tables.at(-1)).toEqual(FIXTURE.levels.last);
    expect(fields.bodies).toEqual([FIXTURE.body]);
  });

  it("holds no field after an init, two blocks and a release", () => {
    const fields = new WorkerFields(module);
    const [first, second, third] = FIXTURE.blocks;
    if (first === undefined || second === undefined || third === undefined) {
      throw new Error("the fixture has three blocks");
    }
    expect(fields.init(FIXTURE.body, first, "payload").kind).toBe("taken");
    expect(fields.chunk(FIXTURE.body, second).kind).toBe("taken");
    expect(fields.chunk(FIXTURE.body, third).kind).toBe("taken");
    // The field keeps a slot for every cell of its level whatever it holds (R09's PartialField).
    expect(fields.heldBytes).toBeGreaterThan(6 * 4 ** 6 * 20);
    const { reply } = answerRequest(
      module,
      { kind: "field-release", id: 9, body: FIXTURE.body },
      fields,
    );
    expect(reply).toEqual({
      kind: "field-held",
      id: 9,
      body: FIXTURE.body,
      heldBytes: 0,
      block: null,
    });
    expect(fields.bodies).toEqual([]);
  });

  it("refuses a block posted before its field_init, and a field started from a later block", () => {
    const fields = new WorkerFields(module);
    const [first, second] = FIXTURE.blocks;
    if (first === undefined || second === undefined) {
      throw new Error("the fixture has two blocks");
    }
    const { reply } = answerRequest(
      module,
      { kind: "field-chunk", id: 1, body: FIXTURE.body, block: second },
      fields,
    );
    expect(reply).toMatchObject({ kind: "field-refused", id: 1, body: FIXTURE.body, heldBytes: 0 });
    const late = fields.init(FIXTURE.body, second, "payload");
    expect(late).toMatchObject({ kind: "refused", message: expect.stringMatching(/block 0/u) });
    // A block posted for another body than its own.
    const other = `${FIXTURE.body.slice(0, -1)}${FIXTURE.body.endsWith("1") ? "2" : "1"}`;
    expect(fields.init(other, first, "payload").kind).toBe("refused");
    expect(fields.bodies).toEqual([]);
    expect(fields.heldBytes).toBe(0);
  });

  it("bakes the fixture's patches as the native bake does, layers and all", () => {
    const fields = heldFields();
    const direct = new SurfaceFields();
    const [system, index] = FIXTURE.body.split(".");
    const args = [BigInt(`0x${system ?? ""}`), Number.parseInt(index ?? "", 16)] as const;
    FIXTURE.blocks.forEach((block, n) => {
      const bytes = new Uint8Array(block);
      const taken = n === 0 ? direct.fieldInit(...args, bytes) : direct.fieldChunk(...args, bytes);
      taken.free();
    });
    const patches = recordsOf("patch");
    expect(patches).toHaveLength(6);
    for (const record of patches) {
      const [, name, face, level, i, j, path, normals, weights] = record.head;
      const key = { face: faceOf(face), level: Number(level), i: Number(i), j: Number(j) };
      const settings: BakeSettings = {
        vertexPath: lookup(VERTEX_PATHS, path),
        normals: lookup(NORMALS, normals),
        source: {
          kind: "body",
          body: FIXTURE.body,
          detailSeed: FIXTURE.seed,
          weights: lookup(WEIGHTS, weights) satisfies WeightsLayerSetting,
        },
      };
      const { reply, transfer } = answerRequest(
        module,
        { kind: "bake", id: 3, key, generation: 5, settings },
        fields,
      );
      if (reply.kind !== "baked" || reply.bake.layers === null) {
        throw new Error(`${String(name)} was not baked: ${JSON.stringify(reply)}`);
      }
      const { bake } = reply;
      const layers = bake.layers;
      if (layers === null) {
        throw new Error(`${String(name)} has no layers`);
      }
      const raw = direct.bake(
        ...args,
        BigInt(`0x${FIXTURE.seed}`),
        key.face,
        key.level,
        key.i,
        key.j,
        WASM_VERTEX_PATH[settings.vertexPath],
        WASM_NORMAL_SCALE[settings.normals],
        WASM_WEIGHTS_LAYER[lookup(WEIGHTS, weights)],
        0,
      );
      if (raw === undefined) {
        throw new Error(`${String(name)} was not baked directly`);
      }
      const rawNormals = raw.normals();
      raw.free();
      const digests = {
        heights: digestHex(f32Digest(bake.heights)),
        offsets: digestHex(f32Digest(bake.offsets ?? new Float32Array())),
        normals: digestHex(f32Digest(rawNormals)),
        weights: digestHex(byteDigest(layers.weights)),
        mask: digestHex(byteDigest(layers.surveyMask)),
      };
      expect({ name, ...digests }).toEqual({
        name,
        ...Object.fromEntries(Object.keys(digests).map((label) => [label, bitsOf(record, label)])),
      });
      expect(bake.normals).toEqual(packNormals(rawNormals));
      const scalars = {
        origin_x: bake.originM.x,
        origin_y: bake.originM.y,
        origin_z: bake.originM.z,
        origin_height: bake.originHeightM,
        lowest: bake.heightRangeM[0],
        highest: bake.heightRangeM[1],
        bounding_radius: bake.boundingRadiusM,
        skirt_depth: bake.skirtDepthM,
        bound: layers.boundM,
        sigma: layers.sigmaM,
      };
      expect(
        Object.fromEntries(Object.entries(scalars).map(([label, v]) => [label, bitsHex(v)])),
      ).toEqual(
        Object.fromEntries(Object.keys(scalars).map((label) => [label, bitsOf(record, label)])),
      );
      expect(paletteWords(layers.palette)).toEqual(record.lines.get("palette"));
      expect(String(layers.complete)).toBe(record.lines.get("complete")?.[0]);
      const unsurveyed = [...layers.unsurveyedCells].map(String);
      expect(unsurveyed.length === 0 ? ["-"] : unsurveyed).toEqual(record.lines.get("unsurveyed"));
      expect(transfer).toContain(layers.weights.buffer);
      expect(transfer).toContain(layers.unsurveyedCells.buffer);
    }
    direct.free();
  });

  it("reads the fixture's points as the native readings do", () => {
    const fields = heldFields();
    const readings = recordsOf("reading");
    expect(readings).toHaveLength(8);
    const answered = readings.map((record) => {
      const [, face, u, v, baseline] = record.head;
      const at: FacePoint = { face: faceOf(face), u: fromBits(u), v: fromBits(v) };
      const reading = fields.reading(FIXTURE.body, FIXTURE.seed, at, fromBits(baseline));
      return reading === null
        ? null
        : {
            elevation: bitsHex(reading.elevationM),
            elevation_sigma: bitsHex(reading.elevationSigmaM),
            slope_rms: bitsHex(reading.slopeRmsRad),
            slope_sigma: bitsHex(reading.slopeSigmaRad),
            slope_baseline: bitsHex(reading.slopeBaselineM),
            resolution: bitsHex(reading.resolutionM),
            resolved: String(reading.resolved),
          };
    });
    const pinned = readings.map((record) =>
      record.lines.has("none")
        ? null
        : {
            elevation: bitsOf(record, "elevation"),
            elevation_sigma: bitsOf(record, "elevation_sigma"),
            slope_rms: bitsOf(record, "slope_rms"),
            slope_sigma: bitsOf(record, "slope_sigma"),
            slope_baseline: bitsOf(record, "slope_baseline"),
            resolution: bitsOf(record, "resolution"),
            resolved: record.lines.get("resolved")?.[0],
          },
    );
    expect(answered).toEqual(pinned);
    expect(pinned.filter((r) => r === null)).toHaveLength(4);
  });

  it("ranges the fixture's lines of sight as the native march does", () => {
    const fields = heldFields();
    const sights = recordsOf("slant");
    expect(sights.map((r) => [...r.lines.keys()][0])).toEqual([
      "ground",
      "not_surveyed",
      "no_ground",
    ]);
    const answered = sights.map((record) => {
      const [ox, oy, oz, dx, dy, dz, reach] = record.head.slice(1).map(fromBits);
      if (
        ox === undefined ||
        oy === undefined ||
        oz === undefined ||
        dx === undefined ||
        dy === undefined ||
        dz === undefined ||
        reach === undefined
      ) {
        throw new Error(`a malformed line of sight: ${record.head.join(" ")}`);
      }
      const sight = fields.slantRange(
        FIXTURE.body,
        FIXTURE.seed,
        [ox, oy, oz],
        [dx, dy, dz],
        reach,
      );
      return sight.kind === "ground"
        ? {
            kind: sight.kind,
            range: bitsHex(sight.rangeM),
            sigma: sight.sigmaM === null ? null : bitsHex(sight.sigmaM),
            resolved: String(sight.resolved),
            hit: sight.hitM.map(bitsHex),
            at: [String(sight.at.face), bitsHex(sight.at.u), bitsHex(sight.at.v)],
          }
        : { kind: sight.kind };
    });
    const pinned = sights.map((record) => {
      if (record.lines.has("no_ground")) {
        return { kind: "no-ground" };
      }
      if (record.lines.has("not_surveyed")) {
        return { kind: "not-surveyed" };
      }
      return {
        kind: "ground",
        range: bitsOf(record, "range"),
        sigma: record.lines.has("sigma") ? null : bitsOf(record, "sigma"),
        resolved: record.lines.get("resolved")?.[0],
        hit: ["hit_x", "hit_y", "hit_z"].map((label) => bitsOf(record, label)),
        at: record.lines.get("at"),
      };
    });
    expect(answered).toEqual(pinned);
  });

  it("names the fixture's materials as the native classifier does", () => {
    const fields = heldFields();
    const materials = recordsOf("material");
    expect(materials).toHaveLength(4);
    const answered = materials.map((record) => {
      const [, face, u, v] = record.head;
      const at: FacePoint = { face: faceOf(face), u: fromBits(u), v: fromBits(v) };
      const material = fields.material(FIXTURE.body, FIXTURE.seed, at);
      if (material === null) {
        return null;
      }
      const p = material.properties;
      return {
        class: [
          String(material.classCode),
          "entry",
          material.entry === null ? "-" : String(material.entry),
        ],
        properties:
          p === null
            ? null
            : {
                albedo: p.normalAlbedoBvr.map(bitsHex),
                phase_law: String(p.phaseLaw),
                limb: bitsHex(p.limbParameter),
                grain_density: bitsHex(p.grainDensityKgM3),
                transition: bitsHex(p.transitionK),
                mechanics: String(p.mechanics),
              },
      };
    });
    const pinned = materials.map((record) =>
      record.lines.has("none")
        ? null
        : {
            class: record.lines.get("class"),
            properties: record.bits.has("albedo_v")
              ? {
                  albedo: ["albedo_b", "albedo_v", "albedo_r"].map((l) => bitsOf(record, l)),
                  phase_law: record.lines.get("phase_law")?.[0],
                  limb: bitsOf(record, "limb"),
                  grain_density: bitsOf(record, "grain_density"),
                  transition: bitsOf(record, "transition"),
                  mechanics: record.lines.get("mechanics")?.[0],
                }
              : null,
          },
    );
    expect(answered).toEqual(pinned);
    expect(pinned.filter((m) => m !== null)).toHaveLength(2);
  });

  it("bakes the same again every patch the rebake rule passes over at a block", () => {
    const fields = new WorkerFields(module);
    const firstPayload = FIXTURE.payloads.filter((p) => p === 1).length;
    FIXTURE.blocks.slice(0, firstPayload).forEach((block, n) => {
      const taken =
        n === 0 ? fields.init(FIXTURE.body, block, "payload") : fields.chunk(FIXTURE.body, block);
      expect(taken.kind).toBe("taken");
    });
    const bakeAll = (): BakedPatch[] =>
      recordsOf("patch").map((record) => {
        const [, , face, level, i, j, path, normals, weights] = record.head;
        const source = {
          kind: "body",
          body: FIXTURE.body,
          detailSeed: FIXTURE.seed,
          weights: lookup(WEIGHTS, weights),
        } as const satisfies HeightSourceSetting;
        const bake = fields.bake(
          { face: faceOf(face), level: Number(level), i: Number(i), j: Number(j) },
          1,
          { vertexPath: lookup(VERTEX_PATHS, path), normals: lookup(NORMALS, normals), source },
          source,
        );
        if (bake === null) {
          throw new Error(`${record.head.join(" ")} was not baked`);
        }
        return bake;
      });
    const before = bakeAll();
    const flagged = before.map(() => false);
    for (const block of FIXTURE.blocks.slice(firstPayload)) {
      const taken = fields.chunk(FIXTURE.body, block);
      if (taken.kind !== "taken" || taken.block === null) {
        throw new Error("the second payload's block was not taken");
      }
      const { block: answer } = taken;
      before.forEach((bake, n) => {
        flagged[n] = (flagged[n] ?? false) || needsRebake(bake.layers, answer);
      });
    }
    const after = bakeAll();
    const passedOver = (bakes: readonly BakedPatch[]): unknown[] =>
      bakes.flatMap((bake, n) =>
        flagged[n] === true ? [] : [[bake.heights, bake.normals, bake.layers]],
      );
    expect(passedOver(after)).toEqual(passedOver(before));
    // The second survey changes the masks of patches beside it, and the incomplete ones.
    expect(flagged.filter(Boolean).length).toBeGreaterThan(0);
    expect(flagged.filter((f) => !f).length).toBeGreaterThan(0);
  });

  it("answers no reading, material or bake of a body whose field is not held", () => {
    const fields = new WorkerFields(module);
    const at: FacePoint = { face: 1, u: 0, v: 0 };
    expect(fields.reading(FIXTURE.body, FIXTURE.seed, at, 2)).toBeNull();
    expect(fields.material(FIXTURE.body, FIXTURE.seed, at)).toBeNull();
    expect(fields.slantRange(FIXTURE.body, FIXTURE.seed, [0, 2e6, 0], [0, -1, 0], 1e7)).toEqual({
      kind: "not-surveyed",
    });
    const bake = answerRequest(
      module,
      {
        kind: "bake",
        id: 1,
        key: { face: 1, level: 7, i: 47, j: 45 },
        generation: 1,
        settings: {
          vertexPath: "face-differences",
          normals: "mesh",
          source: { kind: "body", body: FIXTURE.body, detailSeed: FIXTURE.seed, weights: "baked" },
        },
      },
      fields,
    );
    expect(bake.reply.kind).toBe("bake-unsurveyed");
  });

  it("refuses every argument out of its range, rather than trapping", () => {
    const fields = heldFields();
    const read = (at: FacePoint, baselineM: number) => () =>
      fields.reading(FIXTURE.body, FIXTURE.seed, at, baselineM);
    const sight =
      (
        origin: readonly [number, number, number],
        direction: readonly [number, number, number],
        reach: number,
      ) =>
      () =>
        fields.slantRange(FIXTURE.body, FIXTURE.seed, origin, direction, reach);
    const at: FacePoint = { face: 1, u: 0, v: 0 };
    expect(read({ face: 1, u: 2, v: 0 }, 2)).toThrow(/not a point of a face/u);
    expect(read({ face: 1, u: Number.NaN, v: 0 }, 2)).toThrow(/not a point of a face/u);
    expect(read(at, 0)).toThrow(/baseline/u);
    expect(read(at, 1e-4)).toThrow(/baseline/u);
    expect(read(at, 1e8)).toThrow(/baseline/u);
    expect(sight([0, 0, 0.5], [0, 0, -1], 1e7)).toThrow(/origin/u);
    expect(sight([1e13, 0, 0], [-1, 0, 0], 1e7)).toThrow(/origin/u);
    expect(sight([0, 2e6, 0], [1e-200, 0, 0], 1e7)).toThrow(/direction/u);
    expect(sight([0, 2e6, 0], [0, -1e200, 0], 1e7)).toThrow(/direction/u);
    expect(sight([0, 2e6, 0], [0, -1, 0], 1e13)).toThrow(/reach/u);
    expect(sight([0, 2e6, 0], [0, -1, 0], -1)).toThrow(/reach/u);
    const direct = new SurfaceFields();
    const [system] = FIXTURE.body.split(".");
    const id = BigInt(`0x${system ?? ""}`);
    expect(() => direct.fieldRelease(id, 70_000)).toThrow(/body index/u);
    expect(() => direct.material(id, 3, 0n, 6, 0, 0)).toThrow(/face/u);
    expect(() => direct.material(id, 3, 0n, 263, 0, 0)).toThrow(/face/u);
    expect(() => direct.bake(id, 3, 0n, 1, 300, 0, 0, 0, 0, 0, 0)).toThrow(/level/u);
    expect(() => direct.bake(id, 3, 0n, 1, 5, 99, 0, 0, 0, 0, 0)).toThrow(/outside level 5/u);
    expect(() => direct.bake(id, 3, 0n, 1, 5, 0, 0, 0, 0, 0, -1)).toThrow(/skirt/u);
    direct.free();
  });
});
