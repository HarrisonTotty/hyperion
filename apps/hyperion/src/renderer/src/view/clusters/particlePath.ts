/**
 * A view's path of one particle consumer (plan R14, R14.T5.d): its instances, which its producer
 * places, culled each frame by the instance cull into its (pass, template) buckets, and the bucket
 * draws of a pass. R11.T6.a's pebbles and R11.T9.d's ring particles each hold one.
 *
 * @remarks
 * - **Its inputs**, which a producer writes before {@link ParticlePath.cull}: {@link ParticlePath.instances},
 *   {@link ParticlePath.capacity} records of `particles.ts`' layout, and {@link ParticlePath.count},
 *   whose first `u32` is the number placed (a GPU placement's appends' count, or the CPU's through
 *   {@link ParticlePath.writeInstances}), at most the capacity; both `STORAGE | COPY_DST`.
 * - **Its buckets** (`clusters/buckets.ts`): one a (pass, template), each template's list holding
 *   every instance once a pass, so that no append overflows (plan R14, Design note 16). Each entry is
 *   an instance's index. A bucket draw's material composes `particleInstance.wgsl` and binds the
 *   instances and the lists ({@link PARTICLE_STORAGE_BUFFERS}), which {@link ParticlePath.drawItems}
 *   gives each draw.
 * - **Its cull**, one dispatch a frame (`instanceCull.wgsl`), folded over the capacity: the passes,
 *   at most the path's, camera passes and cascades as the cluster cull takes them (`cullPass.ts`),
 *   each instance entering those its pass mask sets.
 * - **Memory**: the instances, 32 B each, the count, the lists, 4 B an entry, `passes × templates ×
 *   capacity` of them, and the arguments, all `geometry-clusters` (R11's `scatter` category is its
 *   producer's own buffers'). The lists are within `buckets.ts`' 2²⁴ entries: at four templates and
 *   one pass, 4,194,304 instances.
 */

import type { Vec3 } from "../../geometry/vec3";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  ComputeHandle,
  DrawItem,
  MeshHandle,
  RenderEngine,
} from "../engine/types";
import { type BucketDrawBase, ClusterBuckets } from "./buckets";
import { CULL_PASS_BYTES, type CullPass, MAX_CULL_PASSES, packCullPasses } from "./cullPass";
import {
  INSTANCE_CULL_LABEL,
  INSTANCE_CULL_PARAMS_BYTES,
  instanceWorkgroups,
  packInstanceCullParams,
} from "./instanceCull";
import {
  PARTICLE_INSTANCE_BYTES,
  PARTICLE_INSTANCE_WORDS,
  PARTICLE_RADIUS_WORD,
  type ParticleTemplates,
} from "./particles";

/** The part of the engine a particle path uses. */
export type ParticlePathEngine = Pick<
  RenderEngine,
  "createBuffer" | "writeBuffer" | "releaseBuffer" | "dispatch"
>;

/** One frame of a particle path's cull. */
export interface ParticleCull {
  /** The passes, at most the path's, `null` for one not culled this frame. */
  readonly passes: ReadonlyArray<CullPass | null>;
  /** The view axis in the passes' frame, which a cascade's receivers' cull reads. */
  readonly viewAxis: Vec3;
  /** The passes every instance may enter, a bit a pass: all of them where absent. */
  readonly passMask?: number;
}

/** Bytes of the count buffer: its first `u32` the instances placed, padded to a binding's least. */
const COUNT_BYTES = 16;

/** A view's path of one particle consumer (see the module's remarks). */
export class ParticlePath {
  /** The most instances it culls. */
  readonly capacity: number;
  /** The most passes a frame culls for. */
  readonly passes: number;
  readonly templates: ParticleTemplates;
  readonly instances: BufferHandle;
  readonly count: BufferHandle;
  readonly buckets: ClusterBuckets;
  readonly #engine: ParticlePathEngine;
  readonly #passes = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
  readonly #params = new ArrayBuffer(INSTANCE_CULL_PARAMS_BYTES);
  /** Each draw base given, with the path's buffers bound beside its own. */
  readonly #bases = new WeakMap<BucketDrawBase, BucketDrawBase>();
  #disposed = false;

  /**
   * @throws RangeError for a capacity that is not a whole number from 1, passes not 1 to
   *   {@link MAX_CULL_PASSES}, or lists past `buckets.ts`' most entries.
   */
  constructor(
    engine: ParticlePathEngine,
    name: string,
    templates: ParticleTemplates,
    capacity: number,
    passes: number,
  ) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(
        `a particle path holds a whole number of instances from 1, not ${String(capacity)}`,
      );
    }
    if (!Number.isInteger(passes) || passes < 1 || passes > MAX_CULL_PASSES) {
      throw new RangeError(
        `a particle path culls for 1 to ${String(MAX_CULL_PASSES)} passes, not ${String(passes)}`,
      );
    }
    this.#engine = engine;
    this.capacity = capacity;
    this.passes = passes;
    this.templates = templates;
    this.buckets = new ClusterBuckets(
      engine,
      name,
      passes,
      templates.templates,
      templates.templates.map(() => capacity),
    );
    this.instances = engine.createBuffer({
      name: `${name} instances`,
      bytes: capacity * PARTICLE_INSTANCE_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
      category: "geometry-clusters",
    });
    this.count = engine.createBuffer({
      name: `${name} count`,
      bytes: COUNT_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
      category: "geometry-clusters",
    });
  }

  /**
   * Writes `count` instances packed in `data` (`particles.ts`' layout) and their count, through the
   * queue: the CPU's placement, for tests and fixed scenes.
   *
   * @throws RangeError for more instances than the capacity, or `data` too short for them.
   */
  writeInstances(data: ArrayBuffer, count: number): void {
    if (!Number.isInteger(count) || count < 0 || count > this.capacity) {
      throw new RangeError(
        `${String(count)} instances do not fit a path of ${String(this.capacity)}`,
      );
    }
    if (data.byteLength < count * PARTICLE_INSTANCE_BYTES) {
      throw new RangeError(`${String(data.byteLength)} B do not hold ${String(count)} instances`);
    }
    if (this.#disposed) {
      return;
    }
    if (count > 0) {
      this.#engine.writeBuffer(
        this.instances,
        0,
        new Uint8Array(data, 0, count * PARTICLE_INSTANCE_BYTES),
      );
    }
    this.writeCount(count);
  }

  /** Writes the instances' count alone, through the queue, for instances already in place. */
  writeCount(count: number): void {
    if (!this.#disposed) {
      this.#engine.writeBuffer(
        this.count,
        0,
        Uint32Array.of(Math.min(Math.max(count, 0), this.capacity), 0, 0, 0),
      );
    }
  }

  /**
   * Culls the instances into the buckets: writes the passes and the dispatch's parameters, clears
   * the buckets and dispatches the instance cull over the capacity, folded, labelled `label`; its
   * submission precedes every draw encoded after the call.
   *
   * @throws RangeError for more passes than the path's.
   */
  cull(kernel: ComputeHandle, frame: ParticleCull, label: string = INSTANCE_CULL_LABEL): void {
    if (this.#disposed) {
      return;
    }
    if (frame.passes.length > this.passes) {
      throw new RangeError(
        `${String(frame.passes.length)} passes, past the path's ${String(this.passes)}`,
      );
    }
    const buckets = this.buckets;
    packCullPasses(
      frame.passes,
      frame.passes.map((_, index) => buckets.place(index)),
      this.#passes,
    );
    packInstanceCullParams(
      {
        capacity: this.capacity,
        words: PARTICLE_INSTANCE_WORDS,
        radiusWord: PARTICLE_RADIUS_WORD,
        passes: frame.passes.length,
        viewAxis: frame.viewAxis,
        errorFractions: this.templates.errorFractions,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        passMask: frame.passMask ?? (1 << frame.passes.length) - 1,
        boundsScale: this.templates.boundsScale,
      },
      this.#params,
    );
    buckets.clear();
    this.#engine.dispatch(
      kernel,
      {
        uniforms: {
          params: new Uint32Array(this.#params),
          passes: new Uint32Array(this.#passes),
        },
        buffers: {
          instance_words: this.instances,
          instance_count: this.count,
          lists: buckets.lists,
          args: buckets.args,
        },
        sampled: {},
        storage: {},
      },
      instanceWorkgroups(this.capacity),
      label,
    );
  }

  /**
   * Pass `pass`'s bucket draws, one a template, coarsest first: `base` with `mesh` (the templates'
   * mesh), the bucket's arguments and its list's base, and the path's instances and lists bound as
   * {@link PARTICLE_STORAGE_BUFFERS} name them beside `base`'s own.
   */
  drawItems(pass: number, mesh: MeshHandle, base: BucketDrawBase): ReadonlyArray<DrawItem> {
    let bound = this.#bases.get(base);
    if (bound === undefined) {
      bound = {
        ...base,
        storageBuffers: {
          ...base.storageBuffers,
          particleInstances: this.instances,
          particleLists: this.buckets.lists,
        },
      };
      this.#bases.set(base, bound);
    }
    return this.buckets.drawItems(pass, mesh, bound, 1);
  }

  /** Releases its buffers and buckets; nothing more is culled or written. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.buckets.dispose();
    this.#engine.releaseBuffer(this.instances);
    this.#engine.releaseBuffer(this.count);
  }
}
