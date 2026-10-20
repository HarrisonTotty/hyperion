import { add, cross, dot, norm, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import {
  NEAR_PLANE_M,
  project,
  type ProjectionCamera,
  toViewAxes,
  type Viewport,
} from "../camera/projection";
import { rotate, rotationRows } from "../camera/quaternion";
import type { HullOutline } from "../scene/hull";
import type { HullOnView, ScreenPx, ScreenSegment } from "./symbology";

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

/** A hull's bounding radius, m: its farthest vertex from its reference point (`TEST HULL`'s 11.06 m). */
export function hullBoundingRadiusM(hull: HullOutline): number {
  return Math.max(0, ...hull.vertices.map((v) => norm(v)));
}

/**
 * A hull's opaque faces as triangles offset from the craft, turned by its attitude: the occluder
 * that hides the hull's hidden lines (Design note 5), and over the photorealistic image its
 * silhouette (R07.T16.e).
 *
 * @remarks
 * Its windows ({@link HullOutline.windows}) are left out: glass hides nothing, in either style
 * (decision-r07-t16a, item 3). Their edges are among {@link hullEdges}.
 */
export function hullFaces(
  hull: HullOutline,
  attitude: Quaternion,
): ReadonlyArray<readonly [Vec3, Vec3, Vec3]> {
  const turned = hull.vertices.map((v) => rotate(attitude, v));
  const glazed = new Set(hull.windows);
  return hull.faces.flatMap(([i, j, k], index) => {
    if (glazed.has(index)) {
      return [];
    }
    const a = turned[i];
    const b = turned[j];
    const c = turned[k];
    if (a === undefined || b === undefined || c === undefined) {
      throw new Error(`hull ${hull.name} has a face on a vertex it lacks`);
    }
    return [[a, b, c] as const];
  });
}

/** Where a point stands on the view: its pixel, and its distance in front of the camera, m. */
function onView(
  pointM: Vec3,
  projection: ProjectionCamera,
  viewport: Viewport,
): { readonly at: ScreenPx; readonly depthM: number } {
  const p = project(pointM, projection, viewport);
  return {
    at: { xPx: p.xPx, yPx: p.yPx },
    depthM: -toViewAxes(pointM, projection.orientation).z,
  };
}

/**
 * A hull's edge on the view, clipped at the near plane (decision-r10-t12-commit3 §1): both ends
 * projected where both are in front of it, its part in front where it crosses it, and nothing
 * where it lies wholly behind; `a` and `b` from the camera, m.
 */
function clippedEdge(
  a: Vec3,
  b: Vec3,
  projection: ProjectionCamera,
  viewport: Viewport,
): ScreenSegment | null {
  const from = onView(a, projection, viewport);
  const to = onView(b, projection, viewport);
  if (from.depthM >= NEAR_PLANE_M && to.depthM >= NEAR_PLANE_M) {
    return [from.at, to.at];
  }
  if (from.depthM < NEAR_PLANE_M && to.depthM < NEAR_PLANE_M) {
    return null;
  }
  // The point on the edge at the near plane, which stands in for the end behind it.
  const t = (NEAR_PLANE_M - from.depthM) / (to.depthM - from.depthM);
  const atNear = onView(
    add(a, vec3((b.x - a.x) * t, (b.y - a.y) * t, (b.z - a.z) * t)),
    projection,
    viewport,
  ).at;
  return from.depthM >= NEAR_PLANE_M ? [from.at, atNear] : [atNear, to.at];
}

/**
 * Whether the ray from the camera along `direction`, a unit vector, meets the triangle `a`, `b`,
 * `c` (from the camera, m) at or beyond `nearM`, by Möller and Trumbore (1997), "Fast, minimum
 * storage ray-triangle intersection", J. Graphics Tools 2(1), 21-28, in `f64`.
 */
function rayMeetsTriangle(direction: Vec3, a: Vec3, b: Vec3, c: Vec3, nearM: number): boolean {
  const e1 = sub(b, a);
  const e2 = sub(c, a);
  const p = cross(direction, e2);
  const det = dot(e1, p);
  if (det === 0 || !Number.isFinite(det)) {
    return false;
  }
  const inverse = 1 / det;
  // The ray starts at the camera, the origin: s = 0 − a.
  const s = vec3(-a.x, -a.y, -a.z);
  const u = dot(s, p) * inverse;
  if (u < 0 || u > 1) {
    return false;
  }
  const q = cross(s, e1);
  const v = dot(direction, q) * inverse;
  if (v < 0 || u + v > 1) {
    return false;
  }
  return dot(e2, q) * inverse >= nearM;
}

/**
 * A craft's hull as the view's line-of-sight mark gives way to it (plan R10, R10.T12;
 * decision-r10-t12-commit3 §1): every edge, its windows' and those hidden behind its own faces
 * included, projected and clipped at the near plane; and whether one of its opaque faces stands over
 * the point, the picture's centre, where the line of sight, the camera's forward axis, meets it in
 * front of the near plane.
 *
 * @param attitude - The craft's attitude, by which its hull is turned.
 * @param originM - The craft's reference point from the camera, m along the frame's axes.
 */
export function hullOnView(
  hull: HullOutline,
  attitude: Quaternion,
  originM: Vec3,
  projection: ProjectionCamera,
  viewport: Viewport,
): HullOnView {
  const edges = hullEdges(hull, attitude).flatMap(([a, b]) => {
    const edge = clippedEdge(add(originM, a), add(originM, b), projection, viewport);
    return edge === null ? [] : [edge];
  });
  // The camera's forward axis, its view axes' −z, along the frame's axes: the picture's centre.
  const [r0, r1, r2] = rotationRows(projection.orientation);
  const forward = vec3(-r0.z, -r1.z, -r2.z);
  const coversPoint = hullFaces(hull, attitude).some(([a, b, c]) =>
    rayMeetsTriangle(forward, add(originM, a), add(originM, b), add(originM, c), NEAR_PLANE_M),
  );
  return { edges, coversPoint };
}
