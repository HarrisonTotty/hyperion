/**
 * The cluster path's indirect buckets (plan R14, R14.T1.b): one a (pass, template), each a list of
 * (instance, cluster) entries the cull fills and the `drawIndexedIndirect` arguments it counts
 * them into, drawn with its template's indices.
 *
 * @remarks
 * - **The lists**: one `u32` an entry, (instance << 8) | cluster, so a consumer has at most 256
 *   clusters a page and 2²⁴ instances. A pass's buckets lie one after another, its templates in
 *   order, each with room for every cluster of its shape of every instance the consumer may draw
 *   ({@link ClusterBuckets}' `capacities`), so that no append can overflow.
 * - **The arguments**: five `u32` a bucket, `drawIndexedIndirect`'s (index count, instance count,
 *   first index, base vertex, first instance): its template's count and first index, its instance
 *   count the cull's appends, base vertex and first instance 0, since a draw with a non-zero first
 *   instance needs `indirect-first-instance`, which the engine does not ask for. A bucket draw so
 *   reads its entries from its list's base, which it carries in its `Draw` as `clusterBase`.
 * - **Each frame** {@link ClusterBuckets.clear} writes every argument afresh, the counts zero,
 *   through the queue, so that they precede the cull's dispatch and follow the last frame's draws.
 */

import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { BufferHandle, DrawItem, MeshHandle, RenderEngine } from "../engine/types";
import type { CullPassPlace } from "./cullPass";
import type { ClusterTemplate, Stride } from "./templates";

/** Bytes of a bucket's `drawIndexedIndirect` arguments: five `u32`. */
export const BUCKET_ARGS_BYTES = 20;

/** Bytes of a list entry. */
export const LIST_ENTRY_BYTES = 4;

/**
 * The most entries a view's lists may hold: a bucket's base reaches its `Draw` as an `f32` it is
 * read back from as a `u32`, exact below 2²⁴.
 */
export const MAX_LIST_ENTRIES = 2 ** 24;

/** The `Draw` uniform a bucket draw reads its list's base from. */
export const CLUSTER_BASE_UNIFORM = "clusterBase";

/** The parts of a bucket draw its consumer gives: all but its mesh, indirect arguments and base. */
export type BucketDrawBase = Omit<DrawItem, "mesh" | "indirect" | "instanceCount">;

/** A view's buckets for `passes` passes of a consumer's templates (see the module's remarks). */
export class ClusterBuckets {
  readonly #engine: RenderEngine;
  readonly #templates: ReadonlyArray<ClusterTemplate>;
  /** Each template's first list entry within a pass. */
  readonly templateOffsets: ReadonlyArray<number>;
  /** Each template's list capacity, entries. */
  readonly capacities: ReadonlyArray<number>;
  /** The list entries a pass spans. */
  readonly passEntries: number;
  readonly passes: number;
  readonly lists: BufferHandle;
  readonly args: BufferHandle;
  readonly #cleared: Uint32Array;
  /** Each pass's draws, and the base and cap they were made with. */
  readonly #items = new Map<
    number,
    { readonly base: BucketDrawBase; readonly cap: Stride; readonly items: ReadonlyArray<DrawItem> }
  >();
  #disposed = false;

  /**
   * @param capacities - Each template's list capacity a pass, entries: every cluster of its shape of
   *   every instance the consumer may draw.
   * @throws RangeError for capacities not one a template, or lists past {@link MAX_LIST_ENTRIES}.
   */
  constructor(
    engine: RenderEngine,
    name: string,
    passes: number,
    templates: ReadonlyArray<ClusterTemplate>,
    capacities: ReadonlyArray<number>,
  ) {
    if (
      capacities.length !== templates.length ||
      !capacities.every((c) => Number.isInteger(c) && c >= 0)
    ) {
      throw new RangeError(
        `${String(capacities.length)} capacities for ${String(templates.length)} templates`,
      );
    }
    this.#engine = engine;
    this.#templates = templates;
    this.capacities = capacities;
    const offsets: number[] = [];
    let entries = 0;
    for (const capacity of capacities) {
      offsets.push(entries);
      entries += capacity;
    }
    this.templateOffsets = offsets;
    this.passEntries = entries;
    this.passes = passes;
    if (passes * entries > MAX_LIST_ENTRIES) {
      throw new RangeError(
        `${name}'s lists would hold ${String(passes * entries)} entries, past ${String(MAX_LIST_ENTRIES)}`,
      );
    }
    const listBytes = Math.max(LIST_ENTRY_BYTES, passes * entries * LIST_ENTRY_BYTES);
    this.lists = engine.createBuffer({
      name: `${name} lists`,
      bytes: listBytes,
      // COPY_SRC for the smoke harness's twin check, which reads them back (`smoke/clusterCull.ts`).
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
      category: "geometry-clusters",
    });
    const buckets = passes * templates.length;
    this.args = engine.createBuffer({
      name: `${name} arguments`,
      bytes: Math.max(BUCKET_ARGS_BYTES, buckets * BUCKET_ARGS_BYTES),
      usage:
        BUFFER_USAGE.STORAGE |
        BUFFER_USAGE.INDIRECT |
        BUFFER_USAGE.COPY_DST |
        BUFFER_USAGE.COPY_SRC,
      category: "geometry-clusters",
    });
    this.#cleared = new Uint32Array(buckets * (BUCKET_ARGS_BYTES / 4));
    for (let pass = 0; pass < passes; pass += 1) {
      templates.forEach((template, t) => {
        const at = (pass * templates.length + t) * (BUCKET_ARGS_BYTES / 4);
        this.#cleared[at] = template.indexCount;
        this.#cleared[at + 2] = template.firstIndex;
      });
    }
  }

  /** Where pass `pass`'s buckets begin: its first bucket and its first list entry. */
  place(pass: number): CullPassPlace {
    return { bucketBase: pass * this.#templates.length, listBase: pass * this.passEntries };
  }

  /** The initial arguments of every bucket, the counts zero, as {@link ClusterBuckets.clear} writes them. */
  get clearedArgs(): Uint32Array {
    return this.#cleared;
  }

  /** Writes every bucket's arguments afresh, its count zero, before the frame's cull. */
  clear(): void {
    if (this.#disposed) {
      return;
    }
    this.#engine.writeBuffer(this.args, 0, this.#cleared);
  }

  /**
   * Pass `pass`'s bucket draws of the templates at strides up to `cap`, in the templates' order:
   * `base` with `mesh`, the bucket's arguments and its list's base as {@link CLUSTER_BASE_UNIFORM};
   * made again where `base` or `cap` changed.
   */
  drawItems(
    pass: number,
    mesh: MeshHandle,
    base: BucketDrawBase,
    cap: Stride,
  ): ReadonlyArray<DrawItem> {
    const held = this.#items.get(pass);
    if (held?.base === base && held.cap === cap && held.items[0]?.mesh === mesh) {
      return held.items;
    }
    const place = this.place(pass);
    const items: DrawItem[] = [];
    this.#templates.forEach((template, t) => {
      if (template.stride > cap) {
        return;
      }
      items.push({
        ...base,
        mesh,
        uniforms: {
          ...base.uniforms,
          [CLUSTER_BASE_UNIFORM]: Float32Array.of(place.listBase + (this.templateOffsets[t] ?? 0)),
        },
        indirect: { buffer: this.args, offsetBytes: (place.bucketBase + t) * BUCKET_ARGS_BYTES },
      });
    });
    this.#items.set(pass, { base, cap, items });
    return items;
  }

  /** Releases the lists and the arguments; nothing more is written. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#items.clear();
    this.#engine.releaseBuffer(this.lists);
    this.#engine.releaseBuffer(this.args);
  }
}
