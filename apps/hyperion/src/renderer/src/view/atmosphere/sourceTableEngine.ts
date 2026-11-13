/**
 * The lit passes' table-only twins (plan R08, R08.T9.b's lever): an engine that draws a lit body of
 * one latitude band by modules with the per-fragment sliced lookup compiled out, and since R08.T8.e's
 * table marches its air by the ray march's twins, the sliced lookup a step compiled out.
 *
 * @remarks
 * With `AtmosphereSettings.sourceTransmittance`'s `table` a lit module reads each source's
 * transmittance from the body's sources' table, but keeps the sliced lookup beside it, for the table's
 * stub (`per-fragment`) and for a body of several bands, which takes a source near the horizon per
 * fragment. That code costs where it is never taken: on SwiftShader, with it compiled out, the pod's
 * bodies pass fell 37% further at low from orbit. So {@link SourceTableEngine} makes, beside each
 * material and kernel that binds a sources' table, its twin with `surfaceLighting.wgsl`'s
 * `SOURCE_TABLE_ONLY` set (`tableOnlyMaterial`, `tableOnlyKernel`), and draws or dispatches the twin
 * in its place wherever the table bound is a body of one band's
 * ({@link ONE_BAND_SOURCE_TABLE_NAME}, by the texture's name as it was made) and the twin is
 * prepared. There every lookup is the table's in both, the same arithmetic in the same order, so
 * that the twin draws the module's bytes; until it is prepared the module draws, as before.
 *
 * - **Materials.** A material's twin is made at its first draw into a target with a one-band table
 *   bound, and its pipeline prepared there for that target's format and the draw's mesh, off the
 *   frame (`prepareMaterial`); it stands in for draws into such a target with such a mesh once that
 *   preparation has settled, so that a twin never compiles on a frame. A view's draws, which no lit
 *   body takes (they draw into the scene target), take none.
 * - **Kernels.** A kernel's twin is made at its first dispatch with a one-band table bound,
 *   asynchronously, and stands in from the first such dispatch after it is made: the table's own
 *   build, which never binds one there, takes none. The ray march's thin forms declare the same
 *   table and constant (`marchSourceTable.wgsl`; `AtmosphereSettings.marchSourceTransmittance`'s
 *   `table`), and take their twins the same way; their every step's source is the table's in both.
 * - So a frame of no one-band body with air makes no twin, and every other member is forwarded
 *   unchanged. A release releases the twin with its original; after a restore the callers make
 *   their materials and kernels again, and their twins come again with their draws.
 */

import type { KernelPair } from "../engine/kernels";
import type { AllocationEvent, BufferSpec, MemoryCategory, TextureSpec } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { GraphicsFault } from "../engine/status";
import type {
  BufferHandle,
  CompileEvent,
  ComputeBindings,
  ComputeHandle,
  DepthLayersSpec,
  DepthLayerTarget,
  DrawItem,
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
  TargetRender,
  TexelRect,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
  WgslPostProcessSpec,
} from "../engine/types";
import {
  ONE_BAND_SOURCE_TABLE_NAME,
  SOURCE_TABLE_DRAW_TEXTURE,
  SOURCE_TABLE_KERNEL_TEXTURE,
  tableOnlyKernel,
  tableOnlyMaterial,
} from "./surfaceLighting";

/** A material's twin: its spec, its handle once made, and its preparations by mesh and target. */
interface MaterialTwin {
  readonly spec: WgslMaterialSpec;
  twin: MaterialHandle | null;
  /** By mesh, then by target ({@link preparedKey}): `true` once prepared, `false` while preparing. */
  readonly prepared: WeakMap<MeshHandle, Map<string, boolean>>;
}

/** A target's preparation and its key. */
interface TargetPreparation {
  readonly target: PreparedTarget;
  readonly key: string;
}

/** What a target of `spec` prepares a material for: its colour format, with depth where it has one. */
function preparationOf(spec: RenderTargetSpec): TargetPreparation {
  const depth = spec.depth || spec.depthOf !== undefined;
  return { target: { colour: spec.format, depth }, key: `${spec.format}:${String(depth)}` };
}

/** A kernel's twin: its pair, and its handle once made. */
interface KernelTwin {
  readonly pair: KernelPair;
  made: ComputeHandle | null;
  making: boolean;
}

/** The draws of `frame` by their twins where `twinOf` gives one; `frame` itself where none does. */
function withTwins(
  frame: FrameSubmission,
  twinOf: (draw: DrawItem) => MaterialHandle | null,
): FrameSubmission {
  let draws: DrawItem[] | null = null;
  for (const [i, draw] of frame.draws.entries()) {
    const twin = twinOf(draw);
    if (twin !== null) {
      draws ??= [...frame.draws];
      draws[i] = { ...draw, material: twin };
    }
  }
  return draws === null ? frame : { ...frame, draws };
}

/** A target that draws by the engine's twins. */
class TwinTarget implements RenderTarget {
  readonly name: string;
  readonly #inner: RenderTarget;
  readonly #twinOf: (draw: DrawItem) => MaterialHandle | null;

  constructor(
    inner: RenderTarget,
    twinOf: (draw: DrawItem, target: TargetPreparation) => MaterialHandle | null,
    target: TargetPreparation,
  ) {
    this.name = inner.name;
    this.#inner = inner;
    this.#twinOf = (draw) => twinOf(draw, target);
  }

  // Read through, since a resize replaces the attachments.
  get colour(): TextureHandle {
    return this.#inner.colour;
  }

  get depth(): TextureHandle | null {
    return this.#inner.depth;
  }

  resize(size: ViewSize): void {
    this.#inner.resize(size);
  }

  render(frame: FrameSubmission): void {
    this.#inner.render(this.twinned(frame));
  }

  /** `frame` as this target draws it, by the engine's twins where they stand in. */
  twinned(frame: FrameSubmission): FrameSubmission {
    return withTwins(frame, this.#twinOf);
  }

  /** The target as the engine made it, for an engine's `depthOf` (`RenderTargetSpec.depthOf`). */
  get inner(): RenderTarget {
    return this.#inner;
  }

  dispose(): void {
    this.#inner.dispose();
  }
}

/** `inner` drawing each lit body of one band by its modules' table-only twins (the module's remarks). */
export class SourceTableEngine implements RenderEngine {
  readonly #inner: RenderEngine;
  readonly #materials = new WeakMap<MaterialHandle, MaterialTwin>();
  readonly #kernels = new WeakMap<ComputeHandle, KernelTwin>();
  /** The one-band sources' tables made through this engine. */
  readonly #oneBand = new WeakSet<TextureHandle>();
  /** The preparations and makings of twins in flight, which {@link twinsSettled} awaits. */
  readonly #inFlight = new Set<Promise<void>>();
  #swaps = 0;

  constructor(inner: RenderEngine) {
    this.#inner = inner;
  }

  /** The engine it wraps, whose materials and kernels draw as written. */
  get inner(): RenderEngine {
    return this.#inner;
  }

  /** How many draws and dispatches have taken a twin so far, for the smoke's check. */
  get swaps(): number {
    return this.#swaps;
  }

  /** Settles once every twin's preparation or making begun so far has. */
  async twinsSettled(): Promise<void> {
    while (this.#inFlight.size > 0) {
      // Each round's settling may begin more.
      // oxlint-disable-next-line no-await-in-loop
      await Promise.allSettled(this.#inFlight);
    }
  }

  #track(work: Promise<void>): void {
    const tracked = work.finally(() => {
      this.#inFlight.delete(tracked);
    });
    this.#inFlight.add(tracked);
  }

  /** Whether `texture`, bound as a sources' table, is a body of one band's. */
  #holdsEveryLookup(texture: TextureHandle | undefined): boolean {
    return texture !== undefined && this.#oneBand.has(texture);
  }

  /**
   * The prepared twin `draw` into a target of `target` takes, or `null`: its making and its
   * preparation there begun where due.
   */
  #materialTwinOf = (draw: DrawItem, target: TargetPreparation): MaterialHandle | null => {
    const held = this.#materials.get(draw.material);
    if (held === undefined || !this.#holdsEveryLookup(draw.textures[SOURCE_TABLE_DRAW_TEXTURE])) {
      return null;
    }
    const twin = (held.twin ??= this.#inner.createMaterial(held.spec));
    let byTarget = held.prepared.get(draw.mesh);
    if (byTarget === undefined) {
      byTarget = new Map();
      held.prepared.set(draw.mesh, byTarget);
    }
    const state = byTarget.get(target.key);
    if (state === undefined) {
      const preparing = byTarget;
      preparing.set(target.key, false);
      this.#track(
        (async () => {
          try {
            await this.#inner.prepareMaterial(twin, [target.target], [draw.mesh]);
            preparing.set(target.key, true);
          } catch (error: unknown) {
            // A loss meanwhile is no failure: the restore makes the material and its twin again.
            if (!(error instanceof EngineUnavailable)) {
              console.error(`the table-only twin ${twin.name} could not be prepared:`, error);
            }
          }
        })(),
      );
    }
    if (state !== true) {
      return null;
    }
    this.#swaps += 1;
    return twin;
  };

  /** Notes `spec`'s twin, to make at its material `material`'s first draw that takes it. */
  #twin(material: MaterialHandle, spec: WgslMaterialSpec): void {
    const twinSpec = tableOnlyMaterial(spec);
    if (twinSpec !== null) {
      this.#materials.set(material, { spec: twinSpec, twin: null, prepared: new WeakMap() });
    }
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
    return this.#inner.createView(canvas, name);
  }

  createRenderTarget(spec: RenderTargetSpec): RenderTarget {
    const depthOf = spec.depthOf instanceof TwinTarget ? spec.depthOf.inner : spec.depthOf;
    const inner = this.#inner.createRenderTarget(
      depthOf === undefined ? spec : { ...spec, depthOf },
    );
    return new TwinTarget(inner, this.#materialTwinOf, preparationOf(spec));
  }

  /** The inner engine's one submission of each render, its frame by the twins as its own would. */
  renderTargets(renders: ReadonlyArray<TargetRender>): void {
    this.#inner.renderTargets(
      renders.map(({ target, frame }) =>
        target instanceof TwinTarget
          ? { target: target.inner, frame: target.twinned(frame) }
          : { target, frame },
      ),
    );
  }

  createDepthLayers(spec: DepthLayersSpec): DepthLayerTarget {
    // A caster binds no sources' table: its passes draw as written.
    return this.#inner.createDepthLayers(spec);
  }

  createMesh(spec: MeshSpec): MeshHandle {
    return this.#inner.createMesh(spec);
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    const material = this.#inner.createMaterial(spec);
    this.#twin(material, spec);
    return material;
  }

  async createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat>,
    meshes?: ReadonlyArray<MeshHandle>,
  ): Promise<MaterialHandle> {
    const material = await this.#inner.createMaterialAsync(spec, targets, meshes);
    this.#twin(material, spec);
    return material;
  }

  prepareMaterial(
    material: MaterialHandle,
    targets: ReadonlyArray<PreparedTarget>,
    meshes: ReadonlyArray<MeshHandle>,
  ): Promise<void> {
    return this.#inner.prepareMaterial(material, targets, meshes);
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    return this.#inner.createPostProcess(spec);
  }

  createCompute(pair: KernelPair): ComputeHandle {
    const kernel = this.#inner.createCompute(pair);
    this.#kernelTwin(kernel, pair);
    return kernel;
  }

  async createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    const kernel = await this.#inner.createComputeAsync(pair);
    this.#kernelTwin(kernel, pair);
    return kernel;
  }

  /** Notes `pair`'s twin to make at its kernel's first dispatch that takes it. */
  #kernelTwin(kernel: ComputeHandle, pair: KernelPair): void {
    const twinPair = tableOnlyKernel(pair);
    if (twinPair !== null) {
      this.#kernels.set(kernel, { pair: twinPair, made: null, making: false });
    }
  }

  /** The made twin a dispatch of `kernel` with `bindings` takes, its making begun where due. */
  #kernelTwinOf(kernel: ComputeHandle, bindings: ComputeBindings): ComputeHandle | null {
    const held = this.#kernels.get(kernel);
    if (
      held === undefined ||
      !this.#holdsEveryLookup(bindings.sampled[SOURCE_TABLE_KERNEL_TEXTURE])
    ) {
      return null;
    }
    if (held.made === null && !held.making) {
      held.making = true;
      this.#track(
        (async () => {
          try {
            held.made = await this.#inner.createComputeAsync(held.pair);
          } catch (error: unknown) {
            if (!(error instanceof EngineUnavailable)) {
              console.error(`the table-only twin of ${kernel.name} could not be made:`, error);
            }
          } finally {
            held.making = false;
          }
        })(),
      );
    }
    if (held.made !== null) {
      this.#swaps += 1;
    }
    return held.made;
  }

  createBuffer(spec: BufferSpec): BufferHandle {
    return this.#inner.createBuffer(spec);
  }

  createTexture(spec: TextureSpec): TextureHandle {
    const texture = this.#inner.createTexture(spec);
    if (spec.name === ONE_BAND_SOURCE_TABLE_NAME) {
      this.#oneBand.add(texture);
    }
    return texture;
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
    this.#oneBand.delete(texture);
    this.#inner.releaseTexture(texture);
  }

  releaseMesh(mesh: MeshHandle): void {
    this.#inner.releaseMesh(mesh);
  }

  releaseMaterial(material: MaterialHandle): void {
    const held = this.#materials.get(material);
    if (held !== undefined) {
      this.#materials.delete(material);
      if (held.twin !== null) {
        this.#inner.releaseMaterial(held.twin);
      }
    }
    this.#inner.releaseMaterial(material);
  }

  releaseCompute(kernel: ComputeHandle): void {
    const held = this.#kernels.get(kernel);
    if (held !== undefined) {
      this.#kernels.delete(kernel);
      if (held.made !== null) {
        this.#inner.releaseCompute(held.made);
      }
    }
    this.#inner.releaseCompute(kernel);
  }

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass?: string,
  ): void {
    this.#inner.dispatch(
      this.#kernelTwinOf(kernel, bindings) ?? kernel,
      bindings,
      workgroups,
      pass,
    );
  }

  stagedWrites<T>(label: string, write: () => T): T {
    return this.#inner.stagedWrites(label, write);
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

  onCompile(listener: (event: CompileEvent) => void): () => void {
    return this.#inner.onCompile(listener);
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
