import { describe, expect, it } from "vitest";

import { planckBakeSpectrum, SUN_TEFF_K } from "../../../test/atmosphereFixtures";
import { BAKE_BIN_COUNT } from "../medium";
import { type BakeSpectrum, bakeSpectrumOf, sunWeights } from "../spectralColour";
import { binConversion, binOfRow, binsToChannels } from "./binConversion";

/** The suns the conversion is held under: the Sun, and Planck suns of 2,500 K and 30,000 K. */
const SUNS: ReadonlyArray<readonly [string, BakeSpectrum]> = [
  ["the Sun", planckBakeSpectrum(SUN_TEFF_K)],
  ["a 2,500 K sun", planckBakeSpectrum(2_500)],
  ["a 30,000 K sun", planckBakeSpectrum(30_000)],
];

/** A star of light only in the bins listed. */
function only(bins: ReadonlyArray<number>): BakeSpectrum {
  return bakeSpectrumOf(
    Array.from({ length: BAKE_BIN_COUNT }, (_, b) => (bins.includes(b) ? 1 : 0)),
  );
}

/** A star's red colour. */
function redOf(spectrum: BakeSpectrum): number {
  return sunWeights(spectrum).rgb[0];
}

/** The largest relative difference of a colour's channels from one value. */
function worstRelative(rgb: ReadonlyArray<number>, value: number): number {
  return Math.max(...rgb.map((channel) => Math.abs(channel - value) / Math.abs(value)));
}

describe("the conversion of the bins to the channels", () => {
  it.for(SUNS)("converts a flat spectrum to itself under %s, to 10⁻¹²", ([, spectrum]) => {
    const conversion = binConversion(spectrum);
    expect(conversion.guarded).toEqual([false, false, false]);
    for (const value of [1, 3.7e-6, 250]) {
      const flat = new Float64Array(BAKE_BIN_COUNT).fill(value);
      expect(worstRelative(binsToChannels(conversion, flat), value)).toBeLessThan(1e-12);
    }
  });

  it("weights each bin by the star's colour over its rows, so that the kernels' multiply recovers Σ c̄ S X", () => {
    const spectrum = planckBakeSpectrum(SUN_TEFF_K);
    const conversion = binConversion(spectrum);
    const weights = sunWeights(spectrum);
    const values = Float64Array.from({ length: BAKE_BIN_COUNT }, (_, b) => 1 + b * b);
    const converted = binsToChannels(conversion, values);
    const channels = [weights.red, weights.green, weights.blue];
    for (const [c, channel] of channels.entries()) {
      let direct = 0;
      for (const [i, w] of channel.entries()) {
        direct += w * (values[binOfRow(380 + i)] ?? Number.NaN);
      }
      const colour = weights.rgb[c] ?? Number.NaN;
      expect(
        Math.abs((converted[c] ?? Number.NaN) * colour - direct) / Math.abs(direct),
      ).toBeLessThan(1e-12);
    }
  });

  it("counts each row in its bin, the edges' rows in the bin above and 760 nm in the last", () => {
    expect([380, 405, 406, 455, 456, 532, 608, 684, 759, 760].map(binOfRow)).toEqual([
      0, 0, 1, 2, 3, 6, 9, 12, 14, 14,
    ]);
  });

  it("takes the spectrum's own mean for a channel whose star colour is near 0, which also converts a flat spectrum to itself", () => {
    // A star of light only in bin 10 (about 646 nm), where r̄ > 0, and in bins 3–5 (456–532 nm),
    // where r̄ < 0, mixed so that its red colour cancels.
    const warm = only([10]);
    const cool = only([3, 4, 5]);
    const mix = -redOf(warm) / redOf(cool);
    expect(mix).toBeGreaterThan(0);
    const spectrum = bakeSpectrumOf(
      Array.from({ length: BAKE_BIN_COUNT }, (_, b) => (warm[b] ?? 0) + mix * (cool[b] ?? 0)),
    );
    expect(Math.abs(redOf(spectrum))).toBeLessThan(1e-9 * Math.max(...sunWeights(spectrum).rgb));
    const conversion = binConversion(spectrum);
    expect(conversion.guarded).toEqual([true, false, false]);
    const flat = new Float64Array(BAKE_BIN_COUNT).fill(0.42);
    expect(worstRelative(binsToChannels(conversion, flat), 0.42)).toBeLessThan(1e-12);
  });
});
