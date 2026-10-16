/**
 * `node scripts/materials.mjs`: reduces the aerosol materials' refractive indices to the files of
 * `view/atmosphere/materials/` (plan R08, R08.T5.b and R08.T5.d; decision-r08-licences.md rows 2–5,
 * decision-composition §1.9 and §5, decision-r08-t5d).
 *
 * @remarks
 * Each fetched source is read from a path given on the command line, checked against the SHA-256
 * recorded here, and never committed. Only its values reduced onto the file's grid are written
 * ({@link MATERIAL_GRID_NM}, or {@link ACID_GRID_NM} for the two files Venus's ultraviolet fit
 * reads): the real index n interpolated linearly in wavelength between the source's points, and the
 * imaginary index k linearly in log k (linearly where either end is 0), with each file's own
 * exceptions stated in its `reduction`. A dispersion formula is evaluated on the grid, and a
 * uniaxial crystal's two axes are averaged in the dielectric function ({@link COMBINED_MATERIALS}).
 * The files made from no fetched table ({@link DERIVED_MATERIALS}): CO₂ ice, a derived fit
 * (`science-r08-sulphur-co2ice.md`); NH₄SH, a stated stand-in (row 5) whose index is a
 * Lorentz–Lorenz estimate (`science-r08-nonspherical.md` §3.1, {@link nh4shIndex}); since R08.T5.d
 * S₈'s constant index and the molecular ices' Lorentz–Lorenz estimates ({@link ICE_ESTIMATES}). The
 * files are JSON so that the client imports them and `hyperion-fit` reads them as one source of
 * truth (P14.T49.e). Prettier formats them after they are written
 * (`apps/hyperion/scripts/materials.mjs`).
 *
 * Inputs, each downloaded into its own directory and read as data only (T5.b's on 2026-10-09,
 * T5.d's on {@link FETCHED_T5D}):
 *
 * - refractiveindex.info's database (M. N. Polyanskiy, Sci. Data 11, 94 (2024); CC0 1.0), commit
 *   {@link RI_COMMIT}, files under `database/data/`;
 * - optool's `lnk_data/` (C. Dominik, M. Min and R. Tazaki 2021, ascl:2104.010), commit
 *   {@link OPTOOL_COMMIT}; its MIT licence covers its software, and its data files state no terms;
 * - ARIA's Palmer and Williams files and Pollack, Toon and Khare's rocks
 *   (https://eodg.atm.ox.ac.uk/ARIA/; no terms stated);
 * - NASA Ames's Mars dust file as kept in S. Ranjan's repository at commit 1e67a481 (no terms
 *   stated for the data);
 * - HITRAN2024's aerosol archive, `hitran_ri.tar` (https://hitran.org/aerosols/; "free", no
 *   licence stated), SHA-256 {@link HITRAN_TAR_SHA256}, from which only the named ASCII files were
 *   extracted (none of its code was run);
 * - Kitzmann and Heng 2018's compilation, LX-MIE's `compilation/` at commit {@link LX_MIE_COMMIT}
 *   (a GPL-3.0 repository whose data carry no separate terms), as a finding aid and, for FeS,
 *   CaTiO₃, ZnS, MnS and Na₂S, the only machine-readable copy of each primary;
 * - B. T. Draine's graphite dielectric functions (his site; citation asked);
 * - the arXiv e-print sources of Braude et al. 2020 and Irwin et al. 2022 (CC BY 4.0), read for one
 *   table each, and Martikainen et al. 2023's machine-readable Table 6 (CC BY 4.0);
 * - the Handbook of Mineralogy's sulphur sheet, for S₈'s three principal indices, and the
 *   Crystallography Open Database's cells (CC0), for the ices' densities;
 * - since R08.T5.g, omlc.org's table of Pope and Fry 1997, and since R08.T5.h R. Röttgers's WOPP v3
 *   compilation, for Mason, Cone and Fry 2016's table ({@link FETCHED_T5H}); the two water files
 *   take k from both, and IAPWS R9-97's n and Kou et al.'s k are constants here
 *   (decision-r08-t5g);
 * - since R08.T5.k ({@link FETCHED_T5K}), refractiveindex.info's copies of Sinnock and Smith 1969's
 *   liquid Ar, Kr and Xe tables, and HITRAN2024's O₂–O₂ collision-induced absorption file, for
 *   Thalman and Volkamer 2013's cross-sections (decision-p14-t49h-gas-index). The other T5.k
 *   liquids are scaled from their gas rows to measured values and densities cited here
 *   ({@link scaledLiquidIndex}).
 */

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import { type CiaSet, OXYGEN_PAIR_SOURCE, parseCia } from "./atmosphereData";

/**
 * The grid material files are written on by default, nm: 380 to 780 in steps of 5. The two files
 * Venus's ultraviolet fit reads start at 360 nm ({@link ACID_GRID_NM}).
 */
export const MATERIAL_GRID_NM: ReadonlyArray<number> = Array.from(
  { length: 81 },
  (_, i) => 380 + 5 * i,
);

/** The refractiveindex.info database commit the CC0 sources were fetched at. */
export const RI_COMMIT = "c5c2f188e848453def5970e347399d653df2ffc2";

/** The optool commit whose `lnk_data/` the ammonia-ice and enstatite-glass sources come from. */
export const OPTOOL_COMMIT = "0b1fa6df95e3a694dc64abdbf8e36dad127165ab";

/** The SHA-256 of HITRAN2024's `hitran_ri.tar` (207,422,976 bytes), recorded at the first fetch. */
export const HITRAN_TAR_SHA256 = "c22643a856ec98b3371c70e64351b19a9f3ea51b38ce369087e17e7e4ecc231e";

/** The day every source was fetched. */
export const FETCHED = "2026-10-09";

/** One row of a source table: the wavelength, µm, and the complex index n + ik (k may be NaN). */
export interface IndexSample {
  readonly wavelengthUm: number;
  readonly n: number;
  readonly k: number;
}

/** The phase a material file describes. */
export type FilePhase = "liquid" | "solid";

/** The shape class of a material's particles (Design note 6; P14.T24.c's closed physics enum). */
export type FileShape = "sphere" | "nonSphericalMineral" | "crystal" | "aggregate";

/** How a file's values were obtained (decision-composition §1.9). */
export type FileProvenance = "measured" | "derived" | "standIn";

/** The header of a material file: every field but the three arrays. */
export interface MaterialHeader {
  /** The sim's substance key (P14.T49.b). */
  readonly key: string;
  readonly phase: FilePhase;
  /** A composition variant (an acid's concentration), or null for the key's only one. */
  readonly variant: string | null;
  /** What the sample is, in words. */
  readonly name: string;
  /** The sample's temperature, K, or null where the source does not state one. */
  readonly temperatureK: number | null;
  readonly shape: FileShape;
  readonly provenance: FileProvenance;
  /** The paper the values come from. */
  readonly paper: string;
  /** Where the values were read, with the URL, the checksum and the date fetched. */
  readonly source: string;
  /** The licence basis (decision-composition §5). */
  readonly licence: string;
  /** How the values were brought onto the grid, and every exception. */
  readonly reduction: string;
  /** For a stand-in, what stands in and why; otherwise null. */
  readonly standIn: string | null;
  /** For a derived file, the parameterisation its values are computed from; otherwise null. */
  readonly fit: DerivedFit | null;
}

/**
 * A derived index: n = a + b ÷ λ² (λ in µm) and k = αλ ÷ 4π, over the wavelengths it was fitted
 * on.
 */
export interface CauchyFit {
  readonly form: "cauchy";
  readonly a: number;
  readonly bUm2: number;
  /** α, m⁻¹. */
  readonly absorptionPerM: number;
  /** The range of the data the fit was made on, µm. */
  readonly fittedFromUm: number;
  readonly fittedToUm: number;
}

/**
 * A derived real index from its gas's molar refraction scaled to the liquid (R08.T5.k;
 * decision-p14-t49h-gas-index A.1): at each wavelength x = c R(λ) ρ_m and n = √((1 + 2x) ÷ (1 − x)),
 * with R the Lorentz–Lorenz molar refraction of the gas row of the file's key in
 * `view/atmosphere/rayleigh.ts` (its `GAS_DISPERSION` row, or for SO₂ the dispersion of its
 * `ESTIMATED_RAYLEIGH` row), at the row's own reference state and Z ({@link scaledLiquidIndex}).
 *
 * @remarks
 * The form describes n alone: the file's k is its arrays, with k's derivation in its `reduction`.
 */
export interface ScaledMolarRefractionFit {
  readonly form: "scaledMolarRefraction";
  /** ρ_m, the NIST WebBook's saturated-liquid molar density at the file's `temperatureK`, mol/l. */
  readonly molarDensityMolPerL: number;
  /** c, the file's one factor on the gas's molar refraction. */
  readonly factor: number;
  /** The shortest wavelength of the measurements that set c, µm. */
  readonly anchoredFromUm: number;
  /** The longest wavelength of the measurements that set c, µm. */
  readonly anchoredToUm: number;
}

/** A derived file's parameterisation, discriminated by `form`. */
export type DerivedFit = CauchyFit | ScaledMolarRefractionFit;

/** A material file as written: its header and the index on {@link MATERIAL_GRID_NM}. */
export interface MaterialFile extends MaterialHeader {
  readonly wavelengthsNm: ReadonlyArray<number>;
  readonly n: ReadonlyArray<number>;
  readonly k: ReadonlyArray<number>;
}

const NUMBER = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/u;

function numeric(field: string): number {
  if (field === "NaN") {
    return Number.NaN;
  }
  if (!NUMBER.test(field)) {
    throw new Error(`"${field}" is not a number`);
  }
  return Number(field);
}

/**
 * The rows a reduction reads, by wavelength: from the last at or below the grid's first wavelength
 * to the first at or above its last, so that a source's rows elsewhere (an infrared band printed
 * with rounded wavelengths) cannot stop it. Exact duplicate rows are dropped, and two values at one
 * wavelength inside the window are refused.
 *
 * @param grid - The grid the rows are read for, nm, ascending.
 */
function usedRows(
  samples: ReadonlyArray<IndexSample>,
  ignoreBelowUm: number,
  grid: ReadonlyArray<number> = MATERIAL_GRID_NM,
): IndexSample[] {
  const sorted = samples
    .filter((s) => s.wavelengthUm >= ignoreBelowUm)
    .toSorted((a, b) => a.wavelengthUm - b.wavelengthUm);
  const lowUm = (grid[0] ?? Number.NaN) / 1000;
  const highUm = (grid.at(-1) ?? Number.NaN) / 1000;
  const start = Math.max(
    0,
    sorted.findLastIndex((s) => s.wavelengthUm <= lowUm),
  );
  const after = sorted.findIndex((s) => s.wavelengthUm >= highUm);
  const window = sorted.slice(start, after < 0 ? sorted.length : after + 1);
  const out: IndexSample[] = [];
  for (const row of window) {
    const last = out.at(-1);
    if (last !== undefined && last.wavelengthUm === row.wavelengthUm) {
      if (last.n === row.n && Object.is(last.k, row.k)) {
        continue;
      }
      throw new Error(`resample: two rows at ${row.wavelengthUm} µm`);
    }
    out.push(row);
  }
  if (out.length < 2) {
    throw new Error("resample: fewer than two rows near the grid");
  }
  return out;
}

/**
 * The rows of a refractiveindex.info database file with one `tabulated nk` block: λ (µm), n, k.
 *
 * @throws Error if the file has no such block, or more than one data block.
 */
export function parseRefractiveIndexInfo(text: string): IndexSample[] {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim() === "- type: tabulated nk");
  if (start < 0 || lines.filter((line) => line.includes("type:")).length !== 1) {
    throw new Error("refractiveindex.info: expected exactly one `tabulated nk` block");
  }
  const rows: IndexSample[] = [];
  for (const line of lines.slice(start + 2)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3 || !NUMBER.test(fields[0] ?? "")) {
      break;
    }
    rows.push({
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/**
 * The rows of a refractiveindex.info database file with one `tabulated n` block: λ (µm) and n, with
 * k = 0, which the material file states (the source gives no k).
 *
 * @throws Error if the file has no such block, or more than one data block.
 */
export function parseRefractiveIndexInfoN(text: string): IndexSample[] {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim() === "- type: tabulated n");
  if (start < 0 || lines.filter((line) => line.includes("type:")).length !== 1) {
    throw new Error("refractiveindex.info: expected exactly one `tabulated n` block");
  }
  const rows: IndexSample[] = [];
  for (const line of lines.slice(start + 2)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 2 || !NUMBER.test(fields[0] ?? "")) {
      break;
    }
    rows.push({ wavelengthUm: numeric(fields[0] ?? ""), n: numeric(fields[1] ?? ""), k: 0 });
  }
  return rows;
}

/**
 * A refractiveindex.info `formula` entry (the database's `doc/Dispersion formulas.pdf`), λ in µm:
 *
 * - type 1: n² − 1 = C₁ + Σ C₂ᵢ λ² ÷ (λ² − C₂ᵢ₊₁²);
 * - type 2: n² − 1 = C₁ + Σ C₂ᵢ λ² ÷ (λ² − C₂ᵢ₊₁);
 * - type 4: n² = C₁ + C₂ λ^C₃ ÷ (λ² − C₄^C₅) + C₆ λ^C₇ ÷ (λ² − C₈^C₉) + Σ C₂ⱼ λ^C₂ⱼ₊₁ (j ≥ 5).
 */
export interface DispersionFormula {
  readonly type: 1 | 2 | 4;
  /** The range the entry states the formula for, µm. */
  readonly rangeUm: readonly [number, number];
  readonly coefficients: ReadonlyArray<number>;
}

/**
 * The formula of a refractiveindex.info database file with one `formula 1`, `formula 2` or
 * `formula 4` block.
 *
 * @throws Error if the file has no such block, more than one data block, or no range or
 *   coefficients.
 */
export function parseRefractiveIndexInfoFormula(text: string): DispersionFormula {
  const lines = text.split(/\r?\n/u).map((line) => line.trim());
  const typed = lines.filter((line) => line.includes("type:"));
  const type = /^- type: formula ([124])$/u.exec(typed[0] ?? "")?.[1];
  if (typed.length !== 1 || type === undefined) {
    throw new Error("refractiveindex.info: expected exactly one `formula 1`, `2` or `4` block");
  }
  const field = (name: string): number[] => {
    const line = lines.find((l) => l.startsWith(`${name}:`));
    if (line === undefined) {
      throw new Error(`refractiveindex.info: the formula has no ${name}`);
    }
    return line
      .slice(name.length + 1)
      .trim()
      .split(/\s+/u)
      .map(numeric);
  };
  const [from, to] = field("wavelength_range");
  if (from === undefined || to === undefined) {
    throw new Error("refractiveindex.info: the formula's range needs two numbers");
  }
  return {
    type: type === "1" ? 1 : type === "2" ? 2 : 4,
    rangeUm: [from, to],
    coefficients: field("coefficients"),
  };
}

/**
 * A {@link DispersionFormula}'s real index at a wavelength, µm.
 *
 * @throws RangeError for a wavelength outside the formula's stated range.
 */
export function formulaIndex(formula: DispersionFormula, wavelengthUm: number): number {
  const [from, to] = formula.rangeUm;
  if (!(wavelengthUm >= from && wavelengthUm <= to)) {
    throw new RangeError(`${wavelengthUm} µm lies outside the formula's ${from}–${to} µm`);
  }
  const c = (i: number): number => formula.coefficients[i] ?? 0;
  const l2 = wavelengthUm * wavelengthUm;
  let nSquared: number;
  switch (formula.type) {
    case 1:
    case 2: {
      let sum = c(0);
      for (let i = 1; i + 1 < formula.coefficients.length; i += 2) {
        const pole = formula.type === 1 ? c(i + 1) * c(i + 1) : c(i + 1);
        sum += (c(i) * l2) / (l2 - pole);
      }
      nSquared = 1 + sum;
      break;
    }
    case 4: {
      nSquared =
        c(0) +
        (c(1) * wavelengthUm ** c(2)) / (l2 - c(3) ** c(4)) +
        (c(5) * wavelengthUm ** c(6)) / (l2 - c(7) ** c(8));
      for (let i = 9; i + 1 < formula.coefficients.length; i += 2) {
        nSquared += c(i) * wavelengthUm ** c(i + 1);
      }
      break;
    }
  }
  return Math.sqrt(nSquared);
}

/**
 * The rows of an ARIA `.ri` file: `#FORMAT=WAVN N K`, wavenumber (cm⁻¹), n, k, or `#FORMAT=WAVL N
 * K`, wavelength (µm), n, k; either n or k may be NaN.
 *
 * @throws Error if the format line is missing, or a data line is not three numbers.
 */
export function parseAria(text: string): IndexSample[] {
  const lines = text.split(/\r?\n/u);
  const wavenumber = lines.some((line) => line.trim() === "#FORMAT=WAVN N K");
  if (!wavenumber && !lines.some((line) => line.trim() === "#FORMAT=WAVL N K")) {
    throw new Error("ARIA: expected #FORMAT=WAVN N K or #FORMAT=WAVL N K");
  }
  const rows: IndexSample[] = [];
  for (const [i, line] of lines.entries()) {
    if (line.startsWith("#") || line.trim().length === 0) {
      continue;
    }
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3) {
      throw new Error(`ARIA: line ${i + 1} has ${fields.length} fields`);
    }
    const first = numeric(fields[0] ?? "");
    rows.push({
      wavelengthUm: wavenumber ? 1e4 / first : first,
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/**
 * The rows of a file of Kitzmann and Heng 2018's compilation (LX-MIE's `compilation/`): three
 * header lines (the material and its references, `#Data points: N`, the columns), then λ (µm), n
 * and k. Read as data, for its values only.
 *
 * @throws Error if the count disagrees with the rows, or a row is not three numbers.
 */
export function parseKitzmannHeng(text: string): IndexSample[] {
  const lines = text.split(/\r?\n/u).filter((line) => line.trim().length > 0);
  const count = /^#Data points:\s*(\d+)\s*$/u.exec(lines[1] ?? "")?.[1];
  if (
    count === undefined ||
    !(lines[0] ?? "").startsWith("#") ||
    !(lines[2] ?? "").startsWith("#")
  ) {
    throw new Error("Kitzmann and Heng: expected three header lines");
  }
  const rows = lines.slice(3).map((line, i) => {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3) {
      throw new Error(`Kitzmann and Heng: row ${i + 1} has ${fields.length} fields`);
    }
    return {
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    };
  });
  if (rows.length !== Number(count)) {
    throw new Error(`Kitzmann and Heng: the header says ${count} rows, found ${rows.length}`);
  }
  return rows;
}

/** One row of a dielectric function: the wavelength, µm, and ε = ε₁ + iε₂. */
export interface DielectricSample {
  readonly wavelengthUm: number;
  readonly epsilonRe: number;
  readonly epsilonIm: number;
}

/**
 * The rows of B. T. Draine's `callindex.out_*` files: header lines, then a column line (`wave(um)
 * eps_1-1 eps_2 Re(n)-1 Im(n)`) and rows of λ (µm), ε₁ − 1, ε₂, Re(n) − 1 and Im(n); ε is read.
 *
 * @throws Error if the column line is missing, or a row is not five numbers.
 */
export function parseDraineDielectric(text: string): DielectricSample[] {
  const lines = text.split(/\r?\n/u);
  const head = lines.findIndex((line) => /^\s*wave\(um\)\s+eps_1-1\s+eps_2\s/u.test(line));
  if (head < 0) {
    throw new Error("Draine: expected the column line wave(um) eps_1-1 eps_2 Re(n)-1 Im(n)");
  }
  const rows: DielectricSample[] = [];
  for (const [i, line] of lines.slice(head + 1).entries()) {
    if (line.trim().length === 0) {
      continue;
    }
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 5 || !fields.every((f) => NUMBER.test(f))) {
      throw new Error(`Draine: row ${i + 1} is not five numbers`);
    }
    rows.push({
      wavelengthUm: numeric(fields[0] ?? ""),
      epsilonRe: 1 + numeric(fields[1] ?? ""),
      epsilonIm: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/** The index n + ik of a dielectric function ε (n = √ε with n, k ≥ 0). */
export function indexOfDielectric(
  epsilonRe: number,
  epsilonIm: number,
): {
  readonly n: number;
  readonly k: number;
} {
  const modulus = Math.hypot(epsilonRe, epsilonIm);
  const n = Math.sqrt((modulus + epsilonRe) / 2);
  return { n, k: epsilonIm / (2 * n) };
}

/**
 * A uniaxial crystal's two dielectric functions averaged for randomly oriented grains, ε = (2ε_⊥ +
 * ε_∥) ÷ 3 (2ε_o + ε_e), at the wavelengths both sources give, as the index rows of the average.
 *
 * @throws Error if the two sources' wavelengths differ inside the grid's window.
 */
export function averagedUniaxial(
  perpendicular: ReadonlyArray<DielectricSample>,
  parallel: ReadonlyArray<DielectricSample>,
  grid: ReadonlyArray<number> = MATERIAL_GRID_NM,
): IndexSample[] {
  const window = (rows: ReadonlyArray<DielectricSample>): DielectricSample[] => {
    const sorted = rows.toSorted((a, b) => a.wavelengthUm - b.wavelengthUm);
    const low = (grid[0] ?? Number.NaN) / 1000;
    const high = (grid.at(-1) ?? Number.NaN) / 1000;
    const start = Math.max(
      0,
      sorted.findLastIndex((s) => s.wavelengthUm <= low),
    );
    const after = sorted.findIndex((s) => s.wavelengthUm >= high);
    return sorted.slice(start, after < 0 ? sorted.length : after + 1);
  };
  const perp = window(perpendicular);
  const para = window(parallel);
  if (
    perp.length !== para.length ||
    perp.some((p, i) => p.wavelengthUm !== para[i]?.wavelengthUm)
  ) {
    throw new Error("averagedUniaxial: the two sources' wavelengths differ near the grid");
  }
  return perp.map((p, i) => {
    const q = para[i] ?? p;
    const { n, k } = indexOfDielectric(
      (2 * p.epsilonRe + q.epsilonRe) / 3,
      (2 * p.epsilonIm + q.epsilonIm) / 3,
    );
    return { wavelengthUm: p.wavelengthUm, n, k };
  });
}

/**
 * The column of Martikainen et al. 2023's ASCII Tables 3 and 6 (ApJS 268, 47): λ (nm), then one
 * column per sample of `k +or- σ`, read for the sample named, with n the paper's fixed 1.5.
 *
 * @throws Error if the column line lacks the sample, or a row is not one wavelength and a k per
 *   sample.
 */
export function parseMartikainenK(text: string, sample: string): IndexSample[] {
  const lines = text.split(/\r?\n/u).map((line) => line.trim());
  const head = lines.findIndex((line) => line.startsWith("lambda (nm)"));
  const columns = (lines[head] ?? "").split("\t").map((c) => c.trim());
  const column = columns.findIndex((c) => c.startsWith(`k _${sample} `));
  if (head < 0 || column < 1) {
    throw new Error(`Martikainen: no column for ${sample}`);
  }
  const rows: IndexSample[] = [];
  for (const line of lines.slice(head + 1)) {
    if (line.length === 0) {
      continue;
    }
    const cells = line.split("\t").map((c) => c.trim());
    const k = /^([0-9.]+) \+or- [0-9.]+$/u.exec(cells[column] ?? "")?.[1];
    if (cells.length < columns.length - 1 || k === undefined) {
      throw new Error(`Martikainen: the row "${line}" is not a k per sample`);
    }
    rows.push({
      wavelengthUm: numeric(cells[0] ?? "") / 1000,
      n: MARTIKAINEN_FIXED_N,
      k: numeric(k),
    });
  }
  return rows;
}

/** The real index Martikainen et al. 2023 fix for their k retrievals (their Table 5's note). */
export const MARTIKAINEN_FIXED_N = 1.5;

/** One cell of a LaTeX table, without its uncertainty: `$0.033\pm0.003$` → 0.033, `1.95$\times 10^{-3}$` → 1.95e-3. */
function texNumber(cell: string): number {
  const bare = cell
    .replaceAll("$", "")
    .replace(/\\pm.*$/u, "")
    .trim();
  const power = /^([0-9.]+)\\times\s*10\^\{(-?\d+)\}$/u.exec(bare);
  return power === null ? numeric(bare) : numeric(`${power[1]}e${power[2]}`);
}

/** The data rows of the LaTeX table whose `\label{…}` is given: each `&`-separated row of numbers. */
function texTableRows(text: string, label: string): number[][] {
  const lines = text.split(/\r?\n/u);
  const at = lines.findIndex((line) => line.includes(`\\label{${label}}`));
  if (at < 0) {
    throw new Error(`LaTeX: no table labelled ${label}`);
  }
  let start = at;
  while (start > 0 && !(lines[start] ?? "").includes("\\begin{table")) {
    start -= 1;
  }
  let end = at;
  while (end < lines.length && !(lines[end] ?? "").includes("\\end{table")) {
    end += 1;
  }
  const rows: number[][] = [];
  for (const line of lines.slice(start, end)) {
    const cells = line
      .replace(/\\(?:tabularnewline|\\).*$/u, "")
      .split("&")
      .map((c) => c.trim());
    if (cells.length > 1 && /^[0-9.]+$/u.test(cells[0] ?? "")) {
      rows.push(cells.map(texNumber));
    }
  }
  return rows;
}

/** The real index Braude et al. 2020 fix for the chromophore at 700 nm (the preprint's §4.1). */
export const CHROMOPHORE_REAL_INDEX = 1.4;

/**
 * Braude et al. 2020's retrieved chromophore k (their table "Tabulated retrieved imaginary
 * refractive index spectra", label `imagritable`): λ every 50 nm over 450–950 nm, of which the
 * first k column is the adopted "universal chromophore" (r_c 0.05 µm, r_n 1 µm, n_n 1.42), with n
 * the paper's fixed {@link CHROMOPHORE_REAL_INDEX}.
 *
 * @throws Error if the table is missing or a row lacks its four solutions.
 */
export function parseBraudeChromophore(text: string): IndexSample[] {
  return texTableRows(text, "imagritable").map((row) => {
    const [nm, k] = row;
    if (row.length !== 5 || nm === undefined || k === undefined) {
      throw new Error("Braude: a row lacks its four solutions");
    }
    return { wavelengthUm: nm / 1000, n: CHROMOPHORE_REAL_INDEX, k };
  });
}

/** The real index Irwin et al. 2022 assume for every aerosol at 0.8 µm (the preprint's §3.4 and §3.6). */
export const ICE_GIANT_HAZE_REAL_INDEX = 1.4;

/** Aerosol-3's "Mean 1" column in Irwin et al. 2022's n_imag tables: the seventh after λ. */
const IRWIN_AEROSOL3_MEAN1 = 7;

/** Irwin et al. 2022's Aerosol-3 "Mean 1" n_imag, per planet, every 0.1 µm (λ µm, k). */
export function irwinAerosol3(text: string, planet: "uranus" | "neptune"): IndexSample[] {
  return texTableRows(text, planet === "uranus" ? "tab:refuranus" : "tab:refneptune").map((row) => {
    const um = row[0];
    const k = row[IRWIN_AEROSOL3_MEAN1];
    if (row.length !== 10 || um === undefined || k === undefined) {
      throw new Error(`Irwin: a ${planet} row lacks its nine columns`);
    }
    return { wavelengthUm: um, n: ICE_GIANT_HAZE_REAL_INDEX, k };
  });
}

/**
 * The ice giants' haze as one material: at each wavelength both of Irwin et al. 2022's tables give
 * (0.3–1.9 µm), the geometric mean of Uranus's and Neptune's Aerosol-3 "Mean 1" k, with n the
 * paper's {@link ICE_GIANT_HAZE_REAL_INDEX}.
 *
 * @throws Error if either table is missing, or the two disagree in wavelength.
 */
export function parseIrwinAerosol3(text: string): IndexSample[] {
  const uranus = irwinAerosol3(text, "uranus");
  const neptune = irwinAerosol3(text, "neptune");
  return uranus.map((u) => {
    const v = neptune.find((row) => row.wavelengthUm === u.wavelengthUm);
    if (v === undefined) {
      throw new Error(`Irwin: Neptune's table has no row at ${u.wavelengthUm} µm`);
    }
    return { wavelengthUm: u.wavelengthUm, n: ICE_GIANT_HAZE_REAL_INDEX, k: Math.sqrt(u.k * v.k) };
  });
}

/**
 * The rows of an optool `.lnk` file: `#` comments, a count and density line, then λ (µm), n, k.
 *
 * @throws Error if the count disagrees with the rows, or a row is not three numbers.
 */
export function parseLnk(text: string): IndexSample[] {
  const data = text
    .split(/\r?\n/u)
    .filter((line) => !line.startsWith("#") && line.trim().length > 0)
    .map((line) => line.trim().split(/\s+/u));
  const [head, ...body] = data;
  const count = head === undefined ? Number.NaN : numeric(head[0] ?? "");
  if (head?.length !== 2 || count !== body.length) {
    throw new Error(`lnk: the count line says ${count} rows, found ${body.length}`);
  }
  const rows = body.map((fields, i) => {
    if (fields.length !== 3) {
      throw new Error(`lnk: row ${i + 1} has ${fields.length} fields`);
    }
    return {
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    };
  });
  return rows;
}

/**
 * The rows of a HITRAN-RI ASCII table in four columns: wavenumber (cm⁻¹), λ (µm), n, k. Lines that
 * are not four numbers (the header) are skipped.
 */
export function parseHitranTable(text: string): IndexSample[] {
  const rows: IndexSample[] = [];
  for (const line of text.split(/\r?\n/u)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 4 || !fields.every((f) => NUMBER.test(f))) {
      continue;
    }
    rows.push({
      wavelengthUm: numeric(fields[1] ?? ""),
      n: numeric(fields[2] ?? ""),
      k: numeric(fields[3] ?? ""),
    });
  }
  return rows;
}

/** The rows of a table in three columns, λ (µm), n, k, after a header line that is not numbers. */
export function parseWavelengthTable(text: string): IndexSample[] {
  const rows: IndexSample[] = [];
  for (const line of text.split(/\r?\n/u)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3 || !fields.every((f) => NUMBER.test(f))) {
      continue;
    }
    rows.push({
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/**
 * The imaginary index of an absorption coefficient: k = αλ ÷ 4π.
 *
 * @param absorptionPerM - The (Napierian) absorption coefficient α, m⁻¹.
 * @param wavelengthNm - The vacuum wavelength, nm.
 */
export function absorptionIndex(absorptionPerM: number, wavelengthNm: number): number {
  return (absorptionPerM * wavelengthNm * 1e-9) / (4 * Math.PI);
}

/**
 * The rows of an omlc.org absorption table in two columns, λ (nm) and the absorption coefficient a
 * (cm⁻¹), after header lines that are not two numbers: each row's k = aλ ÷ 4π, with n NaN, since
 * the table gives none (R08.T5.g).
 *
 * @param rowCount - The rows the table must hold, so that a truncated download is refused.
 * @throws Error if the table holds another number of rows, or a wavelength twice.
 */
export function parseOmlcAbsorption(text: string, rowCount: number): IndexSample[] {
  const rows: IndexSample[] = [];
  for (const line of text.split(/\r?\n/u)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 2 || !fields.every((f) => NUMBER.test(f))) {
      continue;
    }
    const nm = numeric(fields[0] ?? "");
    const perCm = numeric(fields[1] ?? "");
    rows.push({ wavelengthUm: nm / 1000, n: Number.NaN, k: absorptionIndex(100 * perCm, nm) });
  }
  if (rows.length !== rowCount) {
    throw new Error(`omlc: expected ${rowCount} rows, found ${rows.length}`);
  }
  if (new Set(rows.map((row) => row.wavelengthUm)).size !== rows.length) {
    throw new Error("omlc: a wavelength appears twice");
  }
  return rows;
}

/** A material's index on a file's grid: n and k at each wavelength. */
export interface ReducedIndex {
  readonly n: ReadonlyArray<number>;
  readonly k: ReadonlyArray<number>;
}

/** How one source is brought onto the grid. */
export interface ResampleOptions {
  /**
   * Source rows below this wavelength, µm, are not used: past a gap across an absorption edge, an
   * interpolation would carry ultraviolet values into the visible.
   */
  readonly ignoreBelowUm?: number;
  /**
   * How far beyond the source's first or last used row, nm, its end value may be held; a grid
   * point farther out is an error.
   */
  readonly holdNm: number;
  /**
   * Where the source gives k as NaN (no measurement), it takes the k of the shortest wavelength
   * that has one. Every NaN must lie at shorter wavelengths than that row.
   */
  readonly unmeasuredK?: "shortestMeasured";
  /**
   * Where the source gives n at some rows and k at others (NaN elsewhere), n is read from the rows
   * that give n and k from the rows that give k, each by the same rule.
   */
  readonly separateNK?: true;
  /**
   * Beyond the source's first or last used row, within `holdNm`, k is extrapolated linearly in
   * log k from the two rows at that end, where both are positive, and n is held; otherwise both
   * are held.
   */
  readonly endK?: "hold" | "extrapolate";
  /** The grid, nm: {@link MATERIAL_GRID_NM} unless a file is written on another (ruling 1). */
  readonly grid?: ReadonlyArray<number>;
}

function interpolateK(k0: number, k1: number, t: number): number {
  return k0 > 0 && k1 > 0 ? k0 * (k1 / k0) ** t : k0 + t * (k1 - k0);
}

/** Six significant figures, past what any source measures. */
function rounded(value: number): number {
  return Number(value.toPrecision(6));
}

/**
 * A source reduced onto its grid ({@link MATERIAL_GRID_NM} unless `options.grid` names another): n
 * linear and k log-linear in wavelength.
 *
 * @throws Error if two rows near the grid disagree at one wavelength, a grid point lies farther
 *   beyond the source than `holdNm`, a row has no finite n > 0 and k ≥ 0, or an unmeasured k
 *   cannot be set.
 */
export function resample(
  samples: ReadonlyArray<IndexSample>,
  options: ResampleOptions,
): ReducedIndex {
  if (options.separateNK === true) {
    // Each part's placeholder (k 0, n 1) is never read: the part keeps only its own column.
    const nRows = samples
      .filter((s) => Number.isFinite(s.n))
      .map((s) => ({ wavelengthUm: s.wavelengthUm, n: s.n, k: 0 }));
    const kRows = samples
      .filter((s) => Number.isFinite(s.k))
      .map((s) => ({ wavelengthUm: s.wavelengthUm, n: 1, k: s.k }));
    return { n: resampleRows(nRows, options).n, k: resampleRows(kRows, options).k };
  }
  return resampleRows(samples, options);
}

/** {@link resample} of rows that each give n and k. */
function resampleRows(samples: ReadonlyArray<IndexSample>, options: ResampleOptions): ReducedIndex {
  const grid = options.grid ?? MATERIAL_GRID_NM;
  let rows = usedRows(samples, options.ignoreBelowUm ?? 0, grid);
  if (options.unmeasuredK === "shortestMeasured") {
    const first = rows.find((s) => Number.isFinite(s.k));
    if (first === undefined) {
      throw new Error("resample: no measured k");
    }
    rows = rows.map((s) => {
      if (Number.isFinite(s.k)) {
        return s;
      }
      if (s.wavelengthUm > first.wavelengthUm) {
        throw new Error(`resample: k unmeasured at ${s.wavelengthUm} µm, past the first measured`);
      }
      return { ...s, k: first.k };
    });
  }
  if (rows.some((s) => !Number.isFinite(s.n) || !Number.isFinite(s.k) || s.n <= 0 || s.k < 0)) {
    throw new Error("resample: a row has no finite n > 0 and k ≥ 0");
  }
  const first = rows[0];
  const last = rows.at(-1);
  if (first === undefined || last === undefined) {
    throw new Error("resample: no rows");
  }
  const n: number[] = [];
  const k: number[] = [];
  for (const nm of grid) {
    const um = nm / 1000;
    if (um <= first.wavelengthUm || um >= last.wavelengthUm) {
      const below = um <= first.wavelengthUm;
      const end = below ? first : last;
      const gapNm = Math.abs(end.wavelengthUm - um) * 1000;
      if (gapNm > options.holdNm + 1e-9) {
        throw new Error(`resample: ${nm} nm lies ${gapNm.toFixed(1)} nm beyond the source`);
      }
      n.push(rounded(end.n));
      const next = below ? rows[1] : rows.at(-2);
      const extrapolate =
        options.endK === "extrapolate" && next !== undefined && end.k > 0 && next.k > 0;
      k.push(
        rounded(
          extrapolate
            ? interpolateK(
                end.k,
                next.k,
                (um - end.wavelengthUm) / (next.wavelengthUm - end.wavelengthUm),
              )
            : end.k,
        ),
      );
      continue;
    }
    const upper = rows.findIndex((s) => s.wavelengthUm >= um);
    const hi = rows[upper];
    const lo = rows[upper - 1];
    if (hi === undefined || lo === undefined) {
      throw new Error(`resample: no bracket at ${nm} nm`);
    }
    const t = (um - lo.wavelengthUm) / (hi.wavelengthUm - lo.wavelengthUm);
    n.push(rounded(lo.n + t * (hi.n - lo.n)));
    k.push(rounded(interpolateK(lo.k, hi.k, t)));
  }
  return { n, k };
}

const RI_URL = `https://raw.githubusercontent.com/polyanskiy/refractiveindex.info-database/${RI_COMMIT}/database/data`;
const OPTOOL_URL = `https://raw.githubusercontent.com/cdominik/optool/${OPTOOL_COMMIT}/lnk_data`;
const RI_LICENCE =
  "CC0 1.0 (the refractiveindex.info database, M. N. Polyanskiy, Sci. Data 11, 94 (2024)); reduced values";
const NO_TERMS = "reduced values; the source states no terms (decision-r08-licences.md)";
const OPTOOL_TERMS =
  "reduced values; the source states no terms for its data, and optool's MIT licence covers its software (decision-r08-licences.md)";
const STANDARD_REDUCTION =
  "n linear and k linear in log k in wavelength between the source's rows, onto 380–780 nm every 5 nm, to six significant figures";

/**
 * The grid of the two files Venus's ultraviolet fit reads at 365 nm, nm: 360 to 780 every 5
 * (decision-r08-t5d ruling 1), the 75 wt% sulphuric acid's and `acid_uv_absorber`'s.
 */
export const ACID_GRID_NM: ReadonlyArray<number> = Array.from(
  { length: 85 },
  (_, i) => 360 + 5 * i,
);

/** {@link STANDARD_REDUCTION} onto another grid. */
function standardReductionOnto(grid: ReadonlyArray<number>): string {
  return `n linear and k linear in log k in wavelength between the source's rows, onto ${grid[0] ?? Number.NaN}–${grid.at(-1) ?? Number.NaN} nm every 5 nm, to six significant figures`;
}

/** A material file reduced from one fetched source. */
interface FetchedSpec {
  readonly file: string;
  /** The command-line option that names the source's path. */
  readonly option: string;
  readonly sha256: string;
  readonly parse: (text: string) => IndexSample[];
  readonly resample: ResampleOptions;
  readonly header: MaterialHeader;
  /**
   * A stated change made to the reduced index, on the file's grid: `acid_uv_absorber`'s added k_UA
   * ({@link addAcidUvAbsorber}), and since R08.T5.i NH₃ ice's k set to 0
   * ({@link withoutVisibleAbsorption}). Absent for a file that is the reduction alone.
   */
  readonly adjust?: (index: ReducedIndex, grid: ReadonlyArray<number>) => ReducedIndex;
}

function fetchedSource(what: string, url: string, sha256: string, fetched = FETCHED): string {
  return `${what}, ${url}, SHA-256 ${sha256}, fetched ${fetched}`;
}

/** The day R08.T5.d's sources were fetched. */
export const FETCHED_T5D = "2026-10-14";

/** The day R08.T5.g's sources were fetched. */
export const FETCHED_T5G = "2026-10-14";

/** The day R08.T5.h's sources were fetched. */
export const FETCHED_T5H = "2026-10-15";

/** The day R08.T5.i's sources were fetched (decision-p14-t49c-split §5.3). */
export const FETCHED_T5I = "2026-10-16";

/** The day R08.T5.k's sources were fetched (decision-p14-t49h-gas-index). */
export const FETCHED_T5K = "2026-10-16";

/**
 * The a-priori k of Venus's ultraviolet absorber at 365 nm, k₃₆₅: the ruled value
 * (science-r08-unowned-absorbers, adopted; decision-r08-t5d), not derived here. R08.T5.e refits it
 * to Venus's 365 nm albedo. For scale, Spacek et al.'s §2.1 asks k ≈ 0.003 of mode-2 droplets at
 * 365 nm (the preprint below), 2.5 times this.
 */
export const ACID_UV_K365 = 1.2e-3;

/**
 * The a-priori decay length of Venus's ultraviolet absorber, L, nm (science-r08-unowned-absorbers,
 * ruled): the bound from J. Spacek, P. B. Rimmer, J. J. Petkowski and Y. J. Lee, Astrobiology 26
 * (2026) 727–739, DOI 10.1177/15311074261477502, its values read from the preprint,
 * arXiv:2505.00880v3 (CC BY 4.0), whose decadic absorption coefficients (a = α ÷ ln 10, its §2.1)
 * are 1278 cm⁻¹ at 375 nm and under 54 cm⁻¹ at 455 nm. In k = ln(10) aλ ÷ 4π that is a fall by
 * (1278 × 375) ÷ (54 × 455) = 19.51 over 80 nm, ln 10 cancelling, so L ≤ 80 nm ÷ ln 19.51 =
 * 26.9 nm (the ruling's shorthand 80 nm ÷ ln(1278 ÷ 54), 25.3 nm, leaves out λ). R08.T5.e refits
 * it to Venus's g′ ÷ r′ colour.
 */
export const ACID_UV_DECAY_NM = 26.9;

/**
 * The absorber's k, k_UA, at a wavelength, nm: k₃₆₅ e^(−(λ − 365 nm) ÷ L) over 360–500 nm, times
 * (550 − λ) ÷ 50 nm over 500–550 nm, and exactly 0 from 550 nm, so that the material's κ_ext and ω
 * at 550 nm are H₂SO₄'s (decision-r08-t5d ruling 4).
 */
export function acidUvAbsorberK(wavelengthNm: number): number {
  if (wavelengthNm >= 550) {
    return 0;
  }
  const decay = ACID_UV_K365 * Math.exp(-(wavelengthNm - 365) / ACID_UV_DECAY_NM);
  return wavelengthNm > 500 ? (decay * (550 - wavelengthNm)) / 50 : decay;
}

/** `acid_uv_absorber`'s index from the 75 wt% acid's: n unchanged, k = rounded(k_H₂SO₄ + k_UA). */
function addAcidUvAbsorber(acid: ReducedIndex, grid: ReadonlyArray<number>): ReducedIndex {
  return {
    n: acid.n,
    k: acid.k.map((k, i) => rounded(k + acidUvAbsorberK(grid[i] ?? Number.NaN))),
  };
}

/**
 * NH₃ ice's index with its visible absorption not drawn: n as reduced, and k = 0 at every grid
 * point, a stated stand-in (R08.T5.i; decision-p14-t49c-split §5.3).
 *
 * @remarks
 * Martonchik et al.'s k rows between their 1,462 and 200 nm rows are interpolations, not
 * measurements ({@link MARTONCHIK_TABLE_I_ROWS}), and no measured visible absorption of NH₃ ice was
 * found, so
 * the file draws the colourless crystal lossless: an upper bound on its brightness, as the k = 0
 * ices are (decision-p14-t49c-split §5.6, "Lossless grains (the k = 0 stand-ins): confirmed
 * recorded, as upper bounds and labelled").
 */
export function withoutVisibleAbsorption(index: ReducedIndex): ReducedIndex {
  return { n: index.n, k: index.k.map(() => 0) };
}

/** One row of a table by wavenumber: ν, cm⁻¹, and the complex index n + ik. */
export interface WavenumberRow {
  readonly wavenumberPerCm: number;
  readonly n: number;
  readonly k: number;
}

/**
 * Martonchik, Orton and Appleby 1984's Table I (Appl. Opt. 23, 541) from its 6,840 cm⁻¹ row
 * (1,462 nm) to its 50,000 cm⁻¹ row (200 nm), every row between, as the journal's abstract page
 * shows them ({@link MARTONCHIK_PAGE}, read {@link FETCHED_T5I}).
 *
 * @remarks
 * R08.T5.i reads them as the evidence that the k rows between are interpolations
 * (decision-p14-t49c-split §5.3): the 14,976.7 and 23,113.3 cm⁻¹ rows are the linear fill in k
 * between the 6,840 and 31,250 cm⁻¹ rows ({@link wavenumberFill}), and the rows from 31,250 to
 * 50,000 cm⁻¹ lie on a straight line in wavelength ({@link martonchikUltravioletLineK}). The file's
 * n is reduced from optool's copy of the table, not from these.
 */
export const MARTONCHIK_TABLE_I_ROWS: ReadonlyArray<WavenumberRow> = [
  { wavenumberPerCm: 6840, n: 1.408, k: 5.817e-5 },
  { wavenumberPerCm: 14_976.7, n: 1.423, k: 3.963e-5 },
  { wavenumberPerCm: 23_113.3, n: 1.449, k: 2.109e-5 },
  { wavenumberPerCm: 31_250, n: 1.488, k: 2.546e-6 },
  { wavenumberPerCm: 33_333, n: 1.505, k: 2.387e-6 },
  { wavenumberPerCm: 35_714, n: 1.517, k: 2.228e-6 },
  { wavenumberPerCm: 38_462, n: 1.544, k: 2.069e-6 },
  { wavenumberPerCm: 41_667, n: 1.582, k: 1.91e-6 },
  { wavenumberPerCm: 46_729, n: 1.677, k: 1.703e-6 },
  { wavenumberPerCm: 47_619, n: 1.707, k: 1.671e-6 },
  { wavenumberPerCm: 50_000, n: 1.792, k: 1.592e-6 },
];

const MARTONCHIK_PAGE = fetchedSource(
  "the journal's abstract page",
  "https://opg.optica.org/ao/abstract.cfm?uri=ao-23-4-541",
  "42ed721929a8baeeae4a1ed0f79970bf0c6fc46e93b56c0e46156a3e4bc83092",
  FETCHED_T5I,
);

/** A row of {@link MARTONCHIK_TABLE_I_ROWS} by its wavenumber, cm⁻¹. */
function martonchikRow(wavenumberPerCm: number): WavenumberRow {
  const row = MARTONCHIK_TABLE_I_ROWS.find((r) => r.wavenumberPerCm === wavenumberPerCm);
  if (row === undefined) {
    throw new Error(`no Table I row at ${wavenumberPerCm} cm⁻¹`);
  }
  return row;
}

/**
 * The straight line in wavelength that every row of {@link MARTONCHIK_TABLE_I_ROWS} from 200 to
 * 320 nm lies on, within a unit of the table's last figure (computed for R08.T5.i, as decision-p14-t49c-split
 * §5.3 found the 240–320 nm rows): k = k₂₀₀ + s (λ − 200 nm), with k₂₀₀ the 200 nm row's k and s
 * per nm.
 */
export const MARTONCHIK_UV_LINE = { kAt200Nm: 1.592e-6, slopePerNm: 7.95e-9 } as const;

/** k on {@link MARTONCHIK_UV_LINE} at a wavelength, nm. */
export function martonchikUltravioletLineK(wavelengthNm: number): number {
  return MARTONCHIK_UV_LINE.kAt200Nm + MARTONCHIK_UV_LINE.slopePerNm * (wavelengthNm - 200);
}

/** Where a wavenumber, cm⁻¹, lies between two rows: 0 at the first, 1 at the second. */
export function wavenumberFraction(
  lo: WavenumberRow,
  hi: WavenumberRow,
  wavenumberPerCm: number,
): number {
  return (wavenumberPerCm - lo.wavenumberPerCm) / (hi.wavenumberPerCm - lo.wavenumberPerCm);
}

/** The index the fills between two rows give at a wavenumber between them. */
export interface WavenumberFill {
  /** k linear in k. */
  readonly kLinear: number;
  /** k linear in log k, the reduction's own rule, for comparison. */
  readonly kLogLinear: number;
  /** n linear in n. */
  readonly nLinear: number;
}

/** The index the fills between two rows by wavenumber give at a wavenumber between them, cm⁻¹. */
export function wavenumberFill(
  lo: WavenumberRow,
  hi: WavenumberRow,
  wavenumberPerCm: number,
): WavenumberFill {
  const t = wavenumberFraction(lo, hi, wavenumberPerCm);
  return {
    kLinear: lo.k + t * (hi.k - lo.k),
    kLogLinear: lo.k * (hi.k / lo.k) ** t,
    nLinear: lo.n + t * (hi.n - lo.n),
  };
}

/**
 * `ammonia-ice.json`'s index at 550 nm as R08.T5.b reduced it, before R08.T5.i's k = 0: n and k
 * from optool's copy of Martonchik et al.'s table (`sizeDistributionStop.test.ts` holds its NH₃-ice
 * mode at it).
 */
export const T5B_AMMONIA_ICE_550 = { n: 1.43606, k: 2.88714e-5 } as const;

/** A positive number as `m × 10ⁿ`, its mantissa to the fraction digits given. */
function timesTen(value: number, fractionDigits: number): string {
  const [mantissa, exponent] = value.toExponential(fractionDigits).split("e");
  const superscript = (exponent ?? "")
    .replace("+", "")
    .replace("-", "⁻")
    .replace(/[0-9]/gu, (d) => "⁰¹²³⁴⁵⁶⁷⁸⁹"[Number(d)] ?? d);
  return `${mantissa ?? ""} × 10${superscript}`;
}

/** A row's wavenumber as the files write it: 14,976.7 cm⁻¹. */
function wavenumberText(row: WavenumberRow): string {
  return row.wavenumberPerCm.toLocaleString("en-GB", { maximumFractionDigits: 1 });
}

/** `ammonia-ice.json`'s `reduction`: n as reduced, and the evidence for its k = 0. */
function ammoniaIceReductionText(): string {
  const lo = martonchikRow(6840);
  const hi = martonchikRow(31_250);
  const r668 = martonchikRow(14_976.7);
  const r433 = martonchikRow(23_113.3);
  const f668 = wavenumberFill(lo, hi, r668.wavenumberPerCm);
  const f433 = wavenumberFill(lo, hi, r433.wavenumberPerCm);
  const cm = wavenumberText;
  const lengthMm = (550e-6 / (4 * Math.PI * T5B_AMMONIA_ICE_550.k)).toFixed(1);
  return `n linear in wavelength between the source's rows, onto 380–780 nm every 5 nm, to six significant figures; the source has rows at 320, 433, 668 and 1,462 nm here, so n is interpolated between them; the copy states no temperature. k = 0 at every grid point, a stated stand-in (R08.T5.i, decision-p14-t49c-split §5.3), since the paper's k rows between its 1,462 and 200 nm rows are interpolations, not measurements, and no measured visible absorption was found (the file's source). Its Table I (${MARTONCHIK_PAGE}) has two rows between 1,462 and 320 nm, ${cm(r668)} cm⁻¹ (668 nm, k ${timesTen(r668.k, 3)}) and ${cm(r433)} cm⁻¹ (433 nm, ${timesTen(r433.k, 3)}), at one and two thirds of the interval between its ${cm(lo)} cm⁻¹ row (k ${timesTen(lo.k, 3)}) and its ${cm(hi)} cm⁻¹ row (${timesTen(hi.k, 3)}); they equal the linear interpolation in k between those rows to four figures, ${timesTen(f668.kLinear, 4)} and ${timesTen(f433.kLinear, 4)}, where a fill linear in log k would give ${timesTen(f668.kLogLinear, 2)} and ${timesTen(f433.kLogLinear, 2)} (computed). Every row from 200 to 320 nm lies on k = ${timesTen(MARTONCHIK_UV_LINE.kAt200Nm, 3)} + ${timesTen(MARTONCHIK_UV_LINE.slopePerNm, 2)} (λ − 200 nm) within a unit of the table's last figure (computed). So 380–780 nm lies wholly on interpolations: 380–433 nm runs towards the 320 nm row, an end of the 200–320 nm line, and 668–780 nm towards the 1,462 nm row, at the edge of the 1.5 µm band (Table I's k reaches 2.693 × 10⁻⁴ at 6,500 cm⁻¹). The paper's abstract says only that the real index was "obtained from previously published spectra of the absorption index ni by means of the Kramers-Kronig dispersion relation", and does not name the visible's source (${fetchedSource("OpenAlex's record", "https://api.openalex.org/works/doi:10.1364/AO.23.000541", "6dd81d6a76cdc19ad45c89c21d9d3226fa3eb4d3121a7c4e5fec05203e97b4e0", FETCHED_T5I)}). As R08.T5.b reduced it, k was ${timesTen(T5B_AMMONIA_ICE_550.k, 5)} at 550 nm, an absorption length of ${lengthMm} mm (computed), which makes a white frost come out at a normal albedo of 0.29 in P14.T49.c's model (decision-p14-t49c-split ruling 5). k = 0 draws the colourless crystal lossless, an upper bound on its brightness, as the k = 0 ices are (decision-p14-t49c-split §5.6), and Hapke et al. 1981's bright frost agrees with it (the file's source); the crystal's weak N–H overtone bands are not drawn. n is not part of the fill: the rows' ${r668.n} and ${r433.n} at 668 and 433 nm lie off the straight line between the 1,462 and 320 nm rows' n (${f668.nLinear.toFixed(4)} and ${f433.nLinear.toFixed(4)}, computed), and n stays as reduced`;
}

/**
 * `ammonia-ice.json`'s record of R08.T5.i's search for a measured visible absorption of NH₃ ice
 * (decision-p14-t49c-split §5.3): each candidate, what it covers, and what was read.
 */
const NH3_ICE_SEARCH_TEXT = `R08.T5.i's search for a measured visible absorption of NH3 ice (${FETCHED_T5I}) found none, nor a measured bound on it, for the solid. The liquid's measured total attenuation (C. W. Robertson and D. Williams, "Lambert absorption coefficient of liquid ammonia in the visible region", J. Opt. Soc. Am. 63 (1973) 763, DOI 10.1364/JOSA.63.000763, Table I, ${fetchedSource("read on the journal's page", "https://opg.optica.org/josa/abstract.cfm?uri=josa-63-6-763", "802a0b942b36a10c61509c62e6977e803a8b4fa37a04d61f7ed01b4d0c14beeb", FETCHED_T5I)}, as ammonia-liquid.json cites it) bounds the liquid's k by 6.7 × 10⁻⁷ at 400 nm and 2.8 × 10⁻⁷ at 654 nm (k = αλ ÷ 4π, computed), 17 and 136 times below the fill's k there, on a sample whose temperature and purity the page does not state. Hapke, Wells, Wagner and Partlow 1981 (Icarus 47, 361, DOI 10.1016/0019-1035(81)90184-6; closed; its abstract at NASA NTRS 19820033673, https://ntrs.nasa.gov/api/citations/19820033673, SHA-256 7249b50d768713b24b5a8a691817669776ffe1407c430cd8559b1cf72b493281) measure frosts' reflectance over 0.1–2.5 µm, "NH3 is bright at wavelengths longer than 0.21 microns, where reflectance drops to a value of only a few per cent at shorter wavelengths"; Wagner, Hapke and Wells 1987 (Icarus 69, 14, DOI 10.1016/0019-1035(87)90003-0; closed; its abstract at NTRS 19870044303, https://ntrs.nasa.gov/api/citations/19870044303, SHA-256 3c7ec35217257030126c23f11bb9b8530880a1d5212b9984bda17bfee1e8a9d6) give an atlas of reflectance over 92–1800 nm with NH3 frost among the frosts. Neither gives k: they are the visible frost reflectances for P14.T49.i to invert. Pipes, Browell and Anderson 1974 (Icarus 21, 283, DOI 10.1016/0019-1035(74)90043-8; closed; its title through Crossref, https://api.crossref.org/works/10.1016/0019-1035(74)90043-8, SHA-256 ee41413eb0b6cc82274e6acb0de1752cdea2562652577831e64328f6c7a5e10f) reach 1400–3000 Å only. Browell and Anderson 1975 (J. Opt. Soc. Am. 65, 919, DOI 10.1364/JOSA.65.000919; closed; its abstract through OpenAlex, https://api.openalex.org/works/doi:10.1364/JOSA.65.000919, SHA-256 ab7cf61eac917a62e0f3b70adc8ed6df7138cb2079059608011041fd0f85ce57) give n of amorphous NH3 ice over 1925–3200 Å, and upper limits on its absorption at 1760 Å only. Hudson, Gerakines and Yarnall 2022 (Astrophys. J. 925, 156, DOI 10.3847/1538-4357/ac3e74; its abstract through OpenAlex, https://api.openalex.org/works/doi:10.3847/1538-4357/ac3e74, SHA-256 e6acf9d02b881e941edba56f6a3e37c633a24b6e4497559b19aec0844fa36927) give infrared optical constants and n at 670 nm. Howett, Carlson, Irwin and Calcutt 2007 (J. Opt. Soc. Am. B 24, 126, DOI 10.1364/JOSAB.24.000126; closed; the journal's abstract page, https://opg.optica.org/josab/abstract.cfm?uri=josab-24-1-126, SHA-256 f3f952d8c413c5fdafa928451f294dc3808e7bd53bc5ad91694f0fc94ec4f630) measure thin-film transmission over 1300–12,000 cm⁻¹ (0.83–7.7 µm), and their Table 1 of earlier work on NH3 ice lists none in the visible but Martonchik et al.'s. Kieffer 1974 ("Ring particle sizes and composition derived from eclipse cooling curves and reflection spectra", in The Rings of Saturn, NASA, 1974; NTRS 19750009169, https://ntrs.nasa.gov/api/citations/19750009169/downloads/19750009169.pdf, SHA-256 d542cec141d79e43819a3b5d497c1f60b0f617d44d58876ff4593a04905da7b3, read in full) shows NH3 frost spectra over about 1.25–3 µm only (its Fig. 1). Lebofsky and Fegley 1976 ("Laboratory reflection spectra for the determination of chemical composition of icy bodies", Icarus 28, 379, DOI 10.1016/0019-1035(76)90151-2; https://api.crossref.org/works/10.1016/0019-1035(76)90151-2, SHA-256 ddb3457e0de897d377a75d28496c42d30b6ac32a14d0a709914267986f7f2708) and Rosen and Pipkin 1978 ("Reflectance spectra for sodium and potassium doped ammonia frosts: Implications for Io's surface", Icarus 35, 252, DOI 10.1016/0019-1035(78)90010-6; https://api.openalex.org/works/doi:10.1016/0019-1035(78)90010-6, SHA-256 16f405f46b5503c8001b22fdc371c0e467d31e900a17ba76ba6e1fe9c3fb37fc) are closed, their titles alone reachable`;

/** The LX-MIE commit Kitzmann and Heng 2018's compilation was fetched at (master, 2026-10-06). */
export const LX_MIE_COMMIT = "38cc6d97c2268d58c41ccfc3955283ab6ee2ca44";

const LX_MIE_URL = `https://raw.githubusercontent.com/NewStrangeWorlds/LX-MIE/${LX_MIE_COMMIT}/compilation`;

const KITZMANN_HENG =
  'D. Kitzmann and K. Heng, "Optical properties of potential condensates in exoplanetary atmospheres", Mon. Not. R. Astron. Soc. 475 (2018) 94–107, DOI 10.1093/mnras/stx3141';

const COMPILATION_TERMS =
  "reduced values from Kitzmann and Heng 2018's compilation, the only machine-readable copy found, cited to the primary and to Kitzmann and Heng as the compiler: the measured values are the authors' facts, and the repository's GPL-3.0 covers its code and arrangement, not a reduced visible subset of one material; its compilation/readme.txt sets no separate terms for the data. No compilation file and none of LX-MIE's code is committed (decision-composition §5; decision-r08-licences.md)";

/** The source of a file reduced from one of Kitzmann and Heng 2018's compilation files. */
function compilationSource(file: string, sha256: string): string {
  return `Kitzmann and Heng 2018's compilation (${KITZMANN_HENG}), LX-MIE's compilation/${file} at commit ${LX_MIE_COMMIT}, ${LX_MIE_URL}/${file}, SHA-256 ${sha256}, fetched ${FETCHED_T5D}; read as data, as the only machine-readable copy of its primary`;
}

const CC_BY = "https://creativecommons.org/licenses/by/4.0/";

const POLLACK_1973 =
  'J. B. Pollack, O. B. Toon and B. N. Khare, "Optical properties of some terrestrial rocks and glasses", Icarus 19 (1973) 372–389, DOI 10.1016/0019-1035(73)90115-2';

const LI_1976 =
  'H. H. Li, "Refractive index of alkali halides and its wavelength and temperature derivatives", J. Phys. Chem. Ref. Data 5 (1976) 329–528, DOI 10.1063/1.555536';

/**
 * How far Ar ice's held n may run high, as its `reduction` states it: a Cauchy line n = A + B ÷ λ²,
 * fitted by least squares to Sinnock and Smith's nine rows at 80 K (A = 1.265675, B = 0.0015164
 * µm², residuals under 7 × 10⁻⁵), gives 1.26817 at 780 nm, 0.00113 under the held 1.2693 (computed
 * once from the source; the source is not committed).
 */
const ARGON_HOLD_TEXT =
  "a Cauchy line fitted by least squares to the source's nine rows (residuals under 7 × 10⁻⁵) gives 1.2682 at 780 nm, so the hold runs at most 0.0011 high";

/** Irwin et al. 2022's two planets' Aerosol-3 "Mean 1" k at two of their rows (their tables). */
const IRWIN_PLANETS_TEXT =
  "Uranus 1.50 × 10⁻³ and Neptune 1.01 × 10⁻² at 400 nm, 1.91 × 10⁻⁴ and 5.47 × 10⁻³ at 500 nm";

/**
 * A standard petrographic thin section's thickness, as {@link CATIO3_BOUND_TEXT}'s figures take it:
 * ground "until the sample is only 30 μm thick" (Wikipedia, "Thin section", revision 1323745730,
 * read {@link FETCHED_T5I}).
 */
const THIN_SECTION_SOURCE = `a standard thin section's thickness (Wikipedia, "Thin section", ${fetchedSource("revision 1323745730", "https://en.wikipedia.org/w/index.php?title=Thin_section&oldid=1323745730", "66633dded2f05cad0e37e0e2531a082b8dd19f7e90371e426289b5c4decd51b4", FETCHED_T5I)})`;

/**
 * `calcium-titanate.json`'s record that its visible k is an upper bound, not a measurement of
 * absorption (R08.T5.i; decision-p14-t49c-split §5.3): the source's method, what reflectance can
 * see, the band gap and the mineral in transmitted light. The figures marked computed are from the
 * file's own index at 450 and 550 nm (the tool's test recomputes them).
 */
const CATIO3_BOUND_TEXT = `Its visible k is not a measurement of absorption (R08.T5.i, decision-p14-t49c-split §5.3), and the file keeps it as an upper bound on the pure crystal's sub-gap k, a stated stand-in. Ueda et al.'s abstract: "Normal incidence light reflection measurements have been performed for single crystals over the wide energy region from 0.4 to 35 eV. The dielectric functions and optical constants are evaluated from the reflectance spectrum by the Kramers-Krönig transformation technique" (${fetchedSource("through OpenAlex", "https://api.openalex.org/works/doi:10.1088/0953-8984/10/16/018", "30d02af069bb67a91cd07186919db44614faaa4c42db12bda711ff61f8523a3d", FETCHED_T5I)}; the article is closed). At 550 nm, n 2.33245, the normal reflectance is 0.1598728 with k = 0 and 0.1598730 with the file's k of 1.81089 × 10⁻³, a change of 2.5 × 10⁻⁷ (computed): reflectance cannot see a k this small, so the measurement does not constrain the sub-gap k, which comes from the transform, as Kitzmann and Heng read it from Ueda et al.'s figure (their Table 1, "Data available as figure"; ${fetchedSource("the arXiv copy, arXiv:1710.04946v2", "https://arxiv.org/pdf/1710.04946v2", "ba36c5a2cc6ee4dc71fbf11d461301324f1e144d3700d72f61d1ebb8e83723ec", FETCHED_T5I)}). 380–780 nm is 3.26–1.59 eV, below the compiled crystal's absorption edge near 3.65 eV (its k is 0.027 at 3.62 eV and 0.058 at 3.67 eV, the compilation's rows) and below the band gap of 3.8–4.38 eV that Krause, Weber, Pohl, Rellinghaus, Kersch and Mikolajick 2015 measure in ultrathin layers by low-energy EELS (electron energy loss spectroscopy) ("Investigation of band gap and permittivity of the perovskite CaTiO3 in ultrathin layers", J. Phys. D 48, 415304, DOI 10.1088/0022-3727/48/41/415304; their abstract, ${fetchedSource("through OpenAlex", "https://api.openalex.org/works/doi:10.1088/0022-3727/48/41/415304", "23e4cb27986c8707e63d04a1f403ce90b9b6ab5440f9b41f52bad43bd2cb68e4", FETCHED_T5I)}; the article is closed, so their DUV–VIS ellipsometry was not read). The Handbook of Mineralogy's perovskite sheet (J. W. Anthony, R. A. Bideaux, K. W. Bladh and M. C. Nichols, Mineral Data Publishing, ${fetchedSource("version 1", "https://www.handbookofmineralogy.org/pdfs/perovskite.pdf", "c8f43952ab901561cf0a922bc47b17579ee2cdc9d80188ccf2c7d1065c6c62a6", FETCHED_T5I)}) describes the natural mineral, whose Vogtsberg analysis there carries 25.99% Nb2O5, 5.69% FeO and 3.08% RE2O3, as "Opaque, transparent in thin fragments", "colorless to dark brown in transmitted light", with a "White to grayish white" streak and n = 2.34–2.37. At the file's k a 30 µm section, ${THIN_SECTION_SOURCE}, would transmit 2.1% at 450 nm and 29% at 550 nm before reflection losses, absorption lengths of 7.8 and 24 µm (computed): a section colourless in transmitted light is not possible at these values, even of the impure mineral, so the file's k lies above the crystal's absorption, and a colourless crystal would draw brighter. The k is kept rather than set to 0, since no measurement says the absorption is nil, and a changed k would need the phase table remade; a transmission measurement, if one is found, replaces it. R08.T5.i's search (${FETCHED_T5I}) found none that could be read: A. Linz and K. Herrington, "Electrical and Optical Properties of Synthetic Calcium Titanate Crystal", J. Chem. Phys. 28 (1958) 824–825, DOI 10.1063/1.1744278, whose abstract reads "The optical density of a flame fusion single crystal plate of CaTiO3 is reported. The refractive indices for the visible region are given, with nD = 2.403±0.02" (${fetchedSource("through Crossref", "https://api.crossref.org/works/10.1063/1.1744278", "da8e8cc24a8adf62dbaefcf9b302a5b12111e351d6c87872a298dd81ac5cf81c", FETCHED_T5I)}); the article is closed and its values were not read, so it is the lead for that measurement`;

/**
 * `sodium-sulphide.json`'s record that its visible k bounds the crystal's from above (R08.T5.i;
 * decision-p14-t49c-split §5.3). The figures marked computed are from the file's own index (the
 * tool's test recomputes them).
 */
const NA2S_BOUND_TEXT = `Its k, 1.46 × 10⁻² at 550 nm and 1.26–1.27 × 10⁻² over 600–780 nm with no feature, is not a colourless solid's absorption, so the file bounds the crystal's visible k from above (R08.T5.i, decision-p14-t49c-split §5.3). Its cause is not established: the ruling reads it as a computed spectrum's broadening tail or too small a computed gap, but Kitzmann and Heng took Khachai et al.'s values from a figure (their Table 1, "Data available as figure"; ${fetchedSource("the arXiv copy, arXiv:1710.04946v2", "https://arxiv.org/pdf/1710.04946v2", "ba36c5a2cc6ee4dc71fbf11d461301324f1e144d3700d72f61d1ebb8e83723ec", FETCHED_T5I)}), and the compilation's k stays near 1.25 × 10⁻² from 0.56 to 1.6 µm and is 8.6 × 10⁻³ at 20 µm, flat where a tail below a gap would fall, so a floor from the digitising is as likely. PubChem's Hazardous Substances Data Bank entries for sodium sulfide (CID 14804, "Color/Form", ${fetchedSource("PubChem's record", "https://pubchem.ncbi.nlm.nih.gov/rest/pug_view/data/compound/14804/JSON?heading=Color/Form", "be262d6b1122cc6f6eb45de0a7a702fbce68c931cab62497d0357819e4b26415", FETCHED_T5I)}) give "CLEAR CRYSTALS; YELLOW OR BRICK-RED LUMPS OR FLAKES" (the National Fire Protection Association's Fire Protection Guide on Hazardous Materials, 7th ed., 1978), "AMORPHOUS, YELLOW-PINK OR WHITE CRYSTALS" (Sax 1979) and "White to pink, crystalline solid" (Ashford 1994), and state no cause of the coloured forms. At the file's k a 30 µm crystal would transmit 4.4 × 10⁻⁵ at 550 nm, an absorption length of 3.0 µm (computed): not clear. R08.T5.i's search (${FETCHED_T5I}) found no measured visible constants of Na2S, as R08.T5.d's did not; Walker 1979 ("Crystal growth of sodium sulphide, Na2S", J. Cryst. Growth 47, 598, DOI 10.1016/0022-0248(79)90146-5; ${fetchedSource("OpenAlex's record", "https://api.openalex.org/works/doi:10.1016/0022-0248(79)90146-5", "fd28561691a2125a61393da08e570208147ad1653a5ed693a84352ea0f1826e5", FETCHED_T5I)}) is closed, its title alone reachable`;

/** Every material file reduced from a fetched source, in the registry's order (a key's default first). */
export const FETCHED_MATERIALS: ReadonlyArray<FetchedSpec> = [
  {
    file: "water-ice.json",
    option: "ice",
    sha256: "a8577eaa1fabb9245957397344ce23efb20dd4c57cd7f442710d2d2373e0bbd6",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "H2O",
      phase: "solid",
      variant: null,
      name: "H2O ice Ih at 266 K",
      temperatureK: 266,
      shape: "crystal",
      provenance: "measured",
      paper:
        'S. G. Warren and R. E. Brandt, "Optical constants of ice from the ultraviolet to the microwave: A revised compilation", J. Geophys. Res. 113 (2008) D14220, DOI 10.1029/2007JD009744',
      source: fetchedSource(
        "refractiveindex.info, main/H2O/nk/Warren-2008.yml",
        `${RI_URL}/main/H2O/nk/Warren-2008.yml`,
        "a8577eaa1fabb9245957397344ce23efb20dd4c57cd7f442710d2d2373e0bbd6",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; the source's rows are every 10 nm from 390 nm`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "ammonia-ice.json",
    option: "ammonia",
    sha256: "14a5b66bb35101a89cacb07cd80585413ee4aa8986d966138d866ffd4fb9babd",
    parse: parseLnk,
    resample: { holdNm: 0 },
    header: {
      key: "NH3",
      phase: "solid",
      variant: null,
      name: "crystalline NH3 ice (its absorption not drawn)",
      temperatureK: null,
      shape: "crystal",
      provenance: "standIn",
      paper:
        'J. V. Martonchik, G. S. Orton and J. F. Appleby, "Optical properties of NH3 ice from the far infrared to the near ultraviolet", Appl. Opt. 23 (1984) 541–547, DOI 10.1364/AO.23.000541, for n, and for the k rows between its 1,462 and 200 nm rows, which this file shows to be interpolations and does not draw',
      source: `${fetchedSource(
        "optool, lnk_data/nh3-m-Martonchik1983.lnk",
        `${OPTOOL_URL}/nh3-m-Martonchik1983.lnk`,
        "14a5b66bb35101a89cacb07cd80585413ee4aa8986d966138d866ffd4fb9babd",
      )}. ${NH3_ICE_SEARCH_TEXT}`,
      licence: OPTOOL_TERMS,
      reduction: ammoniaIceReductionText(),
      standIn:
        "a crystal whose visible absorption is not drawn: Martonchik et al.'s visible k is a fill between their 1462 nm and 320 nm values, not a measurement, and no measured visible absorption of NH3 ice was found, so k = 0, an upper bound on its brightness; its real index is theirs",
      fit: null,
    },
    adjust: withoutVisibleAbsorption,
  },
  {
    file: "methane-liquid.json",
    option: "methane-liquid",
    sha256: "75b1cb8ae81a6afed2c597b524bcc16f89492eb9fba4b744813ce5cde9600d67",
    parse: parseRefractiveIndexInfo,
    resample: { ignoreBelowUm: 0.4, holdNm: 20 },
    header: {
      key: "CH4",
      phase: "liquid",
      variant: null,
      name: "liquid CH4 at 90 K",
      temperatureK: 90,
      shape: "sphere",
      provenance: "measured",
      paper:
        'J. V. Martonchik and G. S. Orton, "Optical constants of liquid and solid methane", Appl. Opt. 33 (1994) 8306–8317, DOI 10.1364/AO.33.008306',
      source: fetchedSource(
        "refractiveindex.info, organic/CH4 - methane/nk/Martonchik-liquid-90K.yml",
        `${RI_URL}/organic/CH4%20-%20methane/nk/Martonchik-liquid-90K.yml`,
        "75b1cb8ae81a6afed2c597b524bcc16f89492eb9fba4b744813ce5cde9600d67",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}. The source has no row between 133.7 and 400 nm, across methane's ultraviolet absorption edge, so its rows below 400 nm are not used and its 400 nm values (n 1.300, k 0) are held over 380–400 nm. Its visible k rows sit at band maxima and minima (the database's caution), so k between them is indicative`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "methane-ice.json",
    option: "methane-ice",
    sha256: "6fb859140db36f1039d5c64aafb7cb4d16de7db18b37a9f414f9c061193acfcd",
    parse: parseRefractiveIndexInfo,
    resample: { ignoreBelowUm: 0.4, holdNm: 20 },
    header: {
      key: "CH4",
      phase: "solid",
      variant: null,
      name: "phase I CH4 ice at 90 K",
      temperatureK: 90,
      shape: "crystal",
      provenance: "measured",
      paper:
        'J. V. Martonchik and G. S. Orton, "Optical constants of liquid and solid methane", Appl. Opt. 33 (1994) 8306–8317, DOI 10.1364/AO.33.008306',
      source: fetchedSource(
        "refractiveindex.info, organic/CH4 - methane/nk/Martonchik-solid-90K.yml",
        `${RI_URL}/organic/CH4%20-%20methane/nk/Martonchik-solid-90K.yml`,
        "6fb859140db36f1039d5c64aafb7cb4d16de7db18b37a9f414f9c061193acfcd",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}. The source's visible rows are 400 nm and 1,000 nm, both with k 0, and none between 133.7 and 400 nm, so its rows below 400 nm are not used and its 400 nm values (n 1.326, k 0) are held over 380–400 nm. Its k of 0 is a gap in the source, not a measurement: the liquid shows bands of k about 10⁻⁷ at 620–730 nm, which for ice grains of 10 µm would move 1 − ω by about 10⁻⁵`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "sulphuric-acid-75.json",
    option: "h2so4-75",
    sha256: "ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe",
    parse: parseAria,
    resample: { holdNm: 0, unmeasuredK: "shortestMeasured", grid: ACID_GRID_NM },
    header: {
      key: "H2SO4",
      phase: "liquid",
      variant: "75wt%",
      name: "aqueous H2SO4, mass fraction 75 %, at 300 K",
      temperatureK: 300,
      shape: "sphere",
      provenance: "measured",
      paper:
        'K. F. Palmer and D. Williams, "Optical constants of sulfuric acid; application to the clouds of Venus?", Appl. Opt. 14 (1975) 208–219, DOI 10.1364/AO.14.000208',
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, H2SO4_75%_300K_R_Palmer_1975.ri",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Acids/Sulphuric/70%25_to_79%25/Sulphuric_acid_75%25_300K_(Palmer_and_Williams_1975)/original/H2SO4_75%25_300K_R_Palmer_1975.ri",
        "ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe",
      ),
      licence: NO_TERMS,
      reduction: `${standardReductionOnto(ACID_GRID_NM)}: the grid starts at 360 nm, below the other files' 380 nm, so that Venus's ultraviolet fit can read this file at 365 nm (decision-r08-t5d ruling 1), and its 360–375 nm rows come from the same source rows as the rest. In the visible the source gives n only, at 359.7, 408.2, 449.4, 555.6 and 701.8 nm, with k NaN short of 701.8 nm; there k is held at its shortest measured value, 2.07 × 10⁻⁸ at 701.8 nm. The acid's visible absorption lies below the source's measurement, and at this k a Venus mode-2 droplet's 1 − ω is about 10⁻⁶. n at 555.6 nm, 1.431, lies inside Hansen and Hovenier 1974's 1.44 ± 0.015 at 550 nm (J. Atmos. Sci. 31, 1137)`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "sulphuric-acid-84.json",
    option: "h2so4-84",
    sha256: "6ae3eceb2c3895ec367f9b86e8e082445f1473413d224c2b625d85baaf452dc3",
    parse: parseAria,
    resample: { holdNm: 0, unmeasuredK: "shortestMeasured" },
    header: {
      key: "H2SO4",
      phase: "liquid",
      variant: "84.5wt%",
      name: "aqueous H2SO4, mass fraction 84.5 %, at 300 K",
      temperatureK: 300,
      shape: "sphere",
      provenance: "measured",
      paper:
        'K. F. Palmer and D. Williams, "Optical constants of sulfuric acid; application to the clouds of Venus?", Appl. Opt. 14 (1975) 208–219, DOI 10.1364/AO.14.000208',
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, H2SO4_84.5%_300K_R_Palmer_1975.ri",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Acids/Sulphuric/80%25_to_100%25/Sulphuric_acid_84.5%25_300K_(Palmer_and_Williams_1975)/original/H2SO4_84.5%25_300K_R_Palmer_1975.ri",
        "6ae3eceb2c3895ec367f9b86e8e082445f1473413d224c2b625d85baaf452dc3",
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}. In the visible the source gives n only, at 359.7, 408.2, 449.4, 555.6 and 701.8 nm, with k NaN short of 714.3 nm; there k is held at its shortest measured value, 1.14 × 10⁻⁸ at 714.3 nm, the acid's visible absorption lying below the source's measurement. n at 555.6 nm, 1.438, lies inside Hansen and Hovenier 1974's 1.44 ± 0.015 at 550 nm`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "iron.json",
    option: "iron",
    sha256: "8d774beaac808662370242790c18b9998a9856ba850d9f35c45f6db3da31a7cd",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "Fe",
      phase: "solid",
      variant: null,
      name: "metallic Fe films at room temperature",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'P. B. Johnson and R. W. Christy, "Optical constants of transition metals: Ti, V, Cr, Mn, Fe, Co, Ni, and Pd", Phys. Rev. B 9 (1974) 5056–5070, DOI 10.1103/PhysRevB.9.5056',
      source: fetchedSource(
        "refractiveindex.info, main/Fe/nk/Johnson.yml",
        `${RI_URL}/main/Fe/nk/Johnson.yml`,
        "8d774beaac808662370242790c18b9998a9856ba850d9f35c45f6db3da31a7cd",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; room-temperature solid iron, so a liquid iron cloud takes it as a named analogue (a stand-in), drawn as spheres, while solid grains are non-spherical minerals (science-r08-nonspherical.md §2.2); 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "soot.json",
    option: "soot",
    sha256: "3fb9dd0dfdb19eea4b0fa52c1076df5e9fa0be9473ad2f456c5114c572b7a256",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 56 },
    header: {
      key: "soot",
      phase: "solid",
      variant: null,
      name: "soot from a C3H8 flame at room temperature",
      temperatureK: 295,
      shape: "sphere",
      provenance: "measured",
      paper:
        'W. H. Dalzell and A. F. Sarofim, "Optical constants of soot and their application to heat-flux calculations", J. Heat Transfer 91 (1969) 100–104, DOI 10.1115/1.3580063',
      source: fetchedSource(
        "refractiveindex.info, other/soots/propane soot/nk/Dalzell.yml",
        `${RI_URL}/other/soots/propane%20soot/nk/Dalzell.yml`,
        "3fb9dd0dfdb19eea4b0fa52c1076df5e9fa0be9473ad2f456c5114c572b7a256",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}. The source's visible rows are 435.8, 450, 550, 650 and 806.5 nm, so its 435.8 nm values (n 1.57, k 0.46) are held over 380–435.8 nm; soot is nearly grey across the visible (n 1.56–1.57, k 0.46–0.53 in the source); 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "tholin.json",
    option: "tholin",
    sha256: "8cef815bfcf19123b27dc061bc3a92377a5ad2440d9ba3a0823d19427e0198b6",
    parse: parseHitranTable,
    resample: { holdNm: 0 },
    header: {
      key: "tholin",
      phase: "solid",
      variant: null,
      name: "Titan tholin, a laboratory haze analogue from a simulated Titan atmosphere",
      temperatureK: 295,
      shape: "sphere",
      provenance: "measured",
      paper:
        'B. N. Khare, C. Sagan, E. T. Arakawa, F. Suits, T. A. Callcott and M. W. Williams, "Optical constants of organic tholins produced in a simulated Titanian atmosphere: From soft x-ray to microwave frequencies", Icarus 60 (1984) 127–137, DOI 10.1016/0019-1035(84)90142-8',
      source: `HITRAN2024's aerosol compilation (I. E. Gordon et al., J. Quant. Spectrosc. Radiat. Transfer 353 (2026) 109807), https://hitran.org/data/Aerosols/Aerosols-2024/hitran_ri.tar, SHA-256 ${HITRAN_TAR_SHA256}, its file hitran_ri/ascii/exoplanets/khare_tholins.dat, SHA-256 8cef815bfcf19123b27dc061bc3a92377a5ad2440d9ba3a0823d19427e0198b6, fetched ${FETCHED}; only this ASCII file was read, and none of the archive's code was run`,
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows here are 354.2, 387.4, 413.3, 442.8, 563.5, 688.8 and 873.1 nm. Khare et al. 1984 is taken over He et al. 2022 (Planet. Sci. J. 3, 25; CC BY 4.0), whose data begin at 400 nm and so do not cover 380–400 nm; 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "mars-dust.json",
    option: "mars-dust",
    sha256: "bf6069ad22fbfbbef7bf957d386a8177647c87fd98ed6574b5c5f8d6c8016769",
    parse: parseWavelengthTable,
    resample: { holdNm: 0 },
    header: {
      key: "mars_dust",
      phase: "solid",
      variant: null,
      name: "Mars-type dust from Mars's atmosphere, indices retrieved from Mars Reconnaissance Orbiter spectrometer and Mars Exploration Rover observations",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'M. J. Wolff et al., "Wavelength dependence of dust aerosol single scattering albedo as observed by the Compact Reconnaissance Imaging Spectrometer", J. Geophys. Res. 114 (2009) E00D04, DOI 10.1029/2009JE003350',
      source: fetchedSource(
        "NASA Ames Mars Climate Modeling Group's Dust_Refractive_Indicies.txt, as kept in S. Ranjan's ranjanwordsworthsasselov2017b repository (commit 1e67a481; its MIT licence covers its code, not these data)",
        "https://raw.githubusercontent.com/sukritranjan/ranjanwordsworthsasselov2017b/1e67a4819255ff907df2a97eb8cecd9eee32c928/Raw_Data/ComplexRefractionIndices/Dust_Refractive_Indicies.txt",
        "bf6069ad22fbfbbef7bf957d386a8177647c87fd98ed6574b5c5f8d6c8016769",
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows used are 321, 440, 460, 500, 600, 630, 700 and 800 nm, so 380–440 nm is interpolated between its 321 and 440 nm rows, the 321 nm row being the anchor there (science-r08-nonspherical.md §3.2, ruled 2026-10-10). The indices are retrieved, not laboratory: Wolff et al. fit them to observed single-scattering albedos with non-spherical (T-matrix cylinder) particles, so Mie gives this dust's cross-sections only (Design note 6). The source's two ultraviolet rows, 263 and 321 nm, lie outside Wolff et al. 2009's CRISM range (440–2,920 nm): they are part of Wolff's distributed compilation but separately retrieved (k printed to three figures, where the CRISM rows have six), presumably Wolff et al. 2010's MARCI values (Icarus 208, 143), unconfirmed, since that paper is closed. The 263 nm row lies outside the grid and is not used. The interpolated k at 380 nm, 0.00989, lies between holding the 440 nm row (0.00767), which is wrong because Mars dust's k rises towards the ultraviolet, and extrapolating the 440–500 nm rows' log-slope (0.0107)`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "enstatite-glass.json",
    option: "enstatite",
    sha256: "9f07f23cb0b233fd48210824c38afe031fd5ad84bd0b182b7fe290caa1fe0de8",
    parse: parseLnk,
    resample: { holdNm: 0 },
    header: {
      key: "MgSiO3",
      phase: "solid",
      variant: null,
      name: "amorphous MgSiO3",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'J. Dorschner, B. Begemann, Th. Henning, C. Jäger and H. Mutschke, "Steps toward interstellar silicate mineralogy. II. Study of Mg-Fe-silicate glasses of variable composition", Astron. Astrophys. 300 (1995) 503–520',
      source: fetchedSource(
        "optool, lnk_data/pyr-mg100-Dorschner1995.lnk",
        `${OPTOOL_URL}/pyr-mg100-Dorschner1995.lnk`,
        "9f07f23cb0b233fd48210824c38afe031fd5ad84bd0b182b7fe290caa1fe0de8",
      ),
      licence: OPTOOL_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are every 20–50 nm here, and its placeholder k of 10⁻⁶ below 320 nm lies outside the grid; 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "forsterite-amorphous.json",
    option: "forsterite",
    sha256: "2582d4ab3cd9f7c9f8e53ae9988bd71a13c7080ba68f858bdf60133536c2fa32",
    parse: parseHitranTable,
    resample: { holdNm: 0 },
    header: {
      key: "Mg2SiO4",
      phase: "solid",
      variant: null,
      name: "amorphous Mg2SiO4 made by the sol-gel method",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'C. Jäger, J. Dorschner, H. Mutschke, Th. Posch and Th. Henning, "Steps toward interstellar silicate mineralogy. VII. Spectral properties and crystallization behaviour of magnesium silicates produced by the sol-gel method", Astron. Astrophys. 408 (2003) 193–204, DOI 10.1051/0004-6361:20030916',
      source: `HITRAN2024's aerosol compilation (I. E. Gordon et al., J. Quant. Spectrosc. Radiat. Transfer 353 (2026) 109807), https://hitran.org/data/Aerosols/Aerosols-2024/hitran_ri.tar, SHA-256 ${HITRAN_TAR_SHA256}, its file hitran_ri/ascii/exoplanets/jager_mg2sio4.dat, SHA-256 2582d4ab3cd9f7c9f8e53ae9988bd71a13c7080ba68f858bdf60133536c2fa32, fetched ${FETCHED}; only this ASCII file was read, and none of the archive's code was run`,
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are about 0.1 nm apart, and its k is printed to four decimal places (10⁻⁴), so k here is that coarse. Dorschner et al. 1995 has no Mg-pure olivine glass, so this Mg2SiO4 is Jäger et al.'s, as Kitzmann and Heng 2018's compilation also takes it (MNRAS 475, 94); 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  // R08.T5.d's files, after T5.b's (decision-composition; decision-r08-t5d).
  {
    file: "chromium.json",
    option: "chromium",
    sha256: "756f5c6be6c19c46dc3fb1580db069aafef1bb3b914293e141d7fc4b738ee5e4",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "Cr",
      phase: "solid",
      variant: null,
      name: "metallic Cr films at room temperature",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'P. B. Johnson and R. W. Christy, "Optical constants of transition metals: Ti, V, Cr, Mn, Fe, Co, Ni, and Pd", Phys. Rev. B 9 (1974) 5056–5070, DOI 10.1103/PhysRevB.9.5056',
      source: fetchedSource(
        "refractiveindex.info, main/Cr/nk/Johnson.yml",
        `${RI_URL}/main/Cr/nk/Johnson.yml`,
        "756f5c6be6c19c46dc3fb1580db069aafef1bb3b914293e141d7fc4b738ee5e4",
        FETCHED_T5D,
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; the source's rows here are 368–821 nm, 13–65 nm apart. Johnson and Christy's films, iron's source too, are taken over Rakić et al. 1998's model and Lynch and Hunter's tables in Palik's handbook (Kitzmann and Heng 2018's choice), whose k passes the longwave kernel's 4; 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "iron-sulphide.json",
    option: "iron-sulphide",
    sha256: "ae2ba0daab33afc92d2d7590b1712cf7c09a35f8465915b19ab501396c4d60ea",
    parse: parseKitzmannHeng,
    resample: { holdNm: 0 },
    header: {
      key: "FeS",
      phase: "solid",
      variant: null,
      name: "solid FeS",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: `J. B. Pollack, D. Hollenbach, S. Beckwith, D. P. Simonelli, T. Roush and W. Fong, "Composition and radiative properties of grains in molecular clouds and accretion disks", Astrophys. J. 421 (1994) 615, DOI 10.1086/173677, for the visible; Th. Henning and H. Mutschke, Astron. Astrophys. 327 (1997) 743, for the infrared; as compiled by ${KITZMANN_HENG}`,
      source: compilationSource(
        "FeS.dat",
        "ae2ba0daab33afc92d2d7590b1712cf7c09a35f8465915b19ab501396c4d60ea",
      ),
      licence: COMPILATION_TERMS,
      reduction: `${STANDARD_REDUCTION}. The source's rows near the visible are 300, 460, 610 and 1,000 nm, so 380–780 nm is interpolated across them, as coarse as the source; the visible primary, Pollack et al. 1994, was not found in another machine-readable copy (the Jena database's FeS is infrared only). The copy states no temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "calcium-titanate.json",
    option: "calcium-titanate",
    sha256: "2d4cb5db2740c70dd3c2122d58062f04fb1e245675dfe8254919b801a1f8f2e1",
    parse: parseKitzmannHeng,
    resample: { holdNm: 0 },
    header: {
      key: "CaTiO3",
      phase: "solid",
      variant: null,
      name: "crystalline CaTiO3 (an upper bound on its absorption)",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "standIn",
      paper: `K. Ueda, H. Yanagi, R. Noshiro, H. Hosono and H. Kawazoe, J. Phys.: Condens. Matter 10 (1998) 3669–3677, DOI 10.1088/0953-8984/10/16/018, for the visible; Th. Posch, F. Kerschbaum, D. Fabian, H. Mutschke, J. Dorschner, A. Tamanai and Th. Henning, "Infrared properties of solid titanium oxides: Exploring potential primary dust condensates", Astrophys. J. Suppl. 149 (2003) 437–445, DOI 10.1086/379167, for the infrared; as compiled by ${KITZMANN_HENG}`,
      source: compilationSource(
        "CaTiO3.dat",
        "2d4cb5db2740c70dd3c2122d58062f04fb1e245675dfe8254919b801a1f8f2e1",
      ),
      licence: COMPILATION_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are about 3.6–7.7 nm apart here. No open copy of Ueda et al.'s visible values was found. The copy states no temperature. ${CATIO3_BOUND_TEXT}`,
      standIn:
        "an upper bound on absorption: Ueda et al.'s visible k is a Kramers–Kronig value from normal-incidence reflectance, which cannot detect a k this small, and thin sections of the mineral can be colourless, so the pure crystal absorbs less",
      fit: null,
    },
  },
  {
    file: "zinc-sulphide.json",
    option: "zinc-sulphide",
    sha256: "32607bcb78368bfcbb38a6bef88b03b88277b8644a434dc4385f034bd5c04efa",
    parse: parseKitzmannHeng,
    resample: { holdNm: 0 },
    header: {
      key: "ZnS",
      phase: "solid",
      variant: null,
      name: "cubic ZnS",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: `E. D. Palik and A. Addamiano, "Zinc sulfide (ZnS)", in E. D. Palik (ed.), Handbook of Optical Constants of Solids, Academic Press (1985); as compiled by ${KITZMANN_HENG}`,
      source: compilationSource(
        "ZnS.dat",
        "32607bcb78368bfcbb38a6bef88b03b88277b8644a434dc4385f034bd5c04efa",
      ),
      licence: COMPILATION_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are 354, 382, 413 and 451 nm, then about 8–25 nm apart, and its k falls from 0.04 at 413 nm to 7.4 × 10⁻⁶ at 451 nm, across which it is interpolated in log k. Palik's handbook is closed, so its values are read through the compilation (decision-r08-t5d ruling 5). Its k of 0.04–0.06 at 382–413 nm lies longward of cubic ZnS's band gap, near 340 nm, where Debenham 1984 measured the crystal's n (Appl. Opt. 23, 2238), so it is suspect, perhaps a seam between two data sets; kept as the compilation gives it and recorded. The copy states no temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "manganese-sulphide.json",
    option: "manganese-sulphide",
    sha256: "97d7203885af8950fc567089bcd4e7845c716a347335f404afcc9c5488f8ab91",
    parse: parseKitzmannHeng,
    resample: { holdNm: 0 },
    header: {
      key: "MnS",
      phase: "solid",
      variant: null,
      name: "crystalline MnS",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: `D. R. Huffman and R. L. Wild, "Optical properties of α-MnS", Phys. Rev. 156 (1967) 989–997, DOI 10.1103/PhysRev.156.989, for the visible; R. Montaner et al., Phys. Status Solidi A 52 (1979) 597, for the infrared; as compiled by ${KITZMANN_HENG}`,
      source: compilationSource(
        "MnS.dat",
        "97d7203885af8950fc567089bcd4e7845c716a347335f404afcc9c5488f8ab91",
      ),
      licence: COMPILATION_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are about 7–39 nm apart here, then 600, 700 and 800 nm. Huffman and Wild's paper is closed, so its values are read through the compilation (decision-r08-t5d ruling 5). n is 3.51 at 380 nm, past the longwave kernel's 3.2 up to about 427 nm, where its phase table holds n at the kernel's edge (decision-r08-t5d ruling 2). The copy states no temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "sodium-sulphide.json",
    option: "sodium-sulphide",
    sha256: "1cc0d352d00273decac51c14bd1dd5cc7ad13c982af35d8a3705358b1e02aa27",
    parse: parseKitzmannHeng,
    resample: { holdNm: 0 },
    header: {
      key: "Na2S",
      phase: "solid",
      variant: null,
      name: "crystalline Na2S (a computed index)",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "standIn",
      paper: `H. Khachai, R. Khenata, A. Bouhemadou, A. Haddou, A. H. Reshak, B. Amrani, D. Rached and B. Soudini, "FP-APW+lo calculations of the electronic and optical properties of alkali metal sulfides under pressure", J. Phys.: Condens. Matter 21 (2009) 095404, DOI 10.1088/0953-8984/21/9/095404, for the visible; R. Montaner et al., Phys. Status Solidi A 52 (1979) 597, for the infrared; as compiled by ${KITZMANN_HENG}`,
      source: compilationSource(
        "Na2S.dat",
        "1cc0d352d00273decac51c14bd1dd5cc7ad13c982af35d8a3705358b1e02aa27",
      ),
      licence: COMPILATION_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are about 3.5–8 nm apart here. Its visible part is Khachai et al.'s first-principles (FP-APW+lo) calculation, not a measurement, so the file is a stated stand-in. The copy states no temperature. ${NA2S_BOUND_TEXT}`,
      standIn:
        "a computed index: Khachai et al. 2009's first-principles calculation of crystalline Na2S, since no measured visible optical constants of Na2S were found; its flat visible k is a computed spectrum's, not a colourless solid's absorption, so it is an upper bound on the crystal's absorption",
      fit: null,
    },
  },
  {
    file: "titanium-dioxide.json",
    option: "titanium-dioxide",
    sha256: "9b03947949e54cd98f58a989bd031a5fddb1a36a9d9f7fd64d5ff2bd68c2885c",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "TiO2",
      phase: "solid",
      variant: null,
      name: "TiO2 film grown by atomic layer deposition",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'T. Siefke, S. Kroker, K. Pfeiffer, O. Puffky, K. Dietrich, D. Franta, I. Ohlídal, A. Szeghalmi, E.-B. Kley and A. Tünnermann, "Materials pushing the application limits of wire grid polarizers further into the deep ultraviolet spectral range", Adv. Opt. Mater. 4 (2016) 1780–1786, DOI 10.1002/adom.201600250',
      source: fetchedSource(
        "refractiveindex.info, main/TiO2/nk/Siefke.yml (numerical data provided by the first author, as the database states)",
        `${RI_URL}/main/TiO2/nk/Siefke.yml`,
        "9b03947949e54cd98f58a989bd031a5fddb1a36a9d9f7fd64d5ff2bd68c2885c",
        FETCHED_T5D,
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; the source's rows are about 2–4 nm apart here, from a dispersion model fitted to ellipsometry and spectrophotometry of a 350 nm film, the visible source Kitzmann and Heng 2018 take. Rutile is recorded as the alternative: DeVore 1951's two rays (n only, from 430 nm, so not covering 380–430 nm), averaged for random orientation, give 2.753 at 550 nm against this film's 2.436, a normal-incidence reflectance of 0.218 against 0.175. The source states no temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "argon-ice.json",
    option: "argon",
    sha256: "fda451e35283fe0151001cde90cd7ff4738eb2ae268dc3e0a72c396d02ca3fad",
    parse: parseRefractiveIndexInfoN,
    resample: { holdNm: 137 },
    header: {
      key: "Ar",
      phase: "solid",
      variant: null,
      name: "solid Ar at 80 K",
      temperatureK: 80,
      shape: "crystal",
      provenance: "measured",
      paper:
        'A. C. Sinnock and B. L. Smith, "Refractive indices of the condensed inert gases", Phys. Rev. 181 (1969) 1297–1307, DOI 10.1103/PhysRev.181.1297',
      source: fetchedSource(
        "refractiveindex.info, main/Ar/nk/Sinnock-solid-80K.yml",
        `${RI_URL}/main/Ar/nk/Sinnock-solid-80K.yml`,
        "fda451e35283fe0151001cde90cd7ff4738eb2ae268dc3e0a72c396d02ca3fad",
        FETCHED_T5D,
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}, with k = 0 stated (the source gives n only). The source's rows are 361.2–643.9 nm, so its 643.9 nm value, n 1.2693, is held over 643.9–780 nm; ${ARGON_HOLD_TEXT}. 80 K, as ruled (decision-r08-t5d ruling 3), lies near Ar's triple point (83.8 K), where Ar saturates in a bar-class atmosphere; the source's table at 83.81 K lies 0.0020 lower at 546.1 nm and its 40 K table about 0.016 higher, and Ar's temperature dependence is not modelled. Grace et al. 2017's Sellmeier fit, which also ends short (660 nm), is not taken`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "jovian-chromophore.json",
    option: "chromophore",
    sha256: "96b3dbd706917bd2fd4a75f79c855dd2c9f4060da07ad324938617733390a66c",
    parse: parseBraudeChromophore,
    resample: { holdNm: 70, endK: "extrapolate" },
    header: {
      key: "chromophore",
      phase: "solid",
      variant: null,
      name: "Jovian chromophore, an effective index retrieved from Jupiter's spectrum",
      temperatureK: null,
      shape: "sphere",
      provenance: "standIn",
      paper:
        'A. S. Braude, P. G. J. Irwin, G. S. Orton and L. N. Fletcher, "Colour and tropospheric cloud structure of Jupiter from MUSE/VLT: Retrieving a universal chromophore", Icarus 338 (2020) 113589, DOI 10.1016/j.icarus.2019.113589, its values read from the preprint, arXiv:1912.00918v1',
      source: `the preprint's source, https://arxiv.org/e-print/1912.00918v1, SHA-256 6fe0e947bda8c0f6074bb938dbe205ae7249429621e116de285b5b7f2776e790, fetched ${FETCHED_T5D}, its file jupiter_muse_paper4.tex, SHA-256 96b3dbd706917bd2fd4a75f79c855dd2c9f4060da07ad324938617733390a66c, the table "Tabulated retrieved imaginary refractive index spectra", its first k column, the adopted universal chromophore (chromophore radius 0.05 µm, cloud radius 1 µm, cloud real index 1.42); read as data`,
      licence: `Adapted from CC BY 4.0 material (${CC_BY}): A. S. Braude, P. G. J. Irwin, G. S. Orton and L. N. Fletcher, arXiv:1912.00918v1. Changed: the adopted solution's k reduced onto this grid and extrapolated below 450 nm, with n held at the paper's 1.4; this file is offered under CC BY 4.0`,
      reduction: `${STANDARD_REDUCTION}. The table gives k every 50 nm over 450–950 nm, so below 450 nm k is extrapolated linearly in log k from its 450 and 500 nm rows (0.033 and 0.023), since the chromophore absorbs more towards the blue, and n is held. n is the paper's 1.4, which its retrieval fixes at 700 nm (its §4.1); the paper does not say how n varies elsewhere (a Kramers–Kronig shape, presumably, as in the same group's retrievals), and n is 1.4 throughout`,
      standIn:
        "an effective index retrieved from Jupiter's spectrum for an unidentified colouring material (Braude et al. 2020), not a laboratory measurement",
      fit: null,
    },
  },
  {
    file: "ice-giant-haze.json",
    option: "ice-giant-haze",
    sha256: "7061326a906a8d284d40bd0d2bf0b0991e7f4a1e398de9adf6d4bd3947b8aad9",
    parse: parseIrwinAerosol3,
    resample: { holdNm: 0 },
    header: {
      key: "ice_giant_haze",
      phase: "solid",
      variant: null,
      name: "ice-giant haze, an effective index retrieved from Uranus's and Neptune's spectra",
      temperatureK: null,
      shape: "sphere",
      provenance: "standIn",
      paper:
        'P. G. J. Irwin, N. A. Teanby, L. N. Fletcher, D. Toledo, G. S. Orton, M. H. Wong, M. T. Roman, S. Pérez-Hoyos, A. James and J. Dobinson, "Hazy blue worlds: A holistic aerosol model for Uranus and Neptune, including dark spots", J. Geophys. Res. Planets 127 (2022) e2022JE007189, DOI 10.1029/2022JE007189, its values read from the preprint, arXiv:2201.04516v2',
      source: `the preprint's source, https://arxiv.org/e-print/2201.04516v2, SHA-256 9aac82980dd91c5dacc34d5552629e113cdb8e3fb7c6d39c3dd8f324fa045259, fetched ${FETCHED_T5D}, its file Neptune_Uranus_Haze.tex, SHA-256 7061326a906a8d284d40bd0d2bf0b0991e7f4a1e398de9adf6d4bd3947b8aad9, the tables "Estimated n_imag spectra for Uranus aerosols" and "… for Neptune aerosols", the Aerosol-3 (extended haze) "Mean 1" columns; read as data`,
      licence: `Adapted from CC BY 4.0 material (${CC_BY}): P. G. J. Irwin et al., J. Geophys. Res. Planets 127 (2022) e2022JE007189 (arXiv:2201.04516v2). Changed: the geometric mean of the two planets' Aerosol-3 k, reduced onto this grid, with n held at the paper's 1.4; this file is offered under CC BY 4.0`,
      reduction: `at each of the tables' wavelengths, every 100 nm over 300–1,900 nm, k is the geometric mean of Uranus's and Neptune's Aerosol-3 "Mean 1", since one key serves every ice giant; then ${STANDARD_REDUCTION}. The two planets differ by up to a factor of 29 over 300–800 nm (${IRWIN_PLANETS_TEXT}). n is the paper's 1.4, which its retrieval fixes at 800 nm and carries elsewhere by Kramers–Kronig (its §3.4 and §3.6); that shape is not reproduced, and n is 1.4 throughout`,
      standIn:
        "an effective index retrieved from Uranus's and Neptune's spectra for their unidentified photochemical haze (Irwin et al. 2022), not a laboratory measurement",
      fit: null,
    },
  },
  {
    file: "sulphuric-acid-uv-absorber.json",
    option: "h2so4-75",
    sha256: "ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe",
    parse: parseAria,
    resample: { holdNm: 0, unmeasuredK: "shortestMeasured", grid: ACID_GRID_NM },
    adjust: addAcidUvAbsorber,
    header: {
      key: "acid_uv_absorber",
      phase: "liquid",
      variant: null,
      name: "ultraviolet-absorbing H2SO4: aqueous H2SO4, mass fraction 75 %, at 300 K, with Venus's unidentified absorber added",
      temperatureK: 300,
      shape: "sphere",
      provenance: "standIn",
      paper:
        'K. F. Palmer and D. Williams, "Optical constants of sulfuric acid; application to the clouds of Venus?", Appl. Opt. 14 (1975) 208–219, DOI 10.1364/AO.14.000208, for n and the acid\'s own k; for the absorber\'s a-priori decay length, J. Spacek, P. B. Rimmer, J. J. Petkowski and Y. J. Lee, "A model of UV–blue absorbance in bulk liquid of Venusian cloud aerosols is consistent with efficient organic absorbers at high concentrations", Astrobiology 26 (2026) 727–739, DOI 10.1177/15311074261477502, its values read from the preprint, arXiv:2505.00880v3 (CC BY 4.0)',
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, H2SO4_75%_300K_R_Palmer_1975.ri, the 75 wt% file's source",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Acids/Sulphuric/70%25_to_79%25/Sulphuric_acid_75%25_300K_(Palmer_and_Williams_1975)/original/H2SO4_75%25_300K_R_Palmer_1975.ri",
        "ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe",
      ),
      licence: NO_TERMS,
      reduction: `the 75 wt% file's own reduction (${standardReductionOnto(ACID_GRID_NM)}, k held at 2.07 × 10⁻⁸ short of 701.8 nm), so its n is that file's to the bit, with k = k_H2SO4 + k_UA, rounded to six significant figures: k_UA = k₃₆₅ e^(−(λ − 365 nm) ÷ L) over 360–500 nm, times (550 − λ) ÷ 50 nm over 500–550 nm, and exactly 0 from 550 nm, so that its extinction and single-scattering albedo at 550 nm are the acid's (science-r08-unowned-absorbers; decision-r08-t5d ruling 4). k₃₆₅ = ${ACID_UV_K365} and L = ${ACID_UV_DECAY_NM} nm are the a-priori values (L ≤ 80 nm ÷ ln((1278 × 375) ÷ (54 × 455)), Spacek et al.'s decadic absorption coefficients of 1278 cm⁻¹ at 375 nm and under 54 cm⁻¹ at 455 nm taken to k = ln(10) aλ ÷ 4π, ln 10 cancelling; k₃₆₅ is the ruling's), which R08.T5.e refits to Venus's 365 nm albedo and g′ ÷ r′ colour. The grid starts at 360 nm so that the fit can read 365 nm (decision-r08-t5d ruling 1). Not modelled: the absorber's markings, its decadal and latitude changes, its 283 nm side, and any dependence on a world's chemistry`,
      standIn:
        "an empirical absorber: H2SO4 droplets with an unidentified ultraviolet absorber, whose k is fitted to Venus's albedo spectrum (candidates FeCl3, S2O/OSSO, Sx, organics)",
      fit: null,
    },
  },
  {
    file: "basalt.json",
    option: "basalt",
    sha256: "ad8730a5ce91fb05dc12374cba25ffd16af070ec003ae49cf0cb3ab134f91d2d",
    parse: parseAria,
    resample: { holdNm: 0, separateNK: true },
    header: {
      key: "basalt",
      phase: "solid",
      variant: null,
      name: "basalt, a terrestrial rock",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: POLLACK_1973,
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, basalt_R_Pollack_1973.ri (from the original paper)",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Rocks_and_Conglomerates/Basalt/Basalt_(Pollack_et_al_1973)/original/basalt_R_Pollack_1973.ri",
        "ad8730a5ce91fb05dc12374cba25ffd16af070ec003ae49cf0cb3ab134f91d2d",
        FETCHED_T5D,
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}, n from the rows that give n (1.52 at 270 and 500 nm, 1.51 at 1,000 nm) and k from the rows that give k (every 20 nm over 320–600 nm, then 700 and 800 nm), since the source gives n at only a few wavelengths. The source states no temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "basaltic-glass.json",
    option: "basaltic-glass",
    sha256: "e05d88b2250f8540e7504a7e92673d6e34bfa6d8d0ca2eb2d360023cf53376cf",
    parse: parseAria,
    resample: { holdNm: 20, separateNK: true, endK: "extrapolate" },
    header: {
      key: "lava_glass",
      phase: "solid",
      variant: null,
      name: "basaltic volcanic glass",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: POLLACK_1973,
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, basaltic_glass_R_Pollack_1973.ri (from the original paper)",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Rocks_and_Conglomerates/Basalt/Basaltic_glass_(Pollack_et_al_1973)/original/basaltic_glass_R_Pollack_1973.ri",
        "e05d88b2250f8540e7504a7e92673d6e34bfa6d8d0ca2eb2d360023cf53376cf",
        FETCHED_T5D,
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}, n from the rows that give n (every 20 nm over 210–500 nm, then 1,000 nm) and k from the rows that give k (every 20 nm from 400 nm). The source gives no k below 400 nm, so over 380–400 nm k is extrapolated linearly in log k from its 400 and 420 nm rows (7.9 × 10⁻⁴ and 6.4 × 10⁻⁴), since the glass absorbs more towards the ultraviolet. The source states no temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "mars-regolith-simulant.json",
    option: "mgs1",
    sha256: "a5ed6861dba6eb68aa310936854a29b1c772687fc4b4634ef8321dec3f806cd3",
    parse: (text) => parseMartikainenK(text, "MGS1"),
    resample: { holdNm: 0 },
    header: {
      key: "basalt_fe",
      phase: "solid",
      variant: null,
      name: "Fe-rich basalt, as MGS-1, a Mars regolith simulant",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "standIn",
      paper:
        'J. Martikainen, O. Muñoz, T. Jardiel, J. C. Gómez Martín, M. Peiteado, Y. Willame, A. Penttilä, K. Muinonen, G. Wurm and T. Becker, "Optical constants of Martian dust analogs at UV–visible–near-infrared wavelengths", Astrophys. J. Suppl. 268 (2023) 47, DOI 10.3847/1538-4365/acf0be',
      source: `the article's machine-readable Table 6, https://iopscience.iop.org/0067-0049/268/2/47/suppdata/apjsacf0bet6_ascii.txt?doi=10.3847/1538-4365/acf0be, SHA-256 a5ed6861dba6eb68aa310936854a29b1c772687fc4b4634ef8321dec3f806cd3, fetched ${FETCHED_T5D}, its MGS-1 column; read as data`,
      licence: `Adapted from CC BY 4.0 material (${CC_BY}): J. Martikainen et al., Astrophys. J. Suppl. 268 (2023) 47. Changed: MGS-1's k from Table 6 reduced onto this grid, with n the paper's fixed 1.5; this file is offered under CC BY 4.0`,
      reduction: `${STANDARD_REDUCTION}. Table 6's k is MGS-1's, retrieved with the paper's second Gaussian-random-sphere shapes and n fixed at 1.5; its 650 nm value, 4.33 × 10⁻⁴, is the index Martikainen et al. 2025's MGS-1 database takes (R08.T5.c's fidelity test), where Table 3's first shapes give 3.15 × 10⁻⁴. The rows are every 25 nm to 400 nm, then every 50 nm; the sample is a laboratory simulant at room temperature, unstated`,
      standIn:
        "a Mars regolith simulant stands in for Fe-rich basalt: MGS-1's absorption, retrieved by Martikainen et al. 2023 with its real index fixed at 1.5, since no open visible index of an Fe-rich basalt was found",
      fit: null,
    },
  },
];

/** IAPWS R9-97's coefficients a₀–a₇ and its two resonances, λ̄_UV and λ̄_IR (its Table 1). */
export const IAPWS_R9_97 = {
  a: [
    0.244_257_733, 9.746_344_76e-3, -3.732_349_96e-3, 2.686_784_72e-4, 1.589_205_7e-3,
    2.459_342_59e-3, 0.900_704_92, -1.666_262_19e-2,
  ],
  lambdaUv: 0.229_202,
  lambdaIr: 5.432_937,
} as const;

/**
 * Liquid water's real index with respect to vacuum by IAPWS R9-97's eq. 1 (the International
 * Association for the Properties of Water and Steam, "Release on the Refractive Index of Ordinary
 * Water Substance as a Function of Wavelength, Temperature and Pressure", Erlangen, 1997; after P.
 * Schiebener, J. Straub, J. M. H. Levelt Sengers and J. S. Gallagher, J. Phys. Chem. Ref. Data 19
 * (1990) 677): (n² − 1) ÷ (n² + 2) ÷ ρ̄ = a₀ + a₁ρ̄ + a₂T̄ + a₃λ̄²T̄ + a₄ ÷ λ̄² + a₅ ÷ (λ̄² − λ̄_UV²) +
 * a₆ ÷ (λ̄² − λ̄_IR²) + a₇ρ̄², with T̄ = T ÷ 273.15 K, ρ̄ = ρ ÷ 1000 kg m⁻³ and λ̄ = λ ÷ 0.589 µm.
 *
 * @remarks
 * The release endorses 0.2–1.1 µm and states an uncertainty of 1.5 × 10⁻⁵ over 0.40–0.70 µm at
 * 5–60 °C (ambient liquid), 5 × 10⁻⁴ over 0.21–0.40 µm and 10⁻³ over 0.70–1.1 µm (its Table 2). It
 * names λ only "the wavelength of light"; it is read here as the vacuum wavelength. Read as the
 * air wavelength, n would fall by at most 1.2 × 10⁻⁵ over 380–780 nm, at 380 nm, and by under
 * 5 × 10⁻⁶ from 595 nm ({@link AIR_READING_TEXT}).
 *
 * @param temperatureK - T, K (ITS-90).
 * @param densityKgPerM3 - ρ, kg m⁻³, from IAPWS-95 at the state.
 * @param wavelengthNm - λ, nm.
 */
export function iapwsWaterIndex(
  temperatureK: number,
  densityKgPerM3: number,
  wavelengthNm: number,
): number {
  const [a0, a1, a2, a3, a4, a5, a6, a7] = IAPWS_R9_97.a;
  const t = temperatureK / 273.15;
  const rho = densityKgPerM3 / 1000;
  const l2 = (wavelengthNm / 1000 / 0.589) ** 2;
  const perDensity =
    a0 +
    a1 * rho +
    a2 * t +
    a3 * l2 * t +
    a4 / l2 +
    a5 / (l2 - IAPWS_R9_97.lambdaUv ** 2) +
    a6 / (l2 - IAPWS_R9_97.lambdaIr ** 2) +
    a7 * rho * rho;
  const x = perDensity * rho;
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/** The water files' temperature, K: 25 °C, at which IAPWS R9-97 gives their n (R08.T5.h). */
export const WATER_TEMPERATURE_K = 298.15;

/**
 * Liquid water's density at {@link WATER_TEMPERATURE_K} and 101,325 Pa, kg m⁻³: IAPWS-95 (W. Wagner
 * and A. Pruß, J. Phys. Chem. Ref. Data 31 (2002) 387) as the NIST Chemistry WebBook's fluid
 * properties give it (`WATER_DENSITY_SOURCE`).
 */
export const WATER_DENSITY_KG_PER_M3 = 997.0476;

/** Where {@link WATER_DENSITY_KG_PER_M3} was read. */
const WATER_DENSITY_SOURCE = `the NIST Chemistry WebBook's fluid properties for water (SRD 69; IAPWS-95, W. Wagner and A. Pruß, J. Phys. Chem. Ref. Data 31 (2002) 387, density uncertain by 0.0001% at 1 atm), the isobar at 0.101325 MPa, https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C7732185&Type=IsoBar&Digits=7&P=0.101325&THigh=299.15&TLow=294.15&TInc=1&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm, SHA-256 645d38f5e948a988c941e60f7a49d699f91e7a4bb46910589cb8f278e0e01c6c, fetched ${FETCHED_T5G}`;

/** The rows of Pope and Fry 1997's table, 380–727.5 nm every 2.5 nm. */
export const POPE_FRY_ROWS = 140;

/**
 * The grid's span each source gives water's k over, nm (decision-r08-t5g ruling 2): Mason, Cone and
 * Fry 2016 over 380–495, Pope and Fry 1997 over 500–725, and Kou, Labrie and Chýlek 1993 over
 * 730–780, as IOCCG 2018's Table 1.1 lists them. The seam at 495/500 nm is where the first two
 * cross; the one at 725/730 nm is IOCCG's own.
 */
export const WATER_K_SPANS_NM = {
  mason: [380, 495],
  popeFry: [500, 725],
  kou: [730, 780],
} as const;

/** One row of an absorption table: the vacuum wavelength, nm, and the absorption coefficient a, m⁻¹. */
export type AbsorptionRow = readonly [wavelengthNm: number, absorptionPerM: number];

/** The rows of WOPP v3 that are Mason, Cone and Fry 2016's Table 2: 300–510 nm every 2 nm. */
export const MASON_ROWS = 106;

/**
 * The two rows of Mason, Cone and Fry's table that are taken from J. D. Mason's dissertation instead
 * (Texas A&M University, 2016, hdl 1969.1/174273, Table VI.1, p. 78), a, m⁻¹: 0.00321 at 422 nm and
 * 0.00690 at 446 nm, where WOPP v3 and the journal's Table 2 print 0.00322 and 0.00691
 * (decision-r08-t5g). The dissertation's other 64 rows over 380–510 nm are WOPP v3's.
 */
export const MASON_DISSERTATION_ROWS: ReadonlyArray<AbsorptionRow> = [
  [422, 0.00321],
  [446, 0.0069],
];

/**
 * Mason, Cone and Fry 2016's absorption rows from R. Röttgers's WOPP v3 compilation,
 * `purewater_abs_coefficients_v3.txt`, whose 300–510 nm rows are their Table 2 (its header says so;
 * its "20 °C" heading is not Mason's 23 °C), with {@link MASON_DISSERTATION_ROWS} applied: λ, nm,
 * and a, m⁻¹, every 2 nm.
 *
 * @throws Error if the file's rows over 300–510 nm are not {@link MASON_ROWS} rows 2 nm apart, or an
 *   overridden row is missing.
 */
export function parseWoppMason(text: string): ReadonlyArray<AbsorptionRow> {
  const rows: AbsorptionRow[] = [];
  for (const line of text.split(/\r?\n/u)) {
    const fields = line.trim().split(/\s+/u);
    if (line.startsWith("%") || fields.length < 2 || !fields.every((f) => NUMBER.test(f))) {
      continue;
    }
    const nm = numeric(fields[0] ?? "");
    if (nm >= 300 && nm <= 510) {
      rows.push([nm, numeric(fields[1] ?? "")]);
    }
  }
  if (rows.length !== MASON_ROWS || rows.some(([nm], i) => nm !== 300 + 2 * i)) {
    throw new Error(`WOPP v3: expected ${MASON_ROWS} rows every 2 nm over 300–510 nm`);
  }
  for (const [nm] of MASON_DISSERTATION_ROWS) {
    if (!rows.some(([at]) => at === nm)) {
      throw new Error(`WOPP v3: no row at ${nm} nm to override`);
    }
  }
  return rows.map(([nm, perM]) => [
    nm,
    MASON_DISSERTATION_ROWS.find(([at]) => at === nm)?.[1] ?? perM,
  ]);
}

/**
 * An absorption coefficient between a table's rows, linear in log a, m⁻¹.
 *
 * @throws Error if the wavelength lies outside the rows, or a bracketing row is not positive.
 */
export function logLinearAbsorptionPerM(
  rows: ReadonlyArray<AbsorptionRow>,
  wavelengthNm: number,
): number {
  const upper = rows.findIndex(([nm]) => nm >= wavelengthNm);
  const hi = rows[upper];
  if (hi === undefined) {
    throw new Error(`absorption: ${wavelengthNm} nm lies beyond the table`);
  }
  if (hi[0] === wavelengthNm) {
    return hi[1];
  }
  const lo = rows[upper - 1];
  if (lo === undefined) {
    throw new Error(`absorption: ${wavelengthNm} nm lies before the table`);
  }
  if (!(lo[1] > 0 && hi[1] > 0)) {
    throw new Error(`absorption: no positive bracket at ${wavelengthNm} nm`);
  }
  return lo[1] * (hi[1] / lo[1]) ** ((wavelengthNm - lo[0]) / (hi[0] - lo[0]));
}

/**
 * Kou, Labrie and Chýlek 1993's absorption of pure water at 22 °C over 730–780 nm every 5 nm, m⁻¹
 * (L. Kou, D. Labrie and P. Chýlek, "Refractive indices of water and ice in the 0.65- to 2.5-µm
 * spectral range", Appl. Opt. 32 (1993) 3531–3540, DOI 10.1364/AO.32.003531, closed; its 22 °C from
 * the abstract), as the IOCCG absorption protocol lists them in Table 1.1's a_w column, σ 0.04–0.05
 * m⁻¹ (A. R. Neeley and A. Mannino (eds.), IOCCG Protocol Series vol. 1.0, 2018, DOI
 * 10.25607/OBP-119, Table 1.1 on p. 12, its caption on p. 8), cited as facts.
 */
export const KOU_1993_ABSORPTION_PER_M: ReadonlyArray<AbsorptionRow> = [
  [730, 1.97],
  [735, 2.51],
  [740, 2.78],
  [745, 2.83],
  [750, 2.85],
  [755, 2.88],
  [760, 2.86],
  [765, 2.86],
  [770, 2.82],
  [775, 2.76],
  [780, 2.69],
];

/** water.json's k at a grid wavelength, nm, from the source whose span holds it, unrounded. */
function waterKAt(
  wavelengthNm: number,
  popeFry: ReadonlyArray<IndexSample>,
  mason: ReadonlyArray<AbsorptionRow>,
): number {
  const { mason: masonSpan, popeFry: popeFrySpan, kou: kouSpan } = WATER_K_SPANS_NM;
  if (wavelengthNm >= masonSpan[0] && wavelengthNm <= masonSpan[1]) {
    return absorptionIndex(logLinearAbsorptionPerM(mason, wavelengthNm), wavelengthNm);
  }
  if (wavelengthNm >= popeFrySpan[0] && wavelengthNm <= popeFrySpan[1]) {
    const row = popeFry.find((r) => Math.abs(r.wavelengthUm * 1000 - wavelengthNm) < 1e-9);
    if (row === undefined) {
      throw new Error(`water: Pope and Fry have no row at ${wavelengthNm} nm`);
    }
    return row.k;
  }
  const kou = KOU_1993_ABSORPTION_PER_M.find(([nm]) => nm === wavelengthNm)?.[1];
  if (wavelengthNm >= kouSpan[0] && wavelengthNm <= kouSpan[1] && kou !== undefined) {
    return absorptionIndex(kou, wavelengthNm);
  }
  throw new Error(`water: no source gives k at ${wavelengthNm} nm`);
}

/**
 * Pure water's index on {@link MATERIAL_GRID_NM} (R08.T5.h, decision-r08-t5g rulings 1 and 2): n
 * IAPWS R9-97's at 25 °C and 1 atm ({@link iapwsWaterN}), and k = aλ ÷ 4π from Mason, Cone and Fry
 * 2016 over 380–495 nm (linear in log a between their 2 nm rows), Pope and Fry 1997 over 500–725 nm
 * (each grid point on a row) and Kou et al. 1993 over 730–780 nm ({@link KOU_1993_ABSORPTION_PER_M}),
 * to six figures.
 *
 * @param popeFry - omlc.org's transcription of Pope and Fry's table.
 * @param wopp - WOPP v3's pure-water table.
 */
export function waterIndex(popeFry: string, wopp: string): ReducedIndex {
  const pope = parseOmlcAbsorption(popeFry, POPE_FRY_ROWS);
  const mason = parseWoppMason(wopp);
  return {
    n: MATERIAL_GRID_NM.map(iapwsWaterN),
    k: MATERIAL_GRID_NM.map((nm) => rounded(waterKAt(nm, pope, mason))),
  };
}

/**
 * IAPWS R9-97's n of pure water at 25 °C and 1 atm, to six figures: water.json's n since R08.T5.h
 * (decision-r08-t5g ruling 1).
 */
export function iapwsWaterN(wavelengthNm: number): number {
  return rounded(iapwsWaterIndex(WATER_TEMPERATURE_K, WATER_DENSITY_KG_PER_M3, wavelengthNm));
}

/**
 * Daimon and Masumura 2007's dispersion formula for distilled water at 24.0 °C, n² − 1 = Σ Bᵢλ² ÷
 * (λ² − Cᵢ) over 0.182–1.129 µm (M. Daimon and A. Masumura, "Measurement of the refractive index of
 * distilled water from the near-infrared region to the ultraviolet region", Appl. Opt. 46 (2007)
 * 3811–3820, DOI 10.1364/AO.46.003811), as refractiveindex.info's `main/H2O/nk/Daimon-24.0C.yml`
 * gives it (CC0): the check water.json records on IAPWS R9-97 (R08.T5.h).
 */
export const DAIMON_24C: DispersionFormula = {
  type: 2,
  rangeUm: [0.182, 1.129],
  coefficients: [
    0, 5.66695982e-1, 5.084151894e-3, 1.731900098e-1, 1.818488474e-2, 2.095951857e-2,
    2.625439472e-2, 1.125228406e-1, 1.073842352e1,
  ],
};

/**
 * Liquid water's density at 24 °C (297.15 K) and 101,325 Pa, kg m⁻³: IAPWS-95 on the same WebBook
 * isobar as {@link WATER_DENSITY_KG_PER_M3}, for the check against {@link DAIMON_24C}.
 */
export const WATER_DENSITY_24C_KG_PER_M3 = 997.2994;

/** Daimon and Masumura's 24 °C water less IAPWS R9-97's at 24 °C, at a wavelength, nm. */
export function daimonExcess(wavelengthNm: number): number {
  return (
    formulaIndex(DAIMON_24C, wavelengthNm / 1000) -
    iapwsWaterIndex(297.15, WATER_DENSITY_24C_KG_PER_M3, wavelengthNm)
  );
}

/**
 * water.json's n as R08.T5.g wrote it (committed at 2955006e), at the wavelengths R08.T5.h's record
 * compares, nm: Hale and Querry's, linear between their 25 nm rows.
 */
export const T5G_WATER_N: ReadonlyArray<readonly [wavelengthNm: number, n: number]> = [
  [380, 1.3406],
  [400, 1.339],
  [550, 1.333],
  [700, 1.331],
  [780, 1.3298],
];

/**
 * water.json's k as R08.T5.g wrote it (committed at 2955006e), at the wavelengths R08.T5.h's record
 * compares, nm: Pope and Fry's to 725 nm, Hale and Querry's from 750 nm, and log-linear between the
 * two over 730–745 nm.
 */
export const T5G_WATER_K: ReadonlyArray<readonly [wavelengthNm: number, k: number]> = [
  [380, 3.43822e-10],
  [400, 2.11039e-10],
  [440, 2.22339e-10],
  [550, 2.47287e-9],
  [740, 1.26386e-7],
  [750, 1.56e-7],
  [780, 1.43084e-7],
];

/**
 * Quan and Fry 1995's ten coefficients n₀–n₉ for seawater's real index relative to air (X. Quan and
 * E. S. Fry, "Empirical equation for the index of refraction of seawater", Appl. Opt. 34 (1995)
 * 3477–3480, DOI 10.1364/AO.34.003477, eq. 3), fitted to Austin and Halikas 1976's data from
 * 400–700 nm, as the Ocean Optics Web Book's "Water" page gives them (its eq. 1 and Table 1; CC BY),
 * and Zhang, Hu and He 2009's Table 1 (Opt. Express 17, 5698) too. The paper itself is closed, so its
 * stated temperature and salinity ranges were not read.
 */
export const QUAN_FRY_COEFFICIENTS: readonly [
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
] = [1.31405, 1.779e-4, -1.05e-6, 1.6e-8, -2.02e-6, 15.868, 0.01155, -0.00423, -4382, 1.1455e6];

/**
 * Quan and Fry 1995's index of seawater relative to air: n = n₀ + (n₁ + n₂T + n₃T²)S + n₄T² + (n₅ +
 * n₆S + n₇T) ÷ λ + n₈ ÷ λ² + n₉ ÷ λ³ ({@link QUAN_FRY_COEFFICIENTS}).
 *
 * @param practicalSalinity - S, the practical salinity (PSS-78, dimensionless; the Web Book's PSU).
 * @param temperatureC - T, °C.
 * @param wavelengthNm - λ, nm, fitted over 400–700.
 */
export function quanFryIndex(
  practicalSalinity: number,
  temperatureC: number,
  wavelengthNm: number,
): number {
  const [n0, n1, n2, n3, n4, n5, n6, n7, n8, n9] = QUAN_FRY_COEFFICIENTS;
  const t = temperatureC;
  const l = wavelengthNm;
  return (
    n0 +
    (n1 + n2 * t + n3 * t * t) * practicalSalinity +
    n4 * t * t +
    (n5 + n6 * practicalSalinity + n7 * t) / l +
    n8 / (l * l) +
    n9 / (l * l * l)
  );
}

/**
 * Seawater's excess over pure water's index in Quan and Fry's equation, n(S, T, λ) − n(0, T, λ) =
 * (n₁ + n₂T + n₃T²)S + n₆S ÷ λ: the salinity terms alone, which the seawater file adds to the pure
 * water file's own n (R08.T5.g).
 */
export function quanFrySalinityIncrement(
  practicalSalinity: number,
  temperatureC: number,
  wavelengthNm: number,
): number {
  return (
    quanFryIndex(practicalSalinity, temperatureC, wavelengthNm) -
    quanFryIndex(0, temperatureC, wavelengthNm)
  );
}

/** One fetched input of a {@link CombinedSpec}. */
export interface SourceInput {
  /** The command-line option that names the source's path. */
  readonly option: string;
  readonly sha256: string;
}

/**
 * A material file made from one or more fetched sources by its own reduction: a dispersion formula
 * evaluated on the grid, or a uniaxial crystal's two axes averaged.
 */
export interface CombinedSpec {
  readonly file: string;
  readonly inputs: ReadonlyArray<SourceInput>;
  /** The index on {@link MATERIAL_GRID_NM} from the sources' texts, in the order of `inputs`. */
  readonly reduce: (texts: ReadonlyArray<string>) => ReducedIndex;
  readonly header: MaterialHeader;
}

function textAt(texts: ReadonlyArray<string>, i: number): string {
  const text = texts[i];
  if (text === undefined) {
    throw new Error(`no source text ${i}`);
  }
  return text;
}

/** A dispersion formula's n on {@link MATERIAL_GRID_NM}, to six significant figures, with k = 0. */
export function formulaOnGrid(text: string): ReducedIndex {
  const formula = parseRefractiveIndexInfoFormula(text);
  return {
    n: MATERIAL_GRID_NM.map((nm) => rounded(formulaIndex(formula, nm / 1000))),
    k: MATERIAL_GRID_NM.map(() => 0),
  };
}

/**
 * A transparent uniaxial crystal's two formulas averaged in the dielectric function for randomly
 * oriented grains, n² = (2n_o² + n_e²) ÷ 3, on {@link MATERIAL_GRID_NM} to six significant figures,
 * with k = 0.
 */
export function uniaxialFormulasOnGrid(ordinary: string, extraordinary: string): ReducedIndex {
  const o = parseRefractiveIndexInfoFormula(ordinary);
  const e = parseRefractiveIndexInfoFormula(extraordinary);
  return {
    n: MATERIAL_GRID_NM.map((nm) => {
      const no = formulaIndex(o, nm / 1000);
      const ne = formulaIndex(e, nm / 1000);
      return rounded(Math.sqrt((2 * no * no + ne * ne) / 3));
    }),
    k: MATERIAL_GRID_NM.map(() => 0),
  };
}

const DRAINE_TERMS =
  "reduced values; the source states no terms and asks only for its citation (decision-r08-licences.md)";

/** The SHA-256 of omlc.org's `pope97.txt` (2,134 bytes), fetched {@link FETCHED_T5G} and again {@link FETCHED_T5H}. */
export const POPE_FRY_SHA256 = "5c92c6a1e4be125cc9464fab33490ce4eb883fca7e84a21b0ca2b906cb35c8a7";

/**
 * The SHA-256 of WOPP v3's `purewater_abs_coefficients_v3.txt` (119,419 bytes) at acolite's commit
 * ca477dd9, fetched {@link FETCHED_T5H}.
 */
export const WOPP_V3_SHA256 = "bda82058670cee8ca5684edc46a70e1b46c9d1ccca0eb9d72eb8776f65e696d8";

const WOPP_V3_URL =
  "https://raw.githubusercontent.com/acolite/acolite/ca477dd91dfb5f8eb77333824b38cb0bcce5e083/data/Shared/WOPP/purewater_abs_coefficients_v3.txt";

/** The two water files' fetched inputs: Pope and Fry's table and WOPP v3's, which holds Mason et al.'s. */
const WATER_INPUTS: ReadonlyArray<SourceInput> = [
  { option: "pope-fry", sha256: POPE_FRY_SHA256 },
  { option: "mason", sha256: WOPP_V3_SHA256 },
];

const POPE_FRY_1997 =
  'R. M. Pope and E. S. Fry, "Absorption spectrum (380–700 nm) of pure water. II. Integrating cavity measurements", Appl. Opt. 36 (1997) 8710–8723, DOI 10.1364/AO.36.008710';

const MASON_2016 =
  'J. D. Mason, M. T. Cone and E. S. Fry, "Ultraviolet (250–550 nm) absorption spectrum of pure water", Appl. Opt. 55 (2016) 7163–7172, DOI 10.1364/AO.55.007163';

const KOU_1993 =
  'L. Kou, D. Labrie and P. Chýlek, "Refractive indices of water and ice in the 0.65- to 2.5-µm spectral range", Appl. Opt. 32 (1993) 3531–3540, DOI 10.1364/AO.32.003531';

const IOCCG_2018 =
  'A. R. Neeley and A. Mannino (eds.), "Inherent Optical Property Measurements and Protocols: Absorption Coefficient", IOCCG Protocol Series vol. 1.0 (IOCCG, Dartmouth, 2018), DOI 10.25607/OBP-119';

const HALE_QUERRY_1973 =
  'G. M. Hale and M. R. Querry, "Optical constants of water in the 200-nm to 200-µm wavelength region", Appl. Opt. 12 (1973) 555–563, DOI 10.1364/AO.12.000555';

const IAPWS_R9_97_PAPER =
  'the International Association for the Properties of Water and Steam, IAPWS R9-97, "Release on the Refractive Index of Ordinary Water Substance as a Function of Wavelength, Temperature and Pressure" (Erlangen, 1997), after P. Schiebener, J. Straub, J. M. H. Levelt Sengers and J. S. Gallagher, J. Phys. Chem. Ref. Data 19 (1990) 677–717, DOI 10.1063/1.555859';

/** Where the two water files' inputs, and the constants cited as facts, were read. */
const WATER_SOURCE = `n from the IAPWS release, https://iapws.org/technical-guidance/release/Rindex.download, SHA-256 ef7c8107d200f250b6db1802b3b5ea994526e5b875ac5cb2c72ec78bc9a06ba3, fetched ${FETCHED_T5H} (its eq. 1 and Table 1), at the density of ${WATER_DENSITY_SOURCE}; k over 380–495 nm from R. Röttgers's WOPP v3 compilation, ${WOPP_V3_URL}, SHA-256 ${WOPP_V3_SHA256}, fetched ${FETCHED_T5H}, whose 300–510 nm rows are Mason, Cone and Fry's Table 2 (its header), with two rows from J. D. Mason's dissertation, "Ultraviolet absorption spectrum of pure water" (Texas A&M University, 2016, hdl 1969.1/174273), Table VI.1, read through the Wayback Machine's copy of OAKTrust's PDF, https://web.archive.org/web/20240725201852id_/https://oaktrust.library.tamu.edu/bitstream/handle/1969.1/174273/MASON-DISSERTATION-2016.pdf, SHA-256 921c10aa512e8cb599dac763f855bff43d3a58cae30160bc2cc9ea84c0d5413f, fetched ${FETCHED_T5H}: 0.00321 m⁻¹ at 422 nm and 0.00690 at 446 nm, where WOPP v3 and the journal's Table 2 print 0.00322 and 0.00691 (the dissertation's other 64 rows over 380–510 nm are WOPP v3's; WOPP v3's σ column, not used here, runs a row late over 426–458 nm); the journal's abstract page, which shows Table 2, https://opg.optica.org/ao/abstract.cfm?uri=ao-55-25-7163, SHA-256 c525296cff423f999c570cac368a61391bd108c448b3b28835196e7c7862a53f, fetched ${FETCHED_T5H}, for the cross-check; k over 500–725 nm from omlc.org's transcription of Pope and Fry's table (absorption in cm⁻¹ every 2.5 nm over 380–727.5 nm, at 22 °C), https://omlc.org/spectra/water/data/pope97.txt, SHA-256 ${POPE_FRY_SHA256}, fetched ${FETCHED_T5G} and again ${FETCHED_T5H}; k over 730–780 nm from Kou et al.'s values as the IOCCG protocol's Table 1.1 lists them (p. 12), https://ioccg.org/wp-content/uploads/2020/09/absorption_protocol_final-incl-cover_rev.pdf, SHA-256 6ce9582882352200b05f489ffc326935128b90fad0783420020fc91ecab82290, fetched ${FETCHED_T5H}`;

const WATER_LICENCE =
  "reduced values and constants cited as facts: IAPWS R9-97's coefficients, whose \"publication in whole or in part is allowed in all countries provided that attribution is given to the International Association for the Properties of Water and Steam\" (the release's notice), attributed here and in NOTICE; Mason, Cone and Fry's absorption, cited as facts, read from WOPP v3, whose file states no terms of its own (it is kept in the GPL-3.0 acolite repository), and none of it is committed; omlc.org states no terms for its transcription beyond the site's copyright notice, and the measured values are Pope and Fry's (decision-r08-licences.md); Kou et al.'s eleven values, cited as facts from the IOCCG protocol (©IOCCG 2018, no terms stated)";

/**
 * Mason, Cone and Fry 2016's pure-water absorption at the wavelengths the water file's record
 * compares, a, m⁻¹ ({@link MASON_2016}, Table 2, closed, as the journal's abstract page shows it):
 * 380–500 nm every 20, where WOPP v3's rows are the file's k, and 520, 540 and 550 nm, past WOPP v3's
 * 510 nm, the cross-check of Pope and Fry's (decision-r08-t5g).
 */
export const MASON_2016_ABSORPTION_PER_M: ReadonlyArray<AbsorptionRow> = [
  [380, 0.00143],
  [400, 0.00222],
  [420, 0.00312],
  [440, 0.00522],
  [460, 0.00909],
  [480, 0.01214],
  [500, 0.02073],
  [520, 0.03917],
  [540, 0.04754],
  [550, 0.05629],
];

/**
 * Pope and Fry 1997's absorption at the wavelengths the water file's record compares, m⁻¹ (their
 * table, as omlc.org gives it): 0.01137, 0.00663, 0.00454, 0.00635, 0.00979, 0.0127 and 0.0204 at
 * 380–500 nm every 20, 0.0173 at 495 nm (the seam), and 0.0409, 0.0474 and 0.0565 at 520, 540 and 550
 * nm.
 */
export const POPE_FRY_ABSORPTION_PER_M: ReadonlyArray<AbsorptionRow> = [
  [380, 0.01137],
  [400, 0.00663],
  [420, 0.00454],
  [440, 0.00635],
  [460, 0.00979],
  [480, 0.0127],
  [495, 0.0173],
  [500, 0.0204],
  [520, 0.0409],
  [540, 0.0474],
  [550, 0.0565],
];

/** Pope and Fry over Mason et al. at each of Mason's compared wavelengths, as the water file states it. */
function masonCrossCheckText(): string {
  return MASON_2016_ABSORPTION_PER_M.map(([nm, mason]) => {
    const pope = POPE_FRY_ABSORPTION_PER_M.find(([at]) => at === nm)?.[1] ?? Number.NaN;
    return `${(pope / mason).toFixed(2)} at ${nm} nm`;
  }).join(", ");
}

/** The practical salinity (PSS-78, dimensionless) and temperature of the seawater file: S = 35 at 25 °C. */
export const SEAWATER = { practicalSalinity: 35, temperatureC: 25 } as const;

/**
 * Seawater's n on {@link MATERIAL_GRID_NM}: pure water's n on the grid plus Quan and Fry's salinity
 * terms at {@link SEAWATER}, to six figures.
 */
export function seawaterN(waterN: ReadonlyArray<number>): ReadonlyArray<number> {
  return MATERIAL_GRID_NM.map((nm, i) =>
    rounded(
      (waterN[i] ?? Number.NaN) +
        quanFrySalinityIncrement(SEAWATER.practicalSalinity, SEAWATER.temperatureC, nm),
    ),
  );
}

/**
 * How far Quan and Fry's own pure water (S = 0, 25 °C), times dry air's index (Peck and Reeder 1972,
 * `view/atmosphere/rayleigh.ts`'s `DRY_AIR_DISPERSION`), lies from IAPWS R9-97, as the water file
 * states it (computed once; the tool's test checks it).
 */
const QUAN_FRY_BASE_TEXT =
  "by 1.7 × 10⁻⁵ to 3.5 × 10⁻⁵ over 400–700 nm, and by 6.8 × 10⁻⁵ at 380 nm and 8.9 × 10⁻⁵ at 780 nm, beyond their range";

/**
 * How far water.json's n would fall were R9-97's λ read as the air wavelength, the vacuum wavelength
 * being λ times dry air's index (Peck and Reeder 1972, `view/atmosphere/rayleigh.ts`'s
 * `DRY_AIR_DISPERSION`), as the file states it (computed once; the tool's test checks it).
 */
const AIR_READING_TEXT =
  "read as the air wavelength, n would fall by at most 1.2 × 10⁻⁵, at 380 nm, and by under 5 × 10⁻⁶ from 595 nm, inside the release's uncertainty";

/** A signed figure in the files' form, its sign written unless it rounds to 0: +0.00451, −0.00132. */
function signed(value: number, digits: number): string {
  const text = Math.abs(value).toFixed(digits);
  if (Number(text) === 0) {
    return text;
  }
  return value < 0 ? `−${text}` : `+${text}`;
}

/** A signed figure in scientific notation, as the files write it: −5.6 × 10⁻⁶. */
function signedScientific(value: number): string {
  return `${value < 0 ? "−" : "+"}${timesTen(Math.abs(value), 1)}`;
}

/** water.json's n's record: R9-97's range, its checks, and the change from T5.g's Hale and Querry. */
function waterNRecordText(): string {
  const was = (nm: number): number => T5G_WATER_N.find(([at]) => at === nm)?.[1] ?? Number.NaN;
  const change = (nm: number): string => signed(iapwsWaterN(nm) - was(nm), 5);
  const spanWas = was(380) - was(780);
  const spanNow = iapwsWaterN(380) - iapwsWaterN(780);
  return `n: IAPWS R9-97's eq. 1 at 298.15 K and ${WATER_DENSITY_KG_PER_M3} kg m⁻³, evaluated at each wavelength of 380–780 nm every 5 nm, to six significant figures (decision-r08-t5g ruling 1); λ is read as the vacuum wavelength (${AIR_READING_TEXT}). The release's estimated uncertainty (its Table 2, the liquid at ambient pressure): 1.5 × 10⁻⁵ over 0.40–0.70 µm at 5–60 °C, 5 × 10⁻⁴ over 0.21–0.40 µm and 10⁻³ over 0.70–1.1 µm. Checks: Daimon and Masumura 2007's formula for distilled water at 24.0 °C (Appl. Opt. 46, 3811, DOI 10.1364/AO.46.003811; ${fetchedSource("refractiveindex.info, main/H2O/nk/Daimon-24.0C.yml", `${RI_URL}/main/H2O/nk/Daimon-24.0C.yml`, "1d41f6f3b8a6702e6b3b788b4d2138f9ae7730305802cea2d93e91f38c7ff7c7", FETCHED_T5H)}) lies ${[380, 550, 780].map((nm) => `${signedScientific(daimonExcess(nm))} at ${nm} nm`).join(", ")} from R9-97 at 24 °C and ${WATER_DENSITY_24C_KG_PER_M3} kg m⁻³ (the same isobar); Quan and Fry 1995's pure water (S = 0, 25 °C, relative to air), times dry air's index at 15 °C (Peck and Reeder 1972's standard air), lies above it ${QUAN_FRY_BASE_TEXT}. R08.T5.h's change from Hale and Querry's n (${HALE_QUERRY_1973}), which R08.T5.g's file held: ${change(380)} at 380 nm, ${change(550)} at 550 nm and ${change(780)} at 780 nm, so n(380) − n(780) is ${spanNow.toFixed(5)} where it was ${spanWas.toFixed(4)}`;
}

/** water.json's k's factors against R08.T5.g's file: a fall where it fell, a rise where it rose. */
function waterKChangeText(k: (nm: number) => number): string {
  const was = (nm: number): number => T5G_WATER_K.find(([at]) => at === nm)?.[1] ?? Number.NaN;
  const fall = [380, 400, 440]
    .map((nm) => `${(was(nm) / k(nm)).toFixed(2)} at ${nm} nm`)
    .join(", ");
  const rise = [740, 750, 780]
    .map((nm) => `${(k(nm) / was(nm)).toFixed(2)} at ${nm} nm`)
    .join(", ");
  return `k falls by a factor of ${fall}, and rises by ${rise}`;
}

/** water.json's k on the grid at a wavelength, as written: aλ ÷ 4π from the source whose span holds it. */
function waterKFromConstants(nm: number): number {
  const mason = MASON_2016_ABSORPTION_PER_M.find(([at]) => at === nm)?.[1];
  const kou = KOU_1993_ABSORPTION_PER_M.find(([at]) => at === nm)?.[1];
  const perM = nm <= WATER_K_SPANS_NM.mason[1] ? mason : kou;
  return rounded(absorptionIndex(perM ?? Number.NaN, nm));
}

/** water.json's k's record: its three sources, the seams, the change, the alternative and the temperatures. */
function waterKRecordText(): string {
  return `k = aλ ÷ 4π, to six significant figures (decision-r08-t5g ruling 2): Mason, Cone and Fry 2016's Table 2 over 380–495 nm, linear in log a between its rows every 2 nm (a three-point moving average of rows every 2 nm, at 23 ± 0.5 °C: Mason's dissertation pp. 75 and 80); Pope and Fry 1997's rows over 500–725 nm, every grid point on a row (at 22 °C); and Kou et al. 1993's over 730–780 nm, every 5 nm (at 22 °C), as IOCCG 2018 lists them (${IOCCG_2018}, Table 1.1). The seam at 495/500 nm is where the two cross: Pope and Fry's 0.0173 m⁻¹ at 495 nm lies between Mason et al.'s 0.01648 and 0.01774 at 494 and 496 nm, and their 0.0204 at 500 nm within 1.6% of Mason et al.'s 0.02073. The seam at 725/730 nm is IOCCG's: Kou et al. run about 0.04 m⁻¹ above Pope and Fry at 670 nm and 0.07 m⁻¹ at 725 nm over their overlap (IOCCG §1.1.1), and IOCCG keeps Pope and Fry there, as this file does. R08.T5.h's change from R08.T5.g's file, whose k was Pope and Fry's over 380–725 nm and Hale and Querry's from 750 nm, log-linear between over 730–745 nm: ${waterKChangeText(waterKFromConstants)}; Hale and Querry give nothing to this file. The alternative below 500 nm is Pope and Fry's, which runs ${masonCrossCheckText()} times Mason et al.'s: the original integrating cavity's sensitivity falls slowly over 410–500 nm and rapidly below 410 nm, and Mason et al. agree with Pope and Fry above 440 nm (Mason's dissertation pp. 81–89); the IOCCG protocol (§1.1.1–1.1.2) takes Pope and Fry as the state of the art over about 420–725 nm only, their ultraviolet values "significantly higher than these other careful works, indicating possible contamination". Mason et al.'s 520, 540 and 550 nm rows are the cross-check of Pope and Fry's there. Temperatures: k is at 23 °C to 495 nm and 22 °C from 500 nm, n at 25 °C. Below 550 nm water's temperature coefficient is near zero; near 740 nm it is 0.0161 m⁻¹ °C⁻¹ (IOCCG's Table 1.1, after Röttgers et al. 2014), so 22 → 25 °C would raise a there by about 1.7%, not drawn (one temperature per liquid)`;
}

/**
 * R08.T5.d's to T5.h's files made from fetched sources by their own reductions, the first part of
 * {@link COMBINED_MATERIALS}, which T5.k's liquid tables follow once the gas rows they read are
 * defined.
 */
const COMBINED_MATERIALS_T5D_T5H: ReadonlyArray<CombinedSpec> = [
  {
    file: "potassium-chloride.json",
    inputs: [
      {
        option: "potassium-chloride",
        sha256: "f7b927f60e17c0bd2bf63447bdee9a0de24de9f077090b47a9cb02660e0ea166",
      },
    ],
    reduce: (texts) => formulaOnGrid(textAt(texts, 0)),
    header: {
      key: "KCl",
      phase: "solid",
      variant: null,
      name: "crystalline KCl at 297 K",
      temperatureK: 297,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: LI_1976,
      source: fetchedSource(
        "refractiveindex.info, main/KCl/nk/Li.yml, Li's Sellmeier formula from his critical analysis of several sources' measurements",
        `${RI_URL}/main/KCl/nk/Li.yml`,
        "f7b927f60e17c0bd2bf63447bdee9a0de24de9f077090b47a9cb02660e0ea166",
        FETCHED_T5D,
      ),
      licence: RI_LICENCE,
      reduction:
        "Li's formula (the database's formula 1, stated for 0.18–35 µm) evaluated at each wavelength of 380–780 nm every 5 nm, to six significant figures; k = 0, stated, since the source gives n only and the crystal's visible k is 3 × 10⁻¹⁰ or less (Palik's handbook, as Kitzmann and Heng 2018 compile it), far below anything a grain shows",
      standIn: null,
      fit: null,
    },
  },
  {
    file: "sodium-chloride.json",
    inputs: [
      {
        option: "sodium-chloride",
        sha256: "20e0b06e16a1abb3668487635430994c0046cef05cd68ee0878804ac5dceae89",
      },
    ],
    reduce: (texts) => formulaOnGrid(textAt(texts, 0)),
    header: {
      key: "NaCl",
      phase: "solid",
      variant: null,
      name: "crystalline NaCl at 297 K",
      temperatureK: 297,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper: LI_1976,
      source: fetchedSource(
        "refractiveindex.info, main/NaCl/nk/Li.yml, Li's Sellmeier formula from his critical analysis of several sources' measurements",
        `${RI_URL}/main/NaCl/nk/Li.yml`,
        "20e0b06e16a1abb3668487635430994c0046cef05cd68ee0878804ac5dceae89",
        FETCHED_T5D,
      ),
      licence: RI_LICENCE,
      reduction:
        "Li's formula (the database's formula 1, stated for 0.2–30 µm) evaluated at each wavelength of 380–780 nm every 5 nm, to six significant figures; k = 0, stated, since the source gives n only and the crystal's visible k is under 4 × 10⁻¹⁰ (Eldridge and Palik in Palik's handbook, as Kitzmann and Heng 2018 compile it), far below anything a grain shows",
      standIn: null,
      fit: null,
    },
  },
  {
    file: "silicon-dioxide.json",
    inputs: [
      {
        option: "silicon-dioxide-ordinary",
        sha256: "c63dabfdfe9921c8c19264ff6126d68cd78abb1552e40f536c2ffaa083773e7d",
      },
      {
        option: "silicon-dioxide-extraordinary",
        sha256: "a80ec99225ffbcfa2264f7bc34c491dfa8694dbc1f23158882287d2f91720323",
      },
    ],
    reduce: (texts) => uniaxialFormulasOnGrid(textAt(texts, 0), textAt(texts, 1)),
    header: {
      key: "SiO2",
      phase: "solid",
      variant: null,
      name: "trigonal crystalline SiO2",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'G. Ghosh, "Dispersion-equation coefficients for the refractive index and birefringence of calcite and quartz crystals", Opt. Commun. 163 (1999) 95–102, DOI 10.1016/S0030-4018(99)00091-7',
      source: `refractiveindex.info, main/SiO2/nk/Ghosh-o.yml and Ghosh-e.yml, Ghosh's dispersion formulas for the ordinary and extraordinary rays, ${RI_URL}/main/SiO2/nk/Ghosh-o.yml, SHA-256 c63dabfdfe9921c8c19264ff6126d68cd78abb1552e40f536c2ffaa083773e7d, and ${RI_URL}/main/SiO2/nk/Ghosh-e.yml, SHA-256 a80ec99225ffbcfa2264f7bc34c491dfa8694dbc1f23158882287d2f91720323, fetched ${FETCHED_T5D}`,
      licence: RI_LICENCE,
      reduction:
        "Ghosh's two formulas (the database's formula 2, stated for 0.198–2.0531 µm) evaluated at each wavelength of 380–780 nm every 5 nm and averaged in the dielectric function for randomly oriented grains, n² = (2n_o² + n_e²) ÷ 3, to six significant figures; k = 0, stated, since the sources give n only. The trigonal crystal is taken over fused silica (I. H. Malitson, J. Opt. Soc. Am. 55 (1965) 1205: n 1.460 at 550 nm, against this file's 1.549), which is recorded as the alternative: which form a condensate takes is not modelled; 295 K stands for the source's room temperature",
      standIn: null,
      fit: null,
    },
  },
  {
    file: "aluminium-oxide.json",
    inputs: [
      {
        option: "aluminium-oxide-ordinary",
        sha256: "459f0cb41f105b9a01dc96a7d2eab0a71c5d43f7355fba3c4d3d6969fc76a41b",
      },
      {
        option: "aluminium-oxide-extraordinary",
        sha256: "d92e8619f7b4fde45947c8a6ca1529e2da4b6b3c36f9b9dbeb366408c2fa7f22",
      },
    ],
    reduce: (texts) => uniaxialFormulasOnGrid(textAt(texts, 0), textAt(texts, 1)),
    header: {
      key: "Al2O3",
      phase: "solid",
      variant: null,
      name: "crystalline Al2O3 at 293 K",
      temperatureK: 293,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'I. H. Malitson and M. J. Dodge, "Refractive index and birefringence of synthetic sapphire", J. Opt. Soc. Am. 62 (1972) 1405 (an abstract), with the dispersion formula as M. J. Dodge gives it in "Refractive index", Handbook of Laser Science and Technology, vol. IV, Optical Materials: Part 2, CRC Press (1986), p. 30',
      source: `refractiveindex.info, main/Al2O3/nk/Malitson-o.yml and Malitson-e.yml, the formulas for the ordinary and extraordinary rays of synthetic sapphire at 20 °C, ${RI_URL}/main/Al2O3/nk/Malitson-o.yml, SHA-256 459f0cb41f105b9a01dc96a7d2eab0a71c5d43f7355fba3c4d3d6969fc76a41b, and ${RI_URL}/main/Al2O3/nk/Malitson-e.yml, SHA-256 d92e8619f7b4fde45947c8a6ca1529e2da4b6b3c36f9b9dbeb366408c2fa7f22, fetched ${FETCHED_T5D}`,
      licence: RI_LICENCE,
      reduction:
        "the two formulas (the database's formula 1, stated for 0.20–5.0 µm) evaluated at each wavelength of 380–780 nm every 5 nm and averaged in the dielectric function for randomly oriented grains, n² = (2n_o² + n_e²) ÷ 3, to six significant figures; k = 0, stated, since the sources give n only. The crystal is taken, as the plan's transparent high-index class does; amorphous Al2O3 (Begemann et al. 1997, Astrophys. J. 476, 199; Koike et al. 1995, Icarus 114, 203; n 1.58 and k 0.04 at 420 nm as Kitzmann and Heng 2018 compile them) is recorded as the alternative",
      standIn: null,
      fit: null,
    },
  },
  {
    file: "graphite.json",
    inputs: [
      {
        option: "graphite-perpendicular",
        sha256: "663d2939905710d61706d75f52e28ca819fcc5ecde1be2d2971b23c127a351d9",
      },
      {
        option: "graphite-parallel",
        sha256: "87c609d3ddda2bc0bc5f2343f9b56bbd517fc5833e0754e6a628c8a5c6235a0b",
      },
    ],
    reduce: (texts) =>
      resample(
        averagedUniaxial(
          parseDraineDielectric(textAt(texts, 0)),
          parseDraineDielectric(textAt(texts, 1)),
        ),
        { holdNm: 0 },
      ),
    header: {
      key: "graphite",
      phase: "solid",
      variant: null,
      name: "randomly oriented graphite at 20 K",
      temperatureK: 20,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'B. T. Draine, "Scattering by interstellar dust grains. II. X-rays", Astrophys. J. 598 (2003) 1026–1037, DOI 10.1086/379123, its dielectric functions of graphite for E ∥ c and E ⊥ c',
      source: `B. T. Draine's site, https://www.astro.princeton.edu/~draine/dust/diel/callindex.out_CpeD03_0.01 (E ⊥ c, 22,804 bytes, SHA-256 663d2939905710d61706d75f52e28ca819fcc5ecde1be2d2971b23c127a351d9) and https://www.astro.princeton.edu/~draine/dust/diel/callindex.out_CpaD03_0.01 (E ∥ c, 22,981 bytes, SHA-256 87c609d3ddda2bc0bc5f2343f9b56bbd517fc5833e0754e6a628c8a5c6235a0b), fetched ${FETCHED_T5D}: the tables for grains of radius 0.01 µm at 20 K`,
      licence: DRAINE_TERMS,
      reduction: `the two dielectric functions averaged for randomly oriented grains at each of the sources' wavelengths, ε = (2ε_⊥ + ε_∥) ÷ 3, as Kitzmann and Heng 2018's C.dat takes them (n 2.567, k 1.189 at 550 nm), and n + ik = √ε; then ${STANDARD_REDUCTION}. The sources' rows here are 365, 381, 400, 434, 440, 486, 550, 656, 700 and 800 nm. The tables' grain radius and temperature are the files'`,
      standIn: null,
      fit: null,
    },
  },
  // R08.T5.h (decision-r08-t5g): pure water's n from IAPWS R9-97 and its k from three sources; and
  // seawater, which follows them.
  {
    file: "water.json",
    inputs: WATER_INPUTS,
    reduce: (texts) => waterIndex(textAt(texts, 0), textAt(texts, 1)),
    header: {
      key: "H2O",
      phase: "liquid",
      variant: null,
      name: "liquid H2O at 298 K",
      temperatureK: 298,
      shape: "sphere",
      provenance: "measured",
      paper: `${IAPWS_R9_97_PAPER}, for n; ${MASON_2016}, Table 2, for k over 380–495 nm; ${POPE_FRY_1997}, for k over 500–725 nm; ${KOU_1993}, for k over 730–780 nm, as ${IOCCG_2018}, Table 1.1, lists it`,
      source: WATER_SOURCE,
      licence: WATER_LICENCE,
      reduction: `${waterNRecordText()}. ${waterKRecordText()}`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "seawater.json",
    inputs: WATER_INPUTS,
    reduce: (texts) => {
      const water = waterIndex(textAt(texts, 0), textAt(texts, 1));
      return { n: seawaterN(water.n), k: water.k };
    },
    header: {
      key: "H2O",
      phase: "liquid",
      variant: "S35",
      name: "liquid H2O, practical salinity 35, at 298 K",
      temperatureK: 298,
      shape: "sphere",
      provenance: "measured",
      paper:
        'X. Quan and E. S. Fry, "Empirical equation for the index of refraction of seawater", Appl. Opt. 34 (1995) 3477–3480, DOI 10.1364/AO.34.003477, for the salinity terms; water.json\'s papers for pure water',
      source: `the Ocean Optics Web Book's "Water" page, eq. 1 and Table 1, for Quan and Fry's coefficients (https://www.oceanopticsbook.info/view/optical-constituents-of-the-ocean/water, SHA-256 7191efa9e11f04c9edde4156f2ba70fda81ae06e44e1d2346a9f0f3f54c65a04, fetched ${FETCHED_T5G}; the paper is closed, and Zhang, Hu and He 2009, Opt. Express 17, 5698, Table 1, gives the same); then water.json's: ${WATER_SOURCE}`,
      licence: `${WATER_LICENCE}; Quan and Fry's coefficients cited as facts, read from the Web Book (CC BY)`,
      reduction: `n = water.json's n (IAPWS R9-97's since R08.T5.h) plus Quan and Fry's salinity terms, (n₁ + n₂T + n₃T²)S + n₆S ÷ λ at practical salinity S = ${SEAWATER.practicalSalinity} (PSS-78, dimensionless; the Web Book gives Quan and Fry's S in PSU) and T = ${SEAWATER.temperatureC} °C: +${quanFrySalinityIncrement(SEAWATER.practicalSalinity, SEAWATER.temperatureC, 380).toFixed(5)} at 380 nm, +${quanFrySalinityIncrement(SEAWATER.practicalSalinity, SEAWATER.temperatureC, 550).toFixed(5)} at 550 nm and +${quanFrySalinityIncrement(SEAWATER.practicalSalinity, SEAWATER.temperatureC, 780).toFixed(5)} at 780 nm, to six significant figures. Quan and Fry fitted Austin and Halikas 1976's data from 400–700 nm (the Web Book), relative to air, so their salinity terms alone are added to water.json's own n rather than their equation taken whole: the two files differ by Quan and Fry's salinity effect, and this one follows water.json's n should it change. k = water.json's: sea salts leave pure water's visible absorption unchanged within measurement (Röttgers, McKee and Utschig 2014, Opt. Express 22, 25093, DOI 10.1364/OE.22.025093: the salinity coefficient not significantly different from zero below 500 nm, and −0.18% to +0.04% per g L⁻¹ over 400–2,660 nm); their absorption lies in the ultraviolet. Seawater's molecular scattering is not in the file`,
      standIn: null,
      fit: null,
    },
  },
];

/**
 * CO₂ ice's derived index (`science-r08-sulphur-co2ice.md`): the real part a two-term Cauchy fit to
 * Warren 1986's Table I over 0.30–1.10 µm, and α = 10⁻² m⁻¹, Hansen 2005's estimated visible
 * absorption, below that paper's detection limit over 0.25–1.0 µm (JGR 110, E11003; its abstract).
 */
export const CO2_ICE_FIT: CauchyFit = {
  form: "cauchy",
  a: 1.3994,
  bUm2: 0.004312,
  absorptionPerM: 1e-2,
  fittedFromUm: 0.3,
  fittedToUm: 1.1,
};

/** A derived index at a wavelength, nm: n = a + b ÷ λ², k = αλ ÷ 4π. */
export function cauchyIndex(
  fit: CauchyFit,
  wavelengthNm: number,
): { readonly n: number; readonly k: number } {
  const um = wavelengthNm / 1000;
  return {
    n: fit.a + fit.bUm2 / (um * um),
    k: (fit.absorptionPerM * wavelengthNm * 1e-9) / (4 * Math.PI),
  };
}

/**
 * A gas's measured refractivity in the one-term form n − 1 = A ÷ (B − λ⁻²), λ the vacuum wavelength
 * in µm, stated at 0 °C and 760 mm.
 */
export interface OneTermRefractivity {
  /** A, µm⁻². */
  readonly aPerUm2: number;
  /** B, µm⁻². */
  readonly bPerUm2: number;
  /**
   * The compressibility factor Z of the gas the formula describes, at 0 °C and 760 mm: its molar
   * volume is Z times the ideal gas's.
   */
  readonly compressibility: number;
  readonly source: string;
}

/**
 * NH₃'s refractivity: C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1, p. 22,
 * n − 1 = 0.032 953 ÷ (90.392 − λ⁻²), measured over 480–670.8 nm, with Z = 0.984 798 × 0.7708 ÷
 * 0.7605 = 0.998 14. The same row as `view/atmosphere/rayleigh.ts`'s `GAS_DISPERSION.NH3`, which
 * `view/atmosphere/materials/materials.test.ts` checks.
 *
 * @remarks
 * The paper reduces every refractivity to its "theoretic density" (p. 5): for ammonia (p. 21) it
 * multiplies the real gas's refractivity at 0 °C and 760 mm by 0.7605 ÷ 0.7708, the theoretic over
 * the measured weight of a litre, g. So the formula's gas is all but ideal, and Z is the real gas's,
 * NIST's 0.984 798, over that factor (`rayleigh.ts` holds the reading).
 */
export const NH3_REFRACTIVITY: OneTermRefractivity = {
  aPerUm2: 0.032_953,
  bPerUm2: 90.392,
  compressibility: (0.984_798 * 0.770_8) / 0.760_5,
  source: "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1",
};

/**
 * H₂S's refractivity: C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171, p. 174,
 * n − 1 = 0.053 711 ÷ (86.756 − λ⁻²), measured over 486.1–656.3 nm, at the number density of
 * hydrogen at 0 °C and 760 mm, with Z = 0.991 571 ÷ 0.990 92 = 1.000 66. The same row as
 * `view/atmosphere/rayleigh.ts`'s `GAS_DISPERSION.H2S`, which
 * `view/atmosphere/materials/materials.test.ts` checks.
 */
export const H2S_REFRACTIVITY: OneTermRefractivity = {
  aPerUm2: 0.053_711,
  bPerUm2: 86.756,
  compressibility: 0.991_571 / 0.990_92,
  source: "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171",
};

/**
 * HCl's refractivity, for NH₄Cl's increment ({@link NH4CL_IONIC_INCREMENT}): C. and M. Cuthbertson,
 * Phil. Trans. R. Soc. Lond. A 213 (1914) 1, p. 12, (μ − 1) D ÷ (d₀76) = 4.6425 × 10²⁷ ÷ (10,664 ×
 * 10²⁷ − n²), n = 3 × 10¹⁰ ÷ λ cm s⁻¹ as the paper's frequencies take it: n − 1 = 0.051 583 ÷
 * (118.49 − λ⁻²), measured over 480–670.8 nm.
 *
 * @remarks
 * The same paper as NH₃'s row, so the two gases' reductions match. The paper reduces HCl to the
 * number density of hydrogen at 0 °C and 76 cm through Gray and Burt's volume ratio, 1.0079, and
 * Leduc's expansion coefficient (p. 11), so Z is NIST's for H₂ there, 1.000 624. Reading Gray and
 * Burt's ratio against NIST's H₂ at 16 °C instead gives 0.999 46, and the increment +3.58%, 6.5 ×
 * 10⁻⁴ more.
 */
export const HCL_REFRACTIVITY: OneTermRefractivity = {
  aPerUm2: 0.051_583,
  bPerUm2: 118.49,
  compressibility: 1.000_624,
  source: "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1",
};

/** The Avogadro constant, mol⁻¹, exact in the 2019 SI. */
const AVOGADRO_PER_MOL = 6.022_140_76e23;

/** The Boltzmann constant, J K⁻¹, exact in the 2019 SI. */
const BOLTZMANN_J_PER_K = 1.380_649e-23;

/**
 * The ideal gas's molar volume at 0 °C and 101,325 Pa, cm³ mol⁻¹: N_A k_B × 273.15 K ÷ 101,325 Pa,
 * exact in the 2019 SI (CODATA's 22.413 969 54 × 10⁻³ m³ mol⁻¹).
 */
export const IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL =
  ((AVOGADRO_PER_MOL * BOLTZMANN_J_PER_K * 273.15) / 101_325) * 1e6;

/**
 * The molar volume of a crystal, cm³ mol⁻¹, from its unit cell's volume in Å³ and the formula units
 * the cell holds.
 */
export function cellMolarVolumeCm3PerMol(cellVolumeA3: number, formulaUnits: number): number {
  return (AVOGADRO_PER_MOL * cellVolumeA3 * 1e-24) / formulaUnits;
}

/**
 * NH₄SH's molar volume, cm³ mol⁻¹, from its cell: C. D. West, "The crystal structures of some
 * alkali hydrosulfides and monosulfides", Z. Kristallogr. 88 (1934) 97–115, as COD entry 1010249
 * gives it (CC0): P4/nmm, a = 6.011 Å, c = 4.009 Å, two formula units.
 *
 * @remarks
 * That is 43.62 cm³ mol⁻¹, a density of 1.1717 g cm⁻³ at M = 51.107 g mol⁻¹ (CIAAW's conventional
 * atomic weights). The ruling's 1.1715 matches the entry's rounded cell volume, 144.9 Å³, where a²c
 * is 144.85 Å³; n differs by about 3 × 10⁻⁴. The cell is as published: a room-temperature
 * measurement, and before 1947 possibly in kX units (1 kX = 1.002 02 Å), which would lower n by
 * about 0.005. A cloud near 200 K is presumably denser, raising n by a comparable amount
 * (unmeasured). Both lie inside the stated band.
 */
export const NH4SH_MOLAR_VOLUME_CM3_PER_MOL = cellMolarVolumeCm3PerMol(6.011 ** 2 * 4.009, 2);

/**
 * NH₄SH's molar mass, g mol⁻¹, from CIAAW's conventional atomic weights (H 1.008, N 14.007,
 * S 32.06). Only the file's stated density uses it: n depends on the molar volume alone.
 */
export const NH4SH_MOLAR_MASS_G_PER_MOL = 14.007 + 5 * 1.008 + 32.06;

/** The Lorentz–Lorenz factor (n² − 1) ÷ (n² + 2) of an index. */
function lorentzLorenzFactor(index: number): number {
  return (index * index - 1) / (index * index + 2);
}

/**
 * The additive molar refraction of gases, cm³ mol⁻¹: R = Σ (n² − 1) ÷ (n² + 2) × Z V₀ over their
 * refractivities at 0 °C and 760 mm.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, where every formula's pole lies outside it.
 * @throws RangeError if the wavelength lies at or past a formula's pole.
 */
export function gasMolarRefractionCm3PerMol(
  gases: ReadonlyArray<OneTermRefractivity>,
  wavelengthNm: number,
): number {
  const inverseUm2 = (1000 / wavelengthNm) ** 2;
  let molarRefraction = 0;
  for (const gas of gases) {
    if (!(gas.bPerUm2 > inverseUm2)) {
      throw new RangeError(`${wavelengthNm} nm lies at or past a pole of ${gas.source}`);
    }
    const n = 1 + gas.aPerUm2 / (gas.bPerUm2 - inverseUm2);
    molarRefraction +=
      lorentzLorenzFactor(n) * gas.compressibility * IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL;
  }
  return molarRefraction;
}

/**
 * A solid's ionic increment δ, its excess molar refraction over its constituent gases':
 * (n² − 1) ÷ (n² + 2) × V_m ÷ R − 1, with R their {@link gasMolarRefractionCm3PerMol}.
 *
 * @param wavelengthNm - The vacuum wavelength the solid's index is measured at, nm.
 * @param molarVolumeCm3PerMol - The solid's molar volume V_m.
 * @param index - The solid's real index at the wavelength.
 * @throws RangeError as {@link gasMolarRefractionCm3PerMol} does.
 */
export function ionicIncrement(
  gases: ReadonlyArray<OneTermRefractivity>,
  wavelengthNm: number,
  molarVolumeCm3PerMol: number,
  index: number,
): number {
  return (
    (lorentzLorenzFactor(index) * molarVolumeCm3PerMol) /
      gasMolarRefractionCm3PerMol(gases, wavelengthNm) -
    1
  );
}

/** NH₄Cl's index and density, the ionic increment's calibration. */
export interface IonicCalibration {
  /** n_D, the real index at the sodium D lines. */
  readonly indexD: number;
  /** ρ, g cm⁻³. */
  readonly densityGPerCm3: number;
  /** M, g mol⁻¹. */
  readonly molarMassGPerMol: number;
  /** V_m = M ÷ ρ, cm³ mol⁻¹. */
  readonly molarVolumeCm3PerMol: number;
}

/** NH₄Cl's molar mass, g mol⁻¹, from CIAAW's conventional atomic weights (Cl 35.45). */
const NH4CL_MOLAR_MASS_G_PER_MOL = 14.007 + 4 * 1.008 + 35.45;

/**
 * NH₄Cl's molar volume, cm³ mol⁻¹, from its X-ray cell: H. E. Swanson and E. Tatge, Standard X-ray
 * Diffraction Powder Patterns, NBS Circular 539, vol. 1 (1953), pp. 59–61 (the cell on p. 61):
 * Pm3̄m, a = 3.8756 Å at 26 °C, one formula unit. That is 35.056 cm³ mol⁻¹, 1.5258 g cm⁻³ at M =
 * 53.489 g mol⁻¹ (science-r08-gases-2.md §2.1).
 *
 * @remarks
 * The Lorentz–Lorenz molar volume is the cell's, as NH₄SH's is West's cell
 * ({@link NH4SH_MOLAR_VOLUME_CM3_PER_MOL}). The circular's own 1.527 g cm⁻³ does not follow from its
 * cell at the period's constants or today's (1.5254–1.5259), and is not used. The Handbook of
 * Mineralogy's sal ammoniac gives the same cell (its source is the circular). The bulk densities,
 * the CRC Handbook's 1.519 and the Handbook of Mineralogy's measured 1.532, bracket the cell's and
 * are not used. The index ({@link nh4clIndex}) is at room temperature, unstated, so the two may
 * differ by a few kelvin. ±0.1% in V_m, the ruling's estimate, covers about ±12 K at Adenstedt
 * 1936's linear expansion of 26.9 × 10⁻⁶ K⁻¹ at −10 °C (as Gopalakrishnan 1957's Table III quotes
 * it), and moves the increment by ±0.001 and NH₄SH's n by about ±0.0008, inside its band.
 */
export const NH4CL_MOLAR_VOLUME_CM3_PER_MOL = cellMolarVolumeCm3PerMol(3.8756 ** 3, 1);

/** The sodium D lines' mean, 589.3 nm, as the Cuthbertsons' air wavelengths are read. */
const SODIUM_D_NM = 589.3;

/** The shortest wavelength of {@link nh4clIndex}'s fit, nm: 3130 Å, as the paper states its range. */
const NH4CL_DISPERSION_SHORTEST_NM = 313;

/** The longest wavelength of {@link nh4clIndex}'s fit, nm: 7682 Å, as the paper states its range. */
const NH4CL_DISPERSION_LONGEST_NM = 768.2;

/**
 * NH₄Cl's real index at a wavelength: C. P. Gopalakrishnan, "The thermo-optic behaviour of calcium
 * fluoride, ammonium chloride and zinc-blende at low temperatures", J. Indian Inst. Sci. 39 (1957)
 * 8–12, eq. 7, his fit to the dispersion that G. Poinsot and J.-P. Mathieu measured ("Dispersion de
 * réfraction de quelques cristaux cubiques", Ann. Phys. (Paris) 12e sér., 10 (1955) 481–486, DOI
 * 10.1051/anphys/195510010481), with λ in µm: n² − 1 = 0.295904 + 0.944147 λ² ÷ (λ² − 0.1200²) +
 * 0.376091 λ² ÷ (λ² − 0.1600²) + 4.3044 λ² ÷ (λ² − 69.0²).
 *
 * @remarks
 * The paper states that the formula agrees with the measurements from 3130 to 7682 Å, and that its
 * static limit, 6.92, matches the measured dielectric constant, 6.90. It gives 1.6391 at 589.3 nm,
 * against the Handbook of Mineralogy's 1.639(1) and the 1.641 that NBS Circular 539 measured on its
 * own sample by immersion. Poinsot and Mathieu's paper could not be read, so their data are read
 * through this fit (science-r08-gases-2.md §2.1). The wavelengths are taken as the paper's, in air,
 * as {@link SODIUM_D_NM} is: read as vacuum wavelengths, n moves by under 2 × 10⁻⁵.
 *
 * @param wavelengthNm - The wavelength, nm, within the fit's 313–768.2 nm.
 * @throws RangeError outside the fit's range.
 */
export function nh4clIndex(wavelengthNm: number): number {
  const inFit =
    wavelengthNm >= NH4CL_DISPERSION_SHORTEST_NM && wavelengthNm <= NH4CL_DISPERSION_LONGEST_NM;
  if (!inFit) {
    throw new RangeError(
      `${wavelengthNm} nm lies outside NH4Cl's dispersion fit, ${NH4CL_DISPERSION_SHORTEST_NM}–${NH4CL_DISPERSION_LONGEST_NM} nm`,
    );
  }
  const um2 = (wavelengthNm / 1000) ** 2;
  const nSquaredMinusOne =
    0.295_904 +
    (0.944_147 * um2) / (um2 - 0.12 ** 2) +
    (0.376_091 * um2) / (um2 - 0.16 ** 2) +
    (4.3044 * um2) / (um2 - 69 ** 2);
  return Math.sqrt(1 + nSquaredMinusOne);
}

/**
 * NH₄Cl's index and density, the ionic increment's calibration (science-r08-gases-2.md §2): n_D =
 * 1.6391 from {@link nh4clIndex}, and V_m = 35.056 cm³ mol⁻¹ (1.5258 g cm⁻³) from the X-ray cell
 * ({@link NH4CL_MOLAR_VOLUME_CM3_PER_MOL}), at M = 53.489 g mol⁻¹.
 *
 * @remarks
 * They replace science-r08-nonspherical.md §3.1's n_D 1.642 (Chemister.ru) and 1.519 g cm⁻³ (the
 * CRC Handbook), both read through Wikipedia, which gave +3.51%. The X-ray cell alone, keeping
 * 1.642, gives +3.05%; NBS Circular 539's own 1.641 on the cell gives +2.93%.
 */
export const NH4CL_CALIBRATION: IonicCalibration = {
  indexD: nh4clIndex(SODIUM_D_NM),
  densityGPerCm3: NH4CL_MOLAR_MASS_G_PER_MOL / NH4CL_MOLAR_VOLUME_CM3_PER_MOL,
  molarMassGPerMol: NH4CL_MOLAR_MASS_G_PER_MOL,
  molarVolumeCm3PerMol: NH4CL_MOLAR_VOLUME_CM3_PER_MOL,
};

/**
 * NH₄Cl's ionic increment over NH₃ plus HCl, re-derived from {@link NH3_REFRACTIVITY} and
 * {@link HCL_REFRACTIVITY} (the same paper) at the D line: +2.68%.
 *
 * @remarks
 * Re-derived with the dispersion at 486.1, 546.1, 589.3 and 656.3 nm it is +2.681% to +2.688%, flat
 * across the measured range, which supports a single increment.
 */
export const NH4CL_IONIC_INCREMENT = ionicIncrement(
  [NH3_REFRACTIVITY, HCL_REFRACTIVITY],
  SODIUM_D_NM,
  NH4CL_CALIBRATION.molarVolumeCm3PerMol,
  NH4CL_CALIBRATION.indexD,
);

/**
 * NH₄SH's ionic increment of molar refraction over NH₃'s plus H₂S's, +2.7%: ammonium chloride's,
 * {@link NH4CL_IONIC_INCREMENT} (+2.68%), as science-r08-nonspherical.md §3.1 rules.
 *
 * @remarks
 * The ruling's +4.5% (+4.3% to +4.7%) took NH₃ at the real gas's Z, 0.984 798, and HCl's index
 * from a secondary source with Z from 0.9924 to 1; with NH₃'s formula read at its theoretic density
 * and HCl from the same paper, the increment was +3.51% (R08's Risks, "Deviations in T3.b's
 * follow-up, as built"). NH₄Cl's index and density from primary data, Poinsot and Mathieu's
 * dispersion and the X-ray cell, lower it to +2.68% (science-r08-gases-2.md §2). SH⁻ is more
 * polarisable than Cl⁻, and NH₄Br's increment is about +11% (the ruling's estimate, from memory),
 * so the stated band runs from 0 to +12%.
 */
export const NH4SH_IONIC_INCREMENT = 0.027;

/** The stated uncertainty band of {@link NH4SH_IONIC_INCREMENT}, 0 to +12%, as fractions. */
export const NH4SH_IONIC_INCREMENT_BAND: readonly [number, number] = [0, 0.12];

/**
 * A solid's real index by Lorentz–Lorenz from its constituents' gas refractivities: the additive
 * molar refraction R ({@link gasMolarRefractionCm3PerMol}) raised by an ionic increment δ, then
 * x = (1 + δ) R ÷ V_m and n = √((1 + 2x) ÷ (1 − x)).
 *
 * @param wavelengthNm - The vacuum wavelength, nm, where every formula's pole lies outside it.
 * @param molarVolumeCm3PerMol - The solid's molar volume V_m.
 * @param increment - δ, the solid's excess molar refraction over the gases' sum.
 * @throws RangeError if the wavelength lies at or past a formula's pole, or x is not in (0, 1).
 */
export function lorentzLorenzIndex(
  gases: ReadonlyArray<OneTermRefractivity>,
  wavelengthNm: number,
  molarVolumeCm3PerMol: number,
  increment: number,
): number {
  const x =
    ((1 + increment) * gasMolarRefractionCm3PerMol(gases, wavelengthNm)) / molarVolumeCm3PerMol;
  if (!(x > 0 && x < 1)) {
    throw new RangeError(`Lorentz–Lorenz: x = ${x} is not in (0, 1)`);
  }
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/**
 * NH₄SH's estimated real index at a vacuum wavelength, nm (science-r08-nonspherical.md §3.1):
 * {@link lorentzLorenzIndex} over NH₃ and H₂S at NH₄SH's cell, with NH₄Cl's ionic increment unless
 * another is given.
 *
 * @throws RangeError as {@link lorentzLorenzIndex} does, for a wavelength at or past NH₃'s pole
 *   (about 105 nm).
 */
export function nh4shIndex(wavelengthNm: number, increment = NH4SH_IONIC_INCREMENT): number {
  return lorentzLorenzIndex(
    [NH3_REFRACTIVITY, H2S_REFRACTIVITY],
    wavelengthNm,
    NH4SH_MOLAR_VOLUME_CM3_PER_MOL,
    increment,
  );
}

/**
 * NH₃ ice's molar volume, cm³ mol⁻¹, the method's check on a measured molecular solid: the cubic
 * cell of I. Olovsson and D. H. Templeton, Acta Cryst. 12 (1959) 832 (COD 2310927), a = 5.138 Å with
 * four molecules.
 */
export const NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL = cellMolarVolumeCm3PerMol(5.138 ** 3, 4);

/**
 * How far NH₄SH's estimate moves over {@link NH4SH_IONIC_INCREMENT_BAND} on the file's grid: the
 * largest rise to the band's top and the largest fall to its foot.
 */
export function nh4shBandSpan(): { readonly above: number; readonly below: number } {
  const [low, high] = NH4SH_IONIC_INCREMENT_BAND;
  let above = 0;
  let below = 0;
  for (const nm of MATERIAL_GRID_NM) {
    const n = nh4shIndex(nm);
    above = Math.max(above, nh4shIndex(nm, high) - n);
    below = Math.max(below, n - nh4shIndex(nm, low));
  }
  return { above, below };
}

/**
 * A gas's measured refractivity at the state its source states: the tool's own copy of a row of
 * `view/atmosphere/rayleigh.ts`'s `GAS_DISPERSION`, which `view/atmosphere/materials/materials.test.ts`
 * holds to the files made from it.
 */
export interface GasDispersionCopy {
  /** n − 1 at the reference state, at a vacuum wavelength, nm. */
  readonly nMinusOne: (wavelengthNm: number) => number;
  /** The temperature the formula is stated at, K. */
  readonly referenceK: number;
  /** The pressure the formula is stated at, Pa. */
  readonly referencePa: number;
  /** The compressibility factor Z of the gas at the reference state. */
  readonly compressibility: number;
  readonly source: string;
}

/** σ² in µm⁻² at a vacuum wavelength, nm. */
function wavenumberSquaredPerUm2(wavelengthNm: number): number {
  const perUm = 1_000 / wavelengthNm;
  return perUm * perUm;
}

/** σ² in cm⁻² at a vacuum wavelength, nm. */
function wavenumberSquaredPerCm2(wavelengthNm: number): number {
  const perCm = 1e7 / wavelengthNm;
  return perCm * perCm;
}

/** Where `rayleigh.ts`'s N₂ row passes from Bates 1984's ultraviolet branch to Peck and Khanna's, nm. */
const N2_BRANCH_NM = 468;

/**
 * N₂'s refractivity, as `rayleigh.ts` holds it: Peck and Khanna, J. Opt. Soc. Am. 56 (1966) 1059,
 * 10⁸(n − 1) = 6,497.378 + 3,073,864.9 ÷ (144 − σ²) from 468 nm; below, Bates, Planet. Space Sci. 32
 * (1984) 785, 10⁸(n − 1) = 5,677.465 + 318.818 74 × 10¹² ÷ (14.4 × 10⁹ − ν²), as Sneep and Ubachs
 * 2005 (eq. 11) print it; both at 15 °C and 101,325 Pa, Z = 0.999 715.
 */
export const N2_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    wavelengthNm >= N2_BRANCH_NM
      ? (6_497.378 + 3_073_864.9 / (144 - wavenumberSquaredPerUm2(wavelengthNm))) * 1e-8
      : (5_677.465 + 318.818_74e12 / (14.4e9 - wavenumberSquaredPerCm2(wavelengthNm))) * 1e-8,
  referenceK: 288.15,
  referencePa: 101_325,
  compressibility: 0.999_715,
  source:
    "Peck and Khanna, J. Opt. Soc. Am. 56 (1966) 1059, from 468 nm; below, Bates, Planet. Space Sci. 32 (1984) 785",
};

/**
 * O₂'s refractivity, as `rayleigh.ts` holds it: Křen, Appl. Opt. 50 (2011) 6484, refitting Zhang,
 * Lu and Wang, Appl. Opt. 47 (2008) 3143, n − 1 = 1.181 494 × 10⁻⁴ + 9.708 931 × 10⁻³ ÷ (75.4 −
 * σ²), at 20 °C and 101,325 Pa, Z = 0.999 282.
 */
export const O2_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    1.181_494e-4 + 9.708_931e-3 / (75.4 - wavenumberSquaredPerUm2(wavelengthNm)),
  referenceK: 293.15,
  referencePa: 101_325,
  compressibility: 0.999_282,
  source:
    "Křen, Appl. Opt. 50 (2011) 6484, refitting Zhang, Lu and Wang, Appl. Opt. 47 (2008) 3143",
};

/**
 * CO's refractivity, as `rayleigh.ts` holds it: C. and M. Cuthbertson, Proc. R. Soc. Lond. A 97
 * (1920) 152, n − 1 = 0.040 508 ÷ (123.77 − σ²), at 0 °C and 101,325 Pa, Z = 0.999 54.
 */
export const CO_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) => 0.040_508 / (123.77 - wavenumberSquaredPerUm2(wavelengthNm)),
  referenceK: 273.15,
  referencePa: 101_325,
  compressibility: 0.999_54,
  source: "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 97 (1920) 152",
};

/** H₂S's refractivity, {@link H2S_REFRACTIVITY}, at its 0 °C and 101,325 Pa. */
export const H2S_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    H2S_REFRACTIVITY.aPerUm2 / (H2S_REFRACTIVITY.bPerUm2 - wavenumberSquaredPerUm2(wavelengthNm)),
  referenceK: 273.15,
  referencePa: 101_325,
  compressibility: H2S_REFRACTIVITY.compressibility,
  source: H2S_REFRACTIVITY.source,
};

/**
 * Ar's refractivity, as `rayleigh.ts` holds it, for the method's check: Peck and Fisher, J. Opt. Soc.
 * Am. 54 (1964) 1362, 10⁷(n − 1) = 643.2135 + 286,060.21 ÷ (144 − σ²), at 15 °C and 101,325 Pa,
 * Z = 0.999 26.
 */
export const AR_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    (643.213_5 + 286_060.21 / (144 - wavenumberSquaredPerUm2(wavelengthNm))) * 1e-7,
  referenceK: 288.15,
  referencePa: 101_325,
  compressibility: 0.999_26,
  source: "Peck and Fisher, J. Opt. Soc. Am. 54 (1964) 1362",
};

/**
 * C₂H₆'s refractivity, as `rayleigh.ts` holds it, for liquid ethane's estimate (R08.T5.g): Loria,
 * Ann. Phys. 334 (1909) 605, Table IX, n − 1 = a(1 + b ÷ λ²) with refractiveindex.info's refit a =
 * 7.330 × 10⁻⁴ and Loria's b = 9.308 × 10⁻³ µm², at 0 °C and 760 mm, measured over 523–667.7 nm,
 * Z = 0.998 97.
 */
export const C2H6_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) => 7.33e-4 * (1 + 9.308e-3 * wavenumberSquaredPerUm2(wavelengthNm)),
  referenceK: 273.15,
  referencePa: 101_325,
  compressibility: 0.998_97,
  source: "Loria, Ann. Phys. 334 (1909) 605, Table IX, with refractiveindex.info's refit of a",
};

/**
 * CH₄'s refractivity, as `rayleigh.ts` holds it, for the liquid method's check (R08.T5.g): He, Fang,
 * Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 10, 10⁸(n − 1) = 3,603.09 +
 * 4.403 62 × 10¹⁴ ÷ (1.1741 × 10¹⁰ − ν²), at 15 °C and 101,325 Pa, Z = 1.
 */
export const CH4_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    (3_603.09 + 4.403_62e14 / (1.174_1e10 - wavenumberSquaredPerCm2(wavelengthNm))) * 1e-8,
  referenceK: 288.15,
  referencePa: 101_325,
  compressibility: 1,
  source: "He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 10",
};

/**
 * A gas row's Lorentz–Lorenz molar refraction at a vacuum wavelength, nm, cm³ mol⁻¹: (n² − 1) ÷
 * (n² + 2) × Z N_A k_B T ÷ p at the row's own reference state.
 */
export function gasRowMolarRefractionCm3PerMol(
  gas: GasDispersionCopy,
  wavelengthNm: number,
): number {
  const gasMolarVolumeCm3PerMol =
    ((gas.compressibility * AVOGADRO_PER_MOL * BOLTZMANN_J_PER_K * gas.referenceK) /
      gas.referencePa) *
    1e6;
  return lorentzLorenzFactor(1 + gas.nMinusOne(wavelengthNm)) * gasMolarVolumeCm3PerMol;
}

/**
 * A molecular ice's real index by Lorentz–Lorenz from its gas's refractivity (decision-r08-t5d
 * ruling 5, the method of NH₄SH's estimate with no increment, since the solid is molecular): the
 * molar refraction R = (n² − 1) ÷ (n² + 2) × Z × N_A k_B T ÷ p at the gas's reference state, then x =
 * R ÷ V_m and n = √((1 + 2x) ÷ (1 − x)).
 *
 * @throws RangeError if x is not in (0, 1).
 */
export function molecularIceIndex(
  gas: GasDispersionCopy,
  wavelengthNm: number,
  molarVolumeCm3PerMol: number,
): number {
  const x = gasRowMolarRefractionCm3PerMol(gas, wavelengthNm) / molarVolumeCm3PerMol;
  if (!(x > 0 && x < 1)) {
    throw new RangeError(`Lorentz–Lorenz: x = ${x} is not in (0, 1)`);
  }
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/** A hexagonal cell's volume, Å³, from a and c, Å. */
function hexagonalCellA3(aA: number, cA: number): number {
  return (Math.sqrt(3) / 2) * aA * aA * cA;
}

/** A molecular ice's crystal cell, the stand-in's density, as the Crystallography Open Database gives it. */
export interface IceCell {
  /** The phase in words, and its space group. */
  readonly phase: string;
  readonly cellVolumeA3: number;
  /** Molecules per cell. */
  readonly formulaUnits: number;
  /** The cell's temperature, K, where its paper states one. */
  readonly temperatureK: number | null;
  /** The cell's paper, its COD entry, URL, SHA-256 and the date fetched. */
  readonly source: string;
  /** The entry's own terms, as its file states them. */
  readonly terms: string;
}

/** The terms of a database entry its contributors placed in the public domain. */
const COD_PUBLIC_DOMAIN = "placed in the public domain by its contributors (the entry's notice)";

/**
 * The terms of a database entry taken from the American Mineralogist Crystal Structure Database:
 * use with attribution to the journal article, which the cell's source gives.
 */
const AMCSD_TERMS =
  "from the American Mineralogist Crystal Structure Database, usable with attribution to the journal article, which is given";

function codSource(paper: string, entry: string, sha256: string): string {
  return `${paper}, as the Crystallography Open Database gives it, entry ${entry}, https://www.crystallography.net/cod/${entry}.cif, SHA-256 ${sha256}, fetched ${FETCHED_T5D}`;
}

/** β-N₂, hexagonal P6₃/mmc, a = 4.050 Å, c = 6.604 Å, two molecules, at 46 K. */
export const N2_ICE_CELL: IceCell = {
  phase: "hexagonal β-N2 (P6₃/mmc)",
  cellVolumeA3: hexagonalCellA3(4.05, 6.604),
  formulaUnits: 2,
  temperatureK: 46,
  source: codSource(
    "A. F. Schuch and R. L. Mills, J. Chem. Phys. 52 (1970) 6000, DOI 10.1063/1.1672899",
    "9012476",
    "c1bae956ec04070696962f7e2c4ecd1a2328f27b91081b92536ba74a72f1e96c",
  ),
  terms: AMCSD_TERMS,
};

/** γ-O₂, cubic Pm3̄n, a = 6.78 Å, eight molecules, at 46 K (stable over 43.8–54.4 K). */
export const O2_ICE_CELL: IceCell = {
  phase: "cubic γ-O2 (Pm3̄n)",
  cellVolumeA3: 6.78 ** 3,
  formulaUnits: 8,
  temperatureK: 46,
  source: codSource(
    "D. E. Cox, E. J. Samuelsen and K. H. Beckurts, Phys. Rev. B 7 (1973) 3102, DOI 10.1103/PhysRevB.7.3102",
    "9011647",
    "9d555a0f14de639e2302d0d40f2b46616c82818e470e8661366212c4de36631d",
  ),
  terms: AMCSD_TERMS,
};

/**
 * α-CO, cubic P2₁3, a = 5.63 Å, four molecules, temperature unstated: β-CO's hexagonal cell is not
 * in the database. Published before 1947, possibly in kX units (1 kX = 1.002 02 Å).
 */
export const CO_ICE_CELL: IceCell = {
  phase: "cubic α-CO (P2₁3)",
  cellVolumeA3: 5.63 ** 3,
  formulaUnits: 4,
  temperatureK: null,
  source: codSource(
    "L. Vegard, Z. Phys. 61 (1930) 185, DOI 10.1007/BF01339658",
    "1010305",
    "65728fcee2c0f05fe740ef2fa23a698d6e632da7cda433bff1a3d933fb94c196",
  ),
  terms: COD_PUBLIC_DOMAIN,
};

/**
 * H₂S phase I, cubic Fm3̄m, a = 5.797 Å, four molecules, temperature unstated: the phase nearest the
 * triple point (187.7 K). Published before 1947, possibly in kX units.
 */
export const H2S_ICE_CELL: IceCell = {
  phase: "cubic H2S phase I (Fm3̄m)",
  cellVolumeA3: 5.797 ** 3,
  formulaUnits: 4,
  temperatureK: null,
  source: codSource(
    "L. Vegard and L. Sinding-Oserød, Avh. Nor. Vidensk.-Akad. Oslo, Mat.-Naturvidensk. Kl. 1942, 1",
    "1539470",
    "4b8ac2109ea328521f0516ba4c1f22ccdcd93c062024d4a69c8df450aa56dbc2",
  ),
  terms: COD_PUBLIC_DOMAIN,
};

/**
 * Solid Ar's cell for the method's check: cubic Fm3̄m, a = 5.4 Å, four atoms (E. R. Dobbs, B. F.
 * Figgins, G. O. Jones, D. C. Piercey and D. P. Riley, Nature 178 (1956) 483, COD 1541541),
 * temperature unstated.
 */
export const AR_ICE_CELL: IceCell = {
  phase: "cubic solid Ar (Fm3̄m)",
  cellVolumeA3: 5.4 ** 3,
  formulaUnits: 4,
  temperatureK: null,
  source: codSource(
    "E. R. Dobbs, B. F. Figgins, G. O. Jones, D. C. Piercey and D. P. Riley, Nature 178 (1956) 483, DOI 10.1038/178483a0",
    "1541541",
    "4ac3ecee395a4f006797f4d74469aa7167deaa78a1c68c1b6c4c0bc17122fd23",
  ),
  terms: COD_PUBLIC_DOMAIN,
};

/**
 * Amorphous H₂S ice's measured index and density, the method's check against a measured film:
 * Y. Y. Yarnall and R. L. Hudson, Astrophys. J. Lett. 931 (2022) L4, DOI
 * 10.3847/2041-8213/ac6b32 (CC BY 4.0), Table 1, n 1.407 at 670 nm and 0.944 g cm⁻³ for ice
 * prepared and studied at 19 K.
 */
export const H2S_FILM = { wavelengthNm: 670, index: 1.407, densityGPerCm3: 0.944 } as const;

/** H₂S's molar mass, g mol⁻¹, from CIAAW's conventional atomic weights (H 1.008, S 32.06). */
export const H2S_MOLAR_MASS_G_PER_MOL = 2 * 1.008 + 32.06;

/** Sinnock and Smith 1969's measured n of solid Ar at 546.1 nm and 80 K, the method's check (their table, as refractiveindex.info gives it). */
export const AR_ICE_MEASURED_546 = 1.2708;

/** A molecular ice's estimated index at a wavelength, nm, at its cell. */
export function iceIndex(gas: GasDispersionCopy, cell: IceCell, wavelengthNm: number): number {
  return molecularIceIndex(
    gas,
    wavelengthNm,
    cellMolarVolumeCm3PerMol(cell.cellVolumeA3, cell.formulaUnits),
  );
}

/** The method's checks on measured solids, as each estimate's `source` states them. */
function iceMethodChecks(): string {
  const argon = iceIndex(AR_DISPERSION, AR_ICE_CELL, 546.1);
  const ammonia = lorentzLorenzIndex([NH3_REFRACTIVITY], 550, NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL, 0);
  return `The method's checks on measured solids: Ar from its gas dispersion (${AR_DISPERSION.source}) at the cell of ${AR_ICE_CELL.source} (a = 5.4 Å, four atoms) gives ${argon.toFixed(4)} against Sinnock and Smith 1969's measured ${AR_ICE_MEASURED_546} at 546.1 nm and 80 K (Phys. Rev. 181, 1297); NH3 ice ${ammonia.toFixed(3)} against Martonchik et al. 1984's 1.436 at 550 nm (Appl. Opt. 23, 541); and H2S ice at the density of an amorphous film at 19 K, ${H2S_FILM.densityGPerCm3} g cm⁻³, ${h2sFilmEstimate().toFixed(3)} against the ${H2S_FILM.index} Yarnall and Hudson 2022 measure there at ${H2S_FILM.wavelengthNm} nm (Astrophys. J. Lett. 931, L4; CC BY 4.0), each above the measurement`;
}

/** H₂S ice's estimate at {@link H2S_FILM}'s wavelength and density, the method's third check. */
export function h2sFilmEstimate(): number {
  return molecularIceIndex(
    H2S_DISPERSION,
    H2S_FILM.wavelengthNm,
    H2S_MOLAR_MASS_G_PER_MOL / H2S_FILM.densityGPerCm3,
  );
}

/** What one molecular ice's estimate states. */
interface IceEstimate {
  readonly key: string;
  readonly gas: GasDispersionCopy;
  readonly cell: IceCell;
  /** The gas row's measured range, nm, beyond which it is extrapolated. */
  readonly measuredNm: readonly [number, number];
  /** Why no measured index is taken. */
  readonly noMeasurement: string;
  /** What else the file states it leaves out (O₂'s weak bands). */
  readonly notDrawn: string | null;
  /** Why it stands in, as its `standIn` says it, in player text. */
  readonly standInReason: string;
}

/** An estimated ice's material file. */
function iceEstimateFile(estimate: IceEstimate): MaterialFile {
  const { key, gas, cell } = estimate;
  const at = (nm: number): number => iceIndex(gas, cell, nm);
  const molarVolume = cellMolarVolumeCm3PerMol(cell.cellVolumeA3, cell.formulaUnits);
  const temperature = cell.temperatureK === null ? "" : ` at ${cell.temperatureK} K`;
  return {
    key,
    phase: "solid",
    variant: null,
    name: `${key} ice${temperature} (an estimated index)`,
    temperatureK: cell.temperatureK,
    shape: "crystal",
    provenance: "standIn",
    paper: `the estimate's inputs: ${gas.source}, for ${key}'s gas dispersion; ${cell.source}, for the cell`,
    source: `a Lorentz–Lorenz estimate, not a measurement (decision-r08-t5d rulings 3 and 5): the molar refraction of ${key} from its gas dispersion as stated at ${gas.referenceK} K and ${gas.referencePa} Pa (view/atmosphere/rayleigh.ts's row, Z = ${gas.compressibility}), with no increment, since the solid is molecular, over the molar volume of ${cell.phase}${temperature} (${molarVolume.toFixed(2)} cm³ mol⁻¹ from ${cell.cellVolumeA3.toFixed(3)} Å³ and ${cell.formulaUnits} molecules). The gas dispersion is measured over ${estimate.measuredNm[0]}–${estimate.measuredNm[1]} nm and extrapolated beyond. ${estimate.noMeasurement} ${iceMethodChecks()}. k = 0${estimate.notDrawn === null ? "" : `; ${estimate.notDrawn}`}.`,
    licence: `estimated values from cited constants and a crystal cell from the Crystallography Open Database (its entry ${cell.terms}); no data set is copied (decision-r08-t5d ruling 5)`,
    reduction:
      "n = √((1 + 2x) ÷ (1 − x)), with x the gas's molar refraction over the solid's molar volume, evaluated on 380–780 nm every 5 nm to six significant figures; k = 0",
    standIn: `a non-absorbing crystal whose real index is a Lorentz–Lorenz estimate from the gas's dispersion at the crystal's density, since ${estimate.standInReason}: ${at(380).toFixed(3)} at 380 nm, ${at(550).toFixed(3)} at 550 nm and ${at(780).toFixed(3)} at 780 nm`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(at(nm))),
    k: MATERIAL_GRID_NM.map(() => 0),
  };
}

/** The molecular ices R08.T5.d estimates (decision-r08-t5d rulings 3 and 5). */
export const ICE_ESTIMATES: ReadonlyArray<IceEstimate> = [
  {
    key: "H2S",
    gas: H2S_DISPERSION,
    cell: H2S_ICE_CELL,
    measuredNm: [486.1, 656.3],
    noMeasurement:
      "The one open measurement found, Yarnall and Hudson 2022's n at 670 nm, is of amorphous ice deposited at 19 K, a film less dense than the crystal, not the crystal's visible dispersion; it is the method's third check below.",
    notDrawn: null,
    standInReason:
      "only one wavelength of an amorphous film's index is measured openly, not the crystal's visible dispersion",
  },
  {
    key: "N2",
    gas: N2_DISPERSION,
    cell: N2_ICE_CELL,
    measuredNm: [254, 2_058.6],
    noMeasurement:
      "The measured visible n of N2 ice (Satorre et al. 2008, Planet. Space Sci. 56, 1748; Luna et al. 2012, Icarus 221, 186; at 632.8 nm, of films deposited cold) could not be read: closed.",
    notDrawn: null,
    standInReason: "no open visible optical constants of N2 ice were found",
  },
  {
    key: "CO",
    gas: CO_DISPERSION,
    cell: CO_ICE_CELL,
    measuredNm: [480, 670.8],
    noMeasurement: "No open visible optical constants of CO ice were found.",
    notDrawn: null,
    standInReason: "no open visible optical constants of CO ice were found",
  },
  {
    key: "O2",
    gas: O2_DISPERSION,
    cell: O2_ICE_CELL,
    measuredNm: [400, 1_800],
    noMeasurement:
      "No open visible optical constants of O2 ice were found (refractiveindex.info's O2 entries are of the gas).",
    notDrawn: "solid O2's weak visible absorption bands, which make it pale blue, are not drawn",
    standInReason: "no open visible optical constants of O2 ice were found",
  },
];

/**
 * Crystalline S₈'s principal indices at the sodium D lines (C. Palache, H. Berman and C. Frondel,
 * Dana's System of Mineralogy, 7th edition, vol. I (1944) 140–144, as the Handbook of Mineralogy's
 * sulphur sheet gives them): n_α 1.9579, n_β 2.0377, n_γ 2.2452.
 */
export const S8_PRINCIPAL_INDICES: readonly [number, number, number] = [1.9579, 2.0377, 2.2452];

/**
 * S₈'s constant stand-in index (decision-r08-t5d ruling 5): the principal indices averaged in the
 * dielectric function for randomly oriented grains, √((n_α² + n_β² + n_γ²) ÷ 3).
 */
export const S8_STAND_IN_INDEX = Math.sqrt(
  S8_PRINCIPAL_INDICES.reduce((sum, n) => sum + n * n, 0) / 3,
);

/** A measured index at a wavelength, the input of a fit. */
export interface IndexPoint {
  /** The wavelength, µm (as its source gives it). */
  readonly wavelengthUm: number;
  readonly n: number;
}

/**
 * The least-squares two-term Cauchy line n = a + b ÷ λ² through points, its a and b rounded to six
 * significant figures, so that the file's n is the fit it states (R08.T5.g).
 *
 * @throws Error for fewer than two distinct wavelengths.
 */
export function fitCauchy(points: ReadonlyArray<IndexPoint>): Pick<CauchyFit, "a" | "bUm2"> {
  const xs = points.map((p) => 1 / (p.wavelengthUm * p.wavelengthUm));
  const meanX = xs.reduce((sum, x) => sum + x, 0) / xs.length;
  const meanY = points.reduce((sum, p) => sum + p.n, 0) / points.length;
  const sxx = xs.reduce((sum, x) => sum + (x - meanX) ** 2, 0);
  if (!(sxx > 0)) {
    throw new Error("fitCauchy: needs two distinct wavelengths");
  }
  const sxy = points.reduce(
    (sum, p, i) => sum + ((xs[i] ?? Number.NaN) - meanX) * (p.n - meanY),
    0,
  );
  const b = sxy / sxx;
  return { a: rounded(meanY - b * meanX), bUm2: rounded(b) };
}

/**
 * The method's checks on measured cryogenic liquids, each a Lorentz–Lorenz estimate from its gas's
 * dispersion at the liquid's density against its measured n (R08.T5.g): what liquid ethane's
 * estimate is judged by.
 */
export interface LiquidCheck {
  readonly what: string;
  readonly gas: GasDispersionCopy;
  /** The liquid's molar volume, cm³ mol⁻¹, from its density. */
  readonly molarVolumeCm3PerMol: number;
  readonly wavelengthNm: number;
  /** The measured n there. */
  readonly measured: number;
}

/**
 * Augustin 1915's absolute indices of liquid N₂ at −195.83 °C (77.32 K), ±5 × 10⁻⁵, at the 656.3,
 * 579.0, 546.1 and 435.9 nm lines (H. Augustin, "Über die Dichte des flüssigen Wasserstoffs, den
 * Brechungsexponenten und die Dispersion des flüssigen Wasserstoffs und des flüssigen
 * Stickstoffs", Ann. Phys. 351 (1915) 419–445, DOI 10.1002/andp.19153510307, p. 443, the
 * wavelengths from p. 438's table), as Zenodo record 1514490 gives the article (CC0).
 */
export const AUGUSTIN_N2: ReadonlyArray<IndexPoint> = [
  { wavelengthUm: 0.6563, n: 1.19841 },
  { wavelengthUm: 0.579, n: 1.19873 },
  { wavelengthUm: 0.5461, n: 1.19917 },
  { wavelengthUm: 0.4359, n: 1.20141 },
];

/** Liquid N₂'s derived index: a two-term Cauchy fit to {@link AUGUSTIN_N2}, with k = 0 (R08.T5.g). */
export const N2_LIQUID_FIT: CauchyFit = {
  form: "cauchy",
  ...fitCauchy(AUGUSTIN_N2),
  absorptionPerM: 0,
  fittedFromUm: 0.4359,
  fittedToUm: 0.6563,
};

/**
 * The NIST Chemistry WebBook's fluid-properties query (SRD 69) for a fluid's saturated liquid in
 * mol/l, by its CAS identifier and a temperature range, K: the URL the liquids' sources cite.
 */
function nistSaturation(id: string, low: string, high: string, step: string): string {
  return `https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=${id}&Type=SatP&Digits=7&THigh=${high}&TLow=${low}&TInc=${step}&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=mol%2Fl&HUnit=kJ%2Fmol&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`;
}

/**
 * Saturated liquid N₂'s molar density at Augustin's 77.32 K, mol/l: linear between the WebBook's
 * 28.83232 at 77 K and 28.77488 at 77.355 K (Span, Lemmon, Jacobsen, Wagner and Yokozeki, J. Phys.
 * Chem. Ref. Data 29 (2000) 1361, density uncertain by 0.02%).
 */
export const N2_LIQUID_MOLAR_DENSITY_MOL_PER_L =
  28.832_32 + ((77.32 - 77) / 0.355) * (28.774_88 - 28.832_32);

/** Where {@link N2_LIQUID_MOLAR_DENSITY_MOL_PER_L}'s rows were read. */
const N2_LIQUID_DENSITY_SOURCE = `${nistSaturation("C7727379", "77", "77.355", "0.355")}, SHA-256 5022a4ab7b30bde7942baf2b8c13fbcef6b6b9bf3319ae8f0fa5b001f216dad7, fetched ${FETCHED_T5G}`;

/**
 * Wong and Anderson 1972's n of saturated liquid NH₃ at 632.8 nm and its normal boiling point,
 * 239.80 K: 1.360 (L. Y. Wong and A. Anderson, "Refractive-index measurements of some
 * low-temperature liquids", J. Opt. Soc. Am. 62 (1972) 219–222, DOI 10.1364/JOSA.62.000219, Table II,
 * read from the journal's abstract page; the article is closed).
 */
export const NH3_LIQUID_MEASURED = { wavelengthNm: 632.8, n: 1.36, temperatureK: 239.8 } as const;

/**
 * Saturated liquid NH₃'s molar density at 239.80 K, mol/l: the WebBook's, after Gao, Wu, Bell,
 * Harvey and Lemmon, J. Phys. Chem. Ref. Data 52 (2023) 013102 (its 2018 manuscript, as the page
 * cites it), uncertain by 0.05% in the liquid.
 */
export const NH3_LIQUID_MOLAR_DENSITY_MOL_PER_L = 40.026_78;

/** Where {@link NH3_LIQUID_MOLAR_DENSITY_MOL_PER_L} was read. */
const NH3_LIQUID_DENSITY_SOURCE = `${nistSaturation("C7664417", "239.8", "240", "0.2")}, SHA-256 0a64ea66717939006d109c2f3707e652c4963f336ba1238c5fbb6d656d73a42c, fetched ${FETCHED_T5G}`;

/** A molar density, mol/l, as a molar volume, cm³ mol⁻¹. */
function molarVolumeOf(molPerL: number): number {
  return 1_000 / molPerL;
}

/** A one-term refractivity row as a {@link GasDispersionCopy}, at its 0 °C and 101,325 Pa. */
function oneTermCopy(row: OneTermRefractivity): GasDispersionCopy {
  return {
    nMinusOne: (wavelengthNm) =>
      row.aPerUm2 / (row.bPerUm2 - wavenumberSquaredPerUm2(wavelengthNm)),
    referenceK: 273.15,
    referencePa: 101_325,
    compressibility: row.compressibility,
    source: row.source,
  };
}

/** NH₃'s refractivity, {@link NH3_REFRACTIVITY}, at its 0 °C and 101,325 Pa. */
export const NH3_DISPERSION: GasDispersionCopy = oneTermCopy(NH3_REFRACTIVITY);

/**
 * Liquid NH₃'s derived index (R08.T5.g): a two-term Cauchy line whose slope b is the least-squares
 * fit to the Lorentz–Lorenz estimate from {@link NH3_DISPERSION} at the liquid's density over
 * 380–780 nm, and whose a meets {@link NH3_LIQUID_MEASURED}, with k = 0.
 */
export const NH3_LIQUID_FIT: CauchyFit = (() => {
  const molarVolume = molarVolumeOf(NH3_LIQUID_MOLAR_DENSITY_MOL_PER_L);
  const slope = fitCauchy(
    MATERIAL_GRID_NM.map((nm) => ({
      wavelengthUm: nm / 1000,
      n: molecularIceIndex(NH3_DISPERSION, nm, molarVolume),
    })),
  ).bUm2;
  const um = rounded(NH3_LIQUID_MEASURED.wavelengthNm / 1000);
  return {
    form: "cauchy",
    a: rounded(NH3_LIQUID_MEASURED.n - slope / (um * um)),
    bUm2: slope,
    absorptionPerM: 0,
    fittedFromUm: um,
    fittedToUm: um,
  };
})();

/** Liquid NH₃'s estimate against {@link NH3_LIQUID_MEASURED}: the method's fourth check. */
const NH3_LIQUID_CHECK: LiquidCheck = {
  what: "liquid NH3 at 239.80 K against Wong and Anderson 1972 (J. Opt. Soc. Am. 62, 219)",
  gas: NH3_DISPERSION,
  molarVolumeCm3PerMol: molarVolumeOf(NH3_LIQUID_MOLAR_DENSITY_MOL_PER_L),
  wavelengthNm: NH3_LIQUID_MEASURED.wavelengthNm,
  measured: NH3_LIQUID_MEASURED.n,
};

/**
 * Liquid NH₃'s Lorentz–Lorenz estimate at a wavelength, nm, shifted to meet
 * {@link NH3_LIQUID_MEASURED}: the curve {@link NH3_LIQUID_FIT} stands for.
 */
export function ammoniaLiquidLevelled(wavelengthNm: number): number {
  const molarVolume = molarVolumeOf(NH3_LIQUID_MOLAR_DENSITY_MOL_PER_L);
  return (
    molecularIceIndex(NH3_DISPERSION, wavelengthNm, molarVolume) -
    liquidCheckExcess(NH3_LIQUID_CHECK)
  );
}

/**
 * Saturated liquid CH₄'s molar density at its triple point, 90.694 K, mol/l (the WebBook's, after
 * Setzmann and Wagner 1991): the liquid checks' stand-in for Martonchik and Orton's 90 K.
 */
const CH4_LIQUID_TRIPLE_POINT_MOL_PER_L = 28.141_91;

/**
 * The method's checks on four measured cryogenic liquids (R08.T5.g), each molar density the NIST
 * Chemistry WebBook's at saturation: Ar at 90 K, 34.51052 mol/l (Tegeler, Span and Wagner, J. Phys.
 * Chem. Ref. Data 28 (1999) 779), against Sinnock and Smith 1969's 1.2269 at 546.1 nm
 * (refractiveindex.info's `main/Ar/nk/Sinnock-liquid-90K.yml`, CC0); N₂ at Augustin's 77.32 K
 * ({@link N2_LIQUID_MOLAR_DENSITY_MOL_PER_L}) against his 1.19917 at 546.1 nm; CH₄ at its triple point,
 * 90.694 K, 28.14191 mol/l (Setzmann and Wagner, J. Phys. Chem. Ref. Data 20 (1991) 1061), against
 * Martonchik and Orton 1994's 1.292 at 583.1 nm and 90 K (`methane-liquid.json`'s source); NH₃ at
 * 239.80 K against {@link NH3_LIQUID_MEASURED}.
 */
export const LIQUID_CHECKS: ReadonlyArray<LiquidCheck> = [
  {
    what: "liquid Ar at 90 K against Sinnock and Smith 1969 (Phys. Rev. 181, 1297)",
    gas: AR_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(34.510_52),
    wavelengthNm: 546.1,
    measured: 1.2269,
  },
  {
    what: "liquid N2 at 77.32 K against Augustin 1915 (Ann. Phys. 351, 419)",
    gas: N2_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(N2_LIQUID_MOLAR_DENSITY_MOL_PER_L),
    wavelengthNm: 546.1,
    measured: 1.19917,
  },
  {
    what: "liquid CH4 at 90.694 K against Martonchik and Orton 1994's 90 K (Appl. Opt. 33, 8306)",
    gas: CH4_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(CH4_LIQUID_TRIPLE_POINT_MOL_PER_L),
    wavelengthNm: 583.1,
    measured: 1.292,
  },
  NH3_LIQUID_CHECK,
];

/** Where the liquid checks' molar densities were read, each fetched {@link FETCHED_T5G}. */
const LIQUID_CHECK_DENSITY_SOURCES = `Ar ${nistSaturation("C7440371", "90", "91", "1")}, SHA-256 99a13ee0ffe92c6576e99fdd90c761427cb85a322e1347ce839fcbb6e9873a73, fetched ${FETCHED_T5G}; CH4 ${nistSaturation("C74828", "90", "91", "1")}, SHA-256 5b8e05aee257ebaf342dfce6836443eb61dc6dbe5968aa0aafd3ae4164525560, fetched ${FETCHED_T5G}; N2 ${N2_LIQUID_DENSITY_SOURCE}; NH3 ${NH3_LIQUID_DENSITY_SOURCE}`;

/** A liquid check's estimate less its measurement. */
export function liquidCheckExcess(check: LiquidCheck): number {
  return (
    molecularIceIndex(check.gas, check.wavelengthNm, check.molarVolumeCm3PerMol) - check.measured
  );
}

/** The liquid checks as a file's `source` states them. */
function liquidChecksText(): string {
  return LIQUID_CHECKS.map((check) => {
    const excess = liquidCheckExcess(check);
    return `${check.what}, +${excess.toFixed(4)} at ${check.wavelengthNm} nm`;
  }).join("; ");
}

/**
 * Saturated liquid C₂H₆'s molar density at 94 K, mol/l, Titan's surface temperature as the brief takes
 * it: the WebBook's, after D. Bücker and W. Wagner, J. Phys. Chem. Ref. Data 35 (2006) 205, DOI
 * 10.1063/1.1859286, uncertain by 0.02–0.04% in density from the melting line.
 */
export const C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L = 21.534_85;

/** Where {@link C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L} was read. */
const C2H6_LIQUID_DENSITY_SOURCE = `${nistSaturation("C74840", "94", "95", "1")}, SHA-256 32926b1df39c2492c1da071470aefa212e1c80dbb7bcd887991daecc4704bc2c, fetched ${FETCHED_T5G}`;

/** Liquid C₂H₆'s Lorentz–Lorenz estimate at a wavelength, nm, at 94 K (R08.T5.g). */
export function ethaneLiquidEstimate(wavelengthNm: number): number {
  return molecularIceIndex(
    C2H6_DISPERSION,
    wavelengthNm,
    molarVolumeOf(C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L),
  );
}

/**
 * Ethane's measured Lorentz–Lorenz function at 632.8 nm, cm³ g⁻¹: 0.3794 ± 0.0003, constant within
 * 0.5% over 1–200 amagat (Burton, "The Lorentz-Lorenz function of ethane", University of British
 * Columbia thesis, DOI 10.14288/1.0084897, CC BY, read from its abstract through OpenAlex): the
 * alternative R08.T5.g records against Loria's gas row.
 */
export const C2H6_BURTON_LORENTZ_LORENZ_CM3_PER_G = 0.3794;

/** C₂H₆'s molar mass, g mol⁻¹, from CIAAW's conventional atomic weights (C 12.011, H 1.008). */
const C2H6_MOLAR_MASS_G_PER_MOL = 2 * 12.011 + 6 * 1.008;

/** Burton's Lorentz–Lorenz function at liquid ethane's density at 94 K, as an index at 632.8 nm. */
export function ethaneBurtonIndex(): number {
  const x =
    (C2H6_BURTON_LORENTZ_LORENZ_CM3_PER_G *
      C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L *
      C2H6_MOLAR_MASS_G_PER_MOL) /
    1_000;
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/**
 * Pure liquid C₂H₆'s n, 1.3887 (R08.T5.h, decision-r08-t5g ruling 3): the largest index over the
 * pure constituents of Hagermann et al.'s mixture (their Table 3), ethane's by their text ("nitrogen
 * has the lowest refractive index and electrical permittivity, and ethane the highest", §4.2) (A.
 * Hagermann, J. C. Zarnecki, M. C. Towner, P. D. Rosenberg, R. D. Lorenz, M. R. Leese, B. Hathi and
 * A. J. Ball, "Physical properties as indicators of liquid compositions: Derivation of the
 * composition for Titan's surface liquids from the Huygens SSP measurements", MNRAS 359 (2005)
 * 637–642, DOI 10.1111/j.1365-2966.2005.08935.x, p. 641, read from the NASA ADS scan; cited as a
 * fact). Table 3 states no conditions and marks no source for it; it is taken at their Table 2's
 * 670 nm, 94 K and 1.5 bar, whose indices are Badoz et al. 1992's measurements
 * ({@link HAGERMANN_TABLE_2}).
 */
export const C2H6_LIQUID_MEASURED = { wavelengthNm: 670, n: 1.3887, temperatureK: 94 } as const;

/**
 * Liquid C₂H₆'s Lorentz–Lorenz estimate at a wavelength, nm, shifted to meet
 * {@link C2H6_LIQUID_MEASURED}: the curve {@link C2H6_LIQUID_FIT} stands for.
 */
export function ethaneLiquidLevelled(wavelengthNm: number): number {
  return (
    ethaneLiquidEstimate(wavelengthNm) -
    (ethaneLiquidEstimate(C2H6_LIQUID_MEASURED.wavelengthNm) - C2H6_LIQUID_MEASURED.n)
  );
}

/**
 * Liquid C₂H₆'s derived index (R08.T5.h), as liquid NH₃'s is made: a two-term Cauchy line whose slope
 * b is the least-squares fit to {@link ethaneLiquidEstimate} over 380–780 nm, and whose a meets
 * {@link C2H6_LIQUID_MEASURED}, with k = 0.
 */
export const C2H6_LIQUID_FIT: CauchyFit = (() => {
  const slope = fitCauchy(
    MATERIAL_GRID_NM.map((nm) => ({ wavelengthUm: nm / 1000, n: ethaneLiquidEstimate(nm) })),
  ).bUm2;
  const um = rounded(C2H6_LIQUID_MEASURED.wavelengthNm / 1000);
  return {
    form: "cauchy",
    a: rounded(C2H6_LIQUID_MEASURED.n - slope / (um * um)),
    bUm2: slope,
    absorptionPerM: 0,
    fittedFromUm: um,
    fittedToUm: um,
  };
})();

/** One row of Hagermann et al.'s Table 2: a mixture's composition and its measured properties. */
export interface TitanMixtureRow {
  /** C₂H₆'s mole fraction (read so; the table's header reads "vol. per cent"). */
  readonly ethaneMoleFraction: number;
  /** CH₄'s mole fraction. */
  readonly methaneMoleFraction: number;
  /** N₂'s mole fraction. */
  readonly nitrogenMoleFraction: number;
  /** The measured n at 670 nm, 94 K and 1.5 bar. */
  readonly n: number;
  /** The density, g cm⁻³. */
  readonly densityGPerCm3: number;
}

/**
 * Hagermann et al. 2005's Table 2 (p. 641; {@link C2H6_LIQUID_MEASURED}'s paper): five
 * methane–ethane–nitrogen liquids at 94 K and 1.5 bar, each index at 670 nm one the table marks as
 * Badoz et al. 1992's experimental value (J. Badoz, M. Le Liboux, R. Nahoum, G. Israël, F. Raulin
 * and J. P. Torre, Rev. Sci. Instrum. 63 (1992) 2967, DOI 10.1063/1.1142594: a cryogenic
 * refractometer at 670 nm, accurate to 10⁻³, its abstract), the densities the NIST database's.
 * The quantities their §2.2 solves for are mole fractions, Badoz et al. prepared their mixtures from
 * each gas's volume and pressure (their abstract), and the binary row's measured density follows
 * ideal mixing at mole fractions more closely than at volume fractions, so the compositions are read
 * as mole fractions, though the table's header reads "vol. per cent".
 */
export const HAGERMANN_TABLE_2: ReadonlyArray<TitanMixtureRow> = [
  {
    ethaneMoleFraction: 0.53,
    methaneMoleFraction: 0.47,
    nitrogenMoleFraction: 0,
    n: 1.3538,
    densityGPerCm3: 0.5686,
  },
  {
    ethaneMoleFraction: 0,
    methaneMoleFraction: 1,
    nitrogenMoleFraction: 0,
    n: 1.2872,
    densityGPerCm3: 0.4474,
  },
  {
    ethaneMoleFraction: 0.96,
    methaneMoleFraction: 0,
    nitrogenMoleFraction: 0.04,
    n: 1.3848,
    densityGPerCm3: 0.653,
  },
  {
    ethaneMoleFraction: 0.5,
    methaneMoleFraction: 0.44,
    nitrogenMoleFraction: 0.06,
    n: 1.3445,
    densityGPerCm3: 0.5807,
  },
  {
    ethaneMoleFraction: 0,
    methaneMoleFraction: 0.83,
    nitrogenMoleFraction: 0.17,
    n: 1.2724,
    densityGPerCm3: 0.4998,
  },
];

/** CH₄'s and N₂'s molar masses, g mol⁻¹, from CIAAW's conventional atomic weights (C 12.011, H 1.008, N 14.007). */
const CH4_MOLAR_MASS_G_PER_MOL = 12.011 + 4 * 1.008;
const N2_MOLAR_MASS_G_PER_MOL = 2 * 14.007;

/**
 * Saturated liquid CH₄'s molar density at 94 K, mol/l: 27.86634, the NIST Chemistry WebBook's (U.
 * Setzmann and W. Wagner, J. Phys. Chem. Ref. Data 20 (1991) 1061), for the Titan mixtures' check.
 */
export const CH4_LIQUID_94K_MOL_PER_L = 27.866_34;

/** Where {@link CH4_LIQUID_94K_MOL_PER_L} was read. */
const CH4_LIQUID_94K_SOURCE = `${nistSaturation("C74828", "94", "95", "1")}, SHA-256 f798ca4f529665d29368217893731471a9a6fe1837eec6f51348037e358bc19e, fetched ${FETCHED_T5H}`;

/**
 * `methane-liquid.json`'s n at 670 nm: 1.29, Martonchik and Orton 1994's at 90 K on the file's grid
 * (the tool's test holds it to the committed file).
 */
export const CH4_LIQUID_670_NM_N = 1.29;

/** The index of a Lorentz–Lorenz factor x = (n² − 1) ÷ (n² + 2): n = √((1 + 2x) ÷ (1 − x)). */
function indexOfLorentzLorenz(x: number): number {
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/**
 * `methane-liquid.json`'s n at 670 nm carried by Lorentz–Lorenz from 90 K to 94 K, at the WebBook's
 * triple-point density (standing for 90 K) and {@link CH4_LIQUID_94K_MOL_PER_L}: the check of
 * Hagermann et al.'s scale against the pure-methane row of {@link HAGERMANN_TABLE_2}, 1.2872.
 */
export function methaneLiquidAt94K(): number {
  return indexOfLorentzLorenz(
    (lorentzLorenzFactor(CH4_LIQUID_670_NM_N) * CH4_LIQUID_94K_MOL_PER_L) /
      CH4_LIQUID_TRIPLE_POINT_MOL_PER_L,
  );
}

/**
 * A row of {@link HAGERMANN_TABLE_2}'s n by ideal mixing of molar refraction: (n² − 1) ÷ (n² + 2) =
 * Σ xᵢRᵢ ÷ V, with V = Σ xᵢMᵢ ÷ ρ the row's measured molar volume and Rᵢ each pure liquid's
 * Lorentz–Lorenz molar refraction at 670 nm: C₂H₆'s at the given n and the WebBook's 94 K density,
 * CH₄'s at the table's own pure-methane 1.2872 and {@link CH4_LIQUID_94K_MOL_PER_L}, and N₂'s at
 * `nitrogen-liquid.json`'s fit and Augustin's 77.32 K density ({@link N2_LIQUID_FIT}), a molar
 * refraction carried to 94 K unchanged.
 */
export function titanMixtureIndex(row: TitanMixtureRow, ethaneN: number): number {
  const methaneN = titanRow(0, 1, 0).n;
  const refraction = (n: number, molPerL: number): number =>
    lorentzLorenzFactor(n) * molarVolumeOf(molPerL);
  const nitrogenN = cauchyIndex(N2_LIQUID_FIT, C2H6_LIQUID_MEASURED.wavelengthNm).n;
  const sum =
    row.ethaneMoleFraction * refraction(ethaneN, C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L) +
    row.methaneMoleFraction * refraction(methaneN, CH4_LIQUID_94K_MOL_PER_L) +
    row.nitrogenMoleFraction * refraction(nitrogenN, N2_LIQUID_MOLAR_DENSITY_MOL_PER_L);
  const molarMass =
    row.ethaneMoleFraction * C2H6_MOLAR_MASS_G_PER_MOL +
    row.methaneMoleFraction * CH4_MOLAR_MASS_G_PER_MOL +
    row.nitrogenMoleFraction * N2_MOLAR_MASS_G_PER_MOL;
  return indexOfLorentzLorenz(sum / (molarMass / row.densityGPerCm3));
}

/**
 * A {@link HAGERMANN_TABLE_2} row by its C₂H₆, CH₄ and N₂ mole fractions.
 *
 * @throws Error if the table has no such row.
 */
export function titanRow(
  ethaneMoleFraction: number,
  methaneMoleFraction: number,
  nitrogenMoleFraction: number,
): TitanMixtureRow {
  const row = HAGERMANN_TABLE_2.find(
    (r) =>
      r.ethaneMoleFraction === ethaneMoleFraction &&
      r.methaneMoleFraction === methaneMoleFraction &&
      r.nitrogenMoleFraction === nitrogenMoleFraction,
  );
  if (row === undefined) {
    throw new Error(
      `Hagermann et al.'s Table 2 has no row ${ethaneMoleFraction}/${methaneMoleFraction}/${nitrogenMoleFraction}`,
    );
  }
  return row;
}

/** A Table 2 row's mixing estimate less its measured n, at a pure-ethane n. */
export function titanMixtureResidual(row: TitanMixtureRow, ethaneN: number): number {
  return titanMixtureIndex(row, ethaneN) - row.n;
}

/**
 * The Lorentz–Lorenz function (n² − 1) ÷ ((n² + 2)ρ), cm³ g⁻¹, of an index at a density, g cm⁻³:
 * Burton's quantity, for the record that his gas value does not reach the liquid.
 */
function lorentzLorenzFunction(n: number, densityGPerCm3: number): number {
  return lorentzLorenzFactor(n) / densityGPerCm3;
}

/** The ethane file's checks on its level, as its `source` states them (R08.T5.h). */
function ethaneChecksText(): string {
  const measured = C2H6_LIQUID_MEASURED.n;
  const estimate = ethaneLiquidEstimate(C2H6_LIQUID_MEASURED.wavelengthNm);
  const r = (row: TitanMixtureRow, n: number): string => signed(titanMixtureResidual(row, n), 4);
  const nearlyPure = titanRow(0.96, 0, 0.04);
  const binary = titanRow(0.53, 0.47, 0);
  const ternary = titanRow(0.5, 0.44, 0.06);
  const noEthane = titanRow(0, 0.83, 0.17);
  const liquidDensity = (C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L * C2H6_MOLAR_MASS_G_PER_MOL) / 1000;
  const burtonAtMixture = indexOfLorentzLorenz(
    C2H6_BURTON_LORENTZ_LORENZ_CM3_PER_G * nearlyPure.densityGPerCm3,
  );
  const ethaneVolume = molarVolumeOf(C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L);
  const methaneVolume = molarVolumeOf(CH4_LIQUID_94K_MOL_PER_L);
  const binaryAtMoleFractions =
    (binary.ethaneMoleFraction * C2H6_MOLAR_MASS_G_PER_MOL +
      binary.methaneMoleFraction * CH4_MOLAR_MASS_G_PER_MOL) /
    (binary.ethaneMoleFraction * ethaneVolume + binary.methaneMoleFraction * methaneVolume);
  const binaryAtVolumeFractions =
    (binary.ethaneMoleFraction * C2H6_MOLAR_MASS_G_PER_MOL) / ethaneVolume +
    (binary.methaneMoleFraction * CH4_MOLAR_MASS_G_PER_MOL) / methaneVolume;
  return `The checks, at 670 nm: Table 2's pure-methane row, ${titanRow(0, 1, 0).n.toFixed(4)}, against methane-liquid.json's ${CH4_LIQUID_670_NM_N.toFixed(4)} at 90 K carried by Lorentz–Lorenz to 94 K, ${methaneLiquidAt94K().toFixed(4)} (the WebBook's saturated liquid: ${CH4_LIQUID_TRIPLE_POINT_MOL_PER_L} mol/l at the triple point, 90.694 K, standing for 90 K, and ${CH4_LIQUID_94K_MOL_PER_L} mol/l at 94 K, ${CH4_LIQUID_94K_SOURCE}); its 96/0/4 C2H6/CH4/N2 row, ${nearlyPure.n.toFixed(4)}. Ideal mixing of molar refraction at the rows' mole fractions and measured densities (CH4's at the table's 1.2872, N2's from nitrogen-liquid.json) misses the rows by ${r(nearlyPure, measured)} (96/0/4), ${r(binary, measured)} (53/47/0) and ${r(ternary, measured)} (50/44/6) with ethane at ${measured}, and by ${r(nearlyPure, estimate)}, ${r(binary, estimate)} and ${r(ternary, estimate)} with the estimate's ${estimate.toFixed(4)}; the ethane-free 0/83/17 row misses by ${r(noEthane, measured)}, so the mixtures pin the level only to about ±0.004, and the pure liquid's value is taken. Table 2's header reads "vol. per cent", but the quantities the paper's §2.2 solves for are mole fractions, Badoz et al. prepared their mixtures from each gas's volume and pressure (their abstract), and the binary row's measured ${binary.densityGPerCm3.toFixed(4)} g cm⁻³ lies nearer ideal mixing at mole fractions, ${binaryAtMoleFractions.toFixed(4)}, than at volume fractions, ${binaryAtVolumeFractions.toFixed(4)}. The estimate ran ${(estimate - measured).toFixed(4)} high at 670 nm, as the method runs high on the measured liquids. Burton's ${C2H6_BURTON_LORENTZ_LORENZ_CM3_PER_G} cm³ g⁻¹ at 632.8 nm (University of British Columbia thesis, DOI 10.14288/1.0084897, CC BY; its abstract, through OpenAlex) holds over 1–200 amagat only: at the 96/0/4 row's ${nearlyPure.densityGPerCm3.toFixed(4)} g cm⁻³ it would give ${burtonAtMixture.toFixed(3)}, above every measurement, where that row's own index implies ${lorentzLorenzFunction(nearlyPure.n, nearlyPure.densityGPerCm3).toFixed(4)} cm³ g⁻¹, and the pure liquid's ${measured} at the WebBook's ${liquidDensity.toFixed(4)} g cm⁻³ implies ${lorentzLorenzFunction(measured, liquidDensity).toFixed(4)}`;
}

/** The largest departure of a Cauchy fit from the curve it was fitted to, on the grid. */
export function cauchyResidual(fit: CauchyFit, curve: (wavelengthNm: number) => number): number {
  return Math.max(...MATERIAL_GRID_NM.map((nm) => Math.abs(cauchyIndex(fit, nm).n - curve(nm))));
}

/**
 * Becquerel 1877's n of molten S₈ at the sodium D lines, 1.9290, "Soufre fondu (114 degrés)", taken
 * as the melt began to crystallise (H. Becquerel, "Recherches expérimentales sur la polarisation
 * rotatoire magnétique", Ann. Chim. Phys. (5) 12 (1877) 5–87, p. 37, Tableau n° 2, and pp. 25–26,
 * where the dark-yellow melt read 1.890 a little before), as archive.org gives the volume (public
 * domain mark). R08.T5.g's molten-sulphur stand-in.
 */
export const S8_MOLTEN_INDEX_D = 1.929;

// R08.T5.k: nine liquids from their gases' molar refraction and their own measurements
// (decision-p14-t49h-gas-index and its addendum A.1–A.4, 2026-10-16).

/** Börzsönyi et al. 2008, `rayleigh.ts`'s source of the noble gases' Sellmeier forms. */
const BORZSONYI_2008 =
  "Börzsönyi, Heiner, Kalashnikov, Kovács and Osvay, Appl. Opt. 47 (2008) 4856";

/**
 * n − 1 from a two-term Sellmeier form, n² − 1 = B₁λ² ÷ (λ² − C₁) + B₂λ² ÷ (λ² − C₂), λ in µm and C
 * in µm², written as (n² − 1) ÷ (n + 1): `view/atmosphere/rayleigh.ts`'s `sellmeierNMinusOne`,
 * copied.
 */
function sellmeierNMinusOne(
  wavelengthNm: number,
  b1: number,
  c1Um2: number,
  b2: number,
  c2Um2: number,
): number {
  const l2 = (wavelengthNm / 1_000) ** 2;
  const nSquaredMinusOne = (b1 * l2) / (l2 - c1Um2) + (b2 * l2) / (l2 - c2Um2);
  return nSquaredMinusOne / (Math.sqrt(1 + nSquaredMinusOne) + 1);
}

/**
 * Kr's refractivity, as `rayleigh.ts` holds it, for liquid Kr's checks (R08.T5.k): Börzsönyi et al.
 * 2008's Sellmeier form, B₁ = 26,102.88 × 10⁻⁸, C₁ = 2.01 × 10⁻⁶ µm², B₂ = 56,946.82 × 10⁻⁸ and C₂ =
 * 10.043 × 10⁻³ µm², at 0 °C and 1,000 mbar, Z = 0.997 282.
 */
export const KR_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    sellmeierNMinusOne(wavelengthNm, 26_102.88e-8, 2.01e-6, 56_946.82e-8, 10.043e-3),
  referenceK: 273.15,
  referencePa: 100_000,
  compressibility: 0.997_282,
  source: BORZSONYI_2008,
};

/**
 * Xe's refractivity, as `rayleigh.ts` holds it, for liquid Xe's checks (R08.T5.k): Börzsönyi et al.
 * 2008's Sellmeier form, B₁ = 103,701.61 × 10⁻⁸, C₁ = 12.75 × 10⁻³ µm² (refractiveindex.info's
 * correction of the paper's 12.75 × 10⁻⁶), B₂ = 31,228.61 × 10⁻⁸ and C₂ = 0.561 × 10⁻³ µm², at 0 °C
 * and 1,000 mbar, Z = 0.993 245.
 */
export const XE_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    sellmeierNMinusOne(wavelengthNm, 103_701.61e-8, 12.75e-3, 31_228.61e-8, 0.561e-3),
  referenceK: 273.15,
  referencePa: 100_000,
  compressibility: 0.993_245,
  source: BORZSONYI_2008,
};

/**
 * CO₂'s refractivity, as `rayleigh.ts` holds it, for liquid CO₂'s estimate (R08.T5.k):
 * Bideau-Mehu, Guern, Abjean and Johannin-Gilles, Opt. Commun. 9 (1973) 432, as Sneep and Ubachs
 * 2005 (eq. 13) correct it, n − 1 = 6.991 00 × 10⁻² ÷ (166.175 − σ²) + 1.447 20 × 10⁻³ ÷ (79.609 −
 * σ²) + 6.429 41 × 10⁻⁵ ÷ (56.306 4 − σ²) + 5.213 06 × 10⁻⁵ ÷ (46.019 6 − σ²) + 1.468 47 × 10⁻⁶ ÷
 * (0.058 473 8 − σ²), at 0 °C and 101,325 Pa, Z = 0.993 265.
 */
export const CO2_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) => {
    const s2 = wavenumberSquaredPerUm2(wavelengthNm);
    return (
      6.991_00e-2 / (166.175 - s2) +
      1.447_20e-3 / (79.609 - s2) +
      6.429_41e-5 / (56.306_4 - s2) +
      5.213_06e-5 / (46.019_6 - s2) +
      1.468_47e-6 / (0.058_473_8 - s2)
    );
  },
  referenceK: 273.15,
  referencePa: 101_325,
  compressibility: 0.993_265,
  source:
    "Bideau-Mehu, Guern, Abjean and Johannin-Gilles, Opt. Commun. 9 (1973) 432, as corrected by Sneep and Ubachs 2005, eq. 13",
};

/**
 * N₂O's refractivity, as `rayleigh.ts` holds it, for liquid N₂O's estimate (R08.T5.k): He, Fang,
 * Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 9, 10⁸(n − 1) = 22,095 +
 * 1.662 91 × 10¹⁴ ÷ (6.752 26 × 10⁹ − ν²), at 15 °C and 101,325 Pa, Z = 1.
 */
export const N2O_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) =>
    (22_095 + 1.662_91e14 / (6.752_26e9 - wavenumberSquaredPerCm2(wavelengthNm))) * 1e-8,
  referenceK: 288.15,
  referencePa: 101_325,
  compressibility: 1,
  source: "He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 9",
};

/**
 * SO₂'s refractivity, as `rayleigh.ts`'s `ESTIMATED_RAYLEIGH` row holds its dispersion, for liquid
 * SO₂'s estimate (R08.T5.k): C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171, n − 1 =
 * 0.063 733 ÷ (99.349 − λ⁻²) at 0 °C and 760 mm, Z = 1.
 */
export const SO2_DISPERSION: GasDispersionCopy = {
  nMinusOne: (wavelengthNm) => 0.063_733 / (99.349 - wavenumberSquaredPerUm2(wavelengthNm)),
  referenceK: 273.15,
  referencePa: 101_325,
  compressibility: 1,
  source:
    "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171, its scale from Cuthbertson and Metcalfe, Proc. R. Soc. Lond. A 80 (1908) 406",
};

/**
 * A liquid's real index from its gas's molar refraction scaled by one factor (R08.T5.k;
 * decision-p14-t49h-gas-index §1.2): x = c R(λ) ρ_m and n = √((1 + 2x) ÷ (1 − x)), R the gas row's
 * Lorentz–Lorenz molar refraction at its own reference state and Z. It is
 * {@link molecularIceIndex} at the molar volume V_m ÷ c, V_m = 1,000 ÷ ρ_m.
 *
 * @param molarDensityMolPerL - ρ_m, the liquid's molar density, mol/l.
 * @param factor - c, the file's one factor on the gas's molar refraction: 1 where nothing anchors
 *   it.
 * @throws RangeError as {@link molecularIceIndex} does.
 */
export function scaledLiquidIndex(
  gas: GasDispersionCopy,
  wavelengthNm: number,
  molarDensityMolPerL: number,
  factor: number,
): number {
  return molecularIceIndex(gas, wavelengthNm, molarVolumeOf(molarDensityMolPerL) / factor);
}

/**
 * The factor c on a gas's molar refraction that meets a liquid's measured indices by least squares
 * in n ({@link scaledLiquidIndex}; R08.T5.k), by Gauss–Newton iteration from c = 1, unrounded.
 *
 * @throws Error for no points, or no convergence in 50 steps.
 */
export function leastSquaresFactor(
  gas: GasDispersionCopy,
  molarDensityMolPerL: number,
  points: ReadonlyArray<IndexPoint>,
): number {
  if (points.length === 0) {
    throw new Error("leastSquaresFactor: no points");
  }
  let factor = 1;
  for (let step = 0; step < 50; step += 1) {
    let gradient = 0;
    let curvature = 0;
    for (const point of points) {
      const nm = point.wavelengthUm * 1_000;
      const n = scaledLiquidIndex(gas, nm, molarDensityMolPerL, factor);
      const x = (factor * gasRowMolarRefractionCm3PerMol(gas, nm) * molarDensityMolPerL) / 1_000;
      // dn/dc = dn/dx × x ÷ c, with dn/dx = 3 ÷ (2n(1 − x)²).
      const slope = (3 * x) / (2 * n * (1 - x) ** 2 * factor);
      gradient += (n - point.n) * slope;
      curvature += slope * slope;
    }
    const change = gradient / curvature;
    factor -= change;
    if (Math.abs(change) < 1e-14) {
      return factor;
    }
  }
  throw new Error("leastSquaresFactor: no convergence");
}

/** A liquid check's measured molar refraction over its gas row's: (n² − 1) ÷ (n² + 2) × V_m ÷ R. */
export function molarRefractionRatio(check: LiquidCheck): number {
  return (
    (lorentzLorenzFactor(check.measured) * check.molarVolumeCm3PerMol) /
    gasRowMolarRefractionCm3PerMol(check.gas, check.wavelengthNm)
  );
}

/** A saturated liquid's molar density as the NIST Chemistry WebBook's fluid properties give it. */
export interface WebBookDensity {
  /** ρ_m, mol/l, to the page's seven figures. */
  readonly molPerL: number;
  /** The temperature, K. */
  readonly temperatureK: number;
  /** The equation of state the page cites, with the density uncertainty it states. */
  readonly equationOfState: string;
  /** The query's URL and the table's SHA-256, with the date fetched. */
  readonly source: string;
}

/** A {@link WebBookDensity} read from `nistSaturation`'s query, fetched {@link FETCHED_T5K}. */
function webBookDensity(
  molPerL: number,
  temperatureK: number,
  equationOfState: string,
  query: readonly [id: string, low: string, high: string, step: string],
  sha256: string,
): WebBookDensity {
  const [id, low, high, step] = query;
  return {
    molPerL,
    temperatureK,
    equationOfState,
    source: `${nistSaturation(id, low, high, step)}, SHA-256 ${sha256}, fetched ${FETCHED_T5K}`,
  };
}

/** Lemmon and Span 2006, the WebBook's equation of state for Kr, Xe, CO, N₂O and H₂S. */
const LEMMON_SPAN_2006 =
  "E. W. Lemmon and R. Span, J. Chem. Eng. Data 51 (2006) 785, DOI 10.1021/je050186n";

/** Saturated liquid O₂ at 90.19 K, its normal boiling point (Schmidt and Wagner 1985). */
export const O2_LIQUID_DENSITY = webBookDensity(
  35.662_63,
  90.19,
  "R. Schmidt and W. Wagner, Fluid Phase Equilib. 19 (1985) 175, DOI 10.1016/0378-3812(85)87016-3, density uncertain by 0.1%",
  ["C7782447", "90.19", "90.2", "0.01"],
  "7e0c2e5ffaa0cadd4d2d13af6892cffbcdc5cade4f39b4b832119c4325ad21a6",
);

/** Saturated liquid Ar at 83.81 K, Sinnock and Smith's melting point (Tegeler, Span and Wagner 1999). */
export const AR_LIQUID_DENSITY = webBookDensity(
  35.464_64,
  83.81,
  "Ch. Tegeler, R. Span and W. Wagner, J. Phys. Chem. Ref. Data 28 (1999) 779, DOI 10.1063/1.556037, density uncertain by under 0.02% to 12 MPa and 340 K",
  ["C7440371", "83.81", "83.82", "0.01"],
  "f142e38077b182eac6fe46b6eeae96267af125707c7ec7a3db23fb972202c1b1",
);

/** Saturated liquid Kr at Sinnock and Smith's 115.95 K, for the method's record (Lemmon and Span 2006). */
export const KR_LIQUID_DENSITY = webBookDensity(
  29.181_15,
  115.95,
  `${LEMMON_SPAN_2006}, density uncertain by 0.2% below 100 MPa`,
  ["C7439909", "115.95", "115.96", "0.01"],
  "44dcd0af865ee46d7edd16db112b6a04f563cabaa1640c9943425353ad0aeb02",
);

/**
 * Saturated liquid Xe at 161.405 K, the WebBook's lowest saturated state, standing for Sinnock and
 * Smith's 161.35 K in the method's record (Lemmon and Span 2006).
 */
export const XE_LIQUID_DENSITY = webBookDensity(
  22.592_09,
  161.405,
  `${LEMMON_SPAN_2006}, density uncertain by 0.2% up to 100 MPa`,
  ["C7440633", "161.405", "161.415", "0.01"],
  "36a7482bc8c8e3143d7abb04ef673813285cc0db24fe49bd2de80218e8f2ef7a",
);

/** CO's density uncertainty as the WebBook's page states Lemmon and Span 2006's. */
const CO_EQUATION_OF_STATE = `${LEMMON_SPAN_2006}, density uncertain by 0.3%`;

/** Saturated liquid CO at 68.16 K, the registry row's triple point (Lemmon and Span 2006). */
export const CO_LIQUID_DENSITY = webBookDensity(
  30.329_71,
  68.16,
  CO_EQUATION_OF_STATE,
  ["C630080", "68.16", "68.17", "0.01"],
  "8110ea2f642cf1dde596cf8714e398fcee517c5cf50c4e60c6ff3d14619474e3",
);

/** Saturated liquid CO at Gagnon et al.'s 78.3 K, for the method's record. */
export const CO_LIQUID_78K_DENSITY = webBookDensity(
  28.832_82,
  78.3,
  CO_EQUATION_OF_STATE,
  ["C630080", "78.3", "78.31", "0.01"],
  "fff895d46e5357673ec0668370c616f0534e04d9d1522f005fbf2e04371116a4",
);

/** CO₂'s equation of state as the WebBook's page cites it. */
const CO2_EQUATION_OF_STATE =
  "R. Span and W. Wagner, J. Phys. Chem. Ref. Data 25 (1996) 1509, DOI 10.1063/1.555991, density uncertain by 0.03–0.05% to 30 MPa and 523 K";

/** Saturated liquid CO₂ at 216.6 K, the registry row's triple point, 216.592 K, rounded up. */
export const CO2_LIQUID_DENSITY = webBookDensity(
  26.776_63,
  216.6,
  CO2_EQUATION_OF_STATE,
  ["C124389", "216.6", "216.61", "0.01"],
  "e276ec7e0bfa52c5fd438baea2655f8699aab160d7f76c4ed37633be70486615",
);

/** The WebBook's query for saturated liquid CO₂ over Bleekrode's states, 285.65–297.15 K every 0.5 K. */
const CO2_ROOM_QUERY = ["C124389", "285.65", "297.15", "0.5"] as const;

/** That query's SHA-256. */
const CO2_ROOM_SHA256 = "8b98026a63d71d07318db1307cd1c4cbfd3c6fb265e53eea5a3f1f770c3da7cc";

/** Saturated liquid N₂O at 184.67 K, its normal boiling point (Lemmon and Span 2006). */
export const N2O_LIQUID_DENSITY = webBookDensity(
  27.957_49,
  184.67,
  `${LEMMON_SPAN_2006}, density uncertain by 0.25% below 220 K`,
  ["C10024972", "184.67", "184.68", "0.01"],
  "4fe65961efe63c2a961fe28c160f3d7a30c3557518e61dd325af71771f9040a2",
);

/** SO₂'s equation of state as the WebBook's page cites it. */
const SO2_EQUATION_OF_STATE =
  "K. Gao, J. Wu, P. Zhang and E. W. Lemmon, J. Chem. Eng. Data 61 (2016) 2859, DOI 10.1021/acs.jced.6b00195, density uncertain by 0.1% in the liquid";

/** Saturated liquid SO₂ at 197.7 K, the registry row's triple point (Gao et al. 2016). */
export const SO2_LIQUID_DENSITY = webBookDensity(
  25.411_32,
  197.7,
  SO2_EQUATION_OF_STATE,
  ["C7446095", "197.7", "197.71", "0.01"],
  "4a264e71e640a1e815e1344a6e53f4155b3ff87a87ba8a6593ee319720ed9f20",
);

/** The WebBook's query for saturated liquid SO₂ over 288.15–297.15 K every 0.5 K. */
const SO2_ROOM_QUERY = ["C7446095", "288.15", "297.15", "0.5"] as const;

/** That query's SHA-256. */
const SO2_ROOM_SHA256 = "ec9c096d0eef9a2ea36c4e61c37e660d09f7c0096a84da7adfab7dd830517590";

/** H₂S's equation of state as the WebBook's page cites it. */
const H2S_EQUATION_OF_STATE = `${LEMMON_SPAN_2006}, density uncertain by 0.1% in the liquid`;

/** Saturated liquid H₂S at 187.7 K, the registry row's triple point (Lemmon and Span 2006). */
export const H2S_LIQUID_DENSITY = webBookDensity(
  29.116_33,
  187.7,
  H2S_EQUATION_OF_STATE,
  ["C7783064", "187.7", "187.71", "0.01"],
  "43945332d433fac4e2fb492644a740fa14e726ce79b4a4b0ee5b96f7c4cc1d16",
);

/** The WebBook's query for saturated liquid H₂S at 285.65 and 291.65 K. */
const H2S_ROOM_QUERY = ["C7783064", "285.65", "291.65", "6"] as const;

/** That query's SHA-256. */
const H2S_ROOM_SHA256 = "82c2f290c1512b903d3e410d27f2288934f86d2a6805fcc5037c064ebc340d12";

/** A measured index at a named line or state, a T5.k anchor or check. */
export interface NamedIndexPoint extends IndexPoint {
  /** The line, or the source and its state, in words. */
  readonly line: string;
}

/**
 * Sinnock and Smith 1969's liquid Ar at 83.81 K, its melting point (Phys. Rev. 181, 1297, as
 * refractiveindex.info's CC0 `main/Ar/nk/Sinnock-liquid-83.81K.yml` gives the table): λ, µm, and n.
 * The input checked by SHA-256 must hold these rows exactly.
 */
export const AR_LIQUID_ROWS: ReadonlyArray<IndexPoint> = [
  { wavelengthUm: 0.3612, n: 1.2395 },
  { wavelengthUm: 0.365, n: 1.2392 },
  { wavelengthUm: 0.4063, n: 1.2372 },
  { wavelengthUm: 0.4358, n: 1.2361 },
  { wavelengthUm: 0.4753, n: 1.2349 },
  { wavelengthUm: 0.5086, n: 1.2341 },
  { wavelengthUm: 0.5461, n: 1.2334 },
  { wavelengthUm: 0.578, n: 1.2328 },
  { wavelengthUm: 0.6439, n: 1.2321 },
];

/** Sinnock and Smith 1969's liquid Kr at 115.95 K (`main/Kr/nk/Sinnock-liquid-115.95K.yml`). */
export const KR_LIQUID_ROWS: ReadonlyArray<IndexPoint> = [
  { wavelengthUm: 0.3631, n: 1.312 },
  { wavelengthUm: 0.4063, n: 1.309 },
  { wavelengthUm: 0.4358, n: 1.3074 },
  { wavelengthUm: 0.4753, n: 1.3056 },
  { wavelengthUm: 0.5086, n: 1.3042 },
  { wavelengthUm: 0.5461, n: 1.3032 },
  { wavelengthUm: 0.578, n: 1.3024 },
  { wavelengthUm: 0.6439, n: 1.3011 },
];

/** Sinnock and Smith 1969's liquid Xe at 161.35 K (`main/Xe/nk/Sinnock-liquid-161.35K.yml`). */
export const XE_LIQUID_ROWS: ReadonlyArray<IndexPoint> = [
  { wavelengthUm: 0.3612, n: 1.4111 },
  { wavelengthUm: 0.365, n: 1.4103 },
  { wavelengthUm: 0.4063, n: 1.4041 },
  { wavelengthUm: 0.4358, n: 1.4001 },
  { wavelengthUm: 0.47, n: 1.3967 },
  { wavelengthUm: 0.4806, n: 1.3957 },
  { wavelengthUm: 0.5086, n: 1.3937 },
  { wavelengthUm: 0.5461, n: 1.3918 },
  { wavelengthUm: 0.578, n: 1.39 },
  { wavelengthUm: 0.6439, n: 1.3876 },
];

/**
 * A liquid table's rows from its input, as the tool holds them.
 *
 * @throws Error if the input's rows differ from the rows held, in number, wavelength or n.
 */
export function checkedRows(
  text: string,
  held: ReadonlyArray<IndexPoint>,
): ReadonlyArray<IndexPoint> {
  const rows = parseRefractiveIndexInfoN(text);
  const same =
    rows.length === held.length &&
    rows.every((row, i) => {
      const kept = held[i];
      return (
        kept !== undefined &&
        row.wavelengthUm === kept.wavelengthUm &&
        row.n === kept.n &&
        row.k === 0
      );
    });
  if (!same) {
    throw new Error("a liquid table's rows differ from the rows the tool holds");
  }
  return held;
}

/**
 * Liquid Ar's factor on its gas's molar refraction (decision-p14-t49h-gas-index A.2): the least
 * squares fit in n to {@link AR_LIQUID_ROWS} at {@link AR_LIQUID_DENSITY}, to six figures.
 */
export const AR_LIQUID_FACTOR = rounded(
  leastSquaresFactor(AR_DISPERSION, AR_LIQUID_DENSITY.molPerL, AR_LIQUID_ROWS),
);

/** Liquid Ar's worst row against its scaled curve: the largest miss in n and its wavelength, nm. */
export function arWorstRow(): { readonly miss: number; readonly wavelengthNm: number } {
  let worst = { miss: 0, wavelengthNm: Number.NaN };
  for (const row of AR_LIQUID_ROWS) {
    const nm = row.wavelengthUm * 1_000;
    const miss = Math.abs(
      scaledLiquidIndex(AR_DISPERSION, nm, AR_LIQUID_DENSITY.molPerL, AR_LIQUID_FACTOR) - row.n,
    );
    if (miss > worst.miss) {
      worst = { miss, wavelengthNm: nm };
    }
  }
  return worst;
}

/** The shortest row a liquid table's red tail is fitted from, µm (decision-p14-t49h-gas-index A.3). */
export const RED_TAIL_FROM_UM = 0.4358;

/**
 * A liquid table's red tail (decision-p14-t49h-gas-index A.3): the least-squares two-term Cauchy line
 * over its rows from {@link RED_TAIL_FROM_UM}, a and b to six figures.
 */
export function redTailFit(rows: ReadonlyArray<IndexPoint>): Pick<CauchyFit, "a" | "bUm2"> {
  return fitCauchy(rows.filter((row) => row.wavelengthUm >= RED_TAIL_FROM_UM));
}

/** A red tail's n at a wavelength, nm: a + b ÷ λ², λ in µm. */
export function redTailIndex(tail: Pick<CauchyFit, "a" | "bUm2">, wavelengthNm: number): number {
  const um = wavelengthNm / 1_000;
  return tail.a + tail.bUm2 / (um * um);
}

/** The largest departure of a red tail from the rows it was fitted to. */
export function redTailWorst(
  rows: ReadonlyArray<IndexPoint>,
  tail: Pick<CauchyFit, "a" | "bUm2">,
): number {
  return Math.max(
    ...rows
      .filter((row) => row.wavelengthUm >= RED_TAIL_FROM_UM)
      .map((row) => Math.abs(redTailIndex(tail, row.wavelengthUm * 1_000) - row.n)),
  );
}

/** A red tail's step from a table's last row, at that row's wavelength: the tail less the row. */
export function redTailStep(
  rows: ReadonlyArray<IndexPoint>,
  tail: Pick<CauchyFit, "a" | "bUm2">,
): number {
  const last = rows.at(-1);
  if (last === undefined) {
    throw new Error("redTailStep: no rows");
  }
  return redTailIndex(tail, last.wavelengthUm * 1_000) - last.n;
}

/**
 * A measured liquid table on {@link MATERIAL_GRID_NM} (R08.T5.k): n linear in wavelength between its
 * rows up to its last, and its red tail ({@link redTailFit}) beyond, to six figures; k = 0, which
 * the table does not give.
 */
export function tableWithRedTail(rows: ReadonlyArray<IndexPoint>): ReducedIndex {
  const lastUm = rows.at(-1)?.wavelengthUm ?? Number.NaN;
  const inner = MATERIAL_GRID_NM.filter((nm) => nm / 1_000 <= lastUm);
  const outer = MATERIAL_GRID_NM.filter((nm) => nm / 1_000 > lastUm);
  const tail = redTailFit(rows);
  const reduced = resample(
    rows.map((row) => ({ wavelengthUm: row.wavelengthUm, n: row.n, k: 0 })),
    { holdNm: 0, grid: inner },
  );
  return {
    n: [...reduced.n, ...outer.map((nm) => rounded(redTailIndex(tail, nm)))],
    k: MATERIAL_GRID_NM.map(() => 0),
  };
}

/** Liquid Kr's red tail over its six rows from 435.8 nm. */
export const KR_LIQUID_TAIL = redTailFit(KR_LIQUID_ROWS);

/** Liquid Xe's red tail over its seven rows from 435.8 nm. */
export const XE_LIQUID_TAIL = redTailFit(XE_LIQUID_ROWS);

/**
 * The largest departure of a gas's shape, scaled by least squares to a liquid table, from the
 * table's rows: why Kr's and Xe's files take their rows (decision-p14-t49h-gas-index §2.2).
 */
export function scaledShapeMiss(
  gas: GasDispersionCopy,
  molarDensityMolPerL: number,
  rows: ReadonlyArray<IndexPoint>,
): number {
  const factor = leastSquaresFactor(gas, molarDensityMolPerL, rows);
  return Math.max(
    ...rows.map((row) =>
      Math.abs(
        scaledLiquidIndex(gas, row.wavelengthUm * 1_000, molarDensityMolPerL, factor) - row.n,
      ),
    ),
  );
}

/**
 * Johns and Wilhelm 1937's liquid O₂ at its normal boiling point, 1.2242 at 546.1 nm (Can. J. Res.
 * 15a, 101, DOI 10.1139/cjr37a-013, its abstract through Crossref): oxygen-liquid.json's anchor.
 */
export const O2_LIQUID_ANCHOR: LiquidCheck = {
  what: "liquid O2 at 90.19 K against Johns and Wilhelm 1937 (Can. J. Res. 15a, 101)",
  gas: O2_DISPERSION,
  molarVolumeCm3PerMol: molarVolumeOf(O2_LIQUID_DENSITY.molPerL),
  wavelengthNm: 546.1,
  measured: 1.2242,
};

/** Liquid O₂'s factor: the one that meets {@link O2_LIQUID_ANCHOR}, to six figures. */
export const O2_LIQUID_FACTOR = rounded(molarRefractionRatio(O2_LIQUID_ANCHOR));

/**
 * Liveing and Dewar 1895's liquid O₂ at its boiling point (Phil. Mag. (5) 40, 268, p. 269; public
 * domain): the cadmium spark's blue and red lines, thallium's green, lithium's red and sodium's
 * yellow, at the wavelengths the paper prints, Å as µm. A recorded check on oxygen-liquid.json.
 */
export const LIVEING_DEWAR_1895_O2: ReadonlyArray<NamedIndexPoint> = [
  { line: "cadmium's blue line, 4416 Å", wavelengthUm: 0.4416, n: 1.2249 },
  { line: "cadmium's red line, 6438 Å", wavelengthUm: 0.6438, n: 1.2211 },
  { line: "thallium's green line, printed λ535, 5350 Å", wavelengthUm: 0.535, n: 1.2219 },
  { line: "lithium's red line, 6705 Å", wavelengthUm: 0.6705, n: 1.221 },
  { line: "sodium's yellow lines, 5892 Å", wavelengthUm: 0.5892, n: 1.2214 },
];

/** Liveing and Dewar 1892's liquid O₂ at the D line by a prism of 59° 15′, 1.2236 (p. 208). */
export const LIVEING_DEWAR_1892_O2_D = { wavelengthNm: 589.3, n: 1.2236 } as const;

/** Johns and Wilhelm 1937's liquid N₂ at its normal boiling point, 1.1990 at 546.1 nm (their abstract). */
export const JOHNS_WILHELM_N2_546 = 1.199;

/** The O₂–O₂ set's reference number for Thalman and Volkamer 2013 in HITRAN2024's file. */
export const THALMAN_VOLKAMER_REFERENCE = "32";

/** Thalman and Volkamer 2013's coldest sets' temperature in that file, K. */
export const OXYGEN_PAIR_COLDEST_K = 203;

/** The coldest sets' longest wavelength, nm: their 15,290 cm⁻¹. */
export const OXYGEN_PAIR_LONGEST_NM = 1e7 / 15_290;

/**
 * The grid points the coldest sets reach where their σ is at or below 0, nm: the measurement's
 * scatter about its baseline between the bands, where liquid O₂'s k is 0.
 */
export const OXYGEN_PAIR_ZERO_NM: ReadonlyArray<number> = [
  390, 395, 400, 405, 410, 415, 420, 425, 430, 455, 595, 600, 650,
];

/**
 * Thalman and Volkamer 2013's O₂–O₂ σ at 203 K at two grid points, cm⁵ a molecule pair, to six
 * figures: near the 577 and 630 nm bands' peaks, as HITRAN2024's file gives them (linear in
 * wavenumber between its points 1 cm⁻¹ apart).
 */
export const OXYGEN_PAIR_CROSS_SECTIONS_CM5: ReadonlyArray<
  readonly [wavelengthNm: number, sigmaCm5: number]
> = [
  [575, 1.191_78e-45],
  [630, 8.642_97e-46],
];

/**
 * How far below 0 the coldest sets' σ reaches at the grid's points, cm⁵: their scatter about the
 * baseline between the bands (the 650 nm point's −6.53 × 10⁻⁴⁸).
 */
export const OXYGEN_PAIR_SCATTER_CM5 = 7e-48;

/**
 * Thalman and Volkamer's σ at 577 nm, cm⁵, at 293 and 203 K, to three figures: how the 577 nm band
 * grows on cooling in the same sets.
 */
export const OXYGEN_PAIR_COOLING: ReadonlyArray<readonly [temperatureK: number, sigmaCm5: number]> =
  [
    [293, 1.11e-45],
    [203, 1.36e-45],
  ];

/** Liquid O₂'s number density at {@link O2_LIQUID_DENSITY}, cm⁻³: N = ρ_m N_A. */
export const O2_LIQUID_NUMBER_DENSITY_PER_CM3 =
  (O2_LIQUID_DENSITY.molPerL / 1_000) * AVOGADRO_PER_MOL;

/**
 * Liquid O₂'s k from a pair cross-section at a wavelength, nm (decision-p14-t49h-gas-index §2.6,
 * path 2): a = σN², N the liquid's number density, so a in cm⁻¹, and k = aλ ÷ 4π.
 *
 * @param sigmaCm5 - σ, cm⁵ a molecule pair.
 */
export function oxygenPairK(sigmaCm5: number, wavelengthNm: number): number {
  return absorptionIndex(oxygenPairAbsorptionPerM(sigmaCm5), wavelengthNm);
}

/** Liquid O₂'s absorption coefficient from a pair cross-section, m⁻¹: 100 σN². */
export function oxygenPairAbsorptionPerM(sigmaCm5: number): number {
  return 100 * sigmaCm5 * O2_LIQUID_NUMBER_DENSITY_PER_CM3 ** 2;
}

/** Liquid O₂'s absorption coefficient at a grid point of {@link OXYGEN_PAIR_CROSS_SECTIONS_CM5}, m⁻¹. */
function oxygenPairPerM(wavelengthNm: number): number {
  const sigma = OXYGEN_PAIR_CROSS_SECTIONS_CM5.find(([nm]) => nm === wavelengthNm)?.[1];
  if (sigma === undefined) {
    throw new Error(`no O2-O2 cross-section held at ${wavelengthNm} nm`);
  }
  return oxygenPairAbsorptionPerM(sigma);
}

/**
 * Thalman and Volkamer 2013's O₂–O₂ σ at a vacuum wavelength, nm, in their sets at a temperature
 * (their coldest unless another is given), cm⁵ a molecule pair, linear in wavenumber between the
 * set's points; undefined where no such set reaches.
 *
 * @throws Error if two of those sets reach the wavelength.
 */
export function oxygenPairCrossSectionCm5(
  sets: ReadonlyArray<CiaSet>,
  wavelengthNm: number,
  temperatureK: number = OXYGEN_PAIR_COLDEST_K,
): number | undefined {
  const perCm = 1e7 / wavelengthNm;
  const reaching = sets.filter(
    (set) =>
      set.reference === THALMAN_VOLKAMER_REFERENCE &&
      set.temperatureK === temperatureK &&
      perCm >= set.minPerCm &&
      perCm <= set.maxPerCm,
  );
  if (reaching.length > 1) {
    throw new Error(`two O2-O2 sets at ${temperatureK} K reach ${wavelengthNm} nm`);
  }
  const points = reaching[0]?.points ?? [];
  const upper = points.findIndex(([nu]) => nu >= perCm);
  const hi = points[upper];
  const lo = points[upper - 1] ?? hi;
  if (hi === undefined || lo === undefined) {
    return undefined;
  }
  return hi[0] === lo[0] ? hi[1] : lo[1] + ((perCm - lo[0]) / (hi[0] - lo[0])) * (hi[1] - lo[1]);
}

/**
 * Liquid O₂'s k on {@link MATERIAL_GRID_NM} from HITRAN2024's O₂–O₂ file (R08.T5.k;
 * decision-p14-t49h-gas-index §2.6, path 2), to six figures: {@link oxygenPairK} of Thalman and
 * Volkamer's 203 K sets, and 0 where they do not reach or their σ is at or below 0.
 *
 * @throws Error if the file's coldest Thalman and Volkamer sets are not at
 *   {@link OXYGEN_PAIR_COLDEST_K}, its σ at a point of {@link OXYGEN_PAIR_CROSS_SECTIONS_CM5} is
 *   another, or its zeros are not {@link OXYGEN_PAIR_ZERO_NM}'s: the file's text states them.
 */
export function liquidOxygenK(text: string): ReadonlyArray<number> {
  const sets = parseCia(text);
  const coldest = Math.min(
    ...sets
      .filter((set) => set.reference === THALMAN_VOLKAMER_REFERENCE)
      .map((s) => s.temperatureK),
  );
  if (coldest !== OXYGEN_PAIR_COLDEST_K) {
    throw new Error(`O2-O2: the coldest Thalman and Volkamer set is at ${coldest} K`);
  }
  for (const [nm, sigma] of OXYGEN_PAIR_CROSS_SECTIONS_CM5) {
    if (rounded(oxygenPairCrossSectionCm5(sets, nm) ?? Number.NaN) !== sigma) {
      throw new Error(`O2-O2: σ at ${nm} nm is not the ${sigma} the file states`);
    }
  }
  for (const [kelvin, sigma] of OXYGEN_PAIR_COOLING) {
    const read = oxygenPairCrossSectionCm5(sets, 577, kelvin) ?? Number.NaN;
    if (Number(read.toPrecision(3)) !== sigma) {
      throw new Error(`O2-O2: σ at 577 nm and ${kelvin} K is not the ${sigma} the file states`);
    }
  }
  const sigmas = MATERIAL_GRID_NM.map((nm) => oxygenPairCrossSectionCm5(sets, nm));
  const zeros = MATERIAL_GRID_NM.filter((_, i) => {
    const sigma = sigmas[i];
    return sigma !== undefined && sigma <= 0;
  });
  const reached = MATERIAL_GRID_NM.filter((_, i) => sigmas[i] !== undefined);
  if (
    zeros.join() !== OXYGEN_PAIR_ZERO_NM.join() ||
    sigmas.some((sigma) => sigma !== undefined && sigma < -OXYGEN_PAIR_SCATTER_CM5) ||
    reached.at(-1) !== Math.floor(OXYGEN_PAIR_LONGEST_NM / 5) * 5 ||
    reached[0] !== MATERIAL_GRID_NM[0]
  ) {
    throw new Error("O2-O2: the sets' zeros or reach differ from the file's text");
  }
  return MATERIAL_GRID_NM.map((nm, i) => {
    const sigma = sigmas[i];
    return sigma === undefined || sigma <= 0 ? 0 : rounded(oxygenPairK(sigma, nm));
  });
}

/** One of Liveing and Dewar 1892's prism readings of liquid N₂O (Phil. Mag. (5) 34, 205, p. 209). */
export interface PrismReading extends NamedIndexPoint {
  /** The minimum deviation, arcminutes. */
  readonly deviationArcmin: number;
}

/**
 * Liveing and Dewar 1892's liquid N₂O at its boiling point (p. 209; public domain): the C, D, F and G
 * lines and indium's 4509.6 Å, each with its minimum deviation and the index printed. The C, D, F and
 * G lines are read at 656.3, 589.3, 486.1 and 430.8 nm. N₂O's anchor.
 */
export const LIVEING_DEWAR_1892_N2O: ReadonlyArray<PrismReading> = [
  { line: "C", wavelengthUm: 0.6563, deviationArcmin: 22 * 60 + 53, n: 1.329 },
  { line: "D", wavelengthUm: 0.5893, deviationArcmin: 23 * 60, n: 1.3305 },
  { line: "F", wavelengthUm: 0.4861, deviationArcmin: 23 * 60 + 18, n: 1.3345 },
  { line: "G", wavelengthUm: 0.4308, deviationArcmin: 23 * 60 + 33, n: 1.3378 },
  { line: "In", wavelengthUm: 0.450_96, deviationArcmin: 23 * 60 + 28, n: 1.3368 },
];

/** Their lithium line, 6705.5 Å: 22° 52′ and a printed 1.3257, which its deviation contradicts. */
export const LIVEING_DEWAR_1892_N2O_LITHIUM: PrismReading = {
  line: "Li",
  wavelengthUm: 0.670_55,
  deviationArcmin: 22 * 60 + 52,
  n: 1.3257,
};

/**
 * The prism's angle, arcminutes: 59° 15′, the angle of their liquid-oxygen prism (p. 208), which the
 * five other N₂O readings' printed pairs imply to 0.015° ({@link prismApexArcmin}).
 */
export const LIVEING_DEWAR_PRISM_ARCMIN = 59 * 60 + 15;

/** A prism's index from its angle and the minimum deviation, both arcminutes: sin((A + D) ÷ 2) ÷ sin(A ÷ 2). */
export function prismIndex(apexArcmin: number, deviationArcmin: number): number {
  const radiansPerArcmin = Math.PI / (180 * 60);
  return (
    Math.sin(((apexArcmin + deviationArcmin) * radiansPerArcmin) / 2) /
    Math.sin((apexArcmin * radiansPerArcmin) / 2)
  );
}

/** The prism angle a reading's printed index and deviation imply, arcminutes, by bisection. */
export function prismApexArcmin(reading: PrismReading): number {
  let low = 20 * 60;
  let high = 89 * 60;
  for (let step = 0; step < 100; step += 1) {
    const mid = (low + high) / 2;
    if (prismIndex(mid, reading.deviationArcmin) > reading.n) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return (low + high) / 2;
}

/** Liquid N₂O's factor: the least-squares fit to {@link LIVEING_DEWAR_1892_N2O}, to six figures. */
export const N2O_LIQUID_FACTOR = rounded(
  leastSquaresFactor(N2O_DISPERSION, N2O_LIQUID_DENSITY.molPerL, LIVEING_DEWAR_1892_N2O),
);

/** A saturated liquid's measured index at the sodium D lines and a temperature, with its WebBook density. */
export interface LiquidMeasurement {
  /** Who measured it, as the files name them. */
  readonly who: string;
  readonly temperatureC: number;
  readonly n: number;
  /** The WebBook's saturated-liquid density at the temperature, mol/l. */
  readonly molPerL: number;
}

/** A measurement at the sodium D lines as a {@link LiquidCheck} of a gas row. */
function sodiumCheck(
  liquid: string,
  gas: GasDispersionCopy,
  measurement: LiquidMeasurement,
): LiquidCheck {
  return {
    what: `${liquid} at ${measurement.temperatureC} °C against ${measurement.who}`,
    gas,
    molarVolumeCm3PerMol: molarVolumeOf(measurement.molPerL),
    wavelengthNm: SODIUM_D_NM,
    measured: measurement.n,
  };
}

/**
 * Liquid SO₂'s three measured indices at the sodium D lines, each at the WebBook's density at its
 * temperature: Ketteler 1865's 1.3384 at 24 °C and Bichat 1880's 1.344 at 18.5 °C, as Bleekrode
 * 1884 quotes them, and Bleekrode's own 1.350 at 15 °C (Proc. R. Soc. Lond. 37, 339, p. 350;
 * public domain). Their molar refractions' mean sets SO₂'s factor.
 */
export const SO2_LIQUID_MEASUREMENTS: ReadonlyArray<LiquidMeasurement> = [
  {
    who: "Ketteler 1865, as Bleekrode 1884 quotes it",
    temperatureC: 24,
    n: 1.3384,
    molPerL: 21.379_59,
  },
  {
    who: "Bichat 1880 (J. Phys. 9, 279), as Bleekrode 1884 quotes it",
    temperatureC: 18.5,
    n: 1.344,
    molPerL: 21.619_56,
  },
  { who: "Bleekrode 1884", temperatureC: 15, n: 1.35, molPerL: 21.770_4 },
];

/** {@link SO2_LIQUID_MEASUREMENTS} as checks of SO₂'s gas row. */
export const SO2_LIQUID_CHECKS: ReadonlyArray<LiquidCheck> = SO2_LIQUID_MEASUREMENTS.map(
  (measurement) => sodiumCheck("liquid SO2", SO2_DISPERSION, measurement),
);

/** Liquid SO₂'s factor: the mean of {@link SO2_LIQUID_CHECKS}' molar-refraction ratios, to six figures. */
export const SO2_LIQUID_FACTOR = rounded(
  SO2_LIQUID_CHECKS.reduce((sum, check) => sum + molarRefractionRatio(check), 0) /
    SO2_LIQUID_CHECKS.length,
);

/** CO₂'s molar mass, g mol⁻¹: the WebBook's molecular weight, 44.0095 (decision-p14-t49h-gas-index A.2). */
export const CO2_MOLAR_MASS_G_PER_MOL = 44.0095;

/**
 * Phillips 1920's Lorentz–Lorenz law for CO₂ at 34 °C and 546.1 nm (Proc. R. Soc. Lond. A 97, 225,
 * p. 236; public domain): (n² + 2)ρ ÷ (n² − 1) = 6.581(1 + 0.0172ρ²), ρ in g cm⁻³, measured up to
 * his greatest density, 0.73692 g cm⁻³ (p. 235's table).
 */
export const PHILLIPS_1920 = {
  limitGPerCm3: 6.581,
  coefficientCm6PerG2: 0.0172,
  wavelengthNm: 546.1,
  greatestDensityGPerCm3: 0.736_92,
} as const;

/** Phillips's factor on the gas's molar refraction at a density, g cm⁻³: 1 ÷ (1 + 0.0172ρ²). */
export function phillipsFactor(densityGPerCm3: number): number {
  return 1 / (1 + PHILLIPS_1920.coefficientCm6PerG2 * densityGPerCm3 ** 2);
}

/** A molar density of CO₂, mol/l, in g cm⁻³. */
export function co2DensityGPerCm3(molPerL: number): number {
  return (molPerL * CO2_MOLAR_MASS_G_PER_MOL) / 1_000;
}

/** Liquid CO₂'s factor: Phillips's at {@link CO2_LIQUID_DENSITY}, to six figures. */
export const CO2_LIQUID_FACTOR = rounded(
  phillipsFactor(co2DensityGPerCm3(CO2_LIQUID_DENSITY.molPerL)),
);

/**
 * Bleekrode 1884's liquid CO₂ under its own vapour pressure (p. 353): his tube II, "nearly free from
 * air", at 12.5, 17.5 and 24 °C, the light unstated and read as his sodium light, with the WebBook's
 * densities at those temperatures. ±0.005 is his error estimate (p. 346: "probably it may be
 * expressed by ±√(E² + E₁²) = ±0.0048").
 */
export const BLEEKRODE_CO2: ReadonlyArray<LiquidMeasurement> = [
  { who: "Bleekrode 1884", temperatureC: 12.5, n: 1.199, molPerL: 19.130_33 },
  { who: "Bleekrode 1884", temperatureC: 17.5, n: 1.184, molPerL: 18.145_16 },
  { who: "Bleekrode 1884", temperatureC: 24, n: 1.173, molPerL: 16.474_06 },
];

/** Bleekrode's other two sodium-light values (p. 353's first table): 1.192 at 15.5 °C and 1.186 at 18.5 °C. */
export const BLEEKRODE_CO2_FIRST_TABLE: ReadonlyArray<LiquidMeasurement> = [
  { who: "Bleekrode 1884", temperatureC: 15.5, n: 1.192, molPerL: 18.560_6 },
  { who: "Bleekrode 1884", temperatureC: 18.5, n: 1.186, molPerL: 17.924_24 },
];

/** CO₂'s critical density, mol/l, as the WebBook's page gives Span and Wagner's: 10.6249. */
export const CO2_CRITICAL_MOL_PER_L = 10.6249;

/** The WebBook's fluid page for CO₂ that gives {@link CO2_CRITICAL_MOL_PER_L}, with its SHA-256. */
const CO2_CRITICAL_PAGE = fetchedSource(
  "its fluid page's critical density",
  "https://webbook.nist.gov/cgi/fluid.cgi?Action=Load&ID=C124389&Type=SatP&Digits=7&THigh=216.61&TLow=216.6&TInc=0.01&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=mol%2Fl&HUnit=kJ%2Fmol&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm",
  "445c0564eb90a35bfe3ab272d3c02a46d1a3c15e06c3b92176bbdb93918d2c68",
  FETCHED_T5K,
);

/** The WebBook's species page for CO₂ that gives {@link CO2_MOLAR_MASS_G_PER_MOL}, with its SHA-256. */
const CO2_SPECIES_PAGE = fetchedSource(
  "its species page",
  "https://webbook.nist.gov/cgi/cbook.cgi?ID=C124389&Units=SI",
  "2c9c9ec9dee087961fa10324e5c26397f4d3bfcfb691af9700b7f16cc5a57e48",
  FETCHED_T5K,
);

/** The form's n of liquid CO₂ at a density, mol/l, and wavelength, nm: the gas row at Phillips's factor. */
export function co2FormIndex(molPerL: number, wavelengthNm: number): number {
  return scaledLiquidIndex(
    CO2_DISPERSION,
    wavelengthNm,
    molPerL,
    phillipsFactor(co2DensityGPerCm3(molPerL)),
  );
}

/**
 * The sim's carry of carbon-dioxide-liquid.json's n to another density at constant Lorentz–Lorenz
 * function, less the form's there (decision-p14-t49h-gas-index §2.4): what the carry misses.
 */
export function co2CarryMiss(molPerL: number, wavelengthNm: number): number {
  const fileX = lorentzLorenzFactor(
    scaledLiquidIndex(CO2_DISPERSION, wavelengthNm, CO2_LIQUID_DENSITY.molPerL, CO2_LIQUID_FACTOR),
  );
  return (
    indexOfLorentzLorenz((fileX * molPerL) / CO2_LIQUID_DENSITY.molPerL) -
    co2FormIndex(molPerL, wavelengthNm)
  );
}

/** Gagnon, Gammon, Kiefte and Clouter 1979's liquid CO at 78.3 ± 0.5 K, ±0.2% (Appl. Opt. 18, 1237). */
export const GAGNON_CO: ReadonlyArray<NamedIndexPoint> = [
  { line: "514.5 nm", wavelengthUm: 0.5145, n: 1.2213 },
  { line: "632.8 nm", wavelengthUm: 0.6328, n: 1.2122 },
];

/**
 * How far Gagnon et al.'s two values differ beyond the gas's slope between their wavelengths: the
 * estimate's miss of the 632.8 nm value less its miss of the 514.5 nm value, at the WebBook's
 * 78.3 K density.
 */
export function gagnonSlopeExcess(): number {
  const [blue, red] = GAGNON_CO;
  if (blue === undefined || red === undefined) {
    throw new Error("Gagnon et al.'s two values are missing");
  }
  const miss = (point: NamedIndexPoint): number =>
    scaledLiquidIndex(CO_DISPERSION, point.wavelengthUm * 1_000, CO_LIQUID_78K_DENSITY.molPerL, 1) -
    point.n;
  return miss(red) - miss(blue);
}

/** Gagnon et al.'s two values' combined error in n, their 0.2% each in quadrature. */
export const GAGNON_COMBINED_ERROR = Math.hypot(...GAGNON_CO.map((point) => 0.002 * point.n));

/** Bleekrode 1884's liquid H₂S at the sodium D lines (p. 351): 1.380 at 12.5 °C and 1.384 at 18.5 °C. */
export const BLEEKRODE_H2S: ReadonlyArray<LiquidMeasurement> = [
  { who: "Bleekrode 1884", temperatureC: 12.5, n: 1.38, molPerL: 23.625_73 },
  { who: "Bleekrode 1884", temperatureC: 18.5, n: 1.384, molPerL: 23.211_97 },
];

/**
 * The method's record (decision-p14-t49h-gas-index §2.1): each measured liquid's unscaled estimate
 * at its WebBook density against its measured n, R08.T5.k's beside {@link LIQUID_CHECKS}, which
 * stay as T5.g left them.
 */
export const T5K_LIQUID_RECORD: ReadonlyArray<LiquidCheck> = [
  {
    what: "liquid Ar at 83.81 K against Sinnock and Smith 1969",
    gas: AR_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(AR_LIQUID_DENSITY.molPerL),
    wavelengthNm: 546.1,
    measured: 1.2334,
  },
  {
    what: "liquid Kr at 115.95 K against Sinnock and Smith 1969",
    gas: KR_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(KR_LIQUID_DENSITY.molPerL),
    wavelengthNm: 546.1,
    measured: 1.3032,
  },
  {
    what: "liquid Xe at 161.35 K (the WebBook's 161.405 K) against Sinnock and Smith 1969",
    gas: XE_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(XE_LIQUID_DENSITY.molPerL),
    wavelengthNm: 546.1,
    measured: 1.3918,
  },
  O2_LIQUID_ANCHOR,
  ...LIVEING_DEWAR_1892_N2O.map((reading) => ({
    what: `liquid N2O at 184.67 K against Liveing and Dewar 1892's ${reading.line} line`,
    gas: N2O_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(N2O_LIQUID_DENSITY.molPerL),
    wavelengthNm: rounded(reading.wavelengthUm * 1_000),
    measured: reading.n,
  })),
  ...SO2_LIQUID_CHECKS,
  ...BLEEKRODE_CO2.map((measurement) => sodiumCheck("liquid CO2", CO2_DISPERSION, measurement)),
  ...GAGNON_CO.map((point) => ({
    what: `liquid CO at 78.3 K against Gagnon et al. 1979 at ${point.line}`,
    gas: CO_DISPERSION,
    molarVolumeCm3PerMol: molarVolumeOf(CO_LIQUID_78K_DENSITY.molPerL),
    wavelengthNm: rounded(point.wavelengthUm * 1_000),
    measured: point.n,
  })),
  ...BLEEKRODE_H2S.map((measurement) => sodiumCheck("liquid H2S", H2S_DISPERSION, measurement)),
];

/** A figure as a percentage with its sign, as the files write it: −1.01%. */
function signedPercent(fraction: number): string {
  return `${signed(100 * fraction, 2)}%`;
}

/** The method's record as a T5.k file's `source` states it. */
function t5kRecordText(): string {
  const row = (check: LiquidCheck): string =>
    `${check.what}, ${signed(liquidCheckExcess(check), 4)} at ${check.wavelengthNm} nm (R_liquid ÷ R_gas − 1 = ${signedPercent(molarRefractionRatio(check) - 1)})`;
  return `The method's record (decision-p14-t49h-gas-index §2.1 and A.2), each measured liquid's unscaled estimate (c = 1, its gas row as view/atmosphere/rayleigh.ts holds it, at the NIST Chemistry WebBook's saturated-liquid density) less the measurement, and the measurement's molar refraction over the gas row's: ${[...T5K_LIQUID_RECORD, ...LIQUID_CHECKS].map(row).join("; ")}`;
}

/** A gas row as a T5.k file's `source` names it, with its state, Z and measured range. */
function gasRowText(
  gas: GasDispersionCopy,
  measuredNm: readonly [number, number],
  row: string,
): string {
  return `${row} (${gas.source}), stated at ${gas.referenceK} K and ${gas.referencePa} Pa with Z = ${Number(gas.compressibility.toPrecision(6))}, measured over ${measuredNm[0]}–${measuredNm[1]} nm and extrapolated beyond`;
}

/** A WebBook density as a T5.k file's `source` states it. */
function densityText(density: WebBookDensity): string {
  return `${density.molPerL.toPrecision(7)} mol/l at ${density.temperatureK} K, the NIST Chemistry WebBook's saturated liquid (SRD 69, after ${density.equationOfState}; ${density.source})`;
}

/** Seidel, Lanou and Yao 2002's specific refractions of the liquefied noble gases (their Table II). */
const SEIDEL_TEXT = `Seidel, Lanou and Yao 2002's specific refractions (arXiv:hep-ex/0111054, Table II, after Sinnock 1980; ${fetchedSource("the arXiv PDF", "https://arxiv.org/pdf/hep-ex/0111054", "851f31ed333ec5dc669da52154ae294c42ae6225e0c02b9c31c4cf7a85a45574", FETCHED_T5K)}) put the liquid's against the dilute gas's at 0.1045 and 0.1050 cm³ g⁻¹ for Ar, 0.0771 and 0.0765 for Kr and 0.0801 and 0.0788 for Xe: the heavy atoms' liquids run above the gas, as the record's Kr and Xe rows do`;

/** The licence of a T5.k file whose values are cited measurements and a reduction made here. */
const T5K_DERIVED_LICENCE =
  "measured values cited as facts, the gas dispersion view/atmosphere/rayleigh.ts cites and the NIST Chemistry WebBook's densities (NIST Standard Reference Data), with a factor made here; no data set is copied";

/** Where a T5.k file's measured range of its gas row is stated: `rayleigh.ts`'s `measuredNm`. */
const RAYLEIGH_ROW = "view/atmosphere/rayleigh.ts's GAS_DISPERSION row";

/** A scaled file's n on the grid, to six figures. */
function scaledOnGrid(
  gas: GasDispersionCopy,
  density: WebBookDensity,
  factor: number,
): ReadonlyArray<number> {
  return MATERIAL_GRID_NM.map((nm) => rounded(scaledLiquidIndex(gas, nm, density.molPerL, factor)));
}

/** A scaled file's n at 380, 550 and 780 nm, as its text states it. */
function scaledAtThree(gas: GasDispersionCopy, density: WebBookDensity, factor: number): string {
  return [380, 550, 780]
    .map((nm) => `${rounded(scaledLiquidIndex(gas, nm, density.molPerL, factor))} at ${nm} nm`)
    .join(", ");
}

/** The reduction of a file whose n is a scaled gas, with its band and k. */
function scaledReductionText(factor: number, band: string, k: string): string {
  return `n = √((1 + 2x) ÷ (1 − x)) with x = c R(λ) ρ_m, c = ${factor}, evaluated at each wavelength of 380–780 nm every 5 nm, to six significant figures (decision-p14-t49h-gas-index §1.2); the stated band: ${band}. ${k}`;
}

/** The O₂–O₂ file's citation and checksum, as oxygen-liquid.json's `source` states it. */
const OXYGEN_PAIR_TEXT = `${OXYGEN_PAIR_SOURCE.citation}, ${OXYGEN_PAIR_SOURCE.url}, SHA-256 ${OXYGEN_PAIR_SOURCE.sha256}, fetched ${FETCHED_T5K} (the same file as fetched ${OXYGEN_PAIR_SOURCE.fetched} for the client's O2-O2 absorber)`;

/** oxygen-liquid.json's k, as its `reduction` derives it. */
function liquidOxygenKText(): string {
  const n = O2_LIQUID_NUMBER_DENSITY_PER_CM3;
  const at = OXYGEN_PAIR_CROSS_SECTIONS_CM5.map(([nm, sigma]) => {
    const perM = oxygenPairAbsorptionPerM(sigma);
    return `σ ${timesTen(sigma, 5)} cm⁵ at ${nm} nm, a = ${perM.toFixed(1)} m⁻¹ and k = ${timesTen(rounded(oxygenPairK(sigma, nm)), 5)}`;
  }).join("; ");
  return `k = aλ ÷ 4π with a(λ) = σ(λ) N² (decision-p14-t49h-gas-index §2.6, path 2, Martonchik and Orton's method for liquid methane's unmeasured range, the gas scaled to the liquid's density), σ the O2–O2 collision-induced absorption of Thalman and Volkamer 2013 (Phys. Chem. Chem. Phys. 15, 15371, DOI 10.1039/c3cp50968k) in HITRAN2024's file (its sets of reference ${THALMAN_VOLKAMER_REFERENCE}), at their coldest temperature, ${OXYGEN_PAIR_COLDEST_K} K, where the liquid is at ${O2_LIQUID_DENSITY.temperatureK} K, linear in wavenumber between the sets' points 1 cm⁻¹ apart, and N = ρ_m N_A = ${timesTen(n, 5)} cm⁻³ the liquid's number density, so N² = ${timesTen(n * n, 4)} cm⁻⁶ and a pair's absorption scales with the square of the density: ${at}, to six significant figures. The sets reach 336–${OXYGEN_PAIR_LONGEST_NM.toFixed(1)} nm, so k = 0 from 655 nm (the B band near 688 nm and the A band near 762 nm are other measurements' and are not drawn), and k = 0 at ${OXYGEN_PAIR_ZERO_NM.join(", ")} nm, where the set's σ lies at or below 0 between the bands, within ${timesTen(OXYGEN_PAIR_SCATTER_CM5, 0)} cm⁵ of it. The bands' σ grows on cooling (at 577 nm, ${OXYGEN_PAIR_COOLING.map(([kelvin, sigma]) => `${timesTen(sigma, 2)} cm⁵ at ${kelvin} K`).join(" and ")} in the same sets), so at 90 K the liquid's bands are presumably stronger; the 203 K values are not extrapolated. Liveing and Dewar 1892 found the bands "all of exceptional strength" through about 3 inches of the liquid (p. 206), and Liveing and Dewar 1895 "very much more strongly developed by 0.4 centim. of liquid oxygen than by five times that thickness of liquid air" (p. 271), as the file's k has them: at 575 nm it transmits ${(100 * Math.exp(-oxygenPairPerM(575) * 0.0762)).toFixed(1)}% through 3 inches and ${(100 * Math.exp(-oxygenPairPerM(575) * 0.004)).toFixed(0)}% through 0.4 cm (computed). Liquid O2 is blue (Liveing and Dewar 1892, p. 207)`;
}

/** The record of liquid O₂'s checks, as oxygen-liquid.json's `source` states them. */
function oxygenChecksText(): string {
  const at = (nm: number): number =>
    scaledLiquidIndex(O2_DISPERSION, nm, O2_LIQUID_DENSITY.molPerL, O2_LIQUID_FACTOR);
  const lines = LIVEING_DEWAR_1895_O2.map(
    (p) => `${p.n.toFixed(4)} at ${p.line} (${signed(p.n - at(p.wavelengthUm * 1_000), 4)})`,
  ).join(", ");
  return `Checks, each measurement less the file: Liveing and Dewar 1895's five lines at the boiling point by the method of Terquem and Trannin (Phil. Mag. (5) 40, 268, p. 269; public domain; ${fetchedSource("the archive.org scan of the volume, its text", "https://archive.org/download/londonedinburg5401895lon/londonedinburg5401895lon_djvu.txt", "14c064d3befa142c445a7baec235e0d8c99008fd2b39b302d783a918e613e386", FETCHED_T5K)}, and its page image, https://archive.org/download/londonedinburg5401895lon/page/n282_w1600.jpg, SHA-256 fac216dbc970c340d48d7da2e06dcc707f005b2aad87ec26014b7c7b9f7f3ff8), ${lines}, the authors trusting their cadmium lines over the thallium and sodium; Liveing and Dewar 1892's prism value at the D line, ${LIVEING_DEWAR_1892_O2_D.n} (Phil. Mag. (5) 34, 205, p. 208, ${signed(LIVEING_DEWAR_1892_O2_D.n - at(LIVEING_DEWAR_1892_O2_D.wavelengthNm), 4)}); and Johns and Wilhelm's own liquid N2, ${JOHNS_WILHELM_N2_546.toFixed(4)} at 546.1 nm, ${signed(JOHNS_WILHELM_N2_546 - 1.199_17, 5)} from Augustin 1915's 1.19917 (nitrogen-liquid.json's)`;
}

/** The T5.k files made from fetched liquid tables and O₂–O₂'s file, by their own reductions. */
const LIQUID_TABLE_MATERIALS: ReadonlyArray<CombinedSpec> = [
  {
    file: "argon-liquid.json",
    inputs: [
      {
        option: "argon-liquid",
        sha256: "6e3e9986914b0a3eade6cf787647e311b54225e200ebc61b81e414f52e94361b",
      },
    ],
    reduce: (texts) => {
      checkedRows(textAt(texts, 0), AR_LIQUID_ROWS);
      return {
        n: scaledOnGrid(AR_DISPERSION, AR_LIQUID_DENSITY, AR_LIQUID_FACTOR),
        k: MATERIAL_GRID_NM.map(() => 0),
      };
    },
    header: {
      key: "Ar",
      phase: "liquid",
      variant: null,
      name: "liquid Ar at 84 K",
      temperatureK: 83.81,
      shape: "sphere",
      provenance: "measured",
      paper: `A. C. Sinnock and B. L. Smith, "Refractive indices of the condensed inert gases", Phys. Rev. 181 (1969) 1297–1307, DOI 10.1103/PhysRev.181.1297, for the table; ${AR_DISPERSION.source}, for the dispersion's shape`,
      source: `${fetchedSource(
        "refractiveindex.info, main/Ar/nk/Sinnock-liquid-83.81K.yml, their table for the liquid at its melting point, 83.81 K: nine rows over 361.2–643.9 nm",
        `${RI_URL}/main/Ar/nk/Sinnock-liquid-83.81K.yml`,
        "6e3e9986914b0a3eade6cf787647e311b54225e200ebc61b81e414f52e94361b",
        FETCHED_T5K,
      )}. The grid's n is the gas's shape scaled to the table (decision-p14-t49h-gas-index §2.5 and A.2): ${gasRowText(AR_DISPERSION, [467.9, 2_058.6], `${RAYLEIGH_ROW} for Ar`)}, its molar refraction scaled by c = ${AR_LIQUID_FACTOR}, fitted by least squares in n to the nine rows at ${densityText(AR_LIQUID_DENSITY)}; the rows less the file's curve: ${AR_LIQUID_ROWS.map((row) => `${signedScientific(row.n - scaledLiquidIndex(AR_DISPERSION, row.wavelengthUm * 1_000, AR_LIQUID_DENSITY.molPerL, AR_LIQUID_FACTOR))} at ${(row.wavelengthUm * 1_000).toFixed(1)} nm`).join(", ")}. ${SEIDEL_TEXT}. ${t5kRecordText()}`,
      licence: `${RI_LICENCE}; the gas dispersion and the NIST Chemistry WebBook's density (NIST Standard Reference Data) cited, with a factor made here`,
      reduction: `${scaledReductionText(AR_LIQUID_FACTOR, "±0.0003 in n over the table's rows, and +0.0002 more over 644–780 nm, past its last row, where the curve is the gas's", "k = 0, stated: the source gives n only, and the liquid is colourless.")} c is the least-squares fit in n to the table's nine rows at the WebBook's ${AR_LIQUID_DENSITY.molPerL.toPrecision(7)} mol/l, the worst row ${timesTen(arWorstRow().miss, 2)} at ${arWorstRow().wavelengthNm.toFixed(1)} nm (decision-p14-t49h-gas-index A.2); the rows at 361.2 and 365.0 nm set c but lie below the grid. 83.81 K is the table's melting point, above the registry row's triple point, 83.8058 K`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "krypton-liquid.json",
    inputs: [
      {
        option: "krypton-liquid",
        sha256: "8b768f8878e36c74113db3b356871fbb11e0a417ada7b3a37315d2ee46f99644",
      },
    ],
    reduce: (texts) => tableWithRedTail(checkedRows(textAt(texts, 0), KR_LIQUID_ROWS)),
    header: {
      key: "Kr",
      phase: "liquid",
      variant: null,
      name: "liquid Kr at 116 K",
      temperatureK: 115.77,
      shape: "sphere",
      provenance: "measured",
      paper:
        'A. C. Sinnock and B. L. Smith, "Refractive indices of the condensed inert gases", Phys. Rev. 181 (1969) 1297–1307, DOI 10.1103/PhysRev.181.1297',
      source: `${fetchedSource(
        "refractiveindex.info, main/Kr/nk/Sinnock-liquid-115.95K.yml, their table for the liquid at its melting point, 115.95 K: eight rows over 363.1–643.9 nm",
        `${RI_URL}/main/Kr/nk/Sinnock-liquid-115.95K.yml`,
        "8b768f8878e36c74113db3b356871fbb11e0a417ada7b3a37315d2ee46f99644",
        FETCHED_T5K,
      )}. The rows themselves are the file (decision-p14-t49h-gas-index §2.5): ${gasRowText(KR_DISPERSION, [400, 1_000], `${RAYLEIGH_ROW} for Kr`)}, scaled by least squares to the eight rows at ${densityText(KR_LIQUID_DENSITY)}, misses them by up to ${timesTen(scaledShapeMiss(KR_DISPERSION, KR_LIQUID_DENSITY.molPerL, KR_LIQUID_ROWS), 1)}, the liquid being more dispersive than the gas. Sinnock and Smith state the table as the melting point's, 115.95 K, 0.18 K above the registry row's triple point, 115.77 K, which the file takes, so that the sim's carry starts at the row's state; the difference is recorded, not applied. Sinnock 1980 (J. Phys. C 13, 2375, DOI 10.1088/0022-3719/13/12/018; its abstract, ${fetchedSource("through OpenAlex", "https://api.openalex.org/works/doi:10.1088/0022-3719/13/12/018", "ae4a1333216d5a49a141bf8784598d71a5bc88c053e8133eb6b5249fe69fa817", FETCHED_T5K)}) measured the condensed noble gases again by total internal reflection: "values for krypton obtained by the two methods differ by more than the combined experimental error". ${SEIDEL_TEXT}. ${t5kRecordText()}`,
      licence: RI_LICENCE,
      reduction: `n linear in wavelength between the source's rows over 380–640 nm, to six significant figures (its 363.1 and 406.3 nm rows bracket 380 nm), and from 645 nm the red tail n = ${KR_LIQUID_TAIL.a} + ${KR_LIQUID_TAIL.bUm2} µm² ÷ λ², a two-term Cauchy line fitted by least squares to the six rows from 435.8 nm (decision-p14-t49h-gas-index A.3), which meets them within ${timesTen(redTailWorst(KR_LIQUID_ROWS, KR_LIQUID_TAIL), 1)} and steps by ${signedScientific(redTailStep(KR_LIQUID_ROWS, KR_LIQUID_TAIL))} from the 643.9 nm row; ${rounded(redTailIndex(KR_LIQUID_TAIL, 780))} at 780 nm. k = 0, stated: the source gives n only, and the liquid is colourless. The stated band: ±0.0005 in n`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "xenon-liquid.json",
    inputs: [
      {
        option: "xenon-liquid",
        sha256: "e5f4b87d73eacc5fc9f0a0e9cedca1a9f52e5b4110fc7031847d1fcd67bb8e40",
      },
    ],
    reduce: (texts) => tableWithRedTail(checkedRows(textAt(texts, 0), XE_LIQUID_ROWS)),
    header: {
      key: "Xe",
      phase: "liquid",
      variant: null,
      name: "liquid Xe at 161 K",
      temperatureK: 161.4,
      shape: "sphere",
      provenance: "measured",
      paper:
        'A. C. Sinnock and B. L. Smith, "Refractive indices of the condensed inert gases", Phys. Rev. 181 (1969) 1297–1307, DOI 10.1103/PhysRev.181.1297',
      source: `${fetchedSource(
        "refractiveindex.info, main/Xe/nk/Sinnock-liquid-161.35K.yml, their table for the liquid at its melting point, 161.35 K: ten rows over 361.2–643.9 nm",
        `${RI_URL}/main/Xe/nk/Sinnock-liquid-161.35K.yml`,
        "e5f4b87d73eacc5fc9f0a0e9cedca1a9f52e5b4110fc7031847d1fcd67bb8e40",
        FETCHED_T5K,
      )}. The rows themselves are the file (decision-p14-t49h-gas-index §2.5): ${gasRowText(XE_DISPERSION, [400, 1_000], `${RAYLEIGH_ROW} for Xe`)}, scaled by least squares to the ten rows at ${densityText(XE_LIQUID_DENSITY)}, which stands for 161.35 K (the WebBook's saturation table starts at 161.405 K), misses them by up to ${timesTen(scaledShapeMiss(XE_DISPERSION, XE_LIQUID_DENSITY.molPerL, XE_LIQUID_ROWS), 1)}, the liquid being more dispersive than the gas. Sinnock and Smith state the table as the melting point's, 161.35 K, 0.05 K below the registry row's triple point, 161.4 K, which the file takes, so that its temperature is not below the row's; the difference is recorded, not applied. Grace et al. 2017's Sellmeier fits to Sinnock and Smith's liquid (arXiv:1502.04213v4, Table 2; ${fetchedSource("the arXiv PDF", "https://arxiv.org/pdf/1502.04213v4", "29094376730461bf507366981343a00bf84157ba969748f8a8021590bc462309", FETCHED_T5K)}) carry a₀ = 1.5 ± 0.02 at 162.35 K and 1.4 ± 0.2 at 178 K, too coarse to use, and are not. ${SEIDEL_TEXT}. ${t5kRecordText()}`,
      licence: RI_LICENCE,
      reduction: `n linear in wavelength between the source's rows over 380–640 nm, to six significant figures (its 365.0 and 406.3 nm rows bracket 380 nm), and from 645 nm the red tail n = ${XE_LIQUID_TAIL.a} + ${XE_LIQUID_TAIL.bUm2} µm² ÷ λ², a two-term Cauchy line fitted by least squares to the seven rows from 435.8 nm (decision-p14-t49h-gas-index A.3), which meets them within ${timesTen(redTailWorst(XE_LIQUID_ROWS, XE_LIQUID_TAIL), 1)} and steps by ${signedScientific(redTailStep(XE_LIQUID_ROWS, XE_LIQUID_TAIL))} from the 643.9 nm row; ${rounded(redTailIndex(XE_LIQUID_TAIL, 780))} at 780 nm. The rows below 435.8 nm lie towards xenon's resonance near 147 nm, which a two-term line cannot follow, and are not fitted. k = 0, stated: the source gives n only, and the liquid is colourless. The stated band: ±0.0005 in n`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "oxygen-liquid.json",
    inputs: [{ option: "o2o2", sha256: OXYGEN_PAIR_SOURCE.sha256 }],
    reduce: (texts) => ({
      n: scaledOnGrid(O2_DISPERSION, O2_LIQUID_DENSITY, O2_LIQUID_FACTOR),
      k: liquidOxygenK(textAt(texts, 0)),
    }),
    header: {
      key: "O2",
      phase: "liquid",
      variant: null,
      name: "liquid O2 at 90 K",
      temperatureK: 90.19,
      shape: "sphere",
      provenance: "derived",
      paper: `H. E. Johns and J. O. Wilhelm, "The refractive indices of liquid oxygen, nitrogen, and hydrogen", Can. J. Res. 15a (1937) 101–108, DOI 10.1139/cjr37a-013, for n at 546.1 nm; ${O2_DISPERSION.source}, for the dispersion; R. Thalman and R. Volkamer, Phys. Chem. Chem. Phys. 15 (2013) 15371, DOI 10.1039/c3cp50968k, as HITRAN2024 gives their O2–O2 cross-sections, for k`,
      source: `n from the gas's dispersion scaled to one measurement (decision-p14-t49h-gas-index §2.5): ${gasRowText(O2_DISPERSION, [400, 1_800], `${RAYLEIGH_ROW} for O2`)}, at ${densityText(O2_LIQUID_DENSITY)}, its molar refraction scaled by c = (n² − 1) ÷ (n² + 2) × V_m ÷ R(546.1 nm) = ${O2_LIQUID_FACTOR} (${signedPercent(O2_LIQUID_FACTOR - 1)}), so that n(546.1 nm) = ${O2_LIQUID_ANCHOR.measured}, Johns and Wilhelm's at the normal boiling point by a Wollaston cell, from their abstract (${fetchedSource("through Crossref", "https://api.crossref.org/works/10.1139/cjr37a-013", "838e11e8c42914a1ebac42e7e6bb433552f53e52ee72ef1c4c1fbc0e4d3d9432", FETCHED_T5K)}; the article is closed): the residual there is ${timesTen(Math.abs(scaledLiquidIndex(O2_DISPERSION, 546.1, O2_LIQUID_DENSITY.molPerL, O2_LIQUID_FACTOR) - O2_LIQUID_ANCHOR.measured), 1)}, the factor's sixth figure. n is ${scaledAtThree(O2_DISPERSION, O2_LIQUID_DENSITY, O2_LIQUID_FACTOR)}. ${oxygenChecksText()}. k from ${OXYGEN_PAIR_TEXT}. No measured visible absorption of the liquid could be read: Dianov-Klokov's spectra of condensed oxygen (Opt. Spectrosc. 1959–1966) have no open copy, and Ellis and Kneser 1933 (Phys. Rev. 44, 420, DOI 10.1103/PhysRev.44.420) is closed. ${t5kRecordText()}`,
      licence: `one published index cited as a fact and a factor made here, with the gas dispersion and density (NIST Standard Reference Data) cited, and k reduced from HITRAN2024's O2–O2 file: ${OXYGEN_PAIR_SOURCE.terms}`,
      reduction: scaledReductionText(
        O2_LIQUID_FACTOR,
        "±0.0015 in n, the anchor's own uncertainty plus 0.001 (decision-p14-t49h-gas-index §2.4)",
        `${liquidOxygenKText()}.`,
      ),
      standIn: null,
      fit: {
        form: "scaledMolarRefraction",
        molarDensityMolPerL: O2_LIQUID_DENSITY.molPerL,
        factor: O2_LIQUID_FACTOR,
        anchoredFromUm: 0.5461,
        anchoredToUm: 0.5461,
      },
    },
  },
];

/**
 * Every material file made from fetched sources by its own reduction, after
 * {@link FETCHED_MATERIALS}: R08.T5.d's to T5.h's, then R08.T5.k's liquid tables and liquid O₂.
 */
export const COMBINED_MATERIALS: ReadonlyArray<CombinedSpec> = [
  ...COMBINED_MATERIALS_T5D_T5H,
  ...LIQUID_TABLE_MATERIALS,
];

/** The text of a liquid whose measurements a T5.k file records against its estimate. */
function againstEstimateText(
  gas: GasDispersionCopy,
  values: ReadonlyArray<LiquidMeasurement>,
  wavelengthNm: number,
): string {
  return values
    .map(
      (value) =>
        `${value.n.toFixed(3)} at ${value.temperatureC} °C (${value.molPerL.toPrecision(7)} mol/l), the estimate less it ${signed(scaledLiquidIndex(gas, wavelengthNm, value.molPerL, 1) - value.n, 4)}`,
    )
    .join("; ");
}

/** The T5.k liquids the tool derives from no fetched table, after T5.g's (decision-p14-t49h-gas-index). */
const T5K_DERIVED_LIQUIDS: ReadonlyArray<MaterialFile> = [
  {
    key: "N2O",
    phase: "liquid",
    variant: null,
    name: "liquid N2O at 185 K",
    temperatureK: 184.67,
    shape: "sphere",
    provenance: "derived",
    paper: `G. D. Liveing and J. Dewar, "On the spectrum of liquid oxygen, and on the refractive indices of liquid oxygen, nitrous oxide, and ethylene", Phil. Mag. (5) 34 (1892) 205–209, DOI 10.1080/14786449208620308, for n at five lines; ${N2O_DISPERSION.source}, for the dispersion`,
    source: `n from the gas's dispersion scaled to Liveing and Dewar's liquid at its boiling point (decision-p14-t49h-gas-index §2.5): ${gasRowText(N2O_DISPERSION, [307, 725], `${RAYLEIGH_ROW} for N2O`)}, at ${densityText(N2O_LIQUID_DENSITY)}, its molar refraction scaled by c = ${N2O_LIQUID_FACTOR} (${signedPercent(N2O_LIQUID_FACTOR - 1)}), fitted by least squares in n to their indices at the C, D, F and G lines and indium's 4509.6 Å, ${LIVEING_DEWAR_1892_N2O.map((r) => `${r.n} (${r.line})`).join(", ")}, from minimum deviations of ${LIVEING_DEWAR_1892_N2O.map((r) => `${Math.floor(r.deviationArcmin / 60)}° ${r.deviationArcmin % 60}′`).join(", ")} (p. 209; public domain; ${fetchedSource("the archive.org scan of the volume, its text", "https://archive.org/download/londonedinburgh5341892lon/londonedinburgh5341892lon_djvu.txt", "ee6e15a840fa2b9383fc70fdb200c3ea80a909604092e7622052be3a11b285f4", FETCHED_T5K)}, and its page image, https://archive.org/download/londonedinburgh5341892lon/page/n222_w600.jpg, SHA-256 742aeaf02092f31bf6877ae6aa00038b0d5d633c73517364ae4c58149459cbfc), the lines read at 656.3, 589.3, 486.1, 430.8 and 450.96 nm; the residuals, measurement less the file, ${LIVEING_DEWAR_1892_N2O.map((r) => `${signed(r.n - scaledLiquidIndex(N2O_DISPERSION, r.wavelengthUm * 1_000, N2O_LIQUID_DENSITY.molPerL, N2O_LIQUID_FACTOR), 4)} (${r.line})`).join(", ")}. The lithium line, 6705.5 Å, is left out: its printed ${LIVEING_DEWAR_1892_N2O_LITHIUM.n} contradicts its own deviation, ${Math.floor(LIVEING_DEWAR_1892_N2O_LITHIUM.deviationArcmin / 60)}° ${LIVEING_DEWAR_1892_N2O_LITHIUM.deviationArcmin % 60}′, 1′ less than the C line's, which gives ${prismIndex(LIVEING_DEWAR_PRISM_ARCMIN, LIVEING_DEWAR_1892_N2O_LITHIUM.deviationArcmin).toFixed(4)} through a prism of 59° 15′ (their liquid-oxygen prism, p. 208, the angle the five other lines' printed pairs imply to 0.015°), where the file gives ${scaledLiquidIndex(N2O_DISPERSION, 670.55, N2O_LIQUID_DENSITY.molPerL, N2O_LIQUID_FACTOR).toFixed(4)}. Their own density, 1.255 g cm⁻³ at −90 °C, is not used. Bleekrode 1884's 1.193 at 16 °C and 1.194 at 15 °C at the sodium lines (Proc. R. Soc. Lond. 37, 339, p. 351), in a cell filled at about 60 atmospheres, of unknown density, are recorded, not used. n is ${scaledAtThree(N2O_DISPERSION, N2O_LIQUID_DENSITY, N2O_LIQUID_FACTOR)}. ${t5kRecordText()}`,
    licence: T5K_DERIVED_LICENCE,
    reduction: scaledReductionText(
      N2O_LIQUID_FACTOR,
      "±0.003 in n",
      "k = 0: the liquid is colourless in the visible. The lines' air wavelengths are read as vacuum wavelengths (under 5 × 10⁻⁶ in n)",
    ),
    standIn: null,
    fit: {
      form: "scaledMolarRefraction",
      molarDensityMolPerL: N2O_LIQUID_DENSITY.molPerL,
      factor: N2O_LIQUID_FACTOR,
      anchoredFromUm: 0.4308,
      anchoredToUm: 0.6563,
    },
    wavelengthsNm: MATERIAL_GRID_NM,
    n: scaledOnGrid(N2O_DISPERSION, N2O_LIQUID_DENSITY, N2O_LIQUID_FACTOR),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  {
    key: "SO2",
    phase: "liquid",
    variant: null,
    name: "liquid SO2 at 198 K",
    temperatureK: 197.7,
    shape: "sphere",
    provenance: "derived",
    paper: `L. Bleekrode, "On the experimental determination of the index of refraction of liquefied gases", Proc. R. Soc. Lond. 37 (1884) 339–362, DOI 10.1098/rspl.1884.0039, for n at the sodium lines, his own and Ketteler's and Bichat's as he quotes them; ${SO2_DISPERSION.source}, for the dispersion`,
    source: `n from the gas's dispersion scaled to the liquid's measurements (decision-p14-t49h-gas-index §2.5): ${gasRowText(SO2_DISPERSION, [500, 670], "view/atmosphere/rayleigh.ts's ESTIMATED_RAYLEIGH row for SO2, its dispersion")}, at ${densityText(SO2_LIQUID_DENSITY)}, its molar refraction scaled by c = ${SO2_LIQUID_FACTOR} (${signedPercent(SO2_LIQUID_FACTOR - 1)}), the mean of the three measurements' molar refractions over the gas's at 589.3 nm, each at the WebBook's density at its own temperature (${nistSaturation(...SO2_ROOM_QUERY)}, SHA-256 ${SO2_ROOM_SHA256}, fetched ${FETCHED_T5K}): ${SO2_LIQUID_MEASUREMENTS.map((m) => `${m.n.toFixed(4)} at ${m.temperatureC} °C (${m.who}), at ${m.molPerL.toPrecision(7)} mol/l, ${signedPercent(molarRefractionRatio(sodiumCheck("liquid SO2", SO2_DISPERSION, m)) - 1)}`).join("; ")} (Bleekrode, pp. 349–350; public domain; ${fetchedSource("the archive.org copy, its text", "https://archive.org/download/paper-doi-10_1098_rspl_1884_0039/paper-doi-10_1098_rspl_1884_0039_djvu.txt", "2d7fa8df25c59fbaec00207ed98b89ba7b53e2264e163b5b8065a0375377048d", FETCHED_T5K)}). Bleekrode judges his own value about 0.003 high: "if, therefore, a quantity of 0.003 is subtracted from the value obtained in my experiments, there results 1.347, approaching nearly to the amount 1.344", from Ketteler's and Bichat's change of nearly 0.001 a degree; the mean is kept, as ruled. n is ${scaledAtThree(SO2_DISPERSION, SO2_LIQUID_DENSITY, SO2_LIQUID_FACTOR)}. ${t5kRecordText()}`,
    licence: T5K_DERIVED_LICENCE,
    reduction: scaledReductionText(
      SO2_LIQUID_FACTOR,
      "±0.003 in n",
      "k = 0: the liquid is colourless in the visible; its absorption lies in the ultraviolet",
    ),
    standIn: null,
    fit: {
      form: "scaledMolarRefraction",
      molarDensityMolPerL: SO2_LIQUID_DENSITY.molPerL,
      factor: SO2_LIQUID_FACTOR,
      anchoredFromUm: 0.5893,
      anchoredToUm: 0.5893,
    },
    wavelengthsNm: MATERIAL_GRID_NM,
    n: scaledOnGrid(SO2_DISPERSION, SO2_LIQUID_DENSITY, SO2_LIQUID_FACTOR),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  {
    key: "CO2",
    phase: "liquid",
    variant: null,
    name: "liquid CO2 at 217 K",
    temperatureK: 216.6,
    shape: "sphere",
    provenance: "derived",
    paper: `P. Phillips, "The relation between the refractivity and density of carbon dioxide", Proc. R. Soc. Lond. A 97 (1920) 225–240, DOI 10.1098/rspa.1920.0029, for the density law; ${CO2_DISPERSION.source}, for the dispersion`,
    source: `n from the gas's dispersion scaled by Phillips's measured density law (decision-p14-t49h-gas-index §2.5): ${gasRowText(CO2_DISPERSION, [180.7, 1_694.5], `${RAYLEIGH_ROW} for CO2`)}, at ${densityText(CO2_LIQUID_DENSITY)}, ${co2DensityGPerCm3(CO2_LIQUID_DENSITY.molPerL).toFixed(5)} g cm⁻³ at the WebBook's molecular weight, ${CO2_MOLAR_MASS_G_PER_MOL} (${CO2_SPECIES_PAGE}). Phillips measured CO2's Lorentz–Lorenz function at 34 °C and 546.1 nm with a Fabry–Perot etalon up to ${PHILLIPS_1920.greatestDensityGPerCm3} g cm⁻³: (n² + 2)ρ ÷ (n² − 1) = ${PHILLIPS_1920.limitGPerCm3}(1 + ${PHILLIPS_1920.coefficientCm6PerG2}ρ²) (p. 236; public domain; ${fetchedSource("the archive.org copy, its text", "https://archive.org/download/philtrans01698142/01698142_djvu.txt", "f1470244531e81dfcb236cebc31f779990f7ea9b4fc56b533a9b2927e6431529", FETCHED_T5K)}, and its page image, https://archive.org/download/philtrans01698142/page/n11_w1300.jpg, SHA-256 a0faa688f4e36a5a94caecfa514737020d77acc832529fb627cd238211d12b69), so the molar refraction is the gas's times c = 1 ÷ (1 + ${PHILLIPS_1920.coefficientCm6PerG2}ρ²) = ${CO2_LIQUID_FACTOR} here, ${signed(scaledLiquidIndex(CO2_DISPERSION, 546.1, CO2_LIQUID_DENSITY.molPerL, CO2_LIQUID_FACTOR) - scaledLiquidIndex(CO2_DISPERSION, 546.1, CO2_LIQUID_DENSITY.molPerL, 1), 4)} in n at 546.1 nm, the law extrapolated from his ${PHILLIPS_1920.greatestDensityGPerCm3} to ${co2DensityGPerCm3(CO2_LIQUID_DENSITY.molPerL).toFixed(3)} g cm⁻³. His ρ → 0 limit, ${CO2_MOLAR_MASS_G_PER_MOL} ÷ ${PHILLIPS_1920.limitGPerCm3} = ${(CO2_MOLAR_MASS_G_PER_MOL / PHILLIPS_1920.limitGPerCm3).toFixed(3)} cm³ mol⁻¹ at 546.1 nm, lies ${signedPercent(CO2_MOLAR_MASS_G_PER_MOL / PHILLIPS_1920.limitGPerCm3 / gasRowMolarRefractionCm3PerMol(CO2_DISPERSION, 546.1) - 1)} from the gas row's ${gasRowMolarRefractionCm3PerMol(CO2_DISPERSION, 546.1).toFixed(3)}. Bleekrode 1884's saturated liquid at the sodium lines, his air-free tube II (p. 353; public domain; ${fetchedSource("the archive.org copy's page image", "https://archive.org/download/paper-doi-10_1098_rspl_1884_0039/page/n14_w1400.jpg", "1edc75295d81d2a91e85f92747cc042e12cc9f8a691d89943e4fc9d584970786", FETCHED_T5K)}; ±0.005, his error estimate), at the WebBook's densities (${nistSaturation(...CO2_ROOM_QUERY)}, SHA-256 ${CO2_ROOM_SHA256}, fetched ${FETCHED_T5K}): ${BLEEKRODE_CO2.map((value) => `${value.n.toFixed(3)} at ${value.temperatureC} °C, the file's form less it ${signed(co2FormIndex(value.molPerL, SODIUM_D_NM) - value.n, 4)} and the unscaled estimate less it ${signed(scaledLiquidIndex(CO2_DISPERSION, SODIUM_D_NM, value.molPerL, 1) - value.n, 4)}`).join("; ")}; his first table's two sodium values, ${BLEEKRODE_CO2_FIRST_TABLE.map((value) => `${value.n.toFixed(3)} at ${value.temperatureC} °C (the form less it ${signed(co2FormIndex(value.molPerL, SODIUM_D_NM) - value.n, 4)})`).join(" and ")}, are recorded too; his tube I held air, which lowers the index. Near the critical point the form needs no rule of its own: Phillips's law runs smoothly through the near-critical densities, and the errors there are the density's and the scattering's. The sim's carry of this file to another state at a constant Lorentz–Lorenz function misses the form by ${BLEEKRODE_CO2.map((value) => `${signed(co2CarryMiss(value.molPerL, SODIUM_D_NM), 4)} at ${value.temperatureC} °C`).join(", ")} and ${signed(co2CarryMiss(CO2_CRITICAL_MOL_PER_L, SODIUM_D_NM), 4)} at the critical density, ${CO2_CRITICAL_MOL_PER_L} mol/l (Span and Wagner's, as the WebBook's page gives it: ${CO2_CRITICAL_PAGE}), at 589.3 nm: recorded, not corrected (decision-p14-t49h-gas-index §2.4). n is ${scaledAtThree(CO2_DISPERSION, CO2_LIQUID_DENSITY, CO2_LIQUID_FACTOR)}. ${t5kRecordText()}`,
    licence: T5K_DERIVED_LICENCE,
    reduction: scaledReductionText(
      CO2_LIQUID_FACTOR,
      "±0.007 in n",
      "k = 0: the liquid is colourless in the visible. 216.6 K is the registry row's triple point, 216.592 K, rounded up to 0.01 K",
    ),
    standIn: null,
    fit: {
      form: "scaledMolarRefraction",
      molarDensityMolPerL: CO2_LIQUID_DENSITY.molPerL,
      factor: CO2_LIQUID_FACTOR,
      anchoredFromUm: 0.5461,
      anchoredToUm: 0.5461,
    },
    wavelengthsNm: MATERIAL_GRID_NM,
    n: scaledOnGrid(CO2_DISPERSION, CO2_LIQUID_DENSITY, CO2_LIQUID_FACTOR),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  {
    key: "CO",
    phase: "liquid",
    variant: null,
    name: "liquid CO at 68 K (an estimated index)",
    temperatureK: 68.16,
    shape: "sphere",
    provenance: "standIn",
    paper: `the estimate's inputs: ${CO_DISPERSION.source}, for CO's gas dispersion; ${LEMMON_SPAN_2006}, as the NIST Chemistry WebBook gives it, for the liquid's density`,
    source: `a Lorentz–Lorenz estimate, not a measurement (decision-p14-t49h-gas-index §2.5 and A.4): ${gasRowText(CO_DISPERSION, [480, 670.8], `${RAYLEIGH_ROW} for CO`)}, unscaled (c = 1), at ${densityText(CO_LIQUID_DENSITY)}, the registry row's triple point. Gagnon, Gammon, Kiefte and Clouter 1979 (Appl. Opt. 18, 1237, DOI 10.1364/AO.18.001237; its abstract, ${fetchedSource("on the journal's page", "https://opg.optica.org/ao/abstract.cfm?uri=ao-18-8-1237", "7e549bbfface71135d799643abb6abd5431b4f54ed426ebf8090adba6d5c7aab", FETCHED_T5K)}; the article is closed) measured the liquid at 78.3 ± 0.5 K by interferometry, ${GAGNON_CO.map((p) => `${p.n} at ${p.line}`).join(" and ")}, with 0.2% uncertainty: at the WebBook's ${densityText(CO_LIQUID_78K_DENSITY)} the estimate less them is ${GAGNON_CO.map((p) => signed(scaledLiquidIndex(CO_DISPERSION, p.wavelengthUm * 1_000, CO_LIQUID_78K_DENSITY.molPerL, 1) - p.n, 4)).join(" and ")}, and the two differ from each other by ${gagnonSlopeExcess().toFixed(4)} beyond the gas's slope between their wavelengths, about twice their combined error (0.2% each, ${GAGNON_COMBINED_ERROR.toFixed(4)} in quadrature), so neither anchors the file; both are recorded. ${t5kRecordText()}`,
    licence:
      "estimated values from the gas dispersion view/atmosphere/rayleigh.ts cites and the NIST Chemistry WebBook's density (NIST Standard Reference Data), with two measured values cited as facts; no data set is copied",
    reduction:
      "n = √((1 + 2x) ÷ (1 − x)) with x = R(λ) ρ_m, the gas's molar refraction at the liquid's density, unscaled (c = 1), evaluated at each wavelength of 380–780 nm every 5 nm, to six significant figures; k = 0: the liquid is colourless in the visible. The stated band: +0.000 and −0.015 in n, the estimate likely high, as against Gagnon et al.'s values",
    standIn: `a non-absorbing liquid whose real index is a Lorentz–Lorenz estimate from the gas's dispersion at the liquid's density, since its measured values contradict themselves: 1.2213 at 514.5 nm and 1.2122 at 632.8 nm at 78 K, ${GAGNON_CO.map((p) => (scaledLiquidIndex(CO_DISPERSION, p.wavelengthUm * 1_000, CO_LIQUID_78K_DENSITY.molPerL, 1) - p.n).toFixed(3)).join(" and ")} below the estimate there; at ${Math.round(CO_LIQUID_DENSITY.temperatureK)} K the estimate is ${[
      380, 550, 780,
    ]
      .map(
        (nm) =>
          `${scaledLiquidIndex(CO_DISPERSION, nm, CO_LIQUID_DENSITY.molPerL, 1).toFixed(3)} at ${nm} nm`,
      )
      .join(", ")
      .replace(/, (?=[^,]*$)/u, " and ")}, within +0.000 and -0.015, likely high`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: scaledOnGrid(CO_DISPERSION, CO_LIQUID_DENSITY, 1),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  {
    key: "H2S",
    phase: "liquid",
    variant: null,
    name: "liquid H2S at 188 K (an estimated index)",
    temperatureK: 187.7,
    shape: "sphere",
    provenance: "standIn",
    paper: `the estimate's inputs: ${H2S_DISPERSION.source}, for H2S's gas dispersion; ${LEMMON_SPAN_2006}, as the NIST Chemistry WebBook gives it, for the liquid's density`,
    source: `a Lorentz–Lorenz estimate, not a measurement (decision-p14-t49h-gas-index §2.5 and A.4): ${gasRowText(H2S_DISPERSION, [486.1, 656.3], `${RAYLEIGH_ROW} for H2S`)}, unscaled (c = 1), at ${densityText(H2S_LIQUID_DENSITY)}, the registry row's triple point. Bleekrode 1884 measured the liquid at the sodium lines (Proc. R. Soc. Lond. 37, 339, p. 351; public domain; ${fetchedSource("the archive.org copy, its text", "https://archive.org/download/paper-doi-10_1098_rspl_1884_0039/paper-doi-10_1098_rspl_1884_0039_djvu.txt", "2d7fa8df25c59fbaec00207ed98b89ba7b53e2264e163b5b8065a0375377048d", FETCHED_T5K)}), in 1879 with the compression pump and later in a Faraday tube under its own pressure: ${againstEstimateText(H2S_DISPERSION, BLEEKRODE_H2S, SODIUM_D_NM)}, at the WebBook's densities (${nistSaturation(...H2S_ROOM_QUERY)}, SHA-256 ${H2S_ROOM_SHA256}, fetched ${FETCHED_T5K}). They run the wrong way with temperature, the warmer liquid's index the higher, so neither anchors the file; both are recorded. This is the one liquid of the method's record whose estimate runs low. ${t5kRecordText()}`,
    licence:
      "estimated values from the gas dispersion view/atmosphere/rayleigh.ts cites and the NIST Chemistry WebBook's density (NIST Standard Reference Data), with two measured values cited as facts; no data set is copied",
    reduction:
      "n = √((1 + 2x) ÷ (1 − x)) with x = R(λ) ρ_m, the gas's molar refraction at the liquid's density, unscaled (c = 1), evaluated at each wavelength of 380–780 nm every 5 nm, to six significant figures; k = 0: the liquid is colourless in the visible. The stated band: −0.005 and +0.021 in n, the estimate likely low, as against Bleekrode's values",
    standIn: `a non-absorbing liquid whose real index is a Lorentz–Lorenz estimate from the gas's dispersion at the liquid's density, since its measured values contradict themselves: ${BLEEKRODE_H2S.map((value) => `${value.n.toFixed(3)} at ${Math.round(value.temperatureC + 273.15)} K`).join(" and ")}, at 589 nm, ${BLEEKRODE_H2S.map((value) => (value.n - scaledLiquidIndex(H2S_DISPERSION, SODIUM_D_NM, value.molPerL, 1)).toFixed(3)).join(" and ")} above the estimate there; at ${Math.round(H2S_LIQUID_DENSITY.temperatureK)} K the estimate is ${[
      380, 550, 780,
    ]
      .map(
        (nm) =>
          `${scaledLiquidIndex(H2S_DISPERSION, nm, H2S_LIQUID_DENSITY.molPerL, 1).toFixed(3)} at ${nm} nm`,
      )
      .join(", ")
      .replace(/, (?=[^,]*$)/u, " and ")}, within +0.021 and -0.005, likely low`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: scaledOnGrid(H2S_DISPERSION, H2S_LIQUID_DENSITY, 1),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
];

/** The files made from no fetched table. */
export const DERIVED_MATERIALS: ReadonlyArray<MaterialFile> = [
  {
    key: "CO2",
    phase: "solid",
    variant: null,
    name: "CO2 ice deposited near 80 K",
    temperatureK: 80,
    shape: "crystal",
    provenance: "derived",
    paper:
      'S. G. Warren, "Optical constants of carbon dioxide ice", Appl. Opt. 25 (1986) 2650–2674, DOI 10.1364/AO.25.002650, Table I (pp. 2663–2667), for n; G. B. Hansen, "Ultraviolet to near-infrared absorption spectrum of carbon dioxide ice from 0.174 to 1.8 µm", J. Geophys. Res. 110 (2005) E11003, DOI 10.1029/2005JE002531, for k',
    source:
      "n(λ) = 1.3994 + 0.004312 µm² ÷ λ², a two-term Cauchy fit to Warren 1986's Table I over 0.30–1.10 µm (19 rows; worst residual 0.0009, inside Warren's stated ±0.05), made by the science agent from the author's copy, https://atmos.uw.edu/~sgw/PAPERS/1986_CO2ice_mcx.pdf, SHA-256 55873a3bcb4820867112c1243b4f583fa311f995d9a56f3db26f635664a7b485, fetched 2026-10-09 (science-r08-sulphur-co2ice.md); k = αλ ÷ 4π with α = 10⁻² m⁻¹, Hansen 2005's estimated visible absorption, below its detection limit over 0.25–1.0 µm. Warren's Egan–Spagnolo k, about 10³ larger, is the stated upper bound",
    licence:
      "derived values: Optica holds Warren's article under its pre-2017 agreement (reuse by permission only, text and data mining reserved; its copyright page, 2026-10-09), and AGU's terms for Hansen 2005 could not be read; the data are the authors' (decision-r08-licences.md). No table of Warren's is committed",
    reduction:
      "the fit and k evaluated on 380–780 nm every 5 nm, to six significant figures. Warren's index is for CO2 deposited near 80 K; Egan and Spagnolo's 1.35 ± 0.05 at 195 K suggests a lower n when warmer, recorded and not applied. Hansen 2005's k is for ice grown at 150 K (its abstract), so the file's temperature, 80 K, is n's. Its shape class is crystal, so Mie gives its cross-sections only",
    standIn: null,
    fit: CO2_ICE_FIT,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(CO2_ICE_FIT, nm).n)),
    k: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(CO2_ICE_FIT, nm).k)),
  },
  {
    key: "NH4SH",
    phase: "solid",
    variant: null,
    name: "NH4SH ice (no visible optical constants are published)",
    temperatureK: null,
    shape: "crystal",
    provenance: "standIn",
    paper:
      "the estimate's inputs: C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1–26, DOI 10.1098/rsta.1914.0001, for NH3's and HCl's dispersions; C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171–176, DOI 10.1098/rspa.1910.0003, for H2S's; C. D. West, \"The crystal structures of some alkali hydrosulfides and monosulfides\", Z. Kristallogr. 88 (1934) 97–115, DOI 10.1524/zkri.1934.88.1.97, for NH4SH's cell; C. P. Gopalakrishnan, \"The thermo-optic behaviour of calcium fluoride, ammonium chloride and zinc-blende at low temperatures\", J. Indian Inst. Sci. 39 (1957) 8–12, eq. 7, for NH4Cl's dispersion as G. Poinsot and J.-P. Mathieu measured it (Ann. Phys. (Paris) 12e sér., 10 (1955) 481–486, DOI 10.1051/anphys/195510010481); H. E. Swanson and E. Tatge, Standard X-ray Diffraction Powder Patterns, NBS Circular 539, vol. 1 (1953), pp. 59–61, for NH4Cl's cell",
    source: `a Lorentz–Lorenz estimate, not a measurement (science-r08-nonspherical.md §3.1, ruled 2026-10-10): the molar refraction of NH3 plus H2S from their gas dispersions as stated at 0 °C and 760 mm (view/atmosphere/rayleigh.ts's Cuthbertson rows, each at the density its paper reduces to: Z = ${NH3_REFRACTIVITY.compressibility.toFixed(6)} for NH3 and ${H2S_REFRACTIVITY.compressibility.toFixed(6)} for H2S), raised by NH4Cl's ionic increment of ${(NH4SH_IONIC_INCREMENT * 100).toFixed(1)}% (its n_D ${NH4CL_CALIBRATION.indexD.toFixed(4)}, from Gopalakrishnan 1957's fit to Poinsot and Mathieu 1955's dispersion, and ${NH4CL_CALIBRATION.densityGPerCm3.toFixed(4)} g cm⁻³, from the X-ray cell of NBS Circular 539 (Pm3̄m, a = 3.8756 Å at 26 °C, one formula unit), against NH3 and HCl from Cuthbertson 1914 at ${SODIUM_D_NM} nm, ${(NH4CL_IONIC_INCREMENT * 100).toFixed(2)}%; science-r08-gases-2.md §2), over the molar volume of West 1934's cell as the Crystallography Open Database gives it, entry 1010249, https://www.crystallography.net/cod/1010249.cif, SHA-256 e721ed9fed41cab1c267ecadd4de31692340d3bfcefd6d9183312ae3a746bb71, fetched 2026-10-10 (P4/nmm, a = 6.011 Å, c = 4.009 Å, two formula units: ${NH4SH_MOLAR_VOLUME_CM3_PER_MOL.toFixed(2)} cm³ mol⁻¹, ${(NH4SH_MOLAR_MASS_G_PER_MOL / NH4SH_MOLAR_VOLUME_CM3_PER_MOL).toFixed(4)} g cm⁻³). The cell is as published, at room temperature and possibly in kX units, about ±0.005 in n. The gas dispersions are measured over 480–670.8 nm (NH3) and 486.1–656.3 nm (H2S) and extrapolated beyond. No visible optical constants of NH4SH are published: Howett et al. 2007 (JOSA B 24, 126) measured it only over 1,300–12,000 cm⁻¹ (0.83–7.7 µm). The stated band spans an ionic increment from ${(NH4SH_IONIC_INCREMENT_BAND[0] * 100).toFixed(0)} to +${(NH4SH_IONIC_INCREMENT_BAND[1] * 100).toFixed(0)}%. The same estimate for NH3 ice, at the cell of I. Olovsson and D. H. Templeton (Acta Cryst. 12 (1959) 832; COD 2310927; a = 5.138 Å, four molecules), gives ${lorentzLorenzIndex([NH3_REFRACTIVITY], 550, NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL, 0).toFixed(3)} against the 1.436 at 550 nm that Martonchik et al. 1984 measure (Appl. Opt. 23, 541). k = 0, since pure NH4SH is a white solid (PubChem CID 25515, from ICSC); the colours it takes on Jupiter come from radiolysis products (Loeffler and Hudson 2018, Icarus 302, 418). It replaces the earlier stand-in of 1.80, which Sromovsky et al. 2017 (Icarus 291, 232, §4.3) state without a source, after the best-fit effective index of 1.85 that Sato et al. 2013 (Icarus 222, 100) find for Jupiter's clouds, a fit that particle shape and size trade against. 1.80 would need a molar refraction ${(ionicIncrement([NH3_REFRACTIVITY, H2S_REFRACTIVITY], 550, NH4SH_MOLAR_VOLUME_CM3_PER_MOL, 1.8) * 100).toFixed(0)}% above the additive one.`,
    licence:
      "estimated values from cited constants and CC0 crystal data (the Crystallography Open Database's entry 1010249); no data set is copied (decision-r08-licences.md row 5)",
    reduction:
      "n = √((1 + 2x) ÷ (1 − x)), with x the raised molar refraction over the molar volume, evaluated on 380–780 nm every 5 nm to six significant figures; k = 0",
    standIn: `a non-absorbing particle whose real index is a Lorentz–Lorenz estimate, since no visible optical constants of NH4SH are published: ${nh4shIndex(380).toFixed(3)} at 380 nm, ${nh4shIndex(550).toFixed(3)} at 550 nm and ${nh4shIndex(780).toFixed(3)} at 780 nm, within +${nh4shBandSpan().above.toFixed(3)} and -${nh4shBandSpan().below.toFixed(3)}`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(nh4shIndex(nm))),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  // R08.T5.d's stand-ins (decision-r08-t5d rulings 3 and 5).
  {
    key: "S8",
    phase: "solid",
    variant: null,
    name: "orthorhombic S8 (a constant index)",
    temperatureK: null,
    shape: "nonSphericalMineral",
    provenance: "standIn",
    paper:
      "C. Palache, H. Berman and C. Frondel, Dana's System of Mineralogy, 7th edition, vol. I (1944) 140–144, for the principal indices, as J. W. Anthony, R. A. Bideaux, K. W. Bladh and M. C. Nichols, Handbook of Mineralogy, vol. I (Mineral Data Publishing, 1990) 506, gives them",
    source: `the Handbook of Mineralogy's sulphur sheet, https://www.handbookofmineralogy.org/pdfs/sulphur.pdf, SHA-256 a443e6a2a44e77f4bd5c9c6a70bbbda1a019c0c9294bc2c9538c045886fe39a3, fetched ${FETCHED_T5D}: the principal indices n_α = ${S8_PRINCIPAL_INDICES[0]}, n_β = ${S8_PRINCIPAL_INDICES[1]} and n_γ = ${S8_PRINCIPAL_INDICES[2]} at the sodium D lines, for the room-temperature crystal. No open visible dispersion of S8 was found: refractiveindex.info, Kitzmann and Heng 2018's compilation and HITRAN's aerosol list hold none, and Sasson et al. 1985 (Icarus 64, 368, DOI 10.1016/0019-1035(85)90061-2) is closed`,
    licence:
      "three published principal indices, cited as facts; no data set is copied (decision-r08-t5d ruling 5)",
    reduction: `a constant index: the principal indices averaged in the dielectric function for randomly oriented grains, n = √((n_α² + n_β² + n_γ²) ÷ 3) = ${S8_STAND_IN_INDEX.toFixed(4)}, at every wavelength of 380–780 nm, with k = 0. Sulphur's dispersion (its indices rise towards the blue) and its blue absorption edge are not drawn, so an S8 haze draws too white in the blue`,
    standIn: `a constant index: crystalline S8's principal indices at 589 nm averaged for randomly oriented grains, ${S8_STAND_IN_INDEX.toFixed(3)} with no dispersion and k = 0, since no open visible optical constants of S8 were found; its blue absorption edge is not drawn`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map(() => rounded(S8_STAND_IN_INDEX)),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  ...ICE_ESTIMATES.map(iceEstimateFile),
  // R08.T5.g's liquids: N2's and NH3's derived fits, C2H6's derived since R08.T5.h, and sulphur's
  // stand-in.
  {
    key: "N2",
    phase: "liquid",
    variant: null,
    name: "liquid N2 at 77 K",
    temperatureK: 77,
    shape: "sphere",
    provenance: "derived",
    paper:
      'H. Augustin, "Über die Dichte des flüssigen Wasserstoffs, den Brechungsexponenten und die Dispersion des flüssigen Wasserstoffs und des flüssigen Stickstoffs", Ann. Phys. 351 (1915) 419–445, DOI 10.1002/andp.19153510307',
    source: `n(λ) = ${N2_LIQUID_FIT.a} + ${N2_LIQUID_FIT.bUm2} µm² ÷ λ², a two-term Cauchy fit made here to Augustin's absolute indices of chemically prepared, argon-free liquid N2 at −195.83 °C (77.32 K), ${AUGUSTIN_N2.map((p) => p.n).join(", ")} (±5 × 10⁻⁵) at ${AUGUSTIN_N2.map((p) => (p.wavelengthUm * 1000).toFixed(1)).join(", ")} nm (p. 443, the wavelengths p. 438's), read from the article's scan, Zenodo record 1514490 (CC0), https://zenodo.org/api/records/1514490/files/article.pdf/content, SHA-256 b7c6905a0ae90c3c2bd668cc4c5d738271a24e06e1e1e278a6cdfb4ff29e4bf3, fetched ${FETCHED_T5G}. The fit's residuals are ${AUGUSTIN_N2.map((p) => (cauchyIndex(N2_LIQUID_FIT, p.wavelengthUm * 1000).n - p.n).toExponential(1)).join(", ")}: the 656.3 nm value lies above the line that the other three and N2's gas dispersion give. The other measurements agree: Johns and Wilhelm 1937 (Can. J. Res. 15a, 101, DOI 10.1139/cjr37a-013; closed), whose abstract gives 1.1990 at 546.1 nm at the normal boiling point; Liveing and Dewar 1893's 1.2053 at the D line (Phil. Mag. (5) 36, 328) was of nitrogen with about 5% O2. k = 0: no measurement of liquid N2's visible absorption was found, and Liveing and Dewar call it colourless. The Lorentz–Lorenz method's checks on measured liquids (gas dispersions as view/atmosphere/rayleigh.ts holds them, at each liquid's molar density from the NIST Chemistry WebBook): ${liquidChecksText()}`,
    licence:
      "measured values cited as facts, from a public-domain article (Zenodo's copy, CC0), and a fit made here; no data set is copied",
    reduction:
      "the fit evaluated on 380–780 nm every 5 nm, to six significant figures, beyond the data's 435.9–656.3 nm by extrapolation; k = 0. Augustin's wavelengths are the lines' in air, read as vacuum wavelengths (under 5 × 10⁻⁶ in n); 77 K stands for his 77.32 K, a little below the normal boiling point, 77.355 K",
    standIn: null,
    fit: N2_LIQUID_FIT,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(N2_LIQUID_FIT, nm).n)),
    k: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(N2_LIQUID_FIT, nm).k)),
  },
  {
    key: "NH3",
    phase: "liquid",
    variant: null,
    name: "liquid NH3 at 240 K",
    temperatureK: 240,
    shape: "sphere",
    provenance: "derived",
    paper: `L. Y. Wong and A. Anderson, "Refractive-index measurements of some low-temperature liquids", J. Opt. Soc. Am. 62 (1972) 219–222, DOI 10.1364/JOSA.62.000219, for n at 632.8 nm; ${NH3_REFRACTIVITY.source}, DOI 10.1098/rsta.1914.0001, for the dispersion`,
    source: `n(λ) = ${NH3_LIQUID_FIT.a} + ${NH3_LIQUID_FIT.bUm2} µm² ÷ λ², made here: b is the least-squares Cauchy slope of the Lorentz–Lorenz estimate from NH3's gas dispersion (view/atmosphere/rayleigh.ts's Cuthbertson row) at the saturated liquid's molar density at 239.80 K, ${NH3_LIQUID_MOLAR_DENSITY_MOL_PER_L} mol/l (the NIST Chemistry WebBook, after Gao et al., J. Phys. Chem. Ref. Data 52 (2023) 013102; ${NH3_LIQUID_DENSITY_SOURCE}), over 380–780 nm (worst residual ${cauchyResidual(NH3_LIQUID_FIT, ammoniaLiquidLevelled).toExponential(1)} from the estimate shifted to meet the measurement), and a meets Wong and Anderson's measured ${NH3_LIQUID_MEASURED.n.toFixed(3)} at ${NH3_LIQUID_MEASURED.wavelengthNm} nm and 239.80 K (their Table II, read from the journal's abstract page; the article is closed), ${liquidCheckExcess(NH3_LIQUID_CHECK).toFixed(4)} below the estimate there. Their dn/dT, −5.42 × 10⁻⁴ K⁻¹ to 1.384 at the melting point, 195.45 K, is not drawn. Bleekrode 1884 (Proc. R. Soc. Lond. 37, 339, p. 354; public domain) gives 1.325 at the D line and 16.5 °C under the liquid's own vapour pressure. k = 0, which lies under Robertson and Williams 1973's α, a total attenuation (decision-r08-t5g ruling 5): their Table I (J. Opt. Soc. Am. 63, 763, DOI 10.1364/JOSA.63.000763; closed; read from the journal's page, https://opg.optica.org/josa/abstract.cfm?uri=josa-63-6-763, SHA-256 57b67350aaf9eb76b85c3b77ed914480a2c05484fa91c21fc991ff15bd56778a, fetched ${FETCHED_T5H}, which gives no abstract) lists a Lambert absorption coefficient of 0.285 cm⁻¹ at 370 nm, 0.212 at 400, 0.139 at 455, 0.054 at 654, 0.034 at 714, 0.178 at 800 and 0.027 at 847 nm, Napierian per cm, T = exp(−αx) (D. Williams, "Optical properties of particles in planetary atmospheres: laboratory studies", IAU Symp. 65 (1974) 57–65, eq. 1, read from the NASA ADS scan, https://articles.adsabs.harvard.edu/pdf/1974IAUS...65...57W, SHA-256 f8feea4c8f331bc149297fae25c57f0736729cdcb2465b4242b8467178fb7827, fetched ${FETCHED_T5H}), from transmission with the cell windows' reflection and absorption ratioed out between thicknesses (his eq. 5), on a sample whose purity, path and temperature the page does not state. Its rise into the blue is not ammonia's: the intense bands begin below about 240 nm, and the shortest band Jolly lists for the pure liquid is a weak one at 0.802 µm (W. L. Jolly, "Absorption spectroscopy in liquid ammonia", UCRL-2008 (1952), pp. 3 and 14, eScholarship's copy through the Wayback Machine, https://web.archive.org/web/20240909090731id_/https://escholarship.org/content/qt0pz4s65m/qt0pz4s65m.pdf, SHA-256 2162c83a8124444adcb59e76225b5076f9b3d9cae637e612dfd3c537c8e6191f, fetched ${FETCHED_T5H}); the gas's weak N–H overtone bands at 5520 and 6475 Å (P. G. J. Irwin et al., Icarus 321 (2019) 572, its abstract, https://arxiv.org/abs/1812.05383, SHA-256 4d91608541b8d5b84c8a0d9d50b130f9f32ab818a6adc53f1834e138b9b50f5b, fetched ${FETCHED_T5H}) presumably have weak counterparts in the liquid, so the table's 654 nm value may hold one, but no band explains the smooth rise from 714 to 370 nm; and the liquid is colourless (NOAA's CAMEO Chemicals, CHRIS card AMA, https://cameochemicals.noaa.gov/chris/AMA.pdf, SHA-256 c4e2fe742e7f01fecfbe2c29f1a58e196955d785ad9bcf8068232c42c29ae37f, fetched ${FETCHED_T5H}). As k = αλ ÷ 4π, the table bounds k by 6.7 × 10⁻⁷ at 400 nm and 2.8 × 10⁻⁷ at 654 nm, and its 800 nm band lies past this file's 780 nm`,
    licence:
      "one measured index cited as a fact and a fit made here, with the gas dispersion and density cited, and Robertson and Williams's seven absorption coefficients cited as facts; no data set is copied",
    reduction:
      "the fit evaluated on 380–780 nm every 5 nm, to six significant figures; k = 0. 240 K stands for 239.80 K, NH3's boiling point at 0.101 MPa",
    standIn: null,
    fit: NH3_LIQUID_FIT,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(NH3_LIQUID_FIT, nm).n)),
    k: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(NH3_LIQUID_FIT, nm).k)),
  },
  {
    key: "C2H6",
    phase: "liquid",
    variant: null,
    name: "liquid C2H6 at 94 K",
    temperatureK: 94,
    shape: "sphere",
    provenance: "derived",
    paper: `A. Hagermann, J. C. Zarnecki, M. C. Towner, P. D. Rosenberg, R. D. Lorenz, M. R. Leese, B. Hathi and A. J. Ball, "Physical properties as indicators of liquid compositions: Derivation of the composition for Titan's surface liquids from the Huygens SSP measurements", Mon. Not. R. Astron. Soc. 359 (2005) 637–642, DOI 10.1111/j.1365-2966.2005.08935.x, for n at 670 nm, with J. Badoz, M. Le Liboux, R. Nahoum, G. Israël, F. Raulin and J. P. Torre, Rev. Sci. Instrum. 63 (1992) 2967, DOI 10.1063/1.1142594, whose measured indices its Table 2 gives; ${C2H6_DISPERSION.source}, for the dispersion; D. Bücker and W. Wagner, J. Phys. Chem. Ref. Data 35 (2006) 205, DOI 10.1063/1.1859286, for the liquid's density`,
    source: `n(λ) = ${C2H6_LIQUID_FIT.a} + ${C2H6_LIQUID_FIT.bUm2} µm² ÷ λ², made here (R08.T5.h, decision-r08-t5g ruling 3), as liquid NH3's is: b is the least-squares Cauchy slope of the Lorentz–Lorenz estimate from C2H6's gas dispersion as stated at ${C2H6_DISPERSION.referenceK} K and ${C2H6_DISPERSION.referencePa} Pa (view/atmosphere/rayleigh.ts's row, Z = ${C2H6_DISPERSION.compressibility}, measured over 523–667.7 nm and extrapolated beyond, with no increment, since the liquid is molecular and non-polar), at the saturated liquid's molar volume at 94 K, ${molarVolumeOf(C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L).toFixed(3)} cm³ mol⁻¹ (${C2H6_LIQUID_MOLAR_DENSITY_MOL_PER_L} mol/l from the NIST Chemistry WebBook, ${C2H6_LIQUID_DENSITY_SOURCE}), over 380–780 nm (worst residual ${cauchyResidual(C2H6_LIQUID_FIT, ethaneLiquidLevelled).toExponential(1)} from the estimate shifted to meet the measurement); and a meets ${C2H6_LIQUID_MEASURED.n} at ${C2H6_LIQUID_MEASURED.wavelengthNm} nm, Hagermann et al.'s largest index over the pure constituents of their mixture (their Table 3, which states no conditions; taken at their Table 2's 670 nm, 94 K and 1.5 bar), ethane's by their text ("nitrogen has the lowest refractive index and electrical permittivity, and ethane the highest", §4.2), read from the NASA ADS scan, https://articles.adsabs.harvard.edu/pdf/2005MNRAS.359..637H, SHA-256 d7193a2938962040a41a007125177e21d93e7c155728d5a5ea5d77998356ca37, fetched ${FETCHED_T5H} (p. 641). Table 3 marks no source for the value, and its smallest index, 1.1985, lies within 5 × 10⁻⁴ of nitrogen-liquid.json's ${cauchyIndex(N2_LIQUID_FIT, 670).n.toFixed(4)} at 670 nm and 77 K, so its rows need not all be at 94 K; its Table 2's indices are marked as the experimental values of Badoz et al. 1992, whose refractometer is accurate to 10⁻³ and who measured liquid methane at 91–106 K and its mixtures with ethane, with and without nitrogen, at 94 K and 1.5 bar (their abstract, through OpenAlex). ${ethaneChecksText()}. The method's checks on other measured liquids, each above the measurement: ${liquidChecksText()} (their molar densities: ${LIQUID_CHECK_DENSITY_SOURCES}). R08.T5.g's search (${FETCHED_T5G}) found no open measurement; R08.T5.h's found Hagermann et al.'s value. Sliwinski 1969 (Z. Phys. Chem. 63, 263, DOI 10.1524/zpch.1969.63.5_6.263), the Lorentz–Lorenz function of liquid ethane, is closed, as is Lopez-Calvo and Manzanares 2008 (J. Phys. Chem. A 112, 1730, DOI 10.1021/jp076843v), whose abstract reports the C–H overtone Δv = 6 of liquid ethane at 100 K, past 14,000 cm⁻¹, in the red. k = 0: no open measurement of liquid ethane's visible absorption was found, and its weak C–H overtones in the red are not drawn`,
    licence:
      "one published index cited as a fact and a fit made here, with the gas dispersion and density (NIST Standard Reference Data) cited; no data set is copied",
    reduction:
      "the fit evaluated on 380–780 nm every 5 nm, to six significant figures; k = 0. The slope is the Lorentz–Lorenz estimate's, which reaches the liquid's density from the gas's dispersion; only the level is measured",
    standIn: null,
    fit: C2H6_LIQUID_FIT,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(C2H6_LIQUID_FIT, nm).n)),
    k: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(C2H6_LIQUID_FIT, nm).k)),
  },
  {
    key: "S8",
    phase: "liquid",
    variant: null,
    name: "liquid S8 at 387 K (a constant index)",
    temperatureK: 387,
    shape: "sphere",
    provenance: "standIn",
    paper:
      'H. Becquerel, "Recherches expérimentales sur la polarisation rotatoire magnétique", Ann. Chim. Phys. (5) 12 (1877) 5–87, for the index',
    source: `the archive.org scan of Ann. Chim. Phys. (5) 12, https://archive.org/details/s696id13665280 (public domain mark): its text, SHA-256 8020c247086bffe4a9a7a58d0d08ddf4815ed2b19c5f8b1421bfbfc85891a458, and its page image n36 (p. 37), SHA-256 60b9e263de0cd6061d8e1b83d5e8687d5159bbe2c0a0d246bdf2380e86c417b1, fetched ${FETCHED_T5G}: n = ${S8_MOLTEN_INDEX_D.toFixed(4)} with the sodium D lines for "Soufre fondu (114 degrés)" (Tableau n° 2), read in an iron prism as the melt began to crystallise; the hotter, dark-yellow melt read 1.890 a little before (pp. 25–26), a rise of 0.039 on cooling that expansion is too small to explain, as Becquerel finds for the magnetic rotation. No open measurement of molten sulphur's visible absorption, or of its dispersion, was found (R08.T5.g's search): Sasson and Arakawa 1986 (Appl. Opt. 25, 2675; n and k over 0.4–2.0 µm at 121–183 °C), Sasson et al. 1985 (Icarus 64, 368), Hosokawa, Matsuoka and Tamura 1994 (J. Phys.: Condens. Matter 6, 5273; the absorption coefficient over 130–450 °C), Bass 1953 (J. Chem. Phys. 21, 80) and Meyer et al. 1971 (J. Am. Chem. Soc. 93, 1034) are closed, as is Meyer 1976 (Chem. Rev. 76, 367), the finding aid; the open preprint of Meyer et al. 1971 (UCRL-19660, 1970) gives the red absorption edge's shift, about 23.6 cm⁻¹ per kelvin, but not its position, and refractiveindex.info, HITRAN's aerosol list and Kitzmann and Heng 2018's compilation hold no liquid sulphur`,
    licence:
      "one published index cited as a fact, from a public-domain volume; no data set is copied (decision-r08-t5d ruling 5)",
    reduction: `a constant index: Becquerel's n at 589 nm, ${S8_MOLTEN_INDEX_D.toFixed(4)}, at every wavelength of 380–780 nm, with k = 0. Molten sulphur's dispersion and its blue absorption edge, which makes the melt yellow at its melting point and dark red at its boiling point (Meyer et al. 1971), are not drawn, so a sulphur lake draws too white in the blue. 387 K stands for Becquerel's 114 °C`,
    standIn: `a constant index: liquid S8's real index at 589 nm near its melting point, ${S8_MOLTEN_INDEX_D.toFixed(3)} with no dispersion and k = 0, since no open measurement of the liquid's visible absorption was found; its blue absorption edge is not drawn`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map(() => rounded(S8_MOLTEN_INDEX_D)),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
  // R08.T5.k's liquids made from no fetched table (decision-p14-t49h-gas-index).
  ...T5K_DERIVED_LIQUIDS,
];

/**
 * One fetched source reduced to its material file.
 *
 * @throws Error as its parser and {@link resample} do.
 */
export function reduceSource(
  spec: Pick<FetchedSpec, "parse" | "resample" | "header" | "adjust">,
  text: string,
): MaterialFile {
  const grid = spec.resample.grid ?? MATERIAL_GRID_NM;
  const reduced = resample(spec.parse(text), spec.resample);
  const { n, k } = spec.adjust === undefined ? reduced : spec.adjust(reduced, grid);
  return { ...spec.header, wavelengthsNm: grid, n, k };
}

function readChecked(path: string, sha256: string): string {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${sha256}`);
  }
  return bytes.toString("utf8");
}

/**
 * Each derived file's name, by its key and phase (`CO2/solid`): since R08.T5.g a key may have a
 * derived liquid beside a derived solid.
 */
const DERIVED_FILES: Readonly<Record<string, string>> = {
  "CO2/solid": "carbon-dioxide-ice.json",
  "NH4SH/solid": "ammonium-hydrosulphide.json",
  "S8/solid": "sulphur.json",
  "H2S/solid": "hydrogen-sulphide-ice.json",
  "N2/solid": "nitrogen-ice.json",
  "CO/solid": "carbon-monoxide-ice.json",
  "O2/solid": "oxygen-ice.json",
  "N2/liquid": "nitrogen-liquid.json",
  "NH3/liquid": "ammonia-liquid.json",
  "C2H6/liquid": "ethane-liquid.json",
  "S8/liquid": "sulphur-liquid.json",
  "N2O/liquid": "nitrous-oxide-liquid.json",
  "SO2/liquid": "sulphur-dioxide-liquid.json",
  "CO2/liquid": "carbon-dioxide-liquid.json",
  "CO/liquid": "carbon-monoxide-liquid.json",
  "H2S/liquid": "hydrogen-sulphide-liquid.json",
};

/** The command-line options of every fetched source, each once, in the lists' order. */
export function sourceOptions(): ReadonlyArray<SourceInput> {
  const all = [
    ...FETCHED_MATERIALS.map((spec) => ({ option: spec.option, sha256: spec.sha256 })),
    ...COMBINED_MATERIALS.flatMap((spec) => spec.inputs),
  ];
  return all.filter((input, i) => all.findIndex((o) => o.option === input.option) === i);
}

/**
 * The command line: `--out <dir>` and one `--<option> <path>` per fetched source (the options of
 * {@link sourceOptions}). Writes every material file into the directory. Returns the exit code.
 */
export function main(args: readonly string[]): number {
  try {
    const inputs = sourceOptions();
    const options: Record<string, { readonly type: "string" }> = { out: { type: "string" } };
    for (const input of inputs) {
      options[input.option] = { type: "string" };
    }
    const values: Readonly<Record<string, unknown>> = parseArgs({
      args: [...args],
      options,
      strict: true,
    }).values;
    const out = values["out"];
    if (typeof out !== "string") {
      throw new Error(
        `usage: materials --out <dir> ${inputs.map((s) => `--${s.option} <path>`).join(" ")}`,
      );
    }
    const textOf = (input: SourceInput): string => {
      const path = values[input.option];
      if (typeof path !== "string") {
        throw new Error(`missing --${input.option}`);
      }
      return readChecked(path, input.sha256);
    };
    mkdirSync(out, { recursive: true });
    for (const spec of FETCHED_MATERIALS) {
      const file = reduceSource(spec, textOf(spec));
      writeFileSync(join(out, spec.file), `${JSON.stringify(file, null, 2)}\n`);
    }
    for (const spec of COMBINED_MATERIALS) {
      const { n, k } = spec.reduce(spec.inputs.map(textOf));
      const file: MaterialFile = { ...spec.header, wavelengthsNm: MATERIAL_GRID_NM, n, k };
      writeFileSync(join(out, spec.file), `${JSON.stringify(file, null, 2)}\n`);
    }
    for (const file of DERIVED_MATERIALS) {
      const name = DERIVED_FILES[`${file.key}/${file.phase}`];
      if (name === undefined) {
        throw new Error(`no file name for the derived material ${file.key}, ${file.phase}`);
      }
      writeFileSync(join(out, name), `${JSON.stringify(file, null, 2)}\n`);
    }
    return 0;
  } catch (error: unknown) {
    console.error(`materials: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}
