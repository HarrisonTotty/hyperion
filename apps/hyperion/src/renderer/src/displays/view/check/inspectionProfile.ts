/**
 * The inspection run's frame profile (`--views-check --inspection --timing`; the owner's report,
 * 2026-11-01: a planet with terrain and atmosphere drew at 1 fps or lower): a window of the
 * primary's draws measured from orbit and another closed in, each a trace in the main process and
 * the views probe's figures here, written by the main process under `target/views-check/timing/`.
 *
 * @remarks
 * A window takes {@link InspectionProfileTiming.draws} of the primary's draws, or what it has drawn
 * once {@link InspectionProfileTiming.maxMs} has passed, so that a frame of seconds still ends it;
 * its span is marked in the trace (`performance.measure`), which sets the trace's clock against the
 * page's, and the last frames' pass times are waited for before its figures are read. Nothing is
 * judged: the main process's file is a profile.
 */

import type {
  InspectionTimingRecord,
  InspectionTimingWindow,
  InspectionTimingWindowName,
  ViewsCheckApi,
  ViewsCheckWindow,
} from "../../../../../preload/api";
import { PRIMARY_VIEW_NAME } from "../viewNames";
import type { ViewsProbe } from "./viewsProbe";

/** How long a profiled window lasts. */
export interface InspectionProfileTiming {
  /** The primary's draws a window takes. */
  readonly draws: number;
  /** The longest a window waits for them, ms: past it, it ends with the draws it has. */
  readonly maxMs: number;
  /** How long the window's last frames' pass times are waited for once it ends, ms. */
  readonly settleMs: number;
  /** How often the window looks again at the draws, ms. */
  readonly pollMs: number;
}

/** The profile's timing: 30 draws, or as many as 30 s gives, the last frames' times a second. */
export const INSPECTION_PROFILE_TIMING: InspectionProfileTiming = {
  draws: 30,
  maxMs: 30_000,
  settleMs: 1_000,
  pollMs: 100,
};

/** What the profile measures with. */
export interface InspectionProfileDeps {
  readonly api: Pick<ViewsCheckApi, "startProfile" | "endProfile" | "writeTimingResults">;
  readonly views: Pick<
    ViewsProbe,
    "drawTimes" | "window" | "passTimes" | "memoryTraffic" | "timer"
  >;
  /** Marks a window in the trace as a `performance.measure` span. */
  readonly markWindow: (window: ViewsCheckWindow) => void;
  readonly nowMs: () => number;
  /** A wait, which throws `STOPPED` once the run is stopped (`viewsCheckRun.ts`). */
  readonly sleep: (ms: number) => Promise<void>;
  readonly devicePixelRatio: number;
  readonly timing: InspectionProfileTiming;
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
   * @throws Error as the main process refuses a window, or as a wait is stopped.
   */
  async measure(name: InspectionTimingWindowName, plate: () => ProfiledPlate): Promise<void> {
    const { api, views, timing } = this.#deps;
    await api.startProfile(name);
    const startMs = this.#deps.nowMs();
    const done = (): boolean =>
      views.drawTimes(PRIMARY_VIEW_NAME, startMs, Number.POSITIVE_INFINITY).length >=
        timing.draws || this.#deps.nowMs() - startMs >= timing.maxMs;
    const waited = async (): Promise<void> => {
      if (!done()) {
        await this.#deps.sleep(timing.pollMs);
        await waited();
      }
    };
    await waited();
    const endMs = this.#deps.nowMs();
    const span = { startMs, endMs };
    this.#deps.markWindow(span);
    const said = plate();
    // The last frames' pass times resolve a frame or more late.
    await this.#deps.sleep(timing.settleMs);
    await api.endProfile(name, span);
    const figures = views.window(startMs, endMs, [
      { name: PRIMARY_VIEW_NAME, style: "photorealistic" },
    ]);
    const primary = figures.views[0];
    this.#windows.push({
      name,
      startMs,
      endMs,
      canvas: primary?.canvas ?? { widthPx: 0, heightPx: 0 },
      scales: primary?.scales ?? [],
      intervalsMs: figures.primaryIntervalsMs,
      mainThreadMs: figures.mainThreadMs,
      submitMs: primary?.submitMs ?? [],
      gpuMs: figures.frameGpuMs,
      passes: views.passTimes(startMs, endMs),
      untimedFrames: figures.untimedFrames,
      memory: views.memoryTraffic(startMs, endMs),
      position: said.position,
      statements: said.statements,
    });
  }

  /** The profile's record of the windows measured. */
  record(): InspectionTimingRecord {
    return {
      timer: this.#deps.views.timer(),
      devicePixelRatio: this.#deps.devicePixelRatio,
      windows: this.#windows,
    };
  }

  /** Has the main process write the timing file of the windows measured. */
  async write(): Promise<void> {
    await this.#deps.api.writeTimingResults(this.record());
  }
}
