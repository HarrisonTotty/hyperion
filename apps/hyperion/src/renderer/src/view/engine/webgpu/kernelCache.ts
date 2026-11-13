/**
 * The engine's kernels, one compiled record a device for each distinct kernel, shared by every
 * handle made for it, and released with the last of them (plan R01, R01.T15).
 *
 * @remarks
 * - **One record a key.** A kernel's record (its pipeline and its bindings) is made once for its
 *   {@link kernelKey}: its name, readback class, sources, entry point, constants and layout. Every
 *   `createCompute` or `createComputeAsync` of the key returns a new handle over it, and concurrent
 *   asynchronous creates of one key wait on one compile. A pipeline is immutable, so sharing it
 *   changes nothing a dispatch does.
 * - **Uniforms.** The record's uniform buffers are shared with it. That is safe because each
 *   dispatch writes its uniforms through `GPUQueue.writeBuffer` and then submits its own command
 *   buffer at once (`WebGpuRenderEngine.dispatch`): the queue orders each write before that
 *   submission and after every earlier one, so each dispatch reads its own values. The first
 *   dispatch through any of the record's handles sizes each buffer (`resolveKernelResources`), and
 *   a larger value for it is refused, as it was for one handle before; a kernel's uniforms are its
 *   WGSL's fixed structs, so its every caller passes the same sizes.
 * - **Release.** Each handle counts against its record until `release` lets it go. The last
 *   release destroys the record's uniform buffers, each raising its `destroyed` event, and the
 *   cache forgets the record, which goes with its last reference (WebGPU cannot destroy a
 *   pipeline). A handle that is dropped unreleased keeps its record until the cache is cleared:
 *   no finalizer releases it, so that no `destroyed` event lands at a time a garbage collection
 *   chooses. The records held are so bounded by the distinct keys made.
 * - **Clearing.** On a device loss and at disposal the cache forgets every record and every
 *   compile in flight; a compile that ends afterwards gives its handle a record the cache does not
 *   keep. A handle made before then still dispatches over its record and releases it as before.
 * - **A broken kernel.** A record made by `createCompute` serves a later `createComputeAsync` of
 *   the same key, so that call does not reject where the WGSL is broken; the synchronous creation
 *   has already raised its uncaptured validation error, which fails a smoke run
 *   (`src/smoke/result.ts`). A compile that rejects is not kept.
 */

import type { KernelPair } from "../kernels";
import type { BufferHandle, ComputeHandle } from "../types";
import { kernelKey, type KernelPipelineSpec, type KernelRecord } from "./compute";

/** What the cache compiles records with, and how it destroys their uniform buffers. */
export interface KernelCacheHost {
  /** Compiles a record, blocking the queue's timeline on the compile. */
  compile(pair: KernelPair, spec: KernelPipelineSpec): KernelRecord;
  /** Compiles a record off the queue's timeline. */
  compileAsync(pair: KernelPair, spec: KernelPipelineSpec): Promise<KernelRecord>;
  /** Destroys one of a record's uniform buffers, raising its `destroyed` event. */
  destroyBuffer(buffer: BufferHandle): void;
}

/** A kernel's record as a dispatch takes it: the record and its uniform buffers by name. */
export interface SharedKernel {
  readonly record: KernelRecord;
  /**
   * The record's uniform buffers by uniform name, made at the first dispatch that sets each
   * (`resolveKernelResources`) and destroyed at the record's last release.
   */
  readonly uniforms: Map<string, BufferHandle>;
}

/** A record, with its key and the handles over it not yet released. */
interface Entry extends SharedKernel {
  readonly key: string;
  handles: number;
}

/** The kernels of one device (see the module's remarks). */
export class KernelCache {
  readonly #host: KernelCacheHost;
  /** Each record held, by its key. */
  readonly #entries = new Map<string, Entry>();
  /** Each compile in flight, by its key, which every asynchronous create of the key waits on. */
  readonly #compiling = new Map<string, Promise<KernelRecord>>();
  readonly #handles = new WeakMap<ComputeHandle, Entry>();
  /** The handles released, so that a later use names the release. */
  readonly #released = new WeakSet<ComputeHandle>();
  /** Counts the clears, so that a compile that ends after one is not kept. */
  #generation = 0;

  constructor(host: KernelCacheHost) {
    this.#host = host;
  }

  /** The records the cache holds: one for each distinct key with a handle not yet released. */
  get size(): number {
    return this.#entries.size;
  }

  /** A handle over the record of `pair`'s `spec`, compiled now where the cache holds none. */
  create(pair: KernelPair, spec: KernelPipelineSpec): ComputeHandle {
    const key = kernelKey(pair, spec);
    return this.#handleOver(
      this.#entries.get(key) ?? this.#keep(key, this.#host.compile(pair, spec)),
    );
  }

  /**
   * A handle over the record of `pair`'s `spec`, compiled off the queue's timeline where the cache
   * holds none and none is being compiled.
   *
   * @throws Error, as a rejection, as the compile rejects, for every create waiting on it.
   */
  async createAsync(pair: KernelPair, spec: KernelPipelineSpec): Promise<ComputeHandle> {
    const key = kernelKey(pair, spec);
    const held = this.#entries.get(key);
    if (held !== undefined) {
      return this.#handleOver(held);
    }
    const generation = this.#generation;
    let compiling = this.#compiling.get(key);
    if (compiling === undefined) {
      compiling = this.#host.compileAsync(pair, spec);
      this.#compiling.set(key, compiling);
    }
    let record: KernelRecord;
    try {
      record = await compiling;
    } finally {
      if (this.#compiling.get(key) === compiling) {
        this.#compiling.delete(key);
      }
    }
    if (generation !== this.#generation) {
      // Cleared while it compiled: the record is the handle's alone.
      return this.#handleOver({ key, record, uniforms: new Map(), handles: 0 });
    }
    // A create of the key may have kept a record meanwhile; this one then goes unused.
    return this.#handleOver(this.#entries.get(key) ?? this.#keep(key, record));
  }

  /**
   * The record behind `handle` with its uniform buffers.
   *
   * @throws Error naming the kernel for a handle this cache did not make or has released.
   */
  kernelOf(handle: ComputeHandle): SharedKernel {
    return this.#entryOf(handle);
  }

  /**
   * Lets `handle` go; the last handle over a record destroys its uniform buffers, and the cache
   * forgets the record.
   *
   * @throws Error naming the kernel for a handle this cache did not make or has released.
   */
  release(handle: ComputeHandle): void {
    const entry = this.#entryOf(handle);
    this.#handles.delete(handle);
    this.#released.add(handle);
    entry.handles -= 1;
    if (entry.handles > 0) {
      return;
    }
    if (this.#entries.get(entry.key) === entry) {
      this.#entries.delete(entry.key);
    }
    for (const buffer of entry.uniforms.values()) {
      this.#host.destroyBuffer(buffer);
    }
    entry.uniforms.clear();
  }

  /**
   * Forgets every record and every compile in flight, at a device loss or the engine's disposal;
   * their uniform buffers are the device's, which the loss or the disposal ends.
   */
  clear(): void {
    this.#entries.clear();
    this.#compiling.clear();
    this.#generation += 1;
  }

  #keep(key: string, record: KernelRecord): Entry {
    const entry: Entry = { key, record, uniforms: new Map(), handles: 0 };
    this.#entries.set(key, entry);
    return entry;
  }

  #handleOver(entry: Entry): ComputeHandle {
    const handle: ComputeHandle = Object.freeze({
      kind: "compute",
      name: entry.record.pair.name,
      path: entry.record.path,
    });
    entry.handles += 1;
    this.#handles.set(handle, entry);
    return handle;
  }

  #entryOf(handle: ComputeHandle): Entry {
    const entry = this.#handles.get(handle);
    if (entry === undefined) {
      const refusal = this.#released.has(handle) ? "was released" : "was not made by this engine";
      throw new Error(`kernel ${handle.name} ${refusal}`);
    }
    return entry;
  }
}
