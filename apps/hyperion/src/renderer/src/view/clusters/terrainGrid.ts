/**
 * A terrain cluster page's grid (plan R14, R14.T1.b; read by the height worker from R14.T3.b): the
 * strides, a cluster's quads, a patch's clusters, the skirt edges' vertex order and the padding for
 * the GPU's `f32` placement, in a module that imports nothing of the engine, so that the worker's
 * `clusters/errors.ts` may read them (`tsconfig.worker.json`). `templates.ts` and `terrain.ts`
 * re-export them where they were first made.
 */

/** A template's stride: every s-th vertex of its cluster's grid. */
export type Stride = 1 | 2 | 4 | 8;

/** The strides, finest first: a template's index within its shape is log₂ s. */
export const STRIDES: readonly [Stride, Stride, Stride, Stride] = [1, 2, 4, 8];

/** Quads along a block's or a strip's side. */
export const CLUSTER_QUADS = 8;

/** A terrain patch's clusters: its 64 blocks, then its 32 skirt-foot strips. */
export const TERRAIN_CLUSTERS_PER_PAGE = 96;

/** Its blocks: 8 × 8, row by row from y = 0. */
export const TERRAIN_BLOCKS = 64;

/** Quads along a patch's side. */
const PATCH_QUADS = 64;

/** The least padding of a cluster's radius, m: R05's 1 mm bound on the GPU's placement, ten times. */
export const CLUSTER_PAD_M = 0.01;

/**
 * The padding of a cluster's radius per metre of its sphere's reach from the page's origin: some
 * sixteen `f32` ulps (2⁻²⁴ each), for the narrowed offsets, the record's narrowed centre and the
 * vertex arithmetic's rounding.
 */
export const CLUSTER_PAD_FRACTION = 1e-6;

/** The grid vertex that is the `k`-th of edge `edge`, anticlockwise (`gpu/resources.ts`' own). */
export function edgeVertex(edge: number, k: number): readonly [number, number] {
  switch (edge) {
    case 0:
      return [k, 0];
    case 1:
      return [PATCH_QUADS, k];
    case 2:
      return [PATCH_QUADS - k, PATCH_QUADS];
    default:
      return [0, PATCH_QUADS - k];
  }
}
