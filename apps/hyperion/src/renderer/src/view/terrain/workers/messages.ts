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
 */

import type { BodyIdHex } from "@hyperion/protocol";

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
}

/**
 * Whether the provisional test planet's octaves 8 to 12 are ridged (R05 Design note 12): the spike
 * runs with and without them, since sharp crests are the worst case for pops. R09's surface
 * generator replaces the test planet, and this with it.
 */
export type TestPlanetRidges = "off" | "on";

/** How a patch is baked: the setting's vertex path and normal resolution, and the test planet's ridges. */
export interface BakeSettings {
  readonly vertexPath: TerrainVertexPath;
  readonly normals: TerrainNormals;
  readonly ridges: TestPlanetRidges;
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
   * replacing any field of `body` the worker holds.
   */
  | {
      readonly kind: "field-init";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly block: ArrayBuffer;
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
   * Field message `id` is done: `heldBytes` is what the worker now holds of every body's field,
   * its coarse-field copy (R12 Design note 6).
   */
  | { readonly kind: "field-held"; readonly id: number; readonly heldBytes: number }
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
  return buffers;
}
