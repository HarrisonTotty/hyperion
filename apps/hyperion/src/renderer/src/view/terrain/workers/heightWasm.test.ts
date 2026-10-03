import { beforeAll, describe, expect, it } from "vitest";

import golden from "../../../../../../../../crates/hyperion-surface/tests/golden/bake.golden?raw";
import geometrySource from "../../../../../../../../crates/hyperion-surface/src/geometry.rs?raw";
import libSource from "../../../../../../../../crates/hyperion-surface/src/lib.rs?raw";
import {
  bakePatch,
  bandLimitM,
  finestSpacingM,
  initSync,
  levelTable,
  NormalScale,
  Ridges,
  testPlanetVersion,
  VertexPath,
} from "../../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../../generated/surface/hyperion_surface_bg.wasm?inline";
import type { Face } from "../patchKey";
import { f32Digest } from "./f32Digest";
import {
  answerRequest,
  bakeKey,
  EXPECTED_TEST_PLANET_VERSION,
  type HeightModule,
  packNormals,
  staleModuleMessage,
  WASM_NORMAL_SCALE,
  WASM_RIDGES,
  WASM_VERTEX_PATH,
} from "./heightBake";
import type { BakeSettings } from "./messages";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/** A constant of the surface crate's source, `pub const NAME: TYPE = VALUE;`, as a number. */
function rustConstant(source: string, name: string): number {
  const match = new RegExp(`pub const ${name}: \\w+ = ([0-9._]+);`).exec(source);
  if (match?.[1] === undefined) {
    throw new Error(`the surface crate no longer defines ${name}`);
  }
  return Number(match[1].replaceAll("_", ""));
}

/** The IEEE 754 bits of `value` as 16 hexadecimal digits, as the testkit prints them. */
function bitsHex(value: number): string {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, value);
  return view.getBigUint64(0).toString(16).padStart(16, "0");
}

/** One bake of `bake.golden`: its key, settings, digests and scalars' bits. */
interface GoldenBake {
  readonly face: Face;
  readonly level: number;
  readonly i: number;
  readonly j: number;
  readonly settings: BakeSettings;
  readonly digests: Readonly<Record<string, readonly [number, bigint] | null>>;
  readonly scalars: Readonly<Record<string, string>>;
}

const VERTEX_PATHS = {
  BakedOffsets: "baked-offsets",
  FaceDifferences: "face-differences",
} as const;
const NORMALS = { Mesh: "mesh", Double: "double" } as const;
const RIDGES = { Off: "off", On: "on" } as const;

function isFace(n: number): n is Face {
  return Number.isInteger(n) && n >= 0 && n <= 5;
}

function isKeyOf<T extends object>(table: T, name: string): name is Extract<keyof T, string> {
  return Object.hasOwn(table, name);
}

function lookup<T extends Record<string, string>>(table: T, name: string | undefined): T[keyof T] {
  if (name === undefined || !isKeyOf(table, name)) {
    throw new Error(`bake.golden names an unknown setting ${String(name)}`);
  }
  return table[name];
}

/** The three bakes of `bake.golden`, in its format (`tests/bake_golden.rs`). */
function goldenBakes(): GoldenBake[] {
  const bakes: GoldenBake[] = [];
  let current: {
    face: Face;
    level: number;
    i: number;
    j: number;
    settings: BakeSettings;
    digests: Record<string, readonly [number, bigint] | null>;
    scalars: Record<string, string>;
  } | null = null;
  for (const line of golden.split("\n").slice(1)) {
    const words = line.split(" ");
    const head = words[0];
    if (head === "bake") {
      const [face, level, i, j] = words.slice(1, 5).map(Number);
      if (
        face === undefined ||
        !isFace(face) ||
        level === undefined ||
        i === undefined ||
        j === undefined
      ) {
        throw new Error(`malformed bake line: ${line}`);
      }
      current = {
        face,
        level,
        i,
        j,
        settings: {
          vertexPath: lookup(VERTEX_PATHS, words[5]),
          normals: lookup(NORMALS, words[6]),
          ridges: lookup(RIDGES, words[7]),
        },
        digests: {},
        scalars: {},
      };
      bakes.push(current);
    } else if (
      current !== null &&
      (head === "heights" || head === "offsets" || head === "normals")
    ) {
      current.digests[head] =
        words[1] === "none" ? null : [Number(words[1]), BigInt(words[2] ?? "")];
    } else if (current !== null && words[1] === "=") {
      current.scalars[head ?? ""] = (words[2] ?? "").slice(2);
    }
  }
  return bakes;
}

const module: HeightModule = { bakePatch, testPlanetVersion };

/** A field holder for the bake tests, which post no field. */
function ignoreField(): void {
  // The bake tests post no field.
}

/** The settings of the answer tests. */
const SETTINGS: BakeSettings = { vertexPath: "baked-offsets", normals: "double", ridges: "off" };

/** An array's length and `f32_digest`, as `bake.golden` prints them. */
function digest(values: Float32Array): readonly [number, bigint] {
  return [values.length, f32Digest(values)];
}

describe("the height worker's module", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it("maps the settings to the module's enum values", () => {
    expect(WASM_VERTEX_PATH["baked-offsets"]).toBe(VertexPath.BakedOffsets);
    expect(WASM_VERTEX_PATH["face-differences"]).toBe(VertexPath.FaceDifferences);
    expect(WASM_NORMAL_SCALE.mesh).toBe(NormalScale.Mesh);
    expect(WASM_NORMAL_SCALE.double).toBe(NormalScale.Double);
    expect(WASM_RIDGES.off).toBe(Ridges.Off);
    expect(WASM_RIDGES.on).toBe(Ridges.On);
  });

  it("exports the crate's band limit and finest spacing", () => {
    expect(bandLimitM()).toBe(rustConstant(geometrySource, "BAND_LIMIT_M"));
    expect(finestSpacingM()).toBe(rustConstant(geometrySource, "FINEST_SPACING_M"));
  });

  it("pins the expected test planet version to the crate's", () => {
    expect(testPlanetVersion()).toBe(rustConstant(libSource, "TEST_PLANET_VERSION"));
    expect(EXPECTED_TEST_PLANET_VERSION).toBe(testPlanetVersion());
    expect(staleModuleMessage(module)).toBeNull();
  });

  it("rejects a module that bakes another test planet version", () => {
    expect(staleModuleMessage({ ...module, testPlanetVersion: () => 2 })).toMatch(/version 2/);
  });

  it("hands out a level table of four values for each level from 0 to 24", () => {
    expect(levelTable(Ridges.Off)).toHaveLength(25 * 4);
  });

  it("bakes the native golden's three patches bit for bit", () => {
    const bakes = goldenBakes();
    expect(bakes).toHaveLength(3);
    for (const g of bakes) {
      const raw = bakePatch(
        g.face,
        g.level,
        g.i,
        g.j,
        WASM_VERTEX_PATH[g.settings.vertexPath],
        WASM_NORMAL_SCALE[g.settings.normals],
        WASM_RIDGES[g.settings.ridges],
        0,
      );
      const normals = raw.normals();
      raw.free();
      const key = { face: g.face, level: g.level, i: g.i, j: g.j };
      const bake = bakeKey(module, key, 7, g.settings);
      expect(digest(bake.heights)).toEqual(g.digests["heights"]);
      expect(bake.offsets === null ? null : digest(bake.offsets)).toEqual(g.digests["offsets"]);
      expect(digest(normals)).toEqual(g.digests["normals"]);
      expect(bake.normals).toEqual(packNormals(normals));
      expect(bake.normals).toHaveLength(normals.length);
      expect(bake.generation).toBe(7);
      expect(bitsHex(bake.originM.x)).toBe(g.scalars["origin.x"]);
      expect(bitsHex(bake.originM.y)).toBe(g.scalars["origin.y"]);
      expect(bitsHex(bake.originM.z)).toBe(g.scalars["origin.z"]);
      expect(bitsHex(bake.originHeightM)).toBe(g.scalars["origin_height_m"]);
      expect(bitsHex(bake.heightRangeM[0])).toBe(g.scalars["height_range_m.low"]);
      expect(bitsHex(bake.heightRangeM[1])).toBe(g.scalars["height_range_m.high"]);
      expect(bitsHex(bake.boundingRadiusM)).toBe(g.scalars["bounding_radius_m"]);
      expect(bitsHex(bake.skirtDepthM)).toBe(g.scalars["skirt_depth_m"]);
    }
  });

  it("answers a bake with its arrays to transfer", () => {
    const { reply, transfer } = answerRequest(
      module,
      {
        kind: "bake",
        id: 4,
        key: { face: 1, level: 5, i: 3, j: 30 },
        generation: 2,
        settings: SETTINGS,
      },
      ignoreField,
    );
    if (reply.kind !== "baked") {
      throw new Error(`the bake failed: ${JSON.stringify(reply)}`);
    }
    expect(reply.id).toBe(4);
    expect(reply.bake.heights).toHaveLength(65 * 65 * 2);
    expect(reply.bake.offsets).toHaveLength(65 * 65 * 6);
    expect(reply.bake.normals).toHaveLength(129 * 129 * 2);
    expect(transfer).toEqual([
      reply.bake.heights.buffer,
      reply.bake.normals.buffer,
      reply.bake.offsets?.buffer,
    ]);
  });

  it("answers a key the module refuses with bake-failed and nothing to transfer", () => {
    const failed = answerRequest(
      module,
      {
        kind: "bake",
        id: 5,
        key: { face: 1, level: 5, i: 32, j: 0 },
        generation: 2,
        settings: SETTINGS,
      },
      ignoreField,
    );
    expect(failed.reply.kind).toBe("bake-failed");
    expect(failed.transfer).toEqual([]);
  });

  it("rethrows a trap, so that the worker is replaced", () => {
    const trapping: HeightModule = {
      ...module,
      bakePatch: () => {
        throw new WebAssembly.RuntimeError("unreachable");
      },
    };
    expect(() =>
      answerRequest(
        trapping,
        {
          kind: "bake",
          id: 6,
          key: { face: 0, level: 0, i: 0, j: 0 },
          generation: 1,
          settings: SETTINGS,
        },
        ignoreField,
      ),
    ).toThrow(WebAssembly.RuntimeError);
  });

  it("holds a field and acknowledges it", () => {
    const bytes = new ArrayBuffer(8);
    let held: ArrayBuffer | null = null;
    const answer = answerRequest(module, { kind: "field", id: 9, bytes }, (b) => {
      held = b;
    });
    expect(answer).toEqual({ reply: { kind: "field-loaded", id: 9 }, transfer: [] });
    expect(held).toBe(bytes);
  });
});
