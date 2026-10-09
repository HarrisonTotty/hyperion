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
 */

import type { ColourTokens } from "../../spatial/paint";
import type { CameraPose } from "../camera/pose";
import {
  DEFAULT_FOV_DEG,
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  viewRotation4,
} from "../camera/projection";
import { type CameraState, type CameraTarget, newCameraState, presetPose } from "../camera/state";
import type { RenderEngine, RenderView, ViewSize } from "../engine/types";
import { EARTH_REFERENCE } from "../atmosphere/earth";
import { HillaireAtmosphere, TABLE_SIZES } from "../atmosphere/hillaire";
import { sunIlluminanceRgb } from "../atmosphere/solar";
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
import type { PatchKey } from "../terrain/patchKey";
import { type PlanetGeometry, planetGeometry } from "../terrain/planet";
import type { PatchRequest, Selection, SelectionInput } from "../terrain/select";
import { TerrainPass, type TerrainPool, type TerrainPoolFactory } from "../terrain/terrainPass";
import type { BakedPatch, TestPlanetRidges } from "../terrain/workers/messages";
import { buildWireframeDrawList, type ViewStrokes } from "../wireframe/drawList";
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
import { LitView } from "./litView";
import { SEGMENT_MEASURE_PREFIX } from "./metrics";
import { testPlanetRotationAt } from "./rotation";
import {
  contactRule,
  type ContactRule,
  orbitInstrumentPose,
  siteDirection,
  spikeCameraAt,
  SPIKE_CRAFT,
  spikeContactAt,
  spikeScene,
  spikeSunState,
  sunDirectionBody,
  syntheticField,
  TEST_PLANET_FIGURE,
} from "./spikeScene";
import type { SurfaceQuery, SurfaceQueryWorker } from "./surfaceQuery";
import { HeightWorkerPool } from "../terrain/workers/pool";

/** The full-window view's horizontal field of view, rad: R02's default 60°. */
export const SPIKE_FOV_X_RAD = (DEFAULT_FOV_DEG * Math.PI) / 180;

/**
 * The instruments' horizontal field of view, rad: R02's default, which their label blocks read
 * from their cameras' `fovDeg`.
 */
const INSTRUMENT_FOV_X_RAD = (DEFAULT_FOV_DEG * Math.PI) / 180;

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

/** The default height-worker count (Design note 11): clamp(⌊threads ÷ 4⌋, 1, 3). */
export function defaultSpikeWorkers(hardwareConcurrency: number): number {
  return Math.min(3, Math.max(1, Math.floor(hardwareConcurrency / 4)));
}

/** The spike's own workers and pool: the browser's by default, fakes in a test. */
export interface SpikeWorkers {
  /** Starts the surface query's worker. */
  readonly query: () => SurfaceQueryWorker;
  /** Makes the height-worker pool of `workers` workers, holding the synthetic field. */
  readonly pool: (workers: number) => TerrainPoolFactory;
}

/** The browser's: module workers on the renderer's own files (R04.T10.c). */
export const DEFAULT_SPIKE_WORKERS: SpikeWorkers = {
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
    pool.postField(syntheticField());
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
}

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
  /** Passes submitted this frame: the terrain, the atmosphere, the display and two instruments. */
  readonly passesSubmitted: number;
  /** The patches selected under the hard bound. */
  readonly patchesHard: number;
  readonly streaming: boolean;
}

/** Listeners of the run, for T13.c's metrics. */
export interface SpikeListeners {
  readonly onFrame?: (sample: SpikeFrameSample) => void;
  readonly onPatch?: (event: SpikePatchEvent, key: PatchKey) => void;
  /** Called once the terrain is measured and the descent prepared, before the run flies. */
  readonly onPrepared?: (prepared: PreparedDescent) => void;
  /** Each selection's input and result (TerrainPass's `onSelect`), for T13.c's second pass. */
  readonly onSelect?: (input: SelectionInput, selection: Selection) => void;
  /**
   * The run cannot go on, with the status the view shows: the descent refused, the terrain not
   * measured, or the views not made (T13.c ends the run on it).
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
}

/** The terrain's state in one frame, for the console panel. */
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
  readonly contact: boolean;
  readonly scene: ViewScene;
  /** The cameras of the full-window view and the two instruments. */
  readonly cameras: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, CameraState>>;
  readonly terrain: SpikeTerrainReading;
  readonly ev100: number;
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

/** The spike's drawing, on one engine, into its three views. */
export class SpikeRun {
  readonly #prepared: PreparedDescent;
  readonly #kept: KeptScene;
  readonly #listeners: SpikeListeners;
  readonly #views: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, RenderView>>;
  readonly #sizes: Record<keyof typeof SPIKE_VIEW_NAMES, ViewSize | null> = {
    main: null,
    orbit: null,
    craft: null,
  };
  readonly #terrain: TerrainPass;
  readonly #lit: LitView;
  readonly #atmosphere: HillaireAtmosphere;
  readonly #instruments: Readonly<Record<"orbit" | "craft", WireframeRenderer>>;
  readonly #sunBody;
  #startMs: number | null = null;
  #segment: { readonly name: string; readonly startMs: number } | null = null;
  /** Whether the script has reached its end, whose last segment's span is then closed. */
  #ended = false;
  #lastNowMs: number | null = null;

  /**
   * Makes the run's views on `canvases` and its passes.
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
    },
    canvases: Readonly<Record<keyof typeof SPIKE_VIEW_NAMES, HTMLCanvasElement>>,
    listeners: SpikeListeners = {},
  ) {
    this.#prepared = prepared;
    this.#kept = spikeScene(prepared.profile);
    this.#listeners = listeners;
    this.#sunBody = sunDirectionBody(prepared.profile);
    // What is made is released again, in reverse, if a later part cannot be made.
    const undo: (() => void)[] = [];
    try {
      const view = (key: keyof typeof SPIKE_VIEW_NAMES): RenderView => {
        const made = engine.createView(canvases[key], SPIKE_VIEW_NAMES[key]);
        undo.push(() => {
          made.dispose();
        });
        return made;
      };
      this.#views = { main: view("main"), orbit: view("orbit"), craft: view("craft") };
      const onPatch = listeners.onPatch;
      const terrain = new TerrainPass({
        engine,
        setting: options.setting,
        // The setting's own terrain, or the variant T17's runs ask for (decision-r05-spike-ux.md).
        terrain: terrainSettingsFor(options.setting, options.variant),
        planet: prepared.planet,
        ridges: options.ridges,
        createPool: (bake) => listenedPool(options.createPool(bake), listeners),
        measureSelection: true,
        ...(listeners.onSelect === undefined ? {} : { onSelect: listeners.onSelect }),
        ...(onPatch === undefined
          ? {}
          : { onResident: (key: PatchKey) => onPatch("resident", key) }),
      });
      undo.push(() => {
        terrain.dispose();
      });
      this.#terrain = terrain;
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
      this.#lit = lit;
      const atmosphere = new HillaireAtmosphere(
        engine,
        EARTH_REFERENCE,
        TABLE_SIZES[options.setting],
        TEST_PLANET_FIGURE,
      );
      undo.push(() => {
        atmosphere.dispose();
      });
      this.#atmosphere = atmosphere;
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

  /** Compiles the terrain's and the display's materials; frames draw no terrain until then. */
  async ready(): Promise<void> {
    await Promise.all([this.#terrain.ready(), this.#lit.ready(["canvas"])]);
  }

  /** Draws one frame at the script's time for `input.nowMs`. */
  frame(input: SpikeFrameInput): SpikeFrame {
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
    const sun = spikeSunState(this.#sunBody, rotation);
    const lit = this.#lit;
    const terrainStart = performance.now();
    const frame = this.#terrain.frame({
      view: {
        rotation,
        cameraM: camera.positionM,
        orientation: camera.orientation,
        fovXRad: SPIKE_FOV_X_RAD,
        viewport: lit.renderSize,
      },
      grounded: contact === null ? [] : [contact],
      sunDirectionBodyFixed: sun.directionBodyFixed,
      sunIlluminanceLx: sunIlluminanceRgb(),
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
        this.#atmosphere.drawFrame(
          {
            positionM: pose.positionM,
            orientation: pose.orientation,
            fovXRad: SPIKE_FOV_X_RAD,
            viewport: render,
          },
          sun,
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
    return { tS, pose, contact: contact !== null, scene, cameras, terrain, ev100 };
  }

  /**
   * The harness's copy of a view's last frame, RGBA bytes sRGB-encoded (`RenderView.readBack`):
   * called in the same task as the {@link frame} that drew it.
   */
  readBack(view: keyof typeof SPIKE_VIEW_NAMES): Promise<Uint8Array | Float32Array> {
    return this.#views[view].readBack();
  }

  /** Disposes the passes and the views; the engine frees the GPU objects with itself. */
  dispose(): void {
    if (this.#lastNowMs !== null) {
      this.#segmentSpan(null, this.#lastNowMs);
    }
    this.#terrain.dispose();
    this.#lit.dispose();
    this.#atmosphere.dispose();
    this.#instruments.orbit.dispose();
    this.#instruments.craft.dispose();
    for (const view of Object.values(this.#views)) {
      view.dispose();
    }
  }

  #resize(sizes: SpikeFrameInput["sizes"]): void {
    for (const key of ["main", "orbit", "craft"] as const) {
      const size = sizes[key];
      if (!sameSize(this.#sizes[key], size)) {
        this.#views[key].resize(size);
        if (key === "main") {
          this.#lit.resize(size);
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
      orbit: heldCamera(scene, orbitInstrumentPose(this.#prepared.profile, tS)),
      craft: chase === null ? craft : { ...craft, preset: "chase", pose: chase },
    };
  }
}
