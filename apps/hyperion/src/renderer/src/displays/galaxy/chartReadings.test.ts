import { describe, expect, it } from "vitest";

import { NOW_MODE } from "../../lib/galaxy/model";
import { RADIUS_STEPS_LY } from "../../spatial/scale";
import {
  CHART_FRAME_NAME,
  modeReading,
  placeDecimalsFor,
  type PlacePrecision,
  placePrecisionFor,
  placeReadings,
  radiusReading,
  timeReading,
} from "./chartReadings";

/** The precision of a chart from 10 ly: one decimal for every coordinate, as before P09.T42. */
const ONE_DECIMAL: PlacePrecision = { place: 1, angle: 1 };

/** The finest chart's centre, 0.0003 ly coreward of the black hole and 0.0001 ly north. */
const NEAR_THE_BLACK_HOLE = [0.0003, 0, 0.0001] as const;

describe("the chart's readings (plan 12, P12.T11.b)", () => {
  it("names the GALACTIC frame", () => {
    expect(CHART_FRAME_NAME).toBe("GALACTIC");
  });

  it("writes a place's RADIUS, ANGLE and HEIGHT to 0.1 ly, each in the chart's slot", () => {
    expect(placeReadings([8_000, 6_000, -12.34], ONE_DECIMAL)).toEqual([
      { label: "RADIUS", value: "10,000.0", unit: "ly", widthCh: 9 },
      { label: "ANGLE", value: "036.9°", unit: "", widthCh: 6 },
      { label: "HEIGHT", value: "-12.3", unit: "ly", widthCh: 9 },
    ]);
  });

  it("leaves a place's ANGLE missing on the galactic axis, where it is undefined", () => {
    const [radius, angle, height] = placeReadings([0, 0, 100], ONE_DECIMAL);

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

  it("places the finest chart against the central black hole, its ANGLE to one decimal in the slot widened for five", () => {
    // 19 AU and 6.3 AU from the black hole, which one decimal of a light-year reads as 0.0 and
    // +0.0; one decimal of a degree steps 5.2E-7 ly along the arc there, finer than 1E-5 ly.
    const precision = placePrecisionFor(0.001, NEAR_THE_BLACK_HOLE);

    expect(precision).toEqual({ place: 5, angle: 1 });
    expect(placeReadings(NEAR_THE_BLACK_HOLE, precision)).toEqual([
      { label: "RADIUS", value: "0.00030", unit: "ly", widthCh: 13 },
      { label: "ANGLE", value: "000.0°", unit: "", widthCh: 10 },
      { label: "HEIGHT", value: "+0.00010", unit: "ly", widthCh: 13 },
    ]);
    expect(radiusReading(65_535.999_99, 5)).toHaveLength(12);
  });

  it.each([
    // R, chart radius, angle decimals: the fewest whose arc step is within the place's step.
    [0.0003, 0.001, 1],
    [0.5, 0.001, 3],
    [30, 0.001, 5],
    // Past 57.3 ly from the axis the cap at the place's decimals holds: the stated limit.
    [26_000, 0.001, 5],
    [30, 0.05, 4],
    [0.0003, 0.05, 1],
  ])(
    "reads the ANGLE at %s ly from the axis on a %s ly chart to %s decimals",
    (r, radiusLy, angle) => {
      expect(placePrecisionFor(radiusLy, [r, 0, 0]).angle).toBe(angle);
    },
  );

  it("reads one decimal of ANGLE on the axis, where the angle is missing", () => {
    expect(placePrecisionFor(0.001, [0, 0, 0.0004])).toEqual({ place: 5, angle: 1 });
    expect(placePrecisionFor(0.001, [0, 0, 0])).toEqual({ place: 5, angle: 1 });
  });

  it("keeps every chart from 10 ly at one decimal, wherever its centre", () => {
    for (const radiusLy of RADIUS_STEPS_LY.filter((step) => step >= 10)) {
      for (const centreLy of [
        [26_000, 0, 0],
        [8_000, 6_000, -12.34],
        [0.0003, 0, 0],
        [65_535, 65_535, 0],
      ] as const) {
        expect(placePrecisionFor(radiusLy, centreLy)).toEqual(ONE_DECIMAL);
      }
    }
  });

  it("writes an observer at the Sun's radius on a chart of the centre at the centre's precision", () => {
    const precision = placePrecisionFor(0.001, NEAR_THE_BLACK_HOLE);

    const reading = modeReading({ kind: "observed", observerLy: [26_000, 0, 12] }, precision);

    expect(reading.from).toEqual([
      { label: "RADIUS", value: "26,000.00000", unit: "ly", widthCh: 13 },
      // The centre's one decimal, not the five the observer's own radius would take.
      { label: "ANGLE", value: "000.0°", unit: "", widthCh: 10 },
      { label: "HEIGHT", value: "+12.00000", unit: "ly", widthCh: 13 },
    ]);
  });

  it("writes a radius as the place's RADIUS writes it, without its unit", () => {
    expect(radiusReading(26_000, 1)).toBe("26,000.0");
  });

  it("writes the chart time in universe time to 0.01 yr, in the chart's slot", () => {
    expect(timeReading(12.5)).toEqual({ label: "UT", value: "+12.50", unit: "yr", widthCh: 9 });
  });

  it("writes NOW with no observer", () => {
    expect(modeReading(NOW_MODE, ONE_DECIMAL)).toEqual({ name: "NOW", from: [] });
  });

  it("writes OBSERVED FROM with the observer written as a place", () => {
    expect(
      modeReading({ kind: "observed", observerLy: [8_000, 6_000, -12.34] }, ONE_DECIMAL),
    ).toEqual({
      name: "OBSERVED FROM",
      from: placeReadings([8_000, 6_000, -12.34], ONE_DECIMAL),
    });
  });
});
