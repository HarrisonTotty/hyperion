/**
 * The inspection run's frame profile (`--views-check --inspection --timing`; the owner's report,
 * 2026-11-01: a planet with terrain and atmosphere drew at 1 fps or lower): a window of the
 * primary's draws measured from orbit, another closed in and a third low over the ground, where the
 * shadow cascades draw (main's ask), each a trace in the main process and the views probe's figures
 * here, written by the main process under `target/views-check/timing/`.
 *
 * @remarks
 * A window takes {@link InspectionProfileTiming.draws} of the primary's draws, or what it has drawn
 * once {@link InspectionProfileTiming.maxMs} has passed, so that a frame of seconds still ends it;
 * its span is marked in the trace (`performance.measure`), which sets the trace's clock against the
 * page's, and the last frames' pass times are waited for before its figures are read. Nothing is
 * judged: the main process's file is a profile.
 */

import type {
  InspectionTimingMarchCensus,
  InspectionTimingClusters,
  InspectionTimingRecord,
  InspectionTimingWindow,
  InspectionTimingWindowName,
  ViewsCheckApi,
  ViewsCheckWindow,
} from "../../../../../preload/api";
import { TABLE_SIZES } from "../../../view/atmosphere/hillaire";
import { IMAGE_DETAIL_LIMITED } from "../../../view/budget/viewBudget";
import type { QualitySetting } from "../../../view/quality/qualitySetting";
import { PRIMARY_VIEW_NAME } from "../viewNames";
import type { GpuCallCounter } from "./gpuCallCounter";
import type { LongTaskRecorder } from "./longTasks";
import type { ReactCommitCounter } from "./reactCommits";
import type { ViewsProbe } from "./viewsProbe";

/** How long a profiled window lasts. */
export interface InspectionProfileTiming {
  /** The primary's draws a window takes. */
  readonly draws: number;
  /** The longest a window waits for them, ms: past it, it ends with the draws it has. */
  readonly maxMs: number;
  /**
   * The longest the window's frames' pass times are waited for once it ends, ms: until every frame
   * drawn in it is timed, or this passes, since a slow renderer's resolve lands a frame of seconds
   * late.
   */
  readonly settleMs: number;
  /** How often the window looks again at the draws, ms. */
  readonly pollMs: number;
}

/**
 * The profile's timing: 30 draws, or as many as 30 s gives, their times waited for 10 s at most,
 * looked at four times a second, so that its own looks cost the main thread under 1% of a window.
 */
export const INSPECTION_PROFILE_TIMING: InspectionProfileTiming = {
  draws: 30,
  maxMs: 30_000,
  settleMs: 10_000,
  pollMs: 250,
};

/** What the profile measures with. */
export interface InspectionProfileDeps {
  readonly api: Pick<ViewsCheckApi, "startProfile" | "endProfile" | "writeTimingResults">;
  readonly views: Pick<
    ViewsProbe,
    | "drawTimes"
    | "window"
    | "passTimes"
    | "memoryTraffic"
    | "timer"
    | "marchCensus"
    | "atmosphereReads"
    | "engineAtMs"
    | "compiles"
    | "firstFrames"
  >;
  /** Marks a window in the trace as a `performance.measure` span. */
  readonly markWindow: (window: ViewsCheckWindow) => void;
  readonly nowMs: () => number;
  /** A wait, which throws `STOPPED` once the run is stopped (`viewsCheckRun.ts`). */
  readonly sleep: (ms: number) => Promise<void>;
  readonly devicePixelRatio: number;
  /** The launch's quality setting, whose march's samples a window's air takes unless stepped. */
  readonly setting: QualitySetting;
  readonly timing: InspectionProfileTiming;
  /**
   * The counter of the engine's WebGPU calls and its render passes' attachments
   * (`--count-gpu-calls`; plan R14), whose count over each window is the window's, or `null` on a
   * launch that does not count them.
   */
  readonly calls: Pick<GpuCallCounter, "begin" | "take"> | null;
  /**
   * The counter of React's commits (lane perf-render-thread-2), whose count over each window is the
   * window's, or `null` where React did not take its hook.
   */
  readonly react: Pick<ReactCommitCounter, "begin" | "take"> | null;
  /**
   * The primary's terrain's cluster counters (plan R14), switched on by the call, so that the one
   * at a window's start has them read back by its end; `null` where none have come back.
   */
  readonly clusters: () => InspectionTimingClusters | null;
  /**
   * The page's long tasks over the run (lane perf-page-long-tasks), taken as the record is, or
   * `null` where none are recorded.
   */
  readonly longTasks: Pick<LongTaskRecorder, "take"> | null;
}

/** What the primary's label block says as a window ends. */
export interface ProfiledPlate {
  readonly position: string | null;
  readonly statements: ReadonlyArray<string>;
}

/** The inspection run's profiled windows, measured in turn and written at the run's end. */
export class InspectionProfiler {
  readonly #deps: InspectionProfileDeps;
  readonly #windows: InspectionTimingWindow[] = [];

  constructor(deps: InspectionProfileDeps) {
    this.#deps = deps;
  }

  /** The windows measured so far, in order. */
  get windows(): ReadonlyArray<InspectionTimingWindow> {
    return this.#windows;
  }

  /**
   * Measures the window `name` from now: the main process's trace around the primary's next
   * {@link InspectionProfileTiming.draws} draws, or what it draws within
   * {@link InspectionProfileTiming.maxMs}, and the views probe's figures over it.
   *
   * @param plate - What the primary's label block says as the window ends.
   * @throws Error as the main process refuses a window, or as a wait or `plate` throws, the window
   *   then ended with its trace and nothing measured.
   */
  async measure(name: InspectionTimingWindowName, plate: () => ProfiledPlate): Promise<void> {
    const { api, views, timing } = this.#deps;
    await api.startProfile(name);
    // The terrain's counters on from the window's start, read back by its end (plan R14).
    this.#deps.clusters();
    const startMs = this.#deps.nowMs();
    this.#deps.calls?.begin();
    this.#deps.react?.begin();
    const done = (): boolean =>
      views.drawTimes(PRIMARY_VIEW_NAME, startMs, Number.POSITIVE_INFINITY).length >=
        timing.draws || this.#deps.nowMs() - startMs >= timing.maxMs;
    const waited = async (): Promise<void> => {
      if (!done()) {
        await this.#deps.sleep(timing.pollMs);
        await waited();
      }
    };
    let ending = false;
    let endMs: number;
    let gpuCalls: InspectionTimingWindow["gpuCalls"] = null;
    let gpuAttachments: InspectionTimingWindow["gpuAttachments"] = null;
    let react: InspectionTimingWindow["react"] = null;
    let said: ProfiledPlate;
    try {
      await waited();
      endMs = this.#deps.nowMs();
      // Over the window's draws alone, as its figures are, not the settling after them.
      const counted = this.#deps.calls?.take() ?? null;
      gpuCalls = counted?.calls ?? null;
      gpuAttachments = counted?.attachments ?? null;
      react = this.#deps.react?.take() ?? null;
      this.#deps.markWindow({ startMs, endMs });
      said = plate();
      // The last frames' pass times resolve a frame or more late.
      const settleFromMs = this.#deps.nowMs();
      const timed = (): boolean =>
        views.window(startMs, endMs, [{ name: PRIMARY_VIEW_NAME, style: "photorealistic" }])
          .untimedFrames === 0 || this.#deps.nowMs() - settleFromMs >= timing.settleMs;
      const settled = async (): Promise<void> => {
        await this.#deps.sleep(timing.pollMs);
        if (!timed()) {
          await settled();
        }
      };
      await settled();
      ending = true;
      await api.endProfile(name, { startMs, endMs });
    } catch (error: unknown) {
      // A window a step stopped is ended, its trace stopped and dropped, and measures nothing.
      if (!ending) {
        await api
          .endProfile(name, { startMs, endMs: this.#deps.nowMs() })
          .catch((cause: unknown) => {
            console.error(`views check: the profile of ${name} could not be ended:`, cause);
          });
      }
      throw error;
    }
    // Every per-frame figure in one synchronous step, over the same frames: a resolve that reports
    // in a later wait (the census's) would otherwise count in the passes and not the frames' GPU
    // times, and the main process refuse the record (lane fix-inspection-timing-record).
    const figures = views.window(startMs, endMs, [
      { name: PRIMARY_VIEW_NAME, style: "photorealistic" },
    ]);
    const passes = views.passTimes(startMs, endMs);
    const memory = views.memoryTraffic(startMs, endMs);
    const atmosphereReads = views.atmosphereReads(startMs, endMs);
    const primary = figures.views[0];
    const census = await this.#censusAfter(name);
    this.#windows.push({
      name,
      startMs,
      endMs,
      canvas: primary?.canvas ?? { widthPx: 0, heightPx: 0 },
      scales: primary?.scales ?? [],
      intervalsMs: figures.primaryIntervalsMs,
      frameIntervalsMs: figures.frameIntervalsMs,
      mainThreadMs: figures.mainThreadMs,
      submitMs: primary?.submitMs ?? [],
      gpuMs: figures.frameGpuMs,
      passes,
      untimedFrames: figures.untimedFrames,
      memory,
      gpuCalls,
      gpuAttachments,
      atmosphereReads,
      react,
      clusters: this.#deps.clusters(),
      // The floor's air at the low setting's sizes while it stands stepped (the owner's report).
      marchSamples:
        TABLE_SIZES[said.statements.includes(IMAGE_DETAIL_LIMITED) ? "low" : this.#deps.setting]
          .rayMarchSamples,
      position: said.position,
      statements: said.statements,
      marchCensus: census,
    });
  }

  /**
   * The far limb's census of the march the frame after window `name` dispatches (plan R08,
   * R08.T8.d; `ViewsProbe.marchCensus`), outside the window's frames, or `null` where none is
   * taken within {@link InspectionProfileTiming.settleMs}: no march dispatched, or its census
   * failed, which is said.
   */
  async #censusAfter(
    name: InspectionTimingWindowName,
  ): Promise<InspectionTimingMarchCensus | null> {
    const { sleep, nowMs, timing } = this.#deps;
    const stop = new AbortController();
    const asked = this.#deps.views.marchCensus(stop.signal);
    if (asked === null) {
      return null;
    }
    const taken = asked.catch((error: unknown) => {
      console.warn(`views check: the march's census after ${name} failed:`, error);
      return null;
    });
    const startMs = nowMs();
    let waiting = true;
    const timedOut = async (): Promise<null> => {
      if (!waiting || nowMs() - startMs >= timing.settleMs) {
        return null;
      }
      await sleep(timing.pollMs);
      return timedOut();
    };
    try {
      return await Promise.race([taken, timedOut()]);
    } finally {
      waiting = false;
      // A census not taken is withdrawn, so that none is taken inside a later window.
      stop.abort();
    }
  }

  /**
   * The profile's record of the windows measured, with the run's pipeline compiles and the
   * primary's first frames apart from them (plan R08, fix-r08-decked-air-route).
   */
  record(): InspectionTimingRecord {
    const { views } = this.#deps;
    return {
      timer: views.timer(),
      devicePixelRatio: this.#deps.devicePixelRatio,
      windows: this.#windows,
      longTasks: this.#deps.longTasks?.take() ?? null,
      engineAtMs: views.engineAtMs(),
      compiles: views.compiles(),
      firstFrames: views.firstFrames(PRIMARY_VIEW_NAME),
    };
  }

  /** Has the main process write the timing file of the windows measured. */
  async write(): Promise<void> {
    await this.#deps.api.writeTimingResults(this.record());
  }
}
