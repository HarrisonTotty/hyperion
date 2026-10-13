/**
 * H₂O's visible continuum (plan R08, R08.T4.d; `decision-r08-t4d-kdist.md` §6): the
 * Clough–Kneizys–Davies (CKD) continuum as LOWTRAN 7 tabulates it, reduced to 1 nm cell means from
 * 567 nm, and below 567 nm CKD's ratio to HITRAN2024's lines' Lorentz wing and pedestal,
 * `estimated`. MT_CKD is not an input in any form: AER grants it "for scientific and research
 * purposes only".
 *
 * @remarks
 * LOWTRAN 6's report (F. X. Kneizys et al., AFGL-TR-83-0187, 1983, Eqs. 22–24) gives the form:
 * k_c = ρ ν tanh(hcν ÷ 2kT) [(ρ_s ÷ ρ₀) C_s(ν, T) + (ρ_f ÷ ρ₀) C_f(ν, T)], ρ₀ the density at one
 * atmosphere and 296 K, "originally developed by Clough et al. for use with the line-by-line … code,
 * FASCODE". LOWTRAN 7's `lowtran7.f` holds C_s at 296 and 260 K and C_f at 296 K (`BLOCK DATA
 * SF296`, `SF260` and `BFH2O`, "06/28/82") every 10 cm⁻¹ over −20 to 20,000 cm⁻¹, in units of
 * 10⁻²⁰ cm³ a molecule, and forms the amounts ρ_w² ÷ ρ₀ and ρ_w ρ_f ÷ ρ₀ in amagats against
 * ρ₀ = 273.15 ÷ 296 amagat (its `RHZERO`). So the pairs' σ, per n_w n_partner, is
 * ν tanh(hcν ÷ 2kT) C × 10⁻²⁰ cm² ÷ n₀, n₀ the number density at 101,325 Pa and 296 K.
 * LOWTRAN's own corrections are not taken, so that the tables are CKD's, as RRTMG_SW's band means
 * confirm: its self factor near 1,050 cm⁻¹ (`XH2O`, ≤ 7.1 × 10⁻⁵ over 567–800 nm), and its
 * `FUDGE` far-wing addition to the foreign part (≤ 9.8 × 10⁻²⁹ cm² a molecule at the reference
 * density, 7.8 × 10⁻⁴ of the foreign part's 567–760 nm integral, though above CKD's own value in
 * its windows). LOWTRAN interpolates the self part linearly within 260–296 K and holds it beyond;
 * the power law taken here (`decision-r08-t4d-kdist.md` §6.3) differs by at most 3.6% in the bands'
 * cells and 7.3% in weak ones. The extension's 1,000 cm⁻¹ reach from 380–395 nm passes the fetched
 * lines' 26,400 cm⁻¹, so those cells are slightly low, 8 × 10⁻³¹ cm² at 380 nm before the ratio.
 */

import type { FetchedSource, ReducedPairCrossSections } from "./atmosphereData";
import type { LineList } from "./molecularLines";

/**
 * LOWTRAN 7's source (`decision-r08-t4d-kdist.md` §6.3 and §6.8): `lowtran7.f` from
 * space-physics/lowtran at commit 5b8d3871, whose MIT licence covers its wrapper only. The program
 * is the Air Force Geophysics Laboratory's, its reports "Approved for public release; distribution
 * unlimited", and its code states no terms.
 */
export const LOWTRAN_SOURCE: FetchedSource = {
  citation:
    "Clough-Kneizys-Davies water vapour continuum as tabulated in LOWTRAN 7 (F. X. Kneizys et al., " +
    "Users Guide to LOWTRAN 7, AFGL-TR-88-0177, Air Force Geophysics Laboratory, 1988; formulation " +
    "in F. X. Kneizys et al., AFGL-TR-83-0187, 1983, Eqs. 22-24; S. A. Clough et al., in Atmospheric " +
    "Water Vapor, Academic Press, 1980), BLOCK DATA SF296, SF260 and BFH2O of lowtran7.f " +
    "(https://github.com/space-physics/lowtran, commit 5b8d38714b204eaa12595f5f15e2babec1dbdfb5); " +
    "below 567 nm, its 567-600 nm ratio to the HITRAN2024 lines' Lorentz wing and pedestal (I. E. " +
    "Gordon et al., The HITRAN2024 molecular spectroscopic database, J. Quant. Spectrosc. Radiat. " +
    "Transfer 353 (2026) 109807, DOI 10.1016/j.jqsrt.2026.109807)",
  url: "https://raw.githubusercontent.com/space-physics/lowtran/5b8d38714b204eaa12595f5f15e2babec1dbdfb5/src/lowtran/fortran/lowtran7.f",
  sha256: "25e83d94e24bb8acc3242dfd3b97e0bd8ca8ffceff9ec6dce1f5a37c9c5459ac",
  fetched: "2026-10-12",
  terms:
    "LOWTRAN is a US Air Force Geophysics Laboratory program whose reports are approved for public " +
    "release, distribution unlimited; its code states no terms: reduced values with their citation, " +
    "the raw file not committed (decision-r08-t4d-kdist.md section 6). MT_CKD is not used.",
};

/** LOWTRAN 7's continuum tables: C on −20 to 20,000 cm⁻¹ every 10 cm⁻¹, cm³ a molecule. */
export interface LowtranContinuum {
  readonly firstPerCm: number;
  readonly stepPerCm: number;
  /** C_s at 296 K and 260 K, and C_f at 296 K, cm³ a molecule (the file's values × 10⁻²⁰). */
  readonly self296Cm3: Float64Array;
  readonly self260Cm3: Float64Array;
  readonly foreign296Cm3: Float64Array;
}

/** The points each table holds (its `NPT`). */
const LOWTRAN_POINTS = 2_003;

/** The numbers of a Fortran `DATA` statement after its first `/`. */
function dataNumbers(statement: string): number[] {
  return [
    ...statement.slice(statement.indexOf("/") + 1).matchAll(/[-+]?\d*\.?\d+(?:[Ee][-+]?\d+)?/gu),
  ].map((m) => Number(m[0]));
}

/**
 * One of `lowtran7.f`'s continuum `BLOCK DATA` units: its `V1, V2, DV, NPT` (−20, 20,000, 10 and
 * 2,003) and its values in order, × 10⁻²⁰.
 *
 * @throws Error if the unit is missing, its grid is not −20 to 20,000 every 10, or it holds other
 *   than 2,003 values.
 */
function lowtranBlock(text: string, name: string): Float64Array {
  const header = `BLOCK DATA ${name}\n`;
  const at = text.indexOf(header);
  const start = at + header.length;
  const end = text.indexOf("\n      END", start);
  if (at < 0 || end < 0) {
    throw new Error(`lowtran7.f holds no BLOCK DATA ${name}`);
  }
  // Fortran's fixed form: a C in column 1 is a comment; a character in column 6 continues a line.
  const statements: string[] = [];
  for (const row of text.slice(start, end).split("\n")) {
    if (/^[Cc*]/u.test(row) || row.trim().length === 0) {
      continue;
    }
    const continued = row.length > 5 && row[5] !== " " && row.slice(0, 5).trim().length === 0;
    if (continued && statements.length > 0) {
      statements[statements.length - 1] += ` ${row.slice(6)}`;
    } else {
      statements.push(row.slice(6));
    }
  }
  const data = statements.filter((s) => s.trim().startsWith("DATA "));
  const [grid, ...rest] = data;
  const [v1, v2, dv, npt] = grid === undefined ? [] : dataNumbers(grid);
  if (v1 !== -20 || v2 !== 20_000 || dv !== 10 || npt !== LOWTRAN_POINTS) {
    throw new Error(`lowtran7.f's ${name} is not on -20 to 20000 cm-1 every 10 (2003 points)`);
  }
  const values = rest.flatMap(dataNumbers);
  if (values.length !== LOWTRAN_POINTS) {
    throw new Error(`lowtran7.f's ${name} holds ${values.length} values, not ${LOWTRAN_POINTS}`);
  }
  return Float64Array.from(values, (v) => v * 1e-20);
}

/** `lowtran7.f`'s three water continuum tables ({@link LowtranContinuum}). */
export function parseLowtranContinuum(text: string): LowtranContinuum {
  const unix = text.replaceAll("\r\n", "\n");
  return {
    firstPerCm: -20,
    stepPerCm: 10,
    self296Cm3: lowtranBlock(unix, "SF296"),
    self260Cm3: lowtranBlock(unix, "SF260"),
    foreign296Cm3: lowtranBlock(unix, "BFH2O"),
  };
}

/** The second radiation constant hc ÷ k, cm K (CODATA 2018). */
const C2_CM_K = 1.438_776_877;

/** CKD's radiation term ν tanh(hcν ÷ 2kT), cm⁻¹. */
export function radiationTerm(wavenumberPerCm: number, temperatureK: number): number {
  return wavenumberPerCm * Math.tanh((C2_CM_K * wavenumberPerCm) / (2 * temperatureK));
}

/** The number density at 101,325 Pa and 296 K, cm⁻³: LOWTRAN's ρ₀, 273.15 ÷ 296 amagat. */
export const CKD_REFERENCE_PER_CM3 = 101_325 / (1.380_649e-23 * 296) / 1e6;

/** The vacuum wavenumber range of a 1 nm cell [λ − 0.5, λ + 0.5) nm, cm⁻¹. */
function cellRangePerCm(cellNm: number): readonly [number, number] {
  return [1e7 / (cellNm + 0.5), 1e7 / (cellNm - 0.5)];
}

/**
 * A cell's mean of ν tanh(hcν ÷ 2kT) C(ν) over its wavenumbers, cm² a molecule at the reference
 * density, C linear in ν between the table's points: the mean over a fine grid of 400 steps.
 */
export function ckdCellMean(
  table: LowtranContinuum,
  values: Float64Array,
  cellNm: number,
  temperatureK: number,
): number {
  const [low, high] = cellRangePerCm(cellNm);
  const steps = 400;
  let sum = 0;
  for (let k = 0; k < steps; k += 1) {
    const nu = low + ((k + 0.5) * (high - low)) / steps;
    const x = (nu - table.firstPerCm) / table.stepPerCm;
    const i = Math.floor(x);
    const a = values[i] ?? 0;
    const b = values[i + 1] ?? 0;
    sum += radiationTerm(nu, temperatureK) * (a + (b - a) * (x - i));
  }
  return sum / steps;
}

/** Where CKD's line list ends and its tables are taken from, nm (§6.3). */
export const CKD_FROM_NM = 567;

/** The cells CKD's ratio to the lines is taken over, nm, inclusive (§6.3). */
export const RATIO_CELLS_NM: readonly [number, number] = [567, 600];

/** The lines' reach for the extension, cm⁻¹ from the cell (§6.3). */
export const EXTENSION_REACH_PER_CM = 1_000;

/** The pedestal's distance, cm⁻¹: the lines' 25 cm⁻¹ cut-off. */
const PEDESTAL_PER_CM = 25;

/**
 * A cell's mean over its wavenumbers of the lines' Lorentz wing beyond 25 cm⁻¹ plus their pedestal,
 * cm² a molecule at the reference density and 296 K: Σ S L_γ(max(|ν − ν₀|, 25)), L_γ the Lorentz
 * profile of HITRAN's 296 K half-width at one atmosphere, over the lines within
 * {@link EXTENSION_REACH_PER_CM} of the cell, in closed form.
 *
 * @param widths - Each line's half-width: γ_air for the foreign part, γ_self for the self part.
 */
export function lineWingCellMean(lines: LineList, widths: Float64Array, cellNm: number): number {
  const [low, high] = cellRangePerCm(cellNm);
  let sum = 0;
  for (let i = 0; i < lines.wavenumbersPerCm.length; i += 1) {
    const centre = lines.wavenumbersPerCm[i] ?? Number.NaN;
    if (centre < low - EXTENSION_REACH_PER_CM || centre > high + EXTENSION_REACH_PER_CM) {
      continue;
    }
    const gamma = widths[i] ?? Number.NaN;
    const s = lines.intensitiesCm[i] ?? 0;
    if (!(gamma > 0 && s > 0)) {
      continue;
    }
    const pedestal = gamma / Math.PI / (PEDESTAL_PER_CM * PEDESTAL_PER_CM + gamma * gamma);
    // ∫₀^x of the profile with its pedestal, for x ≥ 0; odd in x.
    const half = (x: number): number =>
      x <= PEDESTAL_PER_CM
        ? x * pedestal
        : PEDESTAL_PER_CM * pedestal +
          (Math.atan(x / gamma) - Math.atan(PEDESTAL_PER_CM / gamma)) / Math.PI;
    const integral = (x: number): number => Math.sign(x) * half(Math.abs(x));
    sum += (s * (integral(high - centre) - integral(low - centre))) / (high - low);
  }
  return sum;
}

/** The continuum's two pairs and the figures its reduction records. */
export interface WaterContinuum {
  readonly self: ReducedPairCrossSections;
  readonly foreign: ReducedPairCrossSections;
  /** CKD's ratio to the lines over {@link RATIO_CELLS_NM}: foreign and self. */
  readonly foreignRatio: number;
  readonly selfRatio: number;
  /** The self part's 260 K ÷ 296 K over the same cells, the extension's temperature law. */
  readonly selfColdRatio: number;
}

/** The temperatures over which the continuum's law is applied, K: H₂O's 150–500 K. */
export const CONTINUUM_LAW_RANGE_K: readonly [number, number] = [150, 500];

/** A number to six significant figures, as the cross-sections files write them. */
function six(value: number): number {
  return value === 0 ? 0 : Number(value.toPrecision(6));
}

/** Σ f(nm) over whole nm from `from` to `to`. */
function sumOverCells(from: number, to: number, f: (nm: number) => number): number {
  let total = 0;
  for (let nm = from; nm <= to; nm += 1) {
    total += f(nm);
  }
  return total;
}

/**
 * The continuum's pairs over 380–800 nm at 1 nm (`decision-r08-t4d-kdist.md` §6.3 and §6.7):
 * `H2O+H2O`, CKD's self part at 260 and 296 K, a power law in T; `H2O+air`, its foreign part at
 * 296 K, held, with every other gas as the partner, each weighted by H₂O's broadening ratio for it.
 * From {@link CKD_FROM_NM} the cells are CKD's tables'; below, CKD's ratio to the lines' wing and
 * pedestal ({@link lineWingCellMean}) over {@link RATIO_CELLS_NM}, the self part's temperature law
 * that of the same cells. σ is per molecule pair, m⁵: the cell mean at the reference density ÷ n₀.
 *
 * @param partners - H₂O's broadening ratio for each foreign gas HITRAN gives (N₂ and O₂ take 1).
 */
export function reduceWaterContinuum(
  table: LowtranContinuum,
  lines: LineList,
  partners: Readonly<Record<string, number>>,
): WaterContinuum {
  const [r0, r1] = RATIO_CELLS_NM;
  const foreignRatio =
    sumOverCells(r0, r1, (nm) => ckdCellMean(table, table.foreign296Cm3, nm, 296)) /
    sumOverCells(r0, r1, (nm) => lineWingCellMean(lines, lines.airHalfWidthsPerCmAtm, nm));
  const selfRatio =
    sumOverCells(r0, r1, (nm) => ckdCellMean(table, table.self296Cm3, nm, 296)) /
    sumOverCells(r0, r1, (nm) => lineWingCellMean(lines, lines.selfHalfWidthsPerCmAtm, nm));
  const selfColdRatio =
    sumOverCells(r0, r1, (nm) => ckdCellMean(table, table.self260Cm3, nm, 260)) /
    sumOverCells(r0, r1, (nm) => ckdCellMean(table, table.self296Cm3, nm, 296));
  const toPair = (cm2: number): number => six((cm2 / CKD_REFERENCE_PER_CM3) * 1e-10);
  const selfRows: number[][] = [];
  const foreignRows: number[][] = [];
  for (let nm = 380; nm <= 800; nm += 1) {
    if (nm >= CKD_FROM_NM) {
      selfRows.push([
        nm,
        toPair(ckdCellMean(table, table.self260Cm3, nm, 260)),
        toPair(ckdCellMean(table, table.self296Cm3, nm, 296)),
      ]);
      foreignRows.push([nm, toPair(ckdCellMean(table, table.foreign296Cm3, nm, 296))]);
    } else {
      const self296Cm3 = selfRatio * lineWingCellMean(lines, lines.selfHalfWidthsPerCmAtm, nm);
      selfRows.push([nm, toPair(self296Cm3 * selfColdRatio), toPair(self296Cm3)]);
      foreignRows.push([
        nm,
        toPair(foreignRatio * lineWingCellMean(lines, lines.airHalfWidthsPerCmAtm, nm)),
      ]);
    }
  }
  const common =
    `Each 1 nm cell, [nm - 0.5, nm + 0.5) on vacuum wavelengths, the mean over its wavenumbers of ` +
    `nu tanh(hc nu/2kT) C(nu) x 1e-20 cm2, C linear between the tables' 10 cm-1 points, divided by ` +
    `n0 = 101325 Pa/(k 296 K) (LOWTRAN's rho0, 273.15/296 amagat) and converted from cm5 to m5, ` +
    `from ${CKD_FROM_NM} nm, where CKD's 1982 line list ends and its tables meet the extension. ` +
    `Below ${CKD_FROM_NM} nm, R times the HITRAN2024 lines' Lorentz wing beyond 25 cm-1 plus their ` +
    `pedestal (each line's Lorentz value at 25 cm-1 within it), summed over the lines within 1000 ` +
    `cm-1 of the cell at 296 K, R being CKD's sum over its ${r0}-${r1} nm cells over the lines' ` +
    `(decision-r08-t4d-kdist.md section 6.3), estimated. LOWTRAN's 1050 cm-1 self factor and its ` +
    `FUDGE foreign addition are not taken. Written to 6 significant figures.`;
  const self: ReducedPairCrossSections = {
    pair: "H2O+H2O",
    members: ["H2O", "H2O"],
    source: LOWTRAN_SOURCE,
    reduction:
      `${common} The self part (C_s at 260 and 296 K; R = ${selfRatio.toPrecision(4)} with ` +
      `gamma_self) is a power law in T through its 260 and 296 K values in each cell, below ` +
      `${CKD_FROM_NM} nm that of CKD's ${r0}-${r1} nm cells (260 K / 296 K = ` +
      `${selfColdRatio.toPrecision(4)}); measured over 260-296 K from ${CKD_FROM_NM} nm, the law ` +
      `extended over 150-500 K and held beyond, estimated (LOWTRAN interpolates linearly within ` +
      `260-296 K and holds beyond; the power law differs by at most 3.6% in the bands' cells).`,
    firstNm: 380,
    stepNm: 1,
    temperaturesK: [260, 296],
    temperatureLaw: "lnLnLinear",
    measuredRangeK: [260, 296],
    lawRangeK: CONTINUUM_LAW_RANGE_K,
    estimatedBelowNm: CKD_FROM_NM,
    rows: selfRows,
  };
  const foreign: ReducedPairCrossSections = {
    pair: "H2O+air",
    members: ["H2O", "air"],
    source: LOWTRAN_SOURCE,
    reduction:
      `${common} The foreign part (C_f at 296 K; R = ${foreignRatio.toPrecision(4)} with ` +
      `gamma_air) is held at 296 K at every temperature, as CKD prescribes; measured with N2 and O2 ` +
      `over 260-296 K from ${CKD_FROM_NM} nm. Its partner "air" is every gas but H2O, each scaled ` +
      `by H2O's broadening ratio for it (partners; 1 for a gas HITRAN gives no width for), ` +
      `measured as air with N2 and O2 and with other gases together up to 2% of the partner ` +
      `molecules (decision-r08-t4d-kdist-chain.md section 15), estimated beyond.`,
    firstNm: 380,
    stepNm: 1,
    temperaturesK: [296],
    temperatureLaw: "constant",
    measuredRangeK: [260, 296],
    lawRangeK: CONTINUUM_LAW_RANGE_K,
    estimatedBelowNm: CKD_FROM_NM,
    partners: { N2: 1, O2: 1, ...partners },
    measuredPartners: ["N2", "O2"],
    rows: foreignRows,
  };
  return { self, foreign, foreignRatio, selfRatio, selfColdRatio };
}
