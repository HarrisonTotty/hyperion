/**
 * The engine-agnostic interface: the only rendering types the rest of the renderer sees.
 *
 * @remarks
 * No type from `@babylonjs/*` appears here or anywhere outside `view/engine/babylon/`, which the
 * boundary test enforces (R01 Design note 1). One `GPUDevice` drives any number of views, each
 * through its own canvas context at its own size (Design note 13). Every material is WGSL (Design
 * note 12); depth is reversed and `depth32float` (Design note 11).
 */

import type { KernelPair } from "./kernels";
import type { AllocationEvent, BufferSpec, MemoryCategory, TextureSpec } from "./memory";
import type { AdapterOutcome, CapabilityOverrides, GpuCapabilities } from "./platform";
import type { GpuTimer, GraphicsFault, GraphicsStatusStore } from "./status";

/** The one depth convention: `depth32float`, cleared to 0, compared greater-or-equal. */
export type DepthPolicy = "reversed-z-float";

/**
 * The colour formats a render target may have. `rgba32float` is kept out: it would double the
 * bandwidth of a bandwidth-bound GPU for no visible gain (the brainstorm's Decision "Light").
 */
export type ColourTargetFormat = "rgba16float" | "rg11b10ufloat" | "rgba8unorm";

/** A size in device pixels. */
export interface ViewSize {
  readonly widthPx: number;
  readonly heightPx: number;
}

/** A mesh the engine owns. */
export interface MeshHandle {
  readonly kind: "mesh";
  readonly name: string;
}

/** A material the engine owns. */
export interface MaterialHandle {
  readonly kind: "material";
  readonly name: string;
}

/** A post-process the engine owns. */
export interface PostProcessHandle {
  readonly kind: "post-process";
  readonly name: string;
}

/** A compute kernel the engine owns, with the variant it chose. */
export interface ComputeHandle {
  readonly kind: "compute";
  readonly name: string;
  readonly path: "reference" | "subgroup";
}

/** A GPU buffer the engine owns. */
export interface BufferHandle {
  readonly kind: "buffer";
  readonly name: string;
  readonly bytes: number;
}

/** A GPU texture the engine owns. */
export interface TextureHandle {
  readonly kind: "texture";
  readonly name: string;
}

/** A render target's colour format, or a view's canvas. */
export type RenderTargetFormat = ColourTargetFormat | "canvas";

/** An offscreen colour target to create. */
export interface RenderTargetSpec {
  readonly name: string;
  readonly size: ViewSize;
  readonly format: ColourTargetFormat;
  /** Sampled mips, generated after rendering. */
  readonly mips: number;
  /** Whether it has a `depth32float` of its own, with `COPY_SRC` and `TEXTURE_BINDING`. */
  readonly depth: boolean;
  /** `render-targets` unless a later plan names another. */
  readonly category: MemoryCategory;
}

/** An offscreen target: drawn like a view, sampled as a texture by later passes. */
export interface RenderTarget {
  readonly name: string;
  readonly colour: TextureHandle;
  readonly depth: TextureHandle | null;
  resize(size: ViewSize): void;
  /** Renders into level 0; a chain of levels is one target per level. */
  render(frame: FrameSubmission): void;
  dispose(): void;
}

/** The GPU-side arguments of an indirect draw or dispatch, in WebGPU's layout. */
export interface IndirectArgs {
  /** Created with `GPUBufferUsage.INDIRECT`. */
  readonly buffer: BufferHandle;
  readonly offsetBytes: number;
}

/** One frame's GPU time per labelled pass. */
export interface PassTimes {
  readonly frame: number;
  readonly timer: GpuTimer;
  readonly passes: ReadonlyArray<{
    readonly label: string;
    readonly ns: number;
    /** Measured around a Babylon-encoded pass, queue gaps included: an upper bound. */
    readonly bracketed: boolean;
  }>;
}

/** What a dispatch binds, by the names its WGSL declares. */
export interface ComputeBindings {
  readonly uniforms: Readonly<Record<string, Float32Array | Uint32Array>>;
  /** Storage buffers, read or read-write. */
  readonly buffers: Readonly<Record<string, BufferHandle>>;
  readonly sampled: Readonly<Record<string, TextureHandle>>;
  readonly storage: Readonly<
    Record<string, { readonly texture: TextureHandle; readonly level: number }>
  >;
}

/** A vertex attribute's data and its components per vertex. */
export interface VertexAttribute {
  readonly data: Float32Array;
  readonly size: 1 | 2 | 3 | 4;
}

/** A mesh to create. */
export interface MeshSpec {
  readonly name: string;
  /** Metres, relative to the draw's offset. */
  readonly positions: Float32Array;
  readonly indices: Uint32Array | null;
  readonly topology: "triangle-list" | "line-list" | "point-list";
  readonly attributes: Readonly<Record<string, VertexAttribute>>;
  /** Per-instance attributes, stepped once per instance (R02's segment instances). */
  readonly instanceAttributes?: Readonly<Record<string, VertexAttribute>>;
}

/** A storage buffer a material reads, read-only in the vertex and fragment stages. */
export interface StorageBufferSpec {
  readonly name: string;
  readonly binding: number;
}

/** An extra input of a post-process: the view's reversed-Z depth or its HDR colour. */
export type PostProcessInput = "depth" | "hdr-colour";

/** An additive `point-list` pass into a 2D `rgba32float` bake target (R06's sky splat). */
export interface PointSplatSpec {
  readonly name: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  /** A bake scratch, never a frame's colour target (R01 Design note 21). */
  readonly format: "rgba32float";
  readonly blend: "additive";
}

/** A point splat the engine owns. */
export interface PointSplatHandle {
  draw(target: TextureHandle, points: BufferHandle, count: number): void;
  dispose(): void;
}

/** A uniform a material or post-process declares. */
export interface UniformSpec {
  readonly name: string;
  readonly type: "f32" | "vec2f" | "vec3f" | "vec4f" | "mat4x4f" | "u32";
}

/** A sampler a material or post-process declares. */
export interface SamplerSpec {
  readonly name: string;
  readonly filter: "nearest" | "linear";
  readonly address: "clamp-to-edge" | "repeat";
}

/** One view: a canvas with its own context and depth, drawn by the engine's one device. */
export interface RenderView {
  readonly name: string;
  /** Resizes this view's attachments only. */
  resize(size: ViewSize): void;
  render(frame: FrameSubmission): void;
  /** The harness's copy of the canvas texture, by `copyTextureToBuffer`. */
  readBack(): Promise<Float32Array | Uint8Array>;
  dispose(): void;
}

/** One frame of one view or target. */
export interface FrameSubmission {
  /** The pass's label in {@link PassTimes}, stable across frames (R12 keys its records on it). */
  readonly label: string;
  /** 4 × 4, translation zero, right-handed (R02 fills it). */
  readonly viewRotation: Float32Array;
  /** 4 × 4, reversed-Z, WebGPU clip space; passed to the shaders unchanged. */
  readonly projection: Float32Array;
  readonly draws: ReadonlyArray<DrawItem>;
  readonly postProcesses: ReadonlyArray<PostProcessHandle>;
}

/** One draw of a mesh with a material. */
export interface DrawItem {
  readonly mesh: MeshHandle;
  readonly material: MaterialHandle;
  /** Metres, differenced in `f64` on the CPU and narrowed (R02). */
  readonly offsetFromCameraM: Float32Array;
  readonly uniforms: Readonly<Record<string, Float32Array>>;
  /** Per-draw sampled textures; a `RenderTarget.depth` binds as `texture_depth_2d` (R05). */
  readonly textures: Readonly<Record<string, TextureHandle>>;
  /** Default 1; the index reaches WGSL as `@builtin(instance_index)` (R02, R05). */
  readonly instanceCount?: number;
  /** Buffers bound to the material's `storageBuffers`, by name (R05). */
  readonly storageBuffers?: Readonly<Record<string, BufferHandle>>;
  /** Instance and vertex counts written on the GPU (R05's and R11's culling); Design note 19. */
  readonly indirect?: IndirectArgs;
}

/** A WGSL material to create. */
export interface WgslMaterialSpec {
  readonly name: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  readonly uniforms: ReadonlyArray<UniformSpec>;
  readonly samplers: ReadonlyArray<SamplerSpec>;
  /** Places the draw in the transparent queue, nothing more. */
  readonly transparent: boolean;
  readonly cullMode: "none" | "back";
  /** `false` for R02's lines and sprites. */
  readonly depthWrite: boolean;
  /** `false` for R02's depth-only occluders. */
  readonly colourWrites: boolean;
  /**
   * Every mode keeps the destination alpha, R07's meter class (R01 Design note 21); `additive` is
   * R02's sprites, in linear light.
   */
  readonly blend: "none" | "additive" | "premultiplied";
  readonly storageBuffers?: ReadonlyArray<StorageBufferSpec>;
  /**
   * Positive is away from the camera, whatever the depth direction (R02's hull occluder faces;
   * never set on a line pass).
   */
  readonly depthBiasAway?: { readonly constant: number; readonly slopeScale: number };
}

/** A WGSL post-process to create. */
export interface WgslPostProcessSpec {
  readonly name: string;
  readonly fragmentWgsl: string;
  readonly uniforms: ReadonlyArray<UniformSpec>;
  /** Extra inputs: the view's reversed-Z depth and its HDR colour (R05's aerial perspective). */
  readonly inputs?: ReadonlyArray<PostProcessInput>;
  readonly samplers?: ReadonlyArray<SamplerSpec>;
}

/** A texture level's region, in texels. */
export interface TexelRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** The engine: one device, any number of views, and the one path by which GPU memory is made. */
export interface RenderEngine {
  /** The device's features, after the harness's overrides. */
  readonly capabilities: GpuCapabilities;
  readonly depthPolicy: DepthPolicy;
  createView(canvas: HTMLCanvasElement, name: string): RenderView;
  /** An offscreen colour target with its own depth: HDR, bloom chains, stills, the harness. */
  createRenderTarget(spec: RenderTargetSpec): RenderTarget;
  createMesh(spec: MeshSpec): MeshHandle;
  createMaterial(spec: WgslMaterialSpec): MaterialHandle;
  /** Resolves once every pipeline the material needs is compiled; no frame waits on a compile. */
  createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat>,
  ): Promise<MaterialHandle>;
  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle;
  /** The variant is chosen by `selectKernel` against {@link RenderEngine.capabilities}. */
  createCompute(pair: KernelPair): ComputeHandle;
  createComputeAsync(pair: KernelPair): Promise<ComputeHandle>;
  /** Every GPU buffer and texture is created here, and nowhere else, with its memory category. */
  createBuffer(spec: BufferSpec): BufferHandle;
  /** 2D, 3D or a cube; sampled and/or storage. */
  createTexture(spec: TextureSpec): TextureHandle;
  /** R06's packed star cube: `rgb9e5ufloat`, every mip written by `copyBufferToTexture`. */
  createPackedCube(sizePx: number, mips: number, category: MemoryCategory): TextureHandle;
  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void;
  /** The same level written from a GPU buffer a kernel filled, with no readback (R06's bake). */
  writePackedCubeLevelFromBuffer(cube: TextureHandle, level: number, packed: BufferHandle): void;
  /**
   * An additive `point-list` pass into a 2D `rgba32float` bake target (R06's sky splat).
   *
   * @throws Float32BlendUnavailable without `float32-blendable`; the caller falls back to a
   * compute splat (R01 Design note 21).
   */
  createPointSplat(spec: PointSplatSpec): PointSplatHandle;
  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    /** Workgroup counts, or GPU-written ones. */
    workgroups: readonly [number, number, number] | IndirectArgs,
    /** The timed pass it belongs to; `compute` when absent. */
    pass?: string,
  ): void;
  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void;
  /** Each write raises an `uploaded` event. */
  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void;
  /**
   * CPU readback.
   *
   * @param access - `tolerance` lifts the refusal for the smoke harness's tolerance checks; the
   * boundary test fails on it anywhere else.
   * @throws PresentationOnlyReadback for a buffer last written by a `presentation-only` kernel.
   */
  readBuffer(buffer: BufferHandle, access?: "cpu" | "tolerance"): Promise<ArrayBuffer>;
  /**
   * CPU readback of a texture level or a region of it, colour or depth.
   *
   * @throws PresentationOnlyReadback as {@link RenderEngine.readBuffer} does.
   */
  readTexture(texture: TextureHandle, level?: number, rect?: TexelRect): Promise<ArrayBuffer>;
  /** Per-pass GPU time for each frame, once its query set resolves; silent without the feature. */
  onPassTimes(listener: (times: PassTimes) => void): () => void;
  /** Every creation, destruction and upload, with its byte size and category. */
  onAllocation(listener: (event: AllocationEvent) => void): () => void;
  onFault(listener: (fault: GraphicsFault) => void): () => void;
  dispose(): void;
}

/** The Babylon module's one export, `view/engine/babylon/engine.ts`. */
export type CreateBabylonEngine = (
  outcome: AdapterOutcome & { readonly kind: "adapter" },
  status: GraphicsStatusStore,
  overrides: CapabilityOverrides | undefined,
) => Promise<RenderEngine>;

/** How `loadRenderEngine` loads the engine. */
export interface LoadEngineOptions {
  /** The harness's withheld-feature runs. */
  readonly overrides?: CapabilityOverrides;
  /** The dynamic import, injectable so that a test fakes it without `vi.mock` of our module. */
  readonly importEngine?: () => Promise<{ readonly createBabylonEngine: CreateBabylonEngine }>;
}

/** Thrown by `createPointSplat` when the device lacks `float32-blendable` (Design note 21). */
export class Float32BlendUnavailable extends Error {
  constructor() {
    super("the device lacks float32-blendable, which an additive rgba32float splat needs");
    this.name = "Float32BlendUnavailable";
  }
}

/** Thrown when a draw samples the depth of the target it renders into (Design note 21). */
export class DepthSelfSample extends Error {
  readonly targetName: string;
  readonly materialName: string;

  constructor(targetName: string, materialName: string) {
    super(`material ${materialName} samples the depth of ${targetName}, which it renders into`);
    this.name = "DepthSelfSample";
    this.targetName = targetName;
    this.materialName = materialName;
  }
}

/** Thrown by `readBuffer` and `readTexture` for a presentation-only result (Design note 16). */
export class PresentationOnlyReadback extends Error {
  readonly kernelName: string;

  constructor(kernelName: string) {
    super(`the result of presentation-only kernel ${kernelName} may not be read back`);
    this.name = "PresentationOnlyReadback";
    this.kernelName = kernelName;
  }
}
