/**
 * The patch cache's slot layout: what one cached patch holds on the GPU and how many fit the
 * setting's budget (plan R05, Design note 10).
 *
 * @remarks
 * R10 owns the layout's sizes and formats per setting (R10 Design note 15 and T14); R05 builds its
 * shape from the start so that nothing is rebuilt when R10 lands. Heights and morph targets (and,
 * from R10, the horizon map, class weights and survey mask) are unfiltered data in storage buffers
 * indexed by slot and vertex; normals, which want filtering, live in a 2D atlas (T11.a). The slot
 * count is fixed when the cache is made.
 */

import type { TerrainSettings } from "../quality/qualitySetting";

/** Vertices along one side of a patch: 64 quads, 65 vertices (`hyperion_surface::PATCH_QUADS` + 1). */
export const PATCH_VERTICES_PER_SIDE = 65;

/** Normal samples along one side of a patch at twice the mesh's resolution. */
export const DOUBLE_NORMALS_PER_SIDE = 2 * (PATCH_VERTICES_PER_SIDE - 1) + 1;

const VERTICES = PATCH_VERTICES_PER_SIDE * PATCH_VERTICES_PER_SIDE;
const F32_BYTES = 4;
/** An `rg16float` texel: an octahedral normal as two half floats. */
const RG16FLOAT_BYTES = 4;

/** Bytes of a patch's heights and morph targets: 65 × 65 × 2 `f32`, 33,800 B. */
export const HEIGHTS_BYTES = VERTICES * 2 * F32_BYTES;

/** Bytes of a patch's baked offsets, `BakedOffsets` only: q₀ and q₁ as 65 × 65 × 6 `f32`, 101,400 B. */
export const OFFSETS_BYTES = VERTICES * 6 * F32_BYTES;

/** Texels of gutter on each side of a normals tile in the atlas, so that filtering stays inside. */
export const NORMALS_GUTTER_TEXELS = 1;

/** Bytes of an atlas tile of `side` × `side` normals with its gutter on every side. */
function normalsTileBytes(side: number): number {
  const stored = side + 2 * NORMALS_GUTTER_TEXELS;
  return stored * stored * RG16FLOAT_BYTES;
}

/**
 * Bytes of a patch's normals at the mesh's resolution as stored: 65 × 65 `rg16float` in a 67 × 67
 * atlas tile, 17,956 B.
 */
export const MESH_NORMALS_BYTES = normalsTileBytes(PATCH_VERTICES_PER_SIDE);

/**
 * Bytes of a patch's normals at twice the mesh's resolution as stored: 129 × 129 `rg16float` in a
 * 131 × 131 atlas tile, 68,644 B.
 */
export const DOUBLE_NORMALS_BYTES = normalsTileBytes(DOUBLE_NORMALS_PER_SIDE);

/**
 * A field of a slot.
 *
 * @remarks
 * R10 adds its own fields (class weights, the survey mask) to this union and sizes `horizon-map`.
 */
export type SlotFieldName = "heights" | "offsets" | "normals" | "horizon-map";

/** Where a slot field lives on the GPU. */
export type SlotFieldStorage =
  /** A storage buffer indexed by slot and vertex in WGSL, tight and untiled. */
  | "storage-buffer"
  /** A filterable texture atlas, one tile a slot. */
  | "texture";

/** One field of a slot: its name, where it lives and the bytes it takes a slot. */
export interface SlotField {
  readonly name: SlotFieldName;
  readonly storage: SlotFieldStorage;
  /** Bytes a slot as stored, an atlas tile's gutter included. */
  readonly bytes: number;
}

/** The fixed slots of a patch cache: the fields of one slot, their total and how many slots fit. */
export interface SlotLayout {
  readonly fields: ReadonlyArray<SlotField>;
  /** The sum of the fields' bytes. */
  readonly bytesPerSlot: number;
  /** The budget the slot count was derived from, in bytes. */
  readonly budgetBytes: number;
  /** ⌊budget ÷ bytes a slot⌋, at least the six roots. */
  readonly slotCount: number;
}

/** The fewest slots a cache may have: the six faces' roots, the draw's last fallback. */
export const MIN_SLOTS = 6;

/**
 * Builds a layout from its fields and a byte budget.
 *
 * @throws Error if a field's size is negative or not an integer, the slot is empty, or the budget
 * holds fewer than {@link MIN_SLOTS} slots.
 */
export function slotLayout(fields: ReadonlyArray<SlotField>, budgetBytes: number): SlotLayout {
  let bytesPerSlot = 0;
  for (const field of fields) {
    if (!Number.isInteger(field.bytes) || field.bytes < 0) {
      throw new Error(`slot field ${field.name} has ${field.bytes} bytes, not a whole number`);
    }
    bytesPerSlot += field.bytes;
  }
  if (bytesPerSlot <= 0) {
    throw new Error("a slot layout needs at least one field of nonzero size");
  }
  const slotCount = Math.floor(budgetBytes / bytesPerSlot);
  if (!(slotCount >= MIN_SLOTS)) {
    throw new Error(
      `a budget of ${budgetBytes} B holds ${slotCount} slots of ${bytesPerSlot} B, fewer than ${MIN_SLOTS}`,
    );
  }
  return { fields, bytesPerSlot, budgetBytes, slotCount };
}

/**
 * This plan's fields for a terrain setting: heights and morph targets, the offsets on the
 * `baked-offsets` path, the normals at the setting's resolution, and the horizon map at zero bytes
 * until R10 sizes it.
 */
export function terrainSlotFields(terrain: TerrainSettings): ReadonlyArray<SlotField> {
  const fields: SlotField[] = [
    { name: "heights", storage: "storage-buffer", bytes: HEIGHTS_BYTES },
  ];
  switch (terrain.vertexPath) {
    case "baked-offsets":
      fields.push({ name: "offsets", storage: "storage-buffer", bytes: OFFSETS_BYTES });
      break;
    case "face-differences":
      break;
  }
  let normalsBytes: number;
  switch (terrain.normals) {
    case "double":
      normalsBytes = DOUBLE_NORMALS_BYTES;
      break;
    case "mesh":
      normalsBytes = MESH_NORMALS_BYTES;
      break;
  }
  fields.push({ name: "normals", storage: "texture", bytes: normalsBytes });
  fields.push({ name: "horizon-map", storage: "storage-buffer", bytes: 0 });
  return fields;
}

/** The slot layout of a terrain setting, over its own cache budget. */
export function terrainSlotLayout(terrain: TerrainSettings): SlotLayout {
  return slotLayout(terrainSlotFields(terrain), terrain.cacheBytes);
}
