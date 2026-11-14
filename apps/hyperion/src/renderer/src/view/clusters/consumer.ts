/**
 * The cluster path's consumers beside the terrain (plan R14, R14.T4): what a consumer registers
 * with the path, and one view's path of a consumer, which culls its instances' clusters into its
 * buckets by one dispatch of the shared cull (`clusterCull.wgsl`) and hands back their draws.
 *
 * @remarks
 * Terrain registers through `TerrainPass`, whose slots are its pages; every other consumer holds a
 * {@link ClusterConsumerPath} of its own: from R14.T4.a the mesh regime's figure, its strided
 * templates at stride 1 and its records written a frame (`bodies/smoothMesh.ts`), and from R14.T4.b
 * the hulls' occluder faces, the mesh template in the DAG-cut mode (`wireframe/hullClusters.ts`).
 *
 * - **Instances**: a consumer's instance record begins with its origin less the camera, three
 *   `f32` in the frame the passes are in, and its page, a `u32` (the cull's instance words), and
 *   is {@link ClusterConsumer.instanceBytes} long, never assumed; beside it each instance's cull
 *   input is two `u32`, the passes it may enter, a bit a pass, and its error unit's `f32` bits.
 * - **Records**: page by page, {@link ClusterConsumer.clustersPerPage} a page, each turned into the
 *   instances' frame by the dispatch's rotation.
 * - **Buckets**: one a (pass, template), each template's capacity a pass its clusters a page
 *   ({@link ClusterConsumer.templateClusters}) times the instances, so that no append overflows;
 *   made again, the instances and the passes rounded up to powers of two, when a frame's pass
 *   theirs, and the cull's inputs with them.
 * - **The fold** (R14.T5.d; `fold.ts`): the cull's workgroups, one an instance, fold over two
 *   dimensions past 65,535 instances.
 * - **Instances culled first** (R14.T5.d; {@link ClusterConsumerPath.cullInstances}): for a
 *   consumer of many instances, a producer's GPU-written ones among them, each instance's bounding
 *   sphere is tested against every pass by the instance cull before any cluster of it
 *   (`instanceCompact.wgsl`), the instances any pass keeps compacted with the passes that keep
 *   them, and the cluster cull dispatched indirectly over them alone, its workgroups folded on the
 *   GPU (`instanceFold.wgsl`). The compaction's order is not reproducible, so its instances'
 *   clusters suit an appended pass's buckets alone, until a stable compaction orders them.
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
import {
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  type CullPass,
  MAX_CULL_PASSES,
  NO_DISCARD_TEMPLATES,
  packCullParams,
  packCullPasses,
} from "./cullPass";
import { foldedWorkgroups } from "./fold";
import {
  foldTail,
  INSTANCE_COMPACT_PARAMS_BYTES,
  instanceWorkgroups,
  packInstanceCompactParams,
} from "./instanceCull";
import type { ClusterTemplate, Stride } from "./templates";

/** What a consumer registers with the cluster path (see the module's remarks). */
export interface ClusterConsumer {
  /** Its name, which its buffers' names begin with. */
  readonly name: string;
  /**
   * Its templates, in its cluster mesh's order: the cull's template is its record's shape × 4 +
   * log₂ of its stride (`clusterCull.wgsl`).
   */
  readonly templates: ReadonlyArray<ClusterTemplate>;
  readonly clustersPerPage: number;
  /**
   * The code of its first shape (`clusters/record.ts`' `CLUSTER_SHAPE_CODE`), from which its
   * records' shapes pick their templates (`CullParams.firstShape`): 0 for the figures' blocks, 2
   * for a mesh's.
   */
  readonly firstShape: number;
  /**
   * Each template's clusters a page that may enter its bucket: its shape's clusters, or 0 for a
   * template the consumer never draws (a stride it never takes).
   */
  readonly templateClusters: ReadonlyArray<number>;
  /** Bytes of its instance record (see the module's remarks). */
  readonly instanceBytes: number;
  /**
   * Its vertex stage over a bucket's list, which reads each entry, (instance << 8) | cluster, from
   * its `Draw`'s `clusterBase`, and places the template's vertex as the consumer's own draw does.
   */
  readonly vertexWgsl: string;
}

/** One frame of a consumer's cull, as {@link ClusterConsumerPath.cull} takes it. */
export interface ConsumerCull {
  /** Its records, page by page. */
  readonly records: BufferHandle;
  /**
   * Its DAG records' parent groups' spheres, one a record (`ClusterLod`, R14.T4.b), or none for a
   * consumer with no DAG record, whose records stand in for them, unread.
   */
  readonly lods?: BufferHandle;
  /** Its instance records, {@link ClusterConsumer.instanceBytes} each. */
  readonly instances: BufferHandle;
  readonly count: number;
  /** Each instance's cull input, two `u32` (see the module's remarks), at least `2 count`. */
  readonly inputs: Uint32Array;
  /** The passes, at most {@link MAX_CULL_PASSES}, `null` for one not culled this frame. */
  readonly passes: ReadonlyArray<CullPass | null>;
  /** The pages' axes into the instances' frame, by column. */
  readonly rotationColumns: readonly [Vec3, Vec3, Vec3];
  /** The view axis in the instances' frame, which a cascade's receivers' cull reads. */
  readonly viewAxis: Vec3;
}

/**
 * One frame of a consumer's cull with its instances culled first ({@link ClusterConsumerPath.cullInstances}):
 * its records and passes as {@link ConsumerCull}'s, and instances a producer may have placed on the
 * GPU, with their count beside them.
 */
export interface ConsumerInstanceCull {
  readonly records: BufferHandle;
  readonly lods?: BufferHandle;
  /**
   * Its instance records, {@link ClusterConsumer.instanceBytes} each, at least `capacity` of them:
   * the cluster cull's first four words, then at word `radiusWord` the radius about the origin, m,
   * of a sphere holding every cluster the instance may draw; negative for no instance.
   */
  readonly instances: BufferHandle;
  /** The live instances' count, the first `u32` of this buffer, at most `capacity`. */
  readonly count: BufferHandle;
  readonly capacity: number;
  /** The word of an instance record that holds its bounding radius, past its page's (word 3). */
  readonly radiusWord: number;
  /** The passes, at most {@link MAX_CULL_PASSES}, `null` for one not culled this frame. */
  readonly passes: ReadonlyArray<CullPass | null>;
  /** The passes every instance may enter, a bit a pass. */
  readonly passMask: number;
  readonly rotationColumns: readonly [Vec3, Vec3, Vec3];
  readonly viewAxis: Vec3;
}

/** The kernels {@link ClusterConsumerPath.cullInstances} dispatches, each made of its `KernelPair`. */
export interface ConsumerInstanceKernels {
  /** `instanceCull.ts`' `INSTANCE_COMPACT_KERNEL`. */
  readonly compact: ComputeHandle;
  /** `instanceCull.ts`' `INSTANCE_FOLD_KERNEL`. */
  readonly fold: ComputeHandle;
  /** `cull.ts`' `CLUSTER_CULL_KERNEL`. */
  readonly cull: ComputeHandle;
}

/** Bytes of the survivors' buffer: their count, then the cluster cull's indirect workgroups. */
const SURVIVORS_BYTES = 16;

/** The survivors' buffer's offset of the cluster cull's indirect workgroups, bytes. */
const SURVIVORS_ARGS_OFFSET_BYTES = 4;

/** The compacted instances' buffers of {@link ClusterConsumerPath.cullInstances}, and their room. */
interface CompactedBuffers {
  /** The survivors' room: the capacity it was made for and its fold's tail. */
  readonly capacity: number;
  readonly places: number;
  readonly instances: BufferHandle;
  readonly inputs: BufferHandle;
  readonly survivors: BufferHandle;
}

/** The part of the engine a consumer's path uses. */
export type ClusterConsumerEngine = Pick<
  RenderEngine,
  "createBuffer" | "writeBuffer" | "releaseBuffer" | "dispatch"
>;

/** The least power of two at least `n`, and at least 1. */
function roomFor(n: number): number {
  return 2 ** Math.ceil(Math.log2(Math.max(1, n)));
}

/** A view's path of one consumer (see the module's remarks). */
export class ClusterConsumerPath {
  readonly #engine: ClusterConsumerEngine;
  readonly #consumer: ClusterConsumer;
  #buckets: ClusterBuckets | null = null;
  #cullInputs: BufferHandle | null = null;
  /** The ordered passes' marks, which no consumer's pass takes yet: one word, never written. */
  #marks: BufferHandle | null = null;
  #compacted: CompactedBuffers | null = null;
  #room = { instances: 0, passes: 0 };
  readonly #passes = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
  readonly #params = new ArrayBuffer(CULL_PARAMS_BYTES);
  #disposed = false;

  constructor(engine: ClusterConsumerEngine, consumer: ClusterConsumer) {
    if (
      consumer.templateClusters.length !== consumer.templates.length ||
      !consumer.templateClusters.every((n) => n >= 0 && n <= consumer.clustersPerPage)
    ) {
      throw new RangeError(
        `${consumer.name}'s ${String(consumer.templateClusters.length)} template cluster counts are not one a template, each at most a page's ${String(consumer.clustersPerPage)}`,
      );
    }
    this.#engine = engine;
    this.#consumer = consumer;
  }

  /** Its buckets, made at its first cull; `null` before. */
  get buckets(): ClusterBuckets | null {
    return this.#buckets;
  }

  /**
   * Culls `frame`'s instances' clusters into the buckets: writes their inputs, the passes and the
   * dispatch's parameters, clears the buckets and dispatches the cull, a workgroup an instance,
   * labelled `label`; its submission precedes every draw encoded after the call.
   */
  cull(kernel: ComputeHandle, frame: ConsumerCull, label: string): void {
    if (this.#disposed || frame.count === 0) {
      return;
    }
    const buckets = this.#reserve(frame.count, frame.passes.length);
    const consumer = this.#consumer;
    packCullPasses(
      frame.passes,
      frame.passes.map((_, index) => buckets.place(index)),
      this.#passes,
    );
    packCullParams(
      {
        rotationColumns: frame.rotationColumns,
        instances: frame.count,
        instanceWords: consumer.instanceBytes / 4,
        clustersPerPage: consumer.clustersPerPage,
        passes: frame.passes.length,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis: frame.viewAxis,
        firstShape: consumer.firstShape,
        discardTemplates: NO_DISCARD_TEMPLATES,
      },
      this.#params,
    );
    const cullInputs = this.#cullInputs;
    const marks = this.#marks;
    if (cullInputs === null || marks === null) {
      throw new Error(`${consumer.name}'s cull has no inputs' buffer`);
    }
    this.#engine.writeBuffer(cullInputs, 0, frame.inputs.subarray(0, 2 * frame.count));
    buckets.clear();
    this.#engine.dispatch(
      kernel,
      {
        uniforms: {
          params: new Uint32Array(this.#params),
          passes: new Uint32Array(this.#passes),
        },
        buffers: {
          instance_words: frame.instances,
          cull_inputs: cullInputs,
          records: frame.records,
          lists: buckets.lists,
          args: buckets.args,
          marks,
          lods: frame.lods ?? frame.records,
        },
        sampled: {},
        storage: {},
      },
      foldedWorkgroups(frame.count),
      label,
    );
  }

  /**
   * Culls `frame`'s instances' bounds first, then their clusters into the buckets (see the
   * module's remarks): three dispatches, the instances' compaction over the capacity (`label`
   * followed by ` instances`), the survivors' fold (the same label) and the cluster cull over the
   * survivors (`label`), each submitted before every draw encoded after the call.
   *
   * @throws RangeError for a radius word not past the page's and within the record, or a capacity
   *   that is not a whole number from 1.
   */
  cullInstances(
    kernels: ConsumerInstanceKernels,
    frame: ConsumerInstanceCull,
    label: string,
  ): void {
    const consumer = this.#consumer;
    const words = consumer.instanceBytes / 4;
    if (
      !Number.isInteger(frame.radiusWord) ||
      frame.radiusWord < 4 ||
      frame.radiusWord >= words ||
      !Number.isInteger(frame.capacity) ||
      frame.capacity < 1
    ) {
      throw new RangeError(
        `${consumer.name}'s instances of ${String(words)} words, their radius at word ${String(frame.radiusWord)}, ${String(frame.capacity)} of them: the radius past the page and within the record, and some instance`,
      );
    }
    if (this.#disposed) {
      return;
    }
    const tail = foldTail(frame.capacity);
    const places = frame.capacity + tail;
    const buckets = this.#reserve(frame.capacity, frame.passes.length);
    const compacted = this.#compactedFor(frame.capacity, places, words);
    const engine = this.#engine;
    packCullPasses(
      frame.passes,
      frame.passes.map((_, index) => buckets.place(index)),
      this.#passes,
    );
    const compact = new ArrayBuffer(INSTANCE_COMPACT_PARAMS_BYTES);
    packInstanceCompactParams(
      {
        capacity: frame.capacity,
        words,
        radiusWord: frame.radiusWord,
        passes: frame.passes.length,
        viewAxis: frame.viewAxis,
        passMask: frame.passMask,
      },
      compact,
    );
    engine.writeBuffer(compacted.survivors, 0, new Uint32Array(SURVIVORS_BYTES / 4));
    const passes = new Uint32Array(this.#passes);
    engine.dispatch(
      kernels.compact,
      {
        uniforms: { params: new Uint32Array(compact), passes },
        buffers: {
          instance_words: frame.instances,
          instance_count: frame.count,
          compacted: compacted.instances,
          compacted_inputs: compacted.inputs,
          survivors: compacted.survivors,
        },
        sampled: {},
        storage: {},
      },
      instanceWorkgroups(frame.capacity),
      `${label} instances`,
    );
    engine.dispatch(
      kernels.fold,
      {
        uniforms: { params: Uint32Array.of(frame.capacity, places, 0, 0) },
        buffers: { survivors: compacted.survivors, compacted_inputs: compacted.inputs },
        sampled: {},
        storage: {},
      },
      [1, 1, 1],
      `${label} instances`,
    );
    // The cluster cull's guard takes every place of the fold: a place past the survivors enters no
    // pass, its input zeroed by the fold.
    packCullParams(
      {
        rotationColumns: frame.rotationColumns,
        instances: places,
        instanceWords: words,
        clustersPerPage: consumer.clustersPerPage,
        passes: frame.passes.length,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis: frame.viewAxis,
        firstShape: consumer.firstShape,
        discardTemplates: NO_DISCARD_TEMPLATES,
      },
      this.#params,
    );
    const marks = this.#marks;
    if (marks === null) {
      throw new Error(`${consumer.name}'s cull has no marks' buffer`);
    }
    buckets.clear();
    engine.dispatch(
      kernels.cull,
      {
        uniforms: { params: new Uint32Array(this.#params), passes },
        buffers: {
          instance_words: compacted.instances,
          cull_inputs: compacted.inputs,
          records: frame.records,
          lists: buckets.lists,
          args: buckets.args,
          marks,
          lods: frame.lods ?? frame.records,
        },
        sampled: {},
        storage: {},
      },
      { buffer: compacted.survivors, offsetBytes: SURVIVORS_ARGS_OFFSET_BYTES },
      label,
    );
  }

  /**
   * The compacted instances of {@link ClusterConsumerPath.cullInstances} as the bucket draws read
   * them, their records in the survivors' places, or `null` before its first cull: a draw of its
   * buckets binds these, not the frame's instances.
   */
  get compactedInstances(): BufferHandle | null {
    return this.#compacted?.instances ?? null;
  }

  /** The compacted buffers with room for `capacity` survivors, made again where short. */
  #compactedFor(capacity: number, places: number, words: number): CompactedBuffers {
    const held = this.#compacted;
    if (held !== null && capacity <= held.capacity) {
      return held;
    }
    this.#releaseCompacted();
    const engine = this.#engine;
    const name = this.#consumer.name;
    const made: CompactedBuffers = {
      capacity,
      places,
      instances: engine.createBuffer({
        name: `${name} compacted instances`,
        bytes: places * words * 4,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "geometry-clusters",
      }),
      inputs: engine.createBuffer({
        name: `${name} compacted inputs`,
        bytes: places * 8,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "geometry-clusters",
      }),
      survivors: engine.createBuffer({
        name: `${name} survivors`,
        bytes: SURVIVORS_BYTES,
        usage:
          BUFFER_USAGE.STORAGE |
          BUFFER_USAGE.INDIRECT |
          BUFFER_USAGE.COPY_DST |
          BUFFER_USAGE.COPY_SRC,
        category: "geometry-clusters",
      }),
    };
    this.#compacted = made;
    return made;
  }

  #releaseCompacted(): void {
    const held = this.#compacted;
    if (held !== null) {
      for (const buffer of [held.instances, held.inputs, held.survivors]) {
        this.#engine.releaseBuffer(buffer);
      }
    }
    this.#compacted = null;
  }

  /**
   * The compacted cull inputs of {@link ClusterConsumerPath.cullInstances}, two `u32` a survivor,
   * for a check's read-back.
   */
  get compactedInputs(): BufferHandle | null {
    return this.#compacted?.inputs ?? null;
  }

  /** The survivors' buffer of {@link ClusterConsumerPath.cullInstances}, for a check's read-back. */
  get survivors(): BufferHandle | null {
    return this.#compacted?.survivors ?? null;
  }

  /**
   * Pass `pass`'s bucket draws of the templates at strides up to `cap`, `base` with `mesh`
   * (`ClusterBuckets.drawItems`); none before the first cull.
   */
  drawItems(
    pass: number,
    mesh: MeshHandle,
    base: BucketDrawBase,
    cap: Stride,
  ): ReadonlyArray<DrawItem> {
    return this.#buckets?.drawItems(pass, mesh, base, cap) ?? [];
  }

  /** Its buckets, with room for `instances` instances and `passes` passes, made again where short. */
  #reserve(instances: number, passes: number): ClusterBuckets {
    const held = this.#buckets;
    if (held !== null && instances <= this.#room.instances && passes <= this.#room.passes) {
      return held;
    }
    const room = {
      instances: Math.max(roomFor(instances), this.#room.instances),
      passes: Math.min(Math.max(roomFor(passes), this.#room.passes), MAX_CULL_PASSES),
    };
    this.#releaseBuffers();
    const consumer = this.#consumer;
    const engine = this.#engine;
    const buckets = new ClusterBuckets(
      engine,
      consumer.name,
      room.passes,
      consumer.templates,
      consumer.templateClusters.map((n) => n * room.instances),
    );
    this.#cullInputs = engine.createBuffer({
      name: `${consumer.name} cull inputs`,
      bytes: 8 * room.instances,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "geometry-clusters",
    });
    this.#marks = engine.createBuffer({
      name: `${consumer.name} marks`,
      bytes: 4,
      usage: BUFFER_USAGE.STORAGE,
      category: "geometry-clusters",
    });
    this.#buckets = buckets;
    this.#room = room;
    return buckets;
  }

  #releaseBuffers(): void {
    this.#buckets?.dispose();
    for (const buffer of [this.#cullInputs, this.#marks]) {
      if (buffer !== null) {
        this.#engine.releaseBuffer(buffer);
      }
    }
    this.#buckets = null;
    this.#cullInputs = null;
    this.#marks = null;
  }

  /** Releases its buckets and buffers; nothing more is culled. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#releaseBuffers();
    this.#releaseCompacted();
  }
}
