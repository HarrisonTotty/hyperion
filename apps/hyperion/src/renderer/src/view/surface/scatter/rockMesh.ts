/**
 * A rock's drawn mesh (plan R11, T4.a, Design note 6): its superquadric tessellated so that every
 * vertex lies on the analytic surface, at the level whose chordal error is within the rock's
 * tolerance.
 *
 * @remarks
 * - **The shape** is the surface crate's (`scatter::shape`): |x ÷ a|ⁿ + |y ÷ b|ⁿ + |z ÷ c|ⁿ = 1 in
 *   the rock's frame about its base centre, x along its a axis, z along the datum's normal, half
 *   of it below the ground at its axis.
 * - **The tessellation** at level N is a cube sphere of N × N quads a face, six faces: each grid
 *   point's direction d, the cube's face point at equal angles (tan(π ÷ 4 · (2k ÷ N − 1)) along each
 *   side, ±1 exactly at the edges and mirrored about the middle), is carried radially onto the unit
 *   superquadric, s = d ÷ (|d_x|ⁿ + |d_y|ⁿ + |d_z|ⁿ)^(1/n), and stretched by the semi-axes,
 *   q = (a s_x, b s_y, c s_z), which satisfies the shape's equation: the vertex is on the analytic
 *   surface to the rounding of its arithmetic. A face edge's vertex is computed from the same
 *   components on both faces, so the seams are the same bits and the mesh is closed. Its normal is
 *   the surface's own, ∇F at q. Triangles are counter-clockwise seen from outside, each quad split
 *   on its (0, 0)–(1, 1) diagonal, as the terrain's are.
 * - **The chordal error**: the shape is convex for n ≥ 1 and every vertex is on it, so the mesh
 *   lies inside it, by at most the distance from a triangle's points to the surface.
 *   {@link chordalBound} bounds it by each triangle's deviations at its centroid and at its edges'
 *   midpoints, each taken along the ray from the rock's centre (which is no shorter than the
 *   distance to the surface), the midpoints' scaled by 4 ÷ 3, the ratio of a quadratic surface's
 *   greatest deviation over an equilateral triangle to its deviation at an edge's middle, and the
 *   whole by {@link CHORDAL_SAFETY}. The tests hold it against a dense sampling of every triangle
 *   for 10³ random rocks.
 * - **The level** ({@link rockLevel}) is the least N whose bound is within the rock's tolerance,
 *   the lesser of {@link CHORDAL_TOLERANCE_M} and {@link CHORDAL_TOLERANCE_SHARE} of its smallest
 *   semi-axis (Design note 6), at most {@link MAX_ROCK_LEVEL}.
 */

import { type Vec3, vec3 } from "../../../geometry/vec3";
import type { Rock } from "./rockLayer";

/** The most a drawn rock may lie inside its shape, m: 5 mm (Design note 6). */
export const CHORDAL_TOLERANCE_M = 0.005;

/** The most a drawn rock may lie inside its shape as a share of its smallest semi-axis: 2%. */
export const CHORDAL_TOLERANCE_SHARE = 0.02;

/**
 * The factor on the sampled deviations that makes {@link chordalBound} a bound: 1.25, above the
 * densely sampled deviation's ratio to the sampled one over 10³ random rocks at every level
 * (`rockMesh.test.ts`).
 */
export const CHORDAL_SAFETY = 1.25;

/** The least tessellation level, N = 2: a face's quads two a side. */
export const MIN_ROCK_LEVEL = 2;

/**
 * The finest tessellation level, N = 48: 27,648 triangles, past what the largest and roundest rock
 * needs (a 6.4 m rock at n = 3 needs about 40).
 */
export const MAX_ROCK_LEVEL = 48;

/** A rock's tessellation at one level (see the module's remarks). */
export interface RockMesh {
  /** N, the quads along a cube face's side. */
  readonly level: number;
  /**
   * Each vertex from the rock's base centre, body-fixed axes, m, three a vertex: the six faces'
   * (N + 1)² grids in turn, row by row, a seam's vertices on each face that holds them.
   */
  readonly positionsM: Float64Array;
  /** Each vertex's outward unit normal, body-fixed, three a vertex. */
  readonly normals: Float64Array;
  /** Three vertex indices a triangle, counter-clockwise seen from outside. */
  readonly triangles: Uint32Array;
}

/** A cube face's outward axis and its two axes along it, right-handed (u × v = n). */
interface CubeFace {
  /** The index of the face's own axis, and its sign. */
  readonly axis: 0 | 1 | 2;
  readonly sign: 1 | -1;
  /** The indices of the axes u and v run along, and their signs. */
  readonly u: 0 | 1 | 2;
  readonly uSign: 1 | -1;
  readonly v: 0 | 1 | 2;
  readonly vSign: 1 | -1;
}

/** The six faces, +x, −x, +y, −y, +z, −z, each right-handed. */
const CUBE_FACES: ReadonlyArray<CubeFace> = [
  { axis: 0, sign: 1, u: 1, uSign: 1, v: 2, vSign: 1 },
  { axis: 0, sign: -1, u: 2, uSign: 1, v: 1, vSign: 1 },
  { axis: 1, sign: 1, u: 2, uSign: 1, v: 0, vSign: 1 },
  { axis: 1, sign: -1, u: 0, uSign: 1, v: 2, vSign: 1 },
  { axis: 2, sign: 1, u: 0, uSign: 1, v: 1, vSign: 1 },
  { axis: 2, sign: -1, u: 1, uSign: 1, v: 0, vSign: 1 },
];

/**
 * The cube's coordinate at each of a side's N + 1 grid points, at equal angles: −1 and 1 exactly
 * at the ends, mirrored about the middle, so that a seam's vertex is the same bits on both faces.
 */
function sideCoordinates(level: number): Float64Array {
  const out = new Float64Array(level + 1);
  for (let k = 0; 2 * k <= level; k += 1) {
    const t = k === 0 ? -1 : Math.tan((Math.PI / 4) * ((2 * k) / level - 1));
    out[k] = t;
    out[level - k] = -t;
  }
  if (level % 2 === 0) {
    out[level / 2] = 0;
  }
  return out;
}

/**
 * The rock's tolerance, m: the lesser of {@link CHORDAL_TOLERANCE_M} and
 * {@link CHORDAL_TOLERANCE_SHARE} of its smallest semi-axis (Design note 6).
 */
export function rockToleranceM(rock: Rock): number {
  return Math.min(CHORDAL_TOLERANCE_M, CHORDAL_TOLERANCE_SHARE * Math.min(...rock.semiAxesM));
}

/**
 * Where the ray from the rock's centre through its frame's point (`x`, `y`, `z`) meets the shape,
 * as a factor on the point: 1 on the surface, above 1 inside.
 */
function toSurface(rock: Rock, x: number, y: number, z: number): number {
  const [a, b, c] = rock.semiAxesM;
  const n = rock.exponent;
  const f = Math.abs(x / a) ** n + Math.abs(y / b) ** n + Math.abs(z / c) ** n;
  return f ** (-1 / n);
}

/**
 * The distance, m, from the frame's point (`x`, `y`, `z`) inside the rock to its surface along the
 * ray from its centre: no shorter than the distance to the surface's nearest point.
 */
export function radialDepthM(rock: Rock, x: number, y: number, z: number): number {
  return Math.hypot(x, y, z) * (toSurface(rock, x, y, z) - 1);
}

/** The rock's frame's points of a level's grid, before the frame is turned into the body's axes. */
interface FrameGrid {
  /** Each vertex in the rock's frame, m, three a vertex, in {@link RockMesh.positionsM}' order. */
  readonly points: Float64Array;
  readonly triangles: Uint32Array;
}

/** Checks that `level` is a whole number of quads, at least 1. */
function checkLevel(level: number): void {
  if (!Number.isInteger(level) || level < 1) {
    throw new RangeError(`a rock's tessellation level is a whole number of quads, not ${level}`);
  }
}

/**
 * Writes into `out` at `at` the frame point of grid point (`i`, `j`) of `face` at the side's
 * coordinates `t` (see the module's remarks): its direction's components set, never computed,
 * so that a seam's point is the same bits on both faces.
 */
function gridPoint(
  rock: Rock,
  face: CubeFace,
  t: Float64Array,
  i: number,
  j: number,
  out: Float64Array,
  at: number,
): void {
  const [a, b, c] = rock.semiAxesM;
  const n = rock.exponent;
  const d = [0, 0, 0];
  d[face.axis] = face.sign;
  d[face.u] = face.uSign * (t[i] ?? 0);
  d[face.v] = face.vSign * (t[j] ?? 0);
  const [dx = 0, dy = 0, dz = 0] = d;
  const r = (Math.abs(dx) ** n + Math.abs(dy) ** n + Math.abs(dz) ** n) ** (1 / n);
  out[at] = (a * dx) / r;
  out[at + 1] = (b * dy) / r;
  out[at + 2] = (c * dz) / r;
}

/** The level's grid of points on the rock's surface in its own frame (see the module's remarks). */
function frameGrid(rock: Rock, level: number): FrameGrid {
  checkLevel(level);
  const side = level + 1;
  const t = sideCoordinates(level);
  const points = new Float64Array(3 * 6 * side * side);
  const triangles = new Uint32Array(3 * 2 * 6 * level * level);
  let p = 0;
  let q = 0;
  CUBE_FACES.forEach((face, f) => {
    const first = f * side * side;
    for (let j = 0; j <= level; j += 1) {
      for (let i = 0; i <= level; i += 1) {
        gridPoint(rock, face, t, i, j, points, p);
        p += 3;
      }
    }
    for (let j = 0; j < level; j += 1) {
      for (let i = 0; i < level; i += 1) {
        const v00 = first + j * side + i;
        const v10 = v00 + 1;
        const v01 = v00 + side;
        const v11 = v01 + 1;
        triangles.set([v00, v10, v11, v00, v11, v01], q);
        q += 6;
      }
    }
  });
  return { points, triangles };
}

/**
 * The outward unit normal of the rock's surface at its frame's point (`x`, `y`, `z`), in the frame:
 * ∇F there, F = |x ÷ a|ⁿ + |y ÷ b|ⁿ + |z ÷ c|ⁿ.
 */
function frameNormal(rock: Rock, x: number, y: number, z: number): Vec3 {
  const [a, b, c] = rock.semiAxesM;
  const n = rock.exponent;
  const g = (w: number, s: number): number => (Math.sign(w) * Math.abs(w / s) ** (n - 1)) / s;
  const [gx, gy, gz] = [g(x, a), g(y, b), g(z, c)];
  const length = Math.hypot(gx, gy, gz);
  return vec3(gx / length, gy / length, gz / length);
}

/** The frame's vector (`x`, `y`, `z`) in the body's axes. */
function toBody(rock: Rock, x: number, y: number, z: number): Vec3 {
  const [ax, bx, cx] = rock.axes;
  return vec3(
    x * ax.x + y * bx.x + z * cx.x,
    x * ax.y + y * bx.y + z * cx.y,
    x * ax.z + y * bx.z + z * cx.z,
  );
}

/**
 * The rock tessellated at `level` (see the module's remarks): its vertices from its base centre in
 * the body's axes, their normals, and its triangles.
 *
 * @throws RangeError where `level` is not a whole number of at least 1.
 */
export function tessellateRock(rock: Rock, level: number): RockMesh {
  const { points, triangles } = frameGrid(rock, level);
  const vertices = points.length / 3;
  const positionsM = new Float64Array(points.length);
  const normals = new Float64Array(points.length);
  for (let v = 0; v < vertices; v += 1) {
    const [x = 0, y = 0, z = 0] = points.subarray(3 * v, 3 * v + 3);
    const at = toBody(rock, x, y, z);
    const normal = frameNormal(rock, x, y, z);
    const turned = toBody(rock, normal.x, normal.y, normal.z);
    positionsM.set([at.x, at.y, at.z], 3 * v);
    normals.set([turned.x, turned.y, turned.z], 3 * v);
  }
  return { level, positionsM, normals, triangles };
}

/**
 * The most the rock's tessellation at `level` lies inside its shape, m (see the module's remarks):
 * a bound on every triangle's points' distance to the surface.
 *
 * @remarks
 * The shape and the grid are each symmetric under a mirror of any frame axis, and a mirror carries
 * a quad's diagonal to its other one, so the bound is taken over the quads of one quarter of each
 * of the three positive faces, each split on both diagonals: every triangle of the mesh is the
 * mirror image of one of those, at a quarter of the cost.
 *
 * @throws RangeError where `level` is not a whole number of at least 1.
 */
export function chordalBound(rock: Rock, level: number): number {
  checkLevel(level);
  const t = sideCoordinates(level);
  const from = Math.floor(level / 2);
  const side = level - from + 1;
  const points = new Float64Array(3 * side * side);
  let most = 0;
  const depth = (x: number, y: number, z: number): number => radialDepthM(rock, x, y, z);
  const middle = (u: number, w: number): number =>
    depth(
      ((points[u] ?? 0) + (points[w] ?? 0)) / 2,
      ((points[u + 1] ?? 0) + (points[w + 1] ?? 0)) / 2,
      ((points[u + 2] ?? 0) + (points[w + 2] ?? 0)) / 2,
    );
  const triangle = (u: number, v: number, w: number): void => {
    const centroid = depth(
      ((points[u] ?? 0) + (points[v] ?? 0) + (points[w] ?? 0)) / 3,
      ((points[u + 1] ?? 0) + (points[v + 1] ?? 0) + (points[w + 1] ?? 0)) / 3,
      ((points[u + 2] ?? 0) + (points[v + 2] ?? 0) + (points[w + 2] ?? 0)) / 3,
    );
    const edges = Math.max(middle(u, v), middle(v, w), middle(w, u));
    most = Math.max(most, centroid, (4 / 3) * edges);
  };
  for (const face of CUBE_FACES.filter((f) => f.sign > 0)) {
    for (let j = 0; j < side; j += 1) {
      for (let i = 0; i < side; i += 1) {
        gridPoint(rock, face, t, from + i, from + j, points, 3 * (j * side + i));
      }
    }
    for (let j = 0; j + 1 < side; j += 1) {
      for (let i = 0; i + 1 < side; i += 1) {
        const v00 = 3 * (j * side + i);
        const [v10, v01, v11] = [v00 + 3, v00 + 3 * side, v00 + 3 * side + 3];
        triangle(v00, v10, v11);
        triangle(v00, v11, v01);
        triangle(v00, v10, v01);
        triangle(v10, v11, v01);
      }
    }
  }
  return CHORDAL_SAFETY * most;
}

/** The level whose bound {@link rockLevel} first takes, from which it estimates the rock's. */
const PROBE_LEVEL = 8;

/**
 * The level Design note 6 picks for the rock: one whose {@link chordalBound} is within
 * {@link rockToleranceM}, from {@link MIN_ROCK_LEVEL} to {@link MAX_ROCK_LEVEL}, and the level
 * below it not, where that is within the range.
 *
 * @remarks
 * A chordal error falls about as 1 ÷ N², so the walk starts from the level the probe level's bound
 * scales to, N = 8 √(bound ÷ tolerance) rounded up, then steps down while the level below is
 * within the tolerance and up while its own is not: a few bounds a rock rather than one a level.
 */
export function rockLevel(rock: Rock): number {
  const tolerance = rockToleranceM(rock);
  const probe = chordalBound(rock, PROBE_LEVEL);
  const clamp = (n: number): number => Math.min(MAX_ROCK_LEVEL, Math.max(MIN_ROCK_LEVEL, n));
  let level = clamp(Math.ceil(PROBE_LEVEL * Math.sqrt(probe / tolerance)));
  while (level > MIN_ROCK_LEVEL && chordalBound(rock, level - 1) <= tolerance) {
    level -= 1;
  }
  while (level < MAX_ROCK_LEVEL && chordalBound(rock, level) > tolerance) {
    level += 1;
  }
  return level;
}
