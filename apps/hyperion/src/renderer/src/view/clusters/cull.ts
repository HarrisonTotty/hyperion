/**
 * The cluster cull's kernel and its TypeScript twin (plan R14, R14.T1.b and T1.c;
 * `clusterCull.wgsl`).
 *
 * @remarks
 * The twin decides as the kernel does, in `f64`: for each instance, each of its page's clusters and
 * each pass its instance may enter, whether the cluster's sphere may draw into the pass where any
 * receiver reads it, and at which template, and appends (instance, cluster) to that template's bucket. Its widening for the
 * kernel's `f32` arithmetic is the kernel's, scaled by `slackScale`: at 1 its decisions are the
 * kernel's but where a test falls within that rounding; at 0 it is the exact test, every cluster of
 * which the kernel must keep; at 2 it bounds every cluster the kernel may keep. The smoke harness
 * holds the device's buckets between the two (`smoke/clusterCull.ts`).
 */

import type { Vec3 } from "../../geometry/vec3";
import type { KernelPair } from "../engine/kernels";
import clusterCullWgsl from "./clusterCull.wgsl?raw";
import type { CascadeCullPass, CullPassPlace } from "./cullPass";
import type { ClusterRecord } from "./record";
import { CLUSTER_SHAPE_CODE } from "./record";
import { STRIDES } from "./templates";

/** The cull's kernel: the reference alone, `presentation-only`, its appends' order not reproducible. */
export const CLUSTER_CULL_KERNEL: KernelPair = {
  name: "cluster cull",
  reference: clusterCullWgsl,
  subgroup: null,
  readback: "presentation-only",
};

/** The kernel's widening per metre of the lengths a test forms (`clusterCull.wgsl`'s). */
export const CULL_ROUNDING_FRACTION = 1e-6;

/** And its least widening, m. */
export const CULL_ROUNDING_M = 1;

/** The receivers' tests' least widening, m (T1.c): their lengths' ulps are the fraction's. */
export const RECEIVER_ROUNDING_M = 0.01;

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
}

/** What one dispatch culls, as the twin takes it. */
export interface CullTwinInput {
  readonly instances: ReadonlyArray<CullInstance>;
  /** Each page's records, a page's clusters in order. */
  readonly records: (page: number, cluster: number) => ClusterRecord;
  readonly clustersPerPage: number;
  /** The page's axes into the instances' frame, by column. */
  readonly rotationColumns: readonly [Vec3, Vec3, Vec3];
  readonly passes: ReadonlyArray<CascadeCullPass>;
  readonly places: ReadonlyArray<CullPassPlace>;
  /** The templates a pass has. */
  readonly templates: number;
  /** The view axis in the instances' frame, for T1.c's receivers' cull. */
  readonly viewAxis: Vec3;
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
 * The twin of one dispatch: each bucket's entries, keyed by bucket, in the order a serial kernel
 * would append them (instance by instance, cluster by cluster, pass by pass).
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
          (instance.passMask & (1 << p)) === 0 ||
          !cascadeKeeps(pass, centre, record.radiusM, slackScale) ||
          receiversRefuse(pass, input.viewAxis, centre, record.radiusM, slackScale)
        ) {
          return;
        }
        const t =
          CLUSTER_SHAPE_CODE[record.shape] * STRIDES.length + strideLevel(pass, instance.unitM);
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
