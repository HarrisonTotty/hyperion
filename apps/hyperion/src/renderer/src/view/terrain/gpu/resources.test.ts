import { describe, expect, it } from "vitest";

import {
  countingRenderEngine,
  fakeDevice,
  LimitExceeded,
} from "../../../test/countingRenderEngine";
import { BUFFER_USAGE } from "../../engine/gpuFlags";
import { MAX_REQUESTED_BUFFER_BYTES } from "../../engine/platform";
import { TERRAIN_SETTINGS } from "../../quality/qualitySetting";
import {
  DOUBLE_NORMALS_BYTES,
  HEIGHTS_BYTES,
  OFFSETS_BYTES,
  terrainSlotLayout,
} from "../slotLayout";
import { allocationTally } from "./allocationTally";
import {
  atlasTile,
  GRID_INDICES,
  GRID_VERTICES,
  INDIRECT_ARGS_BYTES,
  normalsAtlasLayout,
  PATCH_INDICES,
  patchMeshData,
  type SlotUpload,
  SKIRT_VERTICES,
  TerrainResources,
  terrainLayout,
} from "./resources";
import { ContactRecords, InstanceRecords, SLOT_RECORD_BYTES } from "./uniforms";

const WGS84 = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.314_245 };

/** WebGPU's default limits (W3C WebGPU §3.6.2). */
const DEFAULT_LIMITS = {
  maxStorageBufferBindingSize: 134_217_728,
  maxBufferSize: 268_435_456,
  maxTextureDimension2D: 8192,
};

/** The limits `createWebGpuEngine` asks of an adapter offering at least 1 GiB (item 7). */
const RAISED_LIMITS = {
  maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
  maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  maxTextureDimension2D: 8192,
};

const HIGH = TERRAIN_SETTINGS.high;
const LOW = TERRAIN_SETTINGS.low;

/** A bake for `slot` at the layout's sizes, every value `fill`. */
function upload(slot: number, normalsPerSide: number, offsets: boolean, fill = 1): SlotUpload {
  return {
    slot,
    key: { face: 2, level: 12, i: 100, j: 200 },
    heights: new Float32Array(HEIGHTS_BYTES / 4).fill(fill),
    offsets: offsets ? new Float32Array(OFFSETS_BYTES / 4).fill(fill) : null,
    normals: new Float16Array(normalsPerSide * normalsPerSide * 2).fill(fill),
    originHeightM: 120,
    skirtDepthM: 3,
  };
}

/** Whether grid vertex (x, y) lies on the patch's edge. */
function onEdge(x: number, y: number): boolean {
  return x === 0 || x === 64 || y === 0 || y === 64;
}

describe("the shared patch mesh", () => {
  const { positions, indices } = patchMeshData();
  const xy = (v: number): readonly [number, number, number] => [
    positions[3 * v] ?? Number.NaN,
    positions[3 * v + 1] ?? Number.NaN,
    positions[3 * v + 2] ?? Number.NaN,
  ];
  const triangle = (t: number): readonly [number, number, number] => [
    indices[3 * t] ?? -1,
    indices[3 * t + 1] ?? -1,
    indices[3 * t + 2] ?? -1,
  ];

  it("has the grid in the bake's vertex order, then the skirts", () => {
    expect(positions).toHaveLength(3 * (GRID_VERTICES + SKIRT_VERTICES));
    expect(indices).toHaveLength(PATCH_INDICES);
    expect(xy(65 * 7 + 3)).toEqual([3, 7, 0]);
    expect(Math.max(...indices)).toBe(GRID_VERTICES + SKIRT_VERTICES - 1);
  });

  it("splits every quad on its (0, 0)–(1, 1) diagonal, anticlockwise from outside", () => {
    for (let t = 0; t < GRID_INDICES / 3; t += 2) {
      const [a, b, c] = triangle(t).map(xy);
      const [d, e, f] = triangle(t + 1).map(xy);
      if (a === undefined || b === undefined || c === undefined) {
        throw new Error("a short triangle");
      }
      const [x, y] = a;
      expect([a, b, c]).toEqual([
        [x, y, 0],
        [x + 1, y, 0],
        [x + 1, y + 1, 0],
      ]);
      expect([d, e, f]).toEqual([
        [x, y, 0],
        [x + 1, y + 1, 0],
        [x, y + 1, 0],
      ]);
    }
  });

  it("hangs a skirt quad under every edge segment, each skirt vertex under its edge vertex", () => {
    let segments = 0;
    for (let t = GRID_INDICES / 3; t < PATCH_INDICES / 3; t += 2) {
      const [p, pSkirt, qSkirt] = triangle(t);
      const [p2, qSkirt2, q] = triangle(t + 1);
      expect(p2).toBe(p);
      expect(qSkirt2).toBe(qSkirt);
      const [px, py, pk] = xy(p);
      const [qx, qy, qk] = xy(q);
      expect([pk, qk]).toEqual([0, 0]);
      expect(xy(pSkirt)).toEqual([px, py, 1]);
      expect(xy(qSkirt)).toEqual([qx, qy, 1]);
      expect(onEdge(px, py) && onEdge(qx, qy)).toBe(true);
      expect(Math.abs(qx - px) + Math.abs(qy - py)).toBe(1);
      segments += 1;
    }
    expect(segments).toBe(4 * 64);
  });
});

describe("the normals atlas", () => {
  it("holds the high setting's tiles in one layer and its fallback's in two, under 8,192 texels", () => {
    const high = normalsAtlasLayout(129, 1962, 8192, 256);
    expect([high.layers, high.columns, high.rows]).toEqual([1, 62, 32]);
    const fallback = normalsAtlasLayout(129, 3904, 8192, 256);
    expect(fallback.layers).toBe(2);
    expect(fallback.widthTexels).toBeLessThanOrEqual(8192);
    expect(fallback.heightTexels).toBeLessThanOrEqual(8192);
    expect(fallback.tilesPerLayer * fallback.layers).toBeGreaterThanOrEqual(3904);
  });

  it("gives every slot its own tile", () => {
    const atlas = normalsAtlasLayout(65, 300, 1024, 256);
    const seen = new Set<string>();
    for (let slot = 0; slot < 300; slot += 1) {
      const tile = atlasTile(atlas, slot);
      expect(tile.x + atlas.tileTexels).toBeLessThanOrEqual(atlas.widthTexels);
      expect(tile.y + atlas.tileTexels).toBeLessThanOrEqual(atlas.heightTexels);
      expect(tile.layer).toBeLessThan(atlas.layers);
      seen.add(`${tile.x},${tile.y},${tile.layer}`);
    }
    expect(seen.size).toBe(300);
  });

  it("refuses more tiles than the layers hold", () => {
    expect(() => normalsAtlasLayout(129, 3905, 8192, 1)).toThrow(/do not fit/);
  });
});

describe("a terrain layout", () => {
  it("keeps BakedOffsets on the high setting where the device holds a 1 GiB binding", () => {
    const layout = terrainLayout(HIGH, RAISED_LIMITS);
    expect(layout.vertexPath).toBe("baked-offsets");
    expect(layout.fallback).toBe("none");
    expect(layout.slots).toEqual(terrainSlotLayout(HIGH));
  });

  it("falls back to FaceDifferences on the high setting under WebGPU's default limits", () => {
    const layout = terrainLayout(HIGH, DEFAULT_LIMITS);
    expect(layout.vertexPath).toBe("face-differences");
    expect(layout.fallback).toBe("binding-limit");
    expect(layout.slots).toEqual(terrainSlotLayout({ ...HIGH, vertexPath: "face-differences" }));
    expect(layout.slots.slotCount * HEIGHTS_BYTES).toBeLessThanOrEqual(
      DEFAULT_LIMITS.maxStorageBufferBindingSize,
    );
  });

  it("takes the low setting's slot count from its layout, with no fallback", () => {
    const layout = terrainLayout(LOW, DEFAULT_LIMITS);
    expect(layout.fallback).toBe("none");
    expect(layout.slots.slotCount).toBe(terrainSlotLayout(LOW).slotCount);
    expect(layout.atlas.layers).toBe(1);
  });
});

describe("the terrain's resources", () => {
  it("make no buffer above the device's limits, under default limits on the high setting", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, HIGH, WGS84);
    expect(resources.layout.fallback).toBe("binding-limit");
    expect(resources.field("offsets")).toBeNull();
    expect(resources.field("heights")?.bytes).toBeLessThanOrEqual(
      engine.capabilities.maxStorageBufferBindingSize,
    );
    resources.dispose();
  });

  it("tally to the formats' sizes in the height cache", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const tally = allocationTally(engine);
    const resources = new TerrainResources(engine, HIGH, WGS84);
    const { slots, atlas } = resources.layout;
    expect(resources.layout.vertexPath).toBe("baked-offsets");
    const slotsBytes = slots.slotCount * (HEIGHTS_BYTES + OFFSETS_BYTES + SLOT_RECORD_BYTES);
    const atlasBytes = atlas.widthTexels * atlas.heightTexels * atlas.layers * 4;
    expect(tally.liveBytes("height-cache")).toBe(slotsBytes + atlasBytes);
    // The atlas's spare tiles in its last row are its only bytes beyond the layout's.
    expect(atlasBytes - slots.slotCount * DOUBLE_NORMALS_BYTES).toBeLessThan(
      atlas.columns * DOUBLE_NORMALS_BYTES,
    );
    tally.dispose();
    resources.dispose();
  });

  it("overwrite a freed slot in place, with no allocation after warm-up", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const resources = new TerrainResources(engine, LOW, WGS84);
    resources.upload(upload(5, 65, false, 1));
    const live = tally.liveBytes("height-cache");
    expect(live).toBeGreaterThan(0);
    engine.resetCounts();
    resources.upload(upload(5, 65, false, 2));
    expect([engine.counts.buffers, engine.counts.textures, engine.counts.meshes]).toEqual([
      0, 0, 0,
    ]);
    const heights = engine.writes.filter((w) => w.buffer === "terrain heights");
    expect(heights.map((w) => w.offsetBytes)).toEqual([5 * HEIGHTS_BYTES, 5 * HEIGHTS_BYTES]);
    const records = engine.writes.filter((w) => w.buffer === "terrain slot records");
    expect(records.at(-1)?.offsetBytes).toBe(5 * SLOT_RECORD_BYTES);
    expect(tally.liveBytes("height-cache")).toBe(live);
    expect(tally.peakBytes("height-cache")).toBe(live);
    tally.dispose();
    resources.dispose();
  });

  it("write a normals tile with its gutter repeating the tile's edge", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    const patch = upload(0, 65, false);
    // Sample (x, y)'s pair is (x, y) itself, so each stored texel shows which sample it took.
    for (let y = 0; y < 65; y += 1) {
      for (let x = 0; x < 65; x += 1) {
        patch.normals[2 * (65 * y + x)] = x;
        patch.normals[2 * (65 * y + x) + 1] = y;
      }
    }
    resources.upload(patch);
    const write = engine.textureWritten.at(-1);
    expect(write?.size).toEqual({ width: 67, height: 67, depthOrArrayLayers: 1 });
    const texels = new Float16Array(write?.data.buffer ?? new ArrayBuffer(0));
    const at = (x: number, y: number): readonly [number, number] => [
      texels[2 * (67 * y + x)] ?? Number.NaN,
      texels[2 * (67 * y + x) + 1] ?? Number.NaN,
    ];
    expect(at(0, 0)).toEqual([0, 0]);
    expect(at(1, 1)).toEqual([0, 0]);
    expect(at(66, 30)).toEqual([64, 29]);
    expect(at(33, 66)).toEqual([32, 64]);
    resources.dispose();
  });

  it("refuse, writing nothing, a bake with offsets on a face-differences layout", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    engine.resetCounts();
    expect(resources.upload(upload(0, 65, true))).toEqual({
      kind: "refused",
      reason: "vertex-path-mismatch",
    });
    expect([engine.counts.bufferWrites, engine.counts.textureWrites]).toEqual([0, 0]);
    resources.dispose();
  });

  it("refuse, writing nothing, a bake for a slot past the layout's count", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    engine.resetCounts();
    expect(resources.upload(upload(resources.layout.slots.slotCount, 65, false))).toEqual({
      kind: "refused",
      reason: "slot-outside-layout",
    });
    expect([engine.counts.bufferWrites, engine.counts.textureWrites]).toEqual([0, 0]);
    resources.dispose();
  });

  it("throw, writing nothing, for normals at another setting's resolution", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    engine.resetCounts();
    expect(() => resources.upload(upload(0, 129, false))).toThrow(RangeError);
    expect([engine.counts.bufferWrites, engine.counts.textureWrites]).toEqual([0, 0]);
    resources.dispose();
  });

  it("set the indirect draw's index count once and its instance count each frame", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    expect(resources.indirect.bytes).toBe(INDIRECT_ARGS_BYTES);
    const first = engine.writes.find((w) => w.buffer === "terrain indirect");
    expect(Array.from(new Uint32Array(first?.data.buffer ?? new ArrayBuffer(0)))).toEqual([
      PATCH_INDICES,
      0,
      0,
      0,
      0,
    ]);
    const instances = new InstanceRecords(resources.layout.slots.slotCount);
    instances.push(3, { x: 1, y: 2, z: 3 }, 10, 20);
    instances.push(9, { x: 1, y: 2, z: 3 }, 10, 20);
    resources.writeFrame(instances, new ContactRecords());
    const count = engine.writes.at(-1);
    expect(count?.buffer).toBe("terrain indirect");
    expect(count?.offsetBytes).toBe(4);
    expect(Array.from(new Uint32Array(count?.data.buffer ?? new ArrayBuffer(0)))).toEqual([2]);
    resources.dispose();
  });

  it("re-derive the layout on a restore onto a lesser device, within its limits", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const resources = new TerrainResources(engine, HIGH, WGS84);
    expect(resources.layout.vertexPath).toBe("baked-offsets");
    const rebuilt: string[] = [];
    resources.onRebuilt((layout) => {
      rebuilt.push(layout.vertexPath);
    });
    // A 128 MiB adapter: the counting engine refuses, as WebGPU's validation would, any buffer
    // or texture beyond it, so the restore succeeding is the absence of a validation error.
    const lesser = await fakeDevice({
      maxStorageBufferBindingSize: 2 ** 27,
      maxBufferSize: 2 ** 28,
    });
    expect(() => {
      engine.restore(lesser);
    }).not.toThrow();
    expect(rebuilt).toEqual(["face-differences"]);
    expect(resources.layout.fallback).toBe("binding-limit");
    expect(resources.field("offsets")).toBeNull();
    resources.dispose();
  });

  it("would be refused by a device that cannot hold the offsets, which the fallback avoids", async () => {
    const engine = await countingRenderEngine();
    expect(() =>
      engine.createBuffer({
        name: "offsets at the high budget",
        bytes: terrainSlotLayout(HIGH).slotCount * OFFSETS_BYTES,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "height-cache",
      }),
    ).toThrow(LimitExceeded);
  });

  it("stop following restores once disposed", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    let rebuilt = 0;
    resources.onRebuilt(() => {
      rebuilt += 1;
    });
    resources.dispose();
    engine.restore();
    expect(rebuilt).toBe(0);
  });
});
