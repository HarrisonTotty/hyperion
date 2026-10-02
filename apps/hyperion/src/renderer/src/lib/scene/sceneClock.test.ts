import { describe, expect, it } from "vitest";

import type { SceneClock } from "./model";
import { renderTime } from "./sceneClock";

/** 999 Julian years after the epoch, with nanoseconds an `f64` of seconds could not keep. */
const FAR = { seconds: 999 * 31_557_600, nanos: 123_456_789 };

/** The clock window's edge, 1,000 Julian years, in seconds. */
const EDGE_S = 1_000 * 31_557_600;

function clock(rate: number, state: SceneClock["state"] = "running", time = FAR): SceneClock {
  return { time, rate, state };
}

describe("renderTime", () => {
  it("advances at 1× by the real time since the push, keeping the nanoseconds", () => {
    expect(renderTime(clock(1), 1_000, 1_016.5)).toEqual({
      seconds: FAR.seconds,
      nanos: 139_956_789,
    });
  });

  it("advances at 100,000× from a time 999 years out, keeping the nanoseconds", () => {
    // 16.000 25 ms × 10⁵ = 1,600.025 s.
    expect(renderTime(clock(100_000), 2_000, 2_016.000_25)).toEqual({
      seconds: FAR.seconds + 1_600,
      nanos: 148_456_789,
    });
  });

  it("carries a whole hour at 100,000× exactly, 999 years before the epoch", () => {
    const before = { seconds: -FAR.seconds, nanos: FAR.nanos };
    expect(renderTime(clock(100_000, "running", before), 0, 3_600_000)).toEqual({
      seconds: before.seconds + 360_000_000,
      nanos: FAR.nanos,
    });
  });

  it("holds a paused clock and one at the window's limit", () => {
    expect(renderTime(clock(0, "paused"), 0, 5_000)).toEqual(FAR);
    expect(renderTime(clock(1_000, "window_limit"), 0, 5_000)).toEqual(FAR);
  });

  it("stops at the clock window's edge", () => {
    expect(renderTime(clock(100_000), 0, 1e9)).toEqual({ seconds: EDGE_S, nanos: 0 });
    expect(renderTime(clock(100_000, "running", { seconds: -EDGE_S, nanos: 0 }), 0, 0.01)).toEqual({
      seconds: -EDGE_S + 1,
      nanos: 0,
    });
  });

  it("holds a pushed time below the window's lower edge at the edge", () => {
    expect(renderTime(clock(1, "running", { seconds: -EDGE_S - 5, nanos: 7 }), 0, 1)).toEqual({
      seconds: -EDGE_S,
      nanos: 0,
    });
  });

  it("never runs backwards for a frame stamped before the push", () => {
    expect(renderTime(clock(1_000), 500, 400)).toEqual(FAR);
  });
});
