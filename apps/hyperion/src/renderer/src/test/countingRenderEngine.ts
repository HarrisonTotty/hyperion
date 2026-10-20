/**
 * A counting fake of R01's `RenderEngine` (plan R05, R05.T11.a): every creation answers with a
 * handle and raises the allocation event the WebGPU engine would, and every creation, write,
 * dispatch and frame is counted, so that the terrain's and the atmosphere's tests can hold their
 * per-frame work to "nothing new after warm-up".
 *
 * @remarks
 * Views, faults and disposal are delegated to R01's {@link FakeRenderEngine} over `FakeGpu`'s
 * device, whose capabilities it reports; R01's fake itself, the `ResilientEngine`'s inner engine,
 * is left alone. Bytes follow the engine's own rules: a buffer its `bytes`, a texture
 * {@link textureBytes}. Readbacks resolve to zeros of no length; packed cubes and splats are not
 * faked.
 *
 * Creations are held to the device's limits as WebGPU's validation would hold them: a buffer above
 * `maxBufferSize`, a storage buffer above `maxStorageBufferBindingSize` (every binding here spans
 * its whole buffer), or a 2D texture above `maxTextureDimension2D` or 256 layers throws
 * {@link LimitExceeded}, where the device would raise a validation error and draw nothing.
 *
 * A mesh raises a `created` event for each of its buffers, its attributes' then its indices', at
 * the bytes R01's `MeshRecord` makes them with (category `other`), and `releaseMesh` their
 * `destroyed` events (plan R10, R10.T16; the batch review's S11). A device loss can be modelled
 * as a window ({@link CountingRenderEngine.loseDevice} to {@link CountingRenderEngine.restore}) in
 * which every creation throws `EngineUnavailable` and every write is dropped, as `ResilientEngine`
 * does while it has no device, and a material's compilation can be made to fail
 * ({@link CountingRenderEngine.failMaterials}).
 */

import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import { EngineUnavailable } from "../view/engine/resilientEngine";
import type { KernelPair } from "../view/engine/kernels";
import {
  type AllocationEvent,
  type BufferSpec,
  extentOf,
  type MemoryCategory,
  textureBytes,
  type TextureSpec,
} from "../view/engine/memory";
import { type GpuCapabilities, MAX_TEXTURE_ARRAY_LAYERS } from "../view/engine/platform";
import type { GraphicsFault } from "../view/engine/status";
import type {
  BufferHandle,
  ComputeBindings,
  ComputeHandle,
  FrameSubmission,
  IndirectArgs,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  PassTimes,
  PointSplatHandle,
  PostProcessHandle,
  RenderEngine,
  RenderTarget,
  RenderTargetSpec,
  RenderView,
  TextureHandle,
  WgslMaterialSpec,
  WgslPostProcessSpec,
} from "../view/engine/types";
import { FakeAdapter, SWIFTSHADER_INFO } from "./fakeGpu";
import { FakeRenderEngine } from "./fakeRenderEngine";

/** What the engine was asked to do, by kind. */
export interface EngineCounts {
  buffers: number;
  textures: number;
  meshes: number;
  materials: number;
  postProcesses: number;
  kernels: number;
  renderTargets: number;
  bufferWrites: number;
  textureWrites: number;
  dispatches: number;
  frames: number;
  draws: number;
}

/** One recorded dispatch. */
export interface RecordedDispatch {
  readonly kernel: string;
  readonly bindings: ComputeBindings;
  readonly workgroups: readonly [number, number, number] | IndirectArgs;
  readonly pass: string | undefined;
  /**
   * The frames submitted to render targets before it ({@link CountingRenderEngine.targetFrames}'
   * length then), which orders it among them as the queue does (R07.T8.d).
   */
  readonly framesBefore: number;
}

/** One recorded buffer write. */
export interface RecordedWrite {
  readonly buffer: string;
  readonly offsetBytes: number;
  readonly bytes: number;
  /** A copy of the data written, as bytes. */
  readonly data: Uint8Array;
}

/** One recorded texture write. */
export interface RecordedTextureWrite {
  readonly texture: string;
  readonly origin: GPUOrigin3D;
  readonly size: GPUExtent3D;
  /** A copy of the data written, as bytes. */
  readonly data: Uint8Array;
}

/** How many of each kind of handle are live: made and neither released nor lost (R10.T16.b). */
export interface LiveHandles {
  readonly buffers: number;
  readonly textures: number;
  readonly meshes: number;
  readonly materials: number;
}

/** A creation the device's limits would refuse with a validation error. */
export class LimitExceeded extends Error {
  constructor(message: string) {
    super(message);
    this.name = "LimitExceeded";
  }
}

function zeroCounts(): EngineCounts {
  return {
    buffers: 0,
    textures: 0,
    meshes: 0,
    materials: 0,
    postProcesses: 0,
    kernels: 0,
    renderTargets: 0,
    bufferWrites: 0,
    textureWrites: 0,
    dispatches: 0,
    frames: 0,
    draws: 0,
  };
}

function copyOf(data: ArrayBufferView): Uint8Array {
  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice();
}

function notFaked(member: string): Error {
  return new Error(`CountingRenderEngine does not fake ${member}`);
}

/** A fake engine that answers creations with handles and counts everything. */
export class CountingRenderEngine implements RenderEngine {
  readonly depthPolicy = "reversed-z-float" as const;
  /** R01's fake, which makes the views and raises the faults. */
  readonly inner: FakeRenderEngine;
  /** Totals since creation or {@link CountingRenderEngine.resetCounts}. */
  counts: EngineCounts = zeroCounts();
  /** Every dispatch, first first. */
  readonly dispatched: RecordedDispatch[] = [];
  /** Every buffer write, first first. */
  readonly writes: RecordedWrite[] = [];
  /** Every texture write, first first. */
  readonly textureWritten: RecordedTextureWrite[] = [];
  /** Every texture made, render targets' included, first first. */
  readonly textureSpecs: TextureSpec[] = [];
  /** Every frame submitted to a render target, first first (views record their own). */
  readonly targetFrames: FrameSubmission[] = [];
  readonly #allocationListeners = new Set<(event: AllocationEvent) => void>();
  readonly #restoredListeners = new Set<() => void>();
  /** What each buffer and texture was made with, so that a release raises its bytes (R06.T13.h). */
  readonly #made = new Map<
    BufferHandle | TextureHandle,
    { bytes: number; category: MemoryCategory }
  >();
  /** The meshes made and not released or lost (R10.T16.b), each with its buffers' names and bytes. */
  readonly #meshes = new Map<MeshHandle, ReadonlyArray<{ name: string; bytes: number }>>();
  /** The materials made and not released or lost (R10.T16.b). */
  readonly #materials = new Set<MaterialHandle>();
  /** The handles a modelled loss took ({@link CountingRenderEngine.loseHandles}), whose release is dropped. */
  readonly #lost = new WeakSet<BufferHandle | TextureHandle | MeshHandle | MaterialHandle>();
  #capabilities: GpuCapabilities;
  /** Inside a modelled loss's window ({@link CountingRenderEngine.loseDevice}). */
  #unavailable = false;
  /** Which materials' asynchronous compilation rejects, by name; none by default. */
  #failsMaterial: (name: string) => boolean = () => false;

  constructor(device: GPUDevice) {
    this.inner = new FakeRenderEngine(device);
    this.#capabilities = this.inner.capabilities;
  }

  /** The device's capabilities: the first device's, or the one the last restore rebuilt onto. */
  get capabilities(): GpuCapabilities {
    return this.#capabilities;
  }

  /**
   * The handles live now, by kind: made, and neither released nor taken by a modelled loss, so that
   * a test holds an owner's teardown to "nothing left" (R10.T16.b; render targets' textures, which
   * their targets own, count while the engine lives).
   */
  get live(): LiveHandles {
    let buffers = 0;
    let textures = 0;
    for (const handle of this.#made.keys()) {
      if (handle.kind === "buffer") {
        buffers += 1;
      } else {
        textures += 1;
      }
    }
    return { buffers, textures, meshes: this.#meshes.size, materials: this.#materials.size };
  }

  /**
   * Models a device loss's end of every handle made so far, as `ResilientEngine`'s lost engine
   * disposes of them (R10.T16.b): each buffer, texture and mesh buffer raises its `destroyed`
   * event, every handle leaves the live tallies, and a later release of one is dropped, as the
   * resilient engine drops it, once: a second release of it throws, as the resilient engine's does,
   * having forgotten its maker at the first. A test follows it with
   * {@link CountingRenderEngine.restore}.
   */
  loseHandles(): void {
    for (const [handle, made] of this.#made) {
      this.#lost.add(handle);
      this.#emit({ kind: "destroyed", name: handle.name, ...made });
    }
    this.#made.clear();
    for (const [mesh, buffers] of this.#meshes) {
      this.#lost.add(mesh);
      for (const buffer of buffers) {
        this.#emit({ kind: "destroyed", ...buffer, category: "other" });
      }
    }
    this.#meshes.clear();
    for (const material of this.#materials) {
      this.#lost.add(material);
    }
    this.#materials.clear();
  }

  /**
   * Models a device loss's window, as `ResilientEngine` behaves with no device: every handle ends
   * ({@link CountingRenderEngine.loseHandles}), and until {@link CountingRenderEngine.restore}
   * every creation throws `EngineUnavailable`, an asynchronous one rejects with it, and every write
   * is dropped.
   */
  loseDevice(): void {
    this.loseHandles();
    this.#unavailable = true;
  }

  /** Whether the engine is inside a modelled loss's window. */
  get unavailable(): boolean {
    return this.#unavailable;
  }

  /**
   * Makes `createMaterialAsync` reject, with an `Error` naming the material, for each material whose
   * name `fails` holds, as a compilation the device refuses; `() => false` restores every one.
   */
  failMaterials(fails: (name: string) => boolean): void {
    this.#failsMaterial = fails;
  }

  /** Throws `EngineUnavailable` for `member` inside a modelled loss's window. */
  #available(member: string): void {
    if (this.#unavailable) {
      throw new EngineUnavailable(member);
    }
  }

  /** Zeroes {@link CountingRenderEngine.counts}; the recorded lists are kept. */
  resetCounts(): void {
    this.counts = zeroCounts();
  }

  /**
   * Raises a restore after a device loss, as the resilient engine does once it has rebuilt, onto
   * `device` when given (a lesser adapter's, say), whose capabilities hold from then on.
   */
  restore(device?: GPUDevice): void {
    this.#unavailable = false;
    if (device !== undefined) {
      this.#capabilities = new FakeRenderEngine(device).capabilities;
    }
    for (const listener of this.#restoredListeners) {
      listener();
    }
  }

  /** Destroys a buffer or texture the test made, raising its `destroyed` event. */
  destroy(name: string, bytes: number, category: MemoryCategory): void {
    this.#emit({ kind: "destroyed", name, bytes, category });
  }

  #emit(event: AllocationEvent): void {
    for (const listener of this.#allocationListeners) {
      listener(event);
    }
  }

  createView(canvas: HTMLCanvasElement, name: string): RenderView {
    return this.inner.createView(canvas, name);
  }

  createRenderTarget(spec: RenderTargetSpec): RenderTarget {
    this.#available("createRenderTarget");
    this.counts.renderTargets += 1;
    const colour = this.createTexture({
      name: `${spec.name} colour`,
      size: [spec.size.widthPx, spec.size.heightPx],
      dimension: "2d",
      format: spec.format,
      mips: spec.mips,
      usage: 0,
      category: spec.category,
    });
    const depth = spec.depth
      ? this.createTexture({
          name: `${spec.name} depth`,
          size: [spec.size.widthPx, spec.size.heightPx],
          dimension: "2d",
          format: "depth32float",
          mips: 1,
          usage: 0,
          category: spec.category,
        })
      : null;
    // A resize remakes the target's textures, as the adapter's does: new handles, same names.
    const handles = { colour, depth };
    return {
      name: spec.name,
      get colour(): TextureHandle {
        return handles.colour;
      },
      get depth(): TextureHandle | null {
        return handles.depth;
      },
      resize: (): void => {
        handles.colour = { ...handles.colour };
        handles.depth = handles.depth === null ? null : { ...handles.depth };
      },
      render: (frame: FrameSubmission): void => {
        this.counts.frames += 1;
        this.counts.draws += frame.draws.length;
        this.targetFrames.push(frame);
      },
      dispose: (): void => undefined,
    };
  }

  createMesh(spec: MeshSpec): MeshHandle {
    this.#available("createMesh");
    this.counts.meshes += 1;
    const mesh: MeshHandle = { kind: "mesh", name: spec.name };
    // `MeshRecord`'s buffers: one an attribute, the positions first, then the indices', each a
    // whole number of words, at least one.
    const arrays: Array<readonly [string, ArrayBufferView]> = [
      ["position", spec.positions],
      ...Object.entries(spec.attributes).map(([name, { data }]) => [name, data] as const),
      ...Object.entries(spec.instanceAttributes ?? {}).map(
        ([name, { data }]) => [name, data] as const,
      ),
      ...(spec.indices === null ? [] : [["indices", spec.indices] as const]),
    ];
    const buffers = arrays.map(([name, data]) => ({
      name: `${spec.name}:${name}`,
      bytes: Math.max(4, Math.ceil(data.byteLength / 4) * 4),
    }));
    for (const buffer of buffers) {
      this.#emit({ kind: "created", ...buffer, category: "other" });
    }
    this.#meshes.set(mesh, buffers);
    return mesh;
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    this.#available("createMaterial");
    this.counts.materials += 1;
    const material: MaterialHandle = { kind: "material", name: spec.name };
    this.#materials.add(material);
    return material;
  }

  createMaterialAsync(spec: WgslMaterialSpec): Promise<MaterialHandle> {
    if (this.#unavailable) {
      return Promise.reject(new EngineUnavailable("createMaterialAsync"));
    }
    if (this.#failsMaterial(spec.name)) {
      return Promise.reject(new Error(`material ${spec.name} could not be compiled`));
    }
    return Promise.resolve(this.createMaterial(spec));
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    this.#available("createPostProcess");
    this.counts.postProcesses += 1;
    return { kind: "post-process", name: spec.name };
  }

  createCompute(pair: KernelPair): ComputeHandle {
    this.#available("createCompute");
    this.counts.kernels += 1;
    return { kind: "compute", name: pair.name, path: "reference" };
  }

  createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    return Promise.resolve(this.createCompute(pair));
  }

  createBuffer(spec: BufferSpec): BufferHandle {
    this.#available("createBuffer");
    const { maxBufferSize, maxStorageBufferBindingSize } = this.#capabilities;
    if (spec.bytes > maxBufferSize) {
      throw new LimitExceeded(`buffer ${spec.name} of ${spec.bytes} B exceeds ${maxBufferSize} B`);
    }
    if ((spec.usage & BUFFER_USAGE.STORAGE) !== 0 && spec.bytes > maxStorageBufferBindingSize) {
      throw new LimitExceeded(
        `storage buffer ${spec.name} of ${spec.bytes} B exceeds a binding's ${maxStorageBufferBindingSize} B`,
      );
    }
    this.counts.buffers += 1;
    this.#emit({ kind: "created", name: spec.name, bytes: spec.bytes, category: spec.category });
    const handle: BufferHandle = { kind: "buffer", name: spec.name, bytes: spec.bytes };
    this.#made.set(handle, { bytes: spec.bytes, category: spec.category });
    return handle;
  }

  createTexture(spec: TextureSpec): TextureHandle {
    this.#available("createTexture");
    const { width, height, depthOrArrayLayers } = extentOf(spec.size);
    const most = this.#capabilities.maxTextureDimension2D;
    if (
      spec.dimension !== "3d" &&
      (width > most || height > most || depthOrArrayLayers > MAX_TEXTURE_ARRAY_LAYERS)
    ) {
      throw new LimitExceeded(
        `texture ${spec.name} of ${width} × ${height} × ${depthOrArrayLayers} exceeds ${most}² × ${MAX_TEXTURE_ARRAY_LAYERS}`,
      );
    }
    this.counts.textures += 1;
    this.textureSpecs.push(spec);
    const bytes = textureBytes(spec);
    this.#emit({ kind: "created", name: spec.name, bytes, category: spec.category });
    const handle: TextureHandle = { kind: "texture", name: spec.name };
    this.#made.set(handle, { bytes, category: spec.category });
    return handle;
  }

  createPackedCube(): TextureHandle {
    throw notFaked("createPackedCube");
  }

  writePackedCubeLevel(): void {
    throw notFaked("writePackedCubeLevel");
  }

  writePackedCubeLevelFromBuffer(): void {
    throw notFaked("writePackedCubeLevelFromBuffer");
  }

  createPointSplat(): PointSplatHandle {
    throw notFaked("createPointSplat");
  }

  createPointSplatAsync(): Promise<PointSplatHandle> {
    return Promise.reject(notFaked("createPointSplatAsync"));
  }

  /** Raises the buffer's `destroyed` event, as the engine does (R06.T13.h). */
  releaseBuffer(buffer: BufferHandle): void {
    this.#release(buffer);
  }

  /** Raises the texture's `destroyed` event, as the engine does (R06.T13.h). */
  releaseTexture(texture: TextureHandle): void {
    this.#release(texture);
  }

  /**
   * Forgets a mesh, raising its buffers' `destroyed` events, as the engine destroys them
   * (R10.T16.b); a lost one is dropped.
   */
  releaseMesh(mesh: MeshHandle): void {
    if (this.#lost.delete(mesh)) {
      return;
    }
    const buffers = this.#meshes.get(mesh);
    if (buffers === undefined) {
      throw new Error(`mesh ${mesh.name} was not made by this engine, or was released`);
    }
    this.#meshes.delete(mesh);
    for (const buffer of buffers) {
      this.#emit({ kind: "destroyed", ...buffer, category: "other" });
    }
  }

  /** Forgets a material, as the engine forgets its record (R10.T16.b); a lost one is dropped. */
  releaseMaterial(material: MaterialHandle): void {
    if (this.#lost.delete(material)) {
      return;
    }
    if (!this.#materials.delete(material)) {
      throw new Error(`material ${material.name} was not made by this engine, or was released`);
    }
  }

  #release(handle: BufferHandle | TextureHandle): void {
    if (this.#lost.delete(handle)) {
      return;
    }
    const made = this.#made.get(handle);
    if (made === undefined) {
      throw new Error(`${handle.name} was not made by this engine, or was released`);
    }
    this.#made.delete(handle);
    this.#emit({ kind: "destroyed", name: handle.name, ...made });
  }

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass?: string,
  ): void {
    this.counts.dispatches += 1;
    this.dispatched.push({
      kernel: kernel.name,
      bindings,
      workgroups,
      pass,
      framesBefore: this.targetFrames.length,
    });
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    if (this.#unavailable) {
      return;
    }
    this.counts.bufferWrites += 1;
    this.writes.push({
      buffer: buffer.name,
      offsetBytes,
      bytes: data.byteLength,
      data: copyOf(data),
    });
    this.#emit({ kind: "uploaded", name: buffer.name, bytes: data.byteLength });
  }

  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    if (this.#unavailable) {
      return;
    }
    this.counts.textureWrites += 1;
    this.textureWritten.push({ texture: texture.name, origin, size, data: copyOf(data) });
    this.#emit({ kind: "uploaded", name: texture.name, bytes: data.byteLength });
  }

  readBuffer(): Promise<ArrayBuffer> {
    return Promise.resolve(new ArrayBuffer(0));
  }

  readTexture(): Promise<ArrayBuffer> {
    return Promise.resolve(new ArrayBuffer(0));
  }

  onPassTimes(_listener: (times: PassTimes) => void): () => void {
    return () => undefined;
  }

  /** R01's fake's, 0 unless a test moves it. */
  get passTimesFrame(): number {
    return this.inner.passTimesFrame;
  }

  onAllocation(listener: (event: AllocationEvent) => void): () => void {
    this.#allocationListeners.add(listener);
    return () => {
      this.#allocationListeners.delete(listener);
    };
  }

  onFault(listener: (fault: GraphicsFault) => void): () => void {
    return this.inner.onFault(listener);
  }

  onRestored(listener: () => void): () => void {
    this.#restoredListeners.add(listener);
    return () => {
      this.#restoredListeners.delete(listener);
    };
  }

  dispose(): void {
    this.inner.dispose();
    this.#allocationListeners.clear();
    this.#restoredListeners.clear();
  }
}

/** The buffer limits a counting engine's device is granted. */
export interface CountingLimits {
  readonly maxStorageBufferBindingSize: number;
  readonly maxBufferSize: number;
}

/**
 * A counting engine on a fake SwiftShader device, with no optional features: with WebGPU's default
 * limits, or with `limits` asked of an adapter that has them, as `createWebGpuEngine` asks
 * (decisions-r06-r07.md item 7).
 */
export async function countingRenderEngine(limits?: CountingLimits): Promise<CountingRenderEngine> {
  return new CountingRenderEngine(await fakeDevice(limits));
}

/** A fake SwiftShader device granted `limits`, or WebGPU's defaults; for a restore onto it. */
export async function fakeDevice(limits?: CountingLimits): Promise<GPUDevice> {
  if (limits === undefined) {
    return new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
  }
  const adapter = new FakeAdapter({ info: SWIFTSHADER_INFO, features: [], ...limits });
  return adapter.requestDevice({ requiredLimits: { ...limits } });
}
