import { describe, expect, it, vi } from "vitest";

import {
  countingRenderEngine,
  type CountingRenderEngine,
} from "../../../test/countingRenderEngine";
import { SETTINGS } from "../../quality/qualitySetting";
import type { TerrainLayers } from "../../terrain/workers/messages";
import { PALETTE_NONE } from "../../terrain/workers/messages";
import {
  AIRLESS_PRESSURE_PA,
  DETAIL_BODY_BYTES_MAX,
  DETAIL_FORMS,
  DETAIL_KERNELS,
  DETAIL_LAYERS,
  DETAIL_MIPS,
  DETAIL_PASS_LABEL,
  DETAIL_SET_BYTES,
  DETAIL_TEXELS,
  type DetailBody,
  type DetailMaterial,
  detailMaterialsOf,
  DetailKernelSet,
  detailParams,
  DetailSetBake,
  hueAxisOf,
  packDetailParams,
} from "./detailTextureBake";

const MOON: DetailBody = {
  seed: 0x5eed_0001,
  radiusM: 1_737_400,
  level: 22,
  startLevel: 17,
  surfacePressurePa: 0,
  surfaceAgeGyr: 3.8,
  maturity: 0.7,
  feoByEntry: [0.16, 0.06],
};

const MARS: DetailBody = { ...MOON, surfacePressurePa: 600, maturity: null };

const BASALT: DetailMaterial = {
  classCode: 1,
  entry: 0,
  albedoBvr: [0.08, 0.1, 0.12],
  weathering: { fresh: [0.12, 0.14, 0.16], mature: [0.07, 0.09, 0.11] },
};

/** A patch's layers naming `slots`, each (class, entry, B, V, R), with no maturity. */
function layersOf(
  slots: ReadonlyArray<readonly [number, number, number, number, number]>,
): TerrainLayers {
  const palette = new Uint8Array(16).fill(PALETTE_NONE);
  const photometry = new Float32Array(40);
  slots.forEach(([classCode, entry, b, v, r], k) => {
    palette.set([classCode, entry], 2 * k);
    photometry.set([b, v, r, 1, 0.5], 5 * k);
  });
  return {
    palette,
    weights: new Uint8Array(0),
    surveyMask: new Uint8Array(0),
    fieldLevel: 7,
    boundM: 1,
    sigmaM: 0.1,
    complete: true,
    unsurveyedCells: new Uint32Array(0),
    horizonNear: new Uint32Array(0),
    horizonFar: new Uint32Array(0),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(0),
    photometry,
  };
}

/** A generator's packed bytes, by which two generators are told apart. */
function bytesOf(material: DetailMaterial, body: DetailBody): string {
  const params = detailParams(material, body);
  if (params === null) {
    throw new Error("no generator");
  }
  return Array.from(new Uint8Array(packDetailParams(params, 0, 0))).join(",");
}

describe("a material's generator (Design note 21)", () => {
  it("takes the lean's family for each form, and none for Seabed", () => {
    const family = (classCode: number, v = 0.1): string | null =>
      detailParams({ ...BASALT, classCode, albedoBvr: [v, v, v] }, MOON)?.family ?? null;
    expect([0, 1, 2, 3, 14].map((c) => family(c))).toEqual([
      "fractured",
      "grains",
      "grains",
      "grains",
      "grains",
    ]);
    expect([family(5, 0.1), family(5, 0.35)]).toEqual(["vesicular", "flowBanded"]);
    expect([4, 11].map((c) => family(c))).toEqual(["laminae", "laminae"]);
    expect([7, 8, 10].map((c) => family(c))).toEqual(["crystalline", "crystalline", "crystalline"]);
    expect([family(9), family(13), family(6)]).toEqual(["plates", "polygons", "skin"]);
    expect(family(12)).toBeNull();
    expect(family(15)).toBeNull();
    expect(DETAIL_FORMS).toHaveLength(15);
  });

  it("sorts and rounds grains under air, and mixes fresh and mature grains on airless ground", () => {
    const airless = detailParams(BASALT, MOON);
    const air = detailParams(BASALT, MARS);
    expect(airless?.weathered).toBe(0.7);
    expect(air?.weathered).toBe(0);
    expect((air?.sorting ?? 0) > (airless?.sorting ?? 1)).toBe(true);
    expect(
      MOON.surfacePressurePa < AIRLESS_PRESSURE_PA && MARS.surfacePressurePa > AIRLESS_PRESSURE_PA,
    ).toBe(true);
  });

  it("differs between two bodies that differ only in seed, pressure class, albedo, FeO_T or maturity", () => {
    const unweathered = { ...BASALT, weathering: null };
    const base = bytesOf(BASALT, MOON);
    expect(bytesOf(BASALT, { ...MOON, seed: MOON.seed + 1 })).not.toBe(base);
    expect(bytesOf(BASALT, MARS)).not.toBe(base);
    expect(bytesOf({ ...BASALT, albedoBvr: [0.1, 0.12, 0.15] }, MOON)).not.toBe(base);
    expect(bytesOf(unweathered, { ...MOON, feoByEntry: [0.08, 0.06] })).not.toBe(
      bytesOf(unweathered, MOON),
    );
    expect(bytesOf(BASALT, { ...MOON, maturity: 0.3 })).not.toBe(base);
  });

  it("keys each material of one body apart", () => {
    expect(detailParams(BASALT, MOON)?.seed).not.toBe(
      detailParams({ ...BASALT, entry: 1 }, MOON)?.seed,
    );
    expect(detailParams(BASALT, MOON)?.seed).not.toBe(
      detailParams({ ...BASALT, classCode: 2 }, MOON)?.seed,
    );
  });

  it("takes whole powers of 2 of cells across the tile, at most its texels", () => {
    const all = Array.from({ length: 15 }, (_, classCode) =>
      detailParams({ ...BASALT, classCode }, MOON),
    ).flatMap((params) => (params === null ? [] : [params]));
    expect(all).toHaveLength(14);
    for (const params of all) {
      expect(Number.isInteger(Math.log2(params.cells))).toBe(true);
      expect(params.cells).toBeLessThanOrEqual(DETAIL_TEXELS);
      expect(params.texelM).toBeGreaterThan(0);
    }
  });
});

describe("a layer's hue axis", () => {
  it("runs from fresh to mature on airless ground with maturity nodes: darker, unit length", () => {
    const axis = hueAxisOf(BASALT, MOON);
    expect(Math.hypot(...axis)).toBeCloseTo(1, 12);
    expect(axis[1]).toBeLessThan(0);
  });

  it("follows the entry's own colour under air", () => {
    expect(hueAxisOf(BASALT, MARS)).toEqual([0.08 / 0.1 - 1, 0, 0.12 / 0.1 - 1]);
  });
});

describe("a patch's materials", () => {
  it("are its slots with a class and a named entry, in order, with their photometry", () => {
    const materials = detailMaterialsOf(
      layersOf([
        [1, 0, 0.1, 0.12, 0.14],
        [PALETTE_NONE, PALETTE_NONE, 0, 0, 0],
        [3, 1, 0.3, 0.32, 0.35],
      ]),
    );
    expect(materials.map((m) => [m.classCode, m.entry])).toEqual([
      [1, 0],
      [3, 1],
    ]);
    expect(materials[0]?.albedoBvr.map((x) => Math.fround(x))).toEqual(
      [0.1, 0.12, 0.14].map((x) => Math.fround(x)),
    );
    expect(materials[0]?.weathering).toBeNull();
  });
});

describe("the generator's uniform", () => {
  it("packs DetailParams' sixteen words in order", () => {
    const params = detailParams(BASALT, MOON);
    if (params === null) {
      throw new Error("no generator");
    }
    const buffer = packDetailParams(params, 3, 5);
    expect(buffer.byteLength).toBe(64);
    const u32 = new Uint32Array(buffer);
    const f32 = new Float32Array(buffer);
    const i32 = new Int32Array(buffer);
    expect(Array.from(u32.subarray(0, 4))).toEqual([params.seed, 0, 3, 5]);
    expect(f32[4]).toBe(Math.fround(params.texelM));
    expect([u32[5], u32[6]]).toEqual([params.cells, params.fineCells]);
    expect(f32[11]).toBe(Math.fround(params.weathered));
    expect([i32[14], i32[15]]).toEqual(params.bands);
  });
});

/** A kernel set of `engine`'s on the high setting, compiled. */
async function compiledSet(engine: CountingRenderEngine): Promise<DetailKernelSet> {
  const set = DetailKernelSet.forSetting(SETTINGS.high, engine);
  if (set === null) {
    throw new Error("the high setting made no kernel set");
  }
  expect(await set.ready).toBe(true);
  return set;
}

describe("the generator's kernels", () => {
  it("are none on the low setting, which creates no generator pipeline", async () => {
    const engine = await countingRenderEngine();
    expect(DetailKernelSet.forSetting(SETTINGS.low, engine)).toBeNull();
    expect(engine.counts.kernels).toBe(0);
  });

  it("are the four, compiled once on the high setting and released when let go", async () => {
    const engine = await countingRenderEngine();
    const set = await compiledSet(engine);
    expect(engine.kernelPairs.map((pair) => pair.name)).toEqual(
      DETAIL_KERNELS.map((pair) => pair.name),
    );
    expect(set.kernels).not.toBeNull();
    set.dispose();
    expect(set.kernels).toBeNull();
    expect(await set.ready).toBe(true);
  });

  it("are compiled again at a device's restore", async () => {
    const engine = await countingRenderEngine();
    const set = await compiledSet(engine);
    const before = set.kernels;
    engine.loseDevice();
    engine.restore();
    expect(set.kernels).toBeNull();
    expect(await set.ready).toBe(true);
    expect(set.kernels).not.toBeNull();
    expect(set.kernels).not.toBe(before);
    expect(engine.kernelPairs).toHaveLength(2 * DETAIL_KERNELS.length);
    set.dispose();
  });
});

describe("the generator's kernels, let go or restored mid-compile", () => {
  it("are released as their compile settles when the set is let go first", async () => {
    const engine = await countingRenderEngine();
    const released = vi.spyOn(engine, "releaseCompute");
    const set = DetailKernelSet.forSetting(SETTINGS.high, engine);
    set?.dispose();
    expect(await set?.ready).toBe(false);
    expect(released).toHaveBeenCalledTimes(DETAIL_KERNELS.length);
    expect(set?.kernels).toBeNull();
  });

  it("take the restored device's compile, never the lost one's, when restored mid-compile", async () => {
    const engine = await countingRenderEngine();
    const released = vi.spyOn(engine, "releaseCompute");
    const set = DetailKernelSet.forSetting(SETTINGS.high, engine);
    engine.loseDevice();
    engine.restore();
    expect(await set?.ready).toBe(true);
    expect(engine.kernelPairs).toHaveLength(2 * DETAIL_KERNELS.length);
    expect(set?.kernels).not.toBeNull();
    // The lost device's handles are dropped, not released.
    expect(released).not.toHaveBeenCalled();
    set?.dispose();
    expect(released).toHaveBeenCalledTimes(DETAIL_KERNELS.length);
  });

  it("settle false, never rejecting, where a kernel cannot be asked for", async () => {
    const engine = await countingRenderEngine();
    engine.loseDevice();
    const set = DetailKernelSet.forSetting(SETTINGS.high, engine);
    expect(await set?.ready).toBe(false);
    expect(set?.kernels).toBeNull();
    set?.dispose();
  });
});

describe("a body's detail sets", () => {
  it("wait for the kernels, generating nothing before they are compiled", async () => {
    const engine = await countingRenderEngine();
    const bake = new DetailSetBake(engine, { kernels: null }, MOON, "moon");
    bake.noteMaterials([BASALT], MOON.startLevel);
    expect(bake.step()).toBe(false);
    expect(bake.waiting).toBe(1);
    expect(engine.dispatched).toHaveLength(0);
    expect(bake.texture).toBeNull();
  });

  it("generate one layer a step, each in eleven dispatches, under the decoration category", async () => {
    const engine = await countingRenderEngine();
    const set = await compiledSet(engine);
    const bake = new DetailSetBake(engine, set, MOON, "moon");
    bake.note(
      layersOf([
        [1, 0, 0.1, 0.12, 0.14],
        [0, 1, 0.2, 0.22, 0.24],
      ]),
      MOON.startLevel,
    );
    expect(bake.waiting).toBe(2);
    expect(bake.step()).toBe(true);
    expect(engine.dispatched).toHaveLength(2 + DETAIL_MIPS);
    expect(new Set(engine.dispatched.map((d) => d.pass))).toEqual(new Set([DETAIL_PASS_LABEL]));
    expect(engine.textureSpecs.map((s) => s.category)).toEqual(["decoration"]);
    expect(bake.layerOf(1, 0)?.layer).toBe(0);
    expect(bake.layerOf(0, 1)).toBeNull();
    expect(bake.step()).toBe(true);
    expect(bake.step()).toBe(false);
    expect(bake.layers.map((l) => l.layer)).toEqual([0, 1]);
    expect(bake.memoryBytes).toBeGreaterThan(DETAIL_SET_BYTES);
    expect(bake.memoryBytes).toBeLessThanOrEqual(DETAIL_BODY_BYTES_MAX);
    bake.dispose();
    expect(engine.live.textures).toBe(0);
    expect(engine.live.buffers).toBe(0);
    set.dispose();
  });

  it("wait for a patch at the body's finest level, then take every patch's, each material once, at most sixteen", async () => {
    const engine = await countingRenderEngine();
    const bake = new DetailSetBake(engine, { kernels: null }, MOON, "moon");
    bake.note(layersOf([[1, 0, 0.1, 0.12, 0.14]]), MOON.startLevel - 1);
    expect(bake.waiting).toBe(0);
    bake.note(layersOf([[1, 0, 0.1, 0.12, 0.14]]), MOON.startLevel);
    bake.note(layersOf([[1, 0, 0.1, 0.12, 0.14]]), MOON.startLevel);
    expect(bake.waiting).toBe(1);
    bake.note(layersOf([[3, 0, 0.3, 0.32, 0.35]]), MOON.startLevel - 4);
    expect(bake.waiting).toBe(2);
    for (let entry = 0; entry < 20; entry += 1) {
      bake.noteMaterials([{ ...BASALT, entry, classCode: 0 }], MOON.startLevel);
    }
    expect(bake.waiting).toBe(DETAIL_LAYERS);
    bake.dispose();
    expect(bake.waiting).toBe(0);
  });
});
