import { describe, expect, it } from "vitest";

import { planckianChromaticity, starColour } from "./starColour";

const luminance = (rgb: readonly [number, number, number]): number =>
  0.212_673 * rgb[0] + 0.715_152 * rgb[1] + 0.072_175 * rgb[2];

describe("planckianChromaticity", () => {
  it("puts a 6,500 K black body near D65 (0.3127, 0.3290)", () => {
    const { x, y } = planckianChromaticity(6_500);
    // The Planckian locus passes within 0.004 of D65, which lies slightly above it.
    expect(Math.hypot(x - 0.3127, y - 0.329)).toBeLessThan(0.006);
  });

  it("puts a 3,000 K black body at (0.437, 0.404) within 0.002", () => {
    const { x, y } = planckianChromaticity(3_000);
    expect(Math.hypot(x - 0.437, y - 0.404)).toBeLessThan(0.002);
  });
});

describe("starColour", () => {
  it("has unit luminance at every temperature", () => {
    const luminances = [2_000, 3_500, 5_772, 10_000, 30_000].map((t) => luminance(starColour(t)));
    expect(Math.max(...luminances.map((l) => Math.abs(l - 1)))).toBeLessThan(1e-12);
  });

  it("makes a cool star red and a hot star blue", () => {
    const cool = starColour(3_000);
    const hot = starColour(20_000);
    expect([cool[0] > cool[2], hot[2] > hot[0]]).toEqual([true, true]);
  });

  it("draws a star with no temperature white", () => {
    expect(starColour(null)).toEqual([1, 1, 1]);
  });

  it("draws a star with a temperature that is not finite white", () => {
    expect(starColour(Number.NaN)).toEqual([1, 1, 1]);
  });

  it("draws a star hotter than 25,000 K as a 25,000 K one", () => {
    expect(starColour(40_000)).toEqual(starColour(25_000));
  });
});
