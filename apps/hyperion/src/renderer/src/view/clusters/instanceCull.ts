/**
 * The instance cull's kernels, their parameters and their TypeScript twins (plan R14, R14.T5.d):
 * an instance's bounds tested against every pass before any cluster of it.
 *
 * @remarks
 * Two outputs of the one test (`instanceCull.wgsl`, `instanceCompact.wgsl`):
 * - **Particles** ({@link INSTANCE_CULL_KERNEL}): an instance is one cluster, a template a size
 *   (`particles.ts`), so the instance cull is its whole cull: each pass that keeps its sphere
 *   appends its index to the (pass, template) bucket of its projected size.
 * - **Mesh instances** ({@link INSTANCE_COMPACT_KERNEL}, then {@link INSTANCE_FOLD_KERNEL}): the
 *   instances any pass keeps are compacted with the passes that keep them, and the cluster cull
 *   dispatched indirectly over them alone, folded past 65,535 (`consumer.ts`'
 *   `ClusterConsumerPath.cullInstances`), so that a refused instance costs a thread, not a
 *   workgroup of 64.
 *
 * Both compose the cluster cull's pass tests (`cull.ts`' `CULL_PASS_TESTS_WGSL`), so that the twins
 * decide by `cull.ts`' `passKeeps`, whose `slackScale` widens the kernel's rounding allowance as the
 * cluster cull's twin does: at 0 the exact `f64` test, every instance of which the kernel must
 * keep, at 2 a bound of every instance it may keep. The template's choice is `f64` here and `f32`
 * on the GPU, so the two may part where an error falls within the rounding of its tolerance
 * ({@link instanceTemplateNear}).
 */

import type { Vec3 } from "../../geometry/vec3";
import type { KernelPair } from "../engine/kernels";
import { CULL_PASS_TESTS_WGSL, passKeeps } from "./cull";
import type { CullPass, CullPassPlace } from "./cullPass";
import { MAX_CULL_PASSES } from "./cullPass";
import { FOLDED_GROUP_WGSL, foldedIndex, foldedWorkgroups } from "./fold";
import instanceCompactWgsl from "./instanceCompact.wgsl?raw";
import instanceCullWgsl from "./instanceCull.wgsl?raw";
import instanceFoldWgsl from "./instanceFold.wgsl?raw";
import { MAX_PARTICLE_TEMPLATES } from "./particles";

/**
 * The instance kernels' absolute allowance, m (`instanceCull.wgsl`'s `ROUNDING_M`): a millimetre, an
 * instance's centre being the `f32` its producer wrote, where the cluster cull's metre covers a
 * terrain patch's narrowed offsets (`cull.ts`' `CULL_ROUNDING_M`). With the cluster cull's metre the
 * test field of 10⁶ particles at 2 to 200 m drew some 28% more instances than the exact test keeps.
 */
export const INSTANCE_ROUNDING_M = 0.001;

/** The threads of the instance kernels' workgroups, an instance each. */
export const INSTANCE_CULL_WORKGROUP = 64;

/** The particles' cull: `presentation-only`, its appends' order not reproducible. */
export const INSTANCE_CULL_KERNEL: KernelPair = {
  name: "instance cull",
  reference: FOLDED_GROUP_WGSL + CULL_PASS_TESTS_WGSL + instanceCullWgsl,
  subgroup: null,
  readback: "presentation-only",
};

/** The mesh instances' cull above the cluster cull: `presentation-only`, its order not reproducible. */
export const INSTANCE_COMPACT_KERNEL: KernelPair = {
  name: "instance compact",
  reference: FOLDED_GROUP_WGSL + CULL_PASS_TESTS_WGSL + instanceCompactWgsl,
  subgroup: null,
  readback: "presentation-only",
};

/** The survivors' fold into the cluster cull's indirect dispatch: integer, `bit-exact`. */
export const INSTANCE_FOLD_KERNEL: KernelPair = {
  name: "instance fold",
  reference: instanceFoldWgsl,
  subgroup: null,
  readback: "bit-exact",
};

/** The timed label of the particles' cull: one dispatch a frame. */
export const INSTANCE_CULL_LABEL = "instance cull";

/** The workgroups of an instance kernel over `capacity` instances, folded (`fold.ts`). */
export function instanceWorkgroups(capacity: number): readonly [number, number, 1] {
  return foldedWorkgroups(Math.ceil(capacity / INSTANCE_CULL_WORKGROUP));
}

/** Bytes of `instanceCull.wgsl`'s `InstanceCullParams`. */
export const INSTANCE_CULL_PARAMS_BYTES = 96;

/** Bytes of `instanceCompact.wgsl`'s `InstanceCompactParams`. */
export const INSTANCE_COMPACT_PARAMS_BYTES = 32;

/** Bytes of `instanceFold.wgsl`'s `InstanceFoldParams`. */
export const INSTANCE_FOLD_PARAMS_BYTES = 16;

/** Where an instance kernel finds an instance's bounds in its record. */
export interface InstanceLayout {
  /** The instances it may take: the producer's live count is at most this. */
  readonly capacity: number;
  /** `u32` words of a record; its first three the centre (or origin) less the camera. */
  readonly words: number;
  /** The word of the bounding sphere's radius, m. */
  readonly radiusWord: number;
}

/** The particles' cull's parameters (`instanceCull.wgsl`'s head). */
export interface InstanceCullParams extends InstanceLayout {
  readonly passes: number;
  /** The view axis in the passes' frame, which a cascade's receivers' cull reads. */
  readonly viewAxis: Vec3;
  /** Each template's error fraction, coarsest first (`particles.ts`). */
  readonly errorFractions: ReadonlyArray<number>;
  /** Each template's first list entry within its pass, and its capacity (`ClusterBuckets`). */
  readonly templateOffsets: ReadonlyArray<number>;
  readonly templateCapacities: ReadonlyArray<number>;
  /** The passes every instance may enter, a bit a pass. */
  readonly passMask: number;
  /** An instance's bounds over its radius, at least 1 (`particles.ts`' `boundsScale`). */
  readonly boundsScale: number;
}

/** Checks an instance layout's numbers, naming what is wrong. */
function checkLayout(layout: InstanceLayout, passes: number): void {
  if (
    !Number.isInteger(layout.capacity) ||
    layout.capacity < 0 ||
    !Number.isInteger(layout.words) ||
    layout.words < 4 ||
    !Number.isInteger(layout.radiusWord) ||
    layout.radiusWord < 3 ||
    layout.radiusWord >= layout.words ||
    !Number.isInteger(passes) ||
    passes < 0 ||
    passes > MAX_CULL_PASSES
  ) {
    throw new RangeError(
      `instances of ${String(layout.words)} words, their radius at word ${String(layout.radiusWord)}, ${String(layout.capacity)} of them over ${String(passes)} passes: the radius past the centre, within the record, and at most ${String(MAX_CULL_PASSES)} passes`,
    );
  }
}

/**
 * Packs `params` into `out` ({@link INSTANCE_CULL_PARAMS_BYTES} at least), as `InstanceCullParams`.
 *
 * @throws RangeError for a layout past its record, more than {@link MAX_CULL_PASSES} passes,
 *   templates not 1 to {@link MAX_PARTICLE_TEMPLATES}, one error, offset and capacity each, or a
 *   bounds scale below 1.
 */
export function packInstanceCullParams(params: InstanceCullParams, out: ArrayBuffer): void {
  checkLayout(params, params.passes);
  if (!(params.boundsScale >= 1 && Number.isFinite(params.boundsScale))) {
    throw new RangeError(
      `an instance's bounds scale is at least 1, not ${String(params.boundsScale)}`,
    );
  }
  const t = params.errorFractions.length;
  if (
    t < 1 ||
    t > MAX_PARTICLE_TEMPLATES ||
    params.templateOffsets.length !== t ||
    params.templateCapacities.length !== t
  ) {
    throw new RangeError(
      `${String(t)} templates' errors, ${String(params.templateOffsets.length)} offsets and ${String(params.templateCapacities.length)} capacities: 1 to ${String(MAX_PARTICLE_TEMPLATES)}, one of each a template`,
    );
  }
  const view = new DataView(out);
  new Uint8Array(out, 0, INSTANCE_CULL_PARAMS_BYTES).fill(0);
  view.setUint32(0, params.capacity, true);
  view.setUint32(4, params.words, true);
  view.setUint32(8, params.radiusWord, true);
  view.setUint32(12, params.passes, true);
  view.setFloat32(16, params.viewAxis.x, true);
  view.setFloat32(20, params.viewAxis.y, true);
  view.setFloat32(24, params.viewAxis.z, true);
  view.setUint32(28, t, true);
  for (let k = 0; k < t; k += 1) {
    view.setFloat32(32 + 4 * k, params.errorFractions[k] ?? 0, true);
    view.setUint32(48 + 4 * k, params.templateOffsets[k] ?? 0, true);
    view.setUint32(64 + 4 * k, params.templateCapacities[k] ?? 0, true);
  }
  view.setUint32(80, params.passMask >>> 0, true);
  view.setFloat32(84, params.boundsScale, true);
}

/** The mesh instances' cull's parameters (`instanceCompact.wgsl`'s head). */
export interface InstanceCompactParams extends InstanceLayout {
  readonly passes: number;
  readonly viewAxis: Vec3;
  /** The passes every instance may enter, a bit a pass. */
  readonly passMask: number;
}

/**
 * Packs `params` into `out` ({@link INSTANCE_COMPACT_PARAMS_BYTES} at least), as
 * `InstanceCompactParams`.
 *
 * @throws RangeError for a layout past its record or more than {@link MAX_CULL_PASSES} passes.
 */
export function packInstanceCompactParams(params: InstanceCompactParams, out: ArrayBuffer): void {
  checkLayout(params, params.passes);
  const view = new DataView(out);
  new Uint8Array(out, 0, INSTANCE_COMPACT_PARAMS_BYTES).fill(0);
  view.setUint32(0, params.capacity, true);
  view.setUint32(4, params.words, true);
  view.setUint32(8, params.radiusWord, true);
  view.setUint32(12, params.passes, true);
  view.setFloat32(16, params.viewAxis.x, true);
  view.setFloat32(20, params.viewAxis.y, true);
  view.setFloat32(24, params.viewAxis.z, true);
  view.setUint32(28, params.passMask >>> 0, true);
}

/**
 * The places past `capacity` survivors a fold of them may reach, fewer than its rows: the compacted
 * buffers' tail, which the fold's kernel zeroes.
 */
export function foldTail(capacity: number): number {
  return foldedWorkgroups(capacity)[1];
}

/** One instance's bounds as the twins take them: its centre less the camera and its radius. */
export interface InstanceBounds {
  /** In the passes' frame, m. */
  readonly centreM: Vec3;
  /** m; negative for no instance. */
  readonly radiusM: number;
}

/** What one dispatch of the particles' cull culls, as its twin takes it. */
export interface InstanceCullTwinInput {
  /** The instances, at least `count` of them: those past it are not read. */
  readonly instances: ReadonlyArray<InstanceBounds>;
  /** The producer's live count, `instance_count[0]`. */
  readonly count: number;
  /** The path's capacity, `counts.x`. */
  readonly capacity: number;
  /** The passes, `null` for one not culled this frame. */
  readonly passes: ReadonlyArray<CullPass | null>;
  /** Each pass's first bucket and first list entry (`ClusterBuckets.place`). */
  readonly places: ReadonlyArray<CullPassPlace>;
  readonly viewAxis: Vec3;
  readonly errorFractions: ReadonlyArray<number>;
  /** An instance's bounds over its radius (`particles.ts`' `boundsScale`). */
  readonly boundsScale: number;
  /** Each template's first list entry within its pass, and its capacity. */
  readonly templateOffsets: ReadonlyArray<number>;
  readonly templateCapacities: ReadonlyArray<number>;
  readonly passMask: number;
  /** The lists' entries, every pass's (`ClusterBuckets`' `passes × passEntries`). */
  readonly listEntries: number;
  /**
   * The dispatch's workgroups along x and y, its fold; {@link instanceWorkgroups} of the capacity
   * where absent, as the path dispatches it. Any fold covering the capacity fills the same buckets.
   */
  readonly fold?: readonly [number, number];
}

/** The particles' cull's writes as the twin makes them: a bucket's count by its index, and the lists. */
export interface InstanceCullTwinWrites {
  readonly counts: Uint32Array;
  readonly lists: Uint32Array;
}

/**
 * The particles' cull as `instanceCull.wgsl` runs it, in `f64` (see the module's remarks): the
 * fold's workgroups row by row, each its 64 instances' appends counted a (pass, template) and placed
 * after the bucket's count so far, as the kernel's one global add a workgroup does, in the order the
 * twin takes the workgroups (the device's varies with its scheduling, so a check compares a
 * bucket's entries as a set). Its widening is the kernel's times `slackScale`.
 */
export function instanceCullTwin(
  input: InstanceCullTwinInput,
  slackScale = 1,
): InstanceCullTwinWrites {
  const templates = Math.min(input.errorFractions.length, MAX_PARTICLE_TEMPLATES);
  const passCount = Math.min(input.passes.length, MAX_CULL_PASSES);
  const buckets = Math.max(
    0,
    ...input.places.slice(0, passCount).map((place) => place.bucketBase + templates),
  );
  const counts = new Uint32Array(buckets);
  const lists = new Uint32Array(input.listEntries);
  const [x, y] = input.fold ?? instanceWorkgroups(input.capacity);
  const live = Math.min(input.count, input.capacity);
  const group = new Uint32Array(MAX_CULL_PASSES * MAX_PARTICLE_TEMPLATES);
  const kept: Array<{
    readonly instance: number;
    readonly pass: number;
    readonly template: number;
    readonly slot: number;
  }> = [];
  for (let gy = 0; gy < y; gy += 1) {
    for (let gx = 0; gx < x; gx += 1) {
      const first = foldedIndex(gx, gy, x) * INSTANCE_CULL_WORKGROUP;
      if (first >= input.capacity || templates === 0) {
        continue;
      }
      group.fill(0);
      kept.length = 0;
      for (let local = 0; local < INSTANCE_CULL_WORKGROUP; local += 1) {
        const instance = first + local;
        const bounds = instance < live ? input.instances[instance] : undefined;
        if (bounds === undefined || !(bounds.radiusM >= 0)) {
          continue;
        }
        for (let p = 0; p < passCount; p += 1) {
          const pass = input.passes[p] ?? null;
          if (
            pass === null ||
            (input.passMask & (1 << p)) === 0 ||
            !passKeeps(
              pass,
              input.viewAxis,
              bounds.centreM,
              bounds.radiusM * input.boundsScale,
              slackScale,
              INSTANCE_ROUNDING_M,
            )
          ) {
            continue;
          }
          const template = instanceTemplateOf(
            pass,
            input.errorFractions,
            bounds.centreM,
            bounds.radiusM,
            input.boundsScale,
          );
          const counter = p * MAX_PARTICLE_TEMPLATES + template;
          kept.push({ instance, pass: p, template, slot: group[counter] ?? 0 });
          group[counter] = (group[counter] ?? 0) + 1;
        }
      }
      const bases = new Uint32Array(MAX_CULL_PASSES * MAX_PARTICLE_TEMPLATES);
      for (let counter = 0; counter < passCount * MAX_PARTICLE_TEMPLATES; counter += 1) {
        const n = group[counter] ?? 0;
        const p = Math.floor(counter / MAX_PARTICLE_TEMPLATES);
        const t = counter % MAX_PARTICLE_TEMPLATES;
        const bucket = (input.places[p]?.bucketBase ?? 0) + t;
        if (n > 0 && t < templates) {
          bases[counter] = counts[bucket] ?? 0;
          counts[bucket] = (counts[bucket] ?? 0) + n;
        }
      }
      for (const { instance, pass, template, slot } of kept) {
        const at = (bases[pass * MAX_PARTICLE_TEMPLATES + template] ?? 0) + slot;
        if (at < (input.templateCapacities[template] ?? 0)) {
          lists[(input.places[pass]?.listBase ?? 0) + (input.templateOffsets[template] ?? 0) + at] =
            instance;
        }
      }
    }
  }
  return { counts, lists };
}

/**
 * The entries of pass `pass`'s template `template`'s bucket of a cull's writes, in their order:
 * its count, by `counts`' bucket `place.bucketBase + template`, of its list from
 * `place.listBase + templateOffset`, at most `capacity`.
 */
export function instanceBucketEntries(
  writes: InstanceCullTwinWrites,
  place: CullPassPlace,
  template: number,
  templateOffset: number,
  capacity: number,
): Uint32Array {
  const n = Math.min(writes.counts[place.bucketBase + template] ?? 0, capacity);
  const start = place.listBase + templateOffset;
  return writes.lists.slice(start, start + n);
}

/**
 * The template of an instance's sphere at `centreM`, of radius `radiusM`, in `pass`, as
 * `instanceCull.wgsl`'s `instance_template` takes it, in `f64`: the coarsest whose error fraction
 * times the radius is within the pass's tolerance at its bounds, `boundsScale` times the radius,
 * the finest where none is.
 */
export function instanceTemplateOf(
  pass: CullPass,
  errorFractions: ReadonlyArray<number>,
  centreM: Vec3,
  radiusM: number,
  boundsScale = 1,
): number {
  const reach = instanceReachM(pass, centreM, radiusM * boundsScale);
  const last = Math.min(errorFractions.length, MAX_PARTICLE_TEMPLATES) - 1;
  for (let t = 0; t < last; t += 1) {
    if ((errorFractions[t] ?? Infinity) * radiusM <= reach) {
      return t;
    }
  }
  return last;
}

/** The pass's tolerance at bounds, m: a cascade's texel, a camera's τ times the bounds' nearness. */
function instanceReachM(pass: CullPass, centreM: Vec3, radiusM: number): number {
  let reach: number;
  switch (pass.kind) {
    case "cascade":
      reach = pass.toleranceM;
      break;
    case "camera":
      reach =
        pass.toleranceRad * Math.max(Math.hypot(centreM.x, centreM.y, centreM.z) - radiusM, 0);
      break;
  }
  return reach;
}

/**
 * Whether the GPU, in `f32`, may take a template other than the twin's for a sphere: some error
 * of a template the twin passes over or takes lies within a relative `1e-5` of the tolerance.
 */
export function instanceTemplateNear(
  pass: CullPass,
  errorFractions: ReadonlyArray<number>,
  centreM: Vec3,
  radiusM: number,
  boundsScale = 1,
): boolean {
  const reach = instanceReachM(pass, centreM, radiusM * boundsScale);
  return errorFractions.some((e) => Math.abs(e * radiusM - reach) <= 1e-5 * Math.max(reach, 1e-30));
}

/** What one dispatch of the mesh instances' cull culls, as its twin takes it. */
export interface InstanceCompactTwinInput {
  readonly instances: ReadonlyArray<InstanceBounds>;
  readonly passes: ReadonlyArray<CullPass | null>;
  readonly viewAxis: Vec3;
  readonly passMask: number;
}

/**
 * The survivors of the mesh instances' cull, in `f64` (see the module's remarks): each instance any
 * pass keeps, by its index, with the passes that keep it, a bit a pass.
 */
export function instanceCompactTwin(
  input: InstanceCompactTwinInput,
  slackScale = 1,
): ReadonlyMap<number, number> {
  const survivors = new Map<number, number>();
  input.instances.forEach((instance, index) => {
    if (!(instance.radiusM >= 0)) {
      return;
    }
    let bits = 0;
    input.passes.forEach((pass, p) => {
      if (
        pass !== null &&
        p < MAX_CULL_PASSES &&
        (input.passMask & (1 << p)) !== 0 &&
        passKeeps(
          pass,
          input.viewAxis,
          instance.centreM,
          instance.radiusM,
          slackScale,
          INSTANCE_ROUNDING_M,
        )
      ) {
        bits |= 1 << p;
      }
    });
    if (bits !== 0) {
      survivors.set(index, bits >>> 0);
    }
  });
  return survivors;
}
