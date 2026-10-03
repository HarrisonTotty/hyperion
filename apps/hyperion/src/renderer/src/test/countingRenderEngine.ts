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
 */

import type { KernelPair } from "../view/engine/kernels";
import {
  type AllocationEvent,
  type BufferSpec,
  type MemoryCategory,
  textureBytes,
  type TextureSpec,
} from "../view/engine/memory";
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
}

/** One recorded buffer write. */
export interface RecordedWrite {
  readonly buffer: string;
  readonly offsetBytes: number;
  readonly bytes: number;
  /** A copy of the data written, as bytes. */
  readonly data: Uint8Array;
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
  readonly capabilities: RenderEngine["capabilities"];
  readonly depthPolicy = "reversed-z-float" as const;
  /** R01's fake, which makes the views and raises the faults. */
  readonly inner: FakeRenderEngine;
  /** Totals since creation or {@link CountingRenderEngine.resetCounts}. */
  counts: EngineCounts = zeroCounts();
  /** Every dispatch, first first. */
  readonly dispatched: RecordedDispatch[] = [];
  /** Every buffer write, first first. */
  readonly writes: RecordedWrite[] = [];
  /** Every frame submitted to a render target, first first (views record their own). */
  readonly targetFrames: FrameSubmission[] = [];
  readonly #allocationListeners = new Set<(event: AllocationEvent) => void>();
  readonly #restoredListeners = new Set<() => void>();

  constructor(device: GPUDevice) {
    this.inner = new FakeRenderEngine(device);
    this.capabilities = this.inner.capabilities;
  }

  /** Zeroes {@link CountingRenderEngine.counts}; the recorded lists are kept. */
  resetCounts(): void {
    this.counts = zeroCounts();
  }

  /** Raises a restore after a device loss, as the resilient engine does once it has rebuilt. */
  restore(): void {
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
    return {
      name: spec.name,
      colour,
      depth,
      resize: (): void => undefined,
      render: (frame: FrameSubmission): void => {
        this.counts.frames += 1;
        this.counts.draws += frame.draws.length;
        this.targetFrames.push(frame);
      },
      dispose: (): void => undefined,
    };
  }

  createMesh(spec: MeshSpec): MeshHandle {
    this.counts.meshes += 1;
    return { kind: "mesh", name: spec.name };
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    this.counts.materials += 1;
    return { kind: "material", name: spec.name };
  }

  createMaterialAsync(spec: WgslMaterialSpec): Promise<MaterialHandle> {
    return Promise.resolve(this.createMaterial(spec));
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    this.counts.postProcesses += 1;
    return { kind: "post-process", name: spec.name };
  }

  createCompute(pair: KernelPair): ComputeHandle {
    this.counts.kernels += 1;
    return { kind: "compute", name: pair.name, path: "reference" };
  }

  createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    return Promise.resolve(this.createCompute(pair));
  }

  createBuffer(spec: BufferSpec): BufferHandle {
    this.counts.buffers += 1;
    this.#emit({ kind: "created", name: spec.name, bytes: spec.bytes, category: spec.category });
    return { kind: "buffer", name: spec.name, bytes: spec.bytes };
  }

  createTexture(spec: TextureSpec): TextureHandle {
    this.counts.textures += 1;
    this.#emit({
      kind: "created",
      name: spec.name,
      bytes: textureBytes(spec),
      category: spec.category,
    });
    return { kind: "texture", name: spec.name };
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

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass?: string,
  ): void {
    this.counts.dispatches += 1;
    this.dispatched.push({ kernel: kernel.name, bindings, workgroups, pass });
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
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
    _origin: GPUOrigin3D,
    _size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.counts.textureWrites += 1;
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

/** A counting engine on a fake SwiftShader device, with no optional features. */
export async function countingRenderEngine(): Promise<CountingRenderEngine> {
  const adapter = new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] });
  return new CountingRenderEngine(await adapter.requestDevice());
}
