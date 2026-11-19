import { describe, expect, it } from "vitest";

import type { GpuTimer } from "../engine/status";
import type { PassTimes } from "../engine/types";
import { serialPasses, timedPass } from "../../test/passTimes";
import { passRolesOf } from "./passRows";
import { type PerfFrameSample, RepetitionMetrics, TIMESTAMP_QUANTUM_NS } from "./repetitionMetrics";

const ROLES = passRolesOf([
  { role: "primary", style: "photorealistic" },
  { role: "instrument", style: "wireframe" },
]);

const EXTRAS = {
  spanS: 2,
  segments: [
    { name: "first", startS: 0, endS: 1 },
    { name: "second", startS: 1, endS: 2 },
  ],
  timeOriginMs: 0,
  memory: { itemPeakBytes: {}, tallyPeakBytes: null, workerFieldBytes: [], workerWasmBytes: [] },
  latePipelines: 0,
  internalScale: [],
};

function metrics(firstEngineFrame = 0): RepetitionMetrics {
  return new RepetitionMetrics({
    roles: ROLES,
    demandPerS: (segment) => (segment === "first" ? 12 : null),
    firstEngineFrame,
    previousRafMs: 990,
  });
}

function frame(engineFrame: number, scriptTimeS: number, streaming = false): PerfFrameSample {
  return {
    engineFrame,
    scriptTimeS,
    rafTimestampMs: 1000 + scriptTimeS * 1000,
    callbackStartMs: 1001 + scriptTimeS * 1000,
    callbackMs: 3,
    streaming,
  };
}

function times(
  frameNumber: number,
  passes: ReadonlyArray<readonly [string, number]>,
  timer: GpuTimer = "full",
): PassTimes {
  return {
    frame: frameNumber,
    timer,
    passes: serialPasses(passes),
  };
}

describe("a repetition's metrics", () => {
  it("counts a row over the frames whose every resolve reported, a pass over the frames that timed it", () => {
    const m = metrics();
    m.frame(frame(2, 0));
    m.passTimes(times(1, [["bodies", 4e6]]));
    m.passTimes(times(2, [["view:wireframe", 1e6]]));
    m.frame(frame(4, 0.5));
    // Its instrument's resolve never reports: the frame is a gap in the rows.
    m.passTimes(times(3, [["bodies", 6e6]]));
    const result = m.result(EXTRAS);
    expect(result.passes).toContainEqual({
      key: "primary:bodies",
      row: "terrain",
      frames: 2,
      p50Ms: 4,
      p95Ms: 6,
      p99Ms: 6,
    });
    expect(result.rows).toContainEqual({
      row: "terrain",
      frames: 1,
      meanMs: 4,
      p50Ms: 4,
      p95Ms: 4,
      p99Ms: 4,
    });
    expect(result.timedPassesPerFrameMs).toBe(5);
  });

  it("leaves out the resolves numbered before its first frame and after its last", () => {
    const m = metrics(10);
    m.passTimes(times(10, [["bodies", 50e6]]));
    m.frame(frame(11, 0));
    m.passTimes(times(11, [["bodies", 1e6]]));
    m.passTimes(times(12, [["bodies", 50e6]]));
    expect(m.result(EXTRAS).passes).toEqual([
      { key: "primary:bodies", row: "terrain", frames: 1, p50Ms: 1, p95Ms: 1, p99Ms: 1 },
    ]);
  });

  it("gives its frames' intervals from the last frame before it, and their script times", () => {
    const m = metrics();
    m.frame(frame(1, 0));
    m.frame(frame(2, 0.016));
    const { frames } = m.result(EXTRAS);
    expect(frames.intervalsMs).toEqual([10, 16]);
    expect(frames.scriptTimesS).toEqual([0, 0.016]);
    expect(frames.callbackMs).toEqual([3, 3]);
  });

  it("reads the timer as the worst reported, quantized where every time is a multiple of the quantum", () => {
    const quantised = metrics();
    quantised.frame(frame(1, 0));
    quantised.passTimes(times(1, [["bodies", 3 * TIMESTAMP_QUANTUM_NS]]));
    expect(quantised.result(EXTRAS).timer).toBe("quantized");

    const full = metrics();
    full.frame(frame(1, 0));
    full.passTimes(times(1, [["bodies", 3 * TIMESTAMP_QUANTUM_NS + 83]]));
    expect(full.result(EXTRAS).timer).toBe("full");

    const absent = metrics();
    absent.frame(frame(1, 0));
    expect(absent.result(EXTRAS).timer).toBe("absent");
  });

  it("streams by segment: patches made resident a second, the prediction and the time streaming", () => {
    const m = metrics();
    m.frame(frame(1, 0, true));
    m.patch("resident");
    m.patch("baked");
    m.frame(frame(2, 0.5, true));
    m.frame(frame(3, 1.5, true));
    m.patch("resident");
    expect(m.result(EXTRAS).streaming).toEqual([
      { segment: "first", patchesPerS: 1, demandPerS: 12, streamingSeconds: 1 },
      { segment: "second", patchesPerS: 1, demandPerS: null, streamingSeconds: 0.5 },
    ]);
  });

  it("leaves out the resolves it is told to before its first frame, a stand-in's", () => {
    const m = metrics(0);
    m.leaveOutThrough(4);
    m.passTimes({
      frame: 3,
      timer: "full",
      passes: [timedPass("view:wireframe", 2e6)],
    });
    m.frame(frame(6, 0));
    m.passTimes({
      frame: 4,
      timer: "full",
      passes: [timedPass("view:wireframe", 2e6)],
    });
    m.passTimes({
      frame: 5,
      timer: "full",
      passes: [timedPass("bodies", 1e6)],
    });
    m.passTimes({
      frame: 6,
      timer: "full",
      passes: [timedPass("bodies", 1e6)],
    });
    m.leaveOutThrough(5);
    expect(m.result(EXTRAS).passes.map(({ key, frames }) => [key, frames])).toEqual([
      ["primary:bodies", 1],
    ]);
  });
});
