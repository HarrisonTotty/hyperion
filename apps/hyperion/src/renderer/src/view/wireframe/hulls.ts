import type { Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import type { HullOutline } from "../scene/hull";

/** One hull edge as a segment: its ends' offsets from the craft, m along the frame's axes. */
export type HullSegment = readonly [Vec3, Vec3];

/**
 * A hull's edges, each once, as segments offset from the craft's reference point along the frame's
 * axes (plan R02, R02.T12.b; Design note 15).
 *
 * @remarks
 * The offsets are the hull's own vertices turned by the craft's attitude, no larger than the hull,
 * so that the draw adds them to the craft's one `f64`-differenced origin (Design note 2): the hull
 * stays exact however far the craft is from its frame's origin.
 */
export function hullEdges(hull: HullOutline, attitude: Quaternion): HullSegment[] {
  const turned = hull.vertices.map((v) => rotate(attitude, v));
  return hull.edges.map(([i, j]) => {
    const a = turned[i];
    const b = turned[j];
    if (a === undefined || b === undefined) {
      throw new Error(`hull ${hull.name} has an edge to a vertex it lacks`);
    }
    return [a, b] as const;
  });
}

/**
 * A hull's faces as triangles offset from the craft, turned by its attitude: the depth-only
 * occluder that hides the hull's hidden lines (Design note 5).
 */
export function hullFaces(
  hull: HullOutline,
  attitude: Quaternion,
): ReadonlyArray<readonly [Vec3, Vec3, Vec3]> {
  const turned = hull.vertices.map((v) => rotate(attitude, v));
  return hull.faces.map(([i, j, k]) => {
    const a = turned[i];
    const b = turned[j];
    const c = turned[k];
    if (a === undefined || b === undefined || c === undefined) {
      throw new Error(`hull ${hull.name} has a face on a vertex it lacks`);
    }
    return [a, b, c] as const;
  });
}
