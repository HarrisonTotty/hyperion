/**
 * What a pass asks of the cluster cull (plan R14, R14.T1.b and T1.c): up to
 * {@link MAX_CULL_PASSES} passes a dispatch, as WGSL's `array<CullPass, 8>` of `clusterCull.wgsl`
 * lays them out, and the dispatch's own parameters, `CullParams`.
 *
 * @remarks
 * A pass is a shadow cascade's light-space box in phase 1 (T1.b); a camera's frustum, for the
 * overlay's depth and from plan R14's T2 the lit pass, follows the same record. Each pass names
 * the tolerance its level of detail is chosen at (a cascade's texel, m), the coarsest stride it may
 * take, and where its buckets begin (`clusters/buckets.ts`). Every position is camera-relative in
 * the frame the consumer's instance origins are in (terrain's: the body's non-rotating axes, as
 * `TerrainPass`' `#write` forms them), every number formed in `f64` and narrowed once here.
 *
 * `CullPass`, 128 B: `header` (kind: 0 none, 1 cascade; the stride cap as log₂ of it; the pass's
 * first bucket; its first list entry), `scalars` (the tolerance, m; the cascade's radius r, m; the
 * far plane's distance from its centre, away from the light, m; the receivers' cull, 1 on and 0
 * off, T1.c), the light's axes x_L, y_L and z_L (towards the light), the cascade's centre less the
 * camera in those axes, m, and two `vec4f` of T1.c's receivers' cull (`receivers`, `covered`).
 *
 * `CullParams`, 144 B: the page's axes into the instances' frame by column (terrain: the body's
 * rotation), `counts` (instances, `u32` words an instance record, clusters a page, passes), the
 * templates a pass has, and each template's first list entry within its pass and its capacity, by
 * template, four to a `vec4u`.
 */

import type { Vec3 } from "../../geometry/vec3";
import type { Stride } from "./templates";

/** The most passes one dispatch culls for, a uniform array's length. */
export const MAX_CULL_PASSES = 8;

/** Bytes of one `CullPass`. */
export const CULL_PASS_BYTES = 128;

/** Bytes of the dispatch's `CullParams`. */
export const CULL_PARAMS_BYTES = 144;

/** The most templates a pass may have: two `vec4u` of offsets and capacities. */
export const MAX_PASS_TEMPLATES = 8;

/**
 * T1.c's receivers' cull of a cascade (plan R14, R14.T1.c): the view depths its receivers sample
 * it at, and the square below it that covers the receivers that would fall through to it.
 */
export interface CullReceivers {
  /** The view axis in the instances' frame, the camera's forward. */
  readonly viewAxis: Vec3;
  /** The view depths, m along the view axis, at which receivers sample this cascade. */
  readonly depthM: readonly [near: number, far: number];
  /** s, the sun's direction on the view axis, and ℓ, the set's caster reach, m. */
  readonly sunOnViewAxis: number;
  readonly casterReachM: number;
  /**
   * The next nearer cascade's covered square in this pass's light axes, a centre (x, y) less the
   * camera, m, and a half-side, m, within which every receiver is that cascade's; `null` for the
   * nearest, where nothing falls through.
   */
  readonly coveredBelow: {
    readonly centreM: readonly [number, number];
    readonly halfSideM: number;
  } | null;
  /** The margin a receiver's own offset and filter reach add about a caster, m. */
  readonly marginM: number;
}

/** A pass the cull culls for (see the module's remarks). */
export interface CascadeCullPass {
  readonly kind: "cascade";
  /** The light's axes x_L, y_L and z_L (towards the light) in the instances' frame. */
  readonly axes: readonly [Vec3, Vec3, Vec3];
  /** The cascade's snapped centre less the camera, in the light's axes, m. */
  readonly centreFromCameraLightM: Vec3;
  /** r, the cascade's radius, m: its box's half-width. */
  readonly radiusM: number;
  /** The far plane's distance from the centre, away from the light, m. */
  readonly depthFarM: number;
  /** The tolerance its strides are chosen at: its texel, m (T1.d). */
  readonly toleranceM: number;
  /** The coarsest stride it may take: 1 keeps every cluster at stride 1. */
  readonly strideCap: Stride;
  /** T1.c's receivers' cull, or `null` where it is off. */
  readonly receivers: CullReceivers | null;
}

/** A pass of the cull: phase 1's are cascades. */
export type CullPass = CascadeCullPass;

/** Where a pass's buckets begin: its first bucket and its first list entry. */
export interface CullPassPlace {
  readonly bucketBase: number;
  readonly listBase: number;
}

/**
 * Packs `passes`, each at `places`' place of the same index, into `out` (at least
 * {@link MAX_CULL_PASSES} × {@link CULL_PASS_BYTES} bytes), the rest zero, as `clusterCull.wgsl`'s
 * `array<CullPass, 8>`.
 *
 * @throws RangeError for more than {@link MAX_CULL_PASSES} passes, places that do not match them,
 *   or an `out` too small.
 */
export function packCullPasses(
  passes: ReadonlyArray<CullPass>,
  places: ReadonlyArray<CullPassPlace>,
  out: ArrayBuffer,
): void {
  if (passes.length > MAX_CULL_PASSES || places.length !== passes.length) {
    throw new RangeError(
      `${String(passes.length)} cull passes and ${String(places.length)} places, at most ${String(MAX_CULL_PASSES)} of each and one a pass`,
    );
  }
  if (out.byteLength < MAX_CULL_PASSES * CULL_PASS_BYTES) {
    throw new RangeError(
      `cull passes need ${String(MAX_CULL_PASSES * CULL_PASS_BYTES)} B, not ${String(out.byteLength)} B`,
    );
  }
  const view = new DataView(out);
  new Uint8Array(out, 0, MAX_CULL_PASSES * CULL_PASS_BYTES).fill(0);
  passes.forEach((pass, n) => {
    const at = n * CULL_PASS_BYTES;
    const place = places[n] ?? { bucketBase: 0, listBase: 0 };
    view.setUint32(at, 1, true);
    view.setUint32(at + 4, Math.log2(pass.strideCap), true);
    view.setUint32(at + 8, place.bucketBase, true);
    view.setUint32(at + 12, place.listBase, true);
    view.setFloat32(at + 16, pass.toleranceM, true);
    view.setFloat32(at + 20, pass.radiusM, true);
    view.setFloat32(at + 24, pass.depthFarM, true);
    view.setFloat32(at + 28, pass.receivers === null ? 0 : 1, true);
    pass.axes.forEach((axis, k) => {
      view.setFloat32(at + 32 + 16 * k, axis.x, true);
      view.setFloat32(at + 36 + 16 * k, axis.y, true);
      view.setFloat32(at + 40 + 16 * k, axis.z, true);
    });
    const centre = pass.centreFromCameraLightM;
    view.setFloat32(at + 80, centre.x, true);
    view.setFloat32(at + 84, centre.y, true);
    view.setFloat32(at + 88, centre.z, true);
    const receivers = pass.receivers;
    if (receivers !== null) {
      view.setFloat32(at + 96, receivers.depthM[0], true);
      view.setFloat32(at + 100, receivers.depthM[1], true);
      view.setFloat32(at + 104, receivers.sunOnViewAxis, true);
      view.setFloat32(at + 108, receivers.casterReachM, true);
      const below = receivers.coveredBelow;
      view.setFloat32(at + 112, below?.centreM[0] ?? 0, true);
      view.setFloat32(at + 116, below?.centreM[1] ?? 0, true);
      view.setFloat32(at + 120, below === null ? -1 : below.halfSideM, true);
      view.setFloat32(at + 124, receivers.marginM, true);
    }
  });
}

/** One dispatch's parameters (see the module's remarks). */
export interface CullParams {
  /** The page's axes into the instances' frame, by column. */
  readonly rotationColumns: readonly [Vec3, Vec3, Vec3];
  readonly instances: number;
  /** `u32` words of a consumer's instance record; its first four the origin less the camera and the page. */
  readonly instanceWords: number;
  readonly clustersPerPage: number;
  readonly passes: number;
  /** Each template's first list entry within its pass, and its list capacity, by template. */
  readonly templateOffsets: ReadonlyArray<number>;
  readonly templateCapacities: ReadonlyArray<number>;
  /** The view axis in the instances' frame, for T1.c's receivers' cull. */
  readonly viewAxis: Vec3;
}

/**
 * Packs `params` into `out` ({@link CULL_PARAMS_BYTES} bytes at least), as `clusterCull.wgsl`'s
 * `CullParams`.
 *
 * @throws RangeError for more than {@link MAX_PASS_TEMPLATES} templates, or offsets and capacities
 *   of different counts.
 */
export function packCullParams(params: CullParams, out: ArrayBuffer): void {
  const { templateOffsets, templateCapacities } = params;
  if (
    templateOffsets.length > MAX_PASS_TEMPLATES ||
    templateOffsets.length !== templateCapacities.length
  ) {
    throw new RangeError(
      `${String(templateOffsets.length)} template offsets and ${String(templateCapacities.length)} capacities, at most ${String(MAX_PASS_TEMPLATES)} and as many of each`,
    );
  }
  const view = new DataView(out);
  new Uint8Array(out, 0, CULL_PARAMS_BYTES).fill(0);
  params.rotationColumns.forEach((column, k) => {
    view.setFloat32(16 * k, column.x, true);
    view.setFloat32(16 * k + 4, column.y, true);
    view.setFloat32(16 * k + 8, column.z, true);
  });
  view.setUint32(48, params.instances, true);
  view.setUint32(52, params.instanceWords, true);
  view.setUint32(56, params.clustersPerPage, true);
  view.setUint32(60, params.passes, true);
  view.setFloat32(64, params.viewAxis.x, true);
  view.setFloat32(68, params.viewAxis.y, true);
  view.setFloat32(72, params.viewAxis.z, true);
  view.setUint32(76, templateOffsets.length, true);
  templateOffsets.forEach((offset, t) => {
    view.setUint32(80 + 4 * t, offset, true);
  });
  templateCapacities.forEach((capacity, t) => {
    view.setUint32(112 + 4 * t, capacity, true);
  });
}
