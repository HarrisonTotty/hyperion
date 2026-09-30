import { describe, expect, it } from "vitest";

import { SECONDS_PER_JULIAN_YEAR, universeTimeFromYears, universeTimeToYears } from "./time";

describe("universe time in years", () => {
  it.each([
    [1_000, { seconds: 31_557_600_000, nanos: 0 }],
    [-1_000, { seconds: -31_557_600_000, nanos: 0 }],
    [12.5, { seconds: 394_470_000, nanos: 0 }],
    [0, { seconds: 0, nanos: 0 }],
  ])("writes %s years as %o and reads it back", (years, time) => {
    expect(universeTimeFromYears(years)).toEqual(time);
    expect(universeTimeToYears(time)).toBe(years);
  });

  it("counts nanoseconds forward from a negative second", () => {
    const time = universeTimeFromYears(-1.5 / SECONDS_PER_JULIAN_YEAR);

    expect(time).toEqual({ seconds: -2, nanos: 500_000_000 });
  });

  it("round trips fractional years within the clock window", () => {
    for (const years of [-999.999, -500, -0.0005, 0.25, 123.456_789, 999.999]) {
      const back = universeTimeToYears(universeTimeFromYears(years));
      expect(Math.abs(back - years)).toBeLessThan(1e-12);
    }
  });

  it("keeps whole nanoseconds below one second", () => {
    for (const years of [-999.123_456_789, -0.1, 0.3, 777.777]) {
      const { seconds, nanos } = universeTimeFromYears(years);
      expect(Number.isSafeInteger(seconds)).toBe(true);
      expect(Number.isInteger(nanos)).toBe(true);
      expect(nanos).toBeGreaterThanOrEqual(0);
      expect(nanos).toBeLessThan(1_000_000_000);
    }
  });

  it("writes negative zero as zero", () => {
    expect(Object.is(universeTimeFromYears(-0).seconds, 0)).toBe(true);
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, 1e300])(
    "refuses %s years",
    (years) => {
      expect(() => universeTimeFromYears(years)).toThrow(RangeError);
    },
  );
});
