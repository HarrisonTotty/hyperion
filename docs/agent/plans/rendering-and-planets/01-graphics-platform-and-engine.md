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
chosen against their reference twins by feature detection. The Vulkan soak of open question 14 has
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
  format, and one canvas context per view.
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
/** How this launch runs the GPU: the normal Vulkan path, or the declared safe mode. */
export type GraphicsLaunchMode = "vulkan" | "safe";
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
/** The relaunch a Wayland session needs to run through XWayland, or `undefined`. */
export function x11RelaunchArgs(
  argv: readonly string[],
  env: NodeJS.ProcessEnv,
  platform: NodeJS.Platform,
): readonly string[] | undefined;

export const SAFE_MODE_SWITCH = "hyperion-graphics-safe";
export const GPU_TIMING_SWITCH = "hyperion-gpu-timing";

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
    readonly withholdShaderF16: boolean }       // the harness's runs only

// status.ts
export type GraphicsFault =
  | { readonly kind: "device-lost"; readonly reason: GPUDeviceLostReason; readonly message: string }
  | { readonly kind: "gpu-process-gone"; readonly count: number };
export type GraphicsCondition =
  | { readonly kind: "acquiring" }
  | { readonly kind: "nominal"; readonly summary: AdapterSummary; readonly styles: StyleAvailability }
  | { readonly kind: "software-adapter"; readonly summary: AdapterSummary }
  | { readonly kind: "no-adapter" }
  | { readonly kind: "safe-mode" }
  | { readonly kind: "disabled"; readonly losses: number };
export interface GraphicsStatus {
  readonly condition: GraphicsCondition;
  readonly capabilities: GpuCapabilities | null;
  readonly launchMode: GraphicsLaunchMode;
  readonly fault: GraphicsFault | null;          // current, cleared on recovery
  readonly deviceLosses: number; readonly gpuProcessCrashes: number;
}
export type GraphicsEvent = /* adapter outcome, device lost, device restored, process gone */;
export function reduceGraphicsStatus(status: GraphicsStatus, event: GraphicsEvent): GraphicsStatus;
export const DEVICE_LOSS_LIMIT = 3;
export function graphicsAnnunciation(status: GraphicsStatus):
    { readonly text: string; readonly standing: "plain" | "fault" } | null;
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
  readonly capabilities: GpuCapabilities;
  readonly depthPolicy: DepthPolicy;
  createView(canvas: HTMLCanvasElement, name: string): RenderView;
  createMesh(spec: MeshSpec): MeshHandle;
  createMaterial(spec: WgslMaterialSpec): MaterialHandle;
  createPostProcess(spec: WgslPostProcessSpec): PostProcessHandle;
  createCompute(pair: KernelPair): ComputeHandle;
  /** Every GPU buffer and texture is created here, and nowhere else, with its memory category. */
  createBuffer(spec: BufferSpec): BufferHandle;
  createTexture(spec: TextureSpec): TextureHandle; // 2D, 3D, cube; sampled and/or storage
  /** R06's packed star cube: rgb9e5ufloat, every mip written by copyBufferToTexture. */
  createPackedCube(sizePx: number, mips: number, category: MemoryCategory): TextureHandle;
  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void;
  dispatch(
    kernel: ComputeHandle,
    bindings: ComputeBindings,
    workgroups: readonly [number, number, number],
  ): void;
  /** Every creation and destruction, with its byte size and category; R05's tally subscribes. */
  onAllocation(listener: (event: AllocationEvent) => void): () => void;
  onFault(listener: (fault: GraphicsFault) => void): () => void;
  dispose(): void;
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
  readonly size: GPUExtent3DStrict;
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
  readonly textures: Readonly<Record<string, TextureHandle>>; // per-draw sampled textures
}
export interface WgslMaterialSpec {
  readonly name: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  readonly uniforms: ReadonlyArray<UniformSpec>;
  readonly samplers: ReadonlyArray<SamplerSpec>;
  readonly transparent: boolean;
  readonly cullMode: "none" | "back";
  /** Positive is away from the camera, whatever the depth direction (R02's lines). */
  readonly depthBiasAway?: { readonly constant: number; readonly slopeScale: number };
}
export interface WgslPostProcessSpec {
  readonly name: string;
  readonly fragmentWgsl: string;
  readonly uniforms: ReadonlyArray<UniformSpec>;
}
export function loadRenderEngine(
  outcome: AdapterOutcome & { kind: "adapter" },
  status: GraphicsStatusStore,
): Promise<RenderEngine>; // loadEngine.ts, dynamic import
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
```

### Components and tests

- `renderer/src/components/GraphicsPanel.tsx` on the `LINK` display, and the safe-mode and disabled
  banner in `ConsoleFrame`'s status area.
- `renderer/src/test/fakeGpu.ts`: `FakeGpu`, `FakeAdapter`, `FakeDevice` with a controllable `lost`
  promise and feature set, for tests in the `logic` project.
- `smoke/` (new, beside `main`, `preload` and `renderer`): the harness's Electron entry and its
  preload; `renderer/smoke.html`, its test page; `just test-render`.
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
(`createPackedCube`, `writePackedCubeLevel`, R06 Design note 21); R05's per-draw textures and
compute kernels writing 2D and 3D storage textures with their bytes visible to its allocation tally
(R05 Consumes); R12's one creation path with a memory category (R12.T3.a) and one `just test-render`
variant per setting (R12.T7.a); and R10's `depth-clip-control` among the wanted features (Design
note 7).

## Design notes

1. **Where the code lives.** The engine-agnostic interface and its Babylon implementation are
   created here under `renderer/src/view/engine/`, the first files of the `view/` directory the
   brainstorm names. R02 adds the camera, scene builder and styles beside them. Only
   `view/engine/babylon/` may import `@babylonjs/*`; a test, not a lint override, enforces it
   (`engineBoundary.test.ts` reads every renderer source file and fails on an import of the engine
   elsewhere), because `.claude/rules/typescript-dev.md` forbids changing a rule's scope in
   `.oxlintrc.json` without asking and a test needs no permission. The same test fails on any call
   of `registerView`, whose `drawImage` copy the brainstorm rejects.

2. **The switches are data, merged, and set before `ready`.** `graphicsSwitches` returns the
   brainstorm's set on Linux and nothing elsewhere: `use-angle=vulkan`,
   `enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`, and
   `enable-dawn-features=enable_subgroups_intel_gen9`; never `enable-unsafe-webgpu`. Researched
   2026-09-29 by probe on Electron 44.4.3 (`/tmp/r01-research-a/`, not in the repository): an
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
   through X11 or XWayland); only the mechanism differs, and the notes file reports it.

4. **Timing is quantized, and one narrow switch lifts it for measurement.** The brainstorm's step 3
   says GPU time per pass is available uncoarsened on Linux under the forced switches. Researched
   2026-09-29 by probe on the UHD 620: `timestamp-query` is exposed under the switch set, but every
   timestamp is a multiple of 65,536 ns; `--disable-dawn-features=timestamp_quantization` lifts it
   and adds no feature and no adapter, while `--enable-webgpu-developer-features` also lifts it but
   exposes more, `--enable-unsafe-webgpu` lifts it too but adds features and CPU adapters (confirmed
   by R07's research), and `allow_unsafe_apis` does not. So the main process adds that one Dawn
   toggle, merged, only when `--hyperion-gpu-timing` is on the command line, which the performance
   runs of R05 and R12 pass; shipping launches keep the quantization. `GraphicsStatus` states which,
   so a recorded figure says whether its timer was coarse.

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
   `crashLoopDecision` therefore relaunches into safe mode when either `CRASH_LOOP_COUNT` (3)
   GPU-process exits other than `clean-exit` fall within `CRASH_LOOP_WINDOW_MS` (5 minutes), or a
   status event shows Vulkan or WebGPU no longer enabled while the mode is `vulkan`. In safe mode it
   never relaunches, which is what makes it "once". The relaunch appends `--hyperion-graphics-safe`,
   which the main process reads with `app.commandLine.hasSwitch` and passes on to the preload, as it
   passes the server URL. `app.disableDomainBlockingFor3DAPIs()` is called before `ready`: without
   it the probe saw Chromium block WebGPU for the page after the second crash, so the client never
   got the chance to report and recover.

7. **The client requests its own adapter, and Babylon is handed it.** Researched 2026-09-29 in
   `@babylonjs/core` 9.28.0: `WebGPUEngine.initAsync` always calls `navigator.gpu.requestAdapter`
   itself (`Engines/webgpuEngine.pure.js:413-417`), takes no adapter or device, and silently drops
   requested features the adapter lacks (`:430-438`). So `loadRenderEngine` installs a wrapper on
   `navigator.gpu.requestAdapter` that returns the adapter `requestAdapterOutcome` already vetted,
   keeps it installed because Babylon calls it again to restore after a loss, passes
   `deviceDescriptor.requiredFeatures` from `WANTED_FEATURES` intersected with the adapter's own,
   and after `initAsync` checks `engine.enabledExtensions` against what was asked. The wrapper
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
   status store, disposes the engine and every view's context, and recreates them through
   `loadRenderEngine`. The probe saw a GPU-process crash reach the page as `device.lost` with reason
   `unknown`, and no `uncapturederror`; a null adapter on the retry means Chromium has dropped
   WebGPU, which is the `disabled` condition. After `DEVICE_LOSS_LIMIT` (3) losses in a session the
   client stops recreating and shows `disabled`, which matches the brainstorm's "three such losses
   disable WebGPU for the session" and Chromium's own count.

10. **Faults are the console's annunciations, not alerts.** The brainstorm asks for a "ship-system
    fault in the guide's language"; the guide says alerts are raised by the server and "a console
    never invents one" (`docs/frontend/ux-guidelines.md`, Alerts). Researched 2026-09-29 by an
    advisor agent against the guide: the reading closest to both is a Fault the console raises about
    itself, as `SYSTEM DATA INVALID` already is, presented through `StatusLine`'s `fault` standing
    (`--status-caution` text, no tone, no flash, not in the header's alert counts), which the
    guide's "a failed system is being reported" allows. A lost device and a crashed GPU process are
    faults while they last; a refused software adapter, the safe mode and the disabled state are the
    console stating its own condition in `--text`, as the brainstorm's item 7 does for
    `TERRAIN: DETAIL LIMITED`. They are never promoted to a Caution. The wording, drafted in T5.c
    for the owner, is `GRAPHICS SOFTWARE ADAPTER: PHOTOREALISTIC STYLE UNAVAILABLE`,
    `GRAPHICS DEVICE LOST: re-creating`, `GRAPHICS PROCESS RESTARTED`,
    `GRAPHICS SAFE MODE: views unavailable, relaunch to retry` and
    `GRAPHICS DISABLED: 3 DEVICE LOSSES, relaunch to retry`. They show in a `GRAPHICS` panel on the
    `LINK` display; the safe and disabled states, being declared modes, also stand in the header
    strip's status area, as the guide puts every mode banner there. Every later view carries the
    current annunciation in its label block (R02).

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

13. **One device, one canvas context per view, two internals pinned.** Researched 2026-09-29 in
    9.28.0: there is no public way to render a camera into an external canvas context. The route:
    each `RenderView` configures its own `GPUCanvasContext` against `engine._device` (declared
    `/** @internal */ _device: GPUDevice`, `webgpuEngine.pure.d.ts:216`) at the view's own size,
    with `COPY_SRC` added to its usage so that the harness can read it back; wraps the first
    `getCurrentTexture()` with `engine.wrapWebGPUTexture` (`:805`); builds a `RenderTargetTexture`
    with that as its `colorAttachment` and `generateDepthBuffer: true`, which gives the view its own
    `depth32float` (`Engines/WebGPU/Extensions/engine.renderTarget.pure.js:46-54`); sets it as the
    camera's `outputRenderTarget` (`Cameras/camera.pure.d.ts:241`); sets `_disableEngineYFlip` on
    the target's wrapper, which lives on `WebGPURenderTargetWrapper`, not the engine
    (`Engines/WebGPU/webgpuRenderTargetWrapper.d.ts:20`); and each frame calls
    `updateWrappedWebGPUTexture` (`:822`) with the new current texture, which throws if the size
    changed, so a resize rebuilds only that view's target. Both internals are declared in the
    `.d.ts`, so a Babylon upgrade that renames them fails `pnpm typecheck`; one file,
    `view/engine/babylon/internals.ts`, holds both accesses, each behind a one-line
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
    declared `presentation-only` and may never be read back (the adapter's `readBack` refuses it).
    `assertNoF16Subgroups` rejects a module that enables both `f16` and `subgroups`, a coarse but
    mechanical form of the brainstorm's "subgroup operations never take f16 operands". On
    SwiftShader, which exposes subgroups but not `shader-f16`, and the UHD 620 with the Gen9 toggle,
    both paths exist to be tested. The UHD 620 does expose `shader-f16` under the forced switches
    (probe A, and R07's research, `/tmp/rp-plans/research/R07-b.md`); the "no `shader-f16`" list is
    SwiftShader's alone, so the f16 path is exercised by hand on the development machine and the
    no-f16 path automatically in the harness.

17. **The harness runs headless, with more switches than the brainstorm lists.** Researched
    2026-09-29 by probe on Electron 44.4.3 (`/tmp/r01-research-c/`, not in the repository): the
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
      R05's. The harness checks all three (T9.d).
    - _Depth bias per material, not global._ `depthBiasAway` maps to the pipeline's depth bias with
      the sign flipped under reversed depth, so that positive always moves a fragment away. The
      brainstorm's "no global depth bias" stands: this is a per-material setting R02 uses for its
      lines, not a scene-wide one.
    - _Linear-light blending._ Each canvas is configured with its preferred 8-bit format and
      `viewFormats` holding its `-srgb` twin, and the view renders through the sRGB view, so
      blending happens in linear light and the store encodes. If Babylon's `wrapWebGPUTexture`
      cannot take a view format, the fallback is R02's HDR target with an explicit encode in the
      final pass, recorded as a deviation.
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

## Tasks

T1 and T2 are the main process and can run in parallel. T3 needs T2's preload field. T4 follows T3,
T5 follows T4. T6 (interface) can start at any time; T7 (dependency and chunk) follows T6; T8
(engine creation and guard) follows T3 and T7; T9 (harness) follows T8; T10 (kernels) follows T9.
T11–T13 are by hand, after T9. T14 closes.

Paths are under `apps/hyperion/` unless given in full.

### R01.T1 The Linux switches

**R01.T1.a The switch builder.** `src/main/graphics/switches.ts`: `graphicsSwitches`,
`mergeSwitchValue`, `applyGraphicsSwitches`, the switch-name constants. Linux returns the set of
Design note 2; `mode: "safe"` drops `Vulkan`, `VulkanFromANGLE`, `DefaultANGLEVulkan` and
`use-angle=vulkan` and keeps nothing else; `gpuTiming` adds
`disable-dawn-features=timestamp_quantization` (Design note 4); every other platform returns an
empty list.

- Tests (`switches.test.ts`, logic project): the Linux set exactly, in order; no platform or mode
  ever yields `enable-unsafe-webgpu` or `use-webgpu-adapter`; safe mode has no `Vulkan` feature;
  timing adds only the Dawn toggle; `mergeSwitchValue("A,B", ["B","C"])` is `"A,B,C"`, empty
  existing gives the added list, duplicates are dropped; `applyGraphicsSwitches` on a fake
  `CommandLine` holding `enable-features=Foo` appends one `enable-features=Foo,Vulkan,…`.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/graphics/switches.test.ts`.

**R01.T1.b Applying them, and the X11 relaunch.** In `src/main/index.ts`, before `whenReady`:
`x11RelaunchArgs` (Design note 3) and, when it answers, `app.relaunch` and `app.exit(0)`; then
`app.disableDomainBlockingFor3DAPIs()` and `applyGraphicsSwitches` for the launch's mode (read from
`SAFE_MODE_SWITCH`) and timing flag. The `just client` recipe's comment says that
`--hyperion-gpu-timing` may be given after its `--`.

- Tests (`x11Relaunch.test.ts`): Linux with `XDG_SESSION_TYPE=wayland` and no flag gives the argv
  plus `--ozone-platform=x11`; with the flag already present, under `x11` or unset, and on other
  platforms, `undefined`; the relaunch never duplicates the flag.
- By hand, recorded in this plan's as-built notes: on the development machine, `just client`, then
  `navigator.gpu.requestAdapter()` in the devtools console gives `intel`/`gen-9` with `subgroups`;
  `chrome://gpu` is not reachable in the client, so the check is the adapter and
  `app.getGPUFeatureStatus()` logged after `gpu-info-update`.
- Acceptance: the test file passes, and `just ci`.

### R01.T2 GPU-process monitoring and the safe mode

**R01.T2.a The crash-loop policy.** `src/main/graphics/crashLoop.ts`: `GpuProcessEvent`,
`crashLoopDecision`, the two constants (Design note 6). Pure.

- Tests: two crashes within the window give `none`, three give `relaunch-safe`; three spread over
  more than the window give `none`; `clean-exit` never counts; a status event with `vulkan` other
  than `enabled_on` gives `relaunch-safe` at once in `vulkan` mode; nothing ever relaunches in
  `safe` mode; the decision depends on the events and not on their arrival order within a timestamp.
- Acceptance: `vitest run src/main/graphics/crashLoop.test.ts`.

**R01.T2.b The monitor, the relaunch and the preload.** `GpuProcessMonitor` subscribes to
`child-process-gone` (GPU only) and `gpu-info-update` (reading `getGPUFeatureStatus()` then), keeps
the history, sends each crash to every window's `webContents` on one channel, and on `relaunch-safe`
calls `app.relaunch({ args: [...argv, "--hyperion-graphics-safe"] })` and `app.exit(0)`. The preload
gains `graphics.launchMode` (from its argv, as `serverUrl` is) and `graphics.onGpuProcessGone`, a
narrow function over one fixed channel, under the Electron security rules: no channel name crosses
the bridge. `stubHyperionApi.ts` gains the field.

- Tests: the monitor against a fake `app` and `webContents` (logic project): events in, sends and
  the one relaunch out; the preload's argv parse (`launchMode` is `safe` only with the switch).
- By hand, recorded: `kill -9` of the GPU process three times within a minute during `just client`
  relaunches once into safe mode, whose adapter request is null and whose consoles work; a fourth
  crash does not relaunch again.
- Acceptance: the tests pass, and `just ci`.

### R01.T3 Adapter acquisition

`src/renderer/src/view/engine/platform.ts` and `src/renderer/src/test/fakeGpu.ts`:
`summariseAdapter`, `styleAvailability`, `requestAdapterOutcome` (with
`powerPreference: "high-performance"`, which the probe found changes nothing on one GPU but is right
on two), `WANTED_FEATURES`, `CapabilityOverrides` (used only by the harness page). No engine import.

- Tests (logic project, with `FakeGpu`): no `navigator.gpu` gives `no-webgpu`; a null adapter
  `no-adapter`; the Intel probe's info (`intel`, `gen-9`, `isFallbackAdapter` false) gives both
  styles; SwiftShader's (`google`, `swiftshader`, true) gives wireframe only, and so does either
  signal alone; capabilities read each feature and `subgroupMinSize`; overrides withhold subgroups
  or f16.
- Acceptance: `vitest run src/renderer/src/view/engine/platform.test.ts`.

### R01.T4 Graphics status and device loss

`view/engine/status.ts`: `GraphicsStatus`, `GraphicsEvent`, `reduceGraphicsStatus`,
`graphicsAnnunciation` (the strings of Design note 10), `GraphicsStatusStore` and
`useGraphicsStatus` (over `useSyncExternalStore`), and the store's feed from
`window.hyperion.graphics`.

- Tests: each adapter outcome's condition; safe launch mode gives `safe-mode` whatever the adapter;
  a device loss sets the fault and counts, a restore clears the fault and keeps the count; the third
  loss gives `disabled` and later restores do not leave it; a GPU-process report sets its fault with
  the count; each annunciation's text and standing; the hook re-renders on dispatch and unsubscribes
  on unmount.
- Acceptance: `vitest run src/renderer/src/view/engine/status.test.ts` and the hook's test.

### R01.T5 The `GRAPHICS` panel and its words

**R01.T5.a The panel.** `components/GraphicsPanel.tsx` on the `LINK` display after `Server Link`, in
`ConnectionPanel`'s readout pattern: `Adapter` (vendor · architecture), `Software Adapter`
(`YES`/`NO`), `Features` (the wanted ones present), `Styles`, `Mode` (`VULKAN` or `SAFE`),
`GPU Timer` (`QUANTIZED` or `FULL`), `Device Losses`, `Process Restarts`, and the current
annunciation through `StatusLine`. Missing values are the em dash.

- Tests (dom project): each condition's rows and annunciation, found by role and text; no status
  colour on a nominal panel; a fault's text carries the `fault` class.
- Acceptance: `vitest run src/renderer/src/components/GraphicsPanel.test.tsx`; the console-ux
  skill's lint, contrast and glyph scripts pass on the changed files.

**R01.T5.b The mode in the header strip.** `ConsoleFrame` shows the `GRAPHICS SAFE MODE` or
`GRAPHICS DISABLED` annunciation in its status area while it holds, beside the link status, never in
the alert counts.

- Tests: present in safe mode and after the third loss, absent otherwise, and identical on every
  display.
- Acceptance: `vitest run src/renderer/src/components/ConsoleFrame.test.tsx`.

**R01.T5.c Nomenclature drafts, for the owner.** A draft of the guide's new entries: `GRAPHICS`
(system); the statuses `GRAPHICS SOFTWARE ADAPTER`, `GRAPHICS SAFE MODE`, `GRAPHICS DISABLED`; the
faults `GRAPHICS DEVICE LOST` and `GRAPHICS PROCESS RESTARTED`; `UNAVAILABLE` (the owner may prefer
`NOT AVAILABLE`); `VULKAN`, `SAFE`, `QUANTIZED`, `FULL`; and one sentence under Alerts that a
console's report on its own graphics is a Fault or status, not an alert. `PHOTOREALISTIC` and
`WIREFRAME` wait for R02's view class. Written as a proposed diff in the commit message and in this
plan's as-built notes, not applied: guide additions are the owner's call. R02's single pass absorbs
and re-checks it. The code does not wait for the answer, since every string is one constant in
`status.ts`.

- Acceptance: the owner signs off, or amends; the constants then change in one commit.

### R01.T6 The engine-agnostic interface

`view/engine/types.ts`, `view/engine/kernels.ts` (types only here), `view/engine/catalogue.ts` (an
empty `WGSL_CATALOGUE` of `{ kind: "material" | "post-process" | "compute", spec }`), and
`view/engine/engineBoundary.test.ts` (Design note 1).

- Tests: the boundary test finds every `.ts`/`.tsx` under `src/renderer/src`, and fails on an
  `@babylonjs/` import outside `view/engine/babylon/` and on the text `registerView` anywhere; it
  passes on the tree, and fails on a fixture string that breaks each rule.
- Acceptance: `pnpm typecheck` and the test.

### R01.T7 The dependency and the lazy chunk

`pnpm add --filter hyperion @babylonjs/core@9.28.0 --save-exact`, with the pin's reason (Design
note 15) in the module comment of `view/engine/babylon/engine.ts`, since `package.json` holds no
comments; `view/engine/loadEngine.ts` with the dynamic import and a placeholder `babylon/engine.ts`
that exports `createBabylonEngine`; the `codeSplitting` group in `electron.vite.config.mts` (Design
note 14); `scripts/checkChunks.mjs` over `out/renderer`.

- Tests: `loadEngine.test.ts` mocks the dynamic module (a third-party boundary, so `vi.mock` is
  allowed) and checks that nothing is imported until `loadRenderEngine` is called.
- Acceptance: `pnpm build` then `node apps/hyperion/scripts/checkChunks.mjs` passes; the entry chunk
  holds no `@babylonjs` module; `just ci`.

### R01.T8 The Babylon engine

**R01.T8.a Creation.** `view/engine/babylon/engine.ts`: the adapter wrapper on
`navigator.gpu.requestAdapter` (Design note 7), the fixed options (Design note 11), the feature
check after `initAsync`, the loss handling that feeds the status store and rebuilds (Design note 9),
and `RenderEngine`'s `createMesh`, `createMaterial` (WGSL `ShaderMaterial` only),
`createPostProcess` and `createCompute` over Babylon's WGSL paths.

- Tests (logic project, Babylon's `NullEngine` is not WebGPU, so these run on the pure parts): the
  requested features are the intersection; a feature asked and not enabled is reported; the wrapper
  returns the vetted adapter on a second call; the options object has
  `useLargeWorldRendering: false`, `stencil: false` and `doNotHandleContextLost: true`. The rest is
  exercised by T9.
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
current texture with `copyTextureToBuffer` and maps it. A pure
`viewPixelSize(cssSize, devicePixelRatio, maxTextureDimension2D)` rounds and clamps.

The canvas is configured with its `-srgb` view format, and the right-handed scene, explicit cull
mode, per-material depth bias and `alwaysSelectAsActiveMesh` of Design note 18 apply.

- Tests: `viewPixelSize` rounds, clamps to the limit and never returns zero; the depth-bias sign
  mapping under reversed depth; `internals.ts` type-checks against the pinned `.d.ts` (a typecheck
  failure is the pin's alarm).
- Acceptance: `pnpm typecheck`, the tests, `just ci`.

**R01.T8.d Resources and the packed cube.** `view/engine/memory.ts` (`MemoryCategory`,
`textureBytes`, `AllocationEvent`) and the adapter's `createBuffer`, `createTexture`, `dispatch`,
per-draw textures in `DrawItem`, and `createPackedCube` and `writePackedCubeLevel` through
`_hardwareTexture` in `internals.ts` (Design note 18). The boundary test gains the rule on
`device.createBuffer` and `device.createTexture`.

- Tests: `textureBytes` for 2D, 3D, cube, mip chains and `rgb9e5ufloat` against hand-computed sizes;
  every create and destroy raises one event with its category; the boundary rule fails on a fixture.
- Acceptance: the tests; T9.d's cube round trip.

### R01.T9 The headless smoke harness

**R01.T9.a The runner and the readback.** `src/smoke/main.ts` and `src/smoke/preload.ts` (an
Electron entry with its own one-call preload), `src/renderer/smoke.html` with its script (a second
renderer input in `electron.vite.config.mts`, beside `index.html`), and `just test-render`, which
runs `pnpm --filter hyperion build`, then the harness under the heavy-test lock with its switches as
an array (Design note 17), once per variant named in the page's query string. The page acquires the
adapter through `platform.ts`, loads the engine through `loadEngine.ts`, renders one frame of a
clear and a triangle into an offscreen `rgba16float` target and into a view, reads both back, and
asserts properties: every texel finite, the clear colour where nothing was drawn, the triangle's
colour at its centroid. The runner prints the adapter summary and one line a check, and exits with
Design note 17's codes.

- Acceptance: `just test-render` passes on the development machine with `DISPLAY` unset; it exits 1
  when a deliberately broken WGSL module is added to the catalogue, and 2 when run without the
  SwiftShader switches.

**R01.T9.b Offline, every catalogued shader.** The runner cancels and records every request that is
not `file:` or `data:`, through `session.defaultSession.webRequest.onBeforeRequest` over
`<all_urls>`, and fails naming each URL with its resource type; a fixture page that fetches an
external URL proves the check fails. The page renders each `WGSL_CATALOGUE` entry once and asserts
the frame's properties and that the WGSL guard reported nothing. A test entry that uses a GLSL
shader must fail with its name.

- Acceptance: `just test-render` passes with the network cable in or out; the GLSL fixture fails
  with `GlslShaderRefused` and its name, and no request leaves the process.

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
views' readbacks and sizes unchanged. Further checks, from Design note 18: a point drawn at a known
distance under a fixed reversed-Z projection writes the predicted depth (read back from the depth
texture), with no half-Z conversion; a back-facing triangle is culled only under `cullMode: "back"`;
a coplanar line with `depthBiasAway` loses to the surface; 50% coverage of white over black reads
back near 188 of 255, not 128 (linear-light blending); a compute kernel writes a 3D storage texture
that a draw then samples; and a two-level packed cube written with `writePackedCubeLevel` samples
back to the packed values.

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

### R01.T10 Subgroup twins

`view/engine/kernels.ts`: `selectKernel` and `assertNoF16Subgroups` (Design note 16), and a toy
`bit-exact` pair in the catalogue, an integer sum of a buffer of 2¹⁶ `u32` by workgroup reduction,
with and without `subgroupAdd`, whose results the harness compares byte for byte across T9.c's runs;
`readBack` refuses a `presentation-only` pair.

- Tests: selection by features for each combination; a pair without a subgroup variant always takes
  the reference; a module with `enable f16;` and `enable subgroups;` throws with its name; the
  harness's comparison passes.
- Acceptance: the tests and `just test-render`.

### R01.T11 The three canvases on the UHD 620, by hand

Run the T9.d scene in the client's graphics configuration (not SwiftShader) with
`--hyperion-gpu-timing`, full-window cockpit and two instruments, for five minutes with resizes.
Record: GPU time per view from `timestamp-query`, that no copy pass exists (the frame's passes are
the three views' own), the frame interval at the 50th and 95th percentiles, and a screenshot by eye
that each view is the right way up.

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

R07's research has already probed the core (`/tmp/rp-plans/research/R07-b.md` §5.2, 2026-09-29): a
same-origin child opened with `window.open`, its canvas configured against the opener's `GPUDevice`,
renders, reaches the screen and paces at 16.7 ms from either window's `requestAnimationFrame`;
closing the child is not a device loss, but the next submit to its context raises an uncaptured
validation error, "context configuration is invalid", while the device and the main view carry on.
So this task finishes the prototype on a scratch branch, not merged: record a resize of the child,
whether it stays in the opener's process (`app.getAppMetrics()`), and, if a second display can be
borrowed, its pacing there. It also writes down the rule a client that opens such windows must
follow, for R07 to consume: drop the child's `RenderView` on its `pagehide` before the next frame,
and treat that one validation error from a closing child as expected, not as a fault.

- Acceptance: the result recorded beside R07's; open question 15's second half answered for the
  brainstorm; R07.T21 consumes the record.

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
  resizes, and the bit-exact toy kernel across paths.
- **By hand, recorded:** the adapter on the UHD 620 (T1.b); the crash-loop relaunch (T2.b); the
  three canvases' GPU time with no copies (T11); the soak (T12); child windows (T13).
- **By eye:** the `GRAPHICS` panel in each condition, and the header banner in safe mode.

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
  command line. The packaged launcher removes it; in development,
  `just client -- --ozone-platform=x11` avoids it. Whether an Electron relaunch from
  `electron-vite dev` keeps the dev server is to be checked in T1.b; if not, the recipe passes the
  flag itself.
- **Chromium's thresholds were observed, not read.** Three crashes to drop Vulkan and six to drop
  GPU compositing come from the probe, not from `gpu_process_host.cc`; the policy also watches the
  feature status, so it holds if the counts differ.
- **Babylon's internals.** `_device` and `_disableEngineYFlip` are `@internal`; the typecheck and
  the orientation check catch a rename or a change of meaning, and the fallback without them is a
  CSS flip with picking flipped to match, which still needs the device (open question 15).
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
- **Asked by R07, not yet a task here.** A probe of each adapter's render-target rounding mode,
  stated in `GraphicsStatus` (R07 Design note 12: Gen9's render-target writes round toward zero,
  so R07's bloom stays `rgba16float` and its CPU twin models truncation). The roadmap's asks table
  carries it until a task here or in R07 builds it.
