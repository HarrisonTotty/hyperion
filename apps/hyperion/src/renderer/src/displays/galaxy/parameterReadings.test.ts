import type { Parameter, ParameterOrigin, Unit } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { groupLabel, parameterLabel } from "./parameterLabels";
import { toReading } from "./parameterReadings";

function aNumber(value: number, unit: Unit, key = "stellar_mass"): Parameter {
  return { key, origin: "drawn", value: { type: "number", value, unit } };
}

describe("toReading", () => {
  it.each<[string, number, Unit, string]>([
    ["a stellar mass above 10⁷ in E notation", 5.2e10, "msun", "5.20E10"],
    ["a mass of five digits or more below 10⁷ grouped", 4.3e6, "msun", "4,300,000"],
    ["a small mass as masses read elsewhere", 0.43, "msun", "0.43"],
    ["a count above 10⁷ in E notation", 1.21e11, "count", "1.21E11"],
    ["a count of five digits grouped", 12_480, "count", "12,480"],
    ["a small count whole", 2, "count", "2"],
    ["a length of five digits grouped, to one decimal", 16_300, "ly", "16,300.0"],
    // The guide groups from five digits, so the plan's example 8,480.4 reads without a comma.
    ["a four-digit length ungrouped, to one decimal", 8_480.4, "ly", "8480.4"],
    ["a pattern speed to two decimals", 2.227, "deg_per_myr", "2.23"],
    ["an angle to two decimals", 12.5, "deg", "12.50"],
    ["a speed to one decimal", 229.64, "km_per_s", "229.6"],
    ["a time in Gyr to three figures", 6.8, "gyr", "6.80"],
    ["a longer time in Gyr to three figures", 10.24, "gyr", "10.2"],
    ["a time in Myr to three figures", 450.4, "myr", "450"],
    ["a share to three figures", 0.112, "none", "0.112"],
    ["a tiny share in E notation", 0.000_42, "none", "4.20E-4"],
    ["a density in E notation", 0.004, "per_ly3", "4.00E-3"],
    ["a negative value with its sign", -0.25, "none", "-0.250"],
  ])("reads %s", (_, value, unit, text) => {
    const reading = toReading(aNumber(value, unit));

    expect(reading.text).toBe(text);
    expect(reading.unit).toBe(unit);
  });

  it("labels a parameter from the glossary", () => {
    expect(toReading(aNumber(5.2e10, "msun", "stellar_mass")).label).toBe("STELLAR MASS");
    expect(toReading(aNumber(2.227, "deg_per_myr", "bar.pattern_speed")).label).toBe(
      "PATTERN SPEED",
    );
  });

  it("falls back to the key in upper case for a parameter the glossary lacks", () => {
    expect(toReading(aNumber(1, "none", "disc.warp.amplitude")).label).toBe("DISC.WARP.AMPLITUDE");
  });

  it("does not take a label from the object prototype", () => {
    expect(parameterLabel("constructor")).toBe("CONSTRUCTOR");
    expect(groupLabel("toString")).toBe("TOSTRING");
  });

  it("passes a text value through in upper case, with no unit", () => {
    const reading = toReading({
      key: "mass_function",
      origin: "fixed",
      value: { type: "text", value: "kroupa" },
    });

    expect(reading).toEqual({
      key: "mass_function",
      label: "MASS FUNCTION",
      text: "KROUPA",
      unit: null,
      origin: "FIXED",
    });
  });

  it.each<[ParameterOrigin, string]>([
    ["drawn", "DRAWN"],
    ["derived", "DERIVED"],
    ["fixed", "FIXED"],
  ])("gives the origin %s in words", (origin, words) => {
    expect(toReading({ ...aNumber(1, "none"), origin }).origin).toBe(words);
  });
});

describe("groupLabel", () => {
  it("labels a known group and falls back to the key for an unknown one", () => {
    expect(groupLabel("bulge_and_bar")).toBe("BULGE AND BAR");
    expect(groupLabel("dust")).toBe("DUST");
  });
});
