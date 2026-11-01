/**
 * The cluster path's counters (plan R14, R14.T1.b): each pass's clusters and triangles, read from
 * its buckets' indirect arguments behind the measurement switch alone, never in an ordinary frame.
 *
 * @remarks
 * The cull's appends make its buckets `presentation-only` (`clusters/cull.ts`), so the counts, which
 * are integer sums and the same whatever the appends' order, reach the CPU through a `bit-exact`
 * copy of each bucket's instance count into a ring of read-back buffers
 * ({@link CLUSTER_STATS_KERNEL}): a frame samples into a slot no read holds, and the read's answer,
 * some frames later, becomes {@link ClusterStats.latest}. A pass's GPU time is the engine's
 * `PassTimes` of its label (`terrain shadows` for the cascades, `terrain cluster cull` for the
 * cull), where the adapter offers `timestamp-query`.
 */

import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../engine/types";
import type { ClusterBuckets } from "./buckets";
import type { ClusterTemplate } from "./templates";

/** The `bit-exact` copy of each bucket's instance count, the second word of its arguments. */
export const CLUSTER_STATS_KERNEL: KernelPair = {
  name: "cluster stats copy",
  reference: `
@group(0) @binding(0) var<storage, read> args : array<u32>;
@group(0) @binding(1) var<storage, read_write> counts : array<u32>;
@group(0) @binding(2) var<uniform> params : vec4u;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x < params.x) {
    counts[id.x] = args[id.x * 5u + 1u];
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
  /** Its clusters by template, in the consumer's templates' order. */
  readonly byTemplate: ReadonlyArray<number>;
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
    const count = labels.length * templates.length;
    const slot = this.#slot(count);
    if (slot === null) {
      return;
    }
    this.#kernel ??= this.#engine.createCompute(CLUSTER_STATS_KERNEL);
    this.#engine.dispatch(
      this.#kernel,
      {
        uniforms: { params: Uint32Array.of(count, 0, 0, 0) },
        buffers: { args: buckets.args, counts: slot.buffer },
        sampled: {},
        storage: {},
      },
      [Math.max(1, Math.ceil(count / 64)), 1, 1],
      `${this.#name} stats`,
    );
    slot.busy = true;
    void this.#read(slot, count, templates, labels).catch((error: unknown) => {
      console.warn(`${this.#name}: a read of the cluster counts failed`, error);
    });
  }

  /** Reads `slot`'s `count` counts back into {@link ClusterStats.latest}, freeing the slot after. */
  async #read(
    slot: { buffer: BufferHandle; busy: boolean },
    count: number,
    templates: ReadonlyArray<ClusterTemplate>,
    labels: ReadonlyArray<string>,
  ): Promise<void> {
    try {
      const bytes = await this.#engine.readBuffer(slot.buffer);
      if (!this.#disposed) {
        const words = Math.min(count, Math.floor(bytes.byteLength / 4));
        this.#latest = passStats(new Uint32Array(bytes, 0, words), templates, labels);
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

/** Each pass's counts from its buckets' instance counts, pass by pass, template by template. */
export function passStats(
  counts: Uint32Array,
  templates: ReadonlyArray<ClusterTemplate>,
  labels: ReadonlyArray<string>,
): ReadonlyArray<ClusterPassStats> {
  return labels.map((label, pass) => {
    const byTemplate = templates.map((_, t) => counts[pass * templates.length + t] ?? 0);
    return {
      label,
      clusters: byTemplate.reduce((sum, n) => sum + n, 0),
      triangles: byTemplate.reduce((sum, n, t) => sum + n * (templates[t]?.triangles ?? 0), 0),
      byTemplate,
    };
  });
}
