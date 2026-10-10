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
 * `atmosphere/model.h`), which Hillaire 2020 and sebh's reference code inherit, and the wavelengths
 * R05's Earth constants are evaluated at. R08.T4.a fits the triple the smooth terms are evaluated
 * at (`channels.ts`, recorded in `channels.json`; R08 Design note 5); R08.T6.d, which rebuilds Earth
 * at it, makes this constant read `fittedChannels.ts`'s `FITTED_CHANNEL_WAVELENGTHS_NM`.
 */
export const CHANNEL_WAVELENGTHS_NM: Rgb = [680, 550, 440];

/** The span of the bake bins, nm: 380 to 760 (R08 Design note 5). */
export const BAKE_RANGE_NM: readonly [number, number] = [380, 760];

/** The number of bake bins: 15, R06's `SKY_BAKE_BINS`. */
export const BAKE_BIN_COUNT = 15;

/** Each bake bin's width, nm: 380 ÷ 15 = 25.33. */
export const BAKE_BIN_WIDTH_NM = (BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0]) / BAKE_BIN_COUNT;

/**
 * The centres of the 15 bins every off-frame bake is solved in, nm: 392.67 + 25.33 k, k = 0..14,
 * each {@link BAKE_BIN_WIDTH_NM} wide over {@link BAKE_RANGE_NM} (R08 Design note 5, after
 * Bruneton 2017's use of bin midpoints).
 *
 * @remarks
 * R06's `sky::colour::BAKE_WAVELENGTHS_NM` mirrors it, and a star's `HostDiscDto.bake_spectrum` is
 * its spectrum's mean over each bin. Both are tested against
 * `packages/protocol/fixtures/bake_wavelengths_nm.json` (here in `medium.test.ts`).
 */
export const BAKE_WAVELENGTHS_NM: ReadonlyArray<number> = Array.from(
  { length: BAKE_BIN_COUNT },
  (_, k) => BAKE_RANGE_NM[0] + (k + 0.5) * BAKE_BIN_WIDTH_NM,
);

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
 * A phase function given entry by entry, per channel (plan R08, Design note 6): a Mie,
 * literature or aggregate phase function, which no closed form carries.
 *
 * @remarks
 * The entries stand at u = √(θ ÷ π), θ the scattering angle, so that a 256-entry table spaced
 * evenly in u puts its entries densest near the forward peak, where a 1 ÷ x-wide diffraction peak
 * would starve on a cos θ grid. The phase is linear in u between neighbouring entries
 * ({@link phaseAt}). The values are per steradian, each channel normalised so that its integral
 * over the sphere is 1, as the closed forms are; the table's builder (R08.T5.c) holds that, and
 * {@link phaseTable} checks only the shape. The arrays are kept, not copied.
 *
 * The drawn image is scalar and reads `values`, the scattering matrix's a₁, alone. The rest of the
 * matrix, where its source publishes one, is for the reference and the polarisation correction
 * (Design note 10, as ruled on 2026-10-09).
 */
export interface PhaseTable {
  /** u = √(θ ÷ π) at each entry, strictly ascending from 0 (forward) to 1 (backward). */
  readonly u: Float64Array;
  /** The phase per channel at each entry, sr⁻¹: the scattering matrix's a₁. */
  readonly values: readonly [Float64Array, Float64Array, Float64Array];
  /**
   * The scattering matrix's other elements, or `undefined` where its source publishes none, which
   * makes the term a total depolariser in the reference and the polarisation correction.
   */
  readonly matrix: PhaseMatrixTable | undefined;
}

/** One element of a scattering matrix per channel, on a {@link PhaseTable}'s u. */
export type PhaseMatrixElement = readonly [Float64Array, Float64Array, Float64Array];

/**
 * A scattering matrix's elements beside a₁, block-diagonal for a macroscopically isotropic and
 * mirror-symmetric medium (Hovenier, van der Mee and Domke 2004), each normalised as a₁ is.
 *
 * @remarks
 * R08.T5.b and T5.c fill them (a sphere's from T5.a's amplitudes, with a₂ = a₁ and a₄ = a₃);
 * {@link phaseTable} checks only their shape.
 */
export interface PhaseMatrixTable {
  readonly a2: PhaseMatrixElement;
  readonly a3: PhaseMatrixElement;
  readonly a4: PhaseMatrixElement;
  readonly b1: PhaseMatrixElement;
  readonly b2: PhaseMatrixElement;
}

/**
 * A term's phase function.
 *
 * - `rayleigh`: molecular scattering with the depolarisation ratio ρ per channel (natural light;
 *   R08 Design note 4), 3 ÷ (4(1 + 2γ)) × ((1 + 3γ) + (1 − γ) cos²θ) ÷ (4π) with γ = ρ ÷ (2 − ρ),
 *   the dipole phase of a randomly oriented anisotropic molecule from its ᾱ² and γ² (Chandrasekhar,
 *   Radiative Transfer, 1950; the form Bucholtz 1995, Appl. Opt. 34, 2765, uses, not read here),
 *   which is 3 ÷ (16π) × (1 + cos²θ) at ρ = 0. R08.T3.c's molecular term carries the mixture's ρ,
 *   and the kernels draw it per channel since R08.T6.b (`source.wgsl`'s `phaseOf`); R05's constant
 *   media say ρ = 0.
 * - `cornette-shanks`: Cornette and Shanks 1992's form of Henyey–Greenstein with asymmetry g.
 * - `none`: an absorbing-only term, which scatters nothing.
 * - `tabulated`: given entry by entry, per channel ({@link PhaseTable}; R08.T6.a).
 */
export type PhaseFunction =
  | { readonly kind: "rayleigh"; readonly depolarisation: Rgb }
  | { readonly kind: "cornette-shanks"; readonly asymmetry: number }
  | { readonly kind: "none" }
  | { readonly kind: "tabulated"; readonly table: PhaseTable };

/**
 * A phase table from its entries, checked.
 *
 * @remarks
 * The arrays are kept, not copied; the caller does not change them afterwards.
 *
 * @throws RangeError for fewer than two entries, channels of another length, a u that is not
 *   finite and strictly ascending from exactly 0 to exactly 1, a value that is not finite and at
 *   least 0, or a matrix element of another length or not finite.
 */
export function phaseTable(
  u: Float64Array,
  values: readonly [Float64Array, Float64Array, Float64Array],
  matrix?: PhaseMatrixTable,
): PhaseTable {
  const last = u.length - 1;
  if (u.length < 2 || values.some((channel) => channel.length !== u.length)) {
    throw new RangeError(
      `a phase table needs at least two entries and one value a channel each, got ${u.length} and ${values.map((channel) => channel.length).join(", ")}`,
    );
  }
  if (u[0] !== 0 || u[last] !== 1) {
    throw new RangeError(`a phase table spans u from 0 to 1, not ${u[0]} to ${u[last]}`);
  }
  for (let i = 1; i <= last; i += 1) {
    const at = u[i] ?? Number.NaN;
    if (!(Number.isFinite(at) && at > (u[i - 1] ?? Number.NaN))) {
      throw new RangeError(`a phase table's u ascends strictly: ${at} at entry ${i}`);
    }
  }
  for (const [c, channel] of values.entries()) {
    for (const [i, value] of channel.entries()) {
      if (!(Number.isFinite(value) && value >= 0)) {
        throw new RangeError(
          `a phase table's values are finite and not negative: ${value} at entry ${i}, channel ${c}`,
        );
      }
    }
  }
  if (matrix !== undefined) {
    const elements: ReadonlyArray<readonly [string, PhaseMatrixElement]> = [
      ["a2", matrix.a2],
      ["a3", matrix.a3],
      ["a4", matrix.a4],
      ["b1", matrix.b1],
      ["b2", matrix.b2],
    ];
    for (const [name, element] of elements) {
      for (const [c, channel] of element.entries()) {
        if (channel.length !== u.length || !channel.every(Number.isFinite)) {
          throw new RangeError(
            `a phase table's matrix element ${name} needs ${u.length} finite entries in channel ${c}`,
          );
        }
      }
    }
  }
  return { u, values, matrix };
}

/** A phase table per channel at u, linear between the entries that bracket it. */
function tabulatedPhase(table: PhaseTable, u: number): Rgb {
  const entries = table.u;
  let low = 0;
  let high = entries.length - 1;
  while (high - low > 1) {
    const middle = (low + high) >>> 1;
    if ((entries[middle] ?? Number.NaN) <= u) {
      low = middle;
    } else {
      high = middle;
    }
  }
  const u0 = entries[low] ?? Number.NaN;
  const f = Math.min(Math.max((u - u0) / ((entries[high] ?? Number.NaN) - u0), 0), 1);
  const at = (channel: Float64Array): number => {
    const v0 = channel[low] ?? Number.NaN;
    return v0 + ((channel[high] ?? Number.NaN) - v0) * f;
  };
  return [at(table.values[0]), at(table.values[1]), at(table.values[2])];
}

/**
 * Rayleigh's phase with the depolarisation ratio ρ, sr⁻¹: 3 ÷ (16π) × ((1 + 3γ) + (1 − γ) μ²) ÷
 * (1 + 2γ), γ = ρ ÷ (2 − ρ) ({@link PhaseFunction}'s `rayleigh`), which at ρ = 0 is
 * `source.wgsl`'s 3 ÷ (16π) × (1 + μ²) to the bit.
 */
function rayleighPhase(depolarisation: number, mu: number): number {
  const gamma = depolarisation / (2 - depolarisation);
  return ((3 / (16 * Math.PI)) * (1 + 3 * gamma + (1 - gamma) * mu * mu)) / (1 + 2 * gamma);
}

/**
 * A phase function's value per channel, sr⁻¹, for the cosine of the scattering angle.
 *
 * @remarks
 * The closed forms are `source.wgsl`'s `phaseOf`: Rayleigh's (Bruneton and Neyret 2008, eq. 2),
 * here with each channel's ρ (Chandrasekhar's form, which the kernels draw since R08.T6.b), and
 * Cornette and Shanks's, the same in every channel (Appl. Opt. 31 (1992) 3152, as
 * Bruneton and Neyret 2008, eq. 4, g its shape parameter and not the mean cosine), its denominator
 * floored at 10⁻⁶ as there. A `tabulated` phase is read at u = √(θ ÷ π),
 * θ = acos(cos θ), linear between the entries that bracket it ({@link PhaseTable}).
 *
 * @param cosTheta - The cosine between the direction of travel before and after scattering,
 *   clamped to [−1, 1].
 */
export function phaseAt(phase: PhaseFunction, cosTheta: number): Rgb {
  const mu = Math.min(Math.max(cosTheta, -1), 1);
  let value: number;
  switch (phase.kind) {
    case "none":
      value = 0;
      break;
    case "rayleigh": {
      const [red, green, blue] = phase.depolarisation;
      return [rayleighPhase(red, mu), rayleighPhase(green, mu), rayleighPhase(blue, mu)];
    }
    case "cornette-shanks": {
      const g = phase.asymmetry;
      const k = ((3 / (8 * Math.PI)) * (1 - g * g)) / (2 + g * g);
      value = (k * (1 + mu * mu)) / Math.max(1 + g * g - 2 * g * mu, 1e-6) ** 1.5;
      break;
    }
    case "tabulated":
      return tabulatedPhase(phase.table, Math.sqrt(Math.acos(mu) / Math.PI));
  }
  return [value, value, value];
}

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
 * Checks that no tabulated density of a medium is held up to its top from a last level below it.
 *
 * @remarks
 * The `tabulated` rule holds the last level's density beyond it ({@link TabulatedDensity}), as
 * R08.T3.a's column holds its last level's 10⁻⁷ of the ground's, so a medium built on a column
 * takes a top no higher than the column's. A layer whose last level is 0 may end below the top.
 * The CPU twin (`tablesCpu.ts`) and the kernels' packer (`tables.ts`'s `packMedium`) refuse such a
 * medium alike (plan R08, R08.T6.a and T6.b).
 *
 * @throws RangeError for a tabulated density whose last level lies below the medium's top with a
 *   density other than 0.
 */
export function checkTabulatedTops(medium: AtmosphereMedium): void {
  for (const term of medium.terms) {
    const profile = term.density;
    if (profile.kind !== "tabulated") {
      continue;
    }
    const last = profile.altitudesM.length - 1;
    const topLevelM = profile.altitudesM[last] ?? Number.NaN;
    const atTop = profile.relative[last] ?? Number.NaN;
    if (topLevelM < medium.topHeightM && atTop !== 0) {
      throw new RangeError(
        `term ${term.name} of medium ${medium.name} holds a density of ${atTop} from its last level at ${topLevelM} m up to the medium's top at ${medium.topHeightM} m`,
      );
    }
  }
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
