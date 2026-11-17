import { describe, expect, it } from "vitest";

import type { TraceFigures } from "../reduceTrace";
import type { TraceWindowStop } from "../spike";
import { WindowedFrameTrace, type WindowTrace } from "./frameTrace";

/** A trace whose windows the test names, each stop's file recorded. */
class FakeTrace implements WindowTrace {
  readonly written: string[] = [];
  starts = 0;

  start(): Promise<void> {
    this.starts += 1;
    return Promise.resolve();
  }

  stop(path: string): Promise<TraceWindowStop> {
    this.written.push(path);
    return Promise.resolve({ lostData: false, bufferPercent: 10 });
  }

  async cycle(path: string, stopped: (stop: TraceWindowStop) => void): Promise<void> {
    stopped(await this.stop(path));
    await this.start();
  }
}

/** A window's reduction: its presentations at `presentedAtUs`, the page's clock 5 s behind. */
function figuresOf(presentedAtUs: ReadonlyArray<number>): TraceFigures {
  const frames = {
    pid: 1,
    layerTreeHostId: 1,
    presentedAtUs,
    intervalsMs: [],
    presented: presentedAtUs.length,
    dropped: 0,
    droppedAtUs: [],
    noUpdate: 0,
  };
  return {
    span: null,
    clockOffsetUs: 5_000_000,
    frames,
    mainThread: null,
    gpuProcess: null,
    threads: [],
    userTiming: [],
  };
}

describe("a repetition's windowed trace", () => {
  it("cycles its windows on the timer, then reduces each and removes its file", async () => {
    const trace = new FakeTrace();
    const ticks: Array<() => void> = [];
    const removed: string[] = [];
    const reductions: Record<string, ReadonlyArray<number>> = {
      "w-1-0": [5_100_000, 5_116_000],
      "w-1-1": [5_300_000, 5_316_000, 5_333_000],
    };
    const windowed = new WindowedFrameTrace({
      trace,
      reduce: (path) => Promise.resolve(figuresOf(reductions[path] ?? [])),
      path: (repetition, n) => `w-${String(repetition)}-${String(n)}`,
      removeFile: (path) => {
        removed.push(path);
        return Promise.resolve();
      },
      every: (callback) => {
        ticks.push(callback);
        return () => undefined;
      },
      windowMs: 120_000,
      log: () => undefined,
    });
    await windowed.start(1);
    ticks[0]?.();
    const result = await windowed.stop();
    expect(trace.written).toEqual(["w-1-0", "w-1-1"]);
    expect(result).toEqual({
      kind: "presented",
      windowsMs: [
        [100, 116],
        [300, 316, 333],
      ],
    });
    expect(removed).toEqual(["w-1-0", "w-1-1"]);
  });

  it("gives none where no window holds two presentations", async () => {
    const trace = new FakeTrace();
    const windowed = new WindowedFrameTrace({
      trace,
      reduce: () => Promise.resolve(figuresOf([5_100_000])),
      path: () => "w",
      removeFile: () => Promise.resolve(),
      every: () => () => undefined,
      windowMs: 120_000,
      log: () => undefined,
    });
    await windowed.start(1);
    await expect(windowed.stop()).resolves.toEqual({
      kind: "none",
      reason: "no trace window holds two presentations",
    });
  });
});
