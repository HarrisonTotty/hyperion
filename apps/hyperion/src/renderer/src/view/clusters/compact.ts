/**
 * The cluster path's stable compaction (plan R14, R14.T2.a; Design note 5): the kernel that writes
 * an ordered pass's buckets from the cull's marks in the draw set's order, and its TypeScript twin
 * (`compact.wgsl`).
 *
 * @remarks
 * A colour pass needs its draws in the draw set's order, since where two surfaces meet at equal
 * depth the later draw wins (`greater-equal`). The cull's appends are `atomicAdd`s, whose order
 * varies from run to run, so an ordered pass (`CullPassOrder` `ordered`, the lit pass) is marked
 * instead, one word a cluster, and compacted here: a count, an exclusive scan and a scatter, three
 * dispatches of one kernel, integer throughout (WebGPU gives no forward progress between
 * workgroups, so no single-pass look-back). Each template's bucket then holds its kept clusters
 * instance by instance and, within an instance, cluster by cluster. The subgroup variant counts
 * each template's clusters by `subgroupAdd` in place of a workgroup atomic a cluster; the scan is
 * the reference's on both paths, since WGSL does not order a subgroup's invocations by their
 * workgroup index. Both give the same bytes (`bit-exact`).
 */

import type { KernelPair } from "../engine/kernels";
import compactWgsl from "./compact.wgsl?raw";
import { NO_MARK } from "./cull";

/** The most templates a compacted pass has, as `compact.wgsl`'s TEMPLATES. */
export const COMPACT_TEMPLATES = 8;

/** The threads of the kernel's workgroup: one a cluster of a page, at most this many clusters. */
export const COMPACT_WORKGROUP = 128;

/** Bytes of the dispatch's `CompactParams`. */
export const COMPACT_PARAMS_BYTES = 64;

/** The kernel's three dispatches, by `params.counts.w`. */
export const COMPACT_PHASES = ["count", "scan", "scatter"] as const;

/** One of the kernel's dispatches. */
export type CompactPhase = (typeof COMPACT_PHASES)[number];

/** The reference's count: a workgroup atomic a kept cluster. */
const REFERENCE_COUNT = `
fn count(instance : u32, local : u32) {
  if (local < TEMPLATES) {
    atomicStore(&tally[local], 0u);
  }
  workgroupBarrier();
  let mark = mark_of(instance, local);
  if (mark < TEMPLATES) {
    atomicAdd(&tally[mark], 1u);
  }
  workgroupBarrier();
  if (local < TEMPLATES) {
    tallies[instance * TEMPLATES + local] = atomicLoad(&tally[local]);
  }
}
`;

/** The subgroup variant's count: each template's subgroup sum, then one atomic a subgroup. */
const SUBGROUP_COUNT = `
fn count(instance : u32, local : u32, lane : u32) {
  if (local < TEMPLATES) {
    atomicStore(&tally[local], 0u);
  }
  workgroupBarrier();
  let mark = mark_of(instance, local);
  for (var t = 0u; t < TEMPLATES; t = t + 1u) {
    let kept = subgroupAdd(select(0u, 1u, mark == t));
    if (lane == 0u && kept > 0u) {
      atomicAdd(&tally[t], kept);
    }
  }
  workgroupBarrier();
  if (local < TEMPLATES) {
    tallies[instance * TEMPLATES + local] = atomicLoad(&tally[local]);
  }
}
`;

/** `compact.wgsl` with its count, and the lane where the subgroup variant reads it. */
function compactSource(subgroup: boolean): string {
  const source = compactWgsl
    .replace("//COUNT//", subgroup ? SUBGROUP_COUNT : REFERENCE_COUNT)
    .replace("//LANE//", subgroup ? ", @builtin(subgroup_invocation_id) lane : u32" : "")
    .replace("//LANEARG//", subgroup ? ", lane" : "");
  return subgroup ? `enable subgroups;\n${source}` : source;
}

/** The stable compaction's kernel (see the module's remarks): `bit-exact`, with a subgroup variant. */
export const CLUSTER_COMPACT: KernelPair = {
  name: "cluster compaction",
  reference: compactSource(false),
  subgroup: compactSource(true),
  readback: "bit-exact",
};

/** One dispatch's parameters (see `compact.wgsl`'s head). */
export interface CompactParams {
  readonly instances: number;
  readonly clustersPerPage: number;
  readonly templates: number;
  readonly phase: CompactPhase;
  /** The pass's first bucket and its first list entry (`ClusterBuckets.place`). */
  readonly bucketBase: number;
  readonly listBase: number;
  /** Each template's first list entry within the pass. */
  readonly templateOffsets: ReadonlyArray<number>;
}

/**
 * Packs `params` as `compact.wgsl`'s `CompactParams`, {@link COMPACT_PARAMS_BYTES} bytes.
 *
 * @throws RangeError for more than {@link COMPACT_TEMPLATES} templates, offsets not one a template,
 *   or more clusters a page than {@link COMPACT_WORKGROUP}.
 */
export function packCompactParams(params: CompactParams): Uint32Array {
  if (
    params.templates > COMPACT_TEMPLATES ||
    params.templateOffsets.length !== params.templates ||
    params.clustersPerPage > COMPACT_WORKGROUP
  ) {
    throw new RangeError(
      `a compaction of ${String(params.templates)} templates, ${String(params.templateOffsets.length)} offsets and ${String(params.clustersPerPage)} clusters a page: at most ${String(COMPACT_TEMPLATES)}, one a template and ${String(COMPACT_WORKGROUP)}`,
    );
  }
  const words = new Uint32Array(COMPACT_PARAMS_BYTES / 4);
  words.set([
    params.instances,
    params.clustersPerPage,
    params.templates,
    COMPACT_PHASES.indexOf(params.phase),
  ]);
  words.set([params.bucketBase, params.listBase], 4);
  words.set(params.templateOffsets, 8);
  return words;
}

/** The workgroups of `phase`'s dispatch over `instances`: one an instance, and one for the scan. */
export function compactWorkgroups(
  phase: CompactPhase,
  instances: number,
): readonly [number, number, number] {
  return phase === "scan" ? [1, 1, 1] : [Math.max(1, instances), 1, 1];
}

/**
 * The twin of the three dispatches over `marks` (`instances` × `clustersPerPage`, a template or
 * {@link NO_MARK} each): each template's entries in its bucket's order, (instance << 8) | cluster,
 * instance by instance and cluster by cluster; and each template's count, its bucket's instance
 * count.
 */
export function compactTwin(
  marks: Uint32Array,
  instances: number,
  clustersPerPage: number,
  templates: number,
): {
  readonly lists: ReadonlyArray<ReadonlyArray<number>>;
  readonly counts: ReadonlyArray<number>;
} {
  const lists: number[][] = Array.from({ length: templates }, () => []);
  for (let n = 0; n < instances; n += 1) {
    for (let cluster = 0; cluster < clustersPerPage; cluster += 1) {
      const mark = marks[n * clustersPerPage + cluster] ?? NO_MARK;
      if (mark < templates) {
        lists[mark]?.push(n * 256 + cluster);
      }
    }
  }
  return { lists, counts: lists.map((list) => list.length) };
}

/** Each template's exclusive prefix over `tallies` (TEMPLATES words an instance), as the scan's. */
export function scanTwin(tallies: Uint32Array, instances: number): Uint32Array {
  const out = new Uint32Array(tallies.length);
  for (let t = 0; t < COMPACT_TEMPLATES; t += 1) {
    let running = 0;
    for (let n = 0; n < instances; n += 1) {
      out[n * COMPACT_TEMPLATES + t] = running;
      running += tallies[n * COMPACT_TEMPLATES + t] ?? 0;
    }
  }
  return out;
}
