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
 * How a term's density, relative to its value at the reference height, varies with height above
 * the ground.
 *
 * - `exponential`: e^(−h ÷ H) with scale height H, 1 at the ground.
 * - `tent`: zero below `bottomM` and above `topM`, rising linearly to 1 at `peakM` and falling
 *   linearly back (Bruneton 2017's ozone layer).
 */
export type DensityProfile =
  | { readonly kind: "exponential"; readonly scaleHeightM: number }
  | {
      readonly kind: "tent";
      readonly bottomM: number;
      readonly peakM: number;
      readonly topM: number;
    };

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
