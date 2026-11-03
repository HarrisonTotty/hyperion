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
 * target serves every view. An inspection profile's `--composite-probe` (the owner's priority,
 * 2026-11-01) and `--march-probe` (plan R08, R08.T8.d) are the changes it makes: the atmosphere's
 * composite, or its march, made under the probe. It also takes the march's far-limb census beside a
 * march's dispatch when asked ({@link ProbedEngine.marchCensus}), a dispatch of its own after it.
 */

import type {
  AllocationEvent,
  BufferSpec,
  MemoryCategory,
  TextureSpec,
} from "../../../view/engine/memory";
import {
  COMPOSITE_MATERIAL,
  type CompositeProbe,
  compositeProbedMaterial,
} from "../../../view/atmosphere/hillaire";
import {
  MARCH_CENSUS_BUFFER,
  MARCH_CENSUS_KERNEL,
  type MarchCensus,
  marchCensusBindingsOf,
  marchCensusOf,
  type MarchProbe,
  marchProbedKernel,
} from "../../../view/atmosphere/marchProbe";
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
  /**
   * A kernel was dispatched under the pass label `pass` (the inspection run's frame profile; the
   * owner's report, 2026-11-01).
   *
   * @param workgroups - Its workgroups, x × y × z, or `null` for an indirect dispatch, whose count
   *   the GPU holds.
   */
  dispatched(kernel: string, workgroups: number | null, pass: string | undefined): void;
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
  readonly #compositeProbe: CompositeProbe | null;
  readonly #marchProbe: MarchProbe | null;
  /** The censuses asked for and not yet taken, each settled by the next march's census. */
  #censusesAsked: Array<{
    readonly resolve: (census: MarchCensus) => void;
    readonly reject: (cause: unknown) => void;
    /** Stops following the ask's signal, once the ask is taken. */
    readonly release: () => void;
  }> = [];

  /**
   * @param compositeProbe - The inspection profile's probe of the atmosphere's composite
   *   (`--composite-probe`; the owner's priority, 2026-11-01): each composite made through this
   *   engine is made under it ({@link compositeProbedMaterial}); `null`, the default, as drawn.
   * @param marchProbe - The inspection profile's probe of the atmosphere's march (`--march-probe`;
   *   plan R08, R08.T8.d): each of the march's kernels made through this engine is made under it
   *   ({@link marchProbedKernel}); `null`, the default, as drawn.
   */
  constructor(
    inner: RenderEngine,
    probe: EngineProbe,
    nowMs: () => number,
    compositeProbe: CompositeProbe | null = null,
    marchProbe: MarchProbe | null = null,
  ) {
    this.#inner = inner;
    this.#probe = probe;
    this.#nowMs = nowMs;
    this.#compositeProbe = compositeProbe;
    this.#marchProbe = marchProbe;
  }

  /**
   * The far limb's census of the next march dispatched through this engine over an area
   * ({@link marchCensusBindingsOf}), taken by a dispatch of its own after it and read back
   * (`marchCensus.wgsl`; plan R08, R08.T8.d): check-only, for the inspection profile, which asks
   * once a window has been measured. It waits for that march however long it takes, until
   * `signal` aborts: the ask is then withdrawn and the promise never settles, so that the caller
   * bounds the wait and no census is taken later in a window it times.
   *
   * @throws Error, as a rejection, as the census's dispatch or its read-back fails.
   */
  marchCensus(signal: AbortSignal): Promise<MarchCensus> {
    return new Promise((resolve, reject) => {
      if (signal.aborted) {
        return;
      }
      const withdraw = (): void => {
        this.#censusesAsked = this.#censusesAsked.filter((each) => each !== asked);
      };
      const asked = {
        resolve,
        reject,
        release: () => {
          signal.removeEventListener("abort", withdraw);
        },
      };
      this.#censusesAsked.push(asked);
      signal.addEventListener("abort", withdraw, { once: true });
    });
  }

  /** `pair`, or the march's under the launch's probe where it is one of the march's and one is set. */
  #kernelOf(pair: KernelPair): KernelPair {
    return this.#marchProbe === null ? pair : marchProbedKernel(pair, this.#marchProbe);
  }

  /**
   * The census asked for beside the dispatch of `kernel` over `workgroups` with `bindings`, where
   * it is a march's over an area: its kernel and buffer made for it, released once it is read.
   */
  #takeCensus(
    kernel: string,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number],
  ): void {
    const bound = marchCensusBindingsOf(kernel, bindings);
    if (bound === null) {
      return;
    }
    const inner = this.#inner;
    const buffer = inner.createBuffer({
      name: MARCH_CENSUS_KERNEL.name,
      ...MARCH_CENSUS_BUFFER,
      category: "other",
    });
    const asked = this.#censusesAsked;
    this.#censusesAsked = [];
    let counter: ComputeHandle | null = null;
    let read: Promise<MarchCensus>;
    try {
      counter = inner.createCompute(MARCH_CENSUS_KERNEL);
      inner.writeBuffer(buffer, 0, new Uint32Array(MARCH_CENSUS_BUFFER.bytes / 4));
      inner.dispatch(counter, bound(buffer), workgroups, MARCH_CENSUS_KERNEL.name);
      read = inner
        .readBuffer(buffer)
        .then((bytes) => marchCensusOf(kernel, new Uint32Array(bytes)));
    } catch (error: unknown) {
      read = Promise.reject(
        error instanceof Error ? error : new Error(String(error), { cause: error }),
      );
    }
    // Released once read, so that nothing the dispatch holds goes before it has run.
    const made = counter;
    const released = read.finally(() => {
      inner.releaseBuffer(buffer);
      if (made !== null) {
        inner.releaseCompute(made);
      }
    });
    for (const { resolve, reject, release } of asked) {
      release();
      void released.then(resolve, reject);
    }
  }

  /** `spec`, or the composite's under the launch's probe where it is the composite and one is set. */
  #materialOf(spec: WgslMaterialSpec): WgslMaterialSpec {
    return this.#compositeProbe !== null && spec.name === COMPOSITE_MATERIAL.name
      ? compositeProbedMaterial(this.#compositeProbe)
      : spec;
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
    return this.#inner.createMaterial(this.#materialOf(spec));
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
    return this.#inner.createMaterialAsync(this.#materialOf(spec), targets, meshes);
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    return this.#inner.createPostProcess(spec);
  }

  createCompute(pair: KernelPair): ComputeHandle {
    return this.#inner.createCompute(this.#kernelOf(pair));
  }

  createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    return this.#inner.createComputeAsync(this.#kernelOf(pair));
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
    region?: TexelRect,
  ): void {
    this.#inner.writePackedCubeLevelFromBuffer(cube, level, packed, face, region);
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

  releaseCompute(kernel: ComputeHandle): void {
    this.#inner.releaseCompute(kernel);
  }

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass?: string,
  ): void {
    this.#inner.dispatch(kernel, bindings, workgroups, pass);
    this.#probe.dispatched(
      kernel.name,
      Array.isArray(workgroups) ? workgroups[0] * workgroups[1] * workgroups[2] : null,
      pass,
    );
    if (this.#censusesAsked.length > 0 && !("buffer" in workgroups)) {
      this.#takeCensus(kernel.name, bindings, workgroups);
    }
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
