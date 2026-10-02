/**
 * The engine that survives a lost device: it re-creates the engine on a fresh adapter, up to the
 * session's limit, and its views with it.
 *
 * @remarks
 * On a loss the engine reports `device-lost` to the status store, disposes the lost engine and
 * every view's context and target, asks for a fresh adapter (an adapter is consumed by its first
 * device, so the old one is never reused), and loads a new engine through the same import (R01
 * Design notes 7 and 9). A null adapter means Chromium has withdrawn WebGPU: the status becomes
 * `disabled` with cause `adapter-withdrawn`. After `DEVICE_LOSS_LIMIT` losses it stops, and the
 * status is `disabled` with cause `device-losses`. Views survive a rebuild, re-created at their
 * sizes; every other handle belongs to the lost engine, so the caller makes them again when
 * {@link RenderEngine.onRestored} fires. While there is no device a view draws nothing, and a
 * write or a dispatch is dropped, since its handles died with the device; a creation throws
 * {@link EngineUnavailable}. The store is told of every loss, so an `onFault` listener does not
 * dispatch it again.
 */

import type { KernelPair } from "./kernels";
import type { AllocationEvent, BufferSpec, MemoryCategory, TextureSpec } from "./memory";
import { type AdapterOutcome, type GpuCapabilities, requestAdapterOutcome } from "./platform";
import type { GraphicsFault, GraphicsStatusStore } from "./status";
import type {
  BufferHandle,
  ComputeBindings,
  ComputeHandle,
  DepthPolicy,
  FrameSubmission,
  IndirectArgs,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  PassTimes,
  PointSplatHandle,
  PointSplatSpec,
  PostProcessHandle,
  RenderEngine,
  RenderTarget,
  RenderTargetFormat,
  RenderTargetSpec,
  RenderView,
  TexelRect,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
  WgslPostProcessSpec,
} from "./types";

/** Makes an engine on a vetted adapter: `createWebGpuEngine` through the dynamic import. */
export type EngineFactory = (
  outcome: AdapterOutcome & { readonly kind: "adapter" },
) => Promise<RenderEngine>;

/** Thrown by a call that needs a device while there is none: lost, or WebGPU given up on. */
export class EngineUnavailable extends Error {
  constructor(member: string) {
    super(`RenderEngine.${member} has no device: it was lost, or WebGPU is disabled`);
    this.name = "EngineUnavailable";
  }
}

/** A view that outlives the engine that drew it. */
class ResilientView implements RenderView {
  readonly name: string;
  readonly canvas: HTMLCanvasElement;
  #inner: RenderView | null;
  #size: ViewSize | null = null;
  readonly #forget: (view: ResilientView) => void;

  constructor(
    canvas: HTMLCanvasElement,
    name: string,
    inner: RenderView | null,
    forget: (view: ResilientView) => void,
  ) {
    this.canvas = canvas;
    this.name = name;
    this.#inner = inner;
    this.#forget = forget;
  }

  /** The size it was last given, or `null` for the canvas's own. */
  get size(): ViewSize | null {
    return this.#size;
  }

  /**
   * Points the view at a rebuilt engine's view of the same canvas, or at none, disposing the one
   * it held: its context and target belonged to the lost device.
   */
  attach(inner: RenderView | null): void {
    this.#inner?.dispose();
    this.#inner = inner;
    if (inner !== null && this.#size !== null) {
      inner.resize(this.#size);
    }
  }

  resize(size: ViewSize): void {
    this.#size = size;
    this.#inner?.resize(size);
  }

  render(frame: FrameSubmission): void {
    this.#inner?.render(frame);
  }

  readBack(): Promise<Float32Array | Uint8Array> {
    return this.#inner === null
      ? Promise.reject(new EngineUnavailable("RenderView.readBack"))
      : this.#inner.readBack();
  }

  dispose(): void {
    this.#inner?.dispose();
    this.#inner = null;
    this.#forget(this);
  }
}

/** The engine a caller holds: the current one, re-created after each loss. */
export class ResilientEngine implements RenderEngine {
  readonly depthPolicy: DepthPolicy = "reversed-z-float";
  readonly #status: GraphicsStatusStore;
  readonly #gpu: GPU | undefined;
  readonly #create: EngineFactory;
  readonly #views = new Set<ResilientView>();
  readonly #restoredListeners = new Set<() => void>();
  readonly #faultListeners = new Set<(fault: GraphicsFault) => void>();
  readonly #allocationListeners = new Set<(event: AllocationEvent) => void>();
  readonly #passTimeListeners = new Set<(times: PassTimes) => void>();
  #inner: RenderEngine | null = null;
  #capabilities: GpuCapabilities;
  #unsubscribe: Array<() => void> = [];
  #disposed = false;

  /**
   * Wraps the first engine.
   *
   * @param inner - The first engine, already made.
   * @param gpu - `navigator.gpu`, from which each rebuild asks for a fresh adapter.
   */
  constructor(
    inner: RenderEngine,
    status: GraphicsStatusStore,
    gpu: GPU | undefined,
    create: EngineFactory,
  ) {
    this.#status = status;
    this.#gpu = gpu;
    this.#create = create;
    this.#capabilities = inner.capabilities;
    this.#adopt(inner);
  }

  /** The current engine's device's features; the last engine's while there is none. */
  get capabilities(): GpuCapabilities {
    return this.#capabilities;
  }

  /** A view, made at once, or drawn from the restore on if the device is lost now. */
  createView(canvas: HTMLCanvasElement, name: string): RenderView {
    const view = new ResilientView(
      canvas,
      name,
      this.#inner?.createView(canvas, name) ?? null,
      (v) => {
        this.#views.delete(v);
      },
    );
    this.#views.add(view);
    return view;
  }

  createRenderTarget(spec: RenderTargetSpec): RenderTarget {
    return this.#current("createRenderTarget").createRenderTarget(spec);
  }

  createMesh(spec: MeshSpec): MeshHandle {
    return this.#current("createMesh").createMesh(spec);
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    return this.#current("createMaterial").createMaterial(spec);
  }

  createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat>,
    meshes: ReadonlyArray<MeshHandle> = [],
  ): Promise<MaterialHandle> {
    return this.#currentAsync("createMaterialAsync", (inner) =>
      inner.createMaterialAsync(spec, targets, meshes),
    );
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    return this.#current("createPostProcess").createPostProcess(spec);
  }

  createCompute(pair: KernelPair): ComputeHandle {
    return this.#current("createCompute").createCompute(pair);
  }

  createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    return this.#currentAsync("createComputeAsync", (inner) => inner.createComputeAsync(pair));
  }

  createBuffer(spec: BufferSpec): BufferHandle {
    return this.#current("createBuffer").createBuffer(spec);
  }

  createTexture(spec: TextureSpec): TextureHandle {
    return this.#current("createTexture").createTexture(spec);
  }

  createPackedCube(sizePx: number, mips: number, category: MemoryCategory): TextureHandle {
    return this.#current("createPackedCube").createPackedCube(sizePx, mips, category);
  }

  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void {
    this.#current("writePackedCubeLevel").writePackedCubeLevel(cube, level, packed);
  }

  writePackedCubeLevelFromBuffer(cube: TextureHandle, level: number, packed: BufferHandle): void {
    this.#current("writePackedCubeLevelFromBuffer").writePackedCubeLevelFromBuffer(
      cube,
      level,
      packed,
    );
  }

  createPointSplat(spec: PointSplatSpec): PointSplatHandle {
    return this.#current("createPointSplat").createPointSplat(spec);
  }

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass?: string,
  ): void {
    this.#inner?.dispatch(kernel, bindings, workgroups, pass);
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    this.#inner?.writeBuffer(buffer, offsetBytes, data);
  }

  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.#inner?.writeTexture(texture, origin, size, data);
  }

  readBuffer(buffer: BufferHandle, access?: "cpu" | "tolerance"): Promise<ArrayBuffer> {
    return this.#currentAsync("readBuffer", (inner) => inner.readBuffer(buffer, access));
  }

  readTexture(texture: TextureHandle, level?: number, rect?: TexelRect): Promise<ArrayBuffer> {
    return this.#currentAsync("readTexture", (inner) => inner.readTexture(texture, level, rect));
  }

  onPassTimes(listener: (times: PassTimes) => void): () => void {
    return this.#listen(this.#passTimeListeners, listener);
  }

  onAllocation(listener: (event: AllocationEvent) => void): () => void {
    return this.#listen(this.#allocationListeners, listener);
  }

  onFault(listener: (fault: GraphicsFault) => void): () => void {
    return this.#listen(this.#faultListeners, listener);
  }

  onRestored(listener: () => void): () => void {
    return this.#listen(this.#restoredListeners, listener);
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#release();
    this.#views.clear();
    this.#restoredListeners.clear();
    this.#faultListeners.clear();
    this.#allocationListeners.clear();
    this.#passTimeListeners.clear();
  }

  #listen<T>(listeners: Set<T>, listener: T): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }

  /** Whether `dispose` has run, read afresh after an `await`. */
  #isDisposed(): boolean {
    return this.#disposed;
  }

  #current(member: string): RenderEngine {
    if (this.#inner === null) {
      throw new EngineUnavailable(member);
    }
    return this.#inner;
  }

  #currentAsync<T>(member: string, call: (inner: RenderEngine) => Promise<T>): Promise<T> {
    return this.#inner === null ? Promise.reject(new EngineUnavailable(member)) : call(this.#inner);
  }

  /** Makes `inner` the current engine and forwards what it reports. */
  #adopt(inner: RenderEngine): void {
    this.#inner = inner;
    this.#capabilities = inner.capabilities;
    this.#unsubscribe = [
      inner.onFault((fault) => {
        for (const listener of this.#faultListeners) {
          listener(fault);
        }
        if (fault.kind === "device-lost" && inner === this.#inner) {
          void this.#rebuild(fault).catch((error: unknown) => {
            console.error("the rebuild after a device loss stopped:", error);
          });
        }
      }),
      inner.onAllocation((event) => {
        for (const listener of this.#allocationListeners) {
          listener(event);
        }
      }),
      inner.onPassTimes((times) => {
        for (const listener of this.#passTimeListeners) {
          listener(times);
        }
      }),
    ];
  }

  /**
   * Disposes the current engine and every view's attachments to it, then stops forwarding what it
   * reports: its disposal's `destroyed` events reach the allocation listeners, so that a tally
   * built on them lets go of the lost device's bytes.
   */
  #release(): void {
    for (const view of this.#views) {
      view.attach(null);
    }
    this.#inner?.dispose();
    this.#inner = null;
    for (const unsubscribe of this.#unsubscribe) {
      unsubscribe();
    }
    this.#unsubscribe = [];
  }

  /**
   * Re-creates the engine after a loss, on fresh adapters, until one holds, the session's limit is
   * reached, or WebGPU is withdrawn.
   *
   * @remarks
   * A creation that fails (a device request on a GPU that has just crashed, say) counts as another
   * loss, so the limit bounds the retries and the status never stays at `DEVICE LOST`.
   */
  async #rebuild(fault: GraphicsFault & { readonly kind: "device-lost" }): Promise<void> {
    let lost: { readonly reason: GPUDeviceLostReason; readonly message: string } = fault;
    for (;;) {
      this.#status.dispatch({ kind: "device-lost", reason: lost.reason, message: lost.message });
      this.#release();
      if (this.#isDisposed() || this.#status.getSnapshot().condition.kind === "disabled") {
        return;
      }
      // Each attempt waits for the one before it: the adapter request and the creation are serial.
      // oxlint-disable-next-line no-await-in-loop
      const outcome = await requestAdapterOutcome(this.#gpu);
      if (this.#isDisposed()) {
        return;
      }
      if (outcome.kind !== "adapter") {
        this.#status.dispatch({ kind: "adapter-withdrawn" });
        return;
      }
      // Only the creation is a loss when it fails: what the restore runs afterwards (a view's
      // context, a caller's listener) is the new engine's caller's, and the engine stays.
      let inner: RenderEngine;
      try {
        // oxlint-disable-next-line no-await-in-loop
        inner = await this.#create(outcome);
      } catch (error: unknown) {
        console.error("re-creating the engine after a device loss failed:", error);
        lost = {
          reason: "unknown",
          message: error instanceof Error ? error.message : "the engine could not be made",
        };
        continue;
      }
      this.#restore(outcome, inner);
      return;
    }
  }

  /**
   * Makes `inner` current after a rebuild, its views first, then tells the store and callers.
   *
   * @remarks
   * A view whose canvas gives no context, or a listener that throws, is logged and passed over on
   * its own: neither is a loss, and the others are still restored and told.
   */
  #restore(outcome: AdapterOutcome & { readonly kind: "adapter" }, inner: RenderEngine): void {
    if (this.#isDisposed()) {
      inner.dispose();
      return;
    }
    this.#adopt(inner);
    for (const view of this.#views) {
      try {
        view.attach(inner.createView(view.canvas, view.name));
      } catch (error: unknown) {
        console.error(`view ${view.name} could not be re-created after a device loss:`, error);
      }
    }
    this.#status.dispatch({ kind: "device-restored", outcome });
    // The restore writes the adapter's capabilities; the device's are what holds (Design note 7).
    this.#status.dispatch({ kind: "device-capabilities", capabilities: inner.capabilities });
    for (const listener of this.#restoredListeners) {
      try {
        listener();
      } catch (error: unknown) {
        console.error("an onRestored listener failed:", error);
      }
    }
  }
}
