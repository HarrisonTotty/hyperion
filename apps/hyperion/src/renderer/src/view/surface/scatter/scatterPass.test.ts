import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../../test/countingRenderEngine";
import type { BucketDrawBase } from "../../clusters/buckets";
import { cameraCullPassOf } from "../../clusters/instanceScene";
import { BUFFER_USAGE } from "../../engine/gpuFlags";
import type { RenderEngine } from "../../engine/types";
import { SETTINGS } from "../../quality/qualitySetting";
import { NO_PARENT_SLOT } from "../../terrain/gpu/uniforms";
import { pebbleLatticeLevels } from "./pebbleLaw";
import { SCATTER_PASS_LABEL } from "./pebblePlace";
import { ScatterKernelSet } from "./scatterKernels";
import {
  PEBBLE_CAPACITY,
  SCATTER_CULL_LABEL,
  ScatterPass,
  type ScatterFrameInput,
  scatterDrawn,
} from "./scatterPass";

const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
const PROJECTION = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, 0.1, 0]);

/** A frame of one patch 1 m from the camera over buffers of `engine`'s. */
function frameOn(engine: RenderEngine, base: BucketDrawBase): ScatterFrameInput {
  const buffer = (name: string): ReturnType<RenderEngine["createBuffer"]> =>
    engine.createBuffer({
      name,
      bytes: 256,
      usage: BUFFER_USAGE.STORAGE,
      category: "height-cache",
    });
  return {
    patches: [
      {
        originM: { x: 0, y: 0, z: -1 },
        slot: 0,
        morphStartM: 0,
        morphEndM: 0,
        parentSlot: NO_PARENT_SLOT,
        parentQuadrant: 0,
        key: { face: 0, level: 18, i: 5, j: 6 },
        cells: [NO_PARENT_SLOT, 0, 0, 0],
      },
    ],
    reach: [{ nearestM: 1, areaM2: 1024, abundance: 0.2 }],
    levels: pebbleLatticeLevels(18, 32),
    rotationRows: [
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 1, z: 0 },
      { x: 0, y: 0, z: 1 },
    ],
    pxPerRad: 900,
    seed: 7,
    bodies: [],
    cull: cameraCullPassOf(IDENTITY, PROJECTION, 640, 1),
    viewAxis: { x: 0, y: 0, z: -1 },
    ground: { vertices: buffer("heights"), slots: buffer("slots"), classLayers: buffer("layers") },
    base,
  };
}

/** A lit draw's base on `engine`, its bindings none. */
function litBase(engine: RenderEngine): BucketDrawBase {
  return {
    material: engine.createMaterial({
      name: "lit",
      displayName: "LIT",
      vertexWgsl: "",
      fragmentWgsl: "",
      uniforms: [],
      samplers: [],
      cullMode: "back",
      depthWrite: true,
      colourWrites: true,
      blend: "none",
    }),
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: {},
    storageBuffers: {},
  };
}

describe("the decorative scatter's pass", () => {
  it("is drawn on the high setting alone, at a density above 0, and not where the geometry turns it off", () => {
    expect(scatterDrawn(SETTINGS.high, SETTINGS.high.geometry)).toBe(true);
    expect(scatterDrawn(SETTINGS.low, SETTINGS.low.geometry)).toBe(false);
    expect(scatterDrawn({ decoration: true, scatterDensity: 0 }, SETTINGS.high.geometry)).toBe(
      false,
    );
    expect(scatterDrawn(SETTINGS.high, { ...SETTINGS.high.geometry, scatter: "off" })).toBe(false);
  });

  it("compiles its handles once, after the session's materials, and none on the low setting", async () => {
    const engine = await countingRenderEngine();
    expect(
      ScatterKernelSet.forSetting(SETTINGS.low, SETTINGS.low.geometry, "face-differences", engine),
    ).toBeNull();
    expect(
      ScatterKernelSet.forSetting(
        SETTINGS.high,
        { ...SETTINGS.high.geometry, scatter: "off" },
        "baked-offsets",
        engine,
      ),
    ).toBeNull();
    expect(engine.kernelPairs).toHaveLength(0);
    const resolvers: Array<() => void> = [];
    const after = new Promise<void>((resolve) => {
      resolvers.push(resolve);
    });
    const release = (): void => {
      for (const resolve of resolvers) {
        resolve();
      }
    };
    const set = ScatterKernelSet.forSetting(
      SETTINGS.high,
      SETTINGS.high.geometry,
      "baked-offsets",
      engine,
      after,
    );
    expect(set).not.toBeNull();
    // Nothing asked before the session's materials have compiled.
    await Promise.resolve();
    expect(engine.compiles).toHaveLength(0);
    release();
    expect(await set?.ready).toBe(true);
    expect(engine.kernelPairs.map((k) => k.name)).toEqual([
      "scatter pebbles baked-offsets",
      "instance cull",
    ]);
    expect(engine.compiles.map((c) => c.name)).toEqual(["scatter-pebbles-baked-offsets"]);
    expect(set?.kernels?.path).toBe("baked-offsets");
    // Again after a device's loss, on the restored device.
    engine.loseDevice();
    engine.restore();
    expect(await set?.ready).toBe(true);
    expect(engine.compiles.map((c) => c.name)).toEqual([
      "scatter-pebbles-baked-offsets",
      "scatter-pebbles-baked-offsets",
    ]);
    set?.dispose();
    expect(set?.kernels).toBeNull();
  });

  it("makes no pass at a density of 0", async () => {
    const engine = await countingRenderEngine();
    const set = new ScatterKernelSet(engine, "face-differences");
    expect(() => new ScatterPass(engine, "t", set, 0)).toThrow(RangeError);
    set.dispose();
  });

  it("holds its capacity at its density, and places, culls and draws once its handles are compiled", async () => {
    const engine = await countingRenderEngine();
    const set = new ScatterKernelSet(engine, "baked-offsets");
    const pass = new ScatterPass(engine, "t", set, 0.5);
    expect(pass.path.capacity).toBe(PEBBLE_CAPACITY / 2);
    const base = litBase(engine);
    // Before its handles are compiled it draws nothing.
    expect(pass.ready).toBe(false);
    expect(pass.frame(frameOn(engine, base)).draws).toHaveLength(0);
    expect(await set.ready).toBe(true);
    expect(pass.ready).toBe(true);
    const before = engine.dispatched.length;
    const frame = pass.frame(frameOn(engine, base));
    expect(frame.entries).toBeGreaterThan(0);
    expect(frame.patches).toBe(1);
    // A draw a template, by the pebbles' material over the lit draw's bindings.
    expect(frame.draws).toHaveLength(pass.path.templates.templates.length);
    for (const draw of frame.draws) {
      expect(draw.material).toBe(set.kernels?.material);
      expect(draw.mesh).toBe(set.kernels?.mesh);
      expect(Object.keys(draw.storageBuffers ?? {})).toEqual(
        expect.arrayContaining(["scatterPatches", "particleInstances", "particleLists"]),
      );
    }
    expect(engine.dispatched.slice(before).map((d) => d.pass)).toEqual([
      SCATTER_PASS_LABEL,
      SCATTER_CULL_LABEL,
    ]);
    // Its count zeroed before the placement.
    const zeroed = engine.writes.findLast((w) => w.buffer === "t pebbles count");
    expect(new Uint32Array(zeroed?.data.slice().buffer ?? new ArrayBuffer(4))[0]).toBe(0);
    // No entry, no dispatch.
    const none = pass.frame({
      ...frameOn(engine, base),
      reach: [{ nearestM: 1e6, areaM2: 1024, abundance: 0.2 }],
    });
    expect(none.draws).toHaveLength(0);
    pass.dispose();
    expect(pass.frame(frameOn(engine, base)).draws).toHaveLength(0);
    // The handles are the set's, kept past the pass.
    expect(set.kernels).not.toBeNull();
    set.dispose();
  });

  it("names its memory scatter", async () => {
    const engine = await countingRenderEngine();
    const categories = new Map<string, number>();
    engine.onAllocation((event) => {
      if (event.kind === "created") {
        categories.set(event.category, (categories.get(event.category) ?? 0) + event.bytes);
      }
    });
    const set = new ScatterKernelSet(engine, "face-differences");
    const pass = new ScatterPass(engine, "t", set, 1);
    // Instances 32 MB and their count at a density of 1, and the records.
    expect(categories.get("scatter") ?? 0).toBeGreaterThan(32 * 2 ** 20);
    pass.dispose();
    set.dispose();
  });
});
