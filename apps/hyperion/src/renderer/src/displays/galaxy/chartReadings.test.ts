import { describe, expect, it } from "vitest";

import { NOW_MODE } from "../../lib/galaxy/model";
import {
  CHART_FRAME_NAME,
  modeReading,
  placeDecimalsFor,
  placeReadings,
  radiusReading,
  timeReading,
} from "./chartReadings";

describe("the chart's readings (plan 12, P12.T11.b)", () => {
  it("names the GALACTIC frame", () => {
    expect(CHART_FRAME_NAME).toBe("GALACTIC");
  });

  it("writes a place's RADIUS, ANGLE and HEIGHT to 0.1 ly, each in the chart's slot", () => {
    expect(placeReadings([8_000, 6_000, -12.34], 1)).toEqual([
      { label: "RADIUS", value: "10,000.0", unit: "ly", widthCh: 9 },
      { label: "ANGLE", value: "036.9°", unit: "", widthCh: 6 },
      { label: "HEIGHT", value: "-12.3", unit: "ly", widthCh: 9 },
    ]);
  });

  it("leaves a place's ANGLE missing on the galactic axis, where it is undefined", () => {
    const [radius, angle, height] = placeReadings([0, 0, 100], 1);

    expect(radius?.value).toBe("0.0");
    expect(angle?.value).toBeNull();
    expect(height?.value).toBe("+100.0");
  });

  it.each([
    [500, 1],
    [10, 1],
    [5, 2],
    [1, 2],
    [0.5, 3],
    [0.1, 3],
    [0.05, 4],
    [0.01, 4],
    [0.1 * 0.1, 4],
    [0.005, 5],
    [0.001, 5],
  ])("reads a place on a %s ly chart to %s decimals (plan 09, P09.T42)", (radiusLy, decimals) => {
    expect(placeDecimalsFor(radiusLy)).toBe(decimals);
  });

  it("places the finest chart against the central black hole, each slot widened for its decimals", () => {
    // 0.0003 ly coreward of the black hole, at the galactic centre, and 0.0001 ly north: 19 AU and
    // 6.3 AU, which one decimal reads as 0.0 and +0.0.
    expect(placeReadings([0.0003, 0, 0.0001], placeDecimalsFor(0.001))).toEqual([
      { label: "RADIUS", value: "0.00030", unit: "ly", widthCh: 13 },
      { label: "ANGLE", value: "000.00000°", unit: "", widthCh: 10 },
      { label: "HEIGHT", value: "+0.00010", unit: "ly", widthCh: 13 },
    ]);
    expect(radiusReading(65_535.999_99, 5)).toHaveLength(12);
  });

  it("writes a radius as the place's RADIUS writes it, without its unit", () => {
    expect(radiusReading(26_000, 1)).toBe("26,000.0");
  });

  it("writes the chart time in universe time to 0.01 yr, in the chart's slot", () => {
    expect(timeReading(12.5)).toEqual({ label: "UT", value: "+12.50", unit: "yr", widthCh: 9 });
  });

  it("writes NOW with no observer", () => {
    expect(modeReading(NOW_MODE, 1)).toEqual({ name: "NOW", from: [] });
  });

  it("writes OBSERVED FROM with the observer written as a place", () => {
    expect(modeReading({ kind: "observed", observerLy: [8_000, 6_000, -12.34] }, 1)).toEqual({
      name: "OBSERVED FROM",
      from: placeReadings([8_000, 6_000, -12.34], 1),
    });
  });
});
