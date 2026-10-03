/**
 * The messages between the render thread's height-worker pool and its workers (plan R05, T10,
 * Design note 11).
 *
 * @remarks
 * The workers are our own same-origin module scripts, so a message's data is a structured clone of
 * one of these types, trusted rather than checked. A bake's typed arrays are transferred, not
 * copied; a field is cloned, once a worker, one worker at a time.
 */

import type { TerrainNormals, TerrainVertexPath } from "../../quality/qualitySetting";
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
}

/** How a patch is baked: the setting's vertex path and normal resolution. */
export interface BakeSettings {
  readonly vertexPath: TerrainVertexPath;
  readonly normals: TerrainNormals;
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
  /** Copy the coarse field into the worker's module, and drop it from JavaScript. */
  | { readonly kind: "field"; readonly id: number; readonly bytes: ArrayBuffer };

/** A message from a height worker to the pool. */
export type HeightWorkerReply =
  /** The bake of request `id`. */
  | { readonly kind: "baked"; readonly id: number; readonly bake: BakedPatch }
  /** Request `id` could not be baked. */
  | { readonly kind: "bake-failed"; readonly id: number; readonly message: string }
  /** The field of request `id` is in the worker's module. */
  | { readonly kind: "field-loaded"; readonly id: number };

/** The buffers a worker transfers with a bake, so that none is copied twice. */
export function bakeTransferables(bake: BakedPatch): Transferable[] {
  const buffers: Transferable[] = [bake.heights.buffer, bake.normals.buffer];
  if (bake.offsets !== null) {
    buffers.push(bake.offsets.buffer);
  }
  return buffers;
}
