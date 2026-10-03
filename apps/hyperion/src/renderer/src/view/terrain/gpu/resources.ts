/**
 * The terrain pass's GPU resources (plan R05, R05.T11.a, Design notes 4, 5 and 10): the shared
 * index mesh, the slot layout's storage buffers and normals atlas, the per-slot records, and the
 * per-frame instance, contact and indirect buffers, all made through R01's adapter.
 *
 * @remarks
 * The slot count and every field's bytes come from T8's {@link SlotLayout}, never recomputed here.
 * What the device can hold is read from `engine.capabilities`, the device's own limits
 * (decisions-r06-r07.md item 7): where the `BakedOffsets` offsets at the layout's slot count do not
 * fit one storage binding, the layout falls back to `FaceDifferences` over the same byte budget,
 * and {@link TerrainLayout.fallback} says so, for the results file and the label. After a device
 * loss the engine's restore makes every handle anew, so the layout is derived again from the
 * rebuilt device's limits and every slot's contents are gone; {@link TerrainResources.onRebuilt}
 * tells the owner of the cache to start again.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../../engine/gpuFlags";
import { type GpuCapabilities, MAX_TEXTURE_ARRAY_LAYERS } from "../../engine/platform";
import type { BufferHandle, MeshHandle, RenderEngine, TextureHandle } from "../../engine/types";
import type { TerrainSettings, TerrainVertexPath } from "../../quality/qualitySetting";
import type { SpheroidFigure } from "../../atmosphere/hillaire";
import type { PatchKey } from "../patchKey";
import {
  DOUBLE_NORMALS_PER_SIDE,
  PATCH_VERTICES_PER_SIDE,
  type SlotFieldName,
  type SlotLayout,
  terrainSlotLayout,
  NORMALS_GUTTER_TEXELS,
} from "../slotLayout";
import {
  CONTACTS_HEADER_BYTES,
  CONTACT_RECORD_BYTES,
  type ContactRecords,
  INSTANCE_RECORD_BYTES,
  type InstanceRecords,
  MAX_CONTACTS,
  patchTerms,
  SLOT_RECORD_BYTES,
  writeSlotRecord,
} from "./uniforms";

/** Quads along a patch's side. */
const QUADS = PATCH_VERTICES_PER_SIDE - 1;
/** The grid's vertices, (x, y) at `65 y + x`. */
export const GRID_VERTICES = PATCH_VERTICES_PER_SIDE * PATCH_VERTICES_PER_SIDE;
/** The skirts' vertices: 65 along each of the four edges, after the grid's. */
export const SKIRT_VERTICES = 4 * PATCH_VERTICES_PER_SIDE;
/** Indices of the grid's triangles: 64 × 64 quads of two. */
export const GRID_INDICES = QUADS * QUADS * 6;
/** Indices of the shared mesh: the grid's and the skirts' 4 × 64 quads. */
export const PATCH_INDICES = GRID_INDICES + 4 * QUADS * 6;
/** Bytes of `drawIndexedIndirect`'s arguments: five `u32`. */
export const INDIRECT_ARGS_BYTES = 20;

/** The texture format of the normals atlas: an octahedral pair as two half floats. */
export const NORMALS_FORMAT = "rg16float";

/** Where the normals of each slot sit in the layered atlas. */
export interface NormalsAtlasLayout {
  /** Normal samples along a tile's side: 65 at the mesh's resolution, 129 at twice it. */
  readonly samplesPerSide: number;
  /** Texels along a tile's side as stored, the gutter on both sides included. */
  readonly tileTexels: number;
  /** Tiles across a layer. */
  readonly columns: number;
  /** Tiles down a layer. */
  readonly rows: number;
  /** Array layers, each `widthTexels` × `heightTexels`. */
  readonly layers: number;
  /** Slots a layer holds: slot s is in layer ⌊s ÷ tilesPerLayer⌋. */
  readonly tilesPerLayer: number;
  readonly widthTexels: number;
  readonly heightTexels: number;
}

/** A tile's stored corner, its gutter included, in texels and its layer. */
export interface AtlasTile {
  readonly x: number;
  readonly y: number;
  readonly layer: number;
}

/**
 * The atlas for `slots` tiles of `samplesPerSide` normals with their gutter, within a device whose
 * 2D textures are at most `maxDimensionTexels` on a side and whose arrays hold at most
 * `maxLayers`.
 *
 * @remarks
 * Layers are made only when one layer cannot hold every tile, and the tiles are spread evenly over
 * them, so that the texels beyond the tiles' own bytes (the last row's spare tiles) stay under one
 * row a layer. The high setting's 1,962 slots of 131² fit one layer of 8,122 × 4,192; its
 * `FaceDifferences` fallback, 3,904 slots, takes two layers under WebGPU's default 8,192.
 *
 * @throws Error if the tiles do not fit `maxLayers` layers.
 */
export function normalsAtlasLayout(
  samplesPerSide: number,
  slots: number,
  maxDimensionTexels: number,
  maxLayers: number,
): NormalsAtlasLayout {
  const tileTexels = samplesPerSide + 2 * NORMALS_GUTTER_TEXELS;
  const perSide = Math.floor(maxDimensionTexels / tileTexels);
  const perLayer = perSide * perSide;
  const layers = Math.ceil(slots / perLayer);
  if (perSide < 1 || layers > maxLayers) {
    throw new Error(
      `${slots} normal tiles of ${tileTexels}² texels do not fit ${maxLayers} layers of ${maxDimensionTexels}²`,
    );
  }
  const tilesPerLayer = Math.ceil(slots / layers);
  const columns = Math.min(perSide, tilesPerLayer);
  const rows = Math.ceil(tilesPerLayer / columns);
  return {
    samplesPerSide,
    tileTexels,
    columns,
    rows,
    layers,
    tilesPerLayer,
    widthTexels: columns * tileTexels,
    heightTexels: rows * tileTexels,
  };
}

/** The stored corner of `slot`'s tile in `atlas`. */
export function atlasTile(atlas: NormalsAtlasLayout, slot: number): AtlasTile {
  const layer = Math.floor(slot / atlas.tilesPerLayer);
  const within = slot - layer * atlas.tilesPerLayer;
  const column = within % atlas.columns;
  const row = Math.floor(within / atlas.columns);
  return { x: column * atlas.tileTexels, y: row * atlas.tileTexels, layer };
}

/** The device limits the layout is fitted to: `GpuCapabilities`' own. */
export type TerrainLimits = Pick<
  GpuCapabilities,
  "maxStorageBufferBindingSize" | "maxBufferSize" | "maxTextureDimension2D"
>;

/** A setting's terrain layout on one device. */
export interface TerrainLayout {
  /** The vertex path the device can hold: the setting's, or `face-differences` on a fallback. */
  readonly vertexPath: TerrainVertexPath;
  /**
   * `binding-limit` when the setting asked for `baked-offsets` and the offsets at its slot count
   * exceed the device's storage-binding or buffer limit (decisions-r06-r07.md item 7); else `none`.
   */
  readonly fallback: "none" | "binding-limit";
  /** T8's layout for {@link vertexPath} over the setting's cache budget. */
  readonly slots: SlotLayout;
  readonly atlas: NormalsAtlasLayout;
}

/** The bytes a storage-buffer field takes over every slot. */
function fieldBufferBytes(layout: SlotLayout, bytesPerSlot: number): number {
  return bytesPerSlot * layout.slotCount;
}

/** Whether every storage buffer of `layout` fits one binding and one buffer of the device. */
function storageFits(layout: SlotLayout, limits: TerrainLimits): boolean {
  const limit = Math.min(limits.maxStorageBufferBindingSize, limits.maxBufferSize);
  return layout.fields.every(
    (field) => field.storage !== "storage-buffer" || fieldBufferBytes(layout, field.bytes) <= limit,
  );
}

/**
 * The terrain layout of `terrain` on a device with `limits`.
 *
 * @throws Error if even the `face-differences` layout's buffers or the atlas do not fit the
 * device, which no adapter meeting WebGPU's default limits does at either setting's budget.
 */
export function terrainLayout(terrain: TerrainSettings, limits: TerrainLimits): TerrainLayout {
  let slots = terrainSlotLayout(terrain);
  let vertexPath = terrain.vertexPath;
  let fallback: TerrainLayout["fallback"] = "none";
  if (!storageFits(slots, limits) && vertexPath === "baked-offsets") {
    vertexPath = "face-differences";
    fallback = "binding-limit";
    slots = terrainSlotLayout({ ...terrain, vertexPath });
  }
  if (!storageFits(slots, limits)) {
    throw new Error(
      `the terrain's ${slots.slotCount} slots exceed the device's storage binding of ${limits.maxStorageBufferBindingSize} B`,
    );
  }
  let samplesPerSide: number;
  switch (terrain.normals) {
    case "double":
      samplesPerSide = DOUBLE_NORMALS_PER_SIDE;
      break;
    case "mesh":
      samplesPerSide = PATCH_VERTICES_PER_SIDE;
      break;
  }
  const atlas = normalsAtlasLayout(
    samplesPerSide,
    slots.slotCount,
    limits.maxTextureDimension2D,
    MAX_TEXTURE_ARRAY_LAYERS,
  );
  return { vertexPath, fallback, slots, atlas };
}

/** A patch edge, anticlockwise from y = 0: 0 is y = 0, 1 is x = 64, 2 is y = 64, 3 is x = 0. */
type Edge = 0 | 1 | 2 | 3;

const EDGES: ReadonlyArray<Edge> = [0, 1, 2, 3];

/** The grid vertex (x, y) that is the `k`-th of each edge, taken anticlockwise. */
const EDGE_VERTEX: Readonly<Record<Edge, (k: number) => readonly [number, number]>> = {
  0: (k) => [k, 0],
  1: (k) => [QUADS, k],
  2: (k) => [QUADS - k, QUADS],
  3: (k) => [0, QUADS - k],
};

function edge(e: Edge, k: number): readonly [number, number] {
  return EDGE_VERTEX[e](k);
}

/** Grid vertex (x, y)'s index, the bake's own order. */
function gridIndex(x: number, y: number): number {
  return y * PATCH_VERTICES_PER_SIDE + x;
}

/**
 * The shared mesh's vertices and indices.
 *
 * @remarks
 * The terrain has no vertex data of its own beyond indices into the slot buffers, so the mesh's
 * `position` attribute carries each vertex's grid coordinates and whether it is a skirt's:
 * (x, y, 0) for grid vertex (x, y) at index `65 y + x`, which is the bake's own order
 * (`hyperion_surface::patch`, the `heights` layout), and (x, y, 1) for the skirt vertex under edge
 * vertex (x, y), at {@link GRID_VERTICES} + 65 e + k for the k-th vertex of edge e taken
 * anticlockwise (e 0 is y = 0, 1 is x = 64, 2 is y = 64, 3 is x = 0).
 *
 * Each quad (x, y) is split from its (0, 0) corner to its (1, 1) corner, as the bake's mesh and
 * the collision interpolant are: triangles (x, y), (x + 1, y), (x + 1, y + 1) and (x, y),
 * (x + 1, y + 1), (x, y + 1), anticlockwise seen from outside, since every face's e₁ × e₂ is its
 * outward axis a. A skirt quad under edge vertices p then q (anticlockwise) is (p, p′, q′) and
 * (p, q′, q), anticlockwise seen from outside the patch.
 */
export function patchMeshData(): {
  readonly positions: Float32Array;
  readonly indices: Uint32Array;
} {
  const positions = new Float32Array((GRID_VERTICES + SKIRT_VERTICES) * 3);
  for (let y = 0; y <= QUADS; y += 1) {
    for (let x = 0; x <= QUADS; x += 1) {
      const v = 3 * (y * PATCH_VERTICES_PER_SIDE + x);
      positions[v] = x;
      positions[v + 1] = y;
    }
  }
  for (const e of EDGES) {
    for (let k = 0; k <= QUADS; k += 1) {
      const [x, y] = edge(e, k);
      const v = 3 * (GRID_VERTICES + e * PATCH_VERTICES_PER_SIDE + k);
      positions[v] = x;
      positions[v + 1] = y;
      positions[v + 2] = 1;
    }
  }
  const indices = new Uint32Array(PATCH_INDICES);
  let n = 0;
  for (let y = 0; y < QUADS; y += 1) {
    for (let x = 0; x < QUADS; x += 1) {
      indices.set(
        [
          gridIndex(x, y),
          gridIndex(x + 1, y),
          gridIndex(x + 1, y + 1),
          gridIndex(x, y),
          gridIndex(x + 1, y + 1),
          gridIndex(x, y + 1),
        ],
        n,
      );
      n += 6;
    }
  }
  for (const e of EDGES) {
    for (let k = 0; k < QUADS; k += 1) {
      const [px, py] = edge(e, k);
      const [qx, qy] = edge(e, k + 1);
      const p = gridIndex(px, py);
      const q = gridIndex(qx, qy);
      const pSkirt = GRID_VERTICES + e * PATCH_VERTICES_PER_SIDE + k;
      const qSkirt = pSkirt + 1;
      indices.set([p, pSkirt, qSkirt, p, qSkirt, q], n);
      n += 6;
    }
  }
  return { positions, indices };
}

/** One baked patch for a slot, as the pool hands it over (T10's `BakedPatch` with its slot). */
export interface SlotUpload {
  readonly slot: number;
  readonly key: PatchKey;
  /** Own height and morph target per vertex, interleaved, metres; 65 × 65 × 2. */
  readonly heights: Float32Array;
  /** q₀ and q₁ per vertex, metres, on the `baked-offsets` layout only; else `null`. */
  readonly offsets: Float32Array | null;
  /** Octahedral pairs as half floats, N × N × 2 at the setting's resolution. */
  readonly normals: Float16Array;
  /** h₀, the origin's height above the datum, metres. */
  readonly originHeightM: number;
  /** How far the skirts hang below the edge vertices, metres. */
  readonly skirtDepthM: number;
}

/** The slot fields this plan keeps in storage buffers (R10 adds its own to `SlotFieldName`). */
export type SlotBufferField = Extract<SlotFieldName, "heights" | "offsets">;

/** What {@link TerrainResources.upload} did. */
export type SlotUploadResult =
  | { readonly kind: "uploaded" }
  /** Nothing was written: the bake predates the layout (see {@link TerrainResources.upload}). */
  | { readonly kind: "refused"; readonly reason: "slot-outside-layout" | "vertex-path-mismatch" };

const UPLOADED: SlotUploadResult = { kind: "uploaded" };

/** The buffers and textures of one layout, made together. */
interface Made {
  readonly layout: TerrainLayout;
  /** Each storage-buffer field of the layout with bytes, by its name. */
  readonly fields: ReadonlyMap<SlotFieldName, BufferHandle>;
  readonly slotRecords: BufferHandle;
  readonly normals: TextureHandle;
  readonly mesh: MeshHandle;
  readonly instances: BufferHandle;
  readonly contacts: BufferHandle;
  readonly indirect: BufferHandle;
}

/**
 * The terrain's GPU resources for one setting and figure, made at once and written slot by slot.
 *
 * @remarks
 * Every slot buffer and the atlas are made once at the layout's size and written in place, so a
 * freed slot is overwritten with no new allocation. The slot buffers, records and atlas are the
 * `height-cache` category; the per-frame instance, contact and indirect buffers are `other`.
 */
export class TerrainResources {
  readonly #engine: RenderEngine;
  readonly #terrain: TerrainSettings;
  readonly #figure: SpheroidFigure;
  readonly #rebuilt = new Set<(layout: TerrainLayout) => void>();
  readonly #offRestored: () => void;
  readonly #slotRecord = new ArrayBuffer(SLOT_RECORD_BYTES);
  readonly #slotRecordBytes = new Uint8Array(this.#slotRecord);
  readonly #instanceCount = new Uint32Array(1);
  #made: Made;
  #normalsScratch: Uint16Array;

  constructor(engine: RenderEngine, terrain: TerrainSettings, figure: SpheroidFigure) {
    this.#engine = engine;
    this.#terrain = terrain;
    this.#figure = figure;
    this.#made = this.#make();
    this.#normalsScratch = this.#scratchFor(this.#made.layout);
    this.#offRestored = engine.onRestored(() => {
      this.#made = this.#make();
      this.#normalsScratch = this.#scratchFor(this.#made.layout);
      for (const listener of this.#rebuilt) {
        listener(this.#made.layout);
      }
    });
  }

  /** The layout on the current device. */
  get layout(): TerrainLayout {
    return this.#made.layout;
  }

  /** The shared 65 × 65 mesh with skirts ({@link patchMeshData}). */
  get mesh(): MeshHandle {
    return this.#made.mesh;
  }

  /** The normals atlas, `rg16float`, a 2D texture of {@link NormalsAtlasLayout.layers} layers. */
  get normals(): TextureHandle {
    return this.#made.normals;
  }

  /** The per-slot records ({@link SLOT_RECORD_BYTES} each). */
  get slotRecords(): BufferHandle {
    return this.#made.slotRecords;
  }

  /** The instance records ({@link INSTANCE_RECORD_BYTES} each), one per slot at most. */
  get instances(): BufferHandle {
    return this.#made.instances;
  }

  /** The contacts' header and records. */
  get contacts(): BufferHandle {
    return this.#made.contacts;
  }

  /** `drawIndexedIndirect`'s arguments, for `DrawItem.indirect` at offset 0. */
  get indirect(): BufferHandle {
    return this.#made.indirect;
  }

  /**
   * The storage buffer of a slot field (`heights`, `offsets`), or `null` where the layout has none.
   */
  field(name: SlotBufferField): BufferHandle | null {
    return this.#made.fields.get(name) ?? null;
  }

  /**
   * Calls `listener` with the new layout once the resources are made again after a device loss;
   * every slot is then empty. Returns its unsubscribe.
   */
  onRebuilt(listener: (layout: TerrainLayout) => void): () => void {
    this.#rebuilt.add(listener);
    return () => {
      this.#rebuilt.delete(listener);
    };
  }

  /**
   * Writes a baked patch into its slot: heights, offsets where the layout has them, the slot's
   * record and its normals tile with the gutter filled from the tile's edge.
   *
   * @returns `refused` with nothing written for a bake that no longer fits the layout: one baked
   * before a restore re-derived it ({@link TerrainResources.onRebuilt}), whose slot is past the new
   * count or whose offsets the new vertex path does not take; else `uploaded`.
   * @throws RangeError if an array's length is not the layout's, which no bake of the setting gives.
   */
  upload(patch: SlotUpload): SlotUploadResult {
    const { layout, fields } = this.#made;
    const { slot } = patch;
    if (!Number.isInteger(slot) || slot < 0 || slot >= layout.slots.slotCount) {
      return { kind: "refused", reason: "slot-outside-layout" };
    }
    if (fields.has("offsets") === (patch.offsets === null)) {
      return { kind: "refused", reason: "vertex-path-mismatch" };
    }
    this.#checkLength("heights", patch.heights.byteLength);
    if (patch.offsets !== null) {
      this.#checkLength("offsets", patch.offsets.byteLength);
    }
    const n = layout.atlas.samplesPerSide;
    if (patch.normals.length !== n * n * 2) {
      throw new RangeError(`${patch.normals.length / 2} normals, where a tile holds ${n} × ${n}`);
    }
    this.#writeField("heights", slot, patch.heights);
    if (patch.offsets !== null) {
      this.#writeField("offsets", slot, patch.offsets);
    }
    writeSlotRecord(
      this.#slotRecord,
      0,
      patchTerms(patch.key, this.#figure, patch.originHeightM),
      patch.skirtDepthM,
    );
    this.#engine.writeBuffer(
      this.#made.slotRecords,
      slot * SLOT_RECORD_BYTES,
      this.#slotRecordBytes,
    );
    this.#writeNormals(slot, patch.normals);
    return UPLOADED;
  }

  /**
   * Writes a frame's instances and contacts and sets the indirect draw's instance count.
   *
   * @throws RangeError if `instances` holds more records than the buffer, whose capacity is the
   * slot count.
   */
  writeFrame(instances: InstanceRecords, contacts: ContactRecords): void {
    const made = this.#made;
    if (instances.count > made.layout.slots.slotCount) {
      throw new RangeError(
        `${instances.count} instances exceed the ${made.layout.slots.slotCount} slots`,
      );
    }
    if (instances.count > 0) {
      this.#engine.writeBuffer(made.instances, 0, instances.bytes());
    }
    this.#engine.writeBuffer(made.contacts, 0, contacts.bytes());
    this.#instanceCount[0] = instances.count;
    // `instanceCount` is the second of drawIndexedIndirect's five words.
    this.#engine.writeBuffer(made.indirect, 4, this.#instanceCount);
  }

  /** Stops following restores. The engine frees the handles with itself. */
  dispose(): void {
    this.#offRestored();
    this.#rebuilt.clear();
  }

  /** The bytes a slot of `name` holds. */
  #slotBytes(name: SlotBufferField): number {
    const field = this.#made.layout.slots.fields.find((f) => f.name === name);
    if (field === undefined) {
      throw new RangeError(`the layout has no ${name} field`);
    }
    return field.bytes;
  }

  #checkLength(name: SlotBufferField, bytes: number): void {
    const expected = this.#slotBytes(name);
    if (bytes !== expected) {
      throw new RangeError(`${name} of ${bytes} B, where a slot holds ${expected} B`);
    }
  }

  #writeField(name: SlotBufferField, slot: number, data: Float32Array): void {
    const buffer = this.#made.fields.get(name);
    if (buffer === undefined) {
      throw new RangeError(`the layout has no ${name} buffer`);
    }
    this.#engine.writeBuffer(buffer, slot * this.#slotBytes(name), data);
  }

  #writeNormals(slot: number, normals: Float16Array): void {
    const { atlas } = this.#made.layout;
    const n = atlas.samplesPerSide;
    const source = new Uint16Array(normals.buffer, normals.byteOffset, normals.length);
    const stored = atlas.tileTexels;
    const gutter = NORMALS_GUTTER_TEXELS;
    const out = this.#normalsScratch;
    // Every stored texel takes the nearest sample, so the gutter repeats the tile's edge and a
    // filtered read at the edge never reaches the neighbouring tile.
    for (let y = 0; y < stored; y += 1) {
      const sy = Math.min(n - 1, Math.max(0, y - gutter));
      for (let x = 0; x < stored; x += 1) {
        const sx = Math.min(n - 1, Math.max(0, x - gutter));
        const from = 2 * (sy * n + sx);
        const to = 2 * (y * stored + x);
        out[to] = source[from] ?? 0;
        out[to + 1] = source[from + 1] ?? 0;
      }
    }
    const tile = atlasTile(atlas, slot);
    this.#engine.writeTexture(
      this.#made.normals,
      { x: tile.x, y: tile.y, z: tile.layer },
      { width: stored, height: stored, depthOrArrayLayers: 1 },
      out,
    );
  }

  #scratchFor(layout: TerrainLayout): Uint16Array {
    return new Uint16Array(layout.atlas.tileTexels * layout.atlas.tileTexels * 2);
  }

  #make(): Made {
    const engine = this.#engine;
    const layout = terrainLayout(this.#terrain, engine.capabilities);
    const { slotCount } = layout.slots;
    const fields = new Map<SlotFieldName, BufferHandle>();
    for (const field of layout.slots.fields) {
      if (field.storage === "storage-buffer" && field.bytes > 0) {
        fields.set(
          field.name,
          engine.createBuffer({
            name: `terrain ${field.name}`,
            bytes: fieldBufferBytes(layout.slots, field.bytes),
            usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
            category: "height-cache",
          }),
        );
      }
    }
    const slotRecords = engine.createBuffer({
      name: "terrain slot records",
      bytes: slotCount * SLOT_RECORD_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "height-cache",
    });
    const normals = engine.createTexture({
      name: "terrain normals",
      size: {
        width: layout.atlas.widthTexels,
        height: layout.atlas.heightTexels,
        depthOrArrayLayers: layout.atlas.layers,
      },
      dimension: "2d",
      format: NORMALS_FORMAT,
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "height-cache",
    });
    const { positions, indices } = patchMeshData();
    const mesh = engine.createMesh({
      name: "terrain patch",
      positions,
      indices,
      topology: "triangle-list",
      attributes: {},
    });
    const instances = engine.createBuffer({
      name: "terrain instances",
      bytes: slotCount * INSTANCE_RECORD_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    const contacts = engine.createBuffer({
      name: "terrain contacts",
      bytes: CONTACTS_HEADER_BYTES + MAX_CONTACTS * CONTACT_RECORD_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    const indirect = engine.createBuffer({
      name: "terrain indirect",
      bytes: INDIRECT_ARGS_BYTES,
      usage: BUFFER_USAGE.INDIRECT | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    // indexCount, instanceCount, firstIndex, baseVertex, firstInstance: only the count changes.
    engine.writeBuffer(indirect, 0, new Uint32Array([PATCH_INDICES, 0, 0, 0, 0]));
    return { layout, fields, slotRecords, normals, mesh, instances, contacts, indirect };
  }
}
