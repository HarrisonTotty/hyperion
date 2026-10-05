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

import { add, dot, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { PATCH_QUADS, stToUv } from "./cube";
import type { Face, PatchKey } from "./patchKey";
import {
  type BodyFigure,
  type BodyFixedVec3,
  levelHeightRangeM,
  type PlanetGeometry,
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

/**
 * The boundary's vertices, every {@link SAMPLE_STEP}th, in order round the patch and back to the
 * first: their x, then their y.
 */
function boundaryVertices(): readonly [Int32Array, Int32Array] {
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
  ring.push(ring[0] ?? [0, 0]);
  return [Int32Array.from(ring, ([x]) => x), Int32Array.from(ring, ([, y]) => y)];
}

const [RING_X, RING_Y] = boundaryVertices();

/** The points the bounds are built from: each ring vertex at the top and the bottom, then the centre's. */
const POINTS = 2 * RING_X.length + 2;

/**
 * Scratch for {@link patchBounds}: a direction, x, y and z, and the points, three numbers a
 * point. Used within one call, which nothing re-enters.
 */
const DIR = new Float64Array(3);
const XYZ = new Float64Array(3 * POINTS);

/**
 * `canonicalFace` of the integer point (`px`, `py`, `pz`) on the cube of half-width `half`: the
 * lowest-index face holding it.
 *
 * @throws Error for a point on no face.
 */
function canonicalFaceOf(px: number, py: number, pz: number, half: number): Face {
  if (px === half) {
    return 0;
  }
  if (py === half) {
    return 1;
  }
  if (pz === half) {
    return 2;
  }
  if (px === -half) {
    return 3;
  }
  if (py === -half) {
    return 4;
  }
  if (pz === -half) {
    return 5;
  }
  throw new Error(`point (${px}, ${py}, ${pz}) is not on the cube of half-width ${half}`);
}

/**
 * Writes into {@link DIR} `vertexDir(key, x, y)`: the same operations in the same order as
 * `cube.ts`'s `sampleDir` (the vertex on its canonical face, the warp and the norm summed left to
 * right), with no array.
 */
function vertexDirIntoScratch(key: PatchKey, x: number, y: number): void {
  const quads = PATCH_QUADS * 2 ** key.level;
  const cu = 2 * (key.i * PATCH_QUADS + x) - quads;
  const cv = 2 * (key.j * PATCH_QUADS + y) - quads;
  // `facePoint`'s point on the cube of half-width `quads`.
  let px = 0;
  let py = 0;
  let pz = 0;
  switch (key.face) {
    case 0:
      px = quads;
      py = cu;
      pz = cv;
      break;
    case 1:
      px = -cu;
      py = quads;
      pz = cv;
      break;
    case 2:
      px = -cu;
      py = -cv;
      pz = quads;
      break;
    case 3:
      px = -quads;
      py = -cv;
      pz = -cu;
      break;
    case 4:
      px = cv;
      py = -quads;
      pz = -cu;
      break;
    case 5:
      px = cv;
      py = cu;
      pz = -quads;
      break;
  }
  const face = canonicalFaceOf(px, py, pz, quads);
  // `faceCoords`, then `latticeDir`'s warp and `faceUvToXyz`.
  let u = 0;
  let v = 0;
  switch (face) {
    case 0:
      u = py;
      v = pz;
      break;
    case 1:
      u = -px;
      v = pz;
      break;
    case 2:
      u = -px;
      v = -py;
      break;
    case 3:
      u = -pz;
      v = -py;
      break;
    case 4:
      u = -pz;
      v = px;
      break;
    case 5:
      u = py;
      v = px;
      break;
  }
  const s = stToUv((u + quads) / 2 / quads);
  const t = stToUv((v + quads) / 2 / quads);
  let dx = 0;
  let dy = 0;
  let dz = 0;
  switch (face) {
    case 0:
      dx = 1;
      dy = s;
      dz = t;
      break;
    case 1:
      dx = -s;
      dy = 1;
      dz = t;
      break;
    case 2:
      dx = -s;
      dy = -t;
      dz = 1;
      break;
    case 3:
      dx = -1;
      dy = -t;
      dz = -s;
      break;
    case 4:
      dx = t;
      dy = -1;
      dz = -s;
      break;
    case 5:
      dx = t;
      dy = s;
      dz = -1;
      break;
  }
  // `unitDir`: the norm summed left to right, never `Math.hypot`.
  const length = Math.sqrt(dx * dx + dy * dy + dz * dz);
  DIR[0] = dx / length;
  DIR[1] = dy / length;
  DIR[2] = dz / length;
}

/**
 * The squared lengths within which a `Math.hypot` can be the largest of a set, as a fraction of
 * the largest squared length.
 *
 * @remarks
 * `patchBounds` takes the largest `Math.hypot` of a set of differences, and calls it only for the
 * differences whose squared length x² + y² + z² is not below this fraction of the largest: the
 * squares are within 4 ulps of the true squared lengths and `Math.hypot` within a few ulps of the
 * true length, so any other difference is shorter than the longest by thousands of ulps and its
 * `Math.hypot` cannot be the largest or equal it. For finite metres the result is the same number
 * as taking every `Math.hypot`, which costs several times a square root; a NaN makes every
 * difference a candidate, so a NaN still comes out as it did.
 */
const NEAR_LARGEST = 1 - 1e-12;

/** The squared length of the difference of the points at `a` and `b` in {@link XYZ}. */
function squaredGap(a: number, b: number): number {
  const dx = (XYZ[a] ?? 0) - (XYZ[b] ?? 0);
  const dy = (XYZ[a + 1] ?? 0) - (XYZ[b + 1] ?? 0);
  const dz = (XYZ[a + 2] ?? 0) - (XYZ[b + 2] ?? 0);
  return dx * dx + dy * dy + dz * dz;
}

/** The squared length of the point at `a` in {@link XYZ} less (`x`, `y`, `z`). */
function squaredFrom(a: number, x: number, y: number, z: number): number {
  const dx = (XYZ[a] ?? 0) - x;
  const dy = (XYZ[a + 1] ?? 0) - y;
  const dz = (XYZ[a + 2] ?? 0) - z;
  return dx * dx + dy * dy + dz * dz;
}

/**
 * Writes into {@link XYZ} at `at` `surfacePoint(figure, d, h)` at the top and then the bottom of the
 * range, M·d + h·ν with ν = M⁻¹d ÷ |M⁻¹d|, in `planet.ts`'s operations and order.
 */
function writePoints(
  at: number,
  d0: number,
  d1: number,
  d2: number,
  figure: BodyFigure,
  minHeightM: number,
  maxHeightM: number,
): void {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const m0 = d0 / a;
  const m1 = d1 / a;
  const m2 = d2 / c;
  const len = Math.sqrt(m0 * m0 + m1 * m1 + m2 * m2);
  const nu0 = m0 / len;
  const nu1 = m1 / len;
  const nu2 = m2 / len;
  const p0 = a * d0;
  const p1 = a * d1;
  const p2 = c * d2;
  XYZ[at] = p0 + maxHeightM * nu0;
  XYZ[at + 1] = p1 + maxHeightM * nu1;
  XYZ[at + 2] = p2 + maxHeightM * nu2;
  XYZ[at + 3] = p0 + minHeightM * nu0;
  XYZ[at + 4] = p1 + minHeightM * nu1;
  XYZ[at + 5] = p2 + minHeightM * nu2;
}

/**
 * The bounding volume of patch `key`: a sphere and an oriented box enclosing the patch's surface
 * at every height in `heightRangeM`, by default its level's range (all zero with no level table).
 *
 * @remarks
 * Built in scalars over preallocated scratch, so that a call allocates only the bounds it returns
 * (R05.T7 perf (c)): the same `f64` operations, in the same order, as the vector form it replaced,
 * which `bounds.test.ts` keeps as its oracle, bit for bit.
 *
 * @param heightRangeM - The lowest and highest height the patch can reach, metres: a tighter range
 *   inherited from a baked ancestor (selection's `heightRanges`), or the level's.
 */
export function patchBounds(
  planet: PlanetGeometry,
  key: PatchKey,
  heightRangeM: readonly [number, number] = levelHeightRangeM(planet, key.level),
): PatchBounds {
  const [minHeightM, maxHeightM] = heightRangeM;
  const a = planet.figure.equatorialRadiusM;
  const c = planet.figure.polarRadiusM;
  const half = PATCH_QUADS / 2;
  // The centre's spheroid normal ν = M⁻¹d ÷ |M⁻¹d| and the origin M·d + h·ν at the mid height.
  vertexDirIntoScratch(key, half, half);
  const cd0 = DIR[0] ?? 0;
  const cd1 = DIR[1] ?? 0;
  const cd2 = DIR[2] ?? 0;
  const cm0 = cd0 / a;
  const cm1 = cd1 / a;
  const cm2 = cd2 / c;
  const clen = Math.sqrt(cm0 * cm0 + cm1 * cm1 + cm2 * cm2);
  const nx = cm0 / clen;
  const ny = cm1 / clen;
  const nz = cm2 / clen;
  const midM = (minHeightM + maxHeightM) / 2;
  const ox = a * cd0 + midM * nx;
  const oy = a * cd1 + midM * ny;
  const oz = c * cd2 + midM * nz;
  // The tangent t1 along the patch's u, the across vector less its part along ν, and t2 = ν × t1.
  vertexDirIntoScratch(key, PATCH_QUADS, half);
  const ex = DIR[0] ?? 0;
  const ey = DIR[1] ?? 0;
  const ez = DIR[2] ?? 0;
  vertexDirIntoScratch(key, 0, half);
  const acx = ex - (DIR[0] ?? 0);
  const acy = ey - (DIR[1] ?? 0);
  const acz = ez - (DIR[2] ?? 0);
  const along = acx * nx + acy * ny + acz * nz;
  const wx = acx - nx * along;
  const wy = acy - ny * along;
  const wz = acz - nz * along;
  const wlen = Math.hypot(wx, wy, wz);
  if (!(wlen > 0) || !Number.isFinite(wlen)) {
    throw new RangeError(`cannot normalise a vector of length ${String(wlen)}`);
  }
  const inverse = 1 / wlen;
  const t1x = wx * inverse;
  const t1y = wy * inverse;
  const t1z = wz * inverse;
  const t2x = ny * t1z - nz * t1y;
  const t2y = nz * t1x - nx * t1z;
  const t2z = nx * t1y - ny * t1x;

  // Each ring vertex's top and bottom, then the centre's.
  let count = 0;
  for (let n = 0; n < RING_X.length; n += 1) {
    vertexDirIntoScratch(key, RING_X[n] ?? 0, RING_Y[n] ?? 0);
    writePoints(
      count,
      DIR[0] ?? 0,
      DIR[1] ?? 0,
      DIR[2] ?? 0,
      planet.figure,
      minHeightM,
      maxHeightM,
    );
    count += 6;
  }
  writePoints(count, cd0, cd1, cd2, planet.figure, minHeightM, maxHeightM);
  count += 6;
  // The largest chord between neighbouring tops, the ring's six numbers a vertex apart.
  const ringEnd = 6 * RING_X.length;
  let largestSq = 0;
  for (let top = 6; top < ringEnd; top += 6) {
    largestSq = Math.max(largestSq, squaredGap(top, top - 6));
  }
  let marginM = 0;
  for (let top = 6; top < ringEnd; top += 6) {
    if (!(squaredGap(top, top - 6) < largestSq * NEAR_LARGEST)) {
      marginM = Math.max(
        marginM,
        Math.hypot(
          (XYZ[top] ?? 0) - (XYZ[top - 6] ?? 0),
          (XYZ[top + 1] ?? 0) - (XYZ[top - 5] ?? 0),
          (XYZ[top + 2] ?? 0) - (XYZ[top - 4] ?? 0),
        ),
      );
    }
  }

  let lo0 = Infinity;
  let lo1 = Infinity;
  let lo2 = Infinity;
  let hi0 = -Infinity;
  let hi1 = -Infinity;
  let hi2 = -Infinity;
  for (let p = 0; p < count; p += 3) {
    const rx = (XYZ[p] ?? 0) - ox;
    const ry = (XYZ[p + 1] ?? 0) - oy;
    const rz = (XYZ[p + 2] ?? 0) - oz;
    const s0 = rx * nx + ry * ny + rz * nz;
    const s1 = rx * t1x + ry * t1y + rz * t1z;
    const s2 = rx * t2x + ry * t2y + rz * t2z;
    lo0 = Math.min(lo0, s0);
    hi0 = Math.max(hi0, s0);
    lo1 = Math.min(lo1, s1);
    hi1 = Math.max(hi1, s1);
    lo2 = Math.min(lo2, s2);
    hi2 = Math.max(hi2, s2);
  }
  // The box's centre: the origin moved to the middle of each axis's span, axis by axis.
  const m0 = (lo0 + hi0) / 2;
  const m1 = (lo1 + hi1) / 2;
  const m2 = (lo2 + hi2) / 2;
  const bx = ox + nx * m0 + t1x * m1 + t2x * m2;
  const by = oy + ny * m0 + t1y * m1 + t2y * m2;
  const bz = oz + nz * m0 + t1z * m1 + t2z * m2;
  let farthestSq = 0;
  for (let p = 0; p < count; p += 3) {
    farthestSq = Math.max(farthestSq, squaredFrom(p, bx, by, bz));
  }
  let radiusM = 0;
  for (let p = 0; p < count; p += 3) {
    if (!(squaredFrom(p, bx, by, bz) < farthestSq * NEAR_LARGEST)) {
      radiusM = Math.max(
        radiusM,
        Math.hypot((XYZ[p] ?? 0) - bx, (XYZ[p + 1] ?? 0) - by, (XYZ[p + 2] ?? 0) - bz),
      );
    }
  }
  const boxCentre = vec3(bx, by, bz);
  return {
    centre: boxCentre,
    radiusM: radiusM + marginM,
    minHeightM,
    maxHeightM,
    box: {
      centre: boxCentre,
      axes: [vec3(nx, ny, nz), vec3(t1x, t1y, t1z), vec3(t2x, t2y, t2z)],
      halfExtentsM: [
        (hi0 - lo0) / 2 + marginM,
        (hi1 - lo1) / 2 + marginM,
        (hi2 - lo2) / 2 + marginM,
      ],
    },
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
