/**
 * The cluster path's counters (plan R14, R14.T1.b; Design note 5): each pass's clusters, triangles
 * and clusters by stride, reduced on the GPU from its buckets' indirect arguments behind the
 * measurement switch alone, never in an ordinary frame.
 *
 * @remarks
 * The cull's appends make its lists `presentation-only` (`clusters/cull.ts`), but each bucket's
 * instance count is an integer sum of `atomicAdd`s, the same whatever their order. So the counts
 * reach the CPU through a `bit-exact` reduction ({@link CLUSTER_STATS_KERNEL}), never a copy of the
 * arguments: a thread a pass sums its buckets in template order, integers throughout, into
 * {@link PASS_STATS_WORDS} words, which go to a ring of read-back buffers. A frame samples into a
 * slot no read holds, and the read's answer, some frames later, becomes
 * {@link ClusterStats.latest}. A pass's GPU time is the engine's `PassTimes` of its label (`terrain
 * shadows` for the cascades, `terrain cluster cull` for the cull), where the adapter offers
 * `timestamp-query`.
 */

import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../engine/types";
import type { ClusterBuckets } from "./buckets";
import { STRIDES, type ClusterTemplate } from "./templates";

/** The words each pass's reduction writes: its clusters, its triangles and its clusters by stride. */
export const PASS_STATS_WORDS = 2 + STRIDES.length;

/** The most templates a pass's reduction reads: two `vec4u` of triangles and of strides' levels. */
const MAX_STATS_TEMPLATES = 8;

/**
 * The `bit-exact` reduction of each pass's buckets: a thread a pass, its buckets' instance counts
 * (the second word of each's arguments) summed in template order, times each template's triangles,
 * and by each template's stride.
 */
export const CLUSTER_STATS_KERNEL: KernelPair = {
  name: "cluster stats reduction",
  reference: `
struct StatsParams {
  // x: the passes; y: the templates a pass has.
  counts : vec4u,
  // Each template's triangles, four to a vec4u.
  triangles : array<vec4u, 2>,
  // Each template's stride's level, log2 of its stride.
  levels : array<vec4u, 2>,
}
@group(0) @binding(0) var<storage, read> args : array<u32>;
@group(0) @binding(1) var<storage, read_write> counts : array<u32>;
@group(0) @binding(2) var<uniform> params : StatsParams;
const PASS_WORDS : u32 = ${String(PASS_STATS_WORDS)}u;
@compute @workgroup_size(8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let pass_index = id.x;
  if (pass_index >= params.counts.x) {
    return;
  }
  let templates = min(params.counts.y, ${String(MAX_STATS_TEMPLATES)}u);
  var clusters = 0u;
  var triangles = 0u;
  var by_stride = array<u32, ${String(STRIDES.length)}>();
  for (var t = 0u; t < templates; t = t + 1u) {
    let n = args[(pass_index * params.counts.y + t) * 5u + 1u];
    clusters = clusters + n;
    triangles = triangles + n * params.triangles[t / 4u][t % 4u];
    let level = min(params.levels[t / 4u][t % 4u], ${String(STRIDES.length - 1)}u);
    by_stride[level] = by_stride[level] + n;
  }
  let at = pass_index * PASS_WORDS;
  counts[at] = clusters;
  counts[at + 1u] = triangles;
  for (var l = 0u; l < ${String(STRIDES.length)}u; l = l + 1u) {
    counts[at + 2u + l] = by_stride[l];
  }
}
`,
  subgroup: null,
  readback: "bit-exact",
};

/** One pass's counts. */
export interface ClusterPassStats {
  /** The pass's name, as its consumer gives it. */
  readonly label: string;
  /** Its clusters drawn, every template's. */
  readonly clusters: number;
  /** Its triangles drawn: each template's clusters times its triangles. */
  readonly triangles: number;
  /** Its clusters by stride, 1, 2, 4 and 8, every shape's. */
  readonly byStride: ReadonlyArray<number>;
}

/** The read-back ring's slots. */
const RING = 3;

/**
 * Each pass's counts from the last of its samples to have come back (see the module's remarks).
 */
export class ClusterStats {
  readonly #engine: RenderEngine;
  readonly #name: string;
  #kernel: ComputeHandle | null = null;
  readonly #slots: Array<{ buffer: BufferHandle; busy: boolean; buckets: number }> = [];
  #latest: ReadonlyArray<ClusterPassStats> | null = null;
  #disposed = false;

  constructor(engine: RenderEngine, name: string) {
    this.#engine = engine;
    this.#name = name;
  }

  /** The counts of the last sample to have come back, `null` before. */
  latest(): ReadonlyArray<ClusterPassStats> | null {
    return this.#latest;
  }

  /**
   * Copies `buckets`' counts into a free slot of the ring and reads them back, after this frame's
   * cull and before the next; nothing where every slot is being read.
   *
   * @param labels - Each pass's name, as many as the passes sampled.
   */
  sample(
    buckets: ClusterBuckets,
    templates: ReadonlyArray<ClusterTemplate>,
    labels: ReadonlyArray<string>,
  ): void {
    if (this.#disposed) {
      return;
    }
    const count = labels.length * PASS_STATS_WORDS;
    const slot = this.#slot(count);
    if (slot === null) {
      return;
    }
    this.#kernel ??= this.#engine.createCompute(CLUSTER_STATS_KERNEL);
    // `StatsParams`: its counts, then each template's triangles and each template's stride's level.
    const params = new Uint32Array(4 + 2 * MAX_STATS_TEMPLATES);
    params[0] = labels.length;
    params[1] = templates.length;
    templates.slice(0, MAX_STATS_TEMPLATES).forEach((template, t) => {
      params[4 + t] = template.triangles;
      params[4 + MAX_STATS_TEMPLATES + t] = Math.log2(template.stride);
    });
    this.#engine.dispatch(
      this.#kernel,
      {
        uniforms: { params },
        buffers: { args: buckets.args, counts: slot.buffer },
        sampled: {},
        storage: {},
      },
      [Math.max(1, Math.ceil(labels.length / 8)), 1, 1],
      `${this.#name} stats`,
    );
    slot.busy = true;
    void this.#read(slot, count, labels).catch((error: unknown) => {
      console.warn(`${this.#name}: a read of the cluster counts failed`, error);
    });
  }

  /** Reads `slot`'s `count` counts back into {@link ClusterStats.latest}, freeing the slot after. */
  async #read(
    slot: { buffer: BufferHandle; busy: boolean },
    count: number,
    labels: ReadonlyArray<string>,
  ): Promise<void> {
    try {
      const bytes = await this.#engine.readBuffer(slot.buffer);
      if (!this.#disposed) {
        const words = Math.min(count, Math.floor(bytes.byteLength / 4));
        this.#latest = passStats(new Uint32Array(bytes, 0, words), labels);
      }
    } finally {
      slot.busy = false;
    }
  }

  /** A slot no read holds, of room for `count` counts, made at its first need; `null` for none. */
  #slot(count: number): { buffer: BufferHandle; busy: boolean; buckets: number } | null {
    const free = this.#slots.find((each) => !each.busy && each.buckets >= count);
    if (free !== undefined) {
      return free;
    }
    if (this.#slots.length >= RING) {
      const stale = this.#slots.findIndex((each) => !each.busy);
      if (stale < 0) {
        return null;
      }
      const [old] = this.#slots.splice(stale, 1);
      if (old !== undefined) {
        this.#engine.releaseBuffer(old.buffer);
      }
    }
    const made = {
      buffer: this.#engine.createBuffer({
        name: `${this.#name} counts ${String(this.#slots.length + 1)}`,
        bytes: 4 * Math.max(1, count),
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "geometry-clusters",
      }),
      busy: false,
      buckets: count,
    };
    this.#slots.push(made);
    return made;
  }

  /** Releases the ring and the kernel; a read still running is answered into nothing. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    for (const slot of this.#slots) {
      this.#engine.releaseBuffer(slot.buffer);
    }
    this.#slots.length = 0;
    if (this.#kernel !== null) {
      this.#engine.releaseCompute(this.#kernel);
      this.#kernel = null;
    }
  }
}

/** Each pass's counts from its reduction's words ({@link PASS_STATS_WORDS} a pass, in order). */
export function passStats(
  words: Uint32Array,
  labels: ReadonlyArray<string>,
): ReadonlyArray<ClusterPassStats> {
  return labels.map((label, pass) => {
    const at = pass * PASS_STATS_WORDS;
    return {
      label,
      clusters: words[at] ?? 0,
      triangles: words[at + 1] ?? 0,
      byStride: STRIDES.map((_, l) => words[at + 2 + l] ?? 0),
    };
  });
}
