/**
 * Atomic and ionic lines for R08.T4.h's answers (plan R08, R08.T4.h; science-r08-unowned-absorbers
 * §2.2): R. L. Kurucz's line records, Barklem and Collet 2016's ionisation energies, partition
 * functions and H₂ equilibrium constant, a Saha balance and the reference column it gives each
 * ionisation stage, each line's Voigt equivalent width by bake bin, and R08.T4.c's 1 nm cell means.
 *
 * @remarks
 * Built here so that R08.T4.e reuses the reference column and the equivalent widths.
 *
 * - **The reference column** of a stage ({@link referenceColumnPerM2}) is its vertical column above
 *   0.1 bar at g = 10 m s⁻², doubled for the mean slant path, at solar abundances (Asplund et al.
 *   2009, {@link SOLAR_ABUNDANCES}) with every atom of each element free (no molecules but H₂, no
 *   condensate), the element's atoms split among its stages by the Saha balance at 0.1 bar
 *   ({@link sahaBalance}). The balance holds the charge over H and every element to Zn and takes
 *   H₂'s dissociation from Barklem and Collet's table 7.
 * - **The equivalent widths** ({@link lineBinWidthsNm}) are the science ruling's upper-bound method:
 *   each line's own Voigt profile (Humlíček 1982, `molecularLines.ts`'s {@link voigtK}) with Doppler
 *   broadening plus 1 km s⁻¹ of microturbulence, classical radiative damping and Unsöld's van der
 *   Waals width (bounded for a neutral atom's Rydberg levels, {@link vanDerWaalsWidthPerS}) doubled,
 *   its 1 − e^(−τ) integrated over the part of its profile inside each bake
 *   bin, and the lines' widths summed without overlap, which only raises them. Stimulated emission
 *   is neglected, which also only raises them.
 * - **The cell means** ({@link lineCellMeansM2}) are R08.T4.c's method: each line's
 *   π r_e gf e^(−c₂E_low ÷ T) ÷ Q(T) in the 1 nm vacuum cell of its wavelength over the cell's width
 *   in cm⁻¹, over the lines whose lower level lies below the ionisation limit.
 *
 * Kurucz states no terms for his lists and Barklem and Collet's tables are CDS's (free for a
 * scientific use): this module reduces them and commits none of them (`decision-r08-licences.md`;
 * `decision-r08-cmf-licence.md` §§7 and 9).
 */

import { SECOND_RADIATION_CONSTANT_CM_K, SPEED_OF_LIGHT_M_PER_S, voigtK } from "./molecularLines";

/** The classical electron radius r_e, cm (CODATA 2018: 2.817 940 3262 × 10⁻¹⁵ m). */
export const ELECTRON_RADIUS_CM = 2.817_940_326_2e-13;

/** Boltzmann's constant, J K⁻¹ (exact since the SI's 2019 redefinition). */
export const BOLTZMANN_J_PER_K = 1.380_649e-23;

/** Planck's constant, J s (exact). */
const PLANCK_J_S = 6.626_070_15e-34;

/** The reduced Planck constant ħ, J s. */
const HBAR_J_S = PLANCK_J_S / (2 * Math.PI);

/** The electron's mass, kg (CODATA 2018: 9.109 383 7015 × 10⁻³¹). */
const ELECTRON_MASS_KG = 9.109_383_701_5e-31;

/** The atomic mass constant m_u, kg (CODATA 2018: 1.660 539 066 60 × 10⁻²⁷). */
export const ATOMIC_MASS_KG = 1.660_539_066_6e-27;

/** The electronvolt, J (exact). */
const ELECTRONVOLT_J = 1.602_176_634e-19;

/** One electronvolt as a wavenumber, cm⁻¹ (CODATA 2018: 8,065.543 937…). */
export const PER_CM_PER_EV = ELECTRONVOLT_J / (PLANCK_J_S * SPEED_OF_LIGHT_M_PER_S * 100);

/** The Bohr radius a₀, m (CODATA 2018: 5.291 772 109 03 × 10⁻¹¹). */
const BOHR_RADIUS_M = 5.291_772_109_03e-11;

/** e² ÷ 4πε₀, J m: r_e m_e c². */
const COULOMB_E2_J_M = ELECTRON_RADIUS_CM * 1e-2 * ELECTRON_MASS_KG * SPEED_OF_LIGHT_M_PER_S ** 2;

/** The Rydberg energy R∞hc, eV (CODATA 2018: 13.605 693 122 994). */
const RYDBERG_EV = 13.605_693_122_994;

// --- Kurucz's records ---

/**
 * One line of a Kurucz `gfZZII.all` list, as its 160-column format gives it (R. L. Kurucz,
 * `linelists/0linelists.readme`, http://kurucz.harvard.edu/linelists/0linelists.readme, SHA-256
 * `9a65f8be…5127d`, read 2026-10-19: FORMAT(F11.4, F7.3, F6.2, F12.3, F5.2, 1X, A10, F12.3, F5.2, 1X,
 * A10, 3F6.2, A4, 2I2, I3, F6.3, I3, F6.3, …)).
 */
export interface KuruczLine {
  /** The wavelength as listed, nm: in vacuum below 200 nm and in air above. */
  readonly listedNm: number;
  /** The vacuum wavelength, nm ({@link kuruczVacuumNm}). */
  readonly vacuumNm: number;
  /**
   * log₁₀ gf with the record's hyperfine and isotopic log fractions added, the gf a line of the
   * element at its natural abundance takes.
   */
  readonly logGf: number;
  /** The lower level's energy, cm⁻¹: the lesser of the record's two, a negative (predicted) one's magnitude. */
  readonly lowerPerCm: number;
  /** The upper level's energy, cm⁻¹. */
  readonly upperPerCm: number;
  /** The lower and upper levels' J. */
  readonly lowerJ: number;
  readonly upperJ: number;
  /** The lower and upper levels' labels, trimmed. */
  readonly lowerLabel: string;
  readonly upperLabel: string;
  /** Kurucz's reference code for the gf value, trimmed. */
  readonly reference: string;
  /**
   * The record's damping constants (R08.T4.e), each log₁₀ of a Lorentzian full width in angular
   * frequency, 0 where the record gives none (a blank or 0.00 field): the radiative Γ_rad, s⁻¹;
   * Γ_4 ÷ n_e, the quadratic Stark width per electron, rad s⁻¹ cm³, taken as independent of
   * temperature; and Γ_6 ÷ N_H, the van der Waals width per neutral hydrogen atom at 10,000 K,
   * rad s⁻¹ cm³ (the readme's fields 10–12, as Kurucz's `rgfall.for` states them, lines 30–41, the
   * R08.T4.e science check's reading).
   */
  readonly logGammaRadPerS: number;
  readonly logGammaStarkPerElectron: number;
  readonly logGammaVanDerWaalsPerHydrogen: number;
}

/** A fixed-width field of a record, as a number. */
function field(record: string, from: number, to: number, what: string, line: number): number {
  const text = record.slice(from, to).trim();
  const value = Number(text);
  if (text.length === 0 || !Number.isFinite(value)) {
    throw new Error(
      `Kurucz record ${line}: its ${what} (columns ${from + 1}-${to}) is not a number`,
    );
  }
  return value;
}

/**
 * A blank-or-number fixed-width field, 0 when blank: the hyperfine and isotopic log fractions,
 * which most records leave 0.000.
 */
function optionalField(
  record: string,
  from: number,
  to: number,
  what: string,
  line: number,
): number {
  return record.slice(from, to).trim().length === 0 ? 0 : field(record, from, to, what, line);
}

/**
 * A Kurucz `gfZZII.all` list's records (format in {@link KuruczLine}), in file order.
 *
 * @throws Error for a record under 124 columns, or a field that is not a number.
 */
export function parseKuruczLines(text: string): ReadonlyArray<KuruczLine> {
  const lines: KuruczLine[] = [];
  for (const [i, raw] of text.split(/\r?\n/u).entries()) {
    const record = raw.trimEnd();
    if (record.length === 0) {
      continue;
    }
    const n = i + 1;
    if (record.length < 124) {
      throw new Error(`Kurucz record ${n} has ${record.length} columns, under the format's 124`);
    }
    const listedNm = field(record, 0, 11, "wavelength", n);
    const first = Math.abs(field(record, 24, 36, "first energy", n));
    const second = Math.abs(field(record, 52, 64, "second energy", n));
    const firstIsLower = first <= second;
    const firstJ = field(record, 36, 41, "first J", n);
    const secondJ = field(record, 64, 69, "second J", n);
    const firstLabel = record.slice(42, 52).trim();
    const secondLabel = record.slice(70, 80).trim();
    lines.push({
      listedNm,
      vacuumNm: kuruczVacuumNm(listedNm),
      logGf:
        field(record, 11, 18, "log gf", n) +
        optionalField(record, 109, 115, "hyperfine fraction", n) +
        optionalField(record, 118, 124, "isotopic fraction", n),
      lowerPerCm: firstIsLower ? first : second,
      upperPerCm: firstIsLower ? second : first,
      lowerJ: firstIsLower ? firstJ : secondJ,
      upperJ: firstIsLower ? secondJ : firstJ,
      lowerLabel: firstIsLower ? firstLabel : secondLabel,
      upperLabel: firstIsLower ? secondLabel : firstLabel,
      reference: record.slice(98, 102).trim(),
      logGammaRadPerS: optionalField(record, 80, 86, "radiative damping", n),
      logGammaStarkPerElectron: optionalField(record, 86, 92, "Stark damping", n),
      logGammaVanDerWaalsPerHydrogen: optionalField(record, 92, 98, "van der Waals damping", n),
    });
  }
  return lines;
}

/**
 * The refractive index of standard air at a vacuum wavelength, nm: D. C. Morton, Astrophys. J.
 * Suppl. Ser. 130 (2000) 403, after Birch and Downs 1994, the IAU standard, n = 1 + 8.342 54 ×
 * 10⁻⁵ + 2.406 147 × 10⁻² ÷ (130 − s²) + 1.5998 × 10⁻⁴ ÷ (38.9 − s²), s = 10⁴ ÷ λ_vac in Å (as
 * VALD's documentation quotes it, "Air-to-vacuum conversion", https://www.astro.uu.se/valdwiki/,
 * read 2026-10-19; the paper was not read). R08.T4.c took Kurucz's air wavelengths to vacuum by it.
 */
export function mortonAirIndex(vacuumNm: number): number {
  const s2 = (1e4 / (vacuumNm * 10)) ** 2;
  return 1 + 8.342_54e-5 + 2.406_147e-2 / (130 - s2) + 1.5998e-4 / (38.9 - s2);
}

/**
 * An air wavelength's vacuum wavelength, nm: λ_vac = λ_air × n(λ_vac) by {@link mortonAirIndex},
 * solved by fixed-point iteration, which contracts by 5 × 10⁻⁶ to 3 × 10⁻⁵ a step over 360–780 nm, to 10⁻¹² nm.
 */
export function vacuumFromAirNm(airNm: number): number {
  let vacuumNm = airNm;
  for (let i = 0; i < 8; i += 1) {
    const next = airNm * mortonAirIndex(vacuumNm);
    if (Math.abs(next - vacuumNm) < 1e-12) {
      return next;
    }
    vacuumNm = next;
  }
  return vacuumNm;
}

/** Kurucz's listed wavelength in vacuum, nm: as listed below 200 nm, from air above. */
export function kuruczVacuumNm(listedNm: number): number {
  return listedNm < 200 ? listedNm : vacuumFromAirNm(listedNm);
}

// --- Barklem and Collet 2016 (A&A 588, A96; CDS J/A+A/588/A96) ---

/**
 * A table of Barklem and Collet's tables 7 and 8: a value per species at each of the table's 42
 * temperatures (10⁻⁵ to 10⁴ K), log₁₀ pK for table 7 and Q for table 8.
 */
export interface BarklemTable {
  readonly temperaturesK: ReadonlyArray<number>;
  /** Each row by its species' name as the table writes it (`Ni_II`, `H2`). */
  readonly rows: ReadonlyMap<string, ReadonlyArray<number>>;
}

/**
 * Barklem and Collet's table 7 or 8 (`table7.dat`, `table8.dat`): its header's `T [K]` line gives
 * the temperatures, and each row a species' name and its value at each.
 *
 * @throws Error for a table without its temperatures, or a row of another length.
 */
export function parseBarklemTable(text: string, name: string): BarklemTable {
  let temperaturesK: number[] | undefined;
  const rows = new Map<string, number[]>();
  for (const line of text.split(/\r?\n/u)) {
    if (line.startsWith("#")) {
      const header = /^#\s*T \[K\]\s+(.*)$/u.exec(line);
      if (header?.[1] !== undefined) {
        temperaturesK = header[1].trim().split(/\s+/u).map(Number);
      }
      continue;
    }
    const words = line.trim().split(/\s+/u);
    const [species, ...values] = words;
    if (species === undefined || species.length === 0) {
      continue;
    }
    if (temperaturesK === undefined || values.length !== temperaturesK.length) {
      throw new Error(`${name}: row ${species} has ${values.length} values, not the header's`);
    }
    rows.set(species, values.map(Number));
  }
  if (temperaturesK === undefined || temperaturesK.some((t) => !Number.isFinite(t))) {
    throw new Error(`${name}: no temperatures in its header`);
  }
  return { temperaturesK, rows };
}

/**
 * Barklem and Collet's table 4: each element's first, second and third ionisation energies, eV
 * (−1 where the stage does not exist), by its symbol.
 *
 * @throws Error for a row that is not an element's.
 */
export function parseIonisationEnergies(
  text: string,
): ReadonlyMap<string, readonly [number, number, number]> {
  const energies = new Map<string, readonly [number, number, number]>();
  for (const line of text.split(/\r?\n/u)) {
    if (line.startsWith("#") || line.trim().length === 0) {
      continue;
    }
    const [, symbol, first, second, third] = line.trim().split(/\s+/u);
    const values = [Number(first), Number(second), Number(third)] as const;
    if (symbol === undefined || values.some((v) => !Number.isFinite(v))) {
      throw new Error(`table 4's row "${line.trim()}" is not an element's ionisation energies`);
    }
    energies.set(symbol, values);
  }
  return energies;
}

/** A natural cubic spline's second derivatives at its knots (x ascending). */
function splineSecondDerivatives(x: ReadonlyArray<number>, y: ReadonlyArray<number>): Float64Array {
  const n = x.length;
  const m = new Float64Array(n);
  const u = new Float64Array(n);
  for (let i = 1; i < n - 1; i += 1) {
    const xi = x[i] ?? 0;
    const lo = x[i - 1] ?? 0;
    const hi = x[i + 1] ?? 0;
    const sig = (xi - lo) / (hi - lo);
    const p = sig * (m[i - 1] ?? 0) + 2;
    m[i] = (sig - 1) / p;
    const slope =
      ((y[i + 1] ?? 0) - (y[i] ?? 0)) / (hi - xi) - ((y[i] ?? 0) - (y[i - 1] ?? 0)) / (xi - lo);
    u[i] = ((6 * slope) / (hi - lo) - sig * (u[i - 1] ?? 0)) / p;
  }
  for (let i = n - 2; i >= 0; i -= 1) {
    m[i] = (m[i] ?? 0) * (m[i + 1] ?? 0) + (u[i] ?? 0);
  }
  return m;
}

/**
 * A natural cubic spline through (x, y) at a point inside its knots.
 *
 * @throws Error outside the knots.
 */
function splineAt(x: ReadonlyArray<number>, y: ReadonlyArray<number>, at: number): number {
  const n = x.length;
  if (!(at >= (x[0] ?? Infinity) && at <= (x[n - 1] ?? -Infinity))) {
    throw new Error(`${at} lies outside the spline's knots`);
  }
  const m = splineSecondDerivatives(x, y);
  let hi = 1;
  while (hi < n - 1 && (x[hi] ?? 0) < at) {
    hi += 1;
  }
  const lo = hi - 1;
  const h = (x[hi] ?? 0) - (x[lo] ?? 0);
  const a = ((x[hi] ?? 0) - at) / h;
  const b = (at - (x[lo] ?? 0)) / h;
  return (
    a * (y[lo] ?? 0) +
    b * (y[hi] ?? 0) +
    (((a ** 3 - a) * (m[lo] ?? 0) + (b ** 3 - b) * (m[hi] ?? 0)) * h * h) / 6
  );
}

/**
 * A species' partition function at a temperature from table 8, by a cubic spline in log₁₀ Q over
 * log₁₀ T, Barklem and Collet's stated interpolation (§3: better than 10⁻⁴ on their grid).
 *
 * @throws Error for a species the table does not hold, or a temperature outside it.
 */
export function partitionFunctionAt(
  table: BarklemTable,
  species: string,
  temperatureK: number,
): number {
  const row = table.rows.get(species);
  if (row === undefined) {
    throw new Error(`table 8 holds no partition function for ${species}`);
  }
  return (
    10 **
    splineAt(table.temperaturesK.map(Math.log10), row.map(Math.log10), Math.log10(temperatureK))
  );
}

/**
 * H₂'s dissociation energy D₀, eV: Barklem and Collet 2016's adopted value, Table 1 (after Y.-R.
 * Luo 2007, Comprehensive Handbook of Chemical Bond Energies), 4.478 007 ± 0.000 004 eV
 * (`table1.dat`, SHA-256 `85f17e00…b5fc`, read 2026-10-19).
 */
export const H2_DISSOCIATION_EV = 4.478_007;

/**
 * The temperature from which {@link hydrogenEquilibriumPa} splines table 7's values, K: below it
 * the table's six figures of log₁₀ pK (−2.26 × 10⁴ at 1 K) leave log₁₀ pK + 5,040 D₀ ÷ T without a
 * figure.
 */
const EQUILIBRIUM_SPLINE_FROM_K = 100;

/**
 * H₂'s dissociation equilibrium constant K_p = p_H² ÷ p_H₂, Pa (Barklem and Collet 2016, Eq. 7),
 * at a temperature: table 7's log₁₀ pK by a cubic spline in log₁₀ pK + 5,040 D₀ ÷ T over log₁₀ T
 * (their §3's interpolation, D₀ {@link H2_DISSOCIATION_EV} in eV), on the table's nodes from
 * 100 K.
 *
 * @throws Error if table 7 holds no H₂ row, or outside 100 K–10⁴ K.
 */
export function hydrogenEquilibriumPa(table: BarklemTable, temperatureK: number): number {
  const row = table.rows.get("H2");
  if (row === undefined) {
    throw new Error("table 7 holds no H2 row");
  }
  const x: number[] = [];
  const y: number[] = [];
  for (const [i, t] of table.temperaturesK.entries()) {
    if (t >= EQUILIBRIUM_SPLINE_FROM_K) {
      x.push(Math.log10(t));
      y.push((row[i] ?? Number.NaN) + (5_040 * H2_DISSOCIATION_EV) / t);
    }
  }
  return (
    10 ** (splineAt(x, y, Math.log10(temperatureK)) - (5_040 * H2_DISSOCIATION_EV) / temperatureK)
  );
}

// --- Solar abundances and the Saha balance ---

/** An element of the Saha balance: its symbol, solar abundance and standard atomic weight. */
export interface SolarElement {
  readonly symbol: string;
  /** log ε = log₁₀(N ÷ N_H) + 12, the present-day photosphere's. */
  readonly logEpsilon: number;
  /** Its standard atomic weight, Da. */
  readonly atomicWeightDa: number;
}

/**
 * The elements to Zn with their present-day solar photospheric abundances, M. Asplund, N.
 * Grevesse, A. J. Sauval and P. Scott, Annu. Rev. Astron. Astrophys. 47 (2009) 481, Table 1 (read
 * 2026-10-19 in arXiv:0909.0948v1, SHA-256 `e89e9711…5c6f`; He, Ne and Ar their indirect
 * photospheric estimates, bracketed there), and standard atomic weights (IUPAC/CIAAW's abridged
 * values, from memory, not re-read: they move the column by under 10⁻⁴). Heavier elements are left
 * out: the electron donors among them (Ga, Rb and Sr hold 6.3, 1.9 and 4.3 × 10⁻⁴ of Na's atoms)
 * would raise n_e by 2.8 × 10⁻³ at 2,000 K, 3.6 × 10⁻⁴ at 3,000 K and 5 × 10⁻⁵ at 4,600 K, by R08.T4.h's
 * science check, which added Ga, Ge, Rb, Sr, Y, Zr, Nb, Mo, Ba, La, Ce and Nd.
 */
export const SOLAR_ABUNDANCES: ReadonlyArray<SolarElement> = [
  { symbol: "H", logEpsilon: 12, atomicWeightDa: 1.008 },
  { symbol: "He", logEpsilon: 10.93, atomicWeightDa: 4.0026 },
  { symbol: "Li", logEpsilon: 1.05, atomicWeightDa: 6.94 },
  { symbol: "Be", logEpsilon: 1.38, atomicWeightDa: 9.0122 },
  { symbol: "B", logEpsilon: 2.7, atomicWeightDa: 10.81 },
  { symbol: "C", logEpsilon: 8.43, atomicWeightDa: 12.011 },
  { symbol: "N", logEpsilon: 7.83, atomicWeightDa: 14.007 },
  { symbol: "O", logEpsilon: 8.69, atomicWeightDa: 15.999 },
  { symbol: "F", logEpsilon: 4.56, atomicWeightDa: 18.998 },
  { symbol: "Ne", logEpsilon: 7.93, atomicWeightDa: 20.18 },
  { symbol: "Na", logEpsilon: 6.24, atomicWeightDa: 22.99 },
  { symbol: "Mg", logEpsilon: 7.6, atomicWeightDa: 24.305 },
  { symbol: "Al", logEpsilon: 6.45, atomicWeightDa: 26.982 },
  { symbol: "Si", logEpsilon: 7.51, atomicWeightDa: 28.085 },
  { symbol: "P", logEpsilon: 5.41, atomicWeightDa: 30.974 },
  { symbol: "S", logEpsilon: 7.12, atomicWeightDa: 32.06 },
  { symbol: "Cl", logEpsilon: 5.5, atomicWeightDa: 35.45 },
  { symbol: "Ar", logEpsilon: 6.4, atomicWeightDa: 39.948 },
  { symbol: "K", logEpsilon: 5.03, atomicWeightDa: 39.098 },
  { symbol: "Ca", logEpsilon: 6.34, atomicWeightDa: 40.078 },
  { symbol: "Sc", logEpsilon: 3.15, atomicWeightDa: 44.956 },
  { symbol: "Ti", logEpsilon: 4.95, atomicWeightDa: 47.867 },
  { symbol: "V", logEpsilon: 3.93, atomicWeightDa: 50.942 },
  { symbol: "Cr", logEpsilon: 5.64, atomicWeightDa: 51.996 },
  { symbol: "Mn", logEpsilon: 5.43, atomicWeightDa: 54.938 },
  { symbol: "Fe", logEpsilon: 7.5, atomicWeightDa: 55.845 },
  { symbol: "Co", logEpsilon: 4.99, atomicWeightDa: 58.933 },
  { symbol: "Ni", logEpsilon: 6.22, atomicWeightDa: 58.693 },
  { symbol: "Cu", logEpsilon: 4.19, atomicWeightDa: 63.546 },
  { symbol: "Zn", logEpsilon: 4.56, atomicWeightDa: 65.38 },
];

/** The roman numerals of the stages the balance takes: neutral, first and second ion. */
const STAGE_NUMERALS = ["I", "II", "III"] as const;

/** What the Saha balance reads: Barklem and Collet's tables 4, 7 and 8. */
export interface SahaData {
  readonly ionisationEnergiesEv: ReadonlyMap<string, readonly [number, number, number]>;
  readonly partitionFunctions: BarklemTable;
  readonly equilibriumConstants: BarklemTable;
}

/** The balance of a gas at one temperature and pressure: its number densities, m⁻³. */
export interface SahaBalance {
  readonly temperatureK: number;
  readonly pressurePa: number;
  /** The free electrons. */
  readonly electronsPerM3: number;
  /** Every H nucleus, in H, H⁺ and H₂. */
  readonly hydrogenNucleiPerM3: number;
  /** The atomic H, H⁺ and H₂. */
  readonly atomicHydrogenPerM3: number;
  readonly protonsPerM3: number;
  readonly molecularHydrogenPerM3: number;
  /** Each element's share in its neutral, first and second stages, summing to 1, by symbol. */
  readonly stageFractions: ReadonlyMap<string, readonly [number, number, number]>;
}

/** (2π m_e k T ÷ h²)^(3/2), m⁻³: Saha's electron term. */
function electronTermPerM3(temperatureK: number): number {
  return (
    ((2 * Math.PI * ELECTRON_MASS_KG * BOLTZMANN_J_PER_K * temperatureK) / PLANCK_J_S ** 2) ** 1.5
  );
}

/**
 * Saha's n_(i+1) n_e ÷ n_i for an element's stage i at a temperature, m⁻³:
 * 2 (U_(i+1) ÷ U_i) (2π m_e kT ÷ h²)^(3/2) e^(−χ_i ÷ kT), or 0 for a stage that does not exist.
 */
function sahaRatioPerM3(
  data: SahaData,
  symbol: string,
  stage: 0 | 1,
  temperatureK: number,
): number {
  const chiEv = data.ionisationEnergiesEv.get(symbol)?.[stage] ?? -1;
  const upper = `${symbol}_${STAGE_NUMERALS[stage + 1]}`;
  if (chiEv < 0 || !data.partitionFunctions.rows.has(upper)) {
    return 0;
  }
  const lowerQ = partitionFunctionAt(
    data.partitionFunctions,
    `${symbol}_${STAGE_NUMERALS[stage]}`,
    temperatureK,
  );
  const upperQ = partitionFunctionAt(data.partitionFunctions, upper, temperatureK);
  return (
    2 *
    (upperQ / lowerQ) *
    electronTermPerM3(temperatureK) *
    Math.exp((-chiEv * ELECTRONVOLT_J) / (BOLTZMANN_J_PER_K * temperatureK))
  );
}

/** Bisection on a decreasing function of ln x between two bounds, to 10⁻¹² in ln x. */
function bisectLog(lo: number, hi: number, f: (x: number) => number): number {
  let a = Math.log(lo);
  let b = Math.log(hi);
  for (let i = 0; i < 200 && b - a > 1e-12; i += 1) {
    const mid = 0.5 * (a + b);
    if (f(Math.exp(mid)) > 0) {
      a = mid;
    } else {
      b = mid;
    }
  }
  return Math.exp(0.5 * (a + b));
}

/**
 * An element's shares in its neutral, first and second stages from Saha's two ratios n_(i+1) n_e ÷
 * n_i, m⁻³, at an electron density, m⁻³.
 */
function stageShares(
  first: number,
  second: number,
  electrons: number,
): readonly [number, number, number] {
  const r1 = first / electrons;
  const r2 = second / electrons;
  const neutral = 1 / (1 + r1 + r1 * r2);
  return [neutral, r1 * neutral, r1 * r2 * neutral];
}

/**
 * A gas of {@link SOLAR_ABUNDANCES} at a temperature and total pressure in Saha equilibrium, every
 * atom free but H's, which forms H₂ by {@link hydrogenEquilibriumPa}: each element's stages (I, II
 * and III) by Saha's equation with Barklem and Collet's partition functions and ionisation energies,
 * the charge held over H⁺ and every element's ions, and the pressure P = n k T over every particle.
 * H⁻ is left out (about 3 × 10⁻⁴ of the electrons at 4,600 K and 0.1 bar).
 */
export function sahaBalance(data: SahaData, temperatureK: number, pressurePa: number): SahaBalance {
  const totalPerM3 = pressurePa / (BOLTZMANN_J_PER_K * temperatureK);
  const metals = SOLAR_ABUNDANCES.filter((e) => e.symbol !== "H").map((e) => ({
    symbol: e.symbol,
    abundance: 10 ** (e.logEpsilon - 12),
    first: sahaRatioPerM3(data, e.symbol, 0, temperatureK),
    second: sahaRatioPerM3(data, e.symbol, 1, temperatureK),
  }));
  const hydrogenRatio = sahaRatioPerM3(data, "H", 0, temperatureK);
  const molecularK =
    hydrogenEquilibriumPa(data.equilibriumConstants, temperatureK) /
    (BOLTZMANN_J_PER_K * temperatureK);
  // For a given n_e: the H nuclei that fill the pressure, then the charge's excess.
  const atHydrogen = (nuclei: number, electrons: number): readonly [number, number, number] => {
    const s = hydrogenRatio / electrons;
    const atoms = (2 * nuclei) / (1 + s + Math.sqrt((1 + s) ** 2 + (8 * nuclei) / molecularK));
    return [atoms, atoms * s, (atoms * atoms) / molecularK];
  };
  const metalShare = metals.reduce((sum, m) => sum + m.abundance, 0);
  const nucleiFor = (electrons: number): number =>
    bisectLog(1e-30 * totalPerM3, totalPerM3, (nuclei) => {
      const [atoms, protons, molecules] = atHydrogen(nuclei, electrons);
      return totalPerM3 - electrons - (atoms + protons + molecules + metalShare * nuclei);
    });
  const chargeExcess = (electrons: number): number => {
    const nuclei = nucleiFor(electrons);
    const [, protons] = atHydrogen(nuclei, electrons);
    let ions = protons;
    for (const m of metals) {
      const [, single, double] = stageShares(m.first, m.second, electrons);
      ions += m.abundance * nuclei * (single + 2 * double);
    }
    return ions - electrons;
  };
  const electronsPerM3 = bisectLog(1e-40 * totalPerM3, 0.5 * totalPerM3, chargeExcess);
  const hydrogenNucleiPerM3 = nucleiFor(electronsPerM3);
  const [atomicHydrogenPerM3, protonsPerM3, molecularHydrogenPerM3] = atHydrogen(
    hydrogenNucleiPerM3,
    electronsPerM3,
  );
  const stageFractions = new Map<string, readonly [number, number, number]>([
    [
      "H",
      [
        (atomicHydrogenPerM3 + 2 * molecularHydrogenPerM3) / hydrogenNucleiPerM3,
        protonsPerM3 / hydrogenNucleiPerM3,
        0,
      ],
    ],
    ...metals.map((m) => [m.symbol, stageShares(m.first, m.second, electronsPerM3)] as const),
  ]);
  return {
    temperatureK,
    pressurePa,
    electronsPerM3,
    hydrogenNucleiPerM3,
    atomicHydrogenPerM3,
    protonsPerM3,
    molecularHydrogenPerM3,
    stageFractions,
  };
}

/** The reference level's pressure, Pa: 0.1 bar (science-r08-unowned-absorbers §2.2). */
export const REFERENCE_PRESSURE_PA = 1e4;

/** The reference gravity, m s⁻²: 10 (science-r08-unowned-absorbers §2.2). */
export const REFERENCE_GRAVITY_M_S2 = 10;

/** The slant path's factor over the vertical column: 2, the mean slant path (§2.2). */
export const SLANT_FACTOR = 2;

/** The solar gas's mass per H nucleus, kg: Σ_X (N_X ÷ N_H) m_X. */
function massPerHydrogenNucleusKg(): number {
  return (
    ATOMIC_MASS_KG *
    SOLAR_ABUNDANCES.reduce((sum, e) => sum + 10 ** (e.logEpsilon - 12) * e.atomicWeightDa, 0)
  );
}

/**
 * An ionisation stage's reference column, m⁻² (science-r08-unowned-absorbers §2.2, ruled
 * 2026-10-19): the element's vertical column above {@link REFERENCE_PRESSURE_PA} at
 * {@link REFERENCE_GRAVITY_M_S2}, P ÷ (g m_H-nucleus) times its abundance, times the stage's Saha
 * fraction in the balance, times {@link SLANT_FACTOR}.
 *
 * @param stage - 0 for the neutral atom, 1 for its first ion.
 * @throws Error for an element the balance does not hold.
 */
export function referenceColumnPerM2(balance: SahaBalance, symbol: string, stage: 0 | 1): number {
  const element = SOLAR_ABUNDANCES.find((e) => e.symbol === symbol);
  // H I's stage is its free atoms: the balance's neutral share of H counts H₂'s nuclei too
  // (R08.T4.e, a fault for H alone, which R08.T4.h's stages never took).
  const fraction =
    symbol === "H" && stage === 0
      ? balance.atomicHydrogenPerM3 / balance.hydrogenNucleiPerM3
      : balance.stageFractions.get(symbol)?.[stage];
  if (element === undefined || fraction === undefined) {
    throw new Error(`the Saha balance holds no element ${symbol}`);
  }
  const hydrogenColumnPerM2 =
    balance.pressurePa / (REFERENCE_GRAVITY_M_S2 * massPerHydrogenNucleusKg());
  return SLANT_FACTOR * hydrogenColumnPerM2 * 10 ** (element.logEpsilon - 12) * fraction;
}

// --- Line strengths, widths and equivalent widths ---

/**
 * A line's strength per absorber of its stage at a temperature, cm (cm⁻¹ per absorber cm⁻²):
 * π r_e gf e^(−c₂E_low ÷ T) ÷ Q(T), stimulated emission neglected.
 */
export function lineStrengthCm(
  line: KuruczLine,
  temperatureK: number,
  partitionFunction: number,
): number {
  return (
    (Math.PI *
      ELECTRON_RADIUS_CM *
      10 ** line.logGf *
      Math.exp((-SECOND_RADIATION_CONSTANT_CM_K * line.lowerPerCm) / temperatureK)) /
    partitionFunction
  );
}

/** The microturbulence the equivalent widths take, m s⁻¹: 1 km s⁻¹ (§2.2's upper-bound method). */
export const MICROTURBULENCE_M_PER_S = 1_000;

/** The factor on Unsöld's van der Waals width: 2 (§2.2's upper-bound method). */
export const VAN_DER_WAALS_FACTOR = 2;

/**
 * The perturbers' static dipole polarisabilities, a₀³: H's 4.5 (exact), and H₂'s and He's from the
 * client's measured dispersions at λ → ∞ by Lorentz–Lorenz, 3 (n² − 1) ÷ 4πN (n² + 2), at their
 * reference states' densities (`view/atmosphere/rayleigh.ts`'s `GAS_DISPERSION`): H₂ 5.429 from
 * Peck and Huang 1977 (J. Opt. Soc. Am. 67, 1550; 10⁶ (n − 1) → 14,895.6 ÷ 180.7 + 4,903.7 ÷ 92 at
 * 0 °C and 1 atm, Z 1.000 624) and He 1.387 from Mansfield and Peck 1969 (J. Opt. Soc. Am. 59, 199;
 * 10⁵ (n − 1) → 1,470.091 ÷ 423.98, Z 1.000 532), each a long extrapolation of its fit, 0.3% above
 * the computed 5.414 and 1.383 (from memory). Γ₆ goes as α^(2/5), so they move it by 0.1%.
 */
export const PERTURBER_POLARISABILITIES_A03 = { H: 4.5, H2: 5.429, He: 1.387 } as const;

/** The perturbers' masses, Da. */
const PERTURBER_MASSES_DA = { H: 1.008, H2: 2.016, He: 4.0026 } as const;

/** The perturbers' number densities, m⁻³. */
export interface PerturbersPerM3 {
  readonly H: number;
  readonly H2: number;
  readonly He: number;
}

/**
 * The least I − E an Unsöld radius takes, eV: a level within 0.1 eV of the limit, or above it, is
 * taken at 0.1 eV, n*² = 136 Z², where the hydrogenic radius has long passed any the impact
 * approximation describes. The floor narrows Unsöld's width below its value at the level's true n*;
 * where {@link takesRydbergBound} applies, the Rydberg bound ({@link rydbergWidthM3PerS}) now sets
 * these levels' widths ({@link vanDerWaalsWidthPerS}).
 */
export const UNSOLD_BINDING_FLOOR_EV = 0.1;

/**
 * The perturbers' zero-energy electron scattering lengths L, a₀, for the Rydberg bound's Fermi term
 * (decision-r08-t4e-unsold §2): He's 1.19 (T. F. O'Malley, Phys. Rev. 130 (1963) 1020, its abstract
 * read; Omont's Table I and Lei et al. 2025's Table III give it from Massey and Burhop 1969); H₂'s
 * 1.61 (J. Randell et al., J. Phys. B 27 (1994) 2369, as Lei et al. 2025 take it), within Thompson
 * et al. 1987's 1.64 ± 0.07 from Rb Rydberg shifts (Phys. Rev. A 36, 2134, its abstract read) and
 * above Chang 1981's 1.27 (J. Phys. B 14, 893, as main's ruling cites it, not read); and H's
 * spin-weighted √((a_s² + 3a_t²) ÷ 4) = 3.353 of the singlet and triplet lengths 5.965 and 1.7686
 * (C. Schwartz, Phys. Rev. 124 (1961) 1468, not read; Osorio et al. 2015, Astron. Astrophys. 579,
 * A53, §3.3.2, give 5.965 and 1.769; the weights are Alekseev and Sobel'man's eqs. (20)–(22)).
 */
export const PERTURBER_SCATTERING_LENGTHS_A0 = {
  H: Math.sqrt((5.965 ** 2 + 3 * 1.768_6 ** 2) / 4),
  H2: 1.61,
  He: 1.19,
} as const;

/** The least effective principal quantum number the Rydberg bound takes: 6 (Omont 1977, §4.2). */
export const RYDBERG_BOUND_LEAST_N_STAR = 6;

/**
 * The least distance of a level's n* from an integer at which the Rydberg bound applies: 0.1. Nearer,
 * the level lies by the high-l manifold, whose l-mixing collisions, which the elastic model leaves
 * out, widen it: Na 8d–11d + He at 571 K is 2–3 times the bound (Lam, McFarlane and Steel 1984,
 * DTIC ADA146827, as main's table in decision-r08-t4e-unsold §1 gives it).
 */
export const RYDBERG_BOUND_LEAST_DEFECT = 0.1;

/** The atomic unit of speed, αc, m s⁻¹: e² ÷ 4πε₀ħ. */
const ATOMIC_SPEED_M_PER_S = COULOMB_E2_J_M / HBAR_J_S;

/**
 * A neutral atom's level's effective principal quantum number, unfloored: √(Ry ÷ (I − E)), or
 * `Infinity` at or above the limit.
 */
export function effectiveQuantumNumber(limitEv: number, levelEv: number): number {
  const binding = limitEv - levelEv;
  return binding > 0 ? Math.sqrt(RYDBERG_EV / binding) : Infinity;
}

/**
 * Whether a line takes the Rydberg bound (decision-r08-t4e-unsold §2): a neutral atom's line on
 * Unsöld's width (no Kurucz damping field; R08.T4.e's callers give it only lines with no ABO width
 * either, and ABO's tables stop below n* 6), whose upper level's unfloored n* is at least
 * {@link RYDBERG_BOUND_LEAST_N_STAR} and at least {@link RYDBERG_BOUND_LEAST_DEFECT} from an
 * integer, or which lies at or above the limit.
 *
 * @param hydrogenic - True for H I, whose every level lies on the hydrogenic manifold (no quantum
 *   defect), so that only an entry at or above its limit takes the bound: its n* from Barklem and
 *   Collet's limit, rounded to 0.1 meV, and the infinite-mass Rydberg would drift from the integer
 *   by over 0.1 from n 42.
 */
export function takesRydbergBound(
  line: KuruczLine,
  limitEv: number,
  coreCharge: number,
  hydrogenic = false,
): boolean {
  if (coreCharge !== 1 || line.logGammaVanDerWaalsPerHydrogen !== 0) {
    return false;
  }
  const nStar = effectiveQuantumNumber(limitEv, line.upperPerCm / PER_CM_PER_EV);
  return (
    nStar === Infinity ||
    (!hydrogenic &&
      nStar >= RYDBERG_BOUND_LEAST_N_STAR &&
      Math.abs(nStar - Math.round(nStar)) >= RYDBERG_BOUND_LEAST_DEFECT)
  );
}

/**
 * The Rydberg bound on a neutral atom's line width per perturber, the full width in angular
 * frequency per perturber density, m³ s⁻¹ (decision-r08-t4e-unsold §2): Γ_R ÷ N = 2 v̄ (σ_pol +
 * σ_F), in atomic units within, from the upper level's unfloored n* and the pair's mean speed v̄.
 *
 * @remarks
 * - **σ_pol = 3.59 (α_p ÷ v̄)^(2/3)**, the polarisation width's asymptote: V. A. Alekseev and I. I.
 *   Sobel'man, Sov. Phys. JETP 22 (1966) 882, eq. (34), a full width by their eq. (2), as A. Omont,
 *   J. Physique 38 (1977) 1343 (HAL jpa-00208705), eq. (4.3) gives it; both read in full by main.
 *   The coefficient 3.59, not Omont's (4.15) Anderson estimate of 3.18, reproduces his Table III
 *   (Lei et al. 2025's predicted widths take 3.18, and so run about 11% under the bound).
 * - **σ_F**, the Fermi scattering-length term over b₁ < b < 2n*² (Omont's (4.9), (4.11)–(4.17)):
 *   b₁ = (α_p ÷ v̄)^(1/3), b₂ = ½ (0.53 L_p ÷ (n*³ v̄))², and σ_F = 0 where 2n*² ≤ b₁; 2π b₂ (2n*²
 *   − b₁) where b₂ ≤ b₁ (4.17a); π (4 b₂ n*² − b₂² − b₁²) where b₁ < b₂ < 2n*² (4.17b); π (4n*⁴ −
 *   b₁²), the orbit's whole area, where b₂ ≥ 2n*², which (4.12)–(4.13) give there. At or above
 *   the limit, n* → ∞ leaves σ_pol alone.
 * - α_p is {@link PERTURBER_POLARISABILITIES_A03}'s and L_p {@link PERTURBER_SCATTERING_LENGTHS_A0}'s;
 *   the lower level is left out, as Omont's model does (its Unsöld C₆ in the phase would lower Γ_R
 *   35–50%).
 * - Against measurements (main's table, decision-r08-t4e-unsold §1) the bound is 0.6–1.6 times the
 *   low-pressure s- and p-series widths (Rb + He, Na + He, K + He) and 0.35–0.76 times the 1930s
 *   high-density data for He and H₂; it is `estimated`, good to about a factor of 2. From n* 6 to
 *   about 10 it is extrapolated: Omont §2.6 finds its potential "a good approximation for n > 10"
 *   that "might be used in the range 5 < n < 10", and Alekseev and Sobel'man's condition (38), ρ_i
 *   ≪ n*², is marginal there, ρ_i ≈ 10–17 a₀ for H, H₂ and He at 300–1,000 K. Where Omont's Fig. 2
 *   tests it below that (Cs + Ar, n 10–13, n* 6.4–9.4), the model meets or slightly exceeds the
 *   data.
 *
 * @param speedMPerS - The pair's mean relative speed √(8kT ÷ πμ), as Unsöld's width takes it.
 */
export function rydbergWidthM3PerS(
  nStar: number,
  perturber: keyof PerturbersPerM3,
  speedMPerS: number,
): number {
  const v = speedMPerS / ATOMIC_SPEED_M_PER_S;
  const alpha = PERTURBER_POLARISABILITIES_A03[perturber];
  const sigmaPol = 3.59 * (alpha / v) ** (2 / 3);
  let sigmaFermi = 0;
  if (Number.isFinite(nStar)) {
    const orbit = 2 * nStar * nStar;
    const b1 = (alpha / v) ** (1 / 3);
    const b2 = 0.5 * ((0.53 * PERTURBER_SCATTERING_LENGTHS_A0[perturber]) / (nStar ** 3 * v)) ** 2;
    if (orbit <= b1) {
      sigmaFermi = 0;
    } else if (b2 <= b1) {
      sigmaFermi = 2 * Math.PI * b2 * (orbit - b1);
    } else if (b2 < orbit) {
      sigmaFermi = Math.PI * (2 * b2 * orbit - b2 * b2 - b1 * b1);
    } else {
      sigmaFermi = Math.PI * (orbit * orbit - b1 * b1);
    }
  }
  return 2 * speedMPerS * (sigmaPol + sigmaFermi) * BOHR_RADIUS_M ** 2;
}

/** ⟨r²⟩ ÷ a₀² of a level E below the limit I of a stage of core charge Z, Unsöld's 2.5 n*⁴ ÷ Z². */
function unsoldRadiusSquared(limitEv: number, levelEv: number, coreCharge: number): number {
  const binding = Math.max(limitEv - levelEv, UNSOLD_BINDING_FLOOR_EV);
  const nStarSquared = (coreCharge * coreCharge * RYDBERG_EV) / binding;
  return (2.5 * nStarSquared * nStarSquared) / (coreCharge * coreCharge);
}

/**
 * A line's van der Waals damping constant Γ₆, s⁻¹ (the Lorentzian's full width in angular
 * frequency), by A. Unsöld's hydrogenic approximation (Physik der Sternatmosphären, 2nd ed., 1955,
 * not re-read): C₆ = (e² ÷ 4πε₀ħ) α_p (⟨r²⟩_u − ⟨r²⟩_l), ⟨r²⟩ = 2.5 n*⁴ a₀² ÷ Z², and the impact
 * theory's Γ₆ = 8.08 v̄^(3/5) C₆^(2/5) n_p summed over the perturbers H, H₂ and He, v̄ = √(8kT ÷ πμ)
 * (as D. F. Gray, The Observation and Analysis of Stellar Photospheres, 3rd ed., 2005, §11, gives
 * them; not re-read). Where a line takes the Rydberg bound ({@link takesRydbergBound}), each
 * perturber's width is the lesser of Unsöld's and the bound's ({@link rydbergWidthM3PerS}) before the
 * sum (decision-r08-t4e-unsold): Unsöld's C₆ is the R⁻⁶ term of a multipole expansion, valid only
 * for a perturber outside the orbit, and from n* 6 the perturber meets the ion core and the
 * electron apart, each width saturating.
 *
 * @param limitEv - The stage's ionisation energy.
 * @param coreCharge - 1 for a neutral atom, 2 for a first ion.
 * @param hydrogenic - True for H I ({@link takesRydbergBound}).
 */
export function vanDerWaalsWidthPerS(
  line: KuruczLine,
  limitEv: number,
  coreCharge: number,
  atomMassDa: number,
  temperatureK: number,
  perturbers: PerturbersPerM3,
  hydrogenic = false,
): number {
  const deltaR2 = Math.max(
    unsoldRadiusSquared(limitEv, line.upperPerCm / PER_CM_PER_EV, coreCharge) -
      unsoldRadiusSquared(limitEv, line.lowerPerCm / PER_CM_PER_EV, coreCharge),
    0,
  );
  const nStar = takesRydbergBound(line, limitEv, coreCharge, hydrogenic)
    ? effectiveQuantumNumber(limitEv, line.upperPerCm / PER_CM_PER_EV)
    : undefined;
  let width = 0;
  for (const p of ["H", "H2", "He"] as const) {
    const alphaM3 = PERTURBER_POLARISABILITIES_A03[p] * BOHR_RADIUS_M ** 3;
    const c6 = (COULOMB_E2_J_M / HBAR_J_S) * alphaM3 * deltaR2 * BOHR_RADIUS_M ** 2;
    const reducedKg =
      ((atomMassDa * PERTURBER_MASSES_DA[p]) / (atomMassDa + PERTURBER_MASSES_DA[p])) *
      ATOMIC_MASS_KG;
    const speed = Math.sqrt((8 * BOLTZMANN_J_PER_K * temperatureK) / (Math.PI * reducedKg));
    const unsold = 8.08 * speed ** 0.6 * c6 ** 0.4;
    const perPerturber =
      nStar === undefined ? unsold : Math.min(unsold, rydbergWidthM3PerS(nStar, p, speed));
    width += perPerturber * perturbers[p];
  }
  return width;
}

/**
 * The classical radiative damping constant, s⁻¹: γ_cl = 2 r_e ω² ÷ 3c, 0.2223 ÷ λ² with λ in cm
 * (the classical oscillator's, e.g. Gray 2005 §11).
 */
export function classicalDampingPerS(vacuumNm: number): number {
  const omega = (2 * Math.PI * SPEED_OF_LIGHT_M_PER_S) / (vacuumNm * 1e-9);
  return (2 * ELECTRON_RADIUS_CM * 1e-2 * omega * omega) / (3 * SPEED_OF_LIGHT_M_PER_S);
}

/** A line's Doppler half-width at half maximum with microturbulence, cm⁻¹: ν √(ln 2) √(2kT ÷ m + ξ²) ÷ c. */
export function dopplerHalfWidthWithTurbulencePerCm(
  wavenumberPerCm: number,
  temperatureK: number,
  atomMassDa: number,
): number {
  const b = Math.sqrt(
    (2 * BOLTZMANN_J_PER_K * temperatureK) / (atomMassDa * ATOMIC_MASS_KG) +
      MICROTURBULENCE_M_PER_S ** 2,
  );
  return (wavenumberPerCm * Math.sqrt(Math.LN2) * b) / SPEED_OF_LIGHT_M_PER_S;
}

/** The bake bins' edges, nm: 380 + 25.33 k, k = 0..15 (`medium.ts`'s bake bins). */
export const BAKE_BIN_EDGES_NM: ReadonlyArray<number> = Array.from(
  { length: 16 },
  (_, k) => 380 + (k * 380) / 15,
);

/** The share of a bin's light at which a stage's lines count: 10⁻³ (the third reason). */
export const BIN_SHARE_BOUND = 1e-3;

/** The window whose lines are summed, vacuum nm: 360–780 (ruled 2026-10-19), T4.e's. */
export const LINE_WINDOW_NM: readonly [number, number] = [360, 780];

/** Points a decade of the equivalent width's offset grid, either side of the line's centre. */
const OFFSET_POINTS_A_DECADE = 120;

/**
 * A line's equivalent width in each bake bin, nm: ∫ (1 − e^(−τ)) dλ over the part of its profile
 * inside each bin, τ = N S φ_V, φ_V the Voigt profile in wavenumber of the line's Doppler and
 * Lorentzian half-widths (`molecularLines.ts`'s {@link voigtK}). The integral runs over vacuum
 * wavenumber with dλ = 10⁷ dν ÷ ν² nm, by the trapezoid rule on offsets spaced
 * {@link OFFSET_POINTS_A_DECADE} a decade from a fiftieth of the Voigt half-width out to the far
 * bin edge, with the bins' edges among the points; the profile's convex Lorentzian tail makes the
 * rule err high.
 *
 * @param columnPerCm2 - The absorbers' column, cm⁻².
 * @param strengthCm - The line's strength per absorber ({@link lineStrengthCm}).
 */
export function lineBinWidthsNm(
  wavenumberPerCm: number,
  columnPerCm2: number,
  strengthCm: number,
  dopplerPerCm: number,
  lorentzPerCm: number,
): Float64Array {
  const widths = new Float64Array(BAKE_BIN_EDGES_NM.length - 1);
  const edges = BAKE_BIN_EDGES_NM.map((nm) => 1e7 / nm);
  const top = edges[0] ?? 0;
  const bottom = edges.at(-1) ?? 0;
  const amplitude = (columnPerCm2 * strengthCm * Math.sqrt(Math.LN2 / Math.PI)) / dopplerPerCm;
  const x = Math.sqrt(Math.LN2) / dopplerPerCm;
  const y = Math.sqrt(Math.LN2) * (lorentzPerCm / dopplerPerCm);
  const depth = (nu: number): number => {
    const tau = amplitude * voigtK(x * (nu - wavenumberPerCm), y);
    return (tau < 1e-8 ? tau : 1 - Math.exp(-tau)) * (1e7 / (nu * nu));
  };
  const halfWidth =
    0.5346 * lorentzPerCm + Math.sqrt(0.2166 * lorentzPerCm ** 2 + dopplerPerCm ** 2);
  const first = halfWidth / 50;
  // The points on each side: the centre, the offsets, and the bins' edges, within the bins.
  const points = new Set<number>(edges);
  if (wavenumberPerCm > bottom && wavenumberPerCm < top) {
    points.add(wavenumberPerCm);
  }
  const reach = Math.max(top - wavenumberPerCm, wavenumberPerCm - bottom);
  const steps = Math.ceil(Math.log10(Math.max(reach / first, 10)) * OFFSET_POINTS_A_DECADE);
  for (let i = 0; i <= steps; i += 1) {
    const offset = first * 10 ** (i / OFFSET_POINTS_A_DECADE);
    for (const nu of [wavenumberPerCm - offset, wavenumberPerCm + offset]) {
      if (nu > bottom && nu < top) {
        points.add(nu);
      }
    }
  }
  const grid = [...points].toSorted((a, b) => a - b);
  let bin = edges.length - 2;
  let previousNu = grid[0] ?? bottom;
  let previous = depth(previousNu);
  for (let i = 1; i < grid.length; i += 1) {
    const nu = grid[i] ?? previousNu;
    const value = depth(nu);
    // The bins run down in wavenumber from 380 nm; the segment lies in the bin above its lower end.
    while (bin > 0 && previousNu >= (edges[bin] ?? 0)) {
      bin -= 1;
    }
    widths[bin] = (widths[bin] ?? 0) + 0.5 * (value + previous) * (nu - previousNu);
    previousNu = nu;
    previous = value;
  }
  return widths;
}

/** An ionisation stage whose lines are summed: its registry key, element and stage. */
export interface AtomicStage {
  /** The registry key: `Ni` for the neutral atom, `Ni+` for its first ion. */
  readonly key: string;
  readonly symbol: string;
  /** 0 for the neutral atom, 1 for its first ion. */
  readonly stage: 0 | 1;
}

/** A stage's name in Barklem and Collet's tables: `Ni_II`. */
export function barklemName(stage: AtomicStage): string {
  return `${stage.symbol}_${STAGE_NUMERALS[stage.stage]}`;
}

/** A stage's ionisation limit, eV, from table 4. */
export function ionisationLimitEv(data: SahaData, stage: AtomicStage): number {
  const limit = data.ionisationEnergiesEv.get(stage.symbol)?.[stage.stage];
  if (limit === undefined || limit < 0) {
    throw new Error(`table 4 gives ${stage.key} no ionisation energy`);
  }
  return limit;
}

/** A stage's atomic weight, Da. */
function atomMassOf(stage: AtomicStage): number {
  const element = SOLAR_ABUNDANCES.find((e) => e.symbol === stage.symbol);
  if (element === undefined) {
    throw new Error(`no solar abundance for ${stage.key}`);
  }
  return element.atomicWeightDa;
}

/** One line's share of a bin's light, to name the lines a share comes from. */
export interface LineShare {
  readonly line: KuruczLine;
  readonly share: number;
}

/** A stage's lines' shares of each bake bin's light at one temperature, at its reference column. */
export interface StageShares {
  readonly temperatureK: number;
  /** The stage's reference column, m⁻² ({@link referenceColumnPerM2}). */
  readonly columnPerM2: number;
  /** Σ W ÷ 25.33 nm in each bin. */
  readonly binShares: Float64Array;
  /** Each bin's largest single lines, largest first (at most 5). */
  readonly leading: ReadonlyArray<ReadonlyArray<LineShare>>;
  /** The lines summed: those in {@link LINE_WINDOW_NM} from a lower level below the limit. */
  readonly linesSummed: number;
}

/** The lines of a list that a stage's sums take: in the window, from a lower level below its limit. */
export function linesInWindow(
  lines: ReadonlyArray<KuruczLine>,
  limitEv: number,
): ReadonlyArray<KuruczLine> {
  const limitPerCm = limitEv * PER_CM_PER_EV;
  return lines.filter(
    (l) =>
      l.vacuumNm >= LINE_WINDOW_NM[0] &&
      l.vacuumNm <= LINE_WINDOW_NM[1] &&
      l.lowerPerCm < limitPerCm,
  );
}

/**
 * A stage's lines' shares of each bake bin's light at a temperature: each line's equivalent widths
 * by bin ({@link lineBinWidthsNm}) at the stage's reference column in the balance at
 * {@link REFERENCE_PRESSURE_PA}, Doppler broadening with {@link MICROTURBULENCE_M_PER_S}, classical
 * damping and {@link VAN_DER_WAALS_FACTOR} times Unsöld's width with the balance's H, H₂ and He,
 * summed over the lines and divided by the bin's width.
 *
 * @param columnFactor - The column over the reference column: 1, or R08.T4.e's recorded 100 for a
 *   visibility floor's figure at a hundred times it (the widths stay the balance's).
 */
export function stageBinShares(
  data: SahaData,
  stage: AtomicStage,
  lines: ReadonlyArray<KuruczLine>,
  temperatureK: number,
  columnFactor = 1,
): StageShares {
  const balance = sahaBalance(data, temperatureK, REFERENCE_PRESSURE_PA);
  const columnPerM2 = columnFactor * referenceColumnPerM2(balance, stage.symbol, stage.stage);
  const limitEv = ionisationLimitEv(data, stage);
  const q = partitionFunctionAt(data.partitionFunctions, barklemName(stage), temperatureK);
  const atomMassDa = atomMassOf(stage);
  const heliumShare =
    10 ** ((SOLAR_ABUNDANCES.find((e) => e.symbol === "He")?.logEpsilon ?? 0) - 12);
  const perturbers: PerturbersPerM3 = {
    H: balance.atomicHydrogenPerM3,
    H2: balance.molecularHydrogenPerM3,
    He: heliumShare * balance.hydrogenNucleiPerM3,
  };
  const summed = linesInWindow(lines, limitEv);
  const binShares = new Float64Array(BAKE_BIN_EDGES_NM.length - 1);
  const leading: LineShare[][] = Array.from({ length: binShares.length }, () => []);
  const binWidthNm = (BAKE_BIN_EDGES_NM[1] ?? 0) - (BAKE_BIN_EDGES_NM[0] ?? 0);
  for (const line of summed) {
    const wavenumber = 1e7 / line.vacuumNm;
    const lorentzPerCm =
      (classicalDampingPerS(line.vacuumNm) +
        VAN_DER_WAALS_FACTOR *
          vanDerWaalsWidthPerS(
            line,
            limitEv,
            stage.stage + 1,
            atomMassDa,
            temperatureK,
            perturbers,
            stage.symbol === "H",
          )) /
      (4 * Math.PI * SPEED_OF_LIGHT_M_PER_S * 100);
    const widths = lineBinWidthsNm(
      wavenumber,
      columnPerM2 * 1e-4,
      lineStrengthCm(line, temperatureK, q),
      dopplerHalfWidthWithTurbulencePerCm(wavenumber, temperatureK, atomMassDa),
      lorentzPerCm,
    );
    for (const [k, w] of widths.entries()) {
      const share = w / binWidthNm;
      binShares[k] = (binShares[k] ?? 0) + share;
      const top = leading[k];
      if (top !== undefined && share > 0) {
        top.push({ line, share });
        top.sort((a, b) => b.share - a.share);
        top.splice(5);
      }
    }
  }
  return { temperatureK, columnPerM2, binShares, leading, linesSummed: summed.length };
}

/** The number of 1 nm cells over 380–760 nm: 381. */
export const CELL_COUNT = 381;

/** The cell of a vacuum wavelength, 0 to 380, or −1 outside [379.5, 760.5) nm. */
export function cellOf(vacuumNm: number): number {
  const c = Math.floor(vacuumNm - 379.5);
  return c >= 0 && c < CELL_COUNT ? c : -1;
}

/** A 1 nm cell's width in wavenumber, cm⁻¹: 10⁷ ÷ (λ − 0.5) − 10⁷ ÷ (λ + 0.5). */
export function cellWidthPerCm(cell: number): number {
  const centreNm = 380 + cell;
  return 1e7 / (centreNm - 0.5) - 1e7 / (centreNm + 0.5);
}

/** A stage's 1 nm cell means at a temperature, with the line leading each cell. */
export interface CellMeans {
  /** m² an absorber, cells centred on 380 + c nm. */
  readonly cellMeansM2: Float64Array;
  /** Each cell's largest line, or `undefined` for an empty cell. */
  readonly leading: ReadonlyArray<KuruczLine | undefined>;
  /** The lines in range from a lower level below the limit. */
  readonly linesInRange: number;
}

/**
 * A stage's 1 nm cell means at a temperature by R08.T4.c's method: each line with a lower level
 * below the ionisation limit adds its π r_e gf e^(−c₂E_low ÷ T) ÷ Q(T) ({@link lineStrengthCm}) to
 * the cell of its vacuum wavelength, over the cell's width in cm⁻¹; m² an absorber.
 */
export function lineCellMeansM2(
  lines: ReadonlyArray<KuruczLine>,
  temperatureK: number,
  partitionFunction: number,
  limitEv: number,
): CellMeans {
  const limitPerCm = limitEv * PER_CM_PER_EV;
  const sumsCm = new Float64Array(CELL_COUNT);
  const largestCm = new Float64Array(CELL_COUNT);
  const leading: Array<KuruczLine | undefined> = Array.from(
    { length: CELL_COUNT },
    () => undefined,
  );
  let linesInRange = 0;
  for (const line of lines) {
    const c = cellOf(line.vacuumNm);
    if (c < 0 || line.lowerPerCm >= limitPerCm) {
      continue;
    }
    linesInRange += 1;
    const s = lineStrengthCm(line, temperatureK, partitionFunction);
    sumsCm[c] = (sumsCm[c] ?? 0) + s;
    if (s > (largestCm[c] ?? 0)) {
      largestCm[c] = s;
      leading[c] = line;
    }
  }
  const cellMeansM2 = Float64Array.from(sumsCm, (s, c) => (s / cellWidthPerCm(c)) * 1e-4);
  return { cellMeansM2, leading, linesInRange };
}

/**
 * A value rounded up to three significant figures in decimal, so that it stays a bound: the least
 * three-figure decimal at or above it, as the nearest f64 (which is at or above it, since the value
 * is an f64 below the decimal). 0 stays 0.
 *
 * @throws Error for a negative or non-finite value.
 */
export function roundUpToThreeFigures(value: number): number {
  if (!(Number.isFinite(value) && value >= 0)) {
    throw new Error(`${value} is not a finite value at least 0`);
  }
  if (value === 0) {
    return 0;
  }
  const nearest = Number(value.toPrecision(3));
  if (nearest >= value) {
    return nearest;
  }
  const [mantissa = "", exponent = "0"] = value.toExponential(2).split("e");
  const digits = Math.round(Number(mantissa) * 100) + 1;
  return digits === 1_000
    ? Number(`1.00e${Number(exponent) + 1}`)
    : Number(`${digits / 100}e${exponent}`);
}
