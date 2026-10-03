/**
 * The bounding volumes of patches, from the cube-sphere mirror and the level table's height ranges
 * (plan R05, T7.a, Design notes 7 and 8).
 *
 * @remarks
 * A patch's volume is every point M·d + h·ν over its directions d at heights h in its level's
 * range. Its farthest points from an interior point lie on its boundary, its extremes along the
 * box's tangent axes too (they point about 90° from the centre, outside any patch), and its extreme
 * along the centre's normal at the centre vertex (M·ν there is along the centre's direction), so
 * the bounds are built
 * from the boundary's vertices at every fourth vertex, the centre vertex, both ends of the height
 * range, and a margin of the largest chord between neighbouring samples, which no boundary point
 * lies farther than from its nearest sample. Bounds are conservative: they may be larger than the
 * volume, never smaller.
 */

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { PATCH_QUADS, vertexDir, type Xyz } from "./cube";
import type { PatchKey } from "./patchKey";
import {
  type BodyFixedVec3,
  levelHeightRangeM,
  type PlanetGeometry,
  spheroidNormal,
  surfacePoint,
} from "./planet";

/** An oriented box: a centre, three orthonormal axes and the half-extent along each, metres. */
export interface OrientedBox {
  readonly centre: Vec3;
  readonly axes: readonly [Vec3, Vec3, Vec3];
  readonly halfExtentsM: readonly [number, number, number];
}

/** A patch's bounding volume in the body-fixed frame: a sphere, an oriented box and its heights. */
export interface PatchBounds {
  /** The sphere's centre, body-fixed metres. */
  readonly centre: BodyFixedVec3;
  /** The sphere's radius, metres. */
  readonly radiusM: number;
  /** The lowest height the patch can reach above the datum, metres. */
  readonly minHeightM: number;
  /** The highest height the patch can reach above the datum, metres. */
  readonly maxHeightM: number;
  /** The oriented box: the centre's spheroid normal and two tangents, body-fixed metres. */
  readonly box: OrientedBox;
}

/** The step between the boundary vertices sampled, in vertices. */
const SAMPLE_STEP = 4;

function toVec3(p: Xyz): Vec3 {
  return vec3(p[0], p[1], p[2]);
}

/** The boundary's vertices, every {@link SAMPLE_STEP}th, in order round the patch. */
function boundaryVertices(): readonly (readonly [number, number])[] {
  const ring: (readonly [number, number])[] = [];
  for (let n = 0; n < PATCH_QUADS; n += SAMPLE_STEP) {
    ring.push([n, 0]);
  }
  for (let n = 0; n < PATCH_QUADS; n += SAMPLE_STEP) {
    ring.push([PATCH_QUADS, n]);
  }
  for (let n: number = PATCH_QUADS; n > 0; n -= SAMPLE_STEP) {
    ring.push([n, PATCH_QUADS]);
  }
  for (let n: number = PATCH_QUADS; n > 0; n -= SAMPLE_STEP) {
    ring.push([0, n]);
  }
  return ring;
}

const RING = boundaryVertices();

/**
 * The bounding volume of patch `key`: a sphere and an oriented box enclosing the patch's surface
 * at every height its level's range allows (all zero with no level table).
 */
export function patchBounds(planet: PlanetGeometry, key: PatchKey): PatchBounds {
  const [minHeightM, maxHeightM] = levelHeightRangeM(planet, key.level);
  const half = PATCH_QUADS / 2;
  const centreDir = vertexDir(key, half, half);
  const origin = toVec3(surfacePoint(planet.figure, centreDir, (minHeightM + maxHeightM) / 2));
  const n = toVec3(spheroidNormal(planet.figure, centreDir));
  const across = sub(toVec3(vertexDir(key, PATCH_QUADS, half)), toVec3(vertexDir(key, 0, half)));
  const t1 = normalise(sub(across, scale(n, dot(across, n))));
  const t2 = cross(n, t1);
  const axes: readonly [Vec3, Vec3, Vec3] = [n, t1, t2];

  const points: Vec3[] = [];
  let ringTop: Vec3 | null = null;
  let marginM = 0;
  for (const [x, y] of [...RING, RING[0] ?? [0, 0]]) {
    const dir = vertexDir(key, x, y);
    const top = toVec3(surfacePoint(planet.figure, dir, maxHeightM));
    if (ringTop !== null) {
      marginM = Math.max(marginM, norm(sub(top, ringTop)));
    }
    ringTop = top;
    points.push(top, toVec3(surfacePoint(planet.figure, dir, minHeightM)));
  }
  points.push(
    toVec3(surfacePoint(planet.figure, centreDir, maxHeightM)),
    toVec3(surfacePoint(planet.figure, centreDir, minHeightM)),
  );

  const lows = [Infinity, Infinity, Infinity];
  const highs = [-Infinity, -Infinity, -Infinity];
  for (const p of points) {
    const r = sub(p, origin);
    axes.forEach((axis, k) => {
      const s = dot(r, axis);
      lows[k] = Math.min(lows[k] ?? Infinity, s);
      highs[k] = Math.max(highs[k] ?? -Infinity, s);
    });
  }
  let boxCentre = origin;
  const extents = axes.map((axis, k) => {
    const lo = lows[k] ?? 0;
    const hi = highs[k] ?? 0;
    boxCentre = add(boxCentre, scale(axis, (lo + hi) / 2));
    return (hi - lo) / 2 + marginM;
  });
  const halfExtentsM: readonly [number, number, number] = [
    extents[0] ?? 0,
    extents[1] ?? 0,
    extents[2] ?? 0,
  ];
  let radiusM = 0;
  for (const p of points) {
    radiusM = Math.max(radiusM, norm(sub(p, boxCentre)));
  }
  return {
    centre: boxCentre,
    radiusM: radiusM + marginM,
    minHeightM,
    maxHeightM,
    box: { centre: boxCentre, axes, halfExtentsM },
  };
}

/** The eight corners of a box, body-fixed metres. */
export function boxCorners(box: OrientedBox): readonly Vec3[] {
  const corners: Vec3[] = [];
  const [a0, a1, a2] = box.axes;
  const [e0, e1, e2] = box.halfExtentsM;
  for (const s0 of [-1, 1]) {
    for (const s1 of [-1, 1]) {
      for (const s2 of [-1, 1]) {
        corners.push(
          add(add(add(box.centre, scale(a0, s0 * e0)), scale(a1, s1 * e1)), scale(a2, s2 * e2)),
        );
      }
    }
  }
  return corners;
}

/** A patch's bounds relative to a camera: every position is the body-fixed difference, metres. */
export interface CameraRelativeBounds {
  /** The sphere's centre from the camera. */
  readonly centreM: Vec3;
  readonly radiusM: number;
  /** The box, its centre from the camera. */
  readonly box: OrientedBox;
}

/**
 * A patch's bounds relative to the camera at `cameraM`, body-fixed metres from the body's
 * centre: the `f64` differences, formed once, which the culling predicates read.
 */
export function relativeBounds(b: PatchBounds, cameraM: BodyFixedVec3): CameraRelativeBounds {
  return {
    centreM: sub(b.centre, cameraM),
    radiusM: b.radiusM,
    box: { centre: sub(b.box.centre, cameraM), axes: b.box.axes, halfExtentsM: b.box.halfExtentsM },
  };
}

/** The distance from a camera to the nearest point of a patch's box, metres; 0 inside it. */
export function distanceToBoxM(b: CameraRelativeBounds): number {
  // The box's centre from the camera, along the box's own axes, clamped to its extents.
  let sum = 0;
  b.box.axes.forEach((axis, k) => {
    const along = Math.abs(dot(b.box.centre, axis));
    const outside = Math.max(0, along - (b.box.halfExtentsM[k] ?? 0));
    sum += outside * outside;
  });
  return Math.sqrt(sum);
}
