/**
 * What the views check's terrain run measures of `VIEW` on `TERRAIN TEST` (plan R10, R10.T16.d;
 * decision-r10-t16-contracts §6), beside the views probe's frames, and the one act it does to the
 * GPU: the forced device loss.
 *
 * @remarks
 * The terrain run's own seam into `VIEW`, made only on a `--views-check --terrain` launch, which
 * `App` hands `VIEW` (decision-r10-t16d-part1: the paced fixture comes through the launch's own
 * path, and nothing an ordinary launch reaches):
 *
 * - **The engine** ({@link TerrainProbe.source}): the views probe's, its adapter's `requestDevice`
 *   wrapped so that the device the engine is made on is held, which {@link TerrainProbe.destroyDevice}
 *   destroys, as R01's loss tests and the smoke harness's `checkForcedLoss` do. No member of
 *   `RenderEngine` reaches the device. Its allocations named for the terrain (`terrain …`,
 *   `coverage mask …`, `class map …`) are tallied by category: the terrain's GPU bytes.
 * - **The height workers** ({@link TerrainProbe.heightWorkers}): the renderer's own, each message
 *   posted and answered counted (bakes, `field-init`, `field-chunk`, `field-release`), and each
 *   worker made and terminated.
 * - **The kept scenes** ({@link TerrainProbe.sceneOptions}): `VIEW`'s own, `TERRAIN TEST`'s fixture
 *   replaced by one of the probe's, its blocks paced ({@link TerrainProbeOptions.paceMs}) so that
 *   blocks are still arriving when the device is destroyed, its traffic counted, and the script's
 *   time it is told read back.
 * - **The session** ({@link TerrainProbe.watchTerrain}): held while `VIEW` holds it, for its pass,
 *   its masks, its survey and its memory report, and followed for its terrain's fault: each time its
 *   `terrainFaults` holds a view or changes while it does, recorded with the time
 *   ({@link TerrainProbe.terrainFaults}; R10.T16.e), which fails the run.
 * - **The submissions** (through the views probe's watch): each view's draws while it hands
 *   `TEST MOON` over (its pass drew for the view), and whether they held a draw of the terrain's
 *   lit material; and each photorealistic view's scene target read back every
 *   {@link TerrainProbeOptions.readBackMs}, its texels checked finite.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import type {
  TerrainCheckBytes,
  TerrainCheckFault,
  TerrainCheckMemory,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  TerrainCheckSessionFault,
  TerrainCheckTeardownRecord,
  TerrainCheckViewRecord,
} from "../../../../../preload/api";
import type { ViewId } from "../../../view/camera/state";
import type { AllocationEvent, MemoryCategory } from "../../../view/engine/memory";
import type {
  FrameSubmission,
  RenderEngine,
  RenderTarget,
  RenderTargetSpec,
  TextureHandle,
} from "../../../view/engine/types";
import type {
  TerrainFixtureTraffic,
  TerrainTestFixture,
} from "../../../view/scenes/terrainFixture";
import {
  TERRAIN_SCENE_NAME,
  terrainMoon,
  terrainSceneFixture,
} from "../../../view/scenes/terrainScene";
import { EngineUnavailable } from "../../../view/engine/resilientEngine";
import type { TerrainFault } from "../../../view/terrain/annunciation";
import type { TerrainSessionMemory } from "../../../view/terrain/session";
import type {
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../../../view/terrain/workers/messages";
import type { WorkerLike } from "../../../view/terrain/workers/pool";
import { browserHeightWorker, type TerrainFixture } from "../useTerrainSession";
import type { ViewEngineSource } from "../useViewEngine";
import { SCENE_OPTIONS, type SceneOption } from "../viewRun";
import {
  faultLine,
  type ShownView,
  type ViewsProbe,
  viewOf,
  type WindowFigures,
} from "./viewsProbe";

/** The terrain's lit material's names begin with this (`terrainLitMaterialSpec`, `terrain-lit-<path>`). */
export const TERRAIN_LIT_PREFIX = "terrain-lit-";

/**
 * The wireframe's terrain material's names begin with this (`terrainWireMaterialSpec`,
 * `terrain-wire-<path>`; plan R10, R10.T11).
 */
export const TERRAIN_WIRE_PREFIX = "terrain-wire-";

/** The allocations that are the terrain's: its passes' resources, and the session's masks and maps. */
const TERRAIN_ALLOCATION = /^(?:terrain |coverage mask |class map )/u;

/** The most spans of handed-over draws without the terrain a view's record lists. */
const MAX_GAP_SPANS = 8;

/** A photorealistic view's scene target (`sceneTarget.ts`'s `<view>:hdr`). */
const SCENE_TARGET = /^[^: ]+:hdr$/u;

/** What the probe reads of one of a body's passes. */
export interface WatchedPass {
  readonly viewIds: ReadonlySet<string>;
  readonly lastStep: { readonly drawSet: { readonly count: number } } | null;
  seesPatch(id: string): boolean;
}

/**
 * What the probe reads of a terrain session (`TerrainSession`'s members of these names): its body's
 * passes, the lit one and the wireframe's (R10.T11), its mask and survey, its memory report, its
 * worker count, and its terrain's fault and the fault's changes.
 */
export interface WatchedSession {
  pass(body: BodyIdHex): WatchedPass | null;
  wirePass(body: BodyIdHex): WatchedPass | null;
  coverageMask(body: BodyIdHex): TextureHandle | null;
  survey(body: BodyIdHex): { readonly coverage: { maskTexels(): Uint8Array } | null } | null;
  memory(): TerrainSessionMemory;
  readonly workerCount: number;
  /** The views that show the terrain's fault, each with its text (`TerrainSession.terrainFaults`). */
  readonly terrainFaults: ReadonlyMap<ViewId, TerrainFault>;
  /** Listens for a change of `terrainFaults`; returns the unsubscribe. */
  subscribeFaults(listener: () => void): () => void;
}

/** The height workers' traffic, over every worker the probe made. */
export interface WorkerTraffic {
  readonly made: number;
  /** Made and not terminated. */
  readonly live: number;
  readonly bakesPosted: number;
  readonly baked: number;
  /** Bakes answered `bake-failed` or `bake-unsurveyed`. */
  readonly refused: number;
  readonly inits: number;
  readonly chunks: number;
  readonly releases: number;
}

/** {@link WorkerTraffic} as the workers count it. */
interface MutableWorkerTraffic {
  made: number;
  live: number;
  bakesPosted: number;
  baked: number;
  refused: number;
  inits: number;
  chunks: number;
  releases: number;
}

/** Bakes posted and not answered. */
export function bakesInFlight(traffic: WorkerTraffic): number {
  return Math.max(0, traffic.bakesPosted - traffic.baked - traffic.refused);
}

/** `inner`, each message it is posted and answers counted into `traffic`; everything else forwarded. */
export function countedWorker(inner: WorkerLike, traffic: MutableWorkerTraffic): WorkerLike {
  traffic.made += 1;
  traffic.live += 1;
  let live = true;
  inner.addEventListener("message", (event: MessageEvent<HeightWorkerReply>) => {
    switch (event.data.kind) {
      case "baked":
        traffic.baked += 1;
        break;
      case "bake-failed":
      case "bake-unsurveyed":
        traffic.refused += 1;
        break;
      case "field-held":
      case "field-refused":
      case "class-map":
      case "class-map-failed":
      case "ground-readout":
      case "ground-readout-failed":
        break;
    }
  });
  return {
    addEventListener: inner.addEventListener.bind(inner),
    removeEventListener: inner.removeEventListener.bind(inner),
    postMessage: (message: HeightWorkerRequest, transfer: Transferable[]): void => {
      switch (message.kind) {
        case "bake":
          traffic.bakesPosted += 1;
          break;
        case "field-init":
          traffic.inits += 1;
          break;
        case "field-chunk":
          traffic.chunks += 1;
          break;
        case "field-release":
          traffic.releases += 1;
          break;
        case "class-map":
        case "ground-readout":
          break;
      }
      inner.postMessage(message, transfer);
    },
    terminate: (): void => {
      if (live) {
        live = false;
        traffic.live -= 1;
      }
      inner.terminate();
    },
  };
}

/** Live and peak bytes by category, over the allocations `TERRAIN_ALLOCATION` names. */
class TerrainTally {
  readonly #live = new Map<MemoryCategory, number>();
  readonly #peak = new Map<MemoryCategory, number>();

  take(event: AllocationEvent): void {
    if (event.kind === "uploaded" || !TERRAIN_ALLOCATION.test(event.name)) {
      return;
    }
    const now =
      (this.#live.get(event.category) ?? 0) + (event.kind === "created" ? 1 : -1) * event.bytes;
    this.#live.set(event.category, now);
    this.#peak.set(event.category, Math.max(this.#peak.get(event.category) ?? 0, now));
  }

  /** Lowers every peak to its live bytes. */
  resetPeaks(): void {
    this.#peak.clear();
    for (const [category, bytes] of this.#live) {
      this.#peak.set(category, bytes);
    }
  }

  live(): TerrainCheckBytes[] {
    return bytesOf(this.#live);
  }

  peak(): TerrainCheckBytes[] {
    return bytesOf(this.#peak);
  }
}

function bytesOf(map: ReadonlyMap<string, number>): TerrainCheckBytes[] {
  return [...map]
    .map(([category, bytes]) => ({ category, bytes }))
    .toSorted((a, b) => (a.category < b.category ? -1 : 1));
}

/**
 * How many texels of a scene target's read-back are not finite in some channel, for its
 * `rgba16float` or `rgba32float`; `null` for another format, which holds no float.
 */
export function nonFiniteTexels(bytes: ArrayBuffer, format: string): number | null {
  let count = 0;
  if (format === "rgba16float") {
    const halves = new Uint16Array(bytes);
    for (let at = 0; at + 3 < halves.length; at += 4) {
      // A half is not finite where its exponent's five bits are all set (an infinity or a NaN).
      if (
        ((halves[at] ?? 0) & 0x7c00) === 0x7c00 ||
        ((halves[at + 1] ?? 0) & 0x7c00) === 0x7c00 ||
        ((halves[at + 2] ?? 0) & 0x7c00) === 0x7c00 ||
        ((halves[at + 3] ?? 0) & 0x7c00) === 0x7c00
      ) {
        count += 1;
      }
    }
    return count;
  }
  if (format === "rgba32float") {
    const floats = new Float32Array(bytes);
    for (let at = 0; at + 3 < floats.length; at += 4) {
      if (
        !Number.isFinite(floats[at] ?? 0) ||
        !Number.isFinite(floats[at + 1] ?? 0) ||
        !Number.isFinite(floats[at + 2] ?? 0) ||
        !Number.isFinite(floats[at + 3] ?? 0)
      ) {
        count += 1;
      }
    }
    return count;
  }
  return null;
}

/** A view's part of an animation frame, as its submissions showed it. */
interface TerrainFrameView {
  /** `TEST MOON`'s pass drew for the view, with a drawn patch in its view, this frame. */
  handedOver: boolean;
  /** The pass drew for the view with no drawn patch in its view (past the survey). */
  offSurvey: boolean;
  /** One of its submissions held a draw of the terrain's lit material. */
  terrain: boolean;
}

/** An animation frame as the probe saw it. */
interface TerrainFrame {
  /** The script's time the stage last told the fixture, s. */
  readonly scriptS: number | null;
  readonly views: Map<string, TerrainFrameView>;
  /** The patches the pass drew, at the primary's scene target's submission, if it had a pass. */
  patches: number | null;
}

/** A read-back of a view's scene target. */
interface ReadBack {
  readonly view: string;
  /** When it was asked, the page's clock, ms. */
  readonly atMs: number;
  readonly nonFinite: number;
}

/** A mask read back: its texture and the texel indices it holds surveyed. */
export interface MaskReading {
  readonly texture: TextureHandle;
  readonly cells: ReadonlySet<number>;
}

/** The counters at one moment, from which a phase's are differences. */
export interface TerrainMark {
  /** The page's clock, ms. */
  readonly atMs: number;
  readonly scriptS: number | null;
  readonly workers: WorkerTraffic;
  readonly fixture: TerrainFixtureTraffic;
}

/** A span the run's verdicts leave out: from the device's destruction to the restore's first draw. */
export interface QuietSpan {
  readonly fromMs: number;
  readonly toMs: number;
}

/** What makes a {@link TerrainProbe}. */
export interface TerrainProbeOptions {
  /** The fixture's wait before each block and before the answer, ms. */
  readonly paceMs: number;
  /** How often a photorealistic view's scene target is read back, ms. */
  readonly readBackMs: number;
  /** Starts a height worker: the renderer's own module worker, outside a test. */
  readonly createWorker?: () => WorkerLike;
  /** The page's clock, `performance.now()`, on which the views probe stamps frames. */
  readonly nowMs?: () => number;
}

/** The terrain run's probe: see the module's remarks. */
export class TerrainProbe {
  /** `VIEW`'s engine source: the views probe's, its device held. */
  readonly source: ViewEngineSource;
  /** Starts one of the session's height workers, counted; stable in identity. */
  readonly heightWorkers: () => WorkerLike;
  /** `VIEW`'s kept scenes, `TERRAIN TEST`'s fixture the probe's. */
  readonly sceneOptions: ReadonlyArray<SceneOption>;
  /** `TERRAIN TEST`'s fixture as the probe serves it: paced, counted, its script's time read. */
  readonly fixture: TerrainFixture;
  readonly #views: ViewsProbe;
  readonly #inner: TerrainTestFixture;
  readonly #moon: BodyIdHex;
  readonly #nowMs: () => number;
  readonly #readBackMs: number;
  readonly #workers: MutableWorkerTraffic = {
    made: 0,
    live: 0,
    bakesPosted: 0,
    baked: 0,
    refused: 0,
    inits: 0,
    chunks: 0,
    releases: 0,
  };
  readonly #devices: GPUDevice[] = [];
  readonly #tally = new TerrainTally();
  readonly #frames = new Map<number, TerrainFrame>();
  readonly #targets = new Map<
    string,
    { readonly spec: RenderTargetSpec; readonly target: RenderTarget }
  >();
  readonly #readBacks: ReadBack[] = [];
  /** Each time a session's `terrainFaults` held a view or changed while it did (R10.T16.e). */
  readonly #terrainFaults: TerrainCheckSessionFault[] = [];
  readonly #readBackAt = new Map<string, number>();
  readonly #reading = new Set<string>();
  #engine: RenderEngine | null = null;
  #session: WatchedSession | null = null;
  #lastSession: WatchedSession | null = null;
  #sessionsMade = 0;
  #sessionsEnded = 0;
  #scriptS: number | null = null;
  #unwatch: (() => void) | null = null;
  /** When the harness destroyed the device ({@link TerrainProbe.destroyDevice}), ms; `null` before. */
  #destroyedAtMs: number | null = null;

  constructor(views: ViewsProbe, options: TerrainProbeOptions) {
    this.#views = views;
    this.#nowMs = options.nowMs ?? (() => performance.now());
    this.#readBackMs = options.readBackMs;
    this.#moon = terrainMoon();
    this.#inner = terrainSceneFixture(options.paceMs);
    const inner = this.#inner;
    this.fixture = {
      universe: inner.universe,
      open: () => inner.open(),
      notePlayed: (scriptS) => {
        this.#scriptS = scriptS;
        inner.notePlayed(scriptS);
      },
    };
    this.sceneOptions = SCENE_OPTIONS.map((option) =>
      option.name === TERRAIN_SCENE_NAME
        ? Object.assign({}, option, { terrain: this.fixture })
        : option,
    );
    const create = options.createWorker ?? browserHeightWorker;
    this.heightWorkers = () => countedWorker(create(), this.#workers);
    const devices = this.#devices;
    this.source = {
      requestAdapter: async () => {
        const outcome = await views.source.requestAdapter();
        if (outcome.kind === "adapter") {
          const adapter = outcome.adapter;
          const requestDevice = adapter.requestDevice.bind(adapter);
          // The check's own hook on the device the engine is made on (decision-r10-t16-contracts
          // §6): nothing else holds it, and no engine member reaches it.
          adapter.requestDevice = async (descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> => {
            const device = await requestDevice(descriptor);
            devices.push(device);
            return device;
          };
        }
        return outcome;
      },
      load: async (outcome, status) => {
        const engine = await views.source.load(outcome, status);
        this.#engine = engine;
        engine.onAllocation((event) => {
          this.#tally.take(event);
        });
        return engine;
      },
    };
  }

  /** Starts watching the views probe's submissions; returns the stop. */
  start(): () => void {
    this.#unwatch?.();
    const unwatch = this.#views.watch({
      submitted: (owner, frame, frameMs) => {
        this.#submitted(owner, frame, frameMs);
      },
      madeTarget: (spec, target) => {
        this.#targets.set(spec.name, { spec, target });
      },
    });
    this.#unwatch = unwatch;
    return () => {
      unwatch();
      if (this.#unwatch === unwatch) {
        this.#unwatch = null;
      }
    };
  }

  /**
   * Holds `session` while `VIEW` holds it (`ViewDisplay`'s terrain watch), and follows its terrain's
   * fault; returns the release, called when `VIEW` lets it go.
   */
  readonly watchTerrain = (session: WatchedSession): (() => void) => {
    this.#session = session;
    this.#lastSession = session;
    this.#sessionsMade += 1;
    const unsubscribe = session.subscribeFaults(() => {
      this.#takeFaults(session);
    });
    this.#takeFaults(session);
    return () => {
      unsubscribe();
      if (this.#session === session) {
        this.#session = null;
      }
      this.#sessionsEnded += 1;
    };
  };

  /**
   * Each time a session's `terrainFaults` held a view or changed while it did, timed (R10.T16.e;
   * decision-r10-t16-leans §8): the run fails on any.
   */
  terrainFaults(): TerrainCheckSessionFault[] {
    return this.#terrainFaults.map((fault) => ({ ...fault, views: [...fault.views] }));
  }

  /** The session `VIEW` holds, or `null`. */
  get session(): WatchedSession | null {
    return this.#session;
  }

  /** `TEST MOON`, the body the run watches. */
  get moon(): BodyIdHex {
    return this.#moon;
  }

  /** The script's time the stage last told the fixture, s, or `null` before any. */
  get scriptS(): number | null {
    return this.#scriptS;
  }

  /** The height workers' traffic so far. */
  workers(): WorkerTraffic {
    return { ...this.#workers };
  }

  /** The counters now. */
  mark(): TerrainMark {
    return {
      atMs: this.#nowMs(),
      scriptS: this.#scriptS,
      workers: this.workers(),
      fixture: this.#inner.traffic(),
    };
  }

  /** Lowers the tally's peaks to its live bytes, as a phase starts. */
  resetPeaks(): void {
    this.#tally.resetPeaks();
  }

  /**
   * Destroys the device the engine was last made on, the harness's one act on the GPU; `false`
   * where none is held.
   */
  destroyDevice(): boolean {
    const device = this.#devices.at(-1);
    if (device === undefined) {
      return false;
    }
    this.#destroyedAtMs = this.#nowMs();
    device.destroy();
    return true;
  }

  /** The first time from `sinceMs` at which `view` drew the terrain, ms; `null` for none yet. */
  firstTerrainSince(view: string, sinceMs: number): number | null {
    let first: number | null = null;
    for (const [atMs, frame] of this.#frames) {
      if (atMs >= sinceMs && frame.views.get(view)?.terrain === true) {
        first = first === null ? atMs : Math.min(first, atMs);
      }
    }
    return first;
  }

  /** `TEST MOON`'s coverage mask, read back; `null` where there is none or it cannot be read. */
  async readMask(): Promise<MaskReading | null> {
    const engine = this.#engine;
    const texture = this.#session?.coverageMask(this.#moon) ?? null;
    if (engine === null || texture === null) {
      return null;
    }
    try {
      const bytes = new Uint8Array(await engine.readTexture(texture));
      return { texture, cells: surveyedCells(bytes) };
    } catch (error: unknown) {
      console.warn("views check: the coverage mask could not be read back:", error);
      return null;
    }
  }

  /** The cells `TEST MOON`'s store's coverage holds surveyed, as mask texel indices; empty with none. */
  coverageCells(): ReadonlySet<number> {
    const coverage = this.#session?.survey(this.#moon)?.coverage ?? null;
    return coverage === null ? new Set() : surveyedCells(coverage.maskTexels());
  }

  /** The session's memory report and the terrain's GPU bytes, or `null` with no session. */
  memory(): TerrainCheckMemory | null {
    const session = this.#session;
    if (session === null) {
      return null;
    }
    const memory = session.memory();
    return {
      workerFieldBytes: memory.workerFieldBytes,
      keptFieldBytes: memory.keptFieldBytes,
      sessionGpuBytes: bytesOf(memory.gpuBytes),
      liveBytes: this.#tally.live(),
      peakBytes: this.#tally.peak(),
    };
  }

  /** The engine's faults so far, timed, as the record holds them. */
  faults(): TerrainCheckFault[] {
    return this.#views.timedFaults().map(({ atMs, fault }) => ({
      atMs,
      kind: fault.kind,
      reason: fault.kind === "device-lost" ? fault.reason : null,
      line: faultLine(fault),
    }));
  }

  /** The teardown as it stands: the last session's report and the workers and bytes left. */
  teardown(): TerrainCheckTeardownRecord {
    const gpu = this.#lastSession?.memory().gpuBytes ?? new Map<string, number>();
    return {
      disposed:
        this.#session === null &&
        this.#sessionsMade > 0 &&
        this.#sessionsEnded === this.#sessionsMade,
      workersMade: this.#workers.made,
      workersLive: this.#workers.live,
      sessionGpuBytes: [...gpu.values()].reduce((sum, bytes) => sum + bytes, 0),
      liveBytes: this.#tally.live(),
    };
  }

  /**
   * The phase `name` from `start` to `end`, whose views were `shown`, leaving out `quiet` (the
   * loss's restore) where it judges a view's draws.
   */
  phase(
    name: TerrainCheckPhaseName,
    start: TerrainMark,
    end: TerrainMark,
    shown: ReadonlyArray<ShownView>,
    quiet: QuietSpan | null,
  ): TerrainCheckPhaseRecord {
    const window: WindowFigures = this.#views.window(start.atMs, end.atMs, shown);
    const inQuiet = (atMs: number): boolean =>
      quiet !== null && atMs >= quiet.fromMs && atMs <= quiet.toMs;
    const frames = this.#views.frameTimes(start.atMs, end.atMs);
    const frameIndex = new Map(frames.map((atMs, i) => [atMs, i]));
    const views = window.views.map((figures): TerrainCheckViewRecord => {
      const draws = this.#views.drawTimes(figures.name, start.atMs, end.atMs);
      const missedFrames = missedAtCadence(draws, frameIndex, quiet);
      const moments = [start.atMs, ...draws, end.atMs];
      let longestGapMs = 0;
      for (let i = 1; i < moments.length; i += 1) {
        const from = moments[i - 1] ?? 0;
        const to = moments[i] ?? 0;
        const crossesQuiet = quiet !== null && from <= quiet.toMs && to >= quiet.fromMs;
        if (!crossesQuiet) {
          longestGapMs = Math.max(longestGapMs, to - from);
        }
      }
      let handedOverDraws = 0;
      let offSurveyDraws = 0;
      let terrainDraws = 0;
      let terrainGaps = 0;
      const gapSpans: Array<[number, number]> = [];
      let current: [number, number] | null = null;
      let firstHandoverMs: number | null = null;
      let firstHandoverScriptS: number | null = null;
      let firstTerrainMs: number | null = null;
      for (const atMs of draws) {
        const frame = this.#frames.get(atMs);
        const seen = frame?.views.get(figures.name);
        if (seen === undefined) {
          continue;
        }
        if (seen.terrain && firstTerrainMs === null) {
          firstTerrainMs = atMs;
        }
        if (seen.offSurvey && !seen.handedOver) {
          offSurveyDraws += 1;
        }
        if (!seen.handedOver) {
          continue;
        }
        handedOverDraws += 1;
        if (firstHandoverMs === null) {
          firstHandoverMs = atMs;
          firstHandoverScriptS = frame?.scriptS ?? null;
        }
        if (seen.terrain) {
          terrainDraws += 1;
          current = null;
        } else if (firstTerrainMs !== null && !inQuiet(atMs)) {
          terrainGaps += 1;
          const scriptS = frame?.scriptS ?? 0;
          if (current === null) {
            current = [scriptS, scriptS];
            if (gapSpans.length < MAX_GAP_SPANS) {
              gapSpans.push(current);
            }
          } else {
            current[1] = scriptS;
          }
        }
      }
      const readBacks = this.#readBacks.filter(
        (each) => each.view === figures.name && each.atMs >= start.atMs && each.atMs < end.atMs,
      );
      return {
        name: figures.name,
        style: figures.style,
        canvas: figures.canvas,
        draws: figures.draws,
        gpuMs: figures.gpuMs,
        submitMs: figures.submitMs,
        missedFrames,
        longestGapMs,
        handedOverDraws,
        offSurveyDraws,
        terrainDraws,
        terrainGaps,
        terrainGapSpansS: gapSpans,
        firstHandoverMs,
        firstHandoverScriptS,
        firstTerrainMs,
        readBacks: readBacks.length,
        nonFiniteTexels: readBacks.reduce((sum, each) => sum + each.nonFinite, 0),
      };
    });
    const patchesDrawn = [...this.#frames]
      .filter(([atMs, frame]) => atMs >= start.atMs && atMs < end.atMs && frame.patches !== null)
      .toSorted(([a], [b]) => a - b)
      .map(([, frame]) => frame.patches ?? 0);
    return {
      name,
      startMs: start.atMs,
      endMs: end.atMs,
      scriptStartS: start.scriptS ?? 0,
      scriptEndS: end.scriptS ?? 0,
      views,
      frameIntervalsMs: window.frameIntervalsMs,
      mainThreadMs: window.mainThreadMs,
      frameGpuMs: window.frameGpuMs,
      patchesDrawn,
      bakes: {
        posted: end.workers.bakesPosted - start.workers.bakesPosted,
        baked: end.workers.baked - start.workers.baked,
        refused: end.workers.refused - start.workers.refused,
      },
      fieldPosts: {
        inits: end.workers.inits - start.workers.inits,
        chunks: end.workers.chunks - start.workers.chunks,
        releases: end.workers.releases - start.workers.releases,
      },
      fieldAsks: end.fixture.fieldAsks - start.fixture.fieldAsks,
      blocksSent: end.fixture.blocksSent - start.fixture.blocksSent,
      memory: this.memory(),
    };
  }

  #submitted(owner: string, frame: FrameSubmission, frameMs: number | null): void {
    const view = viewOf(owner);
    if (frameMs !== null) {
      const seen = this.#frameAt(frameMs);
      const part = seen.views.get(view) ?? { handedOver: false, offSurvey: false, terrain: false };
      // The lit terrain's material, or the wireframe's (R10.T11).
      if (
        frame.draws.some(
          (draw) =>
            draw.material.name.startsWith(TERRAIN_LIT_PREFIX) ||
            draw.material.name.startsWith(TERRAIN_WIRE_PREFIX),
        )
      ) {
        part.terrain = true;
      }
      const pass = this.#session?.pass(this.#moon) ?? null;
      // A view hands the body over to its lit pass or, in the wireframe, to its wireframe pass.
      for (const each of [pass, this.#session?.wirePass(this.#moon) ?? null]) {
        if (each?.viewIds.has(view) === true) {
          if (each.seesPatch(view)) {
            part.handedOver = true;
          } else {
            part.offSurvey = true;
          }
        }
      }
      seen.views.set(view, part);
      if (pass !== null && owner === `${view}:hdr` && view === "view") {
        seen.patches = pass.lastStep?.drawSet.count ?? 0;
      }
    }
    if (SCENE_TARGET.test(owner)) {
      this.#readBackIfDue(owner, view);
    }
  }

  /** Records `session`'s terrain's fault as it stands, where it holds a view. */
  #takeFaults(session: WatchedSession): void {
    const faults = session.terrainFaults;
    if (faults.size === 0) {
      return;
    }
    this.#terrainFaults.push({
      atMs: this.#nowMs(),
      // The session's one cause, which every view shows alike.
      text: [...new Set(faults.values())].join("; "),
      views: [...faults.keys()],
    });
  }

  #frameAt(frameMs: number): TerrainFrame {
    const known = this.#frames.get(frameMs);
    if (known !== undefined) {
      return known;
    }
    const frame: TerrainFrame = { scriptS: this.#scriptS, views: new Map(), patches: null };
    this.#frames.set(frameMs, frame);
    return frame;
  }

  /** Reads `owner`'s colour back, its texels checked finite, at most once every interval. */
  #readBackIfDue(owner: string, view: string): void {
    const engine = this.#engine;
    const made = this.#targets.get(owner);
    const atMs = this.#nowMs();
    if (
      engine === null ||
      made === undefined ||
      this.#reading.has(owner) ||
      atMs < (this.#readBackAt.get(owner) ?? Number.NEGATIVE_INFINITY) + this.#readBackMs
    ) {
      return;
    }
    this.#readBackAt.set(owner, atMs);
    this.#reading.add(owner);
    void this.#readBack(engine, made.target, made.spec.format, view, atMs)
      .catch((error: unknown) => {
        console.warn(`views check: the read-back of ${owner} failed:`, error);
      })
      .finally(() => {
        this.#reading.delete(owner);
      });
  }

  /**
   * Reads `target`'s colour back and keeps how many of its texels are not finite. A read-back across
   * the device's loss is refused, the frame it would have read gone with it, and is left uncounted:
   * one the engine refuses for the loss (`EngineUnavailable`), or one in flight when the harness
   * destroyed the device. Any other refusal is the check's failure, and is reported.
   */
  async #readBack(
    engine: RenderEngine,
    target: RenderTarget,
    format: string,
    view: string,
    atMs: number,
  ): Promise<void> {
    let bytes: ArrayBuffer;
    try {
      bytes = await engine.readTexture(target.colour);
    } catch (error: unknown) {
      const destroyed = this.#destroyedAtMs;
      if (error instanceof EngineUnavailable || (destroyed !== null && atMs <= destroyed)) {
        return;
      }
      console.warn(`views check: the read-back of ${view}'s scene target was refused:`, error);
      return;
    }
    const nonFinite = nonFiniteTexels(bytes, format);
    if (nonFinite !== null) {
      this.#readBacks.push({ view, atMs, nonFinite });
    }
  }
}

/**
 * The draws a view missed at its own cadence: for each two draws in a row (their frames' indices
 * in `frameIndex`) more frames apart than its most common gap, the cadence's draws that fit between
 * them (a gap of 3 at 60 Hz misses 2, one of 6 at 30 Hz misses 2); gaps that cross `quiet` are left
 * out.
 */
export function missedAtCadence(
  draws: ReadonlyArray<number>,
  frameIndex: ReadonlyMap<number, number>,
  quiet: QuietSpan | null,
): number {
  const gaps: Array<{ readonly frames: number; readonly fromMs: number; readonly toMs: number }> =
    [];
  for (let i = 1; i < draws.length; i += 1) {
    const fromMs = draws[i - 1] ?? 0;
    const toMs = draws[i] ?? 0;
    const from = frameIndex.get(fromMs);
    const to = frameIndex.get(toMs);
    if (from !== undefined && to !== undefined) {
      gaps.push({ frames: to - from, fromMs, toMs });
    }
  }
  const counts = new Map<number, number>();
  for (const gap of gaps) {
    counts.set(gap.frames, (counts.get(gap.frames) ?? 0) + 1);
  }
  let cadence = 1;
  let most = 0;
  for (const [frames, count] of counts) {
    if (count > most || (count === most && frames < cadence)) {
      cadence = frames;
      most = count;
    }
  }
  return gaps
    .filter((gap) => quiet === null || gap.fromMs > quiet.toMs || gap.toMs < quiet.fromMs)
    .reduce((sum, gap) => sum + Math.max(0, Math.round(gap.frames / cadence) - 1), 0);
}

/** The texel indices whose byte is not zero. */
function surveyedCells(bytes: Uint8Array): Set<number> {
  const cells = new Set<number>();
  for (let at = 0; at < bytes.length; at += 1) {
    if ((bytes[at] ?? 0) !== 0) {
      cells.add(at);
    }
  }
  return cells;
}
