/**
 * The colour pipeline (plan R08, Design note 5; R08.T4.a): a star's light through the air, from its
 * spectrum to linear Rec. 709, spectrally or in three channels, and the chromaticity they are
 * compared in.
 *
 * @remarks
 * The spectral colour of light S(λ) R(λ), S a star's spectrum and R what the air does to it, is
 * Σ c̄(λ) S(λ) R(λ) Δλ over the bake range's 1 nm rows, c̄ the CIE 1931 2° matching functions taken
 * to linear Rec. 709 (`colourMatching.json`, adapted from the CIE's table by `src/tools/
 * atmosphereData.ts`; CIE 2019, DOI 10.25039/CIE.DS.xvudnb9b, CC BY-SA 4.0), and S interpolated
 * from the star's 15 bin means ({@link bakeSpectrumAt}). The three-channel colour is the star's own
 * colour, the same sum with R = 1, times R at the three wavelengths. A flat R gives the two the
 * same colour exactly. `channels.ts` fits the channels' wavelengths with it; R08.T4.b's curves of
 * growth and the spectral bakes reduce to the channels through it. It imports nothing of the fit,
 * so that a worker that reads it does not bundle the fit's family.
 */

import type { HostDiscDto } from "@hyperion/protocol";

import { applyRows, inverseRows, type Rgb, type Rows3, XYZ_TO_SRGB } from "../photometry/toneCurve";
import matching from "./colourMatching.json" with { type: "json" };
import { BAKE_BIN_COUNT, BAKE_BIN_WIDTH_NM, BAKE_RANGE_NM } from "./medium";

/** The CIE 1931 2° matching functions as linear Rec. 709, at 1 nm. */
export interface MatchingFunctions {
  /** The first row's wavelength, nm. */
  readonly firstNm: number;
  /** r̄ at `firstNm + i` nm. */
  readonly red: Float64Array;
  /** ḡ at `firstNm + i` nm. */
  readonly green: Float64Array;
  /** b̄ at `firstNm + i` nm. */
  readonly blue: Float64Array;
}

function readMatching(file: {
  readonly firstNm: number;
  readonly stepNm: number;
  readonly rows: ReadonlyArray<ReadonlyArray<number>>;
}): MatchingFunctions {
  if (file.stepNm !== 1) {
    throw new Error(`colourMatching.json's rows are 1 nm apart, not ${file.stepNm}`);
  }
  const n = file.rows.length;
  const red = new Float64Array(n);
  const green = new Float64Array(n);
  const blue = new Float64Array(n);
  for (const [i, row] of file.rows.entries()) {
    const [nm, r, g, b] = row;
    if (nm !== file.firstNm + i || r === undefined || g === undefined || b === undefined) {
      throw new Error(`colourMatching.json's row ${i} is not [${file.firstNm + i}, r, g, b]`);
    }
    red[i] = r;
    green[i] = g;
    blue[i] = b;
  }
  return { firstNm: file.firstNm, red, green, blue };
}

/**
 * The CIE 1931 2° colour-matching functions as linear Rec. 709 r̄, ḡ, b̄, 360–830 nm at 1 nm:
 * `colourMatching.json`, which `scripts/atmosphereData.mjs matching` writes from the CIE's
 * `CIE_xyz_1931_2deg.csv` (CIE 2019, Colour-matching functions of CIE 1931 standard colorimetric
 * observer, International Commission on Illumination (CIE), Vienna, AT, DOI:
 * 10.25039/CIE.DS.xvudnb9b; CC BY-SA 4.0) through {@link XYZ_TO_SRGB}.
 */
export const MATCHING_FUNCTIONS: MatchingFunctions = readMatching(matching);

/**
 * The matching functions r̄, ḡ, b̄ at any wavelength of their table, linear between its 1 nm rows:
 * the weights a grid finer than 1 nm reads (R08.T4.b's methane, at 0.25 nm).
 *
 * @throws RangeError outside the table's 360–830 nm.
 */
export function matchingFunctionsAt(wavelengthNm: number): Rgb {
  const { firstNm, red, green, blue } = MATCHING_FUNCTIONS;
  const x = wavelengthNm - firstNm;
  if (!(x >= 0 && x <= red.length - 1)) {
    throw new RangeError(
      `the matching functions span ${firstNm}–${firstNm + red.length - 1} nm, not ${wavelengthNm} nm`,
    );
  }
  const k = Math.min(Math.floor(x), red.length - 2);
  const t = x - k;
  const at = (row: Float64Array): number =>
    (row[k] ?? Number.NaN) * (1 - t) + (row[k + 1] ?? Number.NaN) * t;
  return [at(red), at(green), at(blue)];
}

/** Linear Rec. 709 to CIE XYZ: the exact inverse of the matrix the matching functions were made with. */
export const REC709_TO_XYZ: Rows3 = inverseRows(XYZ_TO_SRGB);

/**
 * A star's spectrum in the bake bins: `HostDiscDto.bake_spectrum`, R06's mean spectral irradiance
 * over each bin, W m⁻² nm⁻¹ per lux (R06.T3.c).
 */
export type BakeSpectrum = Readonly<HostDiscDto["bake_spectrum"]>;

/**
 * A bake spectrum from a list of its 15 values, as a fixture or a file holds it.
 *
 * @throws RangeError unless there are 15 values, each finite and not negative.
 */
export function bakeSpectrumOf(values: ReadonlyArray<number>): BakeSpectrum {
  const [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o] = values;
  if (
    values.length !== BAKE_BIN_COUNT ||
    !values.every((v) => Number.isFinite(v) && v >= 0) ||
    a === undefined ||
    b === undefined ||
    c === undefined ||
    d === undefined ||
    e === undefined ||
    f === undefined ||
    g === undefined ||
    h === undefined ||
    i === undefined ||
    j === undefined ||
    k === undefined ||
    l === undefined ||
    m === undefined ||
    n === undefined ||
    o === undefined
  ) {
    throw new RangeError(
      `a bake spectrum is ${BAKE_BIN_COUNT} finite values, none negative; got ${values.length}`,
    );
  }
  return [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o];
}

/**
 * A star's spectral irradiance at a wavelength from its 15 bin means: linear between the bins'
 * centres, and each end bin's mean between its centre and the range's edge.
 *
 * @remarks
 * The rule R08.T4.b's curves of growth interpolate S by (Design note 5). It does not keep each
 * bin's mean exactly; a stated approximation, adequate for FGK, A and B stars and not for M dwarfs'
 * TiO bands (R08's Risks).
 *
 * @throws RangeError outside {@link BAKE_RANGE_NM}.
 */
export function bakeSpectrumAt(spectrum: BakeSpectrum, wavelengthNm: number): number {
  const [low, high] = BAKE_RANGE_NM;
  if (!(wavelengthNm >= low && wavelengthNm <= high)) {
    throw new RangeError(`a bake spectrum spans ${low}–${high} nm, not ${wavelengthNm} nm`);
  }
  const x = (wavelengthNm - low) / BAKE_BIN_WIDTH_NM - 0.5;
  if (x <= 0) {
    return spectrum[0];
  }
  if (x >= BAKE_BIN_COUNT - 1) {
    return spectrum[14];
  }
  const k = Math.floor(x);
  const t = x - k;
  return (spectrum[k] ?? Number.NaN) * (1 - t) + (spectrum[k + 1] ?? Number.NaN) * t;
}

/** The rows the colour pipeline sums: every whole nm of {@link BAKE_RANGE_NM}. */
export const PIPELINE_WAVELENGTHS_NM: Float64Array = Float64Array.from(
  { length: BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1 },
  (_, i) => BAKE_RANGE_NM[0] + i,
);

/**
 * A star's colour weights on the pipeline's rows: c̄(λ) S(λ) Δλ per channel, the trapezoid rule's
 * weights at 1 nm, and their sum, the star's own linear Rec. 709 colour over the bake range.
 */
export interface SunWeights {
  readonly red: Float64Array;
  readonly green: Float64Array;
  readonly blue: Float64Array;
  /** The star's colour: the weights' sums. */
  readonly rgb: Rgb;
}

/** A star's {@link SunWeights}. */
export function sunWeights(spectrum: BakeSpectrum): SunWeights {
  const n = PIPELINE_WAVELENGTHS_NM.length;
  const red = new Float64Array(n);
  const green = new Float64Array(n);
  const blue = new Float64Array(n);
  const offset = BAKE_RANGE_NM[0] - MATCHING_FUNCTIONS.firstNm;
  const sums = [0, 0, 0];
  for (const [i, nm] of PIPELINE_WAVELENGTHS_NM.entries()) {
    const trapezoid = i === 0 || i === n - 1 ? 0.5 : 1;
    const s = bakeSpectrumAt(spectrum, nm) * trapezoid;
    const r = (MATCHING_FUNCTIONS.red[offset + i] ?? Number.NaN) * s;
    const g = (MATCHING_FUNCTIONS.green[offset + i] ?? Number.NaN) * s;
    const b = (MATCHING_FUNCTIONS.blue[offset + i] ?? Number.NaN) * s;
    red[i] = r;
    green[i] = g;
    blue[i] = b;
    sums[0] = (sums[0] ?? 0) + r;
    sums[1] = (sums[1] ?? 0) + g;
    sums[2] = (sums[2] ?? 0) + b;
  }
  return { red, green, blue, rgb: [sums[0] ?? 0, sums[1] ?? 0, sums[2] ?? 0] };
}

/** The colour of a star's light times a response sampled on the pipeline's rows. */
export function weightedRgb(weights: SunWeights, response: Float64Array): Rgb {
  let r = 0;
  let g = 0;
  let b = 0;
  for (const [i, value] of response.entries()) {
    r += (weights.red[i] ?? Number.NaN) * value;
    g += (weights.green[i] ?? Number.NaN) * value;
    b += (weights.blue[i] ?? Number.NaN) * value;
  }
  return [r, g, b];
}

/**
 * The linear Rec. 709 colour of a star's light after the air: Σ c̄ S R Δλ over the bake range.
 *
 * @param response - R(λ) at a vacuum wavelength in nm: what the air passes or scatters towards the
 *   eye per unit of the star's light.
 */
export function spectralRgb(
  spectrum: BakeSpectrum,
  response: (wavelengthNm: number) => number,
): Rgb {
  return weightedRgb(sunWeights(spectrum), PIPELINE_WAVELENGTHS_NM.map(response));
}

/**
 * The three-channel colour of the same light: the star's colour times R at each channel's
 * wavelength, as the per-frame tables draw it.
 */
export function channelRgb(
  starColour: Rgb,
  response: (wavelengthNm: number) => number,
  wavelengthsNm: Rgb,
): Rgb {
  return [
    starColour[0] * response(wavelengthsNm[0]),
    starColour[1] * response(wavelengthsNm[1]),
    starColour[2] * response(wavelengthsNm[2]),
  ];
}

/**
 * A linear Rec. 709 colour's CIE 1976 chromaticity (u′, v′) = (4X, 9Y) ÷ (X + 15Y + 3Z)
 * (CIE 015:2018, §8.1).
 *
 * @remarks
 * A colour outside the gamut is still a colour: a negative channel is kept, as the pipeline's
 * signed matching functions give it.
 */
export function uvOfRgb(rgb: Rgb): readonly [number, number] {
  const [x, y, z] = applyRows(REC709_TO_XYZ, rgb);
  const d = x + 15 * y + 3 * z;
  return [(4 * x) / d, (9 * y) / d];
}

/** The distance between two colours' (u′, v′). */
export function deltaUv(a: Rgb, b: Rgb): number {
  const [ua, va] = uvOfRgb(a);
  const [ub, vb] = uvOfRgb(b);
  return Math.hypot(ua - ub, va - vb);
}

/** The Rec. 709 luminance of a linear colour: the Y row of the exact inverse. */
export function luminanceOfRgb(rgb: Rgb): number {
  return applyRows(REC709_TO_XYZ, rgb)[1];
}
