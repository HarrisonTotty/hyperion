# Plan R01: Graphics platform and the engine adapter

- **Milestone:** Rendering milestone RM1 (R01–R04: a wireframe view at real scale, and the
  determinism checks).
- **Depends on:** none. It builds on the bridge client as galaxy plans 04 and 05 left it: the
  Electron main process, the preload's `window.hyperion`, the `LINK` display and `StatusLine`.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)):
  [The engine](../../brainstorming/rendering-and-planets.md#the-engine) in full: its dependency,
  [the large-world feature off](../../brainstorming/rendering-and-planets.md#the-decision-does-not-rest-on-the-engines-large-world-feature)
  and
  [the engine kept at arm's length](../../brainstorming/rendering-and-planets.md#the-engine-is-kept-at-arms-length)
  with its WGSL-only guard and offline render test;
  [The graphics API, and the Intel problem](../../brainstorming/rendering-and-planets.md#the-graphics-api-and-the-intel-problem)
  in full; of
  [Several views in one client](../../brainstorming/rendering-and-planets.md#several-views-in-one-client),
  one device and one canvas context per view, and the same-origin child-window prototype; of
  [Runtime and code shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape),
  "The engine is loaded lazily" and the one-thread, one-device sentence of "Workers hand heights to
  the render thread"; of [Testing](../../brainstorming/rendering-and-planets.md#testing), the
  headless smoke test of "Golden images are rejected for CI" and the first half of "Several views
  and stills"; the [Decisions](../../brainstorming/rendering-and-planets.md#decisions) entries
  "WebGPU is required", "The engine", "The Linux platform" and the canvas-context half of "Several
  views and stills"; [open questions](../../brainstorming/rendering-and-planets.md#open-questions) 1
  (as a check that the feature stays off), 14 and 15; the first half of step 1 of
  [Suggested order of attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack).

## Goal

When this plan is done the bridge client gets a hardware WebGPU adapter on this machine's UHD 620
under Xorg, and on any Linux machine through X11 or XWayland, from switches its main process owns
and tests. It refuses to offer the photorealistic style on a software adapter, reports a lost device
or a crashed GPU process in the guide's voice instead of freezing, and after a crash loop relaunches
once into a declared safe mode. A `GRAPHICS` panel on the `LINK` display states the adapter, its
features, the mode and any fault. Babylon.js is a dependency loaded only by the displays that draw a
scene, behind an engine-agnostic interface whose types are the only ones the rest of the renderer
sees; the engine runs with its large-world feature off, reversed depth on and every GLSL compile an
error that names the shader, so it never fetches a compiler. One `GPUDevice` drives any number of
views, each through its own canvas context at its own size, with no copies, proved by a cockpit
canvas and two instrument canvases. A headless harness renders every material and post-process on
SwiftShader with the network disabled, reads frames back with `copyTextureToBuffer`, and asserts
properties, on the no-`shader-f16` path and again with subgroups withheld; subgroup kernels are
chosen against their reference twins by feature detection. The interface also gives later plans
offscreen render targets, asynchronous pipeline creation, indirect draws and dispatches, per-pass
GPU time and one guarded CPU readback. The Vulkan soak of open question 14 has
been run and recorded. Nothing is drawn at real scale yet: that is R02.

## Scope and non-goals

In scope:

- The main process: the Linux switch set, merged into Chromium's own values; the X11 relaunch of a
  Wayland session; the Gen9 subgroup toggle; a measurement-only timing switch; GPU-process crash
  monitoring, the crash-loop relaunch and its declared mode; handing the mode and the crash events
  to the renderer through the preload.
- The renderer's adapter and device acquisition, the capability summary, the fallback-adapter
  refusal, device-loss handling, and the graphics status that the `LINK` display and every later
  view read.
- The engine-agnostic interface under `apps/hyperion/src/renderer/src/view/engine/` and its Babylon
  implementation, with the WGSL-only guard, the pinned internals, reversed depth and the depth
  format, one canvas context per view, offscreen render targets, asynchronous pipelines, indirect
  draws and dispatches, per-pass GPU time and the guarded CPU readback.
- Lazy loading as a dynamic import and a named chunk, and a check that the main bundle holds no
  engine code.
- The headless SwiftShader smoke harness, the offline render test, and the subgroup-twin selection.
- By-hand runs, recorded: the three-canvas proof on the UHD 620, the Vulkan soak, the child-window
  prototype.
- Draft nomenclature entries for the graphics annunciations, for the owner.

Non-goals, each with its owner:

- The camera, perspective projection, rotation-only view matrix, camera-relative differencing, frame
  rebasing, the reversed-Z projection matrix and its infinite far plane, and the photometric
  pipeline: R02. This plan sets the engine's depth flag and format so that R02's matrix works; the
  arithmetic and its tests are R02's.
- The `VIEW` display, its styles, the free camera and the nine UX-guide items: R02. This plan's
  nomenclature drafts are handed to R02's single pass to absorb and re-check.
- The scene subscription and the wire: R03. WebAssembly, the CSP's `'wasm-unsafe-eval'` and height
  workers: R04 and R05. This plan changes no CSP: the WGSL-only guard means Babylon needs neither
  WebAssembly nor a CDN origin.
- Any real subgroup kernel. The exposure histogram's reduction is R07's; this plan supplies the pair
  mechanism and proves it on a toy reduction.
- Per-view budgets, secondary-view resolution and the one-photorealistic-view rule: R07. Still
  images and their device-loss limits: R11. Performance runs: R05 and R12.
- Windows and macOS switches: none are needed there (the brainstorm's consequence 2); the switch
  builder returns none off Linux.

## Provides

TypeScript paths are under `apps/hyperion/src/`. Signatures are sketches.

### Main process (`main/graphics/`)

```ts
/**
 * How this launch runs the GPU: Chromium's own path off Linux (no switches, never relaunched), the
 * forced Vulkan path on Linux, or the declared safe mode.
 */
export type GraphicsLaunchMode = "default" | "vulkan" | "safe";
/** One Chromium switch; `value` absent for a bare flag. */
export interface ChromiumSwitch {
  readonly name: string;
  readonly value?: string;
}
export interface GraphicsLaunchOptions {
  readonly platform: NodeJS.Platform;
  readonly mode: GraphicsLaunchMode;
  readonly gpuTiming: boolean; // measurement only: lifts timestamp quantization
}
export function graphicsSwitches(options: GraphicsLaunchOptions): ReadonlyArray<ChromiumSwitch>;
/** Union of comma-separated lists, existing values first, no duplicates. */
export function mergeSwitchValue(existing: string, added: ReadonlyArray<string>): string;
export function applyGraphicsSwitches(
  commandLine: Electron.CommandLine,
  switches: ReadonlyArray<ChromiumSwitch>,
): void;
/** The launch's mode: `default` off Linux, else `safe` with the switch and `vulkan` without. */
export function launchModeOf(platform: NodeJS.Platform, safeSwitch: boolean): GraphicsLaunchMode;

// x11Relaunch.ts
/**
 * `RelaunchOptions.args` for a Wayland session to run through XWayland, or `undefined`.
 * `args` is `process.argv.slice(1)`: Electron supplies the executable itself.
 */
export function x11RelaunchArgs(
  args: readonly string[],
  env: NodeJS.ProcessEnv,
  platform: NodeJS.Platform,
): readonly string[] | undefined;

export const SAFE_MODE_SWITCH = "hyperion-graphics-safe";
export const GPU_TIMING_SWITCH = "hyperion-gpu-timing";

// featureStatus.ts
/**
 * Reads the `vulkan` and `webgpu` entries of `app.getGPUFeatureStatus()`, which Chromium 152
 * reports but Electron 44's `GPUFeatureStatus` type does not declare, by narrowing, never `as`.
 */
export function readFeatureStatus(status: unknown): {
  readonly vulkan: string | undefined;
  readonly webgpu: string | undefined;
};

export type GpuProcessEvent =
  | {
      readonly kind: "gone";
      readonly atMs: number;
      readonly reason: string;
      readonly exitCode: number;
    }
  | {
      readonly kind: "status";
      readonly atMs: number;
      readonly vulkan: string;
      readonly webgpu: string;
    };
export type CrashLoopDecision = "none" | "relaunch-safe";
/** Always `none` in `default` and `safe` modes (Design note 6). */
export function crashLoopDecision(
  history: ReadonlyArray<GpuProcessEvent>,
  mode: GraphicsLaunchMode,
): CrashLoopDecision;
export const CRASH_LOOP_COUNT = 3;
export const CRASH_LOOP_WINDOW_MS = 300_000;
export class GpuProcessMonitor {
  /* wires app events, keeps history, relaunches once */
}
```

### Preload (`preload/api.ts`, additions to `HyperionApi`)

```ts
readonly graphics: {
  readonly launchMode: GraphicsLaunchMode;
  /** True when `--hyperion-gpu-timing` lifted timestamp quantization for this launch. */
  readonly gpuTiming: boolean;
  /** Registers a listener for GPU-process crashes; returns its removal. */
  onGpuProcessGone(listener: (event: GpuProcessGoneReport) => void): () => void;
};
export interface GpuProcessGoneReport { readonly reason: string; readonly count: number }
```

### Renderer: platform and status (`renderer/src/view/engine/`)

```ts
// platform.ts — no engine types
export interface AdapterSummary {
  readonly vendor: string; readonly architecture: string; readonly description: string;
  readonly fallback: boolean;           // info.isFallbackAdapter, or architecture "swiftshader"
}
export interface GpuCapabilities {
  readonly subgroups: boolean; readonly shaderF16: boolean; readonly timestampQuery: boolean;
  readonly float32Filterable: boolean; readonly float32Blendable: boolean;
  readonly rg11b10Renderable: boolean; readonly depthClipControl: boolean; // R10's cascades
  readonly maxTextureDimension2D: number; readonly subgroupMinSize: number | null;
}
export type StyleAvailability = { readonly wireframe: boolean; readonly photorealistic: boolean };
export type AdapterOutcome =
  | { readonly kind: "no-webgpu" }                       // navigator.gpu absent
  | { readonly kind: "no-adapter" }
  | { readonly kind: "adapter"; readonly adapter: GPUAdapter; readonly summary: AdapterSummary;
      readonly capabilities: GpuCapabilities; readonly styles: StyleAvailability };
export function summariseAdapter(adapter: GPUAdapter): { summary: AdapterSummary;
    capabilities: GpuCapabilities };
export function styleAvailability(summary: AdapterSummary): StyleAvailability;
export function requestAdapterOutcome(gpu: GPU | undefined): Promise<AdapterOutcome>;
/** Optional features requested whenever present; never required. */
export const WANTED_FEATURES: ReadonlyArray<GPUFeatureName>;
export interface CapabilityOverrides { readonly withholdSubgroups: boolean;
    readonly withholdShaderF16: boolean;
    readonly withholdFloat32Blendable?: boolean } // the harness's runs only (T9.i)
/** WANTED_FEATURES ∩ the adapter's features, minus what the overrides withhold. */
export function requiredFeatures(adapter: GPUAdapter, overrides: CapabilityOverrides | undefined):
    ReadonlyArray<GPUFeatureName>;

// status.ts
export type GraphicsFault =
  | { readonly kind: "device-lost"; readonly reason: GPUDeviceLostReason; readonly message: string }
  | { readonly kind: "gpu-process-gone"; readonly count: number };
export type GraphicsCondition =
  | { readonly kind: "acquiring" }
  | { readonly kind: "nominal"; readonly summary: AdapterSummary;
      readonly styles: StyleAvailability }
  | { readonly kind: "software-adapter"; readonly summary: AdapterSummary }
  | { readonly kind: "no-webgpu" }                        // navigator.gpu absent
  | { readonly kind: "no-adapter" }                       // requestAdapter gave null at start
  | { readonly kind: "safe-mode" }
  | { readonly kind: "disabled"; readonly cause: "device-losses" | "adapter-withdrawn";
      readonly losses: number };
/**
 * Whether timestamps carry Dawn's 65,536 ns quantization (Design note 4), or the device has no
 * `timestamp-query` at all.
 */
export type GpuTimer = "quantized" | "full" | "absent";
/** How the adapter rounds a colour-attachment write, per float format (R07; Design note 22). */
export type TargetRounding = "nearest" | "toward-zero" | "unknown"; // unknown until probed
export interface GraphicsStatus {
  readonly condition: GraphicsCondition;
  readonly capabilities: GpuCapabilities | null;
  readonly launchMode: GraphicsLaunchMode;
  readonly timer: GpuTimer; // gpuTiming from window.hyperion.graphics, and the device's features
  readonly targetRounding: Readonly<Record<"rgba16float" | "rg11b10ufloat", TargetRounding>>;
  readonly fault: GraphicsFault | null;          // current, cleared on recovery
  readonly deviceLosses: number; readonly gpuProcessCrashes: number;
}
export type GraphicsEvent = /* adapter outcome, device lost, device restored, process gone,
    adapter withdrawn on a rebuild */;
export function reduceGraphicsStatus(status: GraphicsStatus, event: GraphicsEvent): GraphicsStatus;
export const DEVICE_LOSS_LIMIT = 3;
/** `standing` is `StatusLine`'s own `StatusStanding`: `refused` for a plain statement. */
export function graphicsAnnunciation(status: GraphicsStatus):
    { readonly text: string;
      readonly standing: Extract<StatusStanding, "refused" | "fault"> } | null;
export class GraphicsStatusStore { subscribe(l: () => void): () => void; getSnapshot(): GraphicsStatus;
    dispatch(e: GraphicsEvent): void }
export function useGraphicsStatus(): GraphicsStatus;
```

### Renderer: the engine-agnostic interface (`renderer/src/view/engine/types.ts`)

No type from `@babylonjs/*` appears in these signatures or anywhere outside `view/engine/babylon/`.

```ts
export type DepthPolicy = "reversed-z-float"; // depth32float, clear 0, compare greater-equal
export type ColourTargetFormat = "rgba16float" | "rg11b10ufloat" | "rgba8unorm";
export interface ViewSize {
  readonly widthPx: number;
  readonly heightPx: number;
}
export interface RenderEngine {
  readonly capabilities: GpuCapabilities; // the device's features, after overrides
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
  /** The variant is chosen by `selectKernel` against `capabilities`. */
  createCompute(pair: KernelPair): ComputeHandle;
  createComputeAsync(pair: KernelPair): Promise<ComputeHandle>;
  /** Every GPU buffer and texture is created here, and nowhere else, with its memory category. */
  createBuffer(spec: BufferSpec): BufferHandle;
  createTexture(spec: TextureSpec): TextureHandle; // 2D, 3D, cube; sampled and/or storage
  /** R06's packed star cube: rgb9e5ufloat, every mip written by copyBufferToTexture. */
  createPackedCube(sizePx: number, mips: number, category: MemoryCategory): TextureHandle;
  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void;
  /** The same level written from a GPU buffer a kernel filled, with no readback (R06's bake). */
  writePackedCubeLevelFromBuffer(cube: TextureHandle, level: number, packed: BufferHandle): void;
  /**
   * An additive `point-list` pass into a 2D `rgba32float` bake target (R06's sky splat). Throws
   * without `float32-blendable`; the caller falls back to a compute splat. Design note 21.
   */
  createPointSplat(spec: PointSplatSpec): PointSplatHandle;
  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number] | IndirectArgs, // indirect: GPU-written counts
    pass?: string, // the timed pass it belongs to; default `compute`
  ): void;
  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void;
  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void; // each write raises an `uploaded` event
  /**
   * CPU readback, refused for a buffer last written by a `presentation-only` kernel. `"tolerance"`
   * lifts the refusal for the smoke harness's tolerance checks; the boundary test fails on it
   * anywhere else.
   */
  readBuffer(buffer: BufferHandle, access?: "cpu" | "tolerance"): Promise<ArrayBuffer>;
  /** CPU readback of a texture level or a region of it, colour or depth; the same refusal. */
  readTexture(
    texture: TextureHandle,
    level?: number,
    rect?: {
      readonly x: number;
      readonly y: number;
      readonly width: number;
      readonly height: number;
    },
  ): Promise<ArrayBuffer>;
  /** Per-pass GPU time for each frame, once its query set resolves; silent without the feature. */
  onPassTimes(listener: (times: PassTimes) => void): () => void;
  /** Every creation and destruction, with its byte size and category; R05's tally subscribes. */
  onAllocation(listener: (event: AllocationEvent) => void): () => void;
  onFault(listener: (fault: GraphicsFault) => void): () => void;
  dispose(): void;
}
export type RenderTargetFormat = ColourTargetFormat | "canvas";
export interface RenderTargetSpec {
  readonly name: string;
  readonly size: ViewSize;
  readonly format: ColourTargetFormat;
  readonly mips: number; // sampled mips, generated after rendering
  readonly depth: boolean; // depth32float of its own, with COPY_SRC and TEXTURE_BINDING
  readonly category: MemoryCategory; // "render-targets" unless a later plan names another
}
/** An offscreen target: drawn like a view, sampled as a texture by later passes. */
export interface RenderTarget {
  readonly name: string;
  readonly colour: TextureHandle;
  readonly depth: TextureHandle | null;
  resize(size: ViewSize): void;
  render(frame: FrameSubmission): void; // level 0; a chain of levels is one target per level
  dispose(): void;
}
/** The GPU-side arguments of an indirect draw or dispatch, in WebGPU's layout. */
export interface IndirectArgs {
  readonly buffer: BufferHandle; // created with GPUBufferUsage.INDIRECT
  readonly offsetBytes: number;
}
/** One frame's GPU time per labelled pass, in nanoseconds, with the timer's resolution. */
export interface PassTimes {
  readonly frame: number;
  readonly timer: GpuTimer;
  readonly passes: ReadonlyArray<{
    readonly label: string;
    readonly ns: number;
    readonly bracketed: boolean;
  }>; // bracketed: measured around a Babylon-encoded pass
}
export interface ComputeBindings {
  readonly uniforms: Readonly<Record<string, Float32Array | Uint32Array>>;
  readonly buffers: Readonly<Record<string, BufferHandle>>; // storage, read or read-write
  readonly sampled: Readonly<Record<string, TextureHandle>>;
  readonly storage: Readonly<
    Record<string, { readonly texture: TextureHandle; readonly level: number }>
  >;
}
// Opaque handles, each owned by the engine that made it: MeshHandle, MaterialHandle,
// PostProcessHandle, ComputeHandle, BufferHandle, TextureHandle.
export interface MeshSpec {
  readonly name: string;
  readonly positions: Float32Array; // metres, relative to the draw's offset
  readonly indices: Uint32Array | null;
  readonly topology: "triangle-list" | "line-list" | "point-list";
  readonly attributes: Readonly<
    Record<string, { readonly data: Float32Array; readonly size: 1 | 2 | 3 | 4 }>
  >;
  /** Per-instance attributes, stepped once per instance (R02's segment instances). */
  readonly instanceAttributes?: Readonly<
    Record<string, { readonly data: Float32Array; readonly size: 1 | 2 | 3 | 4 }>
  >;
}
export interface StorageBufferSpec {
  readonly name: string;
  readonly binding: number; // read-only in the vertex and fragment stages
}
export type PostProcessInput = "depth" | "hdr-colour";
export interface PointSplatSpec {
  readonly name: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  readonly format: "rgba32float"; // a bake scratch, never a frame's colour target
  readonly blend: "additive";
}
export interface PointSplatHandle {
  draw(target: TextureHandle, points: BufferHandle, count: number): void;
  dispose(): void;
}
export interface UniformSpec {
  readonly name: string;
  readonly type: "f32" | "vec2f" | "vec3f" | "vec4f" | "mat4x4f" | "u32";
}
export interface SamplerSpec {
  readonly name: string;
  readonly filter: "nearest" | "linear";
  readonly address: "clamp-to-edge" | "repeat";
}
// memory.ts — the categories; R05's tally and R12's itemisation extend the union
export type MemoryCategory = "render-targets" | "other"; /* | … added by later plans */
export interface BufferSpec {
  readonly name: string;
  readonly bytes: number;
  readonly usage: GPUBufferUsageFlags;
  readonly category: MemoryCategory;
}
export interface TextureSpec {
  readonly name: string;
  readonly size: GPUExtent3D; // TS 7's lib.dom has no GPUExtent3DStrict
  readonly dimension: "2d" | "3d" | "cube";
  readonly format: GPUTextureFormat;
  readonly mips: number;
  readonly usage: GPUTextureUsageFlags;
  readonly category: MemoryCategory;
}
export type AllocationEvent =
  | {
      readonly kind: "created" | "destroyed";
      readonly name: string;
      readonly bytes: number;
      readonly category: MemoryCategory;
    }
  | { readonly kind: "uploaded"; readonly name: string; readonly bytes: number };
export function textureBytes(spec: TextureSpec): number; // mips, layers, packed formats
export interface RenderView {
  readonly name: string;
  resize(size: ViewSize): void; // this view's attachments only
  render(frame: FrameSubmission): void;
  readBack(): Promise<Float32Array | Uint8Array>; // harness only: copyTextureToBuffer
  dispose(): void;
}
export interface FrameSubmission {
  /** The pass's label in `PassTimes`, stable across frames (R12 keys its records on it). */
  readonly label: string;
  readonly viewRotation: Float32Array; // 4 × 4, translation zero, right-handed (R02 fills it)
  readonly projection: Float32Array; // 4 × 4, reversed-Z, WebGPU clip space; passed unchanged
  readonly draws: ReadonlyArray<DrawItem>;
  readonly postProcesses: ReadonlyArray<PostProcessHandle>;
}
export interface DrawItem {
  readonly mesh: MeshHandle;
  readonly material: MaterialHandle;
  readonly offsetFromCameraM: Float32Array; // f64-differenced on the CPU, narrowed (R02)
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
export interface WgslMaterialSpec {
  readonly name: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  readonly uniforms: ReadonlyArray<UniformSpec>;
  readonly samplers: ReadonlyArray<SamplerSpec>;
  readonly transparent: boolean;
  readonly cullMode: "none" | "back";
  readonly depthWrite: boolean; // false for R02's lines and sprites
  readonly colourWrites: boolean; // false for R02's depth-only occluders
  /** Every mode keeps the destination alpha, R07's meter class (Design note 21). */
  readonly blend: "none" | "additive" | "premultiplied"; // additive: R02's sprites, linear light
  readonly storageBuffers?: ReadonlyArray<StorageBufferSpec>;
  /**
   * Positive is away from the camera, whatever the depth direction (R02's hull occluder faces;
   * never set on a line pass).
   */
  readonly depthBiasAway?: { readonly constant: number; readonly slopeScale: number };
}
export interface WgslPostProcessSpec {
  readonly name: string;
  readonly fragmentWgsl: string;
  readonly uniforms: ReadonlyArray<UniformSpec>;
  /** Extra inputs: the view's reversed-Z depth and its HDR colour (R05's aerial perspective). */
  readonly inputs?: ReadonlyArray<PostProcessInput>;
  readonly samplers?: ReadonlyArray<SamplerSpec>;
}
export interface LoadEngineOptions {
  readonly overrides?: CapabilityOverrides; // the harness's withheld-feature runs
  /** The dynamic import, injectable so that a test fakes it without `vi.mock` of our module. */
  readonly importEngine?: () => Promise<{ createBabylonEngine: CreateBabylonEngine }>;
}
export function loadRenderEngine(
  outcome: AdapterOutcome & { kind: "adapter" },
  status: GraphicsStatusStore,
  options?: LoadEngineOptions,
): Promise<RenderEngine>; // loadEngine.ts, dynamic import
/** The Babylon module's one export, `view/engine/babylon/engine.ts`. */
export type CreateBabylonEngine = (
  outcome: AdapterOutcome & { kind: "adapter" },
  status: GraphicsStatusStore,
  overrides: CapabilityOverrides | undefined,
) => Promise<RenderEngine>;
/** Thrown by the WGSL-only guard, naming the effect (Design note 12); `wgslGuard.ts`. */
export class GlslShaderRefused extends Error {
  readonly effectName: string;
}
/** Thrown by `createPointSplat` when the device lacks `float32-blendable` (Design note 21). */
export class Float32BlendUnavailable extends Error {}
/** Thrown when a draw samples the depth of the target it renders into (Design note 21). */
export class DepthSelfSample extends Error {
  readonly targetName: string;
  readonly materialName: string;
}
/** Thrown by `readBuffer` and `readTexture` for a presentation-only result (Design note 16). */
export class PresentationOnlyReadback extends Error {
  readonly kernelName: string;
}
/** Canvas pixels from CSS size, rounded, clamped to the limit, never zero; `viewSize.ts`. */
export function viewPixelSize(
  cssSize: { readonly width: number; readonly height: number },
  devicePixelRatio: number,
  maxTextureDimension2D: number,
): ViewSize;
```

### Renderer: kernels (`renderer/src/view/engine/kernels.ts`)

```ts
export interface KernelPair {
  readonly name: string;
  readonly reference: string; // WGSL without subgroups: the reference
  readonly subgroup: string | null; // WGSL with `enable subgroups;`
  readonly readback: "bit-exact" | "presentation-only";
}
export function selectKernel(
  pair: KernelPair,
  capabilities: GpuCapabilities,
): { readonly path: "reference" | "subgroup"; readonly wgsl: string };
/** Throws if a module enables both `f16` and `subgroups`. */
export function assertNoF16Subgroups(name: string, wgsl: string): void;
/** The harness's tolerance for a presentation-only f32 sum (Design note 16). */
export function highamBound(n: number, sumAbs: number): number; // γ(n − 1) · Σ|x|, u = 2⁻²⁴
```

### Components and tests

- `renderer/src/components/GraphicsPanel.tsx` on the `LINK` display, and the safe-mode and disabled
  banner in `ConsoleFrame`'s status area.
- `renderer/src/test/fakeGpu.ts`: `FakeGpu`, `FakeAdapter`, `FakeDevice` with a controllable `lost`
  promise and feature set, for tests in the `logic` project. A `FakeAdapter` is consumed by its
  first `requestDevice`, as the WebGPU spec's adapter is.
- `smoke/` (new, beside `main`, `preload` and `renderer`): the harness's Electron entry and its
  preload; `renderer/smoke.html`, its test page; `just test-render`. Only the smoke page may pass
  `readBuffer`'s `"tolerance"` access, to check a presentation-only sum against its bound.
- `scripts/checkChunks.mjs` under `apps/hyperion/`: fails if the entry chunk holds Babylon code or
  no `babylon` chunk exists.
- `WGSL_CATALOGUE` in `view/engine/catalogue.ts`: every material, post-process and compute kernel
  the adapter can create, which the smoke harness renders one by one. Later plans register theirs
  there.

## Consumes

R01 is the first rendering plan and consumes no other R-plan. From what is built:

- **Galaxy plan 05** (built): the `LINK` display (`displays.ts`, `ConnectionPanel.tsx`),
  `StatusLine` and its `fault` standing, `ConsoleFrame`'s status area, the `.panel` and `.readout`
  classes, the test setup and fakes under `renderer/src/test/`.
- **Galaxy plan 04** (built): nothing on the wire. The client's command line (`main/cli.ts`), which
  already allows unknown options so that Chromium's switches pass through it.
- **R02:** absorbing this plan's nomenclature drafts (T5.c) into its single pass over the guide
  (R02.T2.f), and supplying the projection and view-rotation matrices that `FrameSubmission`
  carries, in WebGPU clip space and a right-handed scene.

Asks received from sibling plans, each checked against the plan on disk and met here (Design note
18): R02's pass-through projection, handedness, per-material cull mode and depth bias, no engine
frustum culling, and linear-light blending through an sRGB view; R06's packed cube
(`createPackedCube`, `writePackedCubeLevel`, R06 Design note 21), and, at the reconciliation, its
`writePackedCubeLevelFromBuffer` and `createPointSplat` (Design note 21 here); R02's `depthWrite`,
`colourWrites`, `blend` and per-instance vertex buffers with `DrawItem.instanceCount`; R05's
instanced draws, storage buffers on materials, `writeTexture` and post-process inputs (Design note
21); R05's per-draw textures and compute kernels writing 2D and 3D storage textures with their bytes
visible to its allocation tally (R05 Consumes); R12's one creation path with a memory category
(R12.T3.a) and one `just test-render` variant per setting (R12.T7.a); and R10's `depth-clip-control`
among the wanted features (Design note 7). Added after the 2026-09-29 review (Design notes 19 and
20): R07's and R11's offscreen render targets (`createRenderTarget`, R07's HDR target and bloom
chain, R11's stills), R11's indirect draws and asynchronous pipeline creation (`DrawItem.indirect`,
`createMaterialAsync`, `createComputeAsync`), per-pass GPU time for R05, R07, R11 and R12
(`onPassTimes` with stable `FrameSubmission.label`s), and the timer's quantization in
`GraphicsStatus.timer` for R11 and R12. Added at the final cross-plan pass of 2026-09-29: R05's
render-target depth sampled in a draw as `texture_depth_2d` and R07's alpha kept by every blend
mode, with `"premultiplied"` for R08's and R11's translucent layers (Design note 21, R01.T8.i); and
R07's probe of each adapter's render-target rounding, `GraphicsStatus.targetRounding` (Design note
22, R01.T8.j). R05's `RenderView.passTimings()` and R12's `GraphicsStatus.gpuTiming` and
`timestampQuantumNs` are met by R01's own names, which the consumers adopt: `onPassTimes` delivering
`PassTimes`, and `GraphicsStatus.timer` (`"quantized"` means a 65,536 ns quantum, `"full"` the
device's own resolution, Design note 4), from which R12 derives its record's quantum.

The skills' task-ID patterns (`plan_task.py`, `select_checks.py`) accept only `P..` IDs until
R04.T7.c's change lands. That change is needed before R01's first task goes through the
`implement-task` or `validate` skill; until then R01's tasks are built and checked by hand with the
commands in their acceptance lines.

## Design notes

1. **Where the code lives.** The engine-agnostic interface and its Babylon implementation are
   created here under `renderer/src/view/engine/`, the first files of the `view/` directory the
   brainstorm names. R02 adds the camera, scene builder and styles beside them. Only
   `view/engine/babylon/` may import `@babylonjs/*`; a test, not a lint override, enforces it
   (`engineBoundary.test.ts` reads every renderer source file and fails on an import of the engine
   elsewhere), because `.claude/rules/typescript-dev.md` forbids changing a rule's scope in
   `.oxlintrc.json` without asking and a test needs no permission. The same test fails on any call
   of `registerView`, whose `drawImage` copy the brainstorm rejects, and on `readBuffer`'s
   `"tolerance"` access outside the smoke page (Design note 16). It skips its own file, and builds
   its fixture strings at run time, so that its rules never match their own text.

2. **The switches are data, merged, and set before `ready`.** `graphicsSwitches` returns the
   brainstorm's set on Linux and nothing elsewhere: `use-angle=vulkan`,
   `enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`, and
   `enable-dawn-features=enable_subgroups_intel_gen9`; never `enable-unsafe-webgpu`. Researched
   2026-09-29 by probe on Electron 44.4.3 on the UHD 620 (Mesa 26.2.3 ANV, Xorg), a scratch app
   that logged `process.argv`, the GPU process's command line and `getGPUFeatureStatus()`: an
   appended `enable-features` or `enable-dawn-features` replaces any value already on the command
   line, last writer wins, so `applyGraphicsSwitches` reads `app.commandLine.getSwitchValue`, takes
   the union with `mergeSwitchValue` and appends once; Electron's own defaults are merged separately
   and survive. `enable-dawn-features` does reach Dawn (inside `--gpu-preferences`): `subgroups`
   appears with it and not without it. `app.getGPUFeatureStatus()` read at `ready` is stale and is
   read only after `gpu-info-update`.

3. **`--ozone-platform=x11` cannot be appended; it needs a relaunch.** The brainstorm says the main
   process sets it before `ready`. Researched 2026-09-29 by probe: the browser process chooses its
   Ozone platform from `XDG_SESSION_TYPE` before main-process JavaScript runs, so an appended
   `ozone-platform` or `ozone-platform-hint`, and `ELECTRON_OZONE_PLATFORM_HINT`, all leave a
   Wayland session on Wayland; worse, the appended value reaches the GPU process's command line, and
   a mismatch between the two processes gave no adapter and black output. Only the flag on the real
   command line works. So `x11RelaunchArgs` returns the relaunch arguments when the platform is
   Linux, `XDG_SESSION_TYPE` is `wayland` and the argv lacks `--ozone-platform=x11`, and the main
   process calls `app.relaunch({ args })` and `app.exit(0)` before `ready`; under Xorg nothing
   happens. The packaged launcher and `.desktop` entry put the flag on the command line so that the
   relaunch is a fallback, not the rule. The outcome is the brainstorm's (every Linux machine runs
   through X11 or XWayland); only the mechanism differs, and the notes file reports it. A relaunch
   does not survive `electron-vite dev`: electron-vite spawns Electron and exits when that process
   closes (`spawn(electronPath, …)` then `ps.on('close', process.exit)`, electron-vite
   6.0.0-beta.1, `dist/chunks/lib-6EHSwoSb.js:242-243`), taking the dev server with it, so a
   relaunched client loads a dead `ELECTRON_RENDERER_URL`. In development the `client` recipe
   therefore puts `--ozone-platform=x11` on the command line itself when `XDG_SESSION_TYPE` is
   `wayland`, and the crash-loop relaunch is checked by hand on a built client (T2.b).

4. **Timing is quantized, and one narrow switch lifts it for measurement.** The brainstorm's step 3
   says GPU time per pass is available uncoarsened on Linux under the forced switches. Researched
   2026-09-29 by probe on the UHD 620: `timestamp-query` is exposed under the switch set, but every
   timestamp is a multiple of 65,536 ns; `--disable-dawn-features=timestamp_quantization` lifts it
   and adds no feature and no adapter, while `--enable-webgpu-developer-features` also lifts it but
   exposes more, `--enable-unsafe-webgpu` lifts it too but adds features and CPU adapters (confirmed
   by R07's research), and `allow_unsafe_apis` does not. So the main process adds that one Dawn
   toggle, merged, only when `--hyperion-gpu-timing` is on the command line, which the performance
   runs of R05 and R12 pass; shipping launches keep the quantization. The main process passes the
   flag on to the preload as `graphics.gpuTiming`, as it passes the safe switch, and
   `GraphicsStatus.timer` states `quantized` or `full`, so a recorded figure says whether its timer
   was coarse; every `PassTimes` carries the same field.

5. **The safe mode has no WebGPU, and says so.** The brainstorm's declared mode is "without the
   photorealistic style", implying the wireframe survives. Researched 2026-09-29 by probe of eight
   switch sets: every set that gives this machine a hardware adapter contains the `Vulkan` feature,
   which also moves Skia's compositing to Vulkan, and no set keeps Dawn on Vulkan with compositing
   on GL; without `Vulkan`, X11 gives no adapter at all, and the only other adapter is SwiftShader
   through the unsafe flag, which the client never sets. So the safe mode drops the Vulkan features
   and `use-angle=vulkan`, keeps the DOM and Canvas 2D consoles, and has no views: every view shows
   the mode instead of a picture. That is the reading closest to the brainstorm (a declared mode,
   entered once, that does not pretend to draw), and the notes file reports the difference. It is
   still worth relaunching into rather than leaving Chromium to its own fallback, because the probe
   saw that fallback take the page through an adapter that vanishes after three crashes and software
   compositing after six, undeclared.

6. **What a crash loop is.** Researched 2026-09-29 by probe (SIGKILL of the GPU process every 4 s):
   each crash fires `app.on("child-process-gone")` with `type: "GPU"`, then `gpu-info-update`; after
   three, `getGPUFeatureStatus().vulkan` leaves `enabled_on` and `requestAdapter()` returns null;
   after six, GPU compositing goes to software. Nothing reaches `render-process-gone`.
   `crashLoopDecision` therefore relaunches into safe mode, in `vulkan` mode only, when either
   `CRASH_LOOP_COUNT` (3) GPU-process exits other than `clean-exit` fall within
   `CRASH_LOOP_WINDOW_MS` (5 minutes), or a status event shows Vulkan or WebGPU _no longer_
   enabled: `vulkan` leaving `enabled_on`, or `webgpu` leaving `enabled` (the values the probe
   read), after an earlier status event in the same launch held them. A first status that never
   held them does not trigger it; that machine gets no adapter and says so (`no-adapter`). In
   `safe` mode it never relaunches, which is what makes it "once". In `default` mode, which is
   every platform but Linux, it never relaunches either: those platforms get no switches, Chromium
   does not use Vulkan there, so a `vulkan` status of `disabled_off` is normal, and a crash loop is
   left to Chromium's own fallback and reported through the same faults. Electron 44.4.3's typed
   `GPUFeatureStatus` declares neither `vulkan` nor `webgpu` (`electron.d.ts:8641-8657`), though
   Chromium 152 reports both at runtime, so `readFeatureStatus` narrows them from `unknown`. The
   relaunch passes `process.argv.slice(1)` plus `--hyperion-graphics-safe` as `RelaunchOptions.args`
   (Electron supplies the executable itself), and the main process reads the switch with
   `app.commandLine.hasSwitch` and passes it on to the preload in `additionalArguments`, as it
   passes the server URL. `app.disableDomainBlockingFor3DAPIs()` is called before `ready`: without
   it the probe saw Chromium block WebGPU for the page after the second crash, so the client never
   got the chance to report and recover.

7. **The client requests its own adapter, and Babylon is handed it.** Researched 2026-09-29 in
   `@babylonjs/core` 9.28.0: `WebGPUEngine.initAsync` always calls `navigator.gpu.requestAdapter`
   itself (`Engines/webgpuEngine.pure.js:413-417`), takes no adapter or device, and silently drops
   requested features the adapter lacks (`:430-438`). So each engine creation installs a wrapper on
   `navigator.gpu.requestAdapter` for the one `initAsync` call it makes, returning the adapter
   `requestAdapterOutcome` has just vetted, and removes it once `initAsync` settles. Babylon never
   calls it again: with `doNotHandleContextLost: true` its own restore, which would re-run
   `initAsync`, is skipped (`webgpuEngine.pure.js:473-490` sit inside
   `if (!this._doNotHandleContextLost)`). An adapter is consumed by its first `requestDevice`
   (the WebGPU specification's adapter `[[state]]` becomes "consumed", and a second `requestDevice`
   on it does not give a usable device), so every
   rebuild after a loss requests and vets a fresh adapter (Design note 9) and never reuses the old
   one. The creation passes `deviceDescriptor.requiredFeatures` from `requiredFeatures(adapter,
overrides)`, `WANTED_FEATURES` intersected with the adapter's own less what the harness's
   `CapabilityOverrides` withhold, and after `initAsync` checks `engine.enabledExtensions` against
   what was asked; `RenderEngine.capabilities` is read from the device's features, so a withheld
   feature reads as absent everywhere downstream. The wrapper
   touches a browser API, not an engine internal. Every optional feature in `WANTED_FEATURES`
   (`subgroups`, `shader-f16`, `timestamp-query`, `float32-filterable`, `float32-blendable`,
   `rg11b10ufloat-renderable`, and `depth-clip-control` for R10's shadow cascades) is requested when
   present and never required.

8. **A software adapter draws wireframe only.** `styleAvailability` refuses the photorealistic style
   when `adapter.info.isFallbackAdapter` is true or `architecture` is `swiftshader`; the probe found
   both on SwiftShader and neither on the Intel part (`architecture` `gen-9`, `device` and
   `description` empty, so neither is keyed on), and the deprecated `adapter.isFallbackAdapter`
   undefined. Without the unsafe flag Chromium never hands out a CPU adapter, so in the client the
   refusal is a guard; the harness, which does set the flag, is where it is exercised. The wireframe
   stays available on a software adapter, as the brainstorm confines the refusal to the
   photorealistic style.

9. **Loss is ours to handle, not Babylon's.** Researched 2026-09-29: Babylon's own restore after
   `device.lost` calls `initEngine()` without awaiting it (`abstractEngine.pure.js:253`), so its
   rebuild can run against the lost device, and it skips wrapped external textures (`:203`), which
   every view is. The adapter therefore passes `doNotHandleContextLost: true`, awaits
   `engine._device.lost` itself, adds an `uncapturederror` listener, reports `device-lost` to the
   status store, disposes the engine and every view's context, calls `requestAdapterOutcome` again
   for a fresh adapter (Design note 7), and recreates them through `loadRenderEngine`. The probe saw
   a GPU-process crash reach the page as `device.lost` with reason `unknown`, and no
   `uncapturederror`; a null adapter on the retry means Chromium has dropped WebGPU, which is the
   `disabled` condition with cause `adapter-withdrawn`, whatever the count. After
   `DEVICE_LOSS_LIMIT` (3) losses in a session the client stops recreating and shows `disabled`
   with cause `device-losses`, which matches the brainstorm's "three such losses disable WebGPU for
   the session" and Chromium's own count. On Linux the third GPU-process crash also makes the main
   process relaunch into safe mode (Design note 6); the relaunch replaces the page, so whichever
   the renderer showed first is gone a moment later.

10. **Faults are the console's annunciations, not alerts.** The brainstorm asks for a "ship-system
    fault in the guide's language"; the guide says alerts are raised by the server and "a console
    never invents one" (`docs/frontend/ux-guidelines.md`, Alerts). Researched 2026-09-29 by an
    advisor agent against the guide: the reading closest to both is a Fault the console raises about
    itself, as `SYSTEM DATA INVALID` already is, presented through `StatusLine`'s `fault` standing
    (`--status-caution` text, no tone, no flash, not in the header's alert counts), which the
    guide's "a failed system is being reported" allows. A lost device and a crashed GPU process are
    faults while they last, in `StatusLine`'s `fault` standing; a refused software adapter, no
    WebGPU or no adapter, the safe mode and the disabled state are the console stating its own
    condition in `--text`, in the `refused` standing (plain text), as the brainstorm's item 7 does
    for `TERRAIN: DETAIL LIMITED`. They are never promoted to a Caution. The wording, drafted in
    T5.c for the owner, is:
    - `GRAPHICS SOFTWARE ADAPTER: PHOTOREALISTIC STYLE UNAVAILABLE`;
    - `GRAPHICS NOT AVAILABLE: no WebGPU` (`no-webgpu`) and
      `GRAPHICS NO ADAPTER: views unavailable` (`no-adapter`);
    - `GRAPHICS DEVICE LOST: re-creating` and `GRAPHICS PROCESS RESTARTED` (faults);
    - `GRAPHICS SAFE MODE: views unavailable, relaunch to retry`;
    - `GRAPHICS DISABLED: <n> DEVICE LOSSES, relaunch to retry` (cause `device-losses`, n being the
      count) and `GRAPHICS DISABLED: adapter withdrawn, relaunch to retry` (cause
      `adapter-withdrawn`).

    They show in a `GRAPHICS` panel on the `LINK` display. The safe and disabled states are also
    proposed for the header strip's status area. The guide reserves that banner for "A simulation,
    training or replay mode" (`docs/frontend/ux-guidelines.md`, Layout), so the use is part of
    T5.c's draft for the owner, and T5.b builds it meanwhile, as the galaxy slice built to its
    drafts. Every later view carries the current annunciation in its label block (R02).

11. **Babylon options, fixed.**
    `new WebGPUEngine(canvas, { stencil: false, antialias: false, doNotHandleContextLost: true, useLargeWorldRendering: false, powerPreference: "high-performance" })`,
    then `engine.useReverseDepthBuffer = true`. Researched 2026-09-29 in 9.28.0:
    `useLargeWorldRendering` is a read-only constructor option, off by default
    (`abstractEngine.pure.d.ts:114`), and turning it on forces 64-bit matrices and a floating origin
    in every scene, so the adapter passes `false` explicitly and a test asserts the engine reports
    it off and that no scene sets `floatingOriginMode` (open question 1); `stencil: false` gives the
    main pass `depth32float` instead of `depth24plus-stencil8` (`webgpuEngine.pure.js:733`);
    `useReverseDepthBuffer` sets the comparison to greater-or-equal and the clear to 0; a camera
    with `maxZ = 0` gives the infinite reversed projection (`Maths/math.vector.pure.js:7303-7304`),
    but R02 supplies its own matrix and the adapter freezes it on the camera. The engine's own
    canvas is a 1 × 1 canvas never shown; the views draw into their own. `DepthPolicy` has one value
    because the brainstorm allows one.

12. **The WGSL-only guard, in three layers.** Researched 2026-09-29 in 9.28.0: GLSL reaches the
    engine at `_preparePipelineContextAsync` (`webgpuEngine.pure.js:1633-1637`), which fetches
    glslang and twgsl from `cdn.babylonjs.com` when first needed; the compile is handed only
    `(source, type)`, no name; and the effect's preparation is not awaited
    (`Materials/effect.functions.js:193`), so a throw there is an unhandled rejection that Babylon's
    error observables never see. So: (a) `initAsync` is given `{ glslang: Promise.resolve(stub) }`
    and `{ twgsl: stub }` whose compile methods throw, so no fetch can happen; (b) the adapter wraps
    the public `engine.createEffect`, and when the returned effect's `shaderLanguage` is GLSL it
    throws `GlslShaderRefused` naming `effect.name`; (c) a `window` `unhandledrejection` listener
    reports anything the first two miss. `ShaderMaterial` defaults to GLSL (`effect.pure.js:154`),
    so every material the adapter makes passes `shaderLanguage: ShaderLanguage.WGSL`. Babylon's
    standard, PBR, image-processing, bloom, FXAA, tone-map, glow, shadow-map, depth and pass shaders
    have WGSL versions; the standard and lens rendering pipelines, SSAO v1, screen-space
    reflections, refraction, velocity, sprite maps and GPU particles are GLSL-only and are not used.
    Because the stubs keep glslang and twgsl out, Babylon loads no WebAssembly and the CSP needs no
    change here; its `Function(...)`, `importScripts` and blob-worker paths are confined to Babylon
    Native, workers and texture codecs this plan does not import.

13. **One device, one canvas context per view, three internals pinned.** Researched 2026-09-29 in
    9.28.0: there is no public way to render a camera into an external canvas context. The route:
    each `RenderView` configures its own `GPUCanvasContext` against `engine._device` (declared `/**
@internal */ _device: GPUDevice`, `webgpuEngine.pure.d.ts:216`) at the view's own size, with
    `COPY_SRC` added to its usage so that the harness can read it back; wraps the first
    `getCurrentTexture()` with `engine.wrapWebGPUTexture` (`:805`); builds a `RenderTargetTexture`
    with that as its `colorAttachment` and `generateDepthBuffer: true`, which gives the view its own
    `depth32float` (`Engines/WebGPU/Extensions/engine.renderTarget.pure.js:46-54`); sets it as the
    camera's `outputRenderTarget` (`Cameras/camera.pure.d.ts:241`); sets `_disableEngineYFlip` on
    the target's wrapper, which lives on `WebGPURenderTargetWrapper`, not the engine
    (`Engines/WebGPU/webgpuRenderTargetWrapper.d.ts:20`); and each frame calls
    `updateWrappedWebGPUTexture` (`:822`) with the new current texture, which throws if the size
    changed, so a resize rebuilds only that view's target. These two internals, and the third that
    Design note 18 adds (`_hardwareTexture`, on `InternalTexture`, `internalTexture.d.ts:251`), are
    declared in the `.d.ts`, so a Babylon upgrade that renames them fails `pnpm typecheck`; one
    file, `view/engine/babylon/internals.ts`, holds all three accesses, each behind a one-line
    `oxlint-disable-next-line no-underscore-dangle` with its reason, because `.oxlintrc.json` bans
    dangling underscores. The harness's orientation check (T9.d) pins the behaviour, which a name
    check cannot. Open question 15 stays open: its public hook does not exist.

14. **Lazy loading.** Only `loadEngine.ts` imports the Babylon implementation, by a dynamic
    `import()`, which already splits it into its own chunk; the renderer config names the chunk with
    `build.rolldownOptions.output.codeSplitting = { groups: [{ name: "babylon", test: /node_modules[\\/]@babylonjs/ }] }`.
    Researched 2026-09-29: in Vite 8 `rollupOptions` is a deprecated alias of `rolldownOptions`, and
    rolldown ignores `manualChunks` once `codeSplitting` is set, so `manualChunks` is not used even
    though the brainstorm names it; the effect is the brainstorm's. Imports go to Babylon's `.pure`
    modules and the side-effect registrations they need, since `Engines/webgpuEngine.js` drags in
    the audio engine and loaders: about 1.1 MB minified, 270 KB gzipped, in a bare build (to be
    measured in the app's). `scripts/checkChunks.mjs` fails if the entry chunk contains Babylon code
    or if no `babylon` chunk exists.

15. **The dependency is pinned exactly.** `@babylonjs/core` at `9.28.0`, with no range, because the
    brainstorm's case for Babylon is that its visual changes are logged with a flag to restore the
    old look: an upgrade is a deliberate act that reads the breaking-changes log, sets any restoring
    flag, and runs `just test-render`. The pin's comment says so, as `libm`'s does.

16. **Subgroup twins are chosen by features, never by GPU.** `selectKernel` takes the subgroup
    variant only when `capabilities.subgroups` is true and the pair has one; the reference is the
    no-subgroup twin. A pair whose output is read back to the CPU is declared `bit-exact`, and the
    harness then runs both paths and compares bytes; a pair that differs in summation order is
    declared `presentation-only` and may never be read back: the engine remembers which kernel last
    wrote each buffer and texture, and `readBuffer` and `readTexture` throw
    `PresentationOnlyReadback` for one written by a `presentation-only` kernel.
    `assertNoF16Subgroups` rejects a module that enables both `f16` and `subgroups`, a coarse but
    mechanical form of the brainstorm's "subgroup operations never take f16 operands". On
    SwiftShader, which exposes subgroups but not `shader-f16`, and the UHD 620 with the Gen9 toggle,
    both paths exist to be tested. The UHD 620 does expose `shader-f16` under the forced switches
    (probes of 2026-09-29, R01's and R07's, on Electron 44.4.3, Mesa 26.2.3 ANV, the switch set of
    Design note 2); the "no `shader-f16`" list is SwiftShader's alone, so the f16 path is exercised
    by hand on the development machine and the no-f16 path automatically in the harness.

    The harness's checks were researched 2026-09-29 by a research agent against the WGSL
    specification (W3C editor's draft: §6.2.3, concrete integer overflow is modulo 2^bitwidth;
    §17.12.1, `subgroupAdd` is "the sum of e among all active invocations", in no stated order;
    §15.7, an implementation "may reassociate operations"; §15.5, the subgroup size is a power of
    two from 4 to 128, uniform only within a dispatch, with no defined relation to
    `local_invocation_index`) and Higham, _Accuracy and Stability of Numerical Algorithms_, 2nd ed.,
    §4.2 (high confidence on the specification, medium on SwiftShader's behaviour). A wrapping `u32`
    sum is associative, so its two paths agree in any order: a `bit-exact` integer pair proves the
    plumbing, not the hazard. So the catalogue holds two toy pairs (T10):
    - _`bit-exact`, `u32`._ The subgroup and reference bytes equal each other and a CPU
      `wrapping_add` total; the inputs make the sum wrap past 2³² at least once; one dispatch's
      invocation count is not a multiple of the subgroup size, so inactive lanes must add the
      identity; the kernel reads `subgroup_size` and hard-codes none.
    - _`presentation-only`, `f32`._ No byte comparison. Each path is within `highamBound`,
      γ(n − 1) · Σ|xᵢ| with γ(k) = ku ÷ (1 − ku) and u = 2⁻²⁴, of an `f64` CPU sum, over finite,
      normal inputs (WGSL may flush subnormals). Whether the two paths differed is logged, never
      asserted, since that depends on the backend. The engine's ordinary `readBuffer` refuses the
      result, which a negative case asserts; only the smoke page's `"tolerance"` access reads it.
      SwiftShader's f32 `subgroupAdd` being repeatable run to run carries to no other device, and no
      expectation is built on it.

17. **The harness runs headless, with more switches than the brainstorm lists.** Researched
    2026-09-29 by probe on Electron 44.4.3, a scratch harness run with each switch set in turn: the
    brainstorm's `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader` suffices under X11, but
    with no display the harness needs
    `--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan --use-vulkan=swiftshader`
    as well, and its window must be
    `BrowserWindow({ show: false, webPreferences: { offscreen: true } })`; headless Ozone without
    `offscreen` segfaults, Electron's `--headless` aborts, and Xvfb is not installed. So configured,
    30 runs of 30 passed at 2 to 3 s each on a loaded machine, with `DISPLAY` and `WAYLAND_DISPLAY`
    unset. SwiftShader reports `google`/`swiftshader`, `isFallbackAdapter` true, `subgroups`,
    `timestamp-query`, `float32-filterable` and `float32-blendable`, and no `shader-f16`, which
    confirms the brainstorm; the no-subgroup path needs no switch, only a device requested without
    the feature. Dawn treats only SwiftShader as a fallback (`BackendVk.cpp`), so lavapipe is never
    one, and Electron ships `libvk_swiftshader.so`. The harness is a plain Electron entry, not
    Playwright, which would add a dependency and a CDP layer for nothing needed here: a preload
    exposes one result call, the main process prints the JSON and exits with 0 (pass), 1 (a property
    failed), 2 (no adapter, a setup error) or 3 (watchdog). It loads a second renderer input,
    `smoke.html`, which imports the real `view/engine/` modules and never the bridge client, so no
    test switch can reach the client's main process; a test asserts that `graphicsSwitches` never
    returns the unsafe flag or the adapter override. Frames are read back from an offscreen
    `rgba16float` target, where a non-finite texel can exist, and from each view's canvas texture
    (configured with `COPY_SRC`, taken immediately before encoding, since it expires once the task
    yields). The network assertion is `session.defaultSession.webRequest.onBeforeRequest` over
    `<all_urls>`, allowing `file:` and `data:` only, cancelling and recording the rest: the probe
    saw it catch images, style sheets, WebSockets, fetches, beacons and the main process's
    `net.fetch`, while `enableNetworkEmulation({ offline: true })` blocked nothing and the CSP
    blocks silently. Node's own `fetch` bypasses Chromium, so the harness's main process makes no
    request itself.

18. **What the sibling plans asked of the adapter, and how it is met.** Checked 2026-09-29 against
    R02, R05, R06 and R12 as written.
    - _R02's projection is used as given._ Every material is our own WGSL, so R02's view rotation
      and projection reach the shader as uniforms and never pass through Babylon's camera: no half-Z
      conversion, and no Y flip, since each view's target sets `_disableEngineYFlip`. The scene is
      right-handed (`scene.useRightHandedSystem = true`) so that front faces wind as R02's matrices
      expect, and `cullMode` is explicit per material. Every mesh has
      `alwaysSelectAsActiveMesh = true`: Babylon never frustum-culls, since culling is R02's and
      R05's. The harness checks all three (T9.f).
    - _Depth bias per material, not global._ `depthBiasAway` is passed unchanged, as positive
      values, to the material's `zOffset` (slope scale) and `zOffsetUnits` (constant). Babylon
      itself negates both when `useReverseDepthBuffer` is set (`abstractEngine.pure.js`
      lines 523, 531, 538 and 546 in 9.28.0), so positive always moves a fragment away; the adapter
      adds no flip of its own, which a T8.c test asserts on the values handed to Babylon. The
      brainstorm's "no global depth bias" stands: this is a per-material setting R02 uses for its
      lines, not a scene-wide one.
    - _Linear-light blending._ Each canvas is configured with its preferred 8-bit format and
      `viewFormats` holding its `-srgb` twin, and the view renders through the sRGB view, so
      blending happens in linear light and the store encodes. If Babylon's `wrapWebGPUTexture`
      cannot take a view format, the fallback is an `rgba16float` offscreen target
      (`createRenderTarget`, the path R07's HDR pipeline uses) with an explicit encode in the final
      pass, recorded as a deviation. R02's wireframe itself has no HDR target (R02 Design note 12).
    - _R06's packed cube._ `createPackedCube` creates the cube through Babylon (`RawCubeTexture`,
      `TEXTURETYPE_UNSIGNED_INT_5_9_9_9_REV`, null data, mips allocated), and `writePackedCubeLevel`
      writes each level with `copyBufferToTexture` on the cube's GPU texture, reached through
      `_hardwareTexture`. That is a third pinned internal, in `internals.ts` beside the other two,
      pinned by typecheck and by a harness round trip.
    - _Resources, one path._ Buffers and textures are created only through `createBuffer`,
      `createTexture` and `createPackedCube`, each with a `MemoryCategory`, and every creation,
      destruction and upload (`writeBuffer`, `writeTexture`) raises an `AllocationEvent` with its
      byte size from `textureBytes`. R05 builds its tally on those events and R12 itemises them; the
      boundary test (Design note 1) fails on `device.createBuffer` or `device.createTexture` outside
      `view/engine/babylon/`. `MemoryCategory` starts with `render-targets` and `other`; later plans
      add their members in the same file.
    - _Variants per setting._ `just test-render` runs a matrix of settings × capability paths. R01
      has one setting, `default`; a catalogue entry may declare the settings it renders at, and
      R12.T7.a adds the ladder's.

19. **Offscreen targets, asynchronous pipelines, indirect work and per-pass time.** Asked by R07
    and R11 at review (R07's HDR target and bloom chain, R11's stills and GPU-culled scatter, R05's,
    R07's, R11's and R12's per-pass benchmarks). Checked 2026-09-29 in `@babylonjs/core` 9.28.0:
    - _Offscreen targets._ `createRenderTarget` builds a Babylon `RenderTargetTexture` of the given
      format (`rgba16float`, `rg11b10ufloat` or `rgba8unorm`) with `generateDepthBuffer` when
      `depth` is set, its own camera frozen to the submission's matrices, as a view's is, and
      `_disableEngineYFlip` on its wrapper. Its colour is a `TextureHandle` that later draws and
      post-processes sample, and its bytes are raised as `render-targets` allocations. It renders
      into level 0; a chain of levels, such as R07's bloom, is one target per level. Whether
      Babylon's generated depth texture carries `COPY_SRC` for `readTexture` is confirmed in T8.f;
      if it does not, the target creates its own `depth32float` through `createTexture` with
      `COPY_SRC` and hands it to the render target wrapper.
    - _Asynchronous pipelines._ `engine.createRenderPipelineAsync(options)` is public and pre-warms
      Babylon's pipeline cache without a draw (`webgpuEngine.pure.d.ts:1090`, over
      `device.createRenderPipelineAsync`, `WebGPU/webgpuCacheRenderPipeline.js:985`), so
      `createMaterialAsync` resolves once the effect is compiled and the pipeline for every named
      target format is cached, and no frame blocks on a compile, which R11's stills need against
      Chromium's GPU watchdog (R11 Design note 17). `createComputeAsync` does the same for a compute
      shader through `device.createComputePipelineAsync`.
    - _Indirect work._ A compute kernel's dispatch takes GPU-written counts through the public
      `ComputeShader.dispatchIndirect(buffer, offset)` (`Compute/computeShader.pure.d.ts:181`).
      Babylon's own indirect draw path fills its indirect buffer from CPU counts
      (`setIndirectData`, `webgpuEngine.pure.js:2961-2967`), so a `DrawItem` with `indirect` is not
      drawn by Babylon: the adapter encodes it itself, in a render pass on `engine._device` over the
      same view or target attachments with `loadOp: "load"`, right after Babylon's pass for that
      target, through a pipeline built from the same WGSL with `device.createRenderPipelineAsync`.
      That raw pass uses only the pinned `_device` and the target's GPU textures, reached through
      the pinned `_hardwareTexture`; it adds no internal.
    - _Per-pass time._ Babylon measures only the main pass (`gpuTimeInFrameForMainPass`,
      `thinWebGPUEngine.d.ts:53-57`) and each compute shader (`gpuTimeInFrame`,
      `computeShader.pure.d.ts:89`), which is not enough. So, where `timestamp-query` is present,
      the adapter owns one `GPUQuerySet`: every pass it encodes itself (raw indirect passes,
      compute dispatches) carries `timestampWrites`, and each Babylon-encoded pass for a view,
      target or post-process chain is bracketed by two empty compute passes with `timestampWrites`
      submitted just before and just after it, marked `bracketed: true` because the bracket also
      holds queue gaps. Queries resolve into a mappable buffer read a frame or more later, and
      `onPassTimes` reports each frame's passes by the `label` of their `FrameSubmission` or
      dispatch, with `GraphicsStatus.timer`. Without the feature it never fires.

20. **CPU readback is one guarded path.** `readBuffer` and `readTexture` copy through
    `copyBufferToBuffer` or `copyTextureToBuffer` into a `MAP_READ` staging buffer and map it; they
    are what R01's harness, T10's kernels and any later readback use. `RenderView.readBack` stays
    the harness's copy of a canvas texture. Each resource records the kernel that last wrote it, so
    the presentation-only refusal of Design note 16 holds at the one place a result reaches the CPU.

21. **Material state, instancing, storage buffers, texture writes and R06's bake passes.** Asked by
    R02 (its Design notes 5, 9 and 12), R05 (Consumes; T11, T12.c) and R06 (Design note 21) in the
    reconciliation of 2026-09-29, and met here by the names they proposed. Checked in
    `@babylonjs/core` 9.28.0:
    - _Material state._ `depthWrite` and `colourWrites` map to the public
      `Material.disableDepthWrite` and `Material.disableColorWrite`
      (`Materials/material.pure.d.ts:471, 475`); `blend: "additive"` is
      `alphaMode = Constants.ALPHA_ADD` (`Engines/constants.d.ts:10`), blending in linear light
      through the sRGB view of Design note 18.
    - _Instancing._ `MeshSpec.instanceAttributes` and `DrawItem.instanceCount` go through Babylon's
      thin instances (`thinInstanceSetBuffer`, `Meshes/thinInstanceMesh.types.d.ts:63`), so that
      `@builtin(instance_index)` counts them; an indirect draw's instance count is the GPU's, not
      this field (Design note 19).
    - _Storage buffers on materials._ `ShaderMaterial.setStorageBuffer`
      (`shaderMaterial.pure.d.ts:365`) binds each `DrawItem.storageBuffers` entry by its
      `StorageBufferSpec` name, read-only in the vertex and fragment stages.
    - _Writes._ `writeTexture` is `device.queue.writeTexture` on the texture reached through
      `_hardwareTexture`; it and `writeBuffer` each raise an `uploaded` `AllocationEvent`, so R05's
      tally sees every upload.
    - _Post-process inputs._ `WgslPostProcessSpec.inputs` binds the view's or target's own
      `depth32float` and HDR colour as sampled textures, with `samplers` as materials have them.
    - _A target's depth in a draw._ A `RenderTarget.depth` handle in `DrawItem.textures` is bound
      to the material through `ShaderMaterial.setInternalTexture`
      (`Materials/shaderMaterial.pure.d.ts:185`). Babylon's WGSL processor gives a binding the
      `depth` sample type when its declaration is a `texture_depth_*` type
      (`Engines/WebGPU/webgpuShaderProcessorsWGSL.pure.js:191-194`), so the shader declares
      `texture_depth_2d` and reads it with `textureLoad`, unfiltered. This is what R05's deferred
      aerial-perspective pass needs: a full-screen draw that reads the terrain target's reversed-Z
      depth beside its colour and a 3D table, which a post-process cannot do, since it binds no
      textures of its own. WebGPU forbids a texture as attachment and binding in one pass, so the
      adapter refuses such a draw into the same target with `DepthSelfSample`, naming both.
    - _Blending keeps the meter class._ R07 keeps a meter class in the HDR target's alpha (its
      Design note 10), so every `blend` mode leaves the destination alpha as it is. `"additive"`
      is Babylon's `ALPHA_ADD`, whose factors are source alpha and one for colour, zero and one
      for alpha (`States/alphaCullingState.js:186-187`). The shader therefore writes α = 1, or
      its coverage. `"premultiplied"` is colour (one, one minus source alpha) with alpha (zero,
      one), for R08's and R11's translucent layers. No Babylon preset has those factors: its
      premultiplied modes 7 and 8 change the alpha. So the adapter sets mode 7 on the material and,
      in its `onBind`, overrides the factors through the public `AbstractEngine.alphaState`
      (`Engines/abstractEngine.pure.d.ts:419`) and `AlphaState.setAlphaBlendFunctionParameters`
      (`States/alphaCullingState.d.ts:28`). It then calls `setAlphaEquation`, whose WebGPU
      override passes the factors to the pipeline cache
      (`Engines/WebGPU/Extensions/engine.alpha.pure.js`). `Mesh.render` sets a material's alpha
      mode before its bind (`Meshes/mesh.pure.js:2186-2197`), so the override comes after it, and
      only this mode uses 7. `transparent` only places the draw in Babylon's transparent queue.
      The harness checks all three modes in T9.i.
    - _R06's bake._ `writePackedCubeLevelFromBuffer` is `copyBufferToTexture` from the kernel's
      buffer onto the cube's GPU texture, beside `writePackedCubeLevel`, which stays for small CPU
      writes and the harness. `createPointSplat` is a `point-list` pipeline with additive blending
      into a 2D `rgba32float` target made with `createTexture` (`RENDER_ATTACHMENT` usage), and it
      throws `Float32BlendUnavailable` when `float32-blendable` is absent, which R06 answers with a
      compute splat. The brainstorm keeps `rgba32float` out of colour targets because it "would
      double the bandwidth of a bandwidth-bound GPU for no visible gain" (Luminance in physical
      units). A splat target is a per-arrival bake scratch, held one face at a time and never
      presented or drawn into each frame, so that reason does not reach it. `ColourTargetFormat`
      and `RenderTargetSpec` still exclude `rgba32float`, and the README should say so beside the
      brainstorm's Decision ("Light").

22. **How the adapter rounds a colour write is probed, not assumed.** Asked by R07 (its Design
    note 12). R07's probe of 2026-09-29 found that Gen9's colour-attachment writes round toward
    zero, a bias of −0.78% to −1.56% a write in `rg11b10ufloat` and −0.05% in `rgba16float`.
    R07's bloom keeps `rgba16float` unless the adapter rounds to nearest. So once the device
    exists, `roundingProbe.ts` draws one full-screen triangle into a 4 × 1 target of each
    format, `rg11b10ufloat` only where `rg11b10Renderable` holds. It writes constants that sit
    three quarters of the way between neighbouring values of the format: 1 + 0.75 × 2⁻¹⁰ for
    `rgba16float` (10 mantissa bits); 1 + 0.75 × 2⁻⁶ in red and green and 1 + 0.75 × 2⁻⁵ in blue
    for `rg11b10ufloat` (6 and 5 mantissa bits). It reads them back with `readTexture`. The upper
    neighbour means `nearest`, the lower means `toward-zero`, and anything else, a failed read or
    an absent format is `unknown`. The status store holds the result as
    `GraphicsStatus.targetRounding`, dispatched once and again after each rebuild. The draw costs
    one pass and a 16-byte read, off the frame path, before the first view renders.

## Tasks

T1 and T2 are the main process and can run in parallel. T3 needs nothing. T4 follows T2 (its
preload fields) and T3; T5 follows T4. T6 (the interface, the memory types and kernel selection)
follows T3 and T4; T7 (dependency and chunk) follows T6; T8 (the Babylon engine) follows T4 and T7,
its subtasks in order; T9 (harness) follows T8; T10 (the toy kernel pairs)
follows T9. T11–T13 are by hand, after T9. T14 closes.

Paths are under `apps/hyperion/` unless given in full.

### R01.T1 The Linux switches

**R01.T1.a The switch builder.** `src/main/graphics/switches.ts`: `graphicsSwitches`,
`mergeSwitchValue`, `applyGraphicsSwitches`, `launchModeOf`, the switch-name constants. Only
`vulkan` mode on Linux returns switches: the set of Design note 2, plus
`disable-dawn-features=timestamp_quantization` when `gpuTiming` is on (Design note 4). `safe` mode
returns an empty list: no Vulkan features, no `use-angle=vulkan`, no Dawn subgroup toggle (nothing
draws with WebGPU in safe mode) and no timing toggle. `default` mode, every platform but Linux,
returns an empty list.

- Tests (`switches.test.ts`, logic project): the Linux `vulkan` set exactly, in order; `safe` and
  `default` give the empty list, with and without `gpuTiming`; no platform or mode ever yields
  `enable-unsafe-webgpu` or `use-webgpu-adapter`; `launchModeOf` gives `default` off Linux whatever
  the switch, and `safe` on Linux only with it; timing adds only the Dawn toggle;
  `mergeSwitchValue("A,B", ["B","C"])` is `"A,B,C"`, empty existing gives the added list, duplicates
  are dropped; `applyGraphicsSwitches` on a fake `CommandLine` holding `enable-features=Foo` appends
  one `enable-features=Foo,Vulkan,…`.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/graphics/switches.test.ts`.

**R01.T1.b Applying them, and the X11 relaunch.** `src/main/graphics/x11Relaunch.ts`:
`x11RelaunchArgs`. In `src/main/index.ts`, before `whenReady`: `x11RelaunchArgs` over
`process.argv.slice(1)` (Design note 3) and, when it answers, `app.relaunch({ args })` and
`app.exit(0)`; then `app.disableDomainBlockingFor3DAPIs()` and `applyGraphicsSwitches` for the
launch's mode (`launchModeOf(process.platform, app.commandLine.hasSwitch(SAFE_MODE_SWITCH))`) and
timing flag. The `justfile`'s `client` recipe adds `--ozone-platform=x11` itself when
`XDG_SESSION_TYPE` is `wayland`, since a relaunch ends `electron-vite dev` (Design note 3), and its
comment says that `--hyperion-gpu-timing` is given like any other client option,
`just client --hyperion-gpu-timing` (electron-vite passes the arguments after the recipe's own
`--` to Electron without the separator, `dist/chunks/lib-6EHSwoSb.js:228-242`, so Chromium sees
them as switches).

- Tests (`x11Relaunch.test.ts`): Linux with `XDG_SESSION_TYPE=wayland` and no flag gives the args
  plus `--ozone-platform=x11`, with no executable path at their head; with the flag already present,
  under `x11` or unset, and on other platforms, `undefined`; the relaunch never duplicates the flag.
- By hand, recorded in this plan's as-built notes: on the development machine, `just client`, then
  `navigator.gpu.requestAdapter()` in the devtools console gives `intel`/`gen-9` with `subgroups`;
  `chrome://gpu` is not reachable in the client, so the check is the adapter and
  `app.getGPUFeatureStatus()` logged after `gpu-info-update`.
- Acceptance: the test file passes, and `just ci`.

### R01.T2 GPU-process monitoring and the safe mode

**R01.T2.a The crash-loop policy.** `src/main/graphics/crashLoop.ts`: `GpuProcessEvent`,
`crashLoopDecision`, the two constants (Design note 6), and `src/main/graphics/featureStatus.ts`:
`readFeatureStatus`. Pure.

- Tests: two crashes within the window give `none`, three give `relaunch-safe`; three spread over
  more than the window give `none`; `clean-exit` never counts; in `vulkan` mode a status event with
  `vulkan` `enabled_on` then one with `vulkan` other than `enabled_on` gives `relaunch-safe`, and so
  does `webgpu` going from `enabled` to anything else; a first status that never held them gives
  `none`; nothing ever relaunches in `safe` or `default` mode, three crashes and a Windows-like
  `vulkan: "disabled_off"` status included; the decision depends on the events and not on their
  arrival order within a timestamp; `readFeatureStatus` reads the two keys from a record, gives
  `undefined` for a missing or non-string one, and never casts.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/graphics/crashLoop.test.ts src/main/graphics/featureStatus.test.ts`.

**R01.T2.b The monitor, the relaunch and the preload.** `GpuProcessMonitor` subscribes to
`child-process-gone` (GPU only) and `gpu-info-update` (reading `getGPUFeatureStatus()` then), keeps
the history, sends each crash to every window's `webContents` on one channel, and on `relaunch-safe`
calls `app.relaunch({ args: [...process.argv.slice(1), "--hyperion-graphics-safe"] })` and
`app.exit(0)`. The main process adds the launch mode and the timing flag to the window's
`additionalArguments` beside the server URL. The preload gains `graphics.launchMode` and
`graphics.gpuTiming` (from its argv, as `serverUrl` is) and `graphics.onGpuProcessGone`, a narrow
function over one fixed channel, under the Electron security rules: no channel name crosses the
bridge. `stubHyperionApi.ts` gains the fields.

- Tests: the monitor against a fake `app` and `webContents` (logic project): events in, sends and
  the one relaunch out, with no executable path in its args; the preload's argv parse (`launchMode`
  is `safe` only with the switch, `default` off Linux; `gpuTiming` only with its switch).
- By hand, recorded, on a built client (`pnpm --filter hyperion build`, then
  `pnpm --filter hyperion exec electron .`), since a relaunch ends `electron-vite dev`
  (Design note 3): `kill -9` of the GPU process three times within a minute relaunches once into
  safe mode, whose adapter request is null and whose consoles work; a fourth crash does not
  relaunch again.
- Acceptance: the tests pass, and `just ci`.

### R01.T3 Adapter acquisition

`src/renderer/src/view/engine/platform.ts` and `src/renderer/src/test/fakeGpu.ts`:
`summariseAdapter`, `styleAvailability`, `requestAdapterOutcome` (with
`powerPreference: "high-performance"`, which the probe found changes nothing on one GPU but is right
on two), `WANTED_FEATURES`, `requiredFeatures`, `CapabilityOverrides` (used only by the harness
page). No engine import. `FakeAdapter` is consumed by its first `requestDevice`, and a second call
gives a device whose `lost` has already resolved.

- Tests (logic project, with `FakeGpu`): no `navigator.gpu` gives `no-webgpu`; a null adapter
  `no-adapter`; the Intel probe's info (`intel`, `gen-9`, `isFallbackAdapter` false) gives both
  styles; SwiftShader's (`google`, `swiftshader`, true) gives wireframe only, and so does either
  signal alone; capabilities read each feature and `subgroupMinSize`; `requiredFeatures` is the
  intersection with the adapter's features, less what the overrides withhold.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/engine/platform.test.ts`.

### R01.T4 Graphics status and device loss

`view/engine/status.ts`: `GraphicsStatus`, `GraphicsEvent`, `reduceGraphicsStatus`,
`graphicsAnnunciation` (the strings of Design note 10), `GraphicsStatusStore` and
`useGraphicsStatus` (over `useSyncExternalStore`), and the store's feed from
`window.hyperion.graphics` (`launchMode`, `gpuTiming` into `timer`, and the crash reports).

- Tests (`status.test.ts`, logic project): each adapter outcome's condition, `no-webgpu` and
  `no-adapter` included; safe launch mode gives `safe-mode` whatever the adapter; `timer` is
  `absent` without `timestamp-query`, else `full` only with `gpuTiming`; a device loss sets the
  fault and counts, a restore clears the fault and keeps the count; the third loss gives `disabled`
  with cause `device-losses` and later restores do not leave it; an adapter withdrawn on a rebuild
  after one loss gives `disabled` with cause `adapter-withdrawn`; a GPU-process report sets its
  fault with the count; each annunciation's text and standing, a `StatusStanding`.
- Tests (`useGraphicsStatus.test.tsx`, dom project, since `renderHook` needs a DOM): the hook
  re-renders on dispatch and unsubscribes on unmount.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/engine/status.test.ts src/renderer/src/view/engine/useGraphicsStatus.test.tsx`.

### R01.T5 The `GRAPHICS` panel and its words

**R01.T5.a The panel.** `components/GraphicsPanel.tsx` on the `LINK` display after `Server Link`, in
`ConnectionPanel`'s readout pattern: `Adapter` (vendor · architecture), `Software Adapter`
(`YES`/`NO`), `Features` (the wanted ones present), `Styles`, `Mode` (`DEFAULT`, `VULKAN` or
`SAFE`), `GPU Timer` (`QUANTIZED`, `FULL` or `ABSENT`, from `GraphicsStatus.timer`), `Device
Losses`, `Process Restarts`, and the current annunciation through `StatusLine`, with the standing
`graphicsAnnunciation` gives. Missing values are the em dash.

- Tests (dom project): each condition's rows and annunciation, found by role and text; no status
  colour on a nominal panel; a fault's text carries `request-status__text--fault`, `StatusLine`'s
  fault class, and a plain statement does not.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/components/GraphicsPanel.test.tsx`;
  the console-ux skill's lint, contrast and glyph scripts pass on the changed files.

**R01.T5.b The mode in the header strip.** `ConsoleFrame` shows the `GRAPHICS SAFE MODE` or
`GRAPHICS DISABLED` annunciation in its status area while it holds, beside the link status, never in
the alert counts.

- Tests: present in safe mode and in either `disabled` cause, absent otherwise, and identical on
  every display.
- By eye, recorded in the as-built notes: the `GRAPHICS` panel in each condition and the header
  banner in safe mode, on `just client` with the store driven from the devtools console, beside the
  guide's banner rule.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/components/ConsoleFrame.test.tsx`;
  the by-eye record.

**R01.T5.c Nomenclature drafts, for the owner.** A draft of the guide's new entries: `GRAPHICS`
(system); the statuses `GRAPHICS SOFTWARE ADAPTER`, `GRAPHICS NOT AVAILABLE`, `GRAPHICS NO ADAPTER`,
`GRAPHICS SAFE MODE`, `GRAPHICS DISABLED` (with its two causes); the faults `GRAPHICS DEVICE LOST`
and `GRAPHICS PROCESS RESTARTED`; `UNAVAILABLE` (the owner may prefer `NOT AVAILABLE`); `DEFAULT`,
`VULKAN`, `SAFE`, `QUANTIZED`, `FULL`, `ABSENT`; one sentence under Alerts that a console's report
on its own graphics is a Fault or status, not an alert; and one sentence under Layout extending the
header-strip banner, today reserved for "A simulation, training or replay mode", to the graphics
safe and disabled modes (Design note 10). `PHOTOREALISTIC` and `WIREFRAME` wait for R02's view
class. Written as a proposed diff in the commit message and in this plan's as-built notes, not
applied: guide additions are the owner's call. R02's single pass absorbs and re-checks it. The code
does not wait for the answer, since every string is one constant in `status.ts`.

- Acceptance: the owner signs off, or amends; the constants then change in one commit.

### R01.T6 The engine-agnostic interface

`view/engine/types.ts`; `view/engine/memory.ts`'s types (`MemoryCategory`, `BufferSpec`,
`TextureSpec`, `AllocationEvent`; `textureBytes` is T8.d's); `view/engine/viewSize.ts`
(`viewPixelSize`); `view/engine/kernels.ts` in full, since selection is pure (`KernelPair`,
`selectKernel`, `assertNoF16Subgroups`, `highamBound`, Design note 16); `view/engine/catalogue.ts`
(an empty `WGSL_CATALOGUE` of `{ kind: "material" | "post-process" | "compute", spec }`); and
`view/engine/engineBoundary.test.ts` (Design note 1).

- Tests: the boundary test finds every `.ts`/`.tsx` under `src/renderer/src` except itself, and
  fails on an `@babylonjs/` import outside `view/engine/babylon/`, on the text `registerView`
  anywhere, and on `readBuffer`'s `"tolerance"` access outside the smoke page; it passes on the
  tree, and fails on a fixture string that breaks each rule, the fixtures assembled at run time so
  that the test's own source never matches. `selectKernel` for each combination of feature and
  variant, a pair without a subgroup variant always taking the reference; a module with
  `enable f16;` and `enable subgroups;` throws with its name; `highamBound` against a hand-computed
  value; `viewPixelSize` rounds, clamps to the limit and never returns zero.
- Acceptance: `pnpm typecheck` and
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/engine/`.

### R01.T7 The dependency and the lazy chunk

`pnpm add --filter hyperion @babylonjs/core@9.28.0 --save-exact`, with the pin's reason (Design
note 15) in the module comment of `view/engine/babylon/engine.ts`, since `package.json` holds no
comments; `view/engine/loadEngine.ts` with the dynamic import and a placeholder `babylon/engine.ts`
that exports `createBabylonEngine`; the `codeSplitting` group in `electron.vite.config.mts` (Design
note 14); `scripts/checkChunks.mjs` over `out/renderer`.

- Tests: `loadEngine.test.ts` passes a fake `importEngine` through `LoadEngineOptions`, never a
  `vi.mock` of our own module (`.claude/rules/typescript-dev.md`, Tests), and checks that nothing is
  imported until `loadRenderEngine` is called and that the overrides reach `createBabylonEngine`.
- Acceptance: `pnpm build` then `node apps/hyperion/scripts/checkChunks.mjs` passes; the entry chunk
  holds no `@babylonjs` module; `just ci`.

### R01.T8 The Babylon engine

**R01.T8.a Creation.** `view/engine/babylon/engine.ts`: the per-creation adapter wrapper on
`navigator.gpu.requestAdapter` (Design note 7), the fixed options (Design note 11), the feature
check after `initAsync`, `capabilities` from the device's features, and `RenderEngine`'s
`createMesh` (with thin-instance attributes), `createMaterial` (WGSL `ShaderMaterial` only, with
`depthWrite`, `colourWrites`, `blend` and storage buffers, Design note 21), `createPostProcess`
(with its `inputs` and `samplers`) and `createCompute` (the variant from `selectKernel`) over
Babylon's WGSL paths.

- Tests (logic project, Babylon's `NullEngine` is not WebGPU, so these run on the pure parts): the
  requested features are `requiredFeatures`' answer; a feature asked and not enabled is reported;
  the wrapper returns the adapter vetted for that creation and is removed once `initAsync` settles;
  the options object has `useLargeWorldRendering: false`, `stencil: false` and
  `doNotHandleContextLost: true`; each material flag reaches Babylon's own property
  (`disableDepthWrite`, `disableColorWrite`, `alphaMode`). The rest is exercised by T9.
- Acceptance: the tests, `pnpm typecheck`, `just ci`.

**R01.T8.b The WGSL-only guard.** `view/engine/babylon/wgslGuard.ts`: the glslang promise stub and
twgsl stub, the `createEffect` wrapper throwing `GlslShaderRefused` with the effect's name, and the
`unhandledrejection` backstop (Design note 12), all reporting to `console.error` and the status
store.

- Tests: the stubs throw with a message naming the rule; the wrapper passes a WGSL effect through
  and throws on a GLSL one with its name; the backstop reports a `GlslShaderRefused` and ignores
  other rejections.
- Acceptance: the tests; T9's offline run is the integration check.

**R01.T8.c One canvas context per view.** `view/engine/babylon/view.ts` and `internals.ts` (Design
note 13): `RenderView` with its own context, target, depth and camera; `resize` rebuilds only its
own target; `render` updates the wrapped texture and renders its camera with R02's frozen matrices
(an identity rotation and a fixed reversed-Z projection until R02 exists); `readBack` copies the
current texture with `copyTextureToBuffer` and maps it. Canvas sizes come from T6's
`viewPixelSize`.

The canvas is configured with its `-srgb` view format, and the right-handed scene, explicit cull
mode, per-material depth bias and `alwaysSelectAsActiveMesh` of Design note 18 apply.

- Tests: `depthBiasAway` reaches Babylon's `zOffset` and `zOffsetUnits` as the same positive
  values, with no flip by the adapter (Babylon negates them under reversed depth, Design note 18);
  `internals.ts` type-checks against the pinned `.d.ts` (a typecheck failure is the pin's alarm).
- Acceptance: `pnpm typecheck`, the tests, `just ci`.

**R01.T8.d Resources and the packed cube.** `view/engine/memory.ts`'s `textureBytes`, over T6's
types, and the adapter's `createBuffer`, `createTexture`, `writeBuffer`, `writeTexture`, `dispatch`,
per-draw textures and storage buffers in `DrawItem`, and `createPackedCube`, `writePackedCubeLevel`
and `writePackedCubeLevelFromBuffer` through `_hardwareTexture` in `internals.ts` (Design notes 18
and 21). The boundary test gains the rule on `device.createBuffer` and `device.createTexture`.

- Tests: `textureBytes` for 2D, 3D, cube, mip chains and `rgb9e5ufloat` against hand-computed sizes;
  every create and destroy raises one event with its category; every `writeBuffer` and
  `writeTexture` raises one `uploaded` event with its bytes; the boundary rule fails on a fixture.
- Acceptance: the tests; T9.g's cube round trips.

**R01.T8.e Loss and rebuild.** In `view/engine/babylon/engine.ts` (Design note 9): the awaited
`engine._device.lost` through `internals.ts`, the `uncapturederror` listener, the report to the
status store, disposal of the engine and every view's context and target, a fresh
`requestAdapterOutcome` and its vetting, and the rebuild through `loadRenderEngine`; a null adapter
reported as `adapter-withdrawn`; no rebuild after `DEVICE_LOSS_LIMIT`.

- Tests (logic project, with `FakeGpu` and a fake engine module through `importEngine`): a loss
  requests a new adapter and never reuses the consumed one; a null adapter on the retry dispatches
  `adapter-withdrawn`; the third loss stops rebuilding; views are re-created at their sizes; a
  `lost` that follows the engine's own `dispose` is not reported.
- Acceptance: the tests, `just ci`; T9.f's forced loss (`device.destroy()`) is the integration
  check.

**R01.T8.f Offscreen targets and readback.** `view/engine/babylon/target.ts` and
`view/engine/babylon/readback.ts` (Design notes 19 and 20): `createRenderTarget` over a
`RenderTargetTexture` with its own frozen camera and depth; `readBuffer` and `readTexture` through
a `MAP_READ` staging buffer; the last-writer record per resource and the `PresentationOnlyReadback`
refusal, lifted only by the `"tolerance"` access. Confirm whether Babylon's generated depth texture
carries `COPY_SRC`, and if not create the target's own `depth32float` (Design note 19), recording
which in the as-built notes.

- Tests (logic project, on the pure parts): the last-writer record and the refusal; a target's bytes
  raised as `render-targets` allocations; the staging-buffer size for each format and a padded
  `bytesPerRow` (256-byte aligned).
- Acceptance: the tests, `just ci`; T9.h renders into a target and reads its colour and depth back.

**R01.T8.g Asynchronous pipelines, indirect work and pass timing.** `view/engine/babylon/async.ts`,
`view/engine/babylon/rawPass.ts` and `view/engine/babylon/timing.ts` (Design note 19):
`createMaterialAsync` over `engine.createRenderPipelineAsync` and `createComputeAsync` over
`device.createComputePipelineAsync`; `dispatch` with `IndirectArgs` through
`ComputeShader.dispatchIndirect`; `DrawItem.indirect` drawn in the adapter's own raw pass after
Babylon's pass for that target; one `GPUQuerySet`, `timestampWrites` on the adapter's own passes,
bracketing compute passes around each Babylon-encoded pass, and `onPassTimes` by label.

- Tests (logic project, with `FakeDevice`): query indices are allocated per labelled pass and
  resolved in order; a bracketed pass is marked; without `timestamp-query` no query set is created
  and `onPassTimes` never fires; an indirect draw is routed to the raw pass and never to Babylon's
  mesh path.
- Acceptance: the tests, `just ci`; T9.h's checks.

**R01.T8.h R06's point splat.** `view/engine/babylon/pointSplat.ts` (Design note 21):
`createPointSplat` over a raw `point-list` pipeline with additive blending into an `rgba32float`
texture made with `createTexture`, encoded like Design note 19's raw passes, and
`Float32BlendUnavailable` without `float32-blendable`.

- Tests (logic project, with `FakeDevice`): the pipeline descriptor's topology, format and blend
  state; the throw without the feature; the target's bytes raised as an allocation.
- Acceptance: the tests, `just ci`; T9.i's splat round trip.

**R01.T8.i A target's depth in a draw, and blending that keeps alpha.** In
`view/engine/babylon/engine.ts` and `target.ts` (Design note 21): a `RenderTarget.depth` in
`DrawItem.textures` bound through `setInternalTexture` as `texture_depth_2d`, and each target's own
depth made with `TEXTURE_BINDING`; the `DepthSelfSample` refusal; `blend: "premultiplied"` as mode
7 with its factors overridden in the material's `onBind`, and `"additive"` as `ALPHA_ADD`. Asked by
R05 (the depth) and R07 (the alpha, for R06's, R08's and R11's blended passes).

- Tests (logic project, on the pure parts): the refusal names the target and the material; the
  blend table maps each mode to its colour and alpha factors, alpha always (zero, one).
- Acceptance: the tests, `just ci`; T9.i's checks.

**R01.T8.j The rounding probe.** `view/engine/babylon/roundingProbe.ts` (Design note 22): the probe
draw and read after device creation and after each rebuild, its classification, and the
`targetRounding` event to the status store. Asked by R07 (its Design note 12).

- Tests (logic project): the classifier on synthetic read-backs returns `nearest` for the upper
  neighbour, `toward-zero` for the lower and `unknown` otherwise; `rg11b10ufloat` is `unknown`
  without `rg11b10Renderable`; a failed read gives `unknown` and no fault.
- Acceptance: the tests, `just ci`; T9.i reports SwiftShader's answer, and T11 records the UHD
  620's.

### R01.T9 The headless smoke harness

**R01.T9.a The runner and the readback.** `src/smoke/main.ts` and `src/smoke/preload.ts` (an
Electron entry with its own one-call preload), `src/renderer/smoke.html` with its script (a second
renderer input in `electron.vite.config.mts`, beside `index.html`), and `just test-render`, which
runs `pnpm --filter hyperion build`, then the harness under the heavy-test lock with its switches as
an array (Design note 17), once per variant named in the page's query string. The page acquires the
adapter through `platform.ts`, loads the engine through `loadEngine.ts`, renders one frame of a
clear and a triangle into an offscreen `rgba16float` target (`createRenderTarget`) and into a
view, reads both back (`readTexture`, `RenderView.readBack`), and
asserts properties: every texel finite, the clear colour where nothing was drawn, the triangle's
colour at its centroid. The runner prints the adapter summary and one line a check, and exits with
Design note 17's codes.

- Acceptance: `just test-render` passes on the development machine with `DISPLAY` unset; it exits 1
  when a deliberately broken WGSL module is added to the catalogue, and 2 when run with only
  `--enable-unsafe-webgpu` and `--use-webgpu-adapter=swiftshader` removed from its switches (the
  headless Ozone, ANGLE and Vulkan switches kept, so that Electron starts and finds no adapter).

**R01.T9.b Offline, every catalogued shader.** The runner cancels and records every request that is
not `file:` or `data:`, through `session.defaultSession.webRequest.onBeforeRequest` over
`<all_urls>`, and fails naming each URL with its resource type; a fixture page that fetches an
external URL proves the check fails. The page renders each `WGSL_CATALOGUE` entry once and asserts
the frame's properties and that the WGSL guard reported nothing. A test entry that uses a GLSL
shader must fail with its name.

- Acceptance: `just test-render` passes and prints an empty list of cancelled requests for the
  catalogue; the external-fetch fixture fails and prints its URL and resource type; the GLSL fixture
  fails with `GlslShaderRefused` and its name, and its cancelled list is empty (no compiler fetch).

**R01.T9.c The feature paths.** The runner runs the page twice: as SwiftShader offers it (no
`shader-f16`, subgroups present), then with `CapabilityOverrides.withholdSubgroups`, which requests
the device without the feature, and asserts that each run's capability line says which path it ran
on. Adapter-summary assertions: SwiftShader is `fallback`, and the photorealistic style is refused.

The runs form a matrix of settings × capability paths (Design note 18); R01's only setting is
`default`, and a catalogue entry may declare others, which R12.T7.a supplies.

- Acceptance: `just test-render` prints two runs, each passing, and a fixture entry declaring a
  second setting is run at both.

**R01.T9.d Three canvases on one device.** The page creates a 1280 × 720 cockpit canvas and two 320
× 240 instrument canvases, each its own `RenderView` with its own camera, a one-pass post-process
chain and depth. Each renders a pattern with a marker in its top-left quadrant; the readback asserts
the marker is in the top-left of each (right way up, the `_disableEngineYFlip` pin's behavioural
check), that each view's size is its own, and that resizing one instrument leaves the other two
views' readbacks and sizes unchanged.

- Acceptance: `just test-render` passes these checks.

**R01.T9.e Its place beside `just ci`.** `just test-render` stays its own recipe for now, not a step
of `ci`: it needs a frontend build that `ci` does not make today, and an Electron upgrade could
break headless Ozone (it already segfaults without `offscreen`) and so block unrelated commits
(researched 2026-09-29; the research agent's lean, medium confidence). Every task that touches
`view/engine/`, `src/smoke/` or a catalogued shader runs it as part of its own gate, and the
`validate` skill's check plan gains it for those paths. Measure the recipe's wall time, build
included, over ten runs, and record it with the pass count; after one Electron upgrade with no
harness failure, propose moving it into `ci` to the owner.

- Acceptance: the recorded figures; the `validate` skill's routing names the recipe for those paths.

**R01.T9.f Depth, culling, bias and loss.** From Design note 18, into a `createRenderTarget` with
`depth: true` so that `readTexture` reads its depth: a point drawn at a known distance under a fixed
reversed-Z projection writes the predicted depth, with no half-Z conversion; a back-facing triangle
is culled only under `cullMode: "back"`; a coplanar line with `depthBiasAway` loses to the surface.
And T8.e's integration check: the harness page calls `device.destroy()` on the engine's device,
which the engine did not dispose itself and so treats as a loss; `device-lost` is raised, the engine
rebuilds on a freshly requested adapter, and its views and targets render again at their sizes.

- Acceptance: `just test-render` passes these checks.

**R01.T9.g Blending, compute and the packed cube.** From Design note 18: a white quad at alpha 0.5
blended over black reads back near 188 of 255, not 128 (the linear value 0.5 encodes to 0.7354 of
full scale in sRGB, IEC 61966-2-1, so linear-light blending); a compute kernel writes a 3D storage
texture that a draw then samples; and a two-level packed cube written once with
`writePackedCubeLevel` and once with `writePackedCubeLevelFromBuffer` from a kernel's buffer samples
back to the packed values both times.

- Acceptance: `just test-render` passes these checks.

**R01.T9.h Offscreen targets, asynchronous pipelines, indirect work and timing.** From Design note
19: a chain of two `rgba16float` targets, the second sampling the first, reads back the expected
texels; a material made with `createMaterialAsync` draws on the first frame after it resolves, and
a frame submitted meanwhile does not wait on its compile; a compute pass writes an indirect-args
buffer that a `DrawItem.indirect` draw and an indirect `dispatch` then consume, drawing the
instance count the kernel wrote; with SwiftShader's `timestamp-query`, `onPassTimes` reports one
entry per labelled pass, raw passes unbracketed and Babylon's bracketed, each finite and
non-negative (values are not asserted: SwiftShader checks correctness, never speed).

- Acceptance: `just test-render` passes these checks.

**R01.T9.i Material state, instancing, storage buffers, post-process inputs and the splat.** From
Design note 21: an occluder with `colourWrites: false` leaves the colour untouched but hides a line
drawn behind it; a line with `depthWrite: false` leaves depth at the clear value; two additive
sprites over one texel sum in linear light; 64 instances of one quad, each placed from a
per-instance attribute and a storage-buffer entry by `instance_index`, land at their 64 positions; a
post-process reading `depth` and `hdr-colour` writes a texel that is a known function of both; a
`createPointSplat` pass of 1,000 unit points onto a 64 × 64 `rgba32float` target sums, per texel, to
the count of points that land there, and on a run with `float32-blendable` withheld
`createPointSplat` throws `Float32BlendUnavailable`. From T8.i: a full-screen draw into a second
target reads the first target's depth as `texture_depth_2d` and writes a known function of it, and
the same draw into the first target throws `DepthSelfSample`. Over a cleared alpha of 2, an
`"additive"` and a `"premultiplied"` draw (α = 0.5) each leave alpha at 2, and the premultiplied
colour is c_src + 0.5 c_dst. From T8.j: `GraphicsStatus.targetRounding` settles to a value other
than `unknown` for `rgba16float`, and the value is logged.

- Acceptance: `just test-render` passes these checks.

### R01.T10 Subgroup twins

Two toy pairs in the catalogue, over T6's `selectKernel` (Design note 16), run across T9.c's two
capability paths:

- a `bit-exact` integer sum of a buffer of 2¹⁶ `u32` by workgroup reduction, with and without
  `subgroupAdd`, whose inputs make the sum wrap past 2³² at least once, with a second dispatch whose
  invocation count is not a multiple of the subgroup size; both paths' bytes equal each other and a
  CPU `wrapping_add` total, and the kernel reads `subgroup_size` rather than assuming one;
- a `presentation-only` `f32` sum of finite, normal inputs, whose ordinary `readBuffer` throws
  `PresentationOnlyReadback`, and whose `"tolerance"` read on each path lies within
  `highamBound(n, Σ|xᵢ|)` of an `f64` CPU sum; whether the two paths differed is logged, not
  asserted.

- Tests: the CPU reference sums, and `highamBound`, in the logic project; the harness's checks.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/engine/kernels.test.ts`
  and `just test-render`.

### R01.T11 The three canvases on the UHD 620, by hand

Run the T9.d scene in the client's graphics configuration (not SwiftShader) with
`--hyperion-gpu-timing`, full-window cockpit and two instruments, for five minutes with resizes.
Record: GPU time per view from `onPassTimes` (bracketed, Design note 19), that no copy pass exists
(the frame's passes are the three views' own), the frame interval at the 50th and 95th percentiles,
a screenshot by eye that each view is the right way up, and the UHD 620's
`GraphicsStatus.targetRounding` (R07's probe expects `toward-zero` for both formats).

- Acceptance: the figures recorded in this plan's as-built notes.

### R01.T12 The Vulkan soak (open question 14)

Thirty minutes on the development machine with the chosen switches: a photorealistic stand-in (the
T9.d scene at full window with a post-process) and the consoles together, a looping `<video>`,
window resizes in a loop (`xdotool windowsize`), a second window, and the display blanked and
restored (`xset dpms force off`, a minute, `on`). A soak script under `src/smoke/soak/` logs every
`child-process-gone` and `gpu-info-update` with `getGPUFeatureStatus()`, polls `app.getAppMetrics()`
for a changed GPU pid, logs `device.lost` and a frame heartbeat whose gaps over 1 s mark a hang, and
compares known DOM regions in `webContents.capturePage()` every 30 s; Chromium's stderr is captured
with `--enable-logging=stderr`, and a run with `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`
catches `vkAcquireNextImageKHR` errors if the layer loads in the GPU sandbox. Repeat without
`DefaultANGLEVulkan`, and once with `--disable-vulkan-surface`.

- Acceptance: the three runs recorded (restarts, hangs, DOM mismatches, validation messages); the
  switch set changed if the soak says `DefaultANGLEVulkan` is needed or harmful, in a commit that
  cites the record; open question 14 answered in the notes for the brainstorm's next revision.

### R01.T13 Same-origin child windows (open question 15, second half), by hand

R07's research has already probed the core, by hand on 2026-09-29 on Electron 44.4.3 and the UHD
620 under the switch set of Design note 2, and found this: a
same-origin child opened with `window.open`, its canvas configured against the opener's `GPUDevice`,
renders, reaches the screen and paces at 16.7 ms from either window's `requestAnimationFrame`;
closing the child is not a device loss, but the next submit to its context raises an uncaptured
validation error, "context configuration is invalid", while the device and the main view carry on.
So this task finishes the prototype on a scratch branch, not merged: record a resize of the child,
whether it stays in the opener's process (`app.getAppMetrics()`), and, if a second display can be
borrowed, its pacing there. It also writes down the rule a client that opens such windows must
follow, for R07 to consume: drop the child's `RenderView` on its `pagehide` before the next frame,
and treat that one validation error from a closing child as expected, not as a fault.

- Acceptance: the result, with the research probe's findings above, recorded in this plan's as-built
  notes; open question 15's second half answered for the brainstorm; R07.T21 consumes the record.

### R01.T14 Verification pass

Run everything below and fill the as-built notes: the chunk sizes, the harness times, the soak and
the three-canvas figures.

- Acceptance: `just ci` and `just test-render` pass; the notes are complete.

## Verification

- **Platform, automatic:** the switch builder's tests (no unsafe flag, merged values, safe mode),
  the crash-loop policy's tests, adapter acquisition against fakes, the status reducer and its
  words.
- **Engine, automatic:** the boundary test; the chunk check; `just test-render` on SwiftShader:
  every catalogued shader offline, both feature paths, three canvases right way up with independent
  resizes, depth, culling, bias and a forced loss, blending, compute and the packed cube, offscreen
  targets, asynchronous pipelines, indirect work and pass timing, material state, instancing,
  storage buffers, post-process inputs and the point splat, and both toy kernel pairs across paths.
- **By hand, recorded:** the adapter on the UHD 620 (T1.b); the crash-loop relaunch (T2.b); the
  three canvases' GPU time with no copies (T11); the soak (T12); child windows (T13).
- **By eye:** the `GRAPHICS` panel in each condition, and the header banner in safe mode (T5.b).

## Generator version

No change to generated output and no bump: nothing here touches `hyperion-sim`. Nothing on the wire
changes and `PROTOCOL_VERSION` stays at 2. The plan reserves the `view/engine/` directory, the
`WGSL_CATALOGUE` into which later plans register every shader so that the offline test covers it,
the `GRAPHICS` nomenclature family, and the switch names `hyperion-graphics-safe` and
`hyperion-gpu-timing`.

## Risks and open points

- **Every timing cited here is provisional.** The probes of 2026-09-29 ran on a development machine
  shared with other agents' builds and tests (load average near 14 during the harness runs): the
  harness's 2 to 3 s a run, the 16.7 ms pacing and any frame figure are to be re-measured on a quiet
  machine in T9.e, T11 and T14 before they are relied on. The functional findings (features,
  switches, events) do not depend on load.

- **The forced path is the only Linux path.** Vulkan compositing on Gen9.5 is unproven beyond the
  probe's seconds and the soak's thirty minutes; a soak that fails is an answer, and the fallbacks
  are the soak's variants, then the safe mode. Machines with other GPUs under the same switches are
  untested until someone runs one.
- **The Wayland relaunch costs a second start** on every Wayland launch without the flag on its
  command line. The packaged launcher removes it; in development the `client` recipe passes the
  flag itself (T1.b), since a relaunch would end `electron-vite dev` (Design note 3).
- **Chromium's thresholds were observed, not read.** Three crashes to drop Vulkan and six to drop
  GPU compositing come from the probe, not from `gpu_process_host.cc`; the policy also watches the
  feature status, so it holds if the counts differ.
- **Babylon's internals.** `_device`, `_disableEngineYFlip` and `_hardwareTexture` are `@internal`;
  the typecheck, the orientation check and the cube round trip catch a rename or a change of
  meaning, and the fallback without the first two is a CSS flip with picking flipped to match,
  which still needs the device (open question 15). The raw indirect pass and the pass timing of
  Design note 19 lean on the same two, `_device` and `_hardwareTexture`, and add none.
- **Bracketed pass times include queue gaps.** A Babylon-encoded pass is timed by empty compute
  passes before and after it (Design note 19), so its figure is an upper bound; R05's and R12's
  records state `bracketed`. If the bracket proves too loose on the UHD 620 (T11), the fallback is
  Babylon's internal `WebGPUTimestampQuery.startPass` hook (`webgpuTimestampQuery.d.ts:20`), which
  would be a fourth pinned internal.
- **Babylon's generated depth texture may lack `COPY_SRC`.** Then an offscreen target makes its own
  `depth32float` (Design note 19, T8.f), and a canvas view's depth is not read back at all; the
  depth checks run on offscreen targets (T9.f).
- **An `rgba32float` bake target.** R06's splat renders into `rgba32float` (Design note 21), which
  the brainstorm's Decision ("Light") excludes from colour targets for frame bandwidth. R01 reads
  the exclusion as covering per-frame targets only and keeps `rgba32float` out of
  `ColourTargetFormat`; the README should record the reading, or the owner may rule otherwise, in
  which case R06's compute-splat fallback becomes its only path.
- **Relaunch in development.** `electron-vite dev` exits when its Electron does (Design note 3), so
  the crash-loop relaunch is checked on a built client and the `client` recipe puts the X11 flag on
  the command line itself.
- **The skills do not yet read `R` task IDs.** Until R04.T7.c extends `plan_task.py` and
  `select_checks.py`, R01's tasks are built and validated by hand (Consumes).
- **Babylon's unawaited effect preparation** means a WGSL compile error may also surface only as an
  unhandled rejection; the backstop reports it, and `onEffectErrorObservable` covers the ordinary
  case. If an upgrade awaits it, layer (c) becomes redundant, not wrong.
- **The safe mode draws nothing.** A reading forced by the probe (Design note 5); if the owner wants
  a view in safe mode, the only candidate is a Canvas 2D wireframe, a second renderer the WebGPU
  ruling declined, so it is not planned.
- **The harness stays outside `just ci`** (T9.e), so it guards only the tasks that run it; the
  `validate` routing is what keeps it from rotting. Headless Ozone is the fragile part: an Electron
  upgrade that breaks it falls back to a hidden X11 window, which needs a display. SwiftShader
  checks correctness only; a pass says nothing about the UHD 620's speed.
- **The fault wording** is a draft for the owner (T5.c); only constants change if it is amended.
- **The rounding probe reads one value a format.** Design note 22 tells round-to-nearest from
  truncation at one magnitude. An adapter that rounds some other way, or differently by magnitude,
  would still be classed by that one value. R07 reads anything but `nearest` as a reason to keep
  `rgba16float`, so a misclassification costs bandwidth, not accuracy.
- **The premultiplied blend overrides Babylon's factors in `onBind`** (Design note 21). This leans
  on the order in which `Mesh.render` sets the alpha mode and then binds. An upgrade that moves it
  would show in T9.i's alpha check, and the fallback is the adapter's own raw pass (Design note 19)
  with its own blend state.
- **Deviations in T1, as built.**
  - `GraphicsLaunchMode` is declared in `preload/api.ts`, and `launchModeOf`, `SAFE_MODE_SWITCH` and
    `GPU_TIMING_SWITCH` in the dependency-free `preload/graphicsLaunch.ts`; `main/graphics/switches.ts`
    re-exports all four under the plan's names. The sandboxed preload needs them too, and neither it
    nor the renderer may import main-process code (the `serverUrl.ts` pattern).
  - `applyGraphicsSwitches` takes `Pick<CommandLine, "appendSwitch" | "getSwitchValue">`, which
    `app.commandLine` satisfies, so that a fake stands in for it.
  - The merge covers all four list switches (`enable-features`, `disable-features`,
    `enable-dawn-features`, `disable-dawn-features`), since the timing toggle's
    `disable-dawn-features` is last-writer-wins like the others; `mergeSwitchValue` trims items.
  - `x11RelaunchArgs` recognises only the exact `--ozone-platform=x11` spelling, as Design note 3
    words it. The X11 check runs after the command line is parsed, so `--help` and a usage error
    answer without a relaunch. `OZONE_X11_FLAG` is exported.
  - By hand, pending: the adapter from the devtools console on the UHD 620 under `just client`, and
    `app.getGPUFeatureStatus()` after `gpu-info-update` (the monitor reads it but does not log it;
    the check needs a devtools step or a temporary log line).
- **Deviations in T2, as built.**
  - The window's `additionalArguments` carry only `--hyperion-graphics-safe` (safe mode) and
    `--hyperion-gpu-timing`; the preload works the mode out with `launchModeOf(process.platform, …)`
    (`graphicsArguments`, `graphicsLaunchFromArgv`). `gpuTiming` is true only in `vulkan` mode, in
    the main process and the preload alike, since only the forced path carries the Dawn toggle.
  - `readFeatureStatus` returns a named `FeatureStatusReading`; an absent entry is recorded in a
    `status` event as `""`. Two statuses stamped alike decide nothing between them: a drop counts
    only strictly after a status that held the value, which keeps the decision independent of
    arrival order.
  - `GpuProcessMonitor` takes `{ app, windows, mode, args, nowMs }` over narrow `MonitoredApp` and
    `MonitoredWindow` interfaces, exposes `history` and `dispose()` (called on `will-quit`), and
    starts before `ready`. Only exits other than `clean-exit` are sent to windows, and
    `GpuProcessGoneReport.count` counts them over the launch, not within the window.
  - The preload's crash subscription is `subscribeGpuProcessGone(ipcRenderer, listener)` in
    `graphicsLaunch.ts`, which passes on only reports `readGpuProcessGoneReport` accepts; the channel
    is `GPU_PROCESS_GONE_CHANNEL`. `HyperionApi.graphics` is typed as `GraphicsApi`.
    `stubHyperionApi(serverUrl?, graphics?)` takes the graphics fields, `TEST_GRAPHICS` by default.
  - By hand, pending: `kill -9` of the GPU process three times within a minute on a built client
    relaunches once into safe mode, whose adapter request is null and whose consoles work; a fourth
    crash does not relaunch.
- **Deviations in T3, as built.**
  - `StyleAvailability` is an `interface` (object shapes are interfaces under the TypeScript rules).
  - `test/fakeGpu.ts` also exports `INTEL_UHD_620_INFO` and `SWIFTSHADER_INFO` (their subgroup sizes
    are illustrative, not probed); `FakeGpu.requests` records each `requestAdapter`'s options;
    `FakeAdapter` takes `maxTextureDimension2D` in place of a limits object, and its
    `requestDevice` rejects a required feature the adapter lacks; every `FakeDevice` member not
    faked throws, naming itself.
- **Deviations in T4, as built.**
  - `GraphicsStatus` gains `gpuTiming`, the preload's flag, from which the reducer derives `timer`
    when an adapter answers; R12 may read it. `initialGraphicsStatus(launchMode, gpuTiming)` builds
    the start (`safe-mode` in a safe launch, else `acquiring`; `timer` `absent`). The rounding
    record's key type is `ProbedTargetFormat`.
  - `GraphicsEvent` is `adapter-outcome`, `device-lost`, `device-restored` (carrying the fresh vetted
    outcome), `adapter-withdrawn`, `gpu-process-gone` and `target-rounding` (T8.j's probe result).
    `safe-mode` and `disabled` are final for the launch: later outcomes and restores leave them, while
    losses and crashes still count. The disabling loss clears `fault`, since the `disabled` statement
    replaces it; `gpuProcessCrashes` keeps the highest count reported.
  - Precedence of the annunciation: the safe or disabled statement, then a current fault, then the
    adapter's condition. `graphicsModeAnnunciation` gives the first tier alone (T5.b's banner); the
    words are the one constant `GRAPHICS_WORDS`, and the result type is `GraphicsAnnunciation`.
  - `feedGraphicsStatus(store, graphics, gpu)` subscribes to the crash reports and makes the first
    `requestAdapterOutcome`, so that the `LINK` panel knows the adapter before any view loads the
    engine; not in safe mode (Design note 5); a rejected request reads as `no-adapter`. The vetted
    adapter is not kept: `loadRenderEngine` requests its own (Design note 7).
  - `useGraphicsStatus()` reads the store from `GraphicsStatusContext` and throws outside a
    provider; `GraphicsStatusProvider` (`view/engine/GraphicsStatusProvider.tsx`) owns the one store
    and its feed, and `App` mounts it. `navigatorGpu()` narrows `navigator.gpu` from `unknown`, since
    `lib.dom` types it as always present. The store has a `listenerCount` for tests.
  - `capabilities`, and so `timer`, come from the adapter's summary. They equal the device's unless
    the harness withholds a feature; T8 should dispatch the device-read capabilities with the
    engine's first creation and each `device-restored`, so that a withheld feature reads as absent
    in the status too (Design note 7).
  - Open, for the owner or R02: a `GRAPHICS PROCESS RESTARTED` fault clears only on
    `device-restored`, so until a view owns a device (R02), or under `no-adapter`, one GPU-process
    crash leaves the fault standing on the `LINK` panel for the rest of the launch. Candidates: keep
    it for the launch, clear it on the next successful adapter request, or clear it after a set time.
- **Deviations in T5, as built.**
  - The `Features` row lists the wanted features present by their WebGPU names, comma-separated, or
    `NONE`; `Styles` reads `WIREFRAME, PHOTOREALISTIC`, `WIREFRAME`, `NONE` (no views: no WebGPU, no
    adapter, safe or disabled) or the em dash while acquiring. `WIREFRAME` and `PHOTOREALISTIC` are
    used here before R02's view class names them; T5.c lists them for the owner. `GPU Timer` is the
    em dash until an adapter answers. `Adapter` is `vendor · architecture` as the adapter reports
    them (lower case, e.g. `intel · gen-9`).
  - The header banner is its own component, `GraphicsModeBanner`, in `ConsoleFrame`'s status area
    before the clock, an `output` labelled `Graphics mode` in `--text` inside a `--line` rule
    (`.console__banner`). It names the mode alone, `GRAPHICS SAFE MODE` or `GRAPHICS DISABLED`
    (the words before the colon), so that the strip fits at 1280 × 720; the panel carries the
    sentence. `ConsoleFrame` now needs a `GraphicsStatusContext` provider above it.
  - The disabled statement's count is in mixed case (`GRAPHICS DISABLED: 3 device losses, …`),
    since every other clause after a colon is a sentence (the guide's Typography); Design note 10
    wrote `<n> DEVICE LOSSES`. The two counts are `output`s, live without an
    annunciation once the mode is settled.
  - Kept as drafted, for the owner (UX review of T5): `GRAPHICS NOT AVAILABLE: no WebGPU`,
    `GRAPHICS NO ADAPTER: views unavailable` and `GRAPHICS PROCESS RESTARTED` name no operator
    action (the guide's Voice asks for one where known; `relaunch to retry` is the candidate); the
    `acquiring` condition shows only em dashes and no annunciation, which is momentary in practice
    (a waiting `GRAPHICS ACQUIRING ADAPTER` would be a new word); whether the banner looks
    "unmistakably different" enough in a `--line` rule; `GPU` and `WebGPU` on the list; one of
    `UNAVAILABLE` and `NOT AVAILABLE`.
  - By eye, pending: the `GRAPHICS` panel in each condition and the header banner in safe mode on
    `just client`, with the store driven from the devtools console, beside the guide's banner rule.
- **T5.c, the nomenclature draft for the owner (not applied to the guide).** R02.T2.f absorbs and
  re-checks it in its single pass over `docs/frontend/ux-guidelines.md`. The code is built to it
  meanwhile: every string is one constant, `GRAPHICS_WORDS` in `view/engine/status.ts`, and the
  panel's `MODE_WORDS` and `TIMER_WORDS` in `components/GraphicsPanel.tsx`. Awaiting the owner's
  sign-off or amendment; the constants then change in one commit.

  ```diff
  @@ Layout @@
   - A simulation, training or replay mode must look unmistakably different from live
     operation, through a persistent labelled banner in the header strip.
  +- The same banner states a graphics mode that lasts until a relaunch, `GRAPHICS SAFE MODE` or
  +  `GRAPHICS DISABLED`, in `--text`, beside the link status; it is never counted among the
  +  alerts.
  @@ Alerts @@
   - Alerts are raised by the server from simulation state. A console never invents one.
  +- A console's report on its own graphics is a Fault (`StatusLine`'s fault standing, while the
  +  fault lasts) or a status in plain text, never an alert: it takes no tone, no flash and no
  +  place in the header's alert counts.
  @@ Nomenclature list @@
  +| `GRAPHICS` | System | The console's own graphics: adapter, features, mode, GPU timer and faults; the `LINK` display's `GRAPHICS` panel |
  +| `GRAPHICS SOFTWARE ADAPTER`, `GRAPHICS NOT AVAILABLE`, `GRAPHICS NO ADAPTER`, `GRAPHICS SAFE MODE`, `GRAPHICS DISABLED` | Status | The graphics' standing condition, with its cause or remedy after a colon: `GRAPHICS SOFTWARE ADAPTER: PHOTOREALISTIC STYLE UNAVAILABLE`, `GRAPHICS NOT AVAILABLE: no WebGPU`, `GRAPHICS NO ADAPTER: views unavailable`, `GRAPHICS SAFE MODE: views unavailable, relaunch to retry`, `GRAPHICS DISABLED: <n> device losses, relaunch to retry` or `GRAPHICS DISABLED: adapter withdrawn, relaunch to retry` |
  +| `GRAPHICS DEVICE LOST`, `GRAPHICS PROCESS RESTARTED` | Fault | The GPU device was lost and is being re-created (`GRAPHICS DEVICE LOST: re-creating`); the GPU process crashed and was restarted |
  +| `UNAVAILABLE` | Label | Not offered on this adapter or in this mode (the owner may prefer `NOT AVAILABLE`, which `GRAPHICS NOT AVAILABLE` already uses) |
  +| `DEFAULT`, `VULKAN`, `SAFE` | Mode | The launch's graphics mode: the platform's own path, the forced Vulkan path on Linux, the declared safe mode without WebGPU |
  +| `QUANTIZED`, `FULL`, `ABSENT` | State | The GPU timer: timestamps in 65,536 ns steps, at the device's own resolution, or no timestamps |
  ```

  `PHOTOREALISTIC` and `WIREFRAME` wait for R02's view class, though the `GRAPHICS` panel's
  `Styles` row and `GRAPHICS SOFTWARE ADAPTER` already use them (T5 as built). The panel's row
  labels (`Adapter`, `Software Adapter`, `Features`, `Styles`, `Mode`, `GPU Timer`,
  `Device Losses`, `Process Restarts`) are ordinary words, and `GPU` and `WebGPU` are proper names
  of the hardware and the API rather than ship abbreviations; the owner may want `GPU` on the list.
  `NONE`, `YES` and `NO` are the readings' plain words.

- **Deviations in T6, as built.**
  - The opaque handles are `readonly` interfaces with a literal `kind` and a `name`; `ComputeHandle`
    also carries its chosen `path` and `BufferHandle` its `bytes`. They are structural, not
    branded, so T8's adapter checks that a handle is its own when it looks one up.
  - `Float32BlendUnavailable`, `DepthSelfSample` and `PresentationOnlyReadback` are defined in
    `types.ts` now; `GlslShaderRefused` waits for T8's `wgslGuard.ts`. Named types were added for
    shapes Provides writes inline: `VertexAttribute`, `TexelRect` (`readTexture`'s region) and
    `KernelSelection` (`selectKernel`'s result).
  - `engineBoundary.test.ts` reads the sources through `import.meta.glob` (`?raw`, eager, from the
    app's Vite root), since the web project has no Node types. Besides T6's three rules it fails on
    a raw `…device.createBuffer(` or `…device.createTexture(` outside `view/engine/babylon/`
    (Design note 18). Its rules are a textual tripwire: a `"tolerance"` held in a variable passes.
  - `CatalogueEntry` carries an optional `settings` of `CatalogueSetting`, only `"default"` until
    R12.T7.a; absent means `default` alone.
  - `assertNoF16Subgroups` strips comments and reads every `enable` directive, comma lists and
    several on a line included; `highamBound` throws on a count that is not a positive integer or
    where (n − 1)u ≥ 1.
- **Deviations in T7, as built.**
  - `@babylonjs/core` is pinned at `9.28.0` exactly; the pin's reason is the module comment of
    `view/engine/babylon/engine.ts`. The placeholder `createBabylonEngine` imports `WebGPUEngine`
    from `Engines/webgpuEngine.pure` and rejects, naming T8, so that the chunk carries real Babylon
    code until T8 replaces it.
  - The renderer's `codeSplitting` has a second group, `preload-helper`, for Vite's
    `vite/preload-helper` (priority 1): without it rolldown put the helper, shared by the entry and
    every dynamic import, into the `babylon` chunk, and the entry imported that chunk eagerly for
    the helper alone. `checkChunks.mjs` caught it.
  - `scripts/checkChunks.mjs` reads the entry chunks from `out/renderer/index.html`, follows their
    static imports, and fails (exit 1) if any holds Babylon code, recognised by the strings
    `babylonjs` and `Babylon.js`, or if no `babylon-*.js` chunk holds it; exit 2 when there is no
    build. It is not part of `just ci`.
  - Nothing imports `loadEngine.ts` yet, so the build tree-shakes it and the `babylon` chunk is
    absent: `checkChunks.mjs` fails on today's tree with "no `babylon` chunk" and passes once R02's
    `VIEW` display or T9's smoke page imports `loadRenderEngine`. Checked by hand on 2026-09-30 with
    a temporary dynamic import in `main.tsx`, not committed: the check passed (entry, the
    preload helper and the rolldown runtime free of Babylon; `babylon-*.js` 1,255 kB minified, about
    240 kB gzipped, the placeholder's `webgpuEngine.pure` closure alone); with a temporary static
    import of `babylon/engine.ts` it failed, naming the entry. Timings and sizes are provisional
    (a shared machine).
  - `loadEngine.test.ts` fakes `importEngine` through `LoadEngineOptions` and follows the call by a
    factory that rejects with a sentinel, so that no `RenderEngine` is built in the test.
  - `just check-chunks` builds and runs the check; it is in neither `just ci` nor T14's acceptance,
    so T9 (whose smoke page first imports `loadRenderEngine`) or T14 should name it, and the
    Verification section's "the chunk check" under automatic holds only once one does. Laziness is
    also guarded in `just ci` by `engineBoundary.test.ts`: no file outside `view/engine/babylon/`
    imports `babylon/engine` statically, and only `loadEngine.ts` names it at all. The marker
    strings are a heuristic; the build manifest (`build.manifest`) would test module membership
    directly if a user-visible string ever carries `Babylon.js`.
- **Deviations in T8.a, as built.**
  - Materials are WGSL in Babylon's dialect (`attribute`, `varying`, `uniform`, and `var` without
    group or binding; `vertexInputs`, `vertexOutputs`, `fragmentOutputs`), documented on
    `WgslMaterialSpec`; the attributes and textures a material binds are read from its
    declarations. The engine sets three reserved uniforms in each material's `onBind` at every
    draw, `FRAME_UNIFORMS` in `types.ts`: `viewRotation` and `clipProjection` (the submission's
    matrices, column-major and unchanged, `Matrix.FromArrayToRef` then `setMatrix`, which matches
    R02.T7.a's `viewRotation4`) and `offsetFromCameraM`. The names avoid those Babylon fills itself
    (`view`, `projection`, `world`). `StorageBufferSpec.binding` is unused, since Babylon assigns
    bindings. **Awaiting the owner:** shaders tied to Babylon's dialect make a change of engine a
    rewrite of every material, against the brainstorm's "a re-implementation of one adapter";
    the alternatives are standard WGSL with fixed `@group`/`@binding` that the adapter converts, or
    a translator at the switch.
  - Compute kernels are plain WGSL with explicit `@group`/`@binding` and entry point `main`,
    compiled on the engine's device with `layout: "auto"` (`babylon/compute.ts`, bindings read by
    name with `kernelBindings`), and will be dispatched on the adapter's own compute passes, not
    through Babylon's `ComputeShader`: that cannot carry the adapter's `timestampWrites`, and its
    `setStorageTexture` takes no mip level, which `ComputeBindings.storage` needs. Indirect
    dispatch becomes `dispatchWorkgroupsIndirect` (Design note 19's _Indirect work_ and T8.g's
    `ComputeShader.dispatchIndirect` read so).
  - `createPostProcess` is a Babylon `PostProcess` whose fragment source is registered in the
    shader store under a unique key; its inputs bind under `POST_PROCESS_INPUTS` (`types.ts`):
    `hdr-colour` is Babylon's own `textureSampler`, `depth` is `depthTexture`
    (`texture_depth_2d`); its samplers' filter and address are set in `onApply`. The textures are
    bound when a view or target runs it (T8.c, T8.f).
  - Instancing: `instanceAttributes` are instanced vertex buffers (divisor 1) on the mesh's shared
    Babylon `Geometry`, and each draw sets `Mesh.forcedInstanceCount`, not `thinInstanceSetBuffer`:
    thin-instance buffers belong to one Babylon mesh, and a `MeshHandle` is drawn through a pool of
    Babylon meshes, one per (mesh, material, n-th use in the frame), sharing its geometry, each
    with `alwaysSelectAsActiveMesh`. A draw of zero instances is left out. Every draw thus takes
    Babylon's instanced path (`INSTANCES`/`THIN_INSTANCES` defines, harmless while no source
    declares `world0`–`world3`).
  - A `blend` other than `none` puts the material in Babylon's transparent queue whatever
    `transparent` says, since `Mesh.render` sets the alpha mode only there
    (`mesh.pure.js:2187`); within the queue `alphaIndex` is the draw's place in the submission.
  - `premultiplied` throws "built by R01.T8.i" until T8.i overrides mode 7's factors.
  - `createBabylonEngine` dispatches a new `GraphicsEvent`, `device-capabilities`, read from the
    device (`deviceCapabilities` in `platform.ts`), so a withheld feature reads as absent in the
    status; a settled condition ignores it. A rebuild's `device-restored` writes the adapter's
    capabilities, so T8.e sends `device-capabilities` after it. A feature asked for and not
    enabled (`featuresNotEnabled`) is logged with `console.warn`. A failed creation disposes the
    Babylon engine.
  - Registrations are explicit (`babylon/registrations.ts`): the `.pure` modules' register
    functions and the clear-quad and post-process vertex shaders, since `.oxlintrc.json` forbids
    side-effect imports.
  - `internals.ts` holds, besides `_device`, `_disableEngineYFlip` and `_hardwareTexture`'s read,
    two accesses built ahead for T8.c: `setAttachmentFormat`, which writes the hardware wrapper's
    `format` so that Babylon renders a canvas texture through its `-srgb` view, and
    `flushEngine`, over `flushFramebuffer`, declared `@internal` (`webgpuEngine.pure.d.ts:977`), a
    fourth pinned internal: it submits what Babylon has recorded so that the adapter's own passes
    run after it, and ends Babylon's current render pass.
  - Checked by a scratch page (not committed) on SwiftShader: the engine is made on the handed
    adapter, its capabilities read (no `shader-f16`, the rest present), and a material, a mesh, a
    compute kernel and a post-process are created. The RTX 3080 (`ampere` architecture, NVIDIA's
    Vulkan driver, Electron 44.4.3 under the client's switches with `DISPLAY=:0`) exposes
    `subgroups`, `timestamp-query`, `float32-filterable`, `float32-blendable`,
    `rg11b10ufloat-renderable` and `depth-clip-control`, and no `shader-f16`.
- **Deviations in T8.b, as built.**
  - "Reports to the status store" needed a status entry: `GraphicsFault` and `GraphicsEvent` gain
    `shader-refused` (with `effectName`), a fault while it stands, cleared by a restore like the
    others; it never replaces a standing fault, so the stub compiler's unnamed refusal that follows
    the wrapper's named one keeps the name, and a device loss outranks it. Its words, drafted for
    the owner with T5.c's: `GRAPHICS SHADER REFUSED: <effect> is not WGSL`
    (`GRAPHICS_WORDS.shaderRefused`). A refused
    shader is a bug of ours, not the operator's; the fault makes it visible on the `LINK` panel.
  - `guardCreateEffect` takes any `EffectFactory` (the engine's `createEffect`, whatever its
    arguments), reports before it throws, and reads the effect's name from a string or from its
    shader path (`spectorName`, `vertex`, `fragment`, else `(inline source)`); the stubs' errors
    say `(unnamed, at the compiler)`, since the compile is handed no name. The backstop listens on
    `window` from creation to disposal and calls `preventDefault` on what it reports. Babylon's
    `EffectWrapper` without the shader store (which the adapter does not use) makes its `Effect`
    directly, past `createEffect`; the stubs still stop it. No unit test covers the engine's
    releasing the backstop at disposal, since the engine needs a WebGPU device; T9 exercises it.
  - Pending for T9: the offline run with the network refused is the integration check.
- **Deviations in T8.c, as built.**
  - A view draws through its `RenderTargetTexture` directly (`target.render()` with the view's
    camera as `activeCamera` and the frame's meshes as `renderList`), inside one Babylon frame per
    view render (`beginFrame`/`endFrame`), not through `camera.outputRenderTarget` and
    `scene.render`: the frame's draws are the render list, and the canvas texture must be
    submitted before the task yields. Each frame resets depth test, depth writes and colour
    writes first, since a Babylon post-process leaves depth writes off and only a scene's own
    render resets them. The camera is Babylon's base `Camera` at the origin (identity view), its
    projection frozen to the submission's; no material reads Babylon's matrices anyway
    (`FRAME_UNIFORMS`).
  - The canvas is configured with `viewFormats: [<format>-srgb]`, and after each
    `wrapWebGPUTexture` or `updateWrappedWebGPUTexture` the adapter sets the hardware wrapper's
    `format` to the `-srgb` view (`setAttachmentFormat`, a write through `_hardwareTexture`), which
    Babylon then uses for the attachment view and the pipeline: no `rgba16float` fallback was
    needed. Checked on SwiftShader and the RTX 3080 by a scratch page (not committed): a linear
    0.5 reads back 188.
  - Winding: materials set `sideOrientation` to counter-clockwise explicitly; left to the mesh,
    Babylon's default in the right-handed scene reversed it (the scratch page saw a clockwise
    triangle drawn and a counter-clockwise one culled). With the view's `_disableEngineYFlip`,
    counter-clockwise in WebGPU's framebuffer is front, as R02's matrices expect.
  - `RenderView.readBack` returns RGBA bytes, sRGB-encoded, rows from the top, unpadded, whatever
    the canvas's byte order (a `bgra8unorm` canvas is swizzled); it must be called in the task of
    the `render` it reads. `gpuFlags.ts` (`BUFFER_USAGE`, `TEXTURE_USAGE`, `MAP_MODE`) holds
    WebGPU's flag values, since TypeScript 7's `lib.dom` declares the flag types but not the
    namespaces that hold them.
  - `FrameSubmission.postProcesses` run as the view target's Babylon post-process chain
    (`addPostProcess`), which draws the scene into the first pass's input and ends in the canvas.
    Post-processes are made non-reusable (one input each, no ping-pong). The colour input works
    (checked: an inverting pass on both adapters, and again after the view was resized, since a
    view detaches the engine's shared post-processes before it disposes its target). The `depth`
    input, bound to the first pass's input depth under `depthTexture`, got a texture of the view's
    size that read zero on both adapters although the pass's depth test worked, so
    `createPostProcess` with a `depth` input throws "built by R01.T8.f" for now: **open, carried to
    T8.f/T8.i**, where the target's own `depth32float` made with `TEXTURE_BINDING` and `COPY_SRC`
    is the candidate source.
    Post-process uniform values have no path in `RenderEngine` (a `WgslPostProcessSpec` declares
    uniforms, nothing sets them): **open, for the plan that first needs one (R05/R07)**.
  - Checked by the scratch page on SwiftShader and on the RTX 3080 (`DISPLAY=:0`, the client's
    switches): two canvases from one device at their own sizes (64 × 64, then a second resized to
    32 × 16) draw the right way up (a triangle in the upper-right quadrant of view space lands in
    the image's upper right); a back face is culled; a nearer triangle drawn first stays in front
    of a farther one drawn after (reversed depth, greater-or-equal); two draws of one mesh and
    material with different offsets and tints each land. These are not T9's checks, which remain
    pending for T9.
