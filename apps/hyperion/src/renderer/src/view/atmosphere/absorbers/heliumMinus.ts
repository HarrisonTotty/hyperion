/**
 * The negative helium ion's free–free absorption (plan R08, R08.T4.c; `decision-r08-t4b-t12c.md`,
 * follow-ups 3 and its additions, ruling 5): the He–e⁻ pair's cross-section from John 1994's
 * Table 2 (T. L. John, "Absorption coefficients of the negative helium ion", Mon. Not. R. Astron.
 * Soc. 269 (1994) 871–878, ADS 1994MNRAS.269..871J), read from the paper's scan.
 *
 * @remarks
 * John's K_λ(T) is per neutral He atom and unit electron pressure, cm⁴ dyn⁻¹, stimulated emission
 * included (his Eq. 2), from Nesbet 1979's e–He phase shifts by his corrected phase-shift method
 * (Eqs. 13–14, α = 0.5), over 1,400–10,080 K (θ = 5040 ÷ T from 0.5 to 3.6). He gives errors "well
 * below 10 per cent for the range 0.506 ≤ λ(µm) ≤ 1" (§4). So the absorption coefficient is
 * K n_He P_e = K k_B T n_He n_e, and the pair's cross-section is K k_B T, cm⁵. The values are
 * asserted facts of the paper's table, taken onto the client's grid by interpolation; no
 * fit is published.
 *
 * Between his rows the scaled coefficient A_λ = λ⁻² K_λ (his Eq. 16), which he notes varies slowly,
 * is taken linear in 1/λ, and in θ between his columns. Below his shortest row, 506.3 nm, the
 * table holds nothing, and A_λ is held at that row's value, K = A₅₀₆ λ², a stated approximation:
 * He⁻ free–free goes as about λ² (K rises 3.0 times from 506.3 to 911.3 nm at θ 1.0, λ² 3.2),
 * so over 380–506 nm it lies between 0.56 and 1 times its 506 nm value, and the held A_λ gives
 * the lower end, 0.56 at 380 nm.
 */

/** John 1994's Table 2 wavelengths that reach the bake range, µm, ascending. */
const ROWS_UM = [0.5063, 0.5695, 0.6509, 0.7594, 0.9113] as const;

/** John 1994's Table 2 temperatures, as θ = 5040 ÷ T. */
const THETAS = [0.5, 0.6, 0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.8, 3.6] as const;

/**
 * John 1994's Table 2, K_λ ×10⁻²⁶ cm⁴ dyn⁻¹, the rows of {@link ROWS_UM} at {@link THETAS}.
 */
const TABLE: readonly (readonly number[])[] = [
  [0.033, 0.036, 0.043, 0.049, 0.055, 0.061, 0.066, 0.072, 0.078, 0.1, 0.121],
  [0.041, 0.045, 0.053, 0.061, 0.067, 0.074, 0.081, 0.087, 0.094, 0.12, 0.145],
  [0.053, 0.059, 0.069, 0.077, 0.086, 0.094, 0.102, 0.109, 0.117, 0.148, 0.178],
  [0.072, 0.079, 0.092, 0.103, 0.114, 0.124, 0.133, 0.143, 0.152, 0.19, 0.227],
  [0.102, 0.113, 0.131, 0.147, 0.16, 0.173, 0.186, 0.198, 0.21, 0.258, 0.305],
];

/** The temperatures John's table spans, K: 5040 ÷ 3.6 to 5040 ÷ 0.5, 1,400 to 10,080. */
export const HE_MINUS_FREE_FREE_RANGE_K: readonly [number, number] = [5_040 / 3.6, 5_040 / 0.5];

/** Boltzmann's constant, erg K⁻¹ (CODATA 2018, exact). */
const BOLTZMANN_ERG_PER_K = 1.380_649e-16;

/** The unit of {@link TABLE}'s entries, cm⁴ dyn⁻¹. */
const TABLE_UNIT_CM4_PER_DYN = 1e-26;

/** One cm⁵ in m⁵. */
const M5_PER_CM5 = 1e-10;

/** The interval of an ascending list holding a value, and the fraction across it, clamped. */
function bracket(
  values: ReadonlyArray<number>,
  x: number,
): { readonly k: number; readonly t: number } {
  let k = 0;
  while (k < values.length - 2 && (values[k + 1] ?? Number.NaN) < x) {
    k += 1;
  }
  const a = values[k] ?? Number.NaN;
  const b = values[k + 1] ?? Number.NaN;
  return { k, t: Math.min(Math.max((x - a) / (b - a), 0), 1) };
}

/**
 * John 1994's He⁻ free–free coefficient K_λ(T), cm⁴ dyn⁻¹ per He atom and unit electron pressure,
 * at a vacuum wavelength and temperature: his Table 2 interpolated, A_λ = λ⁻² K linear in 1/λ
 * between his rows and in θ between his columns, and held below his shortest row, 506.3 nm.
 *
 * @remarks
 * The temperature is clamped to {@link HE_MINUS_FREE_FREE_RANGE_K}; `absorbers.ts` labels a
 * temperature beyond it `estimated`.
 *
 * @throws RangeError for a wavelength above his 911.3 nm row or not finite, or a temperature not
 *   finite and above 0 K.
 */
export function heMinusFreeFreeCm4PerDyn(wavelengthNm: number, temperatureK: number): number {
  const um = wavelengthNm / 1_000;
  const longest = ROWS_UM[ROWS_UM.length - 1] ?? Number.NaN;
  if (!(Number.isFinite(um) && um <= longest)) {
    throw new RangeError(`John 1994's table reaches ${longest * 1_000} nm, not ${wavelengthNm} nm`);
  }
  if (!(temperatureK > 0 && Number.isFinite(temperatureK))) {
    throw new RangeError(`a temperature is finite and above 0 K, not ${temperatureK}`);
  }
  const inverse = ROWS_UM.map((r) => 1 / r).toReversed();
  const row = bracket(inverse, 1 / um);
  const theta = Math.min(
    Math.max(5_040 / temperatureK, THETAS[0]),
    THETAS[THETAS.length - 1] ?? 3.6,
  );
  const column = bracket(THETAS, theta);
  // Rows reversed to ascend in 1/λ; A = λ⁻² K at each corner, held past the 506.3 nm row.
  const scaled = (r: number, c: number): number => {
    const index = ROWS_UM.length - 1 - r;
    const lambda = ROWS_UM[index] ?? Number.NaN;
    return (TABLE[index]?.[c] ?? Number.NaN) / (lambda * lambda);
  };
  const atRow = (r: number): number =>
    scaled(r, column.k) + (scaled(r, column.k + 1) - scaled(r, column.k)) * column.t;
  const a = atRow(row.k) + (atRow(row.k + 1) - atRow(row.k)) * row.t;
  return a * um * um * TABLE_UNIT_CM4_PER_DYN;
}

/**
 * The He–e⁻ pair's free–free cross-section, m⁵ a pair: K_λ k_B T, so that the absorption
 * coefficient is σ n_He n_e (John 1994, Eq. 2, with P_e = n_e k_B T).
 *
 * @throws RangeError as {@link heMinusFreeFreeCm4PerDyn}.
 */
export function heMinusFreeFreeM5(wavelengthNm: number, temperatureK: number): number {
  return (
    heMinusFreeFreeCm4PerDyn(wavelengthNm, temperatureK) *
    BOLTZMANN_ERG_PER_K *
    temperatureK *
    M5_PER_CM5
  );
}
