/**
 * The cull's count of the clusters its camera passes draw at a stride past their τ′ (plan R14,
 * R14.T3.e), read back every frame for `TERRAIN: DETAIL LIMITED`.
 *
 * @remarks
 * With strides chosen on the GPU the CPU cannot know where the drawn error passes τ′, so the cull
 * counts it (`clusterCull.wgsl`'s `drawn_over`): each cluster a camera pass draws at a stride above
 * 1 whose own need there, (E_L + e_s) ÷ τ_rad at the slewed tolerance τ its stride was chosen at,
 * lies beyond its sphere's nearest point d × τ′ ÷ τ, τ′ the pass's tolerance, is an `atomicAdd` into
 * one word of the pass's past its buckets' arguments (`ClusterBuckets.overWord`), cleared with them.
 * An integer sum is the same whatever the order of its adds, so the words reach the CPU through a
 * `bit-exact` copy ({@link CLUSTER_OVER_KERNEL}) into a ring of read-back buffers, as
 * `clusters/stats.ts`' counts do but every frame, and the answer, some frames later, becomes
 * {@link ClusterOverCounts.latest}. Stride 1 is the selection's own, whose budget the CPU knows
 * (`Selection.limited`), and is not counted.
 */

import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../engine/types";
import type { ClusterBuckets } from "./buckets";

/**
 * The `bit-exact` copy of a dispatch's counts of its strides drawn past τ′: a thread a pass, its word
 * of the buckets' arguments from `params.counts.x` on.
 */
export const CLUSTER_OVER_KERNEL: KernelPair = {
  name: "cluster over counts",
  reference: `
struct OverParams {
  // x: the first word of the counts in args; y: the passes.
  counts : vec4u,
}
@group(0) @binding(0) var<storage, read> args : array<u32>;
@group(0) @binding(1) var<storage, read_write> counts : array<u32>;
@group(0) @binding(2) var<uniform> params : OverParams;
@compute @workgroup_size(8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= params.counts.y) {
    return;
  }
  counts[id.x] = args[params.counts.x + id.x];
}
`,
  subgroup: null,
  readback: "bit-exact",
};

/** The read-back ring's slots. */
const RING = 3;

/** One slot of the read-back ring: its buffer, whether a read holds it, and its words. */
interface OverSlot {
  readonly buffer: BufferHandle;
  busy: boolean;
  readonly words: number;
}

/**
 * Each pass's count of its strides drawn past τ′, from the last of its samples to have come back
 * (see the module's remarks).
 */
export class ClusterOverCounts {
  readonly #engine: RenderEngine;
  readonly #name: string;
  #kernel: ComputeHandle | null = null;
  readonly #slots: OverSlot[] = [];
  #latest: ReadonlyArray<number> | null = null;
  #disposed = false;

  constructor(engine: RenderEngine, name: string) {
    this.#engine = engine;
    this.#name = name;
  }

  /** Each pass's count of the last sample to have come back, in the buckets' pass order; `null` before. */
  latest(): ReadonlyArray<number> | null {
    return this.#latest;
  }

  /**
   * Copies `buckets`' counts into a free slot of the ring and reads them back, after this frame's
   * cull and before the next; nothing where every slot is being read.
   */
  sample(buckets: ClusterBuckets): void {
    if (this.#disposed) {
      return;
    }
    const words = buckets.passes;
    const slot = this.#slot(words);
    if (slot === null) {
      return;
    }
    this.#kernel ??= this.#engine.createCompute(CLUSTER_OVER_KERNEL);
    this.#engine.dispatch(
      this.#kernel,
      {
        uniforms: { params: Uint32Array.of(buckets.overWord, words, 0, 0) },
        buffers: { args: buckets.args, counts: slot.buffer },
        sampled: {},
        storage: {},
      },
      [Math.max(1, Math.ceil(words / 8)), 1, 1],
      `${this.#name} over counts`,
    );
    slot.busy = true;
    void this.#read(slot, words).catch((error: unknown) => {
      console.warn(`${this.#name}: a read of the strides past τ′ failed`, error);
    });
  }

  /** Reads `slot`'s `words` counts back into {@link ClusterOverCounts.latest}, freeing the slot after. */
  async #read(slot: OverSlot, words: number): Promise<void> {
    try {
      const bytes = await this.#engine.readBuffer(slot.buffer);
      if (!this.#disposed) {
        this.#latest = Array.from(
          new Uint32Array(bytes, 0, Math.min(words, Math.floor(bytes.byteLength / 4))),
        );
      }
    } finally {
      slot.busy = false;
    }
  }

  /** A slot no read holds, of room for `words` counts, made at its first need; `null` for none. */
  #slot(words: number): OverSlot | null {
    const free = this.#slots.find((each) => !each.busy && each.words >= words);
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
    const made: OverSlot = {
      buffer: this.#engine.createBuffer({
        name: `${this.#name} over counts ${String(this.#slots.length + 1)}`,
        bytes: 4 * Math.max(1, words),
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "geometry-clusters",
      }),
      busy: false,
      words,
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
