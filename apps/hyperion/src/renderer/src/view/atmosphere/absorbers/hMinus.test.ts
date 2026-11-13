import { describe, expect, it } from "vitest";

import {
  H_MINUS_FREE_FREE_RANGE_K,
  H_MINUS_THRESHOLD_UM,
  hMinusBoundFreeM2,
  hMinusFreeFreeCm4PerDyn,
  hMinusFreeFreeM5,
} from "./hMinus";

/** hc ÷ k, µm K (CODATA 2018). */
const SECOND_RADIATION_UM_K = 14_387.768_775;

/**
 * John 1988's Eq. 3: the bound–free coefficient per H atom and unit electron pressure, cm⁴ dyn⁻¹,
 * from the cross-section per H⁻ with its stimulated-emission factor, by the Saha factor
 * 0.750 T^(−5/2) e^(α ÷ λ₀T).
 */
function boundFreeCoefficientCm4PerDyn(wavelengthNm: number, temperatureK: number): number {
  const sigmaCm2 = hMinusBoundFreeM2(wavelengthNm, temperatureK) * 1e4;
  return (
    0.75 *
    temperatureK ** -2.5 *
    Math.exp(SECOND_RADIATION_UM_K / (H_MINUS_THRESHOLD_UM * temperatureK)) *
    sigmaCm2
  );
}

describe("H⁻'s bound–free absorption", () => {
  // John 1988, Table 1, ×10⁻²⁷ cm⁴ dyn⁻¹, at θ = 5040 ÷ T of 1.0 and 2.0 (5,040 and 2,520 K). The
  // table carries its free–free part at a tenth of his Eq. 6 (the free–free test below), so the
  // bound–free here is the table less a tenth of Eq. 6: 0.2–0.4% of it at θ 1, less at θ 2.
  it.each([
    [400, 1.0, 54.55],
    [500, 1.0, 69.12],
    [700, 1.0, 88.53],
    [400, 2.0, 1_753],
    [500, 2.0, 2_226],
    [700, 2.0, 2_887],
  ])("matches John 1988's Table 1 at %s nm and θ %s to 0.5%", (nm, theta, table) => {
    const t = 5_040 / theta;
    const total = boundFreeCoefficientCm4PerDyn(nm, t) + hMinusFreeFreeCm4PerDyn(nm, t) / 10;
    expect(Math.abs(total / (table * 1e-27) - 1)).toBeLessThan(0.005);
  });

  it("peaks near 850 nm at 4.0 × 10⁻¹⁷ cm² (Wishart 1979, as John fits it)", () => {
    const peak = Array.from({ length: 400 }, (_, i) => 700 + i).reduce(
      (best, nm) => {
        const sigma = hMinusBoundFreeM2(nm, 100) * 1e4;
        return sigma > best.sigma ? { nm, sigma } : best;
      },
      { nm: 0, sigma: 0 },
    );
    expect([Math.abs(peak.nm - 851) <= 2, peak.sigma]).toEqual([true, expect.closeTo(4.0e-17, 18)]);
  });

  it("takes the stimulated-emission factor 1 − e^(−hc ÷ λkT) as its only temperature dependence", () => {
    const factor = -Math.expm1(-SECOND_RADIATION_UM_K / (0.76 * 2_500));
    expect(hMinusBoundFreeM2(760, 2_500) / hMinusBoundFreeM2(760, 100)).toBeCloseTo(factor, 12);
    expect(factor).toBeCloseTo(0.9995, 4);
  });

  it("is 0 past the photodetachment threshold, 1,641.9 nm", () => {
    expect(hMinusBoundFreeM2(1_650, 3_000)).toBe(0);
  });

  it.each([
    [100, 3_000],
    [Number.NaN, 3_000],
    [500, 0],
    [500, Number.NaN],
  ])("refuses %s nm at %s K", (nm, t) => {
    expect(() => hMinusBoundFreeM2(nm, t)).toThrow(RangeError);
  });
});

describe("H⁻'s free–free absorption", () => {
  // Bell and Berrington 1987 (J. Phys. B 20, 801), ×10⁻²⁶ cm⁴ dyn⁻¹, at λ = 91.134 ÷ k nm, as
  // Kurucz's ATLAS9 carries their table (atlas9.for, HMINOP, FFBEG's rows k = 0.20, 0.16 and
  // 0.12, at θ 1.0 and 2.0), which John's Eq. 6 is fitted to "to 1% and better".
  it.each([
    [91.134 / 0.2, 1.0, 0.161],
    [91.134 / 0.2, 2.0, 0.327],
    [91.134 / 0.16, 1.0, 0.241],
    [91.134 / 0.16, 2.0, 0.475],
    [91.134 / 0.12, 1.0, 0.409],
    [91.134 / 0.12, 2.0, 0.774],
  ])("matches Bell and Berrington at %s nm and θ %s to 1%", (nm, theta, value) => {
    expect(Math.abs(hMinusFreeFreeCm4PerDyn(nm, 5_040 / theta) / (value * 1e-26) - 1)).toBeLessThan(
      0.01,
    );
  });

  // John 1988, Table 1's 2.0 µm row, ×10⁻²⁷ cm⁴ dyn⁻¹, past the threshold so free–free alone:
  // 2.610 at θ 1.0 and 4.584 at θ 2.0, a tenth of his own Eq. 6 (and of Bell and Berrington).
  it.each([
    [1.0, 2.61],
    [2.0, 4.584],
  ])(
    "is ten times John 1988's Table 1 at 2.0 µm and θ %s, the table's units slip",
    (theta, table) => {
      const ratio = hMinusFreeFreeCm4PerDyn(2_000, 5_040 / theta) / (table * 1e-27);
      expect(Math.abs(ratio / 10 - 1)).toBeLessThan(0.01);
    },
  );

  it("gives the free–free shares John's §4 states below 0.9 µm: 34%, about 6% and under 1%", () => {
    const share = (theta: number): number => {
      const t = 5_040 / theta;
      const ff = hMinusFreeFreeCm4PerDyn(900, t);
      return ff / (ff + boundFreeCoefficientCm4PerDyn(900, t));
    };
    expect([share(0.5), share(1.0), share(2.0)]).toEqual([
      expect.closeTo(0.34, 2),
      expect.closeTo(0.06, 2),
      expect.closeTo(0.003, 3),
    ]);
  });

  it("is the pair cross-section k_ff k_B T, m⁵", () => {
    const k = 1.380_649e-16;
    expect(hMinusFreeFreeM5(600, 2_500)).toBeCloseTo(
      hMinusFreeFreeCm4PerDyn(600, 2_500) * k * 2_500 * 1e-10,
      70,
    );
  });

  it("holds over 1,400–10,080 K, 0.5 ≤ 5040 ÷ T ≤ 3.6", () => {
    expect(H_MINUS_FREE_FREE_RANGE_K).toEqual([1_400, 10_080]);
  });

  it.each([
    [150, 3_000],
    [500, -1],
  ])("refuses %s nm at %s K", (nm, t) => {
    expect(() => hMinusFreeFreeCm4PerDyn(nm, t)).toThrow(RangeError);
  });
});
