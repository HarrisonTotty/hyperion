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
 *
 * A lit layout (R10.T10.b, `slotLayout.ts`' `TerrainShading`) adds each slot's class weights and
 * class laws, the frame's lights (`terrainMaterial.ts`' `packTerrainLighting`) and the terrain's
 * phase table, whose rows (`TerrainPhaseRows`) the slots' class laws name: kept across a device
 * restore, so that a row's index never changes, and uploaded again whenever one is added. It also
 * holds each slot's survey mask and draws the lit material's own mesh, whose skirts and fins join
 * the terrain to the reference surface at the survey's edge (R10.T10.c, Design note 12).
 *
 * A wireframe layout (plan R10, R10.T11) holds each slot's heights, offsets on their path and
 * survey mask alone, with no normals atlas, and draws the lit material's mesh, whose skirts and fins
 * join the terrain's depth to the body's occluder at the survey's edge; each of its views has its
 * phases' buffer too (`WirePhaseRecords`), which its contours and graticule read.
 *
 * Each view that draws the terrain has its own frame buffers (plan R10, R10.T16.b): its instance,
 * contact and indirect buffers and, on a lit layout, its lights, made at its first frame and
 * released when it leaves ({@link TerrainResources.releaseView}); {@link DEFAULT_TERRAIN_VIEW}'s go
 * by R05's names and with the resources. {@link TerrainResources.dispose} releases every handle the
 * resources made, so that a pass disposed while the engine lives leaves nothing behind
 * (decision-r10-t16-contracts §1); after a device loss the old handles are released first, which
 * `ResilientEngine` drops, as they went with the lost device.
 */

import { PHASE_TABLE_SAMPLES } from "../../appearance/law";
import { packPhaseFactorRows } from "../../appearance/litBodyProbe";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../../engine/gpuFlags";
import { type GpuCapabilities, MAX_TEXTURE_ARRAY_LAYERS } from "../../engine/platform";
import type {
  BufferHandle,
  MeshHandle,
  MeshSpec,
  RenderEngine,
  TextureHandle,
} from "../../engine/types";
import type { TerrainSettings, TerrainVertexPath } from "../../quality/qualitySetting";
import type { SpheroidFigure } from "../../atmosphere/hillaire";
import type { PatchKey } from "../patchKey";
import {
  CLASS_WEIGHTS_BYTES,
  DOUBLE_NORMALS_PER_SIDE,
  HORIZON_FAR_SIDE,
  HORIZON_MAP_BYTES,
  HORIZON_WORDS_PER_TEXEL,
  PATCH_VERTICES_PER_SIDE,
  type SlotFieldName,
  type SlotLayout,
  SURVEY_CELL_CODE_BYTE,
  SURVEY_MASK_BYTES,
  TERRAIN_SHADING_TRAITS,
  type TerrainShading,
  terrainSlotLayout,
  NORMALS_GUTTER_TEXELS,
} from "../slotLayout";
import {
  classRowLaw,
  PHASE_LAW,
  packClassLaws,
  packTerrainLighting,
  TERRAIN_LIGHT_ROWS,
  type TerrainLighting,
  TerrainPhaseRows,
} from "../terrainMaterial";
import type { TerrainLayers } from "../workers/messages";
import {
  CONTACTS_HEADER_BYTES,
  CONTACT_RECORD_BYTES,
  type ContactRecords,
  INSTANCE_RECORD_BYTES,
  type InstanceRecords,
  MAX_CONTACTS,
  patchTerms,
  SLOT_RECORD_BYTES,
  WIRE_PHASE_RECORD_BYTES,
  type WirePhaseRecords,
  writeSlotRecord,
} from "./uniforms";

/** Quads along a patch's side. */
const QUADS = PATCH_VERTICES_PER_SIDE - 1;
/** The grid's vertices, (x, y) at `65 y + x`. */
export const GRID_VERTICES = PATCH_VERTICES_PER_SIDE * PATCH_VERTICES_PER_SIDE;
/** The skirts' vertices: 65 along each of the four edges, after the grid's. */
export const SKIRT_VERTICES = 4 * PATCH_VERTICES_PER_SIDE;
/** Bytes of a horizon map's far field: 9 × 9 points of four `u32` words. */
const HORIZON_FAR_BYTES = HORIZON_FAR_SIDE * HORIZON_FAR_SIDE * HORIZON_WORDS_PER_TEXEL * 4;
/** Indices of the grid's triangles: 64 × 64 quads of two. */
export const GRID_INDICES = QUADS * QUADS * 6;
/** Indices of the shared mesh: the grid's and the skirts' 4 × 64 quads. */
export const PATCH_INDICES = GRID_INDICES + 4 * QUADS * 6;
/** Bytes of `drawIndexedIndirect`'s arguments: five `u32`. */
export const INDIRECT_ARGS_BYTES = 20;

/**
 * What each vertex of the lit material's mesh is ({@link litPatchMeshData}), carried in its
 * `position` attribute's z and, for the first vertex of each triangle, its draw's flat `kind`
 * (`terrainLit.wgsl`): a grid vertex; the foot of an edge's skirt, at min(h − skirt depth, 0)
 * above the datum, R05's skirt hung to the datum as well; the head of an edge's skirt, at
 * max(h, 0), which rises to the datum from ground below it; and the datum's foot of a fin along a
 * column (x constant) or a row (y constant) inside the patch, at the datum (R10.T10.c, Design
 * note 12).
 */
export const LIT_VERTEX_CODE = {
  grid: 0,
  skirtFoot: 1,
  skirtHead: 2,
  columnFin: 4,
  rowFin: 5,
} as const;

/**
 * The columns (x) and rows (y) inside a patch that carry a fin at the survey edge (R10.T10.c): every
 * sixteenth, so that a coarse cell's edge has its skirt in every patch down to two levels coarser
 * than the coarse field, where a cell spans 16 quads.
 */
export const SURVEY_FIN_LINES: ReadonlyArray<number> = [16, 32, 48];

/** The fins' datum vertices: 65 along each fin line, columns then rows. */
const FIN_VERTICES = 2 * SURVEY_FIN_LINES.length * PATCH_VERTICES_PER_SIDE;

/**
 * Indices of the lit material's mesh: the shared mesh's, the skirts' heads' 4 × 64 quads, and the
 * fins' 6 × 64 quads, each in both windings.
 */
export const LIT_PATCH_INDICES =
  PATCH_INDICES + 4 * QUADS * 6 + 2 * SURVEY_FIN_LINES.length * QUADS * 12;

/**
 * The rows a lit layout's phase table is first made with: row 0 and the three laws at L(0) = 1 and
 * 0, which most bodies' materials share; it doubles as rows are added.
 */
const LIT_TABLE_ROWS = 4;

/**
 * The view whose frame buffers go by R05's names (`terrain instances` and the rest): the descent
 * spike's one view, `TerrainPass.frame`'s (R10.T16.b). The empty string, which no `ViewId` is
 * (`viewId` refuses it), so that it never meets a view of `VIEW`'s; its buffers are made at its
 * first frame, as every other view's are (the batch review's C4).
 */
export const DEFAULT_TERRAIN_VIEW = "";

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
  const refusal = atlasRefusal(samplesPerSide, slots, maxDimensionTexels, maxLayers);
  if (refusal !== null) {
    throw new Error(refusal);
  }
  const tileTexels = samplesPerSide + 2 * NORMALS_GUTTER_TEXELS;
  const perSide = Math.floor(maxDimensionTexels / tileTexels);
  const layers = Math.ceil(slots / (perSide * perSide));
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

/** Why {@link normalsAtlasLayout} cannot place its tiles, or `null` where it can. */
function atlasRefusal(
  samplesPerSide: number,
  slots: number,
  maxDimensionTexels: number,
  maxLayers: number,
): string | null {
  const tileTexels = samplesPerSide + 2 * NORMALS_GUTTER_TEXELS;
  const perSide = Math.floor(maxDimensionTexels / tileTexels);
  const layers = Math.ceil(slots / (perSide * perSide));
  return perSide < 1 || layers > maxLayers
    ? `${slots} normal tiles of ${tileTexels}² texels do not fit ${maxLayers} layers of ${maxDimensionTexels}²`
    : null;
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

/** What every {@link TerrainLayout} holds, whatever its shading. */
export interface TerrainLayoutFields {
  /** The vertex path the device can hold: the setting's, or `face-differences` on a fallback. */
  readonly vertexPath: TerrainVertexPath;
  /**
   * `binding-limit` when the setting asked for `baked-offsets` and the offsets at its slot count
   * exceed the device's storage-binding or buffer limit (decisions-r06-r07.md item 7); else `none`.
   */
  readonly fallback: "none" | "binding-limit";
  /** T8's layout for {@link TerrainLayoutFields.vertexPath} over the setting's cache budget. */
  readonly slots: SlotLayout;
}

/** The layout of R05's spike shading or R10's lit material, whose slots hold normals. */
export interface NormalsTerrainLayout extends TerrainLayoutFields {
  /** R05's spike shading, or R10's lit material with its fields. */
  readonly shading: "spike" | "lit";
  /** Where each slot's normals sit. */
  readonly atlas: NormalsAtlasLayout;
}

/** The wireframe's layout (R10.T11): its heights, offsets and survey mask, and no normals. */
export interface WireTerrainLayout extends TerrainLayoutFields {
  /** The wireframe's depth with its contours. */
  readonly shading: "wire";
  /** No atlas: the wireframe's slots hold no normals. */
  readonly atlas: null;
}

/** A setting's terrain layout on one device, by how its patches are shaded. */
export type TerrainLayout = NormalsTerrainLayout | WireTerrainLayout;

/**
 * A layout's normals atlas, for the callers that draw with it: R05's spike shading and R10's lit
 * material, which never hold the wireframe's layout.
 *
 * @throws Error for the wireframe's layout, which holds no normals (R10.T11).
 */
export function normalsAtlasOf(layout: TerrainLayout): NormalsAtlasLayout {
  if (layout.shading === "wire") {
    throw new Error("the wireframe's terrain layout holds no normals atlas");
  }
  return layout.atlas;
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
export function terrainLayout(
  terrain: TerrainSettings,
  limits: TerrainLimits,
  shading: TerrainShading = "spike",
): TerrainLayout {
  const fitted = fitLayout(terrain, limits, shading);
  if (typeof fitted === "string") {
    throw new Error(fitted);
  }
  return fitted;
}

/**
 * {@link terrainLayout}, or `null` where the device holds no layout of `terrain`, which is then
 * refused (R10.T14.d: the session compiles no material of a layout no pass can be made on).
 */
export function fittedTerrainLayout(
  terrain: TerrainSettings,
  limits: TerrainLimits,
  shading: TerrainShading = "spike",
): TerrainLayout | null {
  const fitted = fitLayout(terrain, limits, shading);
  return typeof fitted === "string" ? null : fitted;
}

/** {@link terrainLayout}'s layout, or why the device refuses it. */
function fitLayout(
  terrain: TerrainSettings,
  limits: TerrainLimits,
  shading: TerrainShading,
): TerrainLayout | string {
  let slots = terrainSlotLayout(terrain, shading);
  let vertexPath = terrain.vertexPath;
  let fallback: TerrainLayout["fallback"] = "none";
  if (!storageFits(slots, limits) && vertexPath === "baked-offsets") {
    vertexPath = "face-differences";
    fallback = "binding-limit";
    slots = terrainSlotLayout({ ...terrain, vertexPath }, shading);
  }
  if (!storageFits(slots, limits)) {
    return `the terrain's ${slots.slotCount} slots exceed the device's storage binding of ${limits.maxStorageBufferBindingSize} B`;
  }
  if (shading === "wire") {
    return { shading, vertexPath, fallback, slots, atlas: null };
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
  const refusal = atlasRefusal(
    samplesPerSide,
    slots.slotCount,
    limits.maxTextureDimension2D,
    MAX_TEXTURE_ARRAY_LAYERS,
  );
  if (refusal !== null) {
    return refusal;
  }
  const atlas = normalsAtlasLayout(
    samplesPerSide,
    slots.slotCount,
    limits.maxTextureDimension2D,
    MAX_TEXTURE_ARRAY_LAYERS,
  );
  return { shading, vertexPath, fallback, slots, atlas };
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

/**
 * The lit material's mesh (R10.T10.c, Design note 12): {@link patchMeshData}'s grid and skirts,
 * with what joins the terrain to the reference surface at the survey's edge.
 *
 * @remarks
 * Each vertex's `position` is (x, y, code), the code one of {@link LIT_VERTEX_CODE}. The vertices
 * are the shared mesh's, its skirts' now each edge skirt's foot, then each edge vertex's skirt head
 * at {@link GRID_VERTICES} + {@link SKIRT_VERTICES} + 65 e + k (edge e's k-th vertex,
 * anticlockwise, as the feet are), then the fins' datum vertices, 65 along each of
 * {@link SURVEY_FIN_LINES}' columns and then its rows, vertex k of a line at y = k (a column) or
 * x = k (a row).
 *
 * The first vertex of every triangle names its part, which the fragment reads flat: the grid's
 * triangles are the shared mesh's; each skirt quad under edge vertices p then q is (p′, q′, p) and
 * (q′, q, p), and its head's quad above them (p″, p, q) and (p″, q, q″), both anticlockwise seen
 * from outside the patch as R05's skirt is; each fin quad, between grid vertices p and q of its line
 * and their datum vertices p′ and q′, is drawn in both windings, (p′, q′, q) and (p′, q, p), then
 * (p′, q, q′) and (p′, p, q), since a fin is seen from the side whose ground lies lower, which the
 * survey decides.
 */
export function litPatchMeshData(): {
  readonly positions: Float32Array;
  readonly indices: Uint32Array;
} {
  const shared = patchMeshData();
  const heads = GRID_VERTICES + SKIRT_VERTICES;
  const fins = heads + SKIRT_VERTICES;
  const positions = new Float32Array((fins + FIN_VERTICES) * 3);
  positions.set(shared.positions);
  for (const e of EDGES) {
    for (let k = 0; k <= QUADS; k += 1) {
      const [x, y] = edge(e, k);
      positions.set(
        [x, y, LIT_VERTEX_CODE.skirtHead],
        3 * (heads + e * PATCH_VERTICES_PER_SIDE + k),
      );
    }
  }
  const finVertex = (axis: 0 | 1, line: number, k: number): number =>
    fins + (axis * SURVEY_FIN_LINES.length + line) * PATCH_VERTICES_PER_SIDE + k;
  SURVEY_FIN_LINES.forEach((c, line) => {
    for (let k = 0; k <= QUADS; k += 1) {
      positions.set([c, k, LIT_VERTEX_CODE.columnFin], 3 * finVertex(0, line, k));
      positions.set([k, c, LIT_VERTEX_CODE.rowFin], 3 * finVertex(1, line, k));
    }
  });
  const indices = new Uint32Array(LIT_PATCH_INDICES);
  indices.set(shared.indices.subarray(0, GRID_INDICES));
  let n = GRID_INDICES;
  for (const e of EDGES) {
    for (let k = 0; k < QUADS; k += 1) {
      const [px, py] = edge(e, k);
      const [qx, qy] = edge(e, k + 1);
      const p = gridIndex(px, py);
      const q = gridIndex(qx, qy);
      const pFoot = GRID_VERTICES + e * PATCH_VERTICES_PER_SIDE + k;
      const pHead = heads + e * PATCH_VERTICES_PER_SIDE + k;
      indices.set([pFoot, pFoot + 1, p, pFoot + 1, q, p], n);
      indices.set([pHead, p, q, pHead, q, pHead + 1], n + 6);
      n += 12;
    }
  }
  for (const axis of [0, 1] as const) {
    SURVEY_FIN_LINES.forEach((c, line) => {
      for (let k = 0; k < QUADS; k += 1) {
        const p = axis === 0 ? gridIndex(c, k) : gridIndex(k, c);
        const q = axis === 0 ? gridIndex(c, k + 1) : gridIndex(k + 1, c);
        const pDatum = finVertex(axis, line, k);
        const qDatum = pDatum + 1;
        indices.set([pDatum, qDatum, q, pDatum, q, p, pDatum, q, qDatum, pDatum, p, q], n);
        n += 12;
      }
    });
  }
  return { positions, indices };
}

/**
 * The mesh a pass of `shading` draws every patch with: R05's shared one ({@link patchMeshData})
 * for the spike's shading, and the lit mesh ({@link litPatchMeshData}) for R10's lit material and
 * the wireframe's (R10.T10.c, R10.T11).
 *
 * @remarks
 * Its vertex layout, one `position` of three floats a vertex in a triangle list, is what the
 * engine keys a material's pipelines by beside the target, so the terrain session compiles its
 * materials against a mesh of this one's layout (plan R10, R10.T14.d; `materialSet.ts`).
 */
export function terrainMeshSpec(shading: TerrainShading): MeshSpec {
  const lit = TERRAIN_SHADING_TRAITS[shading].litMesh;
  const { positions, indices } = lit ? litPatchMeshData() : patchMeshData();
  return {
    name: lit ? "terrain lit patch" : "terrain patch",
    positions,
    indices,
    topology: "triangle-list",
    attributes: {},
  };
}

/**
 * How many quads of a patch of `patchLevel` a coarse cell of a field of `fieldLevel` spans along a
 * side, as the code the survey mask carries ({@link SURVEY_CELL_CODE_BYTE}): log₂ of it, the patch
 * level plus 6 less the field's, clamped to [0, 6] (R10.T10.c).
 *
 * @remarks
 * A coarse cell is a quadtree node of the field's level, so its edges lie on a patch's vertex lines
 * every 2^code quads from the patch's corner, or, at 6, on no line inside the patch (the patch is
 * one cell or inside one). The terrain's discard then reads each fragment's cell's state at the
 * vertex at the cell's centre within the patch, which lies on none of its edges, wherever the code
 * is 1 or more; a vertex on a cell's edge is the bake's mask of either cell, as the rounding of its
 * direction falls (`hyperion_surface::bake`'s `PatchKey::containing`). At 0 a cell spans one quad
 * or less, a patch six or more levels coarser than the field, and the interpolated mask's half
 * stands in (Design note 12's).
 *
 * @throws RangeError for a level that is not an integer.
 */
export function surveyCellCode(patchLevel: number, fieldLevel: number): number {
  if (!Number.isInteger(patchLevel) || !Number.isInteger(fieldLevel)) {
    throw new RangeError(`levels ${String(patchLevel)} and ${String(fieldLevel)} are not integers`);
  }
  return Math.min(6, Math.max(0, patchLevel + 6 - fieldLevel));
}

/**
 * A slot's `survey-mask` field (R10.T10.c; `slotLayout.ts`' {@link SURVEY_MASK_BYTES}): the bake's
 * 65 × 65 mask bytes, vertex (x, y) at byte 65 y + x, four to a little-endian `u32`, then the cell
 * code ({@link surveyCellCode}) at byte {@link SURVEY_CELL_CODE_BYTE}, the rest zero.
 *
 * @throws RangeError for a mask of another size, or a code outside [0, 6].
 */
export function packSurveyMask(mask: Uint8Array, cellCode: number): Uint8Array {
  if (mask.length !== GRID_VERTICES) {
    throw new RangeError(
      `a survey mask of ${String(mask.length)} B, where a patch has ${String(GRID_VERTICES)}`,
    );
  }
  if (!Number.isInteger(cellCode) || cellCode < 0 || cellCode > 6) {
    throw new RangeError(`a survey cell code of ${String(cellCode)}, not 0 to 6`);
  }
  const packed = new Uint8Array(SURVEY_MASK_BYTES);
  packed.set(mask);
  packed[SURVEY_CELL_CODE_BYTE] = cellCode;
  return packed;
}

/**
 * Whether the fragment at (x, y) of a patch, in quads from its corner, lies on surveyed ground by
 * its slot's survey-mask field `packed` ({@link packSurveyMask}): the shader's `survey_surveyed`
 * (`terrainLit.wgsl`), its twin in `f64`.
 *
 * @remarks
 * With a cell code c of 1 or more, the state of the coarse cell holding the point, read at that
 * cell's centre vertex within the patch: 2^c quads a cell, the cell ⌊x ÷ 2^c⌋ clamped into the
 * patch, its centre vertex 2^(c − 1) further on. With 0, the mask interpolated bilinearly between
 * the quad's four vertices, at half or more (Design note 12's rule).
 */
export function surveyedAt(packed: Uint8Array, x: number, y: number): boolean {
  const code = packed[SURVEY_CELL_CODE_BYTE] ?? 0;
  const at = (vx: number, vy: number): number =>
    (packed[vy * PATCH_VERTICES_PER_SIDE + vx] ?? 0) / 255;
  if (code === 0) {
    const bx = Math.min(QUADS - 1, Math.max(0, Math.floor(x)));
    const by = Math.min(QUADS - 1, Math.max(0, Math.floor(y)));
    const fx = Math.min(1, Math.max(0, x - bx));
    const fy = Math.min(1, Math.max(0, y - by));
    const low = at(bx, by) + fx * (at(bx + 1, by) - at(bx, by));
    const high = at(bx, by + 1) + fx * (at(bx + 1, by + 1) - at(bx, by + 1));
    return low + fy * (high - low) >= 0.5;
  }
  const quads = 2 ** code;
  const cells = Math.max(1, QUADS / quads);
  const cell = (f: number): number => Math.min(cells - 1, Math.max(0, Math.floor(f / quads)));
  const centre = (f: number): number => Math.min(QUADS - 1, cell(f) * quads + quads / 2);
  return at(centre(x), centre(y)) >= 0.5;
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
  /** R10's layers of a body's bake, which a lit layout draws; `null` for the test planet's. */
  readonly layers: TerrainLayers | null;
}

/**
 * The slot fields kept in storage buffers: R05's, R10.T10.b's class weights and laws, R10.T8.b's
 * horizon map and R10.T10.c's survey mask.
 */
export type SlotBufferField = Extract<
  SlotFieldName,
  "heights" | "offsets" | "class-layers" | "horizon-map" | "survey-mask"
>;

/**
 * A horizon-map word of two codes of −32,767, −90° each (`hyperion_surface`'s `NO_HORIZON`): no
 * ground ahead, so a slot whose bake has no map shades as though nothing rose about it.
 */
export const NO_HORIZON_WORD = 0x8001_8001;

/** What {@link TerrainResources.upload} did. */
export type SlotUploadResult =
  | { readonly kind: "uploaded" }
  /**
   * Nothing was written: the bake predates the layout, or a lit layout was given a bake without
   * R10's layers (see {@link TerrainResources.upload}).
   */
  | {
      readonly kind: "refused";
      readonly reason: "slot-outside-layout" | "vertex-path-mismatch" | "layers-missing";
    };

const UPLOADED: SlotUploadResult = { kind: "uploaded" };

/** A lit layout's phase table (R10.T10.b), which every view's draw binds. */
interface LitMade {
  /** The phase table, `rgba32float`, `tableRows` rows of `PHASE_TABLE_SAMPLES`. */
  table: TextureHandle;
  tableRows: number;
  /** The rows' version the table holds. */
  tableVersion: number;
}

/** One view's frame buffers (R10.T16.b). */
export interface TerrainViewBuffers {
  /** Its instance records ({@link INSTANCE_RECORD_BYTES} each), one per slot at most. */
  readonly instances: BufferHandle;
  /** Its contacts' header and records. */
  readonly contacts: BufferHandle;
  /** Its `drawIndexedIndirect` arguments, for `DrawItem.indirect` at offset 0. */
  readonly indirect: BufferHandle;
  /** A lit layout's lights for it, `TERRAIN_LIGHT_ROWS` `vec4f`; `null` for R05's spike shading. */
  readonly lights: BufferHandle | null;
  /**
   * A wireframe layout's phase records for it ({@link WIRE_PHASE_RECORD_BYTES} each), one per slot
   * at most (R10.T11); `null` on the other layouts.
   */
  readonly phases: BufferHandle | null;
}

/** The buffers and textures of one layout, made together, whatever its shading. */
interface MadeFields {
  /** The lit layout's phase table; `null` for R05's spike shading and the wireframe's. */
  readonly lit: LitMade | null;
  /** Each storage-buffer field of the layout with bytes, by its name. */
  readonly fields: ReadonlyMap<SlotFieldName, BufferHandle>;
  readonly slotRecords: BufferHandle;
  readonly mesh: MeshHandle;
  /** Each view's frame buffers, made at its first frame on the device. */
  readonly views: Map<string, TerrainViewBuffers>;
}

/** A layout with normals made, with its atlas's texture. */
interface NormalsMade extends MadeFields {
  readonly layout: NormalsTerrainLayout;
  readonly normals: TextureHandle;
}

/** The wireframe's layout made (R10.T11), with no atlas. */
interface WireMade extends MadeFields {
  readonly layout: WireTerrainLayout;
  readonly normals: null;
}

/** One layout's buffers and textures, by whether it holds normals. */
type Made = NormalsMade | WireMade;

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
  readonly #shading: TerrainShading;
  /**
   * The lit layout's phase rows, row 0 the body's uniform law (Lambert's row until the first
   * {@link TerrainResources.setLighting}): kept across restores, so that the slots' rows hold.
   */
  readonly #rows = new TerrainPhaseRows(classRowLaw(PHASE_LAW.lambert, 0));
  readonly #rebuilt = new Set<(layout: TerrainLayout) => void>();
  readonly #offRestored: () => void;
  readonly #slotRecord = new ArrayBuffer(SLOT_RECORD_BYTES);
  readonly #slotRecordBytes = new Uint8Array(this.#slotRecord);
  readonly #instanceCount = new Uint32Array(1);
  #made: Made;
  #normalsScratch: Uint16Array;
  /** Whether each slot holds a bake's horizon map (R10.T8.b), cleared with the slots on a restore. */
  #horizonHeld: Uint8Array;
  /** A slot's horizon map of {@link NO_HORIZON_WORD}s, made once a slot is written without one. */
  #noHorizon: Uint32Array | null = null;
  /** Every handle released ({@link TerrainResources.dispose}): nothing more is made. */
  #disposed = false;

  constructor(
    engine: RenderEngine,
    terrain: TerrainSettings,
    figure: SpheroidFigure,
    shading: TerrainShading = "spike",
  ) {
    this.#engine = engine;
    this.#terrain = terrain;
    this.#figure = figure;
    this.#shading = shading;
    this.#made = this.#make();
    this.#normalsScratch = this.#scratchFor(this.#made.layout);
    this.#horizonHeld = new Uint8Array(this.#made.layout.slots.slotCount);
    this.#offRestored = engine.onRestored(() => {
      // The lost device's handles went with it: `ResilientEngine` drops their release. A release
      // that throws is reported, and the restore goes on to make the resources and tell their
      // owners (the batch review's S3).
      this.#release(this.#made);
      this.#made = this.#make();
      this.#normalsScratch = this.#scratchFor(this.#made.layout);
      this.#horizonHeld = new Uint8Array(this.#made.layout.slots.slotCount);
      for (const listener of this.#rebuilt) {
        listener(this.#made.layout);
      }
    });
  }

  /** The layout on the current device. */
  get layout(): TerrainLayout {
    return this.#made.layout;
  }

  /**
   * The 65 × 65 mesh with skirts the layout draws: R05's shared one ({@link patchMeshData}) on
   * R05's spike shading, and the lit material's ({@link litPatchMeshData}), with the survey edge's
   * skirt heads and fins, on a lit or wireframe layout.
   */
  get mesh(): MeshHandle {
    return this.#made.mesh;
  }

  /**
   * The normals atlas, `rg16float`, a 2D texture of {@link NormalsAtlasLayout.layers} layers.
   *
   * @throws Error on the wireframe's layout, which holds no normals (R10.T11).
   */
  get normals(): TextureHandle {
    const normals = this.#made.normals;
    if (normals === null) {
      throw new Error("the wireframe's terrain layout holds no normals");
    }
    return normals;
  }

  /** The per-slot records ({@link SLOT_RECORD_BYTES} each). */
  get slotRecords(): BufferHandle {
    return this.#made.slotRecords;
  }

  /** {@link DEFAULT_TERRAIN_VIEW}'s instance records ({@link INSTANCE_RECORD_BYTES} each). */
  get instances(): BufferHandle {
    return this.viewBuffers(DEFAULT_TERRAIN_VIEW).instances;
  }

  /** {@link DEFAULT_TERRAIN_VIEW}'s contacts' header and records. */
  get contacts(): BufferHandle {
    return this.viewBuffers(DEFAULT_TERRAIN_VIEW).contacts;
  }

  /** {@link DEFAULT_TERRAIN_VIEW}'s `drawIndexedIndirect` arguments, at offset 0. */
  get indirect(): BufferHandle {
    return this.viewBuffers(DEFAULT_TERRAIN_VIEW).indirect;
  }

  /**
   * `view`'s frame buffers (R10.T16.b), made at its first call on the current device: named after
   * the view, as `terrain instances (view)`, all but {@link DEFAULT_TERRAIN_VIEW}'s, which take
   * R05's names.
   */
  viewBuffers(view: string): TerrainViewBuffers {
    if (this.#disposed) {
      throw new Error("the terrain's resources were disposed: they make nothing more");
    }
    const made = this.#made;
    let buffers = made.views.get(view);
    if (buffers === undefined) {
      buffers = this.#makeView(made.layout, view);
      made.views.set(view, buffers);
    }
    return buffers;
  }

  /**
   * Releases `view`'s frame buffers, once it no longer draws the terrain (R10.T16.b); nothing for
   * a view with none made, or for {@link DEFAULT_TERRAIN_VIEW}, whose go with the resources. A
   * caller never releases a view a frame not yet submitted draws.
   */
  releaseView(view: string): void {
    if (view === DEFAULT_TERRAIN_VIEW) {
      return;
    }
    const buffers = this.#made.views.get(view);
    if (buffers !== undefined) {
      this.#made.views.delete(view);
      this.#releaseView(buffers);
    }
  }

  /**
   * The storage buffer of a slot field (`heights`, `offsets`, and a lit layout's `class-layers`,
   * `horizon-map` and `survey-mask`), or `null` where the layout has none.
   */
  field(name: SlotBufferField): BufferHandle | null {
    return this.#made.fields.get(name) ?? null;
  }

  /**
   * Whether `slot` holds a bake's horizon map, which a child's may blend to (R10.T8.b): `false`
   * on a layout without the field, for a slot not written since the layout was made, and for one
   * whose bake had no map.
   */
  holdsHorizon(slot: number): boolean {
    return this.#horizonHeld[slot] === 1;
  }

  /**
   * A lit layout's lights buffer, {@link DEFAULT_TERRAIN_VIEW}'s, and its phase table, or `null`
   * for R05's spike shading.
   */
  get lit(): { readonly lights: BufferHandle; readonly phaseTable: TextureHandle } | null {
    const lit = this.#made.lit;
    const lights = this.viewBuffers(DEFAULT_TERRAIN_VIEW).lights;
    return lit === null || lights === null ? null : { lights, phaseTable: lit.table };
  }

  /** A lit layout's phase table, which every view's draw binds, or `null` for R05's spike shading. */
  get phaseTable(): TextureHandle | null {
    return this.#made.lit?.table ?? null;
  }

  /** The lit layout's phase rows, row 0 the body's uniform law. */
  get phaseRows(): TerrainPhaseRows {
    return this.#rows;
  }

  /**
   * Writes a frame's lights into `view`'s lights buffer of a lit layout, and the body's uniform law
   * into row 0 of its phase table where it changed; nothing on R05's spike shading.
   */
  setLighting(lighting: TerrainLighting, view: string = DEFAULT_TERRAIN_VIEW): void {
    const lit = this.#made.lit;
    const lights = this.viewBuffers(view).lights;
    if (lit === null || lights === null) {
      return;
    }
    this.#rows.setElsewhere(lighting.elsewhere);
    this.#writeTable(lit);
    const { equatorialRadiusM: a, polarRadiusM: c } = this.#figure;
    this.#engine.writeBuffer(lights, 0, packTerrainLighting(lighting, c / a));
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
   * On a lit layout it also writes the slot's class layers, its class weights (zeros where the
   * bake skipped them) then its class laws, adding their rows to the phase table, its horizon
   * map, the near field's words then the far field's (R10.T8.b), or words of no horizon where the
   * bake skipped it, and its survey mask with the code of its survey's cell size
   * ({@link packSurveyMask}, R10.T10.c). On the wireframe's layout (R10.T11) it writes the heights,
   * the offsets on their path, the record and the survey mask alone: the bake's normals are not
   * kept.
   *
   * @returns `refused` with nothing written for a bake that no longer fits the layout: one baked
   * before a restore re-derived it ({@link TerrainResources.onRebuilt}), whose slot is past the new
   * count or whose offsets the new vertex path does not take; or, on a lit or wireframe layout, a
   * bake without R10's layers; else `uploaded`.
   * @throws RangeError also for class weights, a horizon map or a survey mask of another size than
   * a slot's, or a field level that is not an integer, before any write.
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
    const { layers } = patch;
    const masked = TERRAIN_SHADING_TRAITS[layout.shading].surveyMask;
    if (masked && layers === null) {
      return { kind: "refused", reason: "layers-missing" };
    }
    const weightsBytes = layers?.weights.length ?? 0;
    if (this.#made.lit !== null && weightsBytes !== 0 && weightsBytes !== CLASS_WEIGHTS_BYTES) {
      throw new RangeError(
        `class weights of ${String(weightsBytes)} B, where a slot holds ${String(CLASS_WEIGHTS_BYTES)} B`,
      );
    }
    const horizonBytes =
      (layers?.horizonNear.byteLength ?? 0) + (layers?.horizonFar.byteLength ?? 0);
    if (
      this.#made.lit !== null &&
      horizonBytes !== 0 &&
      (horizonBytes !== HORIZON_MAP_BYTES || layers?.horizonFar.byteLength !== HORIZON_FAR_BYTES)
    ) {
      throw new RangeError(
        `a horizon map of ${String(horizonBytes)} B, where a slot holds ${String(HORIZON_MAP_BYTES)} B`,
      );
    }
    const surveyMask =
      masked && layers !== null
        ? packSurveyMask(layers.surveyMask, surveyCellCode(patch.key.level, layers.fieldLevel))
        : null;
    this.#checkLength("heights", patch.heights.byteLength);
    if (patch.offsets !== null) {
      this.#checkLength("offsets", patch.offsets.byteLength);
    }
    const { atlas } = layout;
    if (atlas !== null) {
      const n = atlas.samplesPerSide;
      if (patch.normals.length !== n * n * 2) {
        throw new RangeError(`${patch.normals.length / 2} normals, where a tile holds ${n} × ${n}`);
      }
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
    const made = this.#made;
    if (made.normals !== null) {
      this.#writeNormals(slot, made, patch.normals);
    }
    const lit = this.#made.lit;
    if (lit !== null && layers !== null) {
      const { weights } = layers;
      // The slot's class layers: its weights (zeros where the bake skipped them), then its laws.
      this.#writeField(
        "class-layers",
        slot,
        weights.length === 0 ? new Uint8Array(CLASS_WEIGHTS_BYTES) : weights,
      );
      const { equatorialRadiusM: a, polarRadiusM: c } = this.#figure;
      this.#writeField(
        "class-layers",
        slot,
        packClassLaws(layers.photometry, this.#rows, c / a),
        CLASS_WEIGHTS_BYTES,
      );
      this.#writeTable(lit);
      this.#writeHorizon(slot, layers);
    }
    if (surveyMask !== null) {
      this.#writeField("survey-mask", slot, surveyMask);
    }
    return UPLOADED;
  }

  /**
   * Writes a frame's instances and contacts into `view`'s buffers and sets its indirect draw's
   * instance count.
   *
   * @throws RangeError if `instances` holds more records than the buffer, whose capacity is the
   * slot count.
   */
  writeFrame(
    instances: InstanceRecords,
    contacts: ContactRecords,
    view: string = DEFAULT_TERRAIN_VIEW,
  ): void {
    const made = this.#made;
    if (instances.count > made.layout.slots.slotCount) {
      throw new RangeError(
        `${instances.count} instances exceed the ${made.layout.slots.slotCount} slots`,
      );
    }
    const buffers = this.viewBuffers(view);
    if (instances.count > 0) {
      this.#engine.writeBuffer(buffers.instances, 0, instances.bytes());
    }
    this.#engine.writeBuffer(buffers.contacts, 0, contacts.bytes());
    this.#instanceCount[0] = instances.count;
    // `instanceCount` is the second of drawIndexedIndirect's five words.
    this.#engine.writeBuffer(buffers.indirect, 4, this.#instanceCount);
  }

  /**
   * Writes a frame's phase records into `view`'s phases buffer, one a drawn patch in its instances'
   * order (R10.T11); nothing on a layout other than the wireframe's, or with none this frame.
   *
   * @throws RangeError if `phases` holds more records than the buffer, whose capacity is the slot
   * count.
   */
  writePhases(phases: WirePhaseRecords, view: string = DEFAULT_TERRAIN_VIEW): void {
    const made = this.#made;
    if (phases.count > made.layout.slots.slotCount) {
      throw new RangeError(
        `${phases.count} phase records exceed the ${made.layout.slots.slotCount} slots`,
      );
    }
    const buffer = this.viewBuffers(view).phases;
    if (buffer !== null && phases.count > 0) {
      this.#engine.writeBuffer(buffer, 0, phases.bytes());
    }
  }

  /**
   * Releases every handle the resources made on the current device, every view's buffers with
   * them, and stops following restores (decision-r10-t16-contracts §1). A caller never disposes of
   * resources a frame not yet submitted draws.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    this.#rebuilt.clear();
    this.#release(this.#made);
  }

  /**
   * Releases every handle of `made`: the slot buffers, records and atlas, the mesh, the table and
   * the views'. Each release is tried whatever the one before did; one that throws, a bug (a handle
   * released twice), is reported, so that a restore always goes on to make the resources again
   * and a disposal releases the rest (the batch review's S3).
   */
  #release(made: Made): void {
    const { lit, normals } = made;
    const releases: Array<() => void> = [
      ...[...made.fields.values()].map((buffer) => () => {
        this.#engine.releaseBuffer(buffer);
      }),
      () => {
        this.#engine.releaseBuffer(made.slotRecords);
      },
      ...(normals === null
        ? []
        : [
            (): void => {
              this.#engine.releaseTexture(normals);
            },
          ]),
      () => {
        this.#engine.releaseMesh(made.mesh);
      },
      ...(lit === null
        ? []
        : [
            (): void => {
              this.#engine.releaseTexture(lit.table);
            },
          ]),
      ...[...made.views.values()].map((buffers) => () => {
        this.#releaseView(buffers);
      }),
    ];
    made.views.clear();
    for (const release of releases) {
      try {
        release();
      } catch (error: unknown) {
        console.error("a terrain resource could not be released:", error);
      }
    }
  }

  #releaseView(buffers: TerrainViewBuffers): void {
    this.#engine.releaseBuffer(buffers.instances);
    this.#engine.releaseBuffer(buffers.contacts);
    this.#engine.releaseBuffer(buffers.indirect);
    if (buffers.lights !== null) {
      this.#engine.releaseBuffer(buffers.lights);
    }
    if (buffers.phases !== null) {
      this.#engine.releaseBuffer(buffers.phases);
    }
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

  /** Writes `data` into `slot`'s part of field `name`, `withinBytes` into the slot. */
  #writeField(name: SlotBufferField, slot: number, data: ArrayBufferView, withinBytes = 0): void {
    const buffer = this.#made.fields.get(name);
    if (buffer === undefined) {
      throw new RangeError(`the layout has no ${name} buffer`);
    }
    this.#engine.writeBuffer(buffer, slot * this.#slotBytes(name) + withinBytes, data);
  }

  /**
   * Writes `slot`'s horizon map from `layers`, the near field's words then the far field's, or
   * words of no horizon where the bake skipped it, and notes whether the slot holds one.
   */
  #writeHorizon(slot: number, layers: TerrainLayers): void {
    const { horizonNear, horizonFar } = layers;
    if (horizonNear.length === 0) {
      this.#noHorizon ??= new Uint32Array(HORIZON_MAP_BYTES / 4).fill(NO_HORIZON_WORD);
      this.#writeField("horizon-map", slot, this.#noHorizon);
      this.#horizonHeld[slot] = 0;
      return;
    }
    this.#writeField("horizon-map", slot, horizonNear);
    this.#writeField("horizon-map", slot, horizonFar, horizonNear.byteLength);
    this.#horizonHeld[slot] = 1;
  }

  #writeNormals(slot: number, made: NormalsMade, normals: Float16Array): void {
    const { atlas } = made.layout;
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
      made.normals,
      { x: tile.x, y: tile.y, z: tile.layer },
      { width: stored, height: stored, depthOrArrayLayers: 1 },
      out,
    );
  }

  /** Uploads the phase table if its rows changed, in a larger texture where they outgrew it. */
  #writeTable(lit: LitMade): void {
    const rows = this.#rows;
    if (lit.tableVersion === rows.version) {
      return;
    }
    const laws = rows.laws;
    if (laws.length > lit.tableRows) {
      // The larger table first: one refused during a device loss (`EngineUnavailable`) leaves the
      // table held, never a released handle (the batch review's S3).
      const tableRows = Math.max(2 * lit.tableRows, laws.length);
      const table = this.#table(tableRows);
      this.#engine.releaseTexture(lit.table);
      lit.table = table;
      lit.tableRows = tableRows;
    }
    this.#engine.writeTexture(
      lit.table,
      { x: 0, y: 0 },
      { width: PHASE_TABLE_SAMPLES, height: laws.length },
      packPhaseFactorRows(laws),
    );
    lit.tableVersion = rows.version;
  }

  #table(rows: number): TextureHandle {
    return this.#engine.createTexture({
      name: "terrain phase table",
      size: { width: PHASE_TABLE_SAMPLES, height: rows },
      dimension: "2d",
      format: "rgba32float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "other",
    });
  }

  /** A lit layout's phase table, holding the rows kept. */
  #makeLit(): LitMade {
    const tableRows = Math.max(LIT_TABLE_ROWS, this.#rows.laws.length);
    const lit: LitMade = { table: this.#table(tableRows), tableRows, tableVersion: -1 };
    this.#writeTable(lit);
    return lit;
  }

  /**
   * `view`'s frame buffers on `layout`: its instances, contacts and indirect arguments, on a lit
   * layout its lights, with no star until its first frame's, and on the wireframe's its phases.
   */
  #makeView(layout: TerrainLayout, view: string): TerrainViewBuffers {
    const engine = this.#engine;
    const named = (name: string): string =>
      view === DEFAULT_TERRAIN_VIEW ? `terrain ${name}` : `terrain ${name} (${view})`;
    const instances = engine.createBuffer({
      name: named("instances"),
      bytes: layout.slots.slotCount * INSTANCE_RECORD_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    const contacts = engine.createBuffer({
      name: named("contacts"),
      bytes: CONTACTS_HEADER_BYTES + MAX_CONTACTS * CONTACT_RECORD_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    const indirect = engine.createBuffer({
      name: named("indirect"),
      bytes: INDIRECT_ARGS_BYTES,
      usage: BUFFER_USAGE.INDIRECT | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    const traits = TERRAIN_SHADING_TRAITS[layout.shading];
    // indexCount, instanceCount, firstIndex, baseVertex, firstInstance: only the count changes.
    engine.writeBuffer(
      indirect,
      0,
      new Uint32Array([traits.litMesh ? LIT_PATCH_INDICES : PATCH_INDICES, 0, 0, 0, 0]),
    );
    let lights: BufferHandle | null = null;
    if (traits.lit) {
      lights = engine.createBuffer({
        name: named("lights"),
        bytes: TERRAIN_LIGHT_ROWS * 16,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "other",
      });
      // No star until the view's first frame's lights: nothing is lit.
      engine.writeBuffer(lights, 0, new Float32Array(TERRAIN_LIGHT_ROWS * 4));
    }
    const phases = traits.phases
      ? engine.createBuffer({
          name: named("phases"),
          bytes: layout.slots.slotCount * WIRE_PHASE_RECORD_BYTES,
          usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
          category: "other",
        })
      : null;
    return { instances, contacts, indirect, lights, phases };
  }

  #scratchFor(layout: TerrainLayout): Uint16Array {
    const tile = layout.shading === "wire" ? 0 : layout.atlas.tileTexels;
    return new Uint16Array(tile * tile * 2);
  }

  #make(): Made {
    const engine = this.#engine;
    const layout = terrainLayout(this.#terrain, engine.capabilities, this.#shading);
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
    const traits = TERRAIN_SHADING_TRAITS[layout.shading];
    const rest = (): MadeFields => {
      const mesh = engine.createMesh(terrainMeshSpec(layout.shading));
      // Every view's buffers, the default view's included, are made at its first frame.
      const views = new Map<string, TerrainViewBuffers>();
      return { lit: traits.lit ? this.#makeLit() : null, fields, slotRecords, mesh, views };
    };
    if (layout.shading === "wire") {
      return { ...rest(), layout, normals: null };
    }
    const { atlas } = layout;
    const normals = engine.createTexture({
      name: "terrain normals",
      size: {
        width: atlas.widthTexels,
        height: atlas.heightTexels,
        depthOrArrayLayers: atlas.layers,
      },
      dimension: "2d",
      format: NORMALS_FORMAT,
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "height-cache",
    });
    return { ...rest(), layout, normals };
  }
}
