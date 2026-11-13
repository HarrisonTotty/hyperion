/**
 * An atmosphere's vertical structure and its hydrostatic column (plan R08, R08.T3.a, Design notes 3
 * and 17): the temperature against pressure, and the levels of height, pressure, temperature and
 * number density that the medium's terms are laid on.
 *
 * @remarks
 * The profile is Robinson and Catling 2012's (ApJ 757, 104, eq. 10) scaled adiabat from the
 * surface up to an isothermal skin, T(p) = max(T_ad(p), T_skin). Plan 14 publishes it (P14.T24.e,
 * on the wire from generator version 24) as the adiabat's nodes from (p_s, T_s) to the tropopause,
 * integrated on P14.T49.b's c_p(T), with T_skin above, which the `nodes` profile reads (R08.T3.e).
 * The `radiativeConvective` profile's single β stands for the hand fixtures, and the `isothermal`
 * profile at T_s for a record that does not model the structure, which is about 3× wrong in scale
 * height at Venus's cloud tops (Design note 3). The skin is itself provisional, with no ozone or
 * haze heating. A gas envelope's profile is P14.T24.d's, Guillot's grey T(p) above its join and its
 * adiabat's nodes below (the `envelope` profile, R08.T16.a), and its column runs to a top of its
 * own ({@link ColumnInput.topPa}) with levels at its modes' ends ({@link ColumnInput.levelsPa}).
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
import type { GuillotProfile, ProfileNode } from "../../lib/system/model";
import { ABSORBER_PRESSURE_REFERENCE_PA, type TabulatedDensity, tabulatedDensity } from "./medium";
import {
  BOLTZMANN_J_PER_K,
  type Gas,
  GAS_MOLAR_MASS_G_PER_MOL,
  MOLE_FRACTION_SUM_TOLERANCE,
} from "./rayleigh";
import { exp, expm1, log, pow } from "../../lib/pinnedMath";

/**
 * An atmosphere's temperature against pressure (Design note 3).
 *
 * - `isothermal`: one temperature everywhere. The seam for a record that does not model its
 *   vertical structure.
 * - `radiativeConvective`: T(p) = max(T_s (p ÷ p_s)^β, T_skin) (Robinson and Catling 2012, eq.
 *   10), a scaled dry adiabat from the surface up to an isothermal skin. β = α R ÷ c_p, with R ÷ c_p
 *   the mixture's ({@link dryAdiabatExponent}) and α ≤ 1 the latent heat's flattening of the lapse
 *   rate; T_skin = 2^(−1/4) T_eq, the τ → 0 limit of the grey Eddington atmosphere of P14.T13.c.
 *   The hand fixtures' form, with β stated.
 * - `nodes`: P14.T24.e's published structure, max(T_ad(p), T_skin) with T_ad the power law between
 *   the two nodes about p, below the ground the first segment's power law, and above the last node
 *   (the tropopause) T_skin: the sim's `planetary::hooks::structure::temperature_at`, operation for
 *   operation, so that the picture and the flight model's drag read the same air (R08.T3.e). The
 *   nodes run from the ground (p_s, T_s) up, the pressures strictly descending; the ground alone
 *   leaves the air isothermal at T_skin. The sim's own guarantees, every node at least T_skin and
 *   the last at it, are taken and not checked: the column's ∫ T d ln p leaves out the max with
 *   T_skin, which they confine to a rounding.
 * - `envelope`: P14.T24.d's gas envelope (R08.T16.a): above its join, the first node, Guillot's
 *   grey T(p) ({@link greyTemperatureK}); from the join down the power law between the two nodes
 *   about p, and past the last node the last segment's: the sim's
 *   `planetary::hooks::structure::envelope::EnvelopeProfile::temperature_at`, operation for
 *   operation. The nodes run from the join down, the pressures strictly ascending, two at least;
 *   the grey profile reads the wire's parameters at the bulk section's gravity, as the sim's does.
 */
export type TemperatureProfile =
  | { readonly kind: "isothermal"; readonly temperatureK: number }
  | {
      readonly kind: "radiativeConvective";
      readonly surfaceK: number;
      readonly surfacePa: number;
      readonly beta: number;
      readonly skinK: number;
    }
  | {
      readonly kind: "nodes";
      readonly nodes: ReadonlyArray<ProfileNode>;
      readonly skinK: number;
    }
  | {
      readonly kind: "envelope";
      readonly grey: GuillotProfile;
      readonly internalK: number;
      readonly irradiationK: number;
      /** g, m s⁻², at which the grey profile's thermal optical depth is read. */
      readonly gravityMS2: number;
      readonly nodes: ReadonlyArray<ProfileNode>;
    };

/** A gas envelope's profile, {@link TemperatureProfile}'s `envelope` (R08.T16.a). */
export type EnvelopeProfile = Extract<TemperatureProfile, { readonly kind: "envelope" }>;

/** A finite, positive number. */
function isPositive(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

/**
 * Whether `nodes` are at least one, each pressure and temperature finite and positive, and the
 * pressures strictly monotonic, descending or ascending as `order` says.
 */
function nodesUsable(
  nodes: ReadonlyArray<ProfileNode>,
  order: "descending" | "ascending",
): boolean {
  return (
    nodes.length > 0 &&
    nodes.every((node, i) => {
      const previous = nodes[i - 1];
      return (
        isPositive(node.pressurePa) &&
        isPositive(node.temperatureK) &&
        (previous === undefined ||
          (order === "descending"
            ? node.pressurePa < previous.pressurePa
            : node.pressurePa > previous.pressurePa))
      );
    })
  );
}

/** The profile, checked: its temperatures and pressures finite and positive, β finite and at least 0. */
function checkedProfile(profile: TemperatureProfile): TemperatureProfile {
  switch (profile.kind) {
    case "isothermal":
      if (!isPositive(profile.temperatureK)) {
        throw new RangeError(`an isothermal profile needs T > 0, got ${profile.temperatureK} K`);
      }
      break;
    case "nodes":
      if (!(nodesUsable(profile.nodes, "descending") && isPositive(profile.skinK))) {
        throw new RangeError(
          `a profile of nodes needs at least one node, each p and T > 0 with the pressures descending, and T_skin > 0, got ${profile.nodes.length} nodes and ${profile.skinK} K`,
        );
      }
      break;
    case "envelope": {
      const { grey } = profile;
      const n = grey.opticalDepthExponent;
      if (!(
        nodesUsable(profile.nodes, "ascending") &&
        profile.nodes.length >= 2 &&
        isPositive(grey.thermalOpacityM2PerKg) &&
        n >= 1 &&
        n <= 2 &&
        isPositive(grey.opacityRatio) &&
        grey.opacityRatio <= 1 &&
        isPositive(grey.redistributionFactor) &&
        grey.redistributionFactor <= 1 &&
        Number.isFinite(profile.internalK) &&
        profile.internalK >= 0 &&
        Number.isFinite(profile.irradiationK) &&
        profile.irradiationK >= 0 &&
        isPositive(profile.gravityMS2)
      )) {
        throw new RangeError(
          `an envelope's profile needs two nodes or more, each p and T > 0 with the pressures ascending, κ₁ > 0, n in [1, 2], γ and f in (0, 1], T_int and T_irr ≥ 0 and g > 0, got ${profile.nodes.length} nodes, κ₁ ${grey.thermalOpacityM2PerKg}, n ${n}, γ ${grey.opacityRatio}, f ${grey.redistributionFactor}, ${profile.internalK} K, ${profile.irradiationK} K and ${profile.gravityMS2} m/s²`,
        );
      }
      break;
    }
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
 * The temperature at a pressure, K (Design note 3's formula; for a `nodes` profile, P14.T24.e's
 * `planetary::hooks::structure::temperature_at`, which the sim's flight model's drag reads).
 *
 * @remarks
 * Below p_s the adiabat continues, warmer. A `nodes` profile gives the sim's own bits: at the
 * golden `planetary/vertical_structure`'s levels its Earth's, Venus's and Titan's temperatures,
 * bit for bit (R08.T3.e), and at a node's own pressure that node's temperature.
 *
 * @param pressurePa - Finite and positive, Pa.
 * @throws RangeError for a pressure that is not finite and positive, or a profile with a
 *   temperature or pressure that is not finite and positive, a β that is negative or not finite,
 *   `nodes` that are none or whose pressures do not descend, or an `envelope` of fewer than two
 *   nodes or nodes whose pressures do not ascend, a κ₁ or g not above 0, an n outside [1, 2], a γ
 *   or f outside (0, 1], or a T_int or T_irr negative or not finite.
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
        profile.surfaceK * pow(pressurePa / profile.surfacePa, profile.beta),
        profile.skinK,
      );
      break;
    case "nodes": {
      const { nodes, skinK } = profile;
      const reached = nodesReached(nodes, pressurePa);
      if (nodes.length < 2 || reached >= nodes.length) {
        temperatureK = skinK;
        break;
      }
      // The sim's max(T_ad, T_skin), which a rounding below the skin near the tropopause can reach.
      const lawK = nodeLawK(nodes, reached, pressurePa);
      temperatureK = lawK < skinK ? skinK : lawK;
      break;
    }
    case "envelope": {
      const { nodes } = profile;
      temperatureK =
        pressurePa < nodeAt(nodes, 0).pressurePa
          ? greyTemperatureK(profile, pressurePa)
          : nodeLawK(nodes, nodesReached(nodes, pressurePa), pressurePa);
      break;
    }
  }
  return temperatureK;
}

/**
 * Guillot's p₁, the pressure his thermal opacity κ₁ is given at, Pa: 1 bar, the sim's
 * `REFERENCE_PRESSURE`.
 */
const GREY_REFERENCE_PA = 1e5;

/** √3, as the sim writes it (`envelope.rs`'s `SQRT_3`). */
const SQRT_3 = 1.732_050_807_568_877_2;

/** x⁴ as the sim's `math::powi(x, 4)` forms it, by squaring: (x²)². */
function fourthPower(x: number): number {
  const square = x * x;
  return square * square;
}

/**
 * An envelope's grey temperature at a pressure above its join, K: Guillot 2010's (A&A 520, A27)
 * eq. 29 at the thermal optical depth τ = κ₁ p₁ (p ÷ p₁)ⁿ ÷ (n g),
 * T⁴ = ¾ T_int⁴ (⅔ + τ) + ¾ f T_irr⁴ (⅔ + 1 ÷ (γ√3) + (γ ÷ √3 − 1 ÷ (γ√3)) e^(−γτ√3)), in the
 * sim's operations (`GreyProfile::optical_depth` and `temperature`).
 */
function greyTemperatureK(profile: EnvelopeProfile, pressurePa: number): number {
  const { grey } = profile;
  const n = grey.opticalDepthExponent;
  const tau =
    (grey.thermalOpacityM2PerKg * GREY_REFERENCE_PA * pow(pressurePa / GREY_REFERENCE_PA, n)) /
    (n * profile.gravityMS2);
  const gamma = grey.opacityRatio;
  const int4 = fourthPower(profile.internalK);
  const irr4 = grey.redistributionFactor * fourthPower(profile.irradiationK);
  const inverse = 1 / (gamma * SQRT_3);
  const t4 =
    0.75 * int4 * (2 / 3 + tau) +
    0.75 * irr4 * (2 / 3 + inverse + (gamma / SQRT_3 - inverse) * exp(-gamma * tau * SQRT_3));
  return pow(t4, 0.25);
}

/** Node `i` of `nodes`, which the caller has bounded. */
function nodeAt(nodes: ReadonlyArray<ProfileNode>, i: number): ProfileNode {
  const node = nodes[i];
  if (node === undefined) {
    throw new Error(`a profile has no node ${i} of ${nodes.length}`);
  }
  return node;
}

/**
 * How many of `nodes` p has reached, in the nodes' own order: those at or below p in height for
 * nodes from the ground up (pressures descending), at or above it for nodes from a gas envelope's
 * join down (ascending). It is the sim's `partition_point`, so that a node's own pressure takes the
 * segment that node starts, whose law gives its temperature exactly.
 */
function nodesReached(nodes: ReadonlyArray<ProfileNode>, pressurePa: number): number {
  const descending = (nodes[1]?.pressurePa ?? Number.NaN) < (nodes[0]?.pressurePa ?? Number.NaN);
  let low = 0;
  let high = nodes.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    const nodePa = nodeAt(nodes, middle).pressurePa;
    if (descending ? nodePa >= pressurePa : nodePa <= pressurePa) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }
  return low;
}

/**
 * d ln T ÷ d ln p over the segment from node `a` to node `b`, as the sim computes it:
 * ln(T_b ÷ T_a) ÷ ln(p_b ÷ p_a).
 */
function segmentExponent(a: ProfileNode, b: ProfileNode): number {
  return log(b.temperatureK / a.temperatureK) / log(b.pressurePa / a.pressurePa);
}

/**
 * The piecewise power law through two or more `nodes` at `pressurePa`, of which p has reached
 * `reached` ({@link nodesReached}), K: between two nodes the power law through them, anchored at
 * the first in the nodes' order, T_a e^(e ln(p ÷ p_a)); before the first node the first segment's
 * law, and past the last the last segment's, each anchored at its end node.
 *
 * @remarks
 * It is plan 14's one form of a published adiabat, in the sim's operations: a surface's air from
 * the ground up (P14.T24.e's `RadiativeConvective::temperature_at`), which takes T_skin past its
 * last node, and a gas envelope's below its join (P14.T24.d's `EnvelopeProfile::temperature_at`),
 * whose nodes run from the join down and which takes Guillot's grey profile before its first.
 */
function nodeLawK(nodes: ReadonlyArray<ProfileNode>, reached: number, pressurePa: number): number {
  const last = nodes.length - 1;
  const from = Math.min(Math.max(reached - 1, 0), last - 1);
  const a = nodeAt(nodes, from);
  const b = nodeAt(nodes, from + 1);
  const anchor = reached > last ? b : a;
  return anchor.temperatureK * exp(segmentExponent(a, b) * log(pressurePa / anchor.pressurePa));
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
   * T(p). A `radiativeConvective` or `nodes` profile's own p_s need not be the datum's: the column
   * starts at `temperatureAt(temperature, surfacePa)`.
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
  /**
   * p at the column's top, Pa, below the datum's: a gas envelope's, above every term it carries
   * (R08.T16.a). Absent, p_s × {@link COLUMN_TOP_PRESSURE_RATIO}.
   */
  readonly topPa?: number;
  /**
   * Pressures, Pa, each between the top's and the datum's, at which the column lays a level beside
   * its even ones: a gas envelope's floor and its modes' ends (R08.T16.a).
   */
  readonly levelsPa?: ReadonlyArray<number>;
}

/**
 * An atmosphere's column at its levels, from the datum up to p_s × {@link COLUMN_TOP_PRESSURE_RATIO}
 * or the top it is given (Design note 3): the reference medium every term's density is laid on.
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
  return x === 0 ? 1 : expm1(x) / x;
}

/**
 * ln p of the tropopause, Pa, where the adiabat meets the skin: ln p_s + ln(T_skin ÷ T_s) ÷ β, or
 * a `nodes` profile's last node's. `null` for a profile with none: an isothermal one, a
 * `radiativeConvective` one with β = 0, which is isothermal at max(T_s, T_skin), a `nodes` one
 * of the ground alone, isothermal at T_skin, or an `envelope`, whose grey profile has no skin.
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
          : log(profile.surfacePa) + log(profile.skinK / profile.surfaceK) / profile.beta;
      break;
    case "nodes": {
      const { nodes } = profile;
      lnPa = nodes.length < 2 ? null : log(nodeAt(nodes, nodes.length - 1).pressurePa);
      break;
    }
    case "envelope":
      lnPa = null;
      break;
  }
  return lnPa;
}

/**
 * A piece of a profile in ln p, ℓ in [`lnLowPa`, `lnHighPa`] (either end infinite where the piece
 * is unbounded), along which T = T_a e^(e (ℓ − ℓ_a)): a segment of a node law, anchored at a node,
 * or a constant, e = 0.
 */
interface LawPiece {
  readonly lnLowPa: number;
  readonly lnHighPa: number;
  /** ℓ_a and T_a, K. */
  readonly anchorLnPa: number;
  readonly anchorK: number;
  /** e = d ln T ÷ d ln p. */
  readonly exponent: number;
}

/**
 * The pieces of the node law through two or more `nodes` ({@link nodeLawK}) over every ln p, each
 * anchored as the law is: before the first node, each segment in the nodes' order, and past the
 * last node.
 */
function nodeLawPieces(nodes: ReadonlyArray<ProfileNode>): ReadonlyArray<LawPiece> {
  const last = nodes.length - 1;
  const lnPa = nodes.map((node) => log(node.pressurePa));
  const lnAt = (i: number): number => lnPa[i] ?? Number.NaN;
  const descending = lnAt(1) < lnAt(0);
  const segments: LawPiece[] = [];
  for (let i = 0; i < last; i += 1) {
    segments.push({
      lnLowPa: Math.min(lnAt(i), lnAt(i + 1)),
      lnHighPa: Math.max(lnAt(i), lnAt(i + 1)),
      anchorLnPa: lnAt(i),
      anchorK: nodeAt(nodes, i).temperatureK,
      exponent: segmentExponent(nodeAt(nodes, i), nodeAt(nodes, i + 1)),
    });
  }
  const first = segments[0];
  const final = segments[segments.length - 1];
  if (first === undefined || final === undefined) {
    throw new Error(`a node law needs two nodes, not ${nodes.length}`);
  }
  // An end node's piece, outward from it in pressure, on its own segment's law.
  const end = (node: number, segment: LawPiece, deeper: boolean): LawPiece => ({
    lnLowPa: deeper ? lnAt(node) : Number.NEGATIVE_INFINITY,
    lnHighPa: deeper ? Number.POSITIVE_INFINITY : lnAt(node),
    anchorLnPa: lnAt(node),
    anchorK: nodeAt(nodes, node).temperatureK,
    exponent: segment.exponent,
  });
  return [end(0, first, descending), ...segments, end(last, final, !descending)];
}

/**
 * ∫ T d ln p of `pieces` over [`lnLowPa`, `lnHighPa`], K: on each piece's part of it, from ℓ₀ to
 * ℓ₀ + Δℓ, T_a e^(e (ℓ₀ − ℓ_a)) Δℓ (e^(e Δℓ) − 1) ÷ (e Δℓ), so (T₁ − T₀) ÷ e on a whole segment
 * and T Δℓ on a constant.
 */
function piecesIntegralK(
  pieces: ReadonlyArray<LawPiece>,
  lnLowPa: number,
  lnHighPa: number,
): number {
  let sum = 0;
  for (const piece of pieces) {
    const low = Math.max(lnLowPa, piece.lnLowPa);
    const high = Math.min(lnHighPa, piece.lnHighPa);
    if (high > low) {
      const span = high - low;
      sum +=
        piece.anchorK *
        exp(piece.exponent * (low - piece.anchorLnPa)) *
        span *
        expm1OverX(piece.exponent * span);
    }
  }
  return sum;
}

/**
 * The profile's ∫ T d ln p over [ln p_low, ln p_high], K, as a function of the two ends: its part
 * of the geopotential height between two pressures, which k ÷ (μ m_u g_ref) turns into metres.
 *
 * @remarks
 * In closed form on each piece: T Δ ln p where T is constant, and T_s e^(β ℓ₀) Δℓ (e^(β Δℓ) − 1)
 * ÷ (β Δℓ) along the adiabat, with ℓ = ln(p ÷ p_s) and ℓ₀ the piece's lower end. A `nodes`
 * profile's pieces are its segments, the first one's law below the ground and T_skin above the
 * last node ({@link piecesIntegralK}); the max with T_skin, which binds only by a rounding, is not
 * integrated.
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
      const lnSurfacePa = log(profile.surfacePa);
      integral = (lnLowPa, lnHighPa) => {
        let sum = 0;
        const adiabatLow = Math.max(lnLowPa, lnTropopause);
        if (lnHighPa > adiabatLow) {
          const span = lnHighPa - adiabatLow;
          sum += surfaceK * exp(beta * (adiabatLow - lnSurfacePa)) * span * expm1OverX(beta * span);
        }
        const skinHigh = Math.min(lnHighPa, lnTropopause);
        if (skinHigh > lnLowPa) {
          sum += skinK * (skinHigh - lnLowPa);
        }
        return sum;
      };
      break;
    }
    case "nodes": {
      const { nodes, skinK } = profile;
      if (nodes.length < 2) {
        integral = (lnLowPa, lnHighPa) => skinK * (lnHighPa - lnLowPa);
        break;
      }
      const law = nodeLawPieces(nodes);
      const skin: LawPiece = {
        lnLowPa: Number.NEGATIVE_INFINITY,
        lnHighPa: log(nodeAt(nodes, nodes.length - 1).pressurePa),
        anchorLnPa: 0,
        anchorK: skinK,
        exponent: 0,
      };
      const pieces: ReadonlyArray<LawPiece> = [...law.slice(0, -1), skin];
      integral = (lnLowPa, lnHighPa) => piecesIntegralK(pieces, lnLowPa, lnHighPa);
      break;
    }
    case "envelope": {
      // Below the join, the node law's segments and its extension past the last node; above it,
      // the grey profile in place of the law's piece before the first node.
      const below = nodeLawPieces(profile.nodes).slice(1);
      const grey = greyIntegralK(profile);
      integral = (lnLowPa, lnHighPa) =>
        grey(lnLowPa, lnHighPa) + piecesIntegralK(below, lnLowPa, lnHighPa);
      break;
    }
  }
  return integral;
}

/**
 * Gauss–Legendre's five nodes on [−1, 1] and their weights (Abramowitz and Stegun 1964, table
 * 25.4).
 */
const GAUSS_LEGENDRE_5: ReadonlyArray<readonly [number, number]> = [
  [-0.906_179_845_938_664, 0.236_926_885_056_189_1],
  [-0.538_469_310_105_683_1, 0.478_628_670_499_366_5],
  [0, 0.568_888_888_888_888_9],
  [0.538_469_310_105_683_1, 0.478_628_670_499_366_5],
  [0.906_179_845_938_664, 0.236_926_885_056_189_1],
];

/**
 * The span in ln p of each of the grey integral's panels: 0.25, over which five-point
 * Gauss–Legendre holds Guillot's T, smooth in ln p, far inside the column's 10⁻⁶.
 */
const GREY_PANEL_LN_P = 0.25;

/**
 * An envelope's ∫ T d ln p over the part of [ln p_low, ln p_high] above its join, K, as a function
 * of the two ends: Guillot's T has no closed form in it, so it is taken by five-point
 * Gauss–Legendre on panels of {@link GREY_PANEL_LN_P} counted up from the join, their sums kept as
 * they are first reached, and the panel a end falls in taken from that end.
 */
function greyIntegralK(profile: EnvelopeProfile): (lnLowPa: number, lnHighPa: number) => number {
  const lnJoinPa = log(nodeAt(profile.nodes, 0).pressurePa);
  const panel = (lnLowPa: number, lnHighPa: number): number => {
    const middle = 0.5 * (lnLowPa + lnHighPa);
    const half = 0.5 * (lnHighPa - lnLowPa);
    let sum = 0;
    for (const [x, w] of GAUSS_LEGENDRE_5) {
      sum += w * greyTemperatureK(profile, exp(middle + half * x));
    }
    return sum * half;
  };
  // fromJoin[k], ∫ T d ln p from the join up to k panels above it.
  const fromJoin: number[] = [0];
  const upTo = (lnPa: number): number => {
    const panels = Math.floor((lnJoinPa - lnPa) / GREY_PANEL_LN_P);
    while (fromJoin.length <= panels) {
      const k = fromJoin.length;
      fromJoin.push(
        (fromJoin[k - 1] ?? Number.NaN) +
          panel(lnJoinPa - k * GREY_PANEL_LN_P, lnJoinPa - (k - 1) * GREY_PANEL_LN_P),
      );
    }
    const at = lnJoinPa - panels * GREY_PANEL_LN_P;
    return (fromJoin[panels] ?? Number.NaN) + (lnPa < at ? panel(lnPa, at) : 0);
  };
  return (lnLowPa, lnHighPa) => {
    const high = Math.min(lnHighPa, lnJoinPa);
    return high > lnLowPa ? upTo(lnLowPa) - upTo(high) : 0;
  };
}

/**
 * ln p of the levels, Pa, from the datum's down to the top's, descending: even in ln p, with the
 * tropopause added between two of them.
 *
 * @remarks
 * A column to a top of its own (`lnTopPa`) takes as many intervals as keep them no wider than
 * {@link COLUMN_INTERVALS}' over a span of 10⁷, and its last level is the top's.
 */
function levelLogPressures(
  lnSurfacePa: number,
  lnTopPa: number | null,
  tropopause: number | null,
): Float64Array {
  const ratioSpan = log(COLUMN_TOP_PRESSURE_RATIO);
  const span = lnTopPa === null ? ratioSpan : lnTopPa - lnSurfacePa;
  const intervals =
    lnTopPa === null
      ? COLUMN_INTERVALS
      : Math.max(COLUMN_INTERVALS, Math.ceil((COLUMN_INTERVALS * span) / ratioSpan));
  const step = span / intervals;
  const even = new Float64Array(intervals + 1);
  for (let i = 0; i <= intervals; i += 1) {
    even[i] = lnSurfacePa + (span * i) / intervals;
  }
  if (lnTopPa !== null) {
    even[intervals] = lnTopPa;
  }
  if (tropopause === null) {
    return even;
  }
  const at = (tropopause - lnSurfacePa) / step;
  const nearest = Math.round(at);
  if (at <= 0 || at >= intervals || Math.abs(at - nearest) < TROPOPAUSE_MERGE_FRACTION) {
    return even;
  }
  const below = Math.floor(at);
  const levels = new Float64Array(intervals + 2);
  levels.set(even.subarray(0, below + 1), 0);
  levels[below + 1] = tropopause;
  levels.set(even.subarray(below + 1), below + 2);
  return levels;
}

/**
 * An added level within this of another, in ln p, takes that one's place, or is not added where
 * that one is the datum's or the top's: so that the levels' heights stay strictly ascending.
 */
const ADDED_LEVEL_MERGE_LN_P = 1e-9;

/** A column's levels: ln p, and the pressure asked for at an added level (NaN at the others). */
interface ColumnLevels {
  readonly lnPa: Float64Array;
  readonly askedPa: Float64Array;
}

/**
 * `lnLevels` with a level at each of `addedPa`, descending in ln p (the module's
 * {@link ADDED_LEVEL_MERGE_LN_P}).
 */
function withAddedLevels(lnLevels: Float64Array, addedPa: ReadonlyArray<number>): ColumnLevels {
  if (addedPa.length === 0) {
    return { lnPa: lnLevels, askedPa: new Float64Array(lnLevels.length).fill(Number.NaN) };
  }
  const ends = [lnLevels[0] ?? Number.NaN, lnLevels.at(-1) ?? Number.NaN];
  const levels: Array<readonly [number, number]> = [...lnLevels].map((ln) => [ln, Number.NaN]);
  for (const pa of addedPa) {
    const ln = log(pa);
    const close = (at: number): boolean => Math.abs(at - ln) < ADDED_LEVEL_MERGE_LN_P;
    if (ends.some(close)) {
      continue;
    }
    const near = levels.findIndex(([at]) => close(at));
    if (near >= 0 && !Number.isNaN(levels[near]?.[1] ?? Number.NaN)) {
      continue;
    }
    if (near >= 0) {
      levels.splice(near, 1);
    }
    levels.push([ln, pa]);
  }
  levels.sort((a, b) => b[0] - a[0]);
  return {
    lnPa: Float64Array.from(levels, ([ln]) => ln),
    askedPa: Float64Array.from(levels, ([, pa]) => pa),
  };
}

/**
 * An atmosphere's hydrostatic column, in `f64`, from the datum up to p_s ×
 * {@link COLUMN_TOP_PRESSURE_RATIO} or the top it is given (Design note 3), at g_ref on a sphere of
 * R_ref (Design note 17).
 *
 * @remarks
 * dp ÷ dr = −n μ m_u g with n = p ÷ kT and g = g_ref (R_ref ÷ r)², integrated in closed form rather
 * than by steps: the geopotential height above the datum, Φ(p) = k ÷ (μ m_u g_ref) × ∫ T d ln p
 * from p up to p_s, is exact for every profile, and inverse-square gravity puts it at the height
 * z = R_ref Φ ÷ (R_ref − Φ), the inverse of the U.S. Standard Atmosphere 1976's geopotential
 * height Φ = R_ref z ÷ (R_ref + z) (its eqs. 17–19). Near the datum the local scale height is
 * kT ÷ (μ m_u g_ref); at height z it is that times (1 + z ÷ R_ref)². The vertical column above
 * the datum exceeds p_s ÷ g_ref by about 2H ÷ R_ref, the spherical excess, for an isothermal
 * column of scale height H.
 *
 * The levels are evenly spaced in ln p ({@link COLUMN_INTERVALS}, or as many more as keep a longer
 * span's no wider), with the tropopause and the levels asked for added. The
 * monopole's (R_ref ÷ r)² stands for the level spheroid's free-air gradient, which varies with
 * latitude on a spinning body, ∂γ ÷ ∂h = −(2γ ÷ a)(1 + f + m − 2f sin²φ) (Heiskanen and Moritz
 * 1967, §2-10, eq. 2-121; exactly, eq. 2-120): on Saturn the monopole moves the column above the
 * datum by at most 6 × 10⁻⁴ of itself, at the equator, 0.44 of its 1.4 × 10⁻³ spherical excess
 * (0.13 at the pole).
 *
 * @throws RangeError for inputs that are not finite and positive, a profile as
 *   {@link temperatureAt}, a top not above 0 and below the datum's pressure or a level asked for
 *   outside them, or a column whose geopotential height reaches R_ref below its top, which no
 *   hydrostatic atmosphere under inverse-square gravity holds (its pressure at infinity is above the
 *   top's).
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
  const lnSurfacePa = log(surfacePa);
  const topPa = input.topPa ?? surfacePa * COLUMN_TOP_PRESSURE_RATIO;
  const addedPa = input.levelsPa ?? [];
  if (!(
    isPositive(topPa) &&
    topPa < surfacePa &&
    addedPa.every((p) => p > topPa && p < surfacePa)
  )) {
    throw new RangeError(
      `a column's top lies above its datum, ${surfacePa} Pa, and each added level between them, got a top of ${topPa} Pa and levels at ${addedPa.join(", ")} Pa`,
    );
  }
  const lnTopPa =
    input.topPa === undefined ? lnSurfacePa + log(COLUMN_TOP_PRESSURE_RATIO) : log(input.topPa);
  const tropopause = lnTropopausePa(profile);
  const integralK = temperatureIntegralK(profile, tropopause);
  const metresPerKelvin =
    BOLTZMANN_J_PER_K / (molarMassGPerMol * ATOMIC_MASS_CONSTANT_KG * referenceGravityMS2);
  const heightM = (phiM: number): number => (referenceRadiusM * phiM) / (referenceRadiusM - phiM);
  const topPhiM = metresPerKelvin * integralK(lnTopPa, lnSurfacePa);
  if (!(topPhiM < referenceRadiusM)) {
    throw new RangeError(
      `the column is not bound: its geopotential height reaches R_ref = ${referenceRadiusM} m below p = ${topPa} Pa`,
    );
  }
  const levels = withAddedLevels(
    levelLogPressures(lnSurfacePa, input.topPa === undefined ? null : lnTopPa, tropopause),
    addedPa,
  );
  const lnLevels = levels.lnPa;
  const last = lnLevels.length - 1;
  const altitudesM = new Float64Array(lnLevels.length);
  const pressuresPa = new Float64Array(lnLevels.length);
  const temperaturesK = new Float64Array(lnLevels.length);
  const relative = new Float64Array(lnLevels.length);
  const surfaceK = profileTemperatureK(profile, surfacePa);
  for (const [i, lnPa] of lnLevels.entries()) {
    const askedPa = levels.askedPa[i] ?? Number.NaN;
    const pressurePa =
      i === 0 ? surfacePa : i === last ? topPa : Number.isNaN(askedPa) ? exp(lnPa) : askedPa;
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

/**
 * A column's pressure as a medium carries it (`medium.ts`'s `AtmosphereMedium.pressureBar`; R08.T4.d):
 * p ÷ {@link ABSORBER_PRESSURE_REFERENCE_PA} at each of its levels, over their heights, from which
 * the tables and the twin take each path's absorber-weighted pressure for line absorbers' curves.
 */
export function columnPressureBar(column: AtmosphereColumn): TabulatedDensity {
  return tabulatedDensity(
    column.altitudesM,
    column.pressuresPa.map((p) => p / ABSORBER_PRESSURE_REFERENCE_PA),
  );
}
