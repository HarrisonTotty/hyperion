/**
 * An atmosphere as a list of medium terms (plan R05, Design note 16): the shape Hillaire's four
 * tables are built from, and the one R08 generalises to any composition.
 *
 * @remarks
 * Each term is a density profile over height, scattering and absorption coefficients per channel at
 * unit density, and a phase function, as Bevy 0.19's `ScatteringMedium` is a list of terms. A new
 * gas or aerosol is a new term, never a new model. The three channels are spectral samples at
 * {@link CHANNEL_WAVELENGTHS_NM}, not RGB: `solar.ts` turns them into Rec. 709 luminance.
 */

import type { Rgb } from "../photometry/toneCurve";

/**
 * The wavelengths of the three spectral channels, nm: 680, 550 and 440, red, green and blue.
 *
 * @remarks
 * Bruneton's `kLambdaR`, `kLambdaG` and `kLambdaB` (precomputed_atmospheric_scattering, 2017,
 * `atmosphere/model.h`), which Hillaire 2020 and sebh's reference code inherit. R08 refits them
 * (R08 Design note 8).
 */
export const CHANNEL_WAVELENGTHS_NM: Rgb = [680, 550, 440];

/**
 * A density given level by level (plan R08, Design note 2): R08's hydrostatic column (`column.ts`),
 * a polytrope rather than an exponential, and layers with a base and a top.
 *
 * @remarks
 * The density is linear in height between neighbouring levels and constant beyond the first and
 * the last, the rule a texture sampled with linear filtering and clamped edges follows, and the
 * one R08.T12.a's reference tracer reads a case's `tabulated` profile by
 * (`hyperion-fit`'s `atmosphere::case::DensityProfile::Tabulated`), so that the client and the
 * reference read the same air. The levels are checked by {@link tabulatedDensity}, as the tracer
 * checks a case's: at least two, heights finite and strictly ascending, and relative densities
 * finite and not negative, as many as the heights. Under R08 Design note 17 the heights are
 * gravity-scaled heights h\* = s h, as every profile's are on an oblate body.
 */
export interface TabulatedDensity {
  readonly kind: "tabulated";
  /** The levels' heights above the ground, m, strictly ascending. */
  readonly altitudesM: Float64Array;
  /** The relative density at each level. */
  readonly relative: Float64Array;
}

/**
 * How a term's density, relative to its value at the reference height, varies with height above
 * the ground.
 *
 * - `exponential`: e^(−h ÷ H) with scale height H, 1 at the ground.
 * - `tent`: zero below `bottomM` and above `topM`, rising linearly to 1 at `peakM` and falling
 *   linearly back (Bruneton 2017's ozone layer).
 * - `tabulated`: given level by level ({@link TabulatedDensity}).
 */
export type DensityProfile =
  | { readonly kind: "exponential"; readonly scaleHeightM: number }
  | {
      readonly kind: "tent";
      readonly bottomM: number;
      readonly peakM: number;
      readonly topM: number;
    }
  | TabulatedDensity;

/**
 * A tabulated density from its levels, checked.
 *
 * @remarks
 * The arrays are kept, not copied; the caller does not change them afterwards.
 *
 * @throws RangeError for fewer than two levels, arrays of different lengths, heights that are not
 *   finite and strictly ascending, or a relative density that is not finite and at least 0.
 */
export function tabulatedDensity(
  altitudesM: Float64Array,
  relative: Float64Array,
): TabulatedDensity {
  if (altitudesM.length < 2 || relative.length !== altitudesM.length) {
    throw new RangeError(
      `a tabulated density needs at least two levels and one density each, got ${altitudesM.length} heights and ${relative.length} densities`,
    );
  }
  for (let i = 0; i < altitudesM.length; i += 1) {
    const h = altitudesM[i] ?? Number.NaN;
    const below = i === 0 ? Number.NEGATIVE_INFINITY : (altitudesM[i - 1] ?? Number.NaN);
    if (!(Number.isFinite(h) && h > below)) {
      throw new RangeError(`a tabulated density's heights ascend strictly: ${h} m at level ${i}`);
    }
    const d = relative[i] ?? Number.NaN;
    if (!(Number.isFinite(d) && d >= 0)) {
      throw new RangeError(`a tabulated density is finite and not negative: ${d} at level ${i}`);
    }
  }
  return { kind: "tabulated", altitudesM, relative };
}

/**
 * The index i of the level at or below h, with h in [altitudesM[0], last), by bisection:
 * altitudesM[i] ≤ h < altitudesM[i + 1].
 */
function levelBelow(altitudesM: Float64Array, h: number): number {
  let low = 0;
  let high = altitudesM.length - 1;
  while (high - low > 1) {
    const middle = (low + high) >>> 1;
    if ((altitudesM[middle] ?? Number.NaN) <= h) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return low;
}

/** Level i's interval at h, linear between its ends. */
function withinLevel(profile: TabulatedDensity, i: number, h: number): number {
  const h0 = profile.altitudesM[i] ?? Number.NaN;
  const h1 = profile.altitudesM[i + 1] ?? Number.NaN;
  const d0 = profile.relative[i] ?? Number.NaN;
  const d1 = profile.relative[i + 1] ?? Number.NaN;
  return d0 + ((d1 - d0) * (h - h0)) / (h1 - h0);
}

/** A tabulated density at h: linear between levels, constant beyond the first and the last. */
function tabulatedAt(profile: TabulatedDensity, h: number): number {
  const heights = profile.altitudesM;
  const last = heights.length - 1;
  if (h <= (heights[0] ?? Number.NaN)) {
    return profile.relative[0] ?? Number.NaN;
  }
  if (h >= (heights[last] ?? Number.NaN)) {
    return profile.relative[last] ?? Number.NaN;
  }
  return withinLevel(profile, levelBelow(heights, h), h);
}

/**
 * A tabulated density's integral from the ground to `upToM`, m: the constant below the first
 * level, the trapezoids of the levels, which are exact for the linear interpolant, and the
 * constant above the last, each over the part of it below `upToM`.
 */
function tabulatedColumnM(profile: TabulatedDensity, upToM: number): number {
  const heights = profile.altitudesM;
  const last = heights.length - 1;
  const top = Math.max(upToM, 0);
  const first = heights[0] ?? Number.NaN;
  const final = heights[last] ?? Number.NaN;
  let length = 0;
  if (first > 0) {
    length += (profile.relative[0] ?? Number.NaN) * Math.min(top, first);
  }
  for (let i = 0; i < last; i += 1) {
    const low = Math.max(heights[i] ?? Number.NaN, 0);
    if (low >= top) {
      break;
    }
    const high = Math.min(heights[i + 1] ?? Number.NaN, top);
    if (high > low) {
      length += 0.5 * (high - low) * (withinLevel(profile, i, low) + withinLevel(profile, i, high));
    }
  }
  if (top > final) {
    length += (profile.relative[last] ?? Number.NaN) * (top - Math.max(final, 0));
  }
  return length;
}

/**
 * A term's phase function.
 *
 * - `rayleigh`: 3 ÷ (16π) × (1 + cos²θ).
 * - `cornette-shanks`: Cornette and Shanks 1992's form of Henyey–Greenstein with asymmetry g.
 * - `none`: an absorbing-only term, which scatters nothing.
 */
export type PhaseFunction =
  | { readonly kind: "rayleigh" }
  | { readonly kind: "cornette-shanks"; readonly asymmetry: number }
  | { readonly kind: "none" };

/** One constituent of an atmosphere: a gas, an aerosol or an absorbing layer. */
export interface MediumTerm {
  /** A short lower-case name, unique within its medium: `rayleigh`, `aerosol`, `ozone`. */
  readonly name: string;
  readonly density: DensityProfile;
  /** The scattering coefficient per channel at unit density, m⁻¹. */
  readonly scattering: Rgb;
  /** The absorption coefficient per channel at unit density, m⁻¹. */
  readonly absorption: Rgb;
  readonly phase: PhaseFunction;
}

/**
 * An atmosphere's medium above a body's ground, independent of the body's size and figure.
 *
 * @remarks
 * Heights are geodetic heights above the body's datum; the tables take r = √(MN) + h on the
 * spheroid (Design note 16), so the medium carries no radius of its own.
 */
export interface AtmosphereMedium {
  /** A name for logs and the comparison mode: `earth`, `hillaire-2020`. */
  readonly name: string;
  /** The top of the atmosphere above the ground, m; nothing is drawn above it. */
  readonly topHeightM: number;
  /** The ground's albedo per channel, Lambertian, in [0, 1]. */
  readonly groundAlbedo: Rgb;
  readonly terms: readonly MediumTerm[];
}

/**
 * A profile's relative density at a height above the ground.
 *
 * @param heightM - The height above the ground, m; negative heights are taken as the ground.
 */
export function densityAt(profile: DensityProfile, heightM: number): number {
  const h = Math.max(heightM, 0);
  let density: number;
  switch (profile.kind) {
    case "exponential":
      density = Math.exp(-h / profile.scaleHeightM);
      break;
    case "tent":
      if (h <= profile.bottomM || h >= profile.topM) {
        density = 0;
      } else if (h <= profile.peakM) {
        density = (h - profile.bottomM) / (profile.peakM - profile.bottomM);
      } else {
        density = (profile.topM - h) / (profile.topM - profile.peakM);
      }
      break;
    case "tabulated":
      density = tabulatedAt(profile, h);
      break;
  }
  return density;
}

/**
 * A profile's integral from the ground to a height, m: the vertical path length at unit density
 * that has the same optical depth.
 *
 * @param topM - The upper limit, m, at least 0.
 */
export function columnLengthM(profile: DensityProfile, topM: number): number {
  let length: number;
  switch (profile.kind) {
    case "exponential":
      length = profile.scaleHeightM * -Math.expm1(-topM / profile.scaleHeightM);
      break;
    case "tent":
      length = tentColumnM(profile.bottomM, profile.peakM, profile.topM, topM);
      break;
    case "tabulated":
      length = tabulatedColumnM(profile, topM);
      break;
  }
  return length;
}

/** A tent's integral from the ground to `upToM`: its rising side's area, then its falling side's. */
function tentColumnM(bottomM: number, peakM: number, tentTopM: number, upToM: number): number {
  const up = Math.min(Math.max(upToM, bottomM), peakM) - bottomM;
  const down = Math.min(Math.max(upToM, peakM), tentTopM) - peakM;
  return (up * up) / (2 * (peakM - bottomM)) + down - (down * down) / (2 * (tentTopM - peakM));
}

/** A term's extinction per channel at unit density, m⁻¹: scattering plus absorption. */
export function extinction(term: MediumTerm): Rgb {
  return [
    term.scattering[0] + term.absorption[0],
    term.scattering[1] + term.absorption[1],
    term.scattering[2] + term.absorption[2],
  ];
}

/**
 * The term of a medium with a name.
 *
 * @throws Error if the medium has no such term, a broken invariant of a constant medium.
 */
export function termNamed(medium: AtmosphereMedium, name: string): MediumTerm {
  const term = medium.terms.find((t) => t.name === name);
  if (term === undefined) {
    throw new Error(`atmosphere medium ${medium.name} has no term ${name}`);
  }
  return term;
}
