/**
 * The perf run's driver of the descent scenes (plan R12, R12.T4.c; Design notes 13 and 14):
 * `cockpit-descent` and `station-wireframe-descent`, drawn by R05's spike (`ServedDescentSpike`
 * over `DescentSpike`) on the served world each arrival names, and the measured engine they draw
 * on, kept across the repetitions.
 *
 * @remarks
 * Each mount is a stage `PerfApp` renders under its own `key`, so that a repetition's views, their
 * terrain session and with it the patch cache, its height textures and the height workers' queues
 * are new and empty, and its field is asked of the restarted server again (Design note 13's
 * resets). The engine is R05's measured one (`spikeGpu`: the pipeline tally, the resolve count and
 * the pass-time reads on each device), loaded once and handed to every mount in a wrapper whose
 * `dispose` does nothing, so that its pipelines stay compiled from one repetition to the next, as a
 * player's second descent has them; the run's end disposes it. The descent's measurement (its site,
 * floors and profile) is made once, over the warm-up's world, and reused: it is a function of the
 * world and the spike's seed, which every repetition's fresh server generates the same, and it
 * holds no state of the session it was measured through.
 *
 * The warm-up holds the script's first pose (`DescentSpikePerf.scriptNowMs` constant), and a
 * repetition flies it from its first frame. A frame is told to the run once it is drawn, with the
 * session it drew through, so that the run reads the patch cache of the views it measures.
 */

import type { AdapterRecord, InternalScaleRecord } from "@hyperion/perf";

import type { PerfConfig, PerfDescribedView } from "../../../../preload/api";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import type { RenderStyle } from "../camera/state";
import type { AllocationEvent } from "../engine/memory";
import type { AdapterOutcome, GpuCapabilities } from "../engine/platform";
import type { GraphicsFault, GraphicsStatusStore } from "../engine/status";
import type { PassTimes, RenderEngine } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import { type MeasureDescent, measureWorldDescent } from "../spike/bodyDescent";
import type { DescentSpikePerf, SpikeDrawnFrame, SpikeDrawnSession } from "../spike/DescentSpike";
import { SETTING_VIEWS } from "../spike/demandRecord";
import { meanDemand, type SpikeGpu, spikeGpu } from "../spike/spikeHarness";
import type { PreparedDescent, SpikeListeners } from "../spike/spikeRun";
import { allocationTally } from "../terrain/gpu/allocationTally";
import { selectionTolerancePx } from "../terrain/selectionTolerance";
import type { WorkerMemory } from "../terrain/workers/pool";
import { MemoryLedger } from "./ledger";
import type {
  PerfDrawn,
  PerfDriverChoice,
  PerfSceneDriver,
  PerfSceneSink,
  PerfStage,
} from "./runPerfScene";
import { PERF_LANDING_SEED, PERF_SCENES, type PerfScene } from "./scenes";

/** An instrument's rate where the scene names none, Hz: R07's instruments' (Design note 14). */
const INSTRUMENT_RATE_HZ = 30;

/** Each view drawn at its full internal scale: the spike's session views draw unscaled. */
const UNSCALED: InternalScaleRecord = { min: 1, median: 1 };

/** A run's fault in the words the record's notes give. */
export function faultReason(fault: GraphicsFault): string {
  let reason: string;
  switch (fault.kind) {
    case "device-lost":
      reason = `GraphicsFault: the device was lost (${fault.reason}: ${fault.message})`;
      break;
    case "gpu-process-gone":
      reason = `GraphicsFault: the GPU process is gone (${String(fault.count)} this launch)`;
      break;
    case "shader-refused":
      reason = `GraphicsFault: the shader of ${fault.effectName} was refused`;
      break;
    case "view-refused":
      reason = `GraphicsFault: the view ${fault.viewName} was not made again`;
      break;
  }
  return reason;
}

/** The boolean members of R01's `GpuCapabilities`, by name, as the record lists them. */
export function capabilityFlags(capabilities: GpuCapabilities): Readonly<Record<string, boolean>> {
  const flags: Record<string, boolean> = {};
  for (const [name, value] of Object.entries(capabilities)) {
    if (typeof value === "boolean") {
      flags[name] = value;
    }
  }
  return flags;
}

/**
 * `engine` in everything but `dispose`, which does nothing: the engine a mount's `useViewEngine`
 * holds and releases at its unmount, while the run keeps it. Its other members are the engine's
 * own, its methods bound to it, so that they read its private fields on the engine itself.
 */
export function undisposable(engine: RenderEngine): RenderEngine {
  return new Proxy(engine, {
    get: (target, property): unknown => {
      if (property === "dispose") {
        return () => undefined;
      }
      const value: unknown = Reflect.get(target, property, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
}

/**
 * The run's measured engine (R05's `spikeGpu`), loaded at the first mount and kept for the run:
 * its pass times, allocations and faults go to the sink of the latest mount, and its allocation
 * tally is itemised by a {@link MemoryLedger}.
 */
export class PerfEngine {
  readonly gpu: SpikeGpu;
  readonly #load: ViewEngineSource["load"];
  #outcome: Promise<AdapterOutcome> | null = null;
  #loaded: Promise<RenderEngine> | null = null;
  #engine: RenderEngine | null = null;
  #ledger: MemoryLedger | null = null;
  #adapter: AdapterRecord | null = null;
  readonly #releases: Array<() => void> = [];
  /** The sink the engine's events go to: the latest mount's. */
  sink: PerfSceneSink | null = null;
  /** The script time of the last frame, s, which each pipeline creation is stamped with. */
  scriptTimeS = 0;

  /**
   * @param gpu - The browser's `navigator.gpu`.
   * @param load - Makes the engine: the measured source's own by default, a fake in a test.
   */
  constructor(gpu: GPU | undefined, load?: ViewEngineSource["load"]) {
    this.gpu = spikeGpu(gpu, null, () => this.scriptTimeS);
    this.#load = load ?? this.gpu.source.load;
  }

  /** The engine source every mount draws on: one adapter, one engine, kept. */
  readonly source: ViewEngineSource = {
    requestAdapter: () => {
      this.#outcome ??= this.gpu.source.requestAdapter().then((outcome) => {
        if (outcome.kind === "adapter") {
          const { vendor, architecture, description, fallback } = outcome.summary;
          this.#adapter = { vendor, architecture, description, fallback };
        } else {
          this.sink?.failed(`no GPU adapter (${outcome.kind})`);
        }
        return outcome;
      });
      return this.#outcome;
    },
    load: async (outcome, status) => {
      this.#loaded ??= this.#made(outcome, status);
      return undisposable(await this.#loaded);
    },
  };

  async #made(
    outcome: AdapterOutcome & { readonly kind: "adapter" },
    status: GraphicsStatusStore,
  ): Promise<RenderEngine> {
    let engine: RenderEngine;
    try {
      engine = await this.#load(outcome, status);
    } catch (error: unknown) {
      this.sink?.failed("the engine could not be made");
      throw error;
    }
    this.gpu.resolves.follow(engine);
    this.#engine = engine;
    const tally = allocationTally(engine);
    this.#ledger = new MemoryLedger(tally);
    this.#releases.push(
      engine.onPassTimes((times: PassTimes) => {
        const frame = this.gpu.resolves.runFrame(times.frame);
        this.gpu.reads.ended(frame);
        this.sink?.passTimes({ ...times, frame });
      }),
      engine.onAllocation((event: AllocationEvent) => {
        this.sink?.allocation(event);
      }),
      engine.onFault((fault) => {
        this.sink?.failed(faultReason(fault));
      }),
      () => {
        this.#ledger?.dispose();
        tally.dispose();
      },
    );
    return engine;
  }

  /** The itemised tally, or `null` before the engine is made. */
  get ledger(): MemoryLedger | null {
    return this.#ledger;
  }

  /** The adapter's summary, or `null` before one answers. */
  get adapter(): AdapterRecord | null {
    return this.#adapter;
  }

  /** The device's capabilities, or `null` before the engine is made. */
  get capabilities(): GpuCapabilities | null {
    return this.#engine?.capabilities ?? null;
  }

  /** Releases the engine and its listeners: the run is over, or the page is going. */
  dispose(): void {
    for (const release of this.#releases.splice(0)) {
      release();
    }
    this.#engine?.dispose();
    this.#engine = null;
    this.#ledger = null;
    this.#loaded = null;
    this.#outcome = null;
  }
}

/**
 * `measure`, made once for each world's body and seed and its result reused: the descent is a
 * function of the world and the seed alone, which every repetition's fresh server generates the
 * same (the module's remarks). A measurement that fails is not kept.
 */
export function measuredOnce<S, W extends { readonly body: string }>(
  measure: (session: S, world: W, seed: bigint, signal: AbortSignal) => Promise<PreparedDescent>,
): (session: S, world: W, seed: bigint, signal: AbortSignal) => Promise<PreparedDescent> {
  const measured = new Map<string, PreparedDescent>();
  return async (session, world, seed, signal) => {
    const key = `${world.body}:${seed.toString()}`;
    const kept = measured.get(key);
    if (kept !== undefined) {
      return kept;
    }
    const made = await measure(session, world, seed, signal);
    measured.set(key, made);
    return made;
  };
}

/** What `PerfApp` renders for a mount: `ServedDescentSpike`'s props, under its own `key`. */
export interface DescentStage {
  readonly key: string;
  readonly serverUrl: string;
  readonly universe: string;
  readonly body: string;
  /** The spike's seed, which draws the landing site (`PERF_LANDING_SEED`). */
  readonly seed: bigint;
  readonly setting: QualitySetting;
  /** The main view's style: the scene's first view's. */
  readonly style: RenderStyle;
  readonly workers: number;
  readonly measure: MeasureDescent;
  readonly listeners: SpikeListeners;
  readonly perf: DescentSpikePerf;
}

/** The stage `PerfApp` renders, an external store it subscribes to. */
export class DescentStages {
  #stage: DescentStage | null = null;
  readonly #listeners = new Set<() => void>();

  /** The stage now, or `null` between mounts. */
  readonly current = (): DescentStage | null => this.#stage;

  /** Listens for a change of stage; returns the unsubscribe. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Shows `stage`, or none. */
  set(stage: DescentStage | null): void {
    this.#stage = stage;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/** What a {@link DescentDriver} is given. */
export interface DescentDriverOptions {
  readonly scene: PerfScene;
  readonly setting: QualitySetting;
  /** The height workers the terrain session runs. */
  readonly workers: number;
  readonly engine: PerfEngine;
  readonly stages: DescentStages;
  /** How the descent is measured the first time: R05's `measureWorldDescent`, or a test's. */
  readonly measure?: MeasureDescent;
}

/** A frame's sample, told to the run once the frame is drawn. */
type HeldSample = Parameters<PerfSceneSink["frame"]>[0];

/** The driver of a descent scene: R05's spike over the served world, remounted per repetition. */
export class DescentDriver implements PerfSceneDriver {
  readonly #options: DescentDriverOptions;
  /** The descent's measurement, made once and reused by later mounts (the module's remarks). */
  readonly #measure: MeasureDescent;
  #prepared: PreparedDescent | null = null;
  #demand: ReadonlyMap<string, number> | null = null;
  #stage: DescentStage | null = null;
  #session: SpikeDrawnSession | null = null;
  #body: string | null = null;

  constructor(options: DescentDriverOptions) {
    this.#options = options;
    this.#measure = measuredOnce(options.measure ?? measureWorldDescent);
  }

  mount(stage: PerfStage, sink: PerfSceneSink): Promise<PerfDrawn> {
    const { engine, scene, setting, workers, stages } = this.#options;
    engine.sink = sink;
    const body = stage.arrival.body;
    if (body === null) {
      const reason = `${scene.id}: the arrival placed the ship beside no body`;
      sink.failed(reason);
      return Promise.reject(new Error(reason));
    }
    const main = scene.views[setting][0];
    if (main === undefined) {
      const reason = `${scene.id}: the scene has no view at ${setting}`;
      sink.failed(reason);
      return Promise.reject(new Error(reason));
    }
    return new Promise<PerfDrawn>((resolve, reject) => {
      const current = (): boolean => this.#stage === made;
      let held: HeldSample | null = null;
      let drawn = false;
      const listeners: SpikeListeners = {
        onFrame: (sample) => {
          if (current()) {
            engine.scriptTimeS = sample.scriptTimeS;
            held = { ...sample, engineFrame: engine.gpu.resolves.value };
          }
        },
        onPatch: (event) => {
          if (current()) {
            sink.patch(event);
          }
        },
        onPrepared: (prepared) => {
          if (current()) {
            this.#prepared = prepared;
          }
        },
        onFailed: (status) => {
          if (current()) {
            sink.failed(status);
            reject(new Error(status));
          }
        },
      };
      const perf: DescentSpikePerf = {
        ...(stage.phase === "warm-up" ? { scriptNowMs: () => 0 } : {}),
        onDrawn: (frame) => {
          if (!current()) {
            return;
          }
          this.#session = frame.session;
          const sample = held;
          held = null;
          if (sample !== null) {
            sink.frame(sample);
          }
          if (!drawn && this.#prepared !== null) {
            drawn = true;
            resolve(this.#drawnOf(frame, this.#prepared, main));
          }
        },
      };
      const made: DescentStage = {
        key: stage.phase === "warm-up" ? "warm-up" : `repetition-${String(stage.n)}`,
        serverUrl: stage.serverUrl,
        universe: stage.arrival.universe,
        body,
        seed: BigInt(PERF_LANDING_SEED),
        setting,
        style: main.style,
        workers,
        measure: this.#measure,
        listeners,
        perf,
      };
      this.#stage = made;
      this.#body = body;
      this.#session = null;
      stages.set(made);
    });
  }

  unmount(): void {
    this.#stage = null;
    this.#session = null;
    this.#options.stages.set(null);
  }

  cachedPatches(): number {
    const session = this.#session;
    const body = this.#body;
    if (session === null || body === null) {
      return 0;
    }
    return (session.pass(body)?.usedSlots ?? 0) + (session.wirePass(body)?.usedSlots ?? 0);
  }

  workerMemory(): ReadonlyArray<WorkerMemory> {
    return this.#session?.workerMemory ?? [];
  }

  memory(): MemoryLedger | null {
    return this.#options.engine.ledger;
  }

  latePipelines(): number {
    return this.#options.engine.gpu.tally.late().length;
  }

  endWarmUp(): void {
    this.#options.engine.gpu.tally.endWarmup();
  }

  resolveCount(): number {
    return this.#options.engine.gpu.resolves.value;
  }

  internalScale(views: ReadonlyArray<PerfDescribedView>): ReadonlyArray<InternalScaleRecord> {
    return views.map(() => UNSCALED);
  }

  /** The views as drawn, and the script the descent flies. */
  #drawnOf(
    frame: SpikeDrawnFrame,
    prepared: PreparedDescent,
    main: PerfScene["views"][QualitySetting][number],
  ): PerfDrawn {
    const { scene, setting, workers, engine } = this.#options;
    const instrumentRateHz =
      scene.views[setting].find(({ role }) => role === "instrument")?.rateHz ?? INSTRUMENT_RATE_HZ;
    const instrument = (size: SpikeDrawnFrame["sizes"]["orbit"]): PerfDescribedView => ({
      role: "instrument",
      style: "wireframe",
      widthPx: size.widthPx,
      heightPx: size.heightPx,
      rateHz: instrumentRateHz,
      lineAntialiasing: "analytic",
      exposure: null,
    });
    const capabilities = engine.capabilities;
    const demand = this.#demandOf(prepared);
    return {
      views: [
        {
          role: main.role,
          style: main.style,
          widthPx: frame.sizes.main.widthPx,
          heightPx: frame.sizes.main.heightPx,
          rateHz: main.rateHz,
          lineAntialiasing: main.lineAntialiasing,
          exposure: null,
        },
        instrument(frame.sizes.orbit),
        instrument(frame.sizes.craft),
      ],
      adapter: engine.adapter,
      capabilities: capabilities === null ? null : capabilityFlags(capabilities),
      kernelPaths: null,
      heightWorkers: workers,
      spanS: prepared.profile.durationS,
      segments: prepared.profile.segmentSpans(),
      demandPerS: (segment) => demand.get(segment) ?? null,
      notes: [
        `The renderer drew ${scene.id} with R05's descent spike over the served world: its main view (${main.style}) and its two wireframe instruments every frame, unscaled; kernel paths are not read (R12.T4.c).`,
        ...(main.role === "station"
          ? [
              "The spike's two instruments draw beside the station's view, and their wireframe passes are counted in station-wireframe, since a resolve names no view (lean, R12.T4.c).",
            ]
          : []),
      ],
    };
  }

  /**
   * R05's predicted demand by segment under the hard bound at τ_sel (`SpikeRecorder`'s), made
   * once: the mean over each segment's 1 Hz poses.
   */
  #demandOf(prepared: PreparedDescent): ReadonlyMap<string, number> {
    if (this.#demand !== null) {
      return this.#demand;
    }
    const settingView = SETTING_VIEWS.find((v) => v.setting === this.#options.setting)?.view;
    const demand = new Map<string, number>();
    if (settingView !== undefined) {
      const view = { ...settingView, tauPx: selectionTolerancePx(settingView.tauPx) };
      for (const span of prepared.profile.segmentSpans()) {
        demand.set(span.name, meanDemand(prepared.planet, prepared.profile, span, view));
      }
    }
    this.#demand = demand;
    return demand;
  }
}

/** What {@link descentDriverFor} needs of the page. */
export interface DescentDriverPage {
  readonly engine: PerfEngine;
  readonly stages: DescentStages;
  /** `window.hyperion.graphics.launchMode`. */
  readonly launchMode: PerfConfig["launchMode"];
  /** Whether the renderer has WebGPU (`navigator.gpu`). */
  readonly webGpu: boolean;
  /** The height workers a scene that names none runs: the machine's default. */
  readonly defaultWorkers: () => number;
}

/**
 * The driver of the configured scene: a descent's, or why the renderer has none: a scene of no
 * descent has no driver yet (R12.T4.c), and the graphics safe mode and a renderer without WebGPU
 * draw no view.
 */
export function descentDriverFor(config: PerfConfig, page: DescentDriverPage): PerfDriverChoice {
  const scene = PERF_SCENES[config.launch.scene];
  switch (scene.script.kind) {
    case "descent":
      break;
    case "approach":
    case "arrival":
    case "sweep":
      return { kind: "refused", reason: `${scene.id}: no renderer driver yet (R12.T4.c)` };
  }
  if (page.launchMode === "safe") {
    return { kind: "refused", reason: "the graphics safe mode draws no WebGPU view" };
  }
  if (!page.webGpu) {
    return { kind: "refused", reason: "this renderer has no WebGPU" };
  }
  return {
    kind: "driver",
    driver: new DescentDriver({
      scene,
      setting: config.launch.setting,
      workers: scene.harness.heightWorkers ?? page.defaultWorkers(),
      engine: page.engine,
      stages: page.stages,
    }),
  };
}
