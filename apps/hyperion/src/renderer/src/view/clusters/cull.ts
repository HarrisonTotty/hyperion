/**
 * The cluster cull's kernel and its TypeScript twin (plan R14, R14.T1.b and T1.c, the DAG cut from
 * R14.T4.b; `clusterCull.wgsl`).
 *
 * @remarks
 * The twin decides as the kernel does, in `f64`: for each instance, each of its page's clusters and
 * each pass its instance may enter, whether the cluster's sphere may draw into the pass where any
 * receiver reads it, and at which template, and appends (instance, cluster) to that template's bucket. Its widening for the
 * kernel's `f32` arithmetic is the kernel's, scaled by `slackScale`: at 1 its decisions are the
 * kernel's but where a test falls within that rounding; at 0 it is the exact test, every cluster of
 * which the kernel must keep; at 2 it bounds every cluster the kernel may keep. The smoke harness
 * holds the device's buckets between the two (`smoke/clusterCull.ts`).
 *
 * The two-pass occlusion's ordered passes (R14.T3.d) take each cluster's visibility of the last
 * frame and, for the late pass, the early pass's marks and the depth pyramid's test
 * ({@link CullOcclusionTwin}); {@link cullVisibilityTwin} is the late pass's visibility.
 */

import type { Vec3 } from "../../geometry/vec3";
import type { KernelPair } from "../engine/kernels";
import clusterCullWgsl from "./clusterCull.wgsl?raw";
import clusterOcclusionWgsl from "./clusterOcclusion.wgsl?raw";
import type {
  CameraCullPass,
  CascadeCullPass,
  CullHorizon,
  CullPass,
  CullPassPlace,
} from "./cullPass";
import type { ClusterLod, ClusterRecord } from "./record";
import { CLUSTER_SHAPE_CODE } from "./record";
import { NO_DISCARD_TEMPLATE } from "./cullPass";
import { cameraStrideLevel, INSTANCE_FINEST, type StrideBandsM } from "./strides";
import { STRIDES } from "./templates";

/**
 * The plain cull's DAG LOD spheres (binding 8; R14.T4.b), and its stand-ins for the two-pass
 * occlusion's reads and writes (R14.T3.d): every cluster seen last frame, none refused, nothing
 * written, and no binding of the occlusion's.
 */
const NO_OCCLUSION = `
@group(0) @binding(8) var<storage, read> lods : array<vec4f>;

fn lod_of(at : u32) -> vec4f {
  return lods[at];
}

fn occlusion_seen(word : u32, bit : u32) -> bool {
  return true;
}

fn occlusion_refuses(centre : vec3f, radius : f32) -> bool {
  return false;
}

fn occlusion_write(word : u32, bit : u32, visible : bool) {}
`;

/** The cull's kernel: the reference alone, `presentation-only`, its appends' order not reproducible. */
export const CLUSTER_CULL_KERNEL: KernelPair = {
  name: "cluster cull",
  reference: clusterCullWgsl.replace("//OCCLUSION//", NO_OCCLUSION),
  subgroup: null,
  readback: "presentation-only",
};

/**
 * The cull with the two-pass occlusion's bindings and tests (plan R14, R14.T3.d;
 * `clusterOcclusion.wgsl`): the pyramid's parameters and texels and the clusters' visibility,
 * which terrain's lit pass binds; otherwise {@link CLUSTER_CULL_KERNEL}.
 */
export const CLUSTER_CULL_OCCLUSION_KERNEL: KernelPair = {
  name: "cluster cull occlusion",
  reference: clusterCullWgsl.replace("//OCCLUSION//", clusterOcclusionWgsl),
  subgroup: null,
  readback: "presentation-only",
};

/** The kernel's widening per metre of the lengths a test forms (`clusterCull.wgsl`'s). */
export const CULL_ROUNDING_FRACTION = 1e-6;

/** And its least widening, m. */
export const CULL_ROUNDING_M = 1;

/** The receivers' tests' least widening, m (T1.c): their lengths' ulps are the fraction's. */
export const RECEIVER_ROUNDING_M = 0.01;

/**
 * The horizon test's widening of a sphere's squared distance from the cone's axis, per square metre
 * of its squared distance from the camera (T2.a): above the `f32` rounding of |S|² − (S·axis)², a
 * few ulps of |S|² each (2⁻²⁴), so that the kernel never takes a sphere nearer the axis than it is.
 */
export const HORIZON_ROUNDING = 1e-6;

/** A cluster's mark where the ordered pass refuses it, or its instance does not enter the pass. */
export const NO_MARK = 0xffff_ffff;

/** The threads of the kernel's workgroup. */
export const CULL_WORKGROUP = 64;

/** One instance as the cull takes it: its origin less the camera, its page and its inputs. */
export interface CullInstance {
  /** The origin less the camera, in the passes' frame, m. */
  readonly originM: Vec3;
  readonly page: number;
  /** The passes it may enter, a bit a pass. */
  readonly passMask: number;
  /** Its error unit, m. */
  readonly unitM: number;
  /**
   * Whether each pass it enters keeps every cluster of it, its spheres untested
   * ({@link CULL_INPUT_WHOLE}; plan R10, R10.T16.i's fades and sub-patches); absent, not.
   */
  readonly whole?: boolean;
  /**
   * Its camera passes' stride bands, m (plan R14, R14.T3.c; terrain's `Instance.strides`); absent,
   * none: its camera passes draw it at stride 1.
   */
  readonly strideBandsM?: StrideBandsM;
  /**
   * Its instance record's quadrant word (terrain's `parentQuadrant`): its coarse–fine holds at
   * k = 0 in bits 2 to 5, a camera pass's stride at most 4 on those edges, and bit 18
   * (`INSTANCE_FINEST`), stride 1 (T3.c); absent, 0.
   */
  readonly quadrantWord?: number;
  /**
   * Whether it is a sub-patch of its page (terrain's `Instance.sub.x`, its low five bits not 0),
   * which shares its page's visibility with others: never marked by the occlusion's early pass,
   * and writing no visibility (T3.d); absent, not.
   */
  readonly subPatch?: boolean;
}

/** What the two-pass occlusion's twin reads of a dispatch (T3.d; `clusterCull.wgsl`'s head). */
export interface CullOcclusionTwin {
  /** Whether a page's cluster was visible last frame: its bit in the kernel's `history`. */
  readonly visible: (page: number, cluster: number) => boolean;
  /** The early pass's marks, as {@link cullMarksTwin} gives them, which the late pass reads. */
  readonly earlyMarks: Uint32Array | null;
  /**
   * Whether the late pass's pyramid refuses a sphere at `centreM`, camera-relative, of radius
   * `radiusM` (`hzb.ts`' `hzbRefuses` of its pyramid), or `null` for none, which refuses nothing.
   */
  readonly refuses: ((centreM: Vec3, radiusM: number) => boolean) | null;
}

/**
 * The bit of an instance's cull input (the passes' mask's word) by which each pass it enters keeps
 * every cluster of it, its records' spheres untested: terrain's instance whose heights fade from
 * another surface (plan R10, R10.T16.i; plan R14's design note on culling), which its slot's
 * records, of its newer bake alone, do not bound, and its sub-patch (`DrawnPatch.within`), whose
 * grid placed over a finer area they do not bound either.
 */
export const CULL_INPUT_WHOLE = 0x8000_0000;

/** What one dispatch culls, as the twin takes it. */
export interface CullTwinInput {
  readonly instances: ReadonlyArray<CullInstance>;
  /** Each page's records, a page's clusters in order. */
  readonly records: (page: number, cluster: number) => ClusterRecord;
  readonly clustersPerPage: number;
  /** The page's axes into the instances' frame, by column. */
  readonly rotationColumns: readonly [Vec3, Vec3, Vec3];
  /** The passes, `null` for one not culled for this dispatch. */
  readonly passes: ReadonlyArray<CullPass | null>;
  readonly places: ReadonlyArray<CullPassPlace>;
  /** The templates a pass has. */
  readonly templates: number;
  /** The view axis in the instances' frame, for T1.c's receivers' cull. */
  readonly viewAxis: Vec3;
  /** The consumer's first shape's code (`CullParams.firstShape`, R14.T4.b); 0 where absent. */
  readonly firstShape?: number;
  /**
   * Each shape code's may-discard template (`CullParams.discardTemplates`, R14.T1.g); none where
   * absent.
   */
  readonly discardTemplates?: ReadonlyArray<number>;
  /** Each DAG record's parent group's sphere (R14.T4.b), read by a DAG record alone. */
  readonly lods?: (page: number, cluster: number) => ClusterLod;
}

/** One bucket's entries as the twin appends them, (instance << 8) | cluster, in its order. */
export type CullTwinBuckets = ReadonlyMap<number, ReadonlyArray<number>>;

/** A record's centre turned into the instances' frame. */
function turned(columns: readonly [Vec3, Vec3, Vec3], v: Vec3): Vec3 {
  const [a, b, c] = columns;
  return {
    x: a.x * v.x + b.x * v.y + c.x * v.z,
    y: a.y * v.x + b.y * v.y + c.y * v.z,
    z: a.z * v.x + b.z * v.y + c.z * v.z,
  };
}

/**
 * Whether a sphere at `centreM`, camera-relative, of radius `radiusM`, may draw into cascade box
 * `pass`, widened by `slackScale` times the kernel's rounding allowance (`clusterCull.wgsl`'s
 * `cascade_keeps`).
 */
export function cascadeKeeps(
  pass: CascadeCullPass,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): boolean {
  const [xL, yL, zL] = pass.axes;
  const o = pass.centreFromCameraLightM;
  const vx = centreM.x * xL.x + centreM.y * xL.y + centreM.z * xL.z - o.x;
  const vy = centreM.x * yL.x + centreM.y * yL.y + centreM.z * yL.z - o.y;
  const vz = centreM.x * zL.x + centreM.y * zL.y + centreM.z * zL.z - o.z;
  const slack =
    slackScale *
    (CULL_ROUNDING_FRACTION *
      (Math.hypot(centreM.x, centreM.y, centreM.z) + Math.hypot(o.x, o.y, o.z) + pass.radiusM) +
      CULL_ROUNDING_M);
  const reach = pass.radiusM + radiusM + slack;
  return (
    Math.abs(vx) <= reach && Math.abs(vy) <= reach && vz >= -(pass.depthFarM + radiusM + slack)
  );
}

/**
 * Whether no receiver of cascade `pass` can read a texel a sphere at `centreM`, camera-relative, of
 * radius `radiusM` writes, the test narrowed by `slackScale` times the kernel's rounding allowance
 * (`clusterCull.wgsl`'s `receivers_refuse`; the rule is `clusters/receivers.ts`'): all its receivers
 * beyond the cascade's far end, or all at or below the next nearer cascade's fade start and inside
 * its covered square. Never where the pass's receivers' cull is off.
 *
 * @param viewAxis - The view axis, a unit vector in `centreM`'s frame.
 */
export function receiversRefuse(
  pass: CascadeCullPass,
  viewAxis: Vec3,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): boolean {
  const receivers = pass.receivers;
  if (receivers === null) {
    return false;
  }
  const [near, far] = receivers.depthM;
  const [nearer, farther] = receivers.reachM;
  const below = receivers.coveredBelow;
  const d = centreM.x * viewAxis.x + centreM.y * viewAxis.y + centreM.z * viewAxis.z;
  const slack =
    slackScale *
    (CULL_ROUNDING_FRACTION *
      (Math.hypot(centreM.x, centreM.y, centreM.z) +
        Math.abs(near) +
        Math.abs(far) +
        (below === null ? 0 : Math.hypot(below.centreM[0], below.centreM[1]) + below.halfSideM)) +
      RECEIVER_ROUNDING_M);
  if (d - radiusM - nearer - slack > far) {
    return true;
  }
  if (below === null || d + radiusM + farther + slack > near) {
    return false;
  }
  const [xL, yL] = pass.axes;
  const x = centreM.x * xL.x + centreM.y * xL.y + centreM.z * xL.z - below.centreM[0];
  const y = centreM.x * yL.x + centreM.y * yL.y + centreM.z * yL.z - below.centreM[1];
  const reach = radiusM + below.reachM + slack;
  return Math.abs(x) + reach <= below.halfSideM && Math.abs(y) + reach <= below.halfSideM;
}

/**
 * Whether a sphere at `centreM`, camera-relative, of radius `radiusM`, lies wholly behind
 * `horizon`, the test narrowed by `slackScale` times the kernel's rounding allowance
 * (`clusterCull.wgsl`'s `horizon_refuses`; T2.a): wholly beyond the plane through the tangent
 * circle and wholly inside the cone, a·sin α − d·cos α > ρ, a its distance along the axis and d
 * from it.
 */
export function horizonRefuses(
  horizon: CullHorizon,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): boolean {
  const span = centreM.x * centreM.x + centreM.y * centreM.y + centreM.z * centreM.z;
  const { axis } = horizon;
  const a = centreM.x * axis.x + centreM.y * axis.y + centreM.z * axis.z;
  const slack =
    slackScale * (CULL_ROUNDING_FRACTION * (Math.sqrt(span) + horizon.planeM) + CULL_ROUNDING_M);
  if (a - radiusM - slack <= horizon.planeM) {
    return false;
  }
  const d = Math.sqrt(Math.max(span - a * a, 0) + slackScale * HORIZON_ROUNDING * span);
  return a * horizon.sin - d * horizon.cos > radiusM + slack;
}

/**
 * Whether a sphere at `centreM`, camera-relative, of radius `radiusM`, may show in camera pass
 * `pass`, widened by `slackScale` times the kernel's rounding allowance (`clusterCull.wgsl`'s
 * `camera_keeps`): about the camera, or not wholly outside any plane nor wholly behind the horizon.
 */
export function cameraKeeps(
  pass: CameraCullPass,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): boolean {
  const distanceM = Math.hypot(centreM.x, centreM.y, centreM.z);
  const reach = radiusM + slackScale * (CULL_ROUNDING_FRACTION * distanceM + CULL_ROUNDING_M);
  if (distanceM <= reach) {
    return true;
  }
  return (
    pass.planes.every(
      ({ normal: n, offsetM }) =>
        n.x * centreM.x + n.y * centreM.y + n.z * centreM.z + offsetM >= -reach,
    ) &&
    (pass.horizon === null || !horizonRefuses(pass.horizon, centreM, radiusM, slackScale))
  );
}

/**
 * Whether a sphere may draw into `pass` where anything reads it, by the pass's kind, at
 * `slackScale` times the kernel's allowance (`pass_keeps`).
 */
export function passKeeps(
  pass: CullPass,
  viewAxis: Vec3,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): boolean {
  let keeps: boolean;
  switch (pass.kind) {
    case "cascade":
      keeps =
        cascadeKeeps(pass, centreM, radiusM, slackScale) &&
        !receiversRefuse(pass, viewAxis, centreM, radiusM, slackScale);
      break;
    case "camera":
      keeps = cameraKeeps(pass, centreM, radiusM, slackScale);
      break;
  }
  return keeps;
}

/** The distance from the camera to the nearest point of the sphere at `c` of radius `r`: 0 inside. */
function distance(c: Vec3, r: number): number {
  return Math.max(Math.hypot(c.x, c.y, c.z) - r, 0);
}

/**
 * Whether a DAG record is drawn at `pass`'s tolerance (`clusterCull.wgsl`'s `dag_drawn`; R14.T4.b):
 * its own error within it at its group's sphere, the record's, at `centreM`, camera-relative, and
 * its parent's not at its parent group's, at `parentCentreM` (a camera pass's errors over the
 * distance to each sphere's nearest point, 0 inside it; a cascade's in metres).
 */
export function dagCutDrawn(
  pass: CullPass,
  errorsM: readonly [own: number, parent: number],
  centreM: Vec3,
  radiusM: number,
  parentCentreM: Vec3,
  parentRadiusM: number,
): boolean {
  const [own, parent] = errorsM;
  if (pass.kind === "cascade") {
    return own <= pass.toleranceM && parent > pass.toleranceM;
  }
  return (
    own <= pass.toleranceRad * distance(centreM, radiusM) &&
    parent > pass.toleranceRad * distance(parentCentreM, parentRadiusM)
  );
}

/**
 * The coarsest stride's level, up to `pass`'s cap, whose 2^l × `unitM` is within its tolerance,
 * both narrowed to `f32` as the kernel holds them (`stride_level`); 0 where none is.
 */
export function strideLevel(
  pass: Pick<CascadeCullPass, "strideCap" | "toleranceM">,
  unitM: number,
): number {
  const unit = Math.fround(unitM);
  const tolerance = Math.fround(pass.toleranceM);
  const cap = Math.min(Math.log2(pass.strideCap), STRIDES.length - 1);
  let level = 0;
  for (let l = 1; l <= cap; l += 1) {
    if (Math.fround(2 ** l * unit) <= tolerance) {
      level = l;
    }
  }
  return level;
}

/**
 * Each instance's clusters' sphere centres camera-relative in the instances' frame, as the kernel
 * places them, with their records, for `visit`.
 */
function eachCluster(
  input: CullTwinInput,
  visit: (
    n: number,
    instance: CullInstance,
    cluster: number,
    record: ClusterRecord,
    centre: Vec3,
  ) => void,
): void {
  input.instances.forEach((instance, n) => {
    for (let cluster = 0; cluster < input.clustersPerPage; cluster += 1) {
      const record = input.records(instance.page, cluster);
      const offset = turned(input.rotationColumns, record.centreM);
      visit(n, instance, cluster, record, {
        x: instance.originM.x + offset.x,
        y: instance.originM.y + offset.y,
        z: instance.originM.z + offset.z,
      });
    }
  });
}

/**
 * The edges a terrain block or strip lies on, bits 0 to 3 for x = 0, x = 64, y = 0 and y = 64, by
 * its template reference (`clusterCull.wgsl`'s `record_edges`), 0 for any other shape.
 */
export function recordEdges(record: ClusterRecord): number {
  const ref = record.templateRef;
  if (record.shape === "terrain-block") {
    const x0 = ref & 0xff;
    const y0 = (ref >> 8) & 0xff;
    return (x0 === 0 ? 1 : 0) | (x0 === 56 ? 2 : 0) | (y0 === 0 ? 4 : 0) | (y0 === 56 ? 8 : 0);
  }
  if (record.shape === "terrain-skirt") {
    // Edge 0 y = 0, 1 x = 64, 2 y = 64, 3 x = 0 (`gpu/resources.ts`' numbering).
    return [4, 2, 8, 1][(ref >>> 16) & 3] ?? 0;
  }
  return 0;
}

/**
 * A camera pass's stride's level for `record` of `instance` at `centreM`, camera-relative
 * (`camera_stride_level`; plan R14, R14.T3.c): 0 for an instance with no bands, held at stride 1 or
 * kept whole (its heights fading from another surface, which its spheres do not bound), else
 * `cameraStrideLevel` at the sphere's nearest point, up to the pass's cap and to 2 on an edge a
 * finer patch touches.
 */
export function cameraPassStrideLevel(
  pass: CameraCullPass,
  record: ClusterRecord,
  instance: CullInstance,
  centreM: Vec3,
): number {
  const quadrant = instance.quadrantWord ?? 0;
  const bands = instance.strideBandsM;
  if (bands === undefined || instance.whole === true || (quadrant & INSTANCE_FINEST) !== 0) {
    return 0;
  }
  let cap = Math.min(Math.log2(pass.strideCap), STRIDES.length - 1);
  if ((recordEdges(record) & ((quadrant >>> 2) & 0xf)) !== 0) {
    cap = Math.min(cap, 2);
  }
  return cameraStrideLevel(bands, cap, distance(centreM, record.radiusM));
}

/**
 * A record's template in `pass` (`template_of`): its shape's may-discard template where it may
 * discard and the consumer has one (R14.T1.g); else its shape's less the consumer's first at a
 * cascade's stride for `instance`'s unit, at a camera pass's for its bands (T3.c), and at stride 1
 * for a DAG record; +∞ for a shape below the first, which the kernel's unsigned arithmetic takes
 * past every template.
 */
function templateOf(
  record: ClusterRecord,
  pass: CullPass,
  instance: CullInstance,
  input: CullTwinInput,
  centreM: Vec3,
): number {
  const code = CLUSTER_SHAPE_CODE[record.shape];
  const discarding = record.mayDiscard ? input.discardTemplates?.[code] : undefined;
  if (discarding !== undefined && discarding !== NO_DISCARD_TEMPLATE) {
    return discarding;
  }
  const shape = code - (input.firstShape ?? 0);
  const level =
    record.mode === "dag"
      ? 0
      : pass.kind === "camera"
        ? cameraPassStrideLevel(pass, record, instance, centreM)
        : strideLevel(pass, instance.unitM);
  return shape < 0 ? Infinity : shape * STRIDES.length + level;
}

/**
 * Whether `pass` keeps `record`, at `centreM`, camera-relative, for `instance` (see the kernel's
 * head): a cluster at all, its instance entering the pass, its sphere kept, and a DAG record on
 * the cut.
 */
function recordKept(
  input: CullTwinInput,
  pass: CullPass,
  p: number,
  instance: CullInstance,
  cluster: number,
  record: ClusterRecord,
  centreM: Vec3,
  slackScale: number,
): boolean {
  if (
    !(record.radiusM >= 0) ||
    (instance.passMask & (1 << p)) === 0 ||
    (instance.whole !== true &&
      !passKeeps(pass, input.viewAxis, centreM, record.radiusM, slackScale))
  ) {
    return false;
  }
  if (record.mode !== "dag") {
    return true;
  }
  const lod = input.lods?.(instance.page, cluster);
  if (lod === undefined) {
    throw new Error(`the DAG record of cluster ${String(cluster)} has no LOD sphere`);
  }
  const offset = turned(input.rotationColumns, lod.parentCentreM);
  const parentCentre = {
    x: instance.originM.x + offset.x,
    y: instance.originM.y + offset.y,
    z: instance.originM.z + offset.z,
  };
  return dagCutDrawn(
    pass,
    record.errorsM,
    centreM,
    record.radiusM,
    parentCentre,
    lod.parentRadiusM,
  );
}

/** The dispatch's ordered pass and its index, or `null` where it has none. */
function orderedPass(
  input: CullTwinInput,
): { readonly pass: CameraCullPass; readonly p: number } | null {
  const p = input.passes.findIndex((pass) => pass?.kind === "camera" && pass.order === "ordered");
  const pass = input.passes[p] ?? null;
  return pass?.kind === "camera" ? { pass, p } : null;
}

/**
 * Whether the occlusion's late pass sees a kept cluster (T3.d): its instance kept whole, or its
 * sphere not refused by the pyramid.
 */
function lateSees(
  occlusion: CullOcclusionTwin,
  instance: CullInstance,
  centreM: Vec3,
  radiusM: number,
): boolean {
  return (
    instance.whole === true || occlusion.refuses === null || !occlusion.refuses(centreM, radiusM)
  );
}

/**
 * The twin of one dispatch's ordered pass (T2.a): each cluster's mark, instance by instance and
 * cluster by cluster, its template where the pass keeps it, else {@link NO_MARK}; every mark
 * {@link NO_MARK} where the dispatch has no ordered pass. The occlusion's early pass keeps only
 * the clusters `occlusion` holds visible last frame, of instances that are not sub-patches; its late
 * pass only those the early pass did not mark and the pyramid does not refuse (T3.d).
 *
 * @throws Error for an occlusion's pass without `occlusion`, or a late pass without the early
 *   pass's marks.
 */
export function cullMarksTwin(
  input: CullTwinInput,
  slackScale = 1,
  occlusion?: CullOcclusionTwin,
): Uint32Array {
  const marks = new Uint32Array(input.instances.length * input.clustersPerPage).fill(NO_MARK);
  const ordered = orderedPass(input);
  if (ordered === null) {
    return marks;
  }
  const { pass, p } = ordered;
  const phase = pass.occlusion;
  if (
    phase !== undefined &&
    ((occlusion?.earlyMarks === null && phase === "late") || occlusion === undefined)
  ) {
    throw new Error(`the occlusion's ${phase} pass needs its visibility and the early marks`);
  }
  eachCluster(input, (n, instance, cluster, record, centre) => {
    if (!recordKept(input, pass, p, instance, cluster, record, centre, slackScale)) {
      return;
    }
    const at = n * input.clustersPerPage + cluster;
    const t = templateOf(record, pass, instance, input, centre);
    const marked =
      t < input.templates &&
      (occlusion === undefined ||
        phase === undefined ||
        (phase === "early"
          ? instance.subPatch !== true && occlusion.visible(instance.page, cluster)
          : (occlusion.earlyMarks?.[at] ?? NO_MARK) === NO_MARK &&
            lateSees(occlusion, instance, centre, record.radiusM)));
    if (marked) {
      marks[at] = t;
    }
  });
  return marks;
}

/**
 * The twin of the occlusion's late pass's visibility (T3.d): for each cluster of each instance that
 * is not a sub-patch, keyed page × clusters a page + cluster, whether the pass keeps it at a
 * template and the pyramid does not refuse it, as the kernel writes its bit; empty where the
 * dispatch has no late pass.
 */
export function cullVisibilityTwin(
  input: CullTwinInput,
  occlusion: CullOcclusionTwin,
  slackScale = 1,
): ReadonlyMap<number, boolean> {
  const visible = new Map<number, boolean>();
  const ordered = orderedPass(input);
  if (ordered?.pass.occlusion !== "late") {
    return visible;
  }
  const { pass, p } = ordered;
  eachCluster(input, (_n, instance, cluster, record, centre) => {
    if (instance.subPatch === true) {
      return;
    }
    const kept =
      recordKept(input, pass, p, instance, cluster, record, centre, slackScale) &&
      templateOf(record, pass, instance, input, centre) < input.templates &&
      lateSees(occlusion, instance, centre, record.radiusM);
    visible.set(instance.page * input.clustersPerPage + cluster, kept);
  });
  return visible;
}

/**
 * The twin of one dispatch: each bucket's entries, keyed by bucket, in the order a serial kernel
 * would append them (instance by instance, cluster by cluster, pass by pass). An ordered pass
 * appends nothing: its marks are {@link cullMarksTwin}'s.
 */
export function cullTwin(input: CullTwinInput, slackScale = 1): CullTwinBuckets {
  const buckets = new Map<number, number[]>();
  input.instances.forEach((instance, n) => {
    for (let cluster = 0; cluster < input.clustersPerPage; cluster += 1) {
      const record = input.records(instance.page, cluster);
      const offset = turned(input.rotationColumns, record.centreM);
      const centre = {
        x: instance.originM.x + offset.x,
        y: instance.originM.y + offset.y,
        z: instance.originM.z + offset.z,
      };
      input.passes.forEach((pass, p) => {
        if (
          pass === null ||
          (pass.kind === "camera" && pass.order === "ordered") ||
          !recordKept(input, pass, p, instance, cluster, record, centre, slackScale)
        ) {
          return;
        }
        const t = templateOf(record, pass, instance, input, centre);
        if (t >= input.templates) {
          return;
        }
        const bucket = (input.places[p]?.bucketBase ?? 0) + t;
        const list = buckets.get(bucket) ?? [];
        list.push(n * 256 + cluster);
        buckets.set(bucket, list);
      });
    }
  });
  return buckets;
}
