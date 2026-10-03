/**
 * The engine-agnostic interface: the only rendering types the rest of the renderer sees.
 *
 * @remarks
 * Only `view/engine/webgpu/` allocates on the device, which the boundary test enforces (R01
 * Design note 1). One `GPUDevice` drives any number of views, each through its own canvas context
 * at its own size. Every material is standard WGSL under {@link BIND_GROUPS}' convention (Design
 * note 23); depth is reversed and `depth32float`.
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

/**
 * A render target's colour format, or a view's canvas: `canvas` through its `-srgb` view, and
 * `canvas-in-pass` through its own format, for a submission with `encoding` `"in-pass"` (R07.T15).
 */
export type RenderTargetFormat = ColourTargetFormat | "canvas" | "canvas-in-pass";

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
  /**
   * Renders into level 0; a chain of levels is one target per level.
   *
   * @throws {@link DepthSelfSample} when a draw samples the target's own depth, and
   * {@link ColourSelfSample} when the pass that writes its colour (the draws', or the last
   * post-process's) samples it.
   */
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
    /** Always `false` now: every pass is the adapter's own and carries its timestamps. */
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

/**
 * A storage buffer a material reads, read-only in the vertex and fragment stages, at
 * `@group(2) @binding(binding)` (Design note 23).
 */
export interface StorageBufferSpec {
  readonly name: string;
  readonly binding: number;
}

/**
 * A sampled texture a material or post-process declares at `@group(2) @binding(binding)`, bound
 * per draw from `DrawItem.textures` (or `PostProcessItem.textures`) by `name` (Design note 23).
 */
export interface TextureBindingSpec {
  readonly name: string;
  readonly binding: number;
  /**
   * `float` by default; `depth` for a `RenderTarget.depth` read as `texture_depth_2d` with
   * `textureLoad` (R05); `unfilterable-float` for an `rgba32float` read without
   * `float32-filterable`.
   */
  readonly sampleType?: "float" | "unfilterable-float" | "depth" | "uint" | "sint";
  /**
   * `2d` by default; `2d-array` for layers, `3d` or `cube`. It must be the bound texture's own
   * dimension, which the adapter checks at each bind, except that a `2d-array` binding also takes
   * a single-layer 2D texture as a one-layer array (R05.T11.a).
   */
  readonly viewDimension?: "2d" | "2d-array" | "3d" | "cube";
}

/** The pass's input of a post-process: the colour the chain has drawn so far. */
export type PostProcessInput = "hdr-colour";

/**
 * The bind groups of the shader convention (Design note 23): every material and post-process is
 * standard WGSL with these groups, under explicit pipeline layouts.
 *
 * @remarks
 * - `@group(0) @binding(0) var<uniform> frame : Frame;`, the pass's {@link FrameSubmission}
 *   matrices and viewport, declared once in `view/shaders/frame.wgsl`.
 * - `@group(1) @binding(0) var<uniform> draw : Draw;`, the draw's uniforms: for a material,
 *   `offsetFromCameraM : vec3f` first, then its {@link UniformSpec}s in order; for a post-process,
 *   its uniforms alone. Laid out by WGSL's uniform rules, from a per-frame ring with dynamic
 *   offsets. A source that declares `struct Draw` is checked against that layout at creation.
 * - `@group(2)`: the textures, samplers and storage buffers at the bindings their specifications
 *   declare. A post-process's input colour is fixed at {@link POST_PROCESS_BINDINGS}.
 *
 * Vertex attributes are `@location(n)` in the mesh's order: `position` 0, then
 * `MeshSpec.attributes` in insertion order, then `instanceAttributes`. Entry points are
 * `vertexMain` and `fragmentMain`.
 */
export const BIND_GROUPS = { frame: 0, draw: 1, resources: 2 } as const;

/**
 * The fixed `@group(2)` bindings of a post-process's input: the colour drawn so far as
 * `texture_2d<f32>`, and a linear, clamped sampler for it. A specification's own textures and
 * samplers take other bindings.
 */
export const POST_PROCESS_BINDINGS = { "hdr-colour": 0, "hdr-colour-sampler": 1 } as const;

/**
 * The full-screen vertex stage's output a post-process's `fragmentMain` reads: `@location(0) uv`,
 * (0, 0) at the top left of the output.
 */
export const POST_PROCESS_VARYING = "uv";

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
  /** `linear` binds as a filtering sampler, `nearest` as a non-filtering one. */
  readonly filter: "nearest" | "linear";
  readonly address: "clamp-to-edge" | "repeat";
  /** Its `@group(2)` binding (Design note 23). */
  readonly binding: number;
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

/**
 * One frame of one view or target.
 *
 * @remarks
 * Its draws are encoded in submission order, indirect ones among them, into one pass over the
 * view's or target's colour and depth; with post-processes, that pass draws into an `rgba16float`
 * intermediate of the same size, and each post-process is a full-screen pass reading the colour
 * before it, the last writing the view's or target's colour.
 */
export interface FrameSubmission {
  /** The pass's label in {@link PassTimes}, stable across frames (R12 keys its records on it). */
  readonly label: string;
  /**
   * 4 × 4, translation zero, right-handed (R02 fills it), column-major: sixteen `f32` in WGSL's
   * `mat4x4f` order, as R02's `viewRotation4` gives them. Reaches the shaders unchanged as
   * `frame.viewRotation` ({@link BIND_GROUPS}).
   */
  readonly viewRotation: Float32Array;
  /**
   * 4 × 4, reversed-Z, WebGPU clip space, column-major like {@link FrameSubmission.viewRotation};
   * reaches the shaders unchanged as `frame.clipProjection`.
   */
  readonly projection: Float32Array;
  readonly draws: ReadonlyArray<DrawItem>;
  readonly postProcesses: ReadonlyArray<PostProcessItem>;
  /**
   * How a view's canvas is written: `srgb-view`, the default, through its `-srgb` view, so that
   * the store encodes; `in-pass` through the canvas's own format, the pass writing encoded values
   * itself (R07's tone-mapping pass, which dithers after encoding; R07.T15, decision 2026-10-02,
   * item 6). Offscreen targets ignore it.
   */
  readonly encoding?: "srgb-view" | "in-pass";
  /**
   * `clear`, the default, clears colour and depth before the draws; `load` keeps the colour and
   * depth an earlier submission to the same output drew, so that cased symbology follows the
   * tone-mapping pass (R07.T16). Ignored where the frame has post-processes, whose chain always
   * starts clear. On a view, the earlier submission must be in the same task: a canvas's texture
   * expires once the task yields.
   */
  readonly colourLoad?: "clear" | "load";
}

/** One post-process in a frame, with its per-frame values, shaped like a {@link DrawItem}. */
export interface PostProcessItem {
  readonly postProcess: PostProcessHandle;
  /** By {@link WgslPostProcessSpec.uniforms} name; they fill its `@group(1)` `Draw`. */
  readonly uniforms: Readonly<Record<string, Float32Array>>;
  /** By {@link WgslPostProcessSpec.textures} name (R07's bloom levels). */
  readonly textures?: Readonly<Record<string, TextureHandle>>;
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

/**
 * A WGSL material to create.
 *
 * @remarks
 * The sources are standard WGSL under {@link BIND_GROUPS}' convention (Design note 23): the vertex
 * stage is `@vertex fn vertexMain`, the fragment stage `@fragment fn fragmentMain` writing
 * `@location(0)`, each declaring the groups it reads with explicit `@group`/`@binding`, the
 * `Frame` from `view/shaders/frame.wgsl` included by string concatenation. The two sources may be
 * the same module. A texture bound per draw is a `DrawItem.textures` entry named in
 * {@link WgslMaterialSpec.textures}.
 */
export interface WgslMaterialSpec {
  readonly name: string;
  /**
   * The effect's name on the console (`GRAPHICS SHADER REFUSED: <displayName> …`): upper case, at
   * most three words, what it draws; checked by `catalogue.test.ts`. The log keeps {@link name}.
   */
  readonly displayName: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  /** The `Draw` struct's members after `offsetFromCameraM`, in order. */
  readonly uniforms: ReadonlyArray<UniformSpec>;
  readonly samplers: ReadonlyArray<SamplerSpec>;
  /** The sampled textures it declares in `@group(2)`. */
  readonly textures?: ReadonlyArray<TextureBindingSpec>;
  readonly cullMode: "none" | "back";
  /** `false` for R02's lines and sprites. */
  readonly depthWrite: boolean;
  /** `false` for R02's depth-only occluders. */
  readonly colourWrites: boolean;
  /**
   * Every mode keeps the destination alpha, R07's meter class (R01 Design note 21); `additive` is
   * R02's sprites, in linear light. Nothing is reordered by it: draws are encoded in submission
   * order, so the caller submits its blended draws after its opaque ones.
   */
  readonly blend: "none" | "additive" | "premultiplied";
  readonly storageBuffers?: ReadonlyArray<StorageBufferSpec>;
  /**
   * Positive is away from the camera, whatever the depth direction (R02's hull occluder faces;
   * never set on a line pass).
   */
  readonly depthBiasAway?: { readonly constant: number; readonly slopeScale: number };
}

/**
 * A WGSL post-process to create: a full-screen pass over the colour drawn before it.
 *
 * @remarks
 * The fragment source is standard WGSL under {@link BIND_GROUPS}' convention, with
 * `@fragment fn fragmentMain(@location(0) uv : vec2f) -> @location(0) vec4f`; the adapter supplies
 * the full-screen vertex stage. The colour drawn so far is at {@link POST_PROCESS_BINDINGS}; a
 * target's depth is read by a full-screen draw instead (R05), not here.
 */
export interface WgslPostProcessSpec {
  readonly name: string;
  /**
   * The effect's name on the console (`GRAPHICS SHADER REFUSED: <displayName> …`): upper case, at
   * most three words, what it draws; checked by `catalogue.test.ts`. The log keeps {@link name}.
   */
  readonly displayName: string;
  readonly fragmentWgsl: string;
  /** The `Draw` struct's members, in order, set per frame by `PostProcessItem.uniforms`. */
  readonly uniforms: ReadonlyArray<UniformSpec>;
  /** `hdr-colour` is always bound; listing it changes nothing. */
  readonly inputs?: ReadonlyArray<PostProcessInput>;
  readonly samplers?: ReadonlyArray<SamplerSpec>;
  /** Further textures, bound per frame from `PostProcessItem.textures`. */
  readonly textures?: ReadonlyArray<TextureBindingSpec>;
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
  /**
   * A view drawing into `canvas`, under `name` (the name a `view-refused` fault carries).
   *
   * @throws Error when the canvas gives no WebGPU context or the context cannot be configured;
   *   the caller reports it (`ViewDisplay` shows `NOT_MADE`; a restore reports `view-refused`).
   * @remarks
   * `ResilientEngine` makes a view while there is no device too: it draws from the restore on.
   */
  createView(canvas: HTMLCanvasElement, name: string): RenderView;
  /** An offscreen colour target with its own depth: HDR, bloom chains, stills, the harness. */
  createRenderTarget(spec: RenderTargetSpec): RenderTarget;
  createMesh(spec: MeshSpec): MeshHandle;
  createMaterial(spec: WgslMaterialSpec): MaterialHandle;
  /**
   * Resolves once the material's shaders are compiled and its pipelines for `meshes` into
   * `targets` are made, by `createRenderPipelineAsync`, so that no frame waits on a compile.
   *
   * @remarks
   * A pipeline depends on the mesh's vertex layout as well; a mesh not named here has its pipeline
   * made asynchronously at its first draw, which is left out until it is ready.
   *
   * @throws Error, as a rejection, naming the material and the compiler's messages.
   */
  createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat>,
    meshes?: ReadonlyArray<MeshHandle>,
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
   * @throws {@link Float32BlendUnavailable} without `float32-blendable`; the caller falls back to a
   * compute splat (R01 Design note 21).
   */
  createPointSplat(spec: PointSplatSpec): PointSplatHandle;
  /**
   * Dispatches a kernel.
   *
   * @param workgroups - Workgroup counts, or GPU-written ones (an indirect dispatch).
   * @param pass - The timed pass it belongs to; `compute` when absent.
   */
  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs,
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
   * @throws {@link PresentationOnlyReadback} for a buffer last written by a `presentation-only` kernel.
   */
  readBuffer(buffer: BufferHandle, access?: "cpu" | "tolerance"): Promise<ArrayBuffer>;
  /**
   * CPU readback of a texture level or a region of it, colour or depth.
   *
   * @param access - `tolerance` lifts the refusal for the smoke harness's tolerance checks, as
   * {@link RenderEngine.readBuffer}'s does (R05.T12.b's transmittance table).
   * @throws {@link PresentationOnlyReadback} as {@link RenderEngine.readBuffer} does.
   */
  readTexture(
    texture: TextureHandle,
    level?: number,
    rect?: TexelRect,
    access?: "cpu" | "tolerance",
  ): Promise<ArrayBuffer>;
  /** Per-pass GPU time for each frame, once its query set resolves; silent without the feature. */
  onPassTimes(listener: (times: PassTimes) => void): () => void;
  /** Every creation, destruction and upload, with its byte size and category. */
  onAllocation(listener: (event: AllocationEvent) => void): () => void;
  /**
   * Every fault, as it happens. The engine `loadRenderEngine` returns has already told the status
   * store of a lost device; a listener does not dispatch it again.
   */
  onFault(listener: (fault: GraphicsFault) => void): () => void;
  /**
   * Called once the device is re-created after a loss (R01 Design note 9).
   *
   * @remarks
   * Views survive a rebuild, re-created at their sizes; every other handle belonged to the lost
   * device and is made again by the caller here. Only the engine `loadRenderEngine` returns fires
   * it. Between the loss and the restore, a creation throws `EngineUnavailable`
   * (`resilientEngine.ts`), a view draws nothing, and writes and dispatches are dropped.
   */
  onRestored(listener: () => void): () => void;
  dispose(): void;
}

/** The WebGPU module's one export, `createWebGpuEngine`, which only `loadEngine.ts` imports. */
export type CreateWebGpuEngine = (
  outcome: AdapterOutcome & { readonly kind: "adapter" },
  status: GraphicsStatusStore,
  overrides: CapabilityOverrides | undefined,
) => Promise<RenderEngine>;

/** How `loadRenderEngine` loads the engine. */
export interface LoadEngineOptions {
  /** The harness's withheld-feature runs. */
  readonly overrides?: CapabilityOverrides;
  /** The dynamic import, injectable so that a test fakes it without `vi.mock` of our module. */
  readonly importEngine?: () => Promise<{ readonly createWebGpuEngine: CreateWebGpuEngine }>;
  /** `navigator.gpu`, from which a rebuild asks for a fresh adapter; a test passes a fake. */
  readonly gpu?: GPU;
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

/**
 * Thrown when a draw or a post-process samples the colour of the target it renders into: WebGPU
 * forbids a texture as an attachment and a binding in one pass, and would drop the frame.
 */
export class ColourSelfSample extends Error {
  readonly targetName: string;
  /** The draw's material, or the post-process. */
  readonly ownerName: string;

  constructor(targetName: string, ownerName: string) {
    super(`${ownerName} samples the colour of ${targetName}, which it renders into`);
    this.name = "ColourSelfSample";
    this.targetName = targetName;
    this.ownerName = ownerName;
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
