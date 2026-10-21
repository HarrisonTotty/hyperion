/**
 * The descent spike's run (plan R05, T13.b): the terrain, the atmosphere and the lit view of the
 * full-window view, and the two wireframe instruments, drawn every frame at the script's time.
 *
 * @remarks
 * `prepareDescent` measures what the script needs of the terrain once, before the run (the
 * orchestrator's ruling, 2026-10-03), and `SpikeRun` draws. The frames sample the script at their
 * own display times, so the path is the same every run while the frame rate is free (Design note
 * 19); at the script's end the run holds the last pose.
 *
 * Spike runs only record `performance.measure` spans: {@link FRAME_MEASURE} about each frame's
 * whole callback, `terrain.select` about each selection (the pass's `measureSelection`),
 * `terrain.frame` about each frame's terrain work, and one `spike.segment:<name>` span a segment
 * once it ends. Each span is an entry the browser keeps, measurement overhead the ordinary view
 * never pays, apart from the frame's own, which is cleared as soon as it is made.
 *
 * Over a generated world (plan R10, R10.T14.c; decision-r10-t14c-leans §3; {@link SpikeBodyOptions})
 * the main view is `VIEW`'s own frame drawer over the terrain session instead
 * (`spikeSessionView.ts`), stepped as `VIEW`'s primary steps it, and there is no atmosphere: the
 * body is airless. Its terrain work, the `terrain.frame` span, is the session's step; the session's
 * passes record no `terrain.select` spans. The instruments are the same on either world.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { PHOTOREAL_NOT_CREATED } from "../../displays/view/styleRefusals";
import { browserHeightWorker, stepTerrainSession } from "../../displays/view/useTerrainSession";
import type { ViewRun } from "../../displays/view/viewRun";
import type { Vec3 } from "../../geometry/vec3";
import type { ElementSize } from "../../lib/useElementSize";
import type { ColourTokens } from "../../spatial/paint";
import type { CameraPose } from "../camera/pose";
import {
  DEFAULT_FOV_DEG,
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  viewRotation4,
} from "../camera/projection";
import {
  type CameraState,
  type CameraTarget,
  newCameraState,
  presetPose,
  type RenderStyle,
  viewId,
} from "../camera/state";
import type { RenderEngine, RenderView, ViewSize } from "../engine/types";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  referenceSunIlluminanceLx,
} from "../atmosphere/earth";
import { HillaireAtmosphere, TABLE_SIZES } from "../atmosphere/hillaire";
import { controlEv100 } from "../photometry/exposure";
import {
  type QualitySetting,
  TERRAIN_SETTINGS,
  terrainSettingsFor,
  type TerrainVariant,
} from "../quality/qualitySetting";
import { cameraSceneOf, type ViewScene } from "../scene/model";
import type { KeptScene } from "../scenes/kept";
import type { TerrainAnnunciation } from "../terrain/annunciation";
import { TerrainFailureGuard } from "../terrain/failureGuard";
import type { FrameTerrainPass } from "../terrain/frameTerrain";
import { finestPatchSizeM, heldGroundUnder } from "../terrain/grounded";
import { type PatchKey, patchKeyString } from "../terrain/patchKey";
import { type PlanetGeometry, planetGeometry } from "../terrain/planet";
import type { PatchRequest, Selection, SelectionInput } from "../terrain/select";
import { sceneContacts, type TerrainSession } from "../terrain/session";
import {
  TerrainPass,
  type TerrainPassObserver,
  type TerrainPool,
  type TerrainPoolFactory,
} from "../terrain/terrainPass";
import type { BakedPatch, TestPlanetRidges } from "../terrain/workers/messages";
import type { WorkerLike } from "../terrain/workers/pool";
import {
  buildWireframeDrawList,
  type DrawnSurveyEdge,
  type ViewStrokes,
} from "../wireframe/drawList";
import { WireframeRenderer } from "../wireframe/submit";
import { stretchKeys } from "./demandRecord";
import {
  DescentProfile,
  type DescentPose,
  DescentUnclearable,
  landingSiteOf,
  type TrackStretch,
  trackStretches,
} from "./descentProfile";
import { LitView, spikeExposure } from "./litView";
import { SEGMENT_MEASURE_PREFIX } from "./metrics";
import { testPlanetRotationAt } from "./rotation";
import {
  contactRule,
  type ContactRule,
  orbitInstrumentPose,
  postSyntheticField,
  siteDirection,
  spikeCameraAt,
  SPIKE_CRAFT,
  spikeContactAt,
  spikeScene,
  spikeSunState,
  sunDirectionBody,
  TEST_PLANET_FIGURE,
} from "./spikeScene";
import { SpikeSessionView, type SpikeViewSession } from "./spikeSessionView";
import { type SpikeWorld, TEST_PLANET_WORLD } from "./spikeWorld";
import type { SurfaceQuery, SurfaceQueryWorker } from "./surfaceQuery";
import { defaultHeightWorkers, HeightWorkerPool } from "../terrain/workers/pool";

/** The full-window view's horizontal field of view, rad: R02's default 60°. */
export const SPIKE_FOV_X_RAD = (DEFAULT_FOV_DEG * Math.PI) / 180;

/**
 * The instruments' horizontal field of view, rad: R02's default, which their label blocks read
 * from their cameras' `fovDeg`.
 */
const INSTRUMENT_FOV_X_RAD = (DEFAULT_FOV_DEG * Math.PI) / 180;

/**
 * The spike's Sun at its planet per channel, lx: R06's Sun at 1 au through R07's
 * `starIlluminance` (plan R08, R08.T6.d), in place of R05's `solar.ts` factors.
 */
const SPIKE_SUN_ILLUMINANCE_LX = referenceSunIlluminanceLx();

/** The `performance.measure` name of one frame's terrain work. */
export const TERRAIN_FRAME_MEASURE = "terrain.frame";

/**
 * The `performance.measure` name of one frame's whole callback (R05.T14.g,
 * decision-r05-trace-windows-2.md): the span of {@link SpikeFrameSample.callbackMs}, between the
 * same two `performance.now()` readings.
 *
 * @remarks
 * The trace's split counts "our code" as the union of these spans, and the main process checks
 * each window's spans against the report's `ourCodeMs`, frame by frame. The main process keeps its
 * own copy of the literal (`reduceTrace.ts`), since the two tsconfig projects cannot share it; a
 * test on each side pins it.
 */
export const FRAME_MEASURE = "spike.frame";

/** The engine's names for the spike's three views. */
export const SPIKE_VIEW_NAMES = {
  main: "spike",
  orbit: "spike-orbit",
  craft: "spike-craft",
} as const;

/**
 * The main view's ID in the terrain session over a generated world (R10.T14.c): its reports', its
 * draws' and its faults', and the measurement's.
 */
export const SPIKE_MAIN_VIEW_ID = viewId(SPIKE_VIEW_NAMES.main);

/**
 * The default height-worker count (Design note 11): clamp(⌊threads ÷ 4⌋, 1, 3), the terrain's own
 * `defaultHeightWorkers` (`workers/pool.ts`, where R10.T16.a moved it for `VIEW`'s terrain session),
 * under the name the spike's launch reads.
 */
export function defaultSpikeWorkers(hardwareConcurrency: number): number {
  return defaultHeightWorkers(hardwareConcurrency);
}

/** The spike's own workers and pool: the browser's by default, fakes in a test. */
export interface SpikeWorkers {
  /** Starts the surface query's worker. */
  readonly query: () => SurfaceQueryWorker;
  /** Makes the height-worker pool of `workers` workers, holding the synthetic field. */
  readonly pool: (workers: number) => TerrainPoolFactory;
  /**
   * Starts one height worker of the terrain session over a generated world (R10.T14.c;
   * `TerrainSessionInput.createWorker`), stable in identity.
   */
  readonly height: () => WorkerLike;
}

/** The browser's: module workers on the renderer's own files (R04.T10.c). */
export const DEFAULT_SPIKE_WORKERS: SpikeWorkers = {
  height: browserHeightWorker,
  query: () => new Worker(new URL("./surfaceQuery.worker.ts", import.meta.url), { type: "module" }),
  pool: (workers) => (bake) => {
    const pool = new HeightWorkerPool({
      workers,
      bake,
      createWorker: () =>
        new Worker(new URL("../terrain/workers/height.worker.ts", import.meta.url), {
          type: "module",
        }),
    });
    postSyntheticField(pool);
    return pool;
  },
};

/** The descent, measured against the terrain before the run. */
export interface PreparedDescent {
  readonly planet: PlanetGeometry;
  readonly profile: DescentProfile;
  /** When the craft is a contact (Design note 9, held from its last descent). */
  readonly contact: ContactRule;
  /** The landing site's terrain height, metres above the datum (T4.c's interpolant). */
  readonly siteHeightM: number;
  /** The track's stretches (T13.a's plan, the same for every terrain). */
  readonly stretches: ReadonlyArray<TrackStretch>;
  /** F_k for each stretch: a true upper bound of the finest mesh under it, metres above the datum. */
  readonly stretchMaxHeightsM: ReadonlyArray<number>;
  /** The low pass's floor, metres above the datum, for the readout. */
  readonly trackMaxHeightM: number;
  /** σ_n for levels 0 to 24, metres (T6), for T13.c's min(hard, 4σ_n) pass. */
  readonly omittedSigmaM: Float64Array;
  /**
   * The terrain readout's answer at the landing site over a generated world, for `SITE ELEVATION`
   * (R10.T14.c; decision-r10-t14c-leans §4), or `null` where it gave none (no detail seed or field
   * held, or no worker left); absent over the test planet, whose site height is resolved.
   */
  readonly siteReading?: SiteReading | null;
}

/**
 * The workers' answer of the ground at a generated world's landing site (R10.T14.c), by the terrain
 * readout's kind down a vertical line of sight onto the site: the ground's `reading` at its foot
 * (`surveyed_reading`'s), which `SITE ELEVATION` states in `ELEVATION`'s form
 * (`formatElevationReading`); or ground the ship has not surveyed, or no ground met, which it
 * states as the readout's own `NOT SURVEYED` and `NO GROUND`.
 */
export type SiteReading =
  | {
      readonly kind: "ground";
      /** The survey's elevation there, metres above the datum. */
      readonly elevationM: number;
      /** Its standard uncertainty, metres; 0 where resolved. */
      readonly sigmaM: number;
      /** Whether the survey reaches the band limit there. */
      readonly resolved: boolean;
    }
  /** The line of sight meets ground the ship has not surveyed, or no field is held there. */
  | { readonly kind: "not-surveyed" }
  /** The line of sight meets no ground within its reach. */
  | { readonly kind: "no-ground" };

/**
 * The script cannot be flown over the measured terrain: T13.a's profile found a floor it cannot
 * clear (decision-r05-descent-clearance.md, rule 4), so the runner refuses to fly.
 */
export class DescentRefused extends Error {
  constructor(seed: bigint, cause: unknown) {
    super(`the descent of seed ${seed.toString()} cannot clear its terrain`, { cause });
    this.name = "DescentRefused";
  }
}

/**
 * Measures the descent of `seed` against the test planet (decision-r05-descent-clearance.md): its
 * level table, the landing site's height along the site's direction d, and a floor under each of
 * T13.a's stretches, the highest baked vertex plus ε_n over the stretch's patches and their
 * neighbours (`stretchKeys`), in one batch; then the profile flown over them.
 *
 * @throws {@link DescentRefused} if the profile cannot clear the floors.
 */
export async function prepareDescent(query: SurfaceQuery, seed: bigint): Promise<PreparedDescent> {
  const planet = planetGeometry(TEST_PLANET_FIGURE, await query.levelTable());
  const site = landingSiteOf(seed);
  // The ground track and the stretches do not depend on the terrain: the bare datum's profile
  // gives them.
  const datum = new DescentProfile(TEST_PLANET_FIGURE, site);
  const stretches = trackStretches(datum);
  const [siteHeightM, floors, omittedSigmaM] = await Promise.all([
    query.heightM(siteDirection(datum)),
    query.maxHeightsM(stretches.map((stretch) => stretchKeys(datum, stretch))),
    query.omittedSigmaM(),
  ]);
  const stretchMaxHeightsM = Array.from(floors);
  if (
    stretchMaxHeightsM.length !== stretches.length ||
    !stretchMaxHeightsM.every(Number.isFinite)
  ) {
    // A measurement fault, not a refusal: the query must answer one finite floor a stretch.
    throw new Error(
      `the surface query answered ${stretchMaxHeightsM.length} floors for ${stretches.length} stretches, or a floor not finite`,
    );
  }
  let profile: DescentProfile;
  try {
    profile = new DescentProfile(TEST_PLANET_FIGURE, site, { siteHeightM, stretchMaxHeightsM });
  } catch (error: unknown) {
    throw error instanceof DescentUnclearable ? new DescentRefused(seed, error) : error;
  }
  const lowPass = stretches.findIndex((stretch) => stretch.segment === "low fast pass");
  return {
    planet,
    profile,
    contact: contactRule(profile),
    siteHeightM,
    stretches,
    stretchMaxHeightsM,
    trackMaxHeightM: stretchMaxHeightsM[lowPass] ?? siteHeightM,
    omittedSigmaM,
  };
}

/** A patch's progress, as T14's tallies count it. */
export type SpikePatchEvent = "requested" | "baked" | "resident";

/** What one frame tells T13.c's metrics (T14.a's `FrameSample`, less what T14 adds itself). */
export interface SpikeFrameSample {
  readonly scriptTimeS: number;
  readonly rafTimestampMs: number;
  /**
   * The first frame's `requestAnimationFrame` timestamp, ms: script time 0, from which
   * `scriptTimeS` counts, on `performance.now()`'s clock (T14.d).
   */
  readonly scriptStartMs: number;
  /**
   * The frame callback's start, `performance.now()` ms: the start its {@link FRAME_MEASURE} span
   * receives, which the trace carries as the span's `args.startTime` (R05.T14.h).
   */
  readonly callbackStartMs: number;
  /**
   * The frame callback's own time, ms: its {@link FRAME_MEASURE} span's duration, the same two
   * `performance.now()` readings.
   */
  readonly callbackMs: number;
  /**
   * Passes submitted this frame: over the test planet the terrain, the atmosphere, the display and
   * two instruments; `null` over a generated world, whose main view's drawer submits a number of
   * passes over many timer resolves that neither it nor the engine counts as they are made, so that
   * none is counted rather than one guessed.
   */
  readonly passesSubmitted: number | null;
  /**
   * The patches selected under the hard bound: over a generated world, those of the session's pass
   * in the style drawn, at its last step.
   */
  readonly patchesHard: number;
  /** Whether `TERRAIN: STREAMING` shows: over a generated world, the drawn frame's annunciation. */
  readonly streaming: boolean;
}

/** Listeners of the run, for T13.c's metrics. */
export interface SpikeListeners {
  readonly onFrame?: (sample: SpikeFrameSample) => void;
  /**
   * Each patch's progress. Over a generated world, from the session's pass in the style drawn
   * (R10.T14.c): `requested` the first time a key is in its selection's demand, read after each
   * step; `baked` and `resident`, in that order, each time it makes a patch resident, since the
   * session's lane of its one pool, from which the bakes come, is not the spike's to listen to, so
   * that a bake the cache refuses is not counted.
   */
  readonly onPatch?: (event: SpikePatchEvent, key: PatchKey) => void;
  /** Called once the terrain is measured and the descent prepared, before the run flies. */
  readonly onPrepared?: (prepared: PreparedDescent) => void;
  /**
   * Each selection's input and result (TerrainPass's `onSelect`), for T13.c's second pass; over a
   * generated world each of the session's pass in the style drawn (`TerrainPass.observe`), from its
   * second on, since the session makes and steps a pass in one step, before the run can observe it.
   */
  readonly onSelect?: (input: SelectionInput, selection: Selection) => void;
  /**
   * The run cannot go on, with the status or fault the view shows: the descent refused, the terrain
   * not measured, or the views not made (T13.c ends the run on it); over a generated world also the
   * photorealistic style refused, or the terrain's fault in flight, of either cause, in the spike's
   * words (R10.T14.c; decision-r10-t14c-guide Q2). Called once a run.
   */
  readonly onFailed?: (status: string) => void;
}

/** The canvases' sizes and what the instruments are drawn with. */
export interface SpikeFrameInput {
  /** The `requestAnimationFrame` timestamp, ms. */
  readonly nowMs: number;
  readonly sizes: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, ViewSize>>;
  readonly tokens: ColourTokens;
  /** One rem in device pixels. */
  readonly remPx: number;
  /** How the instruments draw their strokes: `viewStrokesAt` the display's ratio (R07.T16.d). */
  readonly strokes: ViewStrokes;
  /** The target selected in the list, which the instruments mark with the bracket reticle. */
  readonly selection: CameraTarget | null;
  /**
   * The main view's stage as laid out, which `VIEW`'s drawer presents its frames at over a
   * generated world; the test planet's lit view takes `sizes.main` and ignores it. Absent, the
   * drawer draws nothing.
   */
  readonly mainStage?: ElementSize;
}

/**
 * The terrain's state in one frame, for the console panel: over a generated world, the session's
 * pass in the style drawn at its last step, its counts 0 while it has none.
 */
export interface SpikeTerrainReading {
  readonly selected: number;
  readonly drawn: number;
  readonly standingIn: number;
  readonly missing: number;
  readonly limited: boolean;
  readonly annunciation: TerrainAnnunciation | null;
}

/** What one frame drew, for the DOM. */
export interface SpikeFrame {
  readonly tS: number;
  readonly pose: DescentPose;
  /**
   * Whether the craft is a contact, the finest terrain held about it: over the test planet by its
   * contact rule; over a generated world whether it is one of the session's pass's contacts, by the
   * session's own functions (`sceneContacts` over the pass's held ground), and not while no pass is.
   */
  readonly contact: boolean;
  readonly scene: ViewScene;
  /** The cameras of the full-window view and the two instruments. */
  readonly cameras: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, CameraState>>;
  readonly terrain: SpikeTerrainReading;
  readonly ev100: number;
  /**
   * The style the main view was drawn in over a generated world (`ViewFrameDrawn.drawnStyle`, the
   * wireframe while the photorealistic pipelines compile, and before its stage is laid out); `null`
   * over the test planet, whose main view is the spike's own lit view.
   */
  readonly drawnStyle: RenderStyle | null;
  /**
   * The contour interval the main view drew its wireframe terrain at over a generated world, m
   * (`ViewFrameDrawn.contourIntervalM`), for its label block's `CONTOUR INTERVAL`; `null` while it
   * draws none, and over the test planet.
   */
  readonly contourIntervalM: number | null;
  /**
   * The survey edges the main view marked over a generated world (`ViewFrameDrawn.surveyEdges`),
   * for its list's `SURVEY EDGE` notes; none over the test planet.
   */
  readonly surveyEdges: ReadonlyArray<DrawnSurveyEdge>;
  /**
   * Why the run cannot go on, the fault its main view's label block states, or `null`: over a
   * generated world `GRAPHICS STYLE REFUSED` (`PHOTOREAL_NOT_CREATED`) once the photorealistic
   * pipelines of `--style photorealistic` could not be made, when the frame draws nothing, so that
   * no run measures the wireframe in their place.
   */
  readonly refusal: string | null;
}

/** No survey edge marked. */
const NO_SURVEY_EDGES: ReadonlyArray<DrawnSurveyEdge> = [];

/** The terrain's state of a frame that drew none: every count 0, no annunciation. */
const NO_TERRAIN_READING: SpikeTerrainReading = {
  selected: 0,
  drawn: 0,
  standingIn: 0,
  missing: 0,
  limited: false,
  annunciation: null,
};

/** What the run over a generated world reads of a terrain session's pass. */
export interface SpikeTerrainPass
  extends FrameTerrainPass, Pick<TerrainPass, "lastStep" | "planet" | "observe"> {}

/**
 * What the run over a generated world reads of its terrain session (`TerrainSession`, or a test's
 * stand-in): its main view's, its step, and its passes.
 */
export interface SpikeTerrainSession
  extends SpikeViewSession, Pick<TerrainSession, "step" | "failComputation"> {
  pass(body: BodyIdHex): SpikeTerrainPass | null;
  wirePass(body: BodyIdHex): SpikeTerrainPass | null;
}

/**
 * The run over a generated world (R10.T14.c): the world flown, the terrain session its main view
 * draws through, which the caller holds (`DescentSpike`'s, through `useTerrainSession`) and the run
 * never disposes, and the style the main view draws in (`--style`).
 */
export interface SpikeBodyOptions {
  readonly world: SpikeWorld;
  readonly session: SpikeTerrainSession;
  readonly style: RenderStyle;
}

/** The test planet's main view: R05's terrain pass, its lit view and its atmosphere, sunlit. */
interface TestPlanetMain {
  readonly kind: "test-planet";
  readonly terrain: TerrainPass;
  readonly lit: LitView;
  readonly atmosphere: HillaireAtmosphere;
  readonly sunBody: Vec3;
}

/** A generated world's main view: `VIEW`'s drawer over the session, and how the run follows it. */
interface BodyMain {
  readonly kind: "body";
  readonly world: SpikeWorld;
  readonly session: SpikeTerrainSession;
  readonly style: RenderStyle;
  readonly setting: QualitySetting;
  readonly view: SpikeSessionView;
  /** The guard of the session's step, one for the run's one session (R10.T16.e). */
  readonly guard: TerrainFailureGuard;
  readonly passes: PassFollower;
  /** The instruments' EV100: the spike's fixed day exposure's. */
  readonly ev100: number;
}

/**
 * The run's listeners on the session's pass in the style drawn (R10.T14.c): its observer on the
 * pass while it is the one, and the demand's keys already told.
 */
class PassFollower {
  readonly #listeners: SpikeListeners;
  #pass: SpikeTerrainPass | null = null;
  #unobserve: (() => void) | null = null;
  /** The selection whose demand was last read, which is read again only once it changes. */
  #selection: Selection | null = null;
  readonly #requested = new Set<string>();

  constructor(listeners: SpikeListeners) {
    this.#listeners = listeners;
  }

  /**
   * Follows `pass`, the session's pass in the style this frame drew, after the session's step:
   * observes it once it appears and again whenever it is another object, and tells of each key of
   * its last selection's demand not told before.
   */
  follow(pass: SpikeTerrainPass | null): void {
    if (pass !== this.#pass) {
      this.#unobserve?.();
      this.#unobserve = null;
      this.#pass = pass;
      const observer = this.#observer();
      if (pass !== null && observer !== null) {
        this.#unobserve = pass.observe(observer);
      }
    }
    const onPatch = this.#listeners.onPatch;
    const selection = pass?.lastStep?.selection ?? null;
    if (onPatch === undefined || selection === null || selection === this.#selection) {
      return;
    }
    this.#selection = selection;
    for (const request of selection.demand) {
      const keyString = patchKeyString(request.key);
      if (!this.#requested.has(keyString)) {
        this.#requested.add(keyString);
        onPatch("requested", request.key);
      }
    }
  }

  /** Stops observing the pass it follows. */
  dispose(): void {
    this.#unobserve?.();
    this.#unobserve = null;
    this.#pass = null;
  }

  /** The observer of the listeners' seams, or `null` where they have none. */
  #observer(): TerrainPassObserver | null {
    const { onSelect, onPatch } = this.#listeners;
    if (onSelect === undefined && onPatch === undefined) {
      return null;
    }
    return {
      ...(onSelect === undefined ? {} : { onSelect }),
      ...(onPatch === undefined
        ? {}
        : {
            onResident: (key: PatchKey) => {
              onPatch("baked", key);
              onPatch("resident", key);
            },
          }),
    };
  }
}

/** A pool that tells of each request and bake. */
function listenedPool(pool: TerrainPool, listeners: SpikeListeners): TerrainPool {
  const onPatch = listeners.onPatch;
  if (onPatch === undefined) {
    return pool;
  }
  const requested = new Set<string>();
  pool.onBaked((bake: BakedPatch) => {
    onPatch("baked", bake.key);
  });
  return {
    reprioritise: (demand: ReadonlyArray<PatchRequest>) => {
      for (const request of demand) {
        const { face, level, i, j } = request.key;
        const key = `${face}/${level}/${i}/${j}`;
        if (!requested.has(key)) {
          requested.add(key);
          onPatch("requested", request.key);
        }
      }
      pool.reprioritise(demand);
    },
    onBaked: (cb) => pool.onBaked(cb),
    terminate: () => {
      pool.terminate();
    },
  };
}

/**
 * A camera state holding `pose` as a free camera, for the label block and the lists, which read
 * only its preset, pose and field of view; the script, not the flight keys, moves it.
 */
function heldCamera(scene: ViewScene, pose: CameraPose): CameraState {
  return { ...newCameraState(cameraSceneOf(scene), "camera"), preset: "free", pose };
}

function sameSize(a: ViewSize | null, b: ViewSize): boolean {
  return a !== null && a.widthPx === b.widthPx && a.heightPx === b.heightPx;
}

/**
 * Whether the scene's craft is a contact of `pass`, the session's pass of `body` (R10.T14.c): by
 * the session's own rule, `sceneContacts` over the ground the pass holds (`heldGroundUnder` of its
 * resident heights) at its finest patch, as its step judges them; never while no pass is.
 */
function passContact(scene: ViewScene, body: BodyIdHex, pass: SpikeTerrainPass | null): boolean {
  const seen = scene.bodies.find((each) => each.id === body);
  if (pass === null || seen === undefined) {
    return false;
  }
  const ground = heldGroundUnder(pass.planet.figure, pass.heights);
  return sceneContacts(scene, seen, ground, finestPatchSizeM(pass.planet)).length > 0;
}

/** The spike's drawing, on one engine, into its three views. */
export class SpikeRun {
  readonly #prepared: PreparedDescent;
  /** The world flown: the test planet's unless the run is over a generated world. */
  readonly #world: SpikeWorld;
  readonly #kept: KeptScene;
  readonly #listeners: SpikeListeners;
  readonly #views: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, RenderView>>;
  readonly #sizes: Record<keyof typeof SPIKE_VIEW_NAMES, ViewSize | null> = {
    main: null,
    orbit: null,
    craft: null,
  };
  /** The main view's own: the test planet's lit view, or `VIEW`'s drawer over a generated world. */
  readonly #main: TestPlanetMain | BodyMain;
  readonly #instruments: Readonly<Record<"orbit" | "craft", WireframeRenderer>>;
  #startMs: number | null = null;
  #segment: { readonly name: string; readonly startMs: number } | null = null;
  /** Whether the script has reached its end, whose last segment's span is then closed. */
  #ended = false;
  #lastNowMs: number | null = null;

  /**
   * Makes the run's views on `canvases` and its passes: over the test planet R05's terrain pass, lit
   * view and atmosphere; over a generated world (`options.body`) `VIEW`'s frame drawer on the main
   * view, which reads the session's passes and makes none of its own, so that `ridges`, `createPool`
   * and `variant` go unused.
   *
   * @throws Error if the engine cannot make a view on a canvas (no context).
   */
  constructor(
    engine: RenderEngine,
    prepared: PreparedDescent,
    options: {
      readonly setting: QualitySetting;
      readonly ridges: TestPlanetRidges;
      readonly createPool: TerrainPoolFactory;
      /** The terrain variant of `--vertex-path` and `--normals` (T13.c); the setting's own if absent. */
      readonly variant?: TerrainVariant;
      /** The run over a generated world (R10.T14.c); absent, the test planet's. */
      readonly body?: SpikeBodyOptions;
    },
    canvases: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, HTMLCanvasElement>>,
    listeners: SpikeListeners = {},
  ) {
    this.#prepared = prepared;
    const body = options.body;
    this.#world = body?.world ?? TEST_PLANET_WORLD;
    this.#kept = spikeScene(prepared.profile, this.#world);
    this.#listeners = listeners;
    // What is made is released again, in reverse, if a later part cannot be made.
    const undo: (() => void)[] = [];
    // The main view's release: its own, until a drawer that disposes of it with itself is made.
    const mainRelease = { current: (): void => undefined };
    try {
      const view = (key: keyof typeof SPIKE_VIEW_NAMES): RenderView => {
        const made = engine.createView(canvases[key], SPIKE_VIEW_NAMES[key]);
        const release = (): void => {
          made.dispose();
        };
        if (key === "main") {
          mainRelease.current = release;
          undo.push(() => {
            mainRelease.current();
          });
        } else {
          undo.push(release);
        }
        return made;
      };
      this.#views = { main: view("main"), orbit: view("orbit"), craft: view("craft") };
      if (body === undefined) {
        this.#main = SpikeRun.#testPlanetMain(engine, prepared, options, listeners, undo);
      } else {
        const main = SpikeRun.#bodyMain(engine, this.#views.main, body, options.setting, listeners);
        mainRelease.current = () => {
          main.view.dispose();
        };
        this.#main = main;
      }
      const orbit = new WireframeRenderer(engine);
      undo.push(() => {
        orbit.dispose();
      });
      this.#instruments = { orbit, craft: new WireframeRenderer(engine) };
    } catch (error: unknown) {
      for (const release of undo.toReversed()) {
        release();
      }
      throw error;
    }
  }

  /** The test planet's main view's passes, each pushing its release onto `undo` once made. */
  static #testPlanetMain(
    engine: RenderEngine,
    prepared: PreparedDescent,
    options: {
      readonly setting: QualitySetting;
      readonly ridges: TestPlanetRidges;
      readonly createPool: TerrainPoolFactory;
      readonly variant?: TerrainVariant;
    },
    listeners: SpikeListeners,
    undo: (() => void)[],
  ): TestPlanetMain {
    const sunBody = sunDirectionBody(prepared.profile);
    const onPatch = listeners.onPatch;
    const terrain = new TerrainPass({
      engine,
      setting: options.setting,
      // The setting's own terrain, or the variant T17's runs ask for (decision-r05-spike-ux.md).
      terrain: terrainSettingsFor(options.setting, options.variant),
      planet: prepared.planet,
      source: { kind: "test-planet", ridges: options.ridges },
      createPool: (bake) => listenedPool(options.createPool(bake), listeners),
      measureSelection: true,
      ...(listeners.onSelect === undefined ? {} : { onSelect: listeners.onSelect }),
      ...(onPatch === undefined ? {} : { onResident: (key: PatchKey) => onPatch("resident", key) }),
    });
    undo.push(() => {
      terrain.dispose();
    });
    const lit = new LitView(
      engine,
      SPIKE_VIEW_NAMES.main,
      { widthPx: 1, heightPx: 1 },
      TERRAIN_SETTINGS[options.setting].renderHeightPx,
      true,
    );
    undo.push(() => {
      lit.dispose();
    });
    const atmosphere = new HillaireAtmosphere(
      engine,
      EARTH_REFERENCE,
      TABLE_SIZES[options.setting],
      TEST_PLANET_FIGURE,
      EARTH_REFERENCE_CURVES,
    );
    undo.push(() => {
      atmosphere.dispose();
    });
    return { kind: "test-planet", terrain, lit, atmosphere, sunBody };
  }

  /**
   * A generated world's main view: `VIEW`'s frame drawer on `mainView`, which disposes of the view
   * with itself, reporting to the session as `VIEW`'s primary does, with the guard of the session's
   * step and the pass's follower.
   */
  static #bodyMain(
    engine: RenderEngine,
    mainView: RenderView,
    body: SpikeBodyOptions,
    setting: QualitySetting,
    listeners: SpikeListeners,
  ): BodyMain {
    const view = new SpikeSessionView(engine, mainView, {
      id: SPIKE_MAIN_VIEW_ID,
      name: SPIKE_VIEW_NAMES.main,
      streamPriority: "primary",
      readoutBody: body.world.body,
    });
    return {
      kind: "body",
      world: body.world,
      session: body.session,
      style: body.style,
      setting,
      view,
      guard: new TerrainFailureGuard(),
      passes: new PassFollower(listeners),
      ev100: controlEv100(spikeExposure()),
    };
  }

  /**
   * Compiles the terrain's and the display's materials over the test planet; frames draw no terrain
   * until then.
   *
   * @remarks
   * Over a generated world it resolves at once: the drawer draws the wireframe stand-in while its
   * photorealistic pipelines compile, as `VIEW` does, and the session compiles its passes' materials
   * as it makes them. A frame then draws nothing and gives its refusal
   * ({@link SpikeFrame.refusal}) where `--style photorealistic`'s pipelines failed, so that no run
   * measures the wireframe in its place.
   */
  async ready(): Promise<void> {
    const main = this.#main;
    switch (main.kind) {
      case "test-planet":
        await Promise.all([main.terrain.ready(), main.lit.ready(["canvas"])]);
        break;
      case "body":
        break;
    }
  }

  /**
   * Draws one frame at the script's time for `input.nowMs`; over a generated world, where the main
   * view draws in the photorealistic style and its pipelines could not be made, draws nothing and
   * gives the style's refusal ({@link SpikeFrame.refusal}).
   *
   * @throws Error where the engine refuses a frame.
   */
  frame(input: SpikeFrameInput): SpikeFrame {
    const main = this.#main;
    let frame: SpikeFrame;
    switch (main.kind) {
      case "test-planet":
        frame = this.#testPlanetFrame(input, main);
        break;
      case "body":
        frame = this.#bodyFrame(input, main);
        break;
    }
    return frame;
  }

  /** The test planet's frame (T13.b): its terrain, atmosphere and lit view, and the instruments. */
  #testPlanetFrame(input: SpikeFrameInput, main: TestPlanetMain): SpikeFrame {
    const started = performance.now();
    this.#lastNowMs = input.nowMs;
    const startMs = this.#startMs ?? input.nowMs;
    this.#startMs = startMs;
    const { profile } = this.#prepared;
    const tS = Math.min(Math.max((input.nowMs - startMs) / 1000, 0), profile.durationS);
    this.#resize(input.sizes);
    const rotation = testPlanetRotationAt(tS);
    const pose = profile.poseAt(tS);
    this.#segmentSpan(tS < profile.durationS ? pose.segment : null, input.nowMs);
    const camera = spikeCameraAt(profile, tS);
    const contact = spikeContactAt(profile, this.#prepared.contact, tS);
    const sun = spikeSunState(main.sunBody, rotation, SPIKE_SUN_ILLUMINANCE_LX);
    const lit = main.lit;
    const terrainStart = performance.now();
    const frame = main.terrain.frame({
      view: {
        rotation,
        cameraM: camera.positionM,
        orientation: camera.orientation,
        fovXRad: SPIKE_FOV_X_RAD,
        viewport: lit.renderSize,
      },
      grounded: contact === null ? [] : [contact],
      sunDirectionBodyFixed: sun.directionBodyFixed,
      // The atmosphere's sun lights the terrain too, so that sky and ground share one light.
      sunIlluminanceLx: sun.illuminanceLx,
      exposureScale: lit.exposureScale,
      nowMs: input.nowMs,
    });
    performance.measure(TERRAIN_FRAME_MEASURE, { start: terrainStart, end: performance.now() });
    const render = lit.renderSize;
    lit.render(
      this.#views.main,
      viewRotation4(camera.orientation),
      perspectiveReversedInfinite(SPIKE_FOV_X_RAD, render.widthPx / render.heightPx, NEAR_PLANE_M),
      frame.draw === null ? [] : [frame.draw],
      (scene) =>
        main.atmosphere.drawFrame(
          {
            positionM: pose.positionM,
            orientation: pose.orientation,
            fovXRad: SPIKE_FOV_X_RAD,
            viewport: render,
          },
          [sun],
          {
            colour: scene.colour,
            depth: scene.depth,
            nearM: NEAR_PLANE_M,
            exposureScale: lit.exposureScale,
          },
        ),
    );
    const scene = this.#kept.sceneAt(tS);
    const cameras = this.#instrumentCameras(scene, camera, tS);
    const ev100 = controlEv100(lit.exposure);
    this.#drawInstruments(input, scene, cameras, ev100);
    const { drawSet, selection } = frame;
    const terrain: SpikeTerrainReading = {
      selected: selection.patches.size,
      drawn: drawSet.count,
      standingIn: drawSet.standingIn,
      missing: drawSet.missing,
      limited: selection.limited,
      annunciation: frame.annunciation,
    };
    const ended = performance.now();
    performance.measure(FRAME_MEASURE, { start: started, end: ended });
    // The trace has the span; the page keeps no entry of it.
    performance.clearMeasures(FRAME_MEASURE);
    this.#listeners.onFrame?.({
      scriptTimeS: tS,
      rafTimestampMs: input.nowMs,
      scriptStartMs: startMs,
      callbackStartMs: started,
      callbackMs: ended - started,
      // The terrain, the atmosphere's composite, the display and the two instruments.
      passesSubmitted: 5,
      patchesHard: selection.patches.size,
      streaming: frame.conditions.streaming,
    });
    return {
      tS,
      pose,
      contact: contact !== null,
      scene,
      cameras,
      terrain,
      ev100,
      drawnStyle: null,
      contourIntervalM: null,
      surveyEdges: NO_SURVEY_EDGES,
      refusal: null,
    };
  }

  /**
   * A generated world's frame (R10.T14.c; decision-r10-t14c-leans §3): the script's pose and the
   * kept scene at its time, then in `VIEW`'s primary's order the main view's report noted with the
   * session, the session stepped once under its guard (the frame's terrain work), and the main view
   * drawn by `VIEW`'s drawer; then the instruments, as over the test planet. The terrain's reading,
   * the contact and the metrics' sample are the session's pass's in the style drawn.
   */
  #bodyFrame(input: SpikeFrameInput, main: BodyMain): SpikeFrame {
    const started = performance.now();
    this.#lastNowMs = input.nowMs;
    const startMs = this.#startMs ?? input.nowMs;
    this.#startMs = startMs;
    const { profile } = this.#prepared;
    const { world, session } = main;
    const tS = Math.min(Math.max((input.nowMs - startMs) / 1000, 0), profile.durationS);
    // The drawer sizes the main view itself, from its stage.
    this.#resize(input.sizes, ["orbit", "craft"]);
    const pose = profile.poseAt(tS);
    this.#segmentSpan(tS < profile.durationS ? pose.segment : null, input.nowMs);
    const camera = spikeCameraAt(profile, tS, world);
    const scene = this.#kept.sceneAt(tS);
    const cameras = this.#instrumentCameras(scene, camera, tS);
    const run: ViewRun = {
      source: { kind: "kept", kept: this.#kept },
      tS,
      scene,
      camera: { ...cameras.main, style: main.style },
    };
    if (main.style === "photorealistic" && main.view.photorealStatus === "failed") {
      // The style's refusal is the run's end, which its holder states; nothing more is drawn.
      return {
        tS,
        pose,
        contact: false,
        scene,
        cameras: { ...cameras, main: run.camera },
        terrain: NO_TERRAIN_READING,
        ev100: main.ev100,
        drawnStyle: main.view.drawnStyle,
        contourIntervalM: null,
        surveyEdges: NO_SURVEY_EDGES,
        refusal: PHOTOREAL_NOT_CREATED,
      };
    }
    const inputs = main.view.inputs(session, {
      run,
      stage: input.mainStage ?? null,
      tokens: input.tokens,
      selection: input.selection,
      setting: main.setting,
      nowMs: input.nowMs,
    });
    // What it draws now, in the style it last drew in (S8), for the step below.
    main.view.note(session, inputs);
    const terrainStart = performance.now();
    // Guarded: no throw of the step stops the run, and one that lasts faults the terrain.
    stepTerrainSession(session, main.guard, input.nowMs);
    performance.measure(TERRAIN_FRAME_MEASURE, { start: terrainStart, end: performance.now() });
    const drawn = main.view.draw(inputs);
    const drawnStyle = main.view.drawnStyle;
    this.#drawInstruments(input, scene, cameras, main.ev100);
    const pass =
      drawnStyle === "photorealistic" ? session.pass(world.body) : session.wirePass(world.body);
    main.passes.follow(pass);
    const step = pass?.lastStep ?? null;
    const annunciation = drawn?.terrain ?? null;
    const terrain: SpikeTerrainReading = {
      selected: step?.selection.patches.size ?? 0,
      drawn: step?.drawSet.count ?? 0,
      standingIn: step?.drawSet.standingIn ?? 0,
      missing: step?.drawSet.missing ?? 0,
      limited: step?.selection.limited ?? false,
      annunciation,
    };
    const ended = performance.now();
    performance.measure(FRAME_MEASURE, { start: started, end: ended });
    // The trace has the span; the page keeps no entry of it.
    performance.clearMeasures(FRAME_MEASURE);
    this.#listeners.onFrame?.({
      scriptTimeS: tS,
      rafTimestampMs: input.nowMs,
      scriptStartMs: startMs,
      callbackStartMs: started,
      callbackMs: ended - started,
      passesSubmitted: null,
      patchesHard: terrain.selected,
      streaming: annunciation === "TERRAIN: STREAMING",
    });
    return {
      tS,
      pose,
      contact: passContact(scene, world.body, pass),
      scene,
      cameras: { ...cameras, main: run.camera },
      terrain,
      ev100: main.ev100,
      drawnStyle,
      contourIntervalM: drawn?.contourIntervalM ?? null,
      surveyEdges: drawn?.surveyEdges ?? NO_SURVEY_EDGES,
      refusal: null,
    };
  }

  /** Draws the two instruments' wireframes of `scene` from `cameras` at `ev100`. */
  #drawInstruments(
    input: SpikeFrameInput,
    scene: ViewScene,
    cameras: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, CameraState>>,
    ev100: number,
  ): void {
    for (const key of ["orbit", "craft"] as const) {
      const size = input.sizes[key];
      const drawCamera = { pose: cameras[key].pose, fovXRad: INSTRUMENT_FOV_X_RAD };
      // The orbit instrument draws the craft as a target, its mark at a fixed size: as the own
      // ship it would carry none, and its hull is far below a pixel seven planetary radii out.
      const drawn = key === "orbit" ? { ...scene, ownShip: null } : scene;
      const list = buildWireframeDrawList(drawn, drawCamera, size, input.tokens, {
        lowSetting: false,
        ev100,
        // The orbit marks the craft with the bracket reticle until another mark is chosen, so
        // that it reads against the graticule seven planetary radii away.
        selection:
          key === "orbit" && input.selection === null
            ? { kind: "craft", craft: SPIKE_CRAFT }
            : input.selection,
        destination: null,
        remPx: input.remPx,
        ...input.strokes,
      });
      this.#instruments[key].render(this.#views[key], list, drawCamera, size);
    }
  }

  /**
   * The harness's copy of a view's last frame, RGBA bytes sRGB-encoded (`RenderView.readBack`):
   * called in the same task as the {@link frame} that drew it.
   */
  readBack(view: keyof typeof SPIKE_VIEW_NAMES): Promise<Uint8Array | Float32Array> {
    return this.#views[view].readBack();
  }

  /**
   * Disposes the passes and the views; the engine frees the GPU objects with itself. Over a
   * generated world, the drawer with the main view, which the session forgets; the session itself
   * is its holder's.
   */
  dispose(): void {
    if (this.#lastNowMs !== null) {
      this.#segmentSpan(null, this.#lastNowMs);
    }
    const main = this.#main;
    switch (main.kind) {
      case "test-planet":
        main.terrain.dispose();
        main.lit.dispose();
        main.atmosphere.dispose();
        break;
      case "body":
        main.passes.dispose();
        main.view.dispose();
        break;
    }
    this.#instruments.orbit.dispose();
    this.#instruments.craft.dispose();
    for (const key of ["main", "orbit", "craft"] as const) {
      // The drawer has disposed of the main view with itself.
      if (key !== "main" || main.kind === "test-planet") {
        this.#views[key].dispose();
      }
    }
  }

  /** Resizes `keys`' views to `sizes`, each once its size changes, and the lit view with the main. */
  #resize(
    sizes: SpikeFrameInput["sizes"],
    keys: ReadonlyArray<keyof typeof SPIKE_VIEW_NAMES> = ["main", "orbit", "craft"],
  ): void {
    for (const key of keys) {
      const size = sizes[key];
      if (!sameSize(this.#sizes[key], size)) {
        this.#views[key].resize(size);
        if (key === "main" && this.#main.kind === "test-planet") {
          this.#main.lit.resize(size);
        }
        this.#sizes[key] = size;
      }
    }
  }

  /**
   * Ends the open segment's span when the script enters another segment, or its end (`null`),
   * and opens the next; the script's end opens none.
   */
  #segmentSpan(name: string | null, nowMs: number): void {
    const current = this.#segment;
    if (current?.name === name || (current === null && name === null && this.#ended)) {
      return;
    }
    if (current !== null) {
      performance.measure(`${SEGMENT_MEASURE_PREFIX}${current.name}`, {
        start: current.startMs,
        end: nowMs,
      });
    }
    this.#segment = name === null ? null : { name, startMs: nowMs };
    this.#ended = name === null;
  }

  #instrumentCameras(
    scene: ViewScene,
    main: CameraPose,
    tS: number,
  ): Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, CameraState>> {
    const cameraScene = cameraSceneOf(scene);
    const chase = presetPose("chase", "forward", null, cameraScene, scene.defaultPose);
    const craft = newCameraState(cameraScene, "camera");
    return {
      main: heldCamera(scene, main),
      orbit: heldCamera(scene, orbitInstrumentPose(this.#prepared.profile, tS, this.#world)),
      craft: chase === null ? craft : { ...craft, preset: "chase", pose: chase },
    };
  }
}
