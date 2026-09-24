import { describe, expect, it } from "vitest";

import { SECONDS_PER_JULIAN_YEAR } from "@hyperion/protocol";

import {
  type BodyDistanceUnit,
  formatAge,
  formatBearingDeg,
  formatBodyDistance,
  formatGravity,
  formatLengthLy,
  formatListPosition,
  formatLuminosityLsun,
  formatMassMearth,
  formatMassMsun,
  formatNumber,
  formatPeriod,
  formatPressure,
  formatRadiusKm,
  formatRadiusRsun,
  formatScaleLength,
  formatSci,
  formatSignedDeg,
  formatSigned,
  formatTemperatureK,
  formatUniverseTimeDhms,
  formatUniverseTimeYr,
  KM_PER_AU,
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
    [1.57e-4, "reading", "1.6E-4"],
    [9.96e-4, "reading", "1.0E-3"],
    [1e-4, "tick", "1E-4"],
  ] as const)("writes %f to two significant figures in %s form as %s", (value, form, expected) => {
    expect(formatSci(value, form, 2)).toBe(expected);
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

describe("formatPeriod", () => {
  it.each([
    [0.4135, "0.414", "d"],
    [3.5247, "3.52", "d"],
    [87.969, "88.0", "d"],
    [365.256, "365", "d"],
    [999.4, "999", "d"],
    // Rounds to 1,000 d, which must be read in years: 999.6 d is 2.74 Julian years.
    [999.6, "2.74", "yr"],
    [1_000, "2.74", "yr"],
    [4_332.59, "11.9", "yr"],
    [60_190, "165", "yr"],
  ])("formats %f d as %s %s", (periodDays, value, unit) => {
    expect(formatPeriod(periodDays)).toEqual({ value, unit });
  });

  it("keeps the sign of a retrograde rotation", () => {
    expect(formatPeriod(-243.02)).toEqual({ value: "-243", unit: "d" });
  });

  it.each([
    [0.005, "5.00E-3", "d"],
    [3_652_500, "1.00E4", "yr"],
  ])("writes %f d, outside the ladder, in E notation as %s %s", (periodDays, value, unit) => {
    expect(formatPeriod(periodDays)).toEqual({ value, unit });
  });

  it("refuses a period that is not a number", () => {
    expect(() => formatPeriod(Number.NaN)).toThrow(RangeError);
  });
});

describe("formatBodyDistance", () => {
  it.each([
    [0.005, "5.00E-3", "km"],
    [500, "500", "km"],
    [999.4, "999", "km"],
    // Rounds to 1,000 km, which must be read as 1.00 Mm.
    [999.96, "1.00", "Mm"],
    [384_400, "384", "Mm"],
    [1_882_700, "1.88", "Gm"],
    [7_480_000, "7.48", "Gm"],
    [14_900_000, "14.9", "Gm"],
    [0.1 * KM_PER_AU, "0.100", "AU"],
    [57_909_227, "0.387", "AU"],
    [778_570_000, "5.20", "AU"],
    [50_000 * KM_PER_AU, "50,000", "AU"],
    [2_000_000 * KM_PER_AU, "2.00E6", "AU"],
  ])("formats %f km, first shown, as %s %s", (distanceKm, value, unit) => {
    expect(formatBodyDistance(distanceKm, null)).toEqual({ value, unit });
  });

  it.each<[number, BodyDistanceUnit, string, BodyDistanceUnit]>([
    // Up across 1,000 Mm: kept within 5% of the edge, then left.
    [1_020_000, "Mm", "1020", "Mm"],
    [1_060_000, "Mm", "1.06", "Gm"],
    // Down across 1 Gm: kept within 5% of the edge, then left.
    [960_000, "Gm", "0.960", "Gm"],
    [940_000, "Gm", "940", "Mm"],
    // Across 0.1 AU, both ways.
    [0.098 * KM_PER_AU, "AU", "0.0980", "AU"],
    [0.09 * KM_PER_AU, "AU", "13.5", "Gm"],
    [0.104 * KM_PER_AU, "Gm", "15.6", "Gm"],
    [0.11 * KM_PER_AU, "Gm", "0.110", "AU"],
  ])("formats %f km last shown in %s as %s %s", (distanceKm, previous, value, unit) => {
    expect(formatBodyDistance(distanceKm, previous)).toEqual({ value, unit });
  });

  it("does not flicker between units as a distance wanders about a boundary", () => {
    let unit: BodyDistanceUnit | null = null;
    const units: BodyDistanceUnit[] = [];
    // Within a 5% margin either side of 1,000 Mm, up and down, as an eccentric moon's may.
    for (const distanceKm of [990_000, 1_010_000, 995_000, 1_030_000, 1_001_000, 999_000]) {
      unit = formatBodyDistance(distanceKm, unit).unit;
      units.push(unit);
    }

    expect(units).toEqual(["Mm", "Mm", "Mm", "Mm", "Mm", "Mm"]);
  });

  it("keeps the sign of a negative distance, as a signed offset has", () => {
    expect(formatBodyDistance(-384_400, null)).toEqual({ value: "-384", unit: "Mm" });
  });
});

describe("formatLuminosityLsun", () => {
  it.each([
    [1, "1.00"],
    [0.085, "0.0850"],
    [0.001_08, "0.00108"],
    [25_400, "25,400"],
    [999_400, "999,000"],
  ])("writes %f L☉ to three significant figures as %s", (luminosity, text) => {
    expect(formatLuminosityLsun(luminosity)).toBe(text);
  });

  it.each([
    // The pinned white dwarf's 0.0001075 is a hair below the half in binary, and so reads 1.07E-4.
    [0.000_107_6, "1.08E-4"],
    [2_500_000, "2.50E6"],
  ])("writes %f L☉, outside the unit's range, in E notation as %s", (luminosity, text) => {
    expect(formatLuminosityLsun(luminosity)).toBe(text);
  });

  it("refuses a negative luminosity", () => {
    expect(() => formatLuminosityLsun(-1)).toThrow(RangeError);
  });
});

describe("formatRadiusRsun", () => {
  it.each([
    [1, "1.00"],
    [0.011_5, "0.0115"],
    [1_500, "1500"],
  ])("writes %f R☉ to three significant figures as %s", (radius, text) => {
    expect(formatRadiusRsun(radius)).toBe(text);
  });

  it("writes a radius below 0.001 R☉ in E notation", () => {
    expect(formatRadiusRsun(0.000_175_4)).toBe("1.75E-4");
  });

  it("refuses a negative radius", () => {
    expect(() => formatRadiusRsun(-0.5)).toThrow(RangeError);
  });
});

describe("formatRadiusKm", () => {
  it.each([
    [12.2, "12.2"],
    [36.87, "36.9"],
    [69_911, "69,900"],
  ])("writes %f km to three significant figures as %s", (radius, text) => {
    expect(formatRadiusKm(radius)).toBe(text);
  });

  it("writes a radius below 0.01 km in E notation", () => {
    expect(formatRadiusKm(0.004)).toBe("4.00E-3");
  });
});

describe("formatTemperatureK", () => {
  it.each([
    [2.7, "3"],
    [287.6, "288"],
    [1_400, "1400"],
    [12_480, "12,480"],
  ])("formats %f K as %s", (temperatureK, expected) => {
    expect(formatTemperatureK(temperatureK)).toBe(expected);
  });

  it("refuses a temperature below absolute zero", () => {
    expect(() => formatTemperatureK(-1)).toThrow(RangeError);
  });
});

describe("formatPressure", () => {
  it.each([
    [0, "0.00", "Pa"],
    [1, "1.00", "Pa"],
    [610, "610", "Pa"],
    [999.4, "999", "Pa"],
    // Rounds to 1,000 Pa, which must be read as 1.00 kPa; likewise at 1,000 kPa.
    [999.9, "1.00", "kPa"],
    [101_325, "101", "kPa"],
    [146_700, "147", "kPa"],
    [999_960, "1.00", "MPa"],
    [9_200_000, "9.20", "MPa"],
  ])("formats %f Pa as %s %s", (pressurePa, value, unit) => {
    expect(formatPressure(pressurePa)).toEqual({ value, unit });
  });

  it.each([
    [3e-10, "3.00E-10", "Pa"],
    [2e10, "2.00E4", "MPa"],
  ])("writes %f Pa, outside the ladder, in E notation as %s %s", (pressurePa, value, unit) => {
    expect(formatPressure(pressurePa)).toEqual({ value, unit });
  });

  it("refuses a negative pressure", () => {
    expect(() => formatPressure(-1)).toThrow(RangeError);
  });
});

describe("formatGravity", () => {
  it.each([
    [0, "0.00"],
    [0.284, "0.284"],
    [3.721, "3.72"],
    [9.806_65, "9.81"],
    [24.79, "24.8"],
    [274, "274"],
    [0.0057, "5.70E-3"],
  ])("formats %f m/s² as %s", (gravity, expected) => {
    expect(formatGravity(gravity)).toBe(expected);
  });

  it("refuses a negative gravity", () => {
    expect(() => formatGravity(-9.81)).toThrow(RangeError);
  });
});

describe("formatUniverseTimeDhms", () => {
  const H = 1_000 * SECONDS_PER_JULIAN_YEAR;

  it.each([
    ["the epoch", 0, "+0 yr 000/00:00:00"],
    ["one second before it", -1, "-0 yr 000/00:00:01"],
    ["one second after it", 1, "+0 yr 000/00:00:01"],
    ["+H", H, "+1000 yr 000/00:00:00"],
    ["-H", -H, "-1000 yr 000/00:00:00"],
    ["a second inside +H", H - 1, "+999 yr 365/05:59:59"],
    [
      "D24's example",
      12 * SECONDS_PER_JULIAN_YEAR + 183 * 86_400 + 14 * 3_600 + 8 * 60 + 33,
      "+12 yr 183/14:08:33",
    ],
    ["the last second of a Julian year", SECONDS_PER_JULIAN_YEAR - 1, "+0 yr 365/05:59:59"],
  ])("formats %s as %s", (_, seconds, expected) => {
    expect(formatUniverseTimeDhms({ seconds, nanos: 0 })).toBe(expected);
  });

  it("groups the years from five digits, out to the source horizon", () => {
    expect(
      formatUniverseTimeDhms({ seconds: 263_012 * SECONDS_PER_JULIAN_YEAR + 5, nanos: 0 }),
    ).toBe("+263,012 yr 000/00:00:05");
  });

  it("shows the instant's whole seconds, as the wire rounds them, and not its nanoseconds", () => {
    // 1.5 s before the epoch is -2 s and 0.5 s on it.
    expect(formatUniverseTimeDhms({ seconds: -2, nanos: 500_000_000 })).toBe("-0 yr 000/00:00:02");
    expect(formatUniverseTimeDhms({ seconds: 1, nanos: 999_999_999 })).toBe("+0 yr 000/00:00:01");
  });

  it("refuses seconds that are not a whole number", () => {
    expect(() => formatUniverseTimeDhms({ seconds: 1.5, nanos: 0 })).toThrow(RangeError);
  });
});

describe("formatMassMearth", () => {
  it.each([
    [0.33, "0.33"],
    [0.1, "0.10"],
    [9.99, "9.99"],
    [9.996, "10.0"],
    [17.1, "17.1"],
    [99.96, "100"],
    // Four digits are not grouped: the guide groups from five, as plan 05 does everywhere.
    [4_131, "4131"],
    [12_480, "12,480"],
  ])("formats %f Earth masses as %s", (massMearth, expected) => {
    expect(formatMassMearth(massMearth)).toBe(expected);
  });

  it.each([
    // The Moon, 7.346E22 kg of the Earth's 5.972E24 kg: two decimals would read 0.01.
    [0.0123, "0.012"],
    [0.004, "0.0040"],
    [0.05, "0.050"],
    [0.0994, "0.099"],
    [0.001, "0.0010"],
  ])(
    "keeps two significant figures of %f Earth masses, below 0.1, as %s",
    (massMearth, expected) => {
      expect(formatMassMearth(massMearth)).toBe(expected);
    },
  );

  it("reads a mass that rounds up to a step's edge as the edge reads, from either side", () => {
    expect(formatMassMearth(0.0996)).toBe("0.10");
    expect(formatMassMearth(0.1004)).toBe("0.10");
    expect(formatMassMearth(0.000_999)).toBe("0.0010");
  });

  it.each([
    // Ceres, 9.38E20 kg (Park et al. 2016): 1.57E-4 of the Earth's mass. E notation keeps the
    // guide's three significant figures (the orchestrator's ruling 44.1).
    [9.38e20 / 5.972e24, "1.57E-4"],
    [0.000_94, "9.40E-4"],
    [1.8e-9, "1.80E-9"],
  ])("writes %f Earth masses, below 0.001, in E notation as %s", (massMearth, expected) => {
    expect(formatMassMearth(massMearth)).toBe(expected);
  });

  it("reads zero as 0.00, not in E notation", () => {
    expect(formatMassMearth(0)).toBe("0.00");
  });
});
