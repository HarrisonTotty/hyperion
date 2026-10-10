import { describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import { planckianChromaticity } from "../photometry/starColour";
import { type Rgb, XYZ_TO_SRGB } from "../photometry/toneCurve";
import colourMatching from "./colourMatching.json" with { type: "json" };
import { BAKE_BIN_WIDTH_NM, BAKE_RANGE_NM, BAKE_WAVELENGTHS_NM } from "./medium";
import {
  bakeSpectrumAt,
  bakeSpectrumOf,
  channelRgb,
  deltaUv,
  luminanceOfRgb,
  MATCHING_FUNCTIONS,
  spectralRgb,
  sunWeights,
  uvOfRgb,
} from "./spectralColour";

/** Bruneton's triple, R05's `CHANNEL_WAVELENGTHS_NM`. */
const BRUNETON_NM: Rgb = [680, 550, 440];

/** ȳ at a whole nm, back from the matching functions through the exact inverse of the matrix. */
function yBar(nm: number): number {
  const i = nm - MATCHING_FUNCTIONS.firstNm;
  return luminanceOfRgb([
    MATCHING_FUNCTIONS.red[i] ?? Number.NaN,
    MATCHING_FUNCTIONS.green[i] ?? Number.NaN,
    MATCHING_FUNCTIONS.blue[i] ?? Number.NaN,
  ]);
}

/** A row's sum. */
function sumOf(values: Float64Array): number {
  return values.reduce((s, x) => s + x, 0);
}

/** A response the air does not colour. */
function flat(): number {
  return 0.37;
}

/**
 * Planck's law in wavelength, in any units, since only the chromaticity is compared; the second
 * radiation constant hc ÷ k is 1.438776877 × 10⁷ nm K (CODATA 2018, exact).
 */
function planck(nm: number, kelvin: number): number {
  return 1 / (nm ** 5 * Math.expm1(1.438_776_877e7 / (nm * kelvin)));
}

describe("the matching functions", () => {
  it("were taken to Rec. 709 by toneCurve's XYZ_TO_SRGB", () => {
    expect(colourMatching.xyzToRgb).toEqual(XYZ_TO_SRGB);
  });

  it("record the CIE table's checksum", () => {
    expect(colourMatching.sha256).toBe(
      "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1",
    );
  });

  it("give back the CIE's ȳ: 1 at 555 nm and 3.9 × 10⁻⁵ at 380 nm", () => {
    expect(yBar(555)).toBeCloseTo(1, 9);
    expect(yBar(380)).toBeCloseTo(3.9e-5, 12);
  });

  it("hold the equal-energy white's colour to 10⁻³, x̄, ȳ and z̄ having equal sums", () => {
    const rows = XYZ_TO_SRGB.map((row) => row[0] + row[1] + row[2]);
    const green = sumOf(MATCHING_FUNCTIONS.green);
    expect(sumOf(MATCHING_FUNCTIONS.red) / green).toBeCloseTo((rows[0] ?? 0) / (rows[1] ?? 1), 3);
    expect(sumOf(MATCHING_FUNCTIONS.blue) / green).toBeCloseTo((rows[2] ?? 0) / (rows[1] ?? 1), 3);
  });
});

describe("a bake spectrum", () => {
  const ramp = bakeSpectrumOf(Array.from({ length: 15 }, (_, k) => k + 1));

  it("is each bin's mean at its centre", () => {
    const offsets = BAKE_WAVELENGTHS_NM.map((nm, k) =>
      Math.abs(bakeSpectrumAt(ramp, nm) - (k + 1)),
    );
    expect(Math.max(...offsets)).toBeLessThan(1e-12);
  });

  it("is linear between the centres", () => {
    expect(bakeSpectrumAt(ramp, BAKE_RANGE_NM[0] + 2 * BAKE_BIN_WIDTH_NM)).toBeCloseTo(2.5, 12);
  });

  it("holds the end bins' means out to the range's edges", () => {
    expect([bakeSpectrumAt(ramp, 380), bakeSpectrumAt(ramp, 760)]).toEqual([1, 15]);
  });

  it.each([379.9, 760.1, Number.NaN])("is refused at %s nm", (nm) => {
    expect(() => bakeSpectrumAt(ramp, nm)).toThrow(RangeError);
  });

  it.each([
    ["two values", [1, 2]],
    ["a negative value", [...Array.from({ length: 14 }, () => 1), -1]],
    ["a NaN", [...Array.from({ length: 14 }, () => 1), Number.NaN]],
  ])("is refused with %s", (_, values) => {
    expect(() => bakeSpectrumOf(values)).toThrow(RangeError);
  });
});

describe("the colour pipeline", () => {
  it.each(bakeSpectra.suns.map((sun) => [`${sun.teff_k} K (${sun.grid})`, sun.bake_spectrum]))(
    "draws a flat response in three channels exactly as spectrally under a %s star",
    (_, values) => {
      const spectrum = bakeSpectrumOf(values);
      const spectral = spectralRgb(spectrum, flat);
      const drawn = channelRgb(sunWeights(spectrum).rgb, flat, BRUNETON_NM);
      expect(deltaUv(drawn, spectral)).toBeLessThan(1e-12);
    },
  );

  it("puts D65's white, equal linear Rec. 709, at D65's u′v′ (0.1978, 0.4683)", () => {
    const [u, v] = uvOfRgb([1, 1, 1]);
    // Rec. 709's white, D65 at x 0.3127 and y 0.3290 (ITU-R BT.709-6, item 1.4), in u′v′.
    expect(u).toBeCloseTo(0.197_83, 4);
    expect(v).toBeCloseTo(0.468_32, 4);
  });

  it.each([3_000, 5_772, 10_000, 20_000])(
    "gives a %s K black body binned to 15 means its Planckian chromaticity within 0.002",
    (kelvin) => {
      const means = BAKE_WAVELENGTHS_NM.map((centre) => {
        let sum = 0;
        for (let i = 0; i < 100; i += 1) {
          sum += planck(
            centre - BAKE_BIN_WIDTH_NM / 2 + ((i + 0.5) * BAKE_BIN_WIDTH_NM) / 100,
            kelvin,
          );
        }
        return sum / 100;
      });
      const [u, v] = uvOfRgb(sunWeights(bakeSpectrumOf(means)).rgb);
      const { x, y } = planckianChromaticity(kelvin);
      const d = -2 * x + 12 * y + 3;
      expect(Math.hypot(u - (4 * x) / d, v - (9 * y) / d)).toBeLessThan(0.002);
    },
  );
});
