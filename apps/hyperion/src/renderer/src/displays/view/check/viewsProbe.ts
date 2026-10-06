/**
 * What the several-views check measures of `VIEW` as it draws (plan R07, T20): each animation
 * frame's main-thread time, each view's resolves and their GPU time, the canvases' sizes, every
 * allocation and every fault.
 *
 * @remarks
 * The probe sits between `VIEW` and its engine ({@link ViewsProbe.source}, through
 * {@link ProbedEngine}) and around the page's `requestAnimationFrame`
 * ({@link ViewsProbe.installFrameClock}), so that `VIEW` itself is the one every launch runs. Every
 * callback of one animation frame is given the same timestamp, which keys the frame: a resolve
 * belongs to the frame whose callback made it, and a view drew in a frame when one of its renders
 * did. A view's name is its engine name (`view`, `instrument-1`), and a target belongs to the view
 * its name begins with, up to a `:` or a space (`view:hdr`, `instrument-1 blue noise`).
 *
 * Only the check's launch makes a probe; nothing else reads it.
 */

import type {
  ViewsCheckCanvas,
  ViewsCheckPhaseName,
  ViewsCheckPhaseRecord,
  ViewsCheckStyle,
  ViewsCheckViewName,
  ViewsCheckViewRecord,
} from "../../../../../preload/api";
import type { GpuTimer, GraphicsFault } from "../../../view/engine/status";
import type { PassTimes, RenderEngine, ViewSize } from "../../../view/engine/types";
import { DEFAULT_ENGINE_SOURCE, type ViewEngineSource } from "../useViewEngine";
import { type EngineProbe, ProbedEngine } from "./probedEngine";

/** The view a view's or target's name belongs to: the name up to its first `:` or space. */
export function viewOf(owner: string): string {
  return owner.split(/[: ]/u)[0] ?? owner;
}

/** The part of `window` whose animation frames the probe times. */
export interface FrameClockTarget {
  requestAnimationFrame: (callback: FrameRequestCallback) => number;
}

/** A resolve a view's or target's render took. */
interface ResolveEntry {
  readonly view: string;
  /** The animation frame it was made in, or `null` outside any. */
  readonly frameMs: number | null;
}

/** One view's part of an animation frame. */
interface FrameView {
  submitMs: number;
  readonly resolves: string[];
}

/** One animation frame, keyed by its timestamp. */
interface FrameEntry {
  readonly atMs: number;
  mainMs: number;
  readonly views: Map<string, FrameView>;
}

/** A view's scene target given a size: its scale against the view's canvas then. */
interface ScaleEntry {
  readonly atMs: number;
  readonly view: string;
  readonly scale: number;
}

/** A view as the check's driver saw it when a phase ended. */
export interface ShownView {
  readonly name: ViewsCheckViewName;
  readonly style: ViewsCheckStyle;
}

/** An allocation event, timed. */
export interface TimedAllocation {
  readonly atMs: number;
  readonly kind: "created" | "destroyed";
  readonly name: string;
}

/** Records `VIEW`'s frames, resolves, sizes, allocations and faults for the several-views check. */
export class ViewsProbe implements EngineProbe {
  /** The engine source `VIEW` is given: `inner`'s engines, each wrapped in a {@link ProbedEngine}. */
  readonly source: ViewEngineSource;
  readonly #nowMs: () => number;
  /** The timestamp of the animation frame whose callback is running, or `null` between them. */
  #frameMs: number | null = null;
  readonly #frames = new Map<number, FrameEntry>();
  readonly #resolves = new Map<string, ResolveEntry>();
  readonly #times = new Map<string, PassTimes>();
  readonly #allocations: TimedAllocation[] = [];
  readonly #faults: string[] = [];
  readonly #sizes = new Map<string, ViewSize>();
  readonly #scales: ScaleEntry[] = [];
  #timer: GpuTimer | null = null;
  /** Bumped for each engine and restore, since each numbers its resolves from 1 again. */
  #generation = 0;

  /**
   * @param inner - Where the engines come from: R01's by default.
   * @param nowMs - The page's clock, `performance.now()`, on which animation frames are stamped.
   */
  constructor(
    inner: ViewEngineSource = DEFAULT_ENGINE_SOURCE,
    nowMs: () => number = () => performance.now(),
  ) {
    this.#nowMs = nowMs;
    this.source = {
      requestAdapter: inner.requestAdapter,
      load: async (outcome, status) => this.#attach(await inner.load(outcome, status)),
    };
  }

  /**
   * Times every animation frame's callbacks on `target` (the page's `window`) and keys the renders
   * made in them by the frame's timestamp, until the returned function puts the original back.
   */
  installFrameClock(target: FrameClockTarget): () => void {
    const original = target.requestAnimationFrame;
    target.requestAnimationFrame = (callback) =>
      original.call(target, (timeMs) => {
        const outer = this.#frameMs;
        this.#frameMs = timeMs;
        const startMs = this.#nowMs();
        try {
          callback(timeMs);
        } finally {
          this.#frameAt(timeMs).mainMs += this.#nowMs() - startMs;
          this.#frameMs = outer;
        }
      });
    return () => {
      target.requestAnimationFrame = original;
    };
  }

  rendered(owner: string, firstResolve: number, lastResolve: number, cpuMs: number): void {
    const view = viewOf(owner);
    const frameMs = this.#frameMs;
    const keys: string[] = [];
    for (let n = firstResolve + 1; n <= lastResolve; n += 1) {
      const key = this.#key(n);
      this.#resolves.set(key, { view, frameMs });
      keys.push(key);
    }
    if (frameMs === null) {
      return;
    }
    const frame = this.#frameAt(frameMs);
    const part = frame.views.get(view) ?? { submitMs: 0, resolves: [] };
    part.submitMs += cpuMs;
    part.resolves.push(...keys);
    frame.views.set(view, part);
  }

  sized(owner: string, kind: "view" | "target", size: ViewSize): void {
    const last = this.#sizes.get(owner);
    if (last !== undefined && last.widthPx === size.widthPx && last.heightPx === size.heightPx) {
      return;
    }
    this.#sizes.set(owner, size);
    const view = viewOf(owner);
    const canvas = this.#sizes.get(view);
    if (
      kind === "target" &&
      owner === `${view}:hdr` &&
      canvas !== undefined &&
      canvas.widthPx > 0
    ) {
      this.#scales.push({ atMs: this.#nowMs(), view, scale: size.widthPx / canvas.widthPx });
    }
  }

  /** A view's canvas as it was last sized, or 0 × 0 before it was. */
  canvasOf(view: string): ViewsCheckCanvas {
    const size = this.#sizes.get(view);
    return { widthPx: size?.widthPx ?? 0, heightPx: size?.heightPx ?? 0 };
  }

  /** The longest main-thread time of the animation frames stamped from `startMs` to `endMs`, ms. */
  longestFrameMs(startMs: number, endMs: number): number {
    let longest = 0;
    for (const frame of this.#frames.values()) {
      if (frame.atMs >= startMs && frame.atMs < endMs) {
        longest = Math.max(longest, frame.mainMs);
      }
    }
    return longest;
  }

  /** The allocations so far, for {@link allocationsSince}. */
  allocationMark(): number {
    return this.#allocations.length;
  }

  /** The allocations after `mark`, in order. */
  allocationsSince(mark: number): ReadonlyArray<TimedAllocation> {
    return this.#allocations.slice(mark);
  }

  /** Every fault so far, as a line each. */
  faults(): ReadonlyArray<string> {
    return [...this.#faults];
  }

  /** The pass timer's kind, from its reports; `absent` while none has arrived. */
  timer(): GpuTimer {
    return this.#timer ?? "absent";
  }

  /**
   * The figures of the animation frames stamped from `startMs` up to `endMs`, for the phase
   * `name` whose views were `shown`.
   *
   * @remarks
   * A frame's GPU time counts once every resolve made in it has reported; a resolve still
   * unreported is the pass timer's drop (a number taken and never reported), so a caller waits a
   * moment after `endMs` for the last reads before asking.
   */
  phase(
    name: ViewsCheckPhaseName,
    startMs: number,
    endMs: number,
    shown: ReadonlyArray<ShownView>,
  ): ViewsCheckPhaseRecord {
    const frames = [...this.#frames.values()]
      .filter((frame) => frame.atMs >= startMs && frame.atMs < endMs)
      .toSorted((a, b) => a.atMs - b.atMs);
    const gpuOf = (keys: ReadonlyArray<string>): number | null => {
      let ns = 0;
      for (const key of keys) {
        const times = this.#times.get(key);
        if (times === undefined) {
          return null;
        }
        ns += times.passes.reduce((sum, pass) => sum + pass.ns, 0);
      }
      return ns / 1e6;
    };
    const frameGpuMs: number[] = [];
    let untimedFrames = 0;
    const made = new Set<string>();
    for (const frame of frames) {
      const keys = [...frame.views.values()].flatMap((part) => part.resolves);
      if (keys.length === 0) {
        continue;
      }
      for (const key of keys) {
        made.add(key);
      }
      const gpuMs = gpuOf(keys);
      if (gpuMs === null) {
        untimedFrames += 1;
      } else {
        frameGpuMs.push(gpuMs);
      }
    }
    const droppedResolves = [...made].filter((key) => !this.#times.has(key)).length;
    const views = shown.map((view): ViewsCheckViewRecord => {
      const drawn = frames.flatMap((frame) => {
        const part = frame.views.get(view.name);
        return part === undefined ? [] : [part];
      });
      const labels = new Set<string>();
      for (const key of drawn.flatMap((part) => part.resolves)) {
        for (const pass of this.#times.get(key)?.passes ?? []) {
          labels.add(pass.label);
        }
      }
      return {
        name: view.name,
        style: view.style,
        canvas: this.canvasOf(view.name),
        draws: drawn.length,
        gpuMs: drawn.flatMap((part) => {
          const gpuMs = part.resolves.length === 0 ? null : gpuOf(part.resolves);
          return gpuMs === null ? [] : [gpuMs];
        }),
        submitMs: drawn.map((part) => part.submitMs),
        passLabels: [...labels].toSorted(),
        scales: this.#scalesOf(view.name, startMs, endMs),
      };
    });
    return {
      name,
      startMs,
      endMs,
      views,
      frameIntervalsMs: intervalsOf(frames),
      primaryIntervalsMs: intervalsOf(
        frames.filter((frame) => frame.views.has(shown[0]?.name ?? "")),
      ),
      mainThreadMs: frames.map((frame) => frame.mainMs),
      frameGpuMs,
      untimedFrames,
      droppedResolves,
      unattributedGpuMs: this.#unattributedMs(made),
    };
  }

  #attach(engine: RenderEngine): RenderEngine {
    this.#generation += 1;
    // Each listener lives as long as its engine, whose disposal drops it.
    engine.onPassTimes((times) => {
      this.#timer = times.timer;
      this.#times.set(this.#key(times.frame), times);
    });
    engine.onAllocation((event) => {
      if (event.kind !== "uploaded") {
        this.#allocations.push({ atMs: this.#nowMs(), kind: event.kind, name: event.name });
      }
    });
    engine.onFault((fault) => {
      this.#faults.push(faultLine(fault));
    });
    engine.onRestored(() => {
      this.#generation += 1;
    });
    return new ProbedEngine(engine, this, this.#nowMs);
  }

  #key(resolve: number): string {
    return `${String(this.#generation)}/${String(resolve)}`;
  }

  #frameAt(atMs: number): FrameEntry {
    const known = this.#frames.get(atMs);
    if (known !== undefined) {
      return known;
    }
    const frame: FrameEntry = { atMs, mainMs: 0, views: new Map() };
    this.#frames.set(atMs, frame);
    return frame;
  }

  /** The scale in force at `startMs`, then each change before `endMs`. */
  #scalesOf(view: string, startMs: number, endMs: number): number[] {
    const own = this.#scales.filter((entry) => entry.view === view && entry.atMs < endMs);
    const before = own.findLast((entry) => entry.atMs < startMs);
    return [...(before === undefined ? [] : [before]), ...own.filter((e) => e.atMs >= startMs)].map(
      (entry) => entry.scale,
    );
  }

  /**
   * The GPU time of the resolves reported between the first and the last of `made` that no view's
   * or target's render took, ms.
   */
  #unattributedMs(made: ReadonlySet<string>): number {
    const numbers = [...made].map((key) => key.split("/").map(Number));
    const generation = numbers[0]?.[0];
    const resolves = numbers.filter(([g]) => g === generation).map(([, n]) => n ?? 0);
    if (generation === undefined || resolves.length === 0) {
      return 0;
    }
    const low = Math.min(...resolves);
    const high = Math.max(...resolves);
    let ns = 0;
    for (const [key, times] of this.#times) {
      const [g, n] = key.split("/").map(Number);
      if (
        g === generation &&
        n !== undefined &&
        n >= low &&
        n <= high &&
        !this.#resolves.has(key)
      ) {
        ns += times.passes.reduce((sum, pass) => sum + pass.ns, 0);
      }
    }
    return ns / 1e6;
  }
}

/** The intervals between consecutive frames' timestamps, ms. */
function intervalsOf(frames: ReadonlyArray<FrameEntry>): number[] {
  return frames.slice(1).map((frame, i) => frame.atMs - (frames[i]?.atMs ?? frame.atMs));
}

/** A fault as one line of the record. */
function faultLine(fault: GraphicsFault): string {
  let line: string;
  switch (fault.kind) {
    case "device-lost":
      line = `device-lost (${fault.reason}): ${fault.message}`;
      break;
    case "gpu-process-gone":
      line = `gpu-process-gone (crash ${String(fault.count)})`;
      break;
    case "shader-refused":
      line = `shader-refused: ${fault.effectName}`;
      break;
    case "view-refused":
      line = `view-refused: ${fault.viewName}`;
      break;
  }
  return line;
}
