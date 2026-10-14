/**
 * The messages between the render thread's height-worker pool and its workers (plan R05, T10,
 * Design note 11; plan R10, T7, Design note 16).
 *
 * @remarks
 * The workers are our own same-origin module scripts, so a message's data is a structured clone of
 * one of these types, trusted rather than checked. A bake's typed arrays are transferred, not
 * copied. A body's coarse field travels as R09's payload blocks, each at most 1 MiB: its first
 * block in a `field-init`, every later one, a later payload's block 0 included, in a
 * `field-chunk`, each block cloned to every worker, and `field-release` drops the body. Messages
 * to one worker arrive in the order sent, so a bake posted after a block is made with it.
 *
 * A bake names its height source (R10.T6.b): the test planet with its ridges, which reads no field,
 * or a body, whose patches the worker bakes over the field it holds of it with the body's detail
 * seed. A worker answers each block of a payload with what it changed of the survey and the body's
 * level table after it, and a body's bake with R10's layers.
 */

import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";

import type { TerrainNormals, TerrainVertexPath } from "../../quality/terrainKinds";
import type { PatchKey } from "../patchKey";
import type { BodyFixedVec3 } from "../planet";

/** A baked patch as the worker hands it to the render thread (Design note 5). */
export interface BakedPatch {
  readonly key: PatchKey;
  /** The generation of the request it answers. */
  readonly generation: number;
  /** The patch origin, body-fixed metres. */
  readonly originM: BodyFixedVec3;
  /** Own-level height and morph target per vertex, interleaved, metres above the datum. */
  readonly heights: Float32Array;
  /** On `baked-offsets` only: q₀ and q₁ per vertex from the origin, metres; else `null`. */
  readonly offsets: Float32Array | null;
  /** Octahedral normals in the body-fixed frame, packed by the worker into half floats. */
  readonly normals: Float16Array;
  /** The baked heights' range above the datum, metres. */
  readonly heightRangeM: readonly [number, number];
  /** The bounding radius about the origin, metres. */
  readonly boundingRadiusM: number;
  /**
   * The origin's own height above the datum, metres: h₀ of the `face-differences` path
   * (`PatchTerms`' h₀ in `hyperion_surface::patch::vertex`).
   */
  readonly originHeightM: number;
  /** How far the skirts hang below the edge vertices along the datum's normal, metres. */
  readonly skirtDepthM: number;
  /** R10's layers, for a body's patch; `null` for the test planet's, which has none. */
  readonly layers: TerrainLayers | null;
}

/** The palette byte of no entry, and of an empty slot's class and entry (`src/wasm.rs`). */
export const PALETTE_NONE = 255;

/**
 * R10's layers of a body's baked patch (R10.T6.a's `TerrainLayers`, as the worker hands them over;
 * `src/wasm.rs`'s `BakedTerrain`).
 */
export interface TerrainLayers {
  /**
   * The patch's palette, 8 slots of 2 bytes: each its material class's code then its palette
   * entry, {@link PALETTE_NONE} for no entry, and both {@link PALETTE_NONE} for an empty slot.
   */
  readonly palette: Uint8Array;
  /**
   * Each vertex's 8 weight bytes over the palette's slots, vertex (x, y) at `8 (65 y + x)`, summing
   * to 255 where it has weights; empty when the weights were skipped (the wireframe's).
   */
  readonly weights: Uint8Array;
  /** Each vertex's survey mask, 65 × 65: 255 surveyed, 0 not (Design note 12). */
  readonly surveyMask: Uint8Array;
  /** The patch's hard bound, metres. */
  readonly boundM: number;
  /** The patch's σ, the largest omitted RMS over its surveyed cells, metres. */
  readonly sigmaM: number;
  /**
   * Whether every vertex was answered by the field: `false` where some vertex outside the survey
   * took the reference surface or has no weights, which a later block may change.
   */
  readonly complete: boolean;
  /**
   * The index of each cell of the field's level holding a vertex outside the survey, rising, each
   * once: a block that surveys one of them changes the mask (R10's rebake rule, `rebake.ts`).
   */
  readonly unsurveyedCells: Uint32Array;
}

/**
 * Whether the provisional test planet's octaves 8 to 12 are ridged (R05 Design note 12): the spike
 * runs with and without them, since sharp crests are the worst case for pops. R09's surface
 * generator replaces the test planet, and this with it.
 */
export type TestPlanetRidges = "off" | "on";

/** Whether a body's bake makes R10's class weights: the lit style's do, the wireframe's do not. */
export type WeightsLayerSetting = "baked" | "skipped";

/**
 * What a patch's heights come from (R10.T6.b): R05's provisional test planet, with or without its
 * ridges, which the descent spike keeps; or a body, over the field the workers hold of it, with
 * its detail seed (`BodyHooksDto.detail_seed`, R09.T1.b) and whether its weights are baked.
 */
export type HeightSourceSetting =
  | { readonly kind: "test-planet"; readonly ridges: TestPlanetRidges }
  | {
      readonly kind: "body";
      readonly body: BodyIdHex;
      readonly detailSeed: DetailSeedHex;
      readonly weights: WeightsLayerSetting;
    };

/** How a patch is baked: the setting's vertex path and normal resolution, and its height source. */
export interface BakeSettings {
  readonly vertexPath: TerrainVertexPath;
  readonly normals: TerrainNormals;
  readonly source: HeightSourceSetting;
}

/**
 * What a `field-init` starts: R09's payload, whose blocks the worker decodes into the body's field,
 * or a stand-in, blocks of no payload that the worker holds undecoded, as the descent spike's
 * synthetic field stands in for a field's memory on the test planet, which reads none (R10.T7).
 */
export type FieldForm = "payload" | "stand-in";

/** What a block of a payload changed in a worker's field, and the body's level table after it. */
export interface FieldBlockAnswer {
  /** The field's level, 5 to 8, at which the indices of {@link newlySurveyed} are. */
  readonly fieldLevel: number;
  /**
   * The cells the block surveyed that the field did not hold as surveyed, as runs: each the first
   * cell index and the index past the last, flat, rising and disjoint.
   */
  readonly newlySurveyed: Uint32Array;
  /**
   * The body's level table, 25 × 4 as the test planet's `levelTable` (bound, lowest and highest
   * height, largest vertex spacing, metres), as the field now holds it (`worker::level_table`).
   */
  readonly levels: Float64Array;
}

/** A message from the pool to a height worker. */
export type HeightWorkerRequest =
  /** Bake a patch; the answer carries the same `id`. */
  | {
      readonly kind: "bake";
      readonly id: number;
      readonly key: PatchKey;
      readonly generation: number;
      readonly settings: BakeSettings;
    }
  /**
   * Start `body`'s field from its first block, R09's block 0, which carries the whole header,
   * replacing any field of `body` the worker holds; or, for a stand-in, hold the blocks undecoded.
   */
  | {
      readonly kind: "field-init";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly block: ArrayBuffer;
      readonly form: FieldForm;
    }
  /** Add one more block to `body`'s field, which a `field-init` must have started. */
  | {
      readonly kind: "field-chunk";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly block: ArrayBuffer;
    }
  /** Drop `body`'s field; one not held is no error. */
  | { readonly kind: "field-release"; readonly id: number; readonly body: BodyIdHex };

/** A message from a height worker to the pool. */
export type HeightWorkerReply =
  /** The bake of request `id`. */
  | { readonly kind: "baked"; readonly id: number; readonly bake: BakedPatch }
  /** Request `id` could not be baked. */
  | { readonly kind: "bake-failed"; readonly id: number; readonly message: string }
  /**
   * Request `id`, a body's patch, cannot be baked yet: the worker holds no field of the body, or a
   * surveyed vertex reads a cell still to come, a survey's margin in a later block (R10.T6.b). The
   * pool asks again after the body's next block.
   */
  | { readonly kind: "bake-unsurveyed"; readonly id: number; readonly message: string }
  /**
   * Field message `id`, of `body`, is done: `heldBytes` is what the worker now holds of every
   * body's field, its coarse-field copy (R12 Design note 6), and `block` what a payload's block
   * changed, `null` for a release or a stand-in.
   */
  | {
      readonly kind: "field-held";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly heldBytes: number;
      readonly block: FieldBlockAnswer | null;
    }
  /**
   * Field message `id` was refused: a block of a body with no `field-init` before it, or one the
   * worker cannot read. The worker's field of `body` is as it was before the message.
   */
  | {
      readonly kind: "field-refused";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly message: string;
      readonly heldBytes: number;
    };

/** The buffers a worker transfers with a bake, so that none is copied twice. */
export function bakeTransferables(bake: BakedPatch): Transferable[] {
  const buffers: Transferable[] = [bake.heights.buffer, bake.normals.buffer];
  if (bake.offsets !== null) {
    buffers.push(bake.offsets.buffer);
  }
  if (bake.layers !== null) {
    const { palette, weights, surveyMask, unsurveyedCells } = bake.layers;
    buffers.push(palette.buffer, weights.buffer, surveyMask.buffer, unsurveyedCells.buffer);
  }
  return buffers;
}

/** The buffers a worker transfers with a block's answer. */
export function blockTransferables(block: FieldBlockAnswer | null): Transferable[] {
  return block === null ? [] : [block.newlySurveyed.buffer, block.levels.buffer];
}
