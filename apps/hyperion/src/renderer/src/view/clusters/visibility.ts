/**
 * The visibility buffer's ID raster (plan R14, R14.T2.c; Design note 9): the cluster path's lit
 * buckets drawn once into the scene's depth and an `r32uint` ID a pixel, which the resolve (T2.d)
 * shades once a pixel.
 *
 * @remarks
 * - **The ID**: (visible-cluster index << {@link VISIBILITY_TRIANGLE_BITS}) | triangle, where the
 *   visible-cluster index is the bucket entry's index in its view's index space ({@link
 *   VisibleConsumerRange}) and the triangle its template's, at most 128 a cluster (Design note 2);
 *   {@link ID_NONE}, `0xffff_ffff`, where no cluster drew, which the target clears to each frame and
 *   which no ID reaches: an index is below {@link MAX_LIST_ENTRIES}, 2²⁴, so an ID below 2³¹.
 * - **The two paths** (Design note 6): where the device has `primitive-index`, the raster draws
 *   the consumer's own cluster mesh and its fragment takes the triangle from
 *   `@builtin(primitive_index)`, which counts a draw's primitives from 0 for each instance (W3C
 *   WGSL, _recalled_), and a bucket draw's instance is one cluster; the vertex stage hands it the
 *   visible-cluster index, `instance_index` being the vertex stage's alone. Without it the raster
 *   draws the mesh de-indexed ({@link deindexedClusterMesh}): each index its own vertex, the
 *   template's triangle in its `position`'s z beside the lit-mesh code, so that the bucket's
 *   `drawIndexedIndirect` arguments draw it unchanged and its vertex stage writes the ID, at three
 *   vertex invocations a triangle. Each passes one flat `u32` beside the clip position: 20 B a
 *   vertex either way.
 * - **One index space a view, shared** (Design note 9; lean, 2026-11-04): the visible-cluster index
 *   is a bucket entry's index in its consumer's lists, its draw's `clusterBase` plus its instance,
 *   past its range's first index ({@link VISIBLE_FIRST_UNIFORM}), each consumer's lists a range of
 *   the space: each body's terrain's whole lists buffer, all its passes' (the lit pass's from
 *   `CLUSTER_LIT_PASS × passEntries`), the first from 0 and each other after the one before
 *   (R14.T2.d; {@link visibleConsumerRanges}). An entry is the one `u32` its lists hold,
 *   (instance << 8) | cluster, so that a consumer is its range rather than a word of its entry, and
 *   an entry's template is its bucket's, found from its index (`resolve.ts`). A resolve refuses the
 *   IDs outside its range, so that each body's shades its own pixels alone, and a later consumer
 *   (T5.e) takes the range after the last, a pass and not a format.
 */

import { ID_NONE } from "../engine/idTarget";
import type {
  DrawItem,
  MeshSpec,
  RenderEngine,
  RenderTarget,
  TextureHandle,
  ViewSize,
} from "../engine/types";
import { MAX_LIST_ENTRIES } from "./buckets";
import type { ClusterTemplate } from "./templates";

export { ID_NONE } from "../engine/idTarget";

/**
 * The ID raster's pass label (plan R14, R14.T2.c): the lit pass's opaque buckets into the view's
 * visibility target with the scene's depth, after the bodies pass's figures.
 */
export const VISIBILITY_RASTER_LABEL = "terrain id raster";

/**
 * The late ID raster's pass label (plan R14, R14.T2.d with R14.T3.d): the two-pass occlusion's late
 * pass's opaque buckets into the same IDs after the depth pyramid's build, loading them and the
 * scene's depth, before the resolve.
 */
export const VISIBILITY_LATE_RASTER_LABEL = "terrain id raster late";

/**
 * The resolve's pass label (plan R14, R14.T2.d): each consumer class's full-screen resolve into the
 * scene target, then the terrain drawn forward over it, depth-tested against the raster's depth.
 */
export const VISIBILITY_RESOLVE_LABEL = "terrain resolve";

/** The ID's low bits that hold the triangle of its cluster. */
export const VISIBILITY_TRIANGLE_BITS = 7;

/** The most triangles an ID can name in a cluster: 128, Nanite's cluster (Design note 2). */
export const VISIBILITY_TRIANGLES = 2 ** VISIBILITY_TRIANGLE_BITS;

/**
 * The step between triangles in a de-indexed cluster mesh's `position` z, above its lit-mesh code
 * (`gpu/resources.ts`' `LIT_VERTEX_CODE`, 0 to 5): z = code + 8 × triangle.
 */
export const DEINDEXED_TRIANGLE_STEP = 8;

/**
 * The ID of `triangle` of the visible cluster at index `visible` (see the module's remarks):
 * `terrainClusterId.wgsl`'s, on either path.
 *
 * @throws RangeError for an index past {@link MAX_LIST_ENTRIES} or a triangle past
 *   {@link VISIBILITY_TRIANGLES}, which no ID holds.
 */
export function packVisibilityId(visible: number, triangle: number): number {
  if (
    !Number.isInteger(visible) ||
    visible < 0 ||
    visible >= MAX_LIST_ENTRIES ||
    !Number.isInteger(triangle) ||
    triangle < 0 ||
    triangle >= VISIBILITY_TRIANGLES
  ) {
    throw new RangeError(`no ID holds triangle ${String(triangle)} of index ${String(visible)}`);
  }
  return ((visible << VISIBILITY_TRIANGLE_BITS) | triangle) >>> 0;
}

/** The visible-cluster index and triangle an ID holds, or `null` for {@link ID_NONE}. */
export function unpackVisibilityId(
  id: number,
): { readonly visible: number; readonly triangle: number } | null {
  if (id === ID_NONE) {
    return null;
  }
  return { visible: id >>> VISIBILITY_TRIANGLE_BITS, triangle: id & (VISIBILITY_TRIANGLES - 1) };
}

/**
 * The `Draw` uniform an ID raster's draw adds its range's first index from (`terrainClusterId.wgsl`'s
 * `visibleFirst`): absent, 0, the first body's (see the module's remarks).
 */
export const VISIBLE_FIRST_UNIFORM = "visibleFirst";

/**
 * `draw`, an ID raster's, writing the IDs of the range from `first` of the view's index space
 * (plan R14, R14.T2.d): `draw` itself at 0, the range of the frame's first body.
 */
export function withVisibleFirst(draw: DrawItem, first: number): DrawItem {
  return first === 0
    ? draw
    : { ...draw, uniforms: { ...draw.uniforms, [VISIBLE_FIRST_UNIFORM]: Float32Array.of(first) } };
}

/** A consumer class of the visibility buffer: terrain alone until T5.e's meshes. */
export type VisibleConsumer = "terrain";

/** A consumer's range of a view's visible-cluster index space (see the module's remarks). */
export interface VisibleConsumerRange {
  readonly consumer: VisibleConsumer;
  /** Its first index: its lists' entry 0. */
  readonly first: number;
  /** Its entries: its lists' length. */
  readonly entries: number;
}

/**
 * A view's index space for the terrain of a frame's bodies, the lists of each of `terrainEntries`
 * entries, all its passes': each body's range in its order, the first from 0 and each other just
 * after the one before (plan R14, R14.T2.d), so that two bodies' IDs never meet.
 *
 * @throws RangeError for a count that is no whole number of entries, or a space past
 *   {@link MAX_LIST_ENTRIES}, where the frame draws its terrain forward instead.
 */
export function visibleConsumerRanges(
  terrainEntries: ReadonlyArray<number>,
): ReadonlyArray<VisibleConsumerRange> {
  const ranges: VisibleConsumerRange[] = [];
  let first = 0;
  for (const entries of terrainEntries) {
    if (!Number.isInteger(entries) || entries < 0 || first + entries > MAX_LIST_ENTRIES) {
      throw new RangeError(
        `${String(entries)} entries from ${String(first)} do not fit the visible-cluster index space`,
      );
    }
    ranges.push({ consumer: "terrain", first, entries });
    first += entries;
  }
  return ranges;
}

/** A cluster mesh and its templates, as `templates.ts` makes them. */
export interface IndexedClusterMesh {
  readonly templates: ReadonlyArray<ClusterTemplate>;
  readonly mesh: MeshSpec;
}

/**
 * The de-indexed form of `cluster`'s mesh, named `name` (see the module's remarks): vertex i is the
 * vertex its index i names, its `position`'s z the lit-mesh code plus
 * {@link DEINDEXED_TRIANGLE_STEP} times the triangle i ÷ 3 is of its template, counted from the
 * template's first index; and its indices 0 to n − 1, so that each template's range draws its own
 * triangles from the same arguments.
 *
 * @throws Error for an indexed mesh with none, an index past its vertices or outside every
 *   template, a template of more triangles than an ID names, or a mesh with attributes beside its
 *   positions, which this does not de-index.
 */
export function deindexedClusterMesh(cluster: IndexedClusterMesh, name: string): MeshSpec {
  const { positions, indices } = cluster.mesh;
  if (indices === null) {
    throw new Error(`${cluster.mesh.name} has no indices to draw de-indexed`);
  }
  if (Object.keys(cluster.mesh.attributes).length > 0) {
    throw new Error(`${cluster.mesh.name} has attributes beside its positions to de-index`);
  }
  const triangleOf = new Int32Array(indices.length).fill(-1);
  for (const template of cluster.templates) {
    if (template.triangles > VISIBILITY_TRIANGLES) {
      throw new Error(
        `${cluster.mesh.name}'s ${template.shape} template has ${String(template.triangles)} triangles, more than an ID names`,
      );
    }
    for (let k = 0; k < template.indexCount; k += 1) {
      triangleOf[template.firstIndex + k] = Math.floor(k / 3);
    }
  }
  const out = new Float32Array(3 * indices.length);
  for (let i = 0; i < indices.length; i += 1) {
    const vertex = indices[i] ?? -1;
    const triangle = triangleOf[i] ?? -1;
    const x = positions[3 * vertex];
    const y = positions[3 * vertex + 1];
    const code = positions[3 * vertex + 2];
    if (x === undefined || y === undefined || code === undefined || triangle < 0) {
      throw new Error(`${cluster.mesh.name}'s index ${String(i)} names no template's vertex`);
    }
    out[3 * i] = x;
    out[3 * i + 1] = y;
    out[3 * i + 2] = code + DEINDEXED_TRIANGLE_STEP * triangle;
  }
  return {
    ...cluster.mesh,
    name,
    positions: out,
    indices: Uint32Array.from({ length: indices.length }, (_, i) => i),
  };
}

/**
 * A view's visibility target (plan R14, R14.T2.c; Design note 9): an `r32uint` ID a pixel, drawn
 * with its scene target's depth (R14.T2.b's `depthOf`), at the scene's size, R07's internal scale
 * included; each render clears it to {@link ID_NONE} and loads and stores the scene's depth.
 *
 * @remarks
 * Its owner resizes it with the scene target, and disposes of it and makes it again where it makes
 * the scene target again (after a device loss): it is the plan's `VisibilityBuffer`, given the
 * scene target whose depth it draws with.
 */
export class VisibilityBuffer {
  readonly target: RenderTarget;

  /**
   * @throws Error naming the target where the device refuses it, or `scene` has no depth.
   */
  constructor(
    engine: Pick<RenderEngine, "createRenderTarget">,
    name: string,
    scene: RenderTarget,
    size: ViewSize,
  ) {
    this.target = engine.createRenderTarget({
      name: `${name}:ids`,
      size,
      format: "r32uint",
      mips: 1,
      depth: false,
      depthOf: scene,
      category: "render-targets",
    });
  }

  /** The IDs, which the resolve reads as a `texture_2d<u32>` by `textureLoad`. */
  get ids(): TextureHandle {
    return this.target.colour;
  }

  /** Follows the scene target's size. */
  resize(size: ViewSize): void {
    this.target.resize(size);
  }

  dispose(): void {
    this.target.dispose();
  }
}
