import { describe, expect, it } from "vitest";

import { heMinusFreeFreeCm4PerDyn, heMinusFreeFreeM5 } from "./heliumMinus";

/**
 * John 1994's Table 2 (Mon. Not. R. Astron. Soc. 269, 871), K_λ ×10⁻²⁶ cm⁴ dyn⁻¹ at θ 1.0 and 1.2,
 * on the rows 569.5 and 650.9 nm.
 */
const CORNERS = {
  shortUm: 0.5695,
  longUm: 0.6509,
  short: [0.061, 0.067],
  long: [0.077, 0.086],
} as const;

/** John's scaled coefficient A = λ⁻² K at a row, his Eq. 16. */
function scaledA(k: number, rowUm: number): number {
  return k / (rowUm * rowUm);
}

describe("He⁻'s free–free coefficient", () => {
  it("interpolates A = λ⁻² K linearly in 1/λ between rows and in θ between columns", () => {
    // 600 nm and θ 1.1 (T = 5040 ÷ 1.1 K), between both pairs of the corners.
    const um = 0.6;
    const shortA =
      0.5 *
      (scaledA(CORNERS.short[0], CORNERS.shortUm) + scaledA(CORNERS.short[1], CORNERS.shortUm));
    const longA =
      0.5 * (scaledA(CORNERS.long[0], CORNERS.longUm) + scaledA(CORNERS.long[1], CORNERS.longUm));
    const t = (1 / um - 1 / CORNERS.longUm) / (1 / CORNERS.shortUm - 1 / CORNERS.longUm);
    const expected = (longA + (shortA - longA) * t) * um * um * 1e-26;
    expect(heMinusFreeFreeCm4PerDyn(600, 5_040 / 1.1)).toBeCloseTo(expected, 36);
  });

  it("takes the table's own value on a row and a column", () => {
    // 650.9 nm at θ 2.0, 2,520 K: 0.117 × 10⁻²⁶.
    expect(heMinusFreeFreeCm4PerDyn(650.9, 2_520)).toBeCloseTo(0.117e-26, 36);
  });

  it("holds A = λ⁻² K below the table's shortest row, 506.3 nm", () => {
    const ratio = heMinusFreeFreeCm4PerDyn(400, 5_040) / heMinusFreeFreeCm4PerDyn(506.3, 5_040);
    expect(ratio).toBeCloseTo((400 / 506.3) ** 2, 12);
  });

  it("holds the table's coldest column below 1,400 K", () => {
    expect(heMinusFreeFreeCm4PerDyn(700, 1_000)).toBe(heMinusFreeFreeCm4PerDyn(700, 5_040 / 3.6));
  });

  it("holds the table's hottest column above 10,080 K", () => {
    expect(heMinusFreeFreeCm4PerDyn(700, 20_000)).toBe(heMinusFreeFreeCm4PerDyn(700, 10_080));
  });

  it.each([
    ["a wavelength past the 911.3 nm row", 950, 5_040],
    ["a wavelength that is not finite", Number.NaN, 5_040],
    ["a temperature of 0 K", 600, 0],
    ["a temperature that is not finite", 600, Number.POSITIVE_INFINITY],
  ])("refuses %s", (_, nm, temperatureK) => {
    expect(() => heMinusFreeFreeCm4PerDyn(nm, temperatureK)).toThrow(RangeError);
  });
});

describe("the He–e⁻ pair's cross-section", () => {
  it("is K k_B T, in m⁵ a pair", () => {
    const k = 1.380_649e-16;
    const expected = 0.117e-26 * k * 2_520 * 1e-10;
    expect(Math.abs(heMinusFreeFreeM5(650.9, 2_520) / expected - 1)).toBeLessThan(1e-9);
  });
});
