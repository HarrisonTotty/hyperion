import { describe, expect, it } from "vitest";

import { NOW_MODE } from "../../lib/galaxy/model";
import {
  CHART_FRAME_NAME,
  modeReading,
  placeReadings,
  radiusReading,
  timeReading,
} from "./chartReadings";

describe("the chart's readings (plan 12, P12.T11.b)", () => {
  it("names the GALACTIC frame", () => {
    expect(CHART_FRAME_NAME).toBe("GALACTIC");
  });

  it("writes a place's RADIUS, ANGLE and HEIGHT to 0.1 ly, each in the chart's slot", () => {
    expect(placeReadings([8_000, 6_000, -12.34])).toEqual([
      { label: "RADIUS", value: "10,000.0", unit: "ly", widthCh: 9 },
      { label: "ANGLE", value: "036.9°", unit: "", widthCh: 6 },
      { label: "HEIGHT", value: "-12.3", unit: "ly", widthCh: 9 },
    ]);
  });

  it("leaves a place's ANGLE missing on the galactic axis, where it is undefined", () => {
    const [radius, angle, height] = placeReadings([0, 0, 100]);

    expect(radius?.value).toBe("0.0");
    expect(angle?.value).toBeNull();
    expect(height?.value).toBe("+100.0");
  });

  it("writes a radius as the place's RADIUS writes it, without its unit", () => {
    expect(radiusReading(26_000)).toBe("26,000.0");
  });

  it("writes the chart time in universe time to 0.01 yr, in the chart's slot", () => {
    expect(timeReading(12.5)).toEqual({ label: "UT", value: "+12.50", unit: "yr", widthCh: 9 });
  });

  it("writes NOW with no observer", () => {
    expect(modeReading(NOW_MODE)).toEqual({ name: "NOW", from: [] });
  });

  it("writes OBSERVED FROM with the observer written as a place", () => {
    expect(modeReading({ kind: "observed", observerLy: [8_000, 6_000, -12.34] })).toEqual({
      name: "OBSERVED FROM",
      from: placeReadings([8_000, 6_000, -12.34]),
    });
  });
});
