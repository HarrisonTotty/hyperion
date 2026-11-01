/**
 * What a pass asks of the cluster cull (plan R14, R14.T1.b–e): up to
 * {@link MAX_CULL_PASSES} passes a dispatch, as WGSL's `array<CullPass, 8>` of `clusterCull.wgsl`
 * lays them out, and the dispatch's own parameters, `CullParams`.
 *
 * @remarks
 * A pass is a shadow cascade's light-space box (T1.b–d) or a camera's frustum (T1.e: the overlay's
 * terrain depth, and from plan R14's T2 the lit pass). Each pass names the tolerance its level of
 * detail is chosen at (a cascade's texel, m), the coarsest stride it may take, and where its buckets
 * begin (`clusters/buckets.ts`). Every position is camera-relative in the frame the consumer's
 * instance origins are in (terrain's: the body's non-rotating axes, as `TerrainPass`' `#write` forms
 * them), every number formed in `f64` and narrowed once here.
 *
 * `CullPass`, 128 B: `header` (kind: 0 none, 1 cascade, 2 camera; the stride cap as log₂ of it;
 * the pass's first bucket; its first list entry) and `scalars` (the tolerance, m; then a cascade's
 * radius r, m, its far plane's distance from its centre away from the light, m, and its receivers'
 * cull, 1 on and 0 off, T1.c; or a camera's planes' count). A cascade then holds the light's axes
 * x_L, y_L and z_L (towards the light), its centre less the camera in those axes, m, and two
 * `vec4f` of T1.c's receivers' cull: `receivers` (the depths receivers sample the cascade between,
 * m, and their reach nearer and farther than a caster, m) and `covered` (the next nearer cascade's
 * covered square's centre x and y and half-side, −1 for none, and the receivers' reach across the
 * light, m). A camera holds its planes in the six `vec4f` after its scalars, each its inward normal
 * and its offset, m.
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
 * T1.c's receivers' cull of a cascade (plan R14, R14.T1.c; `clusters/receivers.ts`): the view
 * depths its receivers sample it at, how far beyond a caster's sphere its receivers reach, and the
 * square below it that holds the receivers that would fall through to it.
 */
export interface CullReceivers {
  /**
   * The next nearer cascade's fade start and this cascade's far end, m along the view axis:
   * receivers sample this cascade only between them, or below the first by falling through; 0 for
   * the nearest's first, below which nothing falls through.
   */
  readonly depthM: readonly [near: number, far: number];
  /**
   * How far a caster's receivers may lie nearer and farther than its sphere along the view axis, m:
   * ℓ max(0, s) and ℓ max(0, −s), ℓ the set's caster reach and s the light's axis on the view axis,
   * each with a receiver's normal offset and filter reach.
   */
  readonly reachM: readonly [nearer: number, farther: number];
  /**
   * The next nearer cascade's covered square, its centre (x, y) less the camera in this pass's light
   * axes, m, and its half-side, m, within which that cascade covers every receiver with its
   * footprint; and how far beyond a caster's sphere its receivers may lie across the light, m.
   * `null` for the nearest, where nothing falls through.
   */
  readonly coveredBelow: {
    readonly centreM: readonly [number, number];
    readonly halfSideM: number;
    readonly reachM: number;
  } | null;
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

/** A plane of a camera pass, `dot(normal, p) + offsetM ≥ 0` on its inner side, p from the camera. */
export interface CullPlane {
  /** The inward unit normal, in the instances' frame. */
  readonly normal: Vec3;
  readonly offsetM: number;
}

/** The most planes a camera pass has: the six `vec4f` after its header and scalars. */
export const MAX_CULL_PLANES = 6;

/**
 * A camera's frustum as a pass of the cull (plan R14, R14.T1.e): the overlay's terrain depth, and
 * from plan R14's T2 the lit pass. A cluster is refused where its sphere lies wholly outside one of
 * the planes, the camera inside it kept.
 */
export interface CameraCullPass {
  readonly kind: "camera";
  /** The frustum's planes (R05's `frustumOf`: four sides and the near), at most {@link MAX_CULL_PLANES}. */
  readonly planes: ReadonlyArray<CullPlane>;
  /** The tolerance its strides are chosen at, m: unread at stride 1 (plan R14's T3.c). */
  readonly toleranceM: number;
  /** The coarsest stride it may take: the overlay's depth takes 1. */
  readonly strideCap: Stride;
}

/** A pass of the cull: a cascade's box (T1.b–d) or a camera's frustum (T1.e). */
export type CullPass = CascadeCullPass | CameraCullPass;

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
  passes: ReadonlyArray<CullPass | null>,
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
    if (pass === null) {
      return;
    }
    const place = places[n] ?? { bucketBase: 0, listBase: 0 };
    view.setUint32(at, PASS_KIND_CODE[pass.kind], true);
    view.setUint32(at + 4, Math.log2(pass.strideCap), true);
    view.setUint32(at + 8, place.bucketBase, true);
    view.setUint32(at + 12, place.listBase, true);
    view.setFloat32(at + 16, pass.toleranceM, true);
    switch (pass.kind) {
      case "cascade":
        packCascade(view, at, pass);
        return;
      case "camera":
        packCamera(view, at, pass);
        return;
    }
  });
}

/** A pass's kind as `clusterCull.wgsl`'s header holds it, 0 for none. */
const PASS_KIND_CODE: Readonly<Record<CullPass["kind"], number>> = { cascade: 1, camera: 2 };

/** A camera pass's planes after its scalars: their count, then each plane as a `vec4f`. */
function packCamera(view: DataView, at: number, pass: CameraCullPass): void {
  if (pass.planes.length > MAX_CULL_PLANES) {
    throw new RangeError(
      `a camera pass has at most ${String(MAX_CULL_PLANES)} planes, not ${String(pass.planes.length)}`,
    );
  }
  view.setFloat32(at + 20, pass.planes.length, true);
  pass.planes.forEach((plane, k) => {
    view.setFloat32(at + 32 + 16 * k, plane.normal.x, true);
    view.setFloat32(at + 36 + 16 * k, plane.normal.y, true);
    view.setFloat32(at + 40 + 16 * k, plane.normal.z, true);
    view.setFloat32(at + 44 + 16 * k, plane.offsetM, true);
  });
}

/** A cascade pass's box and receivers after its tolerance. */
function packCascade(view: DataView, at: number, pass: CascadeCullPass): void {
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
    view.setFloat32(at + 104, receivers.reachM[0], true);
    view.setFloat32(at + 108, receivers.reachM[1], true);
    const below = receivers.coveredBelow;
    view.setFloat32(at + 112, below?.centreM[0] ?? 0, true);
    view.setFloat32(at + 116, below?.centreM[1] ?? 0, true);
    view.setFloat32(at + 120, below === null ? -1 : below.halfSideM, true);
    view.setFloat32(at + 124, below?.reachM ?? 0, true);
  }
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
