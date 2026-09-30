import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { compareTimes, nextRequestTime, REQUEST_AGAIN_AFTER_S } from "./requestTime";

const ASKED: UniverseTime = { seconds: 1_000_000, nanos: 250 };

function after(seconds: number, nanos = ASKED.nanos): UniverseTime {
  return { seconds: ASKED.seconds + seconds, nanos };
}

describe("nextRequestTime", () => {
  it("asks again after a Julian year", () => {
    expect(REQUEST_AGAIN_AFTER_S).toBe(SECONDS_PER_JULIAN_YEAR);
  });

  it("keeps the time asked for, itself, within a year either way", () => {
    for (const moved of [after(3_600), after(-86_400), after(SECONDS_PER_JULIAN_YEAR)]) {
      expect(nextRequestTime(ASKED, moved, [])).toBe(ASKED);
    }
    expect(nextRequestTime(ASKED, after(-SECONDS_PER_JULIAN_YEAR), [])).toBe(ASKED);
  });

  it("asks at the display time once it is more than a year ahead, by a nanosecond", () => {
    const moved = after(SECONDS_PER_JULIAN_YEAR, ASKED.nanos + 1);

    expect(nextRequestTime(ASKED, moved, [])).toBe(moved);
  });

  it("asks at the display time once it is more than a year behind", () => {
    const moved = after(-SECONDS_PER_JULIAN_YEAR, ASKED.nanos - 1);

    expect(nextRequestTime(ASKED, moved, [])).toBe(moved);
  });

  it("asks again when the display time moves past a change, either way", () => {
    const change = after(7_200, 0);

    expect(nextRequestTime(ASKED, after(7_200), [change])).toEqual(after(7_200));
    expect(nextRequestTime(after(7_200), ASKED, [change])).toBe(ASKED);
    expect(nextRequestTime(ASKED, after(3_600), [change])).toBe(ASKED);
  });

  it("does not ask again for a change on the same side as both times", () => {
    expect(nextRequestTime(ASKED, after(3_600), [after(-7_200)])).toBe(ASKED);
  });
});

describe("compareTimes", () => {
  it("orders instants by their seconds, then their nanoseconds", () => {
    expect(compareTimes({ seconds: -2, nanos: 900 }, { seconds: -1, nanos: 0 })).toBeLessThan(0);
    expect(compareTimes({ seconds: 5, nanos: 2 }, { seconds: 5, nanos: 1 })).toBeGreaterThan(0);
    expect(compareTimes({ seconds: 5, nanos: 1 }, { seconds: 5, nanos: 1 })).toBe(0);
  });
});
