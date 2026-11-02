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
  packCullParams,
  packCullPasses,
} from "./cullPass";
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
      [frame.count, 1, 1],
      label,
    );
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
  }
}
