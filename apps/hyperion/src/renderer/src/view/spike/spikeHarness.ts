/**
 * The descent spike's measurement harness in the renderer (plan R05, T13.c and T14.a's pending
 * wiring): the measured engine source, the pass rows, and the recorder that turns the run's
 * listeners into T14.a's `SpikeMetrics`.
 *
 * @remarks
 * The engine is reached only through the spike's `ViewEngineSource` (R01 leaves no other way):
 * its `GPU` is wrapped (`wrapGpu`) so that the device it gives carries T14.a's pipeline tally and
 * T15.a's capture when `--capture` is given, and the wrapper tells the resolve numbering of each
 * new device, whose timer numbers from 1 again; the same wrapped `GPU` is the engine's
 * `LoadEngineOptions.gpu`, so a rebuild after a device loss is measured too. The engine it loads
 * numbers the resolves (`RenderEngine.passTimesFrame`).
 */

import type { DescentSpikeReport, SpikeLatePipeline, SpikePassRow } from "../../../../preload/api";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { loadRenderEngine } from "../engine/loadEngine";
import type { AllocationEvent } from "../engine/memory";
import { requestAdapterOutcome } from "../engine/platform";
import type { PassTimes, RenderEngine } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import { levelBoundM, type PlanetGeometry } from "../terrain/planet";
import { type SelectionInput, selectPatches } from "../terrain/select";
import { selectionTolerancePx } from "../terrain/selectionTolerance";
import type { GpuCapture } from "./capture";
import { SETTING_VIEWS } from "./demandRecord";
import { boundedPlanet, type DemandView, levelRatio, perLevelDemand } from "./demand";
import type { DescentProfile } from "./descentProfile";
import {
  type FrameSample,
  type PatchEvent,
  SpikeMetrics,
  type SpikeMetricsReport,
} from "./metrics";
import { PassReads, trackPassReads } from "./passReads";
import { PipelineTally, shimPipelines, wrapGpu } from "./pipelineShim";

/** The warm-up the criterion leaves out, s (Design note 21), and the pipeline tally's too. */
export const SPIKE_WARMUP_S = 10;

/**
 * Which of Design note 21's GPU rows a pass counts towards, by its `FrameSubmission.label`: the
 * terrain's, the atmosphere's (`atmosphere composite`, the compute `atmosphere view`, and its
 * tables), and the rest (`spike display`, `view:wireframe`).
 */
export function rowOf(label: string): SpikePassRow {
  if (label.startsWith("terrain")) {
    return "terrain";
  }
  return label.startsWith("atmosphere") ? "atmosphere" : "other";
}

/** The spike's own pass labels and their rows, for the capture's `meta.passRows`. */
export const SPIKE_PASS_ROWS: Readonly<Record<string, SpikePassRow>> = {
  terrain: "terrain",
  "atmosphere composite": "atmosphere",
  "atmosphere view": "atmosphere",
  "spike display": "other",
  "view:wireframe": "other",
};

/**
 * The run's numbering of R01's timer resolves, the engine's timing frame numbers
 * (`PassTimes.frame`) that its asynchronous times are matched against, carried across the device
 * rebuilds of a run.
 *
 * @remarks
 * The count is the engine's own, `RenderEngine.passTimesFrame`, which a resolve dropped while every
 * read-back buffer is still in flight takes too (R07.T19). A count of the `resolveQuerySet` calls
 * on the device, as this once was, misses each dropped resolve, so after the first drop every
 * later report was numbered ahead of every frame and matched none: R05.T14.f's high run lost its
 * pass times for good after 9.75 s.
 */
export class ResolveCounter {
  #engine: Pick<RenderEngine, "passTimesFrame"> | null = null;
  /** The run's numbers taken by the timers of devices before the latest. */
  #base = 0;
  /** The highest number of the latest device's timer seen so far, by a frame or a report. */
  #latest = 0;

  /**
   * Follows `engine`'s numbering, once the engine is made: the one `loadRenderEngine` returns,
   * whose count starts again from 0 at a restore.
   */
  follow(engine: Pick<RenderEngine, "passTimesFrame">): void {
    this.#engine = engine;
  }

  /**
   * The run's last timing frame number now: the latest device's, after every earlier one's. Each
   * read records the number, so that a rebuilt device's numbers follow every number handed out.
   */
  get value(): number {
    this.#latest = Math.max(this.#latest, this.#engine?.passTimesFrame ?? 0);
    return this.#base + this.#latest;
  }

  /**
   * A `PassTimes.frame` of the latest device's timer as the run's frame number: a rebuild after a
   * device loss makes a new timer, which counts from 1 again (`ResilientEngine`). The number is
   * recorded, as {@link ResolveCounter.value}'s is.
   */
  runFrame(timerFrame: number): number {
    this.#latest = Math.max(this.#latest, timerFrame);
    return this.#base + timerFrame;
  }

  /**
   * A new device is made: its engine's timer numbers from 1 again, after every number the run has
   * seen. The lost engine is released before its successor's device is requested, so its count is
   * no longer readable here; the highest seen stands for it.
   */
  deviceMade(): void {
    this.#base = this.value;
    this.#latest = 0;
  }
}

/**
 * The spike's measured GPU: its engine source, the pipeline tally its devices feed, the resolve
 * numbering its engine feeds, and the pass-time reads its devices' timer makes.
 */
export interface SpikeGpu {
  readonly source: ViewEngineSource;
  readonly tally: PipelineTally;
  readonly resolves: ResolveCounter;
  readonly reads: PassReads;
}

/**
 * The spike's engine source over `gpu` (the browser's `navigator.gpu`): every device it gives is
 * shimmed for the pipeline tally and the pass-time reads, then wrapped for the capture where one
 * is given, and told to the resolve numbering, which follows the engine it loads.
 *
 * @param scriptTimeS - The descent's script time now, s, which each pipeline creation is stamped
 *   with.
 */
export function spikeGpu(
  gpu: GPU | undefined,
  capture: GpuCapture | null,
  scriptTimeS: () => number,
): SpikeGpu {
  const tally = new PipelineTally(scriptTimeS);
  const resolves = new ResolveCounter();
  const reads = new PassReads(() => resolves.value);
  const wrapped =
    gpu === undefined
      ? undefined
      : wrapGpu(gpu, (device) => {
          resolves.deviceMade();
          const shimmed = trackPassReads(shimPipelines(device, tally), reads);
          return capture === null ? shimmed : capture.wrapDevice(shimmed);
        });
  return {
    tally,
    resolves,
    reads,
    source: {
      requestAdapter: () => requestAdapterOutcome(wrapped),
      load: async (outcome, status) => {
        const engine = await loadRenderEngine(
          outcome,
          status,
          wrapped === undefined ? {} : { gpu: wrapped },
        );
        resolves.follow(engine);
        return engine;
      },
    },
  };
}

/** What the recorder needs of the pass-time reads (`PassReads`). */
export type SpikeReads = Pick<PassReads, "ended" | "inFlight" | "settled">;

/** What the recorder needs of the measured descent (T13.b's `PreparedDescent`). */
export interface RecordedDescent {
  readonly planet: PlanetGeometry;
  readonly profile: DescentProfile;
  /** σ_n for levels 0 to 24 (`omittedSigmaM`), for the calibrated count. */
  readonly omittedSigmaM: ReadonlyArray<number>;
}

/**
 * A segment's mean per-level D over its 1 Hz poses, at the height above the floor (T13.a), in
 * `view`, whose τ is the one selection runs at.
 *
 * @returns Patches a second; 0 for a span with no whole second.
 */
export function meanDemand(
  planet: PlanetGeometry,
  profile: DescentProfile,
  span: { readonly startS: number; readonly endS: number },
  view: DemandView,
): number {
  let sum = 0;
  let count = 0;
  for (let t = span.startS; t < span.endS; t += 1) {
    const pose = profile.poseAt(t);
    sum += perLevelDemand(planet, { ...pose, altitudeM: pose.heightAboveFloorM }, view).perS;
    count += 1;
  }
  return count === 0 ? 0 : sum / count;
}

/**
 * Turns the spike run's listeners into T14.a's metrics: each frame with its engine frames and its
 * patch counts under both bounds, each patch event at its script time, the pass times and the
 * allocations, and the report at the end.
 *
 * @remarks
 * D under both bounds, and each level's k, are taken at τ_sel, the tolerance the terrain pass
 * selects at (`selectionTolerancePx`), so that D predicts the selection whose demand is measured
 * (decision-r05-record-tau.md).
 */
export class SpikeRecorder {
  readonly #metrics: SpikeMetrics;
  readonly #calibrated: PlanetGeometry;
  readonly #resolves: Pick<ResolveCounter, "value" | "runFrame">;
  readonly #reads: SpikeReads;
  readonly #tally: PipelineTally;
  /** The engine's last timing frame number before the run's first frame. */
  readonly #firstEngineFrame: number;
  /** The engine's last timing frame number by the end of the last frame recorded. */
  #lastEngineFrame: number;
  #lastScriptS = 0;
  #patchesCalibrated = 0;
  #liveBytes = 0;
  #peakBytes = 0;
  #warmupEnded = false;
  /** Patches baked so far, which the smoke run checks. */
  bakedPatches = 0;

  constructor(
    descent: RecordedDescent,
    setting: QualitySetting,
    gpu: {
      readonly resolves: Pick<ResolveCounter, "value" | "runFrame">;
      readonly reads: SpikeReads;
      readonly tally: PipelineTally;
    },
  ) {
    const { planet, profile, omittedSigmaM } = descent;
    this.#calibrated = boundedPlanet(planet, "calibrated", omittedSigmaM);
    this.#resolves = gpu.resolves;
    this.#reads = gpu.reads;
    this.#tally = gpu.tally;
    this.#firstEngineFrame = gpu.resolves.value;
    this.#lastEngineFrame = this.#firstEngineFrame;
    const settingView = SETTING_VIEWS.find((v) => v.setting === setting)?.view;
    if (settingView === undefined) {
      throw new Error(`no view for the ${setting} setting`);
    }
    const view: DemandView = { ...settingView, tauPx: selectionTolerancePx(settingView.tauPx) };
    const spans = profile.segmentSpans();
    const predicted = new Map(
      spans.map((span) => [
        span.name,
        {
          hardPerS: meanDemand(planet, profile, span, view),
          calibratedPerS: meanDemand(this.#calibrated, profile, span, view),
        },
      ]),
    );
    this.#metrics = new SpikeMetrics({
      warmupS: SPIKE_WARMUP_S,
      segments: spans,
      levels: Array.from({ length: planet.finestLevel + 1 }, (_, level) => ({
        level,
        epsilonM: levelBoundM(planet, level),
        k: levelRatio(planet, level, view),
      })),
      rowOf,
      predicted: (segment) => predicted.get(segment) ?? { hardPerS: 0, calibratedPerS: 0 },
      firstEngineFrame: this.#firstEngineFrame,
    });
  }

  /** A frame of the run (`SpikeListeners.onFrame`). */
  frame(sample: Omit<FrameSample, "engineFrame" | "patchesCalibrated">): void {
    this.#lastScriptS = sample.scriptTimeS;
    if (!this.#warmupEnded && sample.scriptTimeS >= SPIKE_WARMUP_S) {
      this.#warmupEnded = true;
      this.#tally.endWarmup();
    }
    this.#lastEngineFrame = this.#resolves.value;
    this.#metrics.frame({
      ...sample,
      engineFrame: this.#lastEngineFrame,
      patchesCalibrated: this.#patchesCalibrated,
    });
  }

  /** A selection's inputs (`SpikeListeners.onSelect`): selected again under min(hard, 4σ_n). */
  select(input: SelectionInput): void {
    this.#patchesCalibrated = selectPatches({ ...input, planet: this.#calibrated }).patches.size;
  }

  /** A patch's progress (`SpikeListeners.onPatch`), at the last frame's script time. */
  patch(event: PatchEvent): void {
    if (event === "baked") {
      this.bakedPatches += 1;
    }
    this.#metrics.patches(event, 1, this.#lastScriptS);
  }

  /** The engine's pass times (`RenderEngine.onPassTimes`), which end their read. */
  passTimes(times: PassTimes): void {
    const frame = this.#resolves.runFrame(times.frame);
    this.#reads.ended(frame);
    this.#metrics.passTimes({ ...times, frame });
  }

  /**
   * Waits for the reads of the recorded frames' pass times still in flight, at most `timeoutMs`
   * (R05.T14.k): the run's control does so after the trace's last stop, before the report.
   *
   * @returns Whether every one ended in time.
   */
  readsSettled(timeoutMs: number): Promise<boolean> {
    return this.#reads.settled(this.#firstEngineFrame, this.#lastEngineFrame, timeoutMs);
  }

  /** The engine's allocations (`RenderEngine.onAllocation`): uploads, and the live peak. */
  allocation(event: AllocationEvent): void {
    this.#metrics.allocation(event);
    if (event.kind === "created") {
      this.#liveBytes += event.bytes;
      this.#peakBytes = Math.max(this.#peakBytes, this.#liveBytes);
    } else if (event.kind === "destroyed") {
      this.#liveBytes = Math.max(0, this.#liveBytes - event.bytes);
    }
  }

  /** The run's report for the results file, less what the run's control adds. */
  report(canvas: DescentSpikeReport["canvas"]): SpikeMetricsReport {
    const latePipelines: SpikeLatePipeline[] = this.#tally.late().map((creation) => ({
      label: creation.label.length > 0 ? creation.label : "(unlabelled)",
      kind: creation.kind,
      async: creation.async,
      scriptTimeS: creation.scriptTimeS,
    }));
    return this.#metrics.report({
      inFlightResolves: this.#reads.inFlight(),
      latePipelines,
      adapterPeakBytes: this.#peakBytes,
      canvas,
    });
  }
}
