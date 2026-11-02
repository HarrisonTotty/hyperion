/**
 * The cluster path's measured errors (plan R14, R14.T3.a, the height worker's output from T3.b;
 * Design note 3): e_s of each of a terrain patch's 96 clusters at strides 1, 2, 4 and 8, the largest
 * distance between the cluster drawn at stride s and drawn at stride 1, metres, in `f64`.
 *
 * @remarks
 * - **What is drawn.** The lit mesh places a grid vertex at mix(q₀, q₁, k), its own offset from the
 *   patch origin and its morph target's (`terrainPatch.wgsl`' `patchVertexOf`), and hangs a skirt
 *   foot from it along the datum's normal ν to the height min(h − d, 0), h = mix(h₀, h₁, k) and d
 *   the slot's skirt depth (`terrainSurvey.wgsl`' `lit_place`). On `baked-offsets` q₀ and q₁ are the
 *   bake's own, as narrowed; on `face-differences` they are the bake's heights placed in `f64` by
 *   the bake's own rule (`hyperion_surface::patch`): P = M·d + h ν at each vertex's own height, and
 *   for q₁ at its morph height where both indices are even or on level 0, else the mean of its two
 *   even neighbours on the parent mesh's diagonal. The GPU's `f32` placement strays from these by
 *   a few ulps of each vertex's offset from the origin, which e_s adds as the cluster's record pads
 *   its sphere for it ({@link CLUSTER_PAD_M} and {@link CLUSTER_PAD_FRACTION} of the cluster's
 *   reach from the origin), so that e_s bounds the meshes as the GPU draws them.
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
 *
 * A pure module of the render thread and the height worker alike: T3.b calls it on each bake and
 * rounds the result up into the record's `f16`s.
 */

import { PATCH_QUADS, vertexDir } from "../terrain/cube";
import type { PatchKey } from "../terrain/patchKey";
import { type PlanetGeometry, spheroidNormal, surfacePoint } from "../terrain/planet";
import { PATCH_VERTICES_PER_SIDE } from "../terrain/slotLayout";
import {
  CLUSTER_PAD_FRACTION,
  CLUSTER_PAD_M,
  edgeVertex,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
} from "./terrain";
import { CLUSTER_QUADS, type Stride, STRIDES } from "./templates";

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
const SIDE = PATCH_VERTICES_PER_SIDE;

/** Vertices of a patch. */
const VERTICES = SIDE * SIDE;

/** A patch's blocks along its side. */
const BLOCKS_PER_SIDE = PATCH_QUADS / CLUSTER_QUADS;

/** The vertices of one channel or of the feet, three numbers each, metres. */
type Points = Float64Array;

/**
 * Each vertex's q₀ and q₁ of `bake` of `key`, metres, from a common origin (see the module's
 * remarks).
 */
function drawnOffsets(
  bake: ClusterErrorBake,
  key: PatchKey,
  planet: PlanetGeometry,
): { readonly own: Points; readonly morph: Points } {
  const own = new Float64Array(3 * VERTICES);
  const morph = new Float64Array(3 * VERTICES);
  const { offsets, heights } = bake;
  if (offsets !== null) {
    for (let v = 0; v < VERTICES; v += 1) {
      for (let c = 0; c < 3; c += 1) {
        own[3 * v + c] = offsets[6 * v + c] ?? Number.NaN;
        morph[3 * v + c] = offsets[6 * v + 3 + c] ?? Number.NaN;
      }
    }
    return { own, morph };
  }
  const figure = planet.figure;
  for (let y = 0; y < SIDE; y += 1) {
    for (let x = 0; x < SIDE; x += 1) {
      const v = y * SIDE + x;
      const dir = vertexDir(key, x, y);
      own.set(surfacePoint(figure, dir, heights[2 * v] ?? Number.NaN), 3 * v);
      morph.set(surfacePoint(figure, dir, heights[2 * v + 1] ?? Number.NaN), 3 * v);
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
        for (let c = 0; c < 3; c += 1) {
          morph[3 * v + c] = 0.5 * ((morph[3 * a + c] ?? 0) + (morph[3 * b + c] ?? 0));
        }
      }
    }
  }
  return { own, morph };
}

/** |p − q| of point `i` of `p` and the point (qx, qy, qz). */
function distanceTo(p: Points, i: number, qx: number, qy: number, qz: number): number {
  return Math.hypot((p[3 * i] ?? 0) - qx, (p[3 * i + 1] ?? 0) - qy, (p[3 * i + 2] ?? 0) - qz);
}

/** The largest distance of `p`'s points `indices` from point `at` of `origin`, metres. */
function reachOf(p: Points, indices: Iterable<number>, origin: Points, at: number): number {
  let reach = 0;
  for (const i of indices) {
    reach = Math.max(
      reach,
      distanceTo(p, i, origin[3 * at] ?? 0, origin[3 * at + 1] ?? 0, origin[3 * at + 2] ?? 0),
    );
  }
  return reach;
}

/** The `f32` placement's allowance for a cluster whose points reach `reachM` from the origin, m. */
function placementAllowanceM(reachM: number): number {
  return CLUSTER_PAD_M + CLUSTER_PAD_FRACTION * reachM;
}

/** The vertex indices of block (`bx`, `by`) in the 65-grid. */
function* blockVertices(bx: number, by: number): Generator<number> {
  for (let ly = 0; ly <= CLUSTER_QUADS; ly += 1) {
    for (let lx = 0; lx <= CLUSTER_QUADS; lx += 1) {
      yield (CLUSTER_QUADS * by + ly) * SIDE + CLUSTER_QUADS * bx + lx;
    }
  }
}

/** 0 to {@link CLUSTER_QUADS}: a strip's points. */
const STRIP_POINTS = Array.from({ length: CLUSTER_QUADS + 1 }, (_, k) => k);

/**
 * The largest distance, over block (`bx`, `by`)'s stride-1 vertices, between `points` and their
 * stride-`stride` mesh.
 */
function blockError(points: Points, bx: number, by: number, stride: Stride): number {
  const x0 = CLUSTER_QUADS * bx;
  const y0 = CLUSTER_QUADS * by;
  const at = (lx: number, ly: number): number => (y0 + ly) * SIDE + x0 + lx;
  let worst = 0;
  for (let ly = 0; ly <= CLUSTER_QUADS; ly += 1) {
    const qy = Math.min(Math.floor(ly / stride) * stride, CLUSTER_QUADS - stride);
    const fy = (ly - qy) / stride;
    for (let lx = 0; lx <= CLUSTER_QUADS; lx += 1) {
      if (lx % stride === 0 && ly % stride === 0) {
        continue;
      }
      const qx = Math.min(Math.floor(lx / stride) * stride, CLUSTER_QUADS - stride);
      const fx = (lx - qx) / stride;
      // The stride-s triangle holding it: (0, 0), (s, 0), (s, s) where fx ≥ fy, else (0, 0),
      // (0, s), (s, s), mixed by 1 − max(fx, fy), |fx − fy| and min(fx, fy).
      const a = at(qx, qy);
      const b = fx >= fy ? at(qx + stride, qy) : at(qx, qy + stride);
      const c = at(qx + stride, qy + stride);
      const w0 = 1 - Math.max(fx, fy);
      const w1 = Math.abs(fx - fy);
      const w2 = Math.min(fx, fy);
      const p = (n: number): number =>
        w0 * (points[3 * a + n] ?? 0) +
        w1 * (points[3 * b + n] ?? 0) +
        w2 * (points[3 * c + n] ?? 0);
      worst = Math.max(worst, distanceTo(points, at(lx, ly), p(0), p(1), p(2)));
    }
  }
  return worst;
}

/**
 * The point at (t, w) of a strip drawn at `stride` from its edge vertices `edge` and their feet
 * `feet`, t along the strip in quads and w from its feet (0) to its edge (1), on the lit mesh's
 * feet's triangles (F_a, F_b, E_a) and (F_b, E_b, E_a), b = a + s.
 */
function stripPoint(
  edge: Points,
  feet: Points,
  stride: Stride,
  t: number,
  w: number,
  out: Float64Array,
): void {
  const a = Math.min(Math.floor(t / stride) * stride, CLUSTER_QUADS - stride);
  const b = a + stride;
  const u = (t - a) / stride;
  for (let n = 0; n < 3; n += 1) {
    const ea = edge[3 * a + n] ?? 0;
    const eb = edge[3 * b + n] ?? 0;
    const fa = feet[3 * a + n] ?? 0;
    const fb = feet[3 * b + n] ?? 0;
    out[n] =
      u + w <= 1
        ? fa + u * (fb - fa) + w * (ea - fa)
        : eb + (1 - u) * (ea - eb) + (1 - w) * (fb - eb);
  }
}

/**
 * The largest distance between a strip drawn at `stride` and at stride 1, over the vertices of
 * their common refinement (see the module's remarks).
 */
function stripError(edge: Points, feet: Points, stride: Stride): number {
  const coarse = new Float64Array(3);
  const fine = new Float64Array(3);
  let worst = 0;
  const measure = (t: number, w: number): void => {
    stripPoint(edge, feet, stride, t, w, coarse);
    stripPoint(edge, feet, 1, t, w, fine);
    worst = Math.max(
      worst,
      Math.hypot(
        (coarse[0] ?? 0) - (fine[0] ?? 0),
        (coarse[1] ?? 0) - (fine[1] ?? 0),
        (coarse[2] ?? 0) - (fine[2] ?? 0),
      ),
    );
  };
  for (let a = 0; a < CLUSTER_QUADS; a += stride) {
    for (let j = 1; j < stride; j += 1) {
      measure(a + j, 0);
      measure(a + j, 1);
      // The stride-s diagonal across the stride-1 quads' sides.
      measure(a + j, 1 - j / stride);
    }
    // And across their diagonals, u = j ÷ (s − 1).
    for (let j = 1; j < stride - 1; j += 1) {
      measure(a + (stride * j) / (stride - 1), 1 - j / (stride - 1));
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
 * @param planet - Its body's geometry, whose figure places the vertices on `face-differences`.
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
  if (bake.offsets !== null && bake.offsets.length !== 6 * VERTICES) {
    throw new RangeError(
      `a bake holds ${String(6 * VERTICES)} offsets, six a vertex, not ${String(bake.offsets.length)}`,
    );
  }
  const errors = new Float64Array(ERRORS_PER_CLUSTER * TERRAIN_CLUSTERS_PER_PAGE);
  const channels = drawnOffsets(bake, key, planet);
  const both = [channels.own, channels.morph] as const;
  // The patch origin: its centre vertex at its own height.
  const origin = (PATCH_QUADS / 2) * SIDE + PATCH_QUADS / 2;
  for (let cluster = 0; cluster < TERRAIN_BLOCKS; cluster += 1) {
    const bx = cluster % BLOCKS_PER_SIDE;
    const by = Math.floor(cluster / BLOCKS_PER_SIDE);
    const allowanceM = placementAllowanceM(
      Math.max(
        ...both.map((points) => reachOf(points, blockVertices(bx, by), channels.own, origin)),
      ),
    );
    for (let n = 1; n < STRIDES.length; n += 1) {
      const stride = STRIDES[n] ?? 1;
      errors[ERRORS_PER_CLUSTER * cluster + n] =
        Math.max(...both.map((points) => blockError(points, bx, by, stride))) + allowanceM;
    }
  }
  const figure = planet.figure;
  const d = bake.skirtDepthM;
  const edge = new Float64Array(3 * (CLUSTER_QUADS + 1));
  const feet = new Float64Array(3 * (CLUSTER_QUADS + 1));
  for (let cluster = TERRAIN_BLOCKS; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
    const e = Math.floor((cluster - TERRAIN_BLOCKS) / BLOCKS_PER_SIDE);
    const m = (cluster - TERRAIN_BLOCKS) % BLOCKS_PER_SIDE;
    let bend = 0;
    let reachM = 0;
    const strides = [0, 0, 0, 0];
    for (let channel = 0; channel < 2; channel += 1) {
      const points = both[channel] ?? channels.own;
      for (let k = 0; k <= CLUSTER_QUADS; k += 1) {
        const [x, y] = edgeVertex(e, CLUSTER_QUADS * m + k);
        const v = y * SIDE + x;
        const h0 = bake.heights[2 * v] ?? Number.NaN;
        const h1 = bake.heights[2 * v + 1] ?? Number.NaN;
        const h = channel === 0 ? h0 : h1;
        const drop = Math.min(h - d, 0) - h;
        const nu = spheroidNormal(figure, vertexDir(key, x, y));
        for (let n = 0; n < 3; n += 1) {
          const q = points[3 * v + n] ?? Number.NaN;
          edge[3 * k + n] = q;
          feet[3 * k + n] = q + drop * (nu[n] ?? 0);
        }
        if ((h0 - d) * (h1 - d) < 0) {
          bend = Math.max(bend, (Math.abs(h0 - d) * Math.abs(h1 - d)) / Math.abs(h1 - h0));
        }
      }
      for (let n = 1; n < STRIDES.length; n += 1) {
        strides[n] = Math.max(strides[n] ?? 0, stripError(edge, feet, STRIDES[n] ?? 1));
      }
      reachM = Math.max(
        reachM,
        reachOf(edge, STRIP_POINTS, channels.own, origin),
        reachOf(feet, STRIP_POINTS, channels.own, origin),
      );
    }
    for (let n = 1; n < STRIDES.length; n += 1) {
      errors[ERRORS_PER_CLUSTER * cluster + n] =
        (strides[n] ?? 0) + 2 * bend + placementAllowanceM(reachM);
    }
  }
  for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
    for (let n = 1; n < STRIDES.length; n += 1) {
      const at = ERRORS_PER_CLUSTER * cluster + n;
      errors[at] = Math.max(errors[at] ?? 0, errors[at - 1] ?? 0);
    }
  }
  return errors;
}

/** Cluster `cluster`'s e_s at `stride` in {@link clusterErrors}' `errors`, metres. */
export function clusterErrorM(errors: Float64Array, cluster: number, stride: Stride): number {
  return errors[ERRORS_PER_CLUSTER * cluster + STRIDES.indexOf(stride)] ?? Number.NaN;
}
