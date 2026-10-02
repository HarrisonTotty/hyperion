import { describe, expect, it } from "vitest";

import {
  compareSpans,
  lightTime,
  SPEED_OF_LIGHT_M_PER_S,
  spanDistance,
  spanSeconds,
  timeBefore,
} from "./lightTime";

describe("lightTime", () => {
  it("is d ÷ c rounded to the nearest nanosecond", () => {
    expect(lightTime(0)).toEqual({ seconds: 0, nanos: 0 });
    expect(lightTime(SPEED_OF_LIGHT_M_PER_S)).toEqual({ seconds: 1, nanos: 0 });
    expect(lightTime(1.495_978_707e11)).toEqual({ seconds: 499, nanos: 4_783_836 });
    expect(lightTime(4.487_93e12)).toEqual({ seconds: 14_970, nanos: 123_097_626 });
  });

  it("floors the exact product by 10⁹, where the rounded product would give a nanosecond more", () => {
    // fraction × 10⁹ rounds up to a whole number here while its exact value lies just below it:
    // the simulation's fused multiply-add sees that, and a naive floor would not.
    expect(lightTime(188_678_696.706_289_44)).toEqual({ seconds: 0, nanos: 629_364_387 });
    expect(lightTime(263_794_603.246_277_3)).toEqual({ seconds: 0, nanos: 879_924_081 });
  });

  it("refuses a negative distance", () => {
    expect(() => lightTime(-1)).toThrow(RangeError);
  });
});

describe("span arithmetic", () => {
  it("subtracts a span from a time with a borrow, across the epoch", () => {
    expect(timeBefore({ seconds: 0, nanos: 100 }, { seconds: 1, nanos: 200 })).toEqual({
      seconds: -2,
      nanos: 999_999_900,
    });
  });

  it("measures the distance between spans either way round", () => {
    const a = { seconds: 5, nanos: 1 };
    const b = { seconds: 4, nanos: 999_999_999 };
    expect([spanDistance(a, b), spanDistance(b, a)]).toEqual([
      { seconds: 0, nanos: 2 },
      { seconds: 0, nanos: 2 },
    ]);
  });

  it("measures whole seconds back as a positive span", () => {
    expect(spanDistance({ seconds: 4, nanos: 0 }, { seconds: 5, nanos: 0 })).toEqual({
      seconds: 1,
      nanos: 0,
    });
  });

  it("orders spans by value", () => {
    expect(
      compareSpans({ seconds: 5, nanos: 1 }, { seconds: 4, nanos: 999_999_999 }),
    ).toBeGreaterThan(0);
  });

  it("gives a span as a float of seconds as the simulation does", () => {
    expect(spanSeconds({ seconds: 4, nanos: 999_999_999 })).toBe(4 + 999_999_999 * 1e-9);
  });
});
