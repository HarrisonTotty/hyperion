/**
 * The patch cache's slot layout: what one cached patch holds on the GPU and how many fit the
 * setting's budget (plan R05, Design note 10).
 *
 * @remarks
 * R10 owns the layout's sizes and formats per setting (R10 Design note 15); R05 built its shape
 * from the start so that nothing was rebuilt when R10 landed. Heights and morph targets (and, from
 * R10, the horizon map, the class layers and the survey mask) are unfiltered data in storage
 * buffers indexed by slot and vertex; normals, which want filtering, live in a 2D atlas (T11.a).
 * The slot count is fixed when the cache is made. R10's tasks sized their fields (T8.b, T10.b,
 * T10.c, T11); `cacheBudget.ts` (R10.T14.a) sums a slot's bytes and sets them beside Design note
 * 15's count model, and R10.T14.b holds the budgets on the hard bound
 * (decision-r10-t14b-constants), re-sized after R10.T4.b.
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
 * Bytes of a patch's class weights (R10.T10.b): 65 × 65 vertices of 8 bytes over the patch's
 * palette, 33,800 B (`TerrainLayers.weights`).
 */
export const CLASS_WEIGHTS_BYTES = VERTICES * 8;

/**
 * Bytes of a patch's class laws (R10.T10.b): its palette's 8 slots as two `vec4f` each, 256 B
 * (`terrainMaterial.ts`' `packClassLaws`).
 */
export const CLASS_LAWS_BYTES = 8 * 2 * 16;

/**
 * Bytes of a patch's class layers (R10.T10.b): its class weights, then its class laws, 34,056 B,
 * one storage buffer for both so that the lit material keeps a storage binding free.
 */
export const CLASS_LAYERS_BYTES = CLASS_WEIGHTS_BYTES + CLASS_LAWS_BYTES;

/** `u32` words a horizon-map texel takes: eight azimuths, two to a word (R10.T8.a). */
export const HORIZON_WORDS_PER_TEXEL = 4;

/** The horizon map's far-field points along a patch's side: every eighth vertex, 9. */
export const HORIZON_FAR_SIDE = 9;

/**
 * Texels of a patch's horizon map: the near field's 65 × 65 vertices, then the far field's 9 × 9
 * points, 4,306 (`hyperion_surface::bake::horizon`'s layout).
 */
export const HORIZON_TEXELS = VERTICES + HORIZON_FAR_SIDE * HORIZON_FAR_SIDE;

/**
 * Bytes of a patch's horizon map (R10.T8.b): {@link HORIZON_TEXELS} of four `u32` words, an
 * `array<vec4u>`, 68,896 B, the near field's 67,600 then the far field's 1,296.
 */
export const HORIZON_MAP_BYTES = HORIZON_TEXELS * HORIZON_WORDS_PER_TEXEL * 4;

/**
 * Bytes of a patch's survey mask as stored (R10.T10.c; Design note 15): its 65 × 65 vertices' mask
 * bytes, four to a `u32`, 4,225 B, then the byte of its survey's cell size
 * ({@link SURVEY_CELL_CODE_BYTE}), in 1,057 words, 4,228 B.
 */
export const SURVEY_MASK_BYTES = Math.ceil((VERTICES + 1) / 4) * 4;

/**
 * The survey mask's byte that states how many quads of the patch a coarse cell of its survey spans
 * along a side (`gpu/resources.ts`' `surveyCellCode`), past the 4,225 vertex bytes: in the last
 * word's spare bytes, which the field's 1,057 words hold anyway.
 */
export const SURVEY_CELL_CODE_BYTE = VERTICES;

/**
 * A field of a slot.
 *
 * @remarks
 * R10 adds its own fields to this union (the class layers, R10.T10.b; the survey mask, R10.T10.c)
 * and sizes `horizon-map` (R10.T8.b); `cacheBudget.ts`' `bytesPerPatch` sums them (R10.T14.a),
 * and R10.T14.b holds the budgets (decision-r10-t14b-constants), re-sized after R10.T4.b.
 */
export type SlotFieldName =
  "heights" | "offsets" | "normals" | "horizon-map" | "class-layers" | "survey-mask";

/**
 * How a terrain's patches are shaded: R05's spike shading, one hand albedo (the test planet); R10's
 * lit material from each patch's class weights and laws (R10.T10.b), which takes their field,
 * `class-layers`; or the wireframe's depth with its contours (R10.T11, a body baked without
 * weights), which takes its heights and its survey mask alone.
 */
export type TerrainShading = "spike" | "lit" | "wire";

/** What one {@link TerrainShading} holds and draws, where the shadings differ. */
export interface TerrainShadingTraits {
  /** Whether its slots hold normals, in the normals atlas: R05's spike shading and R10's lit. */
  readonly normals: boolean;
  /**
   * Whether it is R10's lit material, with its class layers, horizon map, phase table and lights.
   */
  readonly lit: boolean;
  /** Whether its slots hold the survey mask, so that each upload carries its layers (R10.T10.c). */
  readonly surveyMask: boolean;
  /** Whether it draws the lit mesh, with the survey edge's skirt heads and fins (R10.T10.c). */
  readonly litMesh: boolean;
  /** Whether each view holds the wireframe's phase records beside its instances (R10.T11). */
  readonly phases: boolean;
  /**
   * Whether the low setting draws it coarser than the high one, so that it states
   * `TERRAIN: DETAIL LIMITED` there wherever it draws (R05 Design note 26): not the wireframe's,
   * whose τ and presented size are the same on both settings (R10.T11).
   */
  readonly coarserOnLow: boolean;
}

/**
 * Each {@link TerrainShading}'s {@link TerrainShadingTraits}, read wherever the shadings differ, so
 * that a shading added to the union is a compile error until its traits are given.
 *
 * @remarks
 * The wireframe's (R10.T11) draws the lit mesh, whose skirts and fins close its depth at the
 * survey's edge, and holds the survey mask, its heights and its phases, but no normals.
 */
export const TERRAIN_SHADING_TRAITS: Readonly<Record<TerrainShading, TerrainShadingTraits>> = {
  spike: {
    normals: true,
    lit: false,
    surveyMask: false,
    litMesh: false,
    phases: false,
    coarserOnLow: true,
  },
  lit: {
    normals: true,
    lit: true,
    surveyMask: true,
    litMesh: true,
    phases: false,
    coarserOnLow: true,
  },
  wire: {
    normals: false,
    lit: false,
    surveyMask: true,
    litMesh: true,
    phases: true,
    coarserOnLow: false,
  },
};

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
 * `baked-offsets` path, the normals at the setting's resolution, and the horizon map; with R10's
 * lit shading, the horizon map at {@link HORIZON_MAP_BYTES} (R10.T8.b), the class layers, weights
 * and laws (R10.T10.b), and the survey mask at {@link SURVEY_MASK_BYTES} (R10.T10.c), and the
 * horizon map at zero bytes on R05's spike shading, which draws no shadow and no survey, so that
 * R05's test planet and its spike keep their slots. The wireframe's (R10.T11) holds the heights,
 * the offsets on their path and the survey mask alone: it draws depth and its contours from the
 * heights, and no normal, shadow or class (its normals and horizon map at zero bytes).
 */
export function terrainSlotFields(
  terrain: TerrainSettings,
  shading: TerrainShading = "spike",
): ReadonlyArray<SlotField> {
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
  const traits = TERRAIN_SHADING_TRAITS[shading];
  fields.push({
    name: "normals",
    storage: "texture",
    bytes: traits.normals ? normalsBytes : 0,
  });
  fields.push({
    name: "horizon-map",
    storage: "storage-buffer",
    bytes: traits.lit ? HORIZON_MAP_BYTES : 0,
  });
  switch (shading) {
    case "lit":
      fields.push({ name: "class-layers", storage: "storage-buffer", bytes: CLASS_LAYERS_BYTES });
      fields.push({ name: "survey-mask", storage: "storage-buffer", bytes: SURVEY_MASK_BYTES });
      break;
    case "wire":
      fields.push({ name: "survey-mask", storage: "storage-buffer", bytes: SURVEY_MASK_BYTES });
      break;
    case "spike":
      break;
  }
  return fields;
}

/** The slot layout of a terrain setting and shading, over its own cache budget. */
export function terrainSlotLayout(
  terrain: TerrainSettings,
  shading: TerrainShading = "spike",
): SlotLayout {
  return slotLayout(terrainSlotFields(terrain, shading), terrain.cacheBytes);
}
