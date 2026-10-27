/**
 * R08.T11's photorealistic frame's timings (plan R08, R08.T11, the thin benchmarks): the frame VIEW
 * draws, `PhotorealRenderer` over an Earth whose air it draws, timed per pass on both settings'
 * outputs from R08.T8.c's far-limb cameras under each set of sources (`frameTimingScene.ts`), for
 * the run that lifts timestamp quantization (`--smoke-gpu-timing=1`, the hardware runs of
 * R01.T11), whose pass times are the measurement.
 *
 * @remarks
 * Seven parts, each its own checks:
 *
 * - (A) the frame: one renderer on one canvas view, high at 1,920 × 1,080 and low at 1,280 × 720,
 *   role `eye`, the average meter (the histogram runs), EV100 15, no sky, no star sprites, no
 *   overlay, no terrain, on R05's Earth, the measured Earth and the slab Earth (each its own
 *   tables, built once, and its own body), five cameras and the setting's four sets; each pass's
 *   time by label, the frame's GPU pass sum, the atmosphere's share (its view passes and the
 *   composite), the rest, and the main thread's time in `render`, so that a CPU-bound frame shows.
 *   The plan's sources for the Earth are listed once a combination (`listSources`) and must be the
 *   set's.
 * - (B) the atmosphere alone, as `timeFarLimbSetting` draws it (its frame over an empty scene,
 *   then its composite), over the same tables with the same sources (A)'s plan gave, on high, for
 *   `x1` and `x2`: the bare ground's march beside (A)'s, whose face is drawn with depth
 *   (`decision-r08-t10b-depth.md` §5).
 * - (C) small air bodies: the measured Earth whole at 4, 10 and 32 px, with its air and without.
 * - (G) the control without air: (A)'s frames of the measured Earth on high at `x1`, `x2` and
 *   `x4`, no atmosphere drawn, so that R07's regimes draw the body bare.
 * - (D) the per-planet tables' builds, cold (the first in the group) and warm, on the main thread
 *   and the GPU, with their slices, bands, bytes and allocations, and the main thread's split.
 * - (E) the allocations the renderer makes at its first frame that draws air, and at one, two and
 *   four sources.
 * - (F) the main thread's first frame of a new body's air against the steady frame's, and the
 *   renderer's `prepare`.
 *
 * R08.T11.c (QW29, `decision-r08-t11-frame.md` §2; `decision-r08-approximate-plate.md` item 3) adds
 * three, timed and reported as (A) and (C) are:
 *
 * - (H) three small air bodies in one view, 32, 10 and 4 px across, each its own air over the
 *   measured Earth's tables, at `x2` on high (each body lit by the other two's planetshine too,
 *   `trioSourceSet`), with their air and without;
 * - (I) the measured Earth under a class map with its normal map, from 400 km and 10⁷ m, at `x2`,
 *   `x2p` (since R08.T11.d, for QW30) and `x4` on high: R10's fixture's as the smoke builds it
 *   (`smokeClassMapTexels`), judged as (A)'s rows are, the verdict's row, and a generated patch's
 *   whole palette (`generatedClassMapTexels`), judged and reported too since QW30 (main,
 *   2026-10-27: what generated worlds show at 24; a row of it over 16.67 ms at `x2` or `x2p` goes
 *   to main by name);
 * - (J) `TEST SATURN` (`ATMOSPHERE TEST`'s `saturn-oblate`, Saturn's spheroid of five slices and
 *   four bands, R08.T6.f's Saturn-class passes) at (A)'s five cameras scaled to its radius
 *   (`placesScaledTo`), under the setting's four sets on both settings as (A)'s: `x1` and `x2`
 *   since R08.T11.c, `x2p` and the whole count since R08.T11.d (for QW30), its giants placed as
 *   (A)'s but at `SATURN_GIANT_DISTANCE_M`, beyond the farthest of its cameras.
 *
 * The timed parts take QW2's and QW3's form (`decision-r08-t8-leans.md` §10;
 * `frameTimingLedger.ts`' `QW_FORM` and `runTimed`): each combination's 10 warm-up frames, then 30
 * timed, back to back; two sweeps over every combination, the second in the reverse order. There
 * is no idle gap: a frame waits only for the resolves of the frame three before it, so that two
 * frames stay queued and the timer, which holds `TIMING_FRAMES_IN_FLIGHT` (135) resolves in flight,
 * drops none. A frame with any resolve missing is left out and counted. Timings are figures, never
 * held: a check fails only when frames are missing or not finite, or a frame's sources are not its
 * set's.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import type { DiscSurface } from "../view/appearance/bodyAppearance";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import type { PhotometricLaw } from "../view/appearance/law";
import {
  ATMOSPHERE_COMPOSITE_PASS,
  ATMOSPHERE_VIEW_PASS,
  createAtmosphereFrameKernels,
  HillaireAtmosphere,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "../view/atmosphere/hillaire";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
} from "../view/atmosphere/earth";
import { forwardLobeOf } from "../view/atmosphere/forwardLobe";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import {
  absorberCount,
  type AbsorberCurveSlots,
  ATMOSPHERE_TABLES_PASS,
  type AtmosphereBody,
  AtmosphereTables,
  NO_ABSORBER_CURVES,
  packAbsorberCurves,
  packDensityTables,
  packLobeTables,
  packMedium,
  packPhaseTables,
  resampledPhase,
  withoutForwardDelta,
} from "../view/atmosphere/tables";
import { type NormalMapTexels, normalMapTexture } from "../view/bodies/discNormals";
import { type ClassMapTexels, classMapSurface } from "../view/bodies/discSurface";
import type { BodyFramePlan, LitBodyInput } from "../view/bodies/draw";
import type { LitRegime } from "../view/bodies/regime";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { IDENTITY_ROTATION, type Rotation3 } from "../view/coords/rotation";
import type { GpuTimer } from "../view/engine/status";
import type { AllocationEvent, MemoryCategory } from "../view/engine/memory";
import type { RenderEngine, RenderTarget, RenderView, ViewSize } from "../view/engine/types";
import { PHOTOREAL_PASS_LABELS, photorealisticPasses } from "../view/photoreal/passes";
import {
  type FrameAtmosphere,
  type PhotorealFrame,
  PhotorealRenderer,
} from "../view/photoreal/renderer";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { builtOfCase } from "../view/scenes/atmosphereScene";
import { CLASS_MAP_FACE_TEXELS } from "../view/terrain/classMap";
import {
  FAR_LIMB_LONGITUDE_RAD,
  FAR_LIMB_TIMING_PLACES,
  FAR_LIMB_TIMING_SUN_AZIMUTH_DEG,
  FAR_LIMB_TIMING_SUN_ZENITH_DEG,
  microsecondsOf,
  QUANTIZED_TIMESTAMP_NS,
  requireDepth,
  settingOutput,
  SLICES_EARTH,
  SMOKE_EV100,
  SMOKE_EXPOSURE_SCALE,
} from "./atmosphere";
import { addCanvas } from "./frames";
import {
  type FrameRecord,
  framesFinite,
  keptOf,
  labelledNs,
  perFrameSums,
  QW_FORM,
  ResolveLedger,
  runTimed,
  sumNs,
  type Timed,
  TIMED_IN_ALL,
  type TimedFrame,
  timedFrameOf,
  viewPassesOf,
} from "./frameTimingLedger";
import {
  describeSources,
  earthInput,
  farLimbFrameScene,
  type FrameScene,
  GENERATED_CLASSES,
  GENERATED_REGION_TEXELS,
  generatedClassLaws,
  generatedClassMapTexels,
  generatedNormalMapTexels,
  GIANT_DISTANCE_M,
  GIANT_EQUATORIAL_RADIUS_M,
  listSources,
  type ListedSources,
  NORMAL_MAP_LEVEL,
  placesScaledTo,
  planetshineShare,
  SATURN_GIANT_DISTANCE_M,
  SMALL_TRIO,
  type SmallBodiesScene,
  type SourceSet,
  smallEarthScene,
  smallEarthsScene,
  smokeClassLaws,
  smokeClassMapTexels,
  smokeNormalMapTexels,
  sourcesAsSet,
  sourceSetsOf,
  type TimedPlace,
  trioSourceSet,
} from "./frameTimingScene";
import { type Checks, frameOf, NEAR_M } from "./harness";
import { percentile } from "./soak";
import {
  iceGiantMethaneBody,
  saturnBody,
  saturnClassMedium,
  titanClassMethaneMedium,
} from "../test/atmosphereFixtures";

/** The group's checks' names begin so. */
const TASK = "R08.T11";

/**
 * The group's checks' names, each part's from one template, so that the run without
 * `timestamp-query` fails the very checks the full run passes.
 */
const CHECK_NAMES = {
  frameTable: (setting: QualitySetting, medium: string): string =>
    `${TASK} the photorealistic frame's timings, ${setting}, ${medium}`,
  frameSums: (setting: QualitySetting, medium: string, place: string, set: string): string =>
    `${TASK} the frame's per-frame sums, ${setting}, ${medium}, ${place}, ${set}`,
  alone: `${TASK} the atmosphere alone's timings, high`,
  aloneSums: (medium: string, place: string, set: string): string =>
    `${TASK} the atmosphere alone's per-frame sums, high, ${medium}, ${place}, ${set}`,
  small: `${TASK} small air bodies' timings, high`,
  smallSums: (diameterPx: number, air: boolean): string =>
    `${TASK} small air bodies' per-frame sums, high, ${diameterPx} px, ${air ? "with" : "without"} air`,
  trio: `${TASK} three small air bodies in one view's timings, high`,
  trioSums: (air: boolean): string =>
    `${TASK} three small air bodies in one view's per-frame sums, high, ${air ? "with" : "without"} air`,
  bare: (medium: string): string => `${TASK} the frame without its air's timings, high, ${medium}`,
  bareSums: (medium: string, place: string, set: string): string =>
    `${TASK} the frame without its air's per-frame sums, high, ${medium}, ${place}, ${set}`,
  tables: `${TASK} the per-planet tables' builds`,
  bytes: `${TASK} the per-view bytes of a body's air`,
  firstAir: `${TASK} the first air frame on the main thread, high`,
  dropped: `${TASK} no timed frame dropped`,
  wall: `${TASK} the group's parts' wall times`,
} as const;

/** The small air bodies' diameters, px (R08.T11's (C)). */
const SMALL_DIAMETERS_PX = [4, 10, 32] as const;

/** The steady frames (F) takes the median of after each first air frame. */
const STEADY_FRAMES = 20;

/** How many first air frames (F) draws, each of a new body. */
const FIRST_AIR_SAMPLES = 3;

/** The Earths' body ids, one a medium, (I)'s and (J)'s; (F)'s new bodies' and (H)'s are apart. */
const EARTH_IDS = {
  r05: "0200080020000000.0311",
  measured: "0200080020000000.0312",
  slab: "0200080020000000.0313",
  classMap: "0200080020000000.0314",
  saturn: "0200080020000000.0315",
  palette: "0200080020000000.0316",
} as const;

/** (H)'s three small bodies' ids, in `SMALL_TRIO`'s order. */
const TRIO_IDS: readonly BodyIdHex[] = [
  "0200080020000000.0341",
  "0200080020000000.0342",
  "0200080020000000.0343",
];

/** (F)'s new bodies, each the measured Earth under its own id. */
const FIRST_AIR_IDS: readonly BodyIdHex[] = [
  "0200080020000000.0331",
  "0200080020000000.0332",
  "0200080020000000.0333",
];

// --- What a part makes ---------------------------------------------------------------------------

/**
 * What a part makes, released in the reverse order whatever throws: `checks.group` catches a
 * group's throw and part 1 goes on with the same engine, so nothing a group made may outlive it.
 */
class Releases {
  readonly #releases: Array<() => void> = [];

  /** Holds `value`, released by `release` when the part ends, and returns it. */
  hold<T>(value: T, release: (held: T) => void): T {
    this.#releases.push(() => {
      release(value);
    });
    return value;
  }

  /** Releases everything held, the latest first; a release that throws is logged, and the rest run. */
  run(): void {
    for (const release of this.#releases.toReversed()) {
      try {
        release();
      } catch (error: unknown) {
        console.error("R08.T11: a release failed:", error);
      }
    }
    this.#releases.length = 0;
  }
}

/** A canvas view of `size` held by `releases`, its canvas removed after it. */
function heldView(
  releases: Releases,
  engine: RenderEngine,
  name: string,
  size: ViewSize,
): RenderView {
  const canvas = releases.hold(addCanvas(), (made) => {
    made.remove();
  });
  const view = releases.hold(engine.createView(canvas, name), (made) => {
    made.dispose();
  });
  view.resize(size);
  return view;
}

// --- The figures ---------------------------------------------------------------------------------

/** The timer kinds a run's frames saw, and how many of their pass times lie off Dawn's grid. */
interface TimerSummary {
  readonly timers: ReadonlySet<GpuTimer>;
  readonly offGrid: number;
  readonly times: number;
}

/** {@link TimerSummary} of `frames`: a pass that ran (above 0 ns) is one time. */
function timerSummary(frames: ReadonlyArray<TimedFrame>): TimerSummary {
  const timers = new Set<GpuTimer>();
  let offGrid = 0;
  let times = 0;
  for (const frame of frames) {
    for (const timer of frame.timers) {
      timers.add(timer);
    }
    for (const { ns } of frame.passes) {
      if (ns > 0) {
        times += 1;
        offGrid += ns % QUANTIZED_TIMESTAMP_NS === 0 ? 0 : 1;
      }
    }
  }
  return { timers, offGrid, times };
}

/** A {@link TimerSummary} as a check's detail states it. */
function timerText(summary: TimerSummary): string {
  const timers = summary.timers.size > 0 ? [...summary.timers].join(", ") : "none";
  return `timer ${timers}, ${summary.offGrid} of ${summary.times} times off the ${QUANTIZED_TIMESTAMP_NS} ns grid`;
}

/** ms as ns, for {@link microsecondsOf}. */
function nsOfMs(ms: number): number {
  return ms * 1e6;
}

/** A count of kept frames as a table's cell: `60`, or `58 (2 missing)`. */
function framesCell(kept: number, missing: number): string {
  return missing === 0 ? String(kept) : `${kept} (${missing} missing)`;
}

/** A µs median / p95 cell of `pick` over `frames`. */
function cell(frames: ReadonlyArray<TimedFrame>, pick: (frame: TimedFrame) => number): string {
  return frames.length === 0 ? "-" : microsecondsOf(frames.map(pick));
}

// --- The media -----------------------------------------------------------------------------------

/** A body's class-map surface as its input carries it: the surface and its body-fixed axes. */
interface MappedSurface {
  readonly surface: DiscSurface;
  readonly rotation: Rotation3;
}

/** One of (A)'s media: its tables, built once, its body and its light slots. */
interface FrameMedium {
  readonly name: string;
  readonly id: BodyIdHex;
  readonly medium: AtmosphereMedium;
  readonly tables: AtmosphereTables;
  /** The renderer's limit on a source's light slot: the curves' slots, `Infinity` with no absorber. */
  readonly lightSlots: number;
  /** The body's figure, its pole +z: WGS 84 for the Earths, Saturn's spheroid for (J)'s. */
  readonly figure: SpheroidFigure;
  /**
   * The giants' distance from the body's centre, m: `GIANT_DISTANCE_M` for the Earths,
   * `SATURN_GIANT_DISTANCE_M` for (J)'s.
   */
  readonly giantDistanceM: number;
  /** The body's class-map surface, (I)'s; `null` for the Lambert sphere of its ground albedo. */
  readonly surface: MappedSurface | null;
}

/** Earth on WGS 84 with its normal gravity, as `timeFarLimbSetting` builds R05's Earth. */
function earthBody(): AtmosphereBody {
  const gravity = SLICES_EARTH.gravity;
  if (gravity === null) {
    throw new Error("the smoke's WGS 84 Earth has its normal gravity");
  }
  return { figure: SLICES_EARTH.figure, gravity };
}

/** Builds `medium`'s tables on WGS 84 with its normal gravity. */
function earthTables(
  engine: RenderEngine,
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
): AtmosphereTables {
  return new AtmosphereTables(
    engine,
    medium,
    tableRadiusM(SLICES_EARTH.figure),
    curves,
    earthBody(),
  );
}

/** One of (A)'s media before its tables are built: its name, body and lights' curves. */
interface FrameMediumSpec {
  readonly name: string;
  readonly id: BodyIdHex;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
}

/** The measured Earth's name in the checks, the medium (B) to (G) draw. */
const MEASURED_EARTH_NAME = "the measured Earth";

/**
 * (A)'s three media: R05's Earth (no absorber), the measured Earth and the slab Earth, each with
 * its curves twice, a binary's two light slots.
 */
const FRAME_MEDIA: ReadonlyArray<FrameMediumSpec> = [
  {
    name: "R05's Earth",
    id: EARTH_IDS.r05,
    medium: R05_EARTH_REFERENCE,
    curves: NO_ABSORBER_CURVES,
  },
  {
    name: MEASURED_EARTH_NAME,
    id: EARTH_IDS.measured,
    medium: EARTH_REFERENCE,
    curves: [...EARTH_REFERENCE_CURVES, ...EARTH_REFERENCE_CURVES],
  },
  {
    name: "the slab Earth",
    id: EARTH_IDS.slab,
    medium: EARTH_SLAB_REFERENCE,
    curves: [...EARTH_SLAB_REFERENCE_CURVES, ...EARTH_SLAB_REFERENCE_CURVES],
  },
];

/** (A)'s media, each its own tables, built once and held by `releases`, and its own body. */
function frameMedia(engine: RenderEngine, releases: Releases): FrameMedium[] {
  return FRAME_MEDIA.map((spec) => ({
    ...spec,
    tables: releases.hold(earthTables(engine, spec.medium, spec.curves), (made) => {
      made.dispose();
    }),
    lightSlots: absorberCount(spec.medium) > 0 ? spec.curves.length : Infinity,
    figure: SLICES_EARTH.figure,
    giantDistanceM: GIANT_DISTANCE_M,
    surface: null,
  }));
}

/** (I)'s judged medium's name in its checks: the R10 fixture's class map. */
const CLASS_MAP_NAME = "the measured Earth under R10's smoke class map";

/** (I)'s second medium's name in its checks: a generated patch's whole palette, judged and reported. */
const PALETTE_NAME = "the measured Earth under a generated palette's class map";

/** (J)'s medium's name in its checks. */
const SATURN_NAME = "TEST SATURN";

/** A class map (I) draws the measured Earth under: its name, id, texels, laws and normals. */
interface ClassMapSpec {
  readonly name: string;
  readonly id: BodyIdHex;
  readonly texels: ClassMapTexels;
  readonly laws: ReadonlyArray<PhotometricLaw>;
  readonly normals: NormalMapTexels;
  /** What its table's detail says it is. */
  readonly about: string;
}

/**
 * (I)'s two class maps: the R10 fixture's as the smoke builds it, judged (`decision-r08-t11-frame.md`
 * §2), the verdict's row, and a generated patch's whole palette, judged and reported too since QW30
 * (main, 2026-10-27).
 */
function classMapSpecs(): ClassMapSpec[] {
  const figure = SLICES_EARTH.figure;
  return [
    {
      name: CLASS_MAP_NAME,
      id: EARTH_IDS.classMap,
      texels: smokeClassMapTexels(),
      laws: smokeClassLaws(),
      normals: smokeNormalMapTexels(figure),
      about: `R10's fixture's class map as the smoke builds it (R10.T10.d's: 2 classes at R10's level, ${CLASS_MAP_FACE_TEXELS} texels a face, half of every face surveyed, the edge column half, the shares turning texel to texel; R10.T10.g's normal map at level ${NORMAL_MAP_LEVEL} over the same half, its cells tilted ±12°), judged`,
    },
    {
      name: PALETTE_NAME,
      id: EARTH_IDS.palette,
      texels: generatedClassMapTexels(),
      laws: generatedClassLaws(),
      normals: generatedNormalMapTexels(figure),
      about: `a generated patch's whole palette (${GENERATED_CLASSES} classes at R10's level, every texel surveyed, three classes a region of ${GENERATED_REGION_TEXELS} × ${GENERATED_REGION_TEXELS} texels, the shares turning smoothly, every class drawn from its maturity nodes; the normal map at level ${NORMAL_MAP_LEVEL} on every cell, tilted ±12°), judged and reported, a row over 16.67 ms at x2 or x2p to main by name`,
    },
  ];
}

/**
 * One of (I)'s media: the measured Earth's tables and air, its body under `spec`'s class map with
 * its laws and its normal map, under the spec's own id; the textures held by `releases`.
 */
function classMapMedium(
  engine: RenderEngine,
  releases: Releases,
  measured: FrameMedium,
  spec: ClassMapSpec,
): FrameMedium {
  const normals = releases.hold(
    normalMapTexture(engine, `${TASK} ${spec.name} normals`, spec.normals),
    (made) => {
      engine.releaseTexture(made.texture);
    },
  );
  const surface = releases.hold(
    classMapSurface(
      engine,
      `${TASK} ${spec.name}`,
      spec.texels,
      spec.laws,
      PROVISIONAL_PHOTOMETRY.law,
      normals,
    ),
    (made) => {
      engine.releaseTexture(made.weights);
    },
  );
  return {
    ...measured,
    name: spec.name,
    id: spec.id,
    surface: { surface, rotation: IDENTITY_ROTATION },
  };
}

/**
 * (J)'s medium: `ATMOSPHERE TEST`'s `saturn-oblate` as its scene and the cache build it
 * (`builtOfCase`), on Saturn's spheroid with its gravity, its tables built once and held by
 * `releases`; `reference/cases.ts` imported as the app imports it, lazily, as (D) has.
 *
 * @throws Error where the case is not on a spheroid.
 */
async function saturnMedium(engine: RenderEngine, releases: Releases): Promise<FrameMedium> {
  const loaded = await import("../view/atmosphere/reference/cases");
  const built = builtOfCase(loaded.SATURN_OBLATE_CASE, "saturn-oblate");
  const body = built.body;
  if (body === null) {
    throw new Error("TEST SATURN's case is on Saturn's spheroid");
  }
  const tables = releases.hold(
    new AtmosphereTables(engine, built.medium, built.bottomRadiusM, built.curves, body),
    (made) => {
      made.dispose();
    },
  );
  return {
    name: SATURN_NAME,
    id: EARTH_IDS.saturn,
    medium: built.medium,
    tables,
    lightSlots: absorberCount(built.medium) > 0 ? built.curves.length : Infinity,
    figure: body.figure,
    giantDistanceM: SATURN_GIANT_DISTANCE_M,
    surface: null,
  };
}

// --- (A) the frame -------------------------------------------------------------------------------

/** One of (A)'s combinations: its setting, medium, camera and set, its frame and its sources. */
interface FrameCombination {
  readonly setting: QualitySetting;
  readonly viewport: ViewSize;
  readonly medium: FrameMedium;
  readonly place: string;
  readonly set: SourceSet;
  readonly scene: FrameScene;
  readonly frame: PhotorealFrame;
  /** The plan's sources for the Earth at its first frame, listed; `null` before it. */
  listed: ListedSources | null;
}

/** What the renderer's frames need besides their scene: a static scene, nothing but the bodies. */
function photorealFrame(
  setting: QualitySetting,
  viewport: ViewSize,
  camera: FrameScene["camera"],
  lights: FrameScene["lights"],
  bodies: ReadonlyArray<LitBodyInput>,
  atmospheres: ReadonlyArray<FrameAtmosphere>,
  failures: string[],
): PhotorealFrame {
  return {
    camera,
    viewport,
    presentedPx: viewport,
    role: "eye",
    setting,
    exposureScale: SMOKE_EXPOSURE_SCALE,
    sky: [],
    starSprites: [],
    hostDraws: new Map(),
    glareSources: [],
    lights,
    bodies,
    depthWriters: [],
    previousRegimes: new Map(),
    overlay: null,
    meter: "average",
    atmospheres,
    onAtmosphereFailed: (body, error) => {
      failures.push(`${body}: ${error instanceof Error ? error.message : String(error)}`);
    },
  };
}

/**
 * The far-limb frame's combination of `setting`, `medium`, a camera and a set: the medium's body
 * on its figure, under its class map where it has one.
 */
function frameCombination(
  setting: QualitySetting,
  medium: FrameMedium,
  place: TimedPlace,
  set: SourceSet,
  failures: string[],
): Timed<FrameCombination> {
  const viewport = settingOutput(setting);
  const scene = farLimbFrameScene(
    place.distanceM,
    viewport,
    set,
    medium.figure,
    medium.giantDistanceM,
  );
  const earth: LitBodyInput = {
    ...earthInput(medium.id, medium.medium, scene.earthCentreM, medium.figure),
    ...medium.surface,
  };
  const air: FrameAtmosphere = {
    id: medium.id,
    tables: medium.tables,
    camera: scene.air,
    toBodyFixed: IDENTITY_QUATERNION,
  };
  return {
    item: {
      setting,
      viewport,
      medium,
      place: place.label,
      set,
      scene,
      frame: photorealFrame(
        setting,
        viewport,
        scene.camera,
        scene.lights,
        [earth, ...scene.giants],
        [air],
        failures,
      ),
      listed: null,
    },
    records: [],
  };
}

/** The labels of the photorealistic pass list, in its order (`photorealisticPasses`). */
function passOrder(setting: QualitySetting): string[] {
  return photorealisticPasses(setting).passes.flatMap((entry) => entry.labels ?? [entry.label]);
}

/** The labels `frames` hold, in the pass list's order, then any other in turn. */
function labelsSeen(setting: QualitySetting, frames: ReadonlyArray<TimedFrame>): string[] {
  const seen = new Set(frames.flatMap((frame) => frame.passes.map((pass) => pass.label)));
  const ordered = passOrder(setting).filter((label) => seen.has(label));
  const others = [...seen].filter((label) => !ordered.includes(label)).toSorted();
  return [...ordered, ...others];
}

/** The atmosphere's share of a frame: its view passes and its composite. */
const ATMOSPHERE_LABELS: ReadonlySet<string> = new Set([
  ATMOSPHERE_VIEW_PASS,
  ATMOSPHERE_COMPOSITE_PASS,
]);

/** (A)'s results: its combinations, timed, and the failures of any body's air to draw. */
interface FrameTimings {
  readonly combinations: ReadonlyArray<Timed<FrameCombination>>;
  readonly failures: ReadonlyArray<string>;
}

/** A renderer and the view it draws into: (A)'s, (C)'s, (G)'s and (F)'s, or (E)'s own. */
interface FrameView {
  readonly renderer: PhotorealRenderer;
  readonly view: RenderView;
  /** The regimes of the latest frame, the next one's hysteresis. */
  regimes: ReadonlyMap<BodyIdHex, LitRegime>;
}

/**
 * A new renderer on a new canvas view at `setting`'s output, both held by `releases`, prepared for
 * `camera`, with the wall time of its `prepare`, ms.
 *
 * @throws Error where the renderer could not be made.
 */
async function heldFrameView(
  releases: Releases,
  engine: RenderEngine,
  name: string,
  setting: QualitySetting,
  camera: FrameScene["camera"],
): Promise<{ readonly on: FrameView; readonly prepareMs: number }> {
  const viewport = settingOutput(setting);
  const view = heldView(releases, engine, name, viewport);
  const renderer = releases.hold(new PhotorealRenderer(engine, name), (made) => {
    made.dispose();
  });
  const start = performance.now();
  await renderer.prepare(viewport, setting, "eye", camera);
  const prepareMs = performance.now() - start;
  if (renderer.status !== "ready") {
    throw new Error(`the renderer is ${renderer.status}, not ready`);
  }
  return { on: { renderer, view, regimes: new Map() }, prepareMs };
}

/** Draws one frame of `frame` on `on`, with the latest regimes; the main thread's time in `render`. */
function renderFrame(
  on: FrameView,
  frame: PhotorealFrame,
): { plan: BodyFramePlan | null; ms: number } {
  const start = performance.now();
  const plan = on.renderer.render(on.view, { ...frame, previousRegimes: on.regimes });
  const ms = performance.now() - start;
  if (plan !== null) {
    on.regimes = plan.regimes;
  }
  return { plan, ms };
}

/** (A): every combination of the settings, the media, the cameras and the sets, timed. */
async function timeFrames(
  on: FrameView,
  ledger: ResolveLedger,
  media: ReadonlyArray<FrameMedium>,
): Promise<FrameTimings> {
  const failures: string[] = [];
  // Medium-major within each setting, so that the renderer makes a body's atmosphere again only
  // where the medium changes.
  const combinations = (["high", "low"] as const).flatMap((setting) =>
    media.flatMap((medium) =>
      FAR_LIMB_TIMING_PLACES.flatMap((place) =>
        sourceSetsOf(setting).map((set) => frameCombination(setting, medium, place, set, failures)),
      ),
    ),
  );
  return timeCombinations(on, ledger, combinations, failures);
}

/**
 * (I): the measured Earth under each of `mapped`'s class maps, from 400 km and 10⁷ m, at `x2`,
 * `x2p` (since R08.T11.d) and the setting's whole count, on high (`decision-r08-t11-frame.md` §2,
 * QW29's class-map row).
 */
async function timeClassMap(
  on: FrameView,
  ledger: ResolveLedger,
  mapped: ReadonlyArray<FrameMedium>,
): Promise<FrameTimings> {
  const failures: string[] = [];
  const places = FAR_LIMB_TIMING_PLACES.slice(0, 2);
  // x2p since R08.T11.d, for QW30's x2p verdicts (main, 2026-10-27).
  const sets = sourceSetsOf("high").filter((set) => set.name !== "x1");
  const combinations = mapped.flatMap((medium) =>
    places.flatMap((place) =>
      sets.map((set) => frameCombination("high", medium, place, set, failures)),
    ),
  );
  return timeCombinations(on, ledger, combinations, failures);
}

/**
 * (J): `TEST SATURN` at (A)'s five cameras scaled to its radius, under the setting's sets as (A)'s,
 * on both settings (`decision-r08-approximate-plate.md` item 3: R08.T6.f's Saturn-class passes),
 * its giants at `SATURN_GIANT_DISTANCE_M`.
 */
async function timeSaturn(
  on: FrameView,
  ledger: ResolveLedger,
  saturn: FrameMedium,
): Promise<FrameTimings> {
  const failures: string[] = [];
  const combinations = (["high", "low"] as const).flatMap((setting) =>
    placesScaledTo(saturn.figure).flatMap((place) =>
      sourceSetsOf(setting).map((set) => frameCombination(setting, saturn, place, set, failures)),
    ),
  );
  return timeCombinations(on, ledger, combinations, failures);
}

/** `combinations`, timed on `on` in QW2's form, each listing its plan's sources at its first frame. */
async function timeCombinations(
  on: FrameView,
  ledger: ResolveLedger,
  combinations: ReadonlyArray<Timed<FrameCombination>>,
  failures: string[],
): Promise<FrameTimings> {
  let size: ViewSize | null = null;
  await runTimed(
    ledger,
    combinations,
    (combination) => {
      if (size?.widthPx !== combination.viewport.widthPx) {
        on.view.resize(combination.viewport);
        size = combination.viewport;
      }
    },
    (combination) => {
      const { plan, ms } = renderFrame(on, combination.frame);
      if (combination.listed === null && plan !== null) {
        const sources = plan.sources.get(combination.medium.id);
        if (sources !== undefined) {
          combination.listed = listSources(
            sources,
            combination.scene.air,
            combination.medium.figure,
            combination.medium.medium,
            combination.medium.lightSlots,
            TABLE_SIZES[combination.setting].marchSourceCap,
          );
        }
      }
      return ms;
    },
  );
  return { combinations, failures };
}

/** The timed frames of `timings` with a resolve missing. */
function missingOf(timings: FrameTimings): number {
  return timings.combinations.reduce((sum, combination) => sum + keptOf(combination).missing, 0);
}

/** Whether a combination drew its set's sources. */
function sourcesHeld(combination: FrameCombination): boolean {
  const listed = combination.listed;
  return (
    listed !== null &&
    sourcesAsSet(listed, combination.set, TABLE_SIZES[combination.setting].marchSourceCap)
  );
}

/** A combination's sources as its check states them. */
function sourcesText(combination: FrameCombination): string {
  const listed = combination.listed;
  if (listed === null) {
    return "no plan held the Earth's sources";
  }
  const share = planetshineShare(listed);
  const shine = share === null ? "" : `; planetshine ${share.toExponential(2)} of the Sun`;
  return `${describeSources(listed)}${shine}`;
}

/**
 * (A)'s checks, and (I)'s and (J)'s: a table a setting and medium, and each combination's per-frame
 * sums.
 *
 * @param about - What a table's medium draws beyond (A)'s scene, a segment of its detail; `null`
 *   for (A)'s.
 */
function reportFrames(
  checks: Checks,
  timings: FrameTimings,
  about: (medium: FrameMedium) => string | null,
): void {
  const tables = new Map<string, Array<Timed<FrameCombination>>>();
  for (const combination of timings.combinations) {
    const { setting, medium } = combination.item;
    const key = `${setting}, ${medium.name}`;
    const table = tables.get(key);
    if (table === undefined) {
      tables.set(key, [combination]);
    } else {
      table.push(combination);
    }
  }
  for (const combinations of tables.values()) {
    const first = combinations[0]?.item;
    if (first === undefined) {
      continue;
    }
    const { setting, viewport } = first;
    const drawn = about(first.medium);
    const kept = combinations.map((combination) => keptOf(combination));
    const all = kept.flatMap(({ frames }) => frames);
    const labels = labelsSeen(setting, all);
    const heading = [
      "camera",
      "set",
      "sum",
      "atmosphere",
      "rest",
      ...labels,
      "CPU in render",
      "frames",
    ].join(" | ");
    const rows = kept.map(({ combination, frames, missing }) => {
      const byLabel = labels.map((label) => cell(frames, (f) => labelledNs(f, new Set([label]))));
      return [
        combination.item.place,
        combination.item.set.name,
        cell(frames, sumNs),
        cell(frames, (f) => labelledNs(f, ATMOSPHERE_LABELS)),
        cell(frames, (f) => sumNs(f) - labelledNs(f, ATMOSPHERE_LABELS)),
        ...byLabel,
        cell(frames, (f) => nsOfMs(f.cpuMs)),
        framesCell(frames.length, missing),
      ].join(" | ");
    });
    const held = kept.every(
      ({ combination, frames }) =>
        frames.length === TIMED_IN_ALL && framesFinite(frames) && sourcesHeld(combination.item),
    );
    const failures =
      timings.failures.length === 0
        ? []
        : [`air failures: ${timings.failures.slice(0, 5).join("; ")}`];
    checks.check(
      CHECK_NAMES.frameTable(setting, first.medium.name),
      held && timings.failures.length === 0,
      [
        `${setting} at ${viewport.widthPx} × ${viewport.heightPx}, PhotorealRenderer on a canvas view, role eye, the average meter, EV100 ${SMOKE_EV100}, no sky, no star sprites, no overlay, no terrain: the Earth drawn as R07's mesh, as R08.T10.b draws every air body not handed to terrain`,
        ...(drawn === null ? [] : [drawn]),
        timerText(timerSummary(all)),
        `${QW_FORM.warmUp} warm-up and ${QW_FORM.timed} timed frames a combination back to back, two sweeps, the second reversed, each frame after frame n − ${QW_FORM.ahead}'s resolves`,
        `each camera on the equator at longitude ${FAR_LIMB_LONGITUDE_RAD} rad, the datum's limb at the frame's centre, 60° across; the Sun at 1 au ${FAR_LIMB_TIMING_SUN_ZENITH_DEG}° from the zenith at the tangent point and ${FAR_LIMB_TIMING_SUN_AZIMUTH_DEG}° right of the view`,
        ...failures,
        `the frame's GPU pass sum and its passes by label, and the main thread's time in render (performance.now(), 0.1 ms steps here), µs, median / p95: ${[heading, ...rows].join(" || ")}`,
      ].join("; "),
    );
  }
  for (const combination of timings.combinations) {
    const { setting, medium, place, set } = combination.item;
    const { frames } = keptOf(combination);
    checks.check(
      CHECK_NAMES.frameSums(setting, medium.name, place, set.name),
      frames.length === TIMED_IN_ALL && framesFinite(frames) && sourcesHeld(combination.item),
      `${sourcesText(combination.item)}; ${frames.length} of ${TIMED_IN_ALL} frames kept; the GPU pass sum, µs, in timed order: ${perFrameSums(combination)}`,
    );
  }
}

// --- (B) the atmosphere alone --------------------------------------------------------------------

/** One of (B)'s combinations: a medium's atmosphere at a camera with (A)'s sources. */
interface AloneCombination {
  readonly medium: FrameMedium;
  readonly atmosphere: HillaireAtmosphere;
  readonly place: string;
  readonly set: string;
  readonly camera: FrameScene["air"];
  readonly suns: ReadonlyArray<SunState>;
}

/** The atmosphere alone's frames' targets: an empty scene and the composite's output. */
interface AloneTargets {
  readonly scene: RenderTarget;
  readonly output: RenderTarget;
}

/** Disposes a target a part held. */
function releaseTarget(held: RenderTarget): void {
  held.dispose();
}

/** A target of `size` as `timeFarLimbSetting`'s, with depth where asked. */
function timingTarget(
  engine: RenderEngine,
  name: string,
  size: ViewSize,
  depth: boolean,
): RenderTarget {
  return engine.createRenderTarget({
    name,
    size,
    format: "rgba16float",
    mips: 1,
    depth,
    category: "render-targets",
  });
}

/**
 * (B): the atmosphere alone over the same tables and (A)'s sources, on high, `x1` and `x2`.
 *
 * @returns Its timed frames with a resolve missing.
 */
async function timeAlone(
  engine: RenderEngine,
  checks: Checks,
  ledger: ResolveLedger,
  media: ReadonlyArray<FrameMedium>,
  frames: FrameTimings,
): Promise<number> {
  const viewport = settingOutput("high");
  const aspect = viewport.widthPx / viewport.heightPx;
  const releases = new Releases();
  try {
    const atmospheres = media.map((medium) =>
      releases.hold(
        HillaireAtmosphere.over(engine, medium.tables, TABLE_SIZES.high, null),
        (made) => {
          made.dispose();
        },
      ),
    );
    const targets: AloneTargets = {
      scene: releases.hold(
        timingTarget(engine, `${TASK} alone scene`, viewport, true),
        releaseTarget,
      ),
      output: releases.hold(
        timingTarget(engine, `${TASK} alone output`, viewport, true),
        releaseTarget,
      ),
    };
    // Nothing drawn, so that every pixel is the bare ground's or the sky's.
    targets.scene.render(frameOf(`${TASK} alone scene`, [], aspect));
    const scene = {
      colour: targets.scene.colour,
      depth: requireDepth(targets.scene.depth),
      nearM: NEAR_M,
      exposureScale: SMOKE_EXPOSURE_SCALE,
    };
    const missingSources: string[] = [];
    const combinations: Array<Timed<AloneCombination>> = [];
    for (const [m, medium] of media.entries()) {
      const atmosphere = atmospheres[m];
      if (atmosphere === undefined) {
        continue;
      }
      for (const place of FAR_LIMB_TIMING_PLACES) {
        for (const set of ["x1", "x2"]) {
          const from = frames.combinations.find(
            ({ item }) =>
              item.setting === "high" &&
              item.medium === medium &&
              item.place === place.label &&
              item.set.name === set,
          )?.item;
          const listed = from?.listed ?? null;
          if (from === undefined || listed === null) {
            missingSources.push(`${medium.name} ${place.label} ${set}`);
            continue;
          }
          combinations.push({
            item: {
              medium,
              atmosphere,
              place: place.label,
              set,
              camera: from.scene.air,
              suns: listed.kept,
            },
            records: [],
          });
        }
      }
    }
    await runTimed(
      ledger,
      combinations,
      () => {
        // Nothing to set up: every combination draws over the same targets.
      },
      (combination) => {
        const start = performance.now();
        const composite = combination.atmosphere.drawFrame(
          combination.camera,
          combination.suns,
          scene,
        );
        targets.output.render(frameOf(ATMOSPHERE_COMPOSITE_PASS, [composite], aspect));
        return performance.now() - start;
      },
    );
    const kept = combinations.map((combination) => keptOf(combination));
    const all = kept.flatMap(({ frames: f }) => f);
    const heading = [
      "medium",
      "camera",
      "set",
      "sky view",
      "aerial perspective",
      "ray march",
      "composite",
      "sum",
      "frames",
    ].join(" | ");
    const rows = kept.map(({ combination, frames: f, missing }) => {
      const { medium, place, set } = combination.item;
      return [
        medium.name,
        place,
        set,
        cell(f, (frame) => viewPassesOf(frame).skyNs),
        cell(f, (frame) => viewPassesOf(frame).aerialNs),
        cell(f, (frame) => viewPassesOf(frame).marchNs + viewPassesOf(frame).moreNs),
        cell(f, (frame) => labelledNs(frame, new Set([ATMOSPHERE_COMPOSITE_PASS]))),
        cell(f, (frame) => labelledNs(frame, ATMOSPHERE_LABELS)),
        framesCell(f.length, missing),
      ].join(" | ");
    });
    const held =
      missingSources.length === 0 &&
      kept.every(({ frames: f }) => f.length === TIMED_IN_ALL && framesFinite(f));
    checks.check(
      CHECK_NAMES.alone,
      held,
      [
        `high at ${viewport.widthPx} × ${viewport.heightPx}, the atmosphere alone as R08.T8.c's far limb timings draw it (its frame over an empty scene at depth 0, then its composite), over the same tables with the sources (A)'s plan gave each combination`,
        timerText(timerSummary(all)),
        missingSources.length === 0
          ? "every combination's sources from (A)"
          : `no sources from (A) for ${missingSources.join(", ")}`,
        `GPU time, µs, median / p95: ${[heading, ...rows].join(" || ")}`,
      ].join("; "),
    );
    for (const combination of combinations) {
      const { medium, place, set } = combination.item;
      const { frames: f } = keptOf(combination);
      checks.check(
        CHECK_NAMES.aloneSums(medium.name, place, set),
        f.length === TIMED_IN_ALL && framesFinite(f),
        `${f.length} of ${TIMED_IN_ALL} frames kept; the GPU pass sum, µs, in timed order: ${perFrameSums(combination)}`,
      );
    }
    return kept.reduce((sum, { missing }) => sum + missing, 0);
  } finally {
    releases.run();
  }
}

// --- (C) small air bodies ------------------------------------------------------------------------

/** One of (C)'s combinations: the measured Earth at a diameter, with its air or without. */
interface SmallCombination {
  readonly diameterPx: number;
  readonly air: boolean;
  readonly frame: PhotorealFrame;
  /** The Earth's regime at the combination's last frame. */
  regime: LitRegime | null;
}

/** (C): the measured Earth whole at 4, 10 and 32 px, with its air and without, on high, `x1`. */
async function timeSmallBodies(
  on: FrameView,
  checks: Checks,
  ledger: ResolveLedger,
  measured: FrameMedium,
): Promise<number> {
  const viewport = settingOutput("high");
  on.view.resize(viewport);
  const failures: string[] = [];
  const combinations: Array<Timed<SmallCombination>> = SMALL_DIAMETERS_PX.flatMap((diameterPx) =>
    [true, false].map((air) => {
      const scene = smallEarthScene(diameterPx, viewport, SLICES_EARTH.figure);
      const earth = earthInput(measured.id, measured.medium, scene.earthCentreM);
      const atmospheres: FrameAtmosphere[] = air
        ? [
            {
              id: measured.id,
              tables: measured.tables,
              camera: scene.air,
              toBodyFixed: IDENTITY_QUATERNION,
            },
          ]
        : [];
      return {
        item: {
          diameterPx,
          air,
          frame: photorealFrame(
            "high",
            viewport,
            scene.camera,
            scene.lights,
            [earth],
            atmospheres,
            failures,
          ),
          regime: null,
        },
        records: [],
      };
    }),
  );
  await runTimed(
    ledger,
    combinations,
    () => {
      // One view at high's output for every combination.
    },
    (combination) => {
      const { plan, ms } = renderFrame(on, combination.frame);
      combination.regime = plan?.regimes.get(measured.id) ?? null;
      return ms;
    },
  );
  const kept = combinations.map((combination) => keptOf(combination));
  const all = kept.flatMap(({ frames }) => frames);
  const passes = [
    PHOTOREAL_PASS_LABELS.discCells,
    PHOTOREAL_PASS_LABELS.bodies,
    PHOTOREAL_PASS_LABELS.discs,
  ];
  const heading = ["diameter", "air", "regime", ...passes, "atmosphere", "sum", "frames"].join(
    " | ",
  );
  const rows = kept.map(({ combination, frames, missing }) =>
    [
      `${combination.item.diameterPx} px`,
      combination.item.air ? "with" : "without",
      combination.item.regime ?? "none",
      ...passes.map((label) => cell(frames, (f) => labelledNs(f, new Set([label])))),
      cell(frames, (f) => labelledNs(f, ATMOSPHERE_LABELS)),
      cell(frames, sumNs),
      framesCell(frames.length, missing),
    ].join(" | "),
  );
  checks.check(
    CHECK_NAMES.small,
    kept.every(({ frames }) => frames.length === TIMED_IN_ALL && framesFinite(frames)) &&
      failures.length === 0,
    [
      `high at ${viewport.widthPx} × ${viewport.heightPx}, the measured Earth whole, centred, 60° across, the Sun at 1 au 40° off the line of sight (x1), with its air and without`,
      timerText(timerSummary(all)),
      ...(failures.length === 0 ? [] : [`air failures: ${failures.slice(0, 5).join("; ")}`]),
      `GPU time, µs, median / p95: ${[heading, ...rows].join(" || ")}`,
    ].join("; "),
  );
  for (const combination of combinations) {
    const { frames } = keptOf(combination);
    checks.check(
      CHECK_NAMES.smallSums(combination.item.diameterPx, combination.item.air),
      frames.length === TIMED_IN_ALL && framesFinite(frames),
      `${frames.length} of ${TIMED_IN_ALL} frames kept; the GPU pass sum, µs, in timed order: ${perFrameSums(combination)}`,
    );
  }
  return kept.reduce((sum, { missing }) => sum + missing, 0);
}

// --- (H) three small air bodies in one view ------------------------------------------------------

/** (H)'s bodies as its check names them: `32 px at -6°`. */
const SMALL_TRIO_TEXT: readonly string[] = SMALL_TRIO.map(
  ({ diameterPx, offRad }) => `${diameterPx} px at ${Math.round((offRad * 180) / Math.PI)}°`,
);

/** One of (H)'s combinations: the three small Earths, with their air or without. */
interface TrioCombination {
  readonly air: boolean;
  readonly scene: SmallBodiesScene;
  readonly frame: PhotorealFrame;
  /** The bodies' regimes at the combination's last frame, in `SMALL_TRIO`'s order. */
  regimes: ReadonlyArray<LitRegime | null>;
  /** Each body's sources as the plan listed them at the first frame with air; `null` before it. */
  listed: ReadonlyArray<ListedSources | null> | null;
}

/**
 * (H): three small air bodies in one view (QW29, `decision-r08-t11-frame.md` §2), the measured
 * Earth 32, 10 and 4 px across, each its own body and air over the measured Earth's tables, at `x2`
 * on high, with their air and without; recorded, not judged.
 *
 * @returns Its timed frames with a resolve missing.
 */
async function timeSmallTrio(
  on: FrameView,
  checks: Checks,
  ledger: ResolveLedger,
  measured: FrameMedium,
): Promise<number> {
  const viewport = settingOutput("high");
  on.view.resize(viewport);
  const failures: string[] = [];
  const set = sourceSetsOf("high").find((each) => each.name === "x2");
  if (set === undefined) {
    throw new Error("high's sets hold x2");
  }
  const cap = TABLE_SIZES.high.marchSourceCap;
  const combinations: Array<Timed<TrioCombination>> = [true, false].map((air) => {
    const scene = smallEarthsScene(viewport, SLICES_EARTH.figure, set);
    const bodies = scene.bodies.flatMap((body, i) => {
      const id = TRIO_IDS[i];
      return id === undefined ? [] : [{ id, body }];
    });
    const atmospheres: FrameAtmosphere[] = air
      ? bodies.map(({ id, body }) => ({
          id,
          tables: measured.tables,
          camera: body.air,
          toBodyFixed: IDENTITY_QUATERNION,
        }))
      : [];
    return {
      item: {
        air,
        scene,
        frame: photorealFrame(
          "high",
          viewport,
          scene.camera,
          scene.lights,
          bodies.map(({ id, body }) => earthInput(id, measured.medium, body.centreM)),
          atmospheres,
          failures,
        ),
        regimes: [],
        listed: null,
      },
      records: [],
    };
  });
  await runTimed(
    ledger,
    combinations,
    () => {
      // One view at high's output for every combination.
    },
    (combination) => {
      const { plan, ms } = renderFrame(on, combination.frame);
      combination.regimes = TRIO_IDS.map((id) => plan?.regimes.get(id) ?? null);
      if (combination.air && combination.listed === null && plan !== null) {
        combination.listed = combination.scene.bodies.map((body, i) => {
          const id = TRIO_IDS[i];
          const sources = id === undefined ? undefined : plan.sources.get(id);
          return sources === undefined
            ? null
            : listSources(
                sources,
                body.air,
                SLICES_EARTH.figure,
                measured.medium,
                measured.lightSlots,
                cap,
              );
        });
      }
      return ms;
    },
  );
  const drawn = trioSourceSet(set, "high");
  const held = (combination: TrioCombination): boolean =>
    !combination.air ||
    (combination.listed !== null &&
      combination.listed.length === TRIO_IDS.length &&
      combination.listed.every((listed) => listed !== null && sourcesAsSet(listed, drawn, cap)));
  const kept = combinations.map((combination) => keptOf(combination));
  const all = kept.flatMap(({ frames }) => frames);
  const passes = [
    PHOTOREAL_PASS_LABELS.discCells,
    PHOTOREAL_PASS_LABELS.bodies,
    PHOTOREAL_PASS_LABELS.discs,
  ];
  const heading = ["air", "regimes", ...passes, "atmosphere", "sum", "frames"].join(" | ");
  const rows = kept.map(({ combination, frames, missing }) =>
    [
      combination.item.air ? "with" : "without",
      combination.item.regimes.map((regime) => regime ?? "none").join(", "),
      ...passes.map((label) => cell(frames, (f) => labelledNs(f, new Set([label])))),
      cell(frames, (f) => labelledNs(f, ATMOSPHERE_LABELS)),
      cell(frames, sumNs),
      framesCell(frames.length, missing),
    ].join(" | "),
  );
  const sources = combinations
    .find(({ item }) => item.air)
    ?.item.listed?.map((listed, i) =>
      listed === null
        ? `${SMALL_TRIO_TEXT[i] ?? "?"}: no plan held its sources`
        : `${SMALL_TRIO_TEXT[i] ?? "?"}: ${describeSources(listed)}`,
    )
    .join("; ");
  checks.check(
    CHECK_NAMES.trio,
    kept.every(
      ({ combination, frames }) =>
        frames.length === TIMED_IN_ALL && framesFinite(frames) && held(combination.item),
    ) && failures.length === 0,
    [
      `high at ${viewport.widthPx} × ${viewport.heightPx}, the measured Earth whole three times in one view, ${SMALL_TRIO_TEXT.join(", ")} (each its own body and air over the same tables), the Sun at 1 au 40° off the middle one's line of sight and a binary's second star (x2), with their air and without; each body lit by the two stars and the other two's planetshine, ${drawn.stars} + ${drawn.giants}, as the plan gives every lit body`,
      timerText(timerSummary(all)),
      `sources with air: ${sources ?? "none listed"}`,
      ...(failures.length === 0 ? [] : [`air failures: ${failures.slice(0, 5).join("; ")}`]),
      `GPU time, µs, median / p95: ${[heading, ...rows].join(" || ")}`,
    ].join("; "),
  );
  for (const combination of combinations) {
    const { frames } = keptOf(combination);
    checks.check(
      CHECK_NAMES.trioSums(combination.item.air),
      frames.length === TIMED_IN_ALL && framesFinite(frames) && held(combination.item),
      `${frames.length} of ${TIMED_IN_ALL} frames kept; the GPU pass sum, µs, in timed order: ${perFrameSums(combination)}`,
    );
  }
  return kept.reduce((sum, { missing }) => sum + missing, 0);
}

// --- (G) the frame without its air ---------------------------------------------------------------

/** One of (G)'s combinations: (A)'s frame of the measured Earth with no air drawn. */
interface BareCombination {
  readonly place: string;
  readonly set: SourceSet;
  readonly frame: PhotorealFrame;
  /** The Earth's regime at the combination's last frame. */
  regime: LitRegime | null;
}

/**
 * (G), the control without air: (A)'s frames of the measured Earth on high, at the five cameras,
 * `x1`, `x2` and the setting's whole count, with the same scene and sources but no atmosphere
 * (`atmospheres: []`), so that R07's regimes draw the body bare (disc or mesh as the plan picks,
 * the `disc cells` pass where it runs): the air's own share of `bodies` and `discs` (the mesh
 * promotion, the limb's draw, the surface lit through the air) apart from R07's bare frame.
 *
 * @returns Its timed frames with a resolve missing.
 */
async function timeBare(
  on: FrameView,
  checks: Checks,
  ledger: ResolveLedger,
  measured: FrameMedium,
): Promise<number> {
  const viewport = settingOutput("high");
  on.view.resize(viewport);
  const failures: string[] = [];
  const sets = sourceSetsOf("high").filter((set) => set.name !== "x2p");
  const combinations: Array<Timed<BareCombination>> = FAR_LIMB_TIMING_PLACES.flatMap((place) =>
    sets.map((set) => {
      const { item } = frameCombination("high", measured, place, set, failures);
      return {
        item: { place: place.label, set, frame: { ...item.frame, atmospheres: [] }, regime: null },
        records: [],
      };
    }),
  );
  await runTimed(
    ledger,
    combinations,
    () => {
      // One view at high's output for every combination.
    },
    (combination) => {
      const { plan, ms } = renderFrame(on, combination.frame);
      combination.regime = plan?.regimes.get(measured.id) ?? null;
      return ms;
    },
  );
  const kept = combinations.map((combination) => keptOf(combination));
  const all = kept.flatMap(({ frames }) => frames);
  const labels = labelsSeen("high", all);
  const heading = ["camera", "set", "regime", "sum", ...labels, "CPU in render", "frames"].join(
    " | ",
  );
  const rows = kept.map(({ combination, frames, missing }) =>
    [
      combination.item.place,
      combination.item.set.name,
      combination.item.regime ?? "none",
      cell(frames, sumNs),
      ...labels.map((label) => cell(frames, (f) => labelledNs(f, new Set([label])))),
      cell(frames, (f) => nsOfMs(f.cpuMs)),
      framesCell(frames.length, missing),
    ].join(" | "),
  );
  checks.check(
    CHECK_NAMES.bare(measured.name),
    kept.every(({ frames }) => frames.length === TIMED_IN_ALL && framesFinite(frames)) &&
      failures.length === 0,
    [
      `high at ${viewport.widthPx} × ${viewport.heightPx}, (A)'s frames of ${measured.name} with the same scene and sources but no atmosphere drawn (atmospheres: []), R07's regimes drawing the body bare`,
      timerText(timerSummary(all)),
      `the frame's GPU pass sum and its passes by label, and the main thread's time in render (performance.now(), 0.1 ms steps here), µs, median / p95: ${[heading, ...rows].join(" || ")}`,
    ].join("; "),
  );
  for (const combination of combinations) {
    const { frames } = keptOf(combination);
    checks.check(
      CHECK_NAMES.bareSums(measured.name, combination.item.place, combination.item.set.name),
      frames.length === TIMED_IN_ALL && framesFinite(frames),
      `${frames.length} of ${TIMED_IN_ALL} frames kept; the GPU pass sum, µs, in timed order: ${perFrameSums(combination)}`,
    );
  }
  return kept.reduce((sum, { missing }) => sum + missing, 0);
}

// --- (D) the per-planet tables ---------------------------------------------------------------------

/** One medium (D) builds tables for: on its ground, with its curves and its body. */
interface TablesCase {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly bottomRadiusM: number;
  readonly curves: AbsorberCurveSlots;
  readonly body: AtmosphereBody | null;
}

/** The test fixtures' and the smoke's media of (D), on their figures. */
function fixtureCases(): TablesCase[] {
  const earth = earthBody();
  const radiusM = tableRadiusM(earth.figure);
  const saturn = saturnBody();
  const iceGiant = iceGiantMethaneBody();
  const titan = titanClassMethaneMedium();
  return [
    {
      name: "R05's Earth",
      medium: R05_EARTH_REFERENCE,
      bottomRadiusM: radiusM,
      curves: NO_ABSORBER_CURVES,
      body: earth,
    },
    {
      name: "the measured Earth, one light slot",
      medium: EARTH_REFERENCE,
      bottomRadiusM: radiusM,
      curves: EARTH_REFERENCE_CURVES,
      body: earth,
    },
    {
      name: "the measured Earth, two light slots",
      medium: EARTH_REFERENCE,
      bottomRadiusM: radiusM,
      curves: [...EARTH_REFERENCE_CURVES, ...EARTH_REFERENCE_CURVES],
      body: earth,
    },
    {
      name: "the slab Earth, two light slots",
      medium: EARTH_SLAB_REFERENCE,
      bottomRadiusM: radiusM,
      curves: [...EARTH_SLAB_REFERENCE_CURVES, ...EARTH_SLAB_REFERENCE_CURVES],
      body: earth,
    },
    {
      name: "the Saturn-class figure",
      medium: saturnClassMedium(),
      bottomRadiusM: tableRadiusM(saturn.figure),
      curves: NO_ABSORBER_CURVES,
      body: saturn,
    },
    {
      name: "the ice giant",
      medium: iceGiant.medium,
      bottomRadiusM: tableRadiusM(iceGiant.body.figure),
      curves: iceGiant.curves,
      body: iceGiant.body,
    },
    {
      name: "the Titan-class methane",
      medium: titan.medium,
      bottomRadiusM: titan.radiusM,
      curves: titan.curves,
      body: null,
    },
  ];
}

/**
 * `ATMOSPHERE TEST`'s four kept fixtures, as its scene, `AtmosphereFixtures` and the cache build
 * them (`builtOfCase`), from `reference/cases.ts`, imported here as the app imports it, lazily;
 * with the import's main-thread time, ms.
 */
async function keptFixtureCases(): Promise<{
  readonly cases: TablesCase[];
  readonly importMs: number;
}> {
  const start = performance.now();
  const loaded = await import("../view/atmosphere/reference/cases");
  const importMs = performance.now() - start;
  const named = [
    ["earth", loaded.EARTH_CASE],
    ["earth-ozone-slab", loaded.EARTH_OZONE_SLAB_CASE],
    ["mars", loaded.MARS_CASE],
    ["saturn-oblate", loaded.SATURN_OBLATE_CASE],
  ] as const;
  const cases = named.map(([name, referenceCase]): TablesCase => {
    const built = builtOfCase(referenceCase, name);
    return {
      name: `ATMOSPHERE TEST's ${name}`,
      medium: built.medium,
      bottomRadiusM: built.bottomRadiusM,
      curves: built.curves,
      body: built.body,
    };
  });
  return { cases, importMs };
}

/** One build of a case's tables: its times, its plan and bytes, and the allocations it made. */
interface TablesBuild {
  readonly constructorMs: number;
  readonly gpuPasses: number;
  readonly gpuNs: number;
  readonly irradianceMs: number;
  readonly irradiancePasses: number;
  readonly irradianceNs: number;
  readonly slices: number;
  readonly bands: number;
  readonly rowBytes: number;
  readonly created: ReadonlyMap<MemoryCategory, number>;
  readonly destroyedBytes: number;
  readonly uploadedBytes: number;
  readonly complete: boolean;
}

/** The main thread's share of a case's build, each step timed alone after the builds. */
interface TablesSplit {
  readonly keyMs: number;
  readonly packsMs: number;
  /** The forward lobes made anew, {@link LOBE_RUNS} times in turn, each run's ms. */
  readonly lobesUncachedMs: ReadonlyArray<number>;
  readonly lobeTerms: number;
  readonly lobesCachedMs: number;
  readonly curvesMs: number;
}

/** The passes of `record` labelled `label`, counted and summed, and whether it arrived whole. */
function tablePasses(
  record: FrameRecord,
  label: string,
): { readonly count: number; readonly ns: number; readonly complete: boolean } {
  const frame = timedFrameOf(record);
  if (frame === null) {
    return { count: 0, ns: 0, complete: false };
  }
  const passes = frame.passes.filter((pass) => pass.label === label);
  return {
    count: passes.length,
    ns: passes.reduce((sum, pass) => sum + pass.ns, 0),
    complete: true,
  };
}

/**
 * Builds `entry`'s tables once and records the build: the constructor's main-thread time, the GPU
 * time of the passes it dispatched (which ride the flush's resolve), and the same of the sky's
 * irradiance at its first read (`AtmosphereTables.irradiance`), the plan, the bytes and the
 * engine's allocation tally over the constructor.
 */
async function buildTables(
  engine: RenderEngine,
  ledger: ResolveLedger,
  flush: RenderTarget,
  entry: TablesCase,
): Promise<TablesBuild> {
  const created = new Map<MemoryCategory, number>();
  let destroyedBytes = 0;
  let uploadedBytes = 0;
  const stop = engine.onAllocation((event) => {
    if (event.name.startsWith("pass times ")) {
      // The timer's own resolve buffers, made as they are needed.
      return;
    }
    switch (event.kind) {
      case "created":
        created.set(event.category, (created.get(event.category) ?? 0) + event.bytes);
        break;
      case "destroyed":
        destroyedBytes += event.bytes;
        break;
      case "uploaded":
        uploadedBytes += event.bytes;
        break;
    }
  });
  const flushFrame = (): void => {
    flush.render(frameOf(`${TASK} tables flush`, [], 1));
  };
  const releases = new Releases();
  try {
    let built: FrameRecord;
    let constructorMs: number;
    let tables: AtmosphereTables;
    try {
      const before = ledger.count;
      const start = performance.now();
      tables = releases.hold(
        new AtmosphereTables(engine, entry.medium, entry.bottomRadiusM, entry.curves, entry.body),
        (made) => {
          made.dispose();
        },
      );
      constructorMs = performance.now() - start;
      flushFrame();
      built = ledger.frame(before, constructorMs);
    } finally {
      // The tally covers the constructor and its flush alone.
      stop();
    }
    const before = ledger.count;
    const start = performance.now();
    tables.irradiance();
    const irradianceMs = performance.now() - start;
    flushFrame();
    const irradiance = ledger.frame(before, irradianceMs);
    await engine.readTexture(flush.colour, 0, { x: 0, y: 0, width: 1, height: 1 });
    await ledger.settled(built, QW_FORM.waitMs);
    await ledger.settled(irradiance, QW_FORM.waitMs);
    const build = tablePasses(built, ATMOSPHERE_TABLES_PASS);
    const lit = tablePasses(irradiance, ATMOSPHERE_TABLES_PASS);
    return {
      constructorMs,
      gpuPasses: build.count,
      gpuNs: build.ns,
      irradianceMs,
      irradiancePasses: lit.count,
      irradianceNs: lit.ns,
      slices: tables.plan.kappa.length,
      bands: tables.plan.bands.length,
      rowBytes: tables.rowBytes,
      created,
      destroyedBytes,
      uploadedBytes,
      complete: build.complete && lit.complete,
    };
  } finally {
    releases.run();
  }
}

/** The milliseconds `work` takes on the main thread. */
function timedMs(work: () => unknown): number {
  const start = performance.now();
  work();
  return performance.now() - start;
}

/** How many times (D)'s split makes a medium's forward lobes anew: twice, to show their spread. */
const LOBE_RUNS = 2;

/** A medium's forward lobes made anew: each run's ms, and its tabulated terms. */
interface UncachedLobes {
  readonly ms: ReadonlyArray<number>;
  readonly terms: number;
}

/**
 * The main thread's split of a build (`AtmosphereTables.setMedium`'s steps), each timed alone:
 * the key, the medium's packs, the forward-lobe tables uncached (`forwardLobeOf` on each tabulated
 * term, the cache's miss, {@link LOBE_RUNS} times, once a medium) and cached (`packLobeTables`, a
 * hit once built), and the curves' pack.
 */
function splitOf(
  entry: TablesCase,
  lobesUncached: Map<AtmosphereMedium, UncachedLobes>,
): TablesSplit {
  const { medium, bottomRadiusM, curves, body } = entry;
  const direct = { ...medium, terms: medium.terms.map(withoutForwardDelta) };
  const keyMs = timedMs(() => JSON.stringify({ medium, bottomRadiusM, curves, body }));
  const packsMs = timedMs(() => {
    packMedium(direct);
    packDensityTables(direct);
    packPhaseTables(direct);
  });
  let lobes = lobesUncached.get(medium);
  if (lobes === undefined) {
    const tabulated = direct.terms.flatMap((term) =>
      term.phase.kind === "tabulated" ? [term.phase.table] : [],
    );
    const ms = Array.from({ length: tabulated.length === 0 ? 1 : LOBE_RUNS }, () =>
      timedMs(() => {
        for (const table of tabulated) {
          forwardLobeOf(resampledPhase(table));
        }
      }),
    );
    lobes = { ms, terms: tabulated.length };
    lobesUncached.set(medium, lobes);
  }
  const lobesCachedMs = timedMs(() => packLobeTables(direct));
  const curvesMs = timedMs(() => packAbsorberCurves(medium, curves));
  return {
    keyMs,
    packsMs,
    lobesUncachedMs: lobes.ms,
    lobeTerms: lobes.terms,
    lobesCachedMs,
    curvesMs,
  };
}

/** A tally by category as a cell: `atmosphere-tables 1,234,567 B, other 512 B`. */
function tallyText(tally: ReadonlyMap<MemoryCategory, number>): string {
  return tally.size === 0
    ? "none"
    : [...tally].map(([category, bytes]) => `${category} ${bytes} B`).join(", ");
}

/**
 * (D): every case's tables built twice, cold (the first build in the group) and warm, then the
 * main thread's split; one check with both tables.
 */
async function timeTables(
  engine: RenderEngine,
  checks: Checks,
  ledger: ResolveLedger,
): Promise<void> {
  const kept = await keptFixtureCases();
  const cases = [...fixtureCases(), ...kept.cases];
  const rows: string[] = [];
  const splits: string[] = [];
  const lobesUncached = new Map<AtmosphereMedium, UncachedLobes>();
  let held = true;
  const flush = timingTarget(engine, `${TASK} tables flush`, { widthPx: 1, heightPx: 1 }, false);
  try {
    for (const entry of cases) {
      // Each build after the last, cold then warm, so that the second finds what the first left.
      // oxlint-disable-next-line no-await-in-loop
      const cold = await buildTables(engine, ledger, flush, entry);
      // oxlint-disable-next-line no-await-in-loop
      const warm = await buildTables(engine, ledger, flush, entry);
      for (const [tag, build] of [
        ["cold", cold],
        ["warm", warm],
      ] as const) {
        held &&=
          build.complete && Number.isFinite(build.gpuNs) && Number.isFinite(build.constructorMs);
        rows.push(
          [
            entry.name,
            tag,
            build.constructorMs.toFixed(1),
            `${build.gpuPasses}, ${(build.gpuNs / 1_000).toFixed(1)}`,
            `${build.irradianceMs.toFixed(1)}; ${build.irradiancePasses}, ${(build.irradianceNs / 1_000).toFixed(1)}`,
            `${build.slices}, ${build.bands}`,
            String(build.rowBytes),
            tallyText(build.created),
            String(build.destroyedBytes),
            String(build.uploadedBytes),
          ].join(" | "),
        );
      }
      const split = splitOf(entry, lobesUncached);
      const parts = split.keyMs + split.packsMs + split.lobesCachedMs + split.curvesMs;
      splits.push(
        [
          entry.name,
          split.keyMs.toFixed(1),
          split.packsMs.toFixed(1),
          `${split.lobesUncachedMs.map((ms) => ms.toFixed(1)).join(", ")} (${split.lobeTerms})`,
          split.lobesCachedMs.toFixed(1),
          split.curvesMs.toFixed(1),
          (warm.constructorMs - parts).toFixed(1),
          (cold.constructorMs - warm.constructorMs).toFixed(1),
        ].join(" | "),
      );
    }
  } finally {
    flush.dispose();
  }
  const heading = [
    "medium",
    "build",
    "constructor, ms",
    "its passes: count, GPU µs",
    "irradiance at first read: ms; passes, GPU µs",
    "slices, bands",
    "rowBytes, B",
    "allocated by category",
    "destroyed, B",
    "uploaded, B",
  ].join(" | ");
  const splitHeading = [
    "medium",
    "key, ms",
    "packs, ms",
    "lobes uncached, ms, each run (tabulated terms)",
    "lobes cached, ms",
    "curves, ms",
    "the warm constructor's rest, ms",
    "cold − warm, ms",
  ].join(" | ");
  checks.check(
    CHECK_NAMES.tables,
    held,
    [
      `each medium's tables built twice by new AtmosphereTables, cold (the first in this group; R05.T12.b's tables and the earlier groups of part 1 compiled the kernels before) and warm, each disposed after; the GPU time of the passes each dispatched, which ride the next resolve (a 1 × 1 flush's), after a fence; the irradiance at its first read after the build`,
      `reference/cases.ts imported in ${kept.importMs.toFixed(1)} ms`,
      `the builds: ${[heading, ...rows].join(" || ")}`,
      `the main thread's split, each step timed alone after the builds (setMedium's: the key's JSON, packMedium, packDensityTables and packPhaseTables, the forward lobes uncached by forwardLobeOf and cached by packLobeTables, packAbsorberCurves), the rest by difference; a cold build whose excess over the warm one is near the lobes' uncached time missed the lobe cache: ${[splitHeading, ...splits].join(" || ")}`,
    ].join("; "),
  );
}

// --- (E) the per-view bytes ----------------------------------------------------------------------

/** A frame's allocations as (E) records them: creations and destructions listed, uploads summed. */
interface FrameAllocations {
  /** The air's: its per-view tables (`atmosphere-view`), its march's buffers and the air targets. */
  readonly air: string[];
  /** Every other creation or destruction. */
  readonly other: string[];
  uploads: number;
  uploadedBytes: number;
}

/** Whether a creation or destruction is the air's: by its category, or its owner's name. */
function airAllocation(name: string, category: MemoryCategory): boolean {
  return category === "atmosphere-view" || name.startsWith("atmosphere ") || name.includes(":air ");
}

/** Records one event into `into`, the timer's own resolve buffers left out. */
function recordAllocation(into: FrameAllocations, event: AllocationEvent): void {
  if (event.name.startsWith("pass times ")) {
    return;
  }
  switch (event.kind) {
    case "uploaded":
      into.uploads += 1;
      into.uploadedBytes += event.bytes;
      break;
    case "created":
    case "destroyed": {
      const text = `${event.kind} ${event.name} (${event.category}) ${event.bytes} B`;
      (airAllocation(event.name, event.category) ? into.air : into.other).push(text);
      break;
    }
  }
}

/** A list of allocations as (E)'s line states it, `none` for none. */
function listText(items: ReadonlyArray<string>): string {
  return items.length === 0 ? "none" : items.join(", ");
}

/** A frame's allocations as a line of (E)'s check. */
function allocationsText(tag: string, made: FrameAllocations): string {
  return `${tag}: the air's ${listText(made.air)}; other ${listText(made.other)}; ${made.uploads} uploads, ${made.uploadedBytes} B`;
}

/**
 * (E): the allocations a new renderer makes at its first frame that draws air, at high and low,
 * for the measured Earth from 400 km, and at the frames after with one, two and the setting's
 * whole count of sources; with each renderer's `prepare` time, ms.
 */
async function perViewBytes(
  engine: RenderEngine,
  checks: Checks,
  measured: FrameMedium,
): Promise<ReadonlyArray<string>> {
  const prepares: string[] = [];
  const lines: string[] = [];
  const changed: string[] = [];
  let held = true;
  for (const setting of ["high", "low"] as const) {
    const sets = sourceSetsOf(setting).filter((set) => set.name !== "x2p");
    const place = FAR_LIMB_TIMING_PLACES[0];
    if (place === undefined) {
      continue;
    }
    const failures: string[] = [];
    const frames = sets.map(
      (set) => frameCombination(setting, measured, place, set, failures).item,
    );
    const first = frames[0];
    if (first === undefined) {
      continue;
    }
    const releases = new Releases();
    try {
      // Each setting's renderer in turn, released before the next is made.
      // oxlint-disable-next-line no-await-in-loop
      const { on, prepareMs } = await heldFrameView(
        releases,
        engine,
        `${TASK} bytes ${setting}`,
        setting,
        first.scene.camera,
      );
      prepares.push(`${setting} ${prepareMs.toFixed(1)} ms`);
      // A frame without the air first, so that what the first air frame makes is the air's alone.
      renderFrame(on, { ...first.frame, atmospheres: [] });
      for (const [i, combination] of frames.entries()) {
        const made: FrameAllocations = { air: [], other: [], uploads: 0, uploadedBytes: 0 };
        const stop = engine.onAllocation((event) => {
          recordAllocation(made, event);
        });
        try {
          renderFrame(on, combination.frame);
        } finally {
          stop();
        }
        const tag = `${setting} ${combination.set.name}${i === 0 ? ", the first air frame" : ""}`;
        lines.push(allocationsText(tag, made));
        // The first air frame makes the air's targets and the body's per-view tables; the later
        // frames, at more sources, should make none: a figure, recorded, not held.
        if (i > 0 && made.air.length > 0) {
          changed.push(`${setting} ${combination.set.name}`);
        }
      }
      held &&= failures.length === 0;
    } finally {
      releases.run();
    }
  }
  checks.check(
    CHECK_NAMES.bytes,
    held,
    [
      `a new PhotorealRenderer a setting on its own canvas view, the measured Earth from 400 km, one frame without its air, then its first air frame (x1) and frames at two and the setting's whole count of sources; each frame's creations and destructions (onAllocation), the air's (its per-view tables, its march's buffers, the air targets) apart from the rest, and its uploads summed, the timer's resolve buffers left out`,
      `prepare ${prepares.join(", ")}`,
      changed.length === 0
        ? "the air allocates nothing at two and more sources"
        : `the air allocates at more sources: ${changed.join(", ")}`,
      lines.join(" || "),
    ].join("; "),
  );
  return prepares;
}

// --- (F) the first air frame ---------------------------------------------------------------------

/** The renderers' `prepare` times, ms, as (F) reports them: (A)'s and (E)'s. */
interface PrepareTimes {
  readonly frameMs: number;
  readonly bytes: ReadonlyArray<string>;
}

/**
 * (F): the main thread's first frame of a new body's air (its `HillaireAtmosphere.over` and first
 * `drawFrame`) against the median steady frame's, at high, for the measured Earth from 400 km,
 * {@link FIRST_AIR_SAMPLES} times, each a new body over the same tables on (A)'s renderer; then the
 * same steps alone over (A)'s sources for that frame, and the renderers' `prepare`.
 */
async function firstAirFrame(
  engine: RenderEngine,
  checks: Checks,
  on: FrameView,
  ledger: ResolveLedger,
  measured: FrameMedium,
  frames: FrameTimings,
  prepares: PrepareTimes,
): Promise<void> {
  const viewport = settingOutput("high");
  on.view.resize(viewport);
  const place = FAR_LIMB_TIMING_PLACES[0];
  const set = sourceSetsOf("high")[0];
  if (place === undefined || set === undefined) {
    throw new Error("the far limb's cameras and sets are not empty");
  }
  const failures: string[] = [];
  const samples: string[] = [];
  let held = true;
  for (const id of FIRST_AIR_IDS.slice(0, FIRST_AIR_SAMPLES)) {
    const { item } = frameCombination("high", { ...measured, id }, place, set, failures);
    const first = renderFrame(on, item.frame);
    const records: FrameRecord[] = [];
    const ms: number[] = [];
    for (let k = 0; k < STEADY_FRAMES; k += 1) {
      const ahead =
        records.length >= QW_FORM.ahead ? records[records.length - QW_FORM.ahead] : undefined;
      if (ahead !== undefined) {
        // Frame n − 3's resolves, as the timed runs wait, so that the queue stays as theirs.
        // oxlint-disable-next-line no-await-in-loop
        await ledger.settled(ahead, QW_FORM.waitMs);
      }
      const before = ledger.count;
      const frame = renderFrame(on, item.frame);
      records.push(ledger.frame(before, frame.ms));
      ms.push(frame.ms);
    }
    const median = percentile(ms, 0.5);
    held &&= first.plan !== null && Number.isFinite(first.ms) && Number.isFinite(median);
    samples.push(
      `${id}: the first air frame ${first.ms.toFixed(2)} ms against the steady frames' median ${median.toFixed(2)} ms (p95 ${percentile(ms, 0.95).toFixed(2)} ms)`,
    );
  }
  // The same steps alone: the per-frame kernels compiled again (warm), the body's atmosphere made
  // over the tables, and its first and second frames with their composites.
  const suns =
    frames.combinations.find(
      ({ item }) =>
        item.setting === "high" &&
        item.medium.id === measured.id &&
        item.place === place.label &&
        item.set.name === set.name,
    )?.item.listed?.kept ?? null;
  const aspect = viewport.widthPx / viewport.heightPx;
  let alone = "no sources from (A) for the measured Earth's 400 km x1";
  let kernelsMs = Number.NaN;
  const releases = new Releases();
  try {
    const kernelsStart = performance.now();
    const kernels = releases.hold(await createAtmosphereFrameKernels(engine), (made) => {
      engine.releaseMaterial(made.composite);
      engine.releaseMesh(made.triangle);
    });
    kernelsMs = performance.now() - kernelsStart;
    const scene = releases.hold(
      timingTarget(engine, `${TASK} first air scene`, viewport, true),
      releaseTarget,
    );
    const output = releases.hold(
      timingTarget(engine, `${TASK} first air output`, viewport, false),
      releaseTarget,
    );
    scene.render(frameOf(`${TASK} first air scene`, [], aspect));
    const sceneInput = {
      colour: scene.colour,
      depth: requireDepth(scene.depth),
      nearM: NEAR_M,
      exposureScale: SMOKE_EXPOSURE_SCALE,
    };
    const { item } = frameCombination("high", measured, place, set, failures);
    if (suns !== null) {
      const overStart = performance.now();
      const atmosphere = releases.hold(
        HillaireAtmosphere.over(engine, measured.tables, TABLE_SIZES.high, kernels),
        (made) => {
          made.dispose();
        },
      );
      const overMs = performance.now() - overStart;
      const drawMs = [0, 1].map(() =>
        timedMs(() => {
          const composite = atmosphere.drawFrame(item.scene.air, suns, sceneInput);
          output.render(frameOf(ATMOSPHERE_COMPOSITE_PASS, [composite], aspect));
        }),
      );
      alone = `over ${overMs.toFixed(2)} ms, the first drawFrame and its composite ${drawMs[0]?.toFixed(2) ?? "?"} ms, the second ${drawMs[1]?.toFixed(2) ?? "?"} ms`;
    }
    held &&= suns !== null;
    await engine.readTexture(output.colour, 0, { x: 0, y: 0, width: 1, height: 1 });
  } finally {
    releases.run();
  }
  checks.check(
    CHECK_NAMES.firstAir,
    held && failures.length === 0,
    [
      `the measured Earth from 400 km, x1, each time a new body over the same tables on (A)'s renderer, whose render makes its atmosphere (HillaireAtmosphere.over) and draws its first frame, then ${STEADY_FRAMES} steady frames`,
      samples.join("; "),
      `alone: createAtmosphereFrameKernels ${kernelsMs.toFixed(1)} ms (warm: R08.T8.c's timings and the renderers before compiled them), ${alone}`,
      `prepare: (A)'s renderer ${prepares.frameMs.toFixed(1)} ms (after R08.T8.c's timings compiled the per-frame kernels: warm), (E)'s ${prepares.bytes.join(", ")}`,
      ...(failures.length === 0 ? [] : [`air failures: ${failures.slice(0, 5).join("; ")}`]),
    ].join("; "),
  );
}

// --- The group -----------------------------------------------------------------------------------

/**
 * The group's summary checks, which fail together without `timestamp-query`: each setting's and
 * medium's table of (A), and (B), (C), (G), (D), (E) and (F)'s, and the dropped frames', by the
 * full run's own templates ({@link CHECK_NAMES}).
 */
function summaryCheckNames(): string[] {
  const frameTables = (["high", "low"] as const).flatMap((setting) =>
    FRAME_MEDIA.map((spec) => CHECK_NAMES.frameTable(setting, spec.name)),
  );
  return [
    ...frameTables,
    CHECK_NAMES.alone,
    CHECK_NAMES.small,
    CHECK_NAMES.trio,
    CHECK_NAMES.bare(MEASURED_EARTH_NAME),
    CHECK_NAMES.frameTable("high", CLASS_MAP_NAME),
    CHECK_NAMES.frameTable("high", PALETTE_NAME),
    CHECK_NAMES.frameTable("high", SATURN_NAME),
    CHECK_NAMES.frameTable("low", SATURN_NAME),
    CHECK_NAMES.tables,
    CHECK_NAMES.bytes,
    CHECK_NAMES.firstAir,
    CHECK_NAMES.dropped,
  ];
}

/**
 * R08.T11's photorealistic frame's timings (the module's remarks): (D) first, so that its cold
 * builds are as cold as the page leaves them, then (A), (B) on (A)'s sources, (C), (G), R08.T11.c's
 * (H), (I) and (J), then (E) and (F);
 * a check a table, one per combination with its per-frame sums, and one that no timed frame was
 * dropped. Without `timestamp-query` each check fails with the reason.
 */
export async function timeAtmosphereFrame(engine: RenderEngine, checks: Checks): Promise<void> {
  if (!engine.capabilities.timestampQuery) {
    for (const name of summaryCheckNames()) {
      checks.check(name, false, "no timestamp-query, so no pass times");
    }
    return;
  }
  const wall: string[] = [];
  const timedPart = async <T>(name: string, work: () => Promise<T>): Promise<T> => {
    const start = performance.now();
    try {
      return await work();
    } finally {
      wall.push(`${name} ${((performance.now() - start) / 1_000).toFixed(1)} s`);
    }
  };
  const releases = new Releases();
  try {
    const ledger = releases.hold(new ResolveLedger(engine), (made) => {
      made.dispose();
    });
    await timedPart("(D)", () => timeTables(engine, checks, ledger));
    const media = frameMedia(engine, releases);
    const measured = media.find((medium) => medium.name === MEASURED_EARTH_NAME);
    if (measured === undefined) {
      throw new Error("the measured Earth is among (A)'s media");
    }
    // (I)'s and (J)'s media before the renderer, so that they outlive it in `releases`.
    const specs = classMapSpecs();
    const mapped = specs.map((spec) => classMapMedium(engine, releases, measured, spec));
    const saturn = await saturnMedium(engine, releases);
    const firstPlace = FAR_LIMB_TIMING_PLACES[0];
    const firstSet = sourceSetsOf("high")[0];
    if (firstPlace === undefined || firstSet === undefined) {
      throw new Error("the far limb's cameras and sets are not empty");
    }
    const { on, prepareMs } = await heldFrameView(
      releases,
      engine,
      `${TASK} frame`,
      "high",
      farLimbFrameScene(firstPlace.distanceM, settingOutput("high"), firstSet).camera,
    );
    const frames = await timedPart("(A)", () => timeFrames(on, ledger, media));
    reportFrames(checks, frames, () => null);
    const aloneMissing = await timedPart("(B)", () =>
      timeAlone(engine, checks, ledger, media, frames),
    );
    const smallMissing = await timedPart("(C)", () =>
      timeSmallBodies(on, checks, ledger, measured),
    );
    const bareMissing = await timedPart("(G)", () => timeBare(on, checks, ledger, measured));
    const trioMissing = await timedPart("(H)", () => timeSmallTrio(on, checks, ledger, measured));
    const mappedFrames = await timedPart("(I)", () => timeClassMap(on, ledger, mapped));
    const specAbout = new Map(specs.map((spec) => [spec.name, spec.about]));
    reportFrames(
      checks,
      mappedFrames,
      (medium) =>
        `${MEASURED_EARTH_NAME}'s body under ${specAbout.get(medium.name) ?? "a class map"}, under its own id, over ${MEASURED_EARTH_NAME}'s tables`,
    );
    const saturnFrames = await timedPart("(J)", () => timeSaturn(on, ledger, saturn));
    const ratio = saturn.figure.equatorialRadiusM / SLICES_EARTH.figure.equatorialRadiusM;
    reportFrames(
      checks,
      saturnFrames,
      () =>
        `${SATURN_NAME}: ATMOSPHERE TEST's saturn-oblate on Saturn's spheroid, ${saturn.tables.plan.kappa.length} slices and ${saturn.tables.plan.bands.length} bands, its body a grey Lambert sphere of its ground albedo; (A)'s five cameras with every distance scaled by its equatorial radius over WGS 84's (${ratio.toFixed(4)}), so that its disc and limb fill the frame as the Earth's do; the setting's sets as the Earth's, its giants placed as the Earth's but ${(SATURN_GIANT_DISTANCE_M / GIANT_EQUATORIAL_RADIUS_M).toFixed(0)} of their radii off (${SATURN_GIANT_DISTANCE_M.toExponential(2)} m), beyond its farthest camera, so that their planetshine stays past the sky's negligible skip at every camera`,
    );
    const frameMissing = missingOf(frames);
    const mappedMissing = missingOf(mappedFrames);
    const saturnMissing = missingOf(saturnFrames);
    const missing =
      frameMissing +
      aloneMissing +
      smallMissing +
      bareMissing +
      trioMissing +
      mappedMissing +
      saturnMissing;
    checks.check(
      CHECK_NAMES.dropped,
      missing === 0,
      `${missing} timed frames had a resolve missing or none: (A) ${frameMissing}, (B) ${aloneMissing}, (C) ${smallMissing}, (G) ${bareMissing}, (H) ${trioMissing}, (I) ${mappedMissing}, (J) ${saturnMissing}`,
    );
    const bytes = await timedPart("(E)", () => perViewBytes(engine, checks, measured));
    await timedPart("(F)", () =>
      firstAirFrame(engine, checks, on, ledger, measured, frames, { frameMs: prepareMs, bytes }),
    );
  } finally {
    // The renderer and its view, then (J)'s tables and (I)'s textures, then (A)'s tables, then the
    // ledger.
    releases.run();
    checks.check(CHECK_NAMES.wall, true, wall.join(", "));
  }
}
