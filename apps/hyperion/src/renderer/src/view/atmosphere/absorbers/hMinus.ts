/**
 * The negative hydrogen ion's continuous absorption in closed form (plan R08, R08.T4.c): John 1988's
 * fits (T. L. John, "Continuous absorption by the negative hydrogen ion reconsidered", Astron.
 * Astrophys. 193 (1988) 189–192, ADS 1988A&A...193..189J), read from the paper's scan.
 *
 * @remarks
 * - **Bound–free** (photodetachment, H⁻ + hν → H + e⁻), per H⁻ ion: his Eqs. 4–5 and Table 2, fitted
 *   to Wishart 1979's cross-sections "to 1% or better" over 0.125–1.6419 µm. The client takes each
 *   H⁻ column from the record (`decision-r08-t4b-t12c.md`, follow-up 2: ionisation is the
 *   generator's), so his Eq. 3's Saha factor, which turns σ into a coefficient per H atom and unit
 *   electron pressure, is not applied; its stimulated-emission factor, 1 − e^(−hc ÷ λkT), is.
 * - **Free–free** (H + e⁻ + hν → H + e⁻), per H atom and free electron: his Eq. 6 and Tables 3a
 *   and 3b, fitted to Bell and Berrington 1987's coefficients (J. Phys. B 20, 801) "to 1% and
 *   better" over 0.182 µm < λ and 0.5 ≤ 5040 ÷ T ≤ 3.6 (1,400–10,080 K). Eq. 6 gives k_ff in cm⁴
 *   dyn⁻¹ per H atom and unit electron pressure, stimulated emission included, as Bell and
 *   Berrington tabulate it (Kurucz's ATLAS9 adds no stimulated-emission factor to it,
 *   `HMINOP`), so the absorption coefficient is k_ff n_H P_e = k_ff k_B T n_H n_e, and the pair's
 *   cross-section is k_ff k_B T, cm⁵.
 * - **His Table 1 is not a reference for free–free.** It reproduces the fits' bound–free plus a
 *   tenth of Eq. 6's free–free, in units of 10⁻²⁷ cm⁴ dyn⁻¹ (the table prints none; its bound–free
 *   entries match the fit in that unit to 0.4%), to 1.2% at every entry (its rows past the
 *   threshold, free–free alone, are Eq. 6 ÷ 10, to 1.2%), while his §4's free–free shares (34% at
 *   10,080 K, about 6% at 5,040 K and under 1% at 2,520 K below 0.9 µm) and Bell and Berrington's
 *   own values (as `HMINOP` carries them) agree with Eq. 6 as printed. So Table 1's free–free
 *   entries hold Bell and Berrington's 10⁻²⁶ figures in a table of 10⁻²⁷, and the tests take
 *   Table 1 for bound–free and Bell and Berrington for free–free.
 */

/** The H⁻ photodetachment threshold, µm: λ₀ = 1.6419 (John 1988, Eq. 3's text). */
export const H_MINUS_THRESHOLD_UM = 1.6419;

/**
 * The second radiation constant hc ÷ k, µm K: 14,387.768 77 (CODATA 2018, exact), John's α
 * (1.439 × 10⁸ Å K).
 */
const SECOND_RADIATION_UM_K = 14_387.768_775;

/** Boltzmann's constant, erg K⁻¹ (CODATA 2018, exact). */
const BOLTZMANN_ERG_PER_K = 1.380_649e-16;

/** One cm² in m². */
const M2_PER_CM2 = 1e-4;

/** One cm⁵ in m⁵. */
const M5_PER_CM5 = 1e-10;

/** John 1988's Table 2: C₁…C₆ of his Eq. 5. */
const BOUND_FREE_C = [152.519, 49.534, -118.858, 92.536, -34.194, 4.982] as const;

/** The six coefficient columns A…F of John's Eq. 6, n = 1…6 a row. */
type FreeFreeTable = readonly (readonly [number, number, number, number, number, number])[];

/** John 1988's Table 3a: Eq. 6's A_n…F_n for λ > 0.3645 µm. */
const FREE_FREE_LONG: FreeFreeTable = [
  [0, 0, 0, 0, 0, 0],
  [2_483.346, 285.827, -2_054.291, 2_827.776, -1_341.537, 208.952],
  [-3_449.889, -1_158.382, 8_746.523, -11_485.632, 5_303.609, -812.939],
  [2_200.04, 2_427.719, -13_651.105, 16_755.524, -7_510.494, 1_132.738],
  [-696.271, -1_841.4, 8_624.97, -10_051.53, 4_400.067, -655.02],
  [88.283, 444.517, -1_863.864, 2_095.288, -901.788, 132.985],
];

/** John 1988's Table 3b: Eq. 6's A_n…F_n for 0.1823 < λ < 0.3645 µm. */
const FREE_FREE_SHORT: FreeFreeTable = [
  [518.1021, -734.8666, 1_021.1775, -479.0721, 93.1373, -6.4285],
  [473.2636, 1_443.4137, -1_977.3395, 922.3575, -178.9275, 12.36],
  [-482.2089, -737.1616, 1_096.8827, -521.1341, 101.7963, -7.0571],
  [115.5291, 169.6374, -245.649, 114.243, -21.9972, 1.5097],
  [0, 0, 0, 0, 0, 0],
  [0, 0, 0, 0, 0, 0],
];

/** The wavelength at which John's Eq. 6 changes tables, µm (Tables 3a and 3b). */
const FREE_FREE_TABLE_BREAK_UM = 0.3645;

/** The wavelengths over which Eq. 6 holds, µm: from 0.1823 (Table 3b) up. */
const FREE_FREE_SHORTEST_UM = 0.1823;

/** The bound–free fit's shortest wavelength, µm: 0.125 (John, Table 2's range). */
const BOUND_FREE_SHORTEST_UM = 0.125;

/**
 * The temperatures over which John's free–free fit holds, K: 5040 ÷ 3.6 to 5040 ÷ 0.5, 1,400 to
 * 10,080 (his "0.5 ≤ 5040/T ≤ 3.6").
 */
export const H_MINUS_FREE_FREE_RANGE_K: readonly [number, number] = [5_040 / 3.6, 5_040 / 0.5];

/** Refuses a wavelength or temperature the fits cannot take. */
function checkArguments(wavelengthNm: number, temperatureK: number, shortestUm: number): void {
  if (!(wavelengthNm >= shortestUm * 1_000 && Number.isFinite(wavelengthNm))) {
    throw new RangeError(
      `John 1988's fit holds from ${shortestUm * 1_000} nm, not at ${wavelengthNm} nm`,
    );
  }
  if (!(temperatureK > 0 && Number.isFinite(temperatureK))) {
    throw new RangeError(`a temperature is finite and above 0 K, not ${temperatureK}`);
  }
}

/**
 * H⁻'s photodetachment cross-section, cm² an ion, at a vacuum wavelength in µm: John's Eqs. 4–5,
 * σ = 10⁻¹⁸ λ³ (1/λ − 1/λ₀)^(3/2) Σₙ Cₙ (1/λ − 1/λ₀)^((n − 1)/2), 0 at and past λ₀.
 */
function boundFreeCm2(wavelengthUm: number): number {
  if (wavelengthUm >= H_MINUS_THRESHOLD_UM) {
    return 0;
  }
  const x = 1 / wavelengthUm - 1 / H_MINUS_THRESHOLD_UM;
  let f = 0;
  for (const [n, c] of BOUND_FREE_C.entries()) {
    f += c * x ** (n / 2);
  }
  return 1e-18 * wavelengthUm ** 3 * x ** 1.5 * f;
}

/**
 * H⁻'s bound–free absorption cross-section, m² an ion, at a vacuum wavelength and temperature:
 * John 1988's Eqs. 4–5 times the stimulated-emission factor 1 − e^(−hc ÷ λkT) of his Eq. 3.
 *
 * @remarks
 * The factor is the only temperature dependence: at 760 nm it is 0.9995 at 2,500 K and 0.991 at
 * 4,000 K. The fit holds over 125–1,641.9 nm, past which it is 0.
 *
 * @throws RangeError for a wavelength below 125 nm or a temperature not finite and above 0 K.
 */
export function hMinusBoundFreeM2(wavelengthNm: number, temperatureK: number): number {
  checkArguments(wavelengthNm, temperatureK, BOUND_FREE_SHORTEST_UM);
  const um = wavelengthNm / 1_000;
  const stimulated = -Math.expm1(-SECOND_RADIATION_UM_K / (um * temperatureK));
  return boundFreeCm2(um) * stimulated * M2_PER_CM2;
}

/**
 * H⁻'s free–free coefficient k_ff, cm⁴ dyn⁻¹ per H atom and unit electron pressure, John 1988's
 * Eq. 6: 10⁻²⁹ Σₙ (5040 ÷ T)^((n + 1)/2) (λ² Aₙ + Bₙ + Cₙ/λ + Dₙ/λ² + Eₙ/λ³ + Fₙ/λ⁴), λ in µm.
 *
 * @remarks
 * Exported for the tests, which hold it to Bell and Berrington's values; the client reads
 * {@link hMinusFreeFreeM5}.
 *
 * @throws RangeError for a wavelength below 182.3 nm or a temperature not finite and above 0 K.
 */
export function hMinusFreeFreeCm4PerDyn(wavelengthNm: number, temperatureK: number): number {
  checkArguments(wavelengthNm, temperatureK, FREE_FREE_SHORTEST_UM);
  const um = wavelengthNm / 1_000;
  const table = um > FREE_FREE_TABLE_BREAK_UM ? FREE_FREE_LONG : FREE_FREE_SHORT;
  const theta = 5_040 / temperatureK;
  let sum = 0;
  for (const [i, [a, b, c, d, e, f]] of table.entries()) {
    const n = i + 1;
    sum +=
      theta ** ((n + 1) / 2) * (um * um * a + b + c / um + d / um ** 2 + e / um ** 3 + f / um ** 4);
  }
  return 1e-29 * sum;
}

/**
 * The H–e⁻ pair's free–free cross-section, m⁵ a pair, at a vacuum wavelength and temperature:
 * k_ff k_B T, so that the absorption coefficient is σ n_H n_e (John 1988, Eq. 6, with
 * P_e = n_e k_B T).
 *
 * @remarks
 * The temperature is taken as given: {@link H_MINUS_FREE_FREE_RANGE_K} is the fit's range, and
 * `absorbers.ts` holds its ends beyond it and labels the result `estimated`.
 *
 * @throws RangeError as {@link hMinusFreeFreeCm4PerDyn}.
 */
export function hMinusFreeFreeM5(wavelengthNm: number, temperatureK: number): number {
  return (
    hMinusFreeFreeCm4PerDyn(wavelengthNm, temperatureK) *
    BOLTZMANN_ERG_PER_K *
    temperatureK *
    M5_PER_CM5
  );
}
