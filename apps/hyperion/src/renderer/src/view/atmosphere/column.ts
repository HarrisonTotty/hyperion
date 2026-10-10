/**
 * An atmosphere's vertical structure and its hydrostatic column (plan R08, R08.T3.a, Design notes 3
 * and 17): the temperature against pressure, and the levels of height, pressure, temperature and
 * number density that the medium's terms are laid on.
 *
 * @remarks
 * The profile is Robinson and Catling 2012's (ApJ 757, 104, eq. 10) scaled adiabat from the
 * surface up to an isothermal skin, T(p) = max(T_s (p ÷ p_s)^β, T_skin), β = α R ÷ c_p. Plan 14
 * publishes T_s, p_s, β and T_skin (R08.T1's P14.T24.e, unbuilt); until then the `isothermal`
 * profile at T_s stands, which is provisional and about 3× wrong in scale height at Venus's cloud
 * tops (Design note 3). The skin is itself provisional, with no ozone or haze heating.
 *
 * The column is the reference medium of Design note 17: it is built at g_ref = √(g_e g_p)
 * (`oblate.ts`'s `referenceGravity`) on a sphere of R_ref (R05's `tableRadiusM`), and its heights
 * are gravity-scaled heights h\* = s h, s = g(φ) ÷ g_ref, at which every latitude of the body reads
 * it. On a sphere with ω = 0, s = 1 and h\* is the height.
 *
 * Gases are ideal, n = p ÷ kT. Real-gas compressibility changes no column, since the mass above a
 * level is p ÷ g whatever the equation of state, only how it is spread in height: at a Venus-class
 * surface, CO₂ at 735.3 K and 9.21 MPa has Z = 1.0055 (65.9356 kg m⁻³, the NIST Chemistry
 * WebBook's fluid properties against 66.299 for the ideal gas at 44.0095 g mol⁻¹), so its lowest
 * scale height is 0.55% too dense, falling with the pressure above it.
 */

import { ATOMIC_MASS_CONSTANT_KG } from "../../lib/system/constants";
import { type TabulatedDensity, tabulatedDensity } from "./medium";
import {
  BOLTZMANN_J_PER_K,
  type Gas,
  GAS_MOLAR_MASS_G_PER_MOL,
  MOLE_FRACTION_SUM_TOLERANCE,
} from "./rayleigh";

/**
 * An atmosphere's temperature against pressure (Design note 3).
 *
 * - `isothermal`: one temperature everywhere. The provisional seam until P14.T24.e lands.
 * - `radiativeConvective`: T(p) = max(T_s (p ÷ p_s)^β, T_skin) (Robinson and Catling 2012, eq.
 *   10), a scaled dry adiabat from the surface up to an isothermal skin. β = α R ÷ c_p, with R ÷ c_p
 *   the mixture's ({@link dryAdiabatExponent}) and α ≤ 1 the latent heat's flattening of the lapse
 *   rate; T_skin = 2^(−1/4) T_eq, the τ → 0 limit of the grey Eddington atmosphere of P14.T13.c.
 *   Plan 14 publishes β and T_skin with P14.T24.e, and T_s and p_s with P14.T24.a.
 */
export type TemperatureProfile =
  | { readonly kind: "isothermal"; readonly temperatureK: number }
  | {
      readonly kind: "radiativeConvective";
      readonly surfaceK: number;
      readonly surfacePa: number;
      readonly beta: number;
      readonly skinK: number;
    };

/** A finite, positive number. */
function isPositive(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

/** The profile, checked: its temperatures and p_s finite and positive, β finite and at least 0. */
function checkedProfile(profile: TemperatureProfile): TemperatureProfile {
  switch (profile.kind) {
    case "isothermal":
      if (!isPositive(profile.temperatureK)) {
        throw new RangeError(`an isothermal profile needs T > 0, got ${profile.temperatureK} K`);
      }
      break;
    case "radiativeConvective":
      if (!(
        isPositive(profile.surfaceK) &&
        isPositive(profile.surfacePa) &&
        isPositive(profile.skinK) &&
        Number.isFinite(profile.beta) &&
        profile.beta >= 0
      )) {
        throw new RangeError(
          `a radiative-convective profile needs T_s, p_s, T_skin > 0 and β ≥ 0, got ${profile.surfaceK} K, ${profile.surfacePa} Pa, ${profile.skinK} K and ${profile.beta}`,
        );
      }
      break;
  }
  return profile;
}

/**
 * The temperature at a pressure, K (Design note 3's formula, which P14.T24.e's
 * `planetary::temperature_at` is to compute in the sim, for the flight model's drag).
 *
 * @remarks
 * Below p_s the adiabat continues, warmer. A golden list of the sim's levels is to pin the two to
 * 10⁻¹² once P14.T24.e lands (R08.T3.a).
 *
 * @param pressurePa - Finite and positive, Pa.
 * @throws RangeError for a pressure that is not finite and positive, or a profile with a
 *   temperature or p_s that is not finite and positive, or a β that is negative or not finite.
 */
export function temperatureAt(profile: TemperatureProfile, pressurePa: number): number {
  if (!isPositive(pressurePa)) {
    throw new RangeError(`a pressure must be finite and positive, got ${pressurePa} Pa`);
  }
  return profileTemperatureK(checkedProfile(profile), pressurePa);
}

/** {@link temperatureAt} of a checked profile at a checked pressure, K. */
function profileTemperatureK(profile: TemperatureProfile, pressurePa: number): number {
  let temperatureK: number;
  switch (profile.kind) {
    case "isothermal":
      temperatureK = profile.temperatureK;
      break;
    case "radiativeConvective":
      temperatureK = Math.max(
        profile.surfaceK * (pressurePa / profile.surfacePa) ** profile.beta,
        profile.skinK,
      );
      break;
  }
  return temperatureK;
}

/**
 * The bulk properties a gas lends a mixture's column: its molar mass, and its molar heat capacity
 * at constant pressure in units of R.
 *
 * @remarks
 * A species is an entry of data, not a case in the code: any mixture of any species with these
 * properties gives a column ({@link meanMolarMassGPerMol}, {@link dryAdiabatExponent}).
 */
export interface GasProperties {
  /** M, g mol⁻¹. */
  readonly molarMassGPerMol: number;
  /**
   * c_p ÷ R of the ideal gas, held constant (Design note 3): for N fixed degrees of freedom,
   * 1 + N ÷ 2 (Mayer's c_p = c_v + R and equipartition's c_v = N R ÷ 2), so that γ = 1 + 2 ÷ N
   * (Robinson and Catling 2012, eq. 9).
   */
  readonly heatCapacityOverR: number;
  /** Where the two figures come from. */
  readonly source: string;
}

/** A gas's c_p ÷ R at fixed degrees of freedom, and where it comes from. */
export interface HeatCapacity {
  /** c_p ÷ R. */
  readonly overR: number;
  readonly source: string;
}

/** Kinetic theory's c_p ÷ R = 1 + N ÷ 2 for N degrees of freedom. */
function ofDegrees(degrees: number, which: string): HeatCapacity {
  return {
    overR: 1 + degrees / 2,
    source: `N = ${degrees}, ${which} (Robinson and Catling 2012, eq. 9)`,
  };
}

/** CO₂'s γ = 1.3, so c_p ÷ R = γ ÷ (γ − 1) = 13 ÷ 3. */
const LINEAR_TRIATOMIC_CP_OVER_R = 13 / 3;

/**
 * Each gas's c_p ÷ R from kinetic theory with fixed degrees of freedom (Design note 3), as Robinson
 * and Catling 2012 take it (§2.3, eqs. 7–9).
 *
 * @remarks
 * R ÷ c_p is 2 ÷ 7 for H₂, N₂ and O₂ (N = 5: three translational and two rotational degrees);
 * 2 ÷ 5 for the atoms (N = 3, exact for a monatomic ideal gas); 3 ÷ 13 for CO₂, whose γ = 1.3 is
 * Robinson and Catling's for Venus and Mars (their N ≈ 7 with about two vibrational degrees, Bent
 * 1965, rounded), the γ against which their Venus α = 0.8 was fitted (§4.1); and 1 ÷ 4 for H₂O,
 * CH₄ and NH₃ (N = 6, three rotational degrees; medium confidence for the last two). The measured
 * values at 298.15 K (NIST-JANAF, Chase 1998: 0.288, 0.285, 0.283, 0.400, 0.400, 0.224, 0.248,
 * 0.233 and 0.233 for H₂, N₂, O₂, He, Ar, CO₂, H₂O, CH₄ and NH₃) agree within about 7%, CH₄ and
 * NH₃ furthest. Design note 3's 0.237 for NH₃ is 8.314 ÷ 35.06, against JANAF's 35.652 J mol⁻¹
 * K⁻¹.
 *
 * N₂O, beyond plan 14's gases, takes CO₂'s 3 ÷ 13 as the same linear triatomic, at medium
 * confidence: NIST-JANAF's 38.617 J mol⁻¹ K⁻¹ at 298.15 K (Chase 1998, table N-026) gives 0.215,
 * 7% below.
 *
 * CO takes 2 ÷ 7 (N = 5; NIST-JANAF's 29.149 J mol⁻¹ K⁻¹ at 298.15 K, Chase 1998's Shomate fit
 * as the NIST Chemistry WebBook gives it, makes 0.285) and H₂S 1 ÷ 4 (N = 6; 34.197, 0.243).
 * C₂H₆ takes 1 ÷ 4 too,
 * three rotational degrees, at low confidence: that is its c_p near 100 K (35.70 J mol⁻¹ K⁻¹,
 * Gurvich, Veyts et al. 1989, through the WebBook, 0.233), where ethane-bearing atmospheres such as
 * Titan's lie, while by 298 K its torsion and bends lift c_p to 52.49, R ÷ c_p 0.158, which no fixed
 * N follows.
 *
 * A temperature-dependent c_p is not used: it breaks the constant-β form, and Robinson and Catling
 * calibrated α against their constant γ. H₂'s rotation, in part frozen below about 150 K, is a
 * recorded caveat (Design note 3).
 */
export const GAS_HEAT_CAPACITY: Readonly<Record<Gas, HeatCapacity>> = {
  H2: ofDegrees(5, "a diatomic molecule"),
  He: ofDegrees(3, "an atom"),
  H2O: ofDegrees(6, "a bent molecule"),
  CH4: ofDegrees(6, "a non-linear molecule, medium confidence"),
  NH3: ofDegrees(6, "a non-linear molecule, medium confidence"),
  N2: ofDegrees(5, "a diatomic molecule"),
  O2: ofDegrees(5, "a diatomic molecule"),
  CO2: {
    overR: LINEAR_TRIATOMIC_CP_OVER_R,
    source: "γ = 1.3 (Robinson and Catling 2012, §2.3: N ≈ 7, Bent 1965)",
  },
  Ar: ofDegrees(3, "an atom"),
  Ne: ofDegrees(3, "an atom"),
  Kr: ofDegrees(3, "an atom"),
  Xe: ofDegrees(3, "an atom"),
  N2O: {
    overR: LINEAR_TRIATOMIC_CP_OVER_R,
    source: "CO₂'s γ = 1.3, the same linear triatomic, medium confidence (NIST-JANAF: 7% higher)",
  },
  CO: ofDegrees(5, "a diatomic molecule"),
  H2S: ofDegrees(6, "a bent molecule"),
  C2H6: ofDegrees(
    6,
    "a non-linear molecule, its c_p near 100 K, low confidence (by 298 K its torsion and bends raise c_p 47% above that)",
  ),
};

/**
 * A gas's bulk properties for a mixture: its molar mass, `rayleigh.ts`'s
 * `GAS_MOLAR_MASS_G_PER_MOL` (for plan 14's gases the sim's
 * `planetary::derive::atmosphere::Gas::molar_mass_g_per_mol`, IUPAC 2021), and its
 * {@link GAS_HEAT_CAPACITY}.
 */
export function gasProperties(gas: Gas): GasProperties {
  const heatCapacity = GAS_HEAT_CAPACITY[gas];
  return {
    molarMassGPerMol: GAS_MOLAR_MASS_G_PER_MOL[gas],
    heatCapacityOverR: heatCapacity.overR,
    source: `molar mass: rayleigh.ts GAS_MOLAR_MASS_G_PER_MOL; c_p/R: ${heatCapacity.source}`,
  };
}

/** One gas of a mixture, by its mole (number) fraction. */
export interface MixtureComponent {
  readonly properties: GasProperties;
  /** x, in [0, 1]. */
  readonly moleFraction: number;
}

/** Σ xᵢ f(propertiesᵢ) over a mixture whose fractions are checked. */
function mixed(
  mixture: ReadonlyArray<MixtureComponent>,
  property: (properties: GasProperties) => number,
): number {
  let sum = 0;
  let fractions = 0;
  for (const { properties, moleFraction } of mixture) {
    if (!(Number.isFinite(moleFraction) && moleFraction >= 0 && moleFraction <= 1)) {
      throw new RangeError(`a mole fraction lies in [0, 1], got ${moleFraction}`);
    }
    const value = property(properties);
    if (!isPositive(value)) {
      throw new RangeError(`a gas's molar mass and c_p must be positive, got ${value}`);
    }
    sum += moleFraction * value;
    fractions += moleFraction;
  }
  if (!(Math.abs(fractions - 1) <= MOLE_FRACTION_SUM_TOLERANCE)) {
    throw new RangeError(
      `a mixture's mole fractions sum to 1 within ${MOLE_FRACTION_SUM_TOLERANCE}, got ${fractions}`,
    );
  }
  return sum;
}

/**
 * A mixture's mean molar mass μ = Σ xᵢ Mᵢ, g mol⁻¹, by mole fraction.
 *
 * @throws RangeError for a fraction outside [0, 1], fractions that do not sum to 1 within
 *   {@link MOLE_FRACTION_SUM_TOLERANCE}, or a molar mass that is not finite and positive.
 */
export function meanMolarMassGPerMol(mixture: ReadonlyArray<MixtureComponent>): number {
  return mixed(mixture, (p) => p.molarMassGPerMol);
}

/**
 * A mixture's R ÷ c_p, the dry adiabat's exponent in T ∝ p^(R ÷ c_p) (Robinson and Catling 2012,
 * eq. 7, Poisson's equation), mixed as c_p = Σ xᵢ c_p,ᵢ by mole fraction (Design note 3).
 *
 * @remarks
 * Molar heat capacities add over an ideal mixture's moles, so R ÷ c_p = 1 ÷ Σ xᵢ (c_p,ᵢ ÷ R). An
 * average of γ instead is wrong: equal parts of He and CO₂ give 0.2927 by c_p and 0.3258 from the
 * mean γ.
 *
 * @throws RangeError as {@link meanMolarMassGPerMol}, for a c_p ÷ R that is not finite and
 *   positive.
 */
export function dryAdiabatExponent(mixture: ReadonlyArray<MixtureComponent>): number {
  return 1 / mixed(mixture, (p) => p.heatCapacityOverR);
}

/** What a column is built from (Design notes 3 and 17). */
export interface ColumnInput {
  /**
   * p at the datum, Pa: the column's base, at height 0. The surface pressure, which P14.T24.a
   * publishes with T_s, or a gas envelope's 1 bar.
   */
  readonly surfacePa: number;
  /**
   * T(p). A `radiativeConvective` profile's own p_s need not be the datum's: the column starts at
   * `temperatureAt(temperature, surfacePa)`.
   */
  readonly temperature: TemperatureProfile;
  /** μ, g mol⁻¹ ({@link meanMolarMassGPerMol}). */
  readonly meanMolarMassGPerMol: number;
  /**
   * g_ref, m s⁻²: the gravity at the datum the column is built at, R08.T3.d's
   * `bodyGravity(…).referenceGravityMS2` (√(g_e g_p) on a level spheroid, the bulk gravity with no
   * rotation section), never the bulk section's single gravity on a spinning body.
   */
  readonly referenceGravityMS2: number;
  /** R_ref, m: the datum's radius on the column's sphere, R05's `tableRadiusM(figure)`. */
  readonly referenceRadiusM: number;
}

/**
 * An atmosphere's column at its levels, from the datum up to p_s × {@link COLUMN_TOP_PRESSURE_RATIO}
 * (Design note 3): the reference medium every term's density is laid on.
 *
 * @remarks
 * Level i is at gravity-scaled height `altitudesM[i]` with `pressuresPa[i]`, `temperaturesK[i]`
 * and the relative number density `density.relative[i]`; `altitudesM` is `density.altitudesM`. The
 * gases are mixed at every height, so a gas's number density is its mole fraction times
 * `surfaceNumberDensityPerM3` times the relative density. Above the top the density holds the top
 * level's, about 10⁻⁷, as every tabulated density holds its last level's, so a medium built on the
 * column takes a `topHeightM` no higher than {@link AtmosphereColumn.topHeightM}.
 */
export interface AtmosphereColumn {
  /** Heights h\* of the levels above the datum, m: 0 first, ascending. */
  readonly altitudesM: Float64Array;
  /** p at each level, Pa, the datum's first. */
  readonly pressuresPa: Float64Array;
  /** T at each level, K. */
  readonly temperaturesK: Float64Array;
  /** n ÷ n_s at each level, as a density profile over {@link AtmosphereColumn.altitudesM}. */
  readonly density: TabulatedDensity;
  /** n_s = p_s ÷ (k T_s) at the datum, m⁻³: the number density at relative density 1. */
  readonly surfaceNumberDensityPerM3: number;
  /** μ, g mol⁻¹. */
  readonly meanMolarMassGPerMol: number;
  /** g_ref, m s⁻². */
  readonly referenceGravityMS2: number;
  /** R_ref, m. */
  readonly referenceRadiusM: number;
  /** h\* of the top level, m. */
  readonly topHeightM: number;
}

/** The column's top as a fraction of the datum's pressure: 10⁻⁷ (Design note 3). */
export const COLUMN_TOP_PRESSURE_RATIO = 1e-7;

/**
 * The column's intervals, evenly spaced in ln p from the datum to the top, with the tropopause
 * added as a level where it falls between: 1,024.
 *
 * @remarks
 * Each interval spans ln(10⁷) ÷ 1,024 = 0.0157 in ln p, so the density, linear between levels,
 * is high by at most about 0.0157² ÷ 8 = 3.1 × 10⁻⁵ of itself between them, and its integral by
 * 2 × 10⁻⁵, an isothermal layer's figures, which the adiabat's (1 − β) slope in ln n only
 * lowers.
 */
export const COLUMN_INTERVALS = 1_024;

/**
 * A tropopause within this fraction of an interval of a level is not added beside it, so that the
 * levels' heights stay strictly ascending in floating point.
 */
const TROPOPAUSE_MERGE_FRACTION = 1e-3;

/** (e^x − 1) ÷ x, 1 at x = 0. */
function expm1OverX(x: number): number {
  return x === 0 ? 1 : Math.expm1(x) / x;
}

/**
 * ln p of the tropopause, Pa, where the adiabat meets the skin: ln p_s + ln(T_skin ÷ T_s) ÷ β.
 * `null` for a profile with none, an isothermal one or a `radiativeConvective` one with β = 0, which
 * is isothermal at max(T_s, T_skin).
 */
function lnTropopausePa(profile: TemperatureProfile): number | null {
  let lnPa: number | null;
  switch (profile.kind) {
    case "isothermal":
      lnPa = null;
      break;
    case "radiativeConvective":
      lnPa =
        profile.beta === 0
          ? null
          : Math.log(profile.surfacePa) + Math.log(profile.skinK / profile.surfaceK) / profile.beta;
      break;
  }
  return lnPa;
}

/**
 * The profile's ∫ T d ln p over [ln p_low, ln p_high], K, as a function of the two ends: its part
 * of the geopotential height between two pressures, which k ÷ (μ m_u g_ref) turns into metres.
 *
 * @remarks
 * In closed form on each piece: T Δ ln p where T is constant, and T_s e^(β ℓ₀) Δℓ (e^(β Δℓ) − 1)
 * ÷ (β Δℓ) along the adiabat, with ℓ = ln(p ÷ p_s) and ℓ₀ the piece's lower end.
 *
 * @param lnTropopause - {@link lnTropopausePa} of the profile.
 */
function temperatureIntegralK(
  profile: TemperatureProfile,
  lnTropopause: number | null,
): (lnLowPa: number, lnHighPa: number) => number {
  let integral: (lnLowPa: number, lnHighPa: number) => number;
  switch (profile.kind) {
    case "isothermal": {
      const temperatureK = profile.temperatureK;
      integral = (lnLowPa, lnHighPa) => temperatureK * (lnHighPa - lnLowPa);
      break;
    }
    case "radiativeConvective": {
      const { surfaceK, beta, skinK } = profile;
      if (lnTropopause === null) {
        const temperatureK = Math.max(surfaceK, skinK);
        integral = (lnLowPa, lnHighPa) => temperatureK * (lnHighPa - lnLowPa);
        break;
      }
      const lnSurfacePa = Math.log(profile.surfacePa);
      integral = (lnLowPa, lnHighPa) => {
        let sum = 0;
        const adiabatLow = Math.max(lnLowPa, lnTropopause);
        if (lnHighPa > adiabatLow) {
          const span = lnHighPa - adiabatLow;
          sum +=
            surfaceK * Math.exp(beta * (adiabatLow - lnSurfacePa)) * span * expm1OverX(beta * span);
        }
        const skinHigh = Math.min(lnHighPa, lnTropopause);
        if (skinHigh > lnLowPa) {
          sum += skinK * (skinHigh - lnLowPa);
        }
        return sum;
      };
      break;
    }
  }
  return integral;
}

/**
 * ln p of the levels, Pa, from the datum's down to the top's, descending: even in ln p, with the
 * tropopause added between two of them.
 */
function levelLogPressures(lnSurfacePa: number, tropopause: number | null): Float64Array {
  const span = Math.log(COLUMN_TOP_PRESSURE_RATIO);
  const step = span / COLUMN_INTERVALS;
  const even = new Float64Array(COLUMN_INTERVALS + 1);
  for (let i = 0; i <= COLUMN_INTERVALS; i += 1) {
    even[i] = lnSurfacePa + (span * i) / COLUMN_INTERVALS;
  }
  if (tropopause === null) {
    return even;
  }
  const at = (tropopause - lnSurfacePa) / step;
  const nearest = Math.round(at);
  if (at <= 0 || at >= COLUMN_INTERVALS || Math.abs(at - nearest) < TROPOPAUSE_MERGE_FRACTION) {
    return even;
  }
  const below = Math.floor(at);
  const levels = new Float64Array(COLUMN_INTERVALS + 2);
  levels.set(even.subarray(0, below + 1), 0);
  levels[below + 1] = tropopause;
  levels.set(even.subarray(below + 1), below + 2);
  return levels;
}

/**
 * An atmosphere's hydrostatic column, in `f64`, from the datum up to p_s ×
 * {@link COLUMN_TOP_PRESSURE_RATIO} (Design note 3), at g_ref on a sphere of R_ref (Design note 17).
 *
 * @remarks
 * dp ÷ dr = −n μ m_u g with n = p ÷ kT and g = g_ref (R_ref ÷ r)², integrated in closed form rather
 * than by steps: the geopotential height above the datum, Φ(p) = k ÷ (μ m_u g_ref) × ∫ T d ln p
 * from p up to p_s, is exact for both profiles, and inverse-square gravity puts it at the height
 * z = R_ref Φ ÷ (R_ref − Φ), the inverse of the U.S. Standard Atmosphere 1976's geopotential
 * height Φ = R_ref z ÷ (R_ref + z) (its eqs. 17–19). Near the datum the local scale height is
 * kT ÷ (μ m_u g_ref); at height z it is that times (1 + z ÷ R_ref)². The vertical column above
 * the datum exceeds p_s ÷ g_ref by about 2H ÷ R_ref, the spherical excess, for an isothermal
 * column of scale height H.
 *
 * The levels are evenly spaced in ln p ({@link COLUMN_INTERVALS}), with the tropopause added. The
 * monopole's (R_ref ÷ r)² stands for the level spheroid's free-air gradient, which varies with
 * latitude on a spinning body, ∂γ ÷ ∂h = −(2γ ÷ a)(1 + f + m − 2f sin²φ) (Heiskanen and Moritz
 * 1967, §2-10, eq. 2-121; exactly, eq. 2-120): on Saturn the monopole moves the column above the
 * datum by at most 6 × 10⁻⁴ of itself, at the equator, 0.44 of its 1.4 × 10⁻³ spherical excess
 * (0.13 at the pole).
 *
 * @throws RangeError for inputs that are not finite and positive, a profile as
 *   {@link temperatureAt}, or a column whose geopotential height reaches R_ref below its top, which
 *   no hydrostatic atmosphere under inverse-square gravity holds (its pressure at infinity is above
 *   the top's).
 */
export function hydrostaticColumn(input: ColumnInput): AtmosphereColumn {
  const { surfacePa, referenceGravityMS2, referenceRadiusM } = input;
  const molarMassGPerMol = input.meanMolarMassGPerMol;
  if (!(
    isPositive(surfacePa) &&
    isPositive(molarMassGPerMol) &&
    isPositive(referenceGravityMS2) &&
    isPositive(referenceRadiusM)
  )) {
    throw new RangeError(
      `a column needs p_s, μ, g_ref and R_ref finite and positive, got ${surfacePa} Pa, ${molarMassGPerMol} g/mol, ${referenceGravityMS2} m/s² and ${referenceRadiusM} m`,
    );
  }
  const profile = checkedProfile(input.temperature);
  const lnSurfacePa = Math.log(surfacePa);
  const lnTopPa = lnSurfacePa + Math.log(COLUMN_TOP_PRESSURE_RATIO);
  const tropopause = lnTropopausePa(profile);
  const integralK = temperatureIntegralK(profile, tropopause);
  const metresPerKelvin =
    BOLTZMANN_J_PER_K / (molarMassGPerMol * ATOMIC_MASS_CONSTANT_KG * referenceGravityMS2);
  const heightM = (phiM: number): number => (referenceRadiusM * phiM) / (referenceRadiusM - phiM);
  const topPhiM = metresPerKelvin * integralK(lnTopPa, lnSurfacePa);
  if (!(topPhiM < referenceRadiusM)) {
    throw new RangeError(
      `the column is not bound: its geopotential height reaches R_ref = ${referenceRadiusM} m below p = ${surfacePa * COLUMN_TOP_PRESSURE_RATIO} Pa`,
    );
  }
  const lnLevels = levelLogPressures(lnSurfacePa, tropopause);
  const last = lnLevels.length - 1;
  const altitudesM = new Float64Array(lnLevels.length);
  const pressuresPa = new Float64Array(lnLevels.length);
  const temperaturesK = new Float64Array(lnLevels.length);
  const relative = new Float64Array(lnLevels.length);
  const surfaceK = profileTemperatureK(profile, surfacePa);
  for (const [i, lnPa] of lnLevels.entries()) {
    const pressurePa =
      i === 0 ? surfacePa : i === last ? surfacePa * COLUMN_TOP_PRESSURE_RATIO : Math.exp(lnPa);
    const temperatureK = profileTemperatureK(profile, pressurePa);
    altitudesM[i] = heightM(metresPerKelvin * integralK(lnPa, lnSurfacePa));
    pressuresPa[i] = pressurePa;
    temperaturesK[i] = temperatureK;
    relative[i] = (pressurePa / surfacePa) * (surfaceK / temperatureK);
  }
  return {
    altitudesM,
    pressuresPa,
    temperaturesK,
    density: tabulatedDensity(altitudesM, relative),
    surfaceNumberDensityPerM3: surfacePa / (BOLTZMANN_J_PER_K * surfaceK),
    meanMolarMassGPerMol: molarMassGPerMol,
    referenceGravityMS2,
    referenceRadiusM,
    topHeightM: heightM(topPhiM),
  };
}
