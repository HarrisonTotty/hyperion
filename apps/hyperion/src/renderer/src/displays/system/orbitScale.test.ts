import { describe, expect, it } from "vitest";

import { formatBodyDistance, KM_PER_AU } from "../../lib/format";
import { scaleBar } from "../../spatial/scale";
import { formatOrbitScaleLength, METRES_PER_AU, ORBIT_SCALE_UNITS } from "./orbitScale";

/** The bar's label for the longest bar that fits `longestAu`, as the view's scale bar reads it. */
function barFor(longestAu: number): string {
  const { length } = scaleBar(1, longestAu, ORBIT_SCALE_UNITS);
  return formatOrbitScaleLength(length);
}

describe("the orbit map's scale bar", () => {
  it("measures the astronomical unit exactly", () => {
    expect(METRES_PER_AU).toBe(149_597_870_700);
  });

  it.each([
    [7.3, "5 AU"],
    [0.31, "0.2 AU"],
    [0.1, "0.1 AU"],
    [0.099, "10 Gm"],
    [0.004, "500 Mm"],
    [2e-6, "200 km"],
  ])("reads a bar of at most %f AU in 1-2-5 steps of its band's unit: %s", (longestAu, label) => {
    expect(barFor(longestAu)).toBe(label);
  });

  it("gives way from Gm to AU where formatBodyDistance does, at 0.1 AU", () => {
    expect(formatOrbitScaleLength(0.1)).toBe("0.1 AU");
    expect(formatBodyDistance(0.1 * KM_PER_AU, null).unit).toBe("AU");
    expect(formatBodyDistance(0.099 * KM_PER_AU, null).unit).toBe("Gm");
  });

  it("refuses a bar with no length", () => {
    expect(() => formatOrbitScaleLength(0)).toThrow(RangeError);
  });
});
