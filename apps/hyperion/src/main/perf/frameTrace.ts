/**
 * A perf repetition's presentation times from R05's trace (plan R12, R12.T4.b; R05 Design note 18):
 * `spike.ts`'s `SpikeTrace` over the run window's own debugger, recorded in windows the main process
 * cycles on a timer and reduced by `reduceTrace.ts` once the span ends.
 *
 * @remarks
 * R05 cycles its windows at the descent's segment boundaries, which the renderer marks; a perf
 * repetition is measured whole, so its windows are cut every {@link TRACE_WINDOW_MS} instead, each
 * well inside the 768 MiB buffer a window holds (decision-r05-trace-windows.md). The windows are
 * reduced only after the span, so that no reduction's CPU lands in it, and each file is removed once
 * read. A presentation's trace time is the page's `performance.now()` through the window's clock
 * offset; a window without one (no user-timing span on the renderer's main thread) gives none.
 */

import type { TraceFigures } from "../reduceTrace";
import type { TraceWindowStop } from "../spike";
import type { Every } from "./samplers";

/** How long one trace window records before it is cycled, ms: two minutes. */
export const TRACE_WINDOW_MS = 120_000;

/** The trace a {@link WindowedFrameTrace} drives: R05's `SpikeTrace`. */
export interface WindowTrace {
  start(): Promise<void>;
  stop(path: string): Promise<TraceWindowStop>;
  cycle(path: string, stopped: (stop: TraceWindowStop) => void): Promise<void>;
}

/**
 * What a repetition's trace gave: each window's presentation times, page ms, ascending, kept apart
 * since no interval spans the gap a cycle leaves between two windows; or why there are none.
 */
export type FrameTraceResult =
  | { readonly kind: "presented"; readonly windowsMs: ReadonlyArray<ReadonlyArray<number>> }
  | { readonly kind: "none"; readonly reason: string };

/** What a {@link WindowedFrameTrace} is given. */
export interface WindowedFrameTraceOptions {
  readonly trace: WindowTrace;
  /** Reduces a window's file (`reduceTraceFile` with the trace's categories). */
  readonly reduce: (path: string) => Promise<TraceFigures>;
  /** The file of the `n`th window, from 0, of repetition `repetition`. */
  readonly path: (repetition: number, n: number) => string;
  readonly removeFile: (path: string) => Promise<void>;
  readonly every: Every;
  /** {@link TRACE_WINDOW_MS} outside a test. */
  readonly windowMs: number;
  readonly log: (line: string) => void;
}

/** One repetition's trace at a time, in windows. */
export class WindowedFrameTrace {
  readonly #options: WindowedFrameTraceOptions;
  #repetition = 0;
  #windows: string[] = [];
  #lostData = false;
  #cancel: (() => void) | null = null;
  /** The cycle in flight, if any, which a stop waits for. */
  #cycling: Promise<void> = Promise.resolve();
  #failure: string | null = null;

  constructor(options: WindowedFrameTraceOptions) {
    this.#options = options;
  }

  /** Starts repetition `repetition`'s trace. */
  async start(repetition: number): Promise<void> {
    this.#repetition = repetition;
    this.#windows = [];
    this.#lostData = false;
    this.#failure = null;
    await this.#options.trace.start();
    this.#cancel = this.#options.every(() => {
      this.#cycling = this.#cycling.then(() => this.#cycle());
    }, this.#options.windowMs);
  }

  /** Cycles to the next window, keeping the one just written. */
  async #cycle(): Promise<void> {
    if (this.#failure !== null) {
      return;
    }
    const path = this.#options.path(this.#repetition, this.#windows.length);
    try {
      await this.#options.trace.cycle(path, (stop) => {
        this.#windows.push(path);
        this.#lostData ||= stop.lostData;
      });
    } catch (error: unknown) {
      this.#failure = error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * A window's presentation times, page ms, or `null` where it gives none (no clock offset, or it
   * could not be read); its file is removed either way.
   */
  async #presentations(window: string): Promise<ReadonlyArray<number> | null> {
    const { reduce, removeFile, log } = this.#options;
    try {
      const figures = await reduce(window);
      const offsetUs = figures.clockOffsetUs;
      return offsetUs === null
        ? null
        : figures.frames.presentedAtUs.map((us) => (us - offsetUs) / 1000);
    } catch (error: unknown) {
      log(`perf run: a trace window could not be reduced: ${String(error)}`);
      return null;
    } finally {
      await removeFile(window).catch((error: unknown) => {
        log(`perf run: a trace window could not be removed: ${String(error)}`);
      });
    }
  }

  /** Stops the repetition's trace and reduces its windows to their presentation times. */
  async stop(): Promise<FrameTraceResult> {
    this.#cancel?.();
    this.#cancel = null;
    await this.#cycling;
    const { trace, path, log } = this.#options;
    if (this.#failure === null) {
      const last = path(this.#repetition, this.#windows.length);
      try {
        const stop = await trace.stop(last);
        this.#windows.push(last);
        this.#lostData ||= stop.lostData;
      } catch (error: unknown) {
        this.#failure = error instanceof Error ? error.message : String(error);
      }
    }
    // One window at a time, each file removed once read, so that the reductions never hold two.
    const windowsMs = await this.#windows.reduce<Promise<Array<ReadonlyArray<number>>>>(
      async (chain, window) => {
        const done = await chain;
        const presented = await this.#presentations(window);
        return presented === null ? done : [...done, presented];
      },
      Promise.resolve([]),
    );
    if (this.#failure !== null) {
      return { kind: "none", reason: `the trace failed: ${this.#failure}` };
    }
    if (this.#lostData) {
      log("perf run: Chromium lost some of a trace window's data; its presentations are partial");
    }
    return windowsMs.some((each) => each.length >= 2)
      ? { kind: "presented", windowsMs }
      : { kind: "none", reason: "no trace window holds two presentations" };
  }
}
