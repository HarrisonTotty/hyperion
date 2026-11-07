import { describe, expect, it } from "vitest";

import {
  type LongTaskEntryType,
  LongTaskRecorder,
  type LongTaskSource,
  longTaskOf,
} from "./longTasks";

/** A source that hands each type's entries in when `emit` is called, supporting `types`. */
function fakeSource(types: ReadonlyArray<LongTaskEntryType>): {
  readonly source: LongTaskSource;
  readonly emit: (type: LongTaskEntryType, entries: ReadonlyArray<unknown>) => void;
  readonly observing: () => ReadonlyArray<LongTaskEntryType>;
} {
  const listeners = new Map<LongTaskEntryType, (entries: ReadonlyArray<unknown>) => void>();
  return {
    source: {
      supports: (type) => types.includes(type),
      observe: (type, onEntries) => {
        listeners.set(type, onEntries);
        return () => {
          listeners.delete(type);
        };
      },
    },
    emit: (type, entries) => {
      listeners.get(type)?.(entries);
    },
    observing: () => [...listeners.keys()],
  };
}

describe("the page's long tasks (lane perf-page-long-tasks)", () => {
  it("gives a task its frame's rendering update where that began within it, to the task's end", () => {
    const frame = { startMs: 100, endMs: 400, renderStartMs: 350 };
    expect([
      longTaskOf(100, 200, [frame]).renderMs,
      longTaskOf(300, 100, [frame]).renderMs,
      longTaskOf(300, 80, [frame]).renderMs,
      longTaskOf(500, 60, [frame]),
      longTaskOf(100, 300, [{ ...frame, renderStartMs: null }]).renderMs,
    ]).toEqual([0, 50, 30, { startMs: 500, durationMs: 60, renderMs: 0 }, 0]);
  });

  it("never gives a task more rendering than its length, where the rendering starts with it", () => {
    // (1,209,278.2 + 16,956.3) − 1,209,278.2 rounds to 16,956.300000000047.
    const startMs = 1_209_278.2;
    const durationMs = 16_956.3;
    const task = longTaskOf(startMs, durationMs, [
      { startMs, endMs: startMs + durationMs + 5, renderStartMs: startMs },
    ]);
    expect(task.renderMs).toBe(durationMs);
  });

  it("never gives a task less than no rendering, where its frame's whole-millisecond end falls before its rendering's start", () => {
    // A frame of 100 to 150 ms, its 50.4 ms rounded down, rendering from 150.4 ms.
    const task = longTaskOf(100, 51, [{ startMs: 100, endMs: 150, renderStartMs: 150.4 }]);
    expect(task.renderMs).toBe(0);
  });

  it("records each long task with its frame's rendering, from its start, in order, and stops", () => {
    let nowMs = 1_000;
    const { source, emit, observing } = fakeSource(["longtask", "long-animation-frame"]);
    const recorder = new LongTaskRecorder(source, () => nowMs);
    recorder.start();
    emit("long-animation-frame", [
      { startTime: 2_000, duration: 700, renderStart: 2_650 },
      { startTime: "late", duration: 60 },
    ]);
    emit("longtask", [
      { startTime: 2_000, duration: 700 },
      { startTime: 500, duration: 55 },
      { startTime: Number.NaN, duration: 60 },
    ]);
    nowMs = 9_000;
    const taken = recorder.take();
    recorder.stop();
    expect(taken).toEqual({
      observed: { longtask: true, longAnimationFrame: true },
      fromMs: 1_000,
      toMs: 9_000,
      tasks: [
        { startMs: 500, durationMs: 55, renderMs: 0 },
        { startMs: 2_000, durationMs: 700, renderMs: 50 },
      ],
    });
    expect(observing()).toEqual([]);
  });

  it("observes only what the page reports, and says so", () => {
    const { source, emit, observing } = fakeSource(["longtask"]);
    const recorder = new LongTaskRecorder(source, () => 0);
    recorder.start();
    recorder.start();
    emit("longtask", [{ startTime: 10, duration: 70 }]);
    expect([observing(), recorder.take().observed, recorder.take().tasks.length]).toEqual([
      ["longtask"],
      { longtask: true, longAnimationFrame: false },
      1,
    ]);
  });
});
