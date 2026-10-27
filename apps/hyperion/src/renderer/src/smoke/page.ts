/**
 * The headless smoke harness's page (R01.T9, Design note 17): it acquires the adapter through
 * `platform.ts`, loads the engine through `loadEngine.ts`, runs every check against read-back
 * frames and reports once to the harness's main process, which prints and judges the run.
 *
 * @remarks
 * Its variant comes in the query string: `default` runs the device as the adapter offers it,
 * `no-subgroups` requests it with subgroups and `depth-clip-control` withheld (T9.c; R10.T9.b's
 * pulled-back cascades). A fixture adds a broken shader to the
 * catalogue or makes external requests, either of which must fail the run. The part does too: the
 * groups are in twelve parts, each a run of its own under the harness's watchdog (perf-smoke,
 * 2026-10-13), parts 2 to 6 and 8 to 12 the atmosphere's CPU-twin groups (R08.T6, R08.T7,
 * R08.T9.b, R08.T8.a, R08.T8.b, R08.T8.c, R08.T8.e, R08.T11), part 4 with R05.T12.e's marches
 * before them and part 6 with R05.T12.c's frames, part 7 R10's terrain groups, and part 1 every other; with no
 * part given the page runs them all, in its order.
 * Each part checks its path (T9.c) itself. R08.T6.f's slices and bands run in two groups: the
 * Saturn-class figure's in part 5, with R08.T6.i's layer edges, and Earth's, which took R08.T6.g's
 * Earth, in part 8, with R07.T8.e–f's cell pass for every record (`cellPass.ts`) after it;
 * R08.T11.b's face count in part 12; R08.T11.e's unread tables in two, a giant's in part 9 after
 * R08.T8.a's march band and Earth's in part 12 after R08.T11.b's face count; R08.T6.b's kernels
 * over N terms in two, its medium of four
 * absorbers in part 6 and the others in part 2; R08.T9.b's surface lighting in two, Earth's and the
 * Saturn-class figure's, in part 11 before R08.T8.c's footprint; R08.T8.a's march band in part 9 (in part 6 it took the part from
 * 94 s to 140 to 146 s at a load of 2 to 5, the group 64 to 73 s, mostly its four atmospheres'
 * tables and kernels, 2026-10-21); R08.T7's several suns in part 10 with R08.T8.e's slot layers on
 * its binary; R08.T8.b's far limb, with the composite from above
 * the top, in part 4 after R05.T12.e's marches, and its low pick, which has no twin, in part 1;
 * R08.T8.c's far limb's footprint, its frames from 10⁸ m at the output's own pixel, in part 11;
 * and part 7 R10.T10.b's lit terrain, R10.T10.c's survey edge, R10.T16.c's
 * frame, R10.T11's wireframe terrain, R10.T9.b's cascades and R10.T16.b's shared pass (on an
 * engine of its own, whose device it destroys). Three groups of part 1 run only when asked:
 * R08.T8.c's far limb's timings and, after them, R08.T11's photorealistic frame's timings
 * (`frameTiming.ts`), after the low pick, when the run lifted timestamp quantization (`gpuTiming`,
 * the hardware runs of R01.T11), and R08.T8.c's far limb captures, after R07.T10.c's eclipse
 * captures, with the other captures; a plain `just test-render` runs none of them.
 */

import { WGSL_CATALOGUE } from "../view/engine/catalogue";
import { loadRenderEngine } from "../view/engine/loadEngine";
import {
  type CapabilityOverrides,
  type GpuCapabilities,
  requestAdapterOutcome,
  styleAvailability,
} from "../view/engine/platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "../view/engine/status";
import {
  captureAscent,
  captureAtmosphere,
  type CapturedImage,
  captureFarLimb,
  checkAtmosphereFrames,
  checkAtmosphereHeights,
  checkAtmosphereLayerEdges,
  checkAtmosphereSlicesEarth,
  checkAtmosphereSlicesSaturn,
  checkAtmosphereSteps,
  checkAtmosphereTables,
  checkAtmosphereTwins,
  checkAtmosphereTwinsFourAbsorbers,
  checkBinarySlotLayers,
  checkFaceCount,
  checkFarLimb,
  checkFarLimbFootprint,
  checkLowPick,
  checkMarchBand,
  checkSeveralSuns,
  checkSurfaceLightingEarth,
  checkSurfaceLightingSaturn,
  checkUnreadEarth,
  checkUnreadGiant,
  timeFarLimb,
} from "./atmosphere";
import { checkAtmosphereFrame } from "./atmosphereFrame";
import { checkCellPassEveryRecord } from "./cellPass";
import { timeAtmosphereFrame } from "./frameTiming";
import { checkBlendComputeCube, checkMaterialState, checkSplatRefused } from "./blending";
import { checkBloom } from "./bloom";
import { BROKEN_ENTRY, checkCatalogue, makeExternalRequests, type SmokeFixture } from "./catalogue";
import { checkDepthLayers } from "./depthLayers";
import { captureEclipse, checkEclipse } from "./eclipse";
import { addCanvas, checkClearAndTriangle, checkDepthCullBias, checkThreeCanvases } from "./frames";
import { Checks } from "./harness";
import { checkHeightWorker } from "./heightWorker";
import { checkHistogram } from "./histogram";
import { checkLitBody } from "./litBody";
import { checkLitTerrain } from "./litTerrain";
import { checkSurveyEdge } from "./surveyEdge";
import { captureOccultation, checkMeshBodies } from "./meshBodies";
import { checkSkyBake, checkSkyBand, checkSkyDisc } from "./sky";
import { checkPhotoreal } from "./photoreal";
import { checkBodies, checkClassMap, checkPhotorealFrame, checkSpriteDepth } from "./bodies";
import { runChildWindow } from "./childWindow";
import { runSoak } from "./soak";
import { captureSpike } from "./spike";
import { captureTerrain, checkTerrainFrames, checkTerrainResources } from "./terrain";
import { checkTonemap } from "./tonemap";
import { TwinPool, twinWorkerCount } from "./twinPool";
import { checkTwins } from "./twins";
import { checkSpatialStrokeContrast } from "./spatial";
import { checkStrokeContrast } from "./strokeContrast";
import { checkWireframe } from "./wireframe";
import { checkShadowCascades, checkSharedPass } from "./sharedPass";
import { checkTerrainFrame } from "./terrainFrame";
import { checkWireframeTerrain } from "./wireframeTerrain";
import { checkForcedLoss, checkOneLayerArrays, checkTargetsAsyncIndirectTiming } from "./work";

/** What the page reports, as `src/smoke/result.ts` reads it. */
interface Report {
  readonly variant: string;
  readonly adapter: string | null;
  readonly capabilities: string | null;
  readonly checks: Checks["list"];
  readonly setupError: string | null;
  /** R05.T12.c's comparison frames, when the run was asked for them. */
  readonly images?: ReadonlyArray<CapturedImage>;
}

/**
 * The page's variants and the capabilities each withholds.
 *
 * @remarks
 * `no-subgroups` also withholds `depth-clip-control` (plan R10, R10.T9.b): SwiftShader offers it
 * in `just test-render` (the depth-layers group logs "depth-clip-control yes"), as R01's record of
 * its engine task has it ("the rest present") and `platform.test.ts`' `SWIFTSHADER_FEATURES` now
 * does, so the default run takes the shadow cascades' unclipped path, `unclippedDepth` pancaking the
 * casters in front, and `no-subgroups` the pulled-back near plane that every adapter without the
 * feature takes (decision-r10-t9b-design D2).
 */
const VARIANTS: Readonly<Record<string, CapabilityOverrides | undefined>> = {
  default: undefined,
  "no-subgroups": {
    withholdSubgroups: true,
    withholdShaderF16: false,
    withholdDepthClipControl: true,
  },
};

/**
 * A part of the page's groups: 2 to 6 and 8 to 12 the atmosphere's CPU-twin groups (R08.T6,
 * R08.T7, R08.T9.b, R08.T8.a, R08.T8.b, R08.T8.c, R08.T8.e, R08.T11), whose twins outweigh their
 * GPU work (2 R08.T6.b's but its medium of four absorbers, 3 R08.T6.g's, 4 R08.T8.b's far limb, with the
 * composite from above the top, 5 R08.T6.f's Saturn-class figure and R08.T6.i's layer edges, 6
 * R08.T6.b's medium of four absorbers, 8 R08.T6.f's Earth, 9 R08.T8.a's march band, 10 R08.T7's
 * several suns and R08.T8.e's slot layers on its binary, 11 R08.T9.b's Earth and Saturn-class
 * figure and R08.T8.c's far limb's footprint, 12 R08.T11.b's face count and R08.T11.e's Earth), 7
 * R10's terrain groups (R10.T10.b, T10.c, T11, T16 and T9.b), which took part 6 to 163 s at a load of 2 once R10.T9.b's cascades
 * joined them (2026-10-21), and 1 every other group but R05.T12.e's and R05.T12.c's frames, which
 * parts 4 and 6 run first, before their twins start, to even the parts out (R08.T9.b's lit
 * materials, larger, took part 1 past 150 s under load), R08.T8.b's low pick, which has no twin,
 * among them; part 1 also takes the captures, R08.T8.c's far limb's among them, when asked, under
 * the same watchdog, and R08.T8.c's far limb's timings and R08.T11's photorealistic frame's
 * timings when the run lifted timestamp quantization (`gpuTiming`), under an allowance of their
 * own (`result.ts`' `GPU_TIMING_WATCHDOG_MS`).
 *
 * @remarks
 * R08.T8.c's footprints, whose nodes the twins and SwiftShader march on every limb from above the
 * top, took part 4 past its 180 s watchdog (R05.T12.e 89 s, R08.T7 54 s, R08.T8.b's far limb
 * about 40 s) and parts 2, 3, 5 and 6 to 149 to 159 s (2026-10-22), so parts 8 and 9 joined, and
 * R08.T6.i, R08.T6.f's Earth, R08.T8.a's march band and R08.T7 moved to them and to part 5: at
 * those figures part 3 holds 122 s, part 4 129, part 5 117, part 6 80, part 9 130 and part 8 68
 * with R08.T8.c's footprint beside it, every part about 130 s at most but part 2, R08.T6.b's alone
 * at 149 s. R08.T8.e's slot layers and the ray march's kernel by slot then took part 2 past the
 * watchdog (175 s at 100bee27's gate), so R08.T6.b's medium of four absorbers moved to part 6 in a
 * group of its own (59 to 67 s) and R08.T9.b's Saturn-class figure to part 2 (19 s): on SwiftShader
 * at a load of 7 to 12 part 2 holds 140 s (R08.T6.b's group 116 s) and part 6 122 to 124 s
 * (2026-10-23). With the slot layers' texel checks (R08.T6.b's absorbing media and R08.T7's binary)
 * part 2 holds 141 s, part 5 119, part 6 133 to 137 and part 9 158 to 160 at a load of 7 to 14, so
 * the binary's, with its kernel arm, left R08.T7's group for a group of its own (13 s), and with it
 * in part 1 part 9 held 152 to 154 s (R08.T8.a's march band 79 to 81 s, R08.T7's 71 s) and part 1
 * 122 to 123 s at a load of 8 to 11 (2026-10-23). No group of part 9 fitted another part whole, so
 * part 10 joined, with R08.T7's several suns and the binary's slot layers beside it: on the whole
 * render at a load of 9 to 20 (median 14) part 1 holds 131 s, part 9 88 s and part 10 98 s
 * (R08.T7's 79 s, the binary's slot layers 15 s), its runs 3 s outside their groups, Electron's
 * start, the adapter and the page's load (2026-10-23). Part 8 then
 * stood at 151 to 153 s at a load of 8 to 13 (R08.T6.f's Earth 86 to 87 s, R08.T8.c's footprint
 * 64 s), so part 11 joined too, with R08.T8.c's footprint alone. On the whole render at a load of
 * 9 to 20 (median 14), each part's longer variant: part 1 131 s, 2 149, 3 121, 4 138, 5 131, 6 154,
 * 7 126, 8 93 (R08.T6.f's Earth 91 s), 9 88, 10 98 and 11 51 (the footprint 50 s), each run 1 to
 * 7 s outside its groups; the whole render 1,281 s, parts 9 and 10 together 186 s and parts 8 and
 * 11 144 s against 152 to 154 s and 151 to 153 s as one part each at a load of 8 to 13, so the two
 * parts' starts and their twins set up again cost about 30 s of the render's wall clock
 * (2026-10-23). R08.T11.b's face count (49 to 69 s) took part 8 past the watchdog beside
 * R07.T8.e–f's cell pass (185 s at a load of 13, R08.T6.f's Earth 79 s), so part 12 joined, with
 * it (2026-10-27), and R08.T11.e's Earth (60 s), which took part 10 to 164 s, after it.
 */
type PagePart = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12;

/** The part a run's query names, `all` for none, or `null` for one that is not a part. */
function partOf(value: string | null): PagePart | "all" | null {
  switch (value) {
    case null:
    case "all":
      return "all";
    case "1":
      return 1;
    case "2":
      return 2;
    case "3":
      return 3;
    case "4":
      return 4;
    case "5":
      return 5;
    case "6":
      return 6;
    case "7":
      return 7;
    case "8":
      return 8;
    case "9":
      return 9;
    case "10":
      return 10;
    case "11":
      return 11;
    case "12":
      return 12;
    default:
      return null;
  }
}

/** Whether `value` is a known fixture. */
function isFixture(value: string): value is SmokeFixture {
  return value === "none" || value === "broken-wgsl" || value === "external-fetch";
}

/** The capability line: the path each feature puts the run on. */
function capabilityLine(capabilities: GpuCapabilities): string {
  return [
    `subgroups ${capabilities.subgroups ? `yes (min ${capabilities.subgroupMinSize ?? "?"})` : "no"}`,
    `shader-f16 ${capabilities.shaderF16 ? "yes" : "no"}`,
    `timestamp-query ${capabilities.timestampQuery ? "yes" : "no"}`,
    `float32-blendable ${capabilities.float32Blendable ? "yes" : "no"}`,
    `rg11b10 ${capabilities.rg11b10Renderable ? "yes" : "no"}`,
  ].join(", ");
}

/** Every device the page's adapters hand out, so that the loss check can destroy the engine's. */
const devices: GPUDevice[] = [];

/** Wraps `navigator.gpu.requestAdapter` so that each adapter's devices are recorded. */
function recordDevices(gpu: GPU): void {
  const request = gpu.requestAdapter.bind(gpu);
  gpu.requestAdapter = async (options?: GPURequestAdapterOptions): Promise<GPUAdapter | null> => {
    const adapter = await request(options);
    if (adapter !== null) {
      const requestDevice = adapter.requestDevice.bind(adapter);
      adapter.requestDevice = async (descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> => {
        const device = await requestDevice(descriptor);
        devices.push(device);
        return device;
      };
    }
    return adapter;
  };
}

async function run(
  variant: string,
  fixture: SmokeFixture,
  part: PagePart | "all" | null,
): Promise<Report> {
  const failed = (setupError: string): Report => ({
    variant,
    adapter: null,
    capabilities: null,
    checks: [],
    setupError,
  });
  if (part === null) {
    return failed("the part asked for is not 1 to 12 or all");
  }
  const gpu: unknown = Reflect.get(navigator, "gpu");
  if (gpu === undefined) {
    return failed("navigator.gpu is absent: no WebGPU");
  }
  recordDevices(navigator.gpu);
  const outcome = await requestAdapterOutcome(navigator.gpu);
  if (outcome.kind !== "adapter") {
    return failed(`no adapter: ${outcome.kind}`);
  }
  const { summary } = outcome;
  const checks = new Checks();
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", gpuTiming));
  const engine = await loadRenderEngine(
    outcome,
    status,
    VARIANTS[variant] === undefined ? {} : { overrides: VARIANTS[variant] },
  );
  const device = devices.at(-1);
  const capabilities = capabilityLine(engine.capabilities);
  if (fixture === "external-fetch") {
    await makeExternalRequests();
  }

  // T9.c: the adapter summary, and the path the run is on.
  checks.check(
    "T9.c SwiftShader is a fallback adapter",
    summary.architecture !== "swiftshader" || summary.fallback,
    `${summary.vendor}/${summary.architecture}, fallback ${String(summary.fallback)}`,
  );
  checks.check(
    "T9.c the photorealistic style is refused on a fallback adapter",
    !summary.fallback || !styleAvailability(summary).photorealistic,
    `photorealistic ${String(styleAvailability(summary).photorealistic)}`,
  );
  // The default run takes the subgroup path wherever the adapter offers subgroups, so that T10's
  // twins run on both paths across the two variants; the no-subgroups run never does.
  checks.check(
    `T9.c the ${variant} run is on its path`,
    variant === "no-subgroups"
      ? !engine.capabilities.subgroups
      : engine.capabilities.subgroups === outcome.capabilities.subgroups,
    `${capabilities}; the adapter offers subgroups: ${outcome.capabilities.subgroups ? "yes" : "no"}`,
  );
  const rounding = status.getSnapshot().targetRounding;
  checks.check(
    "T9.i the rounding probe settles rgba16float",
    rounding.rgba16float !== "unknown",
    `targetRounding ${JSON.stringify(rounding)}`,
  );

  // The atmosphere's CPU twins, computed beside the GPU's work; its workers start with its first job.
  const twins = new TwinPool(twinWorkerCount(navigator.hardwareConcurrency, twinWorkersAsked));
  /** Runs a group of `of`, the part of the page it belongs to, if this run runs that part. */
  const group = async (of: PagePart, name: string, work: () => Promise<void>): Promise<void> => {
    if (part === "all" || part === of) {
      await checks.group(name, work);
    }
  };

  const entries = fixture === "broken-wgsl" ? [...WGSL_CATALOGUE, BROKEN_ENTRY] : WGSL_CATALOGUE;
  await group(1, "T9.b catalogue", () => checkCatalogue(engine, status, entries, checks));
  await group(1, "T9.a clear and triangle", () => checkClearAndTriangle(engine, checks));
  await group(1, "T9.d three canvases", () => checkThreeCanvases(engine, checks));
  await group(1, "T9.f depth, culling and bias", () => checkDepthCullBias(engine, checks));
  await group(1, "R10.T9.b depth layers and a comparison sampler", () =>
    checkDepthLayers(engine, checks),
  );
  await group(1, "T9.g blending, compute and the packed cube", () =>
    checkBlendComputeCube(engine, checks),
  );
  await group(1, "T9.h targets, asynchronous pipelines, indirect work and timing", () =>
    checkTargetsAsyncIndirectTiming(engine, checks),
  );
  await group(1, "T9.i material state and the splat", () => checkMaterialState(engine, checks));
  await group(1, "R08.T0 one-layer arrays in compute", () => checkOneLayerArrays(engine, checks));

  await group(1, "T10 subgroup twins", () => checkTwins(engine, checks));

  await group(1, "R02.T14.c the wireframe", () => checkWireframe(engine, checks));
  await group(1, "R07.T16.d the view's strokes as drawn", () =>
    checkStrokeContrast(engine, checks),
  );
  // A 2D canvas's, which draws and reads back at once.
  await group(1, "R07.T16.f spatial strokes", () => {
    checkSpatialStrokeContrast(checks);
    return Promise.resolve();
  });

  await group(1, "R05.T10.b the height worker", () => checkHeightWorker(checks));

  await group(1, "R05.T12.b the atmosphere's tables", () => checkAtmosphereTables(engine, checks));
  // Its surface's writes are held under the probed rounding (`decision-r08-t8-leans.md` §8).
  await group(1, "R08.T8.b low's half-resolution pick", () =>
    checkLowPick(engine, status.getSnapshot().targetRounding.rgba16float, checks),
  );
  // The hardware runs' measurement (R08.T11's rows): pass times are figures only where the run
  // lifted their quantization.
  if (gpuTiming) {
    await group(1, "R08.T8.c the far limb's timings", () => timeFarLimb(engine, checks));
    await group(1, "R08.T11 the photorealistic frame's timings", () =>
      timeAtmosphereFrame(engine, checks),
    );
  }
  await group(1, "R07.T12 the exposure histogram", () => checkHistogram(engine, checks));
  await group(1, "R07.T14.b bloom and glare", () =>
    checkBloom(engine, status.getSnapshot().targetRounding.rgba16float, checks),
  );
  await group(1, "R07.T15 tone mapping and output", () => checkTonemap(engine, checks));
  await group(1, "R07.T4.c the lit-body BRDF", () => checkLitBody(engine, checks));
  await group(1, "R07.T7 the photorealistic style", () => checkPhotoreal(engine, checks));
  await group(1, "R07.T8.a point and disc bodies", () => checkBodies(engine, checks));
  await group(1, "R07.T8.a sprite depth", () => checkSpriteDepth(engine, checks));
  await group(1, "R07.T8.b the class-map hook", () => checkClassMap(engine, checks));
  await group(1, "R07.T9 mesh bodies", () => checkMeshBodies(engine, checks));
  await group(1, "R07.T10.c the eclipse scene", () => checkEclipse(engine, checks));
  await group(1, "R07.T8.a the photorealistic frame", () => checkPhotorealFrame(engine, checks));
  await group(6, "R05.T12.c the atmosphere's frames", () => checkAtmosphereFrames(engine, checks));
  await group(4, "R05.T12.e the marches' steps", () => checkAtmosphereSteps(engine, checks));
  await group(2, "R08.T6.b the kernels over N terms", () =>
    checkAtmosphereTwins(engine, checks, twins),
  );
  await group(6, "R08.T6.b the kernels over four absorbers", () =>
    checkAtmosphereTwinsFourAbsorbers(engine, checks, twins),
  );
  await group(5, "R08.T6.f slices and bands on a Saturn-class figure", () =>
    checkAtmosphereSlicesSaturn(engine, checks, twins),
  );
  await group(3, "R08.T6.g heights and angles in f32", () =>
    checkAtmosphereHeights(engine, checks, twins),
  );
  await group(8, "R08.T6.f slices and bands on Earth", () =>
    checkAtmosphereSlicesEarth(engine, checks, twins),
  );
  // In part 8, which had room (66 s at 6f4463c4's gate), with R05's Earth's tables of its own.
  await group(8, "R07.T8.e–f the cell pass for every record", () =>
    checkCellPassEveryRecord(engine, checks),
  );
  await group(12, "R08.T11.b the face's count on a drawn surface", () =>
    checkFaceCount(engine, checks, twins),
  );
  await group(5, "R08.T6.i the marches cut at layer edges", () =>
    checkAtmosphereLayerEdges(engine, checks, twins),
  );
  await group(11, "R08.T9.b surface lighting on Earth", () =>
    checkSurfaceLightingEarth(engine, checks, twins),
  );
  await group(11, "R08.T9.b surface lighting on a Saturn-class figure", () =>
    checkSurfaceLightingSaturn(engine, checks, twins),
  );
  await group(9, "R08.T8.a the march band", () => checkMarchBand(engine, checks, twins));
  await group(9, "R08.T11.e the unread column tables at one texel on a giant", () =>
    checkUnreadGiant(engine, checks),
  );
  await group(7, "R10.T10.b the lit terrain", () => checkLitTerrain(engine, checks));
  await group(7, "R10.T10.c the survey edge", () => checkSurveyEdge(engine, checks));
  await group(7, "R10.T16 the frame", () => checkTerrainFrame(engine, checks));
  await group(7, "R10.T11 the wireframe's terrain", () => checkWireframeTerrain(engine, checks));
  // On the page's engine, whose variant decides the cascades' capability path (R10.T9.b).
  await group(7, "R10.T9.b the high setting's cascades", () => checkShadowCascades(engine, checks));
  // On an engine of its own, whose device it destroys mid-stream (R10.T16.b).
  await group(7, "R10.T16 the shared pass", async () => {
    const second = await requestAdapterOutcome(navigator.gpu);
    if (second.kind !== "adapter") {
      throw new Error(`no second adapter: ${second.kind}`);
    }
    const shared = await loadRenderEngine(
      second,
      new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    );
    try {
      await checkSharedPass(shared, devices.at(-1), checks);
    } finally {
      shared.dispose();
    }
  });
  await group(10, "R08.T7 several suns, and the sky through the air", () =>
    checkSeveralSuns(engine, checks, twins),
  );
  await group(10, "R08.T8.e the binary's slot layers", () => checkBinarySlotLayers(engine, checks));
  await group(12, "R08.T11.e the unread column table at one texel on Earth", () =>
    checkUnreadEarth(engine, checks),
  );
  await group(4, "R08.T8.b the far limb", () => checkFarLimb(engine, checks, twins));
  await group(11, "R08.T8.c the far limb's footprint", () =>
    checkFarLimbFootprint(engine, checks, twins),
  );
  await group(11, "R08.T10.b the air over a drawn ground", () =>
    checkAtmosphereFrame(engine, checks),
  );
  await group(1, "R05.T11.a the terrain's resources", () => checkTerrainResources(engine, checks));
  await group(1, "R05.T11.b the terrain's frames", () => checkTerrainFrames(engine, checks));
  await group(1, "R06.T13.d the sky's band", () => checkSkyBand(engine, checks));
  await group(1, "R06.T13.e the host discs", () => checkSkyDisc(engine, checks));
  await group(1, "R06.T13.g the sky's bake", () => checkSkyBake(engine, checks));
  let images: CapturedImage[] = [];
  if (captures) {
    await group(1, "R05.T12.c the comparison captures", async () => {
      images = await captureAtmosphere(engine);
      checks.check(
        "R05.T12.c the comparison captures",
        images.length > 0,
        `${images.length} frames`,
      );
    });
  }

  if (captures) {
    await group(1, "R08.T8.b the ascent captures", async () => {
      const frames = await captureAscent(engine, checks);
      images = [...images, ...frames];
      checks.check(
        "R08.T8.b the ascent captures",
        frames.length > 0,
        `${String(frames.length)} frames`,
      );
    });
  }

  if (captures) {
    await group(1, "R05.T11.c the terrain captures", async () => {
      images = [...images, ...(await captureTerrain(engine, checks))];
    });
  }

  if (captures) {
    await group(1, "R05.T13.b the spike captures", async () => {
      images = [...images, ...(await captureSpike(engine, checks))];
    });
  }

  if (captures) {
    await group(1, "R07.T9 the occultation captures", async () => {
      const frames = await captureOccultation(engine);
      images = [...images, ...frames];
      checks.check(
        "R07.T9 the occultation captures",
        frames.length > 0,
        `${String(frames.length)} frames`,
      );
    });
  }

  if (captures) {
    await group(1, "R07.T10.c the eclipse captures", async () => {
      const frames = await captureEclipse(engine);
      images = [...images, ...frames];
      checks.check(
        "R07.T10.c the eclipse captures",
        frames.length > 0,
        `${String(frames.length)} frames`,
      );
    });
  }

  if (captures) {
    await group(1, "R08.T8.c the far limb captures", async () => {
      images = [...images, ...(await captureFarLimb(engine, checks))];
    });
  }

  // T9.i's refusal, on a second engine with float32-blendable withheld.
  await group(1, "T9.i splat refused", async () => {
    const second = await requestAdapterOutcome(navigator.gpu);
    if (second.kind !== "adapter") {
      throw new Error(`no second adapter: ${second.kind}`);
    }
    const withheld = await loadRenderEngine(
      second,
      new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      {
        overrides: {
          withholdSubgroups: false,
          withholdShaderF16: false,
          withholdFloat32Blendable: true,
        },
      },
    );
    checkSplatRefused(withheld, checks);
    withheld.dispose();
  });

  // Last, since it rebuilds the engine: T9.f's forced loss.
  await group(1, "T9.f forced loss", async () => {
    if (device === undefined) {
      throw new Error("the engine's device was not recorded");
    }
    await checkForcedLoss(engine, status, device, checks, addCanvas());
  });
  twins.dispose();
  engine.dispose();
  checks.recordTimes();
  return {
    variant,
    adapter: `${summary.vendor}/${summary.architecture}, fallback ${String(summary.fallback)}`,
    capabilities,
    checks: checks.list,
    setupError: null,
    images,
  };
}

/** The bridge's one call, narrowed from the window. */
function smokeReport(): ((result: unknown) => Promise<void>) | null {
  const smoke: unknown = Reflect.get(window, "smoke");
  if (typeof smoke !== "object" || smoke === null) {
    return null;
  }
  const report: unknown = Reflect.get(smoke, "report");
  return typeof report === "function"
    ? (result: unknown) => Promise.resolve(Reflect.apply(report, smoke, [result]))
    : null;
}

const parameters = new URLSearchParams(window.location.search);
const variant = parameters.get("variant") ?? "default";
const fixtureName = parameters.get("fixture") ?? "none";
/** Whether the run lifted timestamp quantization, so that pass times read `full`. */
const gpuTiming = parameters.get("gpuTiming") === "1";
/** Whether the run renders R05.T12.c's comparison frames for the main process to save. */
const captures = parameters.get("captures") === "1";
/** The twin workers the run asked for (`--smoke-twin-workers`), or `null` for the default. */
const twinWorkersAsked = parameters.has("twinWorkers")
  ? Number(parameters.get("twinWorkers"))
  : null;
const report = smokeReport();
const fixture: SmokeFixture = isFixture(fixtureName) ? fixtureName : "none";
/** The by-hand soak of T11 and T12 instead of the checks, for `seconds`. */
const soakSeconds = Number(parameters.get("soak") ?? "0");

/** Runs the soak and reports its figures. */
async function soak(): Promise<Report> {
  const checks = new Checks();
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", gpuTiming));
  await runSoak(status, checks, soakSeconds, parameters.get("video"));
  return {
    variant: "soak",
    adapter: null,
    capabilities: null,
    checks: checks.list,
    setupError: null,
  };
}

/** R07.T21's child window instead of the checks, for `seconds`. */
const childSeconds = Number(parameters.get("child") ?? "0");

/** Runs the child-window scene and reports its figures. */
async function childWindow(): Promise<Report> {
  const checks = new Checks();
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", gpuTiming));
  await runChildWindow(status, checks, childSeconds, {
    frameName: parameters.get("childFrame") ?? "",
    mainHz: Number(parameters.get("mainHz") ?? "0"),
    childHz: Number(parameters.get("childHz") ?? "0"),
    hidden: parameters.get("childHidden") === "1",
  });
  return {
    variant: "child-window",
    adapter: null,
    capabilities: null,
    checks: checks.list,
    setupError: null,
  };
}

void (
  soakSeconds > 0
    ? soak()
    : childSeconds > 0
      ? childWindow()
      : run(variant, fixture, partOf(parameters.get("part")))
)
  .catch((error: unknown): Report => ({
    variant,
    adapter: null,
    capabilities: null,
    checks: [],
    setupError: `the page threw: ${error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error)}`,
  }))
  .then((result) => report?.(result))
  .catch((error: unknown) => {
    console.error("the smoke page could not report:", error);
  });
