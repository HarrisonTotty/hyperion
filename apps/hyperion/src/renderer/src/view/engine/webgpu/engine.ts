/**
 * HYPERION's own WebGPU implementation of the engine-agnostic interface, the one module that
 * `loadEngine.ts` imports dynamically, and so the root of a lazily loaded chunk.
 *
 * @remarks
 * The device is requested on the adapter the client vetted (Design note 7), with the features
 * `requiredFeatures` asks for; its capabilities are the device's, so a feature the smoke harness
 * withholds reads as absent. Every draw, post-process, dispatch, copy and readback is encoded by
 * the adapter itself on that device, through pipelines it makes from standard WGSL under explicit
 * layouts (Design note 23), and every buffer and texture is made through one registry that raises
 * an allocation event for it.
 */

import type { KernelPair } from "../kernels";
import type { AllocationEvent, BufferSpec, MemoryCategory, TextureSpec } from "../memory";
import {
  type AdapterOutcome,
  type CapabilityOverrides,
  deviceCapabilities,
  featuresNotEnabled,
  type GpuCapabilities,
  requiredFeatures,
} from "../platform";
import type {
  GraphicsFault,
  GraphicsStatusStore,
  ProbedTargetFormat,
  TargetRounding,
} from "../status";
import type {
  BufferHandle,
  ComputeBindings,
  ComputeHandle,
  CreateWebGpuEngine,
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
  SamplerSpec,
  TexelRect,
  TextureHandle,
  WgslMaterialSpec,
  WgslPostProcessSpec,
} from "../types";
import {
  createKernel,
  createKernelAsync,
  encodeDispatch,
  type KernelRecord,
  type Workgroups,
} from "./compute";
import { logUncapturedErrors, watchDeviceLoss } from "./deviceLoss";
import {
  Drawing,
  type DrawingHost,
  type FrameOutput,
  type MaterialRecord,
  type PostProcessRecord,
} from "./drawing";
import { resolveKernelResources } from "./kernelResources";
import {
  FULL_SCREEN_WGSL,
  materialBindings,
  type MaterialModules,
  postProcessBindings,
  samplerDescriptor,
} from "./materials";
import { MeshRecord } from "./meshes";
import { MipGenerator } from "./mipmaps";
import {
  assertSplatInputs,
  assertSplatSupported,
  encodeSplat,
  pointSplatPipelineDescriptor,
} from "./pointSplat";
import {
  assertBufferReadable,
  readGpuBuffer,
  readGpuTexture,
  textureRead,
  WriterRecord,
} from "./readback";
import { packedCubeSpec, ResourceRegistry } from "./resources";
import {
  drawAndReadProbe,
  type ProbeHost,
  probedFormats,
  probeTargetRounding,
} from "./roundingProbe";
import { WebGpuRenderTarget } from "./target";
import { PassTimer } from "./timing";
import { assertDrawStruct, OFFSET_MEMBER, uniformLayout } from "./uniforms";
import { srgbViewFormat, WebGpuView } from "./view";

/** A caught value as an `Error`, keeping it as the cause when it is not one. */
function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error), { cause: error });
}

/** The compiler's error messages for `module`, each with its line and column. */
export async function compilationErrors(
  module: Pick<GPUShaderModule, "getCompilationInfo">,
): Promise<ReadonlyArray<string>> {
  const info = await module.getCompilationInfo();
  return info.messages
    .filter((message) => message.type === "error")
    .map((message) => `${message.lineNum}:${message.linePos} ${message.message}`);
}

/** The engine over one device: views, targets, materials, kernels and the memory they use. */
export class WebGpuRenderEngine implements RenderEngine, DrawingHost {
  readonly capabilities: GpuCapabilities;
  readonly depthPolicy: DepthPolicy = "reversed-z-float";
  readonly device: GPUDevice;
  readonly #status: GraphicsStatusStore;
  readonly #meshes = new WeakMap<MeshHandle, MeshRecord>();
  readonly #materials = new WeakMap<MaterialHandle, MaterialRecord>();
  readonly #postProcesses = new WeakMap<PostProcessHandle, PostProcessRecord>();
  readonly #kernels = new WeakMap<ComputeHandle, KernelRecord>();
  readonly #views = new Set<WebGpuView>();
  readonly #targets = new Set<WebGpuRenderTarget>();
  /** The kernel that last wrote each buffer and texture (Design notes 16 and 20). */
  readonly #writers = new WriterRecord();
  #mipGenerator: MipGenerator | null = null;
  readonly #timer: PassTimer;
  readonly #samplers = new Map<string, GPUSampler>();
  readonly #allocationListeners = new Set<(event: AllocationEvent) => void>();
  readonly #faultListeners = new Set<(fault: GraphicsFault) => void>();
  /** The device's loss and error watches, ended at disposal. */
  readonly #watches: ReadonlyArray<() => void>;
  readonly #resources: ResourceRegistry;
  readonly #drawing: Drawing;
  /** Each kernel's uniform buffers, by uniform name. */
  readonly #kernelUniforms = new Map<KernelRecord, Map<string, BufferHandle>>();
  /** The device's loss, once reported. */
  #lost: GraphicsFault | null = null;
  #disposed = false;

  /**
   * Wraps a device requested on the vetted adapter.
   *
   * @param status - Where a shader that fails to compile is reported, and whose `gpuTiming` says
   * whether `--hyperion-gpu-timing` lifted timestamp quantization this launch.
   */
  constructor(device: GPUDevice, status: GraphicsStatusStore) {
    this.device = device;
    this.#status = status;
    this.capabilities = deviceCapabilities(device);
    const { gpuTiming } = status.getSnapshot();
    this.#timer = new PassTimer(
      device,
      this.capabilities.timestampQuery ? (gpuTiming ? "full" : "quantized") : "absent",
    );
    this.#resources = new ResourceRegistry(device, (event) => {
      for (const listener of this.#allocationListeners) {
        listener(event);
      }
    });
    this.#watches = [
      watchDeviceLoss(
        device,
        () => this.#disposed,
        (fault) => {
          this.#lost = fault;
          for (const listener of this.#faultListeners) {
            listener(fault);
          }
        },
      ),
      logUncapturedErrors(device),
    ];
    this.#drawing = new Drawing(this, FULL_SCREEN_WGSL);
  }

  get #mips(): MipGenerator {
    this.#mipGenerator ??= new MipGenerator(this.device);
    return this.#mipGenerator;
  }

  // DrawingHost, ViewHost and TargetHost: what the views, targets and draw path need.

  gpuBufferOf(handle: BufferHandle): GPUBuffer {
    return this.#resources.bufferOf(handle).buffer;
  }

  gpuTextureOf(handle: TextureHandle): GPUTexture {
    return this.#resources.textureOf(handle).texture;
  }

  textureOf(handle: TextureHandle): { readonly texture: GPUTexture; readonly spec: TextureSpec } {
    return this.#resources.textureOf(handle);
  }

  destroyBuffer(handle: BufferHandle): void {
    this.#resources.destroyBuffer(handle);
    this.#drawing.forgetBindGroups();
  }

  destroyTexture(texture: TextureHandle): void {
    this.#resources.destroyTexture(texture);
    this.#drawing.forgetBindGroups();
  }

  sampler(spec: Pick<SamplerSpec, "name" | "filter" | "address">): GPUSampler {
    const key = `${spec.filter}:${spec.address}`;
    let sampler = this.#samplers.get(key);
    if (sampler === undefined) {
      sampler = this.device.createSampler(samplerDescriptor({ ...spec, binding: 0 }));
      this.#samplers.set(key, sampler);
    }
    return sampler;
  }

  wroteByDraw(texture: TextureHandle): void {
    this.#writers.wroteOtherwise(texture);
  }

  generateMips(texture: TextureHandle): void {
    const { texture: gpuTexture, spec } = this.#resources.textureOf(texture);
    const writes = this.#timer.writesFor(`${spec.name} mips`);
    this.#submit(`${spec.name} mips`, (encoder) => {
      this.#mips.encode(encoder, gpuTexture, spec.mips, writes);
    });
    this.#resolveTimes();
  }

  /** Draws `frame` into `output` in the adapter's own passes, timed, and submits them. */
  renderFrame(frame: FrameSubmission, output: FrameOutput): void {
    this.#submit(frame.label, (encoder) => {
      this.#drawing.encodeFrame(encoder, frame, output, (label) => this.#timer.writesFor(label));
    });
    this.#resolveTimes();
  }

  forgetView(view: RenderView): void {
    if (view instanceof WebGpuView) {
      this.#views.delete(view);
    }
  }

  forgetTarget(target: RenderTarget): void {
    if (target instanceof WebGpuRenderTarget) {
      this.#targets.delete(target);
    }
  }

  materialOf(handle: MaterialHandle): MaterialRecord {
    const record = this.#materials.get(handle);
    if (record === undefined) {
      throw new Error(`material ${handle.name} was not made by this engine`);
    }
    return record;
  }

  meshOf(handle: MeshHandle): MeshRecord {
    const record = this.#meshes.get(handle);
    if (record === undefined) {
      throw new Error(`mesh ${handle.name} was not made by this engine`);
    }
    return record;
  }

  postProcessOf(handle: PostProcessHandle): PostProcessRecord {
    const record = this.#postProcesses.get(handle);
    if (record === undefined) {
      throw new Error(`post-process ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /** The kernel behind `handle`, which must be this engine's. */
  kernelOf(handle: ComputeHandle): KernelRecord {
    const record = this.#kernels.get(handle);
    if (record === undefined) {
      throw new Error(`kernel ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /**
   * Probes how the device rounds a colour write into each format it can render (Design note 22).
   *
   * @returns Each format's rounding; a failed probe reads `unknown` and is no fault.
   */
  probeTargetRounding(): Promise<Readonly<Record<ProbedTargetFormat, TargetRounding>>> {
    const host: ProbeHost = {
      device: this.device,
      createTexture: (spec) => this.#resources.createTexture(spec),
      gpuTextureOf: (handle) => this.gpuTextureOf(handle),
      submit: (label, encode) => {
        this.#submit(label, encode);
      },
      readTexture: (handle) => this.readTexture(handle),
      destroyTexture: (handle) => {
        this.destroyTexture(handle);
      },
    };
    return probeTargetRounding(probedFormats(this.capabilities), (format) =>
      drawAndReadProbe(host, format),
    );
  }

  // RenderEngine.

  createView(canvas: HTMLCanvasElement, name: string): RenderView {
    this.#assertLive();
    const view = new WebGpuView(this, canvas, name, navigator.gpu.getPreferredCanvasFormat());
    this.#views.add(view);
    return view;
  }

  createRenderTarget(spec: RenderTargetSpec): RenderTarget {
    this.#assertLive();
    const target = new WebGpuRenderTarget(this, spec);
    this.#targets.add(target);
    return target;
  }

  createMesh(spec: MeshSpec): MeshHandle {
    this.#assertLive();
    const record = new MeshRecord(
      {
        createBuffer: (bufferSpec) => this.#resources.createBuffer(bufferSpec),
        writeBuffer: (handle, offsetBytes, data) => {
          this.#resources.writeBuffer(handle, offsetBytes, data);
        },
        gpuBufferOf: (handle) => this.gpuBufferOf(handle),
      },
      spec,
    );
    const handle: MeshHandle = Object.freeze({ kind: "mesh", name: spec.name });
    this.#meshes.set(handle, record);
    return handle;
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    const { handle, record } = this.#makeMaterial(spec, false);
    void compilationErrorsOf(record.modules)
      .then((errors): void => {
        if (errors.length > 0) {
          record.broken = true;
          this.#reportShaderErrors(`material ${spec.name}`, spec.name, errors);
        }
        return undefined;
      })
      .catch((error: unknown) => {
        this.#reportCompileCheckFailure(`material ${spec.name}`, error);
      });
    return handle;
  }

  /**
   * A material whose shaders are compiled, and whose pipelines for `meshes` into `targets` are
   * made, before it resolves.
   *
   * @remarks
   * A canvas target is the preferred canvas format's sRGB view with depth; a colour target is made
   * both with and without depth, since a target may have either.
   */
  async createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat>,
    meshes: ReadonlyArray<MeshHandle> = [],
  ): Promise<MaterialHandle> {
    const { handle, record } = this.#makeMaterial(spec, true);
    const errors = await compilationErrorsOf(record.modules);
    if (errors.length > 0) {
      record.broken = true;
      this.#reportShaderErrors(`material ${spec.name}`, spec.name, errors);
      throw new Error(`material ${spec.name} failed to compile: ${errors.join("; ")}`);
    }
    const outputs = targets.flatMap((target): Array<readonly [GPUTextureFormat, boolean]> =>
      target === "canvas"
        ? [[srgbViewFormat(navigator.gpu.getPreferredCanvasFormat()), true]]
        : [
            [target, true],
            [target, false],
          ],
    );
    await Promise.all(
      meshes.flatMap((mesh) =>
        outputs.map(([format, hasDepth]) =>
          this.#drawing.prepareMaterialPipeline(record, this.meshOf(mesh), format, hasDepth),
        ),
      ),
    );
    return handle;
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    this.#assertLive();
    const drawLayout = uniformLayout(spec.uniforms);
    assertDrawStruct(`post-process ${spec.name}`, drawLayout, spec.fragmentWgsl);
    const bindings = postProcessBindings(spec);
    const resourceLayout = this.#drawing.resourceLayout(bindings);
    const fragment = this.device.createShaderModule({
      label: spec.name,
      code: spec.fragmentWgsl,
    });
    const record: PostProcessRecord = {
      spec,
      bindings,
      vertex: this.#drawing.fullScreen,
      fragment,
      layout: this.#drawing.pipelineLayout(spec.name, resourceLayout),
      resourceLayout,
      drawLayout,
      pipelines: new Map(),
      broken: false,
    };
    void compilationErrors(fragment)
      .then((errors): void => {
        if (errors.length > 0) {
          record.broken = true;
          this.#reportShaderErrors(`post-process ${spec.name}`, spec.name, errors);
        }
        return undefined;
      })
      .catch((error: unknown) => {
        this.#reportCompileCheckFailure(`post-process ${spec.name}`, error);
      });
    const handle: PostProcessHandle = Object.freeze({ kind: "post-process", name: spec.name });
    this.#postProcesses.set(handle, record);
    return handle;
  }

  createCompute(pair: KernelPair): ComputeHandle {
    this.#assertLive();
    const record = createKernel(this.device, pair, this.capabilities);
    return this.#kernelHandle(pair, record);
  }

  /** A kernel whose pipeline is made by `createComputePipelineAsync`, off the frame path. */
  async createComputeAsync(pair: KernelPair): Promise<ComputeHandle> {
    this.#assertLive();
    const record = await createKernelAsync(this.device, pair, this.capabilities);
    return this.#kernelHandle(pair, record);
  }

  createBuffer(spec: BufferSpec): BufferHandle {
    this.#assertLive();
    return this.#resources.createBuffer(spec);
  }

  createTexture(spec: TextureSpec): TextureHandle {
    this.#assertLive();
    return this.#resources.createTexture(spec);
  }

  createPackedCube(sizePx: number, mips: number, category: MemoryCategory): TextureHandle {
    this.#assertLive();
    return this.#resources.createTexture(packedCubeSpec(sizePx, mips, category));
  }

  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void {
    this.#assertLive();
    this.#resources.writePackedCubeLevel(cube, level, packed);
    this.#writers.wroteOtherwise(cube);
  }

  writePackedCubeLevelFromBuffer(cube: TextureHandle, level: number, packed: BufferHandle): void {
    this.#assertLive();
    this.#submit(`${cube.name} level ${level}`, (encoder) => {
      this.#resources.encodePackedCubeLevelFromBuffer(encoder, cube, level, packed);
    });
    // The cube carries the buffer's writer, so a presentation-only result stays refused.
    this.#writers.copied(packed, cube);
  }

  /**
   * R06's additive point splat into an `rgba32float` bake target.
   *
   * @throws {@link Float32BlendUnavailable} without `float32-blendable`.
   */
  createPointSplat(spec: PointSplatSpec): PointSplatHandle {
    this.#assertLive();
    assertSplatSupported(this.capabilities);
    const vertex = this.device.createShaderModule({
      label: `${spec.name} vertex`,
      code: spec.vertexWgsl,
    });
    const fragment = this.device.createShaderModule({
      label: `${spec.name} fragment`,
      code: spec.fragmentWgsl,
    });
    const pipeline = this.device.createRenderPipeline(
      pointSplatPipelineDescriptor(spec, vertex, fragment),
    );
    let disposed = false;
    return {
      draw: (target: TextureHandle, points: BufferHandle, count: number): void => {
        if (disposed) {
          throw new Error(`splat ${spec.name} is disposed`);
        }
        this.#assertLive();
        const { texture, spec: targetSpec } = this.#resources.textureOf(target);
        const { buffer, spec: pointsSpec } = this.#resources.bufferOf(points);
        assertSplatInputs(spec, targetSpec, pointsSpec);
        const writes = this.#timer.writesFor(spec.name);
        this.#submit(spec.name, (encoder) => {
          encodeSplat(this.device, encoder, pipeline, texture, buffer, count, spec.name, writes);
        });
        this.#writers.wroteOtherwise(target);
        this.#resolveTimes();
      },
      dispose: (): void => {
        disposed = true;
      },
    };
  }

  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
    pass = "compute",
  ): void {
    this.#assertLive();
    const record = this.kernelOf(kernel);
    const uniforms = this.#kernelUniforms.get(record) ?? new Map<string, BufferHandle>();
    this.#kernelUniforms.set(record, uniforms);
    const resources = resolveKernelResources(this.#resources, record, bindings, uniforms);
    const counts: Workgroups =
      "buffer" in workgroups
        ? {
            kind: "indirect",
            buffer: this.gpuBufferOf(workgroups.buffer),
            offsetBytes: workgroups.offsetBytes,
          }
        : { kind: "direct", counts: workgroups };
    const timestampWrites = this.#timer.writesFor(pass);
    this.#submit(pass, (encoder) => {
      encodeDispatch(this.device, encoder, record, resources, counts, {
        label: pass,
        ...(timestampWrites === undefined ? {} : { timestampWrites }),
      });
    });
    for (const [name, handle] of Object.entries(bindings.buffers)) {
      if (record.bindings.get(name)?.writable === true) {
        this.#writers.wroteBy(handle, record.pair);
      }
    }
    for (const { texture } of Object.values(bindings.storage)) {
      this.#writers.wroteBy(texture, record.pair);
    }
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    this.#assertLive();
    this.#resources.writeBuffer(buffer, offsetBytes, data);
    this.#writers.wroteOtherwise(buffer);
  }

  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.#assertLive();
    this.#resources.writeTexture(texture, origin, size, data);
    this.#writers.wroteOtherwise(texture);
  }

  readBuffer(buffer: BufferHandle, access: "cpu" | "tolerance" = "cpu"): Promise<ArrayBuffer> {
    try {
      this.#assertLive();
      this.#writers.assertReadable(buffer, access);
      const { buffer: gpuBuffer, spec } = this.#resources.bufferOf(buffer);
      assertBufferReadable(spec);
      return readGpuBuffer(this.device, gpuBuffer, spec.bytes, (encode) => {
        this.#submit(`${buffer.name} readback`, encode);
      });
    } catch (error: unknown) {
      return Promise.reject(asError(error));
    }
  }

  readTexture(texture: TextureHandle, level = 0, rect?: TexelRect): Promise<ArrayBuffer> {
    try {
      this.#assertLive();
      this.#writers.assertReadable(texture, "cpu");
      const { texture: gpuTexture, spec } = this.#resources.textureOf(texture);
      return readGpuTexture(this.device, gpuTexture, textureRead(spec, level, rect), (encode) => {
        this.#submit(`${texture.name} readback`, encode);
      });
    } catch (error: unknown) {
      return Promise.reject(asError(error));
    }
  }

  onPassTimes(listener: (times: PassTimes) => void): () => void {
    return this.#timer.listen(listener);
  }

  onAllocation(listener: (event: AllocationEvent) => void): () => void {
    this.#allocationListeners.add(listener);
    return () => {
      this.#allocationListeners.delete(listener);
    };
  }

  /** Never fires here: the engine `loadRenderEngine` returns re-creates this one after a loss. */
  onRestored(_listener: () => void): () => void {
    return () => undefined;
  }

  onFault(listener: (fault: GraphicsFault) => void): () => void {
    this.#faultListeners.add(listener);
    // A loss reported before anyone listened (the device lost while the engine was being made) is
    // replayed, so that the engine is never adopted dead.
    const lost = this.#lost;
    if (lost !== null) {
      queueMicrotask(() => {
        if (this.#faultListeners.has(listener)) {
          listener(lost);
        }
      });
    }
    return () => {
      this.#faultListeners.delete(listener);
    };
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    for (const stop of this.#watches) {
      stop();
    }
    for (const view of this.#views) {
      view.dispose();
    }
    this.#views.clear();
    for (const target of this.#targets) {
      target.dispose();
    }
    this.#targets.clear();
    this.#drawing.dispose();
    this.#timer.dispose();
    this.#resources.dispose();
    this.#kernelUniforms.clear();
    this.#samplers.clear();
    this.device.destroy();
    this.#allocationListeners.clear();
    this.#faultListeners.clear();
  }

  #makeMaterial(
    spec: WgslMaterialSpec,
    asyncPipelines: boolean,
  ): { readonly handle: MaterialHandle; readonly record: MaterialRecord } {
    this.#assertLive();
    const drawLayout = uniformLayout([OFFSET_MEMBER, ...spec.uniforms]);
    assertDrawStruct(`material ${spec.name}`, drawLayout, spec.vertexWgsl, spec.fragmentWgsl);
    const bindings = materialBindings(spec);
    const resourceLayout = this.#drawing.resourceLayout(bindings);
    const vertex = this.device.createShaderModule({ label: spec.name, code: spec.vertexWgsl });
    const modules: MaterialModules = {
      vertex,
      fragment:
        spec.fragmentWgsl === spec.vertexWgsl
          ? vertex
          : this.device.createShaderModule({ label: spec.name, code: spec.fragmentWgsl }),
    };
    const record: MaterialRecord = {
      spec,
      bindings,
      modules,
      layout: this.#drawing.pipelineLayout(spec.name, resourceLayout),
      resourceLayout,
      drawLayout,
      asyncPipelines,
      pipelines: new Map(),
      broken: false,
    };
    const handle: MaterialHandle = Object.freeze({ kind: "material", name: spec.name });
    this.#materials.set(handle, record);
    return { handle, record };
  }

  #reportShaderErrors(owner: string, name: string, errors: ReadonlyArray<string>): void {
    if (this.#disposed) {
      return;
    }
    console.error(`${owner} failed to compile; its draws are left out:\n${errors.join("\n")}`);
    this.#status.dispatch({ kind: "shader-refused", effectName: name });
  }

  /** A compile check that could not run (a lost device, say): logged unless the engine is gone. */
  #reportCompileCheckFailure(owner: string, error: unknown): void {
    if (!this.#disposed) {
      console.error(`the compile check of ${owner} failed:`, error);
    }
  }

  #kernelHandle(pair: KernelPair, record: KernelRecord): ComputeHandle {
    const handle: ComputeHandle = Object.freeze({
      kind: "compute",
      name: pair.name,
      path: record.path,
    });
    this.#kernels.set(handle, record);
    return handle;
  }

  /** Encodes and submits one command buffer. */
  #submit(label: string, encode: (encoder: GPUCommandEncoder) => void): void {
    const encoder = this.device.createCommandEncoder({ label });
    encode(encoder);
    this.device.queue.submit([encoder.finish()]);
  }

  /** Resolves the frame's timestamps, when any pass was timed, in a submission of their own. */
  #resolveTimes(): void {
    const encoder = this.device.createCommandEncoder({ label: "pass times" });
    const read = this.#timer.resolve(encoder);
    if (read !== null) {
      this.device.queue.submit([encoder.finish()]);
      read();
    }
  }

  #assertLive(): void {
    if (this.#disposed) {
      throw new Error("the engine has been disposed");
    }
  }
}

/** The compiler's errors for a material's modules, the shared one once. */
async function compilationErrorsOf(modules: MaterialModules): Promise<ReadonlyArray<string>> {
  const stages =
    modules.fragment === modules.vertex ? [modules.vertex] : [modules.vertex, modules.fragment];
  return (await Promise.all(stages.map((module) => compilationErrors(module)))).flat();
}

/**
 * Creates the engine on the vetted adapter: its device, with the features `requiredFeatures` asks
 * for, then the rounding probe (Design note 22) before any view renders.
 */
export const createWebGpuEngine: CreateWebGpuEngine = async (
  outcome: AdapterOutcome & { readonly kind: "adapter" },
  status: GraphicsStatusStore,
  overrides: CapabilityOverrides | undefined,
): Promise<RenderEngine> => {
  const requested = requiredFeatures(outcome.adapter, overrides);
  const device = await outcome.adapter.requestDevice({
    label: "hyperion",
    requiredFeatures: [...requested],
  });
  const notEnabled = featuresNotEnabled(requested, device.features);
  if (notEnabled.length > 0) {
    console.warn(`the device did not enable ${notEnabled.join(", ")}, which it was asked for`);
  }
  const engine = new WebGpuRenderEngine(device, status);
  // The device's features, not the adapter's, so that a withheld feature reads as absent in the
  // status too (Design note 7).
  status.dispatch({ kind: "device-capabilities", capabilities: engine.capabilities });
  // Once a device, before any view renders: a rebuild comes through here too (Design note 22).
  status.dispatch({ kind: "target-rounding", rounding: await engine.probeTargetRounding() });
  return engine;
};
