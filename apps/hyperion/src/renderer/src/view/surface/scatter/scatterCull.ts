/**
 * Decorative scatter's cull against the bodies at the ground (plan R11, R11.T6.a; Design note 7):
 * which bodies cull it, and whether an instance meets one, the reference of `scatterCull.wgsl`.
 *
 * @remarks
 * - **Which bodies.** Every body whose bounding volume comes within
 *   {@link SCATTER_CULL_CLEARANCE_M} of the ground: a craft's hull box ({@link hullBox}), turned by
 *   its attitude, whose lowest point stands at most that far above the ground under its centre
 *   ({@link nearGround}), a grounded craft's and one hovering on its jets alike. At most
 *   {@link MAX_SCATTER_BODIES}, nearest the camera first.
 * - **The test** ({@link intersectsBody}): an instance's bounding sphere meets a box where its
 *   centre lies inside the box widened by its radius on each of the box's axes, so that no pebble
 *   pokes through a hull, a landing leg's pad or a skid, and one straddling a face of the box is
 *   culled while one beside it is not. The widening is the box's slab test with the sphere's radius
 *   added (a sphere meets a box only if its centre lies within its radius of every slab), which is
 *   conservative at the box's edges and corners by at most (√3 − 1) r: a pebble within that of a
 *   corner, and no nearer, goes undrawn.
 * - Pebbles against authoritative rocks are cosmetic and not culled (Design note 7).
 */

import { dot, type Vec3 } from "../../../geometry/vec3";
import type { KernelPair } from "../../engine/kernels";
import type { HullOutline } from "../../scene/hull";
import type { GroundUnderFunction } from "../../terrain/grounded";
import type { BodyFixedVec3 } from "../../terrain/planet";
import scatterCullWgsl from "./scatterCull.wgsl?raw";

/**
 * How near the ground a body's bounding volume must come, m, for decorative scatter to be culled
 * against it (Design note 7): 0.2 m, the authoritative height (`hyperion_surface::scatter::
 * AUTHORITATIVE_ROCK_HEIGHT`), past which no pebble, at most 0.15 m tall, reaches.
 */
export const SCATTER_CULL_CLEARANCE_M = 0.2;

/** The most bodies a frame culls its scatter against: the kernel's uniform array. */
export const MAX_SCATTER_BODIES = 8;

/** Bytes of one box as `scatterCull.wgsl`'s `ScatterBox` holds it: four `vec4f`. */
export const SCATTER_BOX_BYTES = 64;

/** `scatterCull.wgsl`: the cull's library, which the placement composes. */
export const SCATTER_CULL_WGSL: string = scatterCullWgsl;

/** An oriented box: its centre, its three unit axes and its half extent along each, m. */
export interface ScatterBox {
  readonly centreM: Vec3;
  readonly axes: readonly [Vec3, Vec3, Vec3];
  readonly halfExtentsM: readonly [number, number, number];
}

/** A hull's box in its own axes: its centre there and its half extents along x, y and z, m. */
export interface HullBox {
  readonly centreM: Vec3;
  readonly halfExtentsM: readonly [number, number, number];
}

/** The box of `hull`'s vertices in its own axes: their least and greatest along each axis. */
export function hullBox(hull: Pick<HullOutline, "vertices">): HullBox {
  if (hull.vertices.length === 0) {
    return { centreM: { x: 0, y: 0, z: 0 }, halfExtentsM: [0, 0, 0] };
  }
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const v of hull.vertices) {
    [v.x, v.y, v.z].forEach((c, k) => {
      lo[k] = Math.min(lo[k] ?? c, c);
      hi[k] = Math.max(hi[k] ?? c, c);
    });
  }
  const mid = (k: number): number => ((lo[k] ?? 0) + (hi[k] ?? 0)) / 2;
  const half = (k: number): number => ((hi[k] ?? 0) - (lo[k] ?? 0)) / 2;
  return {
    centreM: { x: mid(0), y: mid(1), z: mid(2) },
    halfExtentsM: [half(0), half(1), half(2)],
  };
}

/**
 * A craft's box (see the module's remarks): its hull's box placed at the craft's reference point
 * `positionM` and turned by `turn`, which takes a vector of the hull's axes into the frame
 * `positionM` is in (the craft's attitude, then the body's fixed axes for {@link nearGround}).
 */
export function craftBox(
  positionM: Vec3,
  turn: (hullVector: Vec3) => Vec3,
  box: HullBox,
): ScatterBox {
  const offset = turn(box.centreM);
  return {
    centreM: { x: positionM.x + offset.x, y: positionM.y + offset.y, z: positionM.z + offset.z },
    axes: [turn({ x: 1, y: 0, z: 0 }), turn({ x: 0, y: 1, z: 0 }), turn({ x: 0, y: 0, z: 1 })],
    halfExtentsM: box.halfExtentsM,
  };
}

/** How far `box` reaches from its centre along the unit `direction`: Σ |axis · n| half extent. */
export function boxReachM(box: ScatterBox, direction: Vec3): number {
  return box.axes.reduce(
    (sum, axis, k) => sum + Math.abs(dot(axis, direction)) * (box.halfExtentsM[k] ?? 0),
    0,
  );
}

/**
 * Whether a body-fixed `box` comes within {@link SCATTER_CULL_CLEARANCE_M} of the ground under its
 * centre (`ground`, the workers' ground where held): the centre's height above the ground less the
 * box's reach along the ground's normal, its lowest point's height, at most the clearance. A box
 * below the ground counts.
 */
export function nearGround(box: ScatterBox, ground: GroundUnderFunction): boolean {
  const centre: BodyFixedVec3 = box.centreM;
  const under = ground(centre);
  return under.heightM - boxReachM(box, under.normal) <= SCATTER_CULL_CLEARANCE_M;
}

/** An instance as the cull tests it: its centre and its bounding radius, m, the boxes' frame. */
export interface ScatterInstance {
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/** Whether the sphere of `instance` meets `box` (see the module's remarks): `scatter_box_meets`. */
export function meetsBox(instance: ScatterInstance, box: ScatterBox): boolean {
  const d = {
    x: instance.centreM.x - box.centreM.x,
    y: instance.centreM.y - box.centreM.y,
    z: instance.centreM.z - box.centreM.z,
  };
  return box.axes.every(
    (axis, k) => Math.abs(dot(d, axis)) <= (box.halfExtentsM[k] ?? 0) + instance.radiusM,
  );
}

/**
 * Whether `instance` meets any of `bodies`' boxes, and so is culled (Design note 7): the cull's
 * predicate, `pebbles.wgsl`'s loop over `scatter_box_meets`.
 */
export function intersectsBody(
  instance: ScatterInstance,
  bodies: ReadonlyArray<ScatterBox>,
): boolean {
  return bodies.some((box) => meetsBox(instance, box));
}

/**
 * Packs at most {@link MAX_SCATTER_BODIES} of `boxes` into `out` as `scatterCull.wgsl`'s
 * `ScatterBox`es ({@link SCATTER_BOX_BYTES} each, at least that many in `out`), zeros past them.
 *
 * @returns How many were packed.
 */
export function packScatterBoxes(boxes: ReadonlyArray<ScatterBox>, out: Float32Array): number {
  out.fill(0);
  const count = Math.min(boxes.length, MAX_SCATTER_BODIES, Math.floor(out.length / 16));
  for (let b = 0; b < count; b += 1) {
    const box = boxes[b];
    if (box === undefined) {
      continue;
    }
    const at = 16 * b;
    out.set([box.centreM.x, box.centreM.y, box.centreM.z, 0], at);
    box.axes.forEach((axis, k) => {
      out.set([axis.x, axis.y, axis.z, box.halfExtentsM[k] ?? 0], at + 4 * (k + 1));
    });
  }
  return count;
}

/** The cull's twin kernel's points a dispatch tests: its workgroup's size. */
const TWIN_GROUP = 64;

/**
 * The cull's GPU twin (R11.T6.b's check): `scatterCull.wgsl`'s `scatter_box_meets` over each point
 * of `points` (a `vec4f` each, its centre and radius) against `params.x` boxes of `bodies`, writing
 * 1 where it meets one and 0 elsewhere; `bit-exact`, one path, which the smoke harness holds to
 * {@link intersectsBody}.
 */
export const SCATTER_CULL_TWIN_KERNEL: KernelPair = {
  name: "scatter cull twin",
  reference: `${SCATTER_CULL_WGSL}
@group(0) @binding(0) var<uniform> params : vec4u;
@group(0) @binding(1) var<uniform> bodies : array<ScatterBox, 8>;
@group(0) @binding(2) var<storage, read> points : array<vec4f>;
@group(0) @binding(3) var<storage, read_write> met : array<u32>;

@compute @workgroup_size(${String(TWIN_GROUP)})
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= params.y) {
    return;
  }
  let point = points[id.x];
  var hit = 0u;
  for (var b = 0u; b < min(params.x, 8u); b = b + 1u) {
    if (scatter_box_meets(bodies[b], point.xyz, point.w)) {
      hit = 1u;
    }
  }
  met[id.x] = hit;
}
`,
  subgroup: null,
  readback: "bit-exact",
};

/** The twin kernel's workgroups for `points` points. */
export function scatterCullTwinWorkgroups(points: number): readonly [number, number, number] {
  return [Math.max(1, Math.ceil(points / TWIN_GROUP)), 1, 1];
}
