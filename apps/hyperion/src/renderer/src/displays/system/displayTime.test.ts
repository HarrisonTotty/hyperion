import { SECONDS_PER_JULIAN_YEAR } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { CLOCK_WINDOW_S, clockLimitAt, stepTime, TIME_STEPS } from "./displayTime";

describe("TIME_STEPS", () => {
  it("offers the six steps from an hour to a century, each with its key", () => {
    expect(TIME_STEPS.map((step) => [step.key, step.label, step.seconds])).toEqual([
      ["1", "1 h", 3_600],
      ["2", "1 d", 86_400],
      ["3", "10 d", 864_000],
      ["4", "1 yr", SECONDS_PER_JULIAN_YEAR],
      ["5", "10 yr", 10 * SECONDS_PER_JULIAN_YEAR],
      ["6", "100 yr", 100 * SECONDS_PER_JULIAN_YEAR],
    ]);
  });
});

describe("stepTime", () => {
  it("adds whole seconds and keeps the nanoseconds", () => {
    expect(stepTime({ seconds: 10, nanos: 7 }, 3_600)).toEqual({ seconds: 3_610, nanos: 7 });
    expect(stepTime({ seconds: 10, nanos: 7 }, -86_400)).toEqual({ seconds: -86_390, nanos: 7 });
  });

  it("stops at +H and never leaves the clock window", () => {
    expect(CLOCK_WINDOW_S).toBe(31_557_600_000);
    expect(stepTime({ seconds: CLOCK_WINDOW_S - 10, nanos: 5 }, 3_600)).toEqual({
      seconds: CLOCK_WINDOW_S,
      nanos: 0,
    });
    expect(stepTime({ seconds: CLOCK_WINDOW_S - 3_600, nanos: 5 }, 3_600)).toEqual({
      seconds: CLOCK_WINDOW_S,
      nanos: 0,
    });
  });

  it("stops at -H", () => {
    expect(stepTime({ seconds: 10 - CLOCK_WINDOW_S, nanos: 5 }, -3_600)).toEqual({
      seconds: -CLOCK_WINDOW_S,
      nanos: 0,
    });
  });

  it("refuses a step that is not whole seconds", () => {
    expect(() => stepTime({ seconds: 0, nanos: 0 }, 0.5)).toThrow(RangeError);
  });
});

describe("clockLimitAt", () => {
  it("names the edge a time stands at, and none inside the window", () => {
    expect(clockLimitAt({ seconds: CLOCK_WINDOW_S, nanos: 0 })).toBe("end");
    expect(clockLimitAt({ seconds: -CLOCK_WINDOW_S, nanos: 0 })).toBe("start");
    expect(clockLimitAt({ seconds: -CLOCK_WINDOW_S, nanos: 1 })).toBeNull();
    expect(clockLimitAt({ seconds: 0, nanos: 0 })).toBeNull();
  });
});
