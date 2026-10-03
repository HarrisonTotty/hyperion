/**
 * A fake engine module for tests: an engine that makes views, consumes its
 * adapter as a real one does, and whose device loss the test raises.
 *
 * @remarks
 * Only views, faults, allocation listeners, disposal, buffer handles and releases are faked;
 * every other member throws, naming itself.
 */

import type { AllocationEvent, BufferSpec } from "../view/engine/memory";
import { deviceCapabilities } from "../view/engine/platform";
import type { GraphicsFault } from "../view/engine/status";
import type {
  BufferHandle,
  CreateWebGpuEngine,
  FrameSubmission,
  RenderEngine,
  RenderView,
  TextureHandle,
  ViewSize,
} from "../view/engine/types";

/** The allocation every fake engine holds, and whose `destroyed` event its disposal raises. */
export const FAKE_ENGINE_MEMORY = {
  name: "fake engine memory",
  bytes: 64,
  category: "other",
} as const;

function notFaked(member: string): Error {
  return new Error(`FakeRenderEngine does not fake ${member}`);
}

/** A view that records its sizes and frames. */
export class FakeView implements RenderView {
  readonly name: string;
  readonly canvas: HTMLCanvasElement;
  readonly sizes: ViewSize[] = [];
  readonly frames: FrameSubmission[] = [];
  disposed = false;

  constructor(canvas: HTMLCanvasElement, name: string) {
    this.canvas = canvas;
    this.name = name;
  }

  resize(size: ViewSize): void {
    this.sizes.push(size);
  }
  render(frame: FrameSubmission): void {
    this.frames.push(frame);
  }
  readBack(): Promise<Uint8Array> {
    return Promise.resolve(new Uint8Array(0));
  }
  dispose(): void {
    this.disposed = true;
  }
}

/** An engine made on a device the test may lose. */
export class FakeRenderEngine implements RenderEngine {
  readonly capabilities: RenderEngine["capabilities"];
  readonly depthPolicy = "reversed-z-float" as const;
  readonly views: FakeView[] = [];
  /** The buffers and textures released, in order. */
  readonly released: Array<BufferHandle | TextureHandle> = [];
  disposed = false;
  readonly #faultListeners = new Set<(fault: GraphicsFault) => void>();
  readonly #allocationListeners = new Set<(event: AllocationEvent) => void>();
  readonly #viewless: boolean;
  #lost: GraphicsFault | null = null;

  /**
   * Wraps `device`.
   *
   * @param viewless - Whether `createView` throws, as on a canvas that gives no WebGPU context.
   */
  constructor(device: GPUDevice, viewless = false) {
    this.capabilities = deviceCapabilities(device);
    this.#viewless = viewless;
  }

  /** Raises a device loss, as the WebGPU engine reports one. */
  loseDevice(reason: GPUDeviceLostReason = "unknown"): void {
    const fault: GraphicsFault = { kind: "device-lost", reason, message: "fake loss" };
    this.#lost = fault;
    for (const listener of this.#faultListeners) {
      listener(fault);
    }
  }

  createView(canvas: HTMLCanvasElement, name: string): RenderView {
    if (this.#viewless) {
      throw new Error(`${name}'s canvas gives no WebGPU context`);
    }
    const view = new FakeView(canvas, name);
    this.views.push(view);
    return view;
  }
  /** Replays a loss raised before the listener came, as the WebGPU engine does. */
  onFault(listener: (fault: GraphicsFault) => void): () => void {
    this.#faultListeners.add(listener);
    const lost = this.#lost;
    if (lost !== null) {
      queueMicrotask(() => {
        listener(lost);
      });
    }
    return () => {
      this.#faultListeners.delete(listener);
    };
  }
  onAllocation(listener: (event: AllocationEvent) => void): () => void {
    this.#allocationListeners.add(listener);
    return () => {
      this.#allocationListeners.delete(listener);
    };
  }
  onPassTimes(): () => void {
    return () => undefined;
  }
  onRestored(): () => void {
    return () => undefined;
  }
  /** Releases its one fake allocation, {@link FAKE_ENGINE_MEMORY}, as the WebGPU engine does its own. */
  dispose(): void {
    this.disposed = true;
    for (const listener of this.#allocationListeners) {
      listener({ kind: "destroyed", ...FAKE_ENGINE_MEMORY });
    }
    this.#allocationListeners.clear();
  }
  createRenderTarget(): never {
    throw notFaked("createRenderTarget");
  }
  createMesh(): never {
    throw notFaked("createMesh");
  }
  createMaterial(): never {
    throw notFaked("createMaterial");
  }
  createMaterialAsync(): Promise<never> {
    return Promise.reject(notFaked("createMaterialAsync"));
  }
  createPostProcess(): never {
    throw notFaked("createPostProcess");
  }
  createCompute(): never {
    throw notFaked("createCompute");
  }
  createComputeAsync(): Promise<never> {
    return Promise.reject(notFaked("createComputeAsync"));
  }
  /** A handle with no GPU memory behind it, for tests of what is done with handles. */
  createBuffer(spec: BufferSpec): BufferHandle {
    return Object.freeze({ kind: "buffer", name: spec.name, bytes: spec.bytes });
  }
  createTexture(): never {
    throw notFaked("createTexture");
  }
  createPackedCube(): never {
    throw notFaked("createPackedCube");
  }
  writePackedCubeLevel(): never {
    throw notFaked("writePackedCubeLevel");
  }
  writePackedCubeLevelFromBuffer(): never {
    throw notFaked("writePackedCubeLevelFromBuffer");
  }
  createPointSplat(): never {
    throw notFaked("createPointSplat");
  }
  createPointSplatAsync(): Promise<never> {
    return Promise.reject(notFaked("createPointSplatAsync"));
  }
  /** Records a release, so a test sees what the resilient engine forwarded. */
  releaseBuffer(buffer: BufferHandle): void {
    this.released.push(buffer);
  }
  /** Records a release, as {@link FakeRenderEngine.releaseBuffer} does. */
  releaseTexture(texture: TextureHandle): void {
    this.released.push(texture);
  }
  dispatch(): never {
    throw notFaked("dispatch");
  }
  writeBuffer(): never {
    throw notFaked("writeBuffer");
  }
  writeTexture(): never {
    throw notFaked("writeTexture");
  }
  readBuffer(): Promise<never> {
    return Promise.reject(notFaked("readBuffer"));
  }
  readTexture(): Promise<never> {
    return Promise.reject(notFaked("readTexture"));
  }
}

/** How the fake module's creations go, by their index from 0. */
export interface FakeEngineScript {
  /** Creations that reject, as one on a GPU that has just crashed does. */
  readonly failing?: ReadonlySet<number>;
  /** Creations whose device is lost before the engine is handed over. */
  readonly lostAtBirth?: ReadonlySet<number>;
  /** Creations whose `createView` throws. */
  readonly viewless?: ReadonlySet<number>;
}

/** A fake engine module and the engines it has made, first first. */
export interface FakeEngineModule {
  readonly createWebGpuEngine: CreateWebGpuEngine;
  readonly engines: FakeRenderEngine[];
  /** The adapter each creation was handed. */
  readonly adapters: GPUAdapter[];
}

/** A module whose `createWebGpuEngine` requests a device from the handed adapter, as the real one does. */
export function fakeEngineModule(script: FakeEngineScript = {}): FakeEngineModule {
  const engines: FakeRenderEngine[] = [];
  const adapters: GPUAdapter[] = [];
  const createWebGpuEngine: CreateWebGpuEngine = async (outcome) => {
    const index = adapters.length;
    adapters.push(outcome.adapter);
    const device = await outcome.adapter.requestDevice();
    if (script.failing?.has(index) === true) {
      throw new Error(`creation ${index} failed`);
    }
    const engine = new FakeRenderEngine(device, script.viewless?.has(index) === true);
    engines.push(engine);
    if (script.lostAtBirth?.has(index) === true) {
      engine.loseDevice();
    }
    return engine;
  };
  return { createWebGpuEngine, engines, adapters };
}
