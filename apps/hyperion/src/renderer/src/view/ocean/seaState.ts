/**
 * A sea's state from its wind (plan R11, R11.T8.a; Design note 12): the wave-slope statistics the
 * glint reads, the height, peak and spectrum the Gerstner set is drawn from, and the whitecaps,
 * from the climate field's 10 m wind and the sea's liquid and air.
 *
 * @remarks
 * Pure: no state, no clock, `f64` throughout, the CPU reference for the ocean's WGSL.
 *
 * - **The winds.** The climate record's wind is the month's mean speed at 10 m (R09's `Wind`; the
 *   WMO's exposure), carried to each law's height by a neutral log profile, U(z) = U₁₀ [1 +
 *   (√C_D ÷ κ) ln(z ÷ 10)], C_D = 1.3 × 10⁻³ and κ = 0.4 ({@link windAtHeight}): 1.0201 U₁₀ at
 *   Cox and Munk's 12.5 m and 1.0602 U₁₀ at Pierson and Moskowitz's 19.5 m. Cox–Munk is linear in
 *   U and takes the month's mean speed; Pierson–Moskowitz and the whitecaps are superlinear and take
 *   it as their representative wind, so labelled.
 * - **Other worlds** scale through an equivalent Earth wind ({@link equivalentWind}), U_eq = U
 *   (c_m,⊕ ÷ c_m) √[(ρ_a ÷ ρ_l) ÷ (ρ_a ÷ ρ_l)⊕], c_m = (4gγ ÷ ρ_l)^¼ the minimum phase speed, the
 *   identity at Earth's parameters: a heuristic of low confidence (R11's Risks), which the
 *   short-wave laws, Cox–Munk's slopes and the whitecaps, take in place of U. The height and peak
 *   of the gravity waves (Pierson–Moskowitz, JONSWAP) take the body's own wind and gravity (lean,
 *   2026-11-13): a fully developed sea's peak runs at about 1.2 times the wind that raised it, a
 *   kinematic limit in the body's own U ÷ g (H_s ∝ U² ÷ g, as Lorenz et al. 2005, Icarus 175, 392,
 *   scale Titan's), where U_eq would put a 6 m sea under a 1 m/s breeze on Titan, its peak outrunning
 *   the wind sixfold.
 * - **Glassy below a threshold.** Waves grow where the wind's input beats viscous damping
 *   ({@link generationThreshold}); below that wind the sea is glassy, its slopes at the floor of
 *   10⁻⁴ a axis and no wave drawn, as Cassini found Titan's seas.
 * - **Cox and Munk** (1954, J. Opt. Soc. Am. 44, 838, the clean surface): σ_u² = 3.16 × 10⁻³ U and
 *   σ_c² = 0.003 + 1.92 × 10⁻³ U, U at 12.5 m, clamped to 0–20 m/s; their data reach about
 *   14 m/s, so a sea above that is extrapolated ({@link SeaState.extrapolated}). The two fits sum
 *   to 0.003 + 5.08 × 10⁻³ U, against their own fit of the total, 0.003 + 5.12 × 10⁻³ U (±0.004):
 *   each axis takes its own fit, since the BRDF is anisotropic.
 * - **Pierson and Moskowitz** (1964, J. Geophys. Res. 69, 5181): S(ω) = α g² ω⁻⁵
 *   exp[−β (g ÷ U₁₉.₅ω)⁴], α = 8.1 × 10⁻³ and β = 0.74, whose peak is ω_p = (4β ÷ 5)^¼ g ÷ U₁₉.₅ =
 *   0.877 g ÷ U₁₉.₅ and whose H_s = 4√m₀ = 4 √(α ÷ 4β) U₁₉.₅² ÷ g = 0.209 U₁₉.₅² ÷ g, Design note
 *   12's 0.21. Over a basin, JONSWAP's fetch-limited growth (Hasselmann et al. 1973, Dtsch. Hydrogr.
 *   Z. A8(12)): g H_s ÷ U₁₀² = 1.6 × 10⁻³ (g X ÷ U₁₀²)^½ (their E = 1.6 × 10⁻⁷ χ in g²E ÷ U₁₀⁴) and
 *   ω_p U₁₀ ÷ g = 2π × 3.5 χ^(−0.33), capped at full development, where it reaches
 *   Pierson–Moskowitz. Each spectrum's shape is normalised to its H_s ({@link spectralDensity}).
 * - **Whitecaps** (Monahan and O'Muircheartaigh 1980, J. Phys. Oceanogr. 10, 2094): W = 3.84 ×
 *   10⁻⁶ U₁₀^3.41, the covered fraction, drawn on the high setting only.
 *
 * No source could be re-read when this module was written or re-checked (no network on
 * 2026-11-02; the sources' sites refused on 2026-11-13); each figure is the one the plan's research
 * and the rulings state, re-derived here where it follows from others (ω_p and H_s from α and β),
 * and the rest recorded unchecked in the lane's findings.
 */

/** The drag coefficient of the neutral log profile, C_D at 10 m (Design note 12): 1.3 × 10⁻³. */
export const NEUTRAL_DRAG_COEFFICIENT = 1.3e-3;

/** von Kármán's constant κ (Design note 12): 0.4. */
export const VON_KARMAN = 0.4;

/** The height of the climate record's wind, m: the WMO's 10 m exposure (R09's `Wind`). */
export const CLIMATE_WIND_HEIGHT_M = 10;

/** Cox and Munk's anemometer height, m: 12.5 m (41 ft; Cox and Munk 1954). */
export const COX_MUNK_HEIGHT_M = 12.5;

/** Pierson and Moskowitz's anemometer height, m: 19.5 m (Pierson and Moskowitz 1964). */
export const PIERSON_MOSKOWITZ_HEIGHT_M = 19.5;

/** The wind Cox–Munk is held to, m/s: 0 to 20 (Design note 12). */
export const COX_MUNK_MAX_WIND_MPS = 20;

/** The strongest wind Cox and Munk measured, m/s, about 14: above it the law is extrapolated. */
export const COX_MUNK_MEASURED_WIND_MPS = 14;

/** The slope variance below which no axis falls, the floor that keeps the sun's disc resolved. */
export const SLOPE_VARIANCE_FLOOR = 1e-4;

/** Cox and Munk's clean-surface upwind slope variance per m/s at 12.5 m: 3.16 × 10⁻³. */
export const COX_MUNK_UPWIND_PER_MPS = 3.16e-3;

/** Cox and Munk's clean-surface crosswind slope variance at calm: 0.003. */
export const COX_MUNK_CROSSWIND_CALM = 0.003;

/** Cox and Munk's clean-surface crosswind slope variance per m/s at 12.5 m: 1.92 × 10⁻³. */
export const COX_MUNK_CROSSWIND_PER_MPS = 1.92e-3;

/** Pierson and Moskowitz's α: 8.1 × 10⁻³. */
export const PIERSON_MOSKOWITZ_ALPHA = 8.1e-3;

/** Pierson and Moskowitz's β: 0.74. */
export const PIERSON_MOSKOWITZ_BETA = 0.74;

/** JONSWAP's H_s growth: g H_s ÷ U₁₀² = 1.6 × 10⁻³ χ^½ (Hasselmann et al. 1973). */
export const JONSWAP_HEIGHT_GROWTH = 1.6e-3;

/** JONSWAP's peak: f_p U₁₀ ÷ g = 3.5 χ^(−0.33) (Hasselmann et al. 1973). */
export const JONSWAP_PEAK_FREQUENCY = 3.5;

/** JONSWAP's peak exponent on χ: −0.33 (Hasselmann et al. 1973). */
export const JONSWAP_PEAK_EXPONENT = -0.33;

/** JONSWAP's mean peak enhancement γ: 3.3 (Hasselmann et al. 1973). */
export const JONSWAP_PEAK_ENHANCEMENT = 3.3;

/** JONSWAP's peak widths σ_a below the peak and σ_b above it: 0.07 and 0.09. */
export const JONSWAP_PEAK_WIDTHS: readonly [number, number] = [0.07, 0.09];

/** Monahan and O'Muircheartaigh 1980's whitecap coefficient: 3.84 × 10⁻⁶. */
export const WHITECAP_COEFFICIENT = 3.84e-6;

/** Monahan and O'Muircheartaigh 1980's whitecap exponent: 3.41. */
export const WHITECAP_EXPONENT = 3.41;

/**
 * Donelan and Pierson's wind input coefficient: the energy's growth rate β = 0.194 ω (ρ_a ÷ ρ_l)
 * (U_{π/k} ÷ c − 1)² (Donelan and Pierson 1987, J. Geophys. Res. 92, 4971; the form Hayes et al.
 * 2013, Icarus 225, 403, carried to Titan). From memory: the paper could not be re-read.
 */
export const WIND_INPUT_COEFFICIENT = 0.194;

/** Charnock's constant of the rough-flow roughness length z₀ = α u★² ÷ g: 0.011 (Smith 1988). */
export const CHARNOCK_CONSTANT = 0.011;

/** The smooth-flow roughness length's coefficient, z₀ = 0.11 ν_a ÷ u★ (Smith 1988, JGR 93, 15467). */
export const SMOOTH_ROUGHNESS_COEFFICIENT = 0.11;

/**
 * Earth's sea, the reference at which {@link equivalentWind} is the identity: standard gravity,
 * pure water at 20 °C (998.2 kg m⁻³ and γ = 0.07274 N/m, IAPWS R1-76's 72.74 mN/m; μ = 1.002 mPa s,
 * IAPWS R12-08) and the ISA's sea-level air (1.225 kg m⁻³, ν = 1.461 × 10⁻⁵ m² s⁻¹).
 */
export const EARTH_SEA: {
  readonly gravityMps2: number;
  readonly liquid: SeaLiquid;
  readonly air: SeaAir;
} = {
  gravityMps2: 9.80665,
  liquid: { densityKgM3: 998.2, surfaceTensionNPerM: 0.07274, viscosityPaS: 1.002e-3 },
  air: { densityKgM3: 1.225, kinematicViscosityM2s: 1.461e-5 },
};

/** What the sea state reads of the liquid: its record's, `null` where not modelled. */
export interface SeaLiquid {
  readonly densityKgM3: number;
  readonly surfaceTensionNPerM: number | null;
  readonly viscosityPaS: number | null;
}

/** The air over the sea at the surface: density 0 on an airless body, whose sea never moves. */
export interface SeaAir {
  readonly densityKgM3: number;
  /** ν_a, m² s⁻¹, positive. */
  readonly kinematicViscosityM2s: number;
}

/** A sea's inputs. */
export interface SeaStateInput {
  /** The wind's mean speed, m/s, at least 0. */
  readonly windMps: number;
  /** The height that speed is at, m: {@link CLIMATE_WIND_HEIGHT_M} for the climate's. */
  readonly windHeightM: number;
  readonly gravityMps2: number;
  readonly liquid: SeaLiquid;
  readonly air: SeaAir;
  /** The fetch over a closed basin, m; `null` over the open sea, fully developed. */
  readonly fetchM: number | null;
}

/** Which spectrum's shape a sea's waves take. */
export type SeaSpectrum = "pierson-moskowitz" | "jonswap";

/** A sea's state. */
export interface SeaState {
  readonly gravityMps2: number;
  /** γ ÷ ρ_l, m³ s⁻², the dispersion's capillary term; 0 where the tension is not modelled. */
  readonly capillaryM3PerS2: number;
  /** The body's own wind at 10 m, m/s. */
  readonly wind10Mps: number;
  /** The equivalent Earth wind at 10 m, m/s ({@link equivalentWind}). */
  readonly equivalentWind10Mps: number;
  /** The wind the generation threshold is met at, the body's own at 10 m, m/s. */
  readonly thresholdMps: number;
  /** Whether the wind is below the threshold: no waves, slopes at the floor. */
  readonly glassy: boolean;
  /** The equivalent wind at 12.5 m that Cox–Munk took, m/s, clamped to 0–20. */
  readonly coxMunkWindMps: number;
  /** Whether that wind is above Cox and Munk's measured 14 m/s. */
  readonly extrapolated: boolean;
  /** The total slope variances along the wind and across it, each at least the floor. */
  readonly slopeVariance: { readonly upwind: number; readonly crosswind: number };
  readonly significantHeightM: number;
  readonly peakAngularFrequencyRadS: number;
  readonly peakWavenumberRadPerM: number;
  readonly spectrum: SeaSpectrum;
  /** Whether a basin's fetch limits the sea below full development. */
  readonly fetchLimited: boolean;
  /** The whitecap fraction, 0 to 1. */
  readonly whitecapFraction: number;
}

/** √C_D ÷ κ, the log profile's slope. */
const PROFILE_SLOPE = Math.sqrt(NEUTRAL_DRAG_COEFFICIENT) / VON_KARMAN;

/**
 * The wind at `toHeightM` of a wind `speedMps` at `fromHeightM`, m/s, by the neutral log profile
 * U(z) = U₁₀ [1 + (√C_D ÷ κ) ln(z ÷ 10)].
 */
export function windAtHeight(speedMps: number, fromHeightM: number, toHeightM: number): number {
  const at = (z: number): number => 1 + PROFILE_SLOPE * Math.log(z / CLIMATE_WIND_HEIGHT_M);
  return (speedMps * at(toHeightM)) / at(fromHeightM);
}

/** The minimum phase speed of capillary-gravity waves, (4gγ ÷ ρ_l)^¼, m/s. */
export function minimumPhaseSpeedMps(
  gravityMps2: number,
  surfaceTensionNPerM: number,
  densityKgM3: number,
): number {
  return Math.sqrt(Math.sqrt((4 * gravityMps2 * surfaceTensionNPerM) / densityKgM3));
}

/**
 * The equivalent Earth wind of a wind `windMps` over a sea, m/s: U (c_m,⊕ ÷ c_m) √[(ρ_a ÷ ρ_l) ÷
 * (ρ_a ÷ ρ_l)⊕] (Design note 12), at the same height; the identity at {@link EARTH_SEA}. A liquid
 * whose surface tension is not modelled takes c_m ÷ c_m,⊕ = 1, and an airless sea 0.
 */
export function equivalentWind(
  windMps: number,
  gravityMps2: number,
  liquid: SeaLiquid,
  air: SeaAir,
): number {
  if (!(air.densityKgM3 > 0)) {
    return 0;
  }
  const earth = EARTH_SEA.liquid;
  const tension = earth.surfaceTensionNPerM ?? Number.NaN;
  const phaseRatio =
    liquid.surfaceTensionNPerM === null
      ? 1
      : minimumPhaseSpeedMps(EARTH_SEA.gravityMps2, tension, earth.densityKgM3) /
        minimumPhaseSpeedMps(gravityMps2, liquid.surfaceTensionNPerM, liquid.densityKgM3);
  const densityRatio =
    air.densityKgM3 / liquid.densityKgM3 / (EARTH_SEA.air.densityKgM3 / earth.densityKgM3);
  return windMps * phaseRatio * Math.sqrt(densityRatio);
}

/** ω(k) = √(gk + (γ ÷ ρ) k³), rad/s: capillary-gravity waves on deep liquid. */
export function angularFrequencyOf(
  wavenumberRadPerM: number,
  gravityMps2: number,
  capillaryM3PerS2: number,
): number {
  const k = wavenumberRadPerM;
  return Math.sqrt(gravityMps2 * k + capillaryM3PerS2 * k * k * k);
}

/** The wavenumber whose {@link angularFrequencyOf} is `omegaRadS`, rad/m, by Newton's method. */
export function wavenumberOf(
  omegaRadS: number,
  gravityMps2: number,
  capillaryM3PerS2: number,
): number {
  // The gravity branch's k = ω² ÷ g lies at or above the root, where ω² − gk − (γ/ρ)k³ is concave,
  // so Newton's steps fall monotonically onto it.
  let k = (omegaRadS * omegaRadS) / gravityMps2;
  for (let i = 0; i < 64; i++) {
    const f = gravityMps2 * k + capillaryM3PerS2 * k * k * k - omegaRadS * omegaRadS;
    const df = gravityMps2 + 3 * capillaryM3PerS2 * k * k;
    const next = k - f / df;
    if (!(Math.abs(next - k) > 1e-15 * k)) {
      return next;
    }
    k = next;
  }
  return k;
}

/** The roughness length of the log profile over the sea at friction velocity u★, m (Smith 1988). */
function roughnessLengthM(frictionMps: number, gravityMps2: number, air: SeaAir): number {
  return (
    (SMOOTH_ROUGHNESS_COEFFICIENT * air.kinematicViscosityM2s) / frictionMps +
    (CHARNOCK_CONSTANT * frictionMps * frictionMps) / gravityMps2
  );
}

/** The wind at height z over the sea at friction velocity u★, m/s: (u★ ÷ κ) ln(z ÷ z₀), at least 0. */
function profileWindMps(
  frictionMps: number,
  heightM: number,
  gravityMps2: number,
  air: SeaAir,
): number {
  const z0 = roughnessLengthM(frictionMps, gravityMps2, air);
  return (frictionMps / VON_KARMAN) * Math.max(Math.log(heightM / z0), 0);
}

/** The friction velocity whose wind at `heightM` is `windMps`, m/s, by bisection in ln u★. */
function frictionFor(windMps: number, heightM: number, gravityMps2: number, air: SeaAir): number {
  let low = Math.log(1e-7);
  let high = Math.log(10);
  for (let i = 0; i < 80; i++) {
    const mid = 0.5 * (low + high);
    if (profileWindMps(Math.exp(mid), heightM, gravityMps2, air) < windMps) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return Math.exp(high);
}

/** The wavenumbers the threshold is sought over, rad/m: 1 to 10⁴, 64 a decade. */
const THRESHOLD_WAVENUMBERS: ReadonlyArray<number> = Array.from(
  { length: 4 * 64 + 1 },
  (_, i) => 10 ** (i / 64),
);

/**
 * The wind at which waves begin to grow on a sea, the body's own at 10 m, m/s: the least over
 * wavenumber of the 10 m wind at which Donelan and Pierson's wind input, β = 0.194 ω (ρ_a ÷ ρ_l)
 * (U_{π/k} ÷ c − 1)², meets viscous damping, 4νk² (the energy's; Lamb's 2νk² on the amplitude),
 * the wind at π ÷ k from the log profile with Smith's roughness, z₀ = 0.11 ν_a ÷ u★ + 0.011 u★² ÷ g.
 *
 * @remarks
 * A heuristic of low confidence, labelled so (R11's Risks): about 1.7 m/s on Earth, where Design
 * note 12 says about 1 m/s, and 0.39 m/s on a methane sea under Titan's air, at the foot of Hayes
 * et al. 2013's 0.4–0.7 m/s. Infinite over an airless sea. Where the record does not model the
 * viscosity or the surface tension, the threshold is Earth's, met by the equivalent wind
 * ({@link equivalentWind}), so the returned wind is Earth's threshold carried back by the body's
 * own U ÷ U_eq.
 */
export function generationThreshold(gravityMps2: number, liquid: SeaLiquid, air: SeaAir): number {
  if (!(air.densityKgM3 > 0)) {
    return Number.POSITIVE_INFINITY;
  }
  if (liquid.viscosityPaS === null || liquid.surfaceTensionNPerM === null) {
    const earth = generationThreshold(EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air);
    return earth / equivalentWind(1, gravityMps2, liquid, air);
  }
  const ratio = air.densityKgM3 / liquid.densityKgM3;
  const nu = liquid.viscosityPaS / liquid.densityKgM3;
  const capillary = liquid.surfaceTensionNPerM / liquid.densityKgM3;
  let least = Number.POSITIVE_INFINITY;
  for (const k of THRESHOLD_WAVENUMBERS) {
    const omega = angularFrequencyOf(k, gravityMps2, capillary);
    const c = omega / k;
    const damping = 4 * nu * k * k;
    const need = c * (1 + Math.sqrt(damping / (WIND_INPUT_COEFFICIENT * ratio * omega)));
    const friction = frictionFor(need, Math.PI / k, gravityMps2, air);
    least = Math.min(least, profileWindMps(friction, CLIMATE_WIND_HEIGHT_M, gravityMps2, air));
  }
  return least;
}

/** The fetch χ = gX ÷ U₁₀² at which JONSWAP's H_s meets Pierson–Moskowitz's, dimensionless. */
function fullyDevelopedFetch(): number {
  const pm = 4 * Math.sqrt(PIERSON_MOSKOWITZ_ALPHA / (4 * PIERSON_MOSKOWITZ_BETA));
  const at19 = windAtHeight(1, CLIMATE_WIND_HEIGHT_M, PIERSON_MOSKOWITZ_HEIGHT_M);
  const ratio = (pm * at19 * at19) / JONSWAP_HEIGHT_GROWTH;
  return ratio * ratio;
}

/**
 * A sea's state from its wind, liquid and air ({@link SeaState}): the slopes and whitecaps at the
 * equivalent Earth wind, the height and peak at the body's own wind, each with the body's own
 * gravity.
 */
export function seaState(input: SeaStateInput): SeaState {
  const g = input.gravityMps2;
  const capillary =
    input.liquid.surfaceTensionNPerM === null
      ? 0
      : input.liquid.surfaceTensionNPerM / input.liquid.densityKgM3;
  const wind10 = windAtHeight(Math.max(input.windMps, 0), input.windHeightM, CLIMATE_WIND_HEIGHT_M);
  const threshold = generationThreshold(g, input.liquid, input.air);
  const glassy = !(wind10 >= threshold);
  const eq10 = equivalentWind(wind10, g, input.liquid, input.air);
  const coxMunkWind = Math.min(
    Math.max(windAtHeight(eq10, CLIMATE_WIND_HEIGHT_M, COX_MUNK_HEIGHT_M), 0),
    COX_MUNK_MAX_WIND_MPS,
  );
  const slopeVariance = glassy
    ? { upwind: SLOPE_VARIANCE_FLOOR, crosswind: SLOPE_VARIANCE_FLOOR }
    : {
        upwind: Math.max(COX_MUNK_UPWIND_PER_MPS * coxMunkWind, SLOPE_VARIANCE_FLOOR),
        crosswind: Math.max(
          COX_MUNK_CROSSWIND_CALM + COX_MUNK_CROSSWIND_PER_MPS * coxMunkWind,
          SLOPE_VARIANCE_FLOOR,
        ),
      };
  // The gravity waves' height and peak at the body's own wind (lean, 2026-11-13; see above).
  const u19 = windAtHeight(wind10, CLIMATE_WIND_HEIGHT_M, PIERSON_MOSKOWITZ_HEIGHT_M);
  let height = 0;
  let peak = Number.POSITIVE_INFINITY;
  let spectrum: SeaSpectrum = "pierson-moskowitz";
  let fetchLimited = false;
  if (!glassy && u19 > 0) {
    height =
      (4 * Math.sqrt(PIERSON_MOSKOWITZ_ALPHA / (4 * PIERSON_MOSKOWITZ_BETA)) * u19 * u19) / g;
    peak = (Math.sqrt(Math.sqrt((4 * PIERSON_MOSKOWITZ_BETA) / 5)) * g) / u19;
    if (input.fetchM !== null) {
      const chi = (g * Math.max(input.fetchM, 0)) / (wind10 * wind10);
      if (chi < fullyDevelopedFetch()) {
        height = (JONSWAP_HEIGHT_GROWTH * Math.sqrt(chi) * wind10 * wind10) / g;
        peak = Math.max(
          (2 * Math.PI * JONSWAP_PEAK_FREQUENCY * chi ** JONSWAP_PEAK_EXPONENT * g) / wind10,
          peak,
        );
        spectrum = "jonswap";
        fetchLimited = true;
      }
    }
  }
  const whitecaps = glassy ? 0 : Math.min(WHITECAP_COEFFICIENT * eq10 ** WHITECAP_EXPONENT, 1);
  return {
    gravityMps2: g,
    capillaryM3PerS2: capillary,
    wind10Mps: wind10,
    equivalentWind10Mps: eq10,
    thresholdMps: threshold,
    glassy,
    coxMunkWindMps: coxMunkWind,
    extrapolated: coxMunkWind > COX_MUNK_MEASURED_WIND_MPS,
    slopeVariance,
    significantHeightM: height,
    peakAngularFrequencyRadS: peak,
    peakWavenumberRadPerM: Number.isFinite(peak) ? wavenumberOf(peak, g, capillary) : 0,
    spectrum,
    fetchLimited,
    whitecapFraction: whitecaps,
  };
}

/**
 * A spectrum's shape in units of its peak, at x = ω ÷ ω_p: x⁻⁵ e^(−1.25 x⁻⁴), with JONSWAP's
 * γ^r on it, r = e^(−(x − 1)² ÷ 2σ²).
 */
function unitShape(spectrum: SeaSpectrum, x: number): number {
  if (!(x > 0)) {
    return 0;
  }
  const inverse = 1 / x;
  const fourth = inverse * inverse * inverse * inverse;
  const base = fourth * inverse * Math.exp(-1.25 * fourth);
  let enhancement = 1;
  switch (spectrum) {
    case "pierson-moskowitz":
      break;
    case "jonswap": {
      const width = x <= 1 ? JONSWAP_PEAK_WIDTHS[0] : JONSWAP_PEAK_WIDTHS[1];
      const d = (x - 1) / width;
      enhancement = JONSWAP_PEAK_ENHANCEMENT ** Math.exp(-0.5 * d * d);
      break;
    }
  }
  return base * enhancement;
}

/**
 * ∫ unit shape dx: 1 ÷ 5 for Pierson–Moskowitz, in closed form (y = 1.25 x⁻⁴), and JONSWAP's by
 * the midpoint rule in ln x over 0.2 to 50, 8,192 steps, where its energy lies to under 10⁻¹⁰.
 */
const UNIT_SHAPE_INTEGRALS: Readonly<Record<SeaSpectrum, number>> = {
  "pierson-moskowitz": 1 / 5,
  jonswap: ((): number => {
    const steps = 8192;
    const low = Math.log(0.2);
    const step = (Math.log(50) - low) / steps;
    let sum = 0;
    for (let i = 0; i < steps; i++) {
      const x = Math.exp(low + (i + 0.5) * step);
      sum += unitShape("jonswap", x) * x;
    }
    return sum * step;
  })(),
};

/**
 * The sea's spectral density at angular frequency ω, m² s: its spectrum's shape, normalised so
 * that its zeroth moment is (H_s ÷ 4)²; 0 for a glassy sea.
 */
export function spectralDensity(state: SeaState, omegaRadS: number): number {
  if (!(state.significantHeightM > 0)) {
    return 0;
  }
  const m0 = (state.significantHeightM / 4) ** 2;
  const peak = state.peakAngularFrequencyRadS;
  return (
    (m0 * unitShape(state.spectrum, omegaRadS / peak)) /
    (peak * UNIT_SHAPE_INTEGRALS[state.spectrum])
  );
}
