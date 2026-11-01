/**
 * The cluster path's templates (plan R14, R14.T1.b): the index data a cluster shape is drawn with
 * at each stride, and the stride's error scale, in one mesh a consumer draws every bucket of with
 * `drawIndexedIndirect`, each bucket's arguments naming its template's index range.
 *
 * @remarks
 * Terrain's two shapes (plan R14's design note on the cluster format):
 * - **The block**: 8 × 8 quads of a patch's 65-grid, at stride s every s-th vertex, its quads split
 *   on their (0, 0)–(1, 1) diagonal as the lit mesh's are (`gpu/resources.ts`' `patchMeshData`),
 *   in its order and winding: 128, 32, 8 and 2 triangles at strides 1, 2, 4 and 8.
 * - **The skirt-foot strip**: 8 quads of an edge's skirt feet, each quad's two triangles (p′, q′, p)
 *   and (q′, q, p) as the lit mesh's feet are, p and q its edge vertices taken anticlockwise and p′
 *   and q′ their feet: 16, 8, 4 and 2 triangles. A foot is drawn once, in one winding (the brief's
 *   32 / 16 / 8 / 4 counted the heads with the feet, which only the edge patches draw).
 *
 * The mesh's `position` attribute carries each vertex's place in its cluster and its lit-mesh code
 * (`gpu/resources.ts`' `LIT_VERTEX_CODE`): a block's (lx, ly, 0), lx and ly 0 to 8; a strip's
 * (k, 0, 1) for a foot and (k, 0, 0) for its edge vertex, k 0 to 8 along the edge. The consumer's
 * vertex stage turns them into the lit mesh's own grid coordinates, (x₀ + lx, y₀ + ly, 0) for a
 * block and edge e's k-th vertex from the strip's start for a strip, and places them by `lit_place`
 * unchanged (`clusters/terrain.ts`, `terrainCluster.wgsl`).
 */

import type { MeshSpec } from "../engine/types";
import type { ClusterShape } from "./record";

/** A template's stride: every s-th vertex of its cluster's grid. */
export type Stride = 1 | 2 | 4 | 8;

/** The strides, finest first: a template's index within its shape is log₂ s. */
export const STRIDES: readonly [Stride, Stride, Stride, Stride] = [1, 2, 4, 8];

/** One template: a shape at a stride, as an index range of its consumer's cluster mesh. */
export interface ClusterTemplate {
  readonly shape: ClusterShape;
  readonly stride: Stride;
  /** Its triangles: a block's 128 ÷ s², a strip's 16 ÷ s. */
  readonly triangles: number;
  /** Its first index in the cluster mesh's index buffer, `drawIndexedIndirect`'s `firstIndex`. */
  readonly firstIndex: number;
  /** Its index count, three a triangle. */
  readonly indexCount: number;
  /** The stride's error in the record's unit: s, the stride's quad side in the unit spacing. */
  readonly errorScale: number;
}

/** Vertices of a block's 9 × 9 grid in the terrain cluster mesh, (lx, ly) at 9 ly + lx. */
const BLOCK_VERTICES = 81;

/** Quads along a block's or a strip's side. */
export const CLUSTER_QUADS = 8;

/** The terrain cluster mesh's index of a strip's foot at k, then of its edge vertex at k. */
function stripFoot(k: number): number {
  return BLOCK_VERTICES + k;
}

function stripEdge(k: number): number {
  return BLOCK_VERTICES + CLUSTER_QUADS + 1 + k;
}

/** A block's triangles at `stride`, as the lit mesh's quads, row by row from y = 0. */
function blockIndices(stride: Stride): number[] {
  const at = (x: number, y: number): number => y * (CLUSTER_QUADS + 1) + x;
  const indices: number[] = [];
  for (let y = 0; y < CLUSTER_QUADS; y += stride) {
    for (let x = 0; x < CLUSTER_QUADS; x += stride) {
      indices.push(
        at(x, y),
        at(x + stride, y),
        at(x + stride, y + stride),
        at(x, y),
        at(x + stride, y + stride),
        at(x, y + stride),
      );
    }
  }
  return indices;
}

/** A strip's triangles at `stride`, as the lit mesh's feet, anticlockwise along its edge. */
function stripIndices(stride: Stride): number[] {
  const indices: number[] = [];
  for (let k = 0; k < CLUSTER_QUADS; k += stride) {
    const next = k + stride;
    indices.push(
      stripFoot(k),
      stripFoot(next),
      stripEdge(k),
      stripFoot(next),
      stripEdge(next),
      stripEdge(k),
    );
  }
  return indices;
}

/** Terrain's cluster mesh and its eight templates, made once. */
export interface TerrainClusterMesh {
  /** Blocks at strides 1, 2, 4 and 8, then strips at the same: template t is shape ⌊t ÷ 4⌋ at stride 2^(t mod 4). */
  readonly templates: ReadonlyArray<ClusterTemplate>;
  readonly mesh: MeshSpec;
}

/** Terrain's cluster mesh: a block's 81 vertices, a strip's 18, and every template's indices. */
export function terrainClusterMesh(): TerrainClusterMesh {
  const positions = new Float32Array(3 * (BLOCK_VERTICES + 2 * (CLUSTER_QUADS + 1)));
  for (let ly = 0; ly <= CLUSTER_QUADS; ly += 1) {
    for (let lx = 0; lx <= CLUSTER_QUADS; lx += 1) {
      positions.set([lx, ly, 0], 3 * (ly * (CLUSTER_QUADS + 1) + lx));
    }
  }
  for (let k = 0; k <= CLUSTER_QUADS; k += 1) {
    positions.set([k, 0, 1], 3 * stripFoot(k));
    positions.set([k, 0, 0], 3 * stripEdge(k));
  }
  const indices: number[] = [];
  const templates: ClusterTemplate[] = [];
  for (const [shape, of] of [
    ["terrain-block", blockIndices],
    ["terrain-skirt", stripIndices],
  ] as const) {
    for (const stride of STRIDES) {
      const range = of(stride);
      templates.push({
        shape,
        stride,
        triangles: range.length / 3,
        firstIndex: indices.length,
        indexCount: range.length,
        errorScale: stride,
      });
      indices.push(...range);
    }
  }
  return {
    templates,
    mesh: {
      name: "terrain clusters",
      positions,
      indices: Uint32Array.from(indices),
      topology: "triangle-list",
      attributes: {},
    },
  };
}

/** The template index of `shape` at `stride` in {@link terrainClusterMesh}'s order. */
export function terrainTemplateIndex(
  shape: "terrain-block" | "terrain-skirt",
  stride: Stride,
): number {
  return (shape === "terrain-block" ? 0 : STRIDES.length) + STRIDES.indexOf(stride);
}
