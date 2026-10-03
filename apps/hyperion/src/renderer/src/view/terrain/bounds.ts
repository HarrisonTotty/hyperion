/**
 * The bounding volumes of patches, from the cube-sphere mirror and the level table (plan R05,
 * T7.a, Design note 8).
 */

import type { BodyFixedVec3 } from "./planet";

/** A patch's bounding volume in the body-fixed frame: a sphere and the patch's height range. */
export interface PatchBounds {
  /** The sphere's centre, body-fixed metres. */
  readonly centre: BodyFixedVec3;
  /** The sphere's radius, metres. */
  readonly radiusM: number;
  /** The lowest height the patch can reach above the datum, metres. */
  readonly minHeightM: number;
  /** The highest height the patch can reach above the datum, metres. */
  readonly maxHeightM: number;
}
