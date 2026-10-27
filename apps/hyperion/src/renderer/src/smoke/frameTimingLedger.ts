/**
 * The bookkeeping of R08.T11's frame timings (`frameTiming.ts`), with no GPU of its own: the timer's
 * resolves sorted to the frames that made them ({@link ResolveLedger}), a frame's passes once they
 * have all arrived ({@link timedFrameOf}), the atmosphere's view passes by their place
 * ({@link viewPassesOf}), and QW2's and QW3's timed form ({@link runTimed},
 * `decision-r08-t8-leans.md` §10).
 *
 * @remarks
 * Each submission resolves its own timestamps, numbered from 1 as it is made
 * (`RenderEngine.passTimesFrame`), and reports them a frame or more after its work, in any order
 * (`RenderEngine.onPassTimes`): a frame's resolves are those numbered after the count before its
 * draw up to the count after it. A resolve the timer dropped is never reported, so a frame waits
 * for its resolves only so long, and one with any missing is left out of its combination's figures
 * and counted.
 */

import { ATMOSPHERE_VIEW_PASS } from "../view/atmosphere/hillaire";
import type { GpuTimer } from "../view/engine/status";
import type { PassTimes, RenderEngine } from "../view/engine/types";

/** What the ledger needs of the engine: its timer's count and its reports. */
export type TimerEngine = Pick<RenderEngine, "passTimesFrame" | "onPassTimes">;

/** QW2's and QW3's form: each combination's frames a sweep, and the waits for the timer. */
export interface TimedForm {
  /** The frames a combination draws before its timed ones, a sweep. */
  readonly warmUp: number;
  /** A combination's timed frames a sweep. */
  readonly timed: number;
  /** The frames in flight: before frame n is drawn, frame n − `ahead`'s resolves have arrived. */
  readonly ahead: number;
  /** How long one frame's resolves are waited for before it counts as missing, ms. */
  readonly waitMs: number;
  /** How long the last timed frames' resolves are waited for in all, ms. */
  readonly lastWaitMs: number;
}

/**
 * The timings' form: 10 warm-up frames and 30 timed a combination a sweep, 60 timed over the two
 * sweeps, each frame drawn once frame n − 3's resolves have arrived, so that two frames stay queued
 * on the GPU and the resolves in flight stay a few frames' (a photorealistic frame makes about a
 * dozen), far under the timer's 135 (`TIMING_FRAMES_IN_FLIGHT`); a frame's resolves waited for
 * 2 s, the last frames' 5 s in all.
 */
export const QW_FORM: TimedForm = {
  warmUp: 10,
  timed: 30,
  ahead: 3,
  waitMs: 2_000,
  lastWaitMs: 5_000,
};

/** The timed frames a combination has in all under {@link QW_FORM}: two sweeps' worth. */
export const TIMED_IN_ALL = 2 * QW_FORM.timed;

/** One frame's submissions as the timer numbers and reports them. */
export interface FrameRecord {
  /** Its first resolve's number, `passTimesFrame` before it + 1. */
  readonly first: number;
  /** Its last resolve's number, `passTimesFrame` after it; below `first` where it made none. */
  readonly last: number;
  /** Its resolves as they arrive, by number − `first`. */
  readonly resolves: Array<PassTimes | undefined>;
  /** The main thread's time in its measured call, ms. */
  readonly cpuMs: number;
  /** Its resolves arrived so far. */
  arrived: number;
  /** Wakes the one waiter on it, once it is complete. */
  wake: (() => void) | null;
}

/** Whether every resolve of `record` has arrived. */
function complete(record: FrameRecord): boolean {
  return record.arrived === record.resolves.length;
}

/**
 * The timer's resolves sorted to the frames that made them: a frame's are those numbered after
 * the count before its submissions up to the count after them (`RenderEngine.passTimesFrame`). A
 * frame is registered in the same task as its submissions, before any of their resolves can
 * arrive, so a resolve no frame owns is another submission's (a fence's, a scene's) and is passed
 * over.
 */
export class ResolveLedger {
  readonly #engine: TimerEngine;
  readonly #owners = new Map<number, FrameRecord>();
  readonly #stop: () => void;

  constructor(engine: TimerEngine) {
    this.#engine = engine;
    this.#stop = engine.onPassTimes((times) => {
      this.#take(times);
    });
  }

  #take(times: PassTimes): void {
    const record = this.#owners.get(times.frame);
    if (record === undefined) {
      return;
    }
    this.#owners.delete(times.frame);
    record.resolves[times.frame - record.first] = times;
    record.arrived += 1;
    if (complete(record)) {
      record.wake?.();
      record.wake = null;
    }
  }

  /** The timer's count now, before a frame's submissions. */
  get count(): number {
    return this.#engine.passTimesFrame;
  }

  /**
   * Registers the resolves made since the count was `before` as one frame's.
   *
   * @param cpuMs - The main thread's time in the frame's measured call, ms.
   */
  frame(before: number, cpuMs: number): FrameRecord {
    const last = this.#engine.passTimesFrame;
    const record: FrameRecord = {
      first: before + 1,
      last,
      resolves: Array.from({ length: Math.max(0, last - before) }, () => undefined),
      cpuMs,
      arrived: 0,
      wake: null,
    };
    for (let k = record.first; k <= last; k += 1) {
      this.#owners.set(k, record);
    }
    return record;
  }

  /**
   * Settles once every resolve of `record` has arrived, or `limitMs` after.
   *
   * @returns Whether they had all arrived.
   */
  async settled(record: FrameRecord, limitMs: number): Promise<boolean> {
    if (!complete(record)) {
      await new Promise<void>((resolve) => {
        const timer = setTimeout(
          () => {
            record.wake = null;
            resolve();
          },
          Math.max(0, limitMs),
        );
        record.wake = () => {
          clearTimeout(timer);
          resolve();
        };
      });
    }
    return complete(record);
  }

  /** Stops listening; the frames still waiting count as missing. */
  dispose(): void {
    this.#stop();
    this.#owners.clear();
  }
}

/** One pass of a resolve: its label and its time, ns. */
export interface PassTime {
  readonly label: string;
  readonly ns: number;
}

/** A timed frame that arrived whole: its passes in submission order and its main-thread time. */
export interface TimedFrame {
  readonly passes: ReadonlyArray<PassTime>;
  /** The main thread's time in its measured call, ms. */
  readonly cpuMs: number;
  /** Each of its resolves' timer kinds. */
  readonly timers: ReadonlyArray<GpuTimer>;
}

/** A frame's passes in order, its resolves' in turn, or `null` where it has a resolve missing or none. */
export function timedFrameOf(record: FrameRecord): TimedFrame | null {
  if (record.resolves.length === 0 || !complete(record)) {
    return null;
  }
  const passes: PassTime[] = [];
  const timers: GpuTimer[] = [];
  for (const times of record.resolves) {
    if (times === undefined) {
      return null;
    }
    timers.push(times.timer);
    passes.push(...times.passes.map(({ label, ns }) => ({ label, ns })));
  }
  return { passes, cpuMs: record.cpuMs, timers };
}

/** A frame's GPU pass sum, ns. */
export function sumNs(frame: TimedFrame): number {
  return frame.passes.reduce((sum, pass) => sum + pass.ns, 0);
}

/** A frame's passes under `labels`, summed, ns. */
export function labelledNs(frame: TimedFrame, labels: ReadonlySet<string>): number {
  return frame.passes.reduce((sum, pass) => sum + (labels.has(pass.label) ? pass.ns : 0), 0);
}

/** Whether every pass time of every frame is finite and not negative. */
export function framesFinite(frames: ReadonlyArray<TimedFrame>): boolean {
  return frames.every((frame) => frame.passes.every((p) => Number.isFinite(p.ns) && p.ns >= 0));
}

/** A frame's atmosphere view passes by their place, ns. */
export interface ViewPasses {
  readonly skyNs: number;
  readonly aerialNs: number;
  /** The ray march's passes, summed: since R08.T11.d its two entry points, the centre ray's and the footprint's. */
  readonly marchNs: number;
  /** Any view passes after the ray march's two, summed: none today. */
  readonly moreNs: number;
}

/**
 * A frame's atmosphere view passes (`ATMOSPHERE_VIEW_PASS`) by their place: the sky view, the aerial
 * perspective and the ray march's two (the centre ray's and the footprint's, R08.T11.d's tile
 * cull; the footprint's alone over the whole frame), in their order, then any more; one or two
 * passes are the ray march's, the frame having left the other two unread (from the march band's
 * upper edge up, R08.T8.d's lever 2).
 */
export function viewPassesOf(frame: TimedFrame): ViewPasses {
  const views = frame.passes.filter((pass) => pass.label === ATMOSPHERE_VIEW_PASS).map((p) => p.ns);
  const [sky = 0, aerial = 0, ...rest] = views.length <= 2 ? [0, 0, ...views] : views;
  const march = rest.slice(0, 2).reduce((a, b) => a + b, 0);
  return {
    skyNs: sky,
    aerialNs: aerial,
    marchNs: march,
    moreNs: rest.slice(2).reduce((a, b) => a + b, 0),
  };
}

/** A combination timed: what it draws, and its timed frames' records in timed order. */
export interface Timed<T> {
  readonly item: T;
  readonly records: FrameRecord[];
}

/** A combination's timed frames that arrived whole, and how many did not. */
export function keptOf<T>(combination: Timed<T>): {
  readonly combination: Timed<T>;
  readonly frames: TimedFrame[];
  readonly missing: number;
} {
  const frames: TimedFrame[] = [];
  for (const record of combination.records) {
    const frame = timedFrameOf(record);
    if (frame !== null) {
      frames.push(frame);
    }
  }
  return { combination, frames, missing: combination.records.length - frames.length };
}

/** A timed combination's per-frame sums, µs, in timed order, `-` for a frame left out. */
export function perFrameSums(combination: Timed<unknown>): string {
  return combination.records
    .map((record) => {
      const frame = timedFrameOf(record);
      return frame === null ? "-" : String(Math.round(sumNs(frame) / 1_000));
    })
    .join(" ");
}

/**
 * Draws `combinations` in `form` (QW2's: the module's remarks): two sweeps, the second reversed,
 * each combination's warm-up frames then its timed ones back to back, every frame drawn as soon as
 * the one `form.ahead` before it has resolved, `setUp` called before a combination's first frame of
 * each sweep; then waits for the last timed frames' resolves. Each timed frame's record goes to its
 * combination's `records`, in timed order.
 *
 * @param draw - Draws one frame of a combination and returns the main thread's time in the call
 *   the run measures, ms.
 */
export async function runTimed<T>(
  ledger: ResolveLedger,
  combinations: ReadonlyArray<Timed<T>>,
  setUp: (item: T) => void,
  draw: (item: T) => number,
  form: TimedForm = QW_FORM,
): Promise<void> {
  const order = combinations.map((_, i) => i);
  const frames = [...order, ...order.toReversed()].flatMap((i) =>
    Array.from({ length: form.warmUp + form.timed }, (_, k) => ({ i, k })),
  );
  const recent: FrameRecord[] = [];
  for (const { i, k } of frames) {
    const combination = combinations[i];
    if (combination === undefined) {
      continue;
    }
    if (k === 0) {
      setUp(combination.item);
    }
    const ahead = recent.length >= form.ahead ? recent[recent.length - form.ahead] : undefined;
    if (ahead !== undefined) {
      // The run's one wait: frame n − 3's resolves, so that two frames stay queued.
      // oxlint-disable-next-line no-await-in-loop
      await ledger.settled(ahead, form.waitMs);
    }
    const before = ledger.count;
    const cpuMs = draw(combination.item);
    const record = ledger.frame(before, cpuMs);
    recent.push(record);
    if (recent.length > form.ahead) {
      recent.shift();
    }
    if (k >= form.warmUp) {
      combination.records.push(record);
    }
  }
  const deadline = performance.now() + form.lastWaitMs;
  for (const record of combinations.flatMap((combination) => combination.records)) {
    // The last frames' resolves, a frame or more after their work, within one deadline.
    // oxlint-disable-next-line no-await-in-loop
    await ledger.settled(record, deadline - performance.now());
  }
}
