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
 *
 * Terrain's edge patches' shapes (plan R14, R14.T1.g), each {@link FIN_RUN_QUADS} quads along a
 * line of the lit mesh's, in its order and vertex order, at stride 1 alone, p and q a quad's grid
 * vertices along the run and p′ and q′ their datum or head vertices:
 * - **The fin run** (`terrain-fin`), a survey fin's quads, each (p′, q′, q), (p′, q, p), (p′, q, q′)
 *   and (p′, p, q): 128 triangles.
 * - **The head run** (`terrain-head`), a skirt's head strip's quads, each (p″, p, q) and
 *   (p″, q, q″): 64 triangles.
 *
 * Their vertices are (k, 0, 0) for a grid vertex and (k, 0, 1) for its datum or head vertex, k 0
 * to 32.
 *
 * Every terrain shape has a **may-discard** template at stride 1 after the eight strided ones (the
 * block's, the strip's, the fin run's and the head run's, {@link TERRAIN_DISCARD_TEMPLATES}), drawn
 * by the consumer's form with the survey discard: an edge patch's clusters that the survey's edge
 * comes near (`clusters/terrain.ts`).
 *
 * The mesh shape (plan R14, R14.T4.b) has one template, {@link meshTemplate}: 128 triangles drawn
 * by index pulling, since a mesh's cluster holds its own triangles' corners or 8-bit indices in
 * storage, which WebGPU's index formats cannot draw (plan R14's design note on the cluster format).
 * Its mesh's vertex k is (k mod 3, ⌊k ÷ 3⌋, 0): a corner of triangle ⌊k ÷ 3⌋, which the consumer's
 * vertex stage reads from its page, or places off the view, a degenerate triangle, past the
 * cluster's count. So a cluster costs 384 vertex invocations however few triangles it holds.
 */

import type { MeshSpec } from "../engine/types";
import { NO_DISCARD_TEMPLATE } from "./cullPass";
import { CLUSTER_SHAPE_CODE, type ClusterShape, MESH_CLUSTER_TRIANGLES } from "./record";

import { CLUSTER_QUADS, FIN_RUN_QUADS, type Stride, STRIDES } from "./terrainGrid";

export { CLUSTER_QUADS, FIN_RUN_QUADS, type Stride, STRIDES } from "./terrainGrid";

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
  /**
   * Whether its bucket holds may-discard clusters, drawn by the consumer's discarding form (plan
   * R14, R14.T1.g; `clusters/buckets.ts`' `drawItems`).
   */
  readonly mayDiscard: boolean;
}

/**
 * Terrain's may-discard template of each shape code, `record.ts`' `CLUSTER_SHAPE_CODE` 0 to 7, in
 * {@link terrainClusterMesh}'s order (plan R14, R14.T1.g): the block's, the strip's, none for the
 * mesh shape, the fin run's, the head run's, and none for the codes no shape has; the cull's
 * `CullParams.discardTemplates`.
 */
export const TERRAIN_DISCARD_TEMPLATES: ReadonlyArray<number> = [
  8,
  9,
  NO_DISCARD_TEMPLATE,
  10,
  11,
  NO_DISCARD_TEMPLATE,
  NO_DISCARD_TEMPLATE,
  NO_DISCARD_TEMPLATE,
];

/** Vertices of a block's 9 × 9 grid in the terrain cluster mesh, (lx, ly) at 9 ly + lx. */
const BLOCK_VERTICES = 81;

/** The terrain cluster mesh's index of a strip's foot at k, then of its edge vertex at k. */
function stripFoot(k: number): number {
  return BLOCK_VERTICES + k;
}

function stripEdge(k: number): number {
  return BLOCK_VERTICES + CLUSTER_QUADS + 1 + k;
}

/** The terrain cluster mesh's index of a block's vertex (x, y). */
function blockVertex(x: number, y: number): number {
  return y * (CLUSTER_QUADS + 1) + x;
}

/** A block's triangles at `stride`, as the lit mesh's quads, row by row from y = 0. */
function blockIndices(stride: Stride): number[] {
  const indices: number[] = [];
  for (let y = 0; y < CLUSTER_QUADS; y += stride) {
    for (let x = 0; x < CLUSTER_QUADS; x += stride) {
      indices.push(
        blockVertex(x, y),
        blockVertex(x + stride, y),
        blockVertex(x + stride, y + stride),
        blockVertex(x, y),
        blockVertex(x + stride, y + stride),
        blockVertex(x, y + stride),
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

/**
 * A strip's triangles at `stride` as R05's patch mesh's skirts (`gpu/resources.ts`'
 * `patchMeshData`): each quad under edge vertices p then q is (p, p′, q′) and (p, q′, q), the lit
 * mesh's feet's triangles in the same winding, each begun at its edge vertex.
 */
function patchStripIndices(stride: Stride): number[] {
  const indices: number[] = [];
  for (let k = 0; k < CLUSTER_QUADS; k += stride) {
    const next = k + stride;
    indices.push(
      stripEdge(k),
      stripFoot(k),
      stripFoot(next),
      stripEdge(k),
      stripFoot(next),
      stripEdge(next),
    );
  }
  return indices;
}

/** The first of the fin run's vertices in terrain's cluster mesh: after a block's 81 and a strip's 18. */
const FIN_FIRST_VERTEX = BLOCK_VERTICES + 2 * (CLUSTER_QUADS + 1);

/** The fin run's vertices along it. */
const FIN_RUN_VERTICES = FIN_RUN_QUADS + 1;

/** Terrain's cluster mesh's index of a fin or head run's vertex at k of `kind`: 0 grid, 1 datum. */
function finVertex(kind: 0 | 1, k: number): number {
  return FIN_FIRST_VERTEX + kind * FIN_RUN_VERTICES + k;
}

/** A fin run's triangles, the lit mesh's fin quads' (see the module's remarks), along the run. */
function finIndices(): number[] {
  const indices: number[] = [];
  for (let k = 0; k < FIN_RUN_QUADS; k += 1) {
    const next = k + 1;
    indices.push(
      finVertex(1, k),
      finVertex(1, next),
      finVertex(0, next),
      finVertex(1, k),
      finVertex(0, next),
      finVertex(0, k),
      finVertex(1, k),
      finVertex(0, next),
      finVertex(1, next),
      finVertex(1, k),
      finVertex(0, k),
      finVertex(0, next),
    );
  }
  return indices;
}

/** A head run's triangles, the lit mesh's head quads' (see the module's remarks), along the run. */
function headIndices(): number[] {
  const indices: number[] = [];
  for (let k = 0; k < FIN_RUN_QUADS; k += 1) {
    const next = k + 1;
    indices.push(
      finVertex(1, k),
      finVertex(0, k),
      finVertex(0, next),
      finVertex(1, k),
      finVertex(0, next),
      finVertex(1, next),
    );
  }
  return indices;
}

/** Terrain's cluster mesh and its templates, made once. */
export interface TerrainClusterMesh {
  /**
   * Blocks at strides 1, 2, 4 and 8, then strips at the same: template t below 8 is shape ⌊t ÷ 4⌋
   * at stride 2^(t mod 4); terrain's then has its may-discard templates
   * ({@link TERRAIN_DISCARD_TEMPLATES}).
   */
  readonly templates: ReadonlyArray<ClusterTemplate>;
  readonly mesh: MeshSpec;
}

/**
 * Terrain's cluster mesh: a block's 81 vertices, a strip's 18 and a fin or head run's 66, the eight
 * strided templates' indices, then the may-discard templates' at stride 1, the block's, the strip's,
 * the fin run's and the head run's (plan R14, R14.T1.g).
 */
export function terrainClusterMesh(): TerrainClusterMesh {
  const strided = stridedClusterMesh("terrain clusters", stripIndices, 2 * FIN_RUN_VERTICES);
  const positions = strided.mesh.positions;
  for (let k = 0; k <= FIN_RUN_QUADS; k += 1) {
    for (const kind of [0, 1] as const) {
      positions.set([k, 0, kind], 3 * finVertex(kind, k));
    }
  }
  const indices = Array.from(strided.mesh.indices ?? []);
  const templates = [...strided.templates];
  const discarding = (shape: ClusterShape, range: ReadonlyArray<number>): void => {
    templates.push({
      shape,
      stride: 1,
      triangles: range.length / 3,
      firstIndex: indices.length,
      indexCount: range.length,
      errorScale: 1,
      mayDiscard: true,
    });
    indices.push(...range);
  };
  discarding("terrain-block", blockIndices(1));
  discarding("terrain-skirt", stripIndices(1));
  discarding("terrain-fin", finIndices());
  discarding("terrain-head", headIndices());
  return {
    templates,
    mesh: { ...strided.mesh, positions, indices: Uint32Array.from(indices) },
  };
}

/**
 * The mesh regime's figure's cluster mesh (plan R14, R14.T4.a; `bodies/smoothMesh.ts`): terrain's
 * vertices and blocks, its strips R05's patch mesh's skirts in that mesh's vertex order
 * ({@link patchStripIndices}), so that each template triangle is the figure's instanced draw's own,
 * vertex for vertex.
 */
export function smoothFigureClusterMesh(): TerrainClusterMesh {
  return stridedClusterMesh("figure clusters", patchStripIndices);
}

/**
 * A strided cluster mesh named `name`: the block's and the strip's vertices, its strips by `strips`,
 * and room for `extraVertices` more after them, zeros.
 */
function stridedClusterMesh(
  name: string,
  strips: (stride: Stride) => number[],
  extraVertices = 0,
): TerrainClusterMesh {
  const positions = new Float32Array(3 * (FIN_FIRST_VERTEX + extraVertices));
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
    ["terrain-skirt", strips],
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
        mayDiscard: false,
      });
      indices.push(...range);
    }
  }
  return {
    templates,
    mesh: {
      name,
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

/** The may-discard template index of a terrain `shape` in {@link terrainClusterMesh}'s order. */
export function terrainDiscardTemplateIndex(
  shape: "terrain-block" | "terrain-skirt" | "terrain-fin" | "terrain-head",
): number {
  return TERRAIN_DISCARD_TEMPLATES[CLUSTER_SHAPE_CODE[shape]] ?? NO_DISCARD_TEMPLATE;
}

/** The mesh shape's one template (plan R14, R14.T4.b): 128 triangles by index pulling. */
export function meshTemplate(): ClusterTemplate {
  return {
    shape: "mesh",
    stride: 1,
    triangles: MESH_CLUSTER_TRIANGLES,
    firstIndex: 0,
    indexCount: 3 * MESH_CLUSTER_TRIANGLES,
    errorScale: 1,
    mayDiscard: false,
  };
}

/** The mesh shape's cluster mesh: {@link meshTemplate}'s 384 vertices, (corner, triangle, 0), in order. */
export function meshClusterMesh(): {
  readonly templates: readonly [ClusterTemplate];
  readonly mesh: MeshSpec;
} {
  const vertices = 3 * MESH_CLUSTER_TRIANGLES;
  const positions = new Float32Array(3 * vertices);
  for (let k = 0; k < vertices; k += 1) {
    positions.set([k % 3, Math.floor(k / 3), 0], 3 * k);
  }
  return {
    templates: [meshTemplate()],
    mesh: {
      name: "mesh clusters",
      positions,
      indices: Uint32Array.from({ length: vertices }, (_, k) => k),
      topology: "triangle-list",
      attributes: {},
    },
  };
}
