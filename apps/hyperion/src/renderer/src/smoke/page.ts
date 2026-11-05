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
 * groups are in seventeen parts, each a run of its own under the harness's watchdog (perf-smoke,
 * 2026-10-13), parts 2 to 6 and 8 to 14 the atmosphere's CPU-twin groups (R08.T6, R08.T7,
 * R08.T9.b, R08.T8.a, R08.T8.b, R08.T8.c, R08.T8.e, R08.T11, R08.T14.c, R08.T14.f, R08.T15.a.2),
 * part 4 with R05.T12.e's marches before them and part 6 with R05.T12.c's frames, parts 7 and 15
 * R10's terrain groups and part 16 R14.T3.c's ground under a grounded craft, part 17 R08.T8.d's
 * packed far limb's kernels and their general forms' check, part 13 R08.T14.c's thick read and R08.T14.f's correction, part 14
 * R08.T15.a.2's deck, and part 1 every other; with no part given the page runs them all, in its
 * order.
 * Each part checks its path (T9.c) itself. R08.T6.f's slices and bands run in two groups: the
 * Saturn-class figure's in part 5, with R08.T6.i's layer edges, and Earth's, which took R08.T6.g's
 * Earth, in part 8, with R07.T8.e–f's cell pass for every record (`cellPass.ts`) after it;
 * R08.T11.b's face count in part 12; R08.T11.e's unread tables in two, a giant's in part 9 after
 * R08.T8.a's march band and Earth's in part 12 after R08.T11.b's face count; R08.T11.d's march
 * area in part 8 after R07.T8.e–g's cell pass, and its specialised march over the shared read in
 * part 12 after R08.T11.e's Earth; R08.T11.g's march for one band, edged, in part 8 after
 * R08.T11.d's march area, and its kernels' catalogue in part 9, first; R08.T6.b's kernels
 * over N terms in two, its medium of four
 * absorbers in part 6 and the others in part 2; R08.T9.b's surface lighting in two, Earth's and the
 * Saturn-class figure's, in part 11 before R08.T8.c's footprint; R08.T8.a's march band in part 9 (in part 6 it took the part from
 * 94 s to 140 to 146 s at a load of 2 to 5, the group 64 to 73 s, mostly its four atmospheres'
 * tables and kernels, 2026-10-21); R08.T7's several suns in part 10 with R08.T8.e's slot layers on
 * its binary, and R10.T16.g's datum inside the figure over the image last, with R02.T19's on
 * `TEST SATURN`'s spheroid (`datumFrame.ts`); R08.T8.b's far limb, with the composite from above
 * the top, in part 4 after R05.T12.e's marches, and its low pick, which has no twin, in part 1;
 * R08.T8.c's far limb's footprint, its frames from 10⁸ m at the output's own pixel, in part 11;
 * R08.T11's routes' rows, each its twins and its row's frame drawn untimed, last in its part: (T)'s
 * in part 12, whose catalogue compiles its thermal forms, (R)'s in part 13, whose compiles its thick
 * read, and (S)'s and (U)'s, their deck's marches compiled in their groups, in parts 6 and 2;
 * R08.T14.c's thick read in part 13, its kernels' catalogue first, then its kernels against their
 * twins on Earth, R08.T14.f's correction alone against its twins on Earth and its specialised march, and its kernels against their twins on a Saturn-class
 * figure in part 3 after R08.T6.g's heights; and part 7 R10.T10.b's lit terrain, R10.T10.e's
 * classes' curve in the lit-body kernel, R10.T10.c's
 * survey edge, R10.T11's wireframe terrain, R10.T9.b's cascades and R10.T16.b's shared pass (on an
 * engine of its own, whose device it destroys), and part 15 R10.T16.c's frame alone. Three groups
 * of part 1 run only when asked:
 * R08.T8.c's far limb's timings and, after them, R08.T11's photorealistic frame's timings
 * (`frameTiming.ts`), after the low pick, when the run lifted timestamp quantization (`gpuTiming`,
 * the hardware runs of R01.T11), and R08.T8.c's far limb captures, after R07.T10.c's eclipse
 * captures and fix-view-light's of the view's light, with the other captures, and R02.T19.b's datum
 * captures after them; a plain
 * `just test-render` runs none of them.
 *
 * With `transfer` in its query (`--smoke-transfer`; plan R10, R10.T15.b) the page runs R03.T15's
 * transfer check instead of every group (`transfer.ts`): no adapter, only the server's socket.
 */

import {
  ATMOSPHERE_DECK_ENTRIES,
  ATMOSPHERE_LIMB_ENTRIES,
  ATMOSPHERE_ONE_BAND_ENTRIES,
  ATMOSPHERE_THERMAL_ENTRIES,
  ATMOSPHERE_THICK_ENTRIES,
  DISC_AIR_ENTRIES,
  DISC_EMISSIVE_AIR_ENTRIES,
  THERMAL_ENTRIES,
  TABLE_ONLY_ENTRIES,
  MARCH_TABLE_ONLY_ENTRIES,
  WGSL_CATALOGUE,
} from "../view/engine/catalogue";
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
  checkCompositeArea,
  checkCompositeBehind,
  checkSurfaceTransmittance,
  checkMarchArea,
  checkMarchBand,
  checkMarchOneBand,
  checkMarchOverBody,
  checkMarchPackedGeneral,
  checkMarchTableBySlot,
  checkMarchTableShared,
  checkMarchSpecialised,
  checkCorrectionFrameTwinsEarth,
  checkRoutedTwins,
  checkThermalFrameTwins,
  checkBelowDeckFrameTwins,
  checkDeckFrameTwins,
  checkDeckThermalFrameTwins,
  checkThickFrameTwinsEarth,
  checkThickFrameTwinsSaturn,
  checkThickMarchSpecialised,
  checkSeveralSuns,
  checkSurfaceLightingEarth,
  checkSurfaceLightingBelowDeck,
  checkSurfaceLightingSaturn,
  checkSurfaceLightingThickEarth,
  checkSourceTableEarth,
  checkSourceTableSaturn,
  checkUnreadEarth,
  checkUnreadGiant,
  timeFarLimb,
} from "./atmosphere";
import { checkAtmosphereFrame } from "./atmosphereFrame";
import {
  checkCellPassEveryRecord,
  checkSourceTableFrames,
  checkSourceTableTwins,
} from "./cellPass";
import { checkDiscAir, checkJupiterDiscAir } from "./discAir";
import { checkThermalDiscs } from "./thermal";
import { captureDatum, checkDatumFrame } from "./datumFrame";
import { checkRoutedRow, type RoutedRow, timeAtmosphereFrame } from "./frameTiming";
import {
  handJupiterDeckMedium,
  oneSheetOf,
  type RoutedMedium,
  selfLuminousEarthMedium,
  titanHazeMedium,
  venusDeckMedium,
  withoutOblateTwinMisses,
} from "./frameTimingScene";
import { checkBlendComputeCube, checkMaterialState, checkSplatRefused } from "./blending";
import { checkBloom } from "./bloom";
import { BROKEN_ENTRY, checkCatalogue, makeExternalRequests, type SmokeFixture } from "./catalogue";
import { checkIdTarget } from "./idTarget";
import { checkDepthLayers } from "./depthLayers";
import { captureEclipse, checkEclipse } from "./eclipse";
import { captureViewLight } from "./viewLight";
import { captureViewLightAir } from "./viewLightAir";
import { addCanvas, checkClearAndTriangle, checkDepthCullBias, checkThreeCanvases } from "./frames";
import { Checks } from "./harness";
import { checkHeightWorker } from "./heightWorker";
import { checkHistogram } from "./histogram";
import { checkClassCurveSeam, checkLitBody } from "./litBody";
import { checkLitTerrain } from "./litTerrain";
import { checkClusterCull } from "./clusterCull";
import { checkClusterGrid } from "./clusterGrid";
import { checkOcclusionCull } from "./occlusionCull";
import { captureSurveyLimb, checkSurveyEdge } from "./surveyEdge";
import { captureOccultation, checkMeshBodies } from "./meshBodies";
import { checkSkyBake, checkSkyBand, checkSkyDisc } from "./sky";
import { checkPhotoreal } from "./photoreal";
import {
  capturePlanetshineSlopes,
  checkBodies,
  checkClassMap,
  checkPhotorealFrame,
  checkSpriteDepth,
} from "./bodies";
import { runChildWindow } from "./childWindow";
import { checkTransfer } from "./transfer";
import { runSoak } from "./soak";
import { captureSpike } from "./spike";
import { captureTerrain, checkTerrainFrames, checkTerrainResources } from "./terrain";
import { checkTonemap } from "./tonemap";
import { TwinPool, twinWorkerCount } from "./twinPool";
import { checkTwins } from "./twins";
import { checkSpatialStrokeContrast } from "./spatial";
import { checkStrokeContrast } from "./strokeContrast";
import { checkWireframe } from "./wireframe";
import { checkShadowCascades, checkSharedPass, checkVisibilityCascades } from "./sharedPass";
import { checkStridesUnderCraft, checkTerrainFrame, checkVisibilityFrame } from "./terrainFrame";
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
 * feature takes (decision-r10-t9b-design D2). It withholds `primitive-index` too (plan R14,
 * R14.T2.c), so that on an adapter that offers it the default run's ID raster takes the triangle
 * from `primitive_index` and `no-subgroups`' draws de-indexed; SwiftShader offers neither run it
 * (Electron 45.1.0, 2026-11-04), so both draw de-indexed there.
 */
const VARIANTS: Readonly<Record<string, CapabilityOverrides | undefined>> = {
  default: undefined,
  "no-subgroups": {
    withholdSubgroups: true,
    withholdShaderF16: false,
    withholdDepthClipControl: true,
    withholdPrimitiveIndex: true,
  },
};

/**
 * A part of the page's groups: 2 to 6 and 8 to 12 the atmosphere's CPU-twin groups (R08.T6,
 * R08.T7, R08.T9.b, R08.T8.a, R08.T8.b, R08.T8.c, R08.T8.e, R08.T11), whose twins outweigh their
 * GPU work (2 R08.T6.b's but its medium of four absorbers, 3 R08.T6.g's, 4 R08.T8.b's far limb, with the
 * composite from above the top, 5 R08.T6.f's Saturn-class figure and R08.T6.i's layer edges, 6
 * R08.T6.b's medium of four absorbers, 8 R08.T6.f's Earth, R08.T11.d's march area and R08.T11.g's
 * march for one band, edged, 9 R08.T11.g's kernels' catalogue, R08.T8.a's march band, R08.T18.a's
 * thermal emission with its emissive forms' catalogue and R08.T16.b's disc air with its air forms'
 * catalogue, 10 R08.T7's several suns, R08.T8.e's slot
 * layers on its binary and R10.T16.g's datum over the image, 11
 * R08.T9.b's Earth and Saturn-class figure and R08.T8.c's far limb's footprint, 12 R08.T11.b's
 * face count, R08.T11.e's Earth and R08.T11.d's specialised march over the shared read), 7 and 15
 * R10's terrain groups (R10.T10.b, T10.c, T11, T16 and T9.b; 15 R10.T16's frame), which took part 6
 * to 163 s at a load of 2 once R10.T9.b's cascades joined them (2026-10-21), and 1 every other
 * group but R05.T12.e's and
 * R05.T12.c's frames, which
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
 * R10.T16.g's datum group then moved from part 11 to part 10, last there, as R02.T19 added its five
 * airless `TEST SATURN` frames (decision-r02-t19-leans, item 5): at 1808146a's gate, at a load of 5
 * to 7, part 10 held 81 and 85 s and part 11 130 and 135 s, the group 27 and 31 s of it (part 11
 * 153 to 154 s at e64fcb9b's). With the move, on R02.T19's tree before its gate at a load of 3 to
 * 8, part 10 held 116 and 119 s (the group 36 and 34 s) and part 11 119 s on both variants
 * (2026-10-27). R08.T14.c's thick read then brought its kernels' catalogue (some 33 s at part 1's
 * pace, which took part 1 past the watchdog), its twins on Earth and on a Saturn-class figure and
 * its specialised march. At the last four gates parts 9, 11 and 12 had run 51 to 131 s, 86 to
 * 133 s and 46 to 135 s, which the catalogue, the Earth's twins and the specialised march would
 * take to some 160 to 166 s at their slowest, so part 13 joined, with those three; the
 * Saturn-class figure's twins, the smallest, went to part 3 (67 to 126 s) (2026-10-27).
 * R08.T15.a.2's deck brought its marches' catalogue and its twins on Earth and on a Saturn-class
 * figure, which part 13 (156 s on a pod at R08.T14.d.1's gate) and part 9 (112 s) could not take
 * whole, so part 14 joined, with both (2026-10-30). R14's levers took part 7 to 238 to 242 s of
 * its 360 s scaled watchdog on a pod, R10.T16's frame 124 to 127 s of it, so part 15 joined, with
 * that frame alone (2026-11-02). R14.T3.c's ground under a grounded craft streams a pass of its own
 * to the finest level about the craft, some 90 s on a pod, which took part 15 to 262 s, so part 16
 * joined, with that check alone (2026-11-03). R08.T8.d's packed far limb brought eighteen march
 * kernels' catalogue (some 120 s on a pod) and its general forms' frames on the Saturn-class
 * figure (some 45 s), so part 17 joined, with those two, and part 16 keeps its check alone
 * (2026-11-03). Part 15 stood at 317 s of its 360 s on a 32-CPU pod (2026-11-05), so plan R14's
 * frame by the visibility buffer (R14.T2.c to T2.e), which streams a pass of its own and compiles
 * the shading's forms, took part 18, alone, and its frame under the high setting's cascades, which
 * streams R10.T9.b's pose's pass of its own, part 19 (part 7 stood at 293 s of its 360 s).
 */
type PagePart = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19;

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
    case "13":
      return 13;
    case "14":
      return 14;
    case "15":
      return 15;
    case "16":
      return 16;
    case "17":
      return 17;
    case "18":
      return 18;
    case "19":
      return 19;
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
    `primitive-index ${capabilities.primitiveIndex ? "yes" : "no"}`,
    `sub-pixel bits ${String(capabilities.subpixelBits)}`,
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
    return failed("the part asked for is not 1 to 16 or all");
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
  // The harness's own launch mode, under which the client refuses the style (R01.T16 alone lifts it).
  const styles = styleAvailability(summary, status.getSnapshot().launchMode);
  checks.check(
    "T9.c the photorealistic style is refused on a fallback adapter",
    !summary.fallback || !styles.photorealistic,
    `photorealistic ${String(styles.photorealistic)}`,
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

  // R08.T14.c's thick read's kernels are compiled in part 13, first: with them part 1's catalogue
  // took 33 s more under load, the part past its watchdog. R08.T11.g's one-band march's two are
  // compiled in part 9, first: with them part 1 went from 170 s to past its watchdog on one variant
  // on a slower machine (4 vCPU, 2026-10-29). R08.T18.a's emissive forms are compiled in part 9 too,
  // the lightest, beside their twin's group, and R08.T16.b's air forms after them, and R08.T18.b's
  // forms with both after those. R08.T18.b's thermal forms of the per-frame kernels are compiled in
  // part 12, beside their twins' group.
  const thin = WGSL_CATALOGUE.filter(
    (entry) =>
      !ATMOSPHERE_LIMB_ENTRIES.includes(entry) &&
      !ATMOSPHERE_THICK_ENTRIES.includes(entry) &&
      !ATMOSPHERE_DECK_ENTRIES.includes(entry) &&
      !ATMOSPHERE_ONE_BAND_ENTRIES.includes(entry) &&
      !ATMOSPHERE_THERMAL_ENTRIES.includes(entry) &&
      !THERMAL_ENTRIES.includes(entry) &&
      !DISC_AIR_ENTRIES.includes(entry) &&
      !DISC_EMISSIVE_AIR_ENTRIES.includes(entry) &&
      !TABLE_ONLY_ENTRIES.includes(entry) &&
      !MARCH_TABLE_ONLY_ENTRIES.includes(entry),
  );
  const entries = fixture === "broken-wgsl" ? [...thin, BROKEN_ENTRY] : thin;
  await group(1, "T9.b catalogue", () => checkCatalogue(engine, status, entries, checks));
  await group(13, "T9.b catalogue: the thick read's kernels", () =>
    checkCatalogue(engine, status, ATMOSPHERE_THICK_ENTRIES, checks),
  );
  await group(9, "T9.b catalogue: the one-band march's kernels", () =>
    checkCatalogue(engine, status, ATMOSPHERE_ONE_BAND_ENTRIES, checks),
  );
  // R08.T9.b's lever's table-only twins, in part 16, the lightest (36 s at 0e466e0's gate).
  await group(16, "T9.b catalogue: the table-only twins", () =>
    checkCatalogue(engine, status, TABLE_ONLY_ENTRIES, checks),
  );
  // R08.T8.e's table's march twins, in part 16 after R08.T9.b's.
  await group(16, "T9.b catalogue: the march's table-only twins", () =>
    checkCatalogue(engine, status, MARCH_TABLE_ONLY_ENTRIES, checks),
  );
  // R08.T15.a.2's deck's marches, in part 14 with its twins, first.
  await group(14, "T9.b catalogue: the deck's marches", () =>
    checkCatalogue(engine, status, ATMOSPHERE_DECK_ENTRIES, checks),
  );
  await group(1, "T9.a clear and triangle", () => checkClearAndTriangle(engine, checks));
  await group(1, "T9.d three canvases", () => checkThreeCanvases(engine, checks));
  await group(1, "T9.f depth, culling and bias", () => checkDepthCullBias(engine, checks));
  await group(1, "R10.T9.b depth layers and a comparison sampler", () =>
    checkDepthLayers(engine, checks),
  );
  await group(1, "R14.T2.b the engine's ID target", () => checkIdTarget(engine, checks));
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
      timeAtmosphereFrame(engine, checks, { thickRead }),
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
  await group(
    3,
    "R08.T14.c the thick read's kernels against their twins on a Saturn-class figure",
    () => checkThickFrameTwinsSaturn(engine, checks, twins),
  );
  await group(8, "R08.T6.f slices and bands on Earth", () =>
    checkAtmosphereSlicesEarth(engine, checks, twins),
  );
  // In part 8, which had room (66 s at 6f4463c4's gate), with R05's Earth's tables of its own.
  await group(8, "R07.T8.e–g the cell pass for every record", () =>
    checkCellPassEveryRecord(engine, checks),
  );
  // In part 8 after R07.T8.e–g's cell pass (the part 85 s at e64fcb9b's gate; part 12 took it to
  // 149 to 177 s at a CPU quota of 400%, and part 10 takes R10.T16.g's group).
  await group(8, "R08.T11.d the march over the air's area and the tile cull", () =>
    checkMarchArea(engine, checks, summary.fallback),
  );
  // In part 8 after R08.T11.d's march area, which compiles the march by slot that the slab Earth
  // takes too, so that no part's GPU process compiles both reads' marches.
  await group(8, "R08.T11.g the one-band march with layer edges", () =>
    checkMarchOneBand(engine, checks, summary.fallback),
  );
  // In part 8 after R08.T11.g's, over the march by slot that R08.T11.d's group compiled.
  await group(8, "R08.T16.a.2 the composite over the air's area", () =>
    checkCompositeArea(engine, checks),
  );
  // In part 8 after R08.T16.a.2's, over the march by slot that R08.T11.d's group compiled.
  await group(8, "the owner's priority: what lies behind the sky at the march's resolution", () =>
    checkCompositeBehind(engine, checks),
  );
  // In part 8 after the composite's, over the march by slot that R08.T11.d's group compiled.
  await group(8, "R08.T8.e's table: the march by slot's sources from the sources' table", () =>
    checkMarchTableBySlot(engine, checks, summary.fallback),
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
  await group(11, "R08.T9.b's lever: the sources' table on Earth", () =>
    checkSourceTableEarth(engine, checks),
  );
  await group(11, "R08.T9.b's lever: the sources' table on a Saturn-class figure", () =>
    checkSourceTableSaturn(engine, checks),
  );
  await group(11, "R08.T9.b's lever: bodies drawn with the sources' table", () =>
    checkSourceTableFrames(engine, checks),
  );
  await group(11, "R08.T9.b's lever: bodies of one band drawn by their table-only twins", () =>
    checkSourceTableTwins(engine, checks),
  );
  await group(9, "R08.T8.a the march band", () => checkMarchBand(engine, checks, twins));
  // In part 13 after the thick read's catalogue; the Saturn-class figure's in part 3, for the
  // watchdog: both in part 9 took it to 165 s under load.
  await group(13, "R08.T14.c the thick read's kernels against their twins on Earth", () =>
    checkThickFrameTwinsEarth(engine, checks, twins),
  );
  // In part 13 after the thick table's Earth, whose kernels it draws by, compiled by the part's
  // catalogue: in part 9, the lightest other part, it would compile them again.
  await group(13, "R08.T14.f the correction's kernels against their twins on Earth", () =>
    checkCorrectionFrameTwinsEarth(engine, checks, twins),
  );
  // In part 13, whose catalogue compiles the thick irradiance kernel.
  await group(13, "R08.T14.d.4 the sky's irradiance over the thick table on Earth", () =>
    checkSurfaceLightingThickEarth(engine, checks, twins),
  );
  await group(9, "R08.T11.e the unread column tables at one texel on a giant", () =>
    checkUnreadGiant(engine, checks),
  );
  // R08.T18.a's emissive forms of the lit bodies' kernels, compiled and held to their twin.
  await group(9, "T9.b catalogue: the emissive forms", () =>
    checkCatalogue(engine, status, THERMAL_ENTRIES, checks),
  );
  await group(9, "R08.T18.a the ground's thermal emission against its twin", () =>
    checkThermalDiscs(engine, checks),
  );
  // R08.T16.b's air forms of the lit bodies' kernels, compiled and held to their twin.
  await group(9, "T9.b catalogue: the air forms", () =>
    checkCatalogue(engine, status, DISC_AIR_ENTRIES, checks),
  );
  await group(9, "R08.T16.b the disc's air beyond 10⁹ m against its twin", () =>
    checkDiscAir(engine, checks),
  );
  await group(9, "R08.T16.b.2 the hand Jupiter's disc air beyond 10⁹ m against its twin", () =>
    checkJupiterDiscAir(engine, checks),
  );
  // R08.T18.b's forms with emission and air together, compiled after the air forms.
  await group(9, "T9.b catalogue: the emissive forms with air", () =>
    checkCatalogue(engine, status, DISC_EMISSIVE_AIR_ENTRIES, checks),
  );
  await group(7, "R10.T10.b the lit terrain", () => checkLitTerrain(engine, checks));
  await group(7, "R10.T10.e the classes' curve in the kernel", () =>
    checkClassCurveSeam(engine, checks),
  );
  await group(7, "R10.T10.c the survey edge", () => checkSurveyEdge(engine, checks));
  // In part 15, alone: beside R10's other terrain groups it took part 7 to 242 s of 360 s on a pod
  // (2026-11-02). With the captures, R14's levers' frames: R14.T1.a's here, R14.T1.d's with the
  // cascades; under `--fixture=broken-wgsl`, R14.T1.f's fallback from a refused cull kernel too.
  let images: CapturedImage[] = [];
  await group(15, "R10.T16 the frame", async () => {
    images = [...images, ...(await checkTerrainFrame(engine, checks, captures, fixture))];
  });
  // In part 16, alone: its own pass streams to the finest level about the craft (R14.T3.c).
  // R08.T8.d's packed far limb's kernels and the cull's specialised centre rays, in part 17 beside
  // the general forms' check, first: parts 8 and 12 hold the specialised forms' checks.
  await group(17, "T9.b catalogue: the far limb's packed kernels", () =>
    checkCatalogue(engine, status, ATMOSPHERE_LIMB_ENTRIES, checks),
  );
  await group(17, "R08.T8.d the far limb's node rays packed in the general forms", () =>
    checkMarchPackedGeneral(engine, checks, summary.fallback),
  );
  await group(16, "R14.T3.c the ground under a grounded craft", () =>
    checkStridesUnderCraft(engine, checks),
  );
  // In part 18, alone: its own pass's frame by the visibility buffer against the forward frame
  // (plan R14, R14.T2.c to T2.e), the shading's forms compiled at its first frame by IDs; beside
  // part 15's frame check, which stood at 317 s of its 360 s on a pod (2026-11-05), it would pass
  // its watchdog. With the captures, the frame drawn both ways.
  await group(18, "R14.T2 the frame by the visibility buffer", async () => {
    images = [...images, ...(await checkVisibilityFrame(engine, checks, captures))];
  });
  // In part 19, alone: the high setting's cascades' frame by the visibility buffer (plan R14,
  // R14.T2.d), its own pass streamed to R10.T9.b's pose; beside part 7's cascades, which stood at
  // 293 s of 360 s on a pod (2026-11-05), it would pass that part's two thirds.
  await group(19, "R14.T2.d the cascades' frame by the visibility buffer", async () => {
    images = [...images, ...(await checkVisibilityCascades(engine, checks, captures))];
  });
  await group(7, "R14.T1.b the cluster cull against its twin", () =>
    checkClusterCull(engine, checks),
  );
  await group(7, "R14.T3.d the depth pyramid and the occlusion's passes against their twins", () =>
    checkOcclusionCull(engine, checks),
  );
  await group(7, "R14.T2.a the cluster decode against its twin", () =>
    checkClusterGrid(engine, checks),
  );
  await group(7, "R10.T11 the wireframe's terrain", () => checkWireframeTerrain(engine, checks));
  // On the page's engine, whose variant decides the cascades' capability path (R10.T9.b); with the
  // captures, R14.T1.d's casters at stride 1 and at their texel.
  await group(7, "R10.T9.b the high setting's cascades", async () => {
    images = [...images, ...(await checkShadowCascades(engine, checks, captures))];
  });
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
  // In part 12 after R08.T11.e's Earth (the part 92 s at 415a7f82's gate), apart from part 8's
  // march by slot, so that neither part's GPU process compiles both reads' marches.
  await group(12, "R08.T11.d the specialised march over the shared read", () =>
    checkMarchSpecialised(engine, checks, summary.fallback),
  );
  // fix-disc-air-rows: low's half-resolution march over a body drawn with its own depth, R05's
  // Earth's shared read as the specialised march's, in the same part.
  await group(12, "fix-disc-air-rows the half-resolution march over a drawn body", () =>
    checkMarchOverBody(engine, checks),
  );
  // In part 12 after the specialised march's, over the shared read that its group compiled.
  await group(12, "R08.T8.e's table: the shared read's sources from the sources' table", () =>
    checkMarchTableShared(engine, checks, summary.fallback),
  );
  // R08.T18.b's thermal forms of the per-frame kernels, compiled and held to their twins, in part 12,
  // the lightest (54 s at 03e12ec's tree).
  await group(12, "T9.b catalogue: the thermal forms", () =>
    checkCatalogue(engine, status, ATMOSPHERE_THERMAL_ENTRIES, checks),
  );
  await group(12, "R08.T18.b the medium's own light against its twins", () =>
    checkThermalFrameTwins(engine, checks, twins),
  );
  // R08.T11's routes' rows on SwiftShader, each its twins and its row's frame, untimed: (T)'s here,
  // whose catalogue compiles its thermal forms, (R)'s in part 13, whose compiles its thick read, and
  // (S)'s and (U)'s in parts 6 and 2, the lightest. (S)'s twins take its whole medium, its aerosol
  // decks among them (`OBLATE_TWIN_MISSES` holds none since the fit's least rise, 2026-11-03).
  const routedRow = async (row: RoutedRow, made: () => Promise<RoutedMedium>): Promise<void> => {
    const routed = await made();
    await checkRoutedTwins(engine, checks, twins, [withoutOblateTwinMisses(routed)]);
    await checkRoutedRow(engine, checks, routed, row);
  };
  // (S) and (U), whose deck's marches compile in their groups: part 14, which compiles them first,
  // took 229 s of its 360 s on a pod with (S) and before R08.T15.b.2's thermal twins joined it.
  await group(6, "R08.T11 (S) the hand Jupiter's deck route's row and its twins", () =>
    routedRow("(S)", handJupiterDeckMedium),
  );
  // The fit's least rise (`medium.ts`'s `fitLeastRiseM`, 2026-11-03): (S)'s gas and one tabulated
  // crystal's sheet alone on its oblate figure against its twins, after (S), whose group compiles
  // the deck's marches it draws by.
  await group(
    6,
    "R08.T16.a.2 one crystal sheet on a giant's oblate figure against its twins",
    async () =>
      checkRoutedTwins(engine, checks, twins, [oneSheetOf(await handJupiterDeckMedium())]),
  );
  await group(2, "R08.T11 (U) venus-92bar's deck route's row and its twins", () =>
    routedRow("(U)", venusDeckMedium),
  );
  await group(12, "R08.T11 (T) a self-luminous medium's row and its twins", () =>
    routedRow("(T)", () => Promise.resolve(selfLuminousEarthMedium())),
  );
  // In part 13, last: in part 6 it took the part to its watchdog (180 s), and beside the thin one
  // in part 12 to 166 to 172 s under load.
  await group(13, "R08.T14.c the thick read's specialised march", () =>
    checkThickMarchSpecialised(engine, checks, summary.fallback),
  );
  await group(13, "R08.T11 (R) titan-class's haze route's row and its twins", () =>
    routedRow("(R)", titanHazeMedium),
  );
  // In part 14 after the deck's catalogue, whose marches it draws by.
  await group(14, "R08.T15.a.2 the deck's march against its twins", () =>
    checkDeckFrameTwins(engine, checks, twins),
  );
  // In part 14 after the deck's catalogue, which compiles its marches' thermal forms.
  await group(14, "R08.T15.b.2 the deck's thermal march against its twins", () =>
    checkDeckThermalFrameTwins(engine, checks, twins),
  );
  await group(14, "R08.T15.b.2 the kernels beneath a deck against their twins", () =>
    checkBelowDeckFrameTwins(engine, checks),
  );
  await group(14, "R08.T15.b.2 the surface's lighting beneath a deck", () =>
    checkSurfaceLightingBelowDeck(engine, checks, twins),
  );
  await group(4, "R08.T8.b the far limb", () => checkFarLimb(engine, checks, twins));
  await group(11, "R08.T8.c the far limb's footprint", () =>
    checkFarLimbFootprint(engine, checks, twins),
  );
  await group(11, "R08.T10.b the air over a drawn ground", () =>
    checkAtmosphereFrame(engine, checks),
  );
  await group(11, "R08.T9.c a drawn surface's transmittance per channel", () =>
    checkSurfaceTransmittance(engine, checks),
  );
  await group(10, "R10.T16.g the datum inside the figure", () => checkDatumFrame(engine, checks));
  await group(1, "R05.T11.a the terrain's resources", () => checkTerrainResources(engine, checks));
  await group(1, "R05.T11.b the terrain's frames", () => checkTerrainFrames(engine, checks));
  await group(1, "R06.T13.d the sky's band", () => checkSkyBand(engine, checks));
  await group(1, "R06.T13.e the host discs", () => checkSkyDisc(engine, checks));
  await group(1, "R06.T13.g the sky's bake", () => checkSkyBake(engine, checks));
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
    await group(1, "fix-view-light the view's light with the Sun in frame", async () => {
      images = [...images, ...(await captureViewLight(engine, checks))];
    });
  }

  if (captures) {
    await group(1, "fix-view-light the Sun's glare through the air", async () => {
      images = [...images, ...(await captureViewLightAir(engine, checks))];
    });
  }

  if (captures) {
    await group(1, "R08.T8.c the far limb captures", async () => {
      images = [...images, ...(await captureFarLimb(engine, checks))];
    });
  }

  if (captures) {
    await group(1, "R02.T19 the datum captures", async () => {
      const frames = await captureDatum(engine);
      images = [...images, ...frames];
      checks.check(
        "R02.T19 the datum captures",
        frames.length > 0,
        `${String(frames.length)} frames`,
      );
    });
  }
  // R10.T10.g's planetshine on slopes and R10.T10.c's limb over the survey, in part 7 with the
  // terrain's groups, so that `--part=7` takes them without part 1's captures.
  if (captures) {
    await group(7, "R10.T10.g and R10.T10.c the terrain light's captures", async () => {
      const frames = [
        ...(await capturePlanetshineSlopes(engine)),
        ...(await captureSurveyLimb(engine)),
      ];
      images = [...images, ...frames];
      checks.check(
        "R10.T10.g and R10.T10.c the terrain light's captures",
        frames.length > 0,
        `${String(frames.length)} frames`,
      );
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
/**
 * Whether the frame's timings also draw the measured Earth through the thick read, its bake forced
 * on with its correction summed into it, and with its polarisation correction alone
 * (`--smoke-thick-read=1`; R08.T14.c's and R08.T14.f's frame records): the smoke's override alone.
 */
const thickRead = parameters.get("thickRead") === "1";
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

/** R03.T15's transfer check's server, instead of the checks (plan R10, R10.T15.b), or `null`. */
const transferUrl = parameters.get("transfer");

/** Runs the transfer check, which asks the GPU nothing, and reports its checks. */
async function transfer(url: string): Promise<Report> {
  const checks = new Checks();
  await checkTransfer(
    { url, universe: parameters.get("universe") ?? "", body: parameters.get("body") ?? "" },
    checks,
  );
  return {
    variant: "transfer",
    adapter: null,
    capabilities: null,
    checks: checks.list,
    setupError: null,
  };
}

void (
  transferUrl !== null
    ? transfer(transferUrl)
    : soakSeconds > 0
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
