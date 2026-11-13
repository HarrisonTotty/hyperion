/**
 * What a pass asks of the cluster cull (plan R14, R14.T1.b–e): up to
 * {@link MAX_CULL_PASSES} passes a dispatch, as WGSL's `array<CullPass, 8>` of `clusterCull.wgsl`
 * lays them out, and the dispatch's own parameters, `CullParams`.
 *
 * @remarks
 * A pass is a shadow cascade's light-space box (T1.b–d) or a camera's frustum (T1.e: the overlay's
 * terrain depth; T2.a: the lit pass, with the body's horizon, whose kept clusters the cull marks
 * for the stable compaction rather than appending them). Each pass names the tolerance its level of
 * detail is chosen at (a cascade's texel, m), the coarsest stride it may take, and where its buckets
 * begin (`clusters/buckets.ts`). Every position is camera-relative in the frame the consumer's
 * instance origins are in (terrain's: the body's non-rotating axes, as `TerrainPass`' `#write` forms
 * them), every number formed in `f64` and narrowed once here.
 *
 * `CullPass`, 128 B: `header` (kind: 0 none, 1 cascade, 2 camera appended, 3 camera ordered, 4 and
 * 5 camera ordered, the two-pass occlusion's early and late passes (R14.T3.d); the stride cap as
 * log₂ of it; the pass's first bucket; its first list entry) and `scalars` (the
 * tolerance, a cascade's in m, a camera's in rad (R14.T4.b); then a cascade's radius r, m, its far plane's distance from its centre away from
 * the light, m, and its receivers' cull, 1 on and 0 off, T1.c; or a camera's planes' count, its
 * horizon's plane distance, m, and the sine of its cone's half-angle, 0 for no horizon test). A
 * cascade then holds the light's axes
 * x_L, y_L and z_L (towards the light), its centre less the camera in those axes, m, and two
 * `vec4f` of T1.c's receivers' cull: `receivers` (the depths receivers sample the cascade between,
 * m, and their reach nearer and farther than a caster, m) and `covered` (the next nearer cascade's
 * covered square's centre x and y and half-side, −1 for none, and the receivers' reach across the
 * light, m). A camera holds its planes in the six `vec4f` after its scalars, each its inward normal
 * and its offset, m; one with a horizon holds at most five, its sixth `vec4f` the horizon's axis and
 * the cosine of its cone's half-angle (T2.a, {@link CullHorizon}).
 *
 * `CullParams`, 224 B: the page's axes into the instances' frame by column (terrain: the body's
 * rotation), `counts` (instances, `u32` words an instance record, clusters a page, passes), the
 * templates a pass has, each template's first list entry within its pass and its capacity, by
 * template, four to a `vec4u`, four `vec4u` of each, and `shapes` (the consumer's first shape's
 * code, R14.T4.b; then its may-discard templates, a byte a shape code 0 to 7 over two words,
 * R14.T1.g; then the `args` word its strides past τ′ are counted from, R14.T3.e, 0 for none), by
 * which a record's shape picks its template: (shape − first) × 4 + log₂ s,
 * or, for a record that may discard, its shape's may-discard template at stride 1 where the
 * consumer has one.
 */

import type { Vec3 } from "../../geometry/vec3";
import type { Stride } from "./templates";

/** The most passes one dispatch culls for, a uniform array's length. */
export const MAX_CULL_PASSES = 8;

/** Bytes of one `CullPass`. */
export const CULL_PASS_BYTES = 128;

/** Bytes of the dispatch's `CullParams`. */
export const CULL_PARAMS_BYTES = 224;

/** The most templates a pass may have: four `vec4u` of offsets and capacities. */
export const MAX_PASS_TEMPLATES = 16;

/** A shape's may-discard template where it has none: its records take their strided template. */
export const NO_DISCARD_TEMPLATE = 0xff;

/** The shape codes a consumer's may-discard templates are given for, 0 to 7. */
export const DISCARD_TEMPLATE_CODES = 8;

/** No consumer's shape has a may-discard template: every record takes its strided template. */
export const NO_DISCARD_TEMPLATES: ReadonlyArray<number> = Array.from(
  { length: DISCARD_TEMPLATE_CODES },
  () => NO_DISCARD_TEMPLATE,
);

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

/** A shadow cascade's light-space box as a pass of the cull (see the module's remarks). */
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
 * The body's horizon as a camera pass tests a cluster's sphere against it (plan R14, R14.T2.a;
 * Design note 4): R05's occluder, the sphere of radius R_occ about the body's centre inside every
 * possible surface (`terrain/cull.ts`' `horizonCone`), seen from the camera at C from that centre.
 * Its cone, from the camera tangent to the occluder, and the plane through the tangent circle bound
 * the occluded region: a point inside the cone and beyond the plane lies behind the occluder (Cozzi
 * and Ring 2011, the Cesium horizon-culling method, its point test in R05's `aboveHorizon`), so a
 * sphere wholly inside both is refused. Its terms are formed in `f64` on the CPU, since |C|² − R²
 * cannot be formed in `f32` near the ground.
 */
export interface CullHorizon {
  /** The cone's axis, −C ÷ |C|, from the camera towards the body's centre, in the instances' frame. */
  readonly axis: Vec3;
  /** The plane through the tangent circle: its distance from the camera along the axis, H² ÷ |C|, m. */
  readonly planeM: number;
  /** The sine and cosine of the cone's half-angle: R_occ ÷ |C| and H ÷ |C|, H² = |C|² − R_occ². */
  readonly sin: number;
  readonly cos: number;
}

/**
 * The horizon of the occluder of radius `occluderRadiusM` seen from `cameraFromCentreM`, the camera
 * less the body's centre in the instances' frame, m; `null` at or below the occluder, where R05's
 * test is skipped too.
 */
export function cullHorizonOf(
  cameraFromCentreM: Vec3,
  occluderRadiusM: number,
): CullHorizon | null {
  const c = Math.hypot(cameraFromCentreM.x, cameraFromCentreM.y, cameraFromCentreM.z);
  const r = occluderRadiusM;
  const horizonSq = (c - r) * (c + r);
  if (!(horizonSq > 0 && r > 0 && Number.isFinite(c))) {
    return null;
  }
  return {
    axis: { x: -cameraFromCentreM.x / c, y: -cameraFromCentreM.y / c, z: -cameraFromCentreM.z / c },
    planeM: horizonSq / c,
    sin: r / c,
    cos: Math.sqrt(horizonSq) / c,
  };
}

/**
 * A camera's frustum as a pass of the cull (plan R14, R14.T1.e): the overlay's terrain depth, and
 * from plan R14's T2 the lit pass. A cluster is refused where its sphere lies wholly outside one of
 * the planes, the camera inside it kept.
 */
export interface CameraCullPass {
  readonly kind: "camera";
  /**
   * The frustum's planes (R05's `frustumOf`: four sides and the near), at most
   * {@link MAX_CULL_PLANES}, or one fewer with a horizon.
   */
  readonly planes: ReadonlyArray<CullPlane>;
  /** The body's horizon, which refuses a cluster wholly behind it (T2.a), or `null` for none. */
  readonly horizon: CullHorizon | null;
  /**
   * How its kept clusters are recorded: appended to its buckets in any order (a depth-only pass's,
   * T1.e), or marked, one word a cluster, for the stable compaction to write its buckets in the draw
   * set's order (a colour pass's, T2.a; `compact.ts`). A dispatch has at most one ordered pass.
   */
  readonly order: "appended" | "ordered";
  /**
   * An ordered pass's part of the two-pass occlusion (plan R14, R14.T3.d; `clusters/hzb.ts`): the
   * early pass, which keeps only the clusters visible last frame, or the late pass, which keeps
   * the rest that the depth pyramid does not refuse and writes each cluster's visibility; absent,
   * neither.
   */
  readonly occlusion?: CullOcclusion;
  /**
   * Its tolerance, rad: its τ, px, over its px per radian, W ÷ (2 tan(fov_h ÷ 2)) as
   * `screenSpaceErrorPx` takes it, at which a DAG-cut cluster is drawn where its own error,
   * projected at its group's sphere, is within it and its parent's is not (plan R14, R14.T4.b).
   * A camera pass takes stride 1 alone until plan R14's T3.c.
   */
  readonly toleranceRad: number;
  /** The coarsest stride it may take: every camera pass takes 1 until plan R14's T3.c. */
  readonly strideCap: Stride;
}

/** An ordered camera pass's part of the two-pass occlusion (R14.T3.d). */
export type CullOcclusion = "early" | "late";

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
    view.setUint32(at, passKindCode(pass), true);
    view.setUint32(at + 4, Math.log2(pass.strideCap), true);
    view.setUint32(at + 8, place.bucketBase, true);
    view.setUint32(at + 12, place.listBase, true);
    view.setFloat32(at + 16, pass.kind === "cascade" ? pass.toleranceM : pass.toleranceRad, true);
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

/** A pass's kind as `clusterCull.wgsl`'s header holds it, 0 for none: a camera's by its order. */
function passKindCode(pass: CullPass): number {
  let code: number;
  switch (pass.kind) {
    case "cascade":
      code = 1;
      break;
    case "camera":
      code = pass.order === "appended" ? 2 : occlusionCode(pass.occlusion);
      break;
  }
  return code;
}

/** An ordered camera pass's kind code: 3, or 4 and 5 for the occlusion's early and late passes. */
function occlusionCode(occlusion: CullOcclusion | undefined): number {
  let code: number;
  switch (occlusion) {
    case undefined:
      code = 3;
      break;
    case "early":
      code = 4;
      break;
    case "late":
      code = 5;
      break;
  }
  return code;
}

/** A camera pass's planes after its scalars: their count, then each plane as a `vec4f`. */
function packCamera(view: DataView, at: number, pass: CameraCullPass): void {
  const most = pass.horizon === null ? MAX_CULL_PLANES : MAX_CULL_PLANES - 1;
  if (pass.planes.length > most) {
    throw new RangeError(
      `a camera pass has at most ${String(most)} planes, not ${String(pass.planes.length)}`,
    );
  }
  view.setFloat32(at + 20, pass.planes.length, true);
  const horizon = pass.horizon;
  if (horizon !== null) {
    view.setFloat32(at + 24, horizon.planeM, true);
    view.setFloat32(at + 28, horizon.sin, true);
    view.setFloat32(at + 112, horizon.axis.x, true);
    view.setFloat32(at + 116, horizon.axis.y, true);
    view.setFloat32(at + 120, horizon.axis.z, true);
    view.setFloat32(at + 124, horizon.cos, true);
  }
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
  /**
   * The code of the consumer's first shape (`CLUSTER_SHAPE_CODE`): 0 for terrain's and the
   * figures', 2 for a mesh's (R14.T4.b).
   */
  readonly firstShape: number;
  /**
   * Each shape code's may-discard template, codes 0 to {@link DISCARD_TEMPLATE_CODES} − 1,
   * {@link NO_DISCARD_TEMPLATE} where a record of it that may discard takes its strided template
   * (plan R14, R14.T1.g): terrain's `TERRAIN_DISCARD_TEMPLATES`, {@link NO_DISCARD_TEMPLATES}
   * elsewhere.
   */
  readonly discardTemplates: ReadonlyArray<number>;
  /**
   * The word of the buckets' arguments from which the camera passes' strides drawn past their τ′
   * are counted, one word a pass by its index among the buckets (plan R14, R14.T3.e;
   * `ClusterBuckets.overWord`, `clusters/overCount.ts`); absent or 0, none are counted.
   */
  readonly overWord?: number;
}

/**
 * Packs `params` into `out` ({@link CULL_PARAMS_BYTES} bytes at least), as `clusterCull.wgsl`'s
 * `CullParams`.
 *
 * @throws RangeError for more than {@link MAX_PASS_TEMPLATES} templates, offsets and capacities
 *   of different counts, or may-discard templates not a byte for each of
 *   {@link DISCARD_TEMPLATE_CODES} codes.
 */
export function packCullParams(params: CullParams, out: ArrayBuffer): void {
  const { templateOffsets, templateCapacities, discardTemplates } = params;
  if (
    templateOffsets.length > MAX_PASS_TEMPLATES ||
    templateOffsets.length !== templateCapacities.length
  ) {
    throw new RangeError(
      `${String(templateOffsets.length)} template offsets and ${String(templateCapacities.length)} capacities, at most ${String(MAX_PASS_TEMPLATES)} and as many of each`,
    );
  }
  if (
    discardTemplates.length !== DISCARD_TEMPLATE_CODES ||
    !discardTemplates.every((t) => Number.isInteger(t) && t >= 0 && t <= NO_DISCARD_TEMPLATE)
  ) {
    throw new RangeError(
      `may-discard templates ${discardTemplates.join(", ")} are not ${String(DISCARD_TEMPLATE_CODES)} bytes`,
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
    view.setUint32(144 + 4 * t, capacity, true);
  });
  view.setUint32(208, params.firstShape, true);
  discardTemplates.forEach((template, code) => {
    view.setUint8(212 + code, template);
  });
  view.setUint32(220, params.overWord ?? 0, true);
}
