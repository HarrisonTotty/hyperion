import { describe, expect, it } from "vitest";

import {
  formatAge,
  formatBearingDeg,
  formatLengthLy,
  formatListPosition,
  formatMassMsun,
  formatNumber,
  formatScaleLength,
  formatSci,
  formatSignedDeg,
  formatSigned,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "./format";

describe("formatNumber", () => {
  it.each([
    [0, 0, "0"],
    [12.4, 0, "12"],
    [1234.5, 0, "1235"],
    [9_999, 0, "9999"],
    [10_000, 0, "10,000"],
    [12_480, 0, "12,480"],
    [1_234_567.891, 2, "1,234,567.89"],
    [0.25, 2, "0.25"],
    [-3.2, 2, "-3.20"],
    [-12_345, 0, "-12,345"],
    [-0.004, 2, "0.00"],
    [-0, 1, "0.0"],
  ])("formats %f with %i decimals as %s", (value, decimals, expected) => {
    expect(formatNumber(value, decimals)).toBe(expected);
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY])("refuses %f", (value) => {
    expect(() => formatNumber(value, 0)).toThrow(RangeError);
  });
});

describe("formatSigned", () => {
  it.each([
    [12.5, 2, "+12.50"],
    [-3.2, 2, "-3.20"],
    [0, 2, "+0.00"],
    [-0.004, 2, "+0.00"],
    [0.004, 2, "+0.00"],
    [10_000, 0, "+10,000"],
    [-9_999, 0, "-9999"],
  ])("formats %f with %i decimals as %s", (value, decimals, expected) => {
    expect(formatSigned(value, decimals)).toBe(expected);
  });
});

describe("formatSci", () => {
  it.each([
    [5.2e10, "5.20E10"],
    [1e-4, "1.00E-4"],
    [3.1623, "3.16E0"],
    [9.995e10, "1.00E11"],
    [-4.567e-3, "-4.57E-3"],
    [123_456_789, "1.23E8"],
    [0, "0.00E0"],
  ])("writes the reading %f as %s", (value, expected) => {
    expect(formatSci(value)).toBe(expected);
  });

  it.each([
    [1e-4, "1E-4"],
    [1, "1E0"],
    [100, "1E2"],
    [2.5e3, "2.5E3"],
  ])("writes the tick %f as %s", (value, expected) => {
    expect(formatSci(value, "tick")).toBe(expected);
  });
});

describe("formatLengthLy", () => {
  it.each([
    [26_000, 1, "26,000.0"],
    [49.987, 2, "49.99"],
    [0.01234, 4, "0.0123"],
    [-3.2, 2, "-3.20"],
    [-0.004, 2, "0.00"],
  ])("formats %f ly with %i decimals as %s", (ly, decimals, expected) => {
    expect(formatLengthLy(ly, decimals)).toBe(expected);
  });
});

describe("formatScaleLength", () => {
  it.each([
    [20, "20 ly"],
    [500, "500 ly"],
    [20_000, "20,000 ly"],
    [0.05, "0.05 ly"],
    [0.01, "0.01 ly"],
    [500 / 63_241.077, "500 AU"],
    [200 / 63_241.077, "200 AU"],
    [1 / 63_241.077, "1 AU"],
  ])("formats %f ly as %s", (ly, expected) => {
    expect(formatScaleLength(ly)).toBe(expected);
  });
});

describe("formatBearingDeg", () => {
  it.each([
    [0, "000°"],
    [5, "005°"],
    [45, "045°"],
    [359.4, "359°"],
    [359.6, "000°"],
    [360, "000°"],
    [-0.4, "000°"],
    [-90, "270°"],
    [725, "005°"],
  ])("formats %f° as %s", (angleDeg, expected) => {
    expect(formatBearingDeg(angleDeg)).toBe(expected);
  });

  it.each([
    [45, "045.0°"],
    [359.96, "000.0°"],
    [123.45, "123.5°"],
  ])("formats %f° with one decimal as %s", (angleDeg, expected) => {
    expect(formatBearingDeg(angleDeg, 1)).toBe(expected);
  });
});

describe("formatSignedDeg", () => {
  it.each([
    [30, "+30°"],
    [-5, "-05°"],
    [90, "+90°"],
    [-90, "-90°"],
    [0, "+00°"],
    [-0.4, "+00°"],
    [4.6, "+05°"],
  ])("formats %f° as %s", (angleDeg, expected) => {
    expect(formatSignedDeg(angleDeg)).toBe(expected);
  });
});

describe("formatAge", () => {
  it.each([
    [0.0312, "0.0312", "Myr"],
    [3.1234, "3.12", "Myr"],
    [312.4, "312", "Myr"],
    [999.4, "999", "Myr"],
    // Rounds to 1,000 Myr, which Intl alone would print as "1000": it must switch to Gyr.
    [999.5, "1.00", "Gyr"],
    [999.9, "1.00", "Gyr"],
    [1_000, "1.00", "Gyr"],
    [4_600, "4.60", "Gyr"],
    [13_800, "13.8", "Gyr"],
  ])("formats %f Myr as %s %s", (ageMyr, value, unit) => {
    expect(formatAge(ageMyr)).toEqual({ value, unit });
  });
});

describe("formatMassMsun", () => {
  it.each([
    [0.08, "0.08"],
    [1.234, "1.23"],
    [9.99, "9.99"],
    [9.996, "10.0"],
    [12.34, "12.3"],
    [150, "150.0"],
    [12_345.67, "12,345.7"],
  ])("formats %f solar masses as %s", (massMsun, expected) => {
    expect(formatMassMsun(massMsun)).toBe(expected);
  });
});

describe("formatUniverseTimeYr", () => {
  it.each([
    [12.5, "+12.50"],
    [0, "+0.00"],
    [-1_000, "-1000.00"],
    [-0.004, "+0.00"],
  ])("formats %f yr as %s", (timeYr, expected) => {
    expect(formatUniverseTimeYr(timeYr)).toBe(expected);
  });

  it("names the universe time system UT", () => {
    expect(TIME_SYSTEM_LABEL).toBe("UT");
  });
});

describe("formatListPosition", () => {
  // Four-digit numbers are not grouped: the guide groups from five digits.
  it.each([
    [11, 23, 1_612, "12-24 of 1612"],
    [0, 19, 87, "1-20 of 87"],
    [2_980, 2_999, 3_000, "2981-3000 of 3000"],
    [9_979, 9_999, 12_480, "9980-10,000 of 12,480"],
    [0, 0, 1, "1-1 of 1"],
  ])("formats rows %i to %i of %i as %s", (first, last, total, expected) => {
    expect(formatListPosition(first, last, total)).toBe(expected);
  });

  it.each([
    [-1, 3, 10],
    [4, 3, 10],
    [0, 10, 10],
    [0, 0, 0],
  ])("refuses rows %i to %i of %i", (first, last, total) => {
    expect(() => formatListPosition(first, last, total)).toThrow(RangeError);
  });
});
