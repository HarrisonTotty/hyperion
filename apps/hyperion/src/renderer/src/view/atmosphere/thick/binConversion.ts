/**
 * A spectral bake's values in the 15 bins converted to the render channels on storage (plan R08,
 * R08.T14.b; Design note 5, "Spectral bakes"), per light slot.
 *
 * @remarks
 * A bake's value in bin b, X_b, is per unit of the star's light in that bin. The channel the
 * kernels store is X_c = Σ_b W_cb X_b ÷ C_c, with W_cb = Σ c̄_c(λ) S(λ) Δλ over the colour
 * pipeline's 1 nm rows in bin b (`spectralColour.ts`'s `sunWeights`, its trapezoid weights) and
 * C_c = Σ_b W_cb the star's own linear Rec. 709 colour, so that the kernels multiply X_c by the
 * star's colour as they multiply every table (`channelRgb`'s convention) and recover
 * Σ c̄ S X Δλ. A flat X, the same in every bin, converts to itself.
 *
 * Each row counts in its bin, 760 nm in the last, as `absorbers.ts`'s bins count their grid's
 * points: the bin of row λ is ⌊15 (λ − 380) ÷ 380⌋, in whole numbers, so that the rows on the bins'
 * edges (456, 532, 608 and 684 nm) fall in the bin above.
 *
 * **The guard.** The matching functions are signed, so a star's colour in a channel can be near 0
 * or below it, and dividing by it would amplify the bake's values without bound. A channel whose
 * colour is at most {@link CONVERSION_GUARD} of the star's largest takes the spectrum's own mean
 * over the bins in its place, Σ_b S_b X_b ÷ Σ_b S_b (the bins are equally wide), which also
 * converts a flat X to itself; the light it carries is that small channel's, near none.
 */

import type { Rgb } from "../../photometry/toneCurve";
import { BAKE_BIN_COUNT, BAKE_RANGE_NM } from "../medium";
import { type BakeSpectrum, PIPELINE_WAVELENGTHS_NM, sunWeights } from "../spectralColour";

/** The share of the star's largest channel at or below which a channel is guarded (the module's remarks). */
export const CONVERSION_GUARD = 1e-6;

/** A light's conversion from the bins to the channels. */
export interface BinConversion {
  /** W_cb ÷ C_c, per channel (red, green, blue), one a bin: the factors each bin's value takes. */
  readonly factors: readonly [Float64Array, Float64Array, Float64Array];
  /** Whether each channel took the guard's mean. */
  readonly guarded: readonly [boolean, boolean, boolean];
}

/** The bin of a pipeline row, nm: ⌊15 (λ − 380) ÷ 380⌋, the last bin for 760 nm. */
export function binOfRow(wavelengthNm: number): number {
  const [low, high] = BAKE_RANGE_NM;
  const bin = Math.floor((BAKE_BIN_COUNT * (wavelengthNm - low)) / (high - low));
  return Math.min(Math.max(bin, 0), BAKE_BIN_COUNT - 1);
}

/** A light's {@link BinConversion} (the module's remarks). */
export function binConversion(spectrum: BakeSpectrum): BinConversion {
  const weights = sunWeights(spectrum);
  const sums = [weights.red, weights.green, weights.blue].map((channel) => {
    const perBin = new Float64Array(BAKE_BIN_COUNT);
    for (const [i, nm] of PIPELINE_WAVELENGTHS_NM.entries()) {
      const bin = binOfRow(nm);
      perBin[bin] = (perBin[bin] ?? 0) + (channel[i] ?? Number.NaN);
    }
    return perBin;
  });
  const largest = Math.max(...weights.rgb.map(Math.abs));
  let spectrumSum = 0;
  for (const value of spectrum) {
    spectrumSum += value;
  }
  const guarded: boolean[] = [];
  const factors = sums.map((perBin, c) => {
    const colour = weights.rgb[c] ?? Number.NaN;
    const guard = !(Math.abs(colour) > CONVERSION_GUARD * largest);
    guarded.push(guard);
    return guard
      ? Float64Array.from(spectrum, (value) => value / spectrumSum)
      : perBin.map((w) => w / colour);
  });
  const [red, green, blue] = factors;
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("a conversion has three channels");
  }
  return {
    factors: [red, green, blue],
    guarded: [guarded[0] === true, guarded[1] === true, guarded[2] === true],
  };
}

/** One value a bin, converted to the channels. */
export function binsToChannels(conversion: BinConversion, values: ArrayLike<number>): Rgb {
  const channel = (factors: Float64Array): number => {
    let sum = 0;
    for (const [b, factor] of factors.entries()) {
      sum += factor * (values[b] ?? Number.NaN);
    }
    return sum;
  };
  return [
    channel(conversion.factors[0]),
    channel(conversion.factors[1]),
    channel(conversion.factors[2]),
  ];
}
