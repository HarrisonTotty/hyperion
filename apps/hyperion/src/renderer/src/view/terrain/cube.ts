/**
 * The cube sphere's mapping, the client's mirror of `hyperion_surface::cube` and `geometry` (plan
 * R05, T2, Design notes 1–3).
 *
 * @remarks
 * Selection needs the bounds of patches not yet baked, so the client carries this mirror of the
 * warp and the patch geometry; it decides only what to draw, and every height and vertex position
 * reaching the GPU comes from the worker. The functions follow the Rust line for line, operation
 * for operation: only `+ − × ÷` and `Math.sqrt`, which IEEE 754 rounds exactly in both languages,
 * in the same order (`4 * s * s - 1`, a division by 3, the norm summed left to right and never
 * `Math.hypot`, negations as unary minus so that +0 turns into −0), so that a vitest pins every
 * value bit for bit to the Rust golden `cube_sphere.golden` (`cube.test.ts`).
 */

import { type Vec3, vec3 } from "../../geometry/vec3";
import {
  canonicalFace,
  type Face,
  faceCoords,
  faceOfAxis,
  facePoint,
  MAX_LEVEL,
  type PatchKey,
  unreachable,
} from "./patchKey";

/** Quads along a patch's side: a patch is 65 × 65 vertices (`hyperion_surface::PATCH_QUADS`). */
export const PATCH_QUADS = 64 as const;

/** A direction or a cube point as three numbers, x, y and z in body-fixed axes. */
export type Xyz = readonly [number, number, number];

/** S2's quadratic warp from a cell coordinate s in [0, 1] to a face coordinate u in [−1, 1]. */
export function stToUv(s: number): number {
  if (s >= 0.5) {
    return (4 * s * s - 1) / 3;
  }
  const r = 1 - s;
  return (1 - 4 * r * r) / 3;
}

/** The inverse of {@link stToUv}, from u in [−1, 1] back to s in [0, 1]. */
export function uvToSt(u: number): number {
  if (u >= 0) {
    return 0.5 * Math.sqrt(1 + 3 * u);
  }
  return 1 - 0.5 * Math.sqrt(1 - 3 * u);
}

/** The point (u, v) of `face` on the cube of half-side 1, in body-fixed axes; not normalised. */
export function faceUvToXyz(face: Face, u: number, v: number): Xyz {
  switch (face) {
    case 0:
      return [1, u, v];
    case 1:
      return [-u, 1, v];
    case 2:
      return [-u, -v, 1];
    case 3:
      return [-1, -v, -u];
    case 4:
      return [v, -1, -u];
    case 5:
      return [v, u, -1];
  }
  return unreachable(face);
}

/** `p` scaled to unit length, the norm summed left to right as Rust's `unit_dir` does. */
export function unitDir(p: Xyz): Xyz {
  const [x, y, z] = p;
  const norm = Math.sqrt(x * x + y * y + z * z);
  if (!(Number.isFinite(norm) && norm > 0)) {
    throw new Error(`a direction must be finite and non-zero, got (${p.join(", ")})`);
  }
  return [x / norm, y / norm, z / norm];
}

/** The unit direction of the point (u, v) of `face`. */
export function faceUvToDir(face: Face, u: number, v: number): Vec3 {
  const [x, y, z] = unitDir(faceUvToXyz(face, u, v));
  return vec3(x, y, z);
}

/** The face of the direction `p`: its largest |component|, ties to the lowest face index. */
export function faceOf(p: Xyz): Face {
  if (!p.every((c) => Number.isFinite(c)) || p.every((c) => c === 0)) {
    throw new Error(`a direction must be finite and non-zero, got (${p.join(", ")})`);
  }
  const largest = Math.max(Math.abs(p[0]), Math.abs(p[1]), Math.abs(p[2]));
  let best: Face | null = null;
  for (const axis of [0, 1, 2] as const) {
    const c = p[axis];
    if (Math.abs(c) === largest) {
      const face = faceOfAxis(axis, c);
      if (best === null || face < best) {
        best = face;
      }
    }
  }
  if (best === null) {
    throw new Error("a face of largest magnitude exists for a non-zero direction");
  }
  return best;
}

/** A point on a face. */
export interface FaceUv {
  readonly face: Face;
  readonly u: number;
  readonly v: number;
}

/** The face of the direction `p` and its (u, v) there; `p` need not be normalised. */
export function xyzToFaceUv(p: Xyz): FaceUv {
  const face = faceOf(p);
  const [x, y, z] = p;
  switch (face) {
    case 0:
      return { face, u: y / x, v: z / x };
    case 1:
      return { face, u: -x / y, v: z / y };
    case 2:
      return { face, u: -x / z, v: -y / z };
    case 3:
      return { face, u: z / x, v: y / x };
    case 4:
      return { face, u: z / y, v: -x / y };
    case 5:
      return { face, u: -y / z, v: -x / z };
  }
  return unreachable(face);
}

/** The unit direction of lattice vertex (a, b) of `face` on a lattice of `quads` quads a side. */
function latticeDir(face: Face, a: number, b: number, quads: number): Xyz {
  return unitDir(faceUvToXyz(face, stToUv(a / quads), stToUv(b / quads)));
}

/**
 * The unit direction of sample (`x`, `y`) of a patch on a grid of `perPatch` quads a side (64 for
 * the mesh, 128 for doubled normals), evaluated on its canonical face, the lowest index of the
 * faces that contain it, so that every patch sharing a face-edge or cube-corner vertex gets the
 * same bits (Design note 2; Rust's `sample_dir`).
 */
export function sampleDir(
  k: PatchKey,
  x: number,
  y: number,
  perPatch: typeof PATCH_QUADS | 128,
): Xyz {
  if (
    !(Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0) ||
    x > perPatch ||
    y > perPatch
  ) {
    throw new Error(`vertex (${x}, ${y}) is outside a patch of ${perPatch} quads`);
  }
  const quads = perPatch * 2 ** k.level;
  const alongU = k.i * perPatch + x;
  const alongV = k.j * perPatch + y;
  const point = facePoint(k.face, 2 * alongU - quads, 2 * alongV - quads, quads);
  const face = canonicalFace(point, quads);
  const [u, v] = faceCoords(face, point);
  // Both sums are even and non-negative, so the halving is exact.
  return latticeDir(face, (u + quads) / 2, (v + quads) / 2, quads);
}

/** The unit direction of vertex (`x`, `y`) of the patch's 65 × 65 (Rust's `vertex_dir`). */
export function vertexDir(k: PatchKey, x: number, y: number): Xyz {
  return sampleDir(k, x, y, PATCH_QUADS);
}

/** The band limit of the terrain's geometry, metres (`hyperion_surface::BAND_LIMIT_M`). */
export const BAND_LIMIT_M = 2;

/** The brainstorm's finest sampling, metres (`hyperion_surface::FINEST_SPACING_M`). */
export const FINEST_SPACING_M = 0.5;

/** The largest vertex spacing a body's finest level may have, metres (Design note 3). */
export const MAX_FINEST_SPACING_M = 0.75 * FINEST_SPACING_M;

/** The mean arc rate over a face, per unit of s on the unit sphere (Rust's `MEAN_RATE`). */
const MEAN_RATE = 1.459_213_746_386_106;

function maxRate(): number {
  const u = (Math.sqrt(31) - 2) / 9;
  return ((4 / 3) * Math.sqrt(1 + 3 * u)) / (1 + u * u);
}

function minRate(): number {
  return (2 * Math.sqrt(2)) / 3;
}

/** The smallest, mean and largest vertex spacing at one level of one body, metres. */
export interface SpacingRange {
  readonly minM: number;
  readonly meanM: number;
  readonly maxM: number;
}

/** The vertex spacing of `level` on a body of radius `radiusM` (Rust's `vertex_spacing`). */
export function vertexSpacing(radiusM: number, level: number): SpacingRange {
  if (!(Number.isInteger(level) && level >= 0 && level <= MAX_LEVEL)) {
    throw new Error(`level ${level} is not a quadtree level`);
  }
  if (!(Number.isFinite(radiusM) && radiusM > 0)) {
    throw new Error(`a body's radius must be finite and positive, got ${radiusM}`);
  }
  const scale = radiusM * (1 / (PATCH_QUADS * 2 ** level));
  return { minM: minRate() * scale, meanM: MEAN_RATE * scale, maxM: maxRate() * scale };
}

/**
 * The finest level of a body of radius `radiusM`, metres: the shallowest whose largest vertex
 * spacing is at most {@link MAX_FINEST_SPACING_M} (Design note 3; level 19 for an Earth). For a
 * spheroid pass the equatorial radius.
 */
export function finestLevel(radiusM: number): number {
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    if (vertexSpacing(radiusM, level).maxM <= MAX_FINEST_SPACING_M) {
      return level;
    }
  }
  return MAX_LEVEL;
}
