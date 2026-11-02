/**
 * The cluster path's measured errors (plan R14, R14.T3.a, the height worker's output from T3.b;
 * Design note 3): e_s of each of a terrain patch's 96 clusters at strides 1, 2, 4 and 8, the largest
 * distance between the cluster drawn at stride s and drawn at stride 1, metres, in `f64`.
 *
 * @remarks
 * - **What is drawn.** The lit mesh places a grid vertex at mix(q₀, q₁, k), its own offset from the
 *   patch origin and its morph target's (`terrainPatch.wgsl`' `patchVertexOf`), and hangs a skirt
 *   foot from it along the datum's normal ν to the height min(h − d, 0), h = mix(h₀, h₁, k) and d
 *   the slot's skirt depth (`terrainSurvey.wgsl`' `lit_place`). With the bake's offsets, q₀ and q₁
 *   are those, formed in `f64` by the bake and narrowed (the height worker asks for them on every
 *   path, T3.b); without, they are the bake's heights placed in `f64` by the bake's own rule
 *   (`hyperion_surface::patch`): P = M·d + h ν at each vertex's own height, and for q₁ at its morph
 *   height where both indices are even or on level 0, else the mean of its two even neighbours on
 *   the parent mesh's diagonal. The GPU's `f32` placement strays from these by a few ulps of each
 *   vertex's offset from the origin, which e_s adds as the cluster's record pads its sphere for it
 *   ({@link CLUSTER_PAD_M} and {@link CLUSTER_PAD_FRACTION} of the patch's reach from the origin,
 *   at least any cluster's), so that e_s bounds the meshes as the GPU draws them.
 * - **Both channels.** For one k across a cluster the two meshes differ by (1 − k) D₀ + k D₁, so the
 *   larger of the differences at k = 0 and k = 1 bounds every k (Design note 3). A foot's height
 *   min(h − d, 0) bends where h crosses d, off its chord between k = 0 and 1 by at most
 *   B = |h₀ − d| |h₁ − d| ÷ |h₁ − h₀|; a point of either mesh is a convex mix of feet, so it strays
 *   from its own chord by at most the largest B of its strip's vertices, and the two meshes'
 *   difference by twice it, which a strip adds.
 * - **A block** is drawn at stride s on every s-th vertex, its quads split on their (0, 0)–(1, 1)
 *   diagonal as the stride-1 quads are, so each stride-1 triangle lies in one stride-s triangle; the
 *   difference is affine there and its length convex, so its largest is at a stride-1 vertex, where
 *   it is taken.
 * - **A strip**'s stride-s diagonals cut across the stride-1 quads, so its largest is taken at
 *   every vertex of the two meshes' common refinement: the stride-1 vertices, and where a stride-s
 *   diagonal crosses the stride-1 quads' sides and diagonals.
 * - **The chord's sagitta** is in it: the distances are in 3D, so a stride-s triangle's sag below
 *   the datum's curve, about s² × `chordSagittaM`, is measured with the relief.
 * - **Monotone**: e₁ = 0 ≤ e₂ ≤ e₄ ≤ e₈, each the largest of its own and the finer strides', as the
 *   cull's coarsest-stride-within-tolerance choice needs.
 * - **Its cost** is the height worker's after every bake (T3.b's budget, 2% of a bake): one pass a
 *   stride over each block's vertices, by tables of each vertex's stride-s triangle and weights made
 *   once, with no allocation in its loops. The morph channel's pass reads its even vertices alone:
 *   a morph target off the parent's grid is the mean of its two even neighbours on the parent's
 *   diagonal, which lie in one stride-s triangle for every s ≥ 2 (each a union of the parent's), so
 *   its difference there is the mean of theirs, and none at stride 2, whose grid the even vertices
 *   are. A root patch has no parent's grid, so its morph channel takes every vertex. The narrowed
 *   offsets keep that mean to an ulp or two, which the placement's allowance holds.
 *
 * A pure module of the render thread and the height worker alike: the worker calls it on each bake
 * and rounds the result up into half floats ({@link packClusterErrors}), the record's `f16`s.
 */

import { PATCH_QUADS, vertexDir } from "../terrain/cube";
import type { PatchKey } from "../terrain/patchKey";
import { type PlanetGeometry, spheroidNormal, surfacePoint } from "../terrain/planet";
import {
  CLUSTER_PAD_FRACTION,
  CLUSTER_PAD_M,
  CLUSTER_QUADS,
  edgeVertex,
  type Stride,
  STRIDES,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
} from "./terrainGrid";

/** The errors a cluster has, one a stride, finest first: e_s at `4 cluster + log₂ s`. */
export const ERRORS_PER_CLUSTER = STRIDES.length;

/** What {@link clusterErrors} reads of a bake (`workers/messages.ts`' `BakedPatch`). */
export interface ClusterErrorBake {
  /** Own-level height and morph target per vertex, interleaved, metres above the datum. */
  readonly heights: Float32Array;
  /** On `baked-offsets`, q₀ then q₁ per vertex from the patch origin, metres; else `null`. */
  readonly offsets: Float32Array | null;
  /** How far the skirts hang below the edge vertices, metres. */
  readonly skirtDepthM: number;
}

/** Vertices along a patch's side. */
const SIDE = PATCH_QUADS + 1;

/** Vertices of a patch. */
const VERTICES = SIDE * SIDE;

/** A patch's blocks along its side. */
const BLOCKS_PER_SIDE = PATCH_QUADS / CLUSTER_QUADS;

/** Numbers a vertex holds in a patch's points: q₀ then q₁, three each, the offsets' layout. */
const POINT_STRIDE = 6;

/**
 * One stride's interpolation over a block: for each of its vertices off the stride-s grid, the
 * vertex and the corners of the stride-s triangle holding it, each as the place of its first number
 * in a patch's points less its block's first vertex's, then the corners' barycentric weights.
 */
interface StrideTable {
  readonly count: number;
  /** Four a vertex: the vertex, then its triangle's corners. */
  readonly at: Int32Array;
  /** Three a vertex: the corners' weights. */
  readonly weights: Float64Array;
}

/**
 * The table of `stride` (see {@link StrideTable}) over the vertices off its grid, of every vertex,
 * or with `evenOnly` of those whose two indices are even, and its corners in channel `channel`'s
 * numbers (0 own, 1 morph).
 */
function strideTable(stride: Stride, evenOnly: boolean, channel: 0 | 1): StrideTable {
  const at: number[] = [];
  const weights: number[] = [];
  const place = (lx: number, ly: number): number => POINT_STRIDE * (ly * SIDE + lx) + 3 * channel;
  for (let ly = 0; ly <= CLUSTER_QUADS; ly += 1) {
    const qy = Math.min(Math.floor(ly / stride) * stride, CLUSTER_QUADS - stride);
    const fy = (ly - qy) / stride;
    for (let lx = 0; lx <= CLUSTER_QUADS; lx += 1) {
      if (
        (lx % stride === 0 && ly % stride === 0) ||
        (evenOnly && (lx % 2 === 1 || ly % 2 === 1))
      ) {
        continue;
      }
      const qx = Math.min(Math.floor(lx / stride) * stride, CLUSTER_QUADS - stride);
      const fx = (lx - qx) / stride;
      // The stride-s triangle holding it: (0, 0), (s, 0), (s, s) where fx ≥ fy, else (0, 0),
      // (0, s), (s, s), mixed by 1 − max(fx, fy), |fx − fy| and min(fx, fy).
      at.push(
        place(lx, ly),
        place(qx, qy),
        fx >= fy ? place(qx + stride, qy) : place(qx, qy + stride),
        place(qx + stride, qy + stride),
      );
      weights.push(1 - Math.max(fx, fy), Math.abs(fx - fy), Math.min(fx, fy));
    }
  }
  return {
    count: at.length / 4,
    at: Int32Array.from(at),
    weights: Float64Array.from(weights),
  };
}

/**
 * Each stride's tables, at its index in {@link STRIDES} (none at stride 1): the own channel's over
 * every vertex, and the morph channel's over the even ones, or over every vertex for a root patch
 * (see the module's remarks).
 */
const STRIDE_TABLES: ReadonlyArray<{
  readonly own: StrideTable;
  readonly morph: StrideTable;
  readonly rootMorph: StrideTable;
} | null> = STRIDES.map((stride) =>
  stride === 1
    ? null
    : {
        own: strideTable(stride, false, 0),
        morph: strideTable(stride, true, 1),
        rootMorph: strideTable(stride, false, 1),
      },
);

/**
 * Each vertex's q₀ and q₁ of `bake` of `key`, metres, from the patch origin, six numbers a vertex
 * (see the module's remarks): the bake's offsets where it has them.
 */
function drawnPoints(
  bake: ClusterErrorBake,
  key: PatchKey,
  planet: PlanetGeometry,
): Float32Array | Float64Array {
  if (bake.offsets !== null) {
    return bake.offsets;
  }
  const points = new Float64Array(POINT_STRIDE * VERTICES);
  const { heights } = bake;
  const figure = planet.figure;
  for (let y = 0; y < SIDE; y += 1) {
    for (let x = 0; x < SIDE; x += 1) {
      const v = y * SIDE + x;
      const dir = vertexDir(key, x, y);
      points.set(surfacePoint(figure, dir, heights[2 * v] ?? Number.NaN), POINT_STRIDE * v);
      points.set(surfacePoint(figure, dir, heights[2 * v + 1] ?? Number.NaN), POINT_STRIDE * v + 3);
    }
  }
  if (key.level > 0) {
    // The parent mesh's chord at every vertex not on its grid, from its even neighbours' targets.
    for (let y = 0; y < SIDE; y += 1) {
      for (let x = 0; x < SIDE; x += 1) {
        const oddX = x % 2 === 1;
        const oddY = y % 2 === 1;
        if (!oddX && !oddY) {
          continue;
        }
        const a =
          oddX && !oddY ? y * SIDE + x - 1 : !oddX ? (y - 1) * SIDE + x : (y - 1) * SIDE + x - 1;
        const b =
          oddX && !oddY ? y * SIDE + x + 1 : !oddX ? (y + 1) * SIDE + x : (y + 1) * SIDE + x + 1;
        const v = y * SIDE + x;
        for (let c = 3; c < POINT_STRIDE; c += 1) {
          points[POINT_STRIDE * v + c] =
            0.5 * ((points[POINT_STRIDE * a + c] ?? 0) + (points[POINT_STRIDE * b + c] ?? 0));
        }
      }
    }
  }
  // From the origin: the centre vertex at its own height.
  const origin = POINT_STRIDE * ((PATCH_QUADS / 2) * SIDE + PATCH_QUADS / 2);
  const [ox, oy, oz] = [points[origin] ?? 0, points[origin + 1] ?? 0, points[origin + 2] ?? 0];
  for (let i = 0; i < points.length; i += 3) {
    points[i] = (points[i] ?? 0) - ox;
    points[i + 1] = (points[i + 1] ?? 0) - oy;
    points[i + 2] = (points[i + 2] ?? 0) - oz;
  }
  return points;
}

/** The squared distance of point `v` of `points` from the mix of points `a`, `b` and `c`, m². */
function mixDistanceM2(
  points: Float32Array | Float64Array,
  v: number,
  a: number,
  b: number,
  c: number,
  w0: number,
  w1: number,
  w2: number,
): number {
  const dx =
    (points[v] ?? 0) - (w0 * (points[a] ?? 0) + w1 * (points[b] ?? 0) + w2 * (points[c] ?? 0));
  const dy =
    (points[v + 1] ?? 0) -
    (w0 * (points[a + 1] ?? 0) + w1 * (points[b + 1] ?? 0) + w2 * (points[c + 1] ?? 0));
  const dz =
    (points[v + 2] ?? 0) -
    (w0 * (points[a + 2] ?? 0) + w1 * (points[b + 2] ?? 0) + w2 * (points[c + 2] ?? 0));
  return dx * dx + dy * dy + dz * dz;
}

/**
 * The largest squared distance, m², over the block whose first vertex's numbers start at `base` in
 * `points`, between the vertices `table` names and their stride-s mesh.
 */
function blockErrorM2(
  points: Float32Array | Float64Array,
  base: number,
  table: StrideTable,
): number {
  const { at, weights } = table;
  let worst = 0;
  for (let e = 0; e < table.count; e += 1) {
    worst = Math.max(
      worst,
      mixDistanceM2(
        points,
        base + (at[4 * e] ?? 0),
        base + (at[4 * e + 1] ?? 0),
        base + (at[4 * e + 2] ?? 0),
        base + (at[4 * e + 3] ?? 0),
        weights[3 * e] ?? 0,
        weights[3 * e + 1] ?? 0,
        weights[3 * e + 2] ?? 0,
      ),
    );
  }
  return worst;
}

/**
 * The squared distance, m², between a strip drawn at `stride` and at stride 1 at (t, w), t along the
 * strip in quads and w from its feet (0) to its edge (1): each on the lit mesh's feet's triangles
 * (F_a, F_b, E_a) and (F_b, E_b, E_a), b = a + s, of its edge vertices `edge` and their feet `feet`.
 */
function stripDifferenceM2(
  edge: Float64Array,
  feet: Float64Array,
  stride: number,
  t: number,
  w: number,
): number {
  const a = Math.min(Math.floor(t / stride) * stride, CLUSTER_QUADS - stride);
  const u = (t - a) / stride;
  const lower = u + w <= 1;
  const f = Math.min(Math.floor(t), CLUSTER_QUADS - 1);
  const uf = t - f;
  const lowerFine = uf + w <= 1;
  let d2 = 0;
  for (let n = 0; n < 3; n += 1) {
    const ea = edge[3 * a + n] ?? 0;
    const eb = edge[3 * (a + stride) + n] ?? 0;
    const fa = feet[3 * a + n] ?? 0;
    const fb = feet[3 * (a + stride) + n] ?? 0;
    const coarse = lower
      ? fa + u * (fb - fa) + w * (ea - fa)
      : eb + (1 - u) * (ea - eb) + (1 - w) * (fb - eb);
    const ga = edge[3 * f + n] ?? 0;
    const gb = edge[3 * (f + 1) + n] ?? 0;
    const ha = feet[3 * f + n] ?? 0;
    const hb = feet[3 * (f + 1) + n] ?? 0;
    const fine = lowerFine
      ? ha + uf * (hb - ha) + w * (ga - ha)
      : gb + (1 - uf) * (ga - gb) + (1 - w) * (hb - gb);
    d2 += (coarse - fine) * (coarse - fine);
  }
  return d2;
}

/**
 * The largest squared distance, m², between a strip drawn at `stride` and at stride 1, over the
 * vertices of their common refinement (see the module's remarks).
 */
function stripErrorM2(edge: Float64Array, feet: Float64Array, stride: Stride): number {
  let worst = 0;
  for (let a = 0; a < CLUSTER_QUADS; a += stride) {
    for (let j = 1; j < stride; j += 1) {
      worst = Math.max(
        worst,
        stripDifferenceM2(edge, feet, stride, a + j, 0),
        stripDifferenceM2(edge, feet, stride, a + j, 1),
        // The stride-s diagonal across the stride-1 quads' sides.
        stripDifferenceM2(edge, feet, stride, a + j, 1 - j / stride),
      );
    }
    // And across their diagonals, u = j ÷ (s − 1).
    for (let j = 1; j < stride - 1; j += 1) {
      worst = Math.max(
        worst,
        stripDifferenceM2(
          edge,
          feet,
          stride,
          a + (stride * j) / (stride - 1),
          1 - j / (stride - 1),
        ),
      );
    }
  }
  return worst;
}

/**
 * Each of `bake`'s clusters' e_s for s = 1, 2, 4 and 8, metres, at
 * {@link ERRORS_PER_CLUSTER} × cluster + log₂ s, the clusters in `clusters/terrain.ts`' order:
 * the largest distance between its stride-s and stride-1 meshes over both channels, monotone (see
 * the module's remarks).
 *
 * @param bake - The patch's bake, its heights and offsets as drawn.
 * @param key - The patch's place, which gives each vertex's direction and a foot's normal.
 * @param planet - Its body's geometry, whose figure gives a foot's normal and, without the bake's
 *   offsets, places its vertices.
 * @throws RangeError for a bake whose heights are not 2 × 65² or whose offsets are not 6 × 65².
 */
export function clusterErrors(
  bake: ClusterErrorBake,
  key: PatchKey,
  planet: PlanetGeometry,
): Float64Array {
  if (bake.heights.length !== 2 * VERTICES) {
    throw new RangeError(
      `a bake holds ${String(2 * VERTICES)} heights, two a vertex, not ${String(bake.heights.length)}`,
    );
  }
  if (bake.offsets !== null && bake.offsets.length !== POINT_STRIDE * VERTICES) {
    throw new RangeError(
      `a bake holds ${String(POINT_STRIDE * VERTICES)} offsets, six a vertex, not ${String(bake.offsets.length)}`,
    );
  }
  const errors = new Float64Array(ERRORS_PER_CLUSTER * TERRAIN_CLUSTERS_PER_PAGE);
  const points = drawnPoints(bake, key, planet);
  let reachM2 = 0;
  for (let i = 0; i < points.length; i += 3) {
    const x = points[i] ?? 0;
    const y = points[i + 1] ?? 0;
    const z = points[i + 2] ?? 0;
    reachM2 = Math.max(reachM2, x * x + y * y + z * z);
  }
  for (let cluster = 0; cluster < TERRAIN_BLOCKS; cluster += 1) {
    const bx = cluster % BLOCKS_PER_SIDE;
    const by = Math.floor(cluster / BLOCKS_PER_SIDE);
    const base = POINT_STRIDE * (CLUSTER_QUADS * by * SIDE + CLUSTER_QUADS * bx);
    for (let n = 1; n < STRIDES.length; n += 1) {
      const tables = STRIDE_TABLES[n];
      if (tables !== null && tables !== undefined) {
        const ownM2 = blockErrorM2(points, base, tables.own);
        const morphM2 = blockErrorM2(
          points,
          base,
          key.level === 0 ? tables.rootMorph : tables.morph,
        );
        errors[ERRORS_PER_CLUSTER * cluster + n] = Math.sqrt(Math.max(ownM2, morphM2));
      }
    }
  }
  const { heights, skirtDepthM: d } = bake;
  const figure = planet.figure;
  const edge = new Float64Array(3 * (CLUSTER_QUADS + 1));
  const feet = new Float64Array(3 * (CLUSTER_QUADS + 1));
  const normals = new Float64Array(3 * (CLUSTER_QUADS + 1));
  const vertices = new Int32Array(CLUSTER_QUADS + 1);
  const stripM2 = new Float64Array(STRIDES.length);
  for (let cluster = TERRAIN_BLOCKS; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
    const e = Math.floor((cluster - TERRAIN_BLOCKS) / BLOCKS_PER_SIDE);
    const m = (cluster - TERRAIN_BLOCKS) % BLOCKS_PER_SIDE;
    let bend = 0;
    for (let k = 0; k <= CLUSTER_QUADS; k += 1) {
      const [x, y] = edgeVertex(e, CLUSTER_QUADS * m + k);
      const v = y * SIDE + x;
      vertices[k] = v;
      normals.set(spheroidNormal(figure, vertexDir(key, x, y)), 3 * k);
      const h0 = heights[2 * v] ?? Number.NaN;
      const h1 = heights[2 * v + 1] ?? Number.NaN;
      if ((h0 - d) * (h1 - d) < 0) {
        bend = Math.max(bend, (Math.abs(h0 - d) * Math.abs(h1 - d)) / Math.abs(h1 - h0));
      }
    }
    stripM2.fill(0);
    for (let channel = 0; channel < 2; channel += 1) {
      for (let k = 0; k <= CLUSTER_QUADS; k += 1) {
        const v = vertices[k] ?? 0;
        const h = heights[2 * v + channel] ?? Number.NaN;
        const drop = Math.min(h - d, 0) - h;
        let footM2 = 0;
        for (let n = 0; n < 3; n += 1) {
          const q = points[POINT_STRIDE * v + 3 * channel + n] ?? Number.NaN;
          const foot = q + drop * (normals[3 * k + n] ?? 0);
          edge[3 * k + n] = q;
          feet[3 * k + n] = foot;
          footM2 += foot * foot;
        }
        reachM2 = Math.max(reachM2, footM2);
      }
      for (let n = 1; n < STRIDES.length; n += 1) {
        stripM2[n] = Math.max(stripM2[n] ?? 0, stripErrorM2(edge, feet, STRIDES[n] ?? 1));
      }
    }
    for (let n = 1; n < STRIDES.length; n += 1) {
      errors[ERRORS_PER_CLUSTER * cluster + n] = Math.sqrt(stripM2[n] ?? 0) + 2 * bend;
    }
  }
  const allowanceM = CLUSTER_PAD_M + CLUSTER_PAD_FRACTION * Math.sqrt(reachM2);
  for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
    for (let n = 1; n < STRIDES.length; n += 1) {
      const at = ERRORS_PER_CLUSTER * cluster + n;
      errors[at] = Math.max((errors[at] ?? 0) + allowanceM, errors[at - 1] ?? 0);
    }
  }
  return errors;
}

/** Cluster `cluster`'s e_s at `stride` in {@link clusterErrors}' `errors`, metres. */
export function clusterErrorM(errors: Float64Array, cluster: number, stride: Stride): number {
  return errors[ERRORS_PER_CLUSTER * cluster + STRIDES.indexOf(stride)] ?? Number.NaN;
}

/** The bits of a half float's +∞. */
const F16_INFINITY_BITS = 0x7c00;

/**
 * `errors` rounded up into half floats, each the least at or above its error, +∞ past the largest
 * (65,504) and for a NaN: what a strided record marked measured holds (plan R14, R14.T3.b), so that
 * no packed error is below the error it stands for.
 */
export function packClusterErrors(errors: Float64Array): Float16Array {
  const packed = new Float16Array(errors.length);
  const bits = new Uint16Array(packed.buffer);
  for (let i = 0; i < errors.length; i += 1) {
    const e = errors[i] ?? Number.NaN;
    if (Number.isNaN(e)) {
      bits[i] = F16_INFINITY_BITS;
      continue;
    }
    packed[i] = e;
    // Every error is at least 0, so the next half float up is the next bit pattern.
    if ((packed[i] ?? 0) < e) {
      bits[i] = (bits[i] ?? 0) + 1;
    }
  }
  return packed;
}
