import { describe, expect, it } from "vitest";

import {
  judgeWarmUp,
  mannKendallS,
  type Timed,
  trendOf,
  WARM_UP_MAX_S,
  WARM_UP_MIN_S,
  type WarmUpSeries,
} from "./steadiness";

/** One value a second from `fromS` to `toS`, s, by `valueAt`. */
function each(fromS: number, toS: number, valueAt: (tS: number) => number): Timed[] {
  return Array.from({ length: toS - fromS }, (_, i) => ({
    atMs: (fromS + i) * 1000,
    value: valueAt(fromS + i),
  }));
}

/** A series whose frame intervals follow `intervalAt` and whose clock and package are unread. */
function frames(toS: number, intervalAt: (tS: number) => number): WarmUpSeries {
  return { gpuMHz: [], packageDegC: [], intervalsMs: each(0, toS, intervalAt) };
}

describe("the warm-up's verdict", () => {
  it("warms for two minutes however steady the machine", () => {
    const series = frames(119, () => 33.3);
    expect(judgeWarmUp(series, 0, 119_000)).toEqual({ kind: "warming", elapsedS: 119 });
  });

  it("is steady once a 30 s window holds against the one before", () => {
    const series = frames(WARM_UP_MIN_S + 1, () => 33.3);
    expect(judgeWarmUp(series, 0, (WARM_UP_MIN_S + 1) * 1000).kind).toBe("steady");
  });

  it("waits while the GPU's clock moves by 5% or more, or the package by 2 °C", () => {
    const clock: WarmUpSeries = {
      gpuMHz: each(0, 200, (t) => (t < 170 ? 1150 : 1000)),
      packageDegC: [],
      intervalsMs: each(0, 200, () => 33.3),
    };
    expect(judgeWarmUp(clock, 0, 200_000).kind).toBe("warming");
    const warm: WarmUpSeries = {
      gpuMHz: [],
      packageDegC: each(0, 200, (t) => 60 + t / 10),
      intervalsMs: each(0, 200, () => 33.3),
    };
    expect(judgeWarmUp(warm, 0, 200_000).kind).toBe("warming");
  });

  it("ends a trace that never settles at ten minutes as NOT STEADY", () => {
    const never = frames(WARM_UP_MAX_S + 1, (t) => 30 + (Math.floor(t / 30) % 2) * 5);
    for (let t = WARM_UP_MIN_S; t < WARM_UP_MAX_S; t += 10) {
      expect(judgeWarmUp(never, 0, t * 1000).kind).not.toBe("steady");
    }
    const verdict = judgeWarmUp(never, 0, WARM_UP_MAX_S * 1000);
    expect(verdict.kind).toBe("not-steady");
    expect(verdict.kind === "not-steady" ? verdict.why : "").toContain("median frame interval");
  });
});

describe("the trend screen", () => {
  it("flags a monotone five", () => {
    expect(trendOf([10, 11, 12, 13, 14])).toEqual({ direction: "rising", s: 10 });
    expect(trendOf([14, 13, 12, 11, 10])).toEqual({ direction: "falling", s: -10 });
  });

  it("does not flag four up and one down", () => {
    expect(mannKendallS([10, 11, 12, 14, 13])).toBe(8);
    expect(trendOf([10, 11, 12, 14, 13])).toBeNull();
    expect(trendOf([10, 12, 11, 13, 14])).toBeNull();
  });

  it("reads no fewer than five repetitions", () => {
    expect(trendOf([10, 11, 12, 13])).toBeNull();
  });
});
