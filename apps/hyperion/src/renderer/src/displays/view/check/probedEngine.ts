/**
 * An engine that tells a probe which view or target made each resolve (plan R07, T20).
 *
 * @remarks
 * R01's pass timer numbers each canvas or target `render` as one resolve (`passTimesFrame`), and
 * reports its times later without saying whose they are. The several-views check needs them by
 * view, so {@link ProbedEngine} forwards every member to the engine it wraps and wraps only the
 * views and targets it makes: their `render` notes the resolves it took and its CPU time, and their
 * `resize` the size given. Nothing else changes, so `VIEW` draws as it does on any launch. Only the
 * check's launch uses it (`ViewsProbe`). The terrain run (plan R10, R10.T16.d) also reads each
 * render's submission and each target made, through the same probe. A depth-layer target's render
 * (plan R10, R10.T9.b) notes its CPU time as `<owner>:shadows`'s, its owner the view it names: one
 * target serves every view.
 */

import type {
  AllocationEvent,
  BufferSpec,
  MemoryCategory,
  TextureSpec,
} from "../../../view/engine/memory";
import type { KernelPair } from "../../../view/engine/kernels";
import type { GraphicsFault } from "../../../view/engine/status";
import type {
  BufferHandle,
  ComputeBindings,
  ComputeHandle,
  DepthLayerPass,
  DepthLayersSpec,
  DepthLayerTarget,
  FrameSubmission,
  IndirectArgs,
  MaterialHandle,
  MeshHandle,
  MeshSpec,
  PassTimes,
  PointSplatHandle,
  PointSplatSpec,
  PostProcessHandle,
  PreparedTarget,
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
} from "../../../view/engine/types";

/** What a {@link ProbedEngine} reports. */
export interface EngineProbe {
  /**
   * A view's or target's `render` returned.
   *
   * @param owner - The view's or target's name.
   * @param firstResolve - The engine's `passTimesFrame` before the call: the render's resolves are
   *   the numbers after it, up to `lastResolve`; none where the two are equal (no timer).
   * @param cpuMs - The call's time on the main thread: encoding and submission.
   */
  rendered(owner: string, firstResolve: number, lastResolve: number, cpuMs: number): void;
  /** A view or target was made at, or resized to, `size`. */
  sized(owner: string, kind: "view" | "target", size: ViewSize): void;
  /** A view's or target's `render` of `frame` returned (after {@link EngineProbe.rendered}). */
  submitted(owner: string, frame: FrameSubmission): void;
  /** A target was made of `spec`; its attachments are read through it, as a resize replaces them. */
  madeTarget(spec: RenderTargetSpec, target: RenderTarget): void;
  /** A material's preparation began, which settles with `prepared` (plan R10, R10.T14.d). */
  preparing(prepared: Promise<void>): void;
}

/** A view that reports its renders and sizes. */
class ProbedView implements RenderView {
  readonly name: string;
  readonly #inner: RenderView;
  readonly #engine: RenderEngine;
  readonly #probe: EngineProbe;
  readonly #nowMs: () => number;

  constructor(inner: RenderView, engine: RenderEngine, probe: EngineProbe, nowMs: () => number) {
    this.name = inner.name;
    this.#inner = inner;
    this.#engine = engine;
    this.#probe = probe;
    this.#nowMs = nowMs;
  }

  resize(size: ViewSize): void {
    this.#probe.sized(this.name, "view", size);
    this.#inner.resize(size);
  }

  render(frame: FrameSubmission): void {
    const first = this.#engine.passTimesFrame;
    const startMs = this.#nowMs();
    this.#inner.render(frame);
    this.#probe.rendered(this.name, first, this.#engine.passTimesFrame, this.#nowMs() - startMs);
    this.#probe.submitted(this.name, frame);
  }

  readBack(): Promise<Float32Array | Uint8Array> {
    return this.#inner.readBack();
  }

  dispose(): void {
    this.#inner.dispose();
  }
}

/** A target that reports its renders and sizes. */
class ProbedTarget implements RenderTarget {
  readonly name: string;
  readonly #inner: RenderTarget;
  readonly #engine: RenderEngine;
  readonly #probe: EngineProbe;
  readonly #nowMs: () => number;

  constructor(inner: RenderTarget, engine: RenderEngine, probe: EngineProbe, nowMs: () => number) {
    this.name = inner.name;
    this.#inner = inner;
    this.#engine = engine;
    this.#probe = probe;
    this.#nowMs = nowMs;
  }

  // Read through, since a resize replaces the attachments.
  get colour(): TextureHandle {
    return this.#inner.colour;
  }

  get depth(): TextureHandle | null {
    return this.#inner.depth;
  }

  resize(size: ViewSize): void {
    this.#probe.sized(this.name, "target", size);
    this.#inner.resize(size);
  }

  render(frame: FrameSubmission): void {
    const first = this.#engine.passTimesFrame;
    const startMs = this.#nowMs();
    this.#inner.render(frame);
    this.#probe.rendered(this.name, first, this.#engine.passTimesFrame, this.#nowMs() - startMs);
    this.#probe.submitted(this.name, frame);
  }

  dispose(): void {
    this.#inner.dispose();
  }
}

/**
 * A depth-layer target that reports its renders' main-thread time (plan R10, R10.T9.b).
 *
 * @remarks
 * The engine has one set of cascades, drawn again for each view (decision-r10-t9-split §5(i)), so
 * a render is reported as its owner's, `<owner>:shadows`, which `viewOf` reads as the view. Its
 * passes take no resolve of their own: their GPU time rides the view's next render's resolve, so
 * the resolves it reports are normally none, and need no other attribution (decision-r10-t9b-design
 * D7). It reports no submission: a render is not a {@link FrameSubmission}.
 */
class ProbedDepthLayers implements DepthLayerTarget {
  readonly name: string;
  readonly sizePx: number;
  readonly layers: number;
  readonly depth: TextureHandle;
  readonly #inner: DepthLayerTarget;
  readonly #engine: RenderEngine;
  readonly #probe: EngineProbe;
  readonly #nowMs: () => number;

  constructor(
    inner: DepthLayerTarget,
    engine: RenderEngine,
    probe: EngineProbe,
    nowMs: () => number,
  ) {
    this.name = inner.name;
    this.sizePx = inner.sizePx;
    this.layers = inner.layers;
    this.depth = inner.depth;
    this.#inner = inner;
    this.#engine = engine;
    this.#probe = probe;
    this.#nowMs = nowMs;
  }

  render(owner: string, passes: ReadonlyArray<DepthLayerPass>): void {
    const first = this.#engine.passTimesFrame;
    const startMs = this.#nowMs();
    this.#inner.render(owner, passes);
    this.#probe.rendered(
      `${owner}:shadows`,
      first,
      this.#engine.passTimesFrame,
      this.#nowMs() - startMs,
    );
  }

  dispose(): void {
    this.#inner.dispose();
  }
}

/** `inner`, its views and targets reporting to `probe`; every other member forwarded unchanged. */
export class ProbedEngine implements RenderEngine {
  readonly #inner: RenderEngine;
  readonly #probe: EngineProbe;
  readonly #nowMs: () => number;

  constructor(inner: RenderEngine, probe: EngineProbe, nowMs: () => number) {
    this.#inner = inner;
    this.#probe = probe;
    this.#nowMs = nowMs;
  }

  get capabilities(): RenderEngine["capabilities"] {
    return this.#inner.capabilities;
  }

  get depthPolicy(): RenderEngine["depthPolicy"] {
    return this.#inner.depthPolicy;
  }

  get passTimesFrame(): number {
    return this.#inner.passTimesFrame;
  }

  createView(canvas: HTMLCanvasElement, name: string): RenderView {
    return new ProbedView(this.#inner.createView(canvas, name), this, this.#probe, this.#nowMs);
  }

  createRenderTarget(spec: RenderTargetSpec): RenderTarget {
    const target = this.#inner.createRenderTarget(spec);
    this.#probe.sized(spec.name, "target", spec.size);
    const probed = new ProbedTarget(target, this, this.#probe, this.#nowMs);
    this.#probe.madeTarget(spec, probed);
    return probed;
  }

  createDepthLayers(spec: DepthLayersSpec): DepthLayerTarget {
    return new ProbedDepthLayers(
      this.#inner.createDepthLayers(spec),
      this,
      this.#probe,
      this.#nowMs,
    );
  }

  createMesh(spec: MeshSpec): MeshHandle {
    return this.#inner.createMesh(spec);
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    return this.#inner.createMaterial(spec);
  }

  /**
   * Prepares through the inner engine, the probe counting each preparation in flight until it
   * settles (R10.T14.d): VIEW's opening ends once those its views made have.
   */
  prepareMaterial(
    material: MaterialHandle,
    targets: ReadonlyArray<PreparedTarget>,
    meshes: ReadonlyArray<MeshHandle>,
  ): Promise<void> {
    const prepared = this.#inner.prepareMaterial(material, targets, meshes);
    this.#probe.preparing(prepared);
    return prepared;
  }

  createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat>,
    meshes?: ReadonlyArray<MeshHandle>,
  ): Promise<MaterialHandle> {
    return this.#inner.createMaterialAsync(spec, targets, meshes);
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    return this.#inner.createPostProcess(spec);
  }

  createCompute(pair: KernelPair): ComputeHandle {
    return this.#inner.createCompute(pair);
  }

  createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    return this.#inner.createComputeAsync(pair);
  }

  createBuffer(spec: BufferSpec): BufferHandle {
    return this.#inner.createBuffer(spec);
  }

  createTexture(spec: TextureSpec): TextureHandle {
    return this.#inner.createTexture(spec);
  }

  createPackedCube(
    sizePx: number,
    mips: number,
    category: MemoryCategory,
    name?: string,
  ): TextureHandle {
    return this.#inner.createPackedCube(sizePx, mips, category, name);
  }

  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void {
    this.#inner.writePackedCubeLevel(cube, level, packed);
  }

  writePackedCubeLevelFromBuffer(
    cube: TextureHandle,
    level: number,
    packed: BufferHandle,
    face?: number,
  ): void {
    this.#inner.writePackedCubeLevelFromBuffer(cube, level, packed, face);
  }

  createPointSplat(spec: PointSplatSpec): PointSplatHandle {
    return this.#inner.createPointSplat(spec);
  }

  createPointSplatAsync(spec: PointSplatSpec): Promise<PointSplatHandle> {
    return this.#inner.createPointSplatAsync(spec);
  }

  releaseBuffer(buffer: BufferHandle): void {
    this.#inner.releaseBuffer(buffer);
  }

  releaseTexture(texture: TextureHandle): void {
    this.#inner.releaseTexture(texture);
  }

  releaseMesh(mesh: MeshHandle): void {
    this.#inner.releaseMesh(mesh);
  }

  releaseMaterial(material: MaterialHandle): void {
    this.#inner.releaseMaterial(material);
  }

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass?: string,
  ): void {
    this.#inner.dispatch(kernel, bindings, workgroups, pass);
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    this.#inner.writeBuffer(buffer, offsetBytes, data);
  }

  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.#inner.writeTexture(texture, origin, size, data);
  }

  readBuffer(buffer: BufferHandle, access?: "cpu" | "tolerance"): Promise<ArrayBuffer> {
    return this.#inner.readBuffer(buffer, access);
  }

  readTexture(
    texture: TextureHandle,
    level?: number,
    rect?: TexelRect,
    access?: "cpu" | "tolerance",
  ): Promise<ArrayBuffer> {
    return this.#inner.readTexture(texture, level, rect, access);
  }

  onPassTimes(listener: (times: PassTimes) => void): () => void {
    return this.#inner.onPassTimes(listener);
  }

  onAllocation(listener: (event: AllocationEvent) => void): () => void {
    return this.#inner.onAllocation(listener);
  }

  onFault(listener: (fault: GraphicsFault) => void): () => void {
    return this.#inner.onFault(listener);
  }

  onRestored(listener: () => void): () => void {
    return this.#inner.onRestored(listener);
  }

  dispose(): void {
    this.#inner.dispose();
  }
}
