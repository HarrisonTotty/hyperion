/**
 * The inspection profile's record of the page's long tasks (`--inspection --timing`; lane
 * perf-page-long-tasks): every task of the page's main thread of 50 ms or more over the run, as
 * Chromium's `longtask` entries report them, each with the share of it its frame's rendering update
 * took, so that a profile counts what stalls the page outside the windows' frames as well as in
 * them. The main process names what ran in each from its CPU profile of the page's main thread
 * (`pageProfile.ts`).
 *
 * @remarks
 * Chromium reports a task of 50 ms or more as a `longtask` entry, its start and length alone, and
 * an animation frame of 50 ms or more, its tasks and its rendering update (the animation frame's
 * callbacks, the draw loop's among them, then style, layout and paint), as a `long-animation-frame`
 * entry. A task takes its frame's rendering update where that began within it. Both are observed
 * with the entries buffered before the observer started. The frames' script attribution is not
 * read: Chromium lists no script for a page of an opaque origin, as the app's `file:` page is
 * (none on a probe's 90 to 120 ms handlers). Entries are read at a trust boundary, field by field:
 * one not shaped as expected counts nothing.
 */

import type { InspectionLongTask, InspectionLongTasks } from "../../../../../preload/api";

/** The entry types the recorder observes. */
export type LongTaskEntryType = "longtask" | "long-animation-frame";

/** Where the recorder's entries come from: the page's `PerformanceObserver`, or a test's. */
export interface LongTaskSource {
  /** Whether the page reports entries of `type`. */
  supports(type: LongTaskEntryType): boolean;
  /**
   * Observes the entries of `type`, those buffered before it included, handing each batch to
   * `onEntries`.
   *
   * @returns Its stop.
   */
  observe(
    type: LongTaskEntryType,
    onEntries: (entries: ReadonlyArray<unknown>) => void,
  ): () => void;
}

/** The page's own source of entries, over its `PerformanceObserver`. */
export function pageLongTaskSource(): LongTaskSource {
  return {
    supports: (type) => PerformanceObserver.supportedEntryTypes.includes(type),
    observe: (type, onEntries) => {
      const observer = new PerformanceObserver((list) => {
        onEntries(list.getEntries());
      });
      observer.observe({ type, buffered: true });
      return () => {
        observer.disconnect();
      };
    },
  };
}

/** A `long-animation-frame` entry, as far as the recorder reads it. */
export interface FrameEntry {
  readonly startMs: number;
  readonly endMs: number;
  /** Its rendering update's start, ms, or `null` where the frame rendered nothing. */
  readonly renderStartMs: number | null;
}

function numberField(entry: unknown, key: string): number | null {
  const value: unknown =
    typeof entry === "object" && entry !== null ? Reflect.get(entry, key) : undefined;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/** A `long-animation-frame` entry, or `null` where it is not one. */
function frameOf(entry: unknown): FrameEntry | null {
  const startMs = numberField(entry, "startTime");
  const durationMs = numberField(entry, "duration");
  if (startMs === null || durationMs === null) {
    return null;
  }
  const renderStartMs = numberField(entry, "renderStart");
  return {
    startMs,
    endMs: startMs + durationMs,
    renderStartMs: renderStartMs !== null && renderStartMs > 0 ? renderStartMs : null,
  };
}

/**
 * One long task with its frame's rendering update where that began within it, to the frame's end
 * or the task's, whichever is first; 0 where none did.
 */
export function longTaskOf(
  startMs: number,
  durationMs: number,
  frames: ReadonlyArray<FrameEntry>,
): InspectionLongTask {
  const endMs = startMs + durationMs;
  const frame = frames.find(
    (each) =>
      each.renderStartMs !== null &&
      each.renderStartMs >= startMs &&
      each.renderStartMs < endMs &&
      each.startMs < endMs,
  );
  // Held within the task's length, which `endMs − renderStartMs` passes by an ulp where the
  // rendering starts with the task, and above 0, which a frame's end, its duration coarsened to whole
  // milliseconds, can fall under: the main process refuses a task's rendering outside them.
  const renderMs =
    frame === undefined || frame.renderStartMs === null
      ? 0
      : Math.min(durationMs, Math.max(0, Math.min(frame.endMs, endMs) - frame.renderStartMs));
  return { startMs, durationMs, renderMs };
}

/** The page's long tasks over the inspection run, recorded from {@link LongTaskRecorder.start}. */
export class LongTaskRecorder {
  readonly #source: LongTaskSource;
  readonly #nowMs: () => number;
  readonly #tasks: Array<{ readonly startMs: number; readonly durationMs: number }> = [];
  readonly #frames: FrameEntry[] = [];
  #stops: Array<() => void> = [];
  #fromMs: number | null = null;

  constructor(source: LongTaskSource, nowMs: () => number) {
    this.#source = source;
    this.#nowMs = nowMs;
  }

  /** Starts observing, with the entries buffered before; a second start does nothing. */
  start(): void {
    if (this.#fromMs !== null) {
      return;
    }
    this.#fromMs = this.#nowMs();
    if (this.#source.supports("longtask")) {
      this.#stops.push(
        this.#source.observe("longtask", (entries) => {
          for (const entry of entries) {
            const startMs = numberField(entry, "startTime");
            const durationMs = numberField(entry, "duration");
            if (startMs !== null && durationMs !== null) {
              this.#tasks.push({ startMs, durationMs });
            }
          }
        }),
      );
    }
    if (this.#source.supports("long-animation-frame")) {
      this.#stops.push(
        this.#source.observe("long-animation-frame", (entries) => {
          for (const entry of entries) {
            const frame = frameOf(entry);
            if (frame !== null) {
              this.#frames.push(frame);
            }
          }
        }),
      );
    }
  }

  /** Stops observing. */
  stop(): void {
    for (const stop of this.#stops) {
      stop();
    }
    this.#stops = [];
  }

  /** The long tasks recorded so far, in order, each with its frame's rendering update. */
  take(): InspectionLongTasks {
    const frames = this.#frames.toSorted((a, b) => a.startMs - b.startMs);
    return {
      observed: {
        longtask: this.#source.supports("longtask"),
        longAnimationFrame: this.#source.supports("long-animation-frame"),
      },
      fromMs: this.#fromMs ?? this.#nowMs(),
      toMs: this.#nowMs(),
      tasks: this.#tasks
        .toSorted((a, b) => a.startMs - b.startMs)
        .map((task) => longTaskOf(task.startMs, task.durationMs, frames)),
    };
  }
}
