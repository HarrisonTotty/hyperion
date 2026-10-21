import { describe, expect, it, vi } from "vitest";

import {
  type CountingRenderEngine,
  countingRenderEngine,
  fakeDevice,
  LimitExceeded,
} from "../../../test/countingRenderEngine";
import { BUFFER_USAGE } from "../../engine/gpuFlags";
import { EngineUnavailable } from "../../engine/resilientEngine";
import { MAX_REQUESTED_BUFFER_BYTES } from "../../engine/platform";
import { TERRAIN_SETTINGS } from "../../quality/qualitySetting";
import { PHASE_TABLE_SAMPLES } from "../../appearance/law";
import { lawFor } from "../../appearance/phase";
import {
  CLASS_LAWS_BYTES,
  CLASS_LAYERS_BYTES,
  CLASS_WEIGHTS_BYTES,
  DOUBLE_NORMALS_BYTES,
  HEIGHTS_BYTES,
  HORIZON_MAP_BYTES,
  OFFSETS_BYTES,
  type SlotFieldName,
  SURVEY_CELL_CODE_BYTE,
  SURVEY_MASK_BYTES,
  terrainSlotLayout,
} from "../slotLayout";
import { bindingStages } from "../../../test/wgslStages";
import { materialBindings, storageBuffersPerStage } from "../../engine/webgpu/materials";
import { terrainLitMaterialSpec } from "../terrainMaterial";
import { terrainWireMaterialSpec } from "../terrainWire";
import { terrainMaterialSpec } from "./material";
import type { TerrainLayers } from "../workers/messages";
import { allocationTally } from "./allocationTally";
import {
  atlasTile,
  DEFAULT_TERRAIN_VIEW,
  fittedTerrainLayout,
  GRID_INDICES,
  GRID_VERTICES,
  INDIRECT_ARGS_BYTES,
  LIT_PATCH_INDICES,
  LIT_VERTEX_CODE,
  litPatchMeshData,
  NO_HORIZON_WORD,
  normalsAtlasLayout,
  normalsAtlasOf,
  PATCH_INDICES,
  packSurveyMask,
  patchMeshData,
  type SlotUpload,
  SKIRT_VERTICES,
  SURVEY_FIN_LINES,
  surveyCellCode,
  surveyedAt,
  TerrainResources,
  terrainLayout,
} from "./resources";
import {
  ContactRecords,
  InstanceRecords,
  SLOT_RECORD_BYTES,
  WIRE_PHASE_RECORD_BYTES,
  WirePhaseRecords,
} from "./uniforms";

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
    layers: null,
  };
}

/** Whether grid vertex (x, y) lies on the patch's edge. */
function onEdge(x: number, y: number): boolean {
  return x === 0 || x === 64 || y === 0 || y === 64;
}

/** A lit bake's layers with a horizon map of distinct words in its two fields. */
function mappedLayers(): TerrainLayers {
  return {
    palette: new Uint8Array(16),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(65 * 65),
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(65 * 65 * 4).fill(0x0001_0002),
    horizonFar: new Uint32Array(9 * 9 * 4).fill(0x0003_0004),
    horizonComplete: false,
    horizonRefusedCells: Uint32Array.of(5),
  };
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

describe("the lit material's mesh (R10.T10.c)", () => {
  const { positions, indices } = litPatchMeshData();
  const shared = patchMeshData();
  const at = (v: number): readonly [number, number, number] => [
    positions[3 * v] ?? Number.NaN,
    positions[3 * v + 1] ?? Number.NaN,
    positions[3 * v + 2] ?? Number.NaN,
  ];
  const triangle = (t: number): readonly [number, number, number] => [
    indices[3 * t] ?? -1,
    indices[3 * t + 1] ?? -1,
    indices[3 * t + 2] ?? -1,
  ];
  const C = LIT_VERTEX_CODE;

  it("keeps the shared mesh's vertices and grid, then adds the skirts' heads and the fins' datum ends", () => {
    expect(positions.subarray(0, shared.positions.length)).toEqual(shared.positions);
    expect(indices.subarray(0, GRID_INDICES)).toEqual(shared.indices.subarray(0, GRID_INDICES));
    expect(positions).toHaveLength(3 * (GRID_VERTICES + 2 * SKIRT_VERTICES + 6 * 65));
    expect(indices).toHaveLength(LIT_PATCH_INDICES);
    expect(LIT_PATCH_INDICES).toBe(PATCH_INDICES + 4 * 64 * 6 + 6 * 64 * 12);
    expect(Math.max(...indices)).toBe(positions.length / 3 - 1);
    const codes = new Set<number>();
    for (let v = 0; v < positions.length / 3; v += 1) {
      codes.add(at(v)[2]);
    }
    expect([...codes].toSorted((x, y) => x - y)).toEqual([
      C.grid,
      C.skirtFoot,
      C.skirtHead,
      C.columnFin,
      C.rowFin,
    ]);
  });

  it("starts every triangle with a vertex of its own part, which the fragment reads flat", () => {
    const parts = new Map<number, number>();
    for (let t = 0; t < LIT_PATCH_INDICES / 3; t += 1) {
      const [first, second, third] = triangle(t).map((v) => at(v)[2]);
      parts.set(first ?? -1, (parts.get(first ?? -1) ?? 0) + 1);
      // Every other vertex of a skirt's or a fin's triangle is its own part's or the grid's.
      for (const code of [second, third]) {
        expect([first, C.grid]).toContain(code);
      }
    }
    expect(Object.fromEntries(parts)).toEqual({
      [C.grid]: 64 * 64 * 2,
      [C.skirtFoot]: 4 * 64 * 2,
      [C.skirtHead]: 4 * 64 * 2,
      [C.columnFin]: 3 * 64 * 4,
      [C.rowFin]: 3 * 64 * 4,
    });
  });

  it("hangs each skirt's foot and raises its head under and over every edge segment, anticlockwise from outside", () => {
    const start = GRID_INDICES / 3;
    for (let segment = 0; segment < 4 * 64; segment += 1) {
      const [pFoot, qFoot, p] = triangle(start + 4 * segment);
      const [qFoot2, q, p2] = triangle(start + 4 * segment + 1);
      const [pHead, p3, q3] = triangle(start + 4 * segment + 2);
      const [pHead2, q4, qHead] = triangle(start + 4 * segment + 3);
      expect([qFoot2, p2, p3, q3, pHead2, q4]).toEqual([qFoot, p, p, q, pHead, q]);
      const [px, py] = at(p);
      const [qx, qy] = at(q);
      expect(onEdge(px, py) && onEdge(qx, qy)).toBe(true);
      expect(at(pFoot)).toEqual([px, py, C.skirtFoot]);
      expect(at(qFoot)).toEqual([qx, qy, C.skirtFoot]);
      expect(at(pHead)).toEqual([px, py, C.skirtHead]);
      expect(at(qHead)).toEqual([qx, qy, C.skirtHead]);
    }
    // The foot strips are R05's skirt quads, each triangle's corners turned to start at its foot,
    // which keeps its winding.
    const r05 = (t: number): number[] => [...shared.indices.subarray(3 * t, 3 * t + 3)];
    for (let segment = 0; segment < 4 * 64; segment += 1) {
      for (const k of [0, 1]) {
        const [a, b, c] = triangle(start + 4 * segment + k);
        expect([c, a, b]).toEqual(r05(start + 2 * segment + k));
      }
    }
  });

  it("stands a fin in both windings along columns and rows 16, 32 and 48", () => {
    expect(SURVEY_FIN_LINES).toEqual([16, 32, 48]);
    const start = PATCH_INDICES / 3 + 4 * 64 * 2;
    const lines = new Set<string>();
    for (let quad = 0; quad < 6 * 64; quad += 1) {
      const t = start + 4 * quad;
      const [pDatum, qDatum, q] = triangle(t);
      const [pDatum2, q2, p] = triangle(t + 1);
      expect([pDatum2, q2]).toEqual([pDatum, q]);
      expect([...triangle(t + 2)]).toEqual([pDatum, q, qDatum]);
      expect([...triangle(t + 3)]).toEqual([pDatum, p, q]);
      const [px, py] = at(p);
      const [qx, qy, qCode] = at(qDatum);
      const code = at(pDatum)[2];
      expect(qCode).toBe(code);
      expect(at(pDatum).slice(0, 2)).toEqual([px, py]);
      expect([qx, qy]).toEqual(at(q).slice(0, 2));
      lines.add(code === C.columnFin ? `x ${String(px)}` : `y ${String(py)}`);
    }
    expect([...lines].toSorted()).toEqual(["x 16", "x 32", "x 48", "y 16", "y 32", "y 48"]);
  });
});

/** A survey mask of a patch whose vertices are surveyed where `surveyed(x, y)` says. */
function maskOf(surveyed: (x: number, y: number) => boolean): Uint8Array {
  const mask = new Uint8Array(65 * 65);
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      mask[65 * y + x] = surveyed(x, y) ? 255 : 0;
    }
  }
  return mask;
}

/** Whether a vertex coordinate lies strictly inside the cell of quads 16 to 32. */
function inCell(f: number): boolean {
  return f > 16 && f < 32;
}

describe("the survey mask (R10.T10.c)", () => {
  it("codes a coarse cell's quads along a patch's side as log₂, clamped to 0 to 6", () => {
    expect(surveyCellCode(8, 8)).toBe(6);
    expect(surveyCellCode(19, 5)).toBe(6);
    expect(surveyCellCode(7, 8)).toBe(5);
    expect(surveyCellCode(3, 8)).toBe(1);
    expect(surveyCellCode(2, 8)).toBe(0);
    expect(surveyCellCode(0, 8)).toBe(0);
    expect(() => surveyCellCode(2.5, 8)).toThrow(RangeError);
  });

  it("packs 65 × 65 bytes four to a word, then the code, in 4,228 bytes", () => {
    const mask = maskOf((x, y) => (x + y) % 3 === 0);
    const packed = packSurveyMask(mask, 4);
    expect(packed).toHaveLength(SURVEY_MASK_BYTES);
    const words = new Uint32Array(packed.buffer);
    expect(words).toHaveLength(1_057);
    // Vertex (x, y)'s byte at 65 y + x, byte k of word w for vertex 4 w + k, from its low byte.
    for (const [x, y] of [
      [0, 0],
      [3, 0],
      [7, 9],
      [64, 64],
    ] as const) {
      const n = 65 * y + x;
      expect(((words[Math.floor(n / 4)] ?? 0) >>> (8 * (n % 4))) & 0xff).toBe(mask[n]);
    }
    expect(packed[SURVEY_CELL_CODE_BYTE]).toBe(4);
    expect(((words[1_056] ?? 0) >>> 8) & 0xff).toBe(4);
    expect(() => packSurveyMask(new Uint8Array(4_224), 4)).toThrow(RangeError);
    expect(() => packSurveyMask(mask, 7)).toThrow(RangeError);
  });

  it("reads a fragment's cell at its centre vertex, whatever its edge vertices hold", () => {
    // Cells of 16 quads (code 4): the cell at quads 16 to 32 surveyed, its edge vertices (on x or y
    // 16 or 32) holding the neighbours' state, as a vertex on a cell's edge may.
    const mask = maskOf((x, y) => inCell(x) && inCell(y));
    const packed = packSurveyMask(mask, 4);
    expect(surveyedAt(packed, 16.01, 16.01)).toBe(true);
    expect(surveyedAt(packed, 31.99, 24)).toBe(true);
    expect(surveyedAt(packed, 32.01, 24)).toBe(false);
    expect(surveyedAt(packed, 15.99, 24)).toBe(false);
    // Read by the interpolated mask instead, the surveyed cell would lose half a quad at its edge.
    expect(surveyedAt(packSurveyMask(mask, 0), 16.4, 24)).toBe(false);
    expect(surveyedAt(packSurveyMask(mask, 0), 16.6, 24)).toBe(true);
    // A patch inside one cell (code 6) reads its centre vertex, an edge vertex aside.
    const inside = packSurveyMask(
      maskOf((x) => x !== 0),
      6,
    );
    expect(surveyedAt(inside, 0.2, 0.2)).toBe(true);
    expect(surveyedAt(inside, 64, 64)).toBe(true);
  });

  it("write each lit slot's mask and code, from its bake's mask and level", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    expect(resources.field("survey-mask")?.bytes).toBe(
      resources.layout.slots.slotCount * SURVEY_MASK_BYTES,
    );
    const mask = maskOf((x) => x < 20);
    const bake = upload(5, 65, false);
    resources.upload({ ...bake, layers: { ...mappedLayers(), surveyMask: mask, fieldLevel: 9 } });
    const write = engine.writes.findLast((w) => w.buffer === "terrain survey-mask");
    expect([write?.offsetBytes, write?.bytes]).toEqual([5 * SURVEY_MASK_BYTES, SURVEY_MASK_BYTES]);
    const written = new Uint8Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
    // The bake's key is at level 12 over a field at level 9: cells of 2^(12 + 6 − 9) quads, code 6.
    expect(written).toEqual(packSurveyMask(mask, 6));
    resources.dispose();
  });

  it("refuse a mask of another size before any write", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    engine.resetCounts();
    const short = { ...mappedLayers(), surveyMask: new Uint8Array(64 * 64) };
    expect(() => resources.upload({ ...upload(4, 65, false), layers: short })).toThrow(RangeError);
    expect(engine.counts.bufferWrites).toBe(0);
    resources.dispose();
  });

  it("make the lit mesh and its index count for a lit layout, R05's for the spike", async () => {
    const engine = await countingRenderEngine();
    // The indirect draw's index count, as the resources made last wrote it.
    const indexCount = (): number => {
      const write = engine.writes.findLast(
        (w) => w.buffer === "terrain indirect" && w.offsetBytes === 0,
      );
      return new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(4))[0] ?? -1;
    };
    const lit = new TerrainResources(engine, LOW, WGS84, "lit");
    lit.viewBuffers(DEFAULT_TERRAIN_VIEW);
    expect([lit.mesh.name, indexCount()]).toEqual(["terrain lit patch", LIT_PATCH_INDICES]);
    const spike = new TerrainResources(engine, LOW, WGS84);
    spike.viewBuffers(DEFAULT_TERRAIN_VIEW);
    expect([spike.mesh.name, indexCount()]).toEqual(["terrain patch", PATCH_INDICES]);
    expect(spike.field("survey-mask")).toBeNull();
    lit.dispose();
    spike.dispose();
  });

  it("makes no view's frame buffers, the spike's default view's included, before its first frame", async () => {
    const engine = await countingRenderEngine();
    const created: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "created") {
        created.push(event.name);
      }
    });
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    const before = created.length;
    resources.viewBuffers(DEFAULT_TERRAIN_VIEW);
    expect(created.slice(before)).toEqual([
      "terrain instances",
      "terrain contacts",
      "terrain indirect",
      "terrain lights",
    ]);
    resources.dispose();
  });
});

/** The material's name of each slot field kept in a storage buffer. */
const STORAGE_NAMES: Readonly<Record<SlotFieldName, string>> = {
  heights: "heights",
  offsets: "offsets",
  normals: "normals",
  "horizon-map": "horizonMap",
  "class-layers": "classLayers",
  "survey-mask": "surveyMask",
};

describe("the terrain materials' storage by stage (R10.T10.c; decision-r10-t10b-r09-t2e §5)", () => {
  // Every terrain material, on both vertex paths, in lit and spike shading, with the storage
  // fields its layout makes.
  const cases = (["baked-offsets", "face-differences"] as const).flatMap((path) => [
    { spec: terrainMaterialSpec(path), shading: "spike" as const, path },
    { spec: terrainLitMaterialSpec(path), shading: "lit" as const, path },
  ]);

  it("count at most WebGPU's default eight storage buffers in each stage, by their declared stages", () => {
    const over = cases.filter(({ spec }) => {
      const counts = storageBuffersPerStage(materialBindings(spec));
      return Math.max(counts.vertex, counts.fragment) > 8;
    });
    expect(over.map(({ spec }) => spec.name)).toEqual([]);
    expect(
      cases.map(({ spec }) => [spec.name, storageBuffersPerStage(materialBindings(spec))]),
    ).toEqual([
      ["terrain-baked-offsets", { vertex: 5, fragment: 0 }],
      ["terrain-lit-baked-offsets", { vertex: 6, fragment: 4 }],
      ["terrain-face-differences", { vertex: 4, fragment: 0 }],
      ["terrain-lit-face-differences", { vertex: 5, fragment: 4 }],
    ]);
  });

  it("declare each binding to the stages whose entry points read it, and bind every storage field its layout makes", async () => {
    const engine = await countingRenderEngine();
    for (const { spec, shading, path } of cases) {
      const read = bindingStages(spec.vertexWgsl);
      for (const buffer of spec.storageBuffers ?? []) {
        const declared = [...(buffer.stages ?? ["vertex", "fragment"])].toSorted();
        expect([spec.name, buffer.name, ...declared]).toEqual([
          spec.name,
          buffer.name,
          ...[...(read.get(buffer.binding) ?? [])].toSorted(),
        ]);
      }
      // Each slot field kept in a storage buffer with bytes is one the material declares.
      const resources = new TerrainResources(
        engine,
        { ...LOW, vertexPath: path, cacheBytes: 2 ** 25 },
        WGS84,
        shading,
      );
      const names = new Set((spec.storageBuffers ?? []).map((buffer) => buffer.name));
      const unbound = resources.layout.slots.fields
        .filter((field) => field.storage === "storage-buffer" && field.bytes > 0)
        .filter((field) => !names.has(STORAGE_NAMES[field.name]))
        .map((field) => field.name);
      expect([spec.name, ...unbound]).toEqual([spec.name]);
      resources.dispose();
    }
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

  it("gives no fitted layout where the atlas cannot hold one tile, which terrainLayout throws for (R10.T14.d)", () => {
    const tiny = { ...RAISED_LIMITS, maxTextureDimension2D: 64 };
    expect([
      fittedTerrainLayout(HIGH, tiny, "lit"),
      fittedTerrainLayout(HIGH, tiny, "wire")?.vertexPath,
    ]).toEqual([null, "baked-offsets"]);
    expect(() => terrainLayout(HIGH, tiny, "lit")).toThrow(/do not fit/);
  });

  it("takes the low setting's slot count from its layout, with no fallback", () => {
    const layout = terrainLayout(LOW, DEFAULT_LIMITS);
    expect(layout.fallback).toBe("none");
    expect(layout.slots.slotCount).toBe(terrainSlotLayout(LOW).slotCount);
    expect(normalsAtlasOf(layout).layers).toBe(1);
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
    const { slots } = resources.layout;
    const atlas = normalsAtlasOf(resources.layout);
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

  it("write a lit layout's class weights and laws for each slot, adding the laws' rows", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    expect(resources.field("class-layers")?.bytes).toBe(
      resources.layout.slots.slotCount * CLASS_LAYERS_BYTES,
    );
    expect(resources.lit).not.toBeNull();
    const photometry = new Float32Array(40);
    photometry.set([0.1, 0.12, 0.14, 1, 1, 0.8, 0.82, 0.84, 2, 0.72]);
    const layers: TerrainLayers = {
      palette: new Uint8Array(16),
      weights: new Uint8Array(),
      surveyMask: new Uint8Array(65 * 65),
      fieldLevel: 8,
      boundM: 1,
      sigmaM: 0,
      complete: true,
      unsurveyedCells: new Uint32Array(),
      photometry,
      horizonNear: new Uint32Array(),
      horizonFar: new Uint32Array(),
      horizonComplete: true,
      horizonRefusedCells: new Uint32Array(),
    };
    expect(resources.upload({ ...upload(3, 65, false), layers }).kind).toBe("uploaded");
    // The slot's class layers: its weights, then its laws.
    const [weights, laws] = engine.writes.filter((w) => w.buffer === "terrain class-layers");
    expect([weights?.offsetBytes, weights?.bytes]).toEqual([
      3 * CLASS_LAYERS_BYTES,
      CLASS_WEIGHTS_BYTES,
    ]);
    expect(weights?.data.every((b) => b === 0)).toBe(true);
    expect([laws?.offsetBytes, laws?.bytes]).toEqual([
      3 * CLASS_LAYERS_BYTES + CLASS_WEIGHTS_BYTES,
      CLASS_LAWS_BYTES,
    ]);
    // Row 0 is the body's uniform law; the regolith's and the frost's rows follow it.
    expect(resources.phaseRows.laws.map((law) => law.lommelSeeligerShare)).toEqual(
      [0, 1, 0.72].map(Math.fround),
    );
    const table = engine.textureWritten.findLast((w) => w.texture === "terrain phase table");
    expect(table?.size).toEqual({ width: PHASE_TABLE_SAMPLES, height: 3 });
    resources.dispose();
  });

  it("write a lit layout's horizon map for each slot, the near field then the far", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    expect(resources.field("horizon-map")?.bytes).toBe(
      resources.layout.slots.slotCount * HORIZON_MAP_BYTES,
    );
    expect(resources.holdsHorizon(2)).toBe(false);
    expect(resources.upload({ ...upload(2, 65, false), layers: mappedLayers() }).kind).toBe(
      "uploaded",
    );
    expect(resources.holdsHorizon(2)).toBe(true);
    const [nearWrite, farWrite] = engine.writes.filter((w) => w.buffer === "terrain horizon-map");
    expect([nearWrite?.offsetBytes, nearWrite?.bytes]).toEqual([2 * HORIZON_MAP_BYTES, 67_600]);
    expect([farWrite?.offsetBytes, farWrite?.bytes]).toEqual([
      2 * HORIZON_MAP_BYTES + 67_600,
      1_296,
    ]);
    resources.dispose();
  });

  it("write no horizon, −90° in every azimuth, to a lit slot whose bake has no map, and no map to blend to", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    resources.upload({ ...upload(2, 65, false), layers: mappedLayers() });
    const none = {
      ...mappedLayers(),
      horizonNear: new Uint32Array(),
      horizonFar: new Uint32Array(),
    };
    expect(resources.upload({ ...upload(2, 65, false), layers: none }).kind).toBe("uploaded");
    expect(resources.holdsHorizon(2)).toBe(false);
    const blank = engine.writes.findLast((w) => w.buffer === "terrain horizon-map");
    expect([blank?.offsetBytes, blank?.bytes]).toEqual([2 * HORIZON_MAP_BYTES, HORIZON_MAP_BYTES]);
    const words = new Uint32Array(blank?.data.slice().buffer ?? new ArrayBuffer(0));
    expect(words.every((w) => w === NO_HORIZON_WORD)).toBe(true);
    expect(NO_HORIZON_WORD).toBe(0x8001_8001);
    resources.dispose();
  });

  it("refuse a horizon map of another size before any write", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    engine.resetCounts();
    const short = { ...mappedLayers(), horizonFar: new Uint32Array(4) };
    expect(() => resources.upload({ ...upload(4, 65, false), layers: short })).toThrow(RangeError);
    expect(engine.counts.bufferWrites).toBe(0);
    resources.dispose();
  });

  it("forget every slot's horizon map on a restore", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    resources.upload({ ...upload(2, 65, false), layers: mappedLayers() });
    engine.restore();
    expect(resources.holdsHorizon(2)).toBe(false);
    resources.dispose();
  });

  it("make no horizon-map buffer for R05's spike shading", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    expect(resources.field("horizon-map")).toBeNull();
    expect(resources.holdsHorizon(0)).toBe(false);
    resources.dispose();
  });

  it("refuse, writing nothing, a bake without R10's layers into a lit layout", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    engine.resetCounts();
    expect(resources.upload({ ...upload(0, 65, false), layers: null })).toEqual({
      kind: "refused",
      reason: "layers-missing",
    });
    expect(engine.counts.bufferWrites).toBe(0);
    resources.dispose();
  });

  it("grow a lit layout's phase table as rows are added, and keep its rows across a restore", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    for (let k = 0; k < 6; k += 1) {
      resources.phaseRows.rowOf(1, 0.5 + 0.05 * k);
    }
    resources.setLighting({
      lights: [],
      occluders: [],
      secondaries: [],
      elsewhere: lawFor([0.2, 0.2, 0.2], [0.6, 0.6, 0.6], "moon"),
      exposureScale: 1,
    });
    const rows = resources.phaseRows.laws.length;
    expect(rows).toBe(7);
    const grown = engine.textureSpecs.findLast((spec) => spec.name === "terrain phase table");
    expect(grown?.size).toEqual({ width: PHASE_TABLE_SAMPLES, height: 8 });
    engine.restore();
    expect(resources.phaseRows.laws).toHaveLength(rows);
    const remade = engine.textureWritten.findLast((w) => w.texture === "terrain phase table");
    expect(remade?.size).toEqual({ width: PHASE_TABLE_SAMPLES, height: rows });
    resources.dispose();
  });

  it("keep the phase table they hold where a larger one is refused during a loss, and are made again at the restore", async () => {
    const reported = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    resources.viewBuffers(DEFAULT_TERRAIN_VIEW);
    let rebuilt = 0;
    resources.onRebuilt(() => {
      rebuilt += 1;
    });
    engine.loseDevice();
    for (let k = 0; k < 6; k += 1) {
      resources.phaseRows.rowOf(1, 0.5 + 0.05 * k);
    }
    const lighting = {
      lights: [],
      occluders: [],
      secondaries: [],
      elsewhere: lawFor([0.2, 0.2, 0.2], [0.6, 0.6, 0.6], "moon"),
      exposureScale: 1,
    };
    const refused = ((): unknown => {
      try {
        resources.setLighting(lighting);
        return null;
      } catch (error: unknown) {
        return error;
      }
    })();
    engine.restore();
    // No release failed at the restore: the table held was never released twice.
    expect([refused instanceof EngineUnavailable, rebuilt, reported.mock.calls.length]).toEqual([
      true,
      1,
      0,
    ]);
    resources.dispose();
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

  it("write a shadow cascade's casters into the view's buffer for it (R10.T9.b)", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    const records = new InstanceRecords(2);
    records.pushXyz(3, 1, 2, 3, 0, 1);
    records.pushXyz(4, 4, 5, 6, 0, 1);
    resources.writeCasters("primary", 3, records);
    const write = engine.writes.at(-1);
    expect([write?.buffer, write?.offsetBytes, [...(write?.data ?? [])]]).toEqual([
      "terrain casters (primary) 4",
      0,
      [...records.bytes()],
    ]);
    resources.dispose();
  });

  it("refuse, writing nothing, more of a cascade's casters than the slots (R10.T9.b)", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84);
    const slots = resources.layout.slots.slotCount;
    const records = new InstanceRecords(slots + 1);
    for (let n = 0; n <= slots; n += 1) {
      records.pushXyz(n % slots, 0, 0, 0, 0, 1);
    }
    const writes = engine.writes.length;
    expect(() => {
      resources.writeCasters("primary", 0, records);
    }).toThrow(RangeError);
    expect(engine.writes.length).toBe(writes);
    resources.dispose();
  });
});

/** A wireframe bake's layers (R10.T11): no class weights and no horizon map, its survey mask whole. */
function wireLayers(): TerrainLayers {
  return {
    ...mappedLayers(),
    surveyMask: new Uint8Array(65 * 65).fill(255),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
  };
}

/** A patch's place for the graticule with a distinct value in every word. */
const A_PLACE = {
  radiusM: 6_378_137.5,
  latPhaseRad: 0.01,
  lonPhaseRad: -0.02,
  latIndex: 3,
  lonIndex: -7,
  sinLat: 0.5,
  cosLat: Math.sqrt(3) / 2,
  sinLon: -0.6,
  cosLon: 0.8,
};

/** The names of the buffers `engine` makes while `act` runs, in order. */
function createdDuring(engine: CountingRenderEngine, act: () => void): string[] {
  const created: string[] = [];
  const off = engine.onAllocation((event) => {
    if (event.kind === "created") {
      created.push(event.name);
    }
  });
  act();
  off();
  return created;
}

describe("the wireframe's terrain resources (R10.T11)", () => {
  it("lay the wireframe out with no normals atlas, over its own slot layout", () => {
    const layout = terrainLayout(LOW, DEFAULT_LIMITS, "wire");
    expect([layout.shading, layout.atlas, layout.slots]).toEqual([
      "wire",
      null,
      terrainSlotLayout(LOW, "wire"),
    ]);
  });

  it("give no normals atlas for the wireframe's layout", () => {
    expect(() => normalsAtlasOf(terrainLayout(LOW, DEFAULT_LIMITS, "wire"))).toThrow(
      /the wireframe.s terrain layout holds no normals atlas/u,
    );
  });

  it("make no texture, neither a normals atlas nor a phase table", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    resources.viewBuffers(DEFAULT_TERRAIN_VIEW);
    expect([engine.counts.textures, resources.phaseTable, resources.lit]).toEqual([0, null, null]);
    resources.dispose();
  });

  it("hand out no normals texture, having made none", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    expect(() => resources.normals).toThrow(/holds no normals/u);
    resources.dispose();
  });

  it("make no texture again at a restore", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    engine.restore();
    expect([resources.layout.atlas, engine.counts.textures]).toEqual([null, 0]);
    resources.dispose();
  });

  it("hold the heights and survey masks in storage, and no class layers or horizon map", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const { slotCount } = resources.layout.slots;
    expect([
      resources.field("heights")?.bytes,
      resources.field("survey-mask")?.bytes,
      resources.field("offsets"),
      resources.field("class-layers"),
      resources.field("horizon-map"),
    ]).toEqual([slotCount * HEIGHTS_BYTES, slotCount * SURVEY_MASK_BYTES, null, null, null]);
    resources.dispose();
  });

  it("tally the height cache to the heights, survey masks and slot records alone", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    expect(tally.liveBytes("height-cache")).toBe(
      resources.layout.slots.slotCount * (HEIGHTS_BYTES + SURVEY_MASK_BYTES + SLOT_RECORD_BYTES),
    );
    tally.dispose();
    resources.dispose();
  });

  it("draw the lit material's mesh, its index count in each view's indirect arguments", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    resources.viewBuffers("view");
    const args = engine.writes.findLast(
      (w) => w.buffer === "terrain indirect (view)" && w.offsetBytes === 0,
    );
    expect([
      resources.mesh.name,
      [...new Uint32Array(args?.data.slice().buffer ?? new ArrayBuffer(0))],
    ]).toEqual(["terrain lit patch", [LIT_PATCH_INDICES, 0, 0, 0, 0]]);
    resources.dispose();
  });

  it("make each view's phases buffer beside its frame buffers, R05's names for the default view", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const named = createdDuring(engine, () => {
      resources.viewBuffers("view");
      resources.viewBuffers(DEFAULT_TERRAIN_VIEW);
    });
    expect(named).toEqual([
      "terrain instances (view)",
      "terrain contacts (view)",
      "terrain indirect (view)",
      "terrain phases (view)",
      "terrain instances",
      "terrain contacts",
      "terrain indirect",
      "terrain phases",
    ]);
    resources.dispose();
  });

  it("size a view's phases buffer at 48 B a slot, in the other category", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const made: Array<readonly [number, string]> = [];
    engine.onAllocation((event) => {
      if (event.kind === "created" && event.name === "terrain phases (view)") {
        made.push([event.bytes, event.category]);
      }
    });
    const { phases } = resources.viewBuffers("view");
    expect([WIRE_PHASE_RECORD_BYTES, phases?.bytes, made]).toEqual([
      48,
      resources.layout.slots.slotCount * 48,
      [[resources.layout.slots.slotCount * 48, "other"]],
    ]);
    resources.dispose();
  });

  it("make no phases buffer on a lit or spike layout", async () => {
    const engine = await countingRenderEngine();
    const lit = new TerrainResources(engine, LOW, WGS84, "lit");
    const spike = new TerrainResources(engine, LOW, WGS84);
    expect([lit.viewBuffers("view").phases, spike.viewBuffers("view").phases]).toEqual([
      null,
      null,
    ]);
    lit.dispose();
    spike.dispose();
  });

  it("release a view's phases buffer with the view's other buffers", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const before = engine.live;
    resources.viewBuffers("view");
    const destroyed: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    resources.releaseView("view");
    expect([engine.live, destroyed]).toEqual([
      before,
      [
        "terrain instances (view)",
        "terrain contacts (view)",
        "terrain indirect (view)",
        "terrain phases (view)",
      ],
    ]);
    resources.dispose();
  });

  it("release every handle at dispose, each view's phases buffer among them", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    resources.viewBuffers(DEFAULT_TERRAIN_VIEW);
    resources.viewBuffers("view");
    resources.viewBuffers("instrument");
    resources.dispose();
    expect(engine.live).toEqual({ buffers: 0, textures: 0, meshes: 0, materials: 0 });
  });

  it("write a frame's phase records at the start of the view's phases buffer", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    resources.viewBuffers("view");
    const records = new WirePhaseRecords(resources.layout.slots.slotCount);
    records.push(120.5, 270.75, A_PLACE);
    records.push(3.25, -40, { ...A_PLACE, latIndex: 4, cosLon: -0.8 });
    resources.writePhases(records, "view");
    const write = engine.writes.at(-1);
    expect([write?.buffer, write?.offsetBytes, write?.data]).toEqual([
      "terrain phases (view)",
      0,
      records.bytes(),
    ]);
    resources.dispose();
  });

  it("write no phase record in a frame that draws none", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    resources.viewBuffers("view");
    engine.resetCounts();
    resources.writePhases(new WirePhaseRecords(resources.layout.slots.slotCount), "view");
    expect(engine.counts.bufferWrites).toBe(0);
    resources.dispose();
  });

  it("write no phase record on a lit layout", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "lit");
    resources.viewBuffers("view");
    engine.resetCounts();
    const records = new WirePhaseRecords(resources.layout.slots.slotCount);
    records.push(120.5, 270.75, A_PLACE);
    resources.writePhases(records, "view");
    expect(engine.counts.bufferWrites).toBe(0);
    resources.dispose();
  });

  it("refuse more phase records than the slots", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const { slotCount } = resources.layout.slots;
    const records = new WirePhaseRecords(slotCount + 1);
    for (let n = 0; n <= slotCount; n += 1) {
      records.push(n, n, A_PLACE);
    }
    expect(() => {
      resources.writePhases(records, "view");
    }).toThrow(RangeError);
    resources.dispose();
  });

  it("upload a bake's heights, slot record and survey mask, and no normals", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const before = engine.writes.length;
    engine.resetCounts();
    const result = resources.upload({ ...upload(5, 65, false), layers: wireLayers() });
    expect([
      result.kind,
      engine.writes.slice(before).map((w) => [w.buffer, w.offsetBytes, w.bytes]),
      engine.counts.textureWrites,
    ]).toEqual([
      "uploaded",
      [
        ["terrain heights", 5 * HEIGHTS_BYTES, HEIGHTS_BYTES],
        ["terrain slot records", 5 * SLOT_RECORD_BYTES, SLOT_RECORD_BYTES],
        ["terrain survey-mask", 5 * SURVEY_MASK_BYTES, SURVEY_MASK_BYTES],
      ],
      0,
    ]);
    resources.dispose();
  });

  it("pack a slot's survey mask with the code of its survey's cell size", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    const mask = maskOf((x, y) => x + y < 70);
    const bake = { ...upload(2, 65, false), key: { face: 1, level: 4, i: 3, j: 5 } as const };
    resources.upload({ ...bake, layers: { ...wireLayers(), surveyMask: mask, fieldLevel: 8 } });
    const write = engine.writes.findLast((w) => w.buffer === "terrain survey-mask");
    // A patch at level 4 over a field at level 8: cells of 2^(4 + 6 − 8) quads, code 2.
    expect(write?.data).toEqual(packSurveyMask(mask, 2));
    resources.dispose();
  });

  it("keep none of a bake's normals, whatever their resolution", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    engine.resetCounts();
    const result = resources.upload({ ...upload(0, 129, false), layers: wireLayers() });
    expect([result.kind, engine.counts.textureWrites]).toEqual(["uploaded", 0]);
    resources.dispose();
  });

  it("upload the offsets too on the baked-offsets path", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const resources = new TerrainResources(engine, HIGH, WGS84, "wire");
    resources.upload({ ...upload(3, 129, true), layers: wireLayers() });
    const offsets = engine.writes.filter((w) => w.buffer === "terrain offsets");
    expect([resources.layout.vertexPath, offsets.map((w) => [w.offsetBytes, w.bytes])]).toEqual([
      "baked-offsets",
      [[3 * OFFSETS_BYTES, OFFSETS_BYTES]],
    ]);
    resources.dispose();
  });

  it("refuse, writing nothing, a bake without R10's layers", async () => {
    const engine = await countingRenderEngine();
    const resources = new TerrainResources(engine, LOW, WGS84, "wire");
    engine.resetCounts();
    expect([
      resources.upload({ ...upload(0, 65, false), layers: null }),
      engine.counts.bufferWrites,
    ]).toEqual([{ kind: "refused", reason: "layers-missing" }, 0]);
    resources.dispose();
  });

  it("bind every storage field the layout makes in the wireframe's material, on both paths", async () => {
    const engine = await countingRenderEngine();
    const unbound = (["baked-offsets", "face-differences"] as const).flatMap((path) => {
      const spec = terrainWireMaterialSpec(path);
      const resources = new TerrainResources(
        engine,
        { ...LOW, vertexPath: path, cacheBytes: 2 ** 25 },
        WGS84,
        "wire",
      );
      const names = new Set((spec.storageBuffers ?? []).map((buffer) => buffer.name));
      const missing = resources.layout.slots.fields
        .filter((field) => field.storage === "storage-buffer" && field.bytes > 0)
        .filter((field) => !names.has(STORAGE_NAMES[field.name]))
        .map((field) => `${spec.name} ${field.name}`);
      resources.dispose();
      return missing;
    });
    expect(unbound).toEqual([]);
  });
});
