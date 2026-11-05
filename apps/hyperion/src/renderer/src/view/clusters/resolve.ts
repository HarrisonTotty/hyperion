/**
 * The visibility buffer's resolve (plan R14, R14.T2.d; Design note 9): one full-screen pass a
 * consumer class into the scene target, which reads each pixel's ID, refetches its triangle's three
 * vertices through the consumer's own vertex function, forms the pixel's perspective-correct
 * barycentrics from their clip positions, and shades it once, as the consumer's forward fragment
 * does.
 *
 * @remarks
 * - **The template** of an ID's cluster is its bucket's: a view's buckets lie pass after pass, each
 *   pass's templates in order from fixed offsets (`buckets.ts`), so the template is the last whose
 *   offset in its pass is at most the index's (`visibleTemplate`), from the pass's entries and its
 *   templates' offsets in the resolve's uniforms ({@link resolveUniforms}). Not Design note 9's top
 *   3 bits of the cluster word (lean, 2026-11-04): the lists keep their one `u32` an entry, and no
 *   kernel changes.
 * - **The triangle's corners** are a strided template's, decoded as `templates.ts` lays them out
 *   ({@link templateTriangle}): a block's quads row by row, each split on its (0, 0)–(1, 1)
 *   diagonal, and a strip's feet along its edge; the first corner is the triangle's first index,
 *   the provoking vertex whose flat varyings the rasteriser takes.
 * - **The barycentrics** ({@link clipBarycentrics}): b ∝ u × v, with u_i = x_i − X w_i and v_i =
 *   y_i − Y w_i for the clip positions (x, y, w)_i and the pixel's centre at (X, Y) in normalised
 *   device coordinates: the weights of the clip-space point on the triangle that projects to the
 *   pixel, which are the perspective-correct weights of every varying, normalised to sum to 1, and
 *   clamped to the triangle. The corners are first snapped to the rasteriser's sub-pixel grid
 *   ({@link snappedClip}), as it snaps them before it covers and interpolates, so that the weights
 *   are its own: unsnapped, SwiftShader's 1/16 px grid put the places a median of 1,200 ulps from
 *   its interpolants, and a pixel's centre just outside a triangle seen near edge-on carried them
 *   far beyond it.
 * - **The geometric normal**, which the forward fragment takes from its place's fine screen
 *   derivatives, is the triangle's face normal, from its edges, oriented to the datum's side.
 */

import type { DrawItem, MeshSpec, TextureHandle, WgslMaterialSpec } from "../engine/types";
import type { ClusterBuckets } from "./buckets";
import type { ClusterTemplate } from "./templates";
import { CLUSTER_QUADS } from "./terrainGrid";
import type { VisibleConsumer } from "./visibility";

/**
 * The `DrawItem.textures` name under which a resolve binds the view's IDs
 * (`VisibilityBuffer.ids`), which its frame's renderer gives it ({@link withVisibilityIds}).
 */
export const VISIBILITY_IDS_TEXTURE = "visibilityIds";

/**
 * The resolve's full-screen triangle, in clip space's x and y: its vertex stage sets each corner at
 * reversed depth's near end, so that every pixel passes the depth test, and writes no depth.
 */
export const RESOLVE_SCREEN_MESH: MeshSpec = {
  name: "resolve screen",
  positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
  indices: null,
  topology: "triangle-list",
  attributes: {},
};

/** A resolve's draw as its renderer draws it: with the view's IDs bound as {@link VISIBILITY_IDS_TEXTURE}. */
export function withVisibilityIds(draw: DrawItem, ids: TextureHandle): DrawItem {
  return { ...draw, textures: { ...draw.textures, [VISIBILITY_IDS_TEXTURE]: ids } };
}

/** The opaque templates the ID raster draws, a pass's first eight: blocks, then strips. */
export const RESOLVE_TEMPLATES = 8;

/**
 * What a consumer gives the resolve (Design note 9): its class, and its resolve's material, whose
 * WGSL decodes an ID of its own and shades it as its forward fragment does, within WebGPU's eight
 * storage buffers a stage (Design note 6). A whole material, not Provides' WGSL and storage alone
 * (as built, 2026-11-05): each class resolves in a pass of its own, and a spec carries its WGSL,
 * storage, textures and states; terrain's is `terrainMaterial.ts`' `terrainResolveMaterial`.
 */
export interface ResolveMaterial {
  readonly consumer: VisibleConsumer;
  readonly spec: WgslMaterialSpec;
}

/** The resolve's `Draw` fields of a view's buckets ({@link resolveUniforms}). */
export interface ResolveUniforms {
  /**
   * x a pass's entries; y and z the consumer's range of the view's index space, first and entries;
   * w the cells a pixel of the rasteriser's sub-pixel grid, on a side.
   */
  readonly resolveLists: Float32Array;
  /** The pass's first four templates' first entries, then its next four's. */
  readonly resolveTemplates0: Float32Array;
  readonly resolveTemplates1: Float32Array;
}

/**
 * The resolve's uniforms for `buckets` on a device of `subpixelBits` (`GpuCapabilities`'), its range
 * of the index space from `first` (0, the frame's first body's, until {@link withResolveFirst}
 * moves it): each value an `f32` the engine packs exactly below 2²⁴ (`MAX_LIST_ENTRIES`).
 */
export function resolveUniforms(
  buckets: ClusterBuckets,
  subpixelBits: number,
  first = 0,
): ResolveUniforms {
  const offsets = buckets.templateOffsets;
  const at = (t: number): number => offsets[t] ?? buckets.passEntries;
  return {
    resolveLists: Float32Array.of(
      buckets.passEntries,
      first,
      buckets.passes * buckets.passEntries,
      2 ** subpixelBits,
    ),
    resolveTemplates0: Float32Array.of(at(0), at(1), at(2), at(3)),
    resolveTemplates1: Float32Array.of(at(4), at(5), at(6), at(7)),
  };
}

/**
 * `draw`, a resolve's, shading the range from `first` of the view's index space (plan R14,
 * R14.T2.d): its `resolveLists` with `first` in place of its own, the range's length kept; `draw`
 * itself where `first` is its own already, 0 for the frame's first body (`resolveUniforms`).
 */
export function withResolveFirst(draw: DrawItem, first: number): DrawItem {
  const lists = draw.uniforms["resolveLists"];
  if (lists === undefined || lists[1] === first) {
    return draw;
  }
  return {
    ...draw,
    uniforms: {
      ...draw.uniforms,
      resolveLists: Float32Array.of(lists[0] ?? 0, first, lists[2] ?? 0, lists[3] ?? 0),
    },
  };
}

/**
 * The opaque template of the bucket entry at index `visible` of a view's index space whose passes
 * span `passEntries` entries each, its templates from `offsets` in a pass: the last of the first
 * {@link RESOLVE_TEMPLATES} whose offset is at most the entry's in its pass
 * (`resolveTerrain.wgsl`'s `resolve_template`).
 */
export function visibleTemplate(
  visible: number,
  passEntries: number,
  offsets: ReadonlyArray<number>,
): number {
  const local = visible % passEntries;
  let template = 0;
  for (let t = 1; t < RESOLVE_TEMPLATES; t += 1) {
    if ((offsets[t] ?? Number.POSITIVE_INFINITY) <= local) {
      template = t;
    }
  }
  return template;
}

/** A template vertex: its place in its cluster and its lit-mesh code, as the cluster mesh holds it. */
export type TemplateVertex = readonly [number, number, number];

/**
 * The three corners of `triangle` of the strided `template`, in its index order
 * (`resolveTerrain.wgsl`'s `resolve_corners`; `templates.ts`' `blockIndices`, `stripIndices`).
 *
 * @throws RangeError for a may-discard template, which the ID raster never draws, or a triangle
 *   past the template's.
 */
export function templateTriangle(
  template: ClusterTemplate,
  triangle: number,
): readonly [TemplateVertex, TemplateVertex, TemplateVertex] {
  if (template.mayDiscard || triangle < 0 || triangle >= template.triangles) {
    throw new RangeError(
      `triangle ${String(triangle)} is no ID's of the ${template.shape} template at stride ${String(template.stride)}`,
    );
  }
  const s = template.stride;
  const quad = Math.floor(triangle / 2);
  const second = triangle % 2 === 1;
  if (template.shape === "terrain-block") {
    const across = CLUSTER_QUADS / s;
    const x = (quad % across) * s;
    const y = Math.floor(quad / across) * s;
    return second
      ? [
          [x, y, 0],
          [x + s, y + s, 0],
          [x, y + s, 0],
        ]
      : [
          [x, y, 0],
          [x + s, y, 0],
          [x + s, y + s, 0],
        ];
  }
  const k = quad * s;
  const next = k + s;
  return second
    ? [
        [next, 0, 1],
        [next, 0, 0],
        [k, 0, 0],
      ]
    : [
        [k, 0, 1],
        [next, 0, 1],
        [k, 0, 0],
      ];
}

/** A clip position, (x, y, z, w). */
export type ClipPosition = readonly [number, number, number, number];

/** `v` to the nearest integer, a half to the even one, as WGSL's `round`. */
function roundHalfEven(v: number): number {
  const down = Math.floor(v);
  const part = v - down;
  if (part !== 0.5) {
    return part < 0.5 ? down : down + 1;
  }
  return down % 2 === 0 ? down : down + 1;
}

/**
 * Clip position `c` with its place on a frame of `size` snapped to the nearest of `cells` a pixel
 * on a side, as the rasteriser snaps a corner (see the module's remarks; `resolveTerrain.wgsl`'s
 * `resolve_snapped`): its depth and w kept, and a corner behind the eye kept whole, since the
 * rasteriser snaps the corners it clips to the near plane instead.
 */
export function snappedClip(
  c: ClipPosition,
  size: { readonly widthPx: number; readonly heightPx: number },
  cells: number,
): ClipPosition {
  const [x, y, z, w] = c;
  if (!(w > 0)) {
    return c;
  }
  const snap = (px: number): number => roundHalfEven(px * cells) / cells;
  const column = snap((0.5 + (0.5 * x) / w) * size.widthPx);
  const row = snap((0.5 - (0.5 * y) / w) * size.heightPx);
  return [((2 * column) / size.widthPx - 1) * w, (1 - (2 * row) / size.heightPx) * w, z, w];
}

/**
 * The perspective-correct barycentrics of the point at normalised device coordinates `ndc` on the
 * triangle of clip positions `c0`, `c1` and `c2` (see the module's remarks;
 * `resolveTerrain.wgsl`'s `resolve_barycentrics`), each at least 0 and summing to 1, so that a point
 * outside the triangle is taken on it; the first corner's alone for a triangle seen edge-on.
 */
export function clipBarycentrics(
  c0: ClipPosition,
  c1: ClipPosition,
  c2: ClipPosition,
  ndc: readonly [number, number],
): readonly [number, number, number] {
  const [x, y] = ndc;
  const u = [c0[0] - x * c0[3], c1[0] - x * c1[3], c2[0] - x * c2[3]] as const;
  const v = [c0[1] - y * c0[3], c1[1] - y * c1[3], c2[1] - y * c2[3]] as const;
  const b0 = u[1] * v[2] - u[2] * v[1];
  const b1 = u[2] * v[0] - u[0] * v[2];
  const b2 = u[0] * v[1] - u[1] * v[0];
  // Oriented by the triangle's side, the sign of Σ b_i w_i = det(w, x, y), the same at every pixel
  // (the sum's own sign flips past the plane's horizon), so that the weights within the triangle are
  // positive, and clamped to it: a pixel's centre may fall a hair outside the triangle (on a
  // sub-pixel grid other than the one taken, or at a corner clipped), where one seen near edge-on
  // would carry the weights far beyond it.
  const side = Math.sign(b0 * c0[3] + b1 * c1[3] + b2 * c2[3]);
  const w = [Math.max(0, side * b0), Math.max(0, side * b1), Math.max(0, side * b2)] as const;
  const total = w[0] + w[1] + w[2];
  return total > 0 ? [w[0] / total, w[1] / total, w[2] / total] : [1, 0, 0];
}
