/**
 * The Babylon.js implementation of the engine-agnostic interface, the one module that
 * `loadEngine.ts` imports dynamically, and so the root of the lazily loaded `babylon` chunk.
 *
 * @remarks
 * `@babylonjs/core` is pinned exactly, at 9.28.0 with no range (R01 Design note 15). The case for
 * Babylon is that its visual changes are logged with a flag that restores the old look, so an
 * upgrade is a deliberate act: read the breaking-changes log, set any restoring flag, and run
 * `just test-render`. The pin's reason lives here because `package.json` holds no comments.
 *
 * Imports go to Babylon's `.pure` modules, with the registrations they need made explicitly in
 * `registrations.ts`, since `Engines/webgpuEngine.js` drags in the audio engine and loaders
 * (Design note 14).
 *
 * The engine is made on the adapter the client vetted (Design note 7), with the fixed options of
 * Design note 11, reversed depth and a right-handed scene (Design note 18). Its capabilities are
 * the device's, so a feature the smoke harness withholds reads as absent.
 */

import { type GlslangOptions, WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";
import type { TwgslOptions } from "@babylonjs/core/Engines/WebGPU/webgpuTintWASM";
import type { AbstractMesh } from "@babylonjs/core/Meshes/abstractMesh.pure";
import { ShaderStore } from "@babylonjs/core/Engines/shaderStore";
import type { Effect } from "@babylonjs/core/Materials/effect.pure";
import { ShaderLanguage } from "@babylonjs/core/Materials/shaderLanguage";
import type { ShaderMaterial } from "@babylonjs/core/Materials/shaderMaterial.pure";
import { Constants } from "@babylonjs/core/Engines/constants";
import { PostProcess } from "@babylonjs/core/PostProcesses/postProcess.pure";
import { ThinTexture } from "@babylonjs/core/Materials/Textures/thinTexture";
import { Scene } from "@babylonjs/core/scene.pure";

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
import { type GraphicsFault, type GraphicsStatusStore, navigatorGpu } from "../status";
import {
  type BufferHandle,
  type ComputeBindings,
  type ComputeHandle,
  type CreateBabylonEngine,
  type DepthPolicy,
  type DrawItem,
  FRAME_UNIFORMS,
  type FrameSubmission,
  POST_PROCESS_INPUTS,
  type IndirectArgs,
  type MaterialHandle,
  type MeshHandle,
  type MeshSpec,
  type PassTimes,
  type PointSplatHandle,
  type PointSplatSpec,
  type PostProcessHandle,
  type RenderEngine,
  type RenderTarget,
  type RenderTargetFormat,
  type RenderTargetSpec,
  type RenderView,
  type TexelRect,
  type TextureHandle,
  type UniformSpec,
  type WgslMaterialSpec,
  type WgslPostProcessSpec,
} from "../types";
import { withHandedAdapter } from "./adapterHandoff";
import { createKernel, encodeDispatch, type KernelRecord, type Workgroups } from "./compute";
import { logUncapturedErrors, watchDeviceLoss } from "./deviceLoss";
import { ExternalStorageBuffer } from "./externalStorageBuffer";
import { resolveKernelResources } from "./kernelResources";
import { engineDevice, flushEngine, setSampledView } from "./internals";
import { createShaderMaterial, setUniform, textureSamplerOf } from "./materials";
import { MeshRecord } from "./meshes";
import { babylonEngineOptions } from "./options";
import { registerBabylonModules } from "./registrations";
import { packedCubeSpec, ResourceRegistry, viewDimensionOf } from "./resources";
import { BabylonView, type ViewHost } from "./view";
import {
  GLSLANG_STUB,
  guardCreateEffect,
  listenForUnhandledRefusals,
  TWGSL_STUB,
} from "./wgslGuard";

/** The error a member not yet built throws, naming the subtask that builds it. */
function notBuilt(member: string, task: string): Error {
  return new Error(`RenderEngine.${member} is built by ${task}`);
}

/** Babylon's objects for one material. */
interface MaterialRecord {
  readonly spec: WgslMaterialSpec;
  readonly material: ShaderMaterial;
  readonly uniforms: ReadonlyMap<string, UniformSpec>;
}

/** What one Babylon mesh draws in the frame being rendered. */
interface DrawBinding {
  readonly frame: FrameSubmission;
  readonly draw: DrawItem;
}

const FRAME_MATRIX_UNIFORMS: ReadonlyArray<UniformSpec> = [
  { name: FRAME_UNIFORMS.viewRotation, type: "mat4x4f" },
  { name: FRAME_UNIFORMS.projection, type: "mat4x4f" },
];
const OFFSET_UNIFORM: UniformSpec = { name: FRAME_UNIFORMS.offsetFromCamera, type: "vec3f" };

let postProcessKeys = 0;

/** The engine over one Babylon `WebGPUEngine`, one device and one right-handed scene. */
export class BabylonRenderEngine implements RenderEngine, ViewHost {
  readonly capabilities: GpuCapabilities;
  readonly depthPolicy: DepthPolicy = "reversed-z-float";
  readonly #engine: WebGPUEngine;
  readonly #device: GPUDevice;
  readonly #scene: Scene;
  readonly #meshes = new WeakMap<MeshHandle, MeshRecord>();
  readonly #meshRecords = new Set<MeshRecord>();
  readonly #views = new Set<BabylonView>();
  readonly #materials = new WeakMap<MaterialHandle, MaterialRecord>();
  readonly #postProcesses = new WeakMap<PostProcessHandle, PostProcess>();
  readonly #kernels = new WeakMap<ComputeHandle, KernelRecord>();
  readonly #drawBindings = new WeakMap<AbstractMesh, DrawBinding>();
  readonly #allocationListeners = new Set<(event: AllocationEvent) => void>();
  readonly #faultListeners = new Set<(fault: GraphicsFault) => void>();
  readonly #passTimeListeners = new Set<(times: PassTimes) => void>();
  readonly #releases: ReadonlyArray<() => void>;
  /** The device's loss and error watches, ended at disposal. */
  readonly #watches: ReadonlyArray<() => void>;
  readonly #resources: ResourceRegistry;
  /** Each sampled texture's Babylon wrapper, made when it is first bound. */
  readonly #sampled = new Map<TextureHandle, ThinTexture>();
  /** Each storage buffer's Babylon wrapper, made when it is first bound. */
  readonly #storage = new Map<BufferHandle, ExternalStorageBuffer>();
  /** Each kernel's uniform buffers, by uniform name. */
  readonly #kernelUniforms = new Map<KernelRecord, Map<string, BufferHandle>>();
  /** The device's loss, once reported. */
  #lost: GraphicsFault | null = null;
  #disposed = false;

  /**
   * @param releases - Run once at disposal: the guard's listener, and whatever else the creation
   * started.
   */
  constructor(engine: WebGPUEngine, releases: ReadonlyArray<() => void> = []) {
    this.#engine = engine;
    this.#releases = releases;
    this.#device = engineDevice(engine);
    this.capabilities = deviceCapabilities(this.#device);
    this.#resources = new ResourceRegistry(this.#device, (event) => {
      for (const listener of this.#allocationListeners) {
        listener(event);
      }
    });
    const scene = new Scene(engine, { useFloatingOrigin: false, virtual: true });
    scene.useRightHandedSystem = true;
    scene.autoClear = false;
    scene.skipPointerMovePicking = true;
    this.#scene = scene;
    this.#watches = [
      watchDeviceLoss(
        this.#device,
        () => this.#disposed,
        (fault) => {
          this.#lost = fault;
          for (const listener of this.#faultListeners) {
            listener(fault);
          }
        },
      ),
      logUncapturedErrors(this.#device),
    ];
  }

  /** The Babylon engine, for the adapter's own modules. */
  get babylonEngine(): WebGPUEngine {
    return this.#engine;
  }

  /** The scene every view and target draws from. */
  get scene(): Scene {
    return this.#scene;
  }

  /** The engine's one device. */
  get device(): GPUDevice {
    return this.#device;
  }

  /**
   * Runs `render` as one Babylon frame and submits it.
   *
   * @remarks
   * Each view's render is its own frame: a canvas texture expires once the task yields, so its
   * frame is submitted before `render` returns.
   */
  inFrame(render: () => void): void {
    this.#engine.beginFrame();
    try {
      this.#scene.resetCachedMaterial();
      // Babylon resets these at the start of a scene's own render, not of a target's; a
      // post-process leaves depth writes off (`PostProcess.apply`), which would carry into the
      // next frame's draws.
      this.#engine.setDepthBuffer(true);
      this.#engine.setDepthWrite(true);
      this.#engine.setColorWrite(true);
      render();
    } finally {
      this.#engine.endFrame();
    }
  }

  /**
   * The Babylon post-processes of `frame`, in order, as the chain the next render runs.
   *
   * @remarks
   * The caller renders the frame before it asks for another's chain.
   */
  postProcessesFor(frame: FrameSubmission): ReadonlyArray<PostProcess> {
    return frame.postProcesses.map((handle) => this.postProcessOf(handle));
  }

  forgetView(view: RenderView): void {
    if (view instanceof BabylonView) {
      this.#views.delete(view);
    }
  }

  createView(canvas: HTMLCanvasElement, name: string): RenderView {
    this.#assertLive();
    const view = new BabylonView(this, canvas, name, navigator.gpu.getPreferredCanvasFormat());
    this.#views.add(view);
    return view;
  }

  createRenderTarget(_spec: RenderTargetSpec): RenderTarget {
    throw notBuilt("createRenderTarget", "R01.T8.f");
  }

  createMesh(spec: MeshSpec): MeshHandle {
    this.#assertLive();
    const record = new MeshRecord(this.#engine, this.#scene, spec);
    const handle: MeshHandle = Object.freeze({ kind: "mesh", name: spec.name });
    this.#meshes.set(handle, record);
    this.#meshRecords.add(record);
    return handle;
  }

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    this.#assertLive();
    if (spec.blend === "premultiplied") {
      // Babylon's mode 7 changes the destination alpha until its factors are overridden.
      throw notBuilt("createMaterial with premultiplied blending", "R01.T8.i");
    }
    const material = createShaderMaterial(this.#scene, spec);
    const uniforms = new Map<string, UniformSpec>(
      [...FRAME_MATRIX_UNIFORMS, OFFSET_UNIFORM, ...spec.uniforms].map((uniform) => [
        uniform.name,
        uniform,
      ]),
    );
    // Babylon keeps a material's effect on each sub-mesh, per render pass, and binds it before
    // this runs, so the draw's own bindings go on that effect.
    material.onBindObservable.add((mesh) => {
      const binding = this.#drawBindings.get(mesh);
      const effect = mesh.subMeshes.at(0)?.effect ?? null;
      if (binding !== undefined && effect !== null) {
        this.#bindDraw(effect, binding, uniforms);
      }
    });
    const handle: MaterialHandle = Object.freeze({ kind: "material", name: spec.name });
    this.#materials.set(handle, { spec, material, uniforms });
    return handle;
  }

  createMaterialAsync(
    _spec: WgslMaterialSpec,
    _targets: ReadonlyArray<RenderTargetFormat>,
  ): Promise<MaterialHandle> {
    return Promise.reject(notBuilt("createMaterialAsync", "R01.T8.g"));
  }

  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle {
    this.#assertLive();
    if ((spec.inputs ?? []).includes("depth")) {
      // Bound to the chain's first input, Babylon's scene depth read zero (T8.c as built).
      throw notBuilt("createPostProcess with a depth input", "R01.T8.f");
    }
    postProcessKeys += 1;
    const key = `hyperionPostProcess${postProcessKeys}`;
    ShaderStore.ShadersStoreWGSL[`${key}FragmentShader`] = spec.fragmentWgsl;
    const samplers = (spec.samplers ?? []).map(
      (sampler) => [sampler.name, textureSamplerOf(sampler)] as const,
    );
    const postProcess = new PostProcess(spec.name, key, {
      uniforms: spec.uniforms.map(({ name }) => name),
      samplers: postProcessSamplers(spec),
      size: 1,
      camera: null,
      engine: this.#engine,
      reusable: false,
      textureType: Constants.TEXTURETYPE_HALF_FLOAT,
      shaderLanguage: ShaderLanguage.WGSL,
    });
    // A post-process's samplers are bound each time it runs, as a material's are at its bind.
    postProcess.onApplyObservable.add(() => {
      for (const [name, sampler] of samplers) {
        this.#engine.setTextureSampler(name, sampler);
      }
    });
    const handle: PostProcessHandle = Object.freeze({ kind: "post-process", name: spec.name });
    this.#postProcesses.set(handle, postProcess);
    return handle;
  }

  createCompute(pair: KernelPair): ComputeHandle {
    this.#assertLive();
    const record = createKernel(this.#device, pair, this.capabilities);
    const handle: ComputeHandle = Object.freeze({
      kind: "compute",
      name: pair.name,
      path: record.path,
    });
    this.#kernels.set(handle, record);
    return handle;
  }

  createComputeAsync(_pair: KernelPair): Promise<ComputeHandle> {
    return Promise.reject(notBuilt("createComputeAsync", "R01.T8.g"));
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
  }

  writePackedCubeLevelFromBuffer(cube: TextureHandle, level: number, packed: BufferHandle): void {
    this.#assertLive();
    this.#submit(`${cube.name} level ${level}`, (encoder) => {
      this.#resources.encodePackedCubeLevelFromBuffer(encoder, cube, level, packed);
    });
  }

  createPointSplat(_spec: PointSplatSpec): PointSplatHandle {
    throw notBuilt("createPointSplat", "R01.T8.h");
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
            buffer: this.#resources.bufferOf(workgroups.buffer).buffer,
            offsetBytes: workgroups.offsetBytes,
          }
        : { kind: "direct", counts: workgroups };
    this.#submit(pass, (encoder) => {
      encodeDispatch(this.#device, encoder, record, resources, counts, { label: pass });
    });
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    this.#assertLive();
    this.#resources.writeBuffer(buffer, offsetBytes, data);
  }

  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.#assertLive();
    this.#resources.writeTexture(texture, origin, size, data);
  }

  readBuffer(_buffer: BufferHandle, _access?: "cpu" | "tolerance"): Promise<ArrayBuffer> {
    return Promise.reject(notBuilt("readBuffer", "R01.T8.f"));
  }

  readTexture(_texture: TextureHandle, _level?: number, _rect?: TexelRect): Promise<ArrayBuffer> {
    return Promise.reject(notBuilt("readTexture", "R01.T8.f"));
  }

  onPassTimes(listener: (times: PassTimes) => void): () => void {
    this.#passTimeListeners.add(listener);
    return () => {
      this.#passTimeListeners.delete(listener);
    };
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
    for (const record of this.#meshRecords) {
      record.dispose();
    }
    this.#meshRecords.clear();
    this.#scene.dispose();
    this.#resources.dispose();
    this.#sampled.clear();
    this.#storage.clear();
    this.#kernelUniforms.clear();
    this.#engine.dispose();
    for (const release of this.#releases) {
      release();
    }
    this.#allocationListeners.clear();
    this.#faultListeners.clear();
    this.#passTimeListeners.clear();
  }

  /** The material record behind `handle`, which must be this engine's. */
  materialOf(handle: MaterialHandle): MaterialRecord {
    const record = this.#materials.get(handle);
    if (record === undefined) {
      throw new Error(`material ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /** The mesh record behind `handle`, which must be this engine's. */
  meshOf(handle: MeshHandle): MeshRecord {
    const record = this.#meshes.get(handle);
    if (record === undefined) {
      throw new Error(`mesh ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /** The Babylon post-process behind `handle`, which must be this engine's. */
  postProcessOf(handle: PostProcessHandle): PostProcess {
    const postProcess = this.#postProcesses.get(handle);
    if (postProcess === undefined) {
      throw new Error(`post-process ${handle.name} was not made by this engine`);
    }
    return postProcess;
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
   * The Babylon meshes that draw `frame`, each bound to its draw, in the submission's order.
   *
   * @remarks
   * The transparent queue orders by `alphaIndex` before distance, and every mesh sits at Babylon's
   * origin, so each mesh's `alphaIndex` is its draw's place in the submission. A pooled mesh keeps
   * only its latest draw, so the caller renders the frame before it asks for another's meshes. A
   * draw of no instances is left out.
   */
  meshesFor(frame: FrameSubmission): AbstractMesh[] {
    const uses = new Map<MeshRecord, Map<ShaderMaterial, number>>();
    const meshes: AbstractMesh[] = [];
    frame.draws.forEach((draw, index) => {
      const instances = draw.instanceCount ?? 1;
      if (instances === 0) {
        return;
      }
      const record = this.meshOf(draw.mesh);
      const { material } = this.materialOf(draw.material);
      const byMaterial = uses.get(record) ?? new Map<ShaderMaterial, number>();
      uses.set(record, byMaterial);
      const use = byMaterial.get(material) ?? 0;
      byMaterial.set(material, use + 1);
      const mesh = record.meshFor(material, use);
      mesh.forcedInstanceCount = instances;
      mesh.alphaIndex = index;
      this.#drawBindings.set(mesh, { frame, draw });
      meshes.push(mesh);
    });
    return meshes;
  }

  #bindDraw(
    effect: Effect,
    { frame, draw }: DrawBinding,
    uniforms: ReadonlyMap<string, UniformSpec>,
  ): void {
    const [viewRotation, projection] = FRAME_MATRIX_UNIFORMS;
    if (viewRotation !== undefined && projection !== undefined) {
      setUniform(effect, viewRotation, frame.viewRotation);
      setUniform(effect, projection, frame.projection);
    }
    setUniform(effect, OFFSET_UNIFORM, draw.offsetFromCameraM);
    for (const [name, texture] of Object.entries(draw.textures)) {
      effect.setTexture(name, this.#sampledTexture(texture));
    }
    for (const [name, buffer] of Object.entries(draw.storageBuffers ?? {})) {
      this.#engine.setStorageBuffer(name, this.#storageBuffer(buffer));
    }
    for (const [name, value] of Object.entries(draw.uniforms)) {
      const uniform = uniforms.get(name);
      if (uniform === undefined) {
        throw new Error(`a draw sets uniform ${name}, which its material does not declare`);
      }
      setUniform(effect, uniform, value);
    }
  }

  /**
   * Encodes work of the adapter's own and submits it after what Babylon has recorded so far, so
   * that it runs in the order it was asked for.
   */
  #submit(label: string, encode: (encoder: GPUCommandEncoder) => void): void {
    flushEngine(this.#engine);
    const encoder = this.#device.createCommandEncoder({ label });
    encode(encoder);
    this.#device.queue.submit([encoder.finish()]);
  }

  /** A texture wrapped for Babylon to sample, through a view of all its mips. */
  #sampledTexture(handle: TextureHandle): ThinTexture {
    const existing = this.#sampled.get(handle);
    if (existing !== undefined) {
      return existing;
    }
    const { texture, spec } = this.#resources.textureOf(handle);
    const internal = this.#engine.wrapWebGPUTexture(texture);
    internal.isCube = spec.dimension === "cube";
    internal.is3D = spec.dimension === "3d";
    setSampledView(internal, {
      label: spec.name,
      dimension: viewDimensionOf(spec),
      mipLevelCount: spec.mips,
    });
    const wrapped = new ThinTexture(internal);
    this.#sampled.set(handle, wrapped);
    return wrapped;
  }

  /** A buffer wrapped as Babylon's storage buffer. */
  #storageBuffer(handle: BufferHandle): ExternalStorageBuffer {
    const existing = this.#storage.get(handle);
    if (existing !== undefined) {
      return existing;
    }
    const { buffer } = this.#resources.bufferOf(handle);
    const wrapped = new ExternalStorageBuffer(this.#engine, buffer, handle.bytes);
    this.#storage.set(handle, wrapped);
    return wrapped;
  }

  #assertLive(): void {
    if (this.#disposed) {
      throw new Error("the engine has been disposed");
    }
  }
}

/**
 * The textures a post-process's effect binds besides Babylon's own input: its declared inputs'.
 *
 * @remarks
 * Babylon adds `textureSampler`, the `hdr-colour` input, to every post-process itself.
 */
export function postProcessSamplers(spec: WgslPostProcessSpec): string[] {
  return (spec.inputs ?? [])
    .map((input) => POST_PROCESS_INPUTS[input])
    .filter((name) => name !== POST_PROCESS_INPUTS["hdr-colour"]);
}

/**
 * Creates the engine on the vetted adapter.
 *
 * @throws Error when `navigator.gpu` is absent, which a vetted adapter rules out.
 */
export const createBabylonEngine: CreateBabylonEngine = async (
  outcome: AdapterOutcome & { readonly kind: "adapter" },
  status: GraphicsStatusStore,
  overrides: CapabilityOverrides | undefined,
): Promise<RenderEngine> => {
  const gpu = navigatorGpu();
  if (gpu === undefined) {
    throw new Error("an adapter was vetted, yet navigator.gpu is absent");
  }
  registerBabylonModules();
  const requested = requiredFeatures(outcome.adapter, overrides);
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const engine = new WebGPUEngine(canvas, babylonEngineOptions(requested));
  let renderEngine: BabylonRenderEngine;
  const stopListening = listenForUnhandledRefusals(window, status);
  try {
    const notEnabled = await initialiseOnAdapter(engine, gpu, outcome.adapter, requested);
    if (notEnabled.length > 0) {
      console.warn(`the device did not enable ${notEnabled.join(", ")}, which it was asked for`);
    }
    engine.useReverseDepthBuffer = true;
    guardCreateEffect(engine, status);
    renderEngine = new BabylonRenderEngine(engine, [stopListening]);
  } catch (error: unknown) {
    stopListening();
    engine.dispose();
    throw error;
  }
  // The device's features, not the adapter's, so that a withheld feature reads as absent in the
  // status too (Design note 7).
  status.dispatch({ kind: "device-capabilities", capabilities: renderEngine.capabilities });
  return renderEngine;
};

/** What of a Babylon engine its initialisation needs. */
export interface InitialisableEngine {
  initAsync(glslangOptions: GlslangOptions, twgslOptions: TwgslOptions): Promise<void>;
  readonly enabledExtensions: ReadonlyArray<string>;
}

/**
 * Initialises `engine` on `adapter`, handed to it through `gpu`.
 *
 * @returns The requested features the device did not enable, which Babylon drops without a word.
 */
export async function initialiseOnAdapter(
  engine: InitialisableEngine,
  gpu: GPU,
  adapter: GPUAdapter,
  requested: ReadonlyArray<GPUFeatureName>,
): Promise<ReadonlyArray<GPUFeatureName>> {
  // The stub compilers keep Babylon from ever fetching glslang or twgsl (Design note 12).
  await withHandedAdapter(gpu, adapter, () => engine.initAsync(GLSLANG_STUB, TWGSL_STUB));
  return featuresNotEnabled(requested, new Set(engine.enabledExtensions));
}
